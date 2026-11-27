//! `provider_retry`: el `retry_after` del proveedor como **dato**, no como texto.
//!
//! MEDIDO (2026-11-28): el `retry_after` que da el proveedor **se perdía en
//! el camino**.  `quota_guard` lo parseaba del **texto** del error (para la
//! pausa por cuota, que sí lo usaba), pero el error que viaja por la
//! cascada (`ProviderError`) **no lo llevaba**: el bucle de reintentos usaba
//! SIEMPRE un backoff fijo (`250 ms x 2^intento`) ⇒ el daemon reintentaba
//! **antes de tiempo** y el proveedor le volvía a contestar 429.
//!
//! La cabecera `Retry-After` tampoco se leía: `ureq` con
//! `http_status_as_error(false)` **sí** deja el status y las cabeceras
//! accesibles, pero el código sólo guardaba el status y el cuerpo.
//!
//! Este módulo es la **fuente única** del parseo:
//! * [`parse_retry_after_header`] → cabecera `Retry-After` (delta-seconds).
//! * [`parse_retry_after_ms_header`] → cabecera `retry-after-ms` (Anthropic).
//! * [`parse_retry_after_text`] → el `retry_after` dentro del **cuerpo** del
//!   error (mismas marcas que ya usaba `quota_guard`, más `retry-after-ms`).
//! * [`espera_reintento`] → la espera real, **con tope**.
//!
//! Sin dependencias del crate: es lógica pura y se testea offline.

use std::time::Duration;

/// Tope de la espera que puede imponer el proveedor (segundos).
///
/// MEDIDO: un `Retry-After: 86400` (un día) dejaría el daemon **colgado**
/// reintentando.  120 s es suficiente para que el rate limit expire y
/// suficiente corto para que el agente siga respondiendo.
pub const CAP_RETRY_AFTER_SEGS: u64 = 120;

/// Marcas (en minúsculas) con las que el proveedor escribe el `retry_after`
/// **dentro del cuerpo** del error.
///
/// MEDIDO: son las mismas que ya usaba `quota_guard::parsear_error_cuota`
/// (que las tenía escritas a mano): el orden es el del que se busca, no el
/// de la posición en el texto.
const MARCAS_RETRY_AFTER: [&str; 4] = [
    "retry after ",
    "retry-after: ",
    "retry_after=",
    "try again in ",
];

/// Parsea una cabecera `Retry-After` (delta-seconds).
///
/// Acepta `"120"`, `"120s"` y `" 120 "`.  La forma **HTTP-date**
/// (`"Wed, 21 Oct 2015 07:28:00 GMT"`) **no** se parsea: no hay reloj de
/// pared en el crate y el `retry_after` del cuerpo del error la cubre.
pub fn parse_retry_after_header(valor: &str) -> Option<u64> {
    let v = valor.trim();
    if v.is_empty() {
        return None;
    }
    // "120s" (algunos proveedores) => "120".
    let v = v.strip_suffix('s').unwrap_or(v);
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    v.parse::<u64>().ok()
}

/// Parsea una cabecera `retry-after-ms` (milisegundos) → **segundos**.
///
/// MEDIDO: Anthropic manda `retry-after-ms: 20000`; redondear a segundos es
/// lo que hace el resto del mundo y evita esperas de 0 s por truncamiento.
pub fn parse_retry_after_ms_header(valor: &str) -> Option<u64> {
    let v = valor.trim();
    let v = v.strip_suffix("ms").unwrap_or(v);
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    v.parse::<u64>().ok().map(|ms| ms / 1000)
}

/// Parsea el `retry_after` según el **nombre** de la cabecera.
///
/// Devuelve `None` si la cabecera no es de retry-after o si el valor no es
/// un número: una cabecera rara **no** debe tumbar la llamada.
pub fn retry_after_de_cabecera(nombre: &str, valor: &str) -> Option<u64> {
    match nombre.to_ascii_lowercase().as_str() {
        "retry-after" => parse_retry_after_header(valor),
        "retry-after-ms" => parse_retry_after_ms_header(valor),
        _ => None,
    }
}

/// Parsea el `retry_after` **dentro del texto** del error de un proveedor.
///
/// MEDIDO: es lo que ya hacía `quota_guard` (que lo guardaba en
/// `ErrorCuota::retry_after`), pero aquí es la **fuente única**: si la
/// cabecera no está, el cuerpo es el plan B.
///
/// Mejora sobre el parser anterior: si la **primera** marca que aparece no
/// trae dígitos, se sigue con la siguiente marca en vez de rendirse.
pub fn parse_retry_after_text(texto: &str) -> Option<u64> {
    let t = texto.to_ascii_lowercase();
    // `retry-after-ms` primero: si aparece, sus dígitos son los últimos
    // ("... retry-after-ms: 20000 ...") y `retry after ` puede haber
    //_ctenido_ otra cifra antes.
    if let Some(i) = t.find("retry-after-ms") {
        let resto = &t[i + "retry-after-ms".len()..];
        let digitos = resto.trim_start_matches([':', ' ', '=']);
        let fin = digitos
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(digitos.len());
        if let Ok(ms) = digitos[..fin].parse::<u64>() {
            return Some(ms / 1000);
        }
    }
    for marca in MARCAS_RETRY_AFTER {
        if let Some(i) = t.find(marca) {
            let resto = &t[i + marca.len()..];
            let fin = resto
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(resto.len());
            if let Ok(s) = resto[..fin].trim().parse::<u64>() {
                return Some(s);
            }
            // La marca estaba pero sin cifras: se prueba la siguiente.
        }
    }
    None
}

/// La espera **real** antes de reintentar.
///
/// * `base_ms == 0` ⇒ **no se duerme nunca** (así trabajan los tests: la
///   suite no puede dormir 42 s).
/// * Sin `retry_after` ⇒ el backoff exponencial de siempre
///   (`base_ms x 2^intento`, con el multiplicador topado en 64).
/// * Con `retry_after` ⇒ se espera **al menos** lo que dice el proveedor
///   (topado a [`CAP_RETRY_AFTER_SEGS`]): reintentar antes de su hora
///   guarantees otro 429.
pub fn espera_reintento(base_ms: u64, attempt: u32, retry_after: Option<u64>) -> Duration {
    if base_ms == 0 {
        return Duration::ZERO;
    }
    let mult = 1u64 << attempt.min(6);
    let backoff = base_ms.saturating_mul(mult);
    let pedido = match retry_after {
        Some(seg) => seg
            .saturating_mul(1000)
            .min(CAP_RETRY_AFTER_SEGS.saturating_mul(1000)),
        None => 0,
    };
    Duration::from_millis(backoff.max(pedido))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabecera_delta_seconds() {
        assert_eq!(parse_retry_after_header("120"), Some(120));
        assert_eq!(parse_retry_after_header(" 120 "), Some(120));
        assert_eq!(parse_retry_after_header("120s"), Some(120));
        assert_eq!(parse_retry_after_header("0"), Some(0));
    }

    #[test]
    fn cabecera_vacia_o_rara_no_es_error() {
        assert_eq!(parse_retry_after_header(""), None);
        assert_eq!(parse_retry_after_header("   "), None);
        assert_eq!(parse_retry_after_header("Wed, 21 Oct 2015 07:28:00 GMT"), None);
        assert_eq!(parse_retry_after_header("pronto"), None);
    }

    #[test]
    fn cabecera_en_milisegundos() {
        assert_eq!(parse_retry_after_ms_header("20000"), Some(20));
        assert_eq!(parse_retry_after_ms_header("20000ms"), Some(20));
        assert_eq!(parse_retry_after_ms_header("1500"), Some(1));
        assert_eq!(parse_retry_after_ms_header(""), None);
    }

    #[test]
    fn cabecera_por_nombre() {
        assert_eq!(retry_after_de_cabecera("Retry-After", "30"), Some(30));
        assert_eq!(retry_after_de_cabecera("retry-after", "30s"), Some(30));
        assert_eq!(retry_after_de_cabecera("Retry-After-Ms", "9000"), Some(9));
        assert_eq!(retry_after_de_cabecera("X-Other", "30"), None);
    }

    #[test]
    fn texto_con_la_marca_clasica() {
        // La misma cadena que usa el test de `quota_guard`.
        assert_eq!(
            parse_retry_after_text("429 Too Many Requests, retry after 42 seconds"),
            Some(42)
        );
    }

    #[test]
    fn texto_con_las_cuatro_marcas() {
        assert_eq!(parse_retry_after_text("please retry after 7 now"), Some(7));
        assert_eq!(parse_retry_after_text("retry-after: 11"), Some(11));
        assert_eq!(parse_retry_after_text("retry_after=13"), Some(13));
        assert_eq!(parse_retry_after_text("try again in 17"), Some(17));
    }

    #[test]
    fn texto_sin_marca_no_inventa() {
        assert_eq!(parse_retry_after_text("invalid api key"), None);
        assert_eq!(parse_retry_after_text(""), None);
    }

    #[test]
    fn texto_con_marca_sin_cifras_sigue_a_la_siguiente() {
        // "retry after soon" no trae cifras: se prueba la marca siguiente.
        assert_eq!(
            parse_retry_after_text("retry after soon; try again in 9"),
            Some(9)
        );
    }

    #[test]
    fn texto_con_retry_after_ms_manda() {
        assert_eq!(
            parse_retry_after_text("rate limited, retry-after-ms: 20000"),
            Some(20)
        );
    }

    #[test]
    fn espera_cero_en_tests() {
        // Con `base_ms == 0` la suite NUNCA duerme.
        assert_eq!(espera_reintento(0, 1, Some(42)), Duration::ZERO);
        assert_eq!(espera_reintento(0, 1, None), Duration::ZERO);
    }

    #[test]
    fn espera_backoff_exponencial() {
        assert_eq!(espera_reintento(250, 1, None), Duration::from_millis(500));
        assert_eq!(espera_reintento(250, 2, None), Duration::from_millis(1000));
        assert_eq!(espera_reintento(250, 3, None), Duration::from_millis(2000));
    }

    #[test]
    fn espera_topa_el_multiplicador() {
        // `attempt` enorme no puede multiplicar la espera hasta el infinito.
        assert_eq!(
            espera_reintento(250, 70, None),
            Duration::from_millis(250 * 64)
        );
    }

    #[test]
    fn espera_respeta_al_proveedor() {
        // El proveedor manda: 42 s, aunque el backoff sea de 500 ms.
        assert_eq!(
            espera_reintento(250, 1, Some(42)),
            Duration::from_secs(42)
        );
    }

    #[test]
    fn espera_topa_lo_que_pide_el_proveedor() {
        assert_eq!(
            espera_reintento(250, 1, Some(86_400)),
            Duration::from_secs(CAP_RETRY_AFTER_SEGS)
        );
    }
}

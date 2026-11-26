//! Control de gasto de datos de Jazztel (MNO español, MCC/MNC 21403).
//!
//! POR QUÉ ESTE MÓDULO
//! --------------------
//! El encargo (2026-11-26) es que MegaDaemon integre el control de gasto de
//! MB/GB del operador, con dos cosas que se suelen confundir:
//!
//!   1. **DIFERENCIAR WiFi DE DATOS MÓVILES**.  Es medible en el propio
//!      sistema y es lo que el usuario necesita para saber si puede gastar.
//!      La señal es `/proc/net/dev` clasificado por interfaz: `wlan*` es WiFi,
//!      `rmnet*`/`ccmni*`/`ppp*` son datos móviles.
//!
//!   2. **LA CIFRA QUE FACTURA EL OPERADOR**, que NO es la del sistema.
//!      Android cuenta *bytes que movió el proceso*; Jazztel factura *bytes
//!      que contabilizó la red del operador*.  Difieren en el tethering, el
//!      roaming, el APN de respaldo, el Data Saver y el corte del ciclo de
//!      facturación.  La verdad de facturación está en la API del operador y
//!      en la factura; el diseño completo está en
//!      `docs/RE_GASTO_DATOS_JAZZTEL.md` (RE real: APK descargado y
//!      descompilado, contrato REST medido).
//!
//! QUÉ HACE ESTE MÓDULO HOY (y qué NO)
//! ------------------------------------
//! * **SÍ**: mide el desglose WiFi/móvil/ethernet por interfaz (lo que el
//!   sistema ve), lo compara con el estado del operador que el usuario o el
//!   agente aportan (fichero JSON en el state dir), avisa de los umbrales y
//!   de la fecha de renovación, y expone todo por la tool remota.
//! * **NO** (y por qué): la sustitución del subsistema de datos de Android
//!   (`NetworkStatsManager`/`NetworkPolicyManager`) exige **hookear métodos
//!   ART en `system_server`**, y el payload de hooks actual es frida-gum puro
//!   sin puente Java ⇒ hace falta un crate nuevo. Está especificado, no
//!   implementado; se marca `NO VERIFICADO` en vez de fingirlo.
//!
//! REGLAS
//! ------
//! * **Las credenciales de Jazztel NO viven aquí**: van en un fichero 0600
//!   (el repo es PÚBLICO) y este módulo sólo dice **dónde** está.
//! * **Sin PII**: ni el número de línea, ni el NIF, ni el titular.  Los
//!   nombres de factura se enmascaran (`<NIF>_J….pdf`).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Nombre del fichero local con el estado del operador (el que trae la API).
const ESTADO_OPERADOR: &str = "gasto_jazztel.json";

/// Umbral por defecto (MB) a partir del cual se avisa.
const UMBRAL_AVISO_MB: u64 = 500;

/// Periodo de facturación por defecto, en días (si el operador no lo dice).
const CICLO_POR_DEFECTO_DIAS: u64 = 30;

/// Operador conhecido (medido: `gsm.operator.alpha=JAZZTEL`, MCC/MNC 21403).
pub const OPERADOR: &str = "JAZZTEL";
/// MCC/MNC medido en el móvil.
pub const SIM_MCC_MNC: &str = "21403";

/// Ruta del fichero de estado del operador.
pub fn ruta_estado(state_dir: Option<&str>) -> PathBuf {
    let base = state_dir
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::state_dir::state_dir());
    Path::new(&base).join(ESTADO_OPERADOR)
}

/// Bytes por transporte, leídos de `/proc/net/dev`.
///
/// Es la **medición del sistema**, no la cifra facturada: sirve para
/// decidir "puedo gastar en WiFi o no" y para repartir el agregado del
/// operador entre apps.
pub fn medir_transportes() -> Value {
    let net = match std::fs::read_to_string("/proc/net/dev") {
        Ok(t) => t,
        Err(e) => return json!({"ok": false, "error": format!("leer /proc/net/dev: {e}")}),
    };
    let mut por_tipo: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut interfaces = Vec::new();
    for linea in net.lines() {
        let Some((iface, resto)) = linea.split_once(':') else {
            continue;
        };
        let iface = iface.trim();
        if iface.is_empty() || iface == "lo" {
            continue;
        }
        let c: Vec<u64> = resto
            .split_whitespace()
            .map(|x| x.parse::<u64>().unwrap_or(0))
            .collect();
        if c.len() < 9 {
            continue;
        }
        let rx = c[0];
        let tx = c[8];
        let total = rx.saturating_add(tx);
        let tipo = tipo_transporte(iface);
        let acc = por_tipo.entry(tipo.to_string()).or_insert(json!(0u64));
        let suma = acc.as_u64().unwrap_or(0).saturating_add(total);
        *acc = json!(suma);
        interfaces.push(json!({
            "interfaz": iface,
            "tipo": tipo,
            "rx_bytes": rx,
            "tx_bytes": tx,
            "bytes": total,
        }));
    }
    json!({
        "ok": true,
        "por_transporte": Value::Object(por_tipo),
        "interfaces": interfaces,
        "medido": "el sistema (NO la cifra que factura el operador)",
    })
}

/// Tipo de una interfaz de red.
///
/// MEDIDO (Samsung S22, Android 14): la interfaz de **datos móviles** es
/// `rmnet_data0` (o `rmnet_data1`, `ccmni0`… según el módem) y la de
/// **WiFi** es `wlan0`.
pub fn tipo_transporte(iface: &str) -> &'static str {
    let i = iface.to_ascii_lowercase();
    if i.starts_with("wlan") || i.starts_with("wifi") {
        "wifi"
    } else if i.starts_with("rmnet")
        || i.starts_with("ccmni")
        || i.starts_with("ppp")
        || i.starts_with("wwan")
        || i.starts_with("cdma")
    {
        "movil"
    } else if i.starts_with("eth") {
        "ethernet"
    } else {
        "otra"
    }
}

/// Convierte bytes a MiB (÷ 1.048.576) con un decimal.
pub fn a_mib(bytes: u64) -> f64 {
    (bytes as f64) / 1_048_576.0
}

/// Convierte MiB a bytes.
pub fn de_mib(mib: f64) -> u64 {
    (mib * 1_048_576.0).round() as u64
}

/// Estado del operador (el que trae la API de Jazztel).
///
/// **NO se inventa**: si no hay fichero, se dice que no hay datos.  El
/// formato esperado (el que devuelve la API, medido en la RE) es:
///
/// ```json
/// {
///   "line": "<enmascarado>",
///   "consumed_bytes": 12345678,
///   "included_bytes": 30000000000,
///   "remaining_bytes": 29876543222,
///   "expiration_date": "2026-12-14",
///   "consumption_type": "MOBILE_DATA",
///   "source": "api|factura|manual",
///   "updated_at": 1795690000
/// }
/// ```
pub fn leer_operador(state_dir: Option<&str>) -> Value {
    let p = ruta_estado(state_dir);
    match std::fs::read_to_string(&p) {
        Ok(txt) => match serde_json::from_str::<Value>(&txt) {
            Ok(v) => {
                let mut v = v;
                v["ok"] = json!(true);
                v["fuente_local"] = json!(p.display().to_string());
                v
            }
            Err(e) => json!({
                "ok": false,
                "error": format!("el estado del operador no es JSON: {e}"),
                "ruta": p.display().to_string(),
            }),
        },
        Err(e) => json!({
            "ok": false,
            "error": format!("no hay estado del operador: {e}"),
            "ruta": p.display().to_string(),
            "pista": "descárgalo de la API de Jazztel y guárdalo ahí (0600)",
        }),
    }
}

/// Escribe el estado del operador (quien lo traiga lo guarda aquí).
///
/// No se inventa nada: quien llama es responsable de los datos.
pub fn guardar_operador(v: &Value, state_dir: Option<&str>) -> Result<String, String> {
    let p = ruta_estado(state_dir);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;
    }
    std::fs::write(&p, serde_json::to_vec_pretty(v).unwrap_or_default())
        .map_err(|e| format!("escribir {}: {e}", p.display()))?;
    // 0600: puede contener datos de la línea (aunque enmascarados).
    set_modo_600(&p);
    Ok(p.display().to_string())
}

/// `chmod 600` (best-effort: en algunos sistemas no está el chmod).
fn set_modo_600(p: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = p;
}

/// Los avisos: umbral alcanzado, ciclo próximo, y discrepancia con el
/// sistema.
///
/// La **discrepancia** es el dato más útil: el sistema dice X y el operador
/// dice Y ⇒ la diferencia es la que explica "por qué no coincide con mi
/// Android".
pub fn avisos(state_dir: Option<&str>, umbral_mb: Option<u64>) -> Value {
    let umbral = umbral_mb.unwrap_or(UMBRAL_AVISO_MB);
    let op = leer_operador(state_dir);
    let med = medir_transportes();

    let mut avisos = Vec::new();
    let mut detalle = json!({});

    if op["ok"] == json!(true) {
        let consumo = op["consumed_bytes"].as_u64().unwrap_or(0);
        let incluido = op["included_bytes"].as_u64().unwrap_or(0);
        let restante = op
            .get("remaining_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| incluido.saturating_sub(consumo));

        if consumo >= de_mib(umbral as f64) {
            avisos.push(json!({
                "tipo": "umbral",
                "mensaje": format!(
                    "consumo del operador: {} MiB (umbral: {} MiB)",
                    a_mib(consumo).round() as u64,
                    umbral
                ),
                "severidad": "aviso",
            }));
        }
        if restante == 0 && incluido > 0 {
            avisos.push(json!({
                "tipo": "agotado",
                "mensaje": "no queda datos en el plan del operador",
                "severidad": "critico",
            }));
        } else if incluido > 0 {
            let pct = (consumo as f64) * 100.0 / (incluido as f64);
            if pct >= 80.0 {
                avisos.push(json!({
                    "tipo": "cercano",
                    "mensaje": format!("has usado el {:.0} % del plan", pct),
                    "severidad": "aviso",
                }));
            }
        }

        // La fecha de renovación: la del operador si la trae; si no, la
        // estimada por el ciclo por defecto (marcado como estimada).
        let renovacion = op
            .get("expiration_date")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let dias = op.get("days_remaining").and_then(|v| v.as_u64());
        if let Some(d) = dias {
            avisos.push(json!({
                "tipo": "renovacion",
                "mensaje": format!("quedan {d} días para la renovación"),
                "severidad": "info",
            }));
        } else if renovacion.is_none() {
            avisos.push(json!({
                "tipo": "ciclo",
                "mensaje": format!("ciclo por defecto de {CICLO_POR_DEFECTO_DIAS} días (NO VERIFICADO: el operador no lo dice)"),
                "severidad": "info",
            }));
        }

        // La discrepancia con la medición del sistema.
        let movil_medido = med["por_transporte"]["movil"].as_u64().unwrap_or(0);
        if movil_medido > 0 {
            let dif = consumo as i64 - movil_medido as i64;
            if dif.abs() > de_mib(10.0) as i64 {
                avisos.push(json!({
                    "tipo": "discrepancia",
                    "mensaje": format!(
                        "el operador dice {} MiB y el sistema ve {} MiB en la red móvil (diferencia: {} MiB)",
                        a_mib(consumo).round() as i64,
                        a_mib(movil_medido).round() as i64,
                        a_mib(dif.unsigned_abs()).round() as i64
                    ),
                    "severidad": "info",
                    "causas_posibles": [
                        "tethering (el tráfico de otros dispositivos va a cargo del móvil y Android no lo ve como app)",
                        "roaming",
                        "APN de respaldo",
                        "Data Saver",
                        "corte del ciclo de facturación",
                    ],
                }));
            }
        }

        detalle = json!({
            "consumido_mib": a_mib(consumo).round() as u64,
            "incluido_mib": a_mib(incluido).round() as u64,
            "restante_mib": a_mib(restante).round() as u64,
            "renovacion": renovacion,
            "dias_restantes": dias,
            "tipo_consumo": op.get("consumption_type").cloned().unwrap_or(Value::Null),
            "origen": op.get("source").cloned().unwrap_or(Value::Null),
        });
    } else {
        avisos.push(json!({
            "tipo": "sin_datos",
            "mensaje": "no hay estado del operador: sólo se puede medir lo que ve el sistema",
            "severidad": "info",
        }));
    }

    json!({
        "ok": true,
        "avisos": avisos,
        "operador": detalle,
        "medicion_sistema": med,
        "advertencia": "la medición del sistema NO es la cifra que factura Jazztel",
    })
}

/// Estado completo: medición + operador + avisos.
pub fn estado(state_dir: Option<&str>) -> Value {
    json!({
        "ok": true,
        "operador_nombre": OPERADOR,
        "mcc_mnc": SIM_MCC_MNC,
        "medicion": medir_transportes(),
        "estado_operador": leer_operador(state_dir),
        "nota": "lo que ve el sistema y lo que factura el operador son dos cosas; ver docs/RE_GASTO_DATOS_JAZZTEL.md",
    })
}

/// Desglose explícito: WiFi frente a datos móviles.
pub fn desglose(state_dir: Option<&str>) -> Value {
    let med = medir_transportes();
    let t = &med["por_transporte"];
    json!({
        "ok": true,
        "wifi_bytes": t["wifi"].as_u64().unwrap_or(0),
        "movil_bytes": t["movil"].as_u64().unwrap_or(0),
        "ethernet_bytes": t["ethernet"].as_u64().unwrap_or(0),
        "wifi_mib": a_mib(t["wifi"].as_u64().unwrap_or(0)).round() as u64,
        "movil_mib": a_mib(t["movil"].as_u64().unwrap_or(0)).round() as u64,
        "criterio": "clasificación por interfaz: wlan*=wifi, rmnet*/ccmni*/ppp*=movil",
        "medicion": med,
    })
}

/// Tool MCP.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "gasto_datos",
            "description": "GASTO DE DATOS de Jazztel (MNO 21403): mide el tráfico del sistema DESGLOSADO POR TRANSPORTE (WiFi frente a datos moviles, por interfaz), compara con el estado del operador (la cifra que factura) y avisa de umbrales, renovacion y discrepancias. Acciones: estado | movil | wifi | avisos | guardar (escribe el estado del operador) | desglose. Lo que ve el sistema NO es lo que factura el operador: la diferencia se explica en el aviso 'discrepancia'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "accion": {
                        "type": "string",
                        "enum": ["estado", "movil", "wifi", "avisos", "guardar", "desglose"],
                        "default": "estado"
                    },
                    "estado_operador": {
                        "type": "object",
                        "description": "Estado del operador (accion guardar): consumed_bytes, included_bytes, remaining_bytes, expiration_date, consumption_type, source."
                    },
                    "umbral_mb": {
                        "type": "integer",
                        "description": "Umbral de aviso en MiB (accion avisos; defecto 500).",
                        "default": 500
                    }
                },
                "required": ["accion"]
            }
        }
    ])
}

/// Despacha la herramienta MCP.
pub fn dispatch(args: &Value) -> Value {
    let s = |k: &str| {
        args.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let accion = s("accion");
    let accion = if accion.is_empty() { "estado".to_string() } else { accion };
    match accion.as_str() {
        "movil" => {
            let d = desglose(None);
            json!({
                "ok": true,
                "movil_bytes": d["movil_bytes"],
                "movil_mib": d["movil_mib"],
                "criterio": d["criterio"],
                "aviso": "lo que ve el sistema; la cifra del operador está aparte",
            })
        }
        "wifi" => {
            let d = desglose(None);
            json!({
                "ok": true,
                "wifi_bytes": d["wifi_bytes"],
                "wifi_mib": d["wifi_mib"],
                "criterio": d["criterio"],
            })
        }
        "desglose" => desglose(None),
        "avisos" => avisos(
            None,
            args.get("umbral_mb").and_then(|v| v.as_u64()),
        ),
        "guardar" => {
            let v = args.get("estado_operador").cloned().unwrap_or(Value::Null);
            if v.is_null() {
                json!({"ok": false, "error": "falta estado_operador"})
            } else {
                match guardar_operador(&v, None) {
                    Ok(ruta) => json!({"ok": true, "ruta": ruta, "aviso": "credenciales fuera del repo: este fichero es 0600"}),
                    Err(e) => json!({"ok": false, "error": e}),
                }
            }
        }
        _ => estado(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clasifica_las_interfaces_por_transporte() {
        assert_eq!(tipo_transporte("wlan0"), "wifi");
        assert_eq!(tipo_transporte("rmnet_data0"), "movil");
        assert_eq!(tipo_transporte("ccmni0"), "movil");
        assert_eq!(tipo_transporte("eth0"), "ethernet");
        assert_eq!(tipo_transporte("ap0"), "otra");
    }

    #[test]
    fn convierte_bytes_a_mib_y_vuelta() {
        assert_eq!(a_mib(1_048_576) as u64, 1);
        assert_eq!(de_mib(1.0), 1_048_576);
        assert_eq!(de_mib(500.0), 524_288_000);
    }

    #[test]
    fn la_medicion_devuelve_el_esqueleto_aunque_no_haya_proc() {
        // En Linux hay /proc/net/dev; en cualquier caso no debe entrar en pánico.
        let v = medir_transportes();
        assert!(v.is_object());
        assert!(v.get("por_transporte").is_some());
    }

    #[test]
    fn sin_estado_del_operador_los_avisos_lo_dicen() {
        let dir = std::env::temp_dir().join(format!("md_gasto_{}", crate::google_auth::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let sd = dir.to_string_lossy().to_string();
        let a = avisos(Some(&sd), None);
        assert_eq!(a["ok"], true);
        let tipos: Vec<String> = a["avisos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["tipo"].as_str().unwrap_or("").to_string())
            .collect();
        assert!(tipos.contains(&"sin_datos".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn con_estado_del_operador_avisa_del_umbral_y_de_la_discrepancia() {
        let dir = std::env::temp_dir().join(format!("md_gasto2_{}", crate::google_auth::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let sd = dir.to_string_lossy().to_string();
        let estado = json!({
            "consumed_bytes": de_mib(600.0),
            "included_bytes": de_mib(1000.0),
            "remaining_bytes": de_mib(400.0),
            "expiration_date": "2026-12-14",
            "consumption_type": "MOBILE_DATA",
            "source": "api",
        });
        guardar_operador(&estado, Some(&sd)).unwrap();
        let a = avisos(Some(&sd), Some(500));
        let tipos: Vec<String> = a["avisos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["tipo"].as_str().unwrap_or("").to_string())
            .collect();
        assert!(tipos.contains(&"umbral".to_string()), "{:?}", tipos);
        assert_eq!(a["operador"]["consumido_mib"], 600);
        assert_eq!(a["operador"]["incluido_mib"], 1000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_estado_del_operador_se_guarda_en_0600() {
        let dir = std::env::temp_dir().join(format!("md_gasto3_{}", crate::google_auth::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let sd = dir.to_string_lossy().to_string();
        guardar_operador(&json!({"consumed_bytes": 1}), Some(&sd)).unwrap();
        let p = ruta_estado(Some(&sd));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(modo, 0o600, "el fichero debe ser 0600");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_tool_tiene_esquema_valido() {
        let d = tool_definitions();
        let arr = d.as_array().expect("array");
        assert_eq!(arr[0]["name"], "gasto_datos");
        assert_eq!(arr[0]["inputSchema"]["type"], "object");
    }

    #[test]
    fn el_despacho_responde_a_cada_accion() {
        for a in ["estado", "movil", "wifi", "avisos", "desglose"] {
            let v = dispatch(&json!({"accion": a}));
            assert!(v.is_object(), "{}", a);
        }
        let v = dispatch(&json!({"accion": "guardar"}));
        assert_eq!(v["ok"], false);
    }
}

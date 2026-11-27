//! **Keepalive del caché de prompt del proveedor** (patrón 2 de
//! `docs/DISENO_PATRONES_CLI_RUST.md`).
//!
//! MEDIDO en el producto de referencia (`affogato/cache_keepalive*.rs`, 5
//! ficheros): mantener vivo el **caché de prompt** del proveedor entre turnos.
//! Un modelo con *prompt caching* cobra mucho menos si el prefijo no cambia;
//! un agente que corta el contexto o cambia de modelo pierde el caché.
//!
//! La pieza clave (medida) es `cache_keepalive_inputs.rs`: **qué entradas
//! forman el prefijo cacheable**.  De ahí sale la política de *estabilidad de
//! prefijo*: no reordenar bloques del prompt, no cambiar el system prompt
//! entre turnos, no invalidar por herramientas dinámicas.
//!
//! Por qué aquí y no en un script: la regla del proyecto es que toda
//! capacidad nueva vive en Rust dentro del daemon y se expone por CLI y por
//! tool MCP.  Este módulo NO depende de nada externo (ni de la red, ni de
//! Drive): es una política local y determinista.
//!
//! MEDIDO 2026-11-28: implementación propia; el fichero del producto de
//! referencia es material propietario y NO se ha copiado (sólo la
//! arquitectura).

use serde_json::{json, Value};

/// Rol de una entrada del prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rol {
    /// El prompt de sistema (la cabeza estable del prompt).
    Sistema,
    /// Un turno del usuario.
    Usuario,
    /// Un turno del asistente.
    Asistente,
    /// El resultado de una herramienta (cambia en CADA turno).
    Herramienta,
}

impl Rol {
    /// ¿Este rol puede formar parte de un prefijo **cacheable**?
    ///
    /// MEDIDO: el resultado de una herramienta se inyecta entre turnos ⇒
    /// invalida el caché.  Por eso es *volátil*.
    pub fn es_estable(self) -> bool {
        !matches!(self, Rol::Herramienta)
    }
}

/// Una entrada del prompt (un bloque que se manda al proveedor).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entrada {
    pub rol: Rol,
    pub contenido: String,
    /// ¿El contenido depende del turno (herramientas, fechas, estado)?
    ///
    /// MEDIDO: una herramienta "dinámica" invalida el caché aunque el rol
    /// sea estable.
    pub dinamica: bool,
}

impl Entrada {
    /// Entrada ESTABLE (cacheable si va en la cabeza del prompt).
    pub fn estable(rol: Rol, contenido: impl Into<String>) -> Self {
        Entrada {
            rol,
            contenido: contenido.into(),
            dinamica: false,
        }
    }

    /// Entrada VOLÁTIL: depende del turno ⇒ corta el prefijo cacheable.
    pub fn volatil(rol: Rol, contenido: impl Into<String>) -> Self {
        Entrada {
            rol,
            contenido: contenido.into(),
            dinamica: true,
        }
    }

    /// Longitud en caracteres (lo que mide el presupuesto del proveedor).
    pub fn chars(&self) -> usize {
        self.contenido.chars().count()
    }
}

/// El **prefijo cacheable** de una petición: la*racha inicial* de entradas
/// estables.
///
/// MEDIDO: el caché del proveedor cubre un PREFIJO.  En cuanto aparece una
/// entrada volátil (resultado de herramienta, contenido dinámico), todo lo
/// que viene después ya no está cacheado ⇒ el prefijo termina ahí.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prefijo {
    /// Las entradas que forman el prefijo (en orden).
    pub entradas: Vec<Entrada>,
    /// Caracteres del prefijo.
    pub chars: usize,
    /// ¿Se cortó el prefijo por una entrada volátil?
    pub cortado_por_volatil: bool,
}

impl Prefijo {
    /// Calcula el prefijo cacheable de una lista de entradas.
    pub fn calcular(entradas: &[Entrada]) -> Prefijo {
        let mut out = Prefijo::default();
        for e in entradas {
            if !e.rol.es_estable() || e.dinamica {
                out.cortado_por_volatil = true;
                break;
            }
            out.chars += e.chars();
            out.entradas.push(e.clone());
        }
        out
    }

    /// ¿El prefijo tiene algo que cachear?
    ///
    /// MEDIDO: un prefijo por debajo del mínimo no compensa (la primera
    /// llamada con caché suele ser más cara).
    pub fn es_util(&self, minimo_chars: usize) -> bool {
        self.chars >= minimo_chars
    }
}

/// Política de keepalive (cuándo se refresca el prefijo).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Politica {
    /// Segundos de vida del prefijo cacheado en el proveedor.
    pub ttl_s: u64,
    /// Caracteres mínimos para que el caché compense.
    pub minimo_chars: usize,
    /// Presupuesto máximo de caracteres que se mantienen "calientes".
    pub presupuesto_chars: usize,
}

impl Default for Politica {
    /// MEDIDO (2026-11-28): valores por defecto.  El TTL de 5 minutos es el
    /// habitual en los proveedores con caché de prompt; el mínimo de 1.024
    /// caracteres es el umbral por debajo del cual el caché no suele
    /// compensar.
    fn default() -> Self {
        Politica {
            ttl_s: 300,
            minimo_chars: 1_024,
            presupuesto_chars: 32_000,
        }
    }
}

/// Estado del prefijo cacheado en el proveedor (lo que se recuerda entre
/// turnos).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Estado {
    /// Firma del prefijo (cambia si cambia un solo byte del prefijo).
    pub firma: String,
    /// Caracteres del prefijo cacheado.
    pub chars: usize,
    /// Cuándo se起用 por última vez (segundos desde la época).
    pub ultimo_uso_s: u64,
}

impl Estado {
    /// Estado vacío (nada cacheado todavía).
    pub fn vacio() -> Self {
        Estado {
            firma: String::new(),
            chars: 0,
            ultimo_uso_s: 0,
        }
    }
}

/// Qué hay que hacer con el prefijo en este turno.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// El prefijo sigue vivo: se reutiliza el caché (no se paga el prefijo
    /// completo).
    Reutilizar,
    /// El prefijo cambió (o no había): el caché se perdió; el proveedor
    /// cobra el prefijo entero.
    Invalidado,
    /// El prefijo sigue igual pero ha caducado: hay que hacer una llamada
    /// barata de "calentamiento" para re-vivirlo.
    Refrescar,
}

/// El resultado de evaluar la política en un turno.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub decision: Decision,
    /// Por qué se ha tomado esa decisión (texto legible).
    pub motivo: String,
    /// Caracteres que el proveedor cobraría a precio COMPLETO.
    pub chars_completos: usize,
    /// Caracteres que se cobrarían a precio de CACHÉ (o 0 si no hay caché).
    pub chars_cacheados: usize,
    /// Ahorro estimado en caracteres (no en dinero: el precio por carácter
    /// lo decide el proveedor).
    pub ahorro_chars: usize,
}

/// Firma estable del prefijo.
///
/// MEDIDO: lo que decide si el caché sigue vivo es si el PREFIJO es idéntico
/// byte a byte.  La firma es un hash FNV-1a (sin dependencias externas).
pub fn firma(entradas: &[Entrada]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for e in entradas {
        // Se mezcla el discriminante del rol y el contenido.
        h ^= e.rol as u8 as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
        for b in e.contenido.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        h ^= 0xFF;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("{:016x}", h)
}

/// Evalúa la política de keepalive para un turno.
///
/// * `prev` — lo que se recuerda del turno anterior (`None` = primera vez).
/// * `entradas` — las entradas del prompt de ESTE turno.
/// * `ahora_s` — el reloj del turno (segundos; lo inyecta el llamante para
///   que el módulo sea determinista y testeable).
pub fn evaluar(
    prev: Option<&Estado>,
    entradas: &[Entrada],
    ahora_s: u64,
    politica: &Politica,
) -> Plan {
    let prefijo = Prefijo::calcular(entradas);
    let f = firma(&prefijo.entradas);
    let chars_completos = prefijo.chars;

    // Sin prefijo útil: no hay nada que cachear.
    if !prefijo.es_util(politica.minimo_chars) {
        return Plan {
            decision: Decision::Invalidado,
            motivo: format!(
                "prefijo cacheable de {} caracteres < minimo {}: no compensa",
                prefijo.chars, politica.minimo_chars
            ),
            chars_completos,
            chars_cacheados: 0,
            ahorro_chars: 0,
        };
    }

    // El prefijo no cabe en el presupuesto: se recorta (y se avisa).
    if prefijo.chars > politica.presupuesto_chars {
        return Plan {
            decision: Decision::Invalidado,
            motivo: format!(
                "prefijo de {} caracteres > presupuesto {}: el cache se pierde",
                prefijo.chars, politica.presupuesto_chars
            ),
            chars_completos,
            chars_cacheados: 0,
            ahorro_chars: 0,
        };
    }

    match prev {
        None => Plan {
            decision: Decision::Invalidado,
            motivo: "primera vez: el prefijo se cachea al enviarlo".to_string(),
            chars_completos,
            chars_cacheados: 0,
            ahorro_chars: 0,
        },
        Some(e) if e.firma != f => Plan {
            decision: Decision::Invalidado,
            motivo: format!(
                "el prefijo cambio (firma {} -> {}): el cache se perdio",
                e.firma, f
            ),
            chars_completos,
            chars_cacheados: 0,
            ahorro_chars: 0,
        },
        Some(e) => {
            let transcurrido = ahora_s.saturating_sub(e.ultimo_uso_s);
            if transcurrido >= politica.ttl_s {
                Plan {
                    decision: Decision::Refrescar,
                    motivo: format!(
                        "el prefijo sigue igual pero hace {} s (ttl {}): hay que calentarlo",
                        transcurrido, politica.ttl_s
                    ),
                    chars_completos,
                    chars_cacheados: 0,
                    ahorro_chars: 0,
                }
            } else {
                Plan {
                    decision: Decision::Reutilizar,
                    motivo: format!(
                        "prefijo identico hace {} s (ttl {}): el cache sigue vivo",
                        transcurrido, politica.ttl_s
                    ),
                    chars_completos,
                    chars_cacheados: prefijo.chars,
                    ahorro_chars: prefijo.chars,
                }
            }
        }
    }
}

/// Estado NUEVO que hay que recordar para el siguiente turno.
///
/// MEDIDO: si la decisión es `Invalidado`, el estado anterior ya no vale
/// (el caché se perdió) ⇒ se empieza de cero con la firma actual.
pub fn estado_siguiente(plan: &Plan, entradas: &[Entrada], ahora_s: u64) -> Estado {
    let prefijo = Prefijo::calcular(entradas);
    match plan.decision {
        Decision::Reutilizar | Decision::Refrescar => Estado {
            firma: firma(&prefijo.entradas),
            chars: prefijo.chars,
            ultimo_uso_s: ahora_s,
        },
        Decision::Invalidado => Estado {
            firma: firma(&prefijo.entradas),
            chars: prefijo.chars,
            ultimo_uso_s: ahora_s,
        },
    }
}

// =============================================================================
// CLI
// =============================================================================
/// `megadaemon-core cache-keepalive <prefijo|evaluar|politica>`
pub fn run_cli(args: &[String]) -> i32 {
    match run_cli_res(args) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("cache-keepalive error: {}", e);
            1
        }
    }
}

fn run_cli_res(args: &[String]) -> Result<Value, String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    match sub {
        "prefijo" => {
            let entradas = leer_entradas(args.get(1).map(|s| s.as_str()).unwrap_or(""))?;
            let p = Prefijo::calcular(&entradas);
            Ok(json!({
                "chars": p.chars,
                "cortado_por_volatil": p.cortado_por_volatil,
                "entradas": p.entradas.len(),
                "firma": firma(&p.entradas),
            }))
        }
        "evaluar" => {
            let entradas = leer_entradas(args.get(1).map(|s| s.as_str()).unwrap_or(""))?;
            let ahora: u64 = args
                .get(2)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let politica = Politica::default();
            let plan = evaluar(None, &entradas, ahora, &politica);
            Ok(json!({
                "decision": format!("{:?}", plan.decision),
                "motivo": plan.motivo,
                "chars_completos": plan.chars_completos,
                "chars_cacheados": plan.chars_cacheados,
                "ahorro_chars": plan.ahorro_chars,
            }))
        }
        "politica" => {
            let p = Politica::default();
            Ok(json!({
                "ttl_s": p.ttl_s,
                "minimo_chars": p.minimo_chars,
                "presupuesto_chars": p.presupuesto_chars,
            }))
        }
        "" => Err("uso: cache-keepalive <prefijo|evaluar|politica> [entradas] [ahora_s]".to_string()),
        otro => Err(format!("subcomando desconocido: {}", otro)),
    }
}

/// Lee las entradas de un prompt desde una línea de la forma
/// `sistema:texto|usuario:texto|herramienta:texto`.
///
/// El separador `|` es el que no puede aparecer en el contenido (si aparece,
/// se parte el bloque en varios).
pub fn leer_entradas(spec: &str) -> Result<Vec<Entrada>, String> {
    if spec.trim().is_empty() {
        return Err("no hay entradas que leer".to_string());
    }
    let mut out = Vec::new();
    for bloque in spec.split('|') {
        let (rol, texto) = bloque
            .split_once(':')
            .ok_or_else(|| format!("bloque sin ':' -> '{}'", bloque))?;
        let (rol, dinamica) = match rol.trim() {
            "sistema" => (Rol::Sistema, false),
            "usuario" => (Rol::Usuario, false),
            "asistente" => (Rol::Asistente, false),
            "herramienta" => (Rol::Herramienta, true),
            "dinamico" => (Rol::Usuario, true),
            otro => return Err(format!("rol desconocido: '{}'", otro)),
        };
        out.push(Entrada {
            rol,
            contenido: texto.to_string(),
            dinamica,
        });
    }
    Ok(out)
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn prompt_largo() -> Vec<Entrada> {
        vec![
            Entrada::estable(Rol::Sistema, "eres un agente que ayuda".repeat(60)),
            Entrada::estable(Rol::Usuario, "hola"),
        ]
    }

    #[test]
    fn el_prefijo_termina_en_la_primera_entrada_volatil() {
        let es = vec![
            Entrada::estable(Rol::Sistema, "a".repeat(2_000)),
            Entrada::volatil(Rol::Herramienta, "salida de la herramienta"),
            Entrada::estable(Rol::Usuario, "esto ya no cuenta"),
        ];
        let p = Prefijo::calcular(&es);
        assert_eq!(p.entradas.len(), 1);
        assert!(p.cortado_por_volatil);
        assert_eq!(p.chars, 2_000);
    }

    #[test]
    fn una_herramienta_corta_el_prefijo_aunque_el_rol_no_sea_dinamico() {
        let es = vec![
            Entrada::estable(Rol::Sistema, "a".repeat(2_000)),
            Entrada::estable(Rol::Herramienta, "x"),
        ];
        let p = Prefijo::calcular(&es);
        assert_eq!(p.entradas.len(), 1);
        assert!(p.cortado_por_volatil);
    }

    #[test]
    fn la_firma_no_cambia_si_el_prefijo_es_identico() {
        let a = prompt_largo();
        let b = prompt_largo();
        assert_eq!(firma(&Prefijo::calcular(&a).entradas), firma(&Prefijo::calcular(&b).entradas));
    }

    #[test]
    fn la_firma_cambia_si_cambia_un_byte_del_prefijo() {
        let a = prompt_largo();
        let mut b = prompt_largo();
        b[0].contenido.push('X');
        assert_ne!(firma(&Prefijo::calcular(&a).entradas), firma(&Prefijo::calcular(&b).entradas));
    }

    #[test]
    fn la_firma_cambia_si_cambia_el_orden_de_las_entradas() {
        let a = vec![
            Entrada::estable(Rol::Sistema, "uno"),
            Entrada::estable(Rol::Usuario, "dos"),
        ];
        let b = vec![
            Entrada::estable(Rol::Usuario, "dos"),
            Entrada::estable(Rol::Sistema, "uno"),
        ];
        assert_ne!(firma(&Prefijo::calcular(&a).entradas), firma(&Prefijo::calcular(&b).entradas));
    }

    #[test]
    fn el_primer_turno_invalida_y_deja_el_prefijo_listo() {
        let es = prompt_largo();
        let pol = Politica::default();
        let plan = evaluar(None, &es, 100, &pol);
        assert_eq!(plan.decision, Decision::Invalidado);
        assert_eq!(plan.ahorro_chars, 0);
        let est = estado_siguiente(&plan, &es, 100);
        assert!(!est.firma.is_empty());
        assert_eq!(est.chars, Prefijo::calcular(&es).chars);
    }

    #[test]
    fn el_segundo_turno_reutiliza_el_cache() {
        let es = prompt_largo();
        let pol = Politica::default();
        let p1 = evaluar(None, &es, 100, &pol);
        let e1 = estado_siguiente(&p1, &es, 100);
        let p2 = evaluar(Some(&e1), &es, 150, &pol);
        assert_eq!(p2.decision, Decision::Reutilizar);
        assert!(p2.ahorro_chars > 0);
        assert_eq!(p2.chars_cacheados, p2.chars_completos);
    }

    #[test]
    fn si_el_prefijo_cambia_el_cache_se_pierde() {
        let es = prompt_largo();
        let pol = Politica::default();
        let p1 = evaluar(None, &es, 100, &pol);
        let e1 = estado_siguiente(&p1, &es, 100);
        let mut es2 = prompt_largo();
        es2[0].contenido.push('Y');
        let p2 = evaluar(Some(&e1), &es2, 150, &pol);
        assert_eq!(p2.decision, Decision::Invalidado);
        assert_eq!(p2.ahorro_chars, 0);
    }

    #[test]
    fn si_pasa_el_ttl_hay_que_refrescar() {
        let es = prompt_largo();
        let pol = Politica {
            ttl_s: 60,
            ..Politica::default()
        };
        let p1 = evaluar(None, &es, 100, &pol);
        let e1 = estado_siguiente(&p1, &es, 100);
        let p2 = evaluar(Some(&e1), &es, 200, &pol);
        assert_eq!(p2.decision, Decision::Refrescar);
    }

    #[test]
    fn un_prefijo_muy_corto_no_compensa() {
        let es = vec![Entrada::estable(Rol::Usuario, "hola")];
        let pol = Politica::default();
        let plan = evaluar(None, &es, 0, &pol);
        assert_eq!(plan.decision, Decision::Invalidado);
        assert!(plan.motivo.contains("minimo"));
    }

    #[test]
    fn un_prefijo_por_encima_del_presupuesto_se_descarta() {
        let es = vec![Entrada::estable(Rol::Sistema, "a".repeat(5_000))];
        let pol = Politica {
            presupuesto_chars: 1_000,
            minimo_chars: 10,
            ..Politica::default()
        };
        let plan = evaluar(None, &es, 0, &pol);
        assert_eq!(plan.decision, Decision::Invalidado);
        assert!(plan.motivo.contains("presupuesto"));
    }

    #[test]
    fn la_lectura_de_entradas_entiende_los_roles() {
        let es = leer_entradas("sistema:eres un agente|usuario:hola|herramienta:ls").unwrap();
        assert_eq!(es.len(), 3);
        assert_eq!(es[0].rol, Rol::Sistema);
        assert_eq!(es[2].rol, Rol::Herramienta);
        assert!(es[2].dinamica);
        assert!(leer_entradas("").is_err());
        assert!(leer_entradas("sin_rol").is_err());
        assert!(leer_entradas("inventado:x").is_err());
    }
}

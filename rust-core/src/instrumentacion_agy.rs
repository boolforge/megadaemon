//! MegaDaemon — **instrumentación de Antigravity, automática y total**.
//!
//! EL PROBLEMA (medido el 2026-11-23, queja del usuario)
//! -------------------------------------------------------
//! "Cuando arranco Antigravity no noto nada": las piezas EXISTEN pero nadie
//! las aplica de forma automática:
//!
//! | pieza | módulo | qué hace |
//! |---|---|---|
//! | endpoint local en el `.so` | `antigravity_endpoint` | parchea la URL del proveedor para que el tráfico pase por MegaDaemon |
//! | `.so` **falso** (el daemon posee el original) | `antigravity_fake_so` | el original queda en el dir del daemon, fuera del alcance de la app |
//! | MegaDaemon como modelo | `antigravity_model` | lo registra por RPC (`JetboxWriteState`) y lo pone por defecto |
//! | MITM de la DB de conversaciones | `sqlite_mitm` | bind-mount: la app escribe en la copia, la original queda intacta |
//! | puente MCP | aquí | mete el token de sesión en la config de Antigravity (en **Rust**: el supervisor lo hacía con Python) |
//!
//! QUÉ HACE ESTE MÓDULO
//! ---------------------
//! Un único sitio que decide **qué se aplica**, lo aplica **en Rust** (sin
//! procesos extra: el supervisor lanzaba un Python por cada cambio de token ⇒
//! un proceso de más en un móvil que va justo), y lo **reporta** con
//! precisión (qué estaba aplicado, qué se ha aplicado ahora, qué ha fallado
//! y por qué).
//!
//! ES IDEMPOTENTE Y BARATO
//! -----------------------
//! Cada paso comprueba PRIMERO si ya está aplicado (hash/flag/fichero) ⇒ si
//! no hay nada que hacer no se toca nada.  Esto importa: el móvil va con
//! ~4 GB de RAM y la app de Antigravity puede estar en primer plano.
//!
//! LA POLÍTICA ESTÁ EN LA CONFIG
//! ------------------------------
//! `instrumentacion_agy` en `<state>/config.json` decide qué partes se
//! aplican (ver [`crate::config_efectiva`]).  Así el usuario decide, desde
//! la Web UI, quéDegree de instrumentación quiere.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Puerto local donde vive el gateway de MegaDaemon.
pub const PUERTO_GATEWAY: u16 = 9090;

/// Configuración de la instrumentación (leída de `config.json`).
fn cfg() -> Value {
    crate::config_efectiva::valor("instrumentacion_agy").unwrap_or_else(|| json!({}))
}

/// ¿Está activado un apartado? (por defecto, sí: el objetivo es que arranque solo).
fn on(clave: &str, defecto: bool) -> bool {
    crate::config_efectiva::bool_de(&format!("instrumentacion_agy.{clave}"), defecto)
}

/// ¿Está activo el modo del `.so`?  `falso` (el daemon posee el original),
/// `in situ` (parche en el sitio) o `ninguno`.
fn modo_so() -> String {
    crate::config_efectiva::string_de("instrumentacion_agy.modo_so", "falso")
}

/// Ruta del fichero de estado de la instrumentación.
pub fn ruta_estado() -> PathBuf {
    PathBuf::from(format!(
        "{}/instrumentacion_agy.json",
        crate::cockpit_server::state_dir()
    ))
}

fn leer_estado() -> Value {
    std::fs::read_to_string(ruta_estado())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}))
}

fn guardar_estado(v: &Value) {
    let ruta = ruta_estado();
    if let Some(p) = ruta.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let _ = crate::util_fs::write_atomic(&ruta, v.to_string().as_bytes(), 0o600);
}

/// **Estado real** de cada pieza de la instrumentación.
///
/// Se calcula preguntando a los módulos (no a un caché): lo que se ve es lo
/// que hay, no lo que el daemon cree que dejó.
pub fn estado() -> Value {
    let endpoint = crate::antigravity_endpoint::estado();
    let falso = crate::antigravity_fake_so::estado();
    let modelo = crate::antigravity_model::integracion_estado();
    let mitm = crate::sqlite_mitm::estado_mitm().unwrap_or_else(|e| json!({"ok": false, "error": e}));

    // ¿Está la app de Antigravity corriendo?  (si no, no tiene sentido
    // instrumentarla todavía, pero tampoco es un error).
    let agy_vivo = proceso_vivo("com.agy");

    let modelo_registrado = modelo
        .get("modelo_registrado")
        .and_then(|v| v.get("ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let puente_caido = modelo
        .get("bridge_error")
        .and_then(|v| v.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    json!({
        "ok": true,
        "agy_vivo": agy_vivo,
        "modo_so": modo_so(),
        "endpoint_local": {
            "aplicado": endpoint.get("aplicado").and_then(|v| v.as_bool()).unwrap_or(false),
            "url": endpoint.get("url").cloned().unwrap_or(Value::Null),
            "estado": endpoint,
        },
        "so_falso": {
            "activo": falso.get("activo").and_then(|v| v.as_bool()).unwrap_or(false),
            "estado": falso,
        },
        "modelo": {
            "registrado": modelo_registrado,
            "puente_caido": !puente_caido.is_empty(),
            "error_puente": if puente_caido.is_empty() {
                Value::Null
            } else {
                json!(puente_caido)
            },
            "estado": modelo,
        },
        "mitm_db": {
            "montado": mitm
                .get("montaje")
                .and_then(|m| m.get("montado"))
                .and_then(|v| v.as_u64())
                .map(|v| v == 1)
                .unwrap_or(false),
            "estado": mitm,
        },
        "puente_mcp": puente_mcp_estado(),
        "config": cfg(),
    })
}

/// Estado del puente MCP dentro de la configuración de Antigravity.
fn puente_mcp_estado() -> Value {
    let ruta = crate::antigravity_model::AGY_MCP_CONFIG;
    let existe = Path::new(ruta).exists();
    let mut token_ok = false;
    let mut servidores = 0usize;
    let mut error = None;
    if existe {
        match std::fs::read_to_string(ruta) {
            Ok(txt) => match serde_json::from_str::<Value>(&txt) {
                Ok(v) => {
                    let token = crate::cockpit_server::auth_token();
                    if let Some(map) = v.get("mcpServers").and_then(|m| m.as_object()) {
                        servidores = map.len();
                        for (_, spec) in map {
                            let url = spec.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            if url.contains("127.0.0.1:9090/mcp") || url.contains("localhost:9090/mcp") {
                                let h = spec
                                    .get("headers")
                                    .and_then(|h| h.get("X-Seere-Auth-Token"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                if !token.is_empty() && h == token {
                                    token_ok = true;
                                }
                            }
                        }
                    }
                }
                Err(e) => error = Some(e.to_string()),
            },
            Err(e) => error = Some(e.to_string()),
        }
    }
    json!({
        "ruta": ruta,
        "existe": existe,
        "servidores": servidores,
        "token_al_dia": token_ok,
        "error": error,
    })
}

/// ¿Está el proceso de Antigravity vivo?
fn proceso_vivo(paquete: &str) -> bool {
    // `pidof` no existe en todos los Android; `ps` sí.  Se busca el nombre
    // del proceso de la app.
    match std::process::Command::new("ps").arg("-A").arg("-o").arg("args").output() {
        Ok(o) => {
            let txt = String::from_utf8_lossy(&o.stdout);
            txt.lines().any(|l| l.contains(paquete))
        }
        Err(_) => false,
    }
}

/// **Sincroniza el puente MCP** dentro de la configuración de Antigravity.
///
/// En **Rust** (el supervisor lo hacía con un Python por cada cambio de
/// token: un proceso de más en un móvil que va justo, y además tardaba).
///
/// * No bloquea la app: escribe el fichero y conserva sus permisos.
pub fn sincronizar_puente_mcp() -> Result<Value, String> {
    let ruta = crate::antigravity_model::AGY_MCP_CONFIG;
    let token = crate::cockpit_server::auth_token();
    if token.is_empty() {
        return Err("no hay token de sesión: el puente MCP no se puede sincronizar".to_string());
    }
    let mut v: Value = std::fs::read_to_string(ruta)
        .map_err(|e| format!("leer {ruta}: {e}"))?
        .parse()
        .map_err(|e| format!("parsear {ruta}: {e}"))?;
    let base = format!("http://127.0.0.1:{PUERTO_GATEWAY}/mcp");
    let servers = v
        .as_object_mut()
        .ok_or("la config de Antigravity no es un objeto")?
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("mcpServers no es un objeto")?;
    let mut cambios = 0usize;
    for (_, spec) in servers.iter_mut() {
        let Some(obj) = spec.as_object_mut() else { continue };
        let url = obj.get("url").and_then(|u| u.as_str()).unwrap_or("");
        if !url.contains("127.0.0.1:9090/mcp") && !url.contains("localhost:9090/mcp") {
            continue;
        }
        let headers = obj
            .entry("headers")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or("headers no es un objeto")?;
        if headers.get("X-Seere-Auth-Token").and_then(|t| t.as_str()) != Some(token) {
            headers.insert("X-Seere-Auth-Token".to_string(), json!(token));
            cambios += 1;
        }
    }
    // Si no hay ningun servidor apuntando al gateway, se registra uno: asi el
    // MCP de MegaDaemon queda disponible para Antigravity sin tocar la GUI.
    if !servers.values().any(|spec| {
        spec.get("url")
            .and_then(|u| u.as_str())
            .map(|u| u.contains("127.0.0.1:9090/mcp") || u.contains("localhost:9090/mcp"))
            .unwrap_or(false)
    }) {
        servers.insert(
            "megadaemon".to_string(),
            json!({
                "url": base,
                "headers": { "X-Seere-Auth-Token": token },
            }),
        );
        cambios += 1;
    }
    let total_servidores = servers.len();
    if cambios > 0 {
        let txt = serde_json::to_string_pretty(&v).unwrap_or_default();
        crate::util_fs::write_atomic(Path::new(ruta), txt.as_bytes(), 0o644)
            .map_err(|e| format!("escribir {ruta}: {e}"))?;
    }
    Ok(json!({
        "ok": true,
        "cambios": cambios,
        "ruta": ruta,
        "servidores": total_servidores,
    }))
}

/// ¿Está MegaDaemon registrado como modelo en Antigravity?
///
/// MEDIDO (2026-11-23): se leía `registrado`, pero la clave real del estado
/// es `modelo_registrado.ok` ⇒ el modelo se daba SIEMPRE por pendiente y el
/// ciclo automático lo reintentaba cada minuto (y fallaba, porque la app no
/// está en marcha).
///
/// Devuelve `(registrado, puente_caido)`: si el puente está caído, no se
/// puede ni comprobar ni registrar ⇒ no es un fallo, es un APLAZAMIENTO.
fn estado_modelo() -> (bool, bool) {
    let m = crate::antigravity_model::integracion_estado();
    let registrado = m
        .get("modelo_registrado")
        .and_then(|v| v.get("ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let error_bridge = m
        .get("bridge_error")
        .and_then(|v| v.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let caido = !error_bridge.is_empty();
    (registrado, caido)
}

/// **Revisión BARATA**: qué piezas NO están en su sitio.
///
/// Sólo mira metadatos y ficheros pequeños: nunca hashea el `.so` de 181 MB.
/// Es lo que hace el ciclo automático (cada minuto) sin coste.
pub fn revision_cheap() -> Vec<String> {
    let mut faltan: Vec<String> = Vec::new();
    let modo = modo_so();
    if modo == "falso" {
        if !crate::antigravity_fake_so::instalado_cheap() {
            faltan.push("so".to_string());
        }
    } else if modo == "in situ" {
        let e = crate::antigravity_endpoint::estado();
        if !e
            .get("aplicado")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            faltan.push("so".to_string());
        }
    }
    if on("puente_mcp", true) {
        let p = puente_mcp_estado();
        let existe = p.get("existe").and_then(|v| v.as_bool()).unwrap_or(false);
        let al_dia = p.get("token_al_dia").and_then(|v| v.as_bool()).unwrap_or(false);
        if existe && !al_dia {
            faltan.push("puente_mcp".to_string());
        }
    }
    if on("modelo", true) {
        let (registrado, puente_caido) = estado_modelo();
        // Si el puente está caído (la app no está en marcha) no se puede
        // comprobar el registro: NO es una pieza que falte.
        if !registrado && !puente_caido {
            faltan.push("modelo".to_string());
        }
    }
    faltan
}

/// **Ciclo automático**: comprueba cada `intervalo_s` y re-instrumenta lo que
/// falte.
///
/// Éste es el motivo de que la instrumentación sea "automática y total": si
/// Antigravity restaura su `.so` (al reinstalarse, al actualizarse o al
/// detectar la manipulación), el daemon vuelve a poner el suyo sin que nadie
/// toque nada.  Es idempotente y barato: en el caso normal no hace nada.
pub fn bucle_automatico(puerto: u16) {
    let intervalo = crate::config_efectiva::u64_de(
        "instrumentacion_agy.intervalo_s",
        60,
    )
    .clamp(15, 3600);
    loop {
        std::thread::sleep(std::time::Duration::from_secs(intervalo));
        if !on("automatica", true) {
            continue;
        }
        let faltan = revision_cheap();
        if faltan.is_empty() {
            continue;
        }
        eprintln!(
            "instrumentación AGY: faltan {} → se aplica",
            faltan.join(", ")
        );
        let r = aplicar(puerto, true);
        let piezas = r
            .get("piezas")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for p in piezas {
            let estado = p.get("estado").and_then(|v| v.as_str()).unwrap_or("?");
            let pieza = p.get("pieza").and_then(|v| v.as_str()).unwrap_or("?");
            // El detalle del fallo va al log: si no, el móvil dice «fallo» y
            // no se sabe por qué (medido el 2026-11-23).
            let detalle = match p.get("detalle") {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            };
            let detalle = detalle.chars().take(220).collect::<String>();
            // OJO: el sufijo va como argumento POSICIONAL: `{detalle}` en el
            // formato sin pasarlo es un error de compilacion (el CI lo
            // detectó: «argument never used»).
            let sufijo = if detalle.is_empty() {
                String::new()
            } else {
                format!(": {detalle}")
            };
            eprintln!("instrumentación AGY: {pieza} → {estado}{sufijo}");
        }
    }
}

/// **Aplica** la instrumentación según la configuración.
///
/// Devuelve un informe por pieza: `aplicado` (lo estaba), `aplicado_ahora`
/// (lo se acaba de hacer), `omitido` (la config lo apaga) o `fallo` (con el
/// motivo).  Nunca se lanza: si una parte falla, las demás siguen.
pub fn aplicar(puerto: u16, confirmar: bool) -> Value {
    let mut informe = json!({ "ok": true, "piezas": [] });
    let mut piezas: Vec<Value> = Vec::new();

    // --- 1) el `.so` (falso o in situ) -------------------------------
    let modo = modo_so();
    if modo == "ninguno" {
        piezas.push(json!({
            "pieza": "so", "modo": modo, "estado": "omitido",
            "detalle": "la configuracion apaga la instrumentacion del .so",
        }));
    } else if modo == "falso" {
        match crate::antigravity_fake_so::activar(puerto, confirmar) {
            Ok(v) => {
                let ya = v.get("ya_activo").and_then(|x| x.as_bool()).unwrap_or(false);
                piezas.push(json!({
                    "pieza": "so", "modo": modo,
                    "estado": if ya { "aplicado" } else { "aplicado_ahora" },
                    "detalle": v,
                }));
            }
            Err(e) => piezas.push(json!({
                "pieza": "so", "modo": modo, "estado": "fallo", "detalle": e,
            })),
        }
    } else {
        match crate::antigravity_endpoint::aplicar_endpoint(puerto, confirmar) {
            Ok(v) => {
                let ya = v.get("ya_aplicado").and_then(|x| x.as_bool()).unwrap_or(false);
                piezas.push(json!({
                    "pieza": "so", "modo": modo,
                    "estado": if ya { "aplicado" } else { "aplicado_ahora" },
                    "detalle": v,
                }));
            }
            Err(e) => piezas.push(json!({
                "pieza": "so", "modo": modo, "estado": "fallo", "detalle": e,
            })),
        }
    }

    // --- 2) MegaDaemon como modelo -----------------------------------
    if on("modelo", true) {
        let (_, puente_caido) = estado_modelo();
        if puente_caido || !proceso_vivo("com.agy") {
            // La app no esta en marcha: el registro se hace por RPC contra
            // su language server, asi que se APLAZA (no es un fallo: el
            // ciclo automatico lo hara cuando arranque).
            piezas.push(json!({
                "pieza": "modelo", "estado": "aplazado",
                "detalle": "Antigravity no esta en marcha; se registrara cuando arranque",
            }));
        } else {
            match crate::antigravity_model::integrar(confirmar) {
                Ok(v) => piezas.push(json!({
                    "pieza": "modelo", "estado": "aplicado_ahora", "detalle": v,
                })),
                Err(e) => piezas.push(json!({
                    "pieza": "modelo", "estado": "fallo", "detalle": e,
                })),
            }
        }
    } else {
        piezas.push(json!({
            "pieza": "modelo", "estado": "omitido",
            "detalle": "la configuracion no pone a MegaDaemon como modelo",
        }));
    }

    // --- 3) puente MCP ------------------------------------------------
    if on("puente_mcp", true) {
        match sincronizar_puente_mcp() {
            Ok(v) => {
                let cambios = v.get("cambios").and_then(|x| x.as_u64()).unwrap_or(0);
                piezas.push(json!({
                    "pieza": "puente_mcp",
                    "estado": if cambios > 0 { "aplicado_ahora" } else { "aplicado" },
                    "detalle": v,
                }));
            }
            Err(e) => piezas.push(json!({
                "pieza": "puente_mcp", "estado": "fallo", "detalle": e,
            })),
        }
    } else {
        piezas.push(json!({
            "pieza": "puente_mcp", "estado": "omitido",
            "detalle": "la configuracion apaga el puente MCP",
        }));
    }

    // --- 4) MITM de la DB de conversaciones ---------------------------
    if on("mitm_db", false) {
        let ocultar: Vec<String> = crate::config_efectiva::vector_de("instrumentacion_agy.ocultar")
            .unwrap_or_default();
        let reescribir: Vec<(String, String)> = crate::config_efectiva::pares_de(
            "instrumentacion_agy.reescribir",
        )
        .unwrap_or_default()
        .into_iter()
        .collect();
        match crate::sqlite_mitm::activar(&ocultar, &reescribir) {
            Ok(v) => {
                let ya = v
                    .get("montaje")
                    .and_then(|m| m.get("montaje"))
                    .and_then(|x| x.as_str())
                    .map(|s| s.contains("bind"))
                    .unwrap_or(false);
                piezas.push(json!({
                    "pieza": "mitm_db", "estado": if ya { "aplicado" } else { "aplicado_ahora" },
                    "detalle": v,
                }));
            }
            Err(e) => piezas.push(json!({
                "pieza": "mitm_db", "estado": "fallo", "detalle": e,
            })),
        }
    } else {
        piezas.push(json!({
            "pieza": "mitm_db", "estado": "omitido",
            "detalle": "la configuracion no monta el MITM de la DB (por defecto NO: toca el fichero de la app)",
        }));
    }

    let fallos = piezas
        .iter()
        .filter(|p| p.get("estado").and_then(|e| e.as_str()) == Some("fallo"))
        .count();
    if let Some(o) = informe.as_object_mut() {
        o.insert("piezas".to_string(), json!(piezas));
        o.insert("fallos".to_string(), json!(fallos));
        o.insert("ok".to_string(), json!(fallos == 0));
    }
    let est = estado();
    guardar_estado(&est);
    if let Some(o) = informe.as_object_mut() {
        o.insert("estado".to_string(), est);
    }
    informe
}

/// **Plan**: qué se haría, sin hacerlo (para la UI: "qué va a pasar").
pub fn plan() -> Value {
    let modo = modo_so();
    json!({
        "ok": true,
        "modo_so": modo,
        "piezas": [
            {"pieza": "so", "modo": modo, "activo": modo != "ninguno",
             "que": "el trafico del language server pasa por el gateway de MegaDaemon"},
            {"pieza": "modelo", "activo": on("modelo", true),
             "que": "MegaDaemon aparece en el selector de modelos de Antigravity"},
            {"pieza": "puente_mcp", "activo": on("puente_mcp", true),
             "que": "el MCP de MegaDaemon queda disponible dentro de Antigravity"},
            {"pieza": "mitm_db", "activo": on("mitm_db", false),
             "que": "bind-mount de la DB de conversaciones (la original queda intacta)"},
        ],
        "estado": estado(),
    })
}

/// Definición de las tools MCP de la instrumentación.
pub fn tool_definitions() -> Value {
    json!([{
        "name": "instrumentacion_agy",
        "description": "Instrumentacion de Antigravity (com.agy) en el movil: el .so del language server apuntando al gateway local (falso, con el original en poder del daemon, o parcheado in situ), MegaDaemon registrado como modelo, el puente MCP dentro de Antigravity y el MITM (bind-mount) de la DB de conversaciones. Acciones: estado (que hay aplicado ahora), plan (que se haria), aplicar (aplica lo que diga la configuracion; es idempotente), restore (deja el .so original y desmonta el MITM).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "accion": {
                    "type": "string",
                    "description": "estado | plan | aplicar | restore",
                    "default": "estado"
                },
                "confirmar": {
                    "type": "boolean",
                    "description": "Confirmacion explicita para tocar el .so de la app (destructivo: se puede restaurar)",
                    "default": false
                },
                "puerto": {
                    "type": "integer",
                    "description": "Puerto local del gateway de MegaDaemon (defecto 9090)"
                }
            }
        }
    }])
}

/// Despacho de la tool MCP.
pub fn dispatch(args: &Value) -> Value {
    let accion = args
        .get("accion")
        .and_then(|v| v.as_str())
        .unwrap_or("estado")
        .to_string();
    let puerto = args
        .get("puerto")
        .and_then(|v| v.as_u64())
        .unwrap_or(PUERTO_GATEWAY as u64) as u16;
    let confirmar = args
        .get("confirmar")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    match accion.as_str() {
        "estado" => estado(),
        "plan" => plan(),
        "aplicar" => aplicar(puerto, confirmar),
        "restore" => {
            let a = crate::antigravity_fake_so::desactivar(confirmar);
            let b = crate::sqlite_mitm::desactivar();
            json!({
                "ok": true,
                "so": a,
                "mitm": b,
                "estado": estado(),
            })
        }
        other => json!({"ok": false, "error": format!("accion desconocida: {other} (estado|plan|aplicar|restore)")}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_estado_no_rompe_sin_antigravity() {
        // En el PC no hay Antigravity: el estado debe seguir siendo JSON
        // valido (es lo que la UI pinta).
        let e = estado();
        assert_eq!(e.get("ok").and_then(|v| v.as_bool()), Some(true));
        assert!(e.get("piezas").is_none()); // el estado no tiene piezas
        assert!(e.get("config").is_some());
    }

    #[test]
    fn el_plan_lista_las_cuatro_piezas() {
        let p = plan();
        let piezas = p.get("piezas").and_then(|v| v.as_array()).unwrap();
        assert_eq!(piezas.len(), 4);
        let nombres: Vec<&str> = piezas
            .iter()
            .filter_map(|p| p.get("pieza").and_then(|v| v.as_str()))
            .collect();
        assert!(nombres.contains(&"so"));
        assert!(nombres.contains(&"modelo"));
        assert!(nombres.contains(&"puente_mcp"));
        assert!(nombres.contains(&"mitm_db"));
    }

    #[test]
    fn una_accion_desconocida_no_panica() {
        let v = dispatch(&json!({"accion": "inventada"}));
        assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
    }
}

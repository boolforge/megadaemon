//! MegaDaemon — **lector de logs**.
//!
//! EL PROBLEMA
//! -----------
//! El daemon escribía en 4 sitios distintos (`/sdcard/megadaemon.log`,
//! `state/auto_update.log`, `prefix/tunnel.log`, journals NDJSON) y **ninguno
//! era visible** desde la Web UI.  Cuando algo fallaba en el móvil, el único
//! modo de saberlo era un `root_exec` a mano.
//!
//! QUÉ HACE
//! ---------
//! Un catálogo **único** de fuentes de log, con:
//!
//! * `listar()` ⇒ qué hay, cuánto pesa, cuándo se escribió.
//! * `tail()` ⇒ las últimas N líneas, con filtro de texto y nivel.
//! * soporte de NDJSON (cada línea se parsea como evento ⇒ la UI puede
//!   pintar eventos estructurados, no sólo texto).
//!
//! BARATO
//! ------
//! Lee **sólo el final** del fichero (busca el último salto de línea hacia
//! atrás), así que da igual que el log del móvil pese 50 MB: no se carga en
//! memoria.

use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

/// Una fuente de log.
pub struct Fuente {
    pub id: &'static str,
    pub nombre: &'static str,
    pub tipo: &'static str, // "texto" | "ndjson"
    pub ruta: PathBuf,
}

fn prefijo() -> String {
    std::env::var("MEGADAEMON_PREFIX").unwrap_or_else(|_| "/data/adb/megadaemon".to_string())
}

fn sdcard() -> PathBuf {
    PathBuf::from("/sdcard")
}

/// Catálogo de fuentes: una sola lista, la UI la pinta entera.
pub fn fuentes() -> Vec<Fuente> {
    let estado = crate::cockpit_server::state_dir();
    let p = prefijo();
    vec![
        Fuente {
            id: "daemon",
            nombre: "Daemon (stdout del supervisor)",
            tipo: "texto",
            ruta: PathBuf::from(format!("{sdcard():?}/megadaemon.log")),
        },
        Fuente {
            id: "auto_update",
            nombre: "Auto-actualización",
            tipo: "texto",
            ruta: PathBuf::from(format!("{estado}/auto_update.log")),
        },
        Fuente {
            id: "tunnel",
            nombre: "Túnel web (Cloudflare)",
            tipo: "texto",
            ruta: PathBuf::from(format!("{p}/tunnel.log")),
        },
        Fuente {
            id: "mcp_tunnel",
            nombre: "Túnel MCP",
            tipo: "texto",
            ruta: PathBuf::from(format!("{p}/mcp_tunnel.log")),
        },
        Fuente {
            id: "bridge",
            nombre: "Bridge web (Python legacy)",
            tipo: "texto",
            ruta: PathBuf::from(format!("{p}/bridge.log")),
        },
        Fuente {
            id: "mitm",
            nombre: "MITM de la DB de Antigravity",
            tipo: "texto",
            ruta: PathBuf::from(format!("{p}/mitm_sqlite.log")),
        },
        Fuente {
            id: "supervisor",
            nombre: "Supervisor (service.d)",
            tipo: "texto",
            ruta: PathBuf::from("/data/adb/service.d/99-megadaemon.sh.log"),
        },
        Fuente {
            id: "automation",
            nombre: "Automatizaciones (journal)",
            tipo: "ndjson",
            ruta: PathBuf::from(format!("{estado}/automation_journal.ndjson")),
        },
        Fuente {
            id: "comms",
            nombre: "Comunicaciones (journal)",
            tipo: "ndjson",
            ruta: PathBuf::from(format!("{estado}/comms_journal.ndjson")),
        },
        Fuente {
            id: "ssh",
            nombre: "SSH (auditoría)",
            tipo: "ndjson",
            ruta: PathBuf::from(format!("{estado}/ssh_audit.ndjson")),
        },
        Fuente {
            id: "mesh",
            nombre: "Mesh (journal)",
            tipo: "ndjson",
            ruta: PathBuf::from(format!("{estado}/mesh_journal.ndjson")),
        },
        Fuente {
            id: "fallos",
            nombre: "Fallos del agente",
            tipo: "ndjson",
            ruta: PathBuf::from(format!("{estado}/failures.ndjson")),
        },
        Fuente {
            id: "agyshim",
            nombre: "Shim de Antigravity (traces)",
            tipo: "ndjson",
            ruta: PathBuf::from(format!("{estado}/agyshim.ndjson")),
        },
        Fuente {
            id: "instrumentacion",
            nombre: "Instrumentación AGY (estado)",
            tipo: "texto",
            ruta: crate::instrumentacion_agy::ruta_estado(),
        },
    ]
}

/// Busca una fuente por id.
pub fn buscar(id: &str) -> Option<Fuente> {
    fuentes().into_iter().find(|f| f.id == id)
}

/// **Listado** de fuentes con su estado real en disco.
pub fn listar() -> Value {
    let mut out = Vec::new();
    for f in fuentes() {
        let meta = std::fs::metadata(&f.ruta).ok();
        let (existe, tam, mtime) = match &meta {
            Some(m) => (
                true,
                m.len(),
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            ),
            None => (false, 0u64, 0u64),
        };
        out.push(json!({
            "id": f.id,
            "nombre": f.nombre,
            "tipo": f.tipo,
            "ruta": f.ruta.to_string_lossy(),
            "existe": existe,
            "bytes": tam,
            "mtime": mtime,
        }));
    }
    json!({ "ok": true, "fuentes": out })
}

/// Lee las últimas `lineas` del final de un fichero sin cargarlo entero.
///
/// Estrategia: se leen los últimos 64 KB (o el fichero entero si es menor) y
/// se parte por líneas.  Si no hay suficientes líneas, se amplía el bloque
/// (hasta 1 MB) y se reintenta.
fn leer_final(ruta: &PathBuf, lineas: usize) -> Result<Vec<String>, String> {
    let mut f = std::fs::File::open(ruta).map_err(|e| e.to_string())?;
    let tam = f.metadata().map_err(|e| e.to_string())?.len();
    let mut bloque = 64 * 1024u64;
    loop {
        let inicio = tam.saturating_sub(bloque);
        f.seek(SeekFrom::Start(inicio)).map_err(|e| e.to_string())?;
        let mut buf = Vec::with_capacity(bloque as usize);
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        let texto = String::from_utf8_lossy(&buf);
        let mut l: Vec<&str> = texto.lines().collect();
        // Si el bloque empieza en mitad de una línea, la primera es parcial.
        if inicio > 0 && !l.is_empty() {
            l.remove(0);
        }
        if l.len() > lineas || bloque >= 1024 * 1024 {
            let ini = l.len().saturating_sub(lineas);
            return Ok(l[ini..].iter().map(|s| s.to_string()).collect());
        }
        bloque *= 4;
    }
}

/// Niveles reconocidos en las líneas de log.
fn nivel(linea: &str) -> Option<&'static str> {
    let l = linea.to_ascii_uppercase();
    for n in ["ERROR", "FATAL", "PANIC", "WARN", "WARNING", "INFO", "DEBUG", "TRACE"] {
        if l.contains(n) {
            return Some(match n {
                "ERROR" | "FATAL" | "PANIC" => "error",
                "WARN" | "WARNING" => "warn",
                "INFO" => "info",
                _ => "debug",
            });
        }
    }
    None
}

/// **Tail** de una fuente.
///
/// * `lineas`: cuántas líneas devolver (por defecto 100, máx 2000).
/// * `filtro`: subcadena (sin distinguir mayúsculas) — vacío = todas.
/// * `nivel`: filtro por nivel (`error`/`warn`/`info`/`debug`).
/// * `contiene_error`: sólo líneas de error (atajo).
pub fn tail(id: &str, lineas: usize, filtro: &str, nivel_f: &str, desde: usize) -> Value {
    let Some(f) = buscar(id) else {
        return json!({"ok": false, "error": format!("fuente de log desconocida: {id}")});
    };
    if !f.ruta.exists() {
        return json!({
            "ok": true, "id": id, "ruta": f.ruta.to_string_lossy(),
            "existe": false, "lineas": [], "total": 0,
            "aviso": "el fichero no existe todavia",
        });
    }
    let n = lineas.clamp(1, 2000);
    let todas = match leer_final(&f.ruta, n * 4) {
        Ok(v) => v,
        Err(e) => {
            return json!({"ok": false, "id": id, "error": format!("leer {}: {e}", f.ruta.display())})
        }
    };
    let filtro_l = filtro.to_lowercase();
    let nivel_deseado = if nivel_f.is_empty() { None } else { Some(nivel_f) };
    let mut out = Vec::new();
    let mut total = 0usize;
    for (i, l) in todas.iter().enumerate() {
        if i < desde {
            continue;
        }
        let lvl = nivel(l);
        let pasa_texto = filtro_l.is_empty() || l.to_lowercase().contains(&filtro_l);
        let pasa_nivel = match nivel_deseado {
            None => true,
            Some(nd) => lvl.map(|x| x == nd).unwrap_or(false),
        };
        if !pasa_texto || !pasa_nivel {
            continue;
        }
        total += 1;
        let mut item = json!({"n": i, "texto": l, "nivel": lvl.unwrap_or("info")});
        if f.tipo == "ndjson" {
            if let Ok(v) = serde_json::from_str::<Value>(l) {
                item["evento"] = v;
            }
        }
        out.push(item);
        if out.len() >= n {
            break;
        }
    }
    json!({
        "ok": true, "id": id, "ruta": f.ruta.to_string_lossy(), "existe": true,
        "lineas": out, "total": total, "pedidas": n,
    })
}

/// Resumen: cuántas líneas de error hay por fuente (para la vista de salud).
pub fn resumen() -> Value {
    let mut out = Vec::new();
    for f in fuentes() {
        if !f.ruta.exists() {
            continue;
        }
        let lineas = leer_final(&f.ruta, 400).unwrap_or_default();
        let err = lineas
            .iter()
            .filter(|l| matches!(nivel(l), Some("error")))
            .count();
        let warn = lineas
            .iter()
            .filter(|l| matches!(nivel(l), Some("warn")))
            .count();
        out.push(json!({
            "id": f.id, "nombre": f.nombre, "errores": err, "avisos": warn,
            "lineas_muestreadas": lineas.len(),
        }));
    }
    json!({ "ok": true, "resumen": out })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_catalogo_tiene_fuentes_conocidas() {
        let f = fuentes();
        assert!(f.len() >= 10);
        assert!(f.iter().any(|x| x.id == "daemon"));
        assert!(f.iter().any(|x| x.id == "auto_update"));
        assert!(f.iter().any(|x| x.id == "automation"));
    }

    #[test]
    fn listar_nunca_falla() {
        let v = listar();
        assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(true));
        assert!(v.get("fuentes").and_then(|x| x.as_array()).is_some());
    }

    #[test]
    fn una_fuente_inexistente_no_es_un_error() {
        let v = tail("no-existe", 10, "", "", 0);
        assert_eq!(v.get("ok").and_then(|x| x.as_bool()), Some(false));
        let v2 = tail("daemon", 10, "", "", 0);
        // el daemon puede no existir: debe devolver algo valido igual
        assert!(v2.get("ok").is_some());
    }

    #[test]
    fn el_nivel_se_detecta() {
        assert_eq!(nivel("ERROR: fallo"), Some("error"));
        assert_eq!(nivel("WARN aviso"), Some("warn"));
        assert_eq!(nivel("todo bien"), None);
    }
}

//! Sincronización de la cola de tareas entre nodos (vía Drive).
//!
//! PROBLEMA (2026-11-22): el móvil, el PC y cada agente trabajaban con su
//! cola local ⇒ trabajo duplicado o perdido.  Aquí la cola se **sincroniza**
//! por la carpeta privada de Drive:
//!
//! 1. cada nodo sube su cola como `tasks-<nodo>-<ts>.json`;
//! 2. lista los ficheros `tasks-*.json` de los demás nodos;
//! 3. los descarga y **fusiona por id** (idempotente: importar dos veces
//!    la misma tarea no la duplica);
//! 4. devuelve el resumen (subidos, importados, conflictos).
//!
//! La fusión es por id y gana el estado más avanzado
//! (running > pending > paused > done), de modo que un nodo que acaba de
//! terminar una tarea no la revierte.

use serde_json::{json, Value};
use std::path::PathBuf;

/// Carpeta privada donde vive la cola sincronizada.
const CARPETA_BACKUP: &str = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0";
/// Subcarpeta de la cola dentro de la carpeta privada.
const SUBCARPETA: &str = "cola-tareas";

fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn nodo_actual() -> String {
    std::env::var("MEGADAEMON_NODE")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            std::env::var("HOSTNAME")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "pc".to_string())
        })
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Sube la cola local y trae la de los demás nodos.
///
/// Devuelve un JSON con lo que ha pasado (para el agente y el log).
pub fn sincronizar_drive() -> Result<Value, String> {
    let state = crate::cockpit_server::state_dir();
    let nodo = nodo_actual();

    // 1) Carpeta de la cola (creada si no existe).
    let carpeta = crate::drive_native::ensure_folder(SUBCARPETA, CARPETA_BACKUP, Some(&state))?;

    // 2) Exportar y subir la cola local.
    let export = crate::task_scheduler::export_all_tasks(&nodo);
    let total = export["tasks"].as_array().map(|a| a.len()).unwrap_or(0);
    let fichero = PathBuf::from(&state).join(format!("tasks-{nodo}-{}.json", ahora()));
    let txt = serde_json::to_string_pretty(&export).unwrap_or_default();
    std::fs::write(&fichero, txt.as_bytes()).map_err(|e| format!("escribir local: {e}"))?;
    let up = crate::drive_native::upload_file(
        &fichero,
        &format!("tasks-{nodo}-{}.json", ahora()),
        &crate::drive_client::DriveTarget::folder(&carpeta),
        Some(&state),
    );
    let subido = match up {
        Ok(u) => json!({"ok": true, "id": u.file_id, "bytes": u.bytes, "sha256": u.sha256}),
        Err(e) => json!({"ok": false, "error": e}),
    };

    // 3) Listar los `tasks-*.json` de los demás nodos.
    let q = format!("'{carpeta}' in parents and name contains 'tasks-' and trashed = false");
    let url = format!(
        "{}?q={}&fields=files(id,name)&pageSize=200&supportsAllDrives=true&includeItemsFromAllDrives=true",
        crate::drive_client::DRIVE_API,
        urlencode(&q)
    );
    let (ak, av) = crate::google_auth::auth_header(Some(&state))?;
    let resp = crate::http_transfer::get_texto(
        &url,
        &{
            let mut o = crate::http_transfer::Opciones::default();
            o.cabeceras.push((ak, av));
            o
        },
    )
    .map_err(|e| format!("listar cola en Drive: {e}"))?;
    let lista: Value = serde_json::from_str(&resp.1).unwrap_or(Value::Null);

    // 4) Descargar e importar (fusión por id).
    let mut importadas = 0usize;
    let mut actualizadas = 0usize;
    let mut descargas = Vec::new();
    for f in lista["files"].as_array().cloned().unwrap_or_default() {
        let id = f["id"].as_str().unwrap_or("").to_string();
        let name = f["name"].as_str().unwrap_or("").to_string();
        if id.is_empty() || name.starts_with(&format!("tasks-{nodo}-")) {
            continue;
        }
        let dest = PathBuf::from(&state).join(format!("sync-{name}"));
        match crate::drive_native::download_file(&id, &dest, Some(&state)) {
            Ok(b) => {
                if let Ok(txt) = std::fs::read_to_string(&dest) {
                    if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                        let (i, u) = crate::task_scheduler::merge_tasks(&v);
                        importadas += i;
                        actualizadas += u;
                    }
                }
                descargas.push(json!({"nombre": name, "bytes": b}));
            }
            Err(e) => descargas.push(json!({"nombre": name, "error": e})),
        }
    }

    Ok(json!({
        "ok": true,
        "nodo": nodo,
        "carpeta": carpeta,
        "tareas_locales": total,
        "subida": subido,
        "descargadas": descargas.len(),
        "importadas": importadas,
        "actualizadas": actualizadas,
        "cola_total": crate::task_scheduler::list().len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::urlencode;

    #[test]
    fn urlencode_escapa_lo_que_hace_falta() {
        assert_eq!(urlencode("a b"), "a%20b");
        assert_eq!(urlencode("tasks-1.json"), "tasks-1.json");
    }
}

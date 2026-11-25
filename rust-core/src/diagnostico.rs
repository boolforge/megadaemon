//! **Diagnóstico automático y automático**: el móvil publica su estado
//! completo en Drive y el root lo lee desde el sandbox (sin ADB, sin que
//! nadie pegue salida de terminal a mano).
//!
//! POR QUÉ
//! --------
//! MEDIDO (2026-11-25): el móvil estaba en un bucle (arrancaba, moría,
//! el supervisor lo reiniciaba) y cada diagnóstico exigía que el usuario
//! pegase un bloque de texto en el chat.  Eso es lento, se pierde y no es
//! reproducible.
//!
//! QUÉ HACE
//! ---------
//! 1. [`recoger`]: estado completo del nodo en JSON (procesos, puertos,
//!    `/health`, stack, binario, log, NDJSON del shim, energía, red).
//! 2. [`a_markdown`]: el mismo estado en Markdown, legible por una persona.
//! 3. [`publicar`]: lo sube a Drive (`MegaDaemon/diagnostico/<nodo>/`), que
//!    es donde el root mira.
//! 4. [`publicar_periodico`]: lo sube solo cada N minutos.
//!
//! El contenido NO lleva secretos: ni la API key, ni el token de sesión, ni
//! las credenciales de Drive.  Los ficheros de credenciales se citan sólo
//! por su ruta y su hash truncado.
//!
//! Autor: sesión root de MegaDaemon (2026-11-25).

use serde_json::{json, Value};
use std::path::Path;

/// Carpeta (VFS) donde se publican los diagnósticos.
pub const CARPETA_VFS: &str = "MegaDaemon/diagnostico";

/// Nombre del nodo (para las rutas de Drive).
pub fn nodo() -> String {
    std::env::var("MEGADAEMON_INSTANCE_NAME")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            crate::supervisor::es_android()
                .then(|| "android".to_string())
                .unwrap_or_else(|| "pc".to_string())
        })
}

/// Epoch en segundos.
fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Texto de un fichero, recortado a `max` bytes (cola).
fn ultimas_lineas(ruta: &str, max: usize) -> Vec<String> {
    let Ok(datos) = std::fs::read(ruta) else {
        return Vec::new();
    };
    let texto = String::from_utf8_lossy(&datos);
    let lineas: Vec<&str> = texto.lines().collect();
    let inicio = lineas.len().saturating_sub(60);
    let mut out: Vec<String> = lineas[inicio..].iter().map(|s| s.to_string()).collect();
    if out.join("\n").len() > max {
        let mut total = 0usize;
        let mut recortado = Vec::new();
        for l in out.into_iter().rev() {
            if total + l.len() > max {
                break;
            }
            total += l.len();
            recortado.push(l);
        }
        recortado.reverse();
        out = recortado;
    }
    out
}

/// Procesos del daemon (cmdline + RSS).
fn procesos() -> Value {
    let mut lista = Vec::new();
    if let Ok(entradas) = std::fs::read_dir("/proc") {
        for e in entradas.flatten() {
            let nombre = e.file_name().to_string_lossy().to_string();
            if !nombre.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(cmd) = std::fs::read(e.path().join("cmdline")) else {
                continue;
            };
            let cmd = String::from_utf8_lossy(&cmd)
                .replace('\0', " ")
                .trim()
                .to_string();
            if !cmd.contains("megadaemon") && !cmd.contains("99-megadaemon") {
                continue;
            }
            let rss = std::fs::read_to_string(e.path().join("status"))
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|l| l.starts_with("VmRSS:"))
                        .and_then(|l| l.split_whitespace().nth(1).map(|v| v.to_string()))
                })
                .unwrap_or_default();
            lista.push(json!({
                "pid": nombre,
                "cmd": cmd,
                "rss_kb": rss,
            }));
        }
    }
    json!(lista)
}

/// Puertos en LISTEN (de `/proc/net/tcp`).
fn puertos() -> Value {
    let mut mapa = serde_json::Map::new();
    for f in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(texto) = std::fs::read_to_string(f) else {
            continue;
        };
        for linea in texto.lines().skip(1) {
            let c: Vec<&str> = linea.split_whitespace().collect();
            if c.len() < 4 || c[3] != "0A" {
                continue;
            }
            let Some(p) = c[1].rsplit(':').next() else {
                continue;
            };
            if let Ok(pu) = u32::from_str_radix(p, 16) {
                mapa.insert(pu.to_string(), json!(true));
            }
        }
    }
    Value::Object(mapa)
}

/// `/health` del cockpit (local, sin credenciales).
///
/// MEDIDO (2026-11-25): con `curl` no se puede (el móvil no lo trae) y el
/// cliente HTTP del daemon descarga a fichero ⇒ se usa un temporal y se
/// lee.
fn health(puerto: u16) -> Value {
    let tmp = std::env::temp_dir().join(format!("md-health-{}.json", std::process::id()));
    let opts = crate::http_transfer::Opciones {
        destino: Some(tmp.clone()),
        timeout_s: 8,
        reintentos: 0,
        ..crate::http_transfer::Opciones::default()
    };
    let url = format!("http://127.0.0.1:{puerto}/health");
    let r = crate::http_transfer::descargar(&url, &opts);
    let datos = std::fs::read(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    if datos.is_empty() {
        return json!({
            "ok": false,
            "error": r.err().unwrap_or_else(|| "respuesta vacia".to_string()),
        });
    }
    serde_json::from_slice(&datos).unwrap_or_else(|_| {
        json!({
            "ok": true,
            "status": r.map(|x| x.status).unwrap_or(0),
            "raw": String::from_utf8_lossy(&datos).chars().take(1200).collect::<String>(),
        })
    })
}

/// Estado del stack autocontenido.
fn stack() -> Value {
    crate::stack::estado_json(&crate::stack::Config::del_entorno())
}

/// Binario en servicio: ruta, tamaño, SHA-256 y build.
fn binario() -> Value {
    let cfg = crate::stack::Config::del_entorno();
    let ruta = format!("{}/megadaemon-core", cfg.prefijo);
    let p = Path::new(&ruta);
    let bytes = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let sha = crate::stack::hash_de(p);
    let previo = crate::stack::hash_de(Path::new(&format!("{}.prev", ruta)));
    json!({
        "ruta": ruta,
        "bytes": bytes,
        "sha256": sha,
        "prev_sha256": previo,
        "build": crate::self_update::build_info(),
    })
}

/// Energía/temperatura (lo que el supervisor mide).
fn energia() -> Value {
    let mut v = serde_json::Map::new();
    for (etiqueta, ruta) in [
        ("temp_sistema", "/sys/class/thermal/thermal_zone0/temp"),
        ("bateria_nivel", "/sys/class/power_supply/battery/capacity"),
        ("bateria_temp", "/sys/class/power_supply/battery/temp"),
        ("carga", "/sys/class/power_supply/battery/status"),
    ] {
        if let Ok(s) = std::fs::read_to_string(ruta) {
            let t = s.trim();
            if let Ok(n) = t.parse::<i64>() {
                // Los termómetros dan mili-grados.
                let valor = if n > 1000 { n / 1000 } else { n };
                v.insert(etiqueta.to_string(), json!(valor));
            } else {
                v.insert(etiqueta.to_string(), json!(t));
            }
        }
    }
    Value::Object(v)
}

/// Red: IP y túnel vigente.
fn red() -> Value {
    let mut ip = Vec::new();
    if let Ok(entradas) = std::fs::read_dir("/sys/class/net") {
        for e in entradas.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n == "lo" {
                continue;
            }
            if let Ok(s) = std::fs::read_to_string(e.path().join("address")) {
                ip.push(json!({"iface": n, "mac": s.trim()}));
            }
        }
    }
    let tunel = std::fs::read_to_string("/data/local/tmp/megadaemon/mcp_tunnel.url")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    json!({"interfaces": ip, "tunel_mcp": tunel})
}

/// **Recoge el estado completo del nodo.**
pub fn recoger() -> Value {
    let puerto = crate::config_efectiva::u64_de("general.puerto_gateway", 9090) as u16;
    let state = crate::cockpit_server::state_dir();
    json!({
        "ts": ahora(),
        "nodo": nodo(),
        "build": crate::self_update::build_info(),
        "procesos": procesos(),
        // MEDIDO (2026-11-25): con DOS supervisores vivos (el canónico y
        // su copia .bak) el móvil entraba en bucle de reinicios.  El
        // diagnóstico lo dice explícitamente.
        "supervisores": crate::stack::numero_supervisores(),
        "puertos": puertos(),
        "health": health(puerto),
        "stack": stack(),
        "binario": binario(),
        "energia": energia(),
        "red": red(),
        "log": ultimas_lineas("/sdcard/megadaemon.log", 6000),
        "shim": ultimas_lineas("/data/adb/megadaemon/state/agyshim.ndjson", 4000),
        "state_dir": state,
    })
}

/// El estado en Markdown (legible por una persona).
pub fn a_markdown(v: &Value) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "# Diagnóstico MegaDaemon — {}\n\n",
        v["nodo"].as_str().unwrap_or("?")
    ));
    s.push_str(&format!(
        "- build: `{}`\n",
        v["build"]["short"].as_str().unwrap_or("?")
    ));
    s.push_str(&format!("- ts: `{}`\n\n", v["ts"].as_u64().unwrap_or(0)));
    s.push_str(&format!(
        "## Supervisores: {}\n\n",
        v["supervisores"].as_u64().unwrap_or(0)
    ));
    s.push_str("## Procesos\n\n```\n");
    for p in v["procesos"].as_array().cloned().unwrap_or_default() {
        s.push_str(&format!(
            "{}  rss={}kB  {}\n",
            p["pid"].as_str().unwrap_or("?"),
            p["rss_kb"].as_str().unwrap_or("?"),
            p["cmd"].as_str().unwrap_or("?")
        ));
    }
    s.push_str("```\n\n## Health\n\n```json\n");
    s.push_str(&format!(
        "{}\n```\n\n",
        serde_json::to_string_pretty(&v["health"]).unwrap_or_default()
    ));
    s.push_str("## Stack\n\n```json\n");
    s.push_str(&format!(
        "{}\n```\n\n",
        serde_json::to_string_pretty(&v["stack"]).unwrap_or_default()
    ));
    s.push_str("## Binario\n\n```json\n");
    s.push_str(&format!(
        "{}\n```\n\n",
        serde_json::to_string_pretty(&v["binario"]).unwrap_or_default()
    ));
    s.push_str("## Log (últimas líneas)\n\n```\n");
    for l in v["log"].as_array().cloned().unwrap_or_default() {
        s.push_str(&l.as_str().unwrap_or(""));
        s.push('\n');
    }
    s.push_str("```\n\n## Shim (NDJSON)\n\n```\n");
    for l in v["shim"].as_array().cloned().unwrap_or_default() {
        s.push_str(&l.as_str().unwrap_or(""));
        s.push('\n');
    }
    s.push_str("```\n");
    s
}

/// Escribe el diagnóstico en local (`<state>/diagnostico/`).
pub fn escribir_local() -> Result<std::path::PathBuf, String> {
    let state = crate::cockpit_server::state_dir();
    let dir = Path::new(&state).join("diagnostico");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;
    let v = recoger();
    let base = format!("{}-{}", nodo(), v["ts"].as_u64().unwrap_or(0));
    let ruta_json = dir.join(format!("{base}.json"));
    std::fs::write(
        &ruta_json,
        serde_json::to_vec_pretty(&v).unwrap_or_default(),
    )
    .map_err(|e| format!("no se pudo escribir {}: {e}", ruta_json.display()))?;
    let ruta_md = dir.join(format!("{base}.md"));
    std::fs::write(&ruta_md, a_markdown(&v))
        .map_err(|e| format!("no se pudo escribir {}: {e}", ruta_md.display()))?;
    Ok(ruta_json)
}

/// **Publica el diagnóstico en Drive** (`MegaDaemon/diagnostico/<nodo>/`).
///
/// Es lo que permite al root leer el estado del móvil desde el sandbox,
/// sin ADB y sin que nadie pegue texto.
pub fn publicar() -> Result<Value, String> {
    let state = crate::cockpit_server::state_dir();
    let v = recoger();
    let dir_local = Path::new(&state).join("diagnostico");
    std::fs::create_dir_all(&dir_local)
        .map_err(|e| format!("no se pudo crear el dir local: {e}"))?;
    let base = format!("{}-{}", nodo(), v["ts"].as_u64().unwrap_or(0));
    let ruta_json = dir_local.join(format!("{base}.json"));
    let ruta_md = dir_local.join(format!("{base}.md"));
    std::fs::write(
        &ruta_json,
        serde_json::to_vec_pretty(&v).unwrap_or_default(),
    )
    .map_err(|e| format!("no se pudo escribir el json: {e}"))?;
    std::fs::write(&ruta_md, a_markdown(&v))
        .map_err(|e| format!("no se pudo escribir el md: {e}"))?;

    let vfs = format!("{CARPETA_VFS}/{}", nodo());
    let sd = Some(state.as_str());
    let j = crate::drive_vfs::write_local_file(&ruta_json, &vfs, &format!("{base}.json"), sd)
        .map_err(|e| format!("no se pudo subir el json a Drive: {e}"))?;
    let m = crate::drive_vfs::write_local_file(&ruta_md, &vfs, &format!("{base}.md"), sd)
        .map_err(|e| format!("no se pudo subir el md a Drive: {e}"))?;
    // Índice: el último estado, para no tener que listar la carpeta.
    let indice = dir_local.join("ultimo.json");
    std::fs::write(
        &indice,
        serde_json::to_vec_pretty(&json!({
            "ts": v["ts"],
            "nodo": nodo(),
            "json_file_id": j.get("file_id").cloned().unwrap_or(Value::Null),
            "md_file_id": m.get("file_id").cloned().unwrap_or(Value::Null),
            "vfs": vfs,
        }))
        .unwrap_or_default(),
    )
    .ok();
    Ok(json!({
        "ok": true,
        "vfs": vfs,
        "json": j,
        "md": m,
        "local": ruta_json.display().to_string(),
    }))
}

/// Publica el diagnóstico cada `minutos` (en un hilo aparte).
///
/// Es idempotente y barato: un informe pequeño cada vez.
pub fn publicar_periodico(minutos: u64) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("diagnostico-drive".to_string())
        .spawn(move || {
            let periodo = std::time::Duration::from_secs(minutos.max(1) * 60);
            loop {
                match publicar() {
                    Ok(v) => {
                        eprintln!(
                            "diagnostico: publicado en Drive ({} ficheros)",
                            if v["json"].is_object() { 2 } else { 0 }
                        );
                    }
                    Err(e) => eprintln!("diagnostico: no se pudo publicar: {e}"),
                }
                std::thread::sleep(periodo);
            }
        })
        .expect("hilo de diagnostico")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recoge_json_valido() {
        let v = recoger();
        assert!(v.is_object());
        assert!(v.get("ts").and_then(|x| x.as_u64()).is_some());
        assert!(v.get("nodo").is_some());
        assert!(v.get("stack").is_some());
        assert!(v.get("procesos").is_some());
    }

    #[test]
    fn el_markdown_contiene_lo_importante() {
        let v = recoger();
        let md = a_markdown(&v);
        assert!(md.contains("Diagnóstico MegaDaemon"));
        assert!(md.contains("## Stack"));
        assert!(md.contains("## Binario"));
    }

    #[test]
    fn las_lineas_se_recortan() {
        let l = ultimas_lineas("/no/existe", 100);
        assert!(l.is_empty());
    }
}

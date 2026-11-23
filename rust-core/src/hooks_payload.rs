//! `hooks_payload` — gestión del **payload de instrumentación** (frida-gum)
//! dentro del móvil.
//!
//! QUÉ ES ESTO
//! -----------
//! El payload es una `cdylib` (crate `crates/hooks-payload`) que se compila
//! para `aarch64-linux-android` con los **bindings oficiales de Frida**
//! (`frida-gum`).  Al cargarse en el proceso objetivo instala hooks de
//! SQLite, red y ficheros, y escribe **una línea JSON por evento** (NDJSON)
//! en `/data/local/tmp/megadaemon/state/hooks.ndjson`.
//!
//! Este módulo es el **lado daemon**: NO lleva gum (el binario principal no
//! puede depender de gum, bindgen ni del devkit de Frida).  Desde aquí se:
//!
//! * **instala** el payload (descargando el asset del release y verificando
//!   su SHA-256),
//! * **activa** la carga en el proceso objetivo (invocando el cargador que
//!   haya en el móvil: `frida-server` + cliente `frida`, o un módulo
//!   Zygisk) por `root_exec`,
//! * **lee** las últimas líneas del NDJSON.
//!
//! TODO lo que toca el dispositivo exige `root` (el móvil tiene Magisk) y se
//! hace con `android::root_exec` o con `su`.

use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Dónde se instala el payload en el móvil.
pub const RUTA_PAYLOAD: &str = "/data/local/tmp/megadaemon/hooks-payload.so";

/// Nombre del asset en el release (el `.sha256` es el mismo + `.sha256`).
pub const ASSET_PAYLOAD: &str = "hooks-payload-arm64";

/// NDJSON canónico (el que escribe el payload).
pub const RUTA_NDJSON: &str = "/data/local/tmp/megadaemon/state/hooks.ndjson";

/// Paquete objetivo por defecto (Antigravity en el Samsung: `com.agy`).
pub const PAQUETE_OBJETIVO: &str = "com.agy";

/// Script JS que carga el .so dentro del proceso (con el cliente de Frida).
pub const RUTA_CARGADOR_JS: &str = "/data/local/tmp/megadaemon/load-hooks-payload.js";

/// El cargador JS.  `Module.load(path)` es la API oficial de Frida para
/// cargar una biblioteca nativa desde una ruta (documentada en
/// <https://frida.re/docs/javascript-api/>).
pub const JS_CARGADOR: &str = r#"
// load-hooks-payload.js — carga el payload nativo (frida-gum) en el proceso.
// API de Frida: Module.load(path) carga la .so y devuelve el Module.
const so = '__SO__';
try {
  const m = Module.load(so);
  send({ ok: true, path: so, base: m.base.toString(), size: m.size });
} catch (e) {
  send({ ok: false, error: String(e) });
}
"#;

/// ¿Estamos en Android (el móvil)?
pub fn es_android() -> bool {
    cfg!(target_os = "android")
}

/// Paquete objetivo de la petición.
fn paquete_de(args: &Value) -> String {
    args.get("paquete")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| PAQUETE_OBJETIVO.to_string())
}

/// SHA-256 de un fichero (hex).
fn sha_de_fichero(ruta: &Path) -> Option<String> {
    let datos = std::fs::read(ruta).ok()?;
    Some(crate::self_update::sha256_hex(&datos))
}

/// Modo de permisos de un fichero (`0644`).
fn modo_de(ruta: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(ruta)
        .ok()
        .map(|m| m.permissions().mode() & 0o7777)
}

/// Rutas donde puede estar el NDJSON (las mismas que prueba el payload).
///
/// La primera es la canónica; las demás son las alternativas que el payload
/// usa cuando el proceso de la app no puede escribir en `/data/local/tmp`.
pub fn candidatas_ndjson() -> Vec<String> {
    let mut v: Vec<String> = vec![RUTA_NDJSON.to_string()];
    let dir = Path::new("/data/local/tmp/megadaemon/state");
    if let Ok(entradas) = std::fs::read_dir(dir) {
        let mut extras: Vec<String> = entradas
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with("hooks") && n.ends_with(".ndjson"))
            .map(|n| format!("{}/{}", dir.display(), n))
            .collect();
        extras.sort();
        v.extend(extras);
    }
    for base in ["/data/user/0/com.agy/cache", "/data/data/com.agy/cache"] {
        v.push(format!("{base}/megadaemon-hooks.ndjson"));
    }
    v
}

/// El NDJSON que existe ahora mismo (si hay alguno).
fn ndjson_existente() -> Option<(String, std::fs::Metadata)> {
    for r in candidatas_ndjson() {
        if let Ok(m) = std::fs::metadata(&r) {
            return Some((r, m));
        }
    }
    None
}

/// Últimas `n` líneas de un fichero (lee sólo la cola: el NDJSON puede ser
/// grande).
fn ultimas_lineas(ruta: &Path, n: usize) -> Result<Vec<String>, String> {
    let mut f = std::fs::File::open(ruta).map_err(|e| format!("abrir {}: {e}", ruta.display()))?;
    let size = f.metadata().map_err(|e| e.to_string())?.len();
    let ventana = 64 * 1024u64;
    let inicio = size.saturating_sub(ventana);
    f.seek(SeekFrom::Start(inicio))
        .map_err(|e| format!("seek: {e}"))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| format!("leer: {e}"))?;
    let texto = String::from_utf8_lossy(&buf);
    let mut lineas: Vec<String> = texto.lines().map(|l| l.to_string()).collect();
    if inicio > 0 && !lineas.is_empty() {
        // La primera línea está cortada (leemos desde el medio): se descarta.
        lineas.remove(0);
    }
    if lineas.len() > n {
        lineas = lineas.split_off(lineas.len() - n);
    }
    Ok(lineas)
}

/// PIDs del proceso objetivo.
fn pids_objetivo(paquete: &str) -> Vec<u32> {
    // 1) `pidof` (toybox) — rápido.
    let cmd = format!("pidof {}", crate::android::sh_quote(paquete));
    let r = crate::android::root_exec(&cmd, 5_000, 1_024);
    let mut pids: Vec<u32> = r
        .stdout
        .split_whitespace()
        .filter_map(|t| t.parse::<u32>().ok())
        .collect();
    if !pids.is_empty() {
        pids.sort_unstable();
        return pids;
    }
    // 2) barrido de /proc (no depende de `pidof`).
    let mut v: Vec<u32> = Vec::new();
    if let Ok(entradas) = std::fs::read_dir("/proc") {
        for e in entradas.flatten() {
            let nombre = e.file_name().to_string_lossy().to_string();
            let Ok(pid) = nombre.parse::<u32>() else {
                continue;
            };
            if let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{pid}/cmdline")) {
                let nombre_proceso = cmdline.split('\0').next().unwrap_or("").trim();
                if nombre_proceso == paquete {
                    v.push(pid);
                }
            }
        }
    }
    v.sort_unstable();
    v
}

/// Qué cargadores hay disponibles en el móvil.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Cargadores {
    /// ¿Está corriendo `frida-server`?
    pub frida_server: bool,
    /// ¿Hay cliente `frida` (CLI)?
    pub frida_cli: bool,
    /// Módulo Magisk/Zygisk que parece nuestro cargador.
    pub zygisk: Option<String>,
}

/// Detecta los cargadores disponibles (todo con `root_exec`).
pub fn detectar_cargadores() -> Cargadores {
    let ps = crate::android::root_exec("pidof frida-server", 5_000, 256);
    let frida_server = !ps.stdout.trim().is_empty();
    let cli = crate::android::root_exec(
        "command -v frida 2>/dev/null; ls /data/local/tmp/frida 2>/dev/null",
        5_000,
        512,
    );
    let frida_cli = !cli.stdout.trim().is_empty();
    let zy = crate::android::root_exec(
        "ls -d /data/adb/modules/*hook* /data/adb/modules/*frida* 2>/dev/null | head -3",
        5_000,
        512,
    );
    let zygisk = zy
        .stdout
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    Cargadores {
        frida_server,
        frida_cli,
        zygisk,
    }
}

/// Estado del payload, del proceso objetivo y del NDJSON.
pub fn estado(args: &Value) -> Value {
    let paquete = paquete_de(args);
    let payload = Path::new(RUTA_PAYLOAD);
    let meta = std::fs::metadata(payload).ok();
    let existe = meta.is_some();
    let pids = pids_objetivo(&paquete);
    let ndjson = ndjson_existente();
    let cargadores = detectar_cargadores();
    json!({
        "ok": true,
        "android": es_android(),
        "payload": {
            "ruta": RUTA_PAYLOAD,
            "existe": existe,
            "bytes": meta.as_ref().map(|m| m.len()).unwrap_or(0),
            "sha256": sha_de_fichero(payload),
            "modo": modo_de(payload),
            "asset": ASSET_PAYLOAD,
        },
        "objetivo": {
            "paquete": paquete,
            "vivo": !pids.is_empty(),
            "pids": pids,
        },
        "ndjson": {
            "ruta_canonica": RUTA_NDJSON,
            "existe": ndjson.is_some(),
            "ruta": ndjson.as_ref().map(|(r, _)| r.clone()),
            "bytes": ndjson.as_ref().map(|(_, m)| m.len()).unwrap_or(0),
            "rutas_candidatas": candidatas_ndjson(),
        },
        "cargadores": {
            "frida_server": cargadores.frida_server,
            "frida_cli": cargadores.frida_cli,
            "zygisk": cargadores.zygisk,
        },
    })
}

/// Descarga e instala el payload (verifica el SHA-256 antes de tocar nada).
pub fn instalar(args: &Value) -> Value {
    let _ = args;
    let release = crate::self_update::url_release();
    let url = format!("{release}/{ASSET_PAYLOAD}");
    let url_sha = format!("{url}.sha256");
    let destino = PathBuf::from(RUTA_PAYLOAD);

    // 1) el .sha256 (pequeño) para saber qué hay que instalar.
    let texto_sha = match crate::self_update::descargar_texto(&url_sha) {
        Ok(t) => t,
        Err(e) => {
            return json!({
                "ok": false,
                "error": format!("descargar {url_sha}: {e}"),
                "nota": "el asset del payload lo publica el job 'build-hooks-payload' del CI",
            })
        }
    };
    let esperado = match crate::self_update::parsear_sha256(&texto_sha) {
        Some(s) => s,
        None => {
            return json!({
                "ok": false,
                "error": format!("el .sha256 no trae un sha256 legible: {url_sha}"),
            })
        }
    };

    // 2) ¿ya está instalado y es el mismo?
    if let Some(sha_local) = sha_de_fichero(&destino) {
        if sha_local == esperado {
            return json!({
                "ok": true,
                "ya_instalado": true,
                "ruta": RUTA_PAYLOAD,
                "sha256": sha_local,
                "bytes": std::fs::metadata(&destino).map(|m| m.len()).unwrap_or(0),
            });
        }
    }

    // 3) descarga el asset y verifica.
    let bytes = match crate::self_update::descargar_bytes(&url) {
        Ok(b) => b,
        Err(e) => return json!({"ok": false, "error": format!("descargar {url}: {e}")}),
    };
    let calculado = crate::self_update::sha256_hex(&bytes);
    if calculado != esperado {
        return json!({
            "ok": false,
            "error": "el SHA-256 del payload no coincide: NO se instala nada",
            "esperado": esperado,
            "calculado": calculado,
        });
    }

    // 4) instalación atómica (a `.new`, chmod 0700 y renombrado).
    if let Some(dir) = destino.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = destino.with_extension("so.new");
    if let Err(e) = escribir_en(&tmp, &bytes, 0o700) {
        return json!({
            "ok": false,
            "error": format!("escribir {}: {e}", tmp.display()),
            "nota": "si el daemon no corre como root, escribe en /data/local/tmp con `su`",
        });
    }
    if let Err(e) = std::fs::rename(&tmp, &destino) {
        let _ = std::fs::remove_file(&tmp);
        return json!({"ok": false, "error": format!("renombrar: {e}")});
    }
    json!({
        "ok": true,
        "ruta": RUTA_PAYLOAD,
        "bytes": bytes.len(),
        "sha256": calculado,
        "modo": "0700",
        "url": url,
    })
}

/// Escribe un fichero con permisos (usa `su` si no hay permiso directo).
fn escribir_en(ruta: &Path, bytes: &[u8], modo: u32) -> Result<(), String> {
    match std::fs::write(ruta, bytes) {
        Ok(()) => {
            set_modo(ruta, modo);
            Ok(())
        }
        Err(e) => {
            // Intento con root (`su -mm -c "cat > fichero"`).
            let destino = ruta.to_string_lossy().to_string();
            let cmd = format!("cat > {}", crate::android::sh_quote(&destino));
            let mut child = match std::process::Command::new("su")
                .arg("-mm")
                .arg("-c")
                .arg(&cmd)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e2) => return Err(format!("{e} (y `su` falló: {e2})")),
            };
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(bytes)
                    .map_err(|e2| format!("escribir por `su`: {e2}"))?;
            }
            let salida = child
                .wait_with_output()
                .map_err(|e2| format!("esperar a `su`: {e2}"))?;
            if !salida.status.success() {
                return Err(format!(
                    "{e} (y `su` devolvió {}: {})",
                    salida.status,
                    String::from_utf8_lossy(&salida.stderr).trim()
                ));
            }
            set_modo(ruta, modo);
            Ok(())
        }
    }
}

/// Fija el modo de un fichero (`0700` para el payload).
fn set_modo(ruta: &Path, modo: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(ruta) {
        let mut perm = meta.permissions();
        perm.set_mode(modo);
        let _ = std::fs::set_permissions(ruta, perm);
    }
}

/// Activa la carga del payload en el proceso objetivo.
///
/// `cargador`:
/// * `auto`   → `frida` si hay cliente, si no `zygisk`, si no error con las
///   instrucciones.
/// * `frida`  → cliente `frida` + `frida-server` (el .so se carga con
///   `Module.load`).
/// * `zygisk` → reinicia la app para que el módulo Magisk/Zygisk inyecte.
/// * `manual` → sólo devuelve los comandos.
pub fn activar(args: &Value) -> Value {
    let paquete = paquete_de(args);
    let cargador = args
        .get("cargador")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "auto".to_string());

    if !Path::new(RUTA_PAYLOAD).exists() {
        return json!({
            "ok": false,
            "error": format!("el payload no está instalado en {RUTA_PAYLOAD}: ejecuta primero la acción 'instalar'"),
        });
    }

    let cargadores = detectar_cargadores();
    let elegido = match cargador.as_str() {
        "auto" => {
            if cargadores.frida_cli {
                "frida"
            } else if cargadores.zygisk.is_some() {
                "zygisk"
            } else {
                ""
            }
        }
        c => c,
    };

    match elegido {
        "frida" => activar_con_frida(&paquete, &cargadores),
        "zygisk" => activar_con_zygisk(&paquete, &cargadores),
        "manual" => json!({
            "ok": true,
            "cargador": "manual",
            "comandos": comandos_manuales(&paquete),
            "estado": cargadores,
        }),
        "" => json!({
            "ok": false,
            "error": "no hay ningún cargador disponible en el móvil (ni cliente frida ni módulo Zygisk)",
            "cargadores": cargadores,
            "comandos": comandos_manuales(&paquete),
        }),
        otro => json!({
            "ok": false,
            "error": format!("cargador desconocido: '{otro}' (usa auto|frida|zygisk|manual)"),
        }),
    }
}

/// Activa con el cliente de Frida (`Module.load` carga la .so en el proceso).
fn activar_con_frida(paquete: &str, cargadores: &Cargadores) -> Value {
    if !cargadores.frida_cli {
        return json!({
            "ok": false,
            "error": "no hay cliente `frida` en el móvil (frida-server corre, pero falta el CLI para cargar la .so)",
            "cargadores": cargadores,
            "comandos": comandos_manuales(paquete),
        });
    }
    let js = JS_CARGADOR.replace("__SO__", RUTA_PAYLOAD);
    let ruta_js = PathBuf::from(RUTA_CARGADOR_JS);
    if let Some(dir) = ruta_js.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = escribir_en(&ruta_js, js.as_bytes(), 0o600) {
        return json!({"ok": false, "error": format!("escribir {}: {e}", ruta_js.display())});
    }
    let cmd = format!(
        "timeout 25 frida -U -n {} -l {} -q 2>&1",
        crate::android::sh_quote(paquete),
        crate::android::sh_quote(RUTA_CARGADOR_JS)
    );
    let r = crate::android::root_exec(&cmd, 40_000, 8_192);
    json!({
        "ok": r.code == Some(0),
        "cargador": "frida",
        "comando": cmd,
        "codigo": r.code,
        "salida": r.combined(),
        "nota": "el .so se queda cargado en el proceso aunque el cliente salga: los hooks siguen vivos",
    })
}

/// Activa reiniciando la app (Zygisk inyecta al arrancar el proceso).
fn activar_con_zygisk(paquete: &str, cargadores: &Cargadores) -> Value {
    let cmd = format!("am force-stop {}", crate::android::sh_quote(paquete));
    let r = crate::android::root_exec(&cmd, 20_000, 1_024);
    json!({
        "ok": r.code == Some(0),
        "cargador": "zygisk",
        "comando": cmd,
        "codigo": r.code,
        "modulo": cargadores.zygisk,
        "nota": "la app se ha parado: al abrirla de nuevo, el módulo Magisk/Zygisk inyecta el payload",
    })
}

/// Comandos para hacerlo a mano (por ADB o `su`).
fn comandos_manuales(paquete: &str) -> Vec<String> {
    vec![
        format!("ls -l {RUTA_PAYLOAD}"),
        format!("frida -U -n {paquete} -l {RUTA_CARGADOR_JS}"),
        format!("tail -f {RUTA_NDJSON}"),
    ]
}

/// Lee las últimas líneas del NDJSON.
pub fn leer(args: &Value) -> Value {
    let n = args
        .get("n")
        .and_then(|v| v.as_u64())
        .unwrap_or(50)
        .clamp(1, 1_000) as usize;
    let ruta = args
        .get("ruta")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let candidatas = match &ruta {
        Some(r) => vec![r.clone()],
        None => candidatas_ndjson(),
    };
    for c in &candidatas {
        let p = Path::new(c);
        if !p.exists() {
            continue;
        }
        return match ultimas_lineas(p, n) {
            Ok(lineas) => json!({
                "ok": true,
                "ruta": c,
                "lineas": lineas.len(),
                "eventos": lineas,
            }),
            Err(e) => json!({"ok": false, "error": e, "ruta": c}),
        };
    }
    json!({
        "ok": false,
        "error": "no hay ningún NDJSON de hooks en el móvil",
        "rutas_probadas": candidatas,
        "nota": "el payload escribe en la primera ruta en la que puede abrir; el evento 'payload/iniciado' dice cuál",
    })
}

/// Despacho por acción (lo usa la tool MCP `hooks_payload`).
pub fn dispatch(accion: &str, args: &Value) -> Value {
    match accion {
        "estado" => estado(args),
        "instalar" => instalar(args),
        "activar" => activar(args),
        "leer" => leer(args),
        otro => json!({
            "ok": false,
            "error": format!("acción desconocida: '{otro}' (usa estado|instalar|activar|leer)"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_estado_no_rompe_sin_movil() {
        let v = estado(&json!({}));
        assert_eq!(v["ok"], json!(true));
        assert!(v["payload"]["ruta"].is_string());
        assert!(v["objetivo"]["paquete"].is_string());
        assert!(v["cargadores"]["frida_server"].is_boolean());
    }

    #[test]
    fn el_paquete_se_puede_cambiar() {
        let v = estado(&json!({"paquete": "com.ejemplo"}));
        assert_eq!(v["objetivo"]["paquete"], json!("com.ejemplo"));
        let v = estado(&json!({"paquete": "  "}));
        assert_eq!(v["objetivo"]["paquete"], json!(PAQUETE_OBJETIVO));
    }

    #[test]
    fn leer_sin_ndjson_da_error_explicito() {
        let v = leer(&json!({"ruta": "/tmp/megadaemon_no_existe_hooks.ndjson"}));
        assert_eq!(v["ok"], json!(false));
        assert!(v["error"]
            .as_str()
            .unwrap_or("")
            .contains("no hay ningún NDJSON"));
    }

    #[test]
    fn una_accion_desconocida_da_error() {
        let v = dispatch("inventada", &json!({}));
        assert_eq!(v["ok"], json!(false));
    }

    #[test]
    fn el_cargador_js_usa_module_load() {
        // API oficial de Frida: Module.load(path).
        assert!(JS_CARGADOR.contains("Module.load"));
        assert!(JS_CARGADOR.contains("__SO__"));
    }

    #[test]
    fn las_candidatas_del_ndjson_empezan_por_la_canonica() {
        let c = candidatas_ndjson();
        assert_eq!(c[0], RUTA_NDJSON);
    }
}

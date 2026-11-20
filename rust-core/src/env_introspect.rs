//! MegaDaemon v14 — Introspección del entorno: **preguntar al móvil, no adivinar**.
//!
//! ## Por qué existe
//!
//! En la fase RE del Exynos 2100 seLost dos artefactos "que no existen": `NPU.bin`
//! y `hw_keymaster`. No faltaban: **las rutas del volcado de ROM no correspondían
//! al firmware en vivo**. Un agente que hubiera *supuesto* habría seguido
//! extrayendo de una imagen equivocada; uno que *pregunta al dispositivo* lo
//! detecta en un segundo.
//!
//! Este módulo es la respuesta mecánica a eso: **una tool que responde preguntas
//! del entorno leyéndolo del dispositivo**, en vez de deducirlo.
//!
//! ## Contrato de la respuesta (obligatorio)
//!
//! Toda respuesta lleva **la fuente y el momento**, para que nadie pueda
//! confundir un dato viejo con uno actual:
//!
//! ```json
//! {
//!   "ok": true, "action": "binaries", "question": "¿existe NPU.bin?",
//!   "available": true, "reason": null,
//!   "source": "/vendor/bin,/system/bin,/vendor/lib64,/system/lib64",
//!   "device": "SM-G998B (exynos2100)",
//!   "ts": "2026-11-20T00:47:12Z",
//!   "answer": { ... }
//! }
//! ```
//!
//! Y cuando algo **no existe**, NO se inventa: `available:false` + `reason` con
//! el motivo exacto (ruta no encontrada, sin permiso, binario no desplegado…).
//!
//! ## Acciones
//!
//! | action | qué responde |
//! |---|---|
//! | `partitions` | qué particiones hay de verdad (`/dev/block/by-name`), con tamaño real y sha256 opcional |
//! | `dev` | qué nodos hay en `/dev` (filtro), con modo/uid/gid |
//! | `binaries` | **qué binarios existen** con ese nombre y en qué rutas (sha256 incluido) |
//! | `app` | de una app: ¿instalada?, apk, versión, uid, libs nativas |
//! | `ram` | RAM/carga/uptime del dispositivo |
//! | `path` | inspección de una ruta concreta (tipo, modo, tamaño, sha256, destino del symlink) |
//! | `prop` | `getprop` filtrado |
//! | `processes` | procesos del móvil que casen con un filtro |
//! | `help` | lista de acciones y ejemplos |
//!
//! Todo es **de sólo lectura**: no escribe nada, no reinicia nada.

use serde_json::{json, Value};

/// Rutas donde se buscan binarios (las de un firmware de Samsung).
const BIN_DIRS: &[&str] = &[
    "/vendor/bin",
    "/system/bin",
    "/vendor/lib64",
    "/system/lib64",
    "/vendor/lib64/hw",
    "/odm/bin",
    "/product/bin",
    // AUDIT-ENV-01: faltaban estas rutas, asi que la tool respondia
    // "available: false" sobre ficheros que SI existen:
    // /vendor/firmware/NPU.bin (1.506.272 B, medido en el S21 Ultra) y los
    // .rc del init de EDEN en /vendor/etc/init.
    "/vendor/firmware",
    "/vendor/etc/init",
    "/vendor/etc/midas",
    "/vendor/etc/eden",
    "/system/etc/init",
];

/// Rutas donde se buscan nodos de dispositivo.
const DEV_DIR: &str = "/dev";

/// Timestamp del dispositivo (UTC ISO-8601) con respaldo en el reloj del host.
fn device_ts() -> String {
    // `android::root_exec` devuelve `ExecResult` (no `Result`): se adapta
    // aquí para que el crate compile (fallo de tipos que bloqueaba a todos).
    {
        let r = crate::android::root_exec("date -u +%Y-%m-%dT%H:%M:%SZ", 4_000, 128);
        let t = r.stdout.trim().to_string();
        if !t.is_empty() {
            return t;
        }
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch:{}", secs)
}

/// Identificador del dispositivo (modelo + SoC).
fn device_id() -> String {
    let out = crate::android::root_exec(
        "getprop ro.product.model; getprop ro.hardware; getprop ro.board.platform",
        5_000,
        256,
    )
    .stdout;
    let parts: Vec<&str> = out.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    if parts.is_empty() {
        return "desconocido".to_string();
    }
    parts.join(" / ")
}

/// Envuelve una respuesta con el contrato (fuente, momento, disponible/motivo).
fn envelope(action: &str, question: &str, source: &str, answer: Value) -> Value {
    json!({
        "ok": true,
        "action": action,
        "question": question,
        "available": true,
        "reason": Value::Null,
        "source": source,
        "device": device_id(),
        "ts": device_ts(),
        "answer": answer,
    })
}

/// Respuesta de "no existe": motivo explícito, sin inventar nada.
fn unavailable(action: &str, question: &str, source: &str, reason: &str) -> Value {
    json!({
        "ok": true,
        "action": action,
        "question": question,
        "available": false,
        "reason": reason,
        "source": source,
        "device": device_id(),
        "ts": device_ts(),
        "answer": Value::Null,
    })
}

/// Ejecuta un comando de sólo lectura y devuelve su stdout (trimmed).
fn ro(command: &str, max_chars: usize) -> String {
    let r = crate::android::root_exec(command, 25_000, max_chars.max(1_024));
    if r.timed_out {
        return String::new();
    }
    r.stdout
}

/// ¿Pide sha256?
fn wants_sha(args: &Value) -> bool {
    args.get("sha").and_then(|v| v.as_bool()).unwrap_or(false)
}

/// `ls -la` de una ruta → (existe, modo, owner, tamaño, target).
fn stat_path(path: &str) -> Option<(String, String, u64, String)> {
    let out = ro(&format!("ls -ld {} 2>/dev/null", crate::android::sh_quote(path)), 4_096);
    let line = out.lines().next()?.trim().to_string();
    if line.is_empty() {
        return None;
    }
    // "-rw-r--r-- 1 root root 1234 date path"
    let mut it = line.split_whitespace();
    let mode = it.next().unwrap_or("").to_string();
    let _links = it.next();
    let owner = it.next().unwrap_or("").to_string();
    let size = it.next().unwrap_or("0").parse::<u64>().unwrap_or(0);
    let rest: Vec<&str> = it.collect();
    let target = if rest.len() > 2 {
        rest[2..].join(" ")
    } else {
        String::new()
    };
    Some((mode, owner, size, target))
}

/// sha256 de un fichero (streaming, en el propio dispositivo).
fn sha_of(path: &str) -> Option<String> {
    let out = ro(
        &format!("sha256sum {} 2>/dev/null | cut -d' ' -f1", crate::android::sh_quote(path)),
        256,
    );
    let h = out.trim().to_string();
    if h.len() == 64 {
        Some(h)
    } else {
        None
    }
}

/// Parsea la salida de `ls /dev/block/by-name` + `readlink` + tamaño.
pub fn parse_partitions(list_out: &str, readlink_out: &str, sizes_out: &str) -> Value {
    let mut items = vec![];
    let mut real: Vec<(String, String)> = vec![];
    for line in readlink_out.lines() {
        let mut it = line.split_whitespace();
        if let (Some(name), Some(rp)) = (it.next(), it.next()) {
            real.push((name.to_string(), rp.to_string()));
        }
    }
    let mut sizes: Vec<(String, u64)> = vec![];
    for line in sizes_out.lines() {
        let mut it = line.split_whitespace();
        if let (Some(n), Some(s)) = (it.next(), it.next()) {
            if let Ok(v) = s.parse::<u64>() {
                sizes.push((n.to_string(), v * 512));
            }
        }
    }
    for name in list_out.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        let rp = real
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r.clone())
            .unwrap_or_default();
        let bytes = sizes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| *s)
            .unwrap_or(0);
        items.push(json!({
            "name": name,
            "path": format!("/dev/block/by-name/{}", name),
            "real": rp,
            "bytes": bytes,
        }));
    }
    json!({ "count": items.len(), "partitions": items })
}

/// Parsea la salida de `ls -l` de un directorio de binarios.
pub fn parse_binaries(out: &str, dirs: &[&str]) -> Value {
    let mut items = vec![];
    for line in out.lines() {
        let t = line.trim();
        if t.is_empty() || !t.starts_with('-') {
            continue;
        }
        let mut it = t.split_whitespace();
        let mode = it.next().unwrap_or("");
        let _links = it.next();
        let owner = it.next().unwrap_or("");
        let _grp = it.next();
        let size = it.next().unwrap_or("0");
        let rest: Vec<&str> = it.collect();
        if rest.is_empty() {
            continue;
        }
        let path = rest[rest.len() - 1].to_string();
        let dir = dirs
            .iter()
            .find(|d| path.starts_with(&format!("{}/", d)))
            .copied()
            .unwrap_or("");
        items.push(json!({
            "path": path,
            "dir": dir,
            "mode": mode,
            "owner": owner,
            "bytes": size.parse::<u64>().unwrap_or(0),
        }));
    }
    json!({ "count": items.len(), "matches": items })
}

/// Filtro por subcadena, sin distinguir mayúsculas.
fn matches(hay: &str, needle: &str) -> bool {
    needle.is_empty() || hay.to_lowercase().contains(&needle.to_lowercase())
}

/// Punto de entrada de la tool.
///
/// `question` es texto libre para el agente; `action` es el verbo concreto.
pub fn ask(question: &str, args: &Value) -> Value {
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("help")
        .to_string();
    let filter = args
        .get("filter")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let pkg = args
        .get("package")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    match action.as_str() {
        "partitions" => {
            let src = "/dev/block/by-name";
            let list = ro("ls /dev/block/by-name 2>/dev/null", 8_192);
            if list.trim().is_empty() {
                return unavailable("partitions", question, src, "no se pudo leer /dev/block/by-name");
            }
            let names: Vec<String> = list
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            let mut rl = String::new();
            for n in &names {
                let r = ro(
                    &format!("readlink -f /dev/block/by-name/{} 2>/dev/null", crate::android::sh_quote(n)),
                    256,
                );
                rl.push_str(n);
                rl.push(' ');
                rl.push_str(r.trim());
                rl.push('\n');
            }
            let mut sizes = String::new();
            for n in &names {
                let s = ro(
                    &format!("cat /sys/class/block/{}/size 2>/dev/null", crate::android::sh_quote(n)),
                    64,
                );
                sizes.push_str(n);
                sizes.push(' ');
                sizes.push_str(s.trim());
                sizes.push('\n');
            }
            let mut ans = parse_partitions(&list, &rl, &sizes);
            if let Some(arr) = ans.get_mut("partitions").and_then(|p| p.as_array_mut()) {
                for p in arr.iter_mut() {
                    let nm = p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if !matches(&nm, &filter) {
                        continue;
                    }
                    if wants_sha(args) {
                        let full = format!("/dev/block/by-name/{}", nm);
                        if let Some(h) = sha_of(&full) {
                            if let Some(o) = p.as_object_mut() {
                                o.insert("sha256".to_string(), json!(h));
                            }
                        }
                    }
                }
            }
            envelope("partitions", question, src, ans)
        }
        "dev" => {
            let src = "/dev";
            let out = ro(
                &format!("ls -la {} 2>/dev/null", crate::android::sh_quote(DEV_DIR)),
                24_576,
            );
            if out.trim().is_empty() {
                return unavailable("dev", question, src, "no se pudo listar /dev");
            }
            let mut items = vec![];
            for line in out.lines() {
                let t = line.trim();
                if t.len() < 10 || !t.starts_with(['b', 'c', 'p', '-', 'l', 'd', 's']) {
                    continue;
                }
                let mut it = t.split_whitespace();
                let mode = it.next().unwrap_or("");
                let _l = it.next();
                let owner = it.next().unwrap_or("");
                let _g = it.next();
                let rest: Vec<&str> = it.collect();
                if rest.is_empty() {
                    continue;
                }
                let name = rest[rest.len() - 1].to_string();
                if !matches(&name, &filter) {
                    continue;
                }
                items.push(json!({
                    "name": name,
                    "mode": mode,
                    "owner": owner,
                    "kind": if mode.starts_with('b') { "block" } else if mode.starts_with('c') { "char" } else if mode.starts_with('l') { "symlink" } else if mode.starts_with('d') { "dir" } else { "file" },
                }));
            }
            envelope("dev", question, src, json!({ "count": items.len(), "nodes": items }))
        }
        "binaries" => {
            let src = BIN_DIRS.join(",");
            if filter.trim().is_empty() {
                return unavailable(
                    "binaries",
                    question,
                    &src,
                    "falta 'filter' (el nombre del binario que se busca)",
                );
            }
            let mut cmd = String::new();
            for d in BIN_DIRS {
                cmd.push_str(&format!(
                    "ls -la {} 2>/dev/null | grep -i -- '{}' ; ",
                    crate::android::sh_quote(d),
                    filter.replace('\'', "")
                ));
            }
            let out = ro(&cmd, 32_768);
            let mut ans = parse_binaries(&out, BIN_DIRS);
            if wants_sha(args) {
                if let Some(arr) = ans.get_mut("matches").and_then(|m| m.as_array_mut()) {
                    for m in arr.iter_mut() {
                        let p = m.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if let Some(h) = sha_of(&p) {
                            if let Some(o) = m.as_object_mut() {
                                o.insert("sha256".to_string(), json!(h));
                            }
                        }
                    }
                }
            }
            if ans.get("count").and_then(|c| c.as_u64()).unwrap_or(0) == 0 {
                return unavailable(
                    "binaries",
                    question,
                    &src,
                    &format!("ningún binario casa con '{}' en el dispositivo", filter),
                );
            }
            envelope("binaries", question, &src, ans)
        }
        "app" => {
            let src = "pm (package manager) + /data/app";
            if pkg.trim().is_empty() {
                return unavailable("app", question, src, "falta 'package'");
            }
            let out = ro(
                &format!(
                    "pm path {} 2>/dev/null; dumpsys package {} 2>/dev/null | grep -E 'versionName|versionCode|userId=|primaryCpuAbi' | head -6",
                    crate::android::sh_quote(&pkg),
                    crate::android::sh_quote(&pkg)
                ),
                8_192,
            );
            if out.trim().is_empty() {
                return unavailable(
                    "app",
                    question,
                    src,
                    &format!("la app '{}' no está instalada (o pm no responde)", pkg),
                );
            }
            let mut apk = String::new();
            let mut version = json!({});
            let mut uid = Value::Null;
            let mut abi = Value::Null;
            for line in out.lines() {
                let t = line.trim();
                if t.starts_with("package:") {
                    apk = t.trim_start_matches("package:").to_string();
                } else if let Some(v) = t.strip_prefix("versionName=") {
                    version = json!({ "name": v.trim() });
                } else if let Some(v) = t.strip_prefix("versionCode=") {
                    if let Some(o) = version.as_object_mut() {
                        o.insert("code".to_string(), json!(v.trim()));
                    }
                } else if let Some(v) = t.strip_prefix("userId=") {
                    uid = json!(v.trim());
                } else if let Some(v) = t.strip_prefix("primaryCpuAbi=") {
                    abi = json!(v.trim());
                }
            }
            let mut sha = Value::Null;
            if wants_sha(args) && !apk.is_empty() {
                if let Some(h) = sha_of(&apk) {
                    sha = json!(h);
                }
            }
            envelope(
                "app",
                question,
                src,
                json!({
                    "package": pkg,
                    "installed": !apk.is_empty(),
                    "apk_path": apk,
                    "apk_sha256": sha,
                    "version": version,
                    "uid": uid,
                    "primary_abi": abi,
                }),
            )
        }
        "ram" => {
            let src = "/proc/meminfo + /proc/uptime";
            let out = ro("cat /proc/meminfo 2>/dev/null | head -8; cat /proc/uptime 2>/dev/null", 4_096);
            if out.trim().is_empty() {
                return unavailable("ram", question, src, "no se pudo leer /proc/meminfo");
            }
            let mut mem = serde_json::Map::new();
            for line in out.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let kb: u64 = v
                        .trim()
                        .trim_end_matches(" kB")
                        .trim()
                        .parse()
                        .unwrap_or(0);
                    mem.insert(k.trim().to_string(), json!(kb * 1024));
                }
            }
            envelope(
                "ram",
                question,
                src,
                json!({ "bytes": mem, "raw": out.trim() }),
            )
        }
        "path" => {
            let src = path.clone();
            if path.trim().is_empty() {
                return unavailable("path", question, "ruta", "falta 'path'");
            }
            match stat_path(&path) {
                None => unavailable("path", question, &src, "la ruta no existe (o no es legible)"),
                Some((mode, owner, size, target)) => {
                    let mut ans = json!({
                        "path": path,
                        "exists": true,
                        "mode": mode,
                        "owner": owner,
                        "bytes": size,
                    });
                    if !target.is_empty() {
                        if let Some(o) = ans.as_object_mut() {
                            o.insert("target".to_string(), json!(target));
                        }
                    }
                    if wants_sha(args) {
                        let h = sha_of(&path);
                        if let Some(o) = ans.as_object_mut() {
                            o.insert(
                                "sha256".to_string(),
                                match h {
                                    Some(v) => json!(v),
                                    None => json!(null),
                                },
                            );
                        }
                    }
                    envelope("path", question, &src, ans)
                }
            }
        }
        "prop" => {
            let src = "getprop";
            let out = ro(
                &format!("getprop 2>/dev/null | grep -i -- '{}'", filter.replace('\'', "")),
                16_384,
            );
            let mut props = vec![];
            for line in out.lines() {
                if let Some((k, v)) = line.split_once(": ") {
                    props.push(json!({ "key": k.trim(), "value": v.trim() }));
                }
            }
            if props.is_empty() {
                return unavailable(
                    "prop",
                    question,
                    src,
                    &format!("ninguna propiedad casa con '{}'", filter),
                );
            }
            envelope("prop", question, src, json!({ "count": props.len(), "props": props }))
        }
        "processes" => {
            let src = "ps";
            let out = ro(
                &format!("ps -A -o PID,USER,RSS,NAME 2>/dev/null | grep -i -- '{}'", filter.replace('\'', "")),
                24_576,
            );
            let mut procs = vec![];
            for line in out.lines().skip(1) {
                let mut it = line.split_whitespace();
                let pid = it.next().unwrap_or("");
                let user = it.next().unwrap_or("");
                let rss = it.next().unwrap_or("");
                let name: Vec<&str> = it.collect();
                if pid.is_empty() || name.is_empty() {
                    continue;
                }
                procs.push(json!({
                    "pid": pid, "user": user,
                    "rss_kb": rss.parse::<u64>().unwrap_or(0),
                    "name": name.join(" "),
                }));
            }
            envelope("processes", question, src, json!({ "count": procs.len(), "processes": procs }))
        }
        _ => envelope(
            "help",
            question,
            "módulo env_introspect",
            json!({
                "actions": ["partitions", "dev", "binaries", "app", "ram", "path", "prop", "processes", "help"],
                "nota": "todas las respuestas llevan source (de dónde sale) y ts (momento); si algo no existe: available=false + reason",
                "ejemplos": [
                    {"action":"binaries","filter":"NPU.bin"},
                    {"action":"binaries","filter":"keymaster","sha":true},
                    {"action":"partitions","filter":"keystorage"},
                    {"action":"dev","filter":"vertex"},
                    {"action":"app","package":"com.android.chrome"},
                    {"action":"path","path":"/vendor/bin/NPU.bin","sha":true},
                    {"action":"prop","filter":"ro.boot."},
                ],
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_partitions_une_nombre_real_y_tamano() {
        let list = "boot\nvbmeta\nkeyrefuge\n";
        let rl = "boot /dev/block/sda1\nvbmeta /dev/block/sda2\nkeyrefuge /dev/block/sda21\n";
        let sizes = "boot 131072\nvbmeta 128\nkeyrefuge 106496\n";
        let v = parse_partitions(list, rl, sizes);
        assert_eq!(v["count"], 3);
        let p = &v["partitions"][2];
        assert_eq!(p["name"], "keyrefuge");
        assert_eq!(p["real"], "/dev/block/sda21");
        assert_eq!(p["bytes"], 106496u64 * 512);
    }

    #[test]
    fn parse_binaries_solo_coge_ficheros_y_conserva_el_directorio() {
        let out = "-rwxr-xr-x 1 root root 436920 2026-01-01 00:00 /vendor/lib64/libeden_ud_npu.so\n\
drwxr-xr-x 2 root root 4096 2026-01-01 00:00 /vendor/lib64\n\
-rw-r--r-- 1 root root 317 2026-01-01 00:00 /vendor/etc/init/x.rc\n";
        let v = parse_binaries(out, BIN_DIRS);
        assert_eq!(v["count"], 2);
        assert_eq!(v["matches"][0]["path"], "/vendor/lib64/libeden_ud_npu.so");
        assert_eq!(v["matches"][0]["dir"], "/vendor/lib64");
        assert_eq!(v["matches"][0]["bytes"], 436920u64);
    }

    #[test]
    fn help_documenta_las_acciones_y_el_contrato() {
        let v = ask("", &json!({ "action": "help" }));
        let acts = v["answer"]["actions"].as_array().unwrap();
        assert!(acts.iter().any(|a| a == "binaries"));
        assert!(acts.iter().any(|a| a == "partitions"));
        // el contrato: siempre source y ts
        assert!(v.get("source").is_some());
        assert!(v.get("ts").is_some());
        assert_eq!(v["available"], true);
    }

    #[test]
    fn sin_filtro_binaries_devuelve_motivo_y_no_inventa() {
        let v = ask("¿hay algum binario?", &json!({ "action": "binaries" }));
        assert_eq!(v["available"], false);
        assert!(v["reason"].as_str().unwrap().contains("filter"));
        assert!(v["answer"].is_null());
    }

    #[test]
    fn path_vacio_devuelve_motivo() {
        let v = ask("", &json!({ "action": "path" }));
        assert_eq!(v["available"], false);
        assert!(v["reason"].as_str().unwrap().contains("path"));
    }
}

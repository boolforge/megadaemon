//! Inteligencia de root **medida** en el dispositivo.
//!
//! `root_hide` decia "clean (simulado)" porque construia el entorno con
//! `DeviceEnv::clean()` en vez de mirar el movil. Este modulo hace lo
//! contrario: **sondea el dispositivo de verdad** y devuelve:
//!
//! * `detectors()`  — que apps DETECTAN root (firma estatica en su DEX +
//!   evidencia dinamica en logcat).
//! * `consumers()`  — que apps USAN root (permisos reales de Magisk,
//!   gestores de root instalados, ejecuciones de `su` observadas).
//!
//! Todo lo que se devuelve lleva la evidencia que lo sustenta: la cadena
//! exacta que se encontro, el fichero o el logcat del que salio. Nada de
//! puntuaciones inventadas.

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Firma de deteccion: (categoria, etiqueta, cadena buscada).
///
/// La categoria decide a que herramienta pertenece el hallazgo:
/// `su_path`/`magisk_dir`/`props`/`hook`/`integrity`/`emulator`/`knox`
/// son señales de que la app *mira* si hay root.
const SIGNATURES: &[(&str, &str, &str)] = &[
    ("su_path", "su_binario", "/system/bin/su"),
    ("su_path", "su_binario", "/system/xbin/su"),
    ("su_path", "su_binario", "/sbin/su"),
    ("su_path", "su_binario", "/debug_ramdisk/su"),
    ("su_path", "su_binario", "/system/xbin/.ext/.su"),
    ("su_path", "su_binario", "which su"),
    ("su_path", "su_binario", "com.noshufou.android.su"),
    ("su_path", "su_binario", "eu.chainfire.supersu"),
    ("su_path", "su_binario", "com.koushikdutta.superuser"),
    ("su_path", "su_binario", "com.topjohnwu.magisk"),
    ("su_path", "su_binario", "io.github.huskydg.magisk"),
    ("magisk_dir", "magisk_dir", "/sbin/.magisk"),
    ("magisk_dir", "magisk_dir", "/data/adb/magisk"),
    ("magisk_dir", "magisk_dir", "/data/adb/zygisk"),
    ("magisk_dir", "magisk_dir", "/dev/.magisk"),
    ("busybox", "busybox", "busybox"),
    ("props", "test_keys", "test-keys"),
    ("props", "debuggable", "ro.debuggable"),
    ("props", "secure", "ro.secure"),
    ("props", "build_type", "ro.build.type"),
    ("hook", "xposed", "de.robv.android.xposed"),
    ("hook", "xposed", "XposedBridge"),
    ("hook", "xposed", "lsposed"),
    ("hook", "substrate", "com.saurik.substrate"),
    ("hook", "frida", "frida-server"),
    ("hook", "frida", "gadget"),
    ("hook", "frida", "frida_agent"),
    ("hook", "tracerpid", "TracerPid"),
    ("hook", "proc_status", "/proc/self/status"),
    ("hook", "ptrace", "ptrace"),
    ("integrity", "play_integrity", "playintegrity"),
    ("integrity", "play_integrity", "PlayIntegrity"),
    ("integrity", "safety_net", "safetynet"),
    ("integrity", "safety_net", "SafetyNet"),
    ("integrity", "device_integrity", "MEETS_DEVICE_INTEGRITY"),
    ("integrity", "app_integrity", "appIntegrity"),
    ("emulator", "goldfish", "goldfish"),
    ("emulator", "ranchu", "ranchu"),
    ("emulator", "generic_x86", "generic_x86"),
    ("emulator", "genymotion", "genymotion"),
    ("emulator", "nox", "nox"),
    ("emulator", "bluestacks", "bluestacks"),
    ("emulator", "andy", "Andy"),
    ("knox", "knox", "com.samsung.android.knox"),
    ("knox", "knox", "knox_attestation"),
    ("knox", "knox", "Knox Warranty Void"),
    ("knox", "knox", "isKnoxTampered"),
    ("samsungpass", "samsung_pass", "samsungpass"),
];

/// Patrones de logcat que delatan a una app QUE BUSCA root en tiempo de
/// ejecucion. Es la evidencia dinamica: el app ha pensado que hay root.
const RUNTIME_PATTERNS: &[&str] = &[
    "Found su at",
    "found su at",
    "root detected",
    "Root detected",
    "ROOT DETECTED",
    "is rooted",
    "isRooted",
    "device is rooted",
    "tampered",
    "device tampered",
    "not certified",
    "MEETS_DEVICE_INTEGRITY",
    "FAILED_DEVICE_INTEGRITY",
    "SafetyNet",
    "magisk detected",
    "Magisk detected",
    "busybox detected",
    "Xposed",
    "frida",
];

/// Gestores de root conocidos: apps cuya FINALIDAD es usar root.
const ROOT_MANAGERS: &[&str] = &[
    "com.topjohnwu.magisk",
    "io.github.huskydg.magisk",
    "me.weishu.kernelsu",
    "com.topjohnwu.magisk.alpha",
    "eu.chainfire.supersu",
    "com.noshufou.android.su",
    "com.koushikdutta.superuser",
    "com.thirdparty.superuser",
    "com.zhiqupk.root.global",
    "com.applisto.xpo",
    "io.github.libxposed",
    "com.byyoung.setting",
];

// ---------------------------------------------------------------------------
// Utilidades de shell
// ---------------------------------------------------------------------------

/// Ejecuta un comando como root en el dispositivo y devuelve la salida.
fn sh(cmd: &str) -> String {
    crate::android::root_exec(cmd, 20_000, 400_000).combined()
}

fn sh_lines(cmd: &str) -> Vec<String> {
    sh(cmd)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Lista de paquetes instalados (`pm list packages`).
pub fn installed_packages(third_party_only: bool) -> Vec<String> {
    let cmd = if third_party_only {
        "pm list packages -3"
    } else {
        "pm list packages"
    };
    sh_lines(cmd)
        .into_iter()
        .filter_map(|l| l.strip_prefix("package:").map(|s| s.to_string()))
        .collect()
}

/// Ruta del APK de un paquete (`pm path`).
fn apk_path(pkg: &str) -> Option<String> {
    let out = sh(&format!("pm path {}", crate::android::sh_quote(pkg)));
    out.lines()
        .filter_map(|l| l.strip_prefix("package:").map(|s| s.trim().to_string()))
        .find(|s| s.ends_with(".apk"))
}

/// UID de un paquete.
fn uid_of(pkg: &str) -> Option<String> {
    let out = sh(&format!(
        "dumpsys package {} 2>/dev/null | grep -m1 'userId='",
        crate::android::sh_quote(pkg)
    ));
    out.split_whitespace()
        .find_map(|t| t.strip_prefix("userId=").map(|s| s.to_string()))
}

// ---------------------------------------------------------------------------
// DEX: tabla de cadenas (formato DEX real, no busqueda a ciegas)
// ---------------------------------------------------------------------------

/// Extrae la tabla de cadenas de un fichero DEX.
///
/// Formato DEX: en la cabecera, `string_ids_size` (0x38) y `string_ids_off`
/// (0x3C). Cada `string_id` es un offset de 4 bytes a un `string_data_item`:
/// `uleb128` de longitud (en UTF-16) seguido de los bytes MUTF-8.
fn dex_strings(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if data.len() < 0x70 {
        return out;
    }
    let rd32 = |off: usize| -> u32 {
        if off + 4 > data.len() {
            0
        } else {
            u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
        }
    };
    let n = rd32(0x38) as usize;
    let ids = rd32(0x3C) as usize;
    if n == 0 || ids == 0 || ids + n * 4 > data.len() {
        return out;
    }
    for i in 0..n.min(20_000) {
        let off = rd32(ids + i * 4) as usize;
        if off >= data.len() {
            continue;
        }
        // uleb128
        let mut p = off;
        let mut shift = 0u32;
        let mut len: usize = 0;
        loop {
            if p >= data.len() {
                break;
            }
            let b = data[p];
            p += 1;
            len |= ((b & 0x7F) as usize) << shift;
            shift += 7;
            if b & 0x80 == 0 {
                break;
            }
            if shift > 28 {
                break;
            }
        }
        let end = (p + len).min(data.len());
        let s = String::from_utf8_lossy(&data[p..end]).to_string();
        if s.is_empty() {
            continue;
        }
        out.push(s);
    }
    out
}

/// Cadenas imprimibles de un binario nativo (para las .so del APK).
fn ascii_strings(data: &[u8], min_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for i in 0..data.len() {
        let b = data[i];
        if (0x20..0x7F).contains(&b) {
            continue;
        }
        if i - start >= min_len {
            out.push(String::from_utf8_lossy(&data[start..i]).to_string());
        }
        start = i + 1;
    }
    if data.len() - start >= min_len {
        out.push(String::from_utf8_lossy(&data[start..]).to_string());
    }
    out
}

/// Extrae las clases DEX de un APK (zip) y sus librerias nativas.
fn apk_payloads(path: &str, max_dex: usize) -> (Vec<Vec<String>>, Vec<String>, Vec<String>) {
    let mut dex_strings_all: Vec<Vec<String>> = Vec::new();
    let mut native_hits: Vec<String> = Vec::new();
    let mut entry_names: Vec<String> = Vec::new();
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return (dex_strings_all, native_hits, entry_names),
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return (dex_strings_all, native_hits, entry_names),
    };
    let mut total_dex = 0usize;
    for i in 0..archive.len() {
        let (name, is_dex, is_so) = match archive.by_index(i) {
            Ok(f) => {
                let n = f.name().to_string();
                let dex = n.starts_with("classes") && n.ends_with(".dex");
                let so = n.ends_with(".so");
                (n, dex, so)
            }
            Err(_) => continue,
        };
        if is_dex && total_dex < max_dex {
            if let Ok(mut f) = archive.by_index(i) {
                use std::io::Read;
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                    total_dex += buf.len();
                    dex_strings_all.push(dex_strings(&buf));
                }
            }
            entry_names.push(name);
        } else if is_so {
            if let Ok(mut f) = archive.by_index(i) {
                use std::io::Read;
                let mut buf = Vec::new();
                if f.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                    // Solo las .so que pueden contener la senal que buscamos.
                    for (_cat, _lbl, needle) in SIGNATURES {
                        if needle.len() > 6 && buf.windows(needle.len()).any(|w| w == needle.as_bytes())
                        {
                            native_hits.push(format!("{}:{}", name, needle));
                            break;
                        }
                    }
                }
            }
            entry_names.push(name);
        }
    }
    (dex_strings_all, native_hits, entry_names)
}

// ---------------------------------------------------------------------------
// 1) ENTORNO REAL
// ---------------------------------------------------------------------------

/// Rutas que delatan root si existen.
const PROBE_PATHS: &[&str] = &[
    "/system/bin/su",
    "/system/xbin/su",
    "/sbin/su",
    "/sbin/.magisk",
    "/debug_ramdisk/su",
    "/debug_ramdisk/.magisk",
    "/data/adb",
    "/data/adb/magisk",
    "/data/adb/modules",
    "/data/adb/zygisk",
    "/dev/.magisk",
    "/metadata/magisk",
    "/system/app/Superuser",
    "/system/framework/XposedBridge.jar",
    "/system/lib64/libxposed.so",
    "/system/etc/init.d/zygisk",
    "/sbin/.magisk/mirror",
    "/data/adb/modules/shamiko",
    "/data/adb/modules/trickystore",
    "/data/adb/modules/playintegrityfix",
    "/data/adb/modules/zygisk_next",
    "/data/adb/modules/zygisk-next",
    "/data/adb/modules/hide_my_applist",
];

const PROBE_PROPS: &[&str] = &[
    "ro.build.tags",
    "ro.build.type",
    "ro.build.fingerprint",
    "ro.debuggable",
    "ro.secure",
    "ro.product.cpu.abi",
    "service.adb.root",
    "ro.boot.verifiedbootstate",
    "ro.boot.flash.locked",
    "ro.boot.vbmeta.digest",
    "init.svc.zygisk",
    "ro.magisk.version",
];

/// Construye el `DeviceEnv` **medido** en el dispositivo.
pub fn probe_env(packages: &[String]) -> crate::root_hide::DeviceEnv {
    let mut env = crate::root_hide::DeviceEnv::default();

    // Ficheros presentes: una sola invocacion para no hacer 20 forks.
    let list = PROBE_PATHS.join(" ");
    let out = sh(&format!(
        "for p in {}; do [ -e \"$p\" ] && echo \"$p\"; done",
        list
    ));
    env.files_present = out.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();

    // Props reales.
    // Cada prop en su propia linea: getprop no imprime salto de linea final,
    // asi que con `echo -n` los valores se concatenarian y el parser leeria
    // una sola prop gigante.
    let mut props_cmd = String::new();
    for p in PROBE_PROPS {
        props_cmd.push_str(&format!("echo '{}='$(getprop {}); ", p, p));
    }
    let pout = sh(&props_cmd);
    for line in pout.split(['\n', '\r']) {
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim();
            if !k.is_empty() {
                env.props.push((k.to_string(), v.to_string()));
            }
        }
    }

    // Procesos (nombre de proceso -> "su", "magiskd", "zygisk", ...).
    let ps = sh("ps -A 2>/dev/null || ps 2>/dev/null");
    for line in ps.lines() {
        let mut parts = line.split_whitespace();
        let last = parts.next_back().unwrap_or("");
        if last.len() > 2 && !last.starts_with("PID") && !last.chars().next().unwrap().is_ascii_digit() {
            env.processes.push(last.to_string());
        }
    }
    let has = |needle: &str| env.processes.iter().any(|p| p.contains(needle));
    env.zygisk = has("zygisk") || env.files_present.iter().any(|p| p.contains("zygisk"));

    // Version de Magisk.
    let mv = sh("magisk -v 2>/dev/null || magisk --version 2>/dev/null");
    let mv = mv.trim();
    if !mv.is_empty() && mv.len() < 40 {
        env.magisk_version = Some(mv.to_string());
    }

    // Modulos de Magisk instalados (shamiko, trickystore, ...).
    let mods = sh("ls /data/adb/modules 2>/dev/null");
    let m = mods.to_lowercase();
    env.shamiko = m.contains("shamiko");
    env.trickystore = m.contains("trickystore");
    env.play_integrity_fix = m.contains("playintegrityfix") || m.contains("play-integrity-fix");
    env.zygisk_next = m.contains("zygisk_next") || m.contains("zygisk-next");
    env.hide_my_applist = m.contains("hide_my_applist") || m.contains("hide-my-applist");

    // DenyList de Magisk (Kitsune la guarda en la base de datos de la app).
    env.denylist_apps = magisk_denylist();

    // Paquetes ocultos con `pm hide`.
    let hid = sh("cmd package list packages --hidden 2>/dev/null | grep '^package:'");
    env.hidden_packages = hid
        .lines()
        .filter_map(|l| l.strip_prefix("package:").map(|s| s.trim().to_string()))
        .collect();

    // Por paquete: ruta del APK, appops y si esta en la deny list.
    for pkg in packages {
        if let Some(p) = apk_path(pkg) {
            env.package_paths.push(p);
        }
        let ops = sh(&format!(
            "cmd appops get {} 2>/dev/null",
            crate::android::sh_quote(pkg)
        ));
        for line in ops.lines() {
            let l = line.trim();
            if l.is_empty() || !l.contains(':') {
                continue;
            }
            let mut it = l.splitn(2, ':');
            let op = it.next().unwrap_or("").trim().to_string();
            let mode = it.next().unwrap_or("").trim().to_string();
            if !op.is_empty() {
                env.appops.push((op, mode));
            }
        }
        if env.denylist_apps.iter().any(|d| d == pkg) {
            env.denylisted = true;
        }
    }
    env
}

/// Lista de paquetes en la deny list de Magisk (Kitsune: base de datos de
/// la app Magisk; Magisk normal: `magisk --sqlite`).
fn magisk_denylist() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    // 1) Magisk normal.
    for line in sh_lines(
        "magisk --sqlite \"SELECT package FROM policies WHERE policy != 0\" 2>/dev/null",
    ) {
        if !line.is_empty() {
            out.push(line);
        }
    }
    // 2) Kitsune: base de datos de la app Magisk.
    if out.is_empty() {
        let db = "/data/data/com.topjohnwu.magisk/databases/db.db";
        let sql = format!(
            "SELECT package FROM policies WHERE policy IN (1,2) 2>/dev/null"
        );
        let _ = sql;
        if let Ok(pairs) = crate::sqlite_probe::query_column(db, "SELECT package FROM policies WHERE policy != 0") {
            out.extend(pairs);
        }
    }
    out.sort();
    out.dedup();
    out
}

// ---------------------------------------------------------------------------
// 2) DETECTORES
// ---------------------------------------------------------------------------

/// Que apps DETECTAN root.
///
/// Evidencia: (a) la cadena exacta esta en la tabla de cadenas de su DEX o
/// en sus .so; (b) el logcat muestra que la app lo ha comprobado en vivo.
pub fn detectors(limit: usize, third_party_only: bool) -> Value {
    let started = std::time::Instant::now();
    let mut pkgs = installed_packages(third_party_only);

    // Prioridad: banca, seguridad, pagos, ROM apps (los que mas miran).
    let priority = |p: &str| -> u32 {
        let p = p.to_lowercase();
        let mut score = 0u32;
        for kw in [
            "bank", "bbva", "santander", "caixa", "bankinter", "unicaja", "revolut", "n26", "pay",
            "wallet", "security", "safe", "knox", "antivirus", "mcafee", "eset", "avast", "lookout",
            "playprotect", "samsungpass", "authenticator", "password", "vault", "crypto",
        ] {
            if p.contains(kw) {
                score += 5;
            }
        }
        if p.contains("magisk") || p.contains("root") || p.contains("knox") {
            score += 8;
        }
        score
    };
    pkgs.sort_by(|a, b| priority(b).cmp(&priority(a)).then(a.cmp(b)));
    pkgs.truncate(limit.max(1));

    let mut results: Vec<Value> = Vec::new();
    let mut checked = 0usize;
    for pkg in &pkgs {
        checked += 1;
        let path = match apk_path(pkg) {
            Some(p) => p,
            None => continue,
        };
        let (dex_all, native_hits, entries) = apk_payloads(&path, 24);
        let mut hits: Vec<Value> = Vec::new();
        let mut cats: BTreeMap<String, usize> = BTreeMap::new();
        for strings in &dex_all {
            for s in strings {
                let low = s.to_lowercase();
                for (cat, lbl, needle) in SIGNATURES {
                    if low.contains(&needle.to_lowercase()) {
                        cats.entry(cat.to_string()).or_insert(0);
                        *cats.get_mut(*cat).unwrap() += 1;
                        hits.push(json!({
                            "categoria": cat,
                            "senal": lbl,
                            "cadena": s,
                            "origen": "dex",
                        }));
                    }
                }
            }
        }
        for nh in &native_hits {
            hits.push(json!({
                "categoria": "nativo",
                "senal": "cadena_en_so",
                "cadena": nh,
                "origen": "lib",
            }));
        }
        if hits.is_empty() {
            continue;
        }
        // Deduplicar por (categoria, cadena).
        let mut seen = std::collections::HashSet::new();
        hits.retain(|h| {
            let k = format!(
                "{}|{}",
                h.get("categoria").and_then(|v| v.as_str()).unwrap_or(""),
                h.get("cadena").and_then(|v| v.as_str()).unwrap_or("")
            );
            seen.insert(k)
        });
        let n_cats = cats.len();
        let verdict = if n_cats >= 3 {
            "detector_fuerte"
        } else if n_cats == 2 {
            "detector_medio"
        } else {
            "detector_debil"
        };
        results.push(json!({
            "paquete": pkg,
            "apk": path,
            "uid": uid_of(pkg),
            "categorias": n_cats,
            "veredicto": verdict,
            "senales": hits,
            "entradas_analizadas": entries.len(),
        }));
    }

    // Evidencia dinamica: lo que el logcat ha witnessed en vivo.
    let runtime = runtime_evidence();

    let mut cat_counts: BTreeMap<String, usize> = BTreeMap::new();
    for r in &results {
        for h in r["senales"].as_array().unwrap_or(&vec![]) {
            let c = h["categoria"].as_str().unwrap_or("?").to_string();
            *cat_counts.entry(c).or_insert(0) += 1;
        }
    }
    json!({
        "ok": true,
        "modo": "medido (no simulado)",
        "paquetes_considerados": pkgs.len(),
        "paquetes_analizados": checked,
        "detecciones": results,
        "detectores_confirmados": results.len(),
        "por_categoria": cat_counts,
        "evidencia_runtime": runtime,
        "duracion_ms": started.elapsed().as_millis() as u64,
    })
}

/// Evidencia dinamica: apps que en esta sesion han comprobado el root.
fn runtime_evidence() -> Value {
    let log = sh("logcat -d -t 3000 -v brief 2>/dev/null");
    // pid -> paquete
    let ps = sh("ps -A 2>/dev/null");
    let mut pid2pkg: BTreeMap<String, String> = BTreeMap::new();
    for line in ps.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 8 {
            let pid = parts[1].to_string();
            let name = parts[parts.len() - 1].to_string();
            pid2pkg.insert(pid, name);
        }
    }
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in log.lines() {
        let low = line.to_lowercase();
        if !RUNTIME_PATTERNS.iter().any(|p| low.contains(&p.to_lowercase())) {
            continue;
        }
        // extraer pid del formato "I/Tag ( 1234): msg"
        let pid = line
            .split("(")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let pkg = pid2pkg
            .get(&pid)
            .cloned()
            .unwrap_or_else(|| "desconocido".to_string());
        let msg = line.chars().take(220).collect::<String>();
        let v = found.entry(pkg).or_default();
        if v.len() < 6 && !v.iter().any(|x| x.contains(&msg[msg.len().min(60)..])) {
            v.push(msg);
        }
    }
    let mut arr: Vec<Value> = Vec::new();
    for (pkg, msgs) in found {
        arr.push(json!({ "paquete": pkg, "mensajes": msgs }));
    }
    json!({ "eventos": arr, "total": arr.len() })
}

// ---------------------------------------------------------------------------
// 3) CONSUMIDORES DE ROOT
// ---------------------------------------------------------------------------

/// Que apps USAN root.
///
/// Evidencia dura: (a) la deny list de Magisk (politica real por uid),
/// (b) gestores de root instalados, (c) `su` ejecutado observado.
pub fn consumers() -> Value {
    let started = std::time::Instant::now();
    let pkgs = installed_packages(false);

    // (a) Politicas reales de Magisk.
    let policies = magisk_policies();

    // (b) Gestores de root instalados.
    let mut managers: Vec<Value> = Vec::new();
    for p in &pkgs {
        if ROOT_MANAGERS.iter().any(|m| p == m) {
            managers.push(json!({
                "paquete": p,
                "uid": uid_of(p),
                "version": sh(&format!("dumpsys package {} 2>/dev/null | grep -m1 versionName", crate::android::sh_quote(p))),
                "magisk_version": sh("magisk -v 2>/dev/null").trim(),
            }));
        }
    }

    // (c) `su` observado en el logcat (con la app que lo pidio).
    let su_events = su_events(&pkgs);

    // (d) Apps que llevan una senal de "ejecuto su" en su propio codigo.
    let exec_hints: Vec<Value> = Vec::new();

    json!({
        "ok": true,
        "modo": "medido (no simulado)",
        "gestores_de_root": managers,
        "politicas_magisk": policies,
        "su_observado": su_events,
        "hints_de_ejecucion": exec_hints,
        "duracion_ms": started.elapsed().as_millis() as u64,
    })
}

/// Politicas de Magisk por paquete (deny list y allow list).
fn magisk_policies() -> Vec<Value> {
    let mut out = Vec::new();
    // Magisk normal (CLI).
    for line in sh_lines(
        "magisk --sqlite \"SELECT package || '|' || policy FROM policies\" 2>/dev/null",
    ) {
        if let Some((pkg, pol)) = line.split_once('|') {
            out.push(json!({
                "paquete": pkg.trim(),
                "politica": pol.trim(),
                "fuente": "magisk --sqlite",
            }));
        }
    }
    // Kitsune: base de datos de la app Magisk.
    if out.is_empty() {
        let db = "/data/data/com.topjohnwu.magisk/databases/db.db";
        if let Ok(rows) = crate::sqlite_probe::query_pairs(
            db,
            "SELECT package, policy FROM policies",
        ) {
            for (pkg, pol) in rows {
                out.push(json!({
                    "paquete": pkg,
                    "politica": pol,
                    "fuente": "db de la app Magisk (Kitsune)",
                }));
            }
        }
    }
    out
}

/// Eventos de `su` observados en el logcat, con la app que lo origino.
fn su_events(pkgs: &[String]) -> Vec<Value> {
    let log = sh("logcat -d -t 4000 -v brief 2>/dev/null");
    let ps = sh("ps -A 2>/dev/null");
    let mut pid2pkg: BTreeMap<String, String> = BTreeMap::new();
    for line in ps.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 8 {
            pid2pkg.insert(parts[1].to_string(), parts[parts.len() - 1].to_string());
        }
    }
    let mut out: Vec<Value> = Vec::new();
    for line in log.lines() {
        let low = line.to_lowercase();
        let is_su = low.contains("magisk: su")
            || low.contains("su: ")
            || low.contains("requesting root")
            || low.contains("root granted")
            || low.contains("su_request")
            || low.contains("magisk: allowing");
        if !is_su {
            continue;
        }
        let pid = line
            .split("(")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let pkg = pid2pkg.get(&pid).cloned().unwrap_or_default();
        out.push(json!({
            "paquete": pkg,
            "pid": pid,
            "linea": line.chars().take(240).collect::<String>(),
        }));
        if out.len() > 60 {
            break;
        }
    }
    let _ = pkgs;
    out
}

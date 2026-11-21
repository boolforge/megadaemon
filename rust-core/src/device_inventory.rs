//! W2 · Inventario completo del S21 Ultra (Exynos 2100).
//!
//! Este módulo es la **fuente de verdad** de dos cosas:
//!
//! 1. `capture_plan()` — QUÉ hay que capturar en el dispositivo (ficheros de
//!    referencia + comando de captura + si son privados). El driver de
//!    transporte (MCP) sólo ejecuta ese plan: no duplica la lista.
//! 2. `collect()` — CÓMO se convierte lo capturado en un `inventory.json`
//!    versionado, con el SHA-256 de cada fichero de referencia.
//!
//! Decisiones de diseño (por qué):
//!
//! * **Todo pasa por ficheros de referencia.** El móvil va al 98 % de disco y
//!   con la RAM justa: capturar "en memoria" y filtrarlo por el túnel sería
//!   lento y frágil. Se escriben ficheros pequeños en
//!   `state/inventory/<sello>/` y se leen por trozos (base64) como en W3.
//! * **Cuentas: presencia y estado, NUNCA tokens.** No se lee
//!   `accounts_*.db` ni `auth_tokens`; sólo `dumpsys account`, y de él sólo
//!   el tipo, el número y el hash del nombre. Además el JSON lo declara
//!   explícitamente (`accounts.tokens_included = false`) para que un lector
//!   no tenga que)fiar.
//! * **Privacidad del repo público.** El repo es público: los SSID/BSSID, las
//!   claves ADB autorizadas y los nombres de cuenta NO van en claro al repo;
//!   van hasheados (sha16) y el fichero original sólo a la copia privada de
//!   Drive. `visibility` marca cada referencia como `privada` o `publica`.
//! * **Sin secretos.** Ni tokens, ni claves privadas, ni material de firma.
//!
//! Autocontenido a propósito (sin `crate::`): se puede compilar como arnés
//! con `#[path = "../src/device_inventory.rs"]` para testearlo sin el resto
//! del crate, que es lo que hace `tests/device_inventory_smoke.rs`.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

/// Versión del formato del inventario. Cambia si cambia la forma del JSON.
pub const INVENTORY_VERSION: &str = "w2.1";

/// Permisos que se consideran de riesgo (para el inventario de apps).
pub const RISKY_PERMISSIONS: &[&str] = &[
    "android.permission.ACCESS_BACKGROUND_LOCATION",
    "android.permission.ACCESS_FINE_LOCATION",
    "android.permission.ACCESS_COARSE_LOCATION",
    "android.permission.ACCOUNT_MANAGER",
    "android.permission.BLUETOOTH_CONNECT",
    "android.permission.BLUETOOTH_SCAN",
    "android.permission.BODY_SENSORS",
    "android.permission.CALL_PHONE",
    "android.permission.CAMERA",
    "android.permission.GET_ACCOUNTS",
    "android.permission.INTERNET",
    "android.permission.POST_NOTIFICATIONS",
    "android.permission.PROCESS_OUTGOING_CALLS",
    "android.permission.READ_CALENDAR",
    "android.permission.READ_CALL_LOG",
    "android.permission.READ_CONTACTS",
    "android.permission.READ_EXTERNAL_STORAGE",
    "android.permission.READ_MEDIA_IMAGES",
    "android.permission.READ_MEDIA_VIDEO",
    "android.permission.READ_PHONE_STATE",
    "android.permission.READ_SMS",
    "android.permission.RECEIVE_SMS",
    "android.permission.RECORD_AUDIO",
    "android.permission.SEND_SMS",
    "android.permission.SYSTEM_ALERT_WINDOW",
    "android.permission.READ_MEDIA_AUDIO",
    "android.permission.UWB_RANGING",
    "android.permission.WRITE_EXTERNAL_STORAGE",
];

/// Digest de un fichero de referencia.
#[derive(Debug, Clone, PartialEq)]
pub struct FileDigest {
    pub sha256: String,
    pub bytes: u64,
}

/// Origen de los datos: el dispositivo real o un doble de test.
pub trait Source {
    /// Ejecuta un comando como root y devuelve su salida (o `None`).
    fn shell(&self, cmd: &str) -> Option<String>;
    /// Contenido de un fichero de referencia ya capturado (o `None`).
    fn file(&self, name: &str) -> Option<String>;
    /// SHA-256 y tamaño de un fichero de referencia (o `None`).
    fn digest(&self, name: &str) -> Option<FileDigest>;
}

/// SHA-256 en hexadecimal de un bloque de bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// SHA-256 de un texto, recortado a 16 hex (para nombres que no deben ir en claro).
pub fn sha16(text: &str) -> String {
    sha256_hex(text.as_bytes()).chars().take(16).collect()
}

// ========================================================================= //
// PLAN DE CAPTURA: qué se guarda como fichero de referencia en el móvil
// ========================================================================= //

/// Una captura: identificador, comando (o copiado literal) y visibilidad.
pub struct Capture {
    /// Identificador estable del fichero de referencia.
    pub id: &'static str,
    /// Comando a ejecutar (la salida se guarda tal cual en el fichero).
    pub cmd: &'static str,
    /// Si el fichero contiene datos personales (SSID, cuentas, claves ADB...).
    pub private: bool,
}

/// Definición de una captura literal (copiar un fichero, no ejecutar).
pub struct CopyCapture {
    pub id: &'static str,
    pub path: &'static str,
    pub private: bool,
}

/// Ficheros que se **copian** tal cual (contenido sensible o binario).
pub fn copy_plan() -> &'static [CopyCapture] {
    &[
        CopyCapture {
            id: "adb_keys",
            path: "/data/misc/adb/adb_keys",
            private: true,
        },
        CopyCapture {
            id: "wifi_config_store",
            path: "/data/misc/apexdata/com.android.wifi/WifiConfigStore.xml",
            private: true,
        },
        CopyCapture {
            id: "magisk_modules_dir",
            path: "/data/adb/modules",
            private: false,
        },
    ]
}

/// Comandos que se ejecutan y cuya salida se guarda en un fichero.
pub fn capture_plan() -> &'static [Capture] {
    &[
        Capture { id: "getprop_all", cmd: "getprop", private: false },
        Capture { id: "cpuinfo", cmd: "cat /proc/cpuinfo", private: false },
        Capture { id: "meminfo", cmd: "cat /proc/meminfo", private: false },
        Capture { id: "swaps", cmd: "cat /proc/swaps 2>/dev/null; sysctl vm.swappiness 2>/dev/null", private: false },
        Capture { id: "partitions", cmd: "cat /proc/partitions", private: false },
        Capture { id: "block_by_name", cmd: "ls -la /dev/block/by-name/ 2>/dev/null", private: false },
        Capture { id: "boot_props", cmd: "getprop | grep -iE 'ro\\.boot|bootimage|vbmeta|verifiedboot|verity|avb' 2>/dev/null", private: false },
        Capture { id: "tee_devices", cmd: "ls -la /dev | grep -iE 'tee|keymaster|vertex|sec' 2>/dev/null", private: false },
        Capture { id: "pm_packages", cmd: "pm list packages -f 2>/dev/null", private: false },
        Capture { id: "pm_system", cmd: "pm list packages -s 2>/dev/null", private: false },
        Capture { id: "pm_third_party", cmd: "pm list packages -3 2>/dev/null", private: false },
        Capture { id: "pm_disabled", cmd: "pm list packages -d 2>/dev/null", private: false },
        Capture { id: "pm_versions", cmd: "pm list packages -f --show-versioncode 2>/dev/null || pm list packages --show-versioncode 2>/dev/null", private: false },
        Capture { id: "dumpsys_packages", cmd: "dumpsys package packages 2>/dev/null", private: false },
        Capture { id: "df", cmd: "df -k 2>/dev/null", private: false },
        Capture { id: "du_media", cmd: "du -sk /data/media/0/* 2>/dev/null | sort -rn | head -60", private: false },
        Capture { id: "du_data", cmd: "du -sk /data/* 2>/dev/null | sort -rn | head -40", private: false },
        Capture { id: "net_ifaces", cmd: "ip -o addr 2>/dev/null; echo ---; ip route 2>/dev/null", private: false },
        Capture { id: "net_dns_proxy", cmd: "getprop | grep -iE 'dns' 2>/dev/null; settings get global http_proxy 2>/dev/null; settings get global private_dns_mode 2>/dev/null; settings get global private_dns_specifier 2>/dev/null", private: false },
        Capture { id: "net_vpn", cmd: "dumpsys connectivity 2>/dev/null | grep -iE 'VPN|TRANSPORT_VPN' | head -40; echo ---; ip -o link 2>/dev/null | grep -iE 'tun|ppp|wg' ", private: false },
        Capture { id: "accounts", cmd: "dumpsys account 2>/dev/null | grep -E 'Account \\{|type=|AccountManager' | head -400", private: true },
        Capture { id: "sensors", cmd: "dumpsys sensorservice 2>/dev/null | grep -E '^[0-9]+ \\||handle|vendor|name:' | head -120", private: false },
        Capture { id: "cameras", cmd: "dumpsys media.camera 2>/dev/null | grep -iE 'Number of camera|Camera [0-9]|facing|hal' | head -60", private: false },
        Capture { id: "modem", cmd: "getprop | grep -iE 'gsm\\.|cdma\\.|ro\\.radio|modem|mobile' 2>/dev/null | head -60", private: false },
        Capture { id: "screen", cmd: "wm size; wm density; getprop ro.sf.lcd_density", private: false },
        Capture { id: "selinux", cmd: "getenforce 2>/dev/null; getprop ro.boot.selinux 2>/dev/null; ls -la /sys/fs/selinux 2>/dev/null | head -5", private: false },
        Capture { id: "magisk", cmd: "magisk -v 2>/dev/null; magisk -V 2>/dev/null; echo ---; ls -la /data/adb/modules 2>/dev/null; echo ---; su -v 2>/dev/null; which su 2>/dev/null", private: false },
        Capture { id: "users", cmd: "pm list users 2>/dev/null; echo ---; dumpsys user 2>/dev/null | grep -iE 'UserInfo|Managed|Work|profile' | head -60", private: false },
        Capture { id: "gpu", cmd: "cat /sys/class/kgsl/kgsl-3d0/gpu_busy_percentage 2>/dev/null; cat /sys/class/kgsl/kgsl-3d0/gpu_clocks 2>/dev/null | head -4; getprop ro.hardware.egl; getprop ro.hardware.vulkan", private: false },
        Capture { id: "npu", cmd: "ls -la /dev/vertex* 2>/dev/null; getprop | grep -iE 'npu|vertex|edenn' 2>/dev/null | head -20", private: false },
    ]
}

// ========================================================================= //
// PARSEADORES (puros y testeables)
// ========================================================================= //

/// `getprop` completo → mapa de propiedades.
///
/// El formato REAL de `getprop` es `[clave]: [valor]` (con corchetes), no
/// `clave: valor`; se aceptan las dos formas.
pub fn parse_props(text: &str) -> Map<String, Value> {
    let mut m = Map::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = if let Some(rest) = line.strip_prefix('[') {
            match rest.split_once("]:") {
                Some((k, v)) => (k.trim().to_string(), clean_value(v)),
                None => continue,
            }
        } else if let Some((k, v)) = line.split_once(':') {
            (k.trim().to_string(), clean_value(v))
        } else {
            continue;
        };
        if k.is_empty() {
            continue;
        }
        m.insert(k, json!(v));
    }
    m
}

/// Quita los corchetes y espacios del valor de una propiedad.
fn clean_value(v: &str) -> String {
    v.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim()
        .to_string()
}

/// `/proc/cpuinfo` → SoC, núcleos y hardware.
pub fn parse_cpuinfo(text: &str) -> Value {
    let mut model = String::new();
    let mut hardware = String::new();
    let mut processors = 0usize;
    let mut features = 0usize;
    for line in text.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("Hardware") {
            hardware = v.trim().trim_start_matches(':').trim().to_string();
        }
        if let Some(v) = t.strip_prefix("model name") {
            model = v.trim().trim_start_matches(':').trim().to_string();
        }
        if t.starts_with("processor") && t.contains(':') {
            processors += 1;
        }
        if t.starts_with("Features") {
            features = t.split_whitespace().count().saturating_sub(1);
        }
    }
    json!({
        "model_name": model,
        "hardware": hardware,
        "nucleos_logicos": processors,
        "features_isa": features,
    })
}

/// `/proc/meminfo` → RAM total/libre + swap.
pub fn parse_meminfo(text: &str) -> Value {
    let mut total = 0u64;
    let mut avail = 0u64;
    let mut free = 0u64;
    let mut swap_total = 0u64;
    let mut swap_free = 0u64;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let key = it.next().unwrap_or("");
        let val: u64 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        match key {
            "MemTotal:" => total = val,
            "MemAvailable:" => avail = val,
            "MemFree:" => free = val,
            "SwapTotal:" => swap_total = val,
            "SwapFree:" => swap_free = val,
            _ => {}
        }
    }
    json!({
        "ram_total_kb": total,
        "ram_disponible_kb": avail,
        "ram_libre_kb": free,
        "swap_total_kb": swap_total,
        "swap_libre_kb": swap_free,
        "swap_usada_kb": swap_total.saturating_sub(swap_free),
    })
}

/// `/proc/partitions` → tabla de particiones con tamaño en KB.
pub fn parse_partitions(text: &str) -> Value {
    let mut rows = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            continue;
        }
        if !f[0].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let major: u64 = f[0].parse().unwrap_or(0);
        let minor: u64 = f[1].parse().unwrap_or(0);
        let blocks: u64 = f[2].parse().unwrap_or(0);
        let name = f[3].to_string();
        rows.push(json!({
            "major": major, "minor": minor, "kbytes": blocks, "name": name,
        }));
    }
    json!({ "particiones": rows, "total": rows.len() })
}

/// `df -k` → mapa de sistemas de ficheros.
pub fn parse_df(text: &str) -> Value {
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 6 {
            continue;
        }
        let pct = f[4].trim_end_matches('%');
        rows.push(json!({
            "sistema": f[0],
            "kbytes": f[1].parse::<u64>().unwrap_or(0),
            "usado_kb": f[2].parse::<u64>().unwrap_or(0),
            "libre_kb": f[3].parse::<u64>().unwrap_or(0),
            "uso_pct": pct.parse::<u64>().unwrap_or(0),
            "montado": f[5..].join(" "),
        }));
    }
    json!({ "filesystems": rows })
}

/// `du -sk ruta ...` → mapa de uso por ruta (en KB).
pub fn parse_du(text: &str) -> Value {
    let mut m = Map::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let kb = match it.next().and_then(|v| v.parse::<u64>().ok()) {
            Some(v) => v,
            None => continue,
        };
        let path = line.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
        if path.is_empty() {
            continue;
        }
        m.insert(path, json!(kb));
    }
    Value::Object(m)
}

/// `pm list packages -f` → lista de paquetes con ruta del APK.
pub fn parse_pm_packages(text: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let rest = match line.strip_prefix("package:") {
            Some(r) => r,
            None => continue,
        };
        // Formato real de Android: "package:/ruta/app.apk=com.paquete"
        if let Some((path, pkg)) = rest.rsplit_once('=') {
            out.push(json!({ "paquete": pkg, "ruta_apk": path }));
        } else {
            out.push(json!({ "paquete": rest, "ruta_apk": Value::Null }));
        }
    }
    out
}

/// `pm list packages --show-versioncode` → mapa paquete → versionCode.
pub fn parse_pm_versions(text: &str) -> Map<String, Value> {
    let mut m = Map::new();
    for line in text.lines() {
        let line = line.trim();
        let rest = match line.strip_prefix("package:") {
            Some(r) => r,
            None => continue,
        };
        // "com.paquete versionCode:123" o "ruta.apk=com.paquete versionCode:123"
        let mut pkg = rest.to_string();
        let mut code = Value::Null;
        if let Some((p, c)) = rest.rsplit_once(" versionCode:") {
            pkg = p.rsplit_once('=').map(|x| x.1.to_string()).unwrap_or_else(|| p.to_string());
            code = json!(c.trim().parse::<i64>().unwrap_or(0));
        } else if let Some((p, c)) = rest.rsplit_once('=') {
            pkg = p.to_string();
            let _ = c;
        }
        m.insert(pkg, code);
    }
    m
}

/// Lista simple de paquetes (`pm list packages -s`, `-3`, `-d`).
pub fn parse_pm_simple(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("package:") {
            out.push(rest.to_string());
        }
    }
    out
}

/// Extrae los permisos de riesgo de un bloque de `dumpsys package`.
pub fn risky_permissions(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for perm in RISKY_PERMISSIONS {
        if text.contains(perm) {
            out.push((*perm).to_string());
        }
    }
    out
}

/// `dumpsys account` → presencia y estado por tipo, NUNCA tokens.
pub fn parse_accounts(text: &str) -> Value {
    let mut by_type: Map<String, Value> = Map::new();
    let mut names: Vec<String> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if !t.starts_with("Account {") {
            continue;
        }
        if let Some((name, rest)) = t.split_once("name=") {
            let name = name.trim().trim_end_matches(',').trim();
            if let Some((_ty, after)) = rest.split_once("type=") {
                let ty = after
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-'))
                    .next()
                    .unwrap_or("desconocido")
                    .to_string();
                let entry = by_type.entry(ty.clone()).or_insert_with(|| json!({"total": 0, "nombres_sha16": []}));
                let total = entry["total"].as_u64().unwrap_or(0) + 1;
                entry["total"] = json!(total);
                if let Some(arr) = entry["nombres_sha16"].as_array_mut() {
                    arr.push(json!(sha16(name)));
                }
                names.push(name.to_string());
            }
        }
    }
    json!({
        "tipos": Value::Object(by_type),
        "total_cuentas": names.len(),
        "tokens_included": false,
        "nota": "sólo presencia y estado (tipo + hash del nombre). No se leen accounts_*.db ni auth_tokens.",
    })
}

/// `WifiConfigStore.xml` → SSID/BSSID guardados (hasheados en claro no).
///
/// Android 11+ usa `<string name="SSID">&quot;MiRed&quot;</string>`, no
/// `<SSID>`. Sólo se extrae nombre y BSSID: la PSK **no** se toca (es una
/// clave); sólo se cuenta cuántas redes la tienen.
pub fn parse_wifi_saved(text: &str) -> Value {
    let mut nets = Vec::new();
    for chunk in text.split("<Network>").skip(1) {
        let ssid = xml_value(chunk, "SSID").unwrap_or_default();
        let bssid = xml_value(chunk, "BSSID").unwrap_or_default();
        let psk = xml_value(chunk, "PreSharedKey").unwrap_or_default();
        if ssid.is_empty() && bssid.is_empty() {
            continue;
        }
        nets.push(json!({
            "ssid_sha16": sha16(&ssid),
            "bssid_sha16": sha16(&bssid),
            "ssid_longitud": ssid.chars().count(),
            "contrasena_guardada": !psk.is_empty(),
        }));
    }
    json!({
        "redes_guardadas": nets,
        "total": nets.len(),
        "con_contrasena": nets
            .iter()
            .filter(|n| n["contrasena_guardada"] == json!(true))
            .count(),
        "nota": "SSID/BSSID en claro sólo en la copia privada de Drive; aquí van hasheados (sha16). La contraseña NO se extrae.",
    })
}

/// Extrae el valor de `<string name="CLAVE">valor</string>` (con escapes XML).
fn xml_value(chunk: &str, key: &str) -> Option<String> {
    let needle = format!("name=\"{key}\"");
    let i = chunk.find(&needle)?;
    let rest = &chunk[i + needle.len()..];
    let gt = rest.find('>')?;
    let after = &rest[gt + 1..];
    let end = after.find('<')?;
    Some(
        after[..end]
            .trim()
            .replace("&quot;", "\"")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">"),
    )
}

/// Claves ADB autorizadas: nombres + digest (las claves públicas no son secreto).
pub fn parse_adb_keys(text: &str) -> Value {
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let key = it.next().unwrap_or("");
        let name = it.next().unwrap_or("sin-nombre");
        entries.push(json!({
            "nombre": name,
            "clave_sha16": sha16(key),
            "longitud_clave": key.len(),
        }));
    }
    json!({
        "claves_autorizadas": entries,
        "total": entries.len(),
        "nota": "sólo claves públicas (hash + nombre). La clave privada nunca sale del móvil.",
    })
}

/// `ls -la` de un directorio → mapa nombre → {modo, owner, grupo, destino}.
///
/// Robusto frente a las dos formas de la salida: con fecha (`ls -la` real:
/// `perm links owner group size fecha hora nombre -> destino`) y sin fecha.
pub fn parse_ls_long(text: &str) -> Value {
    let mut m = Map::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            continue;
        }
        let (name, target) = match f.iter().position(|x| *x == "->") {
            Some(pos) if pos >= 1 && pos + 1 < f.len() => {
                (f[pos - 1].to_string(), f[pos + 1].to_string())
            }
            Some(_) => continue,
            None => (f[f.len() - 1].to_string(), String::new()),
        };
        if name == "." || name == ".." || name == "total" {
            continue;
        }
        m.insert(
            name,
            json!({
                "modo": f[0],
                "owner": f.get(2).copied().unwrap_or(""),
                "grupo": f.get(3).copied().unwrap_or(""),
                "destino": target,
            }),
        );
    }
    Value::Object(m)
}

/// `dumpsys package packages` → una entrada por paquete.
///
/// Se parsea el volcado completo (8,4 MB en el S21) en vez de lanzar 1719
/// `dumpsys package` sueltos: en un móvil con la RAM justa, 1719 procesos
/// `app_process` son un riesgo real de OOM.
///
/// Campos por paquete: nombre, versionCode/versionName, min/target SDK, el
/// hash de firma (la plataforma lo **trunca** a 4 bytes en Android 14: se
/// documenta tal cual, sin inventar un hash completo), los permisos pedidos
/// y concedidos, y el estado de habilitación del usuario 0.
pub fn parse_dumpsys_packages(text: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut cur: Option<(String, Value)> = None;
    let mut section = ""; // "install permissions" | "runtime permissions"
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Package [") {
            if let Some((pkg, _)) = rest.split_once(']') {
                if let Some((_, prev)) = cur.take() {
                    out.push(prev);
                }
                let mut v = serde_json::Map::new();
                v.insert("paquete".into(), json!(pkg));
                v.insert("version_code".into(), Value::Null);
                v.insert("version_name".into(), Value::Null);
                v.insert("min_sdk".into(), Value::Null);
                v.insert("target_sdk".into(), Value::Null);
                v.insert("firma_sha256_corto".into(), Value::Null);
                v.insert("permisos_pedidos".into(), json!([]));
                v.insert("permisos_concedidos".into(), json!([]));
                v.insert("habilitada_usuario0".into(), Value::Null);
                cur = Some((pkg.to_string(), Value::Object(v)));
            }
            section = "";
            continue;
        }
        let Some((_, v)) = cur.as_mut() else { continue };
        if t == "install permissions:" {
            section = "install";
            continue;
        }
        if t == "runtime permissions:" {
            section = "runtime";
            continue;
        }
        if t.starts_with("versionCode=") {
            let mut it = t.split_whitespace();
            let vc = it.next().unwrap_or("").trim_start_matches("versionCode=");
            let mut code = vc.to_string();
            let mut min = Value::Null;
            let mut tgt = Value::Null;
            for tok in it {
                if let Some(x) = tok.strip_prefix("minSdk=") {
                    min = json!(x.parse::<i64>().unwrap_or(0));
                }
                if let Some(x) = tok.strip_prefix("targetSdk=") {
                    tgt = json!(x.parse::<i64>().unwrap_or(0));
                }
            }
            code = code.split('=').next().unwrap_or("0").to_string();
            v["version_code"] = json!(code.parse::<i64>().unwrap_or(0));
            v["min_sdk"] = min;
            v["target_sdk"] = tgt;
            continue;
        }
        if let Some(x) = t.strip_prefix("versionName=") {
            v["version_name"] = json!(x.trim());
            continue;
        }
        if t.starts_with("signatures=PackageSignatures{") {
            // Android 14 expone el hash truncado: "signatures:[6052b3ed]".
            if let Some(i) = t.find("signatures:[") {
                let rest = &t[i + "signatures:[".len()..];
                let end = rest.find(']').unwrap_or(rest.len());
                v["firma_sha256_corto"] = json!(rest[..end].trim());
            }
            continue;
        }
        if t.starts_with("User 0:") {
            if let Some(i) = t.find("enabled=") {
                let rest = &t[i + "enabled=".len()..];
                let val: i64 = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0);
                // 0 = DEFAULT (habilitada), 1/2/3 = deshabilitada por el usuario.
                v["habilitada_usuario0"] = json!(val == 0);
            }
            continue;
        }
        if !section.is_empty() && t.contains("android.permission.") {
            let perm = t
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(':')
                .to_string();
            if perm.contains("android.permission.") {
                let granted = t.contains("granted=true");
                if let Some(arr) = v["permisos_pedidos"].as_array_mut() {
                    if !arr.contains(&json!(perm)) {
                        arr.push(json!(perm));
                    }
                }
                if granted {
                    if let Some(arr) = v["permisos_concedidos"].as_array_mut() {
                        if !arr.contains(&json!(perm)) {
                            arr.push(json!(perm));
                        }
                    }
                }
            }
        }
    }
    if let Some((_, v)) = cur.take() {
        out.push(v);
    }
    out
}

/// Filtra de una lista de permisos los que son de riesgo.
pub fn filter_risky(perms: &[Value]) -> Vec<Value> {
    perms
        .iter()
        .filter(|p| {
            p.as_str()
                .map(|s| RISKY_PERMISSIONS.contains(&s))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// Particiones por nombre (`/dev/block/by-name`) → mapa nombre → destino.
pub fn parse_block_by_name(text: &str) -> Value {
    let mut m = Map::new();
    if let Value::Object(ls) = parse_ls_long(text) {
        for (k, v) in ls {
            if let Some(dest) = v.get("destino").and_then(|d| d.as_str()) {
                if !dest.is_empty() {
                    m.insert(k, json!(dest));
                }
            }
        }
    }
    Value::Object(m)
}

// ========================================================================= //
// ENSAMBLADO DEL INVENTARIO
// ========================================================================= //

/// Opciones del inventario.
pub struct Options {
    /// Tope de paquetes con detalle (0 = todos).
    pub app_detail_limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options { app_detail_limit: 0 }
    }
}

/// Ensambla el `inventory.json` completo a partir de lo capturado.
pub fn collect(src: &dyn Source, opts: &Options) -> Value {
    let props = src
        .file("getprop_all")
        .map(|t| parse_props(&t))
        .unwrap_or_default();
    let prop = |k: &str| props.get(k).cloned().unwrap_or(Value::Null);

    // ---- hardware ----
    let cpuinfo = src.file("cpuinfo").unwrap_or_default();
    let meminfo = src.file("meminfo").unwrap_or_default();
    let mem = parse_meminfo(&meminfo);
    let swaps_raw = src.file("swaps").unwrap_or_default();
    let swap_total = mem["swap_total_kb"].as_u64().unwrap_or(0);
    let gpu = src.file("gpu").unwrap_or_default();
    let npu = src.file("npu").unwrap_or_default();
    let screen = src.file("screen").unwrap_or_default();
    let sensors = src.file("sensors").unwrap_or_default();
    let cameras = src.file("cameras").unwrap_or_default();
    let modem = src.file("modem").unwrap_or_default();

    let hardware = json!({
        "modelo": prop("ro.product.model"),
        "dispositivo": prop("ro.product.device"),
        "fabricante": prop("ro.product.manufacturer"),
        "nombre_comercial": prop("ro.product.name"),
        "soc_modelo": prop("ro.soc.model"),
        "soc_fabricante": prop("ro.soc.manufacturer"),
        "soc_plataforma": prop("ro.board.platform"),
        "soc_hardware": prop("ro.hardware"),
        "abi": prop("ro.product.cpu.abi"),
        "cpu": parse_cpuinfo(&cpuinfo),
        "gpu": {
            "egl": prop("ro.hardware.egl"),
            "vulkan": prop("ro.hardware.vulkan"),
            "busy_raw": first_value(&gpu),
            "reloj_raw": lines_after(&gpu, 1).first().cloned().unwrap_or_default(),
            "modelo": prop("ro.product.model"),
        },
        "npu": {
            "dispositivos": parse_ls_long(&npu),
            "props_npu": props
                .iter()
                .filter(|(k, _)| {
                    let k = k.to_ascii_lowercase();
                    k.contains("npu") || k.contains("vertex") || k.contains("edenn")
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Map<String, Value>>(),
        },
        "memoria": {
            "ram_total_kb": mem["ram_total_kb"],
            "ram_disponible_kb": mem["ram_disponible_kb"],
            "swap_total_kb": swap_total,
            "swap_usada_kb": mem["swap_usada_kb"],
            "swaps_raw": swaps_raw.trim().to_string(),
        },
        "pantalla": {
            "tamano": lines_containing(&screen, "Physical size"),
            "densidad": lines_containing(&screen, "dpi"),
            "lcd_density_prop": prop("ro.sf.lcd_density"),
        },
        "sensores": {
            "lineas": sensors.lines().count(),
            "muestra": sensors.lines().take(40).collect::<Vec<_>>(),
        },
        "camaras": {
            "lineas": cameras.lines().count(),
            "muestra": cameras.lines().take(30).collect::<Vec<_>>(),
        },
        "modem": {
            "props": props
                .iter()
                .filter(|(k, _)| {
                    let k = k.to_ascii_lowercase();
                    k.starts_with("gsm.") || k.starts_with("cdma.") || k.contains("radio") || k.contains("modem")
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Map<String, Value>>(),
            "captura": modem.trim().to_string(),
        },
    });

    // ---- software ----
    let magisk_raw = src.file("magisk").unwrap_or_default();
    let selinux_raw = src.file("selinux").unwrap_or_default();
    let users_raw = src.file("users").unwrap_or_default();
    let software = json!({
        "rom": {
            "version_android": prop("ro.build.version.release"),
            "sdk": prop("ro.build.version.sdk"),
            "build_id": prop("ro.build.display.id"),
            "fingerprint": prop("ro.build.fingerprint"),
            "parche_seguridad": prop("ro.build.version.security_patch"),
            "csc": prop("ro.csc.sales_code"),
            "csc_pais": prop("ro.csc.country_code"),
            "csc_iso": prop("ro.csc.countryiso_code"),
            "omc_multi_csc": prop("ro.omc.multi_csc"),
            "csc_ril": prop("ril.official_cscver"),
            "incremental": prop("ro.build.version.incremental"),
            "tags": prop("ro.build.tags"),
            "tipo": prop("ro.build.type"),
            "fecha_build": prop("ro.build.date"),
        },
        "kernel": {
            "uname": src.shell("uname -a").unwrap_or_default().trim().to_string(),
            "version_proc": src.file("cpuinfo").unwrap_or_default().lines().filter(|l| l.starts_with("Linux version")).take(1).collect::<Vec<_>>().join(" "),
        },
        "selinux": {
            "modo": selinux_raw.lines().next().unwrap_or("").trim().to_string(),
            "boot_prop": prop("ro.boot.selinux"),
        },
        "magisk": {
            "version": magisk_raw.lines().next().unwrap_or("").trim().to_string(),
            "version_code": magisk_raw.lines().nth(1).unwrap_or("").trim().to_string(),
            "modulos": magisk_raw
                .lines()
                .skip_while(|l| !l.starts_with("---"))
                .filter(|l| l.starts_with('/') || l.contains("modules"))
                .take(40)
                .collect::<Vec<_>>(),
        },
        "root": {
            "su_presente": magisk_raw.contains("/su") || magisk_raw.contains("su:") || !magisk_raw.is_empty(),
            "magisk_magisk": prop("ro.magisk.version"),
            "contexto": src.shell("id -Z 2>/dev/null").unwrap_or_default().trim().to_string(),
        },
        "perfiles_usuario": {
            "pm_users": users_raw.lines().take(20).collect::<Vec<_>>(),
            "work_profile": users_raw.to_ascii_lowercase().contains("work") || users_raw.to_ascii_lowercase().contains("managed"),
        },
    });

    // ---- boot y particiones ----
    let partitions = src.file("partitions").unwrap_or_default();
    let by_name = src.file("block_by_name").unwrap_or_default();
    let boot_props = src.file("boot_props").unwrap_or_default();
    let boot_map = parse_props(&boot_props);
    let tee_raw = src.file("tee_devices").unwrap_or_default();
    let boot_partitions = json!({
        "tabla_particiones": parse_partitions(&partitions),
        "by_name": parse_block_by_name(&by_name),
        "props_boot": Value::Object(boot_map.clone()),
        "avb": {
            "verified_boot_state": boot_map.get("ro.boot.verifiedbootstate").cloned().unwrap_or(Value::Null),
            "vbmeta_digest": boot_map.get("ro.boot.vbmeta.digest").cloned().unwrap_or(Value::Null),
            "verity_mode": boot_map.get("ro.boot.veritymode").cloned().unwrap_or(Value::Null),
            "bootimage": boot_map.get("ro.bootimage.build.date").cloned().unwrap_or(Value::Null),
            "vbmeta_device_state": boot_map.get("ro.boot.vbmeta.device_state").cloned().unwrap_or(Value::Null),
            "flash_locked": boot_map.get("ro.boot.flash.locked").cloned().unwrap_or(Value::Null),
            "vbmeta_avb_version": boot_map.get("ro.boot.vbmeta.avb_version").cloned().unwrap_or(Value::Null),
            "vbmeta_hash_alg": boot_map.get("ro.boot.vbmeta.hash_alg").cloned().unwrap_or(Value::Null),
            "apk_verity_mode": boot_map.get("ro.apk_verity.mode").cloned().unwrap_or(Value::Null),
            "dmverity": boot_map.get("ro.config.dmverity").cloned().unwrap_or(Value::Null),
            "bootloader_cerrado": boot_map
                .get("ro.boot.flash.locked")
                .and_then(|v| v.as_str())
                .map(|v| v == "1")
                .unwrap_or(false),
        },
        "secure_boot": {
            "secure": boot_map.get("ro.boot.verifiedbootstate").cloned().unwrap_or(Value::Null),
            "dm_verity": boot_map.get("ro.boot.veritymode").cloned().unwrap_or(Value::Null),
        },
        "tee": {
            "dispositivos": parse_ls_long(&tee_raw),
            "props": props
                .iter()
                .filter(|(k, _)| {
                    let k = k.to_ascii_lowercase();
                    k.contains("tee") || k.contains("keymaster") || k.contains("greent")
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Map<String, Value>>(),
            "green_tea": props.keys().any(|k| k.to_ascii_lowercase().contains("greent")),
        },
    });

    // ---- apps ----
    let pm_all = parse_pm_packages(&src.file("pm_packages").unwrap_or_default());
    let system_pkgs = parse_pm_simple(&src.file("pm_system").unwrap_or_default());
    let third_pkgs = parse_pm_simple(&src.file("pm_third_party").unwrap_or_default());
    let disabled = parse_pm_simple(&src.file("pm_disabled").unwrap_or_default());
    let versions = parse_pm_versions(&src.file("pm_versions").unwrap_or_default());
    let dumpsys = parse_dumpsys_packages(&src.file("dumpsys_packages").unwrap_or_default());
    let version_por_paquete: Map<String, Value> = dumpsys
        .iter()
        .map(|d| {
            (
                d["paquete"].as_str().unwrap_or("").to_string(),
                d["version_code"].clone(),
            )
        })
        .collect();
    let mut apps = Vec::new();
    for item in &pm_all {
        let pkg = item["paquete"].as_str().unwrap_or("").to_string();
        let is_system = system_pkgs.iter().any(|s| s == &pkg);
        let is_disabled = disabled.iter().any(|s| s == &pkg);
        let det = dumpsys.iter().find(|d| d["paquete"] == json!(pkg));
        let permisos = det
            .map(|d| d["permisos_pedidos"].as_array().cloned().unwrap_or_default())
            .unwrap_or_default();
        let riesgos = filter_risky(&permisos);
        let enabled_dumpsys = det
            .and_then(|d| d["habilitada_usuario0"].as_bool())
            .unwrap_or(true);
        apps.push(json!({
            "paquete": pkg,
            "ruta_apk": item["ruta_apk"],
            "version_code": version_por_paquete.get(&pkg).cloned().unwrap_or_else(|| versions.get(&pkg).cloned().unwrap_or(Value::Null)),
            "version_name": det.map(|d| d["version_name"].clone()).unwrap_or(Value::Null),
            "min_sdk": det.map(|d| d["min_sdk"].clone()).unwrap_or(Value::Null),
            "target_sdk": det.map(|d| d["target_sdk"].clone()).unwrap_or(Value::Null),
            "firma_sha256_corto": det.map(|d| d["firma_sha256_corto"].clone()).unwrap_or(Value::Null),
            "es_sistema": is_system,
            "es_tercero": !is_system,
            "habilitada": !is_disabled && enabled_dumpsys,
            "permisos_riesgo": riesgos,
            "n_permisos_pedidos": permisos.len(),
        }));
        if opts.app_detail_limit > 0 && apps.len() >= opts.app_detail_limit {
            break;
        }
    }
    let n_total = apps.len();
    let n_system = apps.iter().filter(|a| a["es_sistema"] == json!(true)).count();
    let n_disabled = apps.iter().filter(|a| a["habilitada"] == json!(false)).count();
    let n_disabled_pm = disabled.len();
    let n_risk = apps.iter().filter(|a| {
        a["permisos_riesgo"].as_array().map(|v| !v.is_empty()).unwrap_or(false)
    }).count();
    let apps_section = json!({
        "total": n_total,
        "de_sistema": n_system,
        "de_usuario": n_total - n_system,
        "de_usuario_segun_pm3": third_pkgs.len(),
        "deshabilitadas_pm": n_disabled_pm,
        "deshabilitadas_total": n_disabled,
        "deshabilitadas": n_disabled,
        "con_permisos_de_riesgo": n_risk,
        "paquetes": apps,
        "nota": "hash de firma: Android 14 lo expone TRUNCADO a 4 bytes en dumpsys (PackageSignatures{... signatures:[6052b3ed]}); se documenta tal cual, sin inventar el hash completo.",
        "firma_sha256_completo_disponible": false,
        "firma_sha256_truncado_por_plataforma": true,
    });

    // ---- red ----
    let net_ifaces = src.file("net_ifaces").unwrap_or_default();
    let dns = src.file("net_dns_proxy").unwrap_or_default();
    let vpn = src.file("net_vpn").unwrap_or_default();
    let wifi = src.file("wifi_config_store").unwrap_or_default();
    let adb = src.file("adb_keys").unwrap_or_default();
    let network = json!({
        "interfaces": net_ifaces.trim().to_string(),
        "dns_y_proxy": dns.trim().to_string(),
        "vpn": vpn.trim().to_string(),
        "redes_guardadas": parse_wifi_saved(&wifi),
        "claves_adb_autorizadas": parse_adb_keys(&adb),
    });

    // ---- cuentas (presencia y estado; nunca tokens) ----
    let accounts = parse_accounts(&src.file("accounts").unwrap_or_default());

    // ---- espacio ----
    let df = parse_df(&src.file("df").unwrap_or_default());
    let du_media = parse_du(&src.file("du_media").unwrap_or_default());
    let du_data = parse_du(&src.file("du_data").unwrap_or_default());
    let space = json!({
        "filesystems": df["filesystems"].clone(),
        "uso_por_directorio_media": du_media,
        "uso_por_directorio_data": du_data,
    });

    // ---- referencias (sha256 de cada fichero) ----
    let mut references = Vec::new();
    for c in capture_plan() {
        if let Some(d) = src.digest(c.id) {
            references.push(json!({
                "id": c.id,
                "origen": if c.private {
                    format!("privado:{}", sha16(c.cmd))
                } else {
                    c.cmd.to_string()
                },
                "ruta_absoluta_publicable": !c.private,
                "sha256": d.sha256,
                "bytes": d.bytes,
                "visibilidad": if c.private { "privada" } else { "publica" },
            }));
        }
    }
    for c in copy_plan() {
        if let Some(d) = src.digest(c.id) {
            // Las rutas absolutas de ficheros privados (SSID, claves ADB) son
            // metadato identificable: en la copia pública va el hash de la
            // ruta, y el mapa real sólo en la copia privada de Drive.
            let origen = if c.private {
                format!("privado:{}", sha16(c.path))
            } else {
                c.path.to_string()
            };
            references.push(json!({
                "id": c.id,
                "origen": origen,
                "ruta_absoluta_publicable": !c.private,
                "sha256": d.sha256,
                "bytes": d.bytes,
                "visibilidad": if c.private { "privada" } else { "publica" },
            }));
        }
    }

    json!({
        "version": INVENTORY_VERSION,
        "generado_por": "megadaemon/device_inventory",
        "notas": [
            "Cuentas: sólo presencia y estado, nunca tokens.",
            "SSID/BSSID, claves ADB y nombres de cuenta van hasheados; los ficheros en claro van a la copia privada de Drive.",
            "No se incluyen tokens, claves privadas ni material de firma.",
        ],
        "hardware": hardware,
        "software": software,
        "boot_y_particiones": boot_partitions,
        "apps": apps_section,
        "red": network,
        "cuentas": accounts,
        "espacio": space,
        "referencias": references,
    })
}

/// Manifiesto NDJSON: una línea por fichero de referencia, con su sha256.
pub fn manifest_ndjson(src: &dyn Source) -> String {
    let mut out = String::new();
    for c in capture_plan() {
        if let Some(d) = src.digest(c.id) {
            out.push_str(&format!(
                "{{\"id\":\"{}\",\"sha256\":\"{}\",\"bytes\":{},\"visibilidad\":\"{}\"}}\n",
                c.id,
                d.sha256,
                d.bytes,
                if c.private { "privada" } else { "publica" }
            ));
        }
    }
    for c in copy_plan() {
        if let Some(d) = src.digest(c.id) {
            out.push_str(&format!(
                "{{\"id\":\"{}\",\"sha256\":\"{}\",\"bytes\":{},\"visibilidad\":\"{}\"}}\n",
                c.id,
                d.sha256,
                d.bytes,
                if c.private { "privada" } else { "publica" }
            ));
        }
    }
    out
}

// ---------- helpers de parseo ----------

fn first_value(text: &str) -> String {
    text.lines().next().unwrap_or("").trim().to_string()
}

fn lines_after(text: &str, n: usize) -> Vec<String> {
    text.lines().skip(n).map(|l| l.trim().to_string()).collect()
}

fn lines_containing(text: &str, needle: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.contains(needle))
        .map(|l| l.trim().to_string())
        .collect()
}

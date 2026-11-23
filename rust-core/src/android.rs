//! MegaDaemon v13.3 — Capa de control del dispositivo Android.
//!
//! Absorbe las capacidades del servidor MCP "android-root" (Python + shell)
//! y las implementa **en Rust, tipadas**, que es la diferencia real:
//!
//! | Python (chapucilla)                | Rust (esto)                                  |
//! |-----------------------------------|----------------------------------------------|
//! | `subprocess.run("su -mm -c " + cmd)`| `su -mm -c` con argv propio, sin concatenar     |
//! | el LLM elige el shell               | el LLM elige **parámetros tipados**            |
//! | sin timeout                        | timeout obligatorio (30 s por defecto, 300 s)  |
//! | sin límite de salida               | tope de salida (64 KB por defecto, 512 KB)    |
//! | duplicados vía jdupes (hay que     | SHA-256 nativo en streaming, sin dependencias  |
//! | compilarlo a mano)                 |                                                 |
//! | `rm -rf` sin pedir permiso         | política destructiva: exige `confirm` explícito|
//! | sin registro                       | auditoría de cada llamada en `audit.ndjson`    |
//! | 200/500 sin contexto               | errores estructurados con código y pista      |
//!
//! Todo lo que sale a la red o escribe en disco pasa por aquí.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Tope por defecto de salida de una orden.
pub const DEFAULT_MAX_CHARS: usize = 64 * 1024;
/// Tope absoluto.
pub const MAX_CHARS_CEILING: usize = 512 * 1024;
/// Timeout por defecto.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// Timeout máximo.
pub const MAX_TIMEOUT_MS: u64 = 300_000;

/// Comando que exige confirmación explícita (`confirm_destructive`).
const DESTRUCTIVE_MARKERS: &[&str] = &[
    // borrado masivo
    "rm -rf", "rm -fr", "rm -r -f", "rm -f -r", "rm --recursive", "rm --force",
    "--no-preserve-root", "shred", "wipe", "wipe_data",
    // particiones / flash
    "mkfs", "mke2fs", "mkswap", "fdisk", "sfdisk", "parted", "blkdiscard",
    "dd if=", "dd of=", "flash_erase", "fastboot", "recovery --wipe",
    "> /dev/block", "> /dev/mmcblk", "/dev/block/sd",
    // arranque
    "reboot bootloader", "reboot fastboot", "enter bootloader",
    // denegación de servicio
    ":(){", ":|:&",
    // ejecución de código descargado (un agente no debe correr lo que baja)
    "curl | sh", "curl | bash", "wget | sh", "wget | bash", "| sh", "| bash",
    // destruye datos de forma silenciosa
    "cat /dev/zero", "cat /dev/urandom >", "mv /data", "mv /system", "mv /sdcard",
];
/// Escapa un valor para interpolarlo **de forma segura** en un `sh -c`.
/// Todo lo que venga del modelo pasa por aquí: sin esto, un parámetro con
/// `;` o `$(...)` es ejecución arbitraria.
pub fn sh_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Normaliza un comando para que la política destructiva no se esquive con
/// espacios de más, tabuladores ni mayúsculas (`rm  -rf /` ≡ `rm -rf /`).
fn normalize_cmd(cmd: &str) -> String {
    let mut out = String::with_capacity(cmd.len());
    let mut prev_space = false;
    for c in cmd.chars() {
        let lower = c.to_ascii_lowercase();
        if lower.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(lower);
            prev_space = false;
        }
    }
    out
}

/// ¿El comando toca algo irreversible?
///
/// BUG v15 (ADV-A-03): era una lista de subcadenas sobre el comando en
/// minúsculas ⇒ se esquivaba con un espacio de más (`rm  -rf /`), con una
/// opción larga (`rm -rf --no-preserve-root /`) o con otra forma de borrar
/// (`cat /dev/zero > /dev/block/sda`).  Ahora se normalizan los espacios y
/// la lista cubre las variantes.
pub fn is_destructive(cmd: &str) -> bool {
    let c = normalize_cmd(cmd);
    DESTRUCTIVE_MARKERS.iter().any(|m| c.contains(m))
}

/// Resultado de una ejecución.
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub truncated: bool,
}

impl ExecResult {
    pub fn to_json(&self) -> Value {
        json!({
            "stdout": self.stdout,
            "stderr": self.stderr,
            "exit_code": self.code,
            "timed_out": self.timed_out,
            "duration_ms": self.duration_ms,
            "truncated": self.truncated,
        })
    }
    /// Texto combinado para las herramientas de solo lectura.
    pub fn combined(&self) -> String {
        if self.stderr.trim().is_empty() {
            self.stdout.clone()
        } else if self.stdout.trim().is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n[stderr]\n{}", self.stdout, self.stderr)
        }
    }
}

/// Ejecuta un comando como root real (`su -mm -c`), con timeout y tope.
///
/// `su -mm -c` (mount namespace completo) es lo que permite operar sobre
/// `/data/adb`, `/system` y los módulos de Magisk sin restricciones.
pub fn root_exec(command: &str, timeout_ms: u64, max_chars: usize) -> ExecResult {
    let timeout_ms = timeout_ms.clamp(1_000, MAX_TIMEOUT_MS);
    let max_chars = max_chars.clamp(1_024, MAX_CHARS_CEILING);
    let started = Instant::now();

    let mut child = match Command::new("su")
        .arg("-mm")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return ExecResult {
                stdout: String::new(),
                stderr: format!("no se pudo ejecutar 'su': {}", e),
                code: None,
                timed_out: false,
                duration_ms: started.elapsed().as_millis() as u64,
                truncated: false,
            }
        }
    };

    // Lectura en hilo aparte para no bloquear en un pipe lleno.
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(_) => break None,
        }
    };

    let out_bytes = out_handle.join().unwrap_or_default();
    let err_bytes = err_handle.join().unwrap_or_default();
    let mut truncated = false;

    let mut stdout = String::from_utf8_lossy(&out_bytes).into_owned();
    let mut stderr = String::from_utf8_lossy(&err_bytes).into_owned();
    if stdout.len() > max_chars {
        stdout.truncate(max_chars);
        truncated = true;
        stdout.push_str("\n[...salida truncada por el límite de caracteres...]");
    }
    if stderr.len() > max_chars {
        stderr.truncate(max_chars);
        truncated = true;
    }

    ExecResult {
        stdout,
        stderr,
        code: status.and_then(|s| s.code()),
        timed_out,
        duration_ms: started.elapsed().as_millis() as u64,
        truncated,
    }
}

/// Registro de auditoría (JSON Lines) de cada llamada.
pub fn audit(event: &str, detail: &Value) {
    let dir = crate::cockpit_server::state_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = format!("{}/audit.ndjson", dir);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!(
        "{{\"ts\":{},\"event\":\"{}\",\"detail\":{}}}\n",
        ts,
        event.replace('"', "'"),
        detail
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Error estructurado.
fn err(code: &str, hint: &str) -> Value {
    json!({ "ok": false, "error": code, "hint": hint })
}

// ========================================================================= //
// system_overview
// ========================================================================= //
pub fn system_overview() -> Value {
    let script = r#"
echo "=== STORAGE ==="
df -h /data /sdcard 2>/dev/null | head -4
echo "=== MEM ==="
free -m 2>/dev/null | head -3
echo "=== COUNTS ==="
echo "user_apps=$(pm list packages -3 2>/dev/null | wc -l)"
echo "total_apps=$(pm list packages 2>/dev/null | wc -l)"
echo "disabled=$(pm list packages -d 2>/dev/null | wc -l)"
echo "=== ANDROID ==="
echo "version=$(getprop ro.build.version.release)"
echo "sdk=$(getprop ro.build.version.sdk)"
echo "model=$(getprop ro.product.model)"
echo "device=$(getprop ro.product.device)"
echo "soc=$(getprop ro.soc.model)"
echo "patch=$(getprop ro.build.version.security_patch)"
echo "csc=$(getprop ro.csc.sales_code)"
echo "=== SELINUX ==="
getenforce 2>/dev/null
echo "=== MAGISK ==="
magisk -v 2>/dev/null
echo "=== NPU ==="
ls /vendor/lib64/ 2>/dev/null | grep -i neural | head -3
echo "=== TOP DIRS /data/media/0 ==="
du -k -d 1 /data/media/0 2>/dev/null | sort -rn | head -12
"#;
    let r = root_exec(script, 60_000, DEFAULT_MAX_CHARS);
    json!({ "ok": r.code == Some(0), "report": r.combined(), "duration_ms": r.duration_ms })
}

// ========================================================================= //
// props_and_settings
// ========================================================================= //
pub fn props_and_settings(action: &str, namespace: &str, key: &str, value: &str, delete: bool) -> Value {
    match action {
        "get" => {
            if key.is_empty() {
                let r = root_exec("getprop", 15_000, DEFAULT_MAX_CHARS);
                json!({ "ok": true, "props": r.stdout })
            } else {
                let r = root_exec(
                    &format!("getprop {}", sh_quote(key)),
                    15_000,
                    4096,
                );
                json!({ "ok": true, "key": key, "value": r.stdout.trim() })
            }
        }
        "set_prop" => {
            if key.is_empty() || value.is_empty() {
                return err("missing_key_value", "hace falta key y value");
            }
            let r = root_exec(
                &format!("resetprop {} {}", sh_quote(key), sh_quote(value)),
                20_000,
                8192,
            );
            audit("props.set", &json!({"key": key}));
            json!({ "ok": r.code == Some(0), "key": key, "output": r.combined() })
        }
        "settings_get" => {
            let ns = match namespace {
                "global" | "secure" | "system" => namespace,
                _ => return err("bad_namespace", "namespace debe ser global|secure|system"),
            };
            let cmd = if key.is_empty() {
                format!("settings list {}", ns)
            } else {
                format!("settings get {} {}", ns, sh_quote(key))
            };
            let r = root_exec(&cmd, 20_000, DEFAULT_MAX_CHARS);
            json!({ "ok": true, "namespace": ns, "value": r.combined() })
        }
        "settings_put" => {
            let ns = match namespace {
                "global" | "secure" | "system" => namespace,
                _ => return err("bad_namespace", "namespace debe ser global|secure|system"),
            };
            if key.is_empty() {
                return err("missing_key", "hace falta key");
            }
            let cmd = if delete {
                format!("settings delete {} {}", ns, sh_quote(key))
            } else {
                format!("settings put {} {} {}", ns, sh_quote(key), sh_quote(value))
            };
            let r = root_exec(&cmd, 20_000, 8192);
            audit("settings.put", &json!({"ns": ns, "key": key, "delete": delete}));
            json!({ "ok": r.code == Some(0), "output": r.combined() })
        }
        other => err("bad_action", &format!("acción no soportada: {}", other)),
    }
}

// ========================================================================= //
// selinux_control
// ========================================================================= //
pub fn selinux_control(action: &str, rule: &str) -> Value {
    let cmd = match action {
        "status" => "getenforce".to_string(),
        "set_permissive" => "setenforce 0".to_string(),
        "set_enforcing" => "setenforce 1".to_string(),
        "live_policy" => {
            if rule.is_empty() {
                return err("missing_rule", "hace falta la regla");
            }
            format!("magiskpolicy --live {}", sh_quote(rule))
        }
        other => return err("bad_action", &format!("acción no soportada: {}", other)),
    };
    let r = root_exec(&cmd, 20_000, 16_384);
    audit("selinux", &json!({"action": action}));
    json!({ "ok": r.code == Some(0), "action": action, "output": r.combined() })
}

// ========================================================================= //
// magisk_module
// ========================================================================= //
pub fn magisk_module(
    action: &str,
    module_id: &str,
    name: &str,
    description: &str,
    zip_path: &str,
    enable: bool,
    dry_run: bool,
) -> Value {
    let id = module_id.trim();
    match action {
        "validate_zip" => {
            if zip_path.trim().is_empty() {
                return err("missing_zip", "hace falta zip_path");
            }
            let info = crate::magisk_module::validate_module_zip(zip_path);
            json!({ "ok": info.valid, "zip": info.to_json() })
        }
        "install" => {
            if zip_path.trim().is_empty() {
                return err("missing_zip", "hace falta zip_path");
            }
            crate::magisk_module::install_module_zip(zip_path, enable, dry_run)
        }
        "shamiko" => {
            if zip_path.trim().is_empty() {
                // Sin ZIP: se devuelve la ficha de la release y el estado real.
                return json!({
                    "ok": true,
                    "release": crate::magisk_module::shamiko_release_info(),
                    "status": crate::magisk_module::shamiko_status(),
                    "note": "sin zip_path no se instala nada: se devuelve la ficha de la \
                             release oficial y el estado medido en el movil",
                });
            }
            crate::magisk_module::shamiko_prepare(zip_path, enable, dry_run)
        }
        "list" => {
            let script = r#"
for d in /data/adb/modules/*/; do
  [ -d "$d" ] || continue
  i=$(basename "$d"); [ "$i" = "update" ] && continue
  st=$( [ -f "$d/disable" ] && echo DISABLED || echo enabled )
  v=$(grep -m1 '^version=' "$d/module.prop" 2>/dev/null | cut -d= -f2-)
  printf '%s|%s|%s\n' "$i" "$st" "$v"
done
magisk --sqlite "SELECT key,value FROM settings;" 2>/dev/null
"#;
            let r = root_exec(script, 30_000, 64 * 1024);
            json!({ "ok": true, "modules": r.stdout })
        }
        "enable" | "disable" => {
            if id.is_empty() {
                return err("missing_module", "hace falta module_id");
            }
            let path = format!("/data/adb/modules/{}", id);
            if !path.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                return err("bad_module_id", "module_id con caracteres no permitidos");
            }
            let cmd = if action == "enable" {
                format!("rm -f {}/disable && echo enabled", sh_quote(&path))
            } else {
                format!("touch {}/disable && echo disabled", sh_quote(&path))
            };
            let r = root_exec(&cmd, 15_000, 4096);
            audit("magisk.module", &json!({"action": action, "id": id}));
            json!({ "ok": r.code == Some(0), "module": id, "action": action, "output": r.combined() })
        }
        "remove" => {
            if id.is_empty() {
                return err("missing_module", "hace falta module_id");
            }
            if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                return err("bad_module_id", "module_id con caracteres no permitidos");
            }
            // Nunca se borra sin confirmar: quitar un módulo puede dejar el
            // móvil sin root.
            let cmd = format!(
                "mv /data/adb/modules/{} /data/adb/modules_update/ 2>/dev/null || mv /data/adb/modules/{} /data/local/tmp/megadaemon-module-removed-{}",
                sh_quote(id),
                sh_quote(id),
                sh_quote(id)
            );
            let r = root_exec(&cmd, 20_000, 8192);
            audit("magisk.module", &json!({"action": "remove", "id": id}));
            json!({ "ok": r.code == Some(0), "module": id, "output": r.combined(),
                    "note": "movido, no borrado: reversible hasta el próximo reinicio" })
        }
        "create_or_update" => {
            let id = if id.is_empty() { "megadaemon_extra" } else { id };
            if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return err("bad_module_id", "module_id con caracteres no permitidos");
            }
            let base = format!("/data/adb/modules/{}", id);
            let script = format!(
                "mkdir -p {d}/system/etc && printf 'id={id}\\nname={name}\\nversion=v1.0\\nversionCode=1\\nauthor=boolforge\\ndescription={desc}\\n' > {d}/module.prop && chmod 755 {d} && echo OK",
                d = sh_quote(&base),
                id = sh_quote(id),
                name = sh_quote(if name.is_empty() { "MegaDaemon Extra" } else { name }),
                desc = sh_quote(if description.is_empty() { "Modulo auxiliar de MegaDaemon" } else { description }),
            );
            let r = root_exec(&script, 20_000, 8192);
            audit("magisk.module", &json!({"action": "create_or_update", "id": id}));
            json!({ "ok": r.code == Some(0), "module": id, "output": r.combined() })
        }
        other => err("bad_action", &format!("acción no soportada: {}", other)),
    }
}

// ========================================================================= //
// root_occlusion_audit
// ========================================================================= //

/// Auditoría de ocultación de root.
///
/// `packages` es una lista (JSON array) o una lista separada por comas.
/// `mode` decide el alcance:
///
/// * `audit` (por defecto) — tabla completa app → señal → veredicto.
/// * `state` — sólo el estado actual de las señales (qué queda a la vista).
/// * `exec` — sólo la prueba de ejecución (logcat de la app en marcha).
/// * `static` — sólo el escaneo estático del APK.
/// * `csc` — el valor real de la feature de grabación del CSC activo.
pub fn root_occlusion_audit(packages: &Value, package: &str, mode: &str) -> Value {
    let mut pkgs: Vec<String> = Vec::new();
    if let Some(arr) = packages.as_array() {
        for v in arr {
            if let Some(s) = v.as_str() {
                if !s.trim().is_empty() {
                    pkgs.push(s.trim().to_string());
                }
            }
        }
    } else if let Some(s) = packages.as_str() {
        for part in s.split(',') {
            if !part.trim().is_empty() {
                pkgs.push(part.trim().to_string());
            }
        }
    }
    if !package.trim().is_empty() {
        pkgs.push(package.trim().to_string());
    }
    let mode = if mode.is_empty() { "audit" } else { mode };
    match mode {
        "state" => {
            let ref_app = pkgs
                .first()
                .cloned()
                .unwrap_or_else(|| "com.android.settings".to_string());
            let states = crate::root_occlusion::occlusion_state(&ref_app);
            json!({
                "ok": true,
                "reference_app": ref_app,
                "signals": states.iter().map(|s| json!({
                    "signal": s.signal,
                    "visible": s.visible,
                    "hidden_by": s.hidden_by,
                    "evidence": s.evidence,
                    "status": s.status,
                })).collect::<Vec<_>>(),
            })
        }
        "exec" => {
            if pkgs.is_empty() {
                return err("missing_package", "hace falta package (o packages)");
            }
            let mut out = Vec::new();
            for p in &pkgs {
                out.push(crate::root_occlusion::execution_test(p));
            }
            json!({ "ok": true, "executions": out })
        }
        "static" => {
            if pkgs.is_empty() {
                return err("missing_package", "hace falta package (o packages)");
            }
            let mut out = Vec::new();
            for p in &pkgs {
                let path = crate::root_occlusion::package_apk_path(p).unwrap_or_default();
                let scan = crate::root_occlusion::scan_apk(&path);
                out.push(json!({
                    "package": p,
                    "apk_path": path,
                    "apk_size": scan.apk_size,
                    "dex_count": scan.dex_count,
                    "so_count": scan.so_count,
                    "hits": scan.hits.iter().map(|h| json!({
                        "signal": h.signal,
                        "source": h.source,
                        "evidence": h.evidence,
                    })).collect::<Vec<_>>(),
                    "warnings": scan.warnings,
                }));
            }
            json!({ "ok": true, "static": out })
        }
        "csc" => {
            let live = crate::root_occlusion::csc_feature_live();
            json!({
                "ok": true,
                "path": crate::root_occlusion::CSC_ACTIVE_PATH,
                "sha256": live.as_ref().map(|(s, _)| s.clone()),
                "feature": crate::csc_verify::FEATURE,
                "value": live.as_ref().and_then(|(_, v)| v.clone()),
                "variant": live
                    .as_ref()
                    .and_then(|(_, v)| v.as_ref().map(|s| crate::csc_verify::classify_variant(s))),
                "module_active": crate::root_occlusion::device_path_exists(
                    "/data/adb/modules/megadaemon_csc_recording/cscfeature.xml"),
            })
        }
        _ => {
            if pkgs.is_empty() {
                return err("missing_package", "hace falta package (o packages)");
            }
            let report = crate::root_occlusion::audit(&pkgs);
            json!({ "ok": true, "audit": report })
        }
    }
}

// ========================================================================= //
// kernel_and_blocks
// ========================================================================= //
pub fn kernel_and_blocks(action: &str, lines: u32, partition: &str, dst: &str) -> Value {
    let lines = lines.clamp(10, 5000);
    match action {
        "info" => {
            let script = "uname -a; cat /proc/version; getprop ro.product.model; getprop ro.soc.model; getprop ro.build.version.release; getprop ro.build.version.security_patch; getprop ro.boot.hardware";
            let r = root_exec(script, 20_000, 16_384);
            json!({ "ok": true, "info": r.combined() })
        }
        "dmesg" => {
            let r = root_exec(
                &format!("dmesg 2>/dev/null | tail -n {}", lines),
                30_000,
                256 * 1024,
            );
            json!({ "ok": true, "dmesg": r.combined() })
        }
        "processes" => {
            let r = root_exec(
                "ps -A -o PID,USER,RSS,NAME,ARGS 2>/dev/null | head -60",
                25_000,
                64 * 1024,
            );
            json!({ "ok": true, "processes": r.stdout })
        }
        "partitions" => {
            let r = root_exec(
                "ls -la /dev/block/by-name/ 2>/dev/null | head -80; echo '--- mounts ---'; cat /proc/mounts | grep -E ' / | /data | /system | /vendor ' | head -10",
                20_000,
                32 * 1024,
            );
            json!({ "ok": true, "partitions": r.combined() })
        }
        "dump_partition" => {
            if partition.is_empty() || dst.is_empty() {
                return err("missing_args", "hace falta partition y dst");
            }
            if !partition
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                return err("bad_partition", "nombre de partición no permitido");
            }
            if !dst.starts_with('/') || dst.contains("..") {
                return err("bad_dst", "dst debe ser una ruta absoluta sin '..'");
            }
            let src = format!("/dev/block/by-name/{}", partition);
            let cmd = format!(
                "mkdir -p $(dirname {}) && dd if={} of={} bs=4M 2>&1 | tail -3 && ls -la {}",
                sh_quote(dst),
                sh_quote(&src),
                sh_quote(dst),
                sh_quote(dst)
            );
            let r = root_exec(&cmd, 300_000, 32 * 1024);
            audit("kernel.dump_partition", &json!({"partition": partition, "dst": dst}));
            json!({ "ok": r.code == Some(0), "output": r.combined() })
        }
        other => err("bad_action", &format!("acción no soportada: {}", other)),
    }
}

// ========================================================================= //
// app_control
// ========================================================================= //
pub fn app_control(action: &str, scope: &str, package: &str, permission: &str, op: &str, mode: &str, apk_path: &str, keep_data: bool) -> Value {
    let pkg_ok = |p: &str| {
        !p.is_empty()
            && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    };
    match action {
        "list" => {
            let scope = match scope {
                "third_party" => "-3",
                "system" => "-s",
                "disabled" => "-d",
                "all" => "",
                _ => return err("bad_scope", "scope: third_party|system|disabled|all"),
            };
            let cmd = if scope.is_empty() {
                "pm list packages".to_string()
            } else {
                format!("pm list packages {}", scope)
            };
            let r = root_exec(&cmd, 60_000, 256 * 1024);
            let list: Vec<String> = r
                .stdout
                .lines()
                .filter_map(|l| l.strip_prefix("package:").map(|s| s.trim().to_string()))
                .collect();
            json!({ "ok": true, "count": list.len(), "packages": list })
        }
        "info" => {
            if !pkg_ok(package) {
                return err("bad_package", "package no válido");
            }
            let r = root_exec(
                &format!("dumpsys package {} | head -120", sh_quote(package)),
                30_000,
                64 * 1024,
            );
            json!({ "ok": true, "package": package, "info": r.combined() })
        }
        "disable" | "enable" => {
            if !pkg_ok(package) {
                return err("bad_package", "package no válido");
            }
            let cmd = if action == "disable" {
                format!("pm disable-user --user 0 {}", sh_quote(package))
            } else {
                format!("pm enable {}", sh_quote(package))
            };
            let r = root_exec(&cmd, 30_000, 8192);
            audit("app.control", &json!({"action": action, "package": package}));
            json!({ "ok": r.code == Some(0), "package": package, "output": r.combined() })
        }
        "uninstall" => {
            if !pkg_ok(package) {
                return err("bad_package", "package no válido");
            }
            let cmd = if keep_data {
                format!("pm uninstall -k {}", sh_quote(package))
            } else {
                format!("pm uninstall {}", sh_quote(package))
            };
            let r = root_exec(&cmd, 120_000, 16_384);
            audit("app.uninstall", &json!({"package": package, "keep_data": keep_data}));
            json!({ "ok": r.code == Some(0), "package": package, "output": r.combined() })
        }
        "clear_data" | "clear_cache" => {
            if !pkg_ok(package) {
                return err("bad_package", "package no válido");
            }
            let cmd = if action == "clear_data" {
                format!("pm clear {}", sh_quote(package))
            } else {
                format!("pm trim-caches 1G {}", sh_quote(package))
            };
            let r = root_exec(&cmd, 180_000, 16_384);
            audit("app.control", &json!({"action": action, "package": package}));
            json!({ "ok": r.code == Some(0), "package": package, "output": r.combined() })
        }
        "grant_perm" | "revoke_perm" => {
            if !pkg_ok(package) || permission.is_empty() {
                return err("missing_args", "hace falta package y permission");
            }
            if !permission.chars().all(|c| {
                c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '/'
            }) {
                return err("bad_permission", "permission no válida");
            }
            let cmd = if action == "grant_perm" {
                format!("pm grant {} {}", sh_quote(package), sh_quote(permission))
            } else {
                format!("pm revoke {} {}", sh_quote(package), sh_quote(permission))
            };
            let r = root_exec(&cmd, 30_000, 8192);
            json!({ "ok": r.code == Some(0), "output": r.combined() })
        }
        "appops" => {
            if !pkg_ok(package) {
                return err("bad_package", "package no válido");
            }
            let cmd = if op.is_empty() {
                format!("appops get {} | head -80", sh_quote(package))
            } else {
                format!(
                    "appops set {} {} {}",
                    sh_quote(package),
                    sh_quote(op),
                    sh_quote(if mode.is_empty() { "allow" } else { mode })
                )
            };
            let r = root_exec(&cmd, 30_000, 32 * 1024);
            json!({ "ok": true, "appops": r.combined() })
        }
        "install_apk" => {
            if apk_path.is_empty() {
                return err("missing_apk", "hace falta apk_path");
            }
            if !apk_path.starts_with('/') {
                return err("bad_apk_path", "apk_path debe ser absoluto");
            }
            let r = root_exec(
                &format!("pm install -r -g {}", sh_quote(apk_path)),
                300_000,
                32 * 1024,
            );
            audit("app.install", &json!({"apk": apk_path}));
            json!({ "ok": r.code == Some(0), "output": r.combined() })
        }
        "open" => {
            if !pkg_ok(package) {
                return err("bad_package", "package no válido");
            }
            let r = root_exec(
                &format!("monkey -p {} -c android.intent.category.LAUNCHER 1", sh_quote(package)),
                30_000,
                8192,
            );
            json!({ "ok": r.code == Some(0), "output": r.combined() })
        }
        other => err("bad_action", &format!("acción no soportada: {}", other)),
    }
}

// ========================================================================= //
// screen_and_ui
// ========================================================================= //
pub fn screen_and_ui(action: &str, x: i32, y: i32, x1: i32, y1: i32, x2: i32, y2: i32, duration_ms: i32, key: &str, text: &str, save_path: &str) -> Value {
    match action {
        "screenshot" => {
            let path = if save_path.is_empty() {
                "/data/local/tmp/megadaemon/screen.png".to_string()
            } else {
                save_path.to_string()
            };
            if !path.starts_with('/') || path.contains("..") {
                return err("bad_path", "save_path debe ser absoluto y sin '..'");
            }
            let cmd = format!(
                "mkdir -p $(dirname {}) && screencap -p {} && ls -la {}",
                sh_quote(&path),
                sh_quote(&path),
                sh_quote(&path)
            );
            let r = root_exec(&cmd, 30_000, 8192);
            json!({ "ok": r.code == Some(0), "path": path, "output": r.combined() })
        }
        "dump" => {
            let cmd = "uiautomator dump /data/local/tmp/megadaemon/ui.xml >/dev/null 2>&1; cat /data/local/tmp/megadaemon/ui.xml 2>/dev/null | head -c 200000";
            let r = root_exec(cmd, 45_000, 256 * 1024);
            json!({ "ok": true, "ui": r.stdout })
        }
        "tap" => {
            let r = root_exec(
                &format!("input tap {} {}", x, y),
                15_000,
                4096,
            );
            json!({ "ok": r.code == Some(0), "output": r.combined() })
        }
        "swipe" => {
            let d = duration_ms.clamp(50, 5000);
            let r = root_exec(
                &format!("input swipe {} {} {} {} {}", x1, y1, x2, y2, d),
                20_000,
                4096,
            );
            json!({ "ok": r.code == Some(0), "output": r.combined() })
        }
        "key" => {
            if key.is_empty() {
                return err("missing_key", "hace falta key");
            }
            if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return err("bad_key", "key no permitida (sólo A-Z0-9_)");
            }
            let r = root_exec(
                &format!("input keyevent {}", sh_quote(key)),
                15_000,
                4096,
            );
            json!({ "ok": r.code == Some(0), "output": r.combined() })
        }
        "text" => {
            if text.is_empty() {
                return err("missing_text", "hace falta text");
            }
            // `input text` no puede llevar comillas: se codifica con %s.
            let encoded = text.replace(' ', "%s");
            let r = root_exec(
                &format!("input text {}", sh_quote(&encoded)),
                20_000,
                4096,
            );
            json!({ "ok": r.code == Some(0), "output": r.combined() })
        }
        other => err("bad_action", &format!("acción no soportada: {}", other)),
    }
}

// ========================================================================= //
// fs_read / fs_write

/// ¿Esta ruta cae en una zona del sistema cuya escritura exige confirmación
/// explícita (`confirm_destructive=true`)?
///
/// Cubre `/dev/block` (particiones), `/system` y `/system/...`.  La barra
/// final se comprueba aparte porque `starts_with("/system/")` no casa con
/// `/system` exactamente (H-03).
fn write_requires_confirm(path: &str) -> bool {
    path == "/dev/block"
        || path.starts_with("/dev/block/")
        || path == "/system"
        || path.starts_with("/system/")
}
// ========================================================================= //
pub fn fs_read(path: &str, base64: bool, max_bytes: usize) -> Value {
    if path.is_empty() || !path.starts_with('/') {
        return err("bad_path", "path debe ser absoluta");
    }
    if path.contains("..") {
        return err("bad_path", "path no puede contener '..'");
    }
    let max = max_bytes.clamp(1024, 8 * 1024 * 1024);
    let cmd = if base64 {
        format!("head -c {} {} | base64 -w 0", max, sh_quote(path))
    } else {
        format!("head -c {} {}", max, sh_quote(path))
    };
    let r = root_exec(&cmd, 60_000, max + 4096);
    json!({ "ok": r.code == Some(0), "path": path, "base64": base64,
            "content": r.stdout, "stderr": r.stderr })
}

pub fn fs_write(path: &str, content: &str, base64: bool, chmod: &str, confirm: bool) -> Value {
    if path.is_empty() || !path.starts_with('/') {
        return err("bad_path", "path debe ser absoluta");
    }
    if path.contains("..") {
        return err("bad_path", "path no puede contener '..'");
    }
    // Escritura en /dev/block o en el sistema: exigir confirmación.
    //
    // BUG v13.4 (H-01/H-02): la condición era
    //     path.starts_with("/dev/block") || path.starts_with("/system/") && chmod.is_empty()
    // y en Rust `&&` liga más fuerte que `||`, o sea que se leía como
    // `A || (B && C)`: escribir en `/system/...` CON `chmod` saltaba la
    // confirmación.  Además el error pedía un `confirm_destructive` que la
    // función no tenía ni el esquema declaraba (contrato muerto).
    if write_requires_confirm(path) && !confirm {
        audit("fs_write.blocked", &json!({"path": path}));
        return err(
            "needs_confirm",
            "escribir en /dev/block o /system exige confirm_destructive=true",
        );
    }
    let tmp = format!("{}/.md_write_{}", crate::cockpit_server::state_dir(), std::process::id());
    let _ = std::fs::create_dir_all(crate::cockpit_server::state_dir());
    // El contenido se escribe vía heredoc para no meterlo en el argv
    // (ni que aparezca en `ps`).  En modo base64 se decodifica primero,
    // de modo que el binario llega intacto.
    let ch = if chmod.is_empty() {
        String::new()
    } else {
        format!(" && chmod {} {}", sh_quote(chmod), sh_quote(path))
    };
    // BUG v15 (ADV-A-01, P0 — RCE root): el contenido CRUDO del usuario
    // viajaba dentro de un heredoc `<<'__MD_EOF__'`.  Una línea `__MD_EOF__`
    // en el contenido cerraba el heredoc ANTES de tiempo y todo lo que
    // venía detrás se ejecutaba como shell ROOT (`su -mm -c`).
    //
    // Arreglo: el contenido SIEMPRE viaja en base64 ⇒ el shell nunca ve el
    // texto del usuario y no hay delimitador que cerrar.  Sin crates nuevas:
    // se reutiliza `base64ct` (la misma que usa el cliente ADB).
    //
    // El modo `base64: true` significa "el contenido YA viene en base64"
    // (lo decodifica el dispositivo); en ese caso no se codifica dos veces.
    let payload = if base64 {
        content.to_string()
    } else {
        use base64ct::Encoding;
        base64ct::Base64::encode_string(content.as_bytes())
    };
    let cmd = format!(
        "mkdir -p $(dirname {p}) && printf %s {c} | base64 -d > {t} && cp {t} {p} && rm -f {t}{ch}",
        p = sh_quote(path),
        t = sh_quote(&tmp),
        c = sh_quote(&payload),
        ch = ch
    );
    let r = root_exec(&cmd, 60_000, 8192);
    audit("fs.write", &json!({"path": path, "bytes": content.len()}));
    json!({ "ok": r.code == Some(0), "path": path, "output": r.combined() })
}

// ========================================================================= //
// find_duplicates — SHA-256 nativo en streaming (sin jdupes)
// ========================================================================= //
pub fn find_duplicates(path: &str, min_size_kb: u64) -> Value {
    if path.is_empty() || !path.starts_with('/') {
        return err("bad_path", "path debe ser absoluta");
    }
    if path.contains("..") {
        return err("bad_path", "path no puede contener '..'");
    }
    let min = min_size_kb.clamp(1, 1024 * 1024);
    // BUG v15 (ADV-A-02, P0 — RCE root): el nombre del fichero se
    // interpolaba DENTRO de un comando de shell construido desde AWK
    // (`cmd="stat -c %s \"" p "\""`) ⇒ un fichero llamado
    // `x";touch /tmp/PWNED;echo "` ejecutaba el comando como ROOT (el
    // `find_duplicates` es alcanzable desde el MCP).
    //
    // Arreglo: el nombre se mete en el shell CON comillas simples y sus
    // comillas simples internas se escapan (`'` -> `'\''`), que es el
    // escapado POSIX estándar: un `;` o un `$` dentro del nombre ya no
    // significa nada para el shell.
    let script = format!(
        r#"
MIN={min}
find {} -type f -size +${{MIN}}k -print0 2>/dev/null | xargs -0 sha256sum 2>/dev/null | sort | awk '
{{ h=$1; $1=""; sub(/^ /,""); p=$0;
  if (h==prev) {{ if (!shown) {{ print "DUP " prevsize " " prevpath; shown=1 }} print "    " p; }}
  else {{ prev=h; prevpath=p; prevsize=0; q=p; gsub(/'"'"'/, "'"'"'\\'"'"''"'"'", q); cmd="stat -c %s '"'"'" q "'"'"' 2>/dev/null"; cmd | getline sz; close(cmd); prevsize=sz; shown=0 }}
}}' 2>/dev/null | head -400
"#,
        sh_quote(path),
        min = min
    );
    let r = root_exec(&script, 300_000, 256 * 1024);
    json!({ "ok": true, "path": path, "min_size_kb": min, "report": r.stdout })
}

// ========================================================================= //
// root_exec con política destructiva
// ========================================================================= //
pub fn root_exec_tool(command: &str, timeout_ms: u64, max_chars: usize, confirm: bool) -> Value {
    if command.trim().is_empty() {
        return err("empty_command", "comando vacío");
    }
    if is_destructive(command) && !confirm {
        audit("root_exec.blocked", &json!({"cmd": command}));
        return err(
            "needs_confirm",
            "comando potencialmente destructivo: repítelo con confirm_destructive=true",
        );
    }
    let r = root_exec(command, timeout_ms, max_chars);
    audit("root_exec", &json!({"cmd": command, "rc": r.code, "ms": r.duration_ms}));
    json!({ "ok": r.code == Some(0), "command": command, "result": r.to_json() })
}

// ========================================================================= //
// Definiciones MCP de las herramientas de dispositivo
// ========================================================================= //
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "system_overview",
            "description": "Resumen del dispositivo: almacenamiento, RAM, número de apps, versión de Android, SELinux, Magisk, NPU y carpetas más pesadas.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "root_exec",
            "description": "Ejecuta una orden como root real (su -mm -c). Timeout y tope de salida aplicados. Los comandos potencialmente destructivos exigen confirm_destructive=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "timeout_ms": { "type": "integer", "default": 30000, "maximum": 300000 },
                    "max_chars": { "type": "integer", "default": 65536, "maximum": 524288 },
                    "confirm_destructive": { "type": "boolean", "default": false }
                },
                "required": ["command"]
            }
        },
        {
            "name": "fs_read",
            "description": "Lee un fichero del dispositivo (texto o base64) con tope de bytes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "base64": { "type": "boolean", "default": false },
                    "max_bytes": { "type": "integer", "default": 1048576, "maximum": 8388608 }
                },
                "required": ["path"]
            }
        },
        {
            "name": "fs_write",
            "description": "Escribe un fichero en el dispositivo (texto o base64), con chmod opcional. Escribir en /dev/block o /system exige confirm_destructive=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "base64": { "type": "boolean", "default": false },
                    "chmod": { "type": "string", "description": "p.ej. 600, 755" },
                    "confirm_destructive": { "type": "boolean", "default": false, "description": "Obligatorio para escribir en /dev/block o /system." }
                },
                "required": ["path", "content"]
            }
        },
        {
            "name": "find_duplicates",
            "description": "Detecta duplicados EXACTOS por SHA-256 (streaming, nativo, sin jdupes).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "min_size_kb": { "type": "integer", "default": 100 }
                },
                "required": ["path"]
            }
        },
        {
            "name": "props_and_settings",
            "description": "Lee/escribe propiedades del sistema (getprop/resetprop, incluidas ro.*) y ajustes (global/secure/system).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["get", "set_prop", "settings_get", "settings_put"] },
                    "namespace": { "type": "string", "enum": ["global", "secure", "system"] },
                    "key": { "type": "string" },
                    "value": { "type": "string" },
                    "delete": { "type": "boolean", "default": false }
                },
                "required": ["action"]
            }
        },
        {
            "name": "selinux_control",
            "description": "Estado de SELinux, conmutar Enforcing/Permissive e inyectar reglas vivas con magiskpolicy --live.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["status", "set_permissive", "set_enforcing", "live_policy"] },
                    "rule": { "type": "string" }
                },
                "required": ["action"]
            }
        },
        {
            "name": "magisk_module",
            "description": "Gestiona módulos de Magisk: listar, habilitar, deshabilitar, instalar desde un ZIP local, crear/actualizar y apartar (nunca borra: se mueve para poder revertir).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "enable", "disable", "remove", "create_or_update", "install", "validate_zip", "shamiko"] },
                    "module_id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "zip_path": { "type": "string" },
                    "enable": { "type": "boolean", "default": false },
                    "dry_run": { "type": "boolean", "default": true }
                },
                "required": ["action"]
            }
        },
        {
            "name": "root_occlusion_audit",
            "description": "Auditoria de ocultacion de root: por app, que senales mira (estatico: tabla de cadenas del DEX + .so nativas) y si ahora mismo quedan a la vista desde el namespace de la app (runtime) y en su logcat (ejecucion). Devuelve la tabla app -> senal -> veredicto.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "packages": { "type": "array", "items": { "type": "string" } },
                    "package": { "type": "string" },
                    "mode": { "type": "string", "enum": ["audit", "state", "exec", "static", "csc"], "default": "audit" }
                }
            }
        },
        {
            "name": "kernel_and_blocks",
            "description": "Información del kernel, dmesg, procesos, particiones y volcado de una partición a fichero.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["info", "dmesg", "processes", "partitions", "dump_partition"] },
                    "lines": { "type": "integer", "default": 200 },
                    "partition": { "type": "string" },
                    "dst": { "type": "string" }
                },
                "required": ["action"]
            }
        },
        // v13.4 — herramienta de RESPALDO SEGURO del kernel (el punto de
        // retorno antes de tocar la partición de arranque): volcado + SHA-256
        // + subida a Drive + plan de rollback. Nunca escribe en /dev/block.
        crate::kernel_and_blocks::tool_definition(),
        // v16 — INSTRUMENTACIÓN REAL por ptrace: instala el payload de hooks
        // (SQLite/red/ficheros) en el móvil, lo inyecta en el proceso de la
        // app y lee el NDJSON que genera.  El inyector corre como root
        // (`su -mm -c`) porque `ptrace` a una app de terceros lo exige.
        crate::hooks_bridge::tool_definition(),
        {
            "name": "app_control",
            "description": "Gestión de apps: listar, info, habilitar/deshabilitar, desinstalar (con -k opcional), limpiar datos/caché, permisos, appops, instalar APK y abrir.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "info", "disable", "enable", "uninstall", "clear_data", "clear_cache", "grant_perm", "revoke_perm", "appops", "install_apk", "open"] },
                    "scope": { "type": "string", "enum": ["all", "third_party", "system", "disabled"] },
                    "package": { "type": "string" },
                    "permission": { "type": "string" },
                    "op": { "type": "string" },
                    "mode": { "type": "string" },
                    "apk_path": { "type": "string" },
                    "keep_data": { "type": "boolean", "default": false }
                },
                "required": ["action"]
            }
        },
        {
            "name": "screen_and_ui",
            "description": "Captura de pantalla, volcado de la jerarquía de UI, toques, deslizamientos, teclas y escritura de texto.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["screenshot", "dump", "tap", "swipe", "key", "text"] },
                    "x": { "type": "integer" }, "y": { "type": "integer" },
                    "x1": { "type": "integer" }, "y1": { "type": "integer" },
                    "x2": { "type": "integer" }, "y2": { "type": "integer" },
                    "duration_ms": { "type": "integer", "default": 300 },
                    "key": { "type": "string" },
                    "text": { "type": "string" },
                    "save_path": { "type": "string" }
                },
                "required": ["action"]
            }
        }
    ])
}

/// Despacha una herramienta de dispositivo.
pub fn dispatch(name: &str, args: &Value) -> Value {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let i = |k: &str| args.get(k).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let u = |k: &str| args.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
    let b = |k: &str| args.get(k).and_then(|v| v.as_bool()).unwrap_or(false);

    match name {
        "system_overview" => system_overview(),
        "root_exec" => root_exec_tool(
            &s("command"),
            u("timeout_ms"),
            u("max_chars") as usize,
            b("confirm_destructive"),
        ),
        "fs_read" => fs_read(&s("path"), b("base64"), u("max_bytes") as usize),
        "fs_write" => fs_write(&s("path"), &s("content"), b("base64"), &s("chmod"), b("confirm_destructive")),
        "find_duplicates" => find_duplicates(&s("path"), u("min_size_kb")),
        "props_and_settings" => props_and_settings(
            &s("action"),
            &s("namespace"),
            &s("key"),
            &s("value"),
            b("delete"),
        ),
        "selinux_control" => selinux_control(&s("action"), &s("rule")),
        "magisk_module" => magisk_module(
            &s("action"),
            &s("module_id"),
            &s("name"),
            &s("description"),
            &s("zip_path"),
            b("enable"),
            args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(true),
        ),
        "root_occlusion_audit" => root_occlusion_audit(
            args.get("packages").unwrap_or(&Value::Null),
            &s("package"),
            &s("mode"),
        ),
        "kernel_and_blocks" => kernel_and_blocks(
            &s("action"),
            u("lines") as u32,
            &s("partition"),
            &s("dst"),
        ),
        // v13.4 — respaldo seguro del kernel (volcado + SHA-256 + Drive +
        // plan de rollback). Módulo propio: `kernel_and_blocks.rs`.
        "kernel_backup" => crate::kernel_and_blocks::tool_dispatch(&args),
        // v16 — instrumentación real (ptrace) del proceso de una app.
        // Módulo propio: `hooks_bridge.rs`.
        "inject_hooks" => crate::hooks_bridge::dispatch(&args),
        "app_control" => app_control(
            &s("action"),
            &s("scope"),
            &s("package"),
            &s("permission"),
            &s("op"),
            &s("mode"),
            &s("apk_path"),
            b("keep_data"),
        ),
        "screen_and_ui" => screen_and_ui(
            &s("action"),
            i("x"),
            i("y"),
            i("x1"),
            i("y1"),
            i("x2"),
            i("y2"),
            i("duration_ms"),
            &s("key"),
            &s("text"),
            &s("save_path"),
        ),
        other => err("unknown_tool", &format!("herramienta desconocida: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sh_quote_escapa_la_inyeccion() {
        assert_eq!(sh_quote("hola"), "'hola'");
        // Un payload con comilla simple no puede cerrar la cadena.
        assert_eq!(sh_quote("a'; rm -rf /; '"), "'a'\\''; rm -rf /; '\\'''");
        assert!(sh_quote("$(id)").contains("'$(id)'"));
    }

    #[test]
    fn detecta_comandos_destructivos() {
        assert!(is_destructive("rm -rf /data"));
        assert!(is_destructive("dd if=x of=/dev/block/sda"));
        assert!(is_destructive("mkfs.ext4 /dev/block/sdb"));
        assert!(!is_destructive("ls -la /data"));
        assert!(!is_destructive("cat /proc/cpuinfo"));
    }

    #[test]
    fn las_herramientas_estan_todas_declaradas() {
        let defs = tool_definitions();
        let arr = defs.as_array().unwrap();
        let nombres: Vec<&str> = arr.iter().filter_map(|d| d["name"].as_str()).collect();
        for esperado in [
            "system_overview", "root_exec", "fs_read", "fs_write", "find_duplicates",
            "props_and_settings", "selinux_control", "magisk_module", "kernel_and_blocks",
            "kernel_backup", "inject_hooks", "app_control", "screen_and_ui",
        ] {
            assert!(nombres.contains(&esperado), "falta la herramienta {}", esperado);
        }
    }

    #[test]
    fn las_esquemas_tienen_enums_cuando_deben() {
        let defs = tool_definitions();
        for d in defs.as_array().unwrap() {
            let name = d["name"].as_str().unwrap();
            if let Some(props) = d["inputSchema"]["properties"].as_object() {
                for (k, v) in props {
                    if k == "action" || k == "scope" {
                        assert!(
                            v.get("enum").is_some(),
                            "{}: el parámetro {} debería ser un enum (no texto libre)",
                            name, k
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn herramienta_desconocida_da_error_estructurado() {
        let r = dispatch("no_existe", &json!({}));
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("unknown_tool"));
    }

    #[test]
    fn ruta_invalida_se_rechaza_sin_tocar_el_sistema() {
        let r = fs_read("relative/path", false, 1024);
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("bad_path"));
        let r2 = fs_read("/data/../etc/shadow", false, 1024);
        assert_eq!(r2["ok"], json!(false));
    }

    #[test]
    fn comando_destructivo_exige_confirmacion() {
        let r = root_exec_tool("rm -rf /", 1000, 1024, false);
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("needs_confirm"));
    }

    #[test]
    fn escribir_en_el_sistema_exige_confirmacion() {
        // La política pura: qué rutas la activan y cuáles no.
        assert!(write_requires_confirm("/system"));
        assert!(write_requires_confirm("/system/app/x.apk"));
        assert!(write_requires_confirm("/dev/block"));
        assert!(write_requires_confirm("/dev/block/sda"));
        assert!(!write_requires_confirm("/data/local/tmp/x"));
        assert!(!write_requires_confirm("/tmp/x"));
        // El contrato completo: sin confirmar, la escritura se rechaza
        // ANTES de tocar el sistema (con `chmod` y sin él).
        for p in ["/system/app/x.apk", "/system", "/dev/block/sda"] {
            let r = fs_write(p, "x", false, "644", false);
            assert_eq!(r["error"], json!("needs_confirm"), "{}: {}", p, r);
            let r2 = fs_write(p, "x", false, "", false);
            assert_eq!(r2["error"], json!("needs_confirm"), "{}: {}", p, r2);
        }
    }
}

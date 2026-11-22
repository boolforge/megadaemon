//! `kernel_and_blocks` — estado del kernel del móvil y **respaldo seguro** de
//! particiones.
//!
//! # Por qué existe
//!
//! El encargo es "preparar un cambio de kernel sin dejar el móvil en
//! bootloop". El punto de retorno de un cambio de kernel **no** es un
//! `adb backup` ni un `tar`: es la **imagen de la partición de arranque**
//! (`boot`, `vendor_boot` y —en Samsung— `recovery`) con su SHA-256. Si el
//! kernel nuevo no arranca, se restaura esa imagen y el móvil vuelve como
//! estaba.
//!
//! # Qué hace bien (y qué NO)
//!
//! * Todo lo de este módulo es **de sólo lectura** salvo el `dd` que escribe
//!   el **volcado** en un fichero. **Nunca** escribe en `/dev/block/*`:
//!   la restauración se **genera** como texto (`rollback_plan`) y la ejecuta
//!   el usuario, a mano, con el móvil en recovery.
//! * Cada volcado lleva **SHA-256 del origen** y **SHA-256 del fichero** y
//!   los compara: si no cuadran, el volcado se marca como **INVÁLIDO** y no
//!   sirve como punto de retorno.
//! * `fingerprint` **no** vuelca nada: sólo calcula el hash de la partición
//!   (medido en el S21 Ultra: **262 ms** para 64 MiB). Es la forma de saber
//!   "el kernel ha cambiado" sin escribir un solo byte.
//! * La subida a Drive **nunca** deja el móvil: el token OAuth lo minte el
//!   propio móvil con su `gdrive_credentials.json` (0600) y la subida se hace
//!   por la red del móvil, no por este repo.
//!
//! # Autocontenido a propósito
//!
//! Este módulo **no** usa `crate::` (sólo `serde_json`, `sha2`, `libc` y
//! `ureq`). Así se puede compilar **aislado** en un arnés de test con
//! `#[path = "../src/kernel_and_blocks.rs"]` y sus tests corren aunque el
//! resto del crate no compile.
//!
//! ## Medición real (S21 Ultra Exynos 2100, 2026-11-22)
//!
//! | partición | tamaño | SHA-256 del bloque | tiempo de `dd` |
//! |---|---|---|---|
//! | `boot` | 67.108.864 B | `f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd` | 955 ms |
//! | `vendor_boot` | 67.108.864 B | `b55ecfd0744ae8ae5404635b3345b846764e55eca8349a655e71aebd52d0b2fb` | 433 ms |
//! | `recovery` | 67.108.864 B | `777350644d9f5c345970fef5f92ea5efdb1fbc9d2b0005b4a8b62dcb90703f19` | 829 ms |
//! | `vbmeta` | 65.536 B | `b1ef8b5108b1a333c13e6db5cf1e2c1446a4e017e81d0fe9b9392de70a1238c9` | 111 ms |
//! | `vbmeta_system` | 65.536 B | `254732e930687a6d5a3c0c82965f84c9a4a36767f6aa39eeacabf53af6aba5c6` | 110 ms |
//!
//! ⇒ un respaldo completo son **201.457.664 B (192,06 MiB)** y **2.438 ms** de
//! `dd`: el punto de retorno es barato. No hay excusa para tocar el kernel sin él.
//!
//! ### OJO: `init_boot` NO existe en este móvil
//!
//! Samsung **no** usa la partición `init_boot` de AOSP: su equivalente
//! funcional es `recovery` (el recovery de One UI va ahí). El listado real de
//! `/dev/block/by-name` en el S21 Ultra medido es: `blcmd boot bota cache
//! cp_debug cpefs dqmdbg dtbo efs fmp_selftest harx keyrefuge keystorage
//! metadata misc nad_refer omr optics param persistent prism radio recovery
//! reserved sda sdb sdc sdd sde sec_efs spu steady super uh uhcfg up_param
//! userdata vbmeta vbmeta_system vendor_boot` ⇒ **ni `init_boot` ni
//! `vendor_dlkm`**. El backup pide `init_boot` igualmente (por si otro
//! dispositivo sí lo tiene) pero **marca las particiones ausentes como
//! ausentes**, sin fallar.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Tope de una orden root.
pub const MAX_TIMEOUT_MS: u64 = 300_000;

/// Particiones que se respaldan por defecto: las de arranque.
///
/// `init_boot` se pide también: en AOSP es la partición del ramdisk, pero en
/// Samsung no existe (su equivalente es `recovery`). Si no está, el volcado
/// se marca como **ausente** y se sigue con el resto.
pub const DEFAULT_BOOT_PARTITIONS: &[&str] = &["boot", "vendor_boot", "recovery", "init_boot"];

/// Esquema del manifiesto de respaldo.
pub const MANIFEST_SCHEMA: &str = "megadaemon.kernel.backup/v1";

// ========================================================================= //
// Drive (subida del punto de retorno)
// ========================================================================= //

/// API de Drive (ficheros).
pub const DRIVE_API: &str = "https://www.googleapis.com/drive/v3/files";

/// API de subida de Drive (resumable).
pub const DRIVE_UPLOAD_API: &str = "https://www.googleapis.com/upload/drive/v3/files";

/// Endpoint de OAuth2 de Google.
pub const OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Tamaño del trozo de subida (8 MiB = múltiplo de 256 KiB, que es lo que
/// exige Google en las subidas resumables).
pub const DRIVE_CHUNK: u64 = 8 * 1024 * 1024;

/// Carpeta de Drive por defecto del proyecto (privada).
pub const DEFAULT_DRIVE_FOLDER: &str = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0";

// ========================================================================= //
// Ejecución root
// ========================================================================= //

/// Resultado de una orden.
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
}

impl ExecResult {
    /// Texto combinado (stdout + stderr).
    pub fn combined(&self) -> String {
        if self.stderr.trim().is_empty() {
            self.stdout.clone()
        } else if self.stdout.trim().is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n[stderr]\n{}", self.stdout, self.stderr)
        }
    }

    /// `true` si la orden terminó con código 0 y sin timeout.
    pub fn ok(&self) -> bool {
        self.code == Some(0) && !self.timed_out
    }
}

/// Cita una cadena para una shell POSIX.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// ¿Estamos ya como root? (así no hace falta pasar por `su`).
fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Ejecuta `command` como root real.
///
/// * Si el proceso ya es root (euid 0) la orden se ejecuta **directamente**:
///   es lo que pasa con el daemon y con `md-kernel` lanzado desde una shell
///   de root, y evita un `su` extra.
/// * Si no, usa `su -mm -c` (Magisk/Kitsune Mask), que es el mecanismo que ya
///   usa el resto del daemon (`android::root_exec`).
///
/// La orden **nunca** se interpreta aquí: va como un único argumento a `sh -c`
/// (o a `su -c`), sin construir una cadena con datos del usuario.
pub fn root_exec(command: &str, timeout_ms: u64) -> ExecResult {
    let timeout_ms = timeout_ms.clamp(1_000, MAX_TIMEOUT_MS);
    let started = Instant::now();

    let child = if is_root() {
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else {
        Command::new("su")
            .arg("-mm")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    };

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            return ExecResult {
                stdout: String::new(),
                stderr: format!("no se pudo lanzar la orden: {}", e),
                code: None,
                timed_out: false,
                duration_ms: started.elapsed().as_millis() as u64,
            }
        }
    };

    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = out_pipe {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = err_pipe {
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
                std::thread::sleep(Duration::from_millis(30));
            }
            Err(_) => break None,
        }
    };

    let stdout = String::from_utf8_lossy(&out_handle.join().unwrap_or_default()).to_string();
    let stderr = String::from_utf8_lossy(&err_handle.join().unwrap_or_default()).to_string();

    ExecResult {
        stdout,
        stderr,
        code: status.and_then(|s| s.code()),
        timed_out,
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

// ========================================================================= //
// Validación (fail-closed)
// ========================================================================= //

/// Valida el nombre de una partición: sólo `[A-Za-z0-9_.-]`.
pub fn validate_partition(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("la partición va vacía".to_string());
    }
    if name.len() > 64 {
        return Err("el nombre de la partición es demasiado largo".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(format!(
            "nombre de partición no permitido: '{}' (sólo letras, dígitos, '_', '-' y '.')",
            name
        ));
    }
    if name.starts_with('.') {
        return Err("el nombre de la partición no puede empezar por '.'".to_string());
    }
    Ok(())
}

/// Valida la ruta de destino de un volcado.
///
/// * debe ser **absoluta**;
/// * no puede contener `..`;
/// * **no** puede estar dentro de `/dev/` (ahí viven los dispositivos de
///   bloque: escribir en ellos es justo lo que este módulo evita hacer);
/// * debe estar en un sitio persistente (`/data`), porque el volcado tiene
///   que sobrevivir a un reinicio (y a un `magisk --remove-modules`).
pub fn validate_dst(dst: &str) -> Result<(), String> {
    if dst.is_empty() {
        return Err("la ruta de destino va vacía".to_string());
    }
    if !dst.starts_with('/') {
        return Err("la ruta de destino debe ser absoluta".to_string());
    }
    if dst.contains("..") {
        return Err("la ruta de destino no puede contener '..'".to_string());
    }
    if dst.starts_with("/dev/") {
        return Err(
            "no se permite escribir dentro de /dev (dispositivos de bloque): el volcado va a /data"
                .to_string(),
        );
    }
    if dst.contains('\0') {
        return Err("la ruta de destino tiene un NUL".to_string());
    }
    Ok(())
}

/// Ruta del dispositivo de bloque de una partición.
pub fn partition_device(name: &str) -> String {
    format!("/dev/block/by-name/{}", name)
}

/// Ruta por defecto del volcado de una partición.
pub fn default_dump_path(dir: &str, partition: &str) -> String {
    format!("{}/{}.img", dir.trim_end_matches('/'), partition)
}

// ========================================================================= //
// Particiones
// ========================================================================= //

/// ¿Existe la partición en `/dev/block/by-name`?
pub fn partition_exists(name: &str) -> bool {
    let dev = partition_device(name);
    let r = root_exec(
        &format!("test -b {} && echo yes || echo no", sh_quote(&dev)),
        15_000,
    );
    r.stdout.trim().eq_ignore_ascii_case("yes")
}

/// Tamaño en bytes de una partición (`blockdev --getsize64`).
pub fn partition_size(name: &str) -> Option<u64> {
    let dev = partition_device(name);
    let r = root_exec(
        &format!("blockdev --getsize64 {} 2>/dev/null", sh_quote(&dev)),
        20_000,
    );
    r.stdout.trim().parse::<u64>().ok()
}

/// SHA-256 **del contenido de la partición** (lee el bloque; no escribe nada).
pub fn partition_sha256(name: &str) -> Option<String> {
    let dev = partition_device(name);
    let r = root_exec(
        &format!("sha256sum {} 2>/dev/null", sh_quote(&dev)),
        120_000,
    );
    parse_sha256_line(&r.stdout)
}

/// Extrae el hash de la salida de `sha256sum` (`<hash>  <ruta>`).
pub fn parse_sha256_line(out: &str) -> Option<String> {
    for line in out.lines() {
        let line = line.trim();
        if line.len() >= 64 {
            let head = &line[..64];
            if head.chars().all(|c| c.is_ascii_hexdigit()) && !head.chars().all(|c| c.is_ascii_digit()) {
                return Some(head.to_ascii_lowercase());
            }
        }
    }
    None
}

/// Espacio libre (en bytes) en el sistema de ficheros que contiene `dir`.
pub fn free_space(dir: &str) -> Option<u64> {
    let r = root_exec(&format!("df -P {} 2>/dev/null", sh_quote(dir)), 20_000);
    let last = r.stdout.lines().filter(|l| !l.trim().is_empty()).next_back()?;
    let fields: Vec<&str> = last.split_whitespace().collect();
    if fields.len() < 5 {
        return None;
    }
    // `df -P`: Filesystem, bloques, Usados, Disponibles, % , Montado
    let avail_kb: u64 = fields[3].parse().ok()?;
    Some(avail_kb * 1024)
}

// ========================================================================= //
// Acciones de sólo lectura
// ========================================================================= //

/// Estado del kernel y del dispositivo (uname, /proc/version, props).
pub fn info() -> Value {
    let script = "uname -a; echo '---'; cat /proc/version; echo '---'; \
                  getprop ro.product.model; getprop ro.soc.model; \
                  getprop ro.build.version.release; getprop ro.build.version.sdk; \
                  getprop ro.build.version.security_patch; getprop ro.boot.hardware; \
                  getprop ro.boot.verifiedbootstate; getprop ro.boot.flash.locked; \
                  getprop ro.build.display.id; echo '---'; \
                  (test -r /proc/config.gz && echo 'config.gz: presente' || echo 'config.gz: ausente'); \
                  echo '--- modulos ---'; (cat /proc/modules 2>/dev/null | wc -l); \
                  echo '--- dmesg ---'; (dmesg 2>/dev/null | wc -l)";
    let r = root_exec(script, 25_000);
    json!({ "ok": true, "info": r.combined() })
}

/// Últimas líneas de `dmesg`.
pub fn dmesg(lines: u32) -> Value {
    let lines = lines.clamp(10, 5000);
    let r = root_exec(&format!("dmesg 2>/dev/null | tail -n {}", lines), 30_000);
    json!({ "ok": true, "dmesg": r.combined() })
}

/// Procesos del sistema.
pub fn processes() -> Value {
    let r = root_exec(
        "ps -A -o PID,USER,RSS,NAME,ARGS 2>/dev/null | head -60",
        25_000,
    );
    json!({ "ok": true, "processes": r.stdout })
}

/// Listado de particiones (`by-name` + montajes relevantes).
pub fn partitions() -> Value {
    let r = root_exec(
        "ls -la /dev/block/by-name/ 2>/dev/null | head -80; echo '--- mounts ---'; \
         cat /proc/mounts 2>/dev/null | grep -E ' / | /data | /system | /vendor | /odm ' | head -10",
        20_000,
    );
    json!({ "ok": true, "partitions": r.combined() })
}

/// Huella de una partición: tamaño + SHA-256, **sin volcar nada**.
pub fn fingerprint(partition: &str) -> Value {
    if let Err(e) = validate_partition(partition) {
        return json!({ "ok": false, "error": e });
    }
    let dev = partition_device(partition);
    let started = Instant::now();
    let size = partition_size(partition);
    let sha = partition_sha256(partition);
    let ms = started.elapsed().as_millis() as u64;
    match sha {
        None => json!({
            "ok": false,
            "partition": partition,
            "device": dev,
            "error": "no se pudo leer la partición (¿existe en /dev/block/by-name?)",
        }),
        Some(sha256) => json!({
            "ok": true,
            "partition": partition,
            "device": dev,
            "size_bytes": size,
            "sha256": sha256,
            "duration_ms": ms,
        }),
    }
}

// ========================================================================= //
// Volcado (el punto de retorno)
// ========================================================================= //

/// Vuelca una partición a un fichero y verifica su integridad.
///
/// * No escribe **nunca** en un dispositivo de bloque.
/// * Comprueba el SHA-256 del origen **y** del fichero volcado; si no
///   coinciden, el volcado queda marcado como inválido (`ok: false`).
/// * Si el destino ya existe, no se toca salvo que `overwrite` sea `true`
///   (fail-closed: no se pisa un respaldo bueno por accidente).
pub fn dump_partition(partition: &str, dst: &str, overwrite: bool) -> Value {
    if let Err(e) = validate_partition(partition) {
        return json!({ "ok": false, "error": e });
    }
    if let Err(e) = validate_dst(dst) {
        return json!({ "ok": false, "error": e });
    }
    let dev = partition_device(partition);

    if !partition_exists(partition) {
        return json!({
            "ok": false,
            "partition": partition,
            "device": dev,
            "ausente": true,
            "error": "la partición no existe en /dev/block/by-name (normal en Samsung: no hay init_boot)",
        });
    }

    // ¿Existe ya el destino?
    let exists = root_exec(
        &format!("test -e {} && echo yes || echo no", sh_quote(dst)),
        15_000,
    )
    .stdout
    .trim()
    .eq_ignore_ascii_case("yes");
    if exists && !overwrite {
        return json!({
            "ok": false,
            "error": "el destino ya existe (usa overwrite para rehacerlo)",
            "partition": partition,
            "dst": dst,
        });
    }

    let size = partition_size(partition).unwrap_or(0);

    // Espacio: hace falta el tamaño de la partición (el volcado es crudo).
    let parent = Path::new(dst)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/data".to_string());
    if let Some(free) = free_space(&parent) {
        let need = size + 16 * 1024 * 1024; // + holgura
        if free < need {
            return json!({
                "ok": false,
                "error": format!(
                    "no cabe: hacen falta {} bytes y hay {} libres en {}",
                    need, free, parent
                ),
                "partition": partition,
                "size_bytes": size,
                "free_bytes": free,
            });
        }
    }

    let started = Instant::now();
    let src_sha = partition_sha256(partition);
    let cmd = format!(
        "mkdir -p {} && dd if={} of={} bs=4M conv=fsync 2>&1 | tail -3",
        sh_quote(&parent),
        sh_quote(&dev),
        sh_quote(dst)
    );
    let r = root_exec(&cmd, 300_000);
    let ms = started.elapsed().as_millis() as u64;
    if !r.ok() {
        return json!({
            "ok": false,
            "error": "el volcado falló",
            "partition": partition,
            "dst": dst,
            "output": r.combined(),
        });
    }

    let file_sha = root_exec(
        &format!("sha256sum {} 2>/dev/null", sh_quote(dst)),
        120_000,
    );
    let file_sha = parse_sha256_line(&file_sha.stdout);
    let file_bytes = root_exec(
        &format!("stat -c %s {} 2>/dev/null", sh_quote(dst)),
        20_000,
    )
    .stdout
    .trim()
    .parse::<u64>()
    .ok();

    let match_sha = match (&src_sha, &file_sha) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    };

    json!({
        "ok": match_sha,
        "partition": partition,
        "device": dev,
        "dst": dst,
        "size_bytes": size,
        "file_bytes": file_bytes,
        "sha256": file_sha,
        "source_sha256": src_sha,
        "integrity_ok": match_sha,
        "duration_ms": ms,
        "output": r.combined(),
        "note": if match_sha {
            "volcado verificado: sirve como punto de retorno"
        } else {
            "volcado NO verificado: NO sirve como punto de retorno"
        },
    })
}

// ========================================================================= //
// Manifiesto
// ========================================================================= //

/// Time-stamp ISO-8601 en UTC (`2026-11-22T14:30:00Z`).
///
/// Conversión **en días del calendario gregoriano proleptico**, sin
/// dependencias externas (el crate no tiene `chrono`).
pub fn iso_utc_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Días desde 1970-01-01 → fecha civil (algoritmo de Howard Hinnant).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, h, mi, s
    )
}

/// Ahora mismo en ISO-8601 UTC.
pub fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    iso_utc_from_unix(secs)
}

/// Identificación del dispositivo (para el manifiesto).
pub fn device_identity() -> Value {
    let r = root_exec(
        "getprop ro.product.model; getprop ro.soc.model; getprop ro.build.version.release; \
         getprop ro.build.version.sdk; getprop ro.build.display.id; uname -r",
        20_000,
    );
    let lines: Vec<&str> = r.stdout.lines().map(|l| l.trim()).collect();
    let get = |i: usize| lines.get(i).copied().unwrap_or("").to_string();
    json!({
        "model": get(0),
        "soc": get(1),
        "android": get(2),
        "sdk": get(3),
        "build": get(4),
        "kernel": get(5),
    })
}

/// Volca un conjunto de particiones y escribe el manifiesto.
///
/// Las particiones que **no existen** en el dispositivo (p. ej. `init_boot`
/// en Samsung) se registran con `ausente: true` y **no** invalidan el
/// respaldo: el respaldo es válido si todas las particiones **presentes**
/// han cuadrado.
pub fn backup_set(parts: &[String], dir: &str, overwrite: bool) -> Value {
    if parts.is_empty() {
        return json!({ "ok": false, "error": "no hay particiones que respaldar" });
    }
    if let Err(e) = validate_dst(dir) {
        return json!({ "ok": false, "error": e });
    }
    let mk = root_exec(
        &format!("mkdir -p {}", sh_quote(dir)),
        20_000,
    );
    if !mk.ok() {
        return json!({
            "ok": false,
            "error": format!("no se pudo crear el directorio {}", dir),
            "output": mk.combined(),
        });
    }

    let mut entries: Vec<Value> = Vec::new();
    let mut all_ok = true;
    let mut missing: Vec<String> = Vec::new();
    for p in parts {
        let dst = default_dump_path(dir, p);
        let r = dump_partition(p, &dst, overwrite);
        let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        let ausente = r
            .get("ausente")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if ausente {
            missing.push(p.clone());
        } else if !ok {
            all_ok = false;
        }
        entries.push(json!({
            "partition": p,
            "file": dst,
            "device": partition_device(p),
            "ok": ok,
            "ausente": ausente,
            "size_bytes": r.get("size_bytes").cloned().unwrap_or(Value::Null),
            "file_bytes": r.get("file_bytes").cloned().unwrap_or(Value::Null),
            "sha256": r.get("sha256").cloned().unwrap_or(Value::Null),
            "source_sha256": r.get("source_sha256").cloned().unwrap_or(Value::Null),
            "integrity_ok": r.get("integrity_ok").cloned().unwrap_or(Value::Null),
            "duration_ms": r.get("duration_ms").cloned().unwrap_or(Value::Null),
            "error": r.get("error").cloned().unwrap_or(Value::Null),
        }));
    }

    let manifest = json!({
        "schema": MANIFEST_SCHEMA,
        "created_at": now_iso(),
        "device": device_identity(),
        "dir": dir,
        "entries": entries,
        "absent_partitions": missing,
        "restore_policy": "RESTAURAR SÓLO CON EL MÓVIL EN RECOVERY (sideload) O DESPUÉS DE UN BOOTLOOP. \
El comando dd de este manifiesto ESCRIBE en la partición: no lo ejecutes sin el volcado verificado.",
    });

    let manifest_path = format!("{}/manifest.json", dir.trim_end_matches('/'));
    let text = serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string());
    let w = root_exec(
        &format!(
            "cat > {} <<'MDKERNEL_EOF'\n{}\nMDKERNEL_EOF",
            sh_quote(&manifest_path),
            text
        ),
        30_000,
    );
    let written = w.ok();

    json!({
        "ok": all_ok && written,
        "manifest": manifest_path,
        "manifest_written": written,
        "dir": dir,
        "entries": entries,
        "absent_partitions": missing,
        "created_at": manifest.get("created_at").cloned().unwrap_or(Value::Null),
        "device": manifest.get("device").cloned().unwrap_or(Value::Null),
    })
}

/// Lee un manifiesto de respaldo.
pub fn read_manifest(path: &str) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("no se pudo leer el manifiesto {}: {}", path, e))?;
    let v: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("el manifiesto {} no es JSON válido: {}", path, e))?;
    Ok(v)
}

/// Verifica un manifiesto: re-hashea los ficheros volcados y (opcionalmente)
/// las particiones vivas.
pub fn verify_manifest(path: &str, check_live: bool) -> Value {
    let manifest = match read_manifest(path) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    let entries = manifest
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut checks: Vec<Value> = Vec::new();
    let mut all_ok = true;
    for e in entries {
        let part = e.get("partition").and_then(|v| v.as_str()).unwrap_or("");
        let file = e.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let want = e.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
        let ausente = e.get("ausente").and_then(|v| v.as_bool()).unwrap_or(false);

        if ausente {
            checks.push(json!({
                "partition": part,
                "file": file,
                "ausente": true,
                "file_ok": true,
                "note": "la partición no existe en este dispositivo: nada que verificar",
            }));
            continue;
        }

        let got: Option<String> = match sha256_file(std::path::Path::new(file)) {
            Ok(v) => Some(v),
            Err(_) => {
                let r = root_exec(
                    &format!("sha256sum {} 2>/dev/null", sh_quote(file)),
                    120_000,
                );
                parse_sha256_line(&r.stdout)
            }
        };
        let file_ok = got.is_some() && got.as_deref() == Some(want);

        let (live_sha, live_ok) = if check_live && !part.is_empty() {
            let ls = partition_sha256(part);
            let ok = ls.is_some() && ls.as_deref() == Some(want);
            (ls, ok)
        } else {
            (None, true)
        };

        if !file_ok {
            all_ok = false;
        }
        checks.push(json!({
            "partition": part,
            "file": file,
            "expected_sha256": want,
            "file_sha256": got,
            "file_ok": file_ok,
            "live_sha256": live_sha,
            "live_matches_backup": live_ok,
        }));
    }

    json!({
        "ok": all_ok,
        "manifest": path,
        "checked_live": check_live,
        "checks": checks,
        "note": if all_ok {
            "los volcados están intactos: el punto de retorno es válido"
        } else {
            "ALGÚN volcado no cuadra: NO es un punto de retorno válido"
        },
    })
}

/// Genera (sin ejecutar) el plan de rollback a partir de un manifiesto.
///
/// Devuelve los órdenes exactos que hay que escribir **a mano** en un
/// recovery o en un `adb shell` con root, y los controles previos. **Este
/// módulo no escribe jamás en una partición.**
pub fn rollback_plan(path: &str) -> Value {
    let manifest = match read_manifest(path) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    let entries = manifest
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut steps: Vec<Value> = Vec::new();
    for e in entries {
        let part = e.get("partition").and_then(|v| v.as_str()).unwrap_or("");
        let file = e.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let sha = e.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
        let ausente = e.get("ausente").and_then(|v| v.as_bool()).unwrap_or(false);
        if ausente || sha.is_empty() {
            continue;
        }
        let dev = partition_device(part);
        steps.push(json!({
            "partition": part,
            "device": dev,
            "source_file": file,
            "expected_sha256": sha,
            "preflight": format!("sha256sum {}   # debe imprimir {}", sh_quote(file), sha),
            "write_command": format!(
                "dd if={} of={} bs=4M conv=fsync   # ESCRIBE en la partición",
                sh_quote(file),
                sh_quote(&dev)
            ),
            "postflight": format!("sha256sum {}   # debe imprimir {}", sh_quote(&dev), sha),
        }));
    }

    let script = format!(
        "# PLAN DE ROLLBACK (NO EJECUTADO) generado el {}\n\
         # Sólo si el móvil NO ARRANCA. Con el móvil en recovery (sideload/Odin)\n\
         # o con ADB y root.\n\
         #\n{}\n\
         # Aviso: esto ESCRIBE en la partición de arranque. Si el volcado no\n\
         # cuadra, PARA: un dd con un fichero corrupto deja el móvil igual de\n\
         # roto y además pierdes el original.\n",
        manifest
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("?"),
        steps
            .iter()
            .map(|s| {
                let p = s.get("partition").and_then(|v| v.as_str()).unwrap_or("");
                let pre = s.get("preflight").and_then(|v| v.as_str()).unwrap_or("");
                let w = s.get("write_command").and_then(|v| v.as_str()).unwrap_or("");
                let post = s.get("postflight").and_then(|v| v.as_str()).unwrap_or("");
                format!("# --- {}\n{}\n{}\n{}\n", p, pre, w, post)
            })
            .collect::<Vec<_>>()
            .join("\n")
    );

    json!({
        "ok": true,
        "manifest": path,
        "created_at": manifest.get("created_at").cloned().unwrap_or(Value::Null),
        "device": manifest.get("device").cloned().unwrap_or(Value::Null),
        "steps": steps,
        "script": script,
        "executed": false,
        "note": "plan SOLO; este módulo nunca escribe en /dev/block",
    })
}

// ========================================================================= //
// Drive (subida del punto de retorno)
// ========================================================================= //

/// Cuerpo JSON de la metadata de la subida (con el SHA-256 en
/// `appProperties` para poder verificar el fichero subido sin volver a
/// bajarlo entero).
pub fn drive_metadata_body(name: &str, folder_id: &str, sha256: &str) -> String {
    json!({
        "name": name,
        "parents": [folder_id],
        "appProperties": { "sha256": sha256, "kind": "kernel-backup" },
    })
    .to_string()
}

/// Cabecera `Content-Range` de un trozo de la subida resumable.
pub fn drive_content_range(offset: u64, len: u64, total: u64) -> String {
    let end = offset + len - 1;
    format!("bytes {}-{}/{}", offset, end, total)
}

/// Longitud del trozo que empieza en `offset` para un fichero de `size`.
pub fn drive_chunk_len(size: u64, offset: u64) -> u64 {
    if offset >= size {
        return 0;
    }
    let rest = size - offset;
    if rest < DRIVE_CHUNK {
        rest
    } else {
        DRIVE_CHUNK
    }
}

/// Extrae el `access_token` de la respuesta de OAuth2.
pub fn parse_oauth_token(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    v.get("access_token")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

/// Metadatos de un fichero en Drive.
#[derive(Debug, Clone, Default)]
pub struct DriveFileMeta {
    pub id: String,
    pub name: String,
    pub size: Option<u64>,
    pub md5: Option<String>,
    pub sha256: Option<String>,
}

/// Extrae los metadatos del primer fichero que coincide con `name`.
pub fn parse_drive_file_meta(body: &str, name: &str) -> Option<DriveFileMeta> {
    let v: Value = serde_json::from_str(body).ok()?;
    let files = v.get("files")?.as_array()?;
    let f = files
        .iter()
        .find(|f| f.get("name").and_then(|n| n.as_str()) == Some(name))
        .or_else(|| files.first())?;
    Some(DriveFileMeta {
        id: f.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        name: f.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        size: f.get("size").and_then(|x| x.as_str()).and_then(|s| s.parse().ok()),
        md5: f.get("md5Checksum").and_then(|x| x.as_str()).map(|s| s.to_string()),
        sha256: f
            .get("appProperties")
            .and_then(|p| p.get("sha256"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
    })
}

/// Agente HTTP con tiempos acotados.
fn drive_agent() -> ureq::Agent {
    let cfg = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(900)))
        .timeout_connect(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build();
    ureq::Agent::new_with_config(cfg)
}

/// Mintea un token de acceso con el `refresh_token` del fichero de credenciales.
///
/// El fichero tiene el formato del cliente OAuth de Google:
/// `{"client_id":…,"client_secret":…,"refresh_token":…}`.
pub fn drive_access_token(creds_path: &str) -> Result<String, String> {
    let raw = std::fs::read_to_string(creds_path)
        .map_err(|e| format!("no se pudo leer {}: {}", creds_path, e))?;
    let v: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("{} no es JSON válido: {}", creds_path, e))?;
    let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let (cid, csec, rt) = (get("client_id"), get("client_secret"), get("refresh_token"));
    if cid.is_empty() || csec.is_empty() || rt.is_empty() {
        return Err(format!(
            "{} no tiene client_id/client_secret/refresh_token",
            creds_path
        ));
    }
    let body = format!(
        "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
        urlencode(&cid),
        urlencode(&csec),
        urlencode(&rt)
    );
    let resp = drive_agent()
        .post(OAUTH_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(body.as_bytes())
        .map_err(|e| format!("OAuth2: {}", e))?;
    let st = resp.status().as_u16();
    let txt = resp.into_body().read_to_vec().unwrap_or_default();
    let txt = String::from_utf8_lossy(&txt).to_string();
    if st != 200 {
        return Err(format!("OAuth2 devolvió {}", st));
    }
    parse_oauth_token(&txt).ok_or_else(|| "la respuesta OAuth2 no trae access_token".to_string())
}

/// `urlencode` mínimo (sólo lo que aparece en credenciales de Google).
pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Sube un fichero a Drive con **subida resumible** y verifica el resultado.
///
/// * Abre la sesión con `uploadType=resumable` y manda `X-Upload-Content-Length`.
/// * Sube en trozos de 8 MiB con `Content-Range`.
/// * Al final consulta el fichero por su nombre y comprueba **tamaño** y
///   **SHA-256** (via `appProperties`), que es la verificación doble.
pub fn drive_upload_file(
    path: &str,
    name: &str,
    folder_id: &str,
    token: &str,
    sha256: &str,
) -> Result<Value, String> {
    let p = Path::new(path);
    let meta = std::fs::metadata(p).map_err(|e| format!("{}: {}", path, e))?;
    let size = meta.len();
    let agent = drive_agent();

    // 1) Sesión de subida.
    let url = format!("{}?uploadType=resumable&supportsAllDrives=true", DRIVE_UPLOAD_API);
    let resp = agent
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json; charset=UTF-8")
        .header("X-Upload-Content-Type", "application/octet-stream")
        .header("X-Upload-Content-Length", size.to_string())
        .send(drive_metadata_body(name, folder_id, sha256).as_bytes())
        .map_err(|e| format!("abrir sesión de subida: {}", e))?;
    let st = resp.status().as_u16();
    let session = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    if st != 200 && st != 201 {
        let b = resp.into_body().read_to_vec().unwrap_or_default();
        let t: String = String::from_utf8_lossy(&b).chars().take(200).collect();
        return Err(format!("abrir sesión devolvió {}: {}", st, t));
    }
    let session = session.ok_or("la sesión de subida no trae la cabecera Location")?;

    // 2) Trozos.
    let mut f = std::fs::File::open(p).map_err(|e| format!("abrir {}: {}", path, e))?;
    let mut buf = vec![0u8; DRIVE_CHUNK as usize];
    let mut offset: u64 = 0;
    while offset < size {
        let want = drive_chunk_len(size, offset) as usize;
        let mut got = 0usize;
        while got < want {
            let n = f
                .read(&mut buf[got..want])
                .map_err(|e| format!("leer {}: {}", path, e))?;
            if n == 0 {
                break;
            }
            got += n;
        }
        if got == 0 {
            return Err("lectura corta: el fichero cambió mientras se subía".to_string());
        }
        let resp = agent
            .put(&session)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/octet-stream")
            .header("Content-Range", drive_content_range(offset, got as u64, size))
            .header("Content-Length", got.to_string())
            .send(&buf[..got])
            .map_err(|e| format!("subir trozo en {}: {}", offset, e))?;
        let st = resp.status().as_u16();
        if st != 200 && st != 201 && st != 308 {
            let b = resp.into_body().read_to_vec().unwrap_or_default();
            let t: String = String::from_utf8_lossy(&b).chars().take(200).collect();
            return Err(format!("subir trozo en {} devolvió {}: {}", offset, st, t));
        }
        offset += got as u64;
    }

    // 3) Verificación por nombre (tamaño + SHA-256 en appProperties).
    let q = format!(
        "{}?supportsAllDrives=true&fields=files(id,name,size,md5Checksum,appProperties)&q={}",
        DRIVE_API,
        urlencode(&format!("name = '{}' and trashed = false", name))
    );
    let resp = agent
        .get(&q)
        .header("Authorization", format!("Bearer {}", token))
        .call()
        .map_err(|e| format!("buscar en Drive: {}", e))?;
    let body = resp.into_body().read_to_vec().unwrap_or_default();
    let body = String::from_utf8_lossy(&body).to_string();
    let meta = parse_drive_file_meta(&body, name)
        .ok_or_else(|| format!("no se encontró '{}' en Drive tras subirlo", name))?;
    let size_ok = meta.size.map(|s| s == size).unwrap_or(false);
    let sha_ok = meta.sha256.as_deref() == Some(sha256);
    Ok(json!({
        "ok": size_ok && sha_ok,
        "name": meta.name,
        "file_id": meta.id,
        "size_bytes": size,
        "size_drive": meta.size,
        "size_ok": size_ok,
        "sha256_local": sha256,
        "sha256_drive": meta.sha256,
        "sha256_ok": sha_ok,
        "md5_drive": meta.md5,
        "folder_id": folder_id,
    }))
}

/// Sube a Drive **todas** las imágenes de un manifiesto y actualiza el
/// manifiesto con los ids de Drive.
///
/// No toca el móvil: sólo lee los ficheros ya volcados.
pub fn upload_manifest_to_drive(
    manifest_path: &str,
    folder_id: &str,
    creds_path: &str,
    name_prefix: &str,
) -> Value {
    let mut manifest = match read_manifest(manifest_path) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    let token = match drive_access_token(creds_path) {
        Ok(t) => t,
        Err(e) => return json!({ "ok": false, "error": e, "stage": "oauth" }),
    };
    let entries = manifest
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut results: Vec<Value> = Vec::new();
    let mut all_ok = true;
    for e in &entries {
        let part = e.get("partition").and_then(|v| v.as_str()).unwrap_or("");
        let file = e.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let sha = e.get("sha256").and_then(|v| v.as_str()).unwrap_or("");
        let ausente = e.get("ausente").and_then(|v| v.as_bool()).unwrap_or(false);
        if ausente || sha.is_empty() {
            continue;
        }
        let name = format!("{}-{}.img", name_prefix, part);
        match drive_upload_file(file, &name, folder_id, &token, sha) {
            Ok(r) => {
                let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !ok {
                    all_ok = false;
                }
                let mut r = r;
                if let Some(o) = r.as_object_mut() {
                    o.insert("partition".to_string(), json!(part));
                    o.insert("file".to_string(), json!(file));
                }
                results.push(r);
            }
            Err(e) => {
                all_ok = false;
                results.push(json!({ "ok": false, "partition": part, "file": file, "error": e }));
            }
        }
    }

    // Actualiza el manifiesto en disco con los ids de Drive (para el rollback).
    if let Some(obj) = manifest.as_object_mut() {
        obj.insert("drive_folder_id".to_string(), json!(folder_id));
        obj.insert("drive_uploaded_at".to_string(), json!(now_iso()));
        obj.insert("drive_results".to_string(), json!(results));
    }
    let text = serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string());
    let w = std::fs::write(manifest_path, text);

    json!({
        "ok": all_ok && w.is_ok(),
        "manifest": manifest_path,
        "manifest_updated": w.is_ok(),
        "folder_id": folder_id,
        "results": results,
        "note": "verificación doble: tamaño exacto + SHA-256 en appProperties",
    })
}

// ========================================================================= //
// Cableado MCP
// ========================================================================= //

/// Tool MCP `kernel_backup`.
///
/// Acciones: `info` | `dmesg` | `partitions` | `processes` | `fingerprint` |
/// `backup` | `verify` | `upload_drive` | `rollback_plan`.
pub fn tool_dispatch(args: &Value) -> Value {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let b = |k: &str| args.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    let u = |k: &str| args.get(k).and_then(|v| v.as_u64()).unwrap_or(0);

    let action = s("action");
    match action.as_str() {
        "info" => info(),
        "dmesg" => dmesg(u("lines") as u32),
        "partitions" => partitions(),
        "processes" => processes(),
        "fingerprint" => fingerprint(&s("partition")),
        "backup" => {
            let parts: Vec<String> = s("partitions")
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
            let parts = if parts.is_empty() {
                DEFAULT_BOOT_PARTITIONS
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
            } else {
                parts
            };
            let dir = s("dir");
            let dir = if dir.is_empty() {
                "/data/local/tmp/kernel-backup".to_string()
            } else {
                dir
            };
            backup_set(&parts, &dir, b("overwrite"))
        }
        "verify" => verify_manifest(&s("manifest"), b("check_live")),
        "upload_drive" => {
            let manifest = s("manifest");
            let manifest = if manifest.is_empty() {
                "/data/local/tmp/kernel-backup/manifest.json".to_string()
            } else {
                manifest
            };
            let folder = s("folder_id");
            let folder = if folder.is_empty() {
                DEFAULT_DRIVE_FOLDER.to_string()
            } else {
                folder
            };
            let creds = s("credentials");
            let creds = if creds.is_empty() {
                "/data/user/0/com.agy/files/megadaemon/state/gdrive_credentials.json"
                    .to_string()
            } else {
                creds
            };
            let prefix = s("name_prefix");
            let prefix = if prefix.is_empty() {
                format!("kernel-backup-{}", now_iso().replace(':', "-"))
            } else {
                prefix
            };
            upload_manifest_to_drive(&manifest, &folder, &creds, &prefix)
        }
        "rollback_plan" => rollback_plan(&s("manifest")),
        "" => json!({ "ok": false, "error": "falta la acción" }),
        other => json!({ "ok": false, "error": format!("acción no soportada: {}", other) }),
    }
}

/// Definición de la tool MCP `kernel_backup` (para `tools/list`).
pub fn tool_definition() -> Value {
    json!({
        "name": "kernel_backup",
        "description": "Estado del kernel y RESPALDO SEGURO de particiones de arranque: vuelca a fichero, \
verifica SHA-256 (origen y fichero), sube el respaldo a Google Drive y genera el plan de rollback. \
Nunca escribe en /dev/block: el volcado es de sólo lectura y la restauración se genera como texto.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["info", "dmesg", "partitions", "processes", "fingerprint", "backup", "verify", "upload_drive", "rollback_plan"]
                },
                "partition": { "type": "string", "description": "Nombre de la partición (boot, vendor_boot, recovery…)" },
                "partitions": { "type": "string", "description": "Lista separada por comas para action=backup" },
                "dir": { "type": "string", "description": "Directorio destino del volcado (action=backup)" },
                "manifest": { "type": "string", "description": "Ruta del manifiesto (action=verify / upload_drive / rollback_plan)" },
                "folder_id": { "type": "string", "description": "Carpeta de Drive (action=upload_drive)" },
                "credentials": { "type": "string", "description": "Fichero de credenciales OAuth de Google (action=upload_drive)" },
                "name_prefix": { "type": "string", "description": "Prefijo de los nombres en Drive (action=upload_drive)" },
                "overwrite": { "type": "boolean", "default": false },
                "check_live": { "type": "boolean", "default": false },
                "lines": { "type": "integer", "default": 200 }
            },
            "required": ["action"]
        }
    })
}

// ========================================================================= //
// Utilidades locales
// ========================================================================= //

/// SHA-256 de un fichero local (streaming).
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(path)
        .map_err(|e| format!("abrir {}: {}", path.display(), e))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("leer {}: {}", path.display(), e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// Ruta por defecto del manifiesto en un directorio de respaldo.
pub fn manifest_path(dir: &str) -> PathBuf {
    PathBuf::from(format!("{}/manifest.json", dir.trim_end_matches('/')))
}

// ========================================================================= //
// Tests (sin red, sin root)
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valida_nombres_de_particion() {
        assert!(validate_partition("boot").is_ok());
        assert!(validate_partition("vendor_boot").is_ok());
        assert!(validate_partition("init_boot").is_ok());
        assert!(validate_partition("").is_err());
        assert!(validate_partition("boot;rm").is_err());
        assert!(validate_partition("../boot").is_err());
        assert!(validate_partition(".boot").is_err());
    }

    #[test]
    fn valida_destino_y_prohibioke_dev() {
        assert!(validate_dst("/data/local/tmp/x").is_ok());
        assert!(validate_dst("data/tmp").is_err());
        assert!(validate_dst("/data/../etc").is_err());
        assert!(validate_dst("/dev/block/by-name/boot").is_err());
        assert!(validate_dst("/dev/null").is_err());
    }

    #[test]
    fn extrae_sha256_de_sha256sum() {
        let out = "f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd  /dev/block/by-name/boot";
        assert_eq!(
            parse_sha256_line(out).as_deref(),
            Some("f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd")
        );
        assert!(parse_sha256_line("no hay hash aquí").is_none());
    }

    #[test]
    fn iso_utc_desde_unix() {
        assert_eq!(iso_utc_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc_from_unix(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn content_range_y_trozos() {
        assert_eq!(drive_content_range(0, 8, 16), "bytes 0-7/16");
        assert_eq!(drive_content_range(8, 8, 16), "bytes 8-15/16");
        // el trozo nunca pasa del tamaño del fichero
        assert_eq!(drive_chunk_len(16, 0), 16);
        assert_eq!(drive_chunk_len(16, 8), 8);
        assert_eq!(drive_chunk_len(20, 16), 4);
        assert_eq!(drive_chunk_len(20, 20), 0);
        // con un fichero grande el trozo es el de Drive (8 MiB)
        let big = 20 * 1024 * 1024;
        assert_eq!(drive_chunk_len(big, 0), DRIVE_CHUNK);
        assert_eq!(drive_chunk_len(big, 16 * 1024 * 1024), 4 * 1024 * 1024);
        assert_eq!(DRIVE_CHUNK % 262_144, 0, "Drive exige múltiplos de 256 KiB");
    }

    #[test]
    fn urlencoda_credenciales() {
        assert_eq!(urlencode("abc-123_XY.z~"), "abc-123_XY.z~");
        assert_eq!(urlencode("a b/c"), "a%20b%2Fc");
        assert_eq!(urlencode("a+b"), "a%2Bb");
    }

    #[test]
    fn metadata_de_subida_lleva_sha256() {
        let body = drive_metadata_body("k-boot.img", "folder123", "deadbeef");
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v.get("name").unwrap().as_str().unwrap(), "k-boot.img");
        assert_eq!(
            v.get("parents").unwrap().as_array().unwrap()[0].as_str().unwrap(),
            "folder123"
        );
        assert_eq!(
            v.get("appProperties").unwrap().get("sha256").unwrap().as_str().unwrap(),
            "deadbeef"
        );
    }

    #[test]
    fn token_de_oauth_se_parsea() {
        assert_eq!(
            parse_oauth_token(r#"{"access_token":"abc","expires_in":3599}"#).as_deref(),
            Some("abc")
        );
        assert!(parse_oauth_token(r#"{"error":"invalid_grant"}"#).is_none());
    }

    #[test]
    fn metadatos_de_drive_se_parsean() {
        let body = r#"{"files":[{"id":"ID1","name":"k-boot.img","size":"67108864",
            "md5Checksum":"0abc","appProperties":{"sha256":"f96c"}}]}"#;
        let m = parse_drive_file_meta(body, "k-boot.img").unwrap();
        assert_eq!(m.id, "ID1");
        assert_eq!(m.size, Some(67108864));
        assert_eq!(m.md5.as_deref(), Some("0abc"));
        assert_eq!(m.sha256.as_deref(), Some("f96c"));
    }

    #[test]
    fn rollback_plan_no_escribe_y_filtra_ausentes() {
        let dir = std::env::temp_dir().join("mdk_test_rollback");
        let _ = std::fs::create_dir_all(&dir);
        let manifest = dir.join("manifest.json");
        let txt = json!({
            "schema": MANIFEST_SCHEMA,
            "created_at": "2026-11-22T13:40:39Z",
            "device": {"model": "SM-G998B"},
            "entries": [
                {"partition": "boot", "file": "/data/local/tmp/kernel-backup/boot.img",
                 "sha256": "f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd"},
                {"partition": "init_boot", "file": "/data/local/tmp/kernel-backup/init_boot.img",
                 "sha256": "", "ausente": true}
            ]
        })
        .to_string();
        std::fs::write(&manifest, txt).unwrap();
        let plan = rollback_plan(manifest.to_str().unwrap());
        assert_eq!(plan.get("ok").unwrap().as_bool().unwrap(), true);
        let steps = plan.get("steps").unwrap().as_array().unwrap();
        assert_eq!(steps.len(), 1, "init_boot ausente no debe generar paso de escritura");
        assert_eq!(plan.get("executed").unwrap().as_bool().unwrap(), false);
        let script = plan.get("script").unwrap().as_str().unwrap();
        assert!(script.contains("dd if="));
        assert!(script.contains("ESCRIBE en la partición"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn manifiesto_invalido_da_error_legible() {
        let v = verify_manifest("/no/existe/manifest.json", false);
        assert_eq!(v.get("ok").unwrap().as_bool().unwrap(), false);
        assert!(v.get("error").unwrap().as_str().unwrap().contains("no se pudo leer"));
    }

    #[test]
    fn tool_dispatch_exige_accion() {
        let v = tool_dispatch(&json!({}));
        assert_eq!(v.get("ok").unwrap().as_bool().unwrap(), false);
        let v = tool_dispatch(&json!({"action": "inventada"}));
        assert!(v.get("error").unwrap().as_str().unwrap().contains("no soportada"));
    }

    #[test]
    fn tool_definition_declara_el_contrato() {
        let d = tool_definition();
        assert_eq!(d.get("name").unwrap().as_str().unwrap(), "kernel_backup");
        let actions = d
            .get("inputSchema")
            .unwrap()
            .get("properties")
            .unwrap()
            .get("action")
            .unwrap()
            .get("enum")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        assert!(actions.contains(&json!("backup")));
        assert!(actions.contains(&json!("verify")));
        assert!(actions.contains(&json!("upload_drive")));
        assert!(actions.contains(&json!("rollback_plan")));
    }
}

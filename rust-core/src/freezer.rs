use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLock {
    pub lock_id: String,
    pub target: String,
    pub offset: u64,
    pub offset_hex: String,
    pub value_hex: String,
    #[serde(skip)]
    pub value_bytes: Vec<u8>,
    pub description: String,
    pub active: bool,
    pub write_count: u64,
    pub last_write: f64,
}

/// Raíces permitidas para escribir.  El daemon corre como root en el móvil:
/// sin esta restricción, `/api/memory_freeze` (que NO tiene autenticación)
/// permitía escribir en CUALQUIER ruta del sistema.
/// Ficheros concretos permitidos (la SHM de la VLA vive fuera de las raíces).
pub const SANDBOX_FILES: &[&str] = &["/data/user/0/com.agy/files/snatcher_vla_shm.bin"];

pub const SANDBOX_ROOTS: &[&str] = &[
    "/data/user/0/com.agy/files/megadaemon",
    // MEDIDO (2026-11-23): el daemon ahora vive en `/data/adb/megadaemon`
    // (estado persistente).  Sin esta raíz, el congelador de memoria
    // bloqueaba las escrituras del propio daemon en su state dir.
    "/data/adb/megadaemon",
    "/data/user/0/com.agy/files/snatcher",
    "/data/user/0/com.agy/files/usr",
];

/// Valida que el objetivo de un "memory freeze" esté dentro del sandbox.
///
/// Reglas: ruta absoluta, sin `..`, dentro de una raíz permitida, fichero
/// regular existente y tamaño razonable.
pub fn validate_target(target: &str) -> Result<(), String> {
    if target.is_empty() {
        return Err("target vacio".to_string());
    }
    if !target.starts_with('/') {
        return Err(format!("target debe ser una ruta absoluta: {}", target));
    }
    if target.contains("..") {
        return Err(format!("target con '..' rechazado: {}", target));
    }
    let canonical = std::fs::canonicalize(target)
        .map_err(|e| format!("target inaccesible ({}): {}", target, e))?;
    let c = canonical.to_string_lossy().to_string();
    if !SANDBOX_ROOTS.iter().any(|root| c.starts_with(root))
        && !SANDBOX_FILES.iter().any(|f| c == *f)
    {
        return Err(format!(
            "target fuera del sandbox: {} (raices permitidas: {:?})",
            c, SANDBOX_ROOTS
        ));
    }
    let md = std::fs::metadata(&canonical)
        .map_err(|e| format!("metadata: {}", e))?;
    if !md.is_file() {
        return Err(format!("target no es un fichero regular: {}", c));
    }
    if md.len() > 64 * 1024 * 1024 {
        return Err(format!("target demasiado grande ({} bytes)", md.len()));
    }
    Ok(())
}

static FREEZER_LOCKS: OnceLock<Mutex<HashMap<String, MemoryLock>>> = OnceLock::new();
static FREEZER_THREAD: OnceLock<()> = OnceLock::new();

fn now_sec() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn ensure_freezer_started() -> &'static Mutex<HashMap<String, MemoryLock>> {
    let locks = FREEZER_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    FREEZER_THREAD.get_or_init(|| {
        thread::spawn(|| loop {
            thread::sleep(Duration::from_millis(16)); // ~60 Hz
            if let Some(m) = FREEZER_LOCKS.get() {
                if let Ok(mut map) = m.lock() {
                    let ts = now_sec();
                    for lock in map.values_mut() {
                        if lock.active && !lock.value_bytes.is_empty() {
                            if let Ok(mut f) = OpenOptions::new().read(true).write(true).open(&lock.target) {
                                if f.seek(SeekFrom::Start(lock.offset)).is_ok() {
                                    if f.write_all(&lock.value_bytes).is_ok() {
                                        let _ = f.flush();
                                        lock.write_count += 1;
                                        lock.last_write = ts;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    });
    locks
}

pub fn decode_hex_bytes(hex_str: &str) -> Result<Vec<u8>, String> {
    let clean: String = hex_str
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if clean.len() % 2 != 0 {
        return Err("Odd hex length".into());
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    for i in (0..clean.len()).step_by(2) {
        let b = u8::from_str_radix(&clean[i..i + 2], 16)
            .map_err(|e| format!("Invalid hex: {}", e))?;
        out.push(b);
    }
    Ok(out)
}

pub fn add_lock(
    lock_id: &str,
    target: &str,
    offset: u64,
    value_hex: &str,
    description: &str,
) -> Result<MemoryLock, String> {
    // v13.2 (HW-02): el target viene del cliente HTTP por un endpoint SIN
    // autenticación.  Sin esta validación, el daemon (que corre como root)
    // escribía en cualquier ruta del sistema.
    validate_target(target)?;
    let value_bytes = decode_hex_bytes(value_hex)?;
    let lock = MemoryLock {
        lock_id: lock_id.to_string(),
        target: target.to_string(),
        offset,
        offset_hex: format!("0x{:08X}", offset),
        value_hex: value_hex.to_uppercase(),
        value_bytes,
        description: description.to_string(),
        active: true,
        write_count: 0,
        last_write: 0.0,
    };
    let m = ensure_freezer_started();
    if let Ok(mut map) = m.lock() {
        map.insert(lock_id.to_string(), lock.clone());
    }
    Ok(lock)
}

pub fn remove_lock(lock_id: &str) -> bool {
    let m = ensure_freezer_started();
    if let Ok(mut map) = m.lock() {
        return map.remove(lock_id).is_some();
    }
    false
}

pub fn toggle_lock(lock_id: &str, active: Option<bool>) -> Option<MemoryLock> {
    let m = ensure_freezer_started();
    if let Ok(mut map) = m.lock() {
        if let Some(lock) = map.get_mut(lock_id) {
            lock.active = active.unwrap_or(!lock.active);
            return Some(lock.clone());
        }
    }
    None
}

pub fn list_locks() -> Vec<MemoryLock> {
    let m = ensure_freezer_started();
    if let Ok(map) = m.lock() {
        let mut v: Vec<MemoryLock> = map.values().cloned().collect();
        v.sort_by(|a, b| a.lock_id.cmp(&b.lock_id));
        return v;
    }
    Vec::new()
}

pub fn clear_locks() {
    let m = ensure_freezer_started();
    if let Ok(mut map) = m.lock() {
        map.clear();
    }
}

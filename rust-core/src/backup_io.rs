//! Utilidades de I/O y de sistema **sin dependencias** del resto del crate.
//!
//! POR QUÉ EXISTE ESTE MÓDULO
//! --------------------------
//! `backup_inventory` (el motor de inventario) arrastra ~25 módulos del crate
//! (adb, DEX, kiosk, llamadas, portapapeles…). `backup_apps` (el motor de
//! backup) sólo necesita de él **tres** cosas: bajar la prioridad de I/O,
//! leer el loadavg y sacar bytes del CSPRNG.
//!
//! MEDIDO 2026-11-22: con esas tres funciones dentro de `backup_inventory`,
//! el binario `md-backup` para ARM64 **no se podía compilar** en un contenedor
//! de 3 GB: rustc pide ~2,2 GB para el crate entero y el OOM killer lo mata
//! (6 SIGKILL medidos). Sacándolas a este módulo, el mismo binario se compila
//! con un pico de ~400 MB.
//!
//! ⇒ **Una sola fuente de verdad**: las funciones viven aquí y
//! `backup_inventory` las re-exporta (`pub use`), así que su API pública no
//! cambia y nadie se rompe.

/// Baja la prioridad de I/O del proceso actual.
///
/// * `nice`: prioridad de CPU (0 = normal, 19 = la más baja).
/// * `ionice_class`: 1 = realtime, 2 = best-effort, 3 = idle (la más baja).
///
/// Se aplica al **propio proceso**: el escaneo nunca compite con el usuario.
pub fn apply_io_limits(nice: i32, ionice_class: u8) -> String {
    let mut notes = Vec::new();
    // nice: `setpriority(PRIO_PROCESS, 0, nice)`.
    let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) };
    notes.push(format!(
        "nice={} ({})",
        nice,
        if rc == 0 { "ok" } else { "no aplicado" }
    ));
    // ionice: `ioprio_set(IOPRIO_WHO_PROCESS, 0, prio)` vía syscall.
    #[cfg(target_arch = "aarch64")]
    const SYS_IOPRIO_SET: libc::c_long = 252;
    #[cfg(target_arch = "x86_64")]
    const SYS_IOPRIO_SET: libc::c_long = 251;
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    const SYS_IOPRIO_SET: libc::c_long = -1;
    if SYS_IOPRIO_SET > 0 {
        let class = (ionice_class as i32).clamp(0, 3);
        let prio = (class << 13) | 4; // data=4 (prioridad baja dentro de la clase)
        let rc = unsafe { libc::syscall(SYS_IOPRIO_SET, 1i32, 0i32, prio) };
        notes.push(format!(
            "ionice_class={} ({})",
            class,
            if rc == 0 { "ok" } else { "no aplicado" }
        ));
    }
    notes.join(" ")
}

/// Carga del sistema (1 min, 5 min, 15 min).
pub fn loadavg_tuple(loadavg: &str) -> (f64, f64, f64) {
    let mut it = loadavg.split_whitespace();
    let a = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let b = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let c = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    (a, b, c)
}

/// Bytes aleatorios (CSPRNG del sistema).
pub fn random_bytes(n: usize) -> Result<Vec<u8>, String> {
    let mut out = vec![0u8; n];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut out)
        .map_err(|_| "no se puede leer del CSPRNG del sistema".to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_loadavg_se_parsea_en_tres_numeros() {
        let (a, b, c) = loadavg_tuple("0.52 0.58 0.59 1/234 5678");
        assert!((a - 0.52).abs() < 1e-9);
        assert!((b - 0.58).abs() < 1e-9);
        assert!((c - 0.59).abs() < 1e-9);
    }

    #[test]
    fn un_loadavg_roto_no_panica() {
        let (a, b, c) = loadavg_tuple("");
        assert_eq!((a, b, c), (0.0, 0.0, 0.0));
    }

    #[test]
    fn el_csprng_devuelve_los_bytes_pedidos() {
        let v = random_bytes(32).expect("csprng");
        assert_eq!(v.len(), 32);
        // No puede ser todo ceros con probabilidad practically nula.
        assert!(v.iter().any(|b| *b != 0));
    }
}

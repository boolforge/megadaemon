//! Directorio de estado de la instancia: **una sola fuente de verdad**.
//!
//! ¿POR QUÉ?
//! ---------
//! `cockpit_server::state_dir()` es la ruta que usan el daemon, la bóveda de
//! proveedores (`providers.rs`), el cliente de Drive (`google_auth.rs`) y el
//! motor de backup. Al estar dentro de `cockpit_server` (un módulo de 107 KB
//! que arrastra medio crate), cualquier binario que quiera usar la bóveda
//! tiene que compilar el daemon entero.
//!
//! MEDIDO 2026-11-22: el binario `md-backup` ARM64 pedía ~2,2 GB al rustc y
//! el OOM killer lo mataba (6 SIGKILL) en un contenedor de 3 GB ⇒ el módulo
//! se extrae aquí y `cockpit_server::state_dir()` **delega** en él ⇒ una sola
//! definición, dos usuarios.

/// Directorio de estado por defecto (el de la app en el móvil).
pub const STATE_DIR: &str = "/data/user/0/com.agy/files/megadaemon/state";

/// Directorio de estado de la instancia.
///
/// * `MEGADAEMON_STATE_DIR` si está puesto (y no está vacío);
/// * si no, [`STATE_DIR`].
pub fn state_dir() -> String {
    if let Ok(d) = std::env::var("MEGADAEMON_STATE_DIR") {
        let d = d.trim();
        if !d.is_empty() {
            return d.to_string();
        }
    }
    STATE_DIR.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_default_es_el_de_la_app() {
        // Sin `MEGADAEMON_STATE_DIR` el valor es el del móvil.
        if std::env::var("MEGADAEMON_STATE_DIR").is_err() {
            assert_eq!(state_dir(), STATE_DIR);
        }
    }
}

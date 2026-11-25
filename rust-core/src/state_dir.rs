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

/// Estado de los TESTS: un directorio **por hilo de test**.
///
/// MEDIDO 2026-11-25 (CI rojo, run 0d74b89): los tests de Rust corren en
/// paralelo dentro del mismo proceso y TODOS comparten el mismo
/// `state_dir` ⇒ se pisan.  Rojo medido: `devlog` leía la entrada que
/// había escrito otro test (`left: String("masa")`, `right: "prueba"`) y
/// `task_scheduler` veía la tarea reclamada por otro test.
///
/// Aquí el directorio se deriva del **nombre del hilo**, que en Rust es el
/// nombre del test ⇒ cada test tiene su propio estado (ficheros, cola,
/// bitácora) y los tests que fijan `MEGADAEMON_STATE_DIR` siguen mandando
/// (esa variable tiene prioridad).
#[cfg(test)]
pub fn state_dir_de_test() -> String {
    // Si el estado está FIJADO por entorno, ese es el estado y no se toca:
    // los tests que lo fijan ya eligen un directorio propio (y sus rutas
    // esperadas dependen de él).
    if let Ok(d) = std::env::var("MEGADAEMON_STATE_DIR") {
        let d = d.trim();
        if !d.is_empty() {
            return d.to_string();
        }
    }
    let base = state_dir();
    let nombre = std::thread::current().name().unwrap_or("main").to_string();
    let limpio: String = nombre
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{base}/test-{limpio}")
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

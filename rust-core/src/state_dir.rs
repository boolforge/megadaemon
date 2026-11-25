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
    // MEDIDO 2026-11-25 (CI rojo, 3ª vez): en los TESTS el estado es el
    // temporal por hilo (abajo).  Antes, `devlog::ruta()` usaba esta
    // función ⇒ escribía en la ruta del móvil (`/data/user/0/...`), que en
    // el CI no es escribible ⇒ `append()` fallaba en silencio y el test
    /// leía 0 entradas.  En local pasaba (root) y en CI no.
    #[cfg(test)]
    {
        return state_dir_de_test();
    }
    state_dir_real()
}

/// El estado REAL (producción): la env var si está puesta, si no el de la app.
fn state_dir_real() -> String {
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
    // MEDIDO 2026-11-25 (CI rojo, dos veces): la env var
    // `MEGADAEMON_STATE_DIR` es GLOBAL AL PROCESO y los tests corren en
    // PARALELO => el test que la fijaba se la quitaba al otro de al lado
    // (el `devlog` leía 0 entradas, `task_scheduler` veía la tarea
    // reclamada).  Aquí la env var NO se mira: el estado de cada test es
    // su temporal, derivado del hilo.
    //
    // MEDIDO 2026-11-25 (CI rojo): el state dir por DEFECTO es la ruta del
    // móvil (`/data/user/0/com.agy/files/megadaemon/state`), que en el CI no
    // es escribible => "Permission denied (os error 13)".  Los tests viven
    // en un temporal, por hilo.
    let base = std::env::temp_dir().join(format!("megadaemon-test-{}", std::process::id()));
    // La clave es el **ThreadId**, no el nombre: MEDIDO (CI rojo, 3ª vez) el
    // devlog seguia fallando de forma intermitente porque dos hilos con el
    // MISMO nombre (o sin nombre) compartían el mismo directorio.  El
    // ThreadId es único; el nombre va sólo para que la ruta se entienda.
    let tid = format!("{:?}", std::thread::current().id());
    let tid_limpio: String = tid
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let nombre = std::thread::current().name().unwrap_or("hilo").to_string();
    let nombre_limpio: String = nombre
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    base.join(format!("t-{tid_limpio}-{nombre_limpio}"))
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_default_es_el_de_la_app() {
        // Sin `MEGADAEMON_STATE_DIR`, EN PRODUCCIÓN el valor es el del móvil.
        // MEDIDO 2026-11-25: en los TESTS no (la ruta del móvil no es
        // escribible en el CI y los tests se pisarían entre sí) ⇒ el estado
        // de un test es su temporal por hilo.
        assert_eq!(state_dir_real(), STATE_DIR);
        // Y el estado de un test es un temporal, escribible y único por hilo.
        let s = state_dir();
        assert!(s.starts_with(std::env::temp_dir().to_str().unwrap_or("/tmp")), "{s}");
        assert!(s.contains("megadaemon-test-"), "{s}");
    }

    /// MEDIDO (CI rojo, 3ª vez): dos hilos con el MISMO nombre compartían
    /// el estado ⇒ el test leía lo que había escrito el otro.  Con el
    /// ThreadId la clave es única.
    #[test]
    fn el_estado_es_distinto_por_hilo() {
        let a = state_dir();
        let b = std::thread::spawn(|| state_dir())
            .join()
            .expect("hilo");
        assert_ne!(a, b, "dos hilos no pueden compartir el estado");
    }
}

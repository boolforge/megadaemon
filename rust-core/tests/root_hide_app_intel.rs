//! Cableado de los módulos nuevos para que la suite se pueda ejecutar sin
//! tocar `src/lib.rs`.
//!
//! `src/lib.rs` está **fuera de los ficheros que se pueden tocar** en este
//! encargo, así que los módulos `root_hide` y `app_intel` no quedan
//! registrados como `pub mod` de la biblioteca y `cargo test` no los
//! compilaría. Este fichero de test los incluye con `#[path]`, de forma que
//! los tests que viven dentro de ellos se ejecutan de verdad.
//!
//! Cuando se pueda editar `src/lib.rs`, la alternativa (y la recomendada) es
//! añadir estas dos líneas y borrar este fichero:
//!
//! ```text
//! pub mod app_intel;
//! pub mod root_hide;
//! ```
//!
//! Nota: los tests de esta crate son los de `src/root_hide.rs` y
//! `src/app_intel.rs`; aquí sólo se cablea el acceso.

// Los dos módulos YA estan cableados en `src/lib.rs` (`pub mod root_hide;` y
// `pub mod app_intel;`). Este arnes se include con `#[path]` por historia, pero
// al hacerlo `crate::` pasa a ser la raiz de ESTE crate y `app_intel` deja de
// encontrar a `crate::apk_inventory` (E0433). Patron de la flota: los arneses
// usan la biblioteca (`use megadaemon_core::...;`) en vez de copiar modulos.
use megadaemon_core::{app_intel, root_hide};

/// Comprobación de humo: los dos módulos deben exponer su API principal.
#[test]
fn los_dos_modulos_nuevos_estan_cableados() {
    assert!(root_hide::default_profile().denylist_apps.len() >= 10);
    assert!(app_intel::known_profiles().len() >= 12);
}

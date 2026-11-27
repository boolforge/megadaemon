//! Comprobación de humo de los módulos `root_hide` y `app_intel`.
//!
//! MEDIDO (2026-11-27, CI): este fichero inclusionaba los módulos con
//! `#[path = "../src/root_hide.rs"]`, pero `src/lib.rs` **ya los declara**
//! (`pub mod app_intel;` y `pub mod root_hide;`).  Al re-incluirlos aquí, el
//! módulo se compilaba **dentro del binario de test**, donde `crate::` es el
//! crate del test y NO la biblioteca ⇒ `src/app_intel.rs` rompia con
//!
//! ```text
//! error[E0433]: cannot find `apk_inventory` in the crate root
//!     --> src/app_intel.rs:2738
//! ```
//!
//! y el target de test `root_hide_app_intel` no compilaba (el job de calidad
//! caia).  Ahora se usan los módulos **de la biblioteca**, que es lo que
//! recomienda el propio comentario original de este fichero.

use megadaemon_core::{app_intel, root_hide};

/// Comprobación de humo: los dos módulos deben exponer su API principal.
#[test]
fn los_dos_modulos_nuevos_estan_cableados() {
    assert!(root_hide::default_profile().denylist_apps.len() >= 10);
    assert!(app_intel::known_profiles().len() >= 12);
}

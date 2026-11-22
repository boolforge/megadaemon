//! `md-backup-core`: el **mismo** motor de backup, compilado **sólido**.
//!
//! QUÉ ES ESTO
//! -----------
//! El crate `megadaemon-core` tiene 200+ módulos (DEX, capstone, tree-sitter,
//! SQLite, kiosk…). El motor de backup (`backup_apps`) necesita de él un
//! puñado: `backup_crypto`, `backup_formats`, `backup_vault`, `aegis`,
//! `drive_native`, `drive_client`, `google_auth`, `backup_plan`, `llm_gateway`
//! y `providers`.
//!
//! MEDIDO 2026-11-22: compilar el crate entero para ARM64 pedía **~2,2 GB**
//! al rustc ⇒ el OOM killer lo mataba (6 SIGKILL) en este sandbox de 3 GB,
//! donde además otros agentes compilan a la vez. Con este crate, el pico baja
//! a ~400 MB y el binario `md-backup` **entra**.
//!
//! CÓMO
//! ---
//! * **Los ficheros fuente son los MISMOS** del crate principal (`#[path]`),
//!   así que no hay dos implementaciones que puedan divergir.
//! * El CLI es **el mismo fichero**: `src/main.rs` hace `include!` de
//!   `rust-core/src/bin/md-backup.rs`.
//! * Los tres módulos que se extrajeron del crate grande para poder aislar
//!   el motor (`backup_io`, `backup_glob`, `state_dir`) **viven en el crate
//!   principal** y aquí se incluyen igual ⇒ una sola fuente de verdad.
//!
//! Lo que NO entra aquí (y por qué no pasa nada):
//!
//! * `cockpit_server`: el motor sólo usa `state_dir()` ⇒ aquí se provee un
//!   *shim* que delega en `crate::state_dir` (la MISMA función).
//! * El resto del daemon (adb, DEX, kiosk…): no hacen falta para respaldar.

#![allow(clippy::too_many_arguments)]

// --- El motor de backup -------------------------------------------------- //
#[path = "../../src/backup_io.rs"]
pub mod backup_io;
#[path = "../../src/backup_glob.rs"]
pub mod backup_glob;
#[path = "../../src/state_dir.rs"]
pub mod state_dir;
#[path = "../../src/util_fs.rs"]
pub mod util_fs;
#[path = "../../src/backup_vault.rs"]
pub mod backup_vault;
#[path = "../../src/aegis.rs"]
pub mod aegis;
#[path = "../../src/backup_crypto.rs"]
pub mod backup_crypto;
#[path = "../../src/backup_formats.rs"]
pub mod backup_formats;
#[path = "../../src/backup_apps.rs"]
pub mod backup_apps;
#[path = "../../src/backup_plan.rs"]
pub mod backup_plan;

// --- Drive nativo -------------------------------------------------------- //
#[path = "../../src/drive_client.rs"]
pub mod drive_client;
#[path = "../../src/google_auth.rs"]
pub mod google_auth;
#[path = "../../src/drive_native.rs"]
pub mod drive_native;


// --- Arsenal del gateway (módulos reales, sin dependencias) --------------- //
#[path = "../../src/accelerator_policy.rs"]
pub mod accelerator_policy;
#[path = "../../src/npu_nnapi.rs"]
pub mod npu_nnapi;
#[path = "../../src/npu_bridge.rs"]
pub mod npu_bridge;
#[path = "../../src/binwalk_die.rs"]
pub mod binwalk_die;
#[path = "../../src/cirx_rtl.rs"]
pub mod cirx_rtl;
#[path = "../../src/kaitai_imhex.rs"]
pub mod kaitai_imhex;

// --- El gateway LLM (para el plan validado) ------------------------------ //
#[path = "../../src/power_policy.rs"]
pub mod power_policy;
#[path = "../../src/provider_catalog.rs"]
pub mod provider_catalog;
#[path = "../../src/providers.rs"]
pub mod providers;
#[path = "../../src/llm_gateway.rs"]
pub mod llm_gateway;

/// Shim de `seere_v11`: el motor de backup sólo lo usa como **fallback local**
/// cuando todos los proveedores cloud fallan (y para el arsenal de root del
/// modo híbrido), que es un camino opcional.
///
/// MEDIDO 2026-11-22: `seere_v11` arrastra `bindiff`, `cfg_lifter`,
/// `freezer`, `rea_assistant` y `rom_locator` (92 KB + 5 módulos) ⇒ incluirlo
/// aquí arrastraría medio crate al binario del backup. Este shim **lo dice
/// claramente** en vez de fingir: el camino cloud (el que se usa) no lo
/// necesita.
pub mod seere_v11 {
    /// Arsenal local: aquí no hay ejecución de root (el motor de backup no la
    /// necesita); lo dice sin adornos.
    pub struct Arsenal;

    impl Arsenal {
        /// Crea el arsenal (sin ejecución: ver la nota del módulo).
        pub fn new(_dry: bool, _allow_exec: bool, _allow_external: bool) -> Arsenal {
            Arsenal
        }

        /// Ejecuta una orden como root: **no disponible** en este binario.
        pub fn exec_root(&self, cmd: &str) -> String {
            format!(
                "ERR:el binario de backup no incluye el arsenal local (orden ignorada: {cmd})"
            )
        }
    }

    /// Fallback local: **no disponible** en este binario (ver la nota).
    pub fn run_seere_v11(
        _prompt: &str,
        _max_turns: usize,
        _allow_tools: bool,
        _bin_mode: bool,
        _plan_mode: bool,
        _allow_exec: bool,
        _allow_external: bool,
    ) -> serde_json::Value {
        serde_json::json!({
            "answer": "ERR:el binario de backup no incluye el motor local SEERE \
                       (usa un proveedor cloud: el gateway LLM de MegaDaemon).",
            "tools": [],
            "tokens": 0,
        })
    }
}

/// Shim de `cockpit_server`: el motor sólo usa `state_dir()`.
pub mod cockpit_server {
    /// Directorio de estado (delega en el módulo canónico).
    pub fn state_dir() -> String {
        crate::state_dir::state_dir()
    }
}

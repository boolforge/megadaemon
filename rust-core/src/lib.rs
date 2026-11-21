//! MegaDaemon v13.1 — Rust Core (biblioteca).
//!
//! Antes esto era un único binario (`main.rs` con todos los `mod` dentro), lo
//! que hacía IMPOSIBLE testear nada: `cargo test` no puede importar un
//! crate binario.  Con esta lib + bin fino, la suite de auditoría
//! (`tests/audit_v13.rs`) puede ejercitar el parser, el fundamento y el
//! cliente HTTP del agente local.

pub mod accelerator_policy;
/// Adquiere un mutex sin dejar que un panic previo lo envenene.
///
/// Por que existe: `lock().unwrap()` convierte cualquier panic ajeno en un
/// fallo PERMANENTE. El mutex queda envenenado y todas las peticiones
/// posteriores vuelven a hacer panic al intentar tomar el lock. En un
/// servidor eso significa: proceso vivo, watchdog contento, y 502 en cada
/// peticion (el tunel ve la conexion cortada a medias). El daemon se
/// quedaba asi, en 502 permanente, sin que nada lo reiniciara.
///
/// Con `lock_ok` el servidor sigue sirviendo: se pierde el estado de ese
/// mutex concreto, no el servicio.
pub fn lock_ok<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub mod adb_client;
pub mod adb_tool;
pub mod agent_core;
pub mod agent_voice;
pub mod aegis;
pub mod alerts;
pub mod android;
pub mod antigravity_intel;
pub mod meta_agent;
pub mod backup_ab;
pub mod api_key;
pub mod apk_deobfuscator;
pub mod apk_inventory;
pub mod app_intel;
pub mod argon2;
pub mod backup_formats;
pub mod backup_vault;
pub mod battery_advanced;
pub mod battery_saver;
pub mod bindiff;
pub mod binwalk_die;
pub mod bookmarks;
pub mod capability;
pub mod cert_vault;
pub mod channel;
pub mod clipboard;
pub mod self_update;
pub mod swift_v1;
pub mod pkgmgr;
pub mod sync_chats;
pub mod recordings;
pub mod antispam;
pub mod otp;
pub mod telecom;
pub mod cfg_lifter;
pub mod cirx_rtl;
pub mod cockpit_doctor;
pub mod cockpit_server;
pub mod coordination;
pub mod crypto_scan;
pub mod delegate;
pub mod deobfuscate;
pub mod device_inventory;
pub mod dex_decompile;
pub mod disasm;
pub mod duckdns;
pub mod dict_sync;
pub mod backup_ab;
pub mod drive_client;
pub mod entropy;
pub mod env_introspect;
pub mod voice_runtime;
pub mod freezer;
pub mod format_oracle;
pub mod frida_dbi;
pub mod git_ops;
pub mod github_connector;
pub mod hw_mapper;
pub mod kaitai_imhex;
pub mod kdeconnect;
pub mod lan;
pub mod magisk_module;
pub mod mcp_ext;
pub mod mcp_server;
pub mod mesh;
pub mod messaging;
pub mod npu_bridge;
pub mod npu_vs4l;
pub mod csc_verify;
pub mod rom_migration;
pub mod npu_cv;
pub mod peers;
pub mod npu_nnapi;
pub mod pattern;
pub mod peers;
pub mod pointers;
pub mod power_policy;
pub mod profile_analysis;
pub mod provider_catalog;
pub mod providers;
pub mod rea_assistant;
pub mod recording_vault;
pub mod repo_updates;
pub mod rom_locator;
pub mod root_hide;
pub mod sqlite_probe;
pub mod root_intel;
pub mod root_stealth;
pub mod seere_v11;
pub mod service_bus;
pub mod service_identity;
pub mod selfsigned_cert;
pub mod shm;
pub mod signing;
pub mod signing_vault;
pub mod snapshot;
pub mod sshd;
pub mod store_manager;
pub mod sync;
pub mod sync_web;
pub mod task_scheduler;
pub mod tile_vram;
pub mod unifier;
pub mod util_fs;
pub mod util_text;
pub mod webui;
pub mod zstd;
pub mod backup_crypto;
pub mod backup_engine;
pub mod backup_full;
pub mod call_recorder;
pub mod lab_kit;
pub mod peers;
pub mod google_auth;
pub mod drive_native;

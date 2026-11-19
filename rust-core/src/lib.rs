//! MegaDaemon v13.1 — Rust Core (biblioteca).
//!
//! Antes esto era un único binario (`main.rs` con todos los `mod` dentro), lo
//! que hacía IMPOSIBLE testear nada: `cargo test` no puede importar un
//! crate binario.  Con esta lib + bin fino, la suite de auditoría
//! (`tests/audit_v13.rs`) puede ejercitar el parser, el fundamento y el
//! cliente HTTP del agente local.

pub mod accelerator_policy;
pub mod bindiff;
pub mod binwalk_die;
pub mod cfg_lifter;
pub mod cirx_rtl;
pub mod cockpit_server;
pub mod crypto_scan;
pub mod deobfuscate;
pub mod disasm;
pub mod entropy;
pub mod freezer;
pub mod frida_dbi;
pub mod hw_mapper;
pub mod kaitai_imhex;
pub mod mcp_server;
pub mod npu_bridge;
pub mod npu_cv;
pub mod npu_nnapi;
pub mod pattern;
pub mod pointers;
pub mod power_policy;
pub mod provider_catalog;
pub mod providers;
pub mod rea_assistant;
pub mod rom_locator;
pub mod seere_v11;
pub mod shm;
pub mod snapshot;
pub mod tile_vram;
pub mod unifier;

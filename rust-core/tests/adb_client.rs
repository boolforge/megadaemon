//! Tests de integración del cliente ADB en Rust puro.
//!
//! El módulo se incluye por ruta para poder testearlo sin tocar `lib.rs`
//! (fichero compartido con otros agentes): cuando el módulo se añada a
//! `lib.rs` (`pub mod adb_client;`) este test puede pasar a usar
//! `megadaemon_core::adb_client` sin cambiar lo que comprueba.

#[path = "../src/adb_client.rs"]
mod adb_client;

//! Arnés de pruebas del módulo `signing`.
//!
//! `src/lib.rs` es intocable en esta tarea, así que el módulo se incluye
//! aquí directamente con `#[path]`.  En cuanto se añada `pub mod signing;`
//! a `src/lib.rs`, este arnés puede borrarse sin tocar los tests.

#[path = "../src/signing.rs"]
mod signing;

// Modulo auxiliar del que depende el modulo incluido por `#[path]` (MEDIDO el
// 2026-11-21: al fusionar `fix/robustness-20261120` varios modulos pasaron a
// usar los helpers canonicos `util_text`/`util_fs` y los arneses
// autocontenidos dejaron de compilar: E0432).
#[path = "../src/util_text.rs"]
#[allow(dead_code, unused_imports, unused_mut)]
mod util_text;

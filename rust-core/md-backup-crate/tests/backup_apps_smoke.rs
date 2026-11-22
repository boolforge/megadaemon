//! Los tests del motor de backup **NO están duplicados**: se compila el
//! MISMOS fichero del crate principal (`rust-core/tests/backup_apps_smoke.rs`).
//!
//! ¿POR QUÉ? MEDIDO 2026-11-22: en un contenedor de 3 GB el crate grande
//! (`megadaemon-core`) no llega a compilar para los tests (el rustc pide
//! ~2,2 GB ⇒ OOM killer). El crate delgado (`md-backup-core`) compila los
//! MISMOS fuentes ⇒ los tests se pueden ejecutar aquí, y en el CI (con 7 GB)
//! se ejecutan también desde el crate principal.

#[path = "../../tests/backup_apps_smoke.rs"]
mod smoke;

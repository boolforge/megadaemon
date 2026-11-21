//! Tests de integración del cliente ADB en Rust puro.
//!
//! El módulo se usa desde la libreria (`use megadaemon_core::adb_client;`).
//! MEDIDO el 2026-11-21: antes se copiaba con `#[path = "../src/adb_client.rs"]`;
//! dentro de un arnés `crate::` es la raíz del ARNÉS, así que el `crate::cockpit_server`
//! del módulo copiado no resolvía (E0433) y **el target de test no compilaba**
//! => la suite entera moría sin ejecutar ni un test.

use megadaemon_core::adb_client;

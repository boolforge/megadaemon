// Arnés TEMPORAL de verificación (se borra al terminar).
//
// El encargo prohíbe tocar lib.rs, así que el módulo `src/mesh.rs` no puede
// registrarse en la librería. Este fichero lo incluye directamente para poder
// ejecutar sus tests unitarios con `cargo test --release mesh`.
#[path = "../src/mesh.rs"]
mod mesh;

// Arnés temporal: incluye el módulo por ruta para poder compilarlo y testearlo
// sin tocar `lib.rs` (el cableado definitivo lo hace el responsable del repo).
#[path = "../src/dex_decompile.rs"]
mod dex_decompile;

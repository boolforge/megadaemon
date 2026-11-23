//! `md-proto-regen` — extrae los descriptores protobuf embebidos en un
//! binario Go y escribe un manifiesto TSV.
//!
//! ```text
//! md-proto-regen <binario> [manifiesto.tsv]
//! ```
//!
//! El binario se lee entero en memoria y se recorren sus segmentos
//! `PT_LOAD` de sólo lectura buscando los `FileDescriptorProto` que
//! `protoc-gen-go` guarda como constantes de cadena en `.rodata`.
//!
//! Ver `rust-core/src/proto_regen.rs` para el algoritmo y las medidas.
//!
//! ⚠️ Este binario **no se ha compilado** en el sandbox donde se escribió
//! (el crate pesa +200 módulos y enlazarlo en 3 GB es un OOM). Lo compila
//! y testea el root.

use megadaemon_core::proto_regen;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("uso: md-proto-regen <binario> [manifiesto.tsv]");
        return ExitCode::from(2);
    }
    let binario = std::path::Path::new(&args[1]);
    let manifiesto = args
        .get(2)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("descriptores.tsv"));

    match proto_regen::escribir_manifiesto(binario, &manifiesto) {
        Ok(n) => {
            println!("descriptores extraidos: {n}");
            println!("manifiesto: {}", manifiesto.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("md-proto-regen: {e}");
            ExitCode::FAILURE
        }
    }
}

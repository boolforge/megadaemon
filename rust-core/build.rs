//! Build script del daemon: incrusta el payload de frida-gum.
//
//! MEDIDO (2026-11-24): el payload era un crate AISLADO ⇒ dos builds y el
//! movil tenia que bajarlo de un Release aparte.  Ahora el job de Android
//! compila el payload PRIMERO y lo pasa aqui por la variable de entorno
//! `MEGADAEMON_PAYLOAD_PATH`; este build script lo copia a `OUT_DIR` y
//! publica `MEGADAEMON_PAYLOAD_BIN` ⇒ `include_bytes!` lo mete en el
//! binario del daemon (un solo artefacto, sin red al instalar).
//!
//! Si la variable no esta (build de desarrollo en local), se crea un
//! fichero VACIO: el binario compila igual y `instalar()` cae al respaldo
//! de descargar el asset del Release.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=MEGADAEMON_PAYLOAD_PATH");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let destino = out.join("hooks_payload.so");
    let fuente = std::env::var("MEGADAEMON_PAYLOAD_PATH").ok().map(PathBuf::from);
    match fuente {
        Some(p) if p.is_file() => {
            // MEDIDO (2026-11-25): el payload crudo son 3,12 MiB (7,6 % del
            // binario del daemon).  Se COMPRIME antes de incrustarlo: el
            // daemon lo descomprime al instalar (el .so que acaba en el
            // movil es EXACTAMENTE el mismo, byte a byte).
            let crudo = std::fs::read(&p).expect("leer el payload");
            let comprimido = zstd::bulk::compress(&crudo, 19).expect("comprimir el payload");
            std::fs::write(&destino, &comprimido).expect("escribir el payload comprimido");
            println!(
                "cargo:warning=payload de frida-gum embebido ({} B crudo -> {} B comprimido, -{}%)",
                crudo.len(),
                comprimido.len(),
                100 - (comprimido.len() * 100 / crudo.len().max(1))
            );
        }
        _ => {
            // Build de desarrollo: payload vacio (el daemon lo bajara del
            // Release cuando haga falta).
            std::fs::write(&destino, b"").expect("crear el payload vacio");
            println!("cargo:warning=sin payload de frida-gum embebido (build de desarrollo)");
        }
    }
    // El nombre lo dice: lo que se incrusta esta COMPRIMIDO (el daemon lo
    // descomprime al instalar).
    println!("cargo:rustc-env=MEGADAEMON_PAYLOAD_BIN={}", destino.display());
}

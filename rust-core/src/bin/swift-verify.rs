//! `swift-verify` — verifica la **cabecera** de un backup de Swift Backup
//! (`.dat` / `.med`) y **prueba la passphrase**.
//!
//! ```text
//! swift-verify --file backup.dat --passphrase "mi clave"
//! swift-verify --file backup.dat --passphrase "mi clave" --pretty
//! ```
//!
//! ⚠️ **Fuente de verdad única**: este binario **no implementa** la
//! verificación; delega en `crate::swift_archive` (el mismo módulo que
//! usan el resto del repo y los tests) ⇒ la CLI y la librería no pueden
//! divergir.
//!
//! Qué hace, exactamente:
//! 1. Detecta el magic `SBA1` y **elige la variante de cabecera** (96 B ó
//!    144 B) leyendo el campo `formato`.
//! 2. Parsea la cabecera con el layout **medido** en el escritor nativo
//!    ARM64 de `libsba_archive.so`.
//! 3. **Deriva la clave** con el Argon2id **verificado del repo**
//!    (`crate::argon2::derive_key`: una sola implementación del KDF).
//! 4. **Verifica el key-check**: `SHA-256(etiqueta || salt || nonce2 ||
//!    clave)[:16]`. Como el orden de los tres bloques de la cabecera no
//!    está cerrado en el binario nativo, prueba las 6 permutaciones: la
//!    que cuadra **demuestra que la passphrase es correcta** y además
//!    **revela el orden real** de los bloques.
//!
//! Salida: **JSON por stdout**. `status`:
//! · `ok` ⇒ la passphrase es correcta y la cabecera no está manipulada;
//! · `key_check_mismatch` ⇒ passphrase incorrecta o fichero manipulado;
//! · `kdf_error` ⇒ parámetros del KDF fuera de los que acepta la app;
//! · `layout_no_verificado` ⇒ el fichero no encaja con el layout medido.
//!
//! ⚠️ El **contenido** (AEAD por trozos) **no** se extrae: la derivación del
//! nonce/AAD por trozo es **nativa** y no está replicada. Este binario
//! verifica la cabecera, no descifra el payload.

use megadaemon_core::swift_archive::{
    key_check_tag, sba1_detectar, verificar_y_desempaquetar, KdfParams,
};
use serde_json::json;
use std::fs;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        if let Some(v) = args[i].strip_prefix(&format!("{}=", name)) {
            return Some(v.to_string());
        }
        i += 1;
    }
    None
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn usage() -> ! {
    eprintln!(
        "uso: swift-verify --file <ruta> [--passphrase <texto>] [--iters N] \
         [--mem KIB] [--par N] [--outlen N] [--method <cifrado>] [--pretty]"
    );
    eprintln!(
        "\n--method: sevenzip_aes | aes256_gcm | aes256_gcm_siv | aegis256 | aegis128x2\n\
         (el default es aegis256; la etiqueta de dominio del key-check depende de el)"
    );
    eprintln!(
        "\n--dump <ruta>: vuelca a disco el CONTENIDO DESCIFRADO (si el AEAD por\n\
         trozos se pudo replicar y el encuadre se pudo validar con el MAC del indice)"
    );
    std::process::exit(2);
}

fn main() {
    let path = match arg("--file") {
        Some(p) => p,
        None => usage(),
    };
    let passphrase = arg("--passphrase").unwrap_or_default();
    let params = KdfParams {
        iterations: arg("--iters").and_then(|v| v.parse().ok()).unwrap_or(3),
        memory_kib: arg("--mem").and_then(|v| v.parse().ok()).unwrap_or(16384),
        parallelism: arg("--par").and_then(|v| v.parse().ok()).unwrap_or(4),
        out_len: arg("--outlen").and_then(|v| v.parse().ok()).unwrap_or(32),
    };
    let metodo = arg("--method").unwrap_or_else(|| "aegis256".to_string());
    let pretty = flag("--pretty");

    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("no se pudo leer {}: {}", path, e);
            std::process::exit(1);
        }
    };

    let layout = match sba1_detectar(&bytes) {
        Some(mut l) => {
            // La etiqueta de dominio depende del metodo de cifrado.
            if let (Some(spec), Some(tag)) = (l.key_check.as_mut(), key_check_tag(&metodo)) {
                spec.tag = tag.to_string();
            }
            l
        }
        None => {
            eprintln!(
                "{} no parece un contenedor Swift Backup (sin magic SBA1 en los 4 primeros bytes)",
                path
            );
            std::process::exit(1);
        }
    };
    let report = verificar_y_desempaquetar(&bytes, &layout, &passphrase, params, &metodo);

    // Volcar el contenido descifrado, si se pudo.
    let mut volcado: Option<String> = None;
    if let Some(destino) = arg("--dump") {
        match report
            .descifrado
            .as_ref()
            .and_then(|d| d.contenido.as_ref())
        {
            Some(contenido) => match fs::write(&destino, contenido) {
                Ok(()) => volcado = Some(format!(
                    "{} bytes descifrados -> {}",
                    contenido.len(),
                    destino
                )),
                Err(e) => volcado = Some(format!("no se pudo escribir {}: {}", destino, e)),
            },
            None => volcado = Some(
                "no se pudo descifrar el contenido (mira \"notas\" y \"descifrado\")".into(),
            ),
        }
    }
    let descifrado = report.descifrado.as_ref().map(|d| {
        json!({
            "status": d.status,
            "geometria": d.geometria.map(|g| json!({
                "header_size": g.header_size,
                "payload_off": g.payload_off,
                "payload_len": g.payload_len,
                "index_off": g.index_off,
                "index_len": g.index_len,
                "mac_off": g.mac_off,
                "mac_len": g.mac_len,
            })),
            "nonce": d.nonce,
            "aad": d.aad,
            "bytes_descifrados": d.contenido.as_ref().map(|c| c.len()),
            "intentos": d.intentos,
        })
    });
    let out = json!({
        "fichero": path,
        "metodo_cifrado": metodo,
        "status": report.status,
        "kdf": report.kdf,
        "key_check": report.key_check,
        "payload_hmac_key": report.payload_hmac_key,
        "descifrado": descifrado,
        "volcado": volcado,
        "notas": report.notes,
    });
    let txt = if pretty {
        serde_json::to_string_pretty(&out).unwrap_or_default()
    } else {
        serde_json::to_string(&out).unwrap_or_default()
    };
    println!("{}", txt);
    std::process::exit(if report.status == "ok" { 0 } else { 1 });
}

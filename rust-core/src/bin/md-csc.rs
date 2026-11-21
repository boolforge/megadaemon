//! `megadaemon-core csc` — parche del CSC **en Rust**, sin Python.
//!
//!     megadaemon-core csc list                       # descubre TODAS las features
//!     megadaemon-core csc set RecordingAllowed        # parchea la de grabación
//!     megadaemon-core csc set <valor> --csc TPA
//!     megadaemon-core csc set <valor> --feature CscFeature_Otra
//!     megadaemon-core csc set <valor> --json
//!
//! Escribe en el módulo Magisk `/data/adb/modules/megadaemon_csc_recording`
//! (bind-mount systemless: el fichero original del CSC NO se toca; revertir
//! es desactivar el módulo).  El `contents.db` se actualiza para que el
//! `fileSize` cuadre con el fichero servido.

use std::path::PathBuf;

use megadaemon_core::csc_patch::{
    active_csc, carriers, list_features, patch, read_csc_xml, xml_candidates, FEATURE_RECORDING,
    MODULE_DIR,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        uso();
        std::process::exit(2);
    }
    let mut csc: Option<String> = None;
    let mut feature = FEATURE_RECORDING.to_string();
    let mut json = false;
    let mut valor: Option<String> = None;
    let mut modo = args[0].clone();

    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if let Some(v) = a.strip_prefix("--csc=") {
            csc = Some(v.to_string());
        } else if let Some(v) = a.strip_prefix("--feature=") {
            feature = v.to_string();
        } else if a == "--json" {
            json = true;
        } else if a == "--help" || a == "-h" {
            uso();
            return;
        } else if valor.is_none() && !a.starts_with("--") {
            valor = Some(a.clone());
        }
        i += 1;
    }
    if modo == "list" && valor.is_some() {
        modo = "set".to_string();
    }

    let csc = csc.or_else(active_csc).unwrap_or_else(|| "AMO".to_string());

    match modo.as_str() {
        "list" => {
            match read_csc_xml(&csc) {
                Ok((xml, fuente)) => {
                    let feats = list_features(&xml);
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({
                                "csc": csc,
                                "fichero": fuente.display().to_string(),
                                "features": feats,
                            })
                        );
                    } else {
                        println!("CSC: {csc} | fichero: {}", fuente.display());
                        println!("features CscFeature_*: {}", feats.len());
                        for f in &feats {
                            println!("  {:58} = {}", f.name, f.value);
                        }
                    }
                    let cs = carriers();
                    if !cs.is_empty() {
                        println!("carriers presentes: {}", cs.join(", "));
                    }
                }
                Err(e) => {
                    eprintln!("ERROR: {e}");
                    std::process::exit(1);
                }
            }
        }
        "set" => {
            let Some(valor) = valor else {
                eprintln!("ERROR: falta el valor.  uso: megadaemon-core csc set <valor>");
                std::process::exit(2);
            };
            let modulo = PathBuf::from(MODULE_DIR);
            match patch(&csc, &feature, &valor, &modulo) {
                Ok(r) => {
                    if json {
                        println!("{}", serde_json::to_string(&r).unwrap_or_default());
                    } else {
                        println!("CSC: {}", r.csc);
                        println!("feature: {}", r.feature);
                        println!(
                            "valor: {} -> {}",
                            r.valor_anterior.as_deref().unwrap_or("AUSENTE"),
                            r.valor_nuevo
                        );
                        println!("módulo: {} ({} B)", r.modulo, r.bytes_escritos);
                        println!(
                            "contents.db: {} fila(s) actualizada(s)",
                            r.contents_db_actualizado
                        );
                        println!("origen: {}", r.ruta_original);
                        println!("REINICIA el móvil para que el framework lea el fichero nuevo.");
                    }
                }
                Err(e) => {
                    eprintln!("ERROR: {e}");
                    std::process::exit(1);
                }
            }
        }
        "matrix" => {
            // COMPARA TODOS los CSC del firmware: que features trae cada uno
            let rows = megadaemon_core::csc_patch::matrix();
            if json {
                println!("{}", serde_json::to_string(&rows).unwrap_or_default());
            } else {
                println!("{:<8} {:>8}  {:<44} {}", "CSC", "features", "grabacion", "legible");
                for r in &rows {
                    println!(
                        "{:<8} {:>8}  {:<44} {}",
                        r.csc,
                        r.num_features,
                        r.recording.as_deref().unwrap_or("-"),
                        if r.legible { "si" } else { "no" }
                    );
                }
            }
        }
        "inventory" => {
            match megadaemon_core::csc_patch::inventory(&csc) {
                Ok(rows) => {
                    if json {
                        println!("{}", serde_json::to_string(&rows).unwrap_or_default());
                    } else {
                        println!("inventario del CSC {csc}: {} ficheros", rows.len());
                        for r in &rows {
                            println!("  {:<70} {}", r.file_path, r.file_size);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("ERROR: {e}");
                    std::process::exit(1);
                }
            }
        }
        "rutas" => {
            for p in xml_candidates(&csc) {
                println!("{}", p.display());
            }
        }
        _ => {
            uso();
            std::process::exit(2);
        }
    }
}

fn uso() {
    eprintln!(
        "megadaemon-core csc <list|set|matrix|inventory|rutas> [valor] [--csc=ID] [--feature=NOMBRE] [--json]\n\
         \n\
         list   decodifica el cscfeature.xml del CSC y lista TODAS las CscFeature_*\n\
         set    parchea una feature con el valor pedido (cualquier valor del esquema)\n\
         matrix compara TODOS los CSC del firmware (features y valores de cada uno)\n\
         inventory lista TODOS los ficheros del CSC (que se puede parchear)\n\
         rutas  muestra las rutas candidatas del cscfeature.xml en el movil\n\
         \n\
         modulo Magisk: {MODULE_DIR}"
    );
}

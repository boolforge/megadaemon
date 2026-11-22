//! `md-re` — la CLI del motor de RE de MegaDaemon.
//!
//! Subcomandos:
//!   identify | analyze | strings   — análisis ligero del binario
//!   deobfuscate                     — MBA + CFF + crypto + predicados opacos
//!   mba <expr>                      — simplificación de expresiones MBA
//!   compare                         — comparación de métodos (multi-motor)
//!   fusion                          — reconstrucción fusionada (multi-motor)
//!
//! `compare` y `fusion` orquestan los motores que YA viven en el repo
//! (Ghidra, Rizin, DiE/Binwalk, motor propio y el pclntab) y comparan sus
//! resultados; `fusion` junta lo mejor de cada uno.

use megadaemon_core::re_fusion::{self, OpcionesFusion};
use serde_json::json;
use std::path::{Path, PathBuf};

fn ayuda() {
    eprintln!(
        "uso:
  md-re identify <binario> [--n N]
  md-re analyze  <binario> [--n N]
  md-re strings  <binario> [--n N]
  md-re deobfuscate <binario>
  md-re mba <expresion>
  md-re compare <binario> [--symbols TSV] [--anchors TSV] [--out DIR]
  md-re fusion  <binario> [--symbols TSV] [--anchors TSV] [--out DIR]
               [--project-dir DIR] [--project NAME] [--ghidra-dir DIR]
               [--java-home DIR] [--no-ghidra] [--no-rizin] [--no-die]"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        ayuda();
        return;
    }
    let sub = args[0].clone();
    let mut ruta = String::new();
    let mut max = 60usize;
    let mut simbolos: Option<PathBuf> = None;
    let mut anclas: Option<PathBuf> = None;
    let mut out = PathBuf::from("/workspace/agy-re/tools-out/fusion");
    let mut project_dir = PathBuf::from("/workspace/agy-re/tools-out/proj");
    let mut project = "Go".to_string();
    let mut ghidra_dir: Option<PathBuf> = None;
    let mut java_home: Option<PathBuf> = None;
    let mut con_ghidra = true;
    let mut con_rizin = true;
    let mut con_die = true;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--n" => {
                i += 1;
                if i < args.len() {
                    max = args[i].parse().unwrap_or(60);
                }
            }
            "--symbols" => {
                i += 1;
                if i < args.len() {
                    simbolos = Some(PathBuf::from(&args[i]));
                }
            }
            "--anchors" => {
                i += 1;
                if i < args.len() {
                    anclas = Some(PathBuf::from(&args[i]));
                }
            }
            "--out" => {
                i += 1;
                if i < args.len() {
                    out = PathBuf::from(&args[i]);
                }
            }
            "--project-dir" => {
                i += 1;
                if i < args.len() {
                    project_dir = PathBuf::from(&args[i]);
                }
            }
            "--project" => {
                i += 1;
                if i < args.len() {
                    project = args[i].clone();
                }
            }
            "--ghidra-dir" => {
                i += 1;
                if i < args.len() {
                    ghidra_dir = Some(PathBuf::from(&args[i]));
                }
            }
            "--java-home" => {
                i += 1;
                if i < args.len() {
                    java_home = Some(PathBuf::from(&args[i]));
                }
            }
            "--no-ghidra" => con_ghidra = false,
            "--no-rizin" => con_rizin = false,
            "--no-die" => con_die = false,
            a => {
                if ruta.is_empty() {
                    ruta = a.to_string();
                }
            }
        }
        i += 1;
    }

    let v: serde_json::Value = match sub.as_str() {
        "identify" | "analyze" | "strings" => {
            if ruta.is_empty() {
                eprintln!("falta el fichero");
                std::process::exit(2);
            }
            let p = Path::new(&ruta);
            match sub.as_str() {
                "identify" => match megadaemon_core::re_engine::identificar(p) {
                    Ok(id) => serde_json::to_value(id).unwrap_or_default(),
                    Err(e) => json!({ "ok": false, "error": e }),
                },
                "strings" => match std::fs::read(p) {
                    Ok(d) => json!({
                        "ok": true,
                        "cadenas": megadaemon_core::re_engine::cadenas_interesantes(&d, max),
                    }),
                    Err(e) => json!({ "ok": false, "error": format!("leer: {e}") }),
                },
                _ => {
                    let mut o = megadaemon_core::re_engine::OpcionesAnalisis::default();
                    o.max_cadenas = max;
                    match megadaemon_core::re_engine::analizar(p, &o) {
                        Ok(v) => v,
                        Err(e) => json!({ "ok": false, "error": e }),
                    }
                }
            }
        }
        "deobfuscate" => {
            if ruta.is_empty() {
                eprintln!("falta el fichero");
                std::process::exit(2);
            }
            let p = Path::new(&ruta);
            match megadaemon_core::re_engine::deobfuscar(
                p,
                &megadaemon_core::re_engine::OpcionesDeofuscacion::default(),
            ) {
                Ok(v) => v,
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }
        "mba" => {
            let expr = if ruta.is_empty() {
                args.get(1).cloned().unwrap_or_default()
            } else {
                ruta.clone()
            };
            megadaemon_core::re_engine::mba(&expr)
        }
        "compare" | "fusion" => {
            if ruta.is_empty() {
                eprintln!("falta el binario");
                std::process::exit(2);
            }
            let o = OpcionesFusion {
                binario: PathBuf::from(&ruta),
                simbolos_tsv: simbolos,
                anchors_tsv: anclas,
                ghidra_dir,
                java_home,
                project_dir,
                project_name: project,
                out_dir: out,
                max_memory: "1400M".to_string(),
                timeout: std::time::Duration::from_secs(1800),
                hacer_motor: true,
                hacer_die: con_die,
                hacer_rizin: con_rizin,
                hacer_ghidra: con_ghidra && sub == "fusion",
            };
            re_fusion::ejecutar(&o)
        }
        _ => {
            ayuda();
            return;
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".to_string())
    );
}

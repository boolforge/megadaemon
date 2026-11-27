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
  md-re decompile <binario> --anchors TSV --out DIR [--backend ghidra|rizin-ghidra|both]
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
    let mut backend = String::from("rizin-ghidra");
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
            "--backend" => {
                i += 1;
                if i < args.len() {
                    backend = args[i].clone();
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
        "decompile" => {
            if ruta.is_empty() {
                eprintln!("falta el binario");
                std::process::exit(2);
            }
            let bin = PathBuf::from(&ruta);
            let anchors_path = match &anclas {
                Some(p) => p.clone(),
                None => {
                    eprintln!("falta --anchors (TSV de anclas)");
                    std::process::exit(2);
                }
            };
            let anchors = match megadaemon_core::ghidra::parse_anchors_tsv(&anchors_path) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("anchors: {e}");
                    std::process::exit(2);
                }
            };
            match backend.as_str() {
                "rizin-ghidra" => {
                    let cfg = megadaemon_core::re_decompilers::RizinGhidraConfig::from_env();
                    match megadaemon_core::re_decompilers::decompile_anchors_rizin(
                        &bin,
                        &cfg,
                        &anchors,
                        &out,
                    ) {
                        Ok(o) => json!({
                            "ok": o.report.ok,
                            "motor": o.report.stage,
                            "decompiladas": o.report.functions_decompiled,
                            "segundos": o.report.elapsed_secs,
                            "ficheros": o.files.len(),
                            "informe": o.report,
                        }),
                        Err(e) => json!({ "ok": false, "error": e.to_string() }),
                    }
                }
                "ghidra" => {
                    let mut cfg = megadaemon_core::ghidra::GhidraConfig::default();
                    cfg.ghidra_dir = ghidra_dir.clone();
                    cfg.java_home = java_home.clone();
                    cfg.project_dir = project_dir.clone();
                    cfg.project_name = project.clone();
                    cfg.max_memory = "1400M".to_string();
                    match megadaemon_core::ghidra::GhidraTool::detect(&cfg) {
                        Err(e) => json!({ "ok": false, "error": e.to_string() }),
                        Ok(tool) => {
                            match megadaemon_core::ghidra::decompile_anchors(
                                &tool,
                                &cfg,
                                &anchors_path,
                                &out,
                                "",
                            ) {
                                Ok(o) => json!({
                                    "ok": o.report.ok,
                                    "motor": o.report.stage,
                                    "decompiladas": o.report.functions_decompiled,
                                    "segundos": o.report.elapsed_secs,
                                    "ficheros": o.files.len(),
                                    "informe": o.report,
                                }),
                                Err(e) => json!({ "ok": false, "error": e.to_string() }),
                            }
                        }
                    }
                }
                "both" => {
                    // los dos motores sobre las mismas anclas y luego comparados
                    let mut cfg = megadaemon_core::ghidra::GhidraConfig::default();
                    cfg.ghidra_dir = ghidra_dir.clone();
                    cfg.java_home = java_home.clone();
                    cfg.project_dir = project_dir.clone();
                    cfg.project_name = project.clone();
                    cfg.max_memory = "1400M".to_string();
                    let out_r2 = out.join("rizin-ghidra");
                    let out_gh = out.join("ghidra");
                    let r2 = megadaemon_core::re_decompilers::decompile_anchors_rizin(
                        &bin,
                        &megadaemon_core::re_decompilers::RizinGhidraConfig::from_env(),
                        &anchors,
                        &out_r2,
                    );
                    let gh = match megadaemon_core::ghidra::GhidraTool::detect(&cfg) {
                        Err(e) => Err(e),
                        Ok(tool) => megadaemon_core::ghidra::decompile_anchors(
                            &tool, &cfg, &anchors_path, &out_gh, "",
                        )
                        .map(|o| o.report),
                    };
                    let (filas, resumen) = megadaemon_core::re_decompilers::comparar_con_pseudocodigo(
                        &out_gh,
                        &out_r2,
                        "ghidra",
                        "rizin-ghidra",
                    );
                    json!({
                        "ok": true,
                        "rizin_ghidra": r2.map(|o| o.report).map_err(|e| e.to_string()),
                        "ghidra": gh.map_err(|e| e.to_string()),
                        "comparacion": resumen,
                        "muestras": filas.len(),
                    })
                }
                otro => json!({ "ok": false, "error": format!("backend desconocido: {otro}") }),
            }
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

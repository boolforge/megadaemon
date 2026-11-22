//! `md-ghidra` — soporte de Ghidra headless de MegaDaemon.
//!
//!     md-ghidra doctor                        # ¿hay Ghidra y un JDK utilizable?
//!     md-ghidra import   <binario>            # importa el binario al proyecto
//!     md-ghidra symbols  <symbols.tsv>        # aplica el export de unstrip
//!     md-ghidra decompile <anchors.tsv>       # vuelca el pseudocódigo
//!     md-ghidra info                         # inventario del programa
//!     md-ghidra find     <symbols.tsv> <nombre>
//!
//! Opciones comunes:
//!     --ghidra-dir=DIR    instalación de Ghidra (si no, se autodetecta)
//!     --java-home=DIR     JDK a usar (si no, se autodetecta)
//!     --project-dir=DIR   directorio de proyectos
//!     --project=NAME      nombre del proyecto
//!     --timeout=SECS      timeout por fase (por defecto 1800)
//!     --queue-wait=SECS   espera máxima en la cola global (por defecto 60)
//!     --max-memory=2G     heap de la JVM
//!     --script-dir=DIR    donde se escribe el GhidraScript embebido
//!     --out=DIR           directorio de salida
//!     --filter=NAME       subcadena del nombre de la función a decompilar
//!     --json              salida en JSON
//!
//! Si no hay Java el comando **falla con un mensaje claro** (código 3), nunca
//! con un panic.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use megadaemon_core::ghidra::{
    // MEDIDO 2026-11-22: faltaba `apply_symbols` en la lista de importacion
    // => el subcomando `symbols` no compilaba (E0425).
    apply_symbols, decompile_anchors, find_symbol, import_binary, parse_anchors_tsv,
    parse_symbols_tsv, program_info, GhidraConfig, GhidraTool,
};

fn uso() {
    println!(
        "md-ghidra — soporte de Ghidra headless (MegaDaemon)\n\
         \n\
         USOS:\n  \
         md-ghidra doctor                       ¿hay Ghidra y JDK?\n  \
         md-ghidra import   <binario>           importa al proyecto Ghidra\n  \
         md-ghidra symbols  <symbols.tsv>       aplica el export de unstrip\n  \
         md-ghidra decompile <anchors.tsv>      decompila y vuelca el pseudocódigo\n  \
         md-ghidra info                        inventario del programa\n  \
         md-ghidra find     <symbols.tsv> <n>   dirección de una función\n\
         \n\
         OPCIONES: --ghidra-dir --java-home --project-dir --project --timeout\n\
         \x20         --queue-wait --max-memory --script-dir --out --filter --json"
    );
}

struct Opts {
    cfg: GhidraConfig,
    out: Option<PathBuf>,
    filter: String,
    json: bool,
    positional: Vec<String>,
}

fn parse_opts(args: &[String]) -> Opts {
    let mut cfg = GhidraConfig::default();
    let mut out = None;
    let mut filter = String::new();
    let mut json = false;
    let mut positional = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let mut take = |name: &str| -> Option<String> {
            if let Some(v) = a.strip_prefix(&format!("{}=", name)) {
                return Some(v.to_string());
            }
            if a == name {
                i += 1;
                return args.get(i).cloned();
            }
            None
        };
        if let Some(v) = take("--ghidra-dir") {
            cfg.ghidra_dir = Some(PathBuf::from(v));
        } else if let Some(v) = take("--java-home") {
            cfg.java_home = Some(PathBuf::from(v));
        } else if let Some(v) = take("--project-dir") {
            cfg.project_dir = PathBuf::from(v);
        } else if let Some(v) = take("--project") {
            cfg.project_name = v;
        } else if let Some(v) = take("--timeout") {
            if let Ok(s) = v.parse::<u64>() {
                cfg.timeout = Duration::from_secs(s);
            }
        } else if let Some(v) = take("--queue-wait") {
            if let Ok(s) = v.parse::<u64>() {
                cfg.queue_wait = Duration::from_secs(s);
            }
        } else if let Some(v) = take("--max-memory") {
            cfg.max_memory = v;
        } else if let Some(v) = take("--script-dir") {
            cfg.script_dir = PathBuf::from(v);
        } else if let Some(v) = take("--out") {
            out = Some(PathBuf::from(v));
        } else if let Some(v) = take("--filter") {
            filter = v;
        } else if a == "--json" {
            json = true;
        } else if a == "--help" || a == "-h" {
            uso();
            std::process::exit(0);
        } else if !a.starts_with("--") {
            positional.push(a.clone());
        }
        i += 1;
    }

    Opts {
        cfg,
        out,
        filter,
        json,
        positional,
    }
}

fn die(msg: &str) -> ExitCode {
    eprintln!("md-ghidra: {}", msg);
    ExitCode::from(3)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        uso();
        return ExitCode::from(2);
    }
    let modo = args[0].clone();
    let o = parse_opts(&args[1..]);

    // `doctor` no necesita que la herramienta esté presente.
    if modo == "doctor" {
        let d = match GhidraTool::detect(&o.cfg) {
            Ok(t) => t,
            Err(e) => {
                if o.json {
                    println!(
                        "{}",
                        serde_json::json!({ "ok": false, "error": e.to_string() })
                    );
                } else {
                    println!("md-ghidra: {}", e);
                }
                return ExitCode::from(3);
            }
        };
        if o.json {
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "analyze_headless": d.analyze_headless.display().to_string(),
                    "ghidra_dir": d.ghidra_dir.as_ref().map(|p| p.display().to_string()),
                    "java_home": d.java_home.as_ref().map(|p| p.display().to_string()),
                    "java_bin": d.java_bin.as_ref().map(|p| p.display().to_string()),
                    "has_jdk": d.has_jdk,
                    "project_dir": o.cfg.project_dir.display().to_string(),
                    "project": o.cfg.project_name,
                })
            );
        } else {
            println!("analyzeHeadless : {}", d.analyze_headless.display());
            println!(
                "ghidra dir      : {}",
                d.ghidra_dir
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(inferido)".to_string())
            );
            println!(
                "JAVA_HOME       : {}",
                d.java_home
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(ninguno)".to_string())
            );
            println!(
                "java            : {} (JDK: {})",
                d.java_bin
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(ninguno)".to_string()),
                if d.has_jdk { "sí" } else { "NO (falta javac)" }
            );
            println!("proyecto        : {}/{}", o.cfg.project_dir.display(), o.cfg.project_name);
        }
        return ExitCode::SUCCESS;
    }

    let tool = match GhidraTool::detect(&o.cfg) {
        Ok(t) => t,
        Err(e) => {
            if o.json {
                println!(
                    "{}",
                    serde_json::json!({ "ok": false, "error": e.to_string() })
                );
            }
            return die(&e.to_string());
        }
    };
    if !tool.has_jdk {
        let msg = format!(
            "el Java detectado ({}) no trae `javac`: Ghidra no puede compilar el \
             GhidraScript de Java. Instala un JDK completo (openjdk-21-jdk-headless).",
            tool.java_home
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "?".to_string())
        );
        if o.json {
            println!("{}", serde_json::json!({ "ok": false, "error": msg }));
        }
        return die(&msg);
    }

    let res = match modo.as_str() {
        "import" => {
            let bin = match o.positional.first() {
                Some(b) => PathBuf::from(b),
                None => {
                    return die("falta el binario a importar (md-ghidra import <binario>)");
                }
            };
            import_binary(&tool, &o.cfg, &bin).map(|r| serde_json::json!({
                "ok": r.ok, "stage": r.stage, "exit_code": r.exit_code,
                "elapsed_secs": r.elapsed_secs, "message": r.message,
            }))
        }
        "symbols" => {
            let tsv = match o.positional.first() {
                Some(t) => PathBuf::from(t),
                None => return die("falta el TSV de símbolos (md-ghidra symbols <symbols.tsv>)"),
            };
            apply_symbols(&tool, &o.cfg, &tsv).map(|r| serde_json::json!({
                "ok": r.ok, "stage": r.stage, "exit_code": r.exit_code,
                "elapsed_secs": r.elapsed_secs, "message": r.message,
                "symbols_applied": r.symbols_applied,
            }))
        }
        "decompile" => {
            let tsv = match o.positional.first() {
                Some(t) => PathBuf::from(t),
                None => return die("falta el TSV de anclas (md-ghidra decompile <anchors.tsv>)"),
            };
            let out = o
                .out
                .clone()
                .unwrap_or_else(|| PathBuf::from("/workspace/agy-re/tools-out/decomp"));
            decompile_anchors(&tool, &o.cfg, &tsv, &out, &o.filter).map(|d| {
                serde_json::json!({
                    "ok": d.report.ok, "stage": d.report.stage,
                    "exit_code": d.report.exit_code,
                    "elapsed_secs": d.report.elapsed_secs,
                    "message": d.report.message,
                    "functions_decompiled": d.report.functions_decompiled,
                    "outputs": d.files,
                    "rows": d.rows,
                })
            })
        }
        "info" => {
            let out = o
                .out
                .clone()
                .unwrap_or_else(|| PathBuf::from("/workspace/agy-re/tools-out/decomp"));
            program_info(&tool, &o.cfg, &out).map(|r| serde_json::json!({
                "ok": r.ok, "stage": r.stage, "exit_code": r.exit_code,
                "elapsed_secs": r.elapsed_secs, "message": r.message,
                "outputs": r.outputs,
            }))
        }
        "find" => {
            let tsv = match o.positional.first() {
                Some(t) => PathBuf::from(t),
                None => return die("falta el TSV de símbolos"),
            };
            let needle = match o.positional.get(1) {
                Some(n) => n.clone(),
                None => return die("falta el nombre de la función"),
            };
            match parse_symbols_tsv(&tsv) {
                Ok(syms) => match find_symbol(&syms, &needle) {
                    Some(s) => Ok(serde_json::json!({
                        "ok": true, "addr": format!("0x{:x}", s.addr),
                        "name": s.name, "comment": s.comment,
                    })),
                    None => Ok(serde_json::json!({
                        "ok": false, "error": format!("ninguna función casa con '{}'", needle),
                    })),
                },
                Err(e) => Err(e),
            }
        }
        "anchors" => {
            let tsv = match o.positional.first() {
                Some(t) => PathBuf::from(t),
                None => return die("falta el TSV de anclas"),
            };
            parse_anchors_tsv(&tsv).map(|a| {
                serde_json::json!({ "ok": true, "count": a.len(), "anchors": a })
            })
        }
        other => {
            uso();
            return ExitCode::from(2);
        }
    };

    match res {
        Ok(v) => {
            if o.json {
                println!("{}", v);
            } else {
                let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
                let msg = v
                    .get("message")
                    .and_then(|x| x.as_str())
                    .unwrap_or("(sin mensaje)");
                println!(
                    "{} — {}",
                    if ok { "OK" } else { "FALLO" },
                    msg
                );
                if let Some(files) = v.get("outputs").and_then(|x| x.as_array()) {
                    for f in files {
                        if let Some(s) = f.as_str() {
                            println!("  -> {}", s);
                        }
                    }
                }
            }
            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(4)
            }
        }
        Err(e) => {
            if o.json {
                println!("{}", serde_json::json!({ "ok": false, "error": e.to_string() }));
            }
            die(&e.to_string())
        }
    }
}

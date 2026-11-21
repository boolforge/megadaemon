//! `megadaemon-lab` — el mega-binario de MegaDaemon.
//!
//! Un solo binario para lo que hasta ahora eran 6-8 apps del móvil:
//!
//! ```text
//! megadaemon-lab lab <apk|dex>              Informe de laboratorio (desofuscación)
//! megadaemon-lab verdict <inventory.json>    Veredicto KEEP/REIMPLEMENTAR/DESINSTALAR
//! megadaemon-lab recordings <dir>            Catálogo + plan de transcripción/cifrado/subida
//! megadaemon-lab backup <file>               Catálogo de un backup (AB / tar)
//! ```
//!
//! Todo lo que imprime procede de leer ficheros: no inventa nada.  Los
//! secretos se muestran **redactados** y el gestor de grabaciones **no**
//! graba: gestiona las que ya existen (nota legal en `lab_kit`).

use std::process::ExitCode;

fn usage() -> &'static str {
    "uso:\n  \
     megadaemon-lab lab <apk|dex>\n  \
     megadaemon-lab verdict <inventory.json>\n  \
     megadaemon-lab recordings <dir> [--engine <bin>] [--vault <ruta>] [--target <destino>]\n  \
     megadaemon-lab backup <fichero>\n"
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprint!("{}", usage());
        return ExitCode::from(2);
    }
    let cmd = args[1].as_str();
    let input = args[2].clone();
    let opt = |name: &str| -> Option<String> {
        let mut i = 3;
        while i + 1 < args.len() {
            if args[i] == name {
                return Some(args[i + 1].clone());
            }
            i += 2;
        }
        None
    };

    match cmd {
        "lab" => {
            match megadaemon_core::lab_kit::analyze_apk(&input) {
                Ok(rep) => {
                    print!("{}", megadaemon_core::lab_kit::lab_report_markdown(&rep));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    ExitCode::FAILURE
                }
            }
        }
        "verdict" => {
            let text = match std::fs::read_to_string(&input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("error: no se pudo leer {}: {}", input, e);
                    return ExitCode::FAILURE;
                }
            };
            let inv: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("error: JSON inválido: {}", e);
                    return ExitCode::FAILURE;
                }
            };
            let rows = megadaemon_core::lab_kit::verdict_table(&inv);
            let counts = megadaemon_core::lab_kit::verdict_counts(&rows);
            for (k, v) in &counts {
                println!("{:<24} {}", k, v);
            }
            println!();
            for (pkg, facts, verdict) in &rows {
                let reasons = facts.reasons();
                if !reasons.is_empty() {
                    println!("{} [{}] {}", pkg, verdict.as_str(), reasons.join("; "));
                }
            }
            ExitCode::SUCCESS
        }
        "recordings" => {
            let cat = megadaemon_core::lab_kit::scan_recordings(&input);
            println!("raiz: {}", cat.root);
            println!("grabaciones: {} · total {} bytes", cat.items.len(), cat.total_bytes);
            for i in &cat.items {
                println!(
                    "  [{}] {} bytes sha256={} {}",
                    i.kind, i.bytes, &i.sha256[..i.sha256.len().min(16)], i.path
                );
            }
            let engine = opt("--engine").unwrap_or_else(|| "whisper".to_string());
            let vault = opt("--vault")
                .unwrap_or_else(|| "/data/local/tmp/megadaemon/state/signing_vault".to_string());
            let target = opt("--target").unwrap_or_else(|| "drive".to_string());
            let t = megadaemon_core::lab_kit::plan_transcription(&cat, &engine);
            let e = megadaemon_core::lab_kit::plan_encryption(&cat, &vault);
            let u = megadaemon_core::lab_kit::plan_upload(&cat, &target);
            println!("\nplan: {} transcripciones, {} cifrados, {} subidas", t.len(), e.len(), u.len());
            for j in &t {
                println!("  transcribir {} -> {} (motor {})", j.input, j.output, j.engine);
            }
            for j in &e {
                println!("  cifrar {} -> {} (vault {})", j.input, j.output, j.vault);
            }
            for j in &u {
                println!("  subir {} -> {}", j.input, j.target);
            }
            ExitCode::SUCCESS
        }
        "backup" => match megadaemon_core::lab_kit::catalog_backup(&input) {
            Ok(c) => {
                println!("{} · {}", c.path, c.format);
                for e in &c.entries {
                    println!("  [{}] {} bytes {}", e.kind, e.size, e.path);
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {}", e);
                ExitCode::FAILURE
            }
        },
        _ => {
            eprint!("{}", usage());
            ExitCode::from(2)
        }
    }
}

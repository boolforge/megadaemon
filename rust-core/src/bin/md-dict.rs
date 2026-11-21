//! `md-dict` — diccionarios del usuario: inspección, sincronización y corrección.
//!
//! Uso:
//!     md-dict scan --file <volcado>       # parsea un volcado del diccionario
//!     md-dict correct --dict <fichero> --text "texto" [--apply] [--umbral 0.9]
//!     md-dict merge --a <f1> --b <f2> [--json]
//!
//! El volcado del móvil se obtiene con (root):
//!     su -mm -c 'content query --uri content://user_dictionary/words \
//!         --projection word:shortcut:timestamp'

use std::process::ExitCode;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it.next().cloned();
        }
    }
    None
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn main() -> ExitCode {
    let sub = arg("--").or_else(|| {
        std::env::args()
            .nth(1)
            .filter(|a| !a.starts_with('-'))
    });
    let json = flag("--json");

    match sub.as_deref() {
        Some("scan") => {
            let f = match arg("--file") {
                Some(f) => f,
                None => {
                    eprintln!("Falta --file");
                    return ExitCode::from(2);
                }
            };
            let txt = match std::fs::read_to_string(&f) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("No se pudo leer {f}: {e}");
                    return ExitCode::from(2);
                }
            };
            let entradas = megadaemon_core::dict_sync::parse_user_dictionary(&txt);
            if json {
                let v: Vec<serde_json::Value> = entradas
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "palabra": e.palabra, "atajos": e.atajos, "ts": e.ts
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({"ok": true, "entradas": v.len(), "items": v})
                );
            } else {
                for e in &entradas {
                    let atajos = if e.atajos.is_empty() {
                        String::new()
                    } else {
                        format!("  [{}]", e.atajos.join(", "))
                    };
                    println!("{}{}", e.palabra, atajos);
                }
                println!("— {} entradas", entradas.len());
            }
            ExitCode::from(0)
        }
        Some("correct") => {
            let dict = match arg("--dict") {
                Some(f) => match std::fs::read_to_string(&f) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("No se pudo leer {f}: {e}");
                        return ExitCode::from(2);
                    }
                },
                None => {
                    eprintln!("Falta --dict");
                    return ExitCode::from(2);
                }
            };
            let texto = arg("--text").unwrap_or_default();
            let umbral = arg("--umbral")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.9);
            let entradas = megadaemon_core::dict_sync::parse_user_dictionary(&dict);
            let palabras: Vec<String> = entradas.iter().map(|e| e.palabra.clone()).collect();
            let sugerencias = megadaemon_core::dict_sync::sugerir(&texto, &palabras);
            let (corregido, aplicadas) =
                megadaemon_core::dict_sync::aplicar(&texto, &sugerencias, umbral);
            if json {
                let sug: Vec<serde_json::Value> = sugerencias
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "original": s.original, "corregida": s.corregida,
                            "confianza": s.confianza, "motivo": s.motivo,
                            "aplicada": aplicadas.iter().any(|a| a.original == s.original),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true, "umbral": umbral,
                        "texto": texto, "corregido": corregido,
                        "sugerencias": sug,
                    })
                );
            } else {
                for s in &sugerencias {
                    let marca = if aplicadas.iter().any(|a| a.original == s.original) {
                        "APLICADA"
                    } else {
                        "sugerida"
                    };
                    println!(
                        "  {marca}  {} -> {}  ({:.2}, {})",
                        s.original, s.corregida, s.confianza, s.motivo
                    );
                }
                if flag("--apply") {
                    println!("— corregido: {corregido}");
                }
            }
            ExitCode::from(0)
        }
        Some("merge") => {
            let a = arg("--a").unwrap_or_default();
            let b = arg("--b").unwrap_or_default();
            let leer = |p: &str| -> Vec<megadaemon_core::dict_sync::Entrada> {
                std::fs::read_to_string(p)
                    .map(|t| megadaemon_core::dict_sync::from_ndjson(&t))
                    .unwrap_or_default()
            };
            let m = megadaemon_core::dict_sync::merge(&leer(&a), &leer(&b));
            let nd = megadaemon_core::dict_sync::to_ndjson(&m);
            if json {
                let man = megadaemon_core::dict_sync::manifiesto(&nd);
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true, "entradas": m.len(),
                        "sha256": man.sha256, "bytes": man.bytes,
                    })
                );
            } else {
                print!("{nd}");
            }
            ExitCode::from(0)
        }
        _ => {
            eprintln!(
                "uso:\n  md-dict scan --file <volcado>\n  md-dict correct --dict <fichero> --text \"texto\" [--apply]\n  md-dict merge --a <f1> --b <f2>"
            );
            ExitCode::from(2)
        }
    }
}

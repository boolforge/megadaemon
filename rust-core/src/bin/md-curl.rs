//! `md-curl` — el curl de MegaDaemon, en Rust puro.
//!
//! El móvil NO tiene `curl` ni `wget` (medido el 2026-11-22: `toybox
//! wget` → "Unknown command", rc=127).  Este binario cubre lo que el
//! daemon necesita en el campo:
//!
//! ```text
//! md-curl [-o FICHERO] [-I] [--sha256 HEX] [--retries N] [--timeout S]
//!         [--parallel] [--workers N] URL [URL...]
//! ```
//!
//! Equivalencias: `-o` = salida, `-I` = cabeceras (HEAD), `--sha256` =
//! verificación, `--parallel` = descargas simultáneas, `--resume` es el
//! comportamiento por defecto (pide `Range` si el fichero ya existe).

use std::path::PathBuf;

fn ayuda() {
    println!(
        "md-curl — transferencias HTTP nativas de MegaDaemon (Rust puro)

USO:
  md-curl [opciones] URL [URL...]

OPCIONES:
  -o, --salida <fichero>   destino (con varias URLs: directorio)
  -I, --cabeceras         HEAD: sólo cabeceras
  -s, --sha256 <hex>      SHA-256 esperado (si no coincide, falla)
  -r, --retries <n>       reintentos (defecto 3)
  -t, --timeout <s>       timeout global en segundos (defecto 120)
  -p, --parallel          descarga todas las URLs en paralelo
  -w, --workers <n>       hilos en paralelo (defecto 4)
  -h, --ayuda             esta ayuda

EJEMPLOS:
  md-curl -I https://example.com
  md-curl -o /data/local/tmp/megadaemon/x.bin https://host/x.bin
  md-curl -o /tmp/x.bin --sha256 abcd... https://host/x.bin
  md-curl -p -o /tmp/descargas/ https://a/1 https://b/2 https://c/3"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut o = megadaemon_core::http_transfer::Opciones::default();
    let mut urls: Vec<String> = Vec::new();
    let mut destino: Option<PathBuf> = None;
    let mut head_only = false;
    let mut parallel = false;
    let mut workers = 4usize;

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "-h" | "--ayuda" => {
                ayuda();
                return;
            }
            "-I" | "--cabeceras" => head_only = true,
            "-p" | "--parallel" => parallel = true,
            "-o" | "--salida" => {
                i += 1;
                if i < args.len() {
                    destino = Some(PathBuf::from(&args[i]));
                }
            }
            "-s" | "--sha256" => {
                i += 1;
                if i < args.len() {
                    o.sha256_esperado = Some(args[i].clone());
                }
            }
            "-r" | "--retries" => {
                i += 1;
                if i < args.len() {
                    o.reintentos = args[i].parse().unwrap_or(3);
                }
            }
            "-t" | "--timeout" => {
                i += 1;
                if i < args.len() {
                    o.timeout_s = args[i].parse().unwrap_or(120);
                }
            }
            "-w" | "--workers" => {
                i += 1;
                if i < args.len() {
                    workers = args[i].parse().unwrap_or(4);
                }
            }
            _ => {
                if a.starts_with("http://") || a.starts_with("https://") {
                    urls.push(a.to_string());
                }
            }
        }
        i += 1;
    }

    if urls.is_empty() {
        ayuda();
        std::process::exit(2);
    }

    if head_only {
        for u in &urls {
            match megadaemon_core::http_transfer::head(u, &o) {
                Ok(h) => {
                    println!("HTTP {} {}", h.status, u);
                    for (k, v) in &h.cabeceras {
                        println!("  {k}: {v}");
                    }
                }
                Err(e) => {
                    eprintln!("ERROR {u}: {e}");
                    std::process::exit(1);
                }
            }
        }
        return;
    }

    if parallel || urls.len() > 1 {
        let dir = destino.unwrap_or_else(|| PathBuf::from("."));
        let _ = std::fs::create_dir_all(&dir);
        let rep = megadaemon_core::http_transfer::descargar_paralelo(&urls, &dir, workers, &o);
        let txt = serde_json::to_string_pretty(&rep).unwrap_or_default();
        println!("{txt}");
        let ok = rep.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
        std::process::exit(if ok { 0 } else { 1 });
    }

    o.destino = destino;
    let url = &urls[0];
    match megadaemon_core::http_transfer::descargar_con_reintentos(url, &o) {
        Ok(r) => {
            let txt = serde_json::to_string_pretty(&r).unwrap_or_default();
            println!("{txt}");
            if !r.ok {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}

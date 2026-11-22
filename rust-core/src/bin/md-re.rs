//! `md-re` — el motor de RE de MegaDaemon por línea de órdenes.
//!
//! ```text
//! md-re catalogo                       # qué sabe hacer MegaDaemon de RE
//! md-re identify  <binario>            # formato + evidencia
//! md-re analyze   <binario>            # informe completo (uniforme)
//! md-re strings   <binario> [--n 60]   # cadenas interesting
//! md-re deobfuscate <binario>          # MOTOR de deofuscación (con pruebas)
//! md-re mba <expresion>              # simplifica MBA + devuelve la prueba
//! ```
//!
//! Todo en Rust puro y **estático**: nunca ejecuta el binario analizado.

use std::path::Path;

fn ayuda() {
    println!(
        "md-re — motor de RE/DEOFUSCACION de MegaDaemon

  md-re catalogo                catalogo de capacidades (que modulo hace que)
  md-re identify <fichero>      formato del binario, con evidencia
  md-re analyze  <fichero>      informe completo (strings, secciones, go, apk)
  md-re strings  <fichero> [--n N]   cadenas interesting (URLs, claves, rutas)
  md-re deobfuscate <fichero>   MOTOR de deofuscacion: MBA con prueba BDD,
                                control-flow flattening con prueba diferencial,
                                strings cifrados, constantes ofuscadas y
                                predicados opacos (todo con su evidencia)
  md-re mba <expresion>       simplifica una expresion MBA y devuelve la
                                DEMOSTRACION de equivalencia (BDD)

Es analisis ESTATICO: nunca ejecuta el binario analizado."
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
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--n" => {
                i += 1;
                if i < args.len() {
                    max = args[i].parse().unwrap_or(60);
                }
            }
            a => {
                if ruta.is_empty() {
                    ruta = a.to_string();
                }
            }
        }
        i += 1;
    }

    let v: serde_json::Value = match sub.as_str() {
        "catalogo" => megadaemon_core::re_engine::catalogo(),
        "mba" => {
            // La expresion es el resto de los argumentos (puede tener espacios).
            let expr: String = args[1..].join(" ");
            if expr.trim().is_empty() {
                eprintln!("falta la expresion MBA");
                std::process::exit(2);
            }
            megadaemon_core::re_engine::mba(&expr)
        }
        "deobfuscate" => {
            if ruta.is_empty() {
                eprintln!("falta el fichero");
                std::process::exit(2);
            }
            let mut o = megadaemon_core::re_engine::OpcionesDeofuscacion::default();
            o.max_cadenas = max;
            match megadaemon_core::re_engine::deobfuscar(Path::new(&ruta), &o) {
                Ok(v) => v,
                Err(e) => serde_json::json!({ "ok": false, "error": e }),
            }
        }
        "identify" | "analyze" | "strings" => {
            if ruta.is_empty() {
                eprintln!("falta el fichero");
                std::process::exit(2);
            }
            let p = Path::new(&ruta);
            match sub.as_str() {
                "identify" => match megadaemon_core::re_engine::identificar(p) {
                    Ok(id) => serde_json::to_value(id).unwrap_or_default(),
                    Err(e) => serde_json::json!({ "ok": false, "error": e }),
                },
                "strings" => match std::fs::read(p) {
                    Ok(d) => serde_json::json!({
                        "ok": true,
                        "cadenas": megadaemon_core::re_engine::cadenas_interesantes(&d, max),
                    }),
                    Err(e) => serde_json::json!({ "ok": false, "error": format!("leer: {e}") }),
                },
                _ => {
                    let mut o = megadaemon_core::re_engine::OpcionesAnalisis::default();
                    o.max_cadenas = max;
                    match megadaemon_core::re_engine::analizar(p, &o) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({ "ok": false, "error": e }),
                    }
                }
            }
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

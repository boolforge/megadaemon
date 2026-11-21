//! `md-clipboard` — portapapeles distribuido: historial, búsqueda y sincronización.
//!
//! Uso:
//!     md-clipboard put "texto"            # añade al historial (y lo copia)
//!     md-clipboard get [--id <id>]        # imprime el texto para pegar
//!     md-clipboard history [--limit N]    # lista el historial
//!     md-clipboard search "query"         # busca en el historial
//!     md-clipboard clear                  # borra el historial
//!     md-clipboard watch --interval 5     # vigila el portapapeles del sistema
//!     md-clipboard export [--out f]       # exporta (cifrado si hay passphrase)
//!     md-clipboard import --in f          # importa
//!
//! Opciones comunes:
//!     --state-dir <dir>   donde vive el historial (por defecto, el state_dir
//!                         del daemon)
//!     --json              salida JSON
//!     --mock              usa un portapapeles en memoria (sin dispositivo)
//!
//! La passphrase de cifrado se lee de `MEGADAEMON_CLIP_PASSPHRASE` (nunca de
//! la línea de órdenes: quedaría en el historial del shell).

use megadaemon_core::clipboard::{self, ClipboardProvider, Historial, Item, MockClipboard};
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

/// Opciones que llevan valor detrás (para no confundirlas con posicionales).
const OPCIONES_CON_VALOR: &[&str] = &[
    "--state-dir", "--text", "--id", "--limit", "--interval", "--out", "--in", "--query",
];

/// Primer argumento posicional (el subcomando o el texto).
///
/// Los flags con valor se saltan junto con su valor; los booleanos no.
fn posicional(indice: usize) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1usize;
    let mut vistos = 0usize;
    while i < args.len() {
        let a = &args[i];
        if a == "--" {
            return args.get(i + 1).cloned();
        }
        if a.starts_with("--") {
            if OPCIONES_CON_VALOR.contains(&a.as_str()) {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if vistos == indice {
            return Some(a.clone());
        }
        vistos += 1;
        i += 1;
    }
    None
}

fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ruta del historial.
fn ruta_historial(state_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(state_dir).join("clipboard.ndjson")
}

/// Passphrase de cifrado (sólo del entorno).
fn passphrase() -> Option<String> {
    std::env::var("MEGADAEMON_CLIP_PASSPHRASE")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Carga el historial del disco.
fn cargar(state_dir: &str) -> Historial {
    let mut h = Historial::nuevo();
    let ruta = ruta_historial(state_dir);
    if let Ok(items) = clipboard::cargar(&ruta, passphrase().as_deref()) {
        for i in items {
            h.push(i);
        }
    }
    h
}

/// Guarda el historial.
fn guardar(state_dir: &str, h: &Historial) -> Result<(), String> {
    let items = h.items();
    clipboard::guardar(&ruta_historial(state_dir), &items, passphrase().as_deref())
}

/// Proveedor según `--mock`.
fn proveedor(mock: bool) -> Box<dyn ClipboardProvider> {
    if mock {
        Box::new(MockClipboard::nuevo())
    } else {
        clipboard::proveedor_por_defecto()
    }
}

fn main() -> ExitCode {
    let sub = posicional(0);
    let json = flag("--json");
    let mock = flag("--mock");
    let state_dir = arg("--state-dir").unwrap_or_else(|| {
        std::env::var("MEGADAEMON_STATE_DIR").unwrap_or_else(|_| "/data/local/tmp/megadaemon".to_string())
    });

    match sub.as_deref() {
        Some("put") => {
            // El texto puede venir como argumento o por stdin.
            let texto = match arg("--text") {
                Some(t) => t,
                None => match posicional(1) {
                    Some(t) => t,
                    None => {
                        use std::io::Read;
                        let mut s = String::new();
                        if std::io::stdin().read_to_string(&mut s).is_err() || s.trim().is_empty() {
                            eprintln!("Nada que poner. Uso: md-clipboard put \"texto\"");
                            return ExitCode::from(2);
                        }
                        s.trim().to_string()
                    }
                },
            };
            let mut h = cargar(&state_dir);
            let item = Item::nuevo(&texto, ahora(), "manual");
            let sensible = item.sensible;
            let aceptado = h.push(item);
            if let Err(e) = guardar(&state_dir, &h) {
                eprintln!("No se pudo guardar: {e}");
                return ExitCode::from(1);
            }
            // Intenta copiarlo al portapapeles del sistema (best effort).
            let p = proveedor(mock);
            let copiado = p.escribir(&texto).is_ok();
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "aceptado": aceptado,
                        "sensibles": sensible,
                        "copiado": copiado,
                        "proveedor": p.nombre(),
                    })
                );
            } else {
                println!(
                    "guardado{} (proveedor: {})",
                    if aceptado { "" } else { " [no guardado: sensible]" },
                    p.nombre()
                );
            }
            ExitCode::SUCCESS
        }

        Some("get") => {
            let h = cargar(&state_dir);
            let item = match arg("--id") {
                Some(id) => h.por_id(&id).cloned(),
                None => h.items().first().map(|i| (*i).clone()),
            };
            match item {
                Some(i) => {
                    if json {
                        println!("{}", i.a_json());
                    } else {
                        println!("{}", i.texto);
                    }
                    ExitCode::SUCCESS
                }
                None => {
                    eprintln!("Historial vacio");
                    ExitCode::from(1)
                }
            }
        }

        Some("history") => {
            let h = cargar(&state_dir);
            let limite = arg("--limit")
                .and_then(|l| l.parse::<usize>().ok())
                .unwrap_or(50);
            let items: Vec<Item> = h.items().into_iter().take(limite).cloned().collect();
            if json {
                let v: Vec<serde_json::Value> = items.iter().map(|i| i.a_json()).collect();
                println!("{}", serde_json::json!({ "items": v }));
            } else {
                for i in &items {
                    let resto = if i.restante_s(ahora()) > 0 {
                        format!(" (expira en {}s)", i.restante_s(ahora()))
                    } else {
                        String::new()
                    };
                    let txt = i.texto.replace('\n', " ");
                    let txt = if txt.len() > 90 {
                        format!("{}…", &txt[..90])
                    } else {
                        txt
                    };
                    println!("[{}] {} {}{}", i.tipo.as_str(), &i.id[..8.min(i.id.len())], txt, resto);
                }
            }
            ExitCode::SUCCESS
        }

        Some("search") => {
            let q = match arg("--query") {
                Some(q) => q,
                None => posicional(1).unwrap_or_default(),
            };
            let h = cargar(&state_dir);
            let res = h.buscar(&q, 50);
            if json {
                let v: Vec<serde_json::Value> = res.iter().map(|i| i.a_json()).collect();
                println!("{}", serde_json::json!({ "items": v }));
            } else {
                for i in res {
                    println!("[{}] {}", i.tipo.as_str(), i.texto.replace('\n', " "));
                }
            }
            ExitCode::SUCCESS
        }

        Some("clear") => {
            let mut h = Historial::nuevo();
            if let Err(e) = guardar(&state_dir, &h) {
                eprintln!("No se pudo guardar: {e}");
                return ExitCode::from(1);
            }
            println!("historial vaciado");
            ExitCode::SUCCESS
        }

        Some("watch") => {
            let intervalo = arg("--interval")
                .and_then(|l| l.parse::<u64>().ok())
                .unwrap_or(5);
            let p = proveedor(mock);
            let mut h = cargar(&state_dir);
            let mut ultimo: Option<String> = None;
            println!("vigilando el portapapeles cada {intervalo}s (Ctrl-C para salir)");
            loop {
                match p.leer() {
                    Ok(Some(t)) => {
                        if Some(&t) != ultimo.as_ref() {
                            ultimo = Some(t.clone());
                            let item = Item::nuevo(&t, ahora(), p.nombre());
                            let sensible = item.sensible;
                            let aceptado = h.push(item);
                            let _ = guardar(&state_dir, &h);
                            println!(
                                "capturado{} [{}]",
                                if sensible { " (sensible, no persistente)" } else { "" },
                                &t.chars().take(60).collect::<String>()
                            );
                            let _ = aceptado;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("proveedor {}: {e}", p.nombre());
                        return ExitCode::from(1);
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(intervalo));
            }
        }

        Some("export") => {
            let h = cargar(&state_dir);
            let out = arg("--out").unwrap_or_else(|| {
                format!("{}/clipboard-export.ndjson", state_dir)
            });
            let items = h.items();
            let res = clipboard::guardar(std::path::Path::new(&out), &items, passphrase().as_deref());
            match res {
                Ok(()) => {
                    println!("exportado {} item(s) a {out}", items.len());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("No se pudo exportar: {e}");
                    ExitCode::from(1)
                }
            }
        }

        Some("import") => {
            let inp = match arg("--in") {
                Some(f) => f,
                None => {
                    eprintln!("Falta --in");
                    return ExitCode::from(2);
                }
            };
            let items = match clipboard::cargar(std::path::Path::new(&inp), passphrase().as_deref()) {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("No se pudo importar: {e}");
                    return ExitCode::from(1);
                }
            };
            let mut h = cargar(&state_dir);
            let n = items.len();
            for i in items {
                h.push(i);
            }
            if let Err(e) = guardar(&state_dir, &h) {
                eprintln!("No se pudo guardar: {e}");
                return ExitCode::from(1);
            }
            println!("importados {n} item(s)");
            ExitCode::SUCCESS
        }

        _ => {
            eprintln!(
                "md-clipboard — portapapeles distribuido\n\
                 \n\
                 Uso:\n  \
                   md-clipboard put \"texto\" [--text T]\n  \
                   md-clipboard get [--id <id>]\n  \
                   md-clipboard history [--limit N]\n  \
                   md-clipboard search \"query\"\n  \
                   md-clipboard clear\n  \
                   md-clipboard watch [--interval N]\n  \
                   md-clipboard export [--out f]\n  \
                   md-clipboard import --in f\n\
                 \n\
                 Opciones: --state-dir <dir>  --json  --mock"
            );
            ExitCode::from(2)
        }
    }
}

//! `megadaemon-titanium` — CLI del lector de backups «tar por paquete» y del
//! catálogo de backups de Drive.
//!
//! ```text
//! megadaemon-titanium list    --dir <carpeta>
//! megadaemon-titanium verify  --file <fichero>
//! megadaemon-titanium extract --file <fichero> --dest <carpeta>
//! megadaemon-titanium index   --db <main.db>
//! megadaemon-titanium drive   --folder <id> [--state-dir <dir>] [--recursivo]
//! megadaemon-titanium plan    --folder <id> [--ia] [--modelo <id>] [--tope-extra <bytes>]
//! megadaemon-titanium roundtrip --file <dat> [--passphrase-file <ruta>]
//! ```
//!
//! Reglas:
//! * **sólo lectura** por defecto: nada se escribe fuera de `--dest`;
//! * **JSON por stdout**: la salida es parseable por el bus;
//! * **la passphrase nunca se imprime**: se lee de `--passphrase-file` (0600,
//!   fuera del repo) o de `${SWIFT_BACKUP_PASSPHRASE}` y no se escribe ni en
//!   la salida ni en los logs.

use megadaemon_core::backup_catalog;
use megadaemon_core::migration_plan;
use megadaemon_core::swift_container;
use megadaemon_core::titanium_backup as tb;
use serde_json::json;
use std::path::PathBuf;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        if let Some(v) = args[i].strip_prefix(&format!("{name}=")) {
            return Some(v.to_string());
        }
        i += 1;
    }
    None
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn uso() -> ! {
    eprintln!(
        "uso:\n  \
         megadaemon-titanium list    --dir <carpeta>\n  \
         megadaemon-titanium verify  --file <fichero>\n  \
         megadaemon-titanium extract --file <fichero> --dest <carpeta>\n  \
         megadaemon-titanium index   --db <main.db>\n  \
         megadaemon-titanium drive   --folder <id> [--state-dir <dir>] [--recursivo]\n  \
         megadaemon-titanium plan    --folder <id> [--ia] [--modelo <id>] [--tope-extra <bytes>]\n  \
         megadaemon-titanium roundtrip --file <dat> [--passphrase-file <ruta>]"
    );
    std::process::exit(2);
}

fn main() {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let salida = match cmd.as_str() {
        "list" => {
            let dir = arg("--dir").unwrap_or_else(|| uso());
            match tb::inventariar_directorio(&PathBuf::from(&dir)) {
                Ok(v) => json!({"ok": true, "dir": dir, "paquetes": v, "total": v.len()}),
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "verify" => {
            let file = arg("--file").unwrap_or_else(|| uso());
            match tb::verificar_fichero(&PathBuf::from(&file)) {
                Ok(v) => json!({"ok": v.ok, "verificacion": v}),
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "extract" => {
            let file = arg("--file").unwrap_or_else(|| uso());
            let dest = arg("--dest").unwrap_or_else(|| uso());
            match tb::extraer_paquete(&PathBuf::from(&file), &PathBuf::from(&dest)) {
                Ok(r) => json!({"ok": r.ok, "resumen": r}),
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "index" => {
            let db = arg("--db").unwrap_or_else(|| uso());
            match tb::leer_indice_sqlite(&PathBuf::from(&db)) {
                Ok(v) => json!({"ok": true, "entradas": v.len(), "indice": v}),
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "drive" => {
            let folder = arg("--folder").unwrap_or_else(|| uso());
            let state = arg("--state-dir");
            let recursivo = flag("--recursivo");
            let resultado = if recursivo {
                backup_catalog::listar_drive_recursivo(&folder, state.as_deref(), 200, true)
            } else {
                backup_catalog::listar_drive(&folder, state.as_deref(), 200)
            };
            match resultado {
                Ok(v) => {
                    let r = backup_catalog::resumir(&v);
                    json!({
                        "ok": true,
                        "carpeta": folder,
                        "ficheros": r.ficheros,
                        "bytes": r.bytes,
                        "bytes_legibles": backup_catalog::bytes_legibles(r.bytes),
                        "por_clase": r.por_clase,
                        "entradas": v,
                    })
                }
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "roundtrip" => {
            // La passphrase NUNCA se imprime: se lee del fichero 0600 o del
            // entorno (`$SWIFT_BACKUP_PASSPHRASE`).
            let file = arg("--file").unwrap_or_else(|| uso());
            let passphrase = match arg("--passphrase-file") {
                Some(f) => match swift_container::leer_passphrase_de(&PathBuf::from(&f)) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{}", json!({"ok": false, "error": e}));
                        return;
                    }
                },
                None => match swift_container::leer_passphrase() {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{}", json!({"ok": false, "error": e}));
                        return;
                    }
                },
            };
            let bytes = match std::fs::read(&file) {
                Ok(b) => b,
                Err(e) => {
                    println!("{}", json!({"ok": false, "error": e.to_string()}));
                    return;
                }
            };
            match swift_container::round_trip(&bytes, &passphrase) {
                Ok(r) => json!({"ok": r.ok, "round_trip": r}),
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "plan" => {
            let folder = arg("--folder").unwrap_or_else(|| uso());
            let state = arg("--state-dir");
            let tope = arg("--tope-extra")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(64 * 1024 * 1024);
            let inventario = match backup_catalog::listar_drive(&folder, state.as_deref(), 200) {
                Ok(v) => v,
                Err(e) => {
                    println!("{}", json!({"ok": false, "error": e}));
                    return;
                }
            };
            let base = migration_plan::plan_base(&inventario, tope);
            let mut plan = base.clone();
            let mut ia = json!({"consultada": false});
            if flag("--ia") {
                let modelo = arg("--modelo");
                let prompt = migration_plan::construir_prompt(
                    &base,
                    "Migrar apps y datos de Android entre el móvil y Drive",
                );
                match migration_plan::consultar_ia(&prompt, modelo.as_deref()) {
                    Ok((resp, modelo_id)) => {
                        let (p, v) = migration_plan::validar_plan_ia(&base, &resp);
                        plan = p;
                        plan.modelo = Some(modelo_id);
                        ia = json!({
                            "consultada": true,
                            "validacion": v,
                            "correcciones": plan.correcciones,
                            "descartadas": plan.descartadas,
                        });
                    }
                    Err(e) => {
                        ia = json!({"consultada": true, "error": e});
                    }
                }
            }
            println!(
                "{}",
                json!({
                    "ok": true,
                    "carpeta": folder,
                    "inventario": {
                        "ficheros": base.totales.copiados + base.totales.excluidos + base.totales.no_migrables,
                        "bytes": base.totales.bytes_totales,
                    },
                    "plan": plan,
                    "ia": ia,
                })
            );
        }
        _ => uso(),
    };
    println!("{salida}");
}

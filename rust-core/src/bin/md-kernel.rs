//! `md-kernel` — CLI del estado del kernel y del **punto de retorno**.
//!
//! Se usa desde el propio móvil (con root) para:
//!
//! * medir el kernel real (versión, config, módulos, particiones);
//! * **volcar** las particiones de arranque a fichero y verificar su SHA-256;
//! * **subir** el respaldo a Google Drive (carpeta privada del proyecto);
//! * **verificar** que el punto de retorno sigue siendo válido;
//! * generar el **plan de rollback** (texto; nunca se ejecuta solo).
//!
//! ```text
//! md-kernel info
//! md-kernel backup [--dir /data/local/tmp/kernel-backup] [--partitions boot,vendor_boot,recovery] [--overwrite]
//! md-kernel verify --manifest <dir>/manifest.json [--check-live]
//! md-kernel upload-drive --manifest <dir>/manifest.json [--folder <id>] [--credentials <fichero>] [--prefix <nombre>]
//! md-kernel rollback-plan --manifest <dir>/manifest.json
//! ```
//!
//! **Nunca** escribe en `/dev/block/*`: el volcado va a un fichero y la
//! restauración se genera como texto para que la ejecute el usuario a mano.

use megadaemon_core::kernel_and_blocks as kb;
use serde_json::Value;
use std::process::ExitCode;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        if let Some(v) = args[i].strip_prefix(&format!("{}=", name)) {
            return Some(v.to_string());
        }
        i += 1;
    }
    None
}

fn has(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn usage() -> &'static str {
    "uso: md-kernel <info|backup|verify|upload-drive|rollback-plan> [opciones]\n\
     \n\
     opciones comunes:\n\
     \x20 --json                 salida en JSON (por defecto: resumen legible)\n\
     \n\
     backup:\n\
     \x20 --dir <ruta>            destino del volcado (por defecto /data/local/tmp/kernel-backup)\n\
     \x20 --partitions <a,b,c>    particiones (por defecto: boot,vendor_boot,recovery,init_boot)\n\
     \x20 --overwrite             rehace un volcado que ya existe\n\
     \n\
     verify / upload-drive / rollback-plan:\n\
     \x20 --manifest <ruta>       manifiesto (por defecto <dir>/manifest.json)\n\
     \x20 --check-live            compara además con la partición viva\n\
     \x20 --folder <id>           carpeta de Drive\n\
     \x20 --credentials <fichero> credenciales OAuth de Google\n\
     \x20 --prefix <nombre>       prefijo de los nombres en Drive"
}

fn print_json(v: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".to_string())
    );
}

fn ok_of(v: &Value) -> bool {
    v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("{}", usage());
        return ExitCode::from(2);
    }
    let cmd = args[1].clone();
    let as_json = has("--json");

    let default_dir = arg("--dir").unwrap_or_else(|| "/data/local/tmp/kernel-backup".to_string());
    let default_manifest = arg("--manifest").unwrap_or_else(|| {
        format!("{}/manifest.json", default_dir.trim_end_matches('/'))
    });

    let out: Value = match cmd.as_str() {
        "info" => kb::info(),
        "backup" => {
            let parts: Vec<String> = arg("--partitions")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let parts = if parts.is_empty() {
                kb::DEFAULT_BOOT_PARTITIONS
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            } else {
                parts
            };
            kb::backup_set(&parts, &default_dir, has("--overwrite"))
        }
        "verify" => kb::verify_manifest(&default_manifest, has("--check-live")),
        "rollback-plan" => kb::rollback_plan(&default_manifest),
        "upload-drive" => {
            let folder = arg("--folder").unwrap_or_else(|| kb::DEFAULT_DRIVE_FOLDER.to_string());
            let creds = arg("--credentials").unwrap_or_else(|| {
                "/data/user/0/com.agy/files/megadaemon/state/gdrive_credentials.json".to_string()
            });
            let prefix = arg("--prefix").unwrap_or_else(|| {
                format!("kernel-backup-{}", kb::now_iso().replace(':', "-"))
            });
            kb::upload_manifest_to_drive(&default_manifest, &folder, &creds, &prefix)
        }
        other => {
            eprintln!("comando desconocido: {}\n{}", other, usage());
            return ExitCode::from(2);
        }
    };

    if as_json {
        print_json(&out);
    } else {
        print_human(&cmd, &out);
    }

    if ok_of(&out) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Resumen legible (el JSON completo va con `--json`).
fn print_human(cmd: &str, v: &Value) {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    match cmd {
        "info" => {
            println!("{}", v.get("info").and_then(|x| x.as_str()).unwrap_or(""));
        }
        "backup" => {
            println!("manifiesto: {}", s("manifest"));
            println!("creado:     {}", s("created_at"));
            if let Some(absent) = v.get("absent_partitions").and_then(|x| x.as_array()) {
                if !absent.is_empty() {
                    let names: Vec<String> = absent
                        .iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect();
                    println!(
                        "ausentes:   {} (no existen en este dispositivo: se ignoran)",
                        names.join(", ")
                    );
                }
            }
            if let Some(entries) = v.get("entries").and_then(|x| x.as_array()) {
                for e in entries {
                    let p = e.get("partition").and_then(|x| x.as_str()).unwrap_or("");
                    let sha = e.get("sha256").and_then(|x| x.as_str()).unwrap_or("");
                    let ms = e
                        .get("duration_ms")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0);
                    let ok = e.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
                    let ausente = e
                        .get("ausente")
                        .and_then(|x| x.as_bool())
                        .unwrap_or(false);
                    if ausente {
                        println!("  {:<12} AUSENTE", p);
                    } else {
                        println!(
                            "  {:<12} {}  {} ms  sha256={}",
                            p,
                            if ok { "OK " } else { "MAL" },
                            ms,
                            &sha.chars().take(16).collect::<String>()
                        );
                    }
                }
            }
            println!(
                "resultado:  {}",
                if ok_of(v) {
                    "respaldo verificado"
                } else {
                    "HAY ALGO QUE NO CUADRA"
                }
            );
        }
        "verify" => {
            if let Some(checks) = v.get("checks").and_then(|x| x.as_array()) {
                for c in checks {
                    let p = c.get("partition").and_then(|x| x.as_str()).unwrap_or("");
                    let ok = c.get("file_ok").and_then(|x| x.as_bool()).unwrap_or(false);
                    let live = c.get("live_matches_backup").and_then(|x| x.as_bool());
                    let live_txt = match live {
                        Some(true) => " (vivo == respaldo)",
                        Some(false) => " (¡el móvil ha cambiado!)",
                        None => "",
                    };
                    println!("  {:<12} {}", p, if ok { "OK" } else { "CORRUPTO" });
                    if !live_txt.is_empty() {
                        println!("{}", live_txt);
                    }
                }
            }
            println!("{}", s("note"));
        }
        "rollback-plan" => {
            println!("{}", v.get("script").and_then(|x| x.as_str()).unwrap_or(""));
            println!("(plan SOLO: no se ha escrito nada en ninguna partición)");
        }
        "upload-drive" => {
            if let Some(rs) = v.get("results").and_then(|x| x.as_array()) {
                for r in rs {
                    let p = r.get("partition").and_then(|x| x.as_str()).unwrap_or("");
                    let id = r.get("file_id").and_then(|x| x.as_str()).unwrap_or("");
                    let ok = r.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
                    println!("  {:<12} {}  id={}", p, if ok { "OK " } else { "MAL" }, id);
                }
            }
            println!("carpeta: {}", s("folder_id"));
        }
        _ => print_json(v),
    }
}

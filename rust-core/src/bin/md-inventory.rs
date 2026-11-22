//! `md-inventory` — CLI del motor de **inventario exhaustivo** del móvil.
//!
//! ```text
//! md-inventory matrix                       # la matriz P0..P5
//! md-inventory scan [--perfil=P0] [--json]  # qué existe, cuántos ficheros, tamaño, sha16
//! md-inventory export <perfil> [opciones]   # contenedor cifrado + manifiesto + subida a Drive
//! md-inventory verify <manifiesto> [--key=F]# re-verifica (local o descargado)
//! md-inventory restore <perfil> --app=...   # restaura con --confirm obligatorio
//! md-inventory consolidate [--json]         # informe de consolidación de apps
//! ```
//!
//! ## Regla de oro
//!
//! **De los secretos sólo sale su `sha16` y su tamaño.** Ni al repo, ni al
//! chat, ni al tablón. El valor viaja dentro del contenedor cifrado; la clave
//! se entrega por un canal `0600` (`--key-out`).

use megadaemon_core::backup_inventory::{
    self, consolidate, export, restore, scan, verify_container, ConsolidationReport, Device,
    ExportOptions, InventoryReport, KeyMaterial, Level, ProgressObserver, RealDevice,
    RestoreOptions, RestoreSelection, ScanOptions, SilentObserver,
};
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        return ExitCode::from(2);
    }
    let cmd = args[0].clone();
    let rest = &args[1..];
    let r = match cmd.as_str() {
        "matrix" => cmd_matrix(),
        "scan" => cmd_scan(rest),
        "export" => cmd_export(rest),
        "verify" => cmd_verify(rest),
        "restore" => cmd_restore(rest),
        "consolidate" => cmd_consolidate(rest),
        "--help" | "-h" | "help" => {
            usage();
            return ExitCode::SUCCESS;
        }
        other => {
            eprintln!("md-inventory: comando desconocido '{other}'");
            usage();
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("md-inventory: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!(
        "uso:
  md-inventory matrix
  md-inventory scan [--perfil=P0,P1] [--json] [--dry-run] [--batch=N] [--pause=MS] [--out=F]
  md-inventory export <perfil> [--out=DIR] [--key-out=F] [--passphrase-env=VAR] [--upload] [--folder=ID]
  md-inventory verify <manifiesto|contenedor> [--key=F]
  md-inventory restore <perfil> --app=PKG [--perfil=P0] --confirm [--dest=DIR] [--key=F]
  md-inventory consolidate [--json] [--out=F]"
    );
}

/// Lee `--clave=valor` de los argumentos.
fn opt(args: &[String], name: &str) -> Option<String> {
    let prefix = format!("--{name}=");
    args.iter()
        .find(|a| a.starts_with(&prefix))
        .map(|a| a[prefix.len()..].to_string())
}

/// Lee un flag (presente = true).
fn flag(args: &[String], name: &str) -> bool {
    // MEDIDO 2026-11-22: comparaba `&String` con `String` (E0277).
    let flag = format!("--{name}");
    args.iter().any(|a| a == &flag)
}

/// Parsea una lista de niveles (`P0`, `P0-identidad`, ...).
fn parse_levels(text: &str) -> Result<Vec<Level>, String> {
    let mut out = Vec::new();
    for part in text.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        match Level::parse(p) {
            Some(l) => out.push(l),
            None => return Err(format!("nivel desconocido: '{p}'")),
        }
    }
    Ok(out)
}

/// Comando `matrix`: imprime la matriz del plan.
fn cmd_matrix() -> Result<(), String> {
    for r in backup_inventory::matrix() {
        println!(
            "{}  {:<28} {}",
            r.id,
            r.label,
            if r.sensitive { "[sensible]" } else { "" }
        );
        for p in r.paths {
            println!("      · {p}");
        }
    }
    println!();
    for r in backup_inventory::package_rules() {
        println!("{}  {:<28} /data/user/0/<pkg>", r.id, r.label);
    }
    Ok(())
}

/// Comando `scan`.
fn cmd_scan(args: &[String]) -> Result<(), String> {
    let root = opt(args, "root").unwrap_or_else(|| "/".to_string());
    let levels = match opt(args, "perfil").or_else(|| opt(args, "nivel")) {
        Some(t) => parse_levels(&t)?,
        None => Vec::new(),
    };
    let mut opts = ScanOptions {
        levels,
        dry_run: flag(args, "dry-run"),
        ..ScanOptions::default()
    };
    if let Some(b) = opt(args, "batch") {
        opts.batch_size = b.parse().map_err(|_| "--batch no es un número".to_string())?;
    }
    if let Some(p) = opt(args, "pause") {
        opts.pause_ms = p.parse().map_err(|_| "--pause no es un número".to_string())?;
    }
    if let Some(n) = opt(args, "nice") {
        opts.nice = n.parse().map_err(|_| "--nice no es un número".to_string())?;
    }
    if let Some(d) = opt(args, "max-depth") {
        opts.max_depth = d.parse().map_err(|_| "--max-depth no es un número".to_string())?;
    }
    if let Some(h) = opt(args, "hash-max") {
        opts.hash_max_bytes = h.parse().map_err(|_| "--hash-max no es un número".to_string())?;
    }
    opts.progress = !flag(args, "no-progress");

    let dev = RealDevice { root };
    let mut obs: Box<dyn backup_inventory::ScanObserver> = if opts.progress {
        Box::new(ProgressObserver)
    } else {
        Box::new(SilentObserver)
    };
    let report = scan(&dev, &opts, obs.as_mut());

    if flag(args, "json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&report.to_json()).map_err(|e| e.to_string())?
        );
    } else {
        print!("{}", report.to_text());
    }
    if let Some(out) = opt(args, "out") {
        // El JSON del informe **no lleva valores**: sólo sha16 y tamaños.
        std::fs::write(
            &out,
            serde_json::to_vec_pretty(&json!({
                "report": report.to_json(),
                "entries": report.entries,
            }))
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("no se puede escribir {out}: {e}"))?;
        eprintln!("informe escrito en {out}");
    }
    Ok(())
}

/// Comando `export`.
fn cmd_export(args: &[String]) -> Result<(), String> {
    let profile = args
        .first()
        .cloned()
        .ok_or("hace falta el perfil (P0..P5)")?;
    let level = Level::parse(&profile)
        .ok_or_else(|| format!("perfil desconocido: '{profile}'"))?;
    let out_dir = PathBuf::from(
        opt(args, "out").unwrap_or_else(|| "/data/local/tmp/megadaemon/inventory".to_string()),
    );
    let key_out = opt(args, "key-out").map(PathBuf::from);
    let passphrase = opt(args, "passphrase-env")
        .and_then(|v| std::env::var(v).ok())
        .or_else(|| opt(args, "passphrase"));
    let folder_id = opt(args, "folder").unwrap_or_default();
    let upload = flag(args, "upload") || !folder_id.is_empty();
    let state_dir = opt(args, "state").or_else(|| opt(args, "state-dir"));

    // 1) Escaneo (enumeración + hashes) para saber qué entra.
    let mut sopts = ScanOptions {
        levels: vec![level],
        ..ScanOptions::default()
    };
    if let Some(b) = opt(args, "batch") {
        sopts.batch_size = b.parse().map_err(|_| "--batch no es un número".to_string())?;
    }
    if let Some(p) = opt(args, "pause") {
        sopts.pause_ms = p.parse().map_err(|_| "--pause no es un número".to_string())?;
    }
    sopts.progress = !flag(args, "no-progress");
    let dev = RealDevice::new();
    let mut obs: Box<dyn backup_inventory::ScanObserver> = if sopts.progress {
        Box::new(ProgressObserver)
    } else {
        Box::new(SilentObserver)
    };
    let report = scan(&dev, &sopts, obs.as_mut());

    // 2) Exportación.
    let eopts = ExportOptions {
        level,
        out_dir,
        passphrase,
        key_out,
        upload,
        folder_id,
        state_dir,
        ..ExportOptions::default()
    };
    let res = export(&dev, &eopts, &report.entries)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "contenedor": res.container,
            "manifiesto": res.manifest_path,
            "claves_0600": res.key_path,
            "bytes": res.bytes,
            "ficheros": res.files,
            "bytes_datos": res.data_bytes,
            "ficheros_sensibles": res.sensitive_files,
            "bytes_sensibles": res.sensitive_bytes,
            "sha16_manifiesto": res.manifest_sha16,
            "drive_file_id": res.drive_file_id,
            "aviso": "el valor de las claves NO se imprime: está en el fichero 0600 y dentro del contenedor",
        }))
        .map_err(|e| e.to_string())?
    );
    Ok(())
}

/// Comando `verify`.
fn cmd_verify(args: &[String]) -> Result<(), String> {
    let target = args.first().cloned().ok_or("hace falta el manifiesto o contenedor")?;
    let key_path = opt(args, "key").ok_or("hace falta --key (fichero 0600 con las claves)")?;
    let keys = KeyMaterial::read_secure(&PathBuf::from(&key_path))?;

    let p = PathBuf::from(&target);
    // Si es un manifiesto suelto, se busca el contenedor al lado.
    let container = if p.exists() && p.is_file() && target.ends_with(".json") {
        let stem = target.trim_end_matches(".manifest.json").trim_end_matches(".json");
        let cand = PathBuf::from(format!("{stem}.mdbinv"));
        if cand.exists() {
            cand
        } else {
            p
        }
    } else {
        p
    };

    let rep = verify_container(&container, &keys)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "ok": rep.ok,
            "perfil": rep.profile,
            "flujos": rep.streams,
            "ficheros_verificados": rep.files_checked,
            "ficheros_cojos": rep.files_bad,
            "bytes": rep.bytes,
            "sha16_manifiesto": rep.manifest_sha16,
            "detalle": rep.detail,
        }))
        .map_err(|e| e.to_string())?
    );
    if !rep.ok {
        return Err("la verificación ha fallado".to_string());
    }
    Ok(())
}

/// Comando `restore`.
fn cmd_restore(args: &[String]) -> Result<(), String> {
    let profile = args
        .first()
        .cloned()
        .ok_or("hace falta el perfil (P0..P5)")?;
    let level = Level::parse(&profile)
        .ok_or_else(|| format!("perfil desconocido: '{profile}'"))?;
    let key_path = opt(args, "key").ok_or("hace falta --key (fichero 0600 con las claves)")?;
    let keys = KeyMaterial::read_secure(&PathBuf::from(&key_path))?;
    let container = PathBuf::from(opt(args, "container").unwrap_or_else(|| {
        format!(
            "/data/local/tmp/megadaemon/inventory/{}.mdbinv",
            level.profile()
        )
    }));
    let dest = PathBuf::from(
        opt(args, "dest")
            .unwrap_or_else(|| "/data/local/tmp/megadaemon/restore".to_string()),
    );

    let mut selection = RestoreSelection {
        packages: opt(args, "app")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default(),
        levels: Vec::new(),
    };
    if let Some(p) = opt(args, "perfil") {
        selection.levels = parse_levels(&p)?
            .iter()
            .map(|l| l.id().to_string())
            .collect();
    }
    if selection.packages.is_empty() && selection.levels.is_empty() {
        selection.levels = vec![level.id().to_string()];
    }

    let opts = RestoreOptions {
        dest,
        selection,
        confirm: flag(args, "confirm"),
        overwrite: flag(args, "overwrite"),
        verify_after: true,
    };
    let rep = restore(&container, &keys, &opts)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "simulacro": rep.dry_run,
            "ok": rep.ok,
            "escritos": rep.written.len(),
            "saltados": rep.skipped.len(),
            "fallidos": rep.failed,
            "bytes": rep.bytes,
            "detalle": rep.detail,
            "aviso": if rep.dry_run { "SIN --confirm no se ha escrito NADA" } else { "" },
        }))
        .map_err(|e| e.to_string())?
    );
    if !rep.ok {
        return Err("el restore ha fallado".to_string());
    }
    Ok(())
}

/// Comando `consolidate`.
fn cmd_consolidate(args: &[String]) -> Result<(), String> {
    let dev = RealDevice::new();
    let pkgs = dev.packages();
    // Bytes por paquete: un `du` por paquete es caro; se usa el inventario.
    let opts = ScanOptions {
        levels: vec![Level::P2],
        dry_run: true,
        progress: false,
        ..ScanOptions::default()
    };
    let mut obs = SilentObserver;
    let report = scan(&dev, &opts, &mut obs);
    let rep: ConsolidationReport = consolidate(&pkgs, &report.entries);

    if flag(args, "json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&rep).map_err(|e| e.to_string())?
        );
    } else {
        println!("CONSOLIDACIÓN  ({} paquetes, {} clasificados)", rep.packages_analyzed, rep.packages_classified);
        println!("------------------------------------------------------------------");
        for f in &rep.families {
            println!(
                "  {:<34} {:>3} apps  {:?}",
                f.title,
                f.packages.len(),
                f.consolidation
            );
            if let Some(n) = f.native {
                println!("        equivalente nativo: {n}");
            }
            println!("        {}", f.note);
            if f.packages.len() > 1 {
                println!("        apps: {}", f.packages.join(", "));
            }
        }
        println!("------------------------------------------------------------------");
        println!("  primer corte (propuesta, NO se ejecuta):");
        for it in rep.first_cut.iter().take(40) {
            println!("    - {}  ({} bytes)  {}", it.package, it.data_bytes, it.action);
        }
        if rep.first_cut.len() > 40 {
            println!("    ... y {} más", rep.first_cut.len() - 40);
        }
        println!("  apps del sistema sin datos privados: {}", rep.system_without_data.len());
        println!();
        println!("  LÍMITE: las apps comerciales con licencia y las de cracking/DRM se AUDITAN,");
        println!("  no se sustituyen ni se parchean. Aquí la línea es análisis y auditoría.");
    }
    if let Some(out) = opt(args, "out") {
        std::fs::write(
            &out,
            serde_json::to_vec_pretty(&rep).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("no se puede escribir {out}: {e}"))?;
        eprintln!("informe de consolidación escrito en {out}");
    }
    Ok(())
}

// Silencia el aviso de "campo nunca leído" en `InventoryReport` cuando el
// binario sólo usa el JSON.
#[allow(dead_code)]
fn _unused(r: &InventoryReport) -> usize {
    r.entries.len()
}

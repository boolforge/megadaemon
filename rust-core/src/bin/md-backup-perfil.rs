//! `md-backup-perfil` — segunda pieza del respaldo masivo: **perfil** y
//! **raw**, con subida a Drive en streaming.
//!
//! Complementa a `md-backup` (que hace la primera pieza: APKs). Este binario
//! NO duplica su motor: usa el mismo [`megadaemon_core::backup_engine`] para
//! el contenedor local y el mismo [`megadaemon_core::backup_stream`] para la
//! subida.
//!
//! ```text
//! md-backup-perfil plan       --root / --profile P1 [--packages a,b] [--json]
//! md-backup-perfil capture    --root / --profile P1 --out <contenedor.mdbk>
//!                             [--password <pw>] [--json]
//! md-backup-perfil upload     --root / --profile P1 --name <n>
//!                             [--folder <id>] [--workers 3] [--chunk-bytes N]
//!                             [--state-dir <dir>] [--simulate] [--json]
//! md-backup-perfil parts      [--root /] [--json]
//! md-backup-perfil raw-plan   --root / [--partition userdata] [--json]
//! md-backup-perfil raw-dump   --device /dev/block/by-name/userdata
//!                             --name <n> [--max-bytes N] [--workers 3]
//!                             [--state-dir <dir>] [--simulate] [--json]
//! md-backup-perfil raw-verify --device <dev> --report <manifiesto.json>
//! ```
//!
//! Reglas:
//! * Nada se **borra** ni se **formatea**: el raw es sólo lectura.
//! * El raw **avisa del tamaño** antes de empezar (225 GB no caben en 10 GB).
//! * `--simulate` no toca la red: escribe los tramos en local.

use megadaemon_core::backup_profile as bp;
use megadaemon_core::backup_raw as br;
use megadaemon_core::backup_stream as bs;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

fn opt(name: &str) -> Option<String> {
    let a = args();
    let mut i = 0;
    while i < a.len() {
        if a[i] == name && i + 1 < a.len() {
            return Some(a[i + 1].clone());
        }
        if let Some(v) = a[i].strip_prefix(&format!("{name}=")) {
            return Some(v.to_string());
        }
        i += 1;
    }
    None
}

fn opt_all(name: &str) -> Vec<String> {
    let a = args();
    let mut out = Vec::new();
    let mut i = 0;
    while i < a.len() {
        if a[i] == name && i + 1 < a.len() {
            out.push(a[i + 1].clone());
            i += 2;
            continue;
        }
        if let Some(v) = a[i].strip_prefix(&format!("{name}=")) {
            out.push(v.to_string());
        }
        i += 1;
    }
    out
}

fn flag(name: &str) -> bool {
    args().iter().any(|a| a == name)
}

fn csv(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for v in opt_all(name) {
        for part in v.split(',') {
            let p = part.trim();
            if !p.is_empty() {
                out.push(p.to_string());
            }
        }
    }
    out
}

fn root_dir() -> String {
    opt("--root").unwrap_or_else(|| "/".to_string())
}

fn json_out() -> bool {
    flag("--json")
}

fn print_json(v: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn human(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.2} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn profile_from_args() -> bp::BackupProfile {
    match opt("--profile")
        .unwrap_or_else(|| "P1".to_string())
        .to_ascii_uppercase()
        .as_str()
    {
        "P0" => bp::BackupProfile::P0AppsData,
        "P1" => bp::BackupProfile::P1Profile,
        "P2" => bp::BackupProfile::P2Sdcard,
        "P3" => bp::BackupProfile::P3Raw,
        _ => bp::BackupProfile::P1Profile,
    }
}

fn policy_from_args() -> bp::ProfilePolicy {
    let mut p = bp::ProfilePolicy::for_profile(profile_from_args());
    p.include_packages = csv("--packages");
    p.exclude_packages = csv("--exclude");
    for pat in opt_all("--exclude-pattern") {
        p.exclude_patterns.push(pat);
    }
    if flag("--include-cache") {
        p.include_cache = true;
    }
    if flag("--include-system-blobs") {
        p.include_system_blobs = true;
    }
    if let Some(v) = opt("--max-bytes") {
        p.max_total_bytes = v.parse().unwrap_or(0);
    }
    if let Some(v) = opt("--max-file-bytes") {
        p.max_entry_bytes = v.parse().unwrap_or(0);
    }
    if let Some(v) = opt("--max-files") {
        p.max_files = v.parse().unwrap_or(0);
    }
    p
}

fn chunk_bytes() -> u64 {
    let v = opt("--chunk-bytes")
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(bs::DEFAULT_CHUNK_BYTES);
    // Drive exige múltiplos de 256 KiB.
    let g = bs::DRIVE_GRANULARITY;
    if v % g != 0 {
        return g;
    }
    v
}

fn workers() -> usize {
    let v = opt("--workers")
        .and_then(|x| x.parse::<usize>().ok())
        .unwrap_or(bs::DEFAULT_CONNECTIONS);
    v.clamp(1, 8)
}

// ---------------------------------------------------------------------------
// plan
// ---------------------------------------------------------------------------

fn cmd_plan() -> ExitCode {
    let policy = policy_from_args();
    let plan = bp::plan(&root_dir(), &policy);
    if json_out() {
        let mut j = plan.to_json();
        if flag("--hash") {
            // Los SHA-256 reales de cada fichero (para la prueba en el móvil).
            if let Ok(files) = bp::hash_files(&plan) {
                j["files"] = serde_json::to_value(&files).unwrap_or(json!([]));
            }
        }
        print_json(&j);
        return ExitCode::SUCCESS;
    }
    println!(
        "perfil   : {} ({})",
        policy.profile.as_str(),
        policy.profile.title()
    );
    println!("raíz     : {}", root_dir());
    println!("paquetes : {}", plan.packages);
    println!(
        "ficheros : {} de {} ({} dentro)",
        plan.included().count(),
        plan.entries.len(),
        human(plan.included_bytes)
    );
    println!(
        "bytes    : {} dentro / {} vistos / {} fuera",
        human(plan.included_bytes),
        human(plan.total_bytes),
        human(plan.excluded_bytes)
    );
    println!("por cubo :");
    for (k, (n, b)) in &plan.by_bucket {
        println!("  {k:<14} {n:>7} ficheros  {:>12}", human(*b));
    }
    for w in &plan.warnings {
        println!("AVISO: {w}");
    }
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// capture (contenedor local)
// ---------------------------------------------------------------------------

fn cmd_capture() -> ExitCode {
    let out = match opt("--out") {
        Some(o) => o,
        None => {
            eprintln!("falta --out <contenedor.mdbk>");
            return ExitCode::FAILURE;
        }
    };
    let policy = policy_from_args();
    let plan = bp::plan(&root_dir(), &policy);
    let mut opts = megadaemon_core::backup_engine::CaptureOptions::default();
    if let Some(pw) = opt("--password") {
        opts.passphrase = Some(pw);
    }
    match bp::capture_to_container(&plan, &out, &opts) {
        Ok(res) => {
            if json_out() {
                print_json(&json!({
                    "ok": true,
                    "container": res.container,
                    "bytes": res.bytes,
                    "encrypted": res.encrypted,
                    "payload_sha256": res.payload_sha256,
                    "entries": res.manifest.entries.len(),
                    "plan": plan.to_json(),
                }));
            } else {
                println!("contenedor : {}", res.container);
                println!("bytes      : {}", human(res.bytes));
                println!("cifrado    : {}", res.encrypted);
                println!("sha256     : {}", res.payload_sha256);
                println!("entradas   : {}", res.manifest.entries.len());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("no se pudo capturar: {e}");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// upload (streaming a Drive)
// ---------------------------------------------------------------------------

fn cmd_upload() -> ExitCode {
    let name = opt("--name").unwrap_or_else(|| "perfil".to_string());
    let policy = policy_from_args();
    let plan = bp::plan(&root_dir(), &policy);
    let simulate = flag("--simulate") || flag("--dry-run");

    // 1) Tamaño exacto del TAR (Drive necesita el total ANTES de subir).
    let files = match bp::hash_files(&plan) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("no se pudo hashear el perfil: {e}");
            return ExitCode::FAILURE;
        }
    };
    let manifest_json = serde_json::to_string(&files).unwrap_or_default();
    let sizes: Vec<u64> = files.iter().map(|f| f.size).collect();
    let total = bp::tar_total_bytes(manifest_json.len() as u64, &sizes);

    if !json_out() {
        eprintln!(
            "perfil {} · {} ficheros · {} en total",
            policy.profile.as_str(),
            files.len(),
            human(total)
        );
    }

    // 2) Destino.
    let folder = opt("--folder").unwrap_or_default();
    let state_dir = opt("--state-dir");
    let state_path: Option<PathBuf> = state_dir
        .as_ref()
        .map(|d| PathBuf::from(d).join(format!("md_backup_stream_{}.json", name)));
    let sink: Arc<dyn bs::ChunkSink> = if simulate {
        let dir = opt("--out-dir").unwrap_or_else(|| "/tmp/md-backup-sim".to_string());
        Arc::new(
            bs::FileSink::new(Path::new(&dir), &name).expect("no se pudo preparar el destino local"),
        )
    } else {
        Arc::new(bs::DriveSink::new(
            &name,
            megadaemon_core::drive_client::DriveTarget::folder(&folder),
            state_dir.clone(),
            state_path.clone(),
            total,
        ))
    };

    // 3) Reanudación: los trozos ya subidos se saltan.
    let done = state_path
        .as_ref()
        .and_then(|p| bs::UploadState::load(p))
        .map(|s| s.done())
        .unwrap_or_default();

    let mut pipeline = match bs::Pipeline::new(
        sink.clone(),
        &name,
        total,
        chunk_bytes(),
        workers(),
        done,
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("no se pudo abrir la tubería: {e}");
            return ExitCode::FAILURE;
        }
    };

    // 4) Progreso (el usuario quiere VER que avanza).
    let reporter = bs::ProgressReporter::start(pipeline.progress(), Duration::from_secs(2));

    // 5) Stream: se sube mientras se genera.
    let mut manifest = match bp::stream_to_drive(&plan, &mut pipeline, chunk_bytes()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("fallo al generar el stream: {e}");
            return ExitCode::FAILURE;
        }
    };
    let summary = match pipeline.finish() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("fallo al subir: {e}");
            return ExitCode::FAILURE;
        }
    };
    drop(reporter);
    manifest.sha256 = summary.sha256.clone();
    manifest.parts = summary
        .names
        .iter()
        .zip(summary.file_ids.iter())
        .enumerate()
        .map(|(i, (n, id))| bp::PartRecord {
            stream: i,
            name: n.clone(),
            file_id: id.clone(),
            bytes: 0,
        })
        .collect();

    // 6) El manifiesto final (el que ata todo).
    let manifest_path = PathBuf::from(format!("{name}.manifest.json"));
    if let Err(e) = bp::write_manifest(&manifest_path, &manifest) {
        eprintln!("aviso: no se pudo escribir el manifiesto local: {e}");
    }
    if !simulate {
        let up = megadaemon_core::drive_native::upload_file(
            &manifest_path,
            &format!("{name}.manifest.json"),
            &megadaemon_core::drive_client::DriveTarget::folder(&folder),
            state_dir.as_deref(),
        );
        match up {
            Ok(u) => eprintln!("manifiesto subido: {} ({})", u.name, u.file_id),
            Err(e) => eprintln!("aviso: no se pudo subir el manifiesto: {e}"),
        }
    }

    if json_out() {
        print_json(&json!({
            "ok": true,
            "simulate": simulate,
            "upload": bs::summary_json(&summary),
            "manifest": manifest.to_json(),
        }));
    } else {
        println!("subido     : {}", human(summary.bytes));
        println!("trozos     : {}", summary.chunks);
        println!("conexiones : {}", summary.streams);
        println!("sha256     : {}", summary.sha256);
        for (n, id) in summary.names.iter().zip(summary.file_ids.iter()) {
            println!("  {n} → {id}");
        }
    }
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------------
// parts / raw
// ---------------------------------------------------------------------------

fn cmd_parts() -> ExitCode {
    let parts = br::list_partitions(&root_dir());
    if json_out() {
        print_json(&json!({
            "ok": true,
            "partitions": parts.iter().map(|p| p.to_json()).collect::<Vec<_>>(),
        }));
        return ExitCode::SUCCESS;
    }
    println!(
        "{:<14} {:<22} {:>12} {:<10} {}",
        "NOMBRE", "DISPOSITIVO", "TAMAÑO", "FS", "MONTADA"
    );
    for p in &parts {
        println!(
            "{:<14} {:<22} {:>12} {:<10} {}",
            p.name,
            p.device,
            human(p.size_bytes),
            p.fs.clone().unwrap_or_else(|| "-".into()),
            p.mount.clone().unwrap_or_else(|| "-".into())
        );
    }
    ExitCode::SUCCESS
}

/// Espacio libre del sitio donde viven los datos del usuario.
///
/// En el móvil, `/` es la partición de sistema (sólo lectura): los datos
/// del usuario están en `/data`, así que se pregunta ahí primero.
fn free_bytes(root: &str) -> u64 {
    if Path::new("/data").exists() {
        let f = br::free_space("/data");
        if f > 0 {
            return f;
        }
    }
    br::free_space(root)
}

fn cmd_raw_plan() -> ExitCode {
    let sel = csv("--partition");
    let plan = br::plan_raw(&root_dir(), &sel, free_bytes(&root_dir()));
    if json_out() {
        print_json(&plan.to_json());
        return ExitCode::SUCCESS;
    }
    println!("particiones : {}", plan.partitions.len());
    println!("total       : {}", human(plan.total_bytes));
    for p in &plan.partitions {
        println!(
            "  {:<14} {:<22} {:>12}  fs={}  montada={}",
            p.name,
            p.device,
            human(p.size_bytes),
            p.fs.clone().unwrap_or_else(|| "-".into()),
            p.mount.clone().unwrap_or_else(|| "-".into())
        );
    }
    for w in &plan.warnings {
        println!("AVISO: {w}");
    }
    ExitCode::SUCCESS
}

fn cmd_raw_dump() -> ExitCode {
    let root = root_dir();
    let device = match opt("--device") {
        Some(d) => d,
        None => {
            // Se puede elegir por nombre lógico (`userdata`).
            let sel = csv("--partition");
            let plan = br::plan_raw(&root, &sel, 0);
            match plan.partitions.first() {
                Some(p) => p.device.clone(),
                None => {
                    eprintln!("falta --device (o --partition). Usa `parts` para ver los nombres.");
                    return ExitCode::FAILURE;
                }
            }
        }
    };
    let name = opt("--name").unwrap_or_else(|| "raw".to_string());
    let simulate = flag("--simulate") || flag("--dry-run");
    let max_bytes = opt("--max-bytes")
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(0);
    let dev_path = PathBuf::from(&device);
    let size = std::fs::metadata(&dev_path).map(|m| m.len()).unwrap_or(0);
    if size == 0 {
        eprintln!("no se puede leer el tamaño de {device}");
        return ExitCode::FAILURE;
    }
    let total = if max_bytes > 0 { max_bytes.min(size) } else { size };

    // AVISO de tamaño (requisito: 225 GB no caben en 10 GB libres).
    let free = free_bytes(&root);
    if free > 0 && total > free {
        eprintln!(
            "AVISO: {} no cabe en el móvil ({} libres): el destino tiene que ser Drive.",
            human(total),
            human(free)
        );
    }

    let folder = opt("--folder").unwrap_or_default();
    let state_dir = opt("--state-dir");
    let state_path: Option<PathBuf> = state_dir
        .as_ref()
        .map(|d| PathBuf::from(d).join(format!("md_backup_stream_{}.json", name)));
    let sink: Arc<dyn bs::ChunkSink> = if simulate {
        let dir = opt("--out-dir").unwrap_or_else(|| "/tmp/md-backup-sim".to_string());
        Arc::new(
            bs::FileSink::new(Path::new(&dir), &name).expect("no se pudo preparar el destino local"),
        )
    } else {
        Arc::new(bs::DriveSink::new(
            &name,
            megadaemon_core::drive_client::DriveTarget::folder(&folder),
            state_dir.clone(),
            state_path.clone(),
            total,
        ))
    };
    let done = state_path
        .as_ref()
        .and_then(|p| bs::UploadState::load(p))
        .map(|s| s.done())
        .unwrap_or_default();
    let mut pipeline = match bs::Pipeline::new(sink, &name, total, chunk_bytes(), workers(), done) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("no se pudo abrir la tubería: {e}");
            return ExitCode::FAILURE;
        }
    };
    let reporter = bs::ProgressReporter::start(pipeline.progress(), Duration::from_secs(2));
    let opts = br::RawOptions {
        device: device.clone(),
        chunk_bytes: chunk_bytes(),
        max_bytes,
        label: name.clone(),
    };
    let report = match br::dump_to_stream(&dev_path, &mut pipeline, &opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("fallo al volcar: {e}");
            return ExitCode::FAILURE;
        }
    };
    let summary = match pipeline.finish() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("fallo al subir: {e}");
            return ExitCode::FAILURE;
        }
    };
    drop(reporter);

    let report_path = PathBuf::from(format!("{name}.raw.json"));
    if let Err(e) = br::write_report(&report_path, &report) {
        eprintln!("aviso: no se pudo escribir el manifiesto local: {e}");
    }
    if !simulate {
        let up = megadaemon_core::drive_native::upload_file(
            &report_path,
            &format!("{name}.raw.json"),
            &megadaemon_core::drive_client::DriveTarget::folder(&folder),
            state_dir.as_deref(),
        );
        match up {
            Ok(u) => eprintln!("manifiesto raw subido: {} ({})", u.name, u.file_id),
            Err(e) => eprintln!("aviso: no se pudo subir el manifiesto raw: {e}"),
        }
    }

    if json_out() {
        print_json(&json!({
            "ok": true,
            "simulate": simulate,
            "report": report.to_json(),
            "upload": bs::summary_json(&summary),
        }));
    } else {
        println!("dispositivo : {}", report.device);
        println!("fs          : {}", report.fs.clone().unwrap_or_else(|| "-".into()));
        println!("bytes       : {}", human(report.bytes));
        println!("bloques     : {}", report.chunks.len());
        println!("sha256      : {}", report.sha256);
        for (n, id) in summary.names.iter().zip(summary.file_ids.iter()) {
            println!("  {n} → {id}");
        }
    }
    ExitCode::SUCCESS
}

fn cmd_raw_verify() -> ExitCode {
    let device = match opt("--device") {
        Some(d) => d,
        None => {
            eprintln!("falta --device");
            return ExitCode::FAILURE;
        }
    };
    let report_path = match opt("--report") {
        Some(r) => r,
        None => {
            eprintln!("falta --report <manifiesto.json>");
            return ExitCode::FAILURE;
        }
    };
    let data = match std::fs::read(&report_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("no se pudo leer {report_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let report: br::RawReport = match serde_json::from_slice(&data) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("manifiesto inválido: {e}");
            return ExitCode::FAILURE;
        }
    };
    match br::verify_raw(Path::new(&device), &report) {
        Ok(v) => {
            if json_out() {
                print_json(&json!({"ok": v.ok, "verify": v}));
            } else {
                println!("bloques comprobados : {}", v.blocks_checked);
                println!("bytes releídos      : {}", human(v.bytes));
                println!("resultado           : {}", if v.ok { "OK" } else { "FALLO" });
                for b in &v.blocks_bad {
                    println!("  {b}");
                }
            }
            if v.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("fallo al verificar: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!(
        "md-backup-perfil — perfil (config, certificados, navegadores) y raw de particiones\n\
         \n\
         uso:\n\
         \x20 md-backup-perfil plan       --root / --profile P1 [--packages a,b] [--json]\n\
         \x20 md-backup-perfil capture    --root / --profile P1 --out <contenedor.mdbk> [--password <pw>]\n\
         \x20 md-backup-perfil upload     --root / --profile P1 --name <n> [--folder <id>] [--workers 3]\n\
         \x20 md-backup-perfil parts      [--root /] [--json]\n\
         \x20 md-backup-perfil raw-plan   --root / [--partition userdata] [--json]\n\
         \x20 md-backup-perfil raw-dump   --device /dev/block/by-name/userdata --name <n> [--max-bytes N]\n\
         \x20 md-backup-perfil raw-verify --device <dev> --report <manifiesto.json>\n\
         \n\
         Flags comunes: --simulate (sin red) · --chunk-bytes N · --state-dir <dir> · --json"
    );
}

fn main() -> ExitCode {
    let a = args();
    let cmd = a.first().cloned().unwrap_or_default();
    match cmd.as_str() {
        "plan" => cmd_plan(),
        "capture" => cmd_capture(),
        "upload" => cmd_upload(),
        "parts" => cmd_parts(),
        "raw-plan" => cmd_raw_plan(),
        "raw-dump" => cmd_raw_dump(),
        "raw-verify" => cmd_raw_verify(),
        _ => {
            usage();
            ExitCode::FAILURE
        }
    }
}

//! `md-backup` — motor de **backup nativo de apps** con **subida pipelining**.
//!
//! Subcomandos:
//!
//! ```text
//! md-backup inventory  [--root /] [--packages a,b] [--measure] [--json] [--limit N]
//! md-backup plan       --out DIR [--root /] [--model megadaemon] [--max-apps N]
//!                     [--allow-live-framework] [--json]
//! md-backup create    --out DIR [--packages a,b] [--root /] [--segment-bytes N]
//!                     [--password P] [--key-out F] [--no-apk] [--no-data]
//!                     [--include-external] [--include-media] [--exclude GLOB]
//!                     [--plan F] [--dry-run] [--nice N] [--ionice C] [--load-high F]
//!                     [--sleep-ms N] [--batch N]
//! md-backup list      --out DIR [--json]
//! md-backup verify    DIR [--key F] [--json]
//! md-backup restore   DIR [--key F] [--package P] [--dest D] [--confirm] [--overwrite]
//! md-backup upload    --out DIR --folder ID [--create] [--packages a,b]
//!                     [--workers N] [--chunk-bytes N] [--state-dir S] [--name N]
//!                     [--delete-after-upload] [--dry-run] [--json]
//! md-backup fetch     --folder ID --name N --dest DIR [--state-dir S]
//!                     (baja el contenedor de Drive y verifica cada trozo)
//! md-backup bench-upload --file F --folder ID [--workers N] [--chunk-bytes N]
//! ```
//!
//! Reglas que **no** se negocian:
//!
//! * `restore` **sin `--confirm` no escribe nada** (el defecto es el simulacro).
//! * `upload` **sube mientras recoge**: el productor entrega cada segmento
//!   cerrado y el consumidor lo sube en paralelo; se miden MiB/s y ETA.
//! * Todo el proceso va con `nice`/`ionice` de la clase más baja, duerme
//!   entre lotes y **descansa si la carga del sistema se dispara**.
//! * Ningún secreto va al repo: la credencial de Drive se lee del
//!   `state_dir` y la clave del contenedor, de un fichero `0600`.

use megadaemon_core::backup_apps::{
    self, CreateOptions, DriveUploader, Governor, InventoryOptions, RestoreOptions, SegmentSink,
};
use megadaemon_core::backup_plan;
use serde_json::json;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

/// Estado por defecto del daemon en el móvil (fuera del repo).
const DEFAULT_STATE_DIR: &str = "/data/local/tmp/megadaemon/state";

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

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn arg_usize(name: &str, default: usize) -> usize {
    arg(name)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn arg_u64(name: &str, default: u64) -> u64 {
    arg(name)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn arg_f64(name: &str, default: f64) -> f64 {
    arg(name)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}

fn arg_csv(name: &str) -> Vec<String> {
    arg(name)
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// El gobierno anti-congelación desde la línea de órdenes.
fn governor_from_args() -> Governor {
    let mut gov = Governor::default();
    if let Some(v) = arg("--nice") {
        if let Ok(n) = v.parse::<i32>() {
            gov.nice = n;
        }
    }
    if let Some(v) = arg("--ionice") {
        if let Ok(c) = v.parse::<u8>() {
            gov.ionice_class = c;
        }
    }
    if let Some(v) = arg("--batch") {
        if let Ok(n) = v.parse::<usize>() {
            gov.batch_size = n.max(1);
        }
    }
    if let Some(v) = arg("--sleep-ms") {
        if let Ok(n) = v.parse::<u64>() {
            gov.sleep_ms = n;
        }
    }
    if let Some(v) = arg("--load-high") {
        if let Ok(f) = v.parse::<f64>() {
            gov.load_high = f;
            gov.load_resume = (f * 0.5).max(0.5);
        }
    }
    if let Some(v) = arg("--max-pause-s") {
        if let Ok(n) = v.parse::<u64>() {
            gov.max_pause_s = n;
        }
    }
    if flag("--no-throttle") {
        gov.enabled = false;
    }
    gov
}

fn usage() -> ExitCode {
    eprintln!(
        "uso:\n  \
md-backup inventory  [--root /] [--packages a,b] [--measure] [--json] [--limit N]\n  \
md-backup plan       --out DIR [--root /] [--model M] [--max-apps N] [--allow-live-framework]\n  \
md-backup create    --out DIR [--packages a,b] [--segment-bytes N] [--password P] [--key-out F] [--plan F]\n  \
md-backup list      --out DIR [--json]\n  \
md-backup verify    DIR [--key F] [--json]\n  \
md-backup restore   DIR [--key F] [--package P] [--dest D] [--confirm] [--overwrite]\n  \
md-backup upload    --out DIR --folder ID [--create] [--workers N] [--chunk-bytes N]\n  \
md-backup fetch     --folder ID --name N --dest DIR [--state-dir S]\n  \
md-backup bench-upload --file F --folder ID [--workers N] [--chunk-bytes N]"
    );
    ExitCode::from(2)
}

pub fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let sub = args.get(1).cloned().unwrap_or_default();
    let r = match sub.as_str() {
        "inventory" => cmd_inventory(),
        "plan" => cmd_plan(),
        "create" => cmd_create(),
        "list" => cmd_list(),
        "verify" => cmd_verify(),
        "restore" => cmd_restore(),
        "upload" => cmd_upload(),
        "fetch" => cmd_fetch(),
        "bench-upload" => cmd_bench_upload(),
        _ => {
            usage();
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(code) => code,
        Err(e) => {
            eprintln!("md-backup: {e}");
            ExitCode::from(1)
        }
    }
}

// --------------------------------------------------------------------------
// inventory
// --------------------------------------------------------------------------

fn cmd_inventory() -> Result<ExitCode, String> {
    let root = PathBuf::from(arg("--root").unwrap_or_else(|| "/".to_string()));
    let opts = InventoryOptions {
        root,
        packages: arg_csv("--packages"),
        measure: flag("--measure"),
        max_depth: arg_usize("--max-depth", 8),
        limit: arg_usize("--limit", 0),
        include_disabled: !flag("--no-disabled"),
    };
    let apps = backup_apps::inventory(&opts)?;
    if flag("--json") {
        let out = json!({
            "root": opts.root.to_string_lossy(),
            "packages": apps.len(),
            "measured": opts.measure,
            "apps": apps,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    } else {
        println!(
            "{:<45} {:>12} {:>8} {:>12} {:>6}",
            "PAQUETE", "DATOS", "FICHEROS", "APK", "UID"
        );
        for a in &apps {
            println!(
                "{:<45} {:>12} {:>8} {:>12} {:>6}",
                a.package,
                a.data_bytes
                    .map(|b| human_bytes(b))
                    .unwrap_or_else(|| "-".to_string()),
                a.file_count
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                human_bytes(apk_size(a)),
                a.uid
            );
        }
        println!("\n{} apps (sin escribir nada en disco)", apps.len());
    }
    Ok(ExitCode::SUCCESS)
}

fn apk_size(a: &backup_apps::AppEntry) -> u64 {
    if let Ok(md) = std::fs::metadata(&a.apk_path) {
        return md.len();
    }
    0
}

fn human_bytes(n: u64) -> String {
    const U: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut i = 0usize;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.2} {}", U[i])
    }
}

// --------------------------------------------------------------------------
// plan: el LLM propone, el código valida
// --------------------------------------------------------------------------

/// `md-backup plan`: pide al gateway `megadaemon` una propuesta de plan
/// (qué respaldar / qué excluir) a partir del **inventario real** del móvil
/// y la **valida en código**.
///
/// Lo que se guarda (`plan.json`) es la **decisión validada** con la
/// propuesta cruda dentro: la IA no decide nunca.
fn cmd_plan() -> Result<ExitCode, String> {
    let out = PathBuf::from(arg("--out").ok_or("plan necesita --out <dir>")?);
    let root = PathBuf::from(arg("--root").unwrap_or_else(|| "/".to_string()));
    let model = arg("--model").unwrap_or_else(|| "megadaemon".to_string());
    let max_apps = arg_usize("--max-apps", 200);
    let human_gate = flag("--allow-live-framework");

    // 1. Inventario REAL (medido): el modelo no puede inventar lo que no
    //    está en el móvil.
    let opts = InventoryOptions {
        root: root.clone(),
        packages: arg_csv("--packages"),
        measure: flag("--measure"),
        limit: 0,
        ..Default::default()
    };
    let apps = backup_apps::inventory(&opts)?;
    let real_packages: Vec<String> = apps.iter().map(|a| a.package.clone()).collect();

    // 2. La propuesta del LLM (puede fallar: sin red o sin proveedor).
    let (proposal, llm_error) = match backup_plan::propose_with_llm(&apps, &model, max_apps) {
        Ok(p) => (p, None),
        Err(e) => (
            backup_plan::PlanProposal {
                rationale: "el LLM no respondió: se aplica el plan por defecto".to_string(),
                raw: String::new(),
                ..Default::default()
            },
            Some(e),
        ),
    };

    // 3. Validación EN CÓDIGO (la IA no manda).
    let decision = backup_plan::validate(&proposal, &real_packages, human_gate);

    // 4. Se guarda el plan (decisión + propuesta cruda).
    std::fs::create_dir_all(&out).map_err(|e| format!("no se puede crear {}: {e}", out.display()))?;
    let plan_path = backup_plan::plan_path(&out);
    backup_plan::write_plan(&plan_path, &decision)?;

    if flag("--json") {
        let mut doc = serde_json::to_value(&decision).unwrap_or_default();
        if let Some(e) = &llm_error {
            doc["llm_error"] = Value::String(e.clone());
        }
        println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
    } else {
        println!("fichero del plan : {}", plan_path.display());
        println!("modelo           : {model}");
        if let Some(e) = &llm_error {
            println!("LLM              : ERROR ({e}) ⇒ plan por defecto");
        } else {
            println!("LLM              : propuesta recibida");
        }
        println!("paquetes aceptados ({}):", decision.packages.len());
        for p in &decision.packages {
            println!("  · {p}");
        }
        println!("exclusiones aceptadas ({}):", decision.exclude_patterns.len());
        for p in &decision.exclude_patterns {
            println!("  · {p}");
        }
        println!(
            "media/data        : media={} data={} (puerta humana: {})",
            decision.include_media, decision.include_external, human_gate
        );
        println!("rechazos ({}):", decision.rejections.len());
        for r in &decision.rejections {
            println!("  · {} — {}", r.what, r.reason);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Lee el plan validado de `--plan` (si se pasó).
fn load_plan() -> Result<Option<backup_plan::PlanDecision>, String> {
    match arg("--plan") {
        Some(p) if !p.trim().is_empty() => {
            let d = backup_plan::read_plan(&PathBuf::from(p))?;
            Ok(Some(d))
        }
        _ => Ok(None),
    }
}

// --------------------------------------------------------------------------
// create
// --------------------------------------------------------------------------

fn cmd_create() -> Result<ExitCode, String> {
    let out = PathBuf::from(arg("--out").ok_or("create necesita --out <dir>")?);
    let root = PathBuf::from(arg("--root").unwrap_or_else(|| "/".to_string()));
    let gov = governor_from_args();
    let plan = load_plan()?;
    let opts = CreateOptions {
        root,
        out: out.clone(),
        packages: arg_csv("--packages"),
        segment_bytes: arg_u64("--segment-bytes", backup_apps::DEFAULT_SEGMENT_BYTES),
        passphrase: arg("--password").or_else(|| std::env::var("MD_BACKUP_PASSWORD").ok()),
        key_out: arg("--key-out").map(PathBuf::from),
        include_apk: !flag("--no-apk"),
        include_data: !flag("--no-data"),
        include_external: flag("--include-external"),
        include_media: flag("--include-media"),
        allow_live_framework: flag("--allow-live-framework"),
        exclude_patterns: arg_csv("--exclude"),
        plan,
        dry_run: flag("--dry-run"),
        gov,
    };
    let mut sink = backup_apps::NullSink;
    let report = backup_apps::create(&opts, &mut sink)?;
    if flag("--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        println!("contenedor : {}", report.container);
        println!("paquetes   : {}", report.packages.join(", "));
        println!("ficheros   : {} ({} bytes)", report.files, report.bytes);
        println!("segmentos  : {}", report.segments);
        println!("excluidos  : {} ficheros ({} bytes)", report.excluded_files, report.excluded_bytes);
        println!("manifiesto : sha256 {}", report.manifest_sha256);
        println!("carga      : {} -> {}", report.loadavg_before, report.loadavg_after);
        println!(
            "lectura    : {:.3} MiB/s ({})",
            report.read_mib_per_s,
            human_bytes(report.read_bytes)
        );
        println!(
            "prohibidos : {} ficheros ({} bytes) - CSC y Android/media|data sin puerta",
            report.forbidden_files, report.forbidden_bytes
        );
        println!(
            "anti-congelación: {} · pausas {} ms · sleep {} ms · {} ms totales",
            report.io_limits, report.paused_ms, report.slept_ms, report.elapsed_ms
        );
        if !report.plan_sha256.is_empty() {
            println!("plan (LLM+validación): sha256 {}", report.plan_sha256);
            if let Some(p) = &report.plan {
                if let Some(s) = p.get("summary") {
                    println!("  resumen        : {}", serde_json::to_string(s).unwrap_or_default());
                }
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

// --------------------------------------------------------------------------
// list
// --------------------------------------------------------------------------

fn cmd_list() -> Result<ExitCode, String> {
    let out = PathBuf::from(arg("--out").ok_or("list necesita --out <dir>")?);
    let items = backup_apps::list(&out)?;
    if flag("--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&items).unwrap_or_default()
        );
    } else {
        for it in &items {
            println!(
                "{:<60} {:>8} ficheros {:>12} {:>4} segs  completo={}",
                it.path,
                it.files,
                human_bytes(it.bytes),
                it.segments,
                it.complete
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}

// --------------------------------------------------------------------------
// verify
// --------------------------------------------------------------------------

fn cmd_verify() -> Result<ExitCode, String> {
    let dir = PathBuf::from(arg("--container").or_else(|| arg("--dir")).ok_or(
        "verify necesita la ruta del contenedor (md-backup verify <dir>)",
    )?);
    let key = match arg("--key") {
        Some(p) => Some(backup_apps::read_key_file(&PathBuf::from(p))?),
        None => None,
    };
    let report = backup_apps::verify(&dir, key.as_deref())?;
    if flag("--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        println!("segmentos      : {}", report.segments);
        println!("entradas       : {} ({} bytes)", report.entries, report.bytes);
        println!("manifiesto ok  : {}", report.manifest_ok);
        println!("desajustes     : {}", report.mismatches.len());
        println!("faltantes      : {}", report.missing.len());
        println!("sobrantes      : {}", report.extra.len());
        println!("tiempo         : {} ms", report.elapsed_ms);
        println!("resultado      : {}", if report.ok { "OK" } else { "FALLO" });
        for m in report.mismatches.iter().take(20) {
            println!("  · {m}");
        }
        for m in report.missing.iter().take(20) {
            println!("  · falta: {m}");
        }
        for m in report.extra.iter().take(20) {
            println!("  · sobra: {m}");
        }
    }
    if report.ok {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(3))
    }
}

// --------------------------------------------------------------------------
// restore
// --------------------------------------------------------------------------

fn cmd_restore() -> Result<ExitCode, String> {
    let dir = PathBuf::from(arg("--container").or_else(|| arg("--dir")).ok_or(
        "restore necesita la ruta del contenedor (md-backup restore <dir>)",
    )?);
    let key = match arg("--key") {
        Some(p) => Some(backup_apps::read_key_file(&PathBuf::from(p))?),
        None => None,
    };
    let opts = RestoreOptions {
        dir,
        dest: PathBuf::from(arg("--dest").unwrap_or_else(|| "/".to_string())),
        packages: arg_csv("--package"),
        confirm: flag("--confirm"),
        overwrite: flag("--overwrite"),
        key,
    };
    let report = backup_apps::restore(&opts)?;
    if flag("--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        println!(
            "modo          : {}",
            if report.dry_run { "SIMULACRE (nada escrito)" } else { "REAL" }
        );
        println!("ficheros      : {} ({} bytes)", report.files, report.bytes);
        println!("escritos      : {}", report.written);
        println!("saltados      : {}", report.skipped);
        println!("desajustes    : {}", report.mismatches.len());
        println!("tiempo        : {} ms", report.elapsed_ms);
    }
    if !report.mismatches.is_empty() {
        return Ok(ExitCode::from(3));
    }
    Ok(ExitCode::SUCCESS)
}

// --------------------------------------------------------------------------
// upload (pipelining)
// --------------------------------------------------------------------------

fn cmd_upload() -> Result<ExitCode, String> {
    let out = PathBuf::from(arg("--out").ok_or("upload necesita --out <dir>")?);
    let folder = arg("--folder").ok_or("upload necesita --folder <ID_DE_DRIVE>")?;
    let state_dir = arg("--state-dir").unwrap_or_else(|| DEFAULT_STATE_DIR.to_string());
    // Subcarpeta dentro de la carpeta de Drive (p.ej. `P0-apps`): se
    // **busca o se crea** con el VFS nativo (nada de rclone).
    let folder = match arg("--subfolder") {
        Some(sub) if !sub.trim().is_empty() => {
            megadaemon_core::drive_native::ensure_folder(&sub, &folder, Some(&state_dir))?
        }
        _ => folder,
    };
    let workers = arg_usize("--workers", 3);
    // 1 MiB por defecto (MEDIDO: la subida va a ~297,6 KiB/s ⇒ el trozo no
    // es el cuello; 1 MiB son 16 peticiones por segmento de 16 MiB).
    let chunk_bytes = arg_u64("--chunk-bytes", backup_apps::UPLOAD_CHUNK_BYTES);
    let name = arg("--name").unwrap_or_else(|| {
        out.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "backup".to_string())
    });
    let dry = flag("--dry-run");

    let mut uploader = DriveUploader::new(&folder, &name, workers);
    uploader.chunk_bytes = chunk_bytes;
    uploader.state_dir = Some(state_dir.clone());
    uploader.delete_after_upload = flag("--delete-after-upload");
    uploader.dry_run = dry;
    uploader.container_dir = Some(out.clone());
    uploader.start()?;

    let t0 = Instant::now();
    let mut create_report = None;
    if flag("--create") {
        // **Pipelining de verdad**: se recoge y se sube a la vez.
        let gov = governor_from_args();
        let opts = CreateOptions {
            root: PathBuf::from(arg("--root").unwrap_or_else(|| "/".to_string())),
            out: out.clone(),
            packages: arg_csv("--packages"),
            segment_bytes: arg_u64("--segment-bytes", backup_apps::DEFAULT_SEGMENT_BYTES),
            passphrase: arg("--password").or_else(|| std::env::var("MD_BACKUP_PASSWORD").ok()),
            key_out: arg("--key-out").map(PathBuf::from),
            include_apk: !flag("--no-apk"),
            include_data: !flag("--no-data"),
            include_external: flag("--include-external"),
            include_media: flag("--include-media"),
            allow_live_framework: flag("--allow-live-framework"),
            exclude_patterns: arg_csv("--exclude"),
            plan: load_plan()?,
            dry_run: false,
            gov,
        };
        let rep = backup_apps::create(&opts, &mut uploader)?;
        create_report = Some(rep);
    } else {
        // Subir un contenedor ya existente (reanudable): cada segmento por
        // separado, en paralelo.
        let header = backup_apps::read_header(&out)?;
        for s in &header.segments {
            let p = out.join("segments").join(&s.name);
            if !p.exists() {
                eprintln!("md-backup: falta el segmento {}", s.name);
                continue;
            }
            let seg = backup_apps::SealedSegment {
                index: s.index,
                path: p,
                info: s.clone(),
            };
            uploader.submit(&seg)?;
        }
        let manifest = backup_apps::read_manifest(&out)?;
        uploader.finish(&header, &manifest)?;
    }
    let wall_ms = t0.elapsed().as_millis() as u64;
    let stats = uploader.stats();
    let errors = uploader.errors();

    // Verificación **en el otro extremo**: se descarga un segmento de Drive y
    // se compara su SHA-256 con el del segmento local.
    let mut remote_check: Vec<Value> = Vec::new();
    if flag("--verify-download") {
        let header = backup_apps::read_header(&out)?;
        for (idx, file_id) in uploader.uploaded_segments() {
            let Some(seg) = header.segments.iter().find(|s| s.index == idx) else {
                continue;
            };
            let dest = std::env::temp_dir().join(format!("md-backup-dl-{idx}-{}", std::process::id()));
            let r = megadaemon_core::drive_native::download_file(&file_id, &dest, Some(&state_dir));
            match r {
                Ok(_) => {
                    let sha = megadaemon_core::drive_native::sha256_file(&dest).unwrap_or_default();
                    let _ = fs::remove_file(&dest);
                    remote_check.push(json!({
                        "index": idx,
                        "file_id": file_id,
                        "sha256_descargado": sha,
                        "sha256_local": seg.cipher_sha256,
                        "ok": sha == seg.cipher_sha256,
                    }));
                }
                Err(e) => remote_check.push(json!({
                    "index": idx,
                    "file_id": file_id,
                    "error": e,
                    "ok": false,
                })),
            }
        }
    }
    let up_ms = stats.ended_ms.saturating_sub(stats.started_ms);
    let mibs = backup_apps::mib_per_s(stats.bytes, up_ms.max(1));
    let eta = backup_apps::eta_s(stats.bytes, stats.total_bytes.max(stats.bytes), up_ms.max(1));
    let out_json = json!({
        "ok": errors.is_empty(),
        "container": out.to_string_lossy(),
        "folder_id": folder,
        "workers": workers,
        "chunk_bytes": chunk_bytes,
        "segments": stats.segments,
        "bytes": stats.bytes,
        "total_bytes": stats.total_bytes,
        "upload_ms": up_ms,
        "wall_ms": wall_ms,
        "mib_per_s": (mibs * 100.0).round() / 100.0,
        "eta_s": eta.round() as u64,
        "errors": errors,
        "remote_check": remote_check,
        "create": create_report,
    });
    if flag("--json") {
        println!("{}", serde_json::to_string_pretty(&out_json).unwrap_or_default());
    } else {
        println!("segmentos subidos : {}", stats.segments);
        println!("bytes subidos     : {} ({})", stats.bytes, human_bytes(stats.bytes));
        println!(
            "velocidad         : {:.3} MiB/s (subida pura)",
            mibs
        );
        println!("ETA               : {} s", eta.round() as u64);
        println!("tiempo total      : {} ms", wall_ms);
        println!("errores           : {}", errors.len());
        for e in errors.iter().take(10) {
            println!("  · {e}");
        }
        for rc in &remote_check {
            println!(
                "  · segmento {}: descarga desde Drive {} ({})",
                rc["index"],
                if rc["ok"] == serde_json::json!(true) {
                    "SHA-256 igual"
                } else {
                    "DIFIERE"
                },
                rc.get("file_id").and_then(|v| v.as_str()).unwrap_or("-")
            );
        }
    }
    if errors.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(4))
    }
}

// --------------------------------------------------------------------------
// fetch: baja el contenedor de Drive y lo verifica por SHA-256
// --------------------------------------------------------------------------

fn cmd_fetch() -> Result<ExitCode, String> {
    let folder = arg("--folder").ok_or("fetch necesita --folder <ID_DE_DRIVE>")?;
    let name = arg("--name").ok_or("fetch necesita --name <NOMBRE_DEL_BACKUP>")?;
    let dest = PathBuf::from(arg("--dest").or_else(|| arg("--out")).ok_or(
        "fetch necesita --dest <dir>",
    )?);
    let state_dir = arg("--state-dir").unwrap_or_else(|| DEFAULT_STATE_DIR.to_string());
    let report = backup_apps::fetch_from_drive(&folder, &name, &dest, Some(&state_dir))?;
    if flag("--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        println!("destino    : {}", report.dest);
        println!("segmentos  : {} ({} bytes)", report.segments, human_bytes(report.bytes));
        println!("trozos verificados: {}", report.chunks_verified);
        println!("manifiesto ok: {}", report.manifest_ok);
        println!("desajustes : {}", report.mismatches.len());
        for m in report.mismatches.iter().take(20) {
            println!("  · {m}");
        }
        for f in &report.files {
            println!(
                "  · {} ({})",
                f.get("name").and_then(|v| v.as_str()).unwrap_or("-"),
                if f.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                    "SHA-256 ok"
                } else {
                    "FALLO"
                }
            );
        }
    }
    if report.mismatches.is_empty() && report.manifest_ok {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(3))
    }
}

// --------------------------------------------------------------------------
// bench-upload: mide la subida nativa con parámetros concretos
// --------------------------------------------------------------------------

fn cmd_bench_upload() -> Result<ExitCode, String> {
    let file = PathBuf::from(arg("--file").ok_or("bench-upload necesita --file <fichero>")?);
    let folder = arg("--folder").ok_or("bench-upload necesita --folder <ID_DE_DRIVE>")?;
    let state_dir = arg("--state-dir").unwrap_or_else(|| DEFAULT_STATE_DIR.to_string());
    let folder = match arg("--subfolder") {
        Some(sub) if !sub.trim().is_empty() => {
            megadaemon_core::drive_native::ensure_folder(&sub, &folder, Some(&state_dir))?
        }
        _ => folder,
    };
    let workers = arg_usize("--workers", 1);
    let chunk_bytes = arg_u64("--chunk-bytes", backup_apps::UPLOAD_CHUNK_BYTES);
    let name = arg("--name").unwrap_or_else(|| {
        file.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "bench.bin".to_string())
    });
    let md = std::fs::metadata(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let size = md.len();
    let t0 = Instant::now();
    let up = backup_apps::upload_file_chunked(
        &file,
        &name,
        &folder,
        Some(&state_dir),
        chunk_bytes,
        4,
    )?;
    let ms = t0.elapsed().as_millis() as u64;
    let mibs = backup_apps::mib_per_s(size, ms.max(1));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "file": file.to_string_lossy(),
            "bytes": size,
            "workers": workers,
            "chunk_bytes": chunk_bytes,
            "elapsed_ms": ms,
            "mib_per_s": (mibs * 1000.0).round() / 1000.0,
            "kib_per_s": (mibs * 1024.0 * 1000.0).round() / 1000.0,
            "eta_s_for_196GiB": (196.08 * 1024.0 / mibs).round() as u64,
            "drive_file_id": up.file_id,
        }))
        .unwrap_or_default()
    );
    Ok(ExitCode::SUCCESS)
}

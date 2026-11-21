//! `md-recordings` — índice unificado de grabaciones + backup de seguridad.
//!
//! Herramienta de línea de comandos para **no depender del móvil**: todo lo
//! que hace es leer el disco y escribir en el disco, así que se puede probar
//! en el sandbox con un árbol de ficheros cualquiera.
//!
//! ```text
//! md-recordings scan   --root <dir> [--app <json|dir>]... [--json]
//! md-recordings backup --root <dir> [--dest <dir>] [--app <json|dir>]... [--json]
//! md-recordings verify --dest <dir> [--json]
//! ```
//!
//! * `--root`  : la raíz de audio donde vive todo (en el móvil, algo como
//!   `/sdcard/MegaDaemon/recordings` o `/sdcard`).
//! * `--app`   : la lista de un grabador. Puede ser un **fichero JSON** (la
//!   lista que publica la app) o un **directorio** (se inventaría con el
//!   mismo criterio que `call_recorder::list_recordings`). Se puede repetir.
//! * `--dest`  : la raíz del área de backup (dentro va `MANIFEST.ndjson`,
//!   `manifest.json` y la carga en `<dest>/<AAAA-MM-DD>/<app>/<fichero>`).
//!
//! Códigos de salida:
//! * `0` — sin problemas (puede haber huérfanos o duplicados: son datos).
//! * `1` — hay ausentes, sin audio o corruptos (o falló el backup/verificación).
//! * `2` — uso incorrecto.
//!
//! Este binario **no graba nada**: sólo indexa, unifica y respalda lo que ya
//! existe. El aviso legal de la grabación nativa de Samsung es cosa de
//! `call_recorder`/CSC, no de aquí.
//!
//! Nota de módulos: `call_recorder` y `recordings` todavía no están
//! registrados en `lib.rs`, así que aquí se incluyen con `#[path]` (el mismo
//! truco que usa `md-device-inventory.rs` con `device_inventory.rs`). Cuando
//! se cableen en `lib.rs`, estas tres líneas pueden sustituirse por
//! `use megadaemon_core::{call_recorder, recordings};`.

// Los módulos se copian aquí (`#[path]`) porque todavía no están registrados
// en `lib.rs`. Se silencian los avisos de código no usado: en la librería el
// módulo es público y no da problemas, pero aquí es una copia privada.
#[allow(dead_code)]
#[path = "../call_recorder.rs"]
mod call_recorder;
// `util_fs` es el helper canonico de `walk_files` (MEDIDO el 2026-11-21: al
// fusionar `fix/robustness-20261120` los tres consumidores pasaron a usarlo y
// los arneses autocontenidos dejaron de compilar: E0432).  Solo usa `std`.
#[path = "../util_fs.rs"]
#[allow(dead_code, unused_imports, unused_mut)]
mod util_fs;
#[allow(dead_code)]
#[path = "../drive_client.rs"]
mod drive_client;
#[allow(dead_code)]
#[path = "../recordings.rs"]
mod recordings;

use recordings::{Informe, Recording, RecordingState};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Formatea bytes en unidades legibles.
fn human_bytes(b: u64) -> String {
    const UNIDADES: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b as f64;
    let mut i = 0usize;
    while v >= 1024.0 && i + 1 < UNIDADES.len() {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", UNIDADES[i])
    }
}

/// Valor de una opción (`--nombre valor` o `--nombre=valor`).
fn arg(args: &[String], name: &str) -> Option<String> {
    let mut i = 0usize;
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

/// Todos los valores de una opción repetible.
fn args_multi(args: &[String], name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == name && i + 1 < args.len() {
            out.push(args[i + 1].clone());
            i += 2;
            continue;
        }
        if let Some(v) = args[i].strip_prefix(&format!("{name}=")) {
            out.push(v.to_string());
        }
        i += 1;
    }
    out
}

/// ¿Está la opción presente?
fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn uso() -> &'static str {
    "uso:\n  \
     md-recordings scan   --root <dir> [--app <json|dir>]... [--json]\n  \
     md-recordings backup --root <dir> [--dest <dir>] [--app <json|dir>]... [--json]\n  \
     md-recordings verify --dest <dir> [--json]\n\
     \n\
     opciones:\n  \
     --root <dir>     raíz de audio a indexar\n  \
     --app <json|dir> lista de un grabador (JSON o directorio); repetible\n  \
     --dest <dir>     raíz del área de backup (por defecto <state_dir>/backups/recordings)\n  \
     --state-dir <d>  directorio de estado (por defecto $MEGADAEMON_STATE_DIR o ./state)\n  \
     --json           salida en JSON\n  \
     --help           esta ayuda"
}

/// Construye las listas de los grabadores a partir de los `--app`.
///
/// Un `--app` puede ser:
/// * un **directorio** → se inventaría con el mismo criterio que
///   `call_recorder::list_recordings` (sólo audio, con SHA-256),
/// * un **fichero JSON** → se acepta cualquier forma (`{"app":…,"recordings":[…]}`,
///   `{"apps":[…]}`, `[…]`).
fn construir_listas(rutas: &[String]) -> Result<Vec<Value>, String> {
    let mut out: Vec<Value> = Vec::new();
    for r in rutas {
        let p = Path::new(r);
        if p.is_dir() {
            let app = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| r.clone());
            let lista = call_recorder::list_recordings(r);
            out.push(json!({"app": app, "recordings": lista}));
        } else if p.is_file() {
            let txt = std::fs::read_to_string(p)
                .map_err(|e| format!("no se puede leer {}: {e}", p.display()))?;
            let v: Value = serde_json::from_str(&txt)
                .map_err(|e| format!("{} no es JSON válido: {e}", p.display()))?;
            out.extend(recordings::listas_desde_json(&v));
        } else {
            return Err(format!("no existe: {r}"));
        }
    }
    Ok(out)
}

/// Imprime el informe en texto plano.
fn imprime_informe(informe: &Informe) {
    println!("ÍNDICE DE GRABACIONES");
    println!(
        "  total {} · ok {} · ausentes {} · corruptos {} · sin audio {} · huérfanos {} · duplicados {}",
        informe.total,
        informe.ok,
        informe.missing,
        informe.corrupt,
        informe.no_audio,
        informe.orphans,
        informe.duplicates
    );
    println!(
        "  bytes {} · desperdiciados {}",
        human_bytes(informe.total_bytes),
        human_bytes(informe.wasted_bytes)
    );
}

/// Imprime las grabaciones con problema.
fn imprime_problemas(registros: &[Recording]) {
    let problemas: Vec<&Recording> = registros
        .iter()
        .filter(|r| {
            !matches!(
                r.state,
                RecordingState::Ok | RecordingState::Duplicate | RecordingState::Orphan
            )
        })
        .collect();
    if problemas.is_empty() {
        return;
    }
    println!("  con problemas:");
    for r in problemas {
        println!(
            "    [{}] {} ({} bytes) — {}",
            r.state.label_es(),
            r.path,
            r.bytes,
            r.sha256
        );
    }
}

/// Cuenta de estados (para el resumen por app).
fn cuenta_por_app(registros: &[Recording]) -> Vec<(String, usize)> {
    let mut m: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for r in registros {
        *m.entry(r.app.clone()).or_insert(0) += 1;
    }
    m.into_iter().collect()
}

fn cmd_scan(args: &[String]) -> ExitCode {
    let root = match arg(args, "--root") {
        Some(r) => r,
        None => {
            eprintln!("falta --root\n\n{}", uso());
            return ExitCode::from(2);
        }
    };
    let apps = args_multi(args, "--app");
    let listas = match construir_listas(&apps) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    let (registros, informe) = recordings::reconciliar(&listas, &root);
    let grupos = recordings::grupos_duplicados(&registros);

    if flag(args, "--json") {
        let salida = json!({
            "raiz": root,
            "apps": cuenta_por_app(&registros),
            "informe": informe,
            "grabaciones": registros,
            "duplicados": grupos,
        });
        println!("{}", serde_json::to_string_pretty(&salida).unwrap_or_default());
    } else {
        println!("raíz: {root}");
        let cuentas = cuenta_por_app(&registros);
        if !cuentas.is_empty() {
            let texto: Vec<String> = cuentas
                .iter()
                .map(|(a, n)| format!("{a}={n}"))
                .collect();
            println!("  apps: {}", texto.join(" "));
        }
        imprime_informe(&informe);
        imprime_problemas(&registros);
        if !grupos.is_empty() {
            println!("  duplicados (mismo SHA-256):");
            for g in &grupos {
                println!(
                    "    {} · {} ficheros · {} desperdiciados",
                    &g.sha256[..16.min(g.sha256.len())],
                    g.ficheros.len(),
                    human_bytes(g.wasted_bytes)
                );
            }
        }
    }

    if informe.hay_problemas() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn cmd_backup(args: &[String]) -> ExitCode {
    let root = match arg(args, "--root") {
        Some(r) => r,
        None => {
            eprintln!("falta --root\n\n{}", uso());
            return ExitCode::from(2);
        }
    };
    let state_dir = arg(args, "--state-dir").unwrap_or_else(|| {
        std::env::var("MEGADAEMON_STATE_DIR")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "./state".to_string())
    });
    let dest: PathBuf = match arg(args, "--dest") {
        Some(d) => PathBuf::from(d),
        None => recordings::destino_por_defecto(Path::new(&state_dir)),
    };
    let apps = args_multi(args, "--app");
    let listas = match construir_listas(&apps) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    // Con listas de apps se reconcilia (para saber qué hash declara cada una);
    // sin ellas se respalda todo lo que hay en la raíz (copia de seguridad).
    let (registros, informe) = if listas.is_empty() {
        let regs = recordings::escanear(&root);
        let inf = recordings::informe_de(&regs);
        (regs, inf)
    } else {
        recordings::reconciliar(&listas, &root)
    };

    let opts = recordings::OpcionesBackup {
        dia: recordings::today_stamp(),
        solo_ok: false,
    };
    let resultado = match recordings::respaldar(&registros, &dest, &opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    if flag(args, "--json") {
        let salida = json!({
            "raiz": root,
            "destino": resultado.destino,
            "informe": informe,
            "backup": resultado,
        });
        println!("{}", serde_json::to_string_pretty(&salida).unwrap_or_default());
    } else {
        println!("BACKUP DE GRABACIONES");
        println!("  raíz:    {root}");
        println!("  destino: {}", resultado.destino);
        println!(
            "  copiados {} · saltados {} · verificados {} · duplicados omitidos {}",
            resultado.copiados,
            resultado.saltados,
            resultado.verificados,
            resultado.duplicados_omitidos
        );
        println!("  bytes copiados {}", human_bytes(resultado.bytes_copiados));
        println!(
            "  manifiesto: {} entradas · {} bytes · sha256 {}",
            resultado.total_manifest,
            human_bytes(resultado.bytes_manifest),
            &resultado.manifest_sha256[..16.min(resultado.manifest_sha256.len())]
        );
        for f in &resultado.fallos {
            println!("  FALLO: {f}");
        }
    }

    if !resultado.fallos.is_empty() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn cmd_verify(args: &[String]) -> ExitCode {
    let dest = match arg(args, "--dest") {
        Some(d) => PathBuf::from(d),
        None => {
            eprintln!("falta --dest\n\n{}", uso());
            return ExitCode::from(2);
        }
    };
    let v = match recordings::verificar(&dest) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    if flag(args, "--json") {
        let salida = json!({
            "destino": dest.to_string_lossy(),
            "verificacion": v,
        });
        println!("{}", serde_json::to_string_pretty(&salida).unwrap_or_default());
    } else {
        println!("VERIFICACIÓN DEL BACKUP");
        println!("  destino: {}", dest.to_string_lossy());
        println!(
            "  entradas {} · ok {} · faltan {} · corruptos {}",
            v.total, v.ok, v.faltan, v.corruptos
        );
        println!("  bytes {}", human_bytes(v.bytes));
        println!("  sha256 del manifiesto {}", v.manifest_sha256);
        for p in &v.problemas {
            println!("  PROBLEMA: {p}");
        }
    }
    if v.faltan > 0 || v.corruptos > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 2 {
        eprintln!("{}", uso());
        return ExitCode::from(2);
    }
    if flag(&argv, "--help") || flag(&argv, "-h") {
        println!("{}", uso());
        return ExitCode::SUCCESS;
    }
    let cmd = argv[1].clone();
    let args = &argv[2..];
    match cmd.as_str() {
        "scan" => cmd_scan(args),
        "backup" => cmd_backup(args),
        "verify" => cmd_verify(args),
        other => {
            eprintln!("orden desconocida: {other}\n\n{}", uso());
            ExitCode::from(2)
        }
    }
}

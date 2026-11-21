//! `md-pkg` — CLI de la gestión integrada de paquetes de MegaDaemon.
//!
//! Una sola herramienta para TODO lo que se actualiza: APKs, binarios, repos
//! git, fuentes y diccionarios. El esquema único vive en
//! `megadaemon_core::pkgmgr` (ver `docs/PACKAGE_MANAGEMENT.md`).
//!
//! ```text
//! md-pkg index [--json] [--state-dir <dir>]
//! md-pkg check [--json] [--state-dir <dir>] [--manifest <fichero>] [--offline]
//! md-pkg update --kind <apk|binary|repo|font|dict> --name <nombre>
//!               [--dry-run] [--state-dir <dir>] [--manifest <fichero>]
//!               [--destino <ruta>]
//! md-pkg manifest validate <fichero>
//! ```
//!
//! ## Qué hace cada orden
//!
//! * `index` — la **tabla unificada**: qué hay instalado (APKs del
//!   inventario, binarios del `state_dir`, repos de `git_ops`) × qué hay
//!   disponible (los manifiestos ya conocidos) × **qué está desfasado y por
//!   qué**. **No usa la red**: lee lo que hay en caché.
//! * `check` — refresca los proveedores (respetando el **cursor** de cada uno
//!   y la **cuarentena** de los que fallan) y dice qué hay que actualizar.
//! * `update` — descarga (reanudable, por rango HTTP), verifica el SHA-256 e
//!   instala **de forma atómica** con **rollback**.
//! * `manifest validate` — valida un manifiesto contra el **esquema único**.
//!
//! ## El sandbox (sin móvil)
//!
//! En un PC sin Android el inventario de APKs devuelve **cero** apps y el
//! `git_ops` no encuentra repo ⇒ la tabla sale vacía y la CLI lo dice
//! honestamente ("no hay nada instalado"). **No inventa nada** ni falla.

use megadaemon_core::pkgmgr::{self, IndicePaquetes, Instalado, Manifest, TipoAsset};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

// ── Utilidades de argumentos ──────────────────────────────────────────────

/// Valor de una opción (`--x val` o `--x=val`).
fn arg(nombre: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == nombre && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        if let Some(v) = args[i].strip_prefix(&format!("{nombre}=")) {
            return Some(v.to_string());
        }
        i += 1;
    }
    None
}

/// ¿Está presente una bandera?
fn flag(nombre: &str) -> bool {
    std::env::args().any(|a| a == nombre)
}

/// Texto de uso.
fn usage() -> &'static str {
    concat!(
        "uso: md-pkg <orden> [opciones]\n",
        "\n",
        "órdenes:\n",
        "  index                      tabla unificada (instalado × disponible)\n",
        "  check                      refresca las fuentes y dice qué hay que actualizar\n",
        "  update --kind <k> --name <n>[--dry-run]   instala una actualización\n",
        "  manifest validate <fichero>               valida un manifiesto\n",
        "\n",
        "opciones comunes:\n",
        "  --state-dir <dir>   directorio de estado (por defecto, el del daemon)\n",
        "  --manifest <fichero>  usa este manifiesto en vez de las fuentes\n",
        "  --json              salida en JSON\n",
        "  --os <so> --arch <arquitectura>   plataforma objetivo\n",
        "  --dry-run           no descarga nada (sólo dice lo que haría)\n",
    )
}

/// El `state_dir` que se usa (opción > entorno > el del daemon).
fn state_dir() -> String {
    if let Some(d) = arg("--state-dir") {
        if !d.trim().is_empty() {
            return d.trim().to_string();
        }
    }
    if let Ok(d) = std::env::var("MEGADAEMON_STATE_DIR") {
        if !d.trim().is_empty() {
            return d.trim().to_string();
        }
    }
    megadaemon_core::cockpit_server::state_dir()
}

/// Sistema operativo objetivo.
fn os_objetivo() -> String {
    if let Some(v) = arg("--os") {
        return v.trim().to_ascii_lowercase();
    }
    if let Ok(v) = std::env::var("MEGADAEMON_PKG_OS") {
        if !v.trim().is_empty() {
            return v.trim().to_ascii_lowercase();
        }
    }
    if cfg!(target_os = "android") {
        "android".to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}

/// Arquitectura objetivo.
fn arch_objetivo() -> String {
    if let Some(v) = arg("--arch") {
        return v.trim().to_ascii_lowercase();
    }
    if let Ok(v) = std::env::var("MEGADAEMON_PKG_ARCH") {
        if !v.trim().is_empty() {
            return v.trim().to_ascii_lowercase();
        }
    }
    match std::env::consts::ARCH {
        "aarch64" => "arm64".to_string(),
        "x86_64" => "x86_64".to_string(),
        "arm" => "arm".to_string(),
        otra => otra.to_string(),
    }
}

/// Manifiestos locales (los de caché + los que se pasen por `--manifest`).
fn manifiestos_locales(state: &str) -> Vec<(String, Manifest)> {
    let mut out: Vec<(String, Manifest)> = Vec::new();
    let dir = pkgmgr::dir_estado(state).join("manifests");
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(texto) = std::fs::read_to_string(&p) else {
                continue;
            };
            if let Ok(m) = serde_json::from_str::<Manifest>(&texto) {
                let id = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                out.push((id, m));
            }
        }
    }
    if let Some(ruta) = arg("--manifest") {
        for r in ruta.split(',') {
            let r = r.trim();
            if r.is_empty() {
                continue;
            }
            match leer_manifiesto(Path::new(r)) {
                Ok(m) => out.push((r.to_string(), m)),
                Err(e) => eprintln!("aviso: no se pudo leer {r}: {e}"),
            }
        }
    }
    out
}

/// Lee un manifiesto de disco (validándolo).
fn leer_manifiesto(ruta: &Path) -> Result<Manifest, String> {
    let texto = std::fs::read_to_string(ruta)
        .map_err(|e| format!("no se puede leer {}: {e}", ruta.display()))?;
    Manifest::de_json(&texto)
}

// ── Órdenes ──────────────────────────────────────────────────────────────

/// `md-pkg index`: la tabla unificada (sin red).
fn orden_index(state: &str) -> ExitCode {
    let instalados = pkgmgr::instalados_completos(state, 500);
    let disponibles = manifiestos_locales(state);
    let idx = IndicePaquetes::construir_con_fuentes(&instalados, &disponibles);

    if flag("--json") {
        let mut v = idx.to_json();
        v["installed_count"] = json!(instalados.len());
        v["sources"] = json!(disponibles
            .iter()
            .map(|(id, m)| json!({
                "id": id,
                "version": m.version,
                "assets": m.assets.len(),
            }))
            .collect::<Vec<_>>());
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        return ExitCode::SUCCESS;
    }

    if instalados.is_empty() {
        println!("no hay nada instalado en esta instancia (ni APKs, ni binarios, ni repos)");
    }
    if disponibles.is_empty() {
        println!(
            "no hay ningún manifiesto conocido todavía (usa `md-pkg check` para descargarlos)"
        );
    }
    print!("{}", idx.tabla());
    let r = idx.resumen();
    println!(
        "total {} · a actualizar {} · al día {} · no instalados {} · desconocidos {}",
        r["total"], r["actualizar"], r["ya_al_dia"], r["no_instalado"], r["desconocido"]
    );
    ExitCode::SUCCESS
}

/// `md-pkg check`: refresca las fuentes y dice qué hay que actualizar.
fn orden_check(state: &str) -> ExitCode {
    let offline = flag("--offline");
    let mut fuentes: Vec<(String, Vec<Manifest>)> = Vec::new();
    let mut errores: Vec<Value> = Vec::new();

    if !offline {
        let catalogo = pkgmgr::fuentes_por_defecto(state);
        let transporte = pkgmgr::TransporteUreq::nuevo();
        for f in catalogo {
            match pkgmgr::refrescar_proveedor(&f, state, &transporte) {
                Ok(c) => {
                    if c.sin_cambios {
                        eprintln!("· {}: sin cambios (cursor)", f.descripcion());
                    }
                    fuentes.push((f.id(), c.manifiestos));
                }
                Err(e) => {
                    errores.push(json!({ "fuente": f.id(), "error": e }));
                }
            }
        }
    }

    // Los manifiestos locales (caché + `--manifest`) también cuentan.
    for (id, m) in manifiestos_locales(state) {
        if !fuentes.iter().any(|(i, _)| *i == id) {
            fuentes.push((id, vec![m]));
        }
    }

    let instalados = pkgmgr::instalados_completos(state, 500);
    let plano: Vec<(String, Manifest)> = fuentes
        .iter()
        .flat_map(|(id, ms)| ms.iter().map(move |m| (id.clone(), m.clone())))
        .collect();
    let idx = IndicePaquetes::construir_con_fuentes(&instalados, &plano);

    if flag("--json") {
        let mut v = idx.to_json();
        v["sources"] = json!(fuentes
            .iter()
            .map(|(id, ms)| json!({
                "id": id,
                "manifests": ms.len(),
                "version": ms.first().map(|m| m.version.clone()).unwrap_or_default(),
            }))
            .collect::<Vec<_>>());
        v["errors"] = json!(errores);
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        return if errores.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    if fuentes.is_empty() {
        println!("no hay ninguna fuente configurada (revisa <state_dir>/pkgmgr/fuentes.json)");
    }
    if instalados.is_empty() {
        println!("no hay nada instalado en esta instancia (ni APKs, ni binarios, ni repos)");
    }
    print!("{}", idx.tabla());

    let pendientes = idx.pendientes();
    if !pendientes.is_empty() {
        println!("\npendientes de actualizar:");
        for f in &pendientes {
            println!(
                "  · {} {} → {} ({} bytes, {})",
                f.kind, f.nombre, f.disponible_version, f.disponible_size, f.detalle
            );
        }
    } else {
        println!("\nnada que actualizar");
    }
    for e in &errores {
        eprintln!("fuente con problemas: {}", e["fuente"]);
    }
    ExitCode::SUCCESS
}

/// `md-pkg update`: instala una actualización (atómica, con rollback).
fn orden_update(state: &str) -> ExitCode {
    let Some(kind_txt) = arg("--kind") else {
        eprintln!("falta --kind (apk|binary|repo|font|dict)");
        return ExitCode::from(2);
    };
    let Some(nombre) = arg("--name") else {
        eprintln!("falta --name");
        return ExitCode::from(2);
    };
    let Some(kind) = TipoAsset::from_str(&kind_txt) else {
        eprintln!("--kind desconocido: {kind_txt} (apk|binary|repo|font|dict)");
        return ExitCode::from(2);
    };

    // 1) Buscar el asset en las fuentes conocidas.
    let disponibles = manifiestos_locales(state);
    let (mut encontrado, mut version) = (None, String::new());
    for (_, m) in &disponibles {
        if let Some(a) = m.asset_para(kind, &nombre, &os_objetivo(), &arch_objetivo()) {
            if encontrado.is_none() {
                encontrado = Some(a.clone());
                version = m.version.clone();
            }
        }
    }
    let Some(asset) = encontrado else {
        eprintln!(
            "no se encuentra '{nombre}' ({kind}) en ningún manifiesto conocido (¿`md-pkg check` primero?)"
        );
        return ExitCode::FAILURE;
    };

    let destino = match arg("--destino") {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d.trim()),
        _ => pkgmgr::destino_por_defecto(state, kind, &nombre),
    };

    // 2) ¿Hace falta? (comparación por SHA-256: cero trabajo si es idéntico)
    let mut inst = Instalado::nuevo(kind, &nombre, &destino.to_string_lossy());
    inst.version = version.clone();
    if destino.exists() {
        if let Ok((_, sha)) = pkgmgr::sha256_de_fichero(&destino) {
            inst.sha256 = sha;
        }
    }
    // Un repo se identifica por su **commit**, no por el hash de un fichero.
    if kind == TipoAsset::Repo {
        for r in pkgmgr::instalados_desde_git() {
            if r.name == nombre {
                inst.signature = r.signature.clone();
                inst.version = r.version.clone();
            }
        }
    }
    let m = Manifest {
        schema_version: pkgmgr::SCHEMA_VERSION,
        version: version.clone(),
        released_at: pkgmgr::ahora_iso(),
        assets: vec![asset.clone()],
    };
    let ya_igual = inst.sha256.eq_ignore_ascii_case(&asset.sha256)
        || (kind == TipoAsset::Repo
            && inst.signature.is_some()
            && inst.signature.as_deref() == asset.signature.as_deref());
    if ya_igual {
        println!(
            "'{nombre}' ya está al día (sha-256 {}) — no se descarga nada",
            &asset.sha256[..12.min(asset.sha256.len())]
        );
        return ExitCode::SUCCESS;
    }

    if flag("--dry-run") {
        println!("[dry-run] se instalaría:");
        println!("  origen   {}", asset.url);
        println!("  destino  {}", destino.display());
        println!("  sha-256  {}", asset.sha256);
        println!("  tamaño   {} bytes", asset.size);
        println!("  versión  {version}");
        return ExitCode::SUCCESS;
    }

    // 3) Descargar (reanudable) + verificar + instalar (atómico + rollback).
    match pkgmgr::instalar_con_rollback_detallado(&inst, &m, &destino) {
        Ok(()) => {
            println!(
                "instalado: {} (sha-256 {})",
                destino.display(),
                asset.sha256
            );
            let previo = pkgmgr::ruta_previa(&destino);
            if previo.exists() {
                println!("versión anterior conservada en {}", previo.display());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("no se pudo instalar: {}", e.mensaje);
            if let Some(r) = &e.rollback {
                eprintln!(
                    "rollback: {} ← {} (restaurado={})",
                    r.destino, r.previo, r.restaurado
                );
            }
            ExitCode::FAILURE
        }
    }
}

/// `md-pkg manifest validate <fichero>`: valida contra el esquema único.
fn orden_manifest(args: &[String]) -> ExitCode {
    // Se admite: `manifest validate <fichero>` y `manifest <fichero>`.
    // Los ficheros son los argumentos que no son opciones ni sub-órdenes.
    let rutas: Vec<&String> = args
        .iter()
        .skip(2)
        .filter(|a| !a.starts_with("--") && *a != "validate")
        .collect();
    if rutas.is_empty() {
        eprintln!("falta el fichero del manifiesto");
        return ExitCode::from(2);
    }
    let mut todo_ok = true;
    let mut resultado: Vec<Value> = Vec::new();
    for r in rutas {
        let (ok, detalle) = match leer_manifiesto(Path::new(r)) {
            Ok(m) => (
                true,
                json!({
                    "file": r,
                    "ok": true,
                    "schema_version": m.schema_version,
                    "version": m.version,
                    "released_at": m.released_at,
                    "assets": m.assets.len(),
                }),
            ),
            Err(e) => {
                todo_ok = false;
                (false, json!({ "file": r, "ok": false, "error": e }))
            }
        };
        resultado.push(detalle.clone());
        if flag("--json") {
            println!(
                "{}",
                serde_json::to_string_pretty(&detalle).unwrap_or_default()
            );
        } else if ok {
            let d = &detalle;
            println!(
                "OK   {} (esquema {}, versión {}, {} assets)",
                r, d["schema_version"], d["version"], d["assets"]
            );
        } else {
            println!("ERROR {}: {}", r, detalle["error"]);
        }
    }
    if flag("--json") {
        // La salida por fichero ya se imprimió: sólo el código de salida cuenta.
    } else if !todo_ok {
        return ExitCode::FAILURE;
    }
    if todo_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// ── main ─────────────────────────────────────────────────────────────────

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("{}", usage());
        return ExitCode::from(2);
    }
    let orden = args[1].clone();
    let state = state_dir();

    match orden.as_str() {
        "index" => orden_index(&state),
        "check" => orden_check(&state),
        "update" => orden_update(&state),
        "manifest" => orden_manifest(&args),
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
        otra => {
            eprintln!("orden desconocida: {otra}");
            eprintln!("{}", usage());
            ExitCode::from(2)
        }
    }
}

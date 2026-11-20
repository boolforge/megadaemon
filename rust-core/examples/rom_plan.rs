//! W4 · Generador del plan de migración de ROM (ejemplo de Cargo).
//!
//! Se usa como CLI para que `deploy/rom_migrate.sh` pueda pedir el plan
//! **canónico** (el del módulo `rom_migration`) sin necesidad de que el
//! daemon exponga una tool nueva ni de tocar `Cargo.toml`:
//!
//! ```sh
//! cargo run --release --example rom_plan -- \
//!     --facts facts.json --rom rom.json [--hashes hashes.json] [--text]
//! ```
//!
//! - `--facts`: JSON con los hechos medidos del dispositivo (`DeviceFacts`).
//! - `--rom`: JSON con **una** ROM (`RomPackage`) o **un array** de candidatas.
//! - `--hashes`: JSON opcional con los SHA-256 *calculados* de los artefactos.
//! - `--text`: imprime el informe legible de dry-run en vez del plan JSON.
//!
//! El ejemplo **no flashea nada**: sólo produce el plan.

#[path = "../src/rom_migration.rs"]
mod rom_migration;

use rom_migration::{dry_run_report, plan_json, preflight, RomPackage};
use std::collections::BTreeMap;
use std::io::Read;

fn die(msg: &str) -> ! {
    eprintln!("rom_plan: {}", msg);
    std::process::exit(2);
}

fn read_file(path: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => die(&format!("no se pudo leer {}: {}", path, e)),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut facts_path: Option<String> = None;
    let mut rom_path: Option<String> = None;
    let mut hashes_path: Option<String> = None;
    let mut text = false;

    while let Some(a) = args.next() {
        match a.as_str() {
            "--facts" => facts_path = args.next(),
            "--rom" => rom_path = args.next(),
            "--hashes" => hashes_path = args.next(),
            "--text" => text = true,
            "--help" | "-h" => {
                println!(
                    "uso: rom_plan --facts <facts.json> --rom <rom.json|roms.json> \
                     [--hashes <hashes.json>] [--text]"
                );
                return;
            }
            other => die(&format!("argumento desconocido: {}", other)),
        }
    }

    let facts_path = facts_path.unwrap_or_else(|| die("falta --facts"));
    let rom_path = rom_path.unwrap_or_else(|| die("falta --rom"));

    let facts_raw = read_file(&facts_path);
    let device: rom_migration::DeviceFacts = match serde_json::from_str(&facts_raw) {
        Ok(d) => d,
        Err(e) => die(&format!("facts.json inválido: {}", e)),
    };

    let rom_raw = read_file(&rom_path);
    let candidates: Vec<RomPackage> = match serde_json::from_str(&rom_raw) {
        Ok(v) => vec![v],
        Err(_) => match serde_json::from_str::<Vec<RomPackage>>(&rom_raw) {
            Ok(v) => v,
            Err(e) => die(&format!("rom.json inválido (ni objeto ni array): {}", e)),
        },
    };
    if candidates.is_empty() {
        die("la lista de ROMs está vacía");
    }

    let mut computed: BTreeMap<String, String> = BTreeMap::new();
    if let Some(hp) = hashes_path {
        let raw = read_file(&hp);
        match serde_json::from_str::<BTreeMap<String, String>>(&raw) {
            Ok(m) => computed = m,
            Err(e) => die(&format!("hashes.json inválido: {}", e)),
        }
    }

    // Si hay varias candidatas, se seleccionan con los criterios por defecto;
    // con una sola, se usa tal cual (pero se informa si no pasa el pre-flight).
    let rom = if candidates.len() == 1 {
        candidates.into_iter().next().unwrap()
    } else {
        let sel = rom_migration::select_rom(
            &candidates,
            &device,
            &rom_migration::SelectionCriteria::default(),
        );
        for r in &sel.rejected {
            eprintln!("descartada: {} → {}", r.name, r.reason);
        }
        match sel.chosen {
            Some(r) => r,
            None => die("ninguna ROM cumple los criterios de selección"),
        }
    };

    if text {
        print!("{}", dry_run_report(&device, &[rom.clone()], &computed));
    } else {
        let rep = preflight(&device, &rom, &computed);
        println!("{}", plan_json(&device, &rom, &computed));
        if rep.abort {
            // Sale con código 3 para que el script no siga adelante.
            std::process::exit(3);
        }
    }
}

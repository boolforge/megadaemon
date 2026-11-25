//! Guarda-fuentes del repo: **cada `pub mod` declarado tiene fichero** y
//! **cada módulo tiene su contenido completo**.
//!
//! Por qué existe: dos merges del enjambre dejaron `lib.rs` declarando un
//! módulo cuyo fichero había desaparecido (⇒ `E0433`, la lib no compilaba) y
//! otros dos dejaron un fichero **truncado** (⇒ el arnés `call_audio_smoke`
//! pedía funciones inexistentes y tumbaba el job `quality` del CI). Ninguno
//! de los dos fallos se ve en el diff del módulo: se ven en el árbol.
//!
//! Este test lo detecta **en el CI, antes del despliegue**, y es la red de
//! seguridad de los merges: si un merge borra o trunca un fichero, el job se
//! pone rojo en vez de publicar un binario roto.

use std::path::{Path, PathBuf};

/// Módulos con contenido que ya se perdió una vez en un merge.
/// Si alguno vuelve a truncarse, este test lo dice por su nombre.
const GUARDADOS: &[(&str, &[&str])] = &[
    (
        "call_recorder",
        &[
            "pub enum CaptureSource",
            "pub fn alsa_voice_capture_plan",
            "pub fn csc_switch_risk",
            "pub fn csc_switch_verify_plan",
            "pub fn parse_csc_features",
            "pub fn voice_call_config_recording",
            "pub fn omc_state",
            "pub const SIM_OPERATOR_NUMERIC",
            "pub struct RecordingSupervisor",
            "pub fn supervisor_preflight",
            "pub const RECORDING_VALUE_WITH_DISCLAIMER",
        ],
    ),
    (
        "rom_migration",
        &[
            "pub fn preconfig_state",
            "pub fn preconfig_install_plan",
            "pub const PRECONFIG_PACKAGE",
            "pub fn omc_decode",
            "pub fn omc_encode",
            "pub fn csc_recording_module_files",
            "RECORDING_VALUE_ALLOWED_WITH_DISCLAIMER",
        ],
    ),
    ("aegis", &["pub fn encrypt", "pub fn decrypt", "pub const C0", "pub const C1"]),
    (
        "drive_client",
        &[
            "pub fn chunk_plan",
            "pub fn download_to_file",
            "pub fn query_session_status",
        ],
    ),
    (
        "backup_formats",
        &["pub fn read_tar", "pub fn read_zip", "pub fn scan_swift_backup"],
    ),
    ("recording_vault", &["pub fn scan_recordings", "pub fn vault_pack"]),
    ("backup_vault", &["pub fn aes_round", "pub fn seal_dir_streaming"]),
    (
        "argon2",
        &[
            "pub fn derive_key",
            "pub fn argon2_hash",
            "pub fn blake2b",
            "pub fn h_prime",
            "pub fn h0",
        ],
    ),
    ("zstd", &["pub fn descomprimir", "pub fn comprimir", "pub fn disponible"]),
    (
        "swift_sba1",
        &["pub fn leer_cabecera", "pub fn leer_tar_zstd", "pub fn es_sba1"],
    ),
];

fn raiz() -> PathBuf {
    // CARGO_MANIFEST_DIR = <repo>/rust-core
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn leer(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("no se puede leer {}: {e}", p.display()))
}

#[test]
fn cada_pub_mod_de_lib_rs_tiene_su_fichero() {
    let lib = leer(&raiz().join("src/lib.rs"));
    let mut declarados = Vec::new();
    for linea in lib.lines() {
        let l = linea.trim();
        if let Some(rest) = l.strip_prefix("pub mod ") {
            if let Some(nombre) = rest.strip_suffix(';') {
                let nombre = nombre.split_whitespace().next().unwrap_or("");
                if !nombre.is_empty() && !nombre.contains('{') && !nombre.contains('(') {
                    declarados.push(nombre.to_string());
                }
            }
        }
    }
    assert!(
        declarados.len() > 40,
        "lib.rs sólo declara {} módulos (¿se truncó?)",
        declarados.len()
    );
    let mut faltan = Vec::new();
    for m in &declarados {
        let f = raiz().join(format!("src/{m}.rs"));
        if !f.is_file() {
            faltan.push(format!("src/{m}.rs"));
        }
    }
    assert!(
        faltan.is_empty(),
        "lib.rs declara módulos SIN fichero: {}",
        faltan.join(", ")
    );
}

/// MEDIDO el 2026-11-21: los arneses ya NO copian los módulos de la libreria
/// con `#[path] = "../src/x.rs"`.
///
/// Por qué era un bug (no sólo estilo): dentro de un **arnés**,
/// `crate::` es la raíz del ARNÉS, no de la libreria ⇒ un módulo copiado que
/// usa `crate::util_fs` (o `crate::cockpit_server`, …) **no resolvía** ⇒
/// `error[E0433]` ⇒ **el target de test no compilaba** ⇒ `cargo test` moría
/// con rc=101 **sin ejecutar ni un test** ⇒ el job "calidad" del CI quedaba
/// en rojo sin que nadie supiera por qué (y sin binario identificable).
///
/// Ahora los arneses usan `use megadaemon_core::<modulo>;` ⇒ (a) compilan,
/// (b) no hay copias que se queden viejas, (c) el `crate::` del módulo
/// resuelve contra la libreria de verdad.
///
/// Este test vigila que **no vuelva** el patrón roto: si un arnés vuelve a
/// copiar un módulo de `src/`, el fichero incluido tiene que existir Y el
/// módulo copiado no debe usar `crate::` (si lo usa, no compila).
#[test]
fn los_arnes_usan_la_libreria_y_no_copian_modulos() {
    let dir = raiz().join("tests");
    let mut copias = Vec::new();
    let mut incluidos = 0usize;
    if let Ok(entradas) = std::fs::read_dir(&dir) {
        for e in entradas.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let txt = leer(&p);
            for linea in txt.lines() {
                let l = linea.trim();
                // MEDIDO el 2026-11-22: los comentarios que EXPLICAN el
                // patrón antiguo mencionan `#[path = "../src/x.rs"]` => el
                // guard los contaba como includes reales y fallaba solo
                // (falso positivo).  Sólo cuentan las líneas de CÓDIGO.
                if l.starts_with("//") {
                    continue;
                }
                const PRE: &str = "#[path = \"../src/";
                if let Some(inicio) = l.find(PRE) {
                    let resto = &l[inicio + PRE.len()..];
                    if let Some(fin) = resto.find('"') {
                        let rel = &resto[..fin];
                        incluidos += 1;
                        // MEDIDO (2026-11-25): el path ya trae la
                        // extension (".rs"), y aqui se volvia a anadir
                        // => "src/x.rs.rs" => "NO EXISTE" para todos.
                        let rel = rel.strip_suffix(".rs").unwrap_or(rel);
                        let f = raiz().join("src").join(format!("{rel}.rs"));
                        if !f.is_file() {
                            copias.push(format!("{} -> src/{rel}.rs (NO EXISTE)", p.display()));
                            continue;
                        }
                        // si el módulo copiado usa `crate::`, no va a compilar
                        let cuerpo = leer(&f);
                        if cuerpo.contains("crate::") {
                            copias.push(format!(
                                "{} copia src/{rel}.rs y ese módulo usa `crate::` (no compila en un arnés)",
                                p.display()
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        copias.is_empty(),
        "arneses que copian módulos de la libreria (usa `use megadaemon_core::`): {}",
        copias.join(", ")
    );
    let _ = incluidos;
}

#[test]
fn los_modulos_contenido_vigilado_no_estan_truncados() {
    let mut faltan = Vec::new();
    for (modulo, simbolos) in GUARDADOS {
        let f = raiz().join(format!("src/{modulo}.rs"));
        if !f.is_file() {
            faltan.push(format!("{modulo}.rs (no existe)"));
            continue;
        }
        let txt = leer(&f);
        for s in *simbolos {
            if !txt.contains(s) {
                faltan.push(format!("{modulo}.rs: falta `{s}`"));
            }
        }
    }
    assert!(
        faltan.is_empty(),
        "contenido perdido en un merge: {}",
        faltan.join(" | ")
    );
}

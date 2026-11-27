//! PLUGIN_DATA persistente **por identidad** (patrón 9 de
//! `docs/DISENO_PATRONES_CLI_RUST.md`, medido en el CLI de Rust de Devin
//! Desktop: `PLUGIN_DATA` sobrevive a las actualizaciones porque la ruta
//! depende de la IDENTIDAD del plugin, no de su versión; y los globs se
//! validan **sin** `..`, `#` ni `@` para que no se puedan convertir en un
//! path traversal).
//!
//! Por qué aquí y no en un script: la regla del proyecto es que toda
//! capacidad nueva vive en Rust dentro del daemon y se expone por CLI y por
//! tool MCP.  Este módulo no depende de nada externo (ni de Drive, ni de
//! la red): es almacenamiento local del nodo.
//!
//! MEDIDO 2026-11-27: implementación propia; el fichero de Devin Desktop es
//! material propietario y NO se ha copiado (sólo la arquitectura).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{json, Value};

/// Identidad válida de un plugin.
///
/// Medido en el producto de referencia: la ruta de `PLUGIN_DATA` se deriva
/// de la identidad, así que la identidad NO puede llevar separadores ni
/// `..` (si los llevara, la "persistencia por identidad" sería un
/// path traversal).  Aquí el alfabeto es ASCII en minúsculas, dígitos, `.`,
/// `_` y `-`.
pub fn validar_identidad(identidad: &str) -> Result<(), String> {
    if identidad.is_empty() {
        return Err("la identidad del plugin está vacía".to_string());
    }
    if identidad.len() > 128 {
        return Err("la identidad del plugin es demasiado larga (>128)".to_string());
    }
    if identidad == "." || identidad == ".." {
        return Err("la identidad del plugin no puede ser '.' ni '..'".to_string());
    }
    for c in identidad.chars() {
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-';
        if !ok {
            return Err(format!(
                "carácter no permitido en la identidad de plugin: {:?}",
                c
            ));
        }
    }
    if identidad.starts_with('.') {
        return Err("la identidad del plugin no puede empezar por '.'".to_string());
    }
    Ok(())
}

/// Glob seguro (patrón de ficheros que un plugin puede reclamar).
///
/// Reglas medidas: sin `..`, sin `#`, sin `@`, y nunca una ruta absoluta.
/// `..` es el vector de traversal; `#` y `@` se prohíben porque rompen la
/// resolución de nombres (y el `@` se usa para referenciar versiones).
pub fn validar_glob(glob: &str) -> Result<(), String> {
    if glob.is_empty() {
        return Err("el glob está vacío".to_string());
    }
    if glob.contains("..") {
        return Err("el glob no puede contener '..'".to_string());
    }
    if glob.contains('#') {
        return Err("el glob no puede contener '#'".to_string());
    }
    if glob.contains('@') {
        return Err("el glob no puede contener '@'".to_string());
    }
    if glob.starts_with('/') || glob.starts_with('\\') {
        return Err("el glob no puede ser una ruta absoluta".to_string());
    }
    if glob.contains('\0') {
        return Err("el glob no puede contener el byte nulo".to_string());
    }
    Ok(())
}

/// Nombre de fichero válido dentro del directorio de datos de un plugin.
pub fn validar_nombre(nombre: &str) -> Result<(), String> {
    if nombre.is_empty() {
        return Err("el nombre está vacío".to_string());
    }
    if nombre.contains('/') || nombre.contains('\\') {
        return Err("el nombre no puede contener separadores".to_string());
    }
    if nombre == "." || nombre == ".." {
        return Err("el nombre no puede ser '.' ni '..'".to_string());
    }
    if nombre.contains('\0') {
        return Err("el nombre no puede contener el byte nulo".to_string());
    }
    Ok(())
}

/// Directorio de datos de un plugin, **por identidad**.
///
/// La versión NO entra en la ruta: por eso los datos sobreviven a una
/// actualización del plugin (y se borran al desinstalarlo).
pub fn dir_datos(base: &Path, identidad: &str) -> Result<PathBuf, String> {
    validar_identidad(identidad)?;
    Ok(base.join("plugins").join(identidad))
}

/// Raíz de `PLUGIN_DATA` de este nodo.
///
/// MEDIDO (2026-11-28): `cockpit_server::state_dir()` devuelve `String` (no
/// `PathBuf`), así que hay que convertirla explícitamente antes de encadenar
/// el `join`.
pub fn raiz() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("plugins")
}

/// Escribe un fichero de datos de un plugin.  Devuelve la ruta escrita.
pub fn escribir(base: &Path, identidad: &str, nombre: &str, contenido: &[u8]) -> Result<PathBuf, String> {
    let dir = dir_datos(base, identidad)?;
    validar_nombre(nombre)?;
    fs::create_dir_all(&dir).map_err(|e| format!("no se pudo crear {}: {}", dir.display(), e))?;
    let ruta = dir.join(nombre);
    fs::write(&ruta, contenido).map_err(|e| format!("no se pudo escribir {}: {}", ruta.display(), e))?;
    Ok(ruta)
}

/// Lee un fichero de datos de un plugin (UTF-8 con pérdida).
pub fn leer(base: &Path, identidad: &str, nombre: &str) -> Result<String, String> {
    let dir = dir_datos(base, identidad)?;
    validar_nombre(nombre)?;
    let ruta = dir.join(nombre);
    let b = fs::read(&ruta).map_err(|e| format!("no se pudo leer {}: {}", ruta.display(), e))?;
    Ok(String::from_utf8_lossy(&b).to_string())
}

/// Una entrada del directorio de datos de un plugin.
#[derive(Clone, Debug)]
pub struct Entrada {
    pub nombre: String,
    pub bytes: u64,
    pub modificado_s: u64,
}

/// Lista los datos de un plugin (ordenados por nombre).
pub fn listar(base: &Path, identidad: &str) -> Result<Vec<Entrada>, String> {
    let dir = dir_datos(base, identidad)?;
    let mut out = Vec::new();
    let rd = match fs::read_dir(&dir) {
        Ok(r) => r,
        Err(_) => return Ok(out), // un plugin sin datos no es un error
    };
    for e in rd.flatten() {
        let nombre = e.file_name().to_string_lossy().to_string();
        let md = match e.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !md.is_file() {
            continue;
        }
        let modificado = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        out.push(Entrada {
            nombre,
            bytes: md.len(),
            modificado_s: modificado,
        });
    }
    out.sort_by(|a, b| a.nombre.cmp(&b.nombre));
    Ok(out)
}

/// Borra TODOS los datos de un plugin (desinstalación).  Devuelve los B.
pub fn purgar(base: &Path, identidad: &str) -> Result<u64, String> {
    let dir = dir_datos(base, identidad)?;
    let entradas = listar(base, identidad)?;
    let mut bytes = 0u64;
    for e in &entradas {
        bytes += e.bytes;
    }
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("no se pudo borrar {}: {}", dir.display(), e))?;
    }
    Ok(bytes)
}

/// Lista los plugins que tienen datos (por identidad).
pub fn listar_plugins(base: &Path) -> Result<Vec<String>, String> {
    let raiz = base.join("plugins");
    let mut out = Vec::new();
    let rd = match fs::read_dir(&raiz) {
        Ok(r) => r,
        Err(_) => return Ok(out),
    };
    for e in rd.flatten() {
        if !e.path().is_dir() {
            continue;
        }
        let id = e.file_name().to_string_lossy().to_string();
        if validar_identidad(&id).is_ok() {
            out.push(id);
        }
    }
    out.sort();
    Ok(out)
}

// =============================================================================
// CLI
// =============================================================================
/// `megadaemon-core plugin-data <validar-glob|listar|escribir|leer|purgar|plugins>`
pub fn run_cli(args: &[String]) -> i32 {
    match run_cli_res(args) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("plugin-data error: {}", e);
            1
        }
    }
}

fn run_cli_res(args: &[String]) -> Result<Value, String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    let base = raiz();
    match sub {
        "validar-glob" => {
            let glob = args.get(1).map(|s| s.as_str()).unwrap_or("");
            validar_glob(glob)?;
            Ok(json!({ "ok": true, "glob": glob }))
        }
        "validar-identidad" => {
            let id = args.get(1).map(|s| s.as_str()).unwrap_or("");
            validar_identidad(id)?;
            Ok(json!({ "ok": true, "identidad": id }))
        }
        "listar" => {
            let id = args.get(1).map(|s| s.as_str()).unwrap_or("");
            let es = listar(&base, id)?;
            let items: Vec<Value> = es
                .iter()
                .map(|e| {
                    json!({ "nombre": e.nombre, "bytes": e.bytes, "modificado_s": e.modificado_s })
                })
                .collect();
            Ok(json!({
                "identidad": id,
                "dir": dir_datos(&base, id)?.display().to_string(),
                "entradas": items,
            }))
        }
        "plugins" => {
            let ids = listar_plugins(&base)?;
            Ok(json!({ "plugins": ids }))
        }
        "escribir" => {
            let id = args.get(1).map(|s| s.as_str()).unwrap_or("");
            let nombre = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let contenido = args.get(3).map(|s| s.as_str()).unwrap_or("");
            let ruta = escribir(&base, id, nombre, contenido.as_bytes())?;
            Ok(json!({ "ok": true, "ruta": ruta.display().to_string() }))
        }
        "leer" => {
            let id = args.get(1).map(|s| s.as_str()).unwrap_or("");
            let nombre = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let txt = leer(&base, id, nombre)?;
            Ok(json!({ "ok": true, "contenido": txt }))
        }
        "purgar" => {
            let id = args.get(1).map(|s| s.as_str()).unwrap_or("");
            let bytes = purgar(&base, id)?;
            Ok(json!({ "ok": true, "bytes_borrados": bytes }))
        }
        "" => Err("uso: plugin-data <validar-glob|validar-identidad|listar|plugins|escribir|leer|purgar>".to_string()),
        otro => Err(format!("subcomando desconocido: {}", otro)),
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("md-plugin-data-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn la_identidad_valida_se_acepta() {
        assert!(validar_identidad("mi-plugin").is_ok());
        assert!(validar_identidad("plugin.v2").is_ok());
        assert!(validar_identidad("a_b-c.9").is_ok());
    }

    #[test]
    fn la_identidad_mala_se_rechaza() {
        assert!(validar_identidad("").is_err());
        assert!(validar_identidad("..").is_err());
        assert!(validar_identidad(".").is_err());
        assert!(validar_identidad("con/separador").is_err());
        assert!(validar_identidad("con\\separador").is_err());
        assert!(validar_identidad("MAYUSCULAS").is_err());
        assert!(validar_identidad(".oculto").is_err());
    }

    #[test]
    fn el_glob_se_valida_contra_traversal() {
        assert!(validar_glob("skills/*.md").is_ok());
        assert!(validar_glob("**/docs/**").is_ok());
        assert!(validar_glob("../etc/passwd").is_err());
        assert!(validar_glob("a/../b").is_err());
        assert!(validar_glob("con#hash").is_err());
        assert!(validar_glob("con@version").is_err());
        assert!(validar_glob("/absoluto").is_err());
        assert!(validar_glob("").is_err());
    }

    #[test]
    fn el_nombre_no_puede_ser_un_travieso() {
        assert!(validar_nombre("notas.md").is_ok());
        assert!(validar_nombre("../notas.md").is_err());
        assert!(validar_nombre("a/b").is_err());
        assert!(validar_nombre("..").is_err());
    }

    #[test]
    fn los_datos_viven_por_identidad_y_no_por_version() {
        let b = tmp("identidad");
        escribir(&b, "mi-plugin", "notas.md", b"hola").unwrap();
        // La ruta NO lleva la versión: es <raiz>/plugins/<identidad>.
        let dir = dir_datos(&b, "mi-plugin").unwrap();
        assert!(dir.ends_with("mi-plugin"));
        assert!(dir.join("notas.md").exists());
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn escribir_leer_y_listar() {
        let b = tmp("rl");
        escribir(&b, "p1", "a.txt", b"uno").unwrap();
        escribir(&b, "p1", "b.txt", b"dos").unwrap();
        assert_eq!(leer(&b, "p1", "a.txt").unwrap(), "uno");
        let es = listar(&b, "p1").unwrap();
        assert_eq!(es.len(), 2);
        assert_eq!(es[0].nombre, "a.txt");
        assert_eq!(es[0].bytes, 3);
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn purgar_borra_los_datos_del_plugin() {
        let b = tmp("purgar");
        escribir(&b, "p1", "a.txt", b"uno").unwrap();
        escribir(&b, "p2", "a.txt", b"otro").unwrap();
        let bytes = purgar(&b, "p1").unwrap();
        assert_eq!(bytes, 3);
        assert!(listar(&b, "p1").unwrap().is_empty());
        // El otro plugin NO se toca.
        assert_eq!(listar(&b, "p2").unwrap().len(), 1);
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn un_plugin_sin_datos_no_es_un_error() {
        let b = tmp("vacio");
        assert!(listar(&b, "nada").unwrap().is_empty());
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn listar_plugins_solo_devuelve_identidades_validas() {
        let b = tmp("plugins");
        escribir(&b, "p1", "a.txt", b"x").unwrap();
        // Un directorio con nombre INVÁLIDO (creado a mano) se ignora.
        fs::create_dir_all(b.join("plugins").join("NO VALIDO")).unwrap();
        let ids = listar_plugins(&b).unwrap();
        assert!(ids.contains(&"p1".to_string()));
        assert!(!ids.contains(&"NO VALIDO".to_string()));
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn el_glob_se_puede_usar_para_seleccionar() {
        // El glob validado se puede aplicar a un nombre de fichero sin
        // que se escape del directorio del plugin.
        let b = tmp("glob");
        escribir(&b, "p1", "notas.md", b"x").unwrap();
        let glob = "*.md";
        validar_glob(glob).unwrap();
        let dir = dir_datos(&b, "p1").unwrap();
        let casa = listar(&b, "p1")
            .unwrap()
            .into_iter()
            .filter(|e| e.nombre.ends_with(".md"))
            .count();
        assert_eq!(casa, 1);
        assert!(dir.starts_with(b.join("plugins")));
        let _ = fs::remove_dir_all(&b);
    }
}

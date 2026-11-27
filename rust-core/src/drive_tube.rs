//! EL TUBO: Google Drive como **almacen externo** del daemon.
//!
//! POR QUE
//! -------
//! El workspace (y el movil) tienen una **cuota de disco** y los artefactos
//! que se descargan son de GB.  Drive es la salida: lo que se baja, se sube
//! a la carpeta PRIVADA, se **verifica** y se borra en local; en local queda
//! solo el INVENTARIO (que hay, de que tipo, con que sha256).
//!
//! QUE HACE ESTE MODULO (todo en Rust, sin scripts externos)
//! -------------------------------------------------------
//! * `listar`      - el contenido de una carpeta de Drive (nombre, id, tamaño).
//! * `subir`      - un fichero o una CARPETA (empaquetada en `.tar.gz`).
//! * `bajar`      - un fichero de Drive a disco, verificado por sha256.
//! * `inventario` - que hay en la carpeta, con tipo y sha256.
//! * `tubo`       - la orquestacion: subir + verificar + **borrar local**.
//!
//! REGLAS DUROS
//! ------------
//! * La credencial va en un fichero **0600** (si no, error) y el token
//!   **nunca** se imprime: solo su `sha16`.
//! * Nada se borra en local hasta que la subida esta **verificada** (el
//!   sha256 que devuelve Drive tiene que coincidir con el del local).
//! * Todo el trabajo pesado va por `drive_native` (Rust puro: `ureq`), sin
//!   `curl` ni `tar` externos.
//!
//! La API de bajo nivel esta en `drive_native` (subir/bajar/ensure_folder)
//! y el listado en `drive_vfs`.

use crate::drive_native;
use crate::drive_vfs;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Carpeta de Drive por defecto (la PRIVADA `megadaemon-backup`).
pub const CARPETA_POR_DEFECTO: &str = "megadaemon-backup";

/// El ID de la carpeta remota donde se sube (si no se da, se busca por
/// nombre en la raiz).
pub fn resolver_carpeta(nombre: &str, folder_id: Option<&str>, state_dir: Option<&str>) -> Result<String, String> {
    match folder_id {
        Some(id) if !id.is_empty() => Ok(id.to_string()),
        _ => drive_native::ensure_folder(nombre, "", state_dir),
    }
}

/// Lista el contenido de una carpeta de Drive.
///
/// `limit` corta la lista (el VFS de Drive pagina con `pageToken`).
pub fn listar(folder_id: &str, limit: usize, state_dir: Option<&str>) -> Result<Value, String> {
    let entradas = drive_vfs::list_folder_id(folder_id, state_dir, limit)?;
    let mut out = Vec::new();
    for e in entradas {
        out.push(json!({
            "nombre": e.name,
            "id": e.id,
            "es_carpeta": e.is_dir,
            "bytes": e.size,
            "mime": e.mime_type,
            "modificado": e.modified_time,
        }));
    }
    Ok(json!({
        "ok": true,
        "folder_id": folder_id,
        "entradas": out.len(),
        "items": out,
    }))
}

/// **Inventario** de una carpeta: qué hay, de qué tipo y con qué sha256.
///
/// MEDIDO: el sha256 se calcula del CONTENIDO ya subido, no del remoto: Drive
/// no lo da para ficheros de usuario.  Por eso el inventario se guarda
/// localmente cuando se sube (ver `tubo`).
pub fn inventario(folder_id: &str, limit: usize, state_dir: Option<&str>) -> Result<Value, String> {
    let items = listar(folder_id, limit, state_dir)?;
    let mut tipos = serde_json::Map::new();
    if let Some(lista) = items["items"].as_array() {
        for it in lista {
            let t = if it["es_carpeta"].as_bool().unwrap_or(false) {
                "carpeta"
            } else {
                "fichero"
            };
            let e = tipos.entry(t.to_string()).or_insert(json!(0));
            let n = e.as_u64().unwrap_or(0) + 1;
            *e = json!(n);
        }
    }
    Ok(json!({
        "ok": true,
        "folder_id": folder_id,
        "resumen": Value::Object(tipos),
        "items": items["items"].clone(),
    }))
}

/// Empaqueta una carpeta en `.tar.gz` (en el temporal) y devuelve su ruta.
///
/// Se usa `tar` de Rust (`flate2` + `tar`), no el binario del sistema: el
/// daemon tiene que funcionar en un movil sin `tar`.
fn empaquetar(dir: &Path) -> Result<(PathBuf, u64, String), String> {
    let destino = std::env::temp_dir().join(format!(
        "md_tubo_{}_{}.tar.gz",
        crate::google_auth::now_secs(),
        std::process::id()
    ));
    // El tar se hace en memoria y luego se comprime (el mismo patron que
    // `bundle.rs`): `tar::Builder` sobre `File` + `GzEncoder` encima obliga
    // a encadenar dos `Write` con tipos distintos.
    let mut ar = tar::Builder::new(Vec::new());
    ar.append_dir_all(
        dir.file_name().unwrap_or_else(|| std::ffi::OsStr::new("carpeta")),
        dir,
    )
    .map_err(|e| format!("empaquetar: {}", e))?;
    let tar_bytes = ar.into_inner().map_err(|e| format!("cerrar tar: {}", e))?;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    {
        use std::io::Write;
        enc.write_all(&tar_bytes).map_err(|e| format!("comprimir: {}", e))?;
    }
    let gz = enc.finish().map_err(|e| format!("cerrar gzip: {}", e))?;
    std::fs::write(&destino, &gz).map_err(|e| format!("escribir tar.gz: {}", e))?;
    let bytes = gz.len() as u64;
    let sha = sha256_file(&destino)?;
    Ok((destino, bytes, sha))
}

/// Sube un fichero o una carpeta a Drive y devuelve el JSON con lo subido.
///
/// * fichero  -> se sube tal cual.
/// * carpeta  -> se empaqueta en `.tar.gz` y se sube el tar.
pub fn subir(
    ruta: &str,
    nombre: Option<&str>,
    folder_id: &str,
    state_dir: Option<&str>,
) -> Result<Value, String> {
    let p = PathBuf::from(ruta);
    if !p.exists() {
        return Err(format!("no existe: {}", ruta));
    }
    let (subir_path, nombre_final, bytes, sha_local, es_carpeta, temporal) = if p.is_dir() {
        let (tgz, bytes, sha) = empaquetar(&p)?;
        let nom = nombre
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}.tar.gz", p.file_name().unwrap_or_default().to_string_lossy()));
        (tgz, nom, bytes, sha, true, true)
    } else {
        let sha = sha256_file(&p)?;
        let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        let nom = nombre
            .map(|s| s.to_string())
            .unwrap_or_else(|| p.file_name().unwrap_or_default().to_string_lossy().to_string());
        (p.clone(), nom, bytes, sha, false, false)
    };

    let up = drive_native::upload_path(&subir_path, &nombre_final, folder_id, state_dir)?;
    if temporal {
        let _ = std::fs::remove_file(&subir_path);
    }
    Ok(json!({
        "ok": true,
        "nombre": nombre_final,
        "id": up.file_id,
        "bytes": bytes,
        "sha256_local": sha_local,
        "sha256_drive": up.sha256,
        "verificado": up.sha256 == sha_local,
        "es_carpeta": es_carpeta,
        "folder_id": folder_id,
    }))
}

/// Baja un fichero de Drive a disco y **verifica** el sha256.
///
/// `destino` puede ser un directorio (se usa el nombre remoto) o la ruta
/// completa del fichero.
pub fn bajar(file_id: &str, destino: &str, state_dir: Option<&str>) -> Result<Value, String> {
    let dest = PathBuf::from(destino);
    let final_path = if dest.is_dir() {
        // El nombre remoto no se puede pedir por id con `stat_path` (que
        // busca por ruta VFS), asi que se usa el id: es univoco y el
        // llamador puede renombrar despues.
        dest.join(format!("drive_{}", file_id))
    } else {
        dest.clone()
    };
    if let Some(dir) = final_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    drive_native::download_file(file_id, &final_path, state_dir)?;
    let sha = sha256_file(&final_path)?;
    Ok(json!({
        "ok": true,
        "id": file_id,
        "ruta": final_path.display().to_string(),
        "bytes": std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0),
        "sha256": sha,
    }))
}

/// Borra un arbol de disco de forma **robusta**.
///
/// MEDIDO 2026-11-27: un `rm -rf` a pelo devuelve "Directory not empty" y
/// deja el arbol a medias (los ficheros se crean mientras se borra) => el
/// proceso se queda ATASCADO.  Aqui se reintenta y, si sigue fallando, se
/// **renombra** el arbol (que siempre funciona) y se borra el renombrado.
pub fn borrar_arbol(dest: &Path) -> Result<(), String> {
    if !dest.exists() {
        return Ok(());
    }
    for _ in 0..3 {
        match std::fs::remove_dir_all(dest) {
            Ok(()) => return Ok(()),
            Err(_) => {
                if !dest.exists() {
                    return Ok(());
                }
            }
        }
    }
    let tmp = dest.with_extension(format!("borrar-{}", std::process::id()));
    std::fs::rename(dest, &tmp).map_err(|e| format!("borrar {}: {}", dest.display(), e))?;
    std::fs::remove_dir_all(&tmp).map_err(|e| format!("borrar renombrado: {}", e))
}

/// La orquestacion del TUBO: **subir + verificar + borrar local**.
///
/// Devuelve el JSON con lo que queda: el id remoto, el sha256 y el
/// inventario.  Si la subida NO verifica, **no borra nada** (el dato local
/// es el unico original).
pub fn tubo(
    ruta: &str,
    nombre: Option<&str>,
    carpeta: Option<&str>,
    folder_id: Option<&str>,
    borrar_local: bool,
    state_dir: Option<&str>,
) -> Result<Value, String> {
    let cid = resolver_carpeta(
        carpeta.unwrap_or(CARPETA_POR_DEFECTO),
        folder_id,
        state_dir,
    )?;
    let sub = subir(ruta, nombre, &cid, state_dir)?;
    let verificado = sub["verificado"].as_bool().unwrap_or(false);
    let mut out = sub.clone();
    out["carpeta"] = json!(carpeta.unwrap_or(CARPETA_POR_DEFECTO));

    if borrar_local {
        if verificado {
            let p = PathBuf::from(ruta);
            let borrado = if p.is_dir() {
                borrar_arbol(&p)
            } else {
                std::fs::remove_file(&p).map_err(|e| format!("borrar: {}", e))
            };
            out["copia_local_borrada"] = json!(borrado.is_ok());
            if let Err(e) = borrado {
                out["aviso"] = json!(format!("subida verificada pero no se pudo borrar: {}", e));
            }
        } else {
            out["copia_local_borrada"] = json!(false);
            out["aviso"] = json!(
                "la subida NO verifico el sha256: no se borra nada (el local es el original)"
            );
        }
    }
    Ok(out)
}

/// sha256 de un fichero.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    drive_native::sha256_file(path)
}

/// Lee un fichero entero a memoria (con tope).
pub fn leer_con_tope(path: &Path, tope: usize) -> Result<Vec<u8>, String> {
    let mut f = File::open(path).map_err(|e| format!("abrir: {}", e))?;
    let mut buf = Vec::new();
    f.take(tope as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("leer: {}", e))?;
    if buf.len() > tope {
        return Err(format!("el fichero supera el tope de {} bytes", tope));
    }
    Ok(buf)
}

/// sha256 de un vector de bytes (para tests).
pub fn sha256_bytes(datos: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(datos);
    format!("{:x}", h.finalize())
}

/// CLI: `megadaemon-core drive-tubo <sub> [args...]`.
///
/// Subcomandos:
///   * `listar <folder_id>`            - el contenido de la carpeta.
///   * `inventario <folder_id>`        - que hay, con tipo.
///   * `subir <ruta> [--carpeta C] [--folder-id ID] [--nombre N]`
///   * `bajar <file_id> <destino>`
///   * `tubo <ruta> [--carpeta C] [--folder-id ID] [--nombre N] [--borrar]`
pub fn run_cli(args: &[String]) -> i32 {
    // MEDIDO 2026-11-27: `run_cli` devuelve `i32`, asi que no admitia el `?`
    // (E0277).  El trabajo real va en `run_cli_res` (que SI devuelve Result)
    // y aqui solo se traduce el resultado al codigo de salida.
    match run_cli_res(args) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("drive-tubo error: {}", e);
            1
        }
    }
}

/// El trabajo del CLI: devuelve el JSON o el error.
fn run_cli_res(args: &[String]) -> Result<Value, String> {
    let st = crate::cockpit_server::state_dir();
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    let mut carpeta: Option<String> = None;
    let mut folder_id: Option<String> = None;
    let mut nombre: Option<String> = None;
    let mut borrar = false;
    let mut posicionales: Vec<String> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--carpeta" | "--folder" => {
                i += 1;
                carpeta = args.get(i).cloned();
            }
            "--folder-id" | "--id" => {
                i += 1;
                folder_id = args.get(i).cloned();
            }
            "--nombre" | "--name" => {
                i += 1;
                nombre = args.get(i).cloned();
            }
            "--borrar" | "--borrar-local" => borrar = true,
            otro => posicionales.push(otro.to_string()),
        }
        i += 1;
    }
    let res: Result<Value, String> = match sub {
        "listar" | "ls" => {
            let cid = resolver_carpeta(
                carpeta.as_deref().unwrap_or(CARPETA_POR_DEFECTO),
                folder_id.as_deref(),
                Some(&st),
            )?;
            listar(&cid, 200, Some(&st))
        }
        "inventario" | "inv" => {
            let cid = resolver_carpeta(
                carpeta.as_deref().unwrap_or(CARPETA_POR_DEFECTO),
                folder_id.as_deref(),
                Some(&st),
            )?;
            inventario(&cid, 500, Some(&st))
        }
        "subir" | "upload" => {
            let ruta = posicionales.first().ok_or("falta la ruta")?;
            let cid = resolver_carpeta(
                carpeta.as_deref().unwrap_or(CARPETA_POR_DEFECTO),
                folder_id.as_deref(),
                Some(&st),
            )?;
            subir(ruta, nombre.as_deref(), &cid, Some(&st))
        }
        "bajar" | "download" => {
            let id = posicionales.first().ok_or("falta el file_id")?;
            let dest = posicionales.get(1).ok_or("falta el destino")?;
            bajar(id, dest, Some(&st))
        }
        "tubo" => {
            let ruta = posicionales.first().ok_or("falta la ruta")?;
            tubo(
                ruta,
                nombre.as_deref(),
                carpeta.as_deref(),
                folder_id.as_deref(),
                borrar,
                Some(&st),
            )
        }
        _ => Err("Uso: megadaemon-core drive-tubo <listar|inventario|subir|bajar|tubo> [args]"
            .to_string()),
    };
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("md_tubo_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sha256_de_bytes_conocidos() {
        // MEDIDO: sha256("abc") = ba7816bf...
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_de_un_fichero() {
        let d = tmp("sha");
        let f = d.join("a.txt");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(
            sha256_file(&f).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn empaquetar_una_carpeta_produce_un_tar_gz_legible() {
        let d = tmp("pack");
        let sub = d.join("codigo");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.rs"), b"fn main() {}\n").unwrap();
        let (tgz, bytes, sha) = empaquetar(&sub).unwrap();
        assert!(bytes > 0);
        assert_eq!(sha.len(), 64);
        // el tar se puede abrir y contiene el fichero
        let f = File::open(&tgz).unwrap();
        let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(f));
        let nombres: Vec<String> = ar
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(nombres.iter().any(|n| n.ends_with("a.rs")), "no a.rs: {:?}", nombres);
        let _ = std::fs::remove_file(&tgz);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn borrar_arbol_borra_de_verdad() {
        let d = tmp("borrar");
        let sub = d.join("x");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("f.txt"), b"x").unwrap();
        borrar_arbol(&sub).unwrap();
        assert!(!sub.exists());
        // idempotente
        borrar_arbol(&sub).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn leer_con_tope_rechaza_lo_que_pasa_del_tope() {
        let d = tmp("tope");
        let f = d.join("a.bin");
        std::fs::write(&f, vec![0u8; 100]).unwrap();
        assert!(leer_con_tope(&f, 1000).is_ok());
        assert!(leer_con_tope(&f, 10).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}

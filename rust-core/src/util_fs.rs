//! Utilidades de sistema de ficheros compartidas por varios modulos.
//!
//! `walk_files` estaba triplicado (backup_full, backup_vault, call_recorder) con
//! cuerpo byte a byte identico. Aqui vive la version canonica.
//!
//! `write_atomic` es la **unica** forma canonica de escribir un fichero de
//! estado: varios modulos lo tenian reimplementado (y `cert_vault` lo tenia
//! bien, pero privado).  Escribir con `fs::write` **trunca** el destino: si
//! el proceso muere a mitad, el fichero queda corrupto y el estado anterior
//! se pierde sin dejar rastro.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Escribe `data` en `path` de forma **atomica**.
///
/// * se escribe en `<path>.tmp-<pid>` (nunca en el destino final).  El
///   sufijo con el PID evita que dos procesos (el daemon y la CLI) se pisen
///   el mismo temporal;
/// * el temporal se crea **con el modo pedido** (`0600` para secretos): no
///   existe ni un instante con el modo por defecto del umask;
/// * `fsync` antes de renombrar (el contenido está en disco antes de ser
///   visible bajo su nombre final);
/// * `rename` ⇒ el destino final aparece **completo o no aparece nunca**.
///
/// `mode` se ignora en plataformas que no son Unix.
pub fn write_atomic(path: &Path, data: &[u8], mode: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {}", parent.display(), e))?;
        }
    }
    let tmp = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("tmp"),
        std::process::id()
    ));
    {
        let mut f = crear_con_modo(&tmp, mode)
            .map_err(|e| format!("crear {}: {}", tmp.display(), e))?;
        // Por si el temporal ya existía de una ejecución anterior: `mode` en
        // `OpenOptions` sólo se aplica al CREAR el fichero.
        set_mode(&tmp, mode);
        if let Err(e) = f
            .write_all(data)
            .and_then(|_| f.sync_all())
        {
            let _ = fs::remove_file(&tmp);
            return Err(format!("escribir {}: {}", tmp.display(), e));
        }
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "renombrar {} -> {}: {}",
            tmp.display(),
            path.display(),
            e
        ));
    }
    set_mode(path, mode);
    Ok(())
}

/// Crea (o trunca) un fichero con el modo pedido **en el momento de crearlo**.
#[cfg(unix)]
fn crear_con_modo(path: &Path, mode: u32) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(mode)
        .open(path)
}

#[cfg(not(unix))]
fn crear_con_modo(path: &Path, _mode: u32) -> std::io::Result<fs::File> {
    fs::File::create(path)
}

/// Fija el modo de un fichero (Unix).  En otras plataformas no hace nada.
#[cfg(unix)]
pub fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
        let mut perm = meta.permissions();
        perm.set_mode(mode);
        let _ = fs::set_permissions(path, perm);
    }
}

#[cfg(not(unix))]
pub fn set_mode(_path: &Path, _mode: u32) {}

/// Recorre `root` en profundidad y apila en `out` los ficheros regulares.
/// No sigue enlaces simbolicos (evita ciclos) y ordena por nombre para que el
/// resultado sea reproducible.
pub fn walk_files(root: &Path, out: &mut Vec<PathBuf>) {
    let rd = match fs::read_dir(root) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut entries: Vec<fs::DirEntry> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let md = match fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if md.file_type().is_symlink() {
            continue;
        }
        if md.is_dir() {
            walk_files(&p, out);
        } else if md.is_file() {
            out.push(p);
        }
    }
}

#[cfg(test)]
mod tests_walk {
    use super::*;

    #[test]
    fn walk_files_recorre_en_profundidad_y_ordena() {
        let dir = std::env::temp_dir().join("md_util_fs_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("b.txt"), b"b").unwrap();
        std::fs::write(dir.join("a.txt"), b"a").unwrap();
        std::fs::write(dir.join("sub/c.txt"), b"c").unwrap();
        let mut out = Vec::new();
        walk_files(&dir, &mut out);
        let names: Vec<String> = out
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["a.txt", "b.txt", "c.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn walk_files_no_peta_con_una_ruta_inexistente() {
        let mut out = Vec::new();
        walk_files(Path::new("/no/existe/esto"), &mut out);
        assert!(out.is_empty());
    }
}

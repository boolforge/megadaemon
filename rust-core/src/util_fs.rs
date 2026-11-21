//! Utilidades de sistema de ficheros compartidas por varios modulos.
//!
//! `walk_files` estaba triplicado (backup_full, backup_vault, call_recorder) con
//! cuerpo byte a byte identico. Aqui vive la version canonica.

use std::fs;
use std::path::{Path, PathBuf};

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

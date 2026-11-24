//! **Plan de hooks por dirección**: las funciones Go del `.so` de
//! Antigravity (el language server).
//!
//! POR QUÉ UN PLAN Y NO SÍMBOLOS
//! ------------------------------
//! El `.so` es un binario **Go 1.28 stripped**: sus símbolos dinámicos son
//! 558 (sólo del runtime) ⇒ **no hay `dlsym` que valga** y los ganchos de
//! libc no disparan (Go usa syscalls crudas).  La única forma de hookear
//! una función interna es saber **su dirección**, y esa la da el
//! `pclntab` (lo resuelve `go-retool`, ya en MegaDaemon).
//!
//! FORMATO DEL PLAN (texto plano, una línea por hook, tabulado)
//! ----------------------------------------------------------
//! ```text
//! RUTA_DEL_MODULO<TAB>VA_ELF_HEX<TAB>ID<TAB>TAREA
//! ```
//!
//! * `RUTA_DEL_MODULO`: ruta del fichero del módulo (o su nombre) tal como
//!   se cargó en el proceso (p. ej. `liblanguage_server.so`).
//! * `VA_ELF_HEX`: dirección virtual de la función **en el ELF** (sin el
//!   bias de carga).  El payload le suma el base del módulo en runtime.
//! * `ID`: nombre corto del hook (para el NDJSON).
//! * `TAREA`: `observar` (loguear) o `sustituir_url` (cambiar el retorno).
//!
//! El texto plano (y no JSON) es a propósito: el payload **no** depende de
//! serde y así el plan se puede generar con cualquier herramienta.
//!
//! ⚠️ **La dirección buena es la del `pclntab`**: el índice de Ghidra del
//! corpus está **+0x1000000 (1 MiB)** por encima (medido, 93 % de las
//! funciones).  Un plan hecho con direcciones de Ghidra apunta 1 MiB
//! equivocado.

/// Un hook del plan.
#[derive(Debug, Clone)]
pub struct Objetivo {
    /// Ruta o nombre del módulo donde vive la función.
    pub modulo: String,
    /// Dirección de la función en el ELF (sin bias).
    pub va_elf: u64,
    /// Nombre corto (sale en el NDJSON).
    pub id: String,
    /// Qué se hace con la llamada (`observar`, `sustituir_url`, ...).
    pub tarea: String,
}

/// Dónde se busca el plan por defecto (en el móvil).
pub const RUTA_PLAN_POR_DEFECTO: &str = "/data/local/tmp/megadaemon/hooks-plan.txt";

/// Variable de entorno que puede sobreescribir la ruta del plan.
pub const ENV_PLAN: &str = "MEGADAEMON_HOOK_PLAN";

/// Ruta del plan: la del entorno si está, si no la de por defecto.
pub fn ruta() -> String {
    std::env::var(ENV_PLAN).unwrap_or_else(|_| RUTA_PLAN_POR_DEFECTO.to_string())
}

/// Lee el plan.  Las líneas malformed se ignoran (un plan a medias es
/// mejor que ningún plan).
pub fn lee(ruta: &str) -> Vec<Objetivo> {
    let Ok(txt) = std::fs::read_to_string(ruta) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for linea in txt.lines() {
        let l = linea.trim_end_matches(['\r', '\n']);
        if l.trim().is_empty() || l.trim_start().starts_with('#') {
            continue;
        }
        let p: Vec<&str> = l.split('\t').collect();
        if p.len() < 4 {
            continue;
        }
        let va = u64::from_str_radix(p[1].trim().trim_start_matches("0x"), 16).unwrap_or(0);
        if va == 0 {
            continue;
        }
        out.push(Objetivo {
            modulo: p[0].trim().to_string(),
            va_elf: va,
            id: p[2].trim().to_string(),
            tarea: p[3].trim().to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_un_plan_de_una_linea_por_hook() {
        let txt = "# comentario\n\
                   /data/x/liblanguage_server.so\t0x66a2f78\tGenerateContent\tobservar\n\
                   \n\
                   liblanguage_server.so\t0x66a27f8\tStreamGenerate\tobservar\n";
        let dir = std::env::temp_dir().join("md_plan_test.txt");
        std::fs::write(&dir, txt).unwrap();
        let v = lee(dir.to_str().unwrap());
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].va_elf, 0x66a2f78);
        assert_eq!(v[0].id, "GenerateContent");
        assert_eq!(v[0].tarea, "observar");
        assert_eq!(v[1].modulo, "liblanguage_server.so");
        let _ = std::fs::remove_file(&dir);
    }

    #[test]
    fn ignora_lineas_mal_formadas() {
        let dir = std::env::temp_dir().join("md_plan_bad.txt");
        std::fs::write(&dir, "basura\nsin-tab\nmod\tzzz\tid\tobs\n").unwrap();
        assert!(lee(dir.to_str().unwrap()).is_empty());
        let _ = std::fs::remove_file(&dir);
    }
}

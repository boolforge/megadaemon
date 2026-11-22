//! "Libro legible" del binario Go de Antigravity.
//!
//! HALLAZGO CLAVE (medido 2026-11-22): el export de `go-retool` (`go.funcs.tsv`)
//! tiene TRES columnas:
//!
//! ```text
//! 0x4d06cb8  internal_abi.__RegArgs_.Dump  go: internal/abi.(*RegArgs).Dump | third_party/go/gc/src/internal/abi/abi.go:47
//! ```
//!
//! 1. la **dirección** en el binario;
//! 2. el **símbolo Go** (los nombres NO están ofuscados);
//! 3. el **paquete** y el **fichero:línea ORIGINAL** del código Go del que
//!    salió esa función.
//!
//! ⇒ Con las tres columnas se puede **reconstruir el esqueleto legible del
//! programa**: qué paquetes tiene, qué ficheros, qué función está en qué
//! línea.  No es una aproximación: es el índice que el propio binario lleva
//! dentro (el `pclntab`), validado contra `unstrip` y `go.funcs.tsv`.
//!
//! Qué da este módulo:
//! * `mapa()` ⇒ la **arquitectura**: paquetes (propios y de la stdlib) con
//!   su número de funciones y rango de direcciones;
//! * `esqueleto()` ⇒ el **código legible por fichero**, en orden de línea,
//!   con cada función en su línea real (para entender el programa entero);
//! * `buscar()` ⇒ localizar anchors por nombre (catálogo de modelos,
//!   proxy, MCP, BYOM…);
//! * `funciones_de()` ⇒ el desglose de un paquete.
//!
//! Es estático: sólo lee el TSV. No ejecuta nada del binario analizado.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// Una función tal y como la declara el `pclntab`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Funcion {
    /// Dirección en el binario.
    pub addr: u64,
    /// Dirección en hexadecimal (`0x...`).
    pub addr_hex: String,
    /// Símbolo Go (`paquete.Funcion`).
    pub simbolo: String,
    /// Paquete Go (`go: internal/abi`).
    pub paquete: String,
    /// Nombre legible (`(*RegArgs).Dump`).
    pub nombre: String,
    /// Fichero Go original.
    pub fichero: String,
    /// Línea original.
    pub linea: u32,
}

impl Funcion {
    /// ¿Es código propio de la aplicación (no de la stdlib ni de Go)?
    pub fn es_propia(&self) -> bool {
        !self.fichero.starts_with("third_party/go/") && !self.fichero.starts_with("GOROOT/")
    }
}

/// Divide una línea del TSV en sus tres campos.
pub fn parsear_linea(linea: &str) -> Option<Funcion> {
    let mut it = linea.split('\t');
    let addr_hex = it.next()?.trim();
    let simbolo = it.next()?.trim().to_string();
    let resto = it.next()?.trim().to_string();
    if addr_hex.is_empty() || simbolo.is_empty() {
        return None;
    }
    let addr = u64::from_str_radix(addr_hex.trim_start_matches("0x"), 16).ok()?;
    // "go: pkg.(*T).M | fichero.go:47"
    let (desc, fichero_linea) = match resto.split_once('|') {
        Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
        None => (resto.clone(), String::new()),
    };
    // "go: internal/abi.(*RegArgs).Dump" -> paquete + nombre
    let desc = desc.strip_prefix("go:").unwrap_or(&desc).trim();
    let (paquete, nombre) = match desc.find('.') {
        Some(i) => (desc[..i].to_string(), desc[i + 1..].to_string()),
        None => (desc.to_string(), String::new()),
    };
    let (fichero, linea) = match fichero_linea.rsplit_once(':') {
        Some((f, l)) => (f.trim().to_string(), l.trim().parse::<u32>().unwrap_or(0)),
        None => (fichero_linea, 0),
    };
    Some(Funcion {
        addr,
        addr_hex: addr_hex.to_string(),
        simbolo,
        paquete,
        nombre,
        fichero,
        linea,
    })
}

/// Lee todas las funciones del TSV.
pub fn leer_tsv(ruta: &Path) -> Result<Vec<Funcion>, String> {
    let txt = std::fs::read_to_string(ruta)
        .map_err(|e| format!("leer {}: {e}", ruta.display()))?;
    let mut out = Vec::new();
    for l in txt.lines() {
        if l.trim().is_empty() {
            continue;
        }
        if let Some(f) = parsear_linea(l) {
            out.push(f);
        }
    }
    if out.is_empty() {
        return Err("el TSV no tiene ninguna linea interpretable".to_string());
    }
    Ok(out)
}

/// Un paquete y su resumen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumenPaquete {
    pub paquete: String,
    pub funciones: usize,
    pub ficheros: usize,
    pub addr_min: String,
    pub addr_max: String,
    /// Código propio de la app (no stdlib).
    pub propia: bool,
}

/// **La arquitectura del binario**, agrupada por paquete.
pub fn mapa(funciones: &[Funcion]) -> Vec<ResumenPaquete> {
    let mut acc: BTreeMap<String, (usize, BTreeMap<String, ()>, u64, u64, bool)> = BTreeMap::new();
    for f in funciones {
        let e = acc
            .entry(f.paquete.clone())
            .or_insert((0, BTreeMap::new(), u64::MAX, 0, false));
        e.0 += 1;
        e.1.insert(f.fichero.clone(), ());
        e.2 = e.2.min(f.addr);
        e.3 = e.3.max(f.addr);
        if f.es_propia() {
            e.4 = true;
        }
    }
    let mut v: Vec<ResumenPaquete> = acc
        .into_iter()
        .map(|(p, (n, fs, mn, mx, propia))| ResumenPaquete {
            paquete: p,
            funciones: n,
            ficheros: fs.len(),
            addr_min: format!("0x{mn:x}"),
            addr_max: format!("0x{mx:x}"),
            propia,
        })
        .collect();
    // primero el código propio, luego por tamaño
    v.sort_by(|a, b| {
        b.propia
            .cmp(&a.propia)
            .then(b.funciones.cmp(&a.funciones))
            .then(a.paquete.cmp(&b.paquete))
    });
    v
}

/// El mapa como JSON (para la web y las tools).
pub fn mapa_json(funciones: &[Funcion], top: usize) -> Value {
    let todos = mapa(funciones);
    let total = todos.len();
    let propias = todos.iter().filter(|p| p.propia).count();
    json!({
        "total_funciones": funciones.len(),
        "total_paquetes": total,
        "paquetes_propios": propias,
        "top": todos.into_iter().take(top).collect::<Vec<_>>(),
    })
}

/// Funciones que coinciden con un patrón (sin distinguir mayúsculas).
pub fn buscar(funciones: &[Funcion], patron: &str, limite: usize) -> Vec<Funcion> {
    let p = patron.to_ascii_lowercase();
    let mut out: Vec<Funcion> = funciones
        .iter()
        .filter(|f| {
            f.simbolo.to_ascii_lowercase().contains(&p)
                || f.nombre.to_ascii_lowercase().contains(&p)
                || f.fichero.to_ascii_lowercase().contains(&p)
        })
        .cloned()
        .collect();
    // las más（小 dirección primero) y limitado
    out.sort_by_key(|f| f.addr);
    out.truncate(limite);
    out
}

/// **El código legible**: el esqueleto del programa por fichero y línea.
///
/// Para cada fichero Go, las funciones en orden de línea original, con la
/// línea real.  Esto es lo que permite *entender el programa entero* sin
/// adivinar: los nombres, los paquetes, las líneas y las direcciones son los
/// del binario.
pub fn esqueleto(funciones: &[Funcion], filtro_fichero: &str, limite_funciones: usize) -> String {
    let mut por_fichero: BTreeMap<String, Vec<&Funcion>> = BTreeMap::new();
    let f = filtro_fichero.to_ascii_lowercase();
    for x in funciones {
        if f.is_empty() || x.fichero.to_ascii_lowercase().contains(&f) {
            por_fichero
                .entry(x.fichero.clone())
                .or_default()
                .push(x);
        }
    }
    let mut out = String::new();
    let mut total = 0usize;
    for (fichero, mut fs) in por_fichero {
        fs.sort_by_key(|x| (x.linea, x.addr));
        out.push_str(&format!("\n// ===== {fichero} ({} funciones) =====\n", fs.len()));
        for x in fs {
            if total >= limite_funciones {
                out.push_str("\n// ... (truncado)\n");
                return out;
            }
            total += 1;
            out.push_str(&format!(
                "// {:>6}  {}  @{}\nfunc ({}) {{}}\n",
                x.linea, x.simbolo, x.addr_hex, x.nombre
            ));
        }
    }
    out
}

/// Desglose de un paquete concreto.
pub fn funciones_de(funciones: &[Funcion], paquete: &str) -> Vec<Funcion> {
    let p = paquete.to_ascii_lowercase();
    let mut v: Vec<Funcion> = funciones
        .iter()
        .filter(|f| f.paquete.to_ascii_lowercase().contains(&p))
        .cloned()
        .collect();
    v.sort_by_key(|f| (f.fichero.clone(), f.linea, f.addr));
    v
}

/// Informe listo para publicar (el "libro" en Markdown).
pub fn informe_markdown(funciones: &[Funcion], ruta_tsv: &Path, top: usize) -> String {
    let mut s = String::new();
    s.push_str("# El binario de Antigravity, legible\n\n");
    s.push_str(&format!(
        "Fuente: `{}` (export de `go-retool`, pclntab del propio binario).\n\n",
        ruta_tsv.display()
    ));
    s.push_str(&format!(
        "**{} funciones** en **{} paquetes**.\n\n",
        funciones.len(),
        mapa(funciones).len()
    ));
    s.push_str("## Arquitectura (top paquetes)\n\n");
    s.push_str("| paquete | funciones | ficheros | rango | propio |\n");
    s.push_str("|---|---:|---:|---|---|\n");
    for p in mapa(funciones).into_iter().take(top) {
        s.push_str(&format!(
            "| `{}` | {} | {} | {}–{} | {} |\n",
            p.paquete,
            p.funciones,
            p.ficheros,
            p.addr_min,
            p.addr_max,
            if p.propia { "sí" } else { "stdlib" }
        ));
    }
    s.push_str("\n## Código propio (los paquetes de la app, no la stdlib)\n\n");
    let propios: Vec<ResumenPaquete> = {
        let mut m = mapa(funciones);
        m.retain(|p| p.propia);
        m
    };
    s.push_str(&format!("**{} paquetes propios**\n\n", propios.len()));
    for p in propios.iter().take(top) {
        s.push_str(&format!("- `{}` — {} funciones\n", p.paquete, p.funciones));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_una_linea_real() {
        let l = "0x4d06cb8\tinternal_abi.__RegArgs_.Dump\tgo: internal/abi.(*RegArgs).Dump | third_party/go/gc/src/internal/abi/abi.go:47";
        let f = parsear_linea(l).expect("debe parsear");
        assert_eq!(f.addr, 0x4d06cb8);
        assert_eq!(f.paquete, "internal/abi");
        assert_eq!(f.nombre, "(*RegArgs).Dump");
        assert_eq!(f.fichero, "third_party/go/gc/src/internal/abi/abi.go");
        assert_eq!(f.linea, 47);
        assert!(!f.es_propia());
    }

    #[test]
    fn detecta_el_codigo_propio() {
        let l = "0x100\tmain.main\tgo: main.main | main.go:12";
        let f = parsear_linea(l).unwrap();
        assert!(f.es_propia());
        assert_eq!(f.paquete, "main");
    }

    #[test]
    fn el_mapa_agrupa_por_paquete() {
        let fs = vec![
            parsear_linea("0x1\tmain.a\tgo: main.a | main.go:1").unwrap(),
            parsear_linea("0x2\tmain.b\tgo: main.b | main.go:2").unwrap(),
            parsear_linea("0x3\tfmt.Println\tgo: fmt.Println | fmt.go:3").unwrap(),
        ];
        let m = mapa(&fs);
        assert_eq!(m.len(), 2);
        let main = m.iter().find(|p| p.paquete == "main").unwrap();
        assert_eq!(main.funciones, 2);
        assert!(main.propia);
    }

    #[test]
    fn el_esqueleto_agrupa_y_ordena_por_linea() {
        let fs = vec![
            parsear_linea("0x2\tmain.b\tgo: main.b | main.go:20").unwrap(),
            parsear_linea("0x1\tmain.a\tgo: main.a | main.go:10").unwrap(),
        ];
        let s = esqueleto(&fs, "main.go", 10);
        let i_a = s.find("main.a").unwrap();
        let i_b = s.find("main.b").unwrap();
        assert!(i_a < i_b, "deben salir en orden de linea");
    }

    #[test]
    fn buscar_encuentra_por_nombre() {
        let fs = vec![
            parsear_linea("0x1\tmain.GetAvailableModels\tgo: main.GetAvailableModels | main.go:1").unwrap(),
            parsear_linea("0x2\tmain Otro\tgo: main.Otro | main.go:2").unwrap(),
        ];
        let r = buscar(&fs, "getavailablemodels", 10);
        assert_eq!(r.len(), 1);
    }
}

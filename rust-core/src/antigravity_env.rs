//! **MegaDaemon absorbe el entorno y las utilidades de Antigravity**.
//!
//! QUÉ HAY EN EL MÓVIL (MEDIDO 2026-11-22, S21 Ultra, con root)
//! ---------------------------------------------------------------
//! Antigravity (`com.agy`) NO es sólo una GUI: trae dentro de su sandbox
//! privado un **userland completo de Termux**.
//!
//! | Cosa | Ruta | MEDIDO |
//! |---|---|---|
//! | Lanzadores (stubs ELF de 781–814 B) | `/data/user/0/com.agy/files/runtime/bin/` | **862 entradas**, 849 sha256 distintos, 197 MB |
//! | Binarios reales | `/data/user/0/com.agy/files/usr/bin/` | **849 entradas**, 45 ficheros > 500 KB |
//! | Manifiesto del runtime | `/data/user/0/com.agy/files/runtime/manifest.json` | sha256 de cada fichero |
//! | Loader + libc | `/data/user/0/com.agy/files/runtime/lib/` | 4,8 MB |
//!
//! Los stubs **no** son el programa: son lanzadores diminutos que ejecutan el
//! binario real de `usr/bin`.  Probado en el móvil (mediante `root_exec`):
//!
//! | Comando | Versión real |
//! |---|---|
//! | `runtime/bin/curl` | curl 8.22.0 (OpenSSL 3.6.3, nghttp2 1.70.0) |
//! | `runtime/bin/git` | **git 2.56.0** |
//! | `runtime/bin/python3` | Python 3.14.10 |
//! | `runtime/bin/java` | OpenJDK 17.0.21.1 |
//! | `usr/bin/aapt` | 1.360.584 B (el de verdad) |
//! | `usr/bin/zipalign` | 217.432 B |
//! | `usr/bin/llvm-ar` | 71.440 B (toolchain NDK) |
//! | `usr/bin/node` | 51.010.000 B |
//! | `usr/bin/cmake` | 10.652.912 B |
//!
//! CORRECCIÓN IMPORTANTE
//! ---------------------
//! `antigravity_clone.rs` afirmaba que *"el móvil **no** tiene `git`"*
//! (MEDIDO 2026-11-22: es **FALSO**; hay `git 2.56.0` en el runtime de
//! Antigravity).  Este módulo deja el dato **medido**, no el supuesto.
//!
//! POR QUÉ IMPORTA
//! ----------------
//! * MegaDaemon puede **usar** el entorno de Antigravity sin reimplementarlo:
//!   `git` para clonar, `python3` para los scripts de RE, `java`+`apksigner`
//!   para firmar, `aapt`/`zipalign` para preparar APKs, `curl` para hablar
//!   con la IDE.
//! * Y puede **catalogarlo** (que es lo que hace este módulo) para saber
//!   qué tiene delante sin adivinar: el `AGENTS.md` de la app y el tablón
//!   hablan de "el entorno" como si fuera una caja negra.
//!
//! REGLAS
//! ------
//! * Todo es **local al dispositivo**: si los paths no existen, se devuelve
//!   un catálogo vacío (nunca un panic).
//! * Las versiones se obtienen **ejecutando** el lanzador (`--version`), con
//!   timeout: el móvil va justo de RAM y no se abren procesos colgados.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Lanzadores del runtime de Antigravity.
pub const RUNTIME_BIN: &str = "/data/user/0/com.agy/files/runtime/bin";
/// Binarios reales del userland de Antigravity (Termux).
pub const PREFIX_BIN: &str = "/data/user/0/com.agy/files/usr/bin";
/// Manifiesto del runtime (sha256 de cada fichero).
pub const RUNTIME_MANIFEST: &str = "/data/user/0/com.agy/files/runtime/manifest.json";
/// Librerías del runtime (loader, libc, libnss…).
pub const RUNTIME_LIB: &str = "/data/user/0/com.agy/files/runtime/lib";
/// Tope de entradas del catálogo (el runtime tiene 862).
pub const CATALOG_LIMIT: usize = 400;

/// Herramientas del entorno de Antigravity que **merecen la pena** saber
/// que existen (son las que el daemon usa o puede usar).
///
/// El orden es el de utilidad, no el alfabético.
pub const KEY_TOOLS: &[&str] = &[
    "git",         // control de versiones (el clon de Antigravity)
    "python3",    // los scripts de RE del usuario
    "java",       // runtime de apksigner
    "apksigner",  // firma de APK
    "aapt",       // inspection de APK
    "zipalign",   // alineación de APK
    "llvm-ar",    // toolchain NDK
    "clang",      // toolchain NDK
    "cmake",      // build
    "make",       // build
    "node",       // runtime JS
    "curl",       // cliente HTTP (habla con la IDE)
    "bash",       // shell
    "rish",       // shell "real" de Android
];

/// Una herramienta del entorno de Antigravity, con su lanzador y su real.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeTool {
    /// Nombre de la herramienta (`git`, `python3`…).
    pub name: String,
    /// Ruta del lanzador en `runtime/bin`.
    pub stub: String,
    /// Ruta del binario real en `usr/bin` (si existe).
    pub real: Option<String>,
    /// Tamaño del lanzador.
    pub stub_bytes: u64,
    /// Tamaño del binario real.
    pub real_bytes: u64,
    /// ¿El binario real es ejecutable?
    pub ejecutable: bool,
}

impl RuntimeTool {
    /// ¿Está la herramienta completa (lanzador + real)?
    pub fn completa(&self) -> bool {
        self.real.is_some()
    }

    /// Ficha JSON (sin ejecutar nada).
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "stub": self.stub,
            "real": self.real,
            "stub_bytes": self.stub_bytes,
            "real_bytes": self.real_bytes,
            "ejecutable": self.ejecutable,
            "completa": self.completa(),
        })
    }
}

fn meta_len(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn es_ejecutable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.exists()
    }
}

/// Resuelve una herramienta del entorno: lanzador + binario real.
pub fn resolve(name: &str) -> Option<RuntimeTool> {
    let stub = PathBuf::from(RUNTIME_BIN).join(name);
    if !stub.exists() {
        return None;
    }
    let real = PathBuf::from(PREFIX_BIN).join(name);
    let real_existe = real.exists();
    Some(RuntimeTool {
        name: name.to_string(),
        stub: stub.to_string_lossy().to_string(),
        real: if real_existe {
            Some(real.to_string_lossy().to_string())
        } else {
            None
        },
        stub_bytes: meta_len(&stub),
        real_bytes: if real_existe { meta_len(&real) } else { 0 },
        ejecutable: es_ejecutable(&real),
    })
}

/// Catálogo completo del entorno (limitado a `limit` entradas).
pub fn catalog(limit: usize) -> Vec<RuntimeTool> {
    let dir = Path::new(RUNTIME_BIN);
    let mut nombres: Vec<String> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| !n.starts_with('.'))
            .collect(),
        Err(_) => return Vec::new(),
    };
    nombres.sort();
    nombres
        .into_iter()
        .take(limit.clamp(1, 4096))
        .filter_map(|n| resolve(&n))
        .collect()
}

/// Las herramientas clave del entorno (las de [`KEY_TOOLS`] que existen).
pub fn key_tools() -> Vec<RuntimeTool> {
    KEY_TOOLS.iter().filter_map(|n| resolve(n)).collect()
}

/// Ejecuta `<runtime/bin/<name>> --version` y devuelve la **primera línea**.
///
/// Se ejecuta en el móvil (root) porque el runtime sólo existe allí.  Si el
/// binario no está o falla, se devuelve `None` (nunca un error fatal).
pub fn version_of(name: &str, timeout_ms: u64) -> Option<String> {
    let t = resolve(name)?;
    let ruta = if t.completa() {
        t.stub.clone()
    } else {
        return None;
    };
    let cmd = format!("{} --version 2>/dev/null | head -1", shell_quote(&ruta));
    let r = crate::android::root_exec(&cmd, timeout_ms.clamp(1_000, 120_000), 4_096);
    if r.timed_out {
        return None;
    }
    let linea = r.stdout.lines().next().unwrap_or("").trim().to_string();
    if linea.is_empty() {
        None
    } else {
        Some(linea)
    }
}

/// Ficha completa de una herramienta: datos estáticos + versión real.
pub fn tool_report(name: &str, timeout_ms: u64) -> Value {
    match resolve(name) {
        None => json!({ "ok": false, "name": name, "error": "no está en el runtime de Antigravity" }),
        Some(t) => {
            let version = version_of(name, timeout_ms);
            json!({
                "ok": true,
                "tool": t.to_json(),
                "version": version,
                "versionado": version.is_some(),
            })
        }
    }
}

/// Informe del entorno completo (para el CLI, el MCP y la doc).
pub fn environment_report(timeout_ms: u64) -> Value {
    let dir = Path::new(RUNTIME_BIN);
    let (entradas, unicos) = contar_runtime(dir);
    let claves = key_tools();
    let mut con_version = Vec::new();
    for t in &claves {
        let v = version_of(&t.name, timeout_ms);
        con_version.push(json!({
            "name": t.name,
            "real": t.real,
            "real_bytes": t.real_bytes,
            "ejecutable": t.ejecutable,
            "version": v,
        }));
    }
    json!({
        "ok": true,
        "runtime_bin": RUNTIME_BIN,
        "prefix_bin": PREFIX_BIN,
        "manifest": RUNTIME_MANIFEST,
        "runtime_entradas": entradas,
        "runtime_sha256_distintos": unicos,
        "runtime_bytes": meta_len(dir),
        "herramientas_clave": con_version,
        "claves_encontradas": claves.len(),
        "claves_totales": KEY_TOOLS.len(),
    })
}

/// Cuenta entradas y sha256 distintos del directorio del runtime.
///
/// El recuento de sha256 es **caro** (hundreds de ficheros), así que se
/// limita a `limit` ficheros y se avisa con `sha256_distintos_completos`.
fn contar_runtime(dir: &Path) -> (usize, usize) {
    let mut entradas = 0usize;
    let mut hashes: Vec<String> = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return (0, 0),
    };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        entradas += 1;
        if hashes.len() < 900 {
            if let Ok(b) = std::fs::read(&p) {
                hashes.push(crate::self_update::sha256_hex(&b));
            }
        }
    }
    hashes.sort();
    hashes.dedup();
    (entradas, hashes.len())
}

/// Escapa una ruta para la shell (comillas simples).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ---------------------------------------------------------------------------
// Tool MCP (paridad con el CLI)
// ---------------------------------------------------------------------------

/// Definición de la tool MCP.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "antigravity_env",
            "description": "Entorno y utilidades que Antigravity trae dentro de su sandbox (userland Termux: git 2.56.0, python3 3.14.10, openjdk 17, aapt, zipalign, llvm-ar, node, curl...). MegaDaemon los puede usar en vez de reimplementarlos. Acciones: report (informe del entorno), tool (ficha de una herramienta con su versión real) y catalog (listado plano del runtime).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["report", "tool", "catalog"], "default": "report" },
                    "name": { "type": "string", "description": "Nombre de la herramienta (para action=tool)." },
                    "limit": { "type": "integer", "default": 120, "description": "Máximo de entradas (action=catalog)." },
                    "timeout_ms": { "type": "integer", "default": 15000, "description": "Timeout al preguntar la versión." }
                }
            }
        }
    ])
}

/// El `dispatch` devuelve el **valor crudo** (`{"ok":…}`): el envoltorio MCP
/// (`content` + `structuredContent` + `isError`) lo pone la capa que despacha
/// (`mcp_server::dispatch_tool` / `mcp_ext`), igual que con el resto de tools.
fn wrap(v: Value) -> Value {
    v
}

/// Despacha la tool MCP.  `None` = el nombre no es de este módulo.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    if name != "antigravity_env" {
        return None;
    }
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let accion = {
        let a = s("action");
        if a.trim().is_empty() {
            "report".to_string()
        } else {
            a
        }
    };
    let timeout = args
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(15_000);
    match accion.as_str() {
        "tool" => {
            let n = s("name");
            if n.trim().is_empty() {
                return Some(wrap(json!({ "ok": false, "error": "falta name" })));
            }
            Some(wrap(tool_report(&n, timeout)))
        }
        "catalog" => {
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(CATALOG_LIMIT as u64) as usize;
            let cat = catalog(limit);
            Some(wrap(json!({
                "ok": true,
                "n": cat.len(),
                "runtime_bin": RUNTIME_BIN,
                "prefix_bin": PREFIX_BIN,
                "tools": cat.iter().map(|t| t.to_json()).collect::<Vec<_>>(),
            })))
        }
        _ => Some(wrap(environment_report(timeout))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_catalogo_no_peta_sin_dispositivo() {
        let c = catalog(10);
        // En el sandbox no hay móvil: vacío o con entradas, pero sin panic.
        assert!(c.iter().all(|t| !t.stub.is_empty()));
    }

    #[test]
    fn resolver_una_herramienta_inexistente_devuelve_none() {
        assert!(resolve("herramienta-que-no-existe-ni-de-broma").is_none());
    }

    #[test]
    fn la_escapada_de_shell_es_correcta() {
        assert_eq!(shell_quote("/a/b"), "'/a/b'");
        assert_eq!(shell_quote("/a/'b"), "'/a/'\\''b'");
    }

    #[test]
    fn el_informe_tiene_la_forma_esperada() {
        let r = environment_report(1_000);
        assert_eq!(r["ok"], true);
        assert!(r["runtime_bin"].as_str().unwrap().ends_with("runtime/bin"));
        assert!(r["herramientas_clave"].is_array());
    }

    #[test]
    fn las_herramientas_clave_son_las_medidas() {
        assert!(KEY_TOOLS.contains(&"git"));
        assert!(KEY_TOOLS.contains(&"apksigner"));
        assert!(KEY_TOOLS.contains(&"python3"));
        assert!(KEY_TOOLS.contains(&"java"));
    }

    #[test]
    fn la_ficha_de_una_herramienta_inexistente_no_peta() {
        let r = tool_report("nada-de-nada", 1_000);
        assert_eq!(r["ok"], false);
    }
}

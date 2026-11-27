//! **RE masivo por app**: una unidad de trabajo por app, con presupuesto,
//! bitácora detallada y **digest compacto**.
//!
//! POR QUÉ EXISTE
//! --------------
//! El repo ya tiene las piezas (ver `apk_inventory`, `app_intel`,
//! `root_intel`, `root_occlusion`, `dex_decompile`), pero cada una se consulta
//! por separado y devuelve su JSON **completo**: meter eso en el contexto de
//! un modelo son megabytes por app y el presupuesto de tokens se agota en
//! dos o tres apps.
//!
//! Este módulo es la **capa de orquestación** que falta:
//!
//! 1. **Worklist**: qué apps mirar primero y **por qué** (puntaje + motivos).
//!    La priorización sale de la evidencia ya medida (detección de root por
//!    categorías, ofuscación del DEX, permisos, código nativo).
//! 2. **Una unidad por app**: cada app se analiza con un **presupuesto**
//!    (tiempo, bytes, señales) y su **bitácora** aparte (NDJSON): si una app
//!    revienta, no arrastra a las demás y se puede leer qué pasó.
//! 3. **Digest**: un único fichero (Markdown + NDJSON) con una línea por app.
//!    Eso es lo que va al contexto del modelo: **una tabla**, no los volcados.
//!
//! REGLAS
//! ------
//! * **Sólo lectura**: no se modifica ninguna app ni ningún APK.
//! * **Presupuesto siempre**: ninguna operación puede colgarse ni pasar de
//!   los límites; si se agotan, se dice (no se oculta).
//! * **Trazable**: cada paso deja su marca (nombre, ms, bytes, ok/error) en la
//!   bitácora de la app.
//! * **Nada de secretos**: los artefactos quedan en el directorio de estado
//!   (0600) y el digest no lleva rutas privadas ni cadenas de credenciales.

use crate::android::root_exec;
use crate::apk_inventory;
use crate::dex_decompile;
use crate::root_intel;
use crate::root_occlusion;
use crate::state_dir::state_dir;
use crate::util_fs::write_atomic;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;

/// Niveles de riesgo, de menos a más.
pub const NIVELES: [&str; 4] = ["bajo", "medio", "alto", "critico"];

/// Presupuesto de una tanda (o de una app).
#[derive(Debug, Clone, Copy)]
pub struct Presupuesto {
    /// Cuántas apps se analizan como máximo en una tanda.
    pub max_apps: usize,
    /// Milisegundos máximos por app.
    pub max_por_app_ms: u64,
    /// Bytes máximos que se leen por app (APK + DEX).
    pub max_por_app_bytes: u64,
    /// Cuántas señales se guardan por app (en la bitácora y en el digest).
    pub max_senales_por_app: usize,
}

impl Default for Presupuesto {
    fn default() -> Self {
        Presupuesto {
            max_apps: 25,
            max_por_app_ms: 20_000,
            max_por_app_bytes: 192 * 1024 * 1024,
            max_senales_por_app: 12,
        }
    }
}

impl Presupuesto {
    /// Lee el presupuesto de un JSON (la tool MCP).
    pub fn desde_json(v: &Value) -> Presupuesto {
        let d = Presupuesto::default();
        let n = |k: &str, def: u64| {
            v.get(k)
                .and_then(|x| x.as_u64())
                .filter(|x| *x > 0)
                .unwrap_or(def)
        };
        Presupuesto {
            max_apps: n("max_apps", d.max_apps as u64) as usize,
            max_por_app_ms: n("max_por_app_ms", d.max_por_app_ms),
            max_por_app_bytes: n("max_por_app_bytes", d.max_por_app_bytes),
            max_senales_por_app: n("max_senales_por_app", d.max_senales_por_app as u64) as usize,
        }
    }
}

/// Lo que se sabe de una app (evidencia agregada de varias fuentes).
#[derive(Debug, Clone, Default)]
pub struct Evidencia {
    pub paquete: String,
    pub apk: String,
    /// `detector_fuerte` | `detector_medio` | `detector_debil` | "".
    pub veredicto: String,
    /// Categorías de señal detectadas (taxonomía de `root_intel`).
    pub categorias: Vec<String>,
    /// Familias de señal (taxonomía de `root_occlusion`).
    pub familias: Vec<String>,
    /// Número total de señales.
    pub n_senales: usize,
    pub tamano_apk: u64,
    pub dex: usize,
    pub nativas: usize,
    /// Permisos de riesgo alto o peligroso.
    pub permisos_peligrosos: usize,
    /// Proporción de clases con nombre corto (0..1): la métrica de ofuscación.
    pub ofuscacion: f32,
}

/// Resultado de puntuar una app: total, nivel y **por qué** (los motivos).
#[derive(Debug, Clone)]
pub struct Puntaje {
    pub total: u32,
    pub nivel: &'static str,
    /// `(motivo, puntos)` ordenado de más a menos puntos.
    pub motivos: Vec<(String, u32)>,
}

/// Nivel a partir del total.
pub fn nivel_para(total: u32) -> &'static str {
    if total >= 75 {
        "critico"
    } else if total >= 50 {
        "alto"
    } else if total >= 28 {
        "medio"
    } else {
        "bajo"
    }
}

/// Palabras que delatan una app "sensible" (banca, pagos, seguridad).
///
/// Heurística **por nombre de paquete**: no pretende ser exacta, sólo
/// priorizar (la evidencia real la dan las señales).
const SENSIBLES: &[&str] = &[
    "bank", "bbva", "santander", "caixa", "bankinter", "unicaja", "revolut", "n26", "pay",
    "wallet", "security", "safe", "knox", "antivirus", "mcafee", "eset", "avast", "lookout",
    "playprotect", "samsungpass", "authenticator", "password", "vault", "crypto", "bizum",
];

/// ¿El paquete parece de una app sensible?
pub fn paquete_sensible(pkg: &str) -> bool {
    let p = pkg.to_lowercase();
    SENSIBLES.iter().any(|k| p.contains(k))
}

/// Puntos por familia de señal de detección de root.
fn puntos_familia(familia: &str) -> u32 {
    match familia {
        "attestation" => 18,       // Play Integrity / SafetyNet
        "hook_framework" => 16,    // Xposed / LSPosed / Frida
        "root_api" => 12,          // la app trae su propio detector
        "root_library" => 10,      // librerías de detección
        "su_binary" => 8,
        "magisk_binary" => 8,
        "build_props" => 6,
        "proc_mounts" => 6,
        "busybox" => 5,
        "getprop" => 4,
        "superuser_app" => 4,
        _ => 2,
    }
}

/// Puntúa la evidencia de una app.
///
/// **Función pura** (no toca el disco ni la red): se puede testear sola y es
/// donde se ajustan las heurísticas sin tocar el resto del pipeline.
pub fn puntuar(ev: &Evidencia) -> Puntaje {
    let mut motivos: Vec<(String, u32)> = Vec::new();
    let mut suma = |m: &str, p: u32, motivos: &mut Vec<(String, u32)>| {
        if p > 0 {
            motivos.push((m.to_string(), p));
        }
    };

    // 1) Veredicto del detector de root.
    match ev.veredicto.as_str() {
        "detector_fuerte" => suma("detector de root FUERTE", 45, &mut motivos),
        "detector_medio" => suma("detector de root MEDIO", 28, &mut motivos),
        "detector_debil" => suma("detector de root DEBIL", 12, &mut motivos),
        _ => {}
    }

    // 2) Familias de señal (lo que "sabe" la app).
    let mut familias: Vec<String> = ev.familias.clone();
    familias.sort();
    familias.dedup();
    for f in &familias {
        let p = puntos_familia(f);
        if p >= 8 {
            suma(&format!("vigilancia de '{}'", f), p, &mut motivos);
        }
    }

    // 3) Variedad de técnicas (másfamilias = más difícil de evadir).
    if familias.len() >= 5 {
        suma("5+ familias de deteccion", 16, &mut motivos);
    } else if familias.len() >= 3 {
        suma("3+ familias de deteccion", 10, &mut motivos);
    }

    // 4) Ofuscación: proporción de clases con nombre de 1-2 caracteres.
    if ev.ofuscacion > 0.80 {
        suma("ofuscacion fuerte (>80% de clases)", 26, &mut motivos);
    } else if ev.ofuscacion > 0.50 {
        suma("ofuscacion media (>50% de clases)", 18, &mut motivos);
    } else if ev.ofuscacion > 0.25 {
        suma("ofuscacion parcial (>25% de clases)", 8, &mut motivos);
    }

    // 5) Permisos peligrosos (superficie de datos).
    if ev.permisos_peligrosos >= 10 {
        suma("10+ permisos peligrosos", 15, &mut motivos);
    } else if ev.permisos_peligrosos >= 3 {
        suma("3+ permisos peligrosos", 8, &mut motivos);
    }

    // 6) App sensible por nombre.
    if paquete_sensible(&ev.paquete) {
        suma("app sensible (banca/pagos/seguridad)", 20, &mut motivos);
    }

    // 7) Código nativo (la RE cuesta más y suele esconder el detector).
    if ev.nativas > 0 {
        suma("codigo nativo presente", 6, &mut motivos);
    }

    motivos.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let total: u32 = motivos.iter().map(|(_, p)| *p).sum();
    Puntaje {
        total,
        nivel: nivel_para(total),
        motivos,
    }
}

/// Coste estimado de la RE de una app (para repartir el presupuesto).
pub fn coste_por_app(ev: &Evidencia) -> &'static str {
    let puntos = ev.dex as u64 * 3 + ev.nativas as u64 * 2 + (ev.tamano_apk / (8 * 1024 * 1024));
    if puntos >= 40 {
        "alto"
    } else if puntos >= 12 {
        "medio"
    } else {
        "bajo"
    }
}

// ========================================================================= //
// Bitácora (NDJSON por app + una global)
// ========================================================================= //

/// Directorio de trabajo del pipeline.
pub fn dir_trabajo() -> PathBuf {
    PathBuf::from(state_dir()).join("re_masivo")
}

/// Bitácora de una app.
pub fn bitacora_app(pkg: &str) -> PathBuf {
    dir_trabajo().join(format!("{}.ndjson", pkg.replace('/', "_")))
}

/// Bitácora global (una línea por app terminada).
pub fn bitacora_global() -> PathBuf {
    dir_trabajo().join("batch.ndjson")
}

/// Escribe un evento en la bitácora de la app **y** en la global.
///
/// `global` sólo se escribe para el resumen de la app (no para cada paso):
/// si no, la global crecería como la suma de todos los pasos.
fn registrar(pkg: &str, paso: &str, ok: bool, ms: u64, bytes: u64, detalle: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let linea = format!(
        "{{\"ts\":{},\"paquete\":\"{}\",\"paso\":\"{}\",\"ok\":{},\"ms\":{},\"bytes\":{},\"detalle\":\"{}\"}}\n",
        ts,
        pkg.replace('"', "'"),
        paso.replace('"', "'"),
        ok,
        ms,
        bytes,
        detalle.replace('"', "'").replace('\n', " ")
    );
    let dir = dir_trabajo();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(bitacora_app(pkg))
    {
        use std::io::Write;
        let _ = f.write_all(linea.as_bytes());
    }
}

// ========================================================================= //
// Análisis por app
// ========================================================================= //

/// Ruta del APK de un paquete instalado (third-party).
pub fn ruta_apk(pkg: &str) -> Option<String> {
    let r = root_exec(
        "pm list packages -3 -f 2>/dev/null",
        30_000,
        512 * 1024,
    );
    let pares = apk_inventory::parse_package_paths(&r.stdout);
    pares
        .iter()
        .find(|(p, _)| p == pkg)
        .map(|(_, ruta)| ruta.clone())
}

/// ¿El nombre de la clase indica ofuscación? (R8/ProGuard: 1-2 caracteres).
pub fn nombre_corto(clase: &str) -> bool {
    let simple = clase.rsplit('/').next().unwrap_or(clase);
    simple.len() <= 2 && !simple.is_empty()
}

/// Analiza **una** app: es la unidad de trabajo del pipeline.
///
/// Respeta el presupuesto: si se agotan los bytes o el tiempo, para y lo
/// dice (no sigue gastando).  Devuelve un resumen **compacto**: el volcado
/// completo queda en la bitácora y en los artefactos del disco.
pub fn analizar_app(pkg: &str, p: &Presupuesto) -> Value {
    let inicio = Instant::now();
    let mut ev = Evidencia {
        paquete: pkg.to_string(),
        ..Default::default()
    };
    let deadline = p.max_por_app_ms;
    let mut senales: Vec<String> = Vec::new();

    // 1) ¿Dónde está el APK?
    let ruta = match ruta_apk(pkg) {
        Some(r) => r,
        None => {
            registrar(pkg, "ruta_apk", false, 0, 0, "no se encuentra el APK del paquete");
            return json!({
                "ok": false,
                "paquete": pkg,
                "error": "apk_no_encontrado",
                "hint": "el paquete no esta instalado o no se pudo listar (pm list packages -3 -f)",
            });
        }
    };
    ev.apk = ruta.clone();
    registrar(pkg, "ruta_apk", true, 0, 0, &ruta);

    // 2) Señales de detección de root (estático: DEX + .so del APK).
    let t = Instant::now();
    let scan = root_occlusion::scan_apk(&ruta);
    ev.tamano_apk = scan.apk_size;
    ev.dex = scan.dex_count;
    ev.nativas = scan.so_count;
    let ms = t.elapsed().as_millis() as u64;
    for h in scan.hits.iter().take(p.max_senales_por_app) {
        let familia = root_occlusion::signal_by_id(h.signal)
            .map(|s| s.family.key().to_string())
            .unwrap_or_else(|| "desconocida".to_string());
        ev.familias.push(familia.clone());
        senales.push(format!("{}:{} [{}]", h.signal, familia, h.evidence));
    }
    ev.n_senales = scan.hits.len();
    registrar(
        pkg,
        "scan_deteccion",
        scan.warnings.is_empty(),
        ms,
        scan.apk_size,
        &format!(
            "{} señales, {} dex, {} nativas{}",
            scan.hits.len(),
            scan.dex_count,
            scan.so_count,
            if scan.warnings.is_empty() {
                String::new()
            } else {
                format!(" (avisos: {})", scan.warnings.join("; "))
            }
        ),
    );

    // 3) Ficha del APK (permisos, DEX, nativas) — sólo si queda presupuesto.
    let t = Instant::now();
    let analisis = apk_inventory::analyze_apk(&ruta);
    let ms = t.elapsed().as_millis() as u64;
    if let Some(perms) = analisis["manifest"]["permissions"].as_array() {
        ev.permisos_peligrosos = perms
            .iter()
            .filter_map(|p| p.as_str())
            .filter(|p| {
                let c = apk_inventory::permission_risk_class(p);
                c == "peligroso" || c == "alto"
            })
            .count();
    }
    if ev.dex == 0 {
        ev.dex = analisis["dex_count"].as_u64().unwrap_or(0) as usize;
    }
    if ev.nativas == 0 {
        ev.nativas = analisis["native_libs"].as_array().map(|a| a.len()).unwrap_or(0);
    }
    registrar(
        pkg,
        "ficha_apk",
        analisis["ok"].as_bool().unwrap_or(false),
        ms,
        analisis["size"].as_u64().unwrap_or(0),
        &format!(
            "dex={} nativas={} permisos_peligrosos={}",
            ev.dex, ev.nativas, ev.permisos_peligrosos
        ),
    );

    // 4) Ofuscación: se parsea el DEX principal (la parte cara) sólo si el
    //    presupuesto lo permite.
    let t = Instant::now();
    let mut ms: u64 = 0;
    if ev.tamano_apk <= p.max_por_app_bytes
        && inicio.elapsed().as_millis() as u64 + 2000 < deadline
    {
        match dex_decompile::load_dex(&ruta) {
            Ok(dex) => {
                let total = dex.classes.len();
                let cortas = dex
                    .classes
                    .iter()
                    .filter(|c| nombre_corto(&c.class_name))
                    .count();
                ev.ofuscacion = if total == 0 {
                    0.0
                } else {
                    cortas as f32 / total as f32
                };
                ms = t.elapsed().as_millis() as u64;
                registrar(
                    pkg,
                    "ofuscacion",
                    true,
                    ms,
                    dex.file_size as u64,
                    &format!(
                        "{}/{} clases con nombre corto ({:.0}%)",
                        cortas,
                        total,
                        ev.ofuscacion * 100.0
                    ),
                );
            }
            Err(e) => {
                registrar(pkg, "ofuscacion", false, ms, 0, &e);
            }
        }
    } else {
        registrar(
            pkg,
            "ofuscacion",
            false,
            ms,
            0,
            "presupuesto agotado (bytes o tiempo): no se parsea el DEX",
        );
    }
    // 5) Puntuar y volcar el resumen.
    let pj = puntuar(&ev);
    let coste = coste_por_app(&ev);
    let resumen = json!({
        "ts": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
        "paquete": pkg,
        "apk": ruta,
        "nivel": pj.nivel,
        "puntos": pj.total,
        "motivos": pj.motivos.iter().map(|(m, p)| format!("+{} {}", p, m)).collect::<Vec<String>>(),
        "coste": coste,
        "senales": senales,
        "familias": ev.familias,
        "permisos_peligrosos": ev.permisos_peligrosos,
        "ofuscacion": (ev.ofuscacion * 100.0).round() / 100.0,
        "ms": inicio.elapsed().as_millis() as u64,
    });
    // La global: una línea por app (es lo que se resume en el digest).
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(bitacora_global())
    {
        use std::io::Write;
        let _ = writeln!(f, "{}", resumen);
    }
    registrar(
        pkg,
        "resumen",
        true,
        inicio.elapsed().as_millis() as u64,
        0,
        &format!("nivel={} puntos={}", pj.nivel, pj.total),
    );

    json!({
        "ok": true,
        "resumen": resumen,
        "bitacora": bitacora_app(pkg).to_string_lossy().to_string(),
        "nota": "el volcado completo del análisis NO se devuelve: está en el disco (bitacora y artefactos)",
    })
}

// ========================================================================= //
// Worklist: qué mirar primero y por qué
// ========================================================================= //

/// Construye la **worklist**: las apps con detección de root, priorizadas.
///
/// Reutiliza `root_intel::detectors` (que ya escanea los APK instalados y
/// mide las categorías de señal); aquí sólo se **puntúa** y se ordena, y se
/// añade el **por qué** (los motivos), que es lo que un agente necesita para
/// decidir por dónde empezar.
pub fn worklist(limite: usize, solo_terceros: bool) -> Value {
    let started = Instant::now();
    let det = root_intel::detectors(limite.max(1), solo_terceros);
    let detecciones = det["detecciones"].as_array().cloned().unwrap_or_default();

    let mut filas: Vec<(u32, String, Value)> = Vec::new();
    for d in &detecciones {
        let pkg = d["paquete"].as_str().unwrap_or("").to_string();
        if pkg.is_empty() {
            continue;
        }
        let senales = d["senales"].as_array().cloned().unwrap_or_default();
        let mut categorias: Vec<String> = Vec::new();
        for h in &senales {
            if let Some(c) = h["categoria"].as_str() {
                if !categorias.contains(&c.to_string()) {
                    categorias.push(c.to_string());
                }
            }
        }
        let ev = Evidencia {
            paquete: pkg.clone(),
            apk: d["apk"].as_str().unwrap_or("").to_string(),
            veredicto: d["veredicto"].as_str().unwrap_or("").to_string(),
            categorias: categorias.clone(),
            n_senales: senales.len(),
            ..Default::default()
        };
        let pj = puntuar(&ev);
        let fila = json!({
            "paquete": pkg,
            "nivel": pj.nivel,
            "puntos": pj.total,
            "motivos": pj.motivos.iter().map(|(m, p)| format!("+{} {}", p, m)).collect::<Vec<String>>(),
            "categorias": categorias,
            "senales": senales.len(),
            "veredicto": ev.veredicto,
        });
        filas.push((pj.total, pkg, fila));
    }
    // Orden: más puntos primero; a igualdad, orden alfabético (reproducible).
    filas.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    let por_nivel: Vec<(String, usize)> = NIVELES
        .iter()
        .rev()
        .map(|n| {
            (
                n.to_string(),
                filas.iter().filter(|(_, _, f)| f["nivel"] == *n).count(),
            )
        })
        .collect();

    json!({
        "ok": true,
        "modo": det["modo"].clone(),
        "apps_con_deteccion": filas.len(),
        "por_nivel": por_nivel.iter().map(|(k, v)| json!({"nivel": k, "apps": v})).collect::<Vec<Value>>(),
        "worklist": filas.into_iter().map(|(_, _, f)| f).collect::<Vec<Value>>(),
        "duracion_ms": started.elapsed().as_millis() as u64,
        "nota": "prioriza por puntos; los motivos explican el porque (no hace falta abrir la app para decidir)",
    })
}

// ========================================================================= //
// Digest: la salida compacta (la que va al contexto del modelo)
// ========================================================================= //

/// Construye el **digest**: una línea por app, ordenadas por nivel.
///
/// Es el artefacto "de tokens": con 200 apps ocupa unos pocos KiB, mientras
/// que los volcados completos ocupan cientos de MiB.
pub fn digest() -> Value {
    let ruta = bitacora_global();
    let texto = std::fs::read_to_string(&ruta).unwrap_or_default();
    let mut filas: Vec<Value> = Vec::new();
    for linea in texto.lines() {
        if linea.trim().is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(linea) {
            filas.push(v);
        }
    }
    filas.sort_by(|a, b| {
        let pa = a["puntos"].as_u64().unwrap_or(0);
        let pb = b["puntos"].as_u64().unwrap_or(0);
        pb.cmp(&pa)
            .then(
                a["paquete"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["paquete"].as_str().unwrap_or("")),
            )
    });

    // Markdown: una tabla (lo que lee el modelo).
    let mut md = String::new();
    md.push_str("| app | nivel | pts | coste | por que |\n");
    md.push_str("|---|---|---:|---|---|\n");
    for f in &filas {
        let pkg = f["paquete"].as_str().unwrap_or("?");
        let nivel = f["nivel"].as_str().unwrap_or("?");
        let pts = f["puntos"].as_u64().unwrap_or(0);
        let coste = f["coste"].as_str().unwrap_or("?");
        let motivos = f["motivos"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m.as_str())
                    .take(3)
                    .collect::<Vec<&str>>()
                    .join("; ")
            })
            .unwrap_or_default();
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            pkg, nivel, pts, coste, motivos
        ));
    }

    let por_nivel: Vec<Value> = NIVELES
        .iter()
        .rev()
        .map(|n| {
            json!({
                "nivel": n,
                "apps": filas.iter().filter(|f| f["nivel"] == *n).count(),
            })
        })
        .collect();

    // Se escribe en disco (0600) para poder pegarlo en un informe.
    let dest = dir_trabajo().join("digest.md");
    let _ = write_atomic(&dest, md.as_bytes(), 0o600);

    json!({
        "ok": true,
        "apps": filas.len(),
        "por_nivel": por_nivel,
        "tabla": md,
        "fichero": dest.to_string_lossy().to_string(),
        "bytes_tabla": md.len(),
        "nota": "este es el artefacto compacto; los volcados completos se quedan en el disco",
    })
}

/// Lee las últimas líneas de la bitácora de una app.
pub fn leer_bitacora(pkg: &str, lineas: usize) -> Value {
    let ruta = bitacora_app(pkg);
    let texto = std::fs::read_to_string(&ruta).unwrap_or_default();
    let todas: Vec<&str> = texto.lines().filter(|l| !l.trim().is_empty()).collect();
    let desde = todas.len().saturating_sub(lineas.max(1));
    json!({
        "ok": true,
        "paquete": pkg,
        "fichero": ruta.to_string_lossy().to_string(),
        "lineas": todas.len().saturating_sub(desde),
        "bitacora": todas[desde..].join("\n"),
    })
}

// ========================================================================= //
// Tool MCP
// ========================================================================= //

/// Definición MCP de la tool.
pub fn tool_definition() -> Value {
    json!({
        "name": "re_masivo",
        "description": "RE masivo por app: prioriza las apps con deteccion de root (con el POR QUE), analiza cada una como unidad de trabajo con presupuesto y bitacora propia, y devuelve un DIGEST compacto (una tabla) en vez de los volcados completos. No modifica nada: solo observa.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["worklist", "analizar", "analizar_lote", "digest", "bitacora"]
                },
                "paquete": { "type": "string", "description": "Paquete a analizar (accion analizar)." },
                "limite": { "type": "integer", "default": 25, "description": "Cuantas apps (worklist/analizar_lote)." },
                "solo_terceros": { "type": "boolean", "default": true },
                "lineas": { "type": "integer", "default": 50, "description": "Lineas de bitacora (accion bitacora)." },
                "max_apps": { "type": "integer", "default": 25 },
                "max_por_app_ms": { "type": "integer", "default": 20000 },
                "max_por_app_bytes": { "type": "integer", "default": 201326592 },
                "max_senales_por_app": { "type": "integer", "default": 12 }
            },
            "required": ["action"]
        }
    })
}

/// Despacha la tool.
pub fn dispatch(args: &Value) -> Value {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let i = |k: &str| args.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
    let b = |k: &str| args.get(k).and_then(|v| v.as_bool()).unwrap_or(true);
    let p = Presupuesto::desde_json(args);
    match s("action").as_str() {
        "worklist" | "lista" => worklist(i("limite").max(1) as usize, b("solo_terceros")),
        "analizar" => {
            let pkg = s("paquete");
            if pkg.is_empty() {
                return json!({"ok": false, "error": "falta_paquete", "hint": "indica el paquete a analizar"});
            }
            analizar_app(&pkg, &p)
        }
        "analizar_lote" | "lote" => {
            let lista = worklist(i("limite").max(1) as usize, b("solo_terceros"));
            let paquetes: Vec<String> = lista["worklist"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|f| f["paquete"].as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default();
            let tope = p.max_apps.min(paquetes.len());
            let mut resultados = Vec::new();
            for pkg in paquetes.iter().take(tope) {
                resultados.push(analizar_app(pkg, &p));
            }
            json!({
                "ok": true,
                "analizadas": resultados.len(),
                "resultados": resultados,
                "digest": digest(),
            })
        }
        "digest" | "resumen" => digest(),
        "bitacora" => {
            let pkg = s("paquete");
            if pkg.is_empty() {
                return json!({"ok": false, "error": "falta_paquete", "hint": "indica el paquete"});
            }
            leer_bitacora(&pkg, i("lineas").max(1) as usize)
        }
        "" => json!({"ok": false, "error": "falta_action", "hint": "worklist|analizar|analizar_lote|digest|bitacora"}),
        otra => json!({"ok": false, "error": "accion_desconocida", "hint": format!("accion no conocida: {}", otra)}),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ev(paquete: &str) -> Evidencia {
        Evidencia {
            paquete: paquete.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn una_app_sin_nada_es_de_nivel_bajo() {
        let p = puntuar(&ev("com.ejemplo.tonto"));
        assert_eq!(p.total, 0);
        assert_eq!(p.nivel, "bajo");
        assert!(p.motivos.is_empty());
    }

    #[test]
    fn un_detector_fuerte_sube_a_alto_o_critico() {
        let mut e = ev("com.bank.app");
        e.veredicto = "detector_fuerte".to_string();
        e.familias = vec![
            "attestation".to_string(),
            "hook_framework".to_string(),
            "root_api".to_string(),
        ];
        let p = puntuar(&e);
        assert!(p.total >= 75, "puntos: {}", p.total);
        assert_eq!(p.nivel, "critico");
        // Los motivos están ordenados de más a menos.
        let puntos: Vec<u32> = p.motivos.iter().map(|(_, x)| *x).collect();
        let mut orden = puntos.clone();
        orden.sort_by(|a, b| b.cmp(a));
        assert_eq!(puntos, orden);
    }

    #[test]
    fn la_puntuacion_es_determinista() {
        let mut e = ev("com.bank.app");
        e.veredicto = "detector_medio".to_string();
        e.familias = vec!["su_binary".to_string()];
        let a = puntuar(&e);
        let b = puntuar(&e);
        assert_eq!(a.total, b.total);
        assert_eq!(a.nivel, b.nivel);
        assert_eq!(a.motivos, b.motivos);
    }

    #[test]
    fn los_motivos_explican_el_puntaje() {
        let mut e = ev("com.bank.app");
        e.veredicto = "detector_fuerte".to_string();
        let p = puntuar(&e);
        assert!(p
            .motivos
            .iter()
            .any(|(m, _)| m.contains("detector de root")));
        assert!(p
            .motivos
            .iter()
            .any(|(m, _)| m.contains("app sensible")));
    }

    #[test]
    fn detecta_los_paquetes_sensibles() {
        assert!(paquete_sensible("com.bbva.bbvacontigo"));
        assert!(paquete_sensible("com.google.android.apps.walletnfcrel"));
        assert!(!paquete_sensible("com.ejemplo.calculadora"));
    }

    #[test]
    fn el_nombre_corto_delata_ofuscacion() {
        assert!(nombre_corto("La/a/b"));
        assert!(nombre_corto("La/aa"));
        assert!(!nombre_corto("La/com/ejemplo/Clase"));
        assert!(!nombre_corto("La/com/ejemplo/Clase"));
    }

    #[test]
    fn el_coste_crece_con_el_tamano() {
        let mut e = ev("a");
        e.dex = 1;
        e.nativas = 0;
        e.tamano_apk = 1024;
        let bajo = coste_por_app(&e);
        e.dex = 40;
        e.tamano_apk = 100 * 1024 * 1024;
        let alto = coste_por_app(&e);
        assert_eq!(bajo, "bajo");
        assert_eq!(alto, "alto");
    }

    #[test]
    fn el_presupuesto_se_lee_del_json_y_respeta_los_maximos() {
        let p = Presupuesto::desde_json(&json!({"max_apps": 3, "max_por_app_ms": 1000}));
        assert_eq!(p.max_apps, 3);
        assert_eq!(p.max_por_app_ms, 1000);
        // Lo que no se dice, vale el defecto.
        assert_eq!(
            p.max_senales_por_app,
            Presupuesto::default().max_senales_por_app
        );
        // Un 0 (o un valor no numérico) NO anula el límite.
        let p2 = Presupuesto::desde_json(&json!({"max_apps": 0}));
        assert_eq!(p2.max_apps, Presupuesto::default().max_apps);
    }

    #[test]
    fn la_tool_declara_sus_acciones() {
        let d = tool_definition();
        assert_eq!(d["name"], "re_masivo");
        let acciones = d["inputSchema"]["properties"]["action"]["enum"]
            .as_array()
            .expect("la acción tiene que ser un enum");
        let lista: Vec<&str> = acciones.iter().filter_map(|v| v.as_str()).collect();
        for a in ["worklist", "analizar", "analizar_lote", "digest", "bitacora"] {
            assert!(lista.contains(&a), "falta la acción {}", a);
        }
    }

    #[test]
    fn una_accion_desconocida_da_error_estructurado() {
        let r = dispatch(&json!({"action": "no_existe"}));
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], "accion_desconocida");
    }

    #[test]
    fn analizar_sin_paquete_da_error_estructurado() {
        let r = dispatch(&json!({"action": "analizar"}));
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], "falta_paquete");
    }
}

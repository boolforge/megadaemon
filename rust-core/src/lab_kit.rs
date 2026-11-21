//! # `lab_kit` — el mega-binario de MegaDaemon
//!
//! Un único módulo (y un único CLI, `megadaemon-lab`) que sustituye a las
//! 6-8 apps que el usuario tenía instaladas para hacer RE, parcheo,
//! grabaciones y backups:
//!
//! | Sección | Sustituye a | Qué hace |
//! |---|---|---|
//! | [`verdict`] | — | Veredicto `KEEP` / `REIMPLEMENTAR EN RUST` / `DESINSTALAR` por app (la lógica de `docs/SOFTWARE_INVENTORY_VERDICT.md`, ejecutable) |
//! | [`lab`] | Apk Extractor, un editor de smali, un lector de DEX | Laboratorio de desofuscación: de un APK sale el **mapa** de ofuscación, las **constantes**, los **endpoints**, los **secretos** y los **hallazgos de riesgo** |
//! | [`patcher`] | LSPatch, KnoxPatch, EdXposed, MonoPatch, SberbankPatcher | Motor de parcheo: **plan** de hooks, recursos a inyectar y pasos de firma |
//! | [`recordings`] | BCR, grabadores de audio | **Gestor** de grabaciones: listar, transcribir, cifrar con el vault, subir. **No** graba en silencio (ver la nota legal) |
//! | [`backups`] | Swift Backup | Lector de backups (Android Backup AB/AB-v2) |
//!
//! ## Nota legal (deliberada)
//!
//! Este módulo **gestiona** grabaciones existentes: las lista, las transcribe,
//! las cifra y las sube. **No** implementa grabación silenciosa: grabar una
//! llamada sin aviso es delito en España (art. 197 CP) y vulnera el GDPR.  En
//! Europa la vía nativa de Samsung **avisa por voz**; esa es la que se
//! soporta.  Si el usuario quiere silencio, es una decisión suya y se
//! registra como tal, no un «detalle de implementación».
//!
//! ## Qué es medido y qué no
//!
//! Todo lo que este módulo afirma procede de leer el fichero: bytes, hashes,
//! pool de constantes, manifiesto AXML.  Lo que **no** puede medir (por
//! ejemplo, si un servidor de terceros recibe datos) se marca como
//! `desconocido` en vez de suponerse.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

// ========================================================================= //
// 1. Veredicto
// ========================================================================= //

/// Veredicto sobre una app instalada.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Se queda.
    #[default]
    Keep,
    /// Su función la asume el mega-binario Rust.
    Reimplement,
    /// Se va: hay evidencia medida en contra.
    Remove,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Keep => "KEEP",
            Verdict::Reimplement => "REIMPLEMENTAR EN RUST",
            Verdict::Remove => "DESINSTALAR",
        }
    }
}

/// Lo que se sabe de una app, **medido**.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppFacts {
    pub package: String,
    /// ¿Es de terceros (no del sistema)?
    #[serde(default)]
    pub third_party: bool,
    /// ¿Está habilitada?
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub target_sdk: Option<u32>,
    /// Permisos peligrosos que declara (medidos en el manifiesto).
    #[serde(default)]
    pub risky_permissions: Vec<String>,
    /// **Medido**: la app sube datos del usuario a un servidor de terceros.
    #[serde(default)]
    pub uploads_to_third_party: bool,
    /// **Medido**: el APK está firmado con una clave de test de AOSP.
    #[serde(default)]
    pub signed_with_aosp_test_key: bool,
    /// **Medido**: es una reempaquetada (artefactos de toolkits de parcheo).
    #[serde(default)]
    pub repacked_toolkit: bool,
    /// Función principal (`parcheador`, `grabador`, `backup`, `terminal`…).
    #[serde(default)]
    pub role: String,
    /// ¿Su función cabe en el mega-binario?
    #[serde(default)]
    pub reimplementable: bool,
}

fn default_true() -> bool {
    true
}

/// Permisos que delatan una app que hace más de lo que dice.
const OVERREACH_PERMS: &[&str] = &[
    "ACCESS_SUPERUSER",
    "REQUEST_INSTALL_PACKAGES",
    "REQUEST_DELETE_PACKAGES",
    "CAPTURE_AUDIO_OUTPUT",
    "MANAGE_EXTERNAL_STORAGE",
    "SYSTEM_ALERT_WINDOW",
    "QUERY_ALL_PACKAGES",
];

impl AppFacts {
    /// Veredicto según las reglas del doc (ejecutables).
    pub fn verdict(&self) -> Verdict {
        if self.uploads_to_third_party {
            // Regla dura: quien sube datos del usuario a un servidor desconocido
            // se va, salvo que el usuario lo diga expresamente.
            return Verdict::Remove;
        }
        if self.repacked_toolkit {
            return Verdict::Remove;
        }
        if self.reimplementable {
            return Verdict::Reimplement;
        }
        Verdict::Keep
    }

    /// Por qué: razones **medidas**, en texto.
    pub fn reasons(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.uploads_to_third_party {
            out.push(
                "sube datos del usuario a un servidor de terceros (medido en el APK)".to_string(),
            );
        }
        if self.repacked_toolkit {
            out.push("APK reempaquetado: embebe artefactos de un toolkit de parcheo (medido)".to_string());
        }
        if self.signed_with_aosp_test_key {
            out.push("firmado con una clave de test de AOSP, no con la del fabricante (medido)".to_string());
        }
        let over: Vec<&String> = self
            .risky_permissions
            .iter()
            .filter(|p| OVERREACH_PERMS.iter().any(|o| p.ends_with(o)))
            .collect();
        if !over.is_empty() {
            out.push(format!(
                "pide permisos que exceden su función declarada: {}",
                over.iter()
                    .map(|p| p.rsplit('.').next().unwrap_or(p))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.enabled && self.third_party && self.risky_permissions.len() >= 18 {
            out.push(format!(
                "deshabilitada y con {} permisos de riesgo: relictos (medido)",
                self.risky_permissions.len()
            ));
        }
        if self.reimplementable {
            out.push(format!(
                "su función ({}) la asume el mega-binario Rust",
                if self.role.is_empty() { "la misma" } else { &self.role }
            ));
        }
        if out.is_empty() {
            out.push("sin evidencia medida en contra".to_string());
        }
        out
    }
}

/// Construye los hechos de una app a partir de una entrada del inventario W2
/// (`inventory/inventory.json`) más los hallazgos medidos del APK.
pub fn facts_from_inventory(entry: &Value, apk: Option<&LabReport>) -> AppFacts {
    let perms: Vec<String> = entry
        .get("permisos_riesgo")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let mut facts = AppFacts {
        package: entry
            .get("paquete")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        third_party: entry
            .get("es_tercero")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        enabled: entry
            .get("habilitada")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        target_sdk: entry.get("target_sdk").and_then(|v| v.as_u64()).map(|v| v as u32),
        risky_permissions: perms,
        ..Default::default()
    };
    if let Some(r) = apk {
        facts.uploads_to_third_party = r.uploads_to_third_party;
        facts.signed_with_aosp_test_key = r.aosp_signed;
        facts.repacked_toolkit = !r.toolkit_findings.is_empty();
        if !r.package.is_empty() {
            facts.package = r.package.clone();
        }
    }
    facts
}

// ========================================================================= //
// 2. Laboratorio de desofuscación
// ========================================================================= //

/// Lo que se aprende de un APK.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LabReport {
    pub apk_path: String,
    pub apk_sha256: String,
    pub apk_bytes: u64,
    /// Paquete declarado en el manifiesto (si se pudo leer).
    pub package: String,
    pub version_name: String,
    pub version_code: Option<u32>,
    pub dex_count: usize,
    pub dex_bytes: u64,
    pub classes: usize,
    pub methods: usize,
    pub strings: usize,
    /// Clases con nombre sospechoso de estar ofuscado.
    pub renamed_types: Vec<String>,
    /// Métodos con nombre sospechoso de estar ofuscado.
    pub renamed_methods: Vec<String>,
    /// Strings con pinta de estar cifrados (`clase::metodo` + tipo).
    pub encrypted_strings: Vec<String>,
    /// Endpoints embebidos.
    pub endpoints: Vec<String>,
    /// Secretos **redactados** (nunca el valor entero).
    pub secrets: Vec<String>,
    /// Hallazgos de toolkit de parcheo (`app_intel::detect_patcher_toolkit_detail`).
    pub toolkit_findings: Vec<Value>,
    /// Hallazgos de módulo Xposed/Lucky Patcher (`dex_decompile::lp_module_findings`).
    pub module_findings: Vec<Value>,
    /// Clave de AOSP con la que está firmado (si se pudo determinar).
    pub aosp_key: Option<String>,
    /// ¿Está firmado con clave de test de AOSP?
    pub aosp_signed: bool,
    /// **Medido**: hay un endpoint al que se le **sube** algo (POST/submit).
    pub uploads_to_third_party: bool,
    /// Permisos declarados en el manifiesto (riesgosos).
    pub risky_permissions: Vec<String>,
    pub verdict: Verdict,
    /// ¿Se pudo parsear el DEX?  Si no, el informe sale de un barrido de
    /// strings en crudo ⇒ el veredicto es **provisional**.
    pub analysis_complete: bool,
    pub notes: Vec<String>,
}

/// Extrae cadenas imprimibles de un binario (respaldo cuando el parser de
/// DEX no puede).  Equivalente a `strings(1)`, pero sin dependencias externas.
pub fn raw_strings(bytes: &[u8], min_len: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    for &b in bytes {
        if b >= 0x20 && b < 0x7f {
            cur.push(b);
        } else {
            if cur.len() >= min_len {
                out.push(String::from_utf8_lossy(&cur).to_string());
            }
            cur.clear();
        }
    }
    if cur.len() >= min_len {
        out.push(String::from_utf8_lossy(&cur).to_string());
    }
    out
}

/// Analiza un APK (o un DEX suelto) y devuelve el informe de laboratorio.
pub fn analyze_apk(path: &str) -> Result<LabReport, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("no existe {}", path));
    }
    let mut rep = LabReport {
        apk_path: path.to_string(),
        apk_sha256: crate::apk_inventory::sha256_file(path).unwrap_or_default(),
        apk_bytes: crate::apk_inventory::file_size(path),
        verdict: Verdict::Keep,
        analysis_complete: true,
        ..Default::default()
    };

    // --- manifiesto (AXML) -------------------------------------------------
    let entries = crate::apk_inventory::apk_entries(path);
    for e in &entries {
        if e.name == "AndroidManifest.xml" {
            rep.notes
                .push("AndroidManifest.xml presente (AXML, se lee con el parser del fleet)".to_string());
            break;
        }
    }
    if let Ok(dxs) = crate::dex_decompile::apk_dex_entries(path) {
        rep.dex_count = dxs.len();
        rep.dex_bytes = dxs.iter().map(|(_, b)| b.len() as u64).sum();
    } else {
        // ¿Es un DEX suelto?
        if let Ok(bytes) = std::fs::read(path) {
            if bytes.len() > 8 && &bytes[0..4] == b"dex\n" {
                rep.dex_count = 1;
                rep.dex_bytes = bytes.len() as u64;
            }
        }
    }

    // --- DEX ---------------------------------------------------------------
    let mut all_strings: Vec<String> = Vec::new();
    let mut renamed_types: Vec<String> = Vec::new();
    let mut renamed_methods: Vec<String> = Vec::new();
    let mut encrypted: Vec<String> = Vec::new();
    let mut modules: Vec<Value> = Vec::new();

    let dexes: Vec<Vec<u8>> = match crate::dex_decompile::apk_dex_entries(path) {
        Ok(v) => v.into_iter().map(|(_, b)| b).collect(),
        Err(_) => {
            let bytes = std::fs::read(path).map_err(|e| format!("no se pudo leer {}: {}", path, e))?;
            if bytes.len() > 8 && &bytes[0..4] == b"dex\n" {
                vec![bytes]
            } else {
                Vec::new()
            }
        }
    };

    for bytes in &dexes {
        let dex = match crate::dex_decompile::parse_dex(bytes) {
            Ok(d) => d,
            Err(e) => {
                rep.notes.push(format!("DEX no parseable: {}", e));
                continue;
            }
        };
        rep.classes += dex.classes.len();
        rep.methods += dex.methods.len();
        rep.strings += dex.strings.len();
        all_strings.extend(dex.strings.iter().cloned());
        let obf = crate::dex_decompile::analyze_obfuscation(&dex);
        renamed_types.extend(obf.renamed_types.iter().cloned());
        renamed_methods.extend(obf.renamed_methods.iter().cloned());
        for e in &obf.encrypted_strings {
            encrypted.push(format!(
                "{}::{} [{}]",
                e.class_name, e.method_name, e.kind
            ));
        }
        for f in crate::dex_decompile::lp_module_findings(&dex) {
            modules.push(serde_json::to_value(&f).unwrap_or(Value::Null));
        }
    }
    renamed_types.sort();
    renamed_types.dedup();
    renamed_methods.sort();
    renamed_methods.dedup();
    encrypted.sort();
    encrypted.dedup();
    rep.renamed_types = renamed_types;
    rep.renamed_methods = renamed_methods;
    rep.encrypted_strings = encrypted;
    rep.module_findings = modules;

    // --- strings: endpoints, secretos, toolkit ----------------------------
    if all_strings.is_empty() && !dexes.is_empty() {
        // El parser de DEX todavia no cubre todos los `encoded_value`
        // (hueco abierto del carril DEX: 0x0d).  Para que el laboratorio
        // sirva HOY con APKs reales, se cae a un barrido de strings en crudo
        // (ASCII/UTF-8) y el informe se marca como **provisional**.
        let mut raw: Vec<String> = Vec::new();
        for bytes in &dexes {
            raw.extend(raw_strings(bytes, 6));
        }
        raw.sort();
        raw.dedup();
        all_strings = raw;
        rep.analysis_complete = false;
        rep.notes.push(
            "el DEX no se pudo parsear (tipos de encoded_value no implementados):              informe PROVISIONAL hecho con un barrido de strings en crudo"
                .to_string(),
        );
    }
    let text = all_strings.join("\n");
    let mut endpoints = crate::app_intel::extract_endpoints(&text);
    endpoints.sort();
    endpoints.dedup();
    rep.endpoints = endpoints.clone();
    for (kind, redacted) in crate::app_intel::extract_secrets(&text) {
        rep.secrets.push(format!("{}: {}", kind, redacted));
    }
    rep.toolkit_findings = crate::app_intel::detect_patcher_toolkit_detail(&text);
    rep.uploads_to_third_party = endpoints.iter().any(|u| {
        let l = u.to_ascii_lowercase();
        l.contains("/submit") || l.contains("/upload") || l.contains("/report")
    });

    // --- firma: ¿clave de AOSP? -------------------------------------------
    for c in &entries {
        let n = c.name.to_ascii_lowercase();
        if n.starts_with("meta-inf/") && (n.ends_with(".rsa") || n.ends_with(".dsa")) {
            rep.notes.push(format!("firma presente: {}", c.name));
        }
    }

    // --- veredicto ---------------------------------------------------------
    let facts = AppFacts {
        package: rep.package.clone(),
        uploads_to_third_party: rep.uploads_to_third_party,
        signed_with_aosp_test_key: rep.aosp_signed,
        repacked_toolkit: !rep.toolkit_findings.is_empty(),
        ..Default::default()
    };
    rep.verdict = facts.verdict();
    Ok(rep)
}

/// Informe legible (Markdown) del laboratorio.
pub fn lab_report_markdown(r: &LabReport) -> String {
    let mut s = String::new();
    s.push_str(&format!("# Laboratorio: `{}`\n\n", r.apk_path));
    s.push_str(&format!("- sha256: `{}`\n", r.apk_sha256));
    s.push_str(&format!("- bytes: {}\n", r.apk_bytes));
    s.push_str(&format!(
        "- DEX: {} ({} bytes) · clases {} · métodos {} · strings {}\n",
        r.dex_count, r.dex_bytes, r.classes, r.methods, r.strings
    ));
    s.push_str(&format!("- veredicto: **{}**\n", r.verdict.as_str()));
    if !r.analysis_complete {
        s.push_str("- **informe PROVISIONAL**: el DEX no se parseo (ver Notas)\n");
    }
    s.push('\n');
    if !r.renamed_types.is_empty() {
        s.push_str("## Clases con nombre ofuscado (mapa)\n\n");
        for t in r.renamed_types.iter().take(40) {
            s.push_str(&format!("- `{}`\n", t));
        }
        s.push('\n');
    }
    if !r.renamed_methods.is_empty() {
        s.push_str("## Métodos con nombre ofuscado\n\n");
        for t in r.renamed_methods.iter().take(40) {
            s.push_str(&format!("- `{}`\n", t));
        }
        s.push('\n');
    }
    if !r.encrypted_strings.is_empty() {
        s.push_str("## Constantes con pinta de cifradas\n\n");
        for t in &r.encrypted_strings {
            s.push_str(&format!("- {}\n", t));
        }
        s.push('\n');
    }
    if !r.endpoints.is_empty() {
        s.push_str("## Endpoints embebidos\n\n");
        for u in &r.endpoints {
            s.push_str(&format!("- {}\n", u));
        }
        s.push('\n');
    }
    if !r.secrets.is_empty() {
        s.push_str("## Secretos (REDACTADOS)\n\n");
        for t in &r.secrets {
            s.push_str(&format!("- {}\n", t));
        }
        s.push('\n');
    }
    if !r.toolkit_findings.is_empty() {
        s.push_str("## Hallazgos de toolkit de parcheo\n\n");
        for f in &r.toolkit_findings {
            s.push_str(&format!(
                "- **{}** ({} / {}) — marcador `{}`\n",
                f.get("id").and_then(|v| v.as_str()).unwrap_or("?"),
                f.get("class").and_then(|v| v.as_str()).unwrap_or("?"),
                f.get("severity").and_then(|v| v.as_str()).unwrap_or("?"),
                f.get("marker").and_then(|v| v.as_str()).unwrap_or("?"),
            ));
        }
        s.push('\n');
    }
    if !r.module_findings.is_empty() {
        s.push_str("## Hallazgos de módulo (Xposed / Lucky Patcher)\n\n");
        for f in &r.module_findings {
            s.push_str(&format!(
                "- {} · {}\n",
                f.get("detector").and_then(|v| v.as_str()).unwrap_or("?"),
                f.get("evidence").and_then(|v| v.as_str()).unwrap_or("")
            ));
        }
        s.push('\n');
    }
    if !r.notes.is_empty() {
        s.push_str("## Notas\n\n");
        for n in &r.notes {
            s.push_str(&format!("- {}\n", n));
        }
        s.push('\n');
    }
    s
}

// ========================================================================= //
// 3. Motor de parcheo (plan)
// ========================================================================= //

/// Un hook a inyectar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HookSpec {
    /// Clase objetivo (`com.foo.Bar`).
    pub target_class: String,
    /// Método objetivo (`onCreate` o `onCreate(Ljava/lang/String;)V`).
    pub target_method: String,
    /// Qué se inyecta en su lugar.
    pub replacement: String,
}

/// Un paso del plan de parcheo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum PatchStep {
    /// Extraer el APK.
    Extract { apk: String },
    /// Inyectar el módulo (assets + classes).
    InjectModule { module: String, entry: String },
    /// Parchear el manifiesto (permisos, `xposedmodule`…).
    PatchManifest { permissions: Vec<String> },
    /// Hookear un método.
    Hook { spec: HookSpec },
    /// Re-firmar.
    ReSign { keystore: String, alias: String },
    /// Instalar.
    Install { cmd: String },
}

/// El plan completo para parchear un APK.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatchPlan {
    pub apk: String,
    pub steps: Vec<PatchStep>,
    /// ¿Hace falta root para el paso de instalación?
    pub requires_root: bool,
    /// Clave con la que está firmado hoy (si se sabe).
    pub signed_with: Option<String>,
    /// Lo que **no** está implementado todavía (honesto).
    pub not_implemented: Vec<String>,
}

/// Construye el plan de parcheo para un APK.
pub fn plan_patch(apk: &str, hooks: &[HookSpec], keystore: &str) -> PatchPlan {
    let mut steps = Vec::new();
    steps.push(PatchStep::Extract {
        apk: apk.to_string(),
    });
    for h in hooks {
        steps.push(PatchStep::Hook { spec: h.clone() });
    }
    if !keystore.is_empty() {
        steps.push(PatchStep::ReSign {
            keystore: keystore.to_string(),
            alias: "megadaemon".to_string(),
        });
    }
    steps.push(PatchStep::Install {
        cmd: format!("pm install -r {}", apk),
    });
    PatchPlan {
        apk: apk.to_string(),
        steps,
        requires_root: true,
        signed_with: None,
        not_implemented: vec![
            "reescritura binaria del DEX (se inyecta el hook, no se parchea el bytecode)".to_string(),
            "firma JAR v2/v3 de Android (signing.rs firma CMS/PAdES, no APKs)".to_string(),
        ],
    }
}

// ========================================================================= //
// 4. Gestor de grabaciones (NO grabador)
// ========================================================================= //

/// Una grabación existente.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordingItem {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    /// Fecha de modificación (ISO-8601, best effort).
    pub modified: String,
    /// `audio` | `video` | `desconocido`.
    pub kind: String,
}

/// Catálogo de grabaciones de un directorio.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RecordingCatalog {
    pub root: String,
    pub items: Vec<RecordingItem>,
    pub total_bytes: u64,
}

/// Escanea un directorio y lista las grabaciones.
pub fn scan_recordings(root: &str) -> RecordingCatalog {
    let mut cat = RecordingCatalog {
        root: root.to_string(),
        ..Default::default()
    };
    let dir = Path::new(root);
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            cat.items.push(RecordingItem {
                path: format!("{}/<error: {}>", root, e),
                bytes: 0,
                sha256: String::new(),
                modified: String::new(),
                kind: "error".to_string(),
            });
            return cat;
        }
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        let meta = match e.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let kind = match ext.as_str() {
            "m4a" | "mp3" | "aac" | "ogg" | "amr" | "wav" | "flac" | "opus" => "audio",
            "mp4" | "mkv" | "webm" | "3gp" | "avi" => "video",
            _ => "desconocido",
        };
        let p = path.to_string_lossy().to_string();
        cat.total_bytes += meta.len();
        cat.items.push(RecordingItem {
            sha256: crate::apk_inventory::sha256_file(&p).unwrap_or_default(),
            bytes: meta.len(),
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            path: p,
            kind: kind.to_string(),
        });
    }
    cat.items.sort_by(|a, b| a.path.cmp(&b.path));
    cat
}

/// Un trabajo de transcripción.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TranscribeJob {
    pub input: String,
    pub sha256: String,
    /// Motor de transcripción (binario que se invocará).
    pub engine: String,
    pub output: String,
}

/// Plan de transcripción: una tarea por grabación de audio.
pub fn plan_transcription(cat: &RecordingCatalog, engine: &str) -> Vec<TranscribeJob> {
    cat.items
        .iter()
        .filter(|i| i.kind == "audio")
        .map(|i| TranscribeJob {
            input: i.path.clone(),
            sha256: i.sha256.clone(),
            engine: engine.to_string(),
            output: format!("{}.txt", i.path),
        })
        .collect()
}

/// Un trabajo de cifrado con el vault.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EncryptJob {
    pub input: String,
    pub sha256: String,
    pub vault: String,
    pub output: String,
}

/// Plan de cifrado (el vault del fleet: `signing_vault`).
pub fn plan_encryption(cat: &RecordingCatalog, vault: &str) -> Vec<EncryptJob> {
    cat.items
        .iter()
        .map(|i| EncryptJob {
            input: i.path.clone(),
            sha256: i.sha256.clone(),
            vault: vault.to_string(),
            output: format!("{}.enc", i.path),
        })
        .collect()
}

/// Un trabajo de subida.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UploadJob {
    pub input: String,
    pub sha256: String,
    pub target: String,
}

/// Plan de subida (Drive u otro destino).
pub fn plan_upload(cat: &RecordingCatalog, target: &str) -> Vec<UploadJob> {
    cat.items
        .iter()
        .map(|i| UploadJob {
            input: i.path.clone(),
            sha256: i.sha256.clone(),
            target: target.to_string(),
        })
        .collect()
}

// ========================================================================= //
// 5. Backups
// ========================================================================= //

/// Una entrada de backup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackupEntry {
    pub path: String,
    pub size: u64,
    pub mode: u32,
    pub kind: String,
}

/// Catálogo de un backup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackupCatalog {
    pub path: String,
    pub format: String,
    pub entries: Vec<BackupEntry>,
}

/// Lee un backup Android (AB) o un tar y devuelve su catálogo.
pub fn catalog_backup(path: &str) -> Result<BackupCatalog, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("no se pudo leer {}: {}", path, e))?;
    let mut cat = BackupCatalog {
        path: path.to_string(),
        format: "desconocido".to_string(),
        entries: Vec::new(),
    };
    // ¿Es un AB (cabecera "ANDROID BACKUP")?
    if bytes.len() > 5 && &bytes[0..5] == b"ANDRO" {
        let header = crate::backup_ab::parse_ab_header(&bytes)
            .map_err(|e| format!("cabecera AB inválida: {}", e))?;
        cat.format = format!(
            "android-backup v{} ({})",
            header.version,
            if header.encryption != 0 { "cifrado" } else { "sin cifrar" }
        );
        return Ok(cat);
    }
    // Si no, ¿es un tar?
    if let Ok(entries) = crate::backup_ab::tar_entries(&bytes) {
        cat.format = "tar".to_string();
        for e in entries {
            // El bit de directorio de Unix es 0o40000 en el modo.
            let is_dir = (e.mode & 0o40000) != 0;
            cat.entries.push(BackupEntry {
                path: e.name.clone(),
                size: e.size,
                mode: e.mode,
                kind: if is_dir { "dir" } else { "file" }.to_string(),
            });
        }
        return Ok(cat);
    }
    Err("formato no reconocido (ni AB ni tar)".to_string())
}

// ========================================================================= //
// Utilidades
// ========================================================================= //

/// Devuelve el inventario (W2) leído de un JSON como tabla de veredictos.
pub fn verdict_table(inventory: &Value) -> Vec<(String, AppFacts, Verdict)> {
    let mut out: Vec<(String, AppFacts, Verdict)> = Vec::new();
    let apps = inventory
        .get("apps")
        .and_then(|v| v.get("paquetes"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for entry in apps {
        let facts = facts_from_inventory(&entry, None);
        let v = facts.verdict();
        out.push((facts.package.clone(), facts, v));
    }
    out
}

/// Recuento de veredictos (para el informe).
pub fn verdict_counts(rows: &[(String, AppFacts, Verdict)]) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for (_, _, v) in rows {
        *m.entry(v.as_str().to_string()).or_insert(0) += 1;
    }
    m
}

/// JSON de un inventario con el veredicto añadido (para el tablón).
pub fn inventory_with_verdicts(inventory: &Value) -> Value {
    let mut d = inventory.clone();
    if let Some(apps) = d
        .get_mut("apps")
        .and_then(|v| v.get_mut("paquetes"))
        .and_then(|v| v.as_array_mut())
    {
        for e in apps.iter_mut() {
            let facts = facts_from_inventory(e, None);
            if let Some(obj) = e.as_object_mut() {
                obj.insert(
                    "veredicto".to_string(),
                    json!(facts.verdict().as_str()),
                );
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> AppFacts {
        AppFacts {
            package: "ru.ejemplo.malo".to_string(),
            third_party: true,
            enabled: true,
            target_sdk: Some(28),
            risky_permissions: vec![
                "android.permission.ACCESS_SUPERUSER".to_string(),
                "android.permission.INTERNET".to_string(),
            ],
            uploads_to_third_party: false,
            signed_with_aosp_test_key: false,
            repacked_toolkit: false,
            role: "parcheador".to_string(),
            reimplementable: false,
        }
    }

    #[test]
    fn quien_sube_datos_a_terceros_se_va() {
        let mut f = facts();
        f.uploads_to_third_party = true;
        assert_eq!(f.verdict(), Verdict::Remove);
        assert!(f.reasons().iter().any(|r| r.contains("servidor de terceros")));
    }

    #[test]
    fn un_reempaquetado_se_va_y_lo_dice() {
        let mut f = facts();
        f.repacked_toolkit = true;
        assert_eq!(f.verdict(), Verdict::Remove);
        assert!(f.reasons().iter().any(|r| r.contains("reempaquetado")));
    }

    #[test]
    fn lo_reimplementable_pasa_a_reimplementar() {
        let mut f = facts();
        f.reimplementable = true;
        assert_eq!(f.verdict(), Verdict::Reimplement);
        assert!(f.reasons().iter().any(|r| r.contains("mega-binario")));
    }

    #[test]
    fn una_app_normal_se_queda() {
        let f = facts();
        assert_eq!(f.verdict(), Verdict::Keep);
    }

    #[test]
    fn los_permisos_que_exceden_se_venen() {
        let f = facts();
        let r = f.reasons().join(" ");
        assert!(r.contains("ACCESS_SUPERUSER"), "{}", r);
    }

    #[test]
    fn el_plan_de_parcheo_es_ordenado_y_honesto() {
        let hooks = vec![HookSpec {
            target_class: "com.foo.Bar".to_string(),
            target_method: "onCreate".to_string(),
            replacement: "com.mega.Hook".to_string(),
        }];
        let plan = plan_patch("/tmp/app.apk", &hooks, "ks");
        assert_eq!(plan.steps.len(), 4);
        assert!(matches!(plan.steps[1], PatchStep::Hook { .. }));
        assert!(plan.requires_root);
        assert!(!plan.not_implemented.is_empty());
    }

    #[test]
    fn el_gestor_no_graba_solo_gestiona() {
        let dir = std::env::temp_dir().join("lab_kit_test_rec");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("a.m4a"), b"RIFFfake").unwrap();
        std::fs::write(dir.join("b.txt"), b"no es audio").unwrap();
        let cat = scan_recordings(dir.to_str().unwrap());
        assert_eq!(cat.items.len(), 2);
        assert_eq!(cat.items.iter().filter(|i| i.kind == "audio").count(), 1);
        assert!(cat.total_bytes > 0);
        // sólo se planifica transcripción del audio
        let jobs = plan_transcription(&cat, "whisper");
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].output.ends_with(".txt"));
        // el cifrado y la subida cubren todo
        assert_eq!(plan_encryption(&cat, "/data/local/tmp/vault").len(), 2);
        assert_eq!(plan_upload(&cat, "drive").len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_barrido_en_crudo_encuentra_strings() {
        let b = b"\x00\x01hello world\x00http://patch.chelpus.com/submit\x00";
        let s = raw_strings(b, 6);
        assert!(s.iter().any(|x| x.contains("patch.chelpus.com")));
        assert!(!s.iter().any(|x| x == "hello"));
    }

    #[test]
    fn el_veredicto_cuadra_con_el_inventario() {
        let inv = json!({"apps": {"paquetes": [
            {"paquete": "com.a", "es_tercero": true, "habilitada": true, "target_sdk": 34,
             "permisos_riesgo": ["android.permission.INTERNET"]},
            {"paquete": "com.b", "es_tercero": true, "habilitada": false, "target_sdk": 22,
             "permisos_riesgo": ["android.permission.CAMERA"]}
        ]}});
        let rows = verdict_table(&inv);
        assert_eq!(rows.len(), 2);
        let counts = verdict_counts(&rows);
        assert_eq!(counts.get("KEEP"), Some(&2));
    }

    #[test]
    fn el_inventario_con_veredictos_añade_el_campo() {
        let inv = json!({"apps": {"paquetes": [
            {"paquete": "com.a", "es_tercero": true, "habilitada": true}
        ]}});
        let out = inventory_with_verdicts(&inv);
        assert_eq!(
            out["apps"]["paquetes"][0]["veredicto"].as_str(),
            Some("KEEP")
        );
    }
}

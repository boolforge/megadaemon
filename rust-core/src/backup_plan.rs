//! **Plan de backup**: el LLM *propone* qué respaldar y qué excluir; el
//! **código valida** la propuesta y **nunca se confía en la IA**.
//!
//! QUÉ ES Y POR QUÉ
//! ----------------
//! El gateway `megadaemon` (`crate::llm_gateway`, commit `855380e`) expone
//! `chat_completions(&Value) -> Result<Value, GatewayError>`: un endpoint
//! OpenAI-compatible que enruta al proveedor por defecto con *fallback* real.
//! Eso permite que el **LLM proponga un plan** de backup (qué paquetes
//! respaldar, qué excluir) a partir del **inventario real** del móvil.
//!
//! Pero un modelo puede:
//!
//! * inventarse un paquete que no está instalado;
//! * proponer excluir `/data/user/0/com.WhatsApp` (justo lo que hay que
//!   respaldar);
//! * pedir la **puerta** `Android/media` / `Android/data` (el framework los
//!   está usando: copiarlos en caliente da ficheros inconsistentes);
//! * proponer tocar el **CSC** de Samsung (`/data/misc/keystore`, `keychain`,
//!   `credstore`, `knoxcore`, `fabric_crypto`): material criptográfico vivo.
//!
//! Por eso la propuesta **se valida en código, con reglas fijas**, y la
//! decisión que se aplica es la **validada**, no la que pidió el modelo. La
//! propuesta original (con su texto crudo) **se registra** en el plan para
//! que quede auditoría: qué pidió la IA, qué se aceptó y qué se rechazó.
//!
//! REGLAS DURAS (en código, no en el prompt)
//! -----------------------------------------
//!
//! | # | Regla | Motivo |
//! |---|-------|--------|
//! | 1 | `allow_live_framework` **siempre `false`** | sólo el humano abre esa puerta |
//! | 2 | `include_media` / `include_external` sólo si el humano la abrió | el framework está en uso |
//! | 3 | un paquete sólo vale si **existe en el móvil** | la IA no inventa inventario |
//! | 4 | los patrones de exclusión no pueden contener `..`, rutas absolutas ni `\0` | no se sale del contenedor |
//! | 5 | los patrones no pueden apuntar al CSC | el CSC no se toca ni se "excluye": está prohibido |
//! | 6 | tope de paquetes propuestos (200) | una propuesta absurda no se aplica entera |
//!
//! USO
//! ---
//!
//! ```text
//! md-backup plan --out DIR [--model megadaemon] [--allow-live-framework]
//! md-backup create --out DIR --plan DIR/plan.json ...
//! ```
//!
//! `plan` escribe `plan.json` (la **decisión validada** + la propuesta cruda).
//! `create --plan` **no** vuelve a preguntar al modelo: aplica el plan ya
//! validado y lo deja registrado en el contenedor (`plan.json` + SHA-256 en
//! la cabecera).

use crate::backup_apps::{is_forbidden_path, AppEntry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nombre del fichero de plan dentro del contenedor.
pub const PLAN_FILE: &str = "plan.json";
/// Tope de paquetes que se aceptan de una propuesta.
pub const MAX_PLANNED_PACKAGES: usize = 200;
/// Tope de patrones de exclusión que se aceptan de una propuesta.
pub const MAX_PLANNED_PATTERNS: usize = 64;

// ========================================================================= //
// 1. La propuesta (lo que pide la IA)
// ========================================================================= //

/// Lo que el modelo **propone**. Puede ser incorrecto: por eso existe
/// [`validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PlanProposal {
    /// Paquetes que el modelo quiere respaldar.
    #[serde(default)]
    pub packages: Vec<String>,
    /// Globs que el modelo quiere excluir.
    #[serde(default, alias = "exclude")]
    pub exclude_patterns: Vec<String>,
    /// ¿Quiere incluir `Android/media`?
    #[serde(default)]
    pub include_media: bool,
    /// ¿Quiere incluir `Android/data`?
    #[serde(default)]
    pub include_external: bool,
    /// ¿Pide la puerta `allow_live_framework`?
    #[serde(default)]
    pub allow_live_framework: bool,
    /// Por qué lo propone (el modelo lo dice; queda auditado).
    #[serde(default, alias = "reason", alias = "explanation")]
    pub rationale: String,
    /// Texto crudo que devolvió el modelo (auditoría completa).
    #[serde(default)]
    pub raw: String,
}

// ========================================================================= //
// 2. La decisión (lo que el código autoriza)
// ========================================================================= //

/// Un motivo de rechazo (queda en el plan).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rejection {
    /// Qué se rechazó (paquete o patrón).
    pub what: String,
    /// Por qué.
    pub reason: String,
}

/// El plan **validado**: es lo único que el motor aplica.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanDecision {
    /// La propuesta original (con su texto crudo): auditoría.
    pub proposal: PlanProposal,
    /// Paquetes **aceptados** (existen en el móvil).
    pub packages: Vec<String>,
    /// Patrones de exclusión **aceptados**.
    pub exclude_patterns: Vec<String>,
    /// ¿Se incluye `Android/media`? (sólo si el humano abrió la puerta).
    pub include_media: bool,
    /// ¿Se incluye `Android/data`? (sólo si el humano abrió la puerta).
    pub include_external: bool,
    /// **Siempre `false`**: la IA no abre esa puerta.
    pub allow_live_framework: bool,
    /// Lo que se rechazó y por qué.
    pub rejections: Vec<Rejection>,
    /// Cuándo se validó (epoch).
    pub validated_at: u64,
}

impl PlanDecision {
    /// Resumen para la cabecera del contenedor (lo que se registra).
    pub fn summary(&self) -> Value {
        json!({
            "packages": self.packages.len(),
            "exclude_patterns": self.exclude_patterns.len(),
            "include_media": self.include_media,
            "include_external": self.include_external,
            "allow_live_framework": self.allow_live_framework,
            "rejections": self.rejections.len(),
            "rationale": self.proposal.rationale,
            "validated_at": self.validated_at,
        })
    }

    /// SHA-256 canónico del plan (para atarlo a la cabecera).
    pub fn hash(&self) -> String {
        let mut v = json!({
            "packages": self.packages,
            "exclude_patterns": self.exclude_patterns,
            "include_media": self.include_media,
            "include_external": self.include_external,
            "allow_live_framework": self.allow_live_framework,
            "validated_at": self.validated_at,
        });
        // Orden estable de las claves: `serde_json` con `BTreeMap` no está
        // garantizado, así que se serializa a texto y se hashea.
        let text = serde_json::to_string(&v).unwrap_or_default();
        v = Value::Null;
        let _ = v;
        crate::backup_apps::sha256_bytes(text.as_bytes())
    }
}

// ========================================================================= //
// 3. Validación (el código manda)
// ========================================================================= //

/// Valida una propuesta contra el **inventario real** del móvil.
///
/// `human_gate` es la puerta que abre **el humano** (`--allow-live-framework`
/// en la CLI): sin ella, ni la IA ni el plan tocan `Android/media` ni
/// `Android/data`.
pub fn validate(
    proposal: &PlanProposal,
    real_packages: &[String],
    human_gate: bool,
) -> PlanDecision {
    let mut rejections: Vec<Rejection> = Vec::new();
    let mut packages: Vec<String> = Vec::new();
    let mut exclude_patterns: Vec<String> = Vec::new();

    // Regla 1: la IA **no** abre la puerta del framework vivo.
    if proposal.allow_live_framework {
        rejections.push(Rejection {
            what: "allow_live_framework".to_string(),
            reason: "la puerta del framework vivo la abre el humano, no el modelo".to_string(),
        });
    }

    // Regla 3: un paquete sólo vale si existe en el móvil.
    let mut unknown = 0usize;
    for p in &proposal.packages {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        if !real_packages.iter().any(|r| r == p) {
            unknown += 1;
            if unknown <= 20 {
                rejections.push(Rejection {
                    what: p.to_string(),
                    reason: "no está instalado en el móvil (no se respalda nada que no exista)"
                        .to_string(),
                });
            }
            continue;
        }
        if packages.len() >= MAX_PLANNED_PACKAGES {
            rejections.push(Rejection {
                what: p.to_string(),
                reason: format!("se pasó el tope de {MAX_PLANNED_PACKAGES} paquetes del plan"),
            });
            continue;
        }
        if !packages.contains(&p.to_string()) {
            packages.push(p.to_string());
        }
    }
    if unknown > 20 {
        rejections.push(Rejection {
            what: "…".to_string(),
            reason: format!("{unknown} paquetes más inexistentes (resumidos)"),
        });
    }

    // Regla 4 y 5: los patrones tienen que ser seguros.
    for pat in &proposal.exclude_patterns {
        let pat = pat.trim();
        if pat.is_empty() {
            continue;
        }
        // El CSC va PRIMERO: es la regla MAS ESPECIFICA y la que explica al
        // usuario por qué su patrón se rechaza ("excluir el CSC" no es un
        // patrón inseguro cualquiera: es algo prohibido siempre).
        // MEDIDO (2026-11-23, test rojo el_csc_no_se_puede_excluir_ni_tocar):
        // '/data/misc/keystore' empieza por '/' => se rechazaba antes como
        // 'patrón inseguro' y el motivo no mencionaba el CSC.
        if is_forbidden_path(pat) {
            rejections.push(Rejection {
                what: pat.to_string(),
                reason: "el CSC está prohibido siempre: no se excluye, no se toca".to_string(),
            });
            continue;
        }
        if pat.contains("..") || pat.starts_with('/') || pat.contains('\0') {
            rejections.push(Rejection {
                what: pat.to_string(),
                reason: "patrón inseguro (ruta absoluta, `..` o NUL): no se aplica".to_string(),
            });
            continue;
        }
        if exclude_patterns.len() >= MAX_PLANNED_PATTERNS {
            rejections.push(Rejection {
                what: pat.to_string(),
                reason: format!("se pasó el tope de {MAX_PLANNED_PATTERNS} patrones del plan"),
            });
            continue;
        }
        if !exclude_patterns.contains(&pat.to_string()) {
            exclude_patterns.push(pat.to_string());
        }
    }

    // Regla 2: media/data sólo con la puerta del humano.
    let include_media = proposal.include_media && human_gate;
    if proposal.include_media && !human_gate {
        rejections.push(Rejection {
            what: "include_media".to_string(),
            reason: "Android/media sólo con la puerta del humano (--allow-live-framework)"
                .to_string(),
        });
    }
    let include_external = proposal.include_external && human_gate;
    if proposal.include_external && !human_gate {
        rejections.push(Rejection {
            what: "include_external".to_string(),
            reason: "Android/data sólo con la puerta del humano (--allow-live-framework)"
                .to_string(),
        });
    }

    PlanDecision {
        proposal: proposal.clone(),
        packages,
        exclude_patterns,
        include_media,
        include_external,
        allow_live_framework: false,
        rejections,
        validated_at: now_epoch(),
    }
}

// ========================================================================= //
// 4. Parseo de la respuesta del modelo
// ========================================================================= //

/// Saca el primer objeto JSON balanceado de un texto (el modelo a veces
/// mete prosa o vallas ```` ```json ```` alrededor).
pub fn extract_json_object(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let start = text.find('{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for i in start..bytes.len() {
        let c = bytes[i] as char;
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Convierte la respuesta cruda del modelo en una [`PlanProposal`].
pub fn parse_proposal(text: &str) -> Result<PlanProposal, String> {
    let raw = text.to_string();
    let json_text = extract_json_object(text)
        .ok_or_else(|| "el modelo no devolvió un objeto JSON".to_string())?;
    let v: Value = serde_json::from_str(&json_text)
        .map_err(|e| format!("JSON del modelo inválido: {e}"))?;

    let strings = |key: &str| -> Vec<String> {
        v.get(key)
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };
    let flag = |key: &str| -> bool { v.get(key).and_then(|x| x.as_bool()).unwrap_or(false) };

    // `exclude` es el alias más común; `exclude_patterns` el canónico.
    let mut exclude_patterns = strings("exclude_patterns");
    if exclude_patterns.is_empty() {
        exclude_patterns = strings("exclude");
    }
    // `packages` puede venir como array o como texto con comas.
    let mut packages = strings("packages");
    if packages.is_empty() {
        if let Some(s) = v.get("packages").and_then(|x| x.as_str()) {
            packages = s
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
        }
    }
    let rationale = ["rationale", "reason", "explanation", "why"]
        .iter()
        .find_map(|k| v.get(*k).and_then(|x| x.as_str()))
        .unwrap_or("")
        .trim()
        .to_string();

    Ok(PlanProposal {
        packages,
        exclude_patterns,
        include_media: flag("include_media"),
        include_external: flag("include_external"),
        allow_live_framework: flag("allow_live_framework"),
        rationale,
        raw,
    })
}

// ========================================================================= //
// 5. La llamada al LLM
// ========================================================================= //

/// Construye el prompt con el inventario **medido** del móvil.
///
/// El prompt es corto a propósito: el móvil manda, el modelo decide, el
/// código manda de verdad.
pub fn build_prompt(inventory: &[AppEntry], max_apps: usize) -> String {
    let mut s = String::new();
    s.push_str(
        "Eres un planificador de copias de seguridad de un movil Android.\n\
         Responde SOLO con un objeto JSON, sin prosa y sin vallas de codigo.\n\
         Formato exacto:\n\
         {\"packages\":[\"pkg1\"],\"exclude\":[\"glob\"],\"include_media\":false,\
         \"include_external\":false,\"rationale\":\"...\"}\n\
         Reglas que ya se aplican en codigo (no hace falta pedirlas):\n\
         - no puedes incluir Android/media ni Android/data: se ignoran.\n\
         - no puedes excluir /data/misc/keystore ni el CSC: estan prohibidos.\n\
         - un paquete que no esta en la lista no existe: no lo pongas.\n\
         - excluye lo regenerable (cache/, code_cache/, no_backup/, *.dm).\n\
         Inventario medido (paquete, MiB de datos privados, MiB del APK):\n",
    );
    for a in inventory.iter().take(max_apps) {
        let data = a
            .data_bytes
            .map(|b| format!("{:.1}", b as f64 / (1024.0 * 1024.0)))
            .unwrap_or_else(|| "?".to_string());
        let apk = a
            .apk_bytes
            .map(|b| format!("{:.1}", b as f64 / (1024.0 * 1024.0)))
            .unwrap_or_else(|| "?".to_string());
        s.push_str(&format!("{} | datos {} MiB | apk {} MiB\n", a.package, data, apk));
    }
    if inventory.len() > max_apps {
        s.push_str(&format!(
            "... y {} apps mas (no las incluyas salvo que digan 'todas')\n",
            inventory.len() - max_apps
        ));
    }
    s.push_str(
        "Elige los paquetes que hay que respaldar por prioridad (el usuario no puede perderlos) \
         y excluye lo que no aporta. No inventes nombres de paquete.\n",
    );
    s
}

/// Llama al gateway `megadaemon` y devuelve la propuesta **sin validar**.
///
/// El modelo **no** decide: esto es sólo la propuesta. Quien manda es
/// [`validate`].
pub fn propose_with_llm(
    inventory: &[AppEntry],
    model: &str,
    max_apps: usize,
) -> Result<PlanProposal, String> {
    let prompt = build_prompt(inventory, max_apps);
    let body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "Respondes solo con JSON valido."},
            {"role": "user", "content": prompt}
        ],
        "max_tokens": 1200,
    });
    let resp = crate::llm_gateway::chat_completions(&body)
        .map_err(|e| format!("el gateway LLM fallo: {}: {}", e.status, e.message))?;
    let text = resp
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| "el gateway LLM no devolvio contenido".to_string())?;
    parse_proposal(text)
}

// ========================================================================= //
// 6. Persistencia del plan
// ========================================================================= //

/// Escribe el plan (decisión + propuesta) en `path`.
pub fn write_plan(path: &Path, decision: &PlanDecision) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("no se puede crear {}: {e}", parent.display()))?;
    }
    let doc = json!({
        "magic": "MDPLAN1",
        "decision": decision,
        "hash": decision.hash(),
    });
    std::fs::write(path, serde_json::to_vec_pretty(&doc).unwrap_or_default())
        .map_err(|e| format!("no se puede escribir {}: {e}", path.display()))
}

/// Lee un plan validado de `path`.
pub fn read_plan(path: &Path) -> Result<PlanDecision, String> {
    let data =
        std::fs::read_to_string(path).map_err(|e| format!("no se puede leer {}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(&data)
        .map_err(|e| format!("plan inválido en {}: {e}", path.display()))?;
    let d = v
        .get("decision")
        .ok_or_else(|| "el fichero de plan no trae 'decision'".to_string())?;
    serde_json::from_value::<PlanDecision>(d.clone())
        .map_err(|e| format!("decisión del plan inválida: {e}"))
}

/// Ruta por defecto del plan dentro de un contenedor.
pub fn plan_path(container: &Path) -> PathBuf {
    container.join(PLAN_FILE)
}

/// Epoch actual.
pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ========================================================================= //
// 7. Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    fn real() -> Vec<String> {
        vec![
            "com.android.settings".to_string(),
            "com.google.android.youtube".to_string(),
            "app.lawnchair".to_string(),
        ]
    }

    #[test]
    fn extrae_el_primer_objeto_json_sin_romperlo_con_llaves_dentro_de_cadenas() {
        let t = "antes {\"packages\":[\"a\"],\"rationale\":\"llave { dentro\"} despues";
        let j = extract_json_object(t).expect("debe extraer");
        let v: Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["packages"][0], "a");
    }

    #[test]
    fn parsea_una_propuesta_con_alias() {
        let p = parse_proposal(
            r#"{"packages":["com.android.settings"],"exclude":["*.tmp"],"reason":"porque si"}"#,
        )
        .unwrap();
        assert_eq!(p.packages, vec!["com.android.settings".to_string()]);
        assert_eq!(p.exclude_patterns, vec!["*.tmp".to_string()]);
        assert_eq!(p.rationale, "porque si");
    }

    #[test]
    fn la_ia_no_puede_abrir_la_puerta_del_framework_vivo() {
        let p = PlanProposal {
            allow_live_framework: true,
            include_media: true,
            include_external: true,
            ..Default::default()
        };
        let d = validate(&p, &real(), false);
        assert!(!d.allow_live_framework);
        assert!(!d.include_media);
        assert!(!d.include_external);
        assert_eq!(d.rejections.len(), 3);
    }

    #[test]
    fn la_puerta_del_humano_si_deja_media_y_data() {
        let p = PlanProposal {
            include_media: true,
            include_external: true,
            ..Default::default()
        };
        let d = validate(&p, &real(), true);
        assert!(d.include_media && d.include_external);
        // La puerta del framework vivo la abre el humano, no la IA.
        assert!(!d.allow_live_framework);
    }

    #[test]
    fn un_paquete_inexistente_se_rechaza() {
        let p = PlanProposal {
            packages: vec![
                "com.android.settings".to_string(),
                "com.no.existe.nada".to_string(),
            ],
            ..Default::default()
        };
        let d = validate(&p, &real(), false);
        assert_eq!(d.packages, vec!["com.android.settings".to_string()]);
        assert!(d.rejections.iter().any(|r| r.what == "com.no.existe.nada"));
    }

    #[test]
    fn los_patrones_inseguros_se_rechazan() {
        let p = PlanProposal {
            exclude_patterns: vec![
                "../../etc".to_string(),
                "/data/user/0".to_string(),
                "*.tmp".to_string(),
            ],
            ..Default::default()
        };
        let d = validate(&p, &real(), false);
        assert_eq!(d.exclude_patterns, vec!["*.tmp".to_string()]);
        assert_eq!(d.rejections.len(), 2);
    }

    #[test]
    fn el_csc_no_se_puede_excluir_ni_tocar() {
        let p = PlanProposal {
            exclude_patterns: vec!["/data/misc/keystore".to_string()],
            ..Default::default()
        };
        let d = validate(&p, &real(), false);
        assert!(d.exclude_patterns.is_empty());
        assert!(d.rejections[0].reason.contains("CSC"));
    }

    #[test]
    fn el_plan_sobrevive_a_un_viaje_de_ida_y_vuelta() {
        let p = PlanProposal {
            packages: vec!["app.lawnchair".to_string()],
            exclude_patterns: vec!["*.tmp".to_string()],
            rationale: "la app de escritorio".to_string(),
            ..Default::default()
        };
        let d = validate(&p, &real(), false);
        let dir = std::env::temp_dir().join(format!("md-plan-{}", std::process::id()));
        let path = dir.join("plan.json");
        write_plan(&path, &d).unwrap();
        let back = read_plan(&path).unwrap();
        assert_eq!(back.packages, d.packages);
        assert_eq!(back.exclude_patterns, d.exclude_patterns);
        assert_eq!(back.hash(), d.hash());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_prompt_no_inventa_paquetes() {
        let inv = vec![AppEntry {
            package: "app.lawnchair".to_string(),
            system: false,
            disabled: false,
            uid: 10123,
            apk_path: "/data/app/x/base.apk".to_string(),
            apk_paths: vec![],
            apk_bytes: Some(12 * 1024 * 1024),
            data_dir: "/data/user/0/app.lawnchair".to_string(),
            external_dir: String::new(),
            media_dir: String::new(),
            data_bytes: Some(40 * 1024 * 1024),
            file_count: Some(120),
        }];
        let p = build_prompt(&inv, 10);
        assert!(p.contains("app.lawnchair"));
        assert!(p.contains("40.0"));
        assert!(p.contains("12.0"));
    }
}

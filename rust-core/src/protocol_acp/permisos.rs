//! **Permisos ACP** (`mdaemon/policy/evaluate` y `mdaemon/sandbox/describe`),
//! módulo `permissions.rs` del plan de `protocol.rs`
//! (`docs/DECISION_ACP.md` §3).
//!
//! La idea del producto de referencia (medida): el **permiso lógico y el
//! aislamiento son el mismo objeto**.  En MegaDaemon eso ya existe:
//! `tool_policy.rs` decide (allow/ask/deny) y `sandbox_linux.rs` (patrón 8)
//! **compila esa decisión a una regla de aislamiento**.
//!
//! Esta capa NO reimplementa la política: la **pregunta** a
//! `tool_policy::evaluate_policy` y devuelve su veredicto en el formato que
//! espera un cliente ACP, con el estado del aislamiento al lado.
//!
//! El orden de las reglas es **deny > ask > allow** (lo que dice el propio
//! `tool_policy.rs`): si algo está denegado, ninguna otra regla lo revive.

use serde_json::{json, Value};

use crate::sandbox_linux;
use crate::tool_policy::{evaluate_policy, PolicyDecision, ToolContext};

/// El veredicto, en el formato de `mdaemon/policy/evaluate`.
#[derive(Debug, Clone, PartialEq)]
pub struct Veredicto {
    /// `allow`, `ask` o `deny`.
    pub decision: &'static str,
    /// Por qué (vacío si se permite).
    pub motivo: String,
    /// La clase de capacidad de la herramienta (`read_only`, `network`...).
    pub capacidad: String,
    /// Si el aislamiento está disponible en esta máquina.
    pub aislamiento: Option<&'static str>,
}

impl Veredicto {
    /// ¿Se puede ejecutar sin preguntar?
    pub fn permitido(&self) -> bool {
        self.decision == "allow"
    }

    /// El JSON que ve el cliente.
    pub fn a_json(&self) -> Value {
        json!({
            "decision": self.decision,
            "motivo": self.motivo,
            "capacidad": self.capacidad,
            "aislamiento": self.aislamiento,
        })
    }
}

/// Evalúa una herramienta con la política del repo (`deny > ask > allow`).
///
/// `aislamiento` se puede forzar a `None` para no mirar el sistema (los
/// tests); por defecto se usa el aislamiento REAL de la máquina.
pub fn evaluar(
    ctx: &ToolContext,
    tool_name: &str,
    args: &Value,
    aislamiento: Option<Option<&'static str>>,
) -> Veredicto {
    let d = evaluate_policy(ctx, tool_name, args);
    let aislamiento = match aislamiento {
        Some(a) => a,
        None => sandbox_linux::aislamiento_disponible(),
    };
    let (decision, motivo) = match d {
        PolicyDecision::Allow => ("allow", String::new()),
        PolicyDecision::Prompt { reason } => ("ask", reason),
        PolicyDecision::Deny { reason } => ("deny", reason),
    };
    Veredicto {
        decision,
        motivo,
        capacidad: crate::tool_policy::classify_tool(tool_name, args).as_str().to_string(),
        aislamiento,
    }
}

/// `mdaemon/sandbox/describe`: qué aislamiento hay en esta máquina.
pub fn describir_sandbox() -> Value {
    json!({
        "bwrap": sandbox_linux::bwrap_disponible(),
        "seatbelt": sandbox_linux::seatbelt_disponible(),
        "aislamiento": sandbox_linux::aislamiento_disponible(),
        // MEDIDO: el sandbox FALLA CERRADO: sin `bwrap` (ni `sandbox-exec`)
        // no hay aislamiento real ⇒ el permiso lógico solo, y se dice.
        "nota": "sin bwrap ni sandbox-exec no hay aislamiento real: el permiso lógico es lo único que hay",
    })
}

// =============================================================================
// CLI
// =============================================================================
/// `megadaemon-core acp-permisos <evaluar|describir>`
pub fn run_cli(args: &[String]) -> i32 {
    match run_cli_res(args) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("acp-permisos error: {}", e);
            1
        }
    }
}

fn run_cli_res(args: &[String]) -> Result<Value, String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    match sub {
        "evaluar" => {
            let tool = args.get(1).map(|s| s.as_str()).unwrap_or("");
            if tool.is_empty() {
                return Err("falta el nombre de la herramienta".to_string());
            }
            let args_json: Value = args
                .get(2)
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| json!({}));
            let ctx = ToolContext::for_agent(
                std::path::PathBuf::from("/tmp"),
                "acp-cli",
                false,
            );
            let v = evaluar(&ctx, tool, &args_json, Some(None));
            Ok(json!({ "ok": true, "veredicto": v.a_json() }))
        }
        "describir" => Ok(json!({ "ok": true, "sandbox": describir_sandbox() })),
        "" => Err("uso: acp-permisos <evaluar|describir> [tool] [args_json]".to_string()),
        otro => Err(format!("subcomando desconocido: {}", otro)),
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ctx() -> ToolContext {
        ToolContext::for_agent(PathBuf::from("/tmp"), "sesion-test", false)
    }

    #[test]
    fn una_lectura_se_permite() {
        let v = evaluar(&ctx(), "read_file", &json!({"path":"/tmp/x"}), Some(None));
        assert_eq!(v.decision, "allow");
        assert!(v.permitido());
    }

    #[test]
    fn un_comando_destructivo_se_deniega() {
        let v = evaluar(
            &ctx(),
            "run_command",
            &json!({"command": "rm -rf /"}),
            Some(None),
        );
        assert_eq!(v.decision, "deny");
        assert!(!v.permitido());
        assert!(!v.motivo.is_empty());
    }

    #[test]
    fn una_escritura_pide_confirmacion() {
        let v = evaluar(&ctx(), "write_file", &json!({"path":"/tmp/x"}), Some(None));
        assert_eq!(v.decision, "ask");
        assert!(!v.permitido());
    }

    #[test]
    fn el_denegar_gana_a_todo() {
        // MEDIDO: la regla es deny > ask > allow: un comando destructivo
        // se deniega aunque la política de la sesión permita escrituras.
        let v = evaluar(
            &ctx(),
            "run_command",
            &json!({"command": "mkfs.ext4 /dev/block/sda"}),
            Some(None),
        );
        assert_eq!(v.decision, "deny");
    }

    #[test]
    fn el_verdicto_conserva_la_capacidad() {
        let v = evaluar(&ctx(), "read_file", &json!({}), Some(None));
        assert_eq!(v.capacidad, "read_only");
    }

    #[test]
    fn el_json_del_veredicto_trae_las_cuatro_cosas() {
        let v = evaluar(&ctx(), "read_file", &json!({}), Some(None));
        let j = v.a_json();
        assert!(j.get("decision").is_some());
        assert!(j.get("motivo").is_some());
        assert!(j.get("capacidad").is_some());
        assert!(j.get("aislamiento").is_some());
    }

    #[test]
    fn la_descripcion_del_sandbox_no_promete_nada() {
        let s = describir_sandbox();
        assert!(s.get("bwrap").is_some());
        assert!(s.get("seatbelt").is_some());
        assert!(s.get("nota").is_some());
    }
}

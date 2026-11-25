//! Humo de la VOZ del daemon (`src/agent_voice.rs`).
//!
//! El módulo se usa desde la libreria (`use megadaemon_core::...`).
//! MEDIDO el 2026-11-21: antes se copiaba con `#[path = "../src/x.rs"]`; dentro de
//! un arnés `crate::` es la raíz del ARNÉS, así que un módulo copiado que usa
//! `crate::otro_modulo` no resolvía (E0433) y **el target de test no compilaba**
//! => la suite entera moría sin ejecutar ni un test.

use megadaemon_core::agent_voice;

use megadaemon_core::agent_voice::*;
use serde_json::{json, Value};
use std::cell::RefCell;

/// Tablero de pruebas: 3 tareas (una tomada, dos libres) y 2 peers.
fn ctx() -> VoiceContext {
    let mut ctx = VoiceContext::default();
    ctx.profile = VoiceProfile::new("megadaemon-voice", "samsung-s21-ultra", "https://x/mcp", true);
    ctx.tasks = vec![
        TaskView {
            task_id: "re-exynos2100".into(),
            title: "RE Exynos 2100".into(),
            capability: "reverse".into(),
            priority: 10,
            state: "published".into(),
            claimed_by: "cloud-sandbox".into(),
        },
        TaskView {
            task_id: "rom-migration".into(),
            title: "Plan de migración de ROM".into(),
            capability: "device".into(),
            priority: 8,
            state: "published".into(),
            claimed_by: String::new(),
        },
        TaskView {
            task_id: "backup-ultra".into(),
            title: "Backup ultra completo".into(),
            capability: "device".into(),
            priority: 9,
            state: "published".into(),
            claimed_by: String::new(),
        },
    ];
    ctx.peers = vec![
        PeerView {
            node_id: "cloud-sandbox".into(),
            instance: "cloud".into(),
            capabilities: vec!["reverse".into(), "npu".into()],
            last_seen: 0,
        },
        PeerView {
            node_id: "ui-v2".into(),
            instance: "sandbox".into(),
            capabilities: vec!["ui".into()],
            last_seen: 0,
        },
    ];
    ctx
}

struct Recorder {
    calls: RefCell<Vec<(String, Value)>>,
}

impl Executor for Recorder {
    fn call(&self, tool: &str, args: &Value) -> Result<Value, String> {
        self.calls.borrow_mut().push((tool.to_string(), args.clone()));
        Ok(json!({"summary": format!("hecho: {}", tool)}))
    }
}

#[test]
fn la_voz_reclama_la_tarea_libre_mas_prioritaria() {
    let ctx = ctx();
    let exec = Recorder {
        calls: RefCell::new(Vec::new()),
    };
    let out = tick(&ctx, &RuleBrain, &exec);
    assert!(out.ok, "la acción debe ejecutarse: {:?}", out.detail);
    assert_eq!(
        exec.calls.borrow()[0].0,
        "mesh_claim",
        "reclamar una tarea va por la tool mesh_claim"
    );
    assert_eq!(exec.calls.borrow()[0].1["task_id"], json!("backup-ultra"));
}

#[test]
fn la_voz_difunde_por_el_journal_sin_secretos() {
    let ctx = ctx();
    let exec = Recorder {
        calls: RefCell::new(Vec::new()),
    };
    let out = tick(&ctx, &RuleBrain, &exec);
    assert_eq!(out.broadcast.len(), 1);
    let line: Value = serde_json::from_str(&out.broadcast[0]).expect("NDJSON válido");
    assert_eq!(line["event"], json!("voice_action"));
    assert_eq!(line["node"], json!("megadaemon-voice"));
    assert_eq!(line["ok"], json!(true));
    let raw = out.broadcast[0].clone();
    assert!(!raw.contains("mdaemon_sk_"));
    assert!(!raw.contains("X-Seere-Auth-Token"));
    assert!(!raw.contains("c17398e"));
}

#[test]
fn la_voz_no_usa_llm_si_no_hay_tarea_para_mi() {
    let mut ctx = ctx();
    // Tarea libre de una capacidad que no tengo (solo tengo device/reverse).
    ctx.tasks = vec![TaskView {
        task_id: "gpu-only".into(),
        title: "solo GPU".into(),
        capability: "gpu".into(),
        priority: 99,
        state: "published".into(),
        claimed_by: String::new(),
    }];
    let exec = Recorder {
        calls: RefCell::new(Vec::new()),
    };
    let out = tick(&ctx, &RuleBrain, &exec);
    // No puede reclamarla: pregunta a un peer que sí tiene una capacidad que yo NO tengo
    // ("ui" la tiene ui-v2; "npu" ya la tengo yo, así que no le pregunto por eso).
    match &out.action {
        Action::Ask { peer, .. } => assert_eq!(peer, "ui-v2"),
        other => panic!("esperaba ask, {:?}", other),
    }
    assert_eq!(exec.calls.borrow()[0].0, "mesh_ask");
}

#[test]
fn el_llm_manda_y_si_responde_basura_se_cae_al_criterio_fijo() {
    let ctx = ctx();
    let exec = Recorder {
        calls: RefCell::new(Vec::new()),
    };
    // 1) El LLM manda: habla.
    let brain = LlmBrain::new("local-seere");
    let mut llm = brain;
    llm.extra_instructions = "habla".into();
    // Brain determinista que "es" el LLM: simulamos su salida parseándola.
    struct FakeLlm {
        inner: LlmBrain,
        reply: String,
    }
    impl Brain for FakeLlm {
        fn decide(&self, _ctx: &VoiceContext) -> Action {
            self.inner
                .parse_action(&self.reply)
                .unwrap_or_else(|| self.inner.fallback.decide(_ctx))
        }
    }
    let fake = FakeLlm {
        inner: LlmBrain::new("local-seere"),
        reply: r#"{"action":"speak","text":"hola enjambre"}"#.into(),
    };
    let out = tick(&ctx, &fake, &exec);
    assert_eq!(out.action, Action::Speak { text: "hola enjambre".into() });
    assert_eq!(exec.calls.borrow()[0].0, "swarm_chat");

    // 2) El LLM devuelve basura → el criterio fijo decide (reclamar).
    let broken = FakeLlm {
        inner: LlmBrain::new("local-seere"),
        reply: "lo siento, no puedo".into(),
    };
    let out2 = tick(&ctx, &broken, &exec);
    assert!(matches!(out2.action, Action::Claim { .. }));
}

#[test]
fn el_estado_de_la_voz_es_legible_por_la_ui() {
    let ctx = ctx();
    let s = voice_status(&ctx);
    assert_eq!(s["node_id"], json!("megadaemon-voice"));
    assert_eq!(s["tasks_total"], json!(3));
    assert_eq!(s["tasks_free"], json!(2));
    assert_eq!(s["peers"], json!(2));
    assert!(s["capabilities"].as_array().unwrap().len() >= 4);
    assert_eq!(whoami(&ctx.profile)["role"], json!("voice"));
}

#[test]
fn la_voz_respeta_el_cooldown_y_el_tope_por_tick() {
    let mut ctx = ctx();
    ctx.secs_since_last_tick = 0;
    ctx.cooldown_secs = 300;
    let exec = Recorder {
        calls: RefCell::new(Vec::new()),
    };
    let out = tick(&ctx, &RuleBrain, &exec);
    assert!(matches!(out.action, Action::Idle { .. }));
    assert!(exec.calls.borrow().is_empty(), "en cooldown no se ejecuta nada");
    assert!(out.broadcast.is_empty(), "un idle no se difunde");
}

// Modulo auxiliar del que depende el modulo incluido por `#[path]` (MEDIDO el
// 2026-11-21: al fusionar `fix/robustness-20261120` varios modulos pasaron a
// usar los helpers canonicos `util_text`/`util_fs` y los arneses
// autocontenidos dejaron de compilar: E0432).
#[path = "../src/util_text.rs"]
#[allow(dead_code, unused_imports, unused_mut)]
mod util_text;

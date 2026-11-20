//! F3 — arnés de integración del bucle agéntico recursivo (meta-agente).
//!
//! Un test por cada criterio de aceptación del encargo:
//!   1. memoria de fallos  -> el mismo error no se repite en el siguiente ciclo
//!   2. presupuesto por hora + coste real con su fuente (o NO VERIFICADO)
//!   3. degradación honesta (criterio fijo, nunca una acción inventada)
//!   4. el prompt NUNCA lleva la API key ni el token de sesión
//!   5. meta-agente: enruta al nodo/servicio correcto
//!   6. circuit breaker de 3 estados con backoff y ventana de recuperación medida
//!   + tope duro por tick (el móvil no aguanta un bucle cerrado de tools).

use megadaemon_core::agent_loop::{
    parse_decision, Action, AgentLoop, AgentLoopConfig, BreakerState, ChatBackend, ChatOutcome,
    CircuitBreaker, Cost, FailureMemory, HourBudget, PriceBook, ProviderRef, Router,
    ToolExecutor,
};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("megadaemon-f3-it-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

/// Backend simulado: devuelve respuestas en orden; `Err` simula un proveedor
/// caído (el 502 del maestro, por ejemplo).
struct Sim {
    replies: Vec<Result<String, String>>,
    seen: Vec<String>,
}

impl ChatBackend for Sim {
    fn chat(&mut self, provider: &ProviderRef, prompt: &str) -> Result<ChatOutcome, String> {
        let i = self.seen.len();
        self.seen.push(prompt.to_string());
        let r = self
            .replies
            .get(i)
            .cloned()
            .unwrap_or_else(|| Ok("{\"action\":\"idle\",\"reason\":\"fin\"}".into()));
        r.map(|reply| ChatOutcome {
            reply,
            tokens_used: 120,
            prompt_tokens: 60,
            latency_ms: 9.5,
            model_id: provider.model.clone(),
        })
    }
}

struct Exec {
    calls: Vec<String>,
    fail: Option<String>,
}

impl ToolExecutor for Exec {
    fn call(&mut self, action: &Action, _tick: u32) -> Result<String, String> {
        let k = action.key();
        self.calls.push(k.clone());
        if let Some(f) = &self.fail {
            if *f == k {
                return Err("EACCES: permission denied".into());
            }
        }
        Ok("ok".into())
    }
    fn spawns_per_call(&self) -> u32 {
        2 // cada `su -mm -c sh -c ...` son 2+ procesos en el movil
    }
}

struct Rtr {
    log: Vec<String>,
    task: Option<String>,
}

impl Router for Rtr {
    fn route(&self, provider: &str, task_type: &str) -> Option<String> {
        Some(format!("nodo:{} tarea:{}", provider, task_type))
    }
    fn fixed_fallback_task(&self) -> Option<String> {
        self.task.clone()
    }
}

fn loop_agente(path: PathBuf) -> AgentLoop {
    let precios = PriceBook::new()
        .with_source("modelo-remoto", 0.00015, 0.0006, "tarifa publicada (fuente: test)")
        .without_source("modelo-local");
    AgentLoop::new(
        AgentLoopConfig {
            max_calls_per_tick: 3,
            max_shell_spawns_per_tick: 4,
            failure_window_ticks: 5,
            budget: HourBudget::new(50_000, Some(0.50), 60),
        },
        FailureMemory::new(path),
        precios,
        vec![
            ProviderRef::remote("remoto", "modelo-remoto"),
            ProviderRef::local("seere", "modelo-local"),
        ],
    )
}

/// Criterio 1 — memoria de fallos.
#[test]
fn c1_el_mismo_error_no_se_repite_en_el_siguiente_ciclo() {
    let path = tmp("c1");
    let mut ag = loop_agente(path.clone());
    let mut sim = Sim {
        replies: vec![
            Ok("{\"action\":\"tool\",\"name\":\"shell\",\"args\":{}}".into()),
            Ok("{\"action\":\"tool\",\"name\":\"shell\",\"args\":{}}".into()),
            Ok("{\"action\":\"tool\",\"name\":\"apk_install\",\"args\":{}}".into()),
        ],
        seen: Vec::new(),
    };
    let mut ex = Exec {
        calls: Vec::new(),
        fail: Some("tool:shell".into()),
    };
    let mut rtr = Rtr {
        log: Vec::new(),
        task: None,
    };

    let r = ag.tick(1_700_000_000, "instala el apk de prueba", &mut sim, &mut ex, &mut rtr);

    // (a) la tool que falló queda registrada en `failures.ndjson`
    let errores = ag.failures.recent_errors("tool:shell", r.tick, 5);
    assert!(!errores.is_empty(), "el fallo debe quedar en la memoria");

    // (b) en el mismo tick el bucle se corrige: NO repite la tool que falló
    assert!(
        r.actions.iter().any(|a| a.contains("evitada")),
        "debe evitar la acción que ya falló: {:?}",
        r.actions
    );
    assert_eq!(
        ex.calls.iter().filter(|c| *c == "tool:shell").count(),
        1,
        "la tool que falla no se ejecuta dos veces: {:?}",
        ex.calls
    );
    assert!(ex.calls.contains(&"tool:apk_install".to_string()));

    // (c) el siguiente ciclo lleva la memoria en el prompt
    let mut sim2 = Sim {
        replies: vec![Ok("{\"action\":\"tool\",\"name\":\"otra\",\"args\":{}}".into())],
        seen: Vec::new(),
    };
    let mut ex2 = Exec {
        calls: Vec::new(),
        fail: None,
    };
    let mut rtr2 = Rtr {
        log: Vec::new(),
        task: None,
    };
    ag.tick(1_700_000_001, "instala el apk de prueba", &mut sim2, &mut ex2, &mut rtr2);
    assert!(
        sim2.seen[0].contains("YA fallo"),
        "el prompt siguiente debe llevar la memoria de fallos:\n{}",
        sim2.seen[0]
    );
}

/// Criterio 2 — presupuesto por hora + coste real con su fuente.
#[test]
fn c2_presupuesto_por_hora_y_coste_con_fuente_o_no_verificado() {
    let path = tmp("c2");
    let mut ag = loop_agente(path);

    // (a) el precio con fuente se cita
    let c = ag.prices.cost_of("modelo-remoto", 1_000, 1_000);
    assert!(c.is_verified());
    assert_eq!(c.source.as_deref(), Some("tarifa publicada (fuente: test)"));
    assert!((c.usd.unwrap() - 0.00075).abs() < 1e-9);

    // (b) el precio sin fuente NO se inventa
    let c2 = ag.prices.cost_of("modelo-local", 1_000, 1_000);
    assert!(c2.usd.is_none());
    assert!(c2.note.contains("NO VERIFICADO"));
    assert!(c2.source.is_none());

    // (c) el presupuesto es por HORA, no por acción
    let mut b = HourBudget::new(1_000, None, 5);
    let neutro = Cost {
        usd: None,
        source: None,
        note: String::new(),
    };
    assert!(b.allows(100, 600, &neutro).is_ok());
    b.spend(100, 600, &neutro);
    assert!(b.allows(100, 600, &neutro).is_ok(), "cabe dentro de la hora");
    b.spend(100, 600, &neutro);
    assert!(b.allows(100, 1, &neutro).is_err(), "agotado hasta la próxima hora");
    assert!(b.allows(100 + 3_600, 1, &neutro).is_ok(), "la ventana se reinicia");

    // (d) el bucle para cuando no queda presupuesto de hora
    ag.cfg.budget = HourBudget::new(10, None, 5);
    let mut sim = Sim {
        replies: vec![Ok("{\"action\":\"speak\",\"text\":\"hola\"}".into())],
        seen: Vec::new(),
    };
    let mut ex = Exec {
        calls: Vec::new(),
        fail: None,
    };
    let mut rtr = Rtr {
        log: Vec::new(),
        task: None,
    };
    let r = ag.tick(1_700_000_000, "tarea", &mut sim, &mut ex, &mut rtr);
    assert!(r.calls == 0, "no debe llamar al LLM sin presupuesto");
    assert!(r.stopped_reason.is_some());
}

/// Criterio 3 — degradación honesta.
#[test]
fn c3_si_el_modelo_devuelve_basura_se_aplica_un_criterio_fijo() {
    let path = tmp("c3");
    let mut ag = loop_agente(path);
    let mut sim = Sim {
        replies: vec![Ok("lorem ipsum sin formato ni json".into())],
        seen: Vec::new(),
    };
    let mut ex = Exec {
        calls: Vec::new(),
        fail: None,
    };
    let mut rtr = Rtr {
        log: Vec::new(),
        task: Some("t-42".into()),
    };
    let r = ag.tick(1_700_000_000, "tarea", &mut sim, &mut ex, &mut rtr);
    assert!(r.degraded, "debe marcar la degradación");
    match &r.final_action {
        Action::Tool { name, .. } => assert_eq!(name, "claim_task"),
        other => panic!("nunca inventar acciones; obtuve {:?}", other),
    }
    // Y sin tarea libre, el criterio fijo es "callar" (no inventar).
    let mut ag2 = loop_agente(tmp("c3b"));
    let mut sim2 = Sim {
        replies: vec![Ok("???".into())],
        seen: Vec::new(),
    };
    let mut ex2 = Exec {
        calls: Vec::new(),
        fail: None,
    };
    let mut rtr2 = Rtr {
        log: Vec::new(),
        task: None,
    };
    let r2 = ag2.tick(1_700_000_000, "tarea", &mut sim2, &mut ex2, &mut rtr2);
    assert!(matches!(r2.final_action, Action::Idle { .. }));
}

/// Criterio 4 — el prompt no lleva credenciales.
#[test]
fn c4_el_prompt_no_lleva_la_api_key_ni_el_token() {
    let path = tmp("c4");
    let ag = loop_agente(path);
    let prompt = ag.build_prompt("tarea sensible");
    for prohibido in [
        "X-Seere-Auth-Token",
        "X-API-Key",
        "mdaemon_sk",
        "MEGADAEMON_API_KEY",
        "MEGADAEMON_COCKPIT_TOKEN",
    ] {
        assert!(
            !prompt.contains(prohibido),
            "el prompt no debe contener {:?}",
            prohibido
        );
    }
    assert!(prompt.contains("tarea sensible"));
}

/// Criterio 5 — meta-agente: enruta al nodo/servicio correcto.
#[test]
fn c5_el_meta_agente_enruta_al_servicio_correcto() {
    let path = tmp("c5");
    let mut ag = loop_agente(path);
    let mut sim = Sim {
        replies: vec![Ok(
            "{\"action\":\"delegate\",\"provider\":\"remoto\",\"task_type\":\"vision\"}".into(),
        )],
        seen: Vec::new(),
    };
    let mut ex = Exec {
        calls: Vec::new(),
        fail: None,
    };
    let mut rtr = Rtr {
        log: Vec::new(),
        task: None,
    };
    let r = ag.tick(1_700_000_000, "analiza la pantalla", &mut sim, &mut ex, &mut rtr);
    assert!(
        r.actions.iter().any(|a| a.contains("enrutado a nodo:remoto")),
        "debe enrutar: {:?}",
        r.actions
    );
    // El parseo de la decisión es determinista.
    assert_eq!(
        parse_decision("{\"action\":\"delegate\",\"provider\":\"x\",\"task_type\":\"vision\"}"),
        Some(Action::Delegate {
            provider: "x".to_string(),
            task_type: "vision".to_string()
        })
    );
    assert_eq!(parse_decision("basura sin formato"), None);
}

/// Criterio 6 — circuit breaker de 3 estados.
#[test]
fn c6_circuit_breaker_tres_estados_backoff_y_recuperacion_medida() {
    let mut br = CircuitBreaker::new("proveedor");
    br.base_backoff_secs = 10;
    br.failure_threshold = 2;
    assert_eq!(br.state(), BreakerState::Closed);
    br.on_failure(1_000);
    assert_eq!(br.state(), BreakerState::Closed);
    br.on_failure(1_010);
    assert_eq!(br.state(), BreakerState::Open);
    assert!(!br.allows(1_015), "respeta el backoff");
    assert!(br.allows(1_020), "pasado el backoff => semiabierto");
    assert_eq!(br.state(), BreakerState::HalfOpen);
    br.on_success(1_033);
    assert_eq!(br.state(), BreakerState::Closed);
    // La ventana de recuperación es MEDIDA (13 s del open al close).
    assert_eq!(br.measured_recovery_secs(), Some(13));
    assert_eq!(br.mean_recovery_secs(), Some(13.0));
}

/// Requisito del móvil: tope duro de llamadas y de procesos por tick.
#[test]
fn el_tope_por_tick_protege_al_movil() {
    let path = tmp("tope");
    let mut ag = loop_agente(path);
    ag.cfg.max_calls_per_tick = 2;
    ag.cfg.max_shell_spawns_per_tick = 2; // 1 tool = 2 procesos => 1 tool por tick
    let mut sim = Sim {
        replies: vec![
            Err("caido".into()),
            Err("caido".into()),
            Err("caido".into()),
            Err("caido".into()),
        ],
        seen: Vec::new(),
    };
    let mut ex = Exec {
        calls: Vec::new(),
        fail: None,
    };
    let mut rtr = Rtr {
        log: Vec::new(),
        task: None,
    };
    let r = ag.tick(1_700_000_000, "tarea", &mut sim, &mut ex, &mut rtr);
    assert!(r.calls <= 2, "llamadas <= tope: {}", r.calls);
    // Y el ejecutor no puede gastar más procesos de shell que el tope.
    assert!(ex.calls.len() <= 1, "procesos <= tope: {:?}", ex.calls);
}

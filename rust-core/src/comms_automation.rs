//! MegaDaemon v16 — AUTOMATIZACIÓN DE COMUNICACIONES.
//!
//! Auto-respuestas, reenvíos, registro de llamadas y bitácora de todo lo que
//! pasa. Este módulo es la parte "de rottenura" de la automatización: el
//! usuario decide qué se hace y este código lo hace **de verdad**, sin
//! dry-run, sin bloqueos implícitos y sinermission para escribir mensajes.
//!
//! ## Autorización
//!
//! La autorización vive en un fichero de consentimiento
//! (`comms_consent.json`) que el usuario edita. Por defecto **todo está
//! concedido**: `auto_reply`, `forwarding`, `outgoing_allowed` y
//! `allow_record_calls` a `true`, y `dry_run` a `false` (o sea: se ejecuta).
//! Es el usuario quien lo escribe, no el programa.
//!
//! ## Registro (lo que sí es innegociable)
//!
//! Cada decisión produce una entrada en el journal (`comms_journal.ndjson`)
//! con: quién, qué, cuándo y con qué texto. Sin journal no hay auditoría, y
//! una automatización que manda mensajes en nombre de alguien sin rastro es
//! indistinguible de una application's de spam.

use serde_json::{json, Value};

/// Tipos de evento de comunicación.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommsEventKind {
    SmsReceived,
    SmsSent,
    CallIncoming,
    CallMissed,
    CallEnded,
}

impl CommsEventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommsEventKind::SmsReceived => "sms_received",
            CommsEventKind::SmsSent => "sms_sent",
            CommsEventKind::CallIncoming => "call_incoming",
            CommsEventKind::CallMissed => "call_missed",
            CommsEventKind::CallEnded => "call_ended",
        }
    }
    pub fn from_str(s: &str) -> Option<CommsEventKind> {
        Some(match s {
            "sms_received" => CommsEventKind::SmsReceived,
            "sms_sent" => CommsEventKind::SmsSent,
            "call_incoming" => CommsEventKind::CallIncoming,
            "call_missed" => CommsEventKind::CallMissed,
            "call_ended" => CommsEventKind::CallEnded,
            _ => return None,
        })
    }
    /// ¿Es un evento de llamada (y por tanto grabable)?
    pub fn is_call(&self) -> bool {
        matches!(
            self,
            CommsEventKind::CallIncoming | CommsEventKind::CallMissed | CommsEventKind::CallEnded
        )
    }
}

/// Evento de comunicación.
#[derive(Clone, Debug)]
pub struct CommsEvent {
    pub kind: CommsEventKind,
    pub ts: i64,
    /// Número (SMS) o identificador de la llamada.
    pub peer: String,
    /// Cuerpo del SMS o nota.
    pub body: Option<String>,
    pub thread_id: Option<String>,
    /// Duración de la llamada en segundos (si la hubo).
    pub duration_secs: Option<u32>,
}

impl CommsEvent {
    pub fn new(kind: CommsEventKind, ts: i64, peer: &str) -> CommsEvent {
        CommsEvent {
            kind,
            ts,
            peer: peer.to_string(),
            body: None,
            thread_id: None,
            duration_secs: None,
        }
    }
    pub fn with_body(mut self, b: &str) -> CommsEvent {
        self.body = Some(b.to_string());
        self
    }
    pub fn with_duration(mut self, d: u32) -> CommsEvent {
        self.duration_secs = Some(d);
        self
    }
}

/// Consentimiento del usuario: lo que está autorizado a hacer el sistema.
///
/// Por defecto **todo concedido** (el usuario es quien edita este fichero).
#[derive(Clone, Debug, PartialEq)]
pub struct Consent {
    /// Motor de automatizaciones de comunicación activo.
    pub enabled: bool,
    /// Responder automáticamente a los SMS que llegan.
    pub auto_reply: bool,
    /// Reenviar lo que llega a otro destino.
    pub forwarding: bool,
    /// Permitir mensajes salientes en nombre del usuario.
    pub outgoing_allowed: bool,
    /// Permitir la grabación de llamadas.
    pub allow_record_calls: bool,
    /// Modo simulación: si es `true` no se envía nada (sólo se registra).
    pub dry_run: bool,
    /// Tope de respuestas por hora (0 = sin tope).
    pub max_replies_per_hour: u32,
    /// Segundos mínimos entre dos respuestas al mismo interlocutor (0 = sin tope).
    pub cooldown_secs: u64,
    /// Interlocutores permitidos (vacío = todos).
    pub allowed_peers: Vec<String>,
    /// Interlocutores vetados por el usuario.
    pub blocked_peers: Vec<String>,
    /// Plantilla de respuesta automática.
    pub reply_template: String,
    /// A quién se reenvía (vacío = no hay reenvío).
    pub forward_to: Option<String>,
    /// Número desde el que se responde (si el sistema lo necesita).
    pub from_number: Option<String>,
}

impl Default for Consent {
    fn default() -> Consent {
        Consent {
            enabled: true,
            auto_reply: true,
            forwarding: true,
            outgoing_allowed: true,
            allow_record_calls: true,
            dry_run: false,
            max_replies_per_hour: 0,
            cooldown_secs: 0,
            allowed_peers: Vec::new(),
            blocked_peers: Vec::new(),
            reply_template: "Recibido, lo leo en cuanto pueda.".to_string(),
            forward_to: None,
            from_number: None,
        }
    }
}

/// Lo que el motor decide hacer con un evento.
#[derive(Clone, Debug, PartialEq)]
pub enum CommsDecision {
    /// Responder automáticamente.
    Reply { to: String, body: String },
    /// Reenviar a un tercer destino.
    Forward { to: String, body: String },
    /// Grabar la llamada.
    Record { peer: String, started_at: i64 },
    /// Anotar en la bitácora.
    Log { note: String },
    /// No hay nada que hacer (o no se puede: falta el interlocutor…).
    Ignore { reason: String },
}

impl CommsDecision {
    pub fn kind(&self) -> &'static str {
        match self {
            CommsDecision::Reply { .. } => "reply",
            CommsDecision::Forward { .. } => "forward",
            CommsDecision::Record { .. } => "record",
            CommsDecision::Log { .. } => "log",
            CommsDecision::Ignore { .. } => "ignore",
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            CommsDecision::Reply { to, body } => json!({"kind":"reply","to":to,"body":body}),
            CommsDecision::Forward { to, body } => json!({"kind":"forward","to":to,"body":body}),
            CommsDecision::Record { peer, started_at } => {
                json!({"kind":"record","peer":peer,"started_at":started_at})
            }
            CommsDecision::Log { note } => json!({"kind":"log","note":note}),
            CommsDecision::Ignore { reason } => json!({"kind":"ignore","reason":reason}),
        }
    }
}

/// Historial mínimo: qué se ha contestado y cuándo.
#[derive(Clone, Debug, Default)]
pub struct History {
    /// Epochs de las respuestas enviadas (para el tope por hora).
    pub replies: Vec<i64>,
    /// Último mensaje enviado a cada interlocutor.
    pub last_reply: Vec<(String, i64)>,
}

impl History {
    /// Nº de respuestas en la hora que acaba de empezar (`now`).
    pub fn replies_in_last_hour(&self, now: i64) -> u32 {
        self.replies.iter().filter(|t| now - **t < 3600).count() as u32
    }

    pub fn last_reply_to(&self, peer: &str) -> Option<i64> {
        self.last_reply
            .iter()
            .find(|(p, _)| p == peer)
            .map(|(_, t)| *t)
    }
}

/// ¿Se permite actuar con este interlocutor?
///
/// Sólo manda lo que el usuario configured: `blocked_peers` (su lista negra)
/// y `allowed_peers` (su lista blanca; vacía = todos).
pub fn peer_allowed(consent: &Consent, peer: &str) -> bool {
    if consent.blocked_peers.iter().any(|p| p == peer) {
        return false;
    }
    if consent.allowed_peers.is_empty() {
        return true;
    }
    consent.allowed_peers.iter().any(|p| p == peer)
}

/// Decide qué hacer con un evento.
pub fn decide(ev: &CommsEvent, consent: &Consent, history: &History, now: i64) -> Vec<CommsDecision> {
    let mut out: Vec<CommsDecision> = Vec::new();
    if !consent.enabled {
        out.push(CommsDecision::Ignore {
            reason: "automatización de comunicaciones desactivada".into(),
        });
        return out;
    }
    if !peer_allowed(consent, &ev.peer) {
        out.push(CommsDecision::Ignore {
            reason: format!("interlocutor '{}' no permitido por el usuario", ev.peer),
        });
        return out;
    }

    match ev.kind {
        CommsEventKind::SmsReceived => {
            if consent.auto_reply && consent.outgoing_allowed {
                let over_hour = consent.max_replies_per_hour > 0
                    && history.replies_in_last_hour(now) >= consent.max_replies_per_hour;
                let in_cooldown = consent.cooldown_secs > 0
                    && history
                        .last_reply_to(&ev.peer)
                        .map(|t| now - t < consent.cooldown_secs as i64)
                        .unwrap_or(false);
                if over_hour {
                    out.push(CommsDecision::Ignore {
                        reason: format!(
                            "tope de {} respuestas/hora alcanzado",
                            consent.max_replies_per_hour
                        ),
                    });
                } else if in_cooldown {
                    out.push(CommsDecision::Ignore {
                        reason: format!("en enfriamiento ({} s)", consent.cooldown_secs),
                    });
                } else {
                    let body = consent.reply_template.clone();
                    out.push(CommsDecision::Reply {
                        to: ev.peer.clone(),
                        body,
                    });
                }
            }
            if consent.forwarding {
                if let Some(to) = &consent.forward_to {
                    if !to.is_empty() && *to != ev.peer {
                        out.push(CommsDecision::Forward {
                            to: to.clone(),
                            body: ev.body.clone().unwrap_or_default(),
                        });
                    }
                }
            }
        }
        CommsEventKind::CallIncoming | CommsEventKind::CallEnded => {
            if consent.allow_record_calls {
                out.push(CommsDecision::Record {
                    peer: ev.peer.clone(),
                    started_at: ev.ts,
                });
            }
        }
        CommsEventKind::CallMissed => {
            out.push(CommsDecision::Log {
                note: format!("llamada perdida de {}", ev.peer),
            });
        }
        CommsEventKind::SmsSent => {
            out.push(CommsDecision::Log {
                note: format!("SMS enviado a {}", ev.peer),
            });
        }
    }

    // Siempre queda rastro: el registro es parte del contrato.
    out.push(CommsDecision::Log {
        note: format!("evento {} de {}", ev.kind.as_str(), ev.peer),
    });
    out
}

// ========================================================================= //
// Consentimiento: JSON y persistencia
// ========================================================================= //

pub fn consent_to_json(c: &Consent) -> Value {
    json!({
        "version": 1,
        "enabled": c.enabled,
        "auto_reply": c.auto_reply,
        "forwarding": c.forwarding,
        "outgoing_allowed": c.outgoing_allowed,
        "allow_record_calls": c.allow_record_calls,
        "dry_run": c.dry_run,
        "max_replies_per_hour": c.max_replies_per_hour,
        "cooldown_secs": c.cooldown_secs,
        "allowed_peers": c.allowed_peers,
        "blocked_peers": c.blocked_peers,
        "reply_template": c.reply_template,
        "forward_to": c.forward_to,
        "from_number": c.from_number,
    })
}

pub fn consent_from_json(v: &Value) -> Consent {
    let d = Consent::default();
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(|x| x.to_string());
    let b = |k: &str, def: bool| v.get(k).and_then(|x| x.as_bool()).unwrap_or(def);
    let list = |k: &str| -> Vec<String> {
        v.get(k)
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };
    Consent {
        enabled: b("enabled", d.enabled),
        auto_reply: b("auto_reply", d.auto_reply),
        forwarding: b("forwarding", d.forwarding),
        outgoing_allowed: b("outgoing_allowed", d.outgoing_allowed),
        allow_record_calls: b("allow_record_calls", d.allow_record_calls),
        dry_run: b("dry_run", d.dry_run),
        max_replies_per_hour: v
            .get("max_replies_per_hour")
            .and_then(|x| x.as_u64())
            .unwrap_or(d.max_replies_per_hour as u64) as u32,
        cooldown_secs: v.get("cooldown_secs").and_then(|x| x.as_u64()).unwrap_or(d.cooldown_secs),
        allowed_peers: list("allowed_peers"),
        blocked_peers: list("blocked_peers"),
        reply_template: s("reply_template").unwrap_or(d.reply_template),
        forward_to: s("forward_to"),
        from_number: s("from_number"),
    }
}

/// Carga el consentimiento (si no existe, el de por defecto: todo concedido).
pub fn load_consent(path: &str) -> Consent {
    let Ok(txt) = std::fs::read_to_string(path) else {
        return Consent::default();
    };
    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
        return Consent::default();
    };
    consent_from_json(&v)
}

/// Guarda el consentimiento.
pub fn save_consent(path: &str, c: &Consent) -> Result<(), String> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(path, serde_json::to_string_pretty(&consent_to_json(c)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Entrada de bitácora de un evento con sus decisiones.
pub fn journal_entry(ev: &CommsEvent, decisions: &[CommsDecision], now: i64) -> Value {
    json!({
        "ts": now,
        "event_ts": ev.ts,
        "event": ev.kind.as_str(),
        "peer": ev.peer,
        "body": ev.body,
        "duration_secs": ev.duration_secs,
        "decisions": decisions.iter().map(|d| d.to_json()).collect::<Vec<_>>(),
    })
}

/// Añade una entrada al journal (JSON Lines).
pub fn append_journal(path: &str, entry: &Value) -> Result<(), String> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    writeln!(f, "{}", serde_json::to_string(entry).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Lee las últimas `n` entradas del journal.
pub fn journal_tail(path: &str, n: usize) -> Vec<Value> {
    let Ok(txt) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let lines: Vec<&str> = txt.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines[start..]
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn por_defecto_todo_esta_autorizado() {
        let c = Consent::default();
        assert!(c.enabled);
        assert!(c.auto_reply);
        assert!(c.forwarding);
        assert!(c.outgoing_allowed);
        assert!(c.allow_record_calls);
        assert!(!c.dry_run);
    }

    #[test]
    fn un_sms_dispara_respuesta() {
        let ev = CommsEvent::new(CommsEventKind::SmsReceived, 100, "+34600111222").with_body("hola");
        let d = decide(&ev, &Consent::default(), &History::default(), 100);
        assert!(d.iter().any(|x| matches!(x, CommsDecision::Reply { .. })));
        assert!(d.iter().any(|x| matches!(x, CommsDecision::Log { .. })));
    }

    #[test]
    fn el_reenvio_sucede_si_hay_destino() {
        let mut c = Consent::default();
        c.forward_to = Some("+34600999888".into());
        let ev = CommsEvent::new(CommsEventKind::SmsReceived, 100, "+34600111222").with_body("urgente");
        let d = decide(&ev, &c, &History::default(), 100);
        let f = d
            .iter()
            .find_map(|x| match x {
                CommsDecision::Forward { to, body } => Some((to.clone(), body.clone())),
                _ => None,
            })
            .expect("debe reenviar");
        assert_eq!(f.0, "+34600999888");
        assert_eq!(f.1, "urgente");
    }

    #[test]
    fn no_se_reenvia_a_si_mismo() {
        let mut c = Consent::default();
        c.forward_to = Some("+34600111222".into());
        let ev = CommsEvent::new(CommsEventKind::SmsReceived, 100, "+34600111222");
        let d = decide(&ev, &c, &History::default(), 100);
        assert!(!d.iter().any(|x| matches!(x, CommsDecision::Forward { .. })));
    }

    #[test]
    fn una_llamada_se_graba_si_esta_autorizado() {
        let ev = CommsEvent::new(CommsEventKind::CallIncoming, 100, "+34600111222");
        let d = decide(&ev, &Consent::default(), &History::default(), 100);
        assert!(d.iter().any(|x| matches!(x, CommsDecision::Record { .. })));
    }

    #[test]
    fn si_el_usuario_lo_desactiva_no_se_graba() {
        let mut c = Consent::default();
        c.allow_record_calls = false;
        let ev = CommsEvent::new(CommsEventKind::CallIncoming, 100, "+34600111222");
        let d = decide(&ev, &c, &History::default(), 100);
        assert!(!d.iter().any(|x| matches!(x, CommsDecision::Record { .. })));
    }

    #[test]
    fn la_lista_negra_del_usuario_se_respeta() {
        let mut c = Consent::default();
        c.blocked_peers = vec!["+34600111222".into()];
        let ev = CommsEvent::new(CommsEventKind::SmsReceived, 100, "+34600111222");
        let d = decide(&ev, &c, &History::default(), 100);
        assert!(!d.iter().any(|x| matches!(x, CommsDecision::Reply { .. })));
        assert!(d.iter().any(|x| matches!(x, CommsDecision::Ignore { .. })));
    }

    #[test]
    fn la_lista_blanca_limita() {
        let mut c = Consent::default();
        c.allowed_peers = vec!["+34600555555".into()];
        let ev = CommsEvent::new(CommsEventKind::SmsReceived, 100, "+34600111222");
        let d = decide(&ev, &c, &History::default(), 100);
        assert!(!d.iter().any(|x| matches!(x, CommsDecision::Reply { .. })));
        let ok = CommsEvent::new(CommsEventKind::SmsReceived, 100, "+34600555555");
        let d2 = decide(&ok, &c, &History::default(), 100);
        assert!(d2.iter().any(|x| matches!(x, CommsDecision::Reply { .. })));
    }

    #[test]
    fn el_tope_por_hora_se_cumple() {
        let mut c = Consent::default();
        c.max_replies_per_hour = 2;
        let h = History {
            replies: vec![100, 200],
            last_reply: vec![],
        };
        let ev = CommsEvent::new(CommsEventKind::SmsReceived, 300, "+34600111222");
        let d = decide(&ev, &c, &h, 300);
        assert!(!d.iter().any(|x| matches!(x, CommsDecision::Reply { .. })));
        assert!(d.iter().any(|x| matches!(x, CommsDecision::Ignore { .. })));
    }

    #[test]
    fn el_enfriamiento_por_interlocutor_se_cumple() {
        let mut c = Consent::default();
        c.cooldown_secs = 600;
        let h = History {
            replies: vec![100],
            last_reply: vec![("+34600111222".into(), 100)],
        };
        let ev = CommsEvent::new(CommsEventKind::SmsReceived, 300, "+34600111222");
        let d = decide(&ev, &c, &h, 300);
        assert!(!d.iter().any(|x| matches!(x, CommsDecision::Reply { .. })));
    }

    #[test]
    fn el_motor_apagado_no_hace_nada() {
        let mut c = Consent::default();
        c.enabled = false;
        let ev = CommsEvent::new(CommsEventKind::SmsReceived, 100, "+34600111222");
        let d = decide(&ev, &c, &History::default(), 100);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], CommsDecision::Ignore { .. }));
    }

    #[test]
    fn el_consentimiento_va_y_vuelve_de_json() {
        let mut c = Consent::default();
        c.forward_to = Some("+34600999888".into());
        c.dry_run = true;
        c.blocked_peers = vec!["+34600111222".into()];
        let j = consent_to_json(&c);
        let back = consent_from_json(&j);
        assert_eq!(back, c);
    }

    #[test]
    fn el_consentimiento_se_guarda_y_se_carga() {
        let p = "/tmp/md_test_comms_consent.json";
        let mut c = Consent::default();
        c.reply_template = "hola".into();
        save_consent(p, &c).unwrap();
        assert_eq!(load_consent(p).reply_template, "hola");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn la_bitacora_guarda_y_se_lee() {
        let p = "/tmp/md_test_comms_journal.ndjson";
        let _ = std::fs::remove_file(p);
        let ev = CommsEvent::new(CommsEventKind::SmsReceived, 100, "+34600111222").with_body("hola");
        let d = decide(&ev, &Consent::default(), &History::default(), 100);
        let entry = journal_entry(&ev, &d, 100);
        append_journal(p, &entry).unwrap();
        let tail = journal_tail(p, 5);
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0]["peer"], json!("+34600111222"));
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn los_tipos_de_evento_se_identifican() {
        for k in [
            CommsEventKind::SmsReceived,
            CommsEventKind::SmsSent,
            CommsEventKind::CallIncoming,
            CommsEventKind::CallMissed,
            CommsEventKind::CallEnded,
        ] {
            assert_eq!(CommsEventKind::from_str(k.as_str()), Some(k));
        }
        assert!(CommsEventKind::CallIncoming.is_call());
        assert!(!CommsEventKind::SmsReceived.is_call());
    }
}

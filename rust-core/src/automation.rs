//! MegaDaemon v16 — MOTOR DE DISPARADORES Y REGLAS ("si X entonces Y").
//!
//! Este módulo es la pieza que le faltaba a MegaDaemon para competir de verdad
//! con Tasker/AutoTasker/MacroDroid: un motor de reglas **en Rust puro**, sin
//! depender de ninguna app de terceros instalada en el móvil.
//!
//! ## Modelo
//!
//! * Un **evento** describe algo que ha pasado en el dispositivo (SMS entrante,
//!   llamada perdida, carga de batería, app abierta, fichero nuevo, temperatura,
//!   red, SSID de WiFi, tick de planificador…).
//! * Una **regla** dice: "si ocurre este evento Y se cumplen estas condiciones,
//!   ejecuta estas acciones".
//! * Las **acciones** son las que el usuario espera de un automatizador:
//!   notificación, script, HTTP, escritura de fichero, cambio de ajuste de
//!   sistema, comando root o emisión de un broadcast.
//!
//! ## Por qué este módulo NO habla con el dispositivo
//!
//! Todo el módulo es **puro**: no usa `crate::` ni hace E/S de dispositivo.
//! El acceso real al móvil vive en `crate::automation_runtime`, que implementa
//! el trait [`Effector`]. Ventajas:
//!
//! * las reglas se pueden probar en el PC sin móvil (tests deterministas);
//! * el motor no puede quedar colgado por un comando lento: el `Effector` es
//!   quien impone el tiempo límite;
//! * la misma lógica sirve para el CLI, para la herramienta MCP y para el
//!   planificador interno, sin tres implementaciones distintas.
//!
//! ## Salvaguardas
//!
//! * Cada regla tiene un `cooldown_secs`: un evento repetido (p. ej. la
//!   temperatura sube un grado) no dispara la acción 300 veces por minuto.
//! * Cada regla tiene `max_fires`: tope duro de disparos.
//! * `allow_root` es explícito por regla: las acciones `RootCommand` sólo se
//!   ejecutan si la regla lo permite (por defecto sí, pero **activar o
//!   modificar reglas exige confirmación** en la capa MCP).

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Tipos de evento que el motor sabe entender.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    SmsReceived,
    CallMissed,
    CallEnded,
    CallIncoming,
    BatteryChanged,
    ChargerChanged,
    AppOpened,
    AppClosed,
    FileCreated,
    TemperatureChanged,
    NetworkChanged,
    WifiChanged,
    Schedule,
    ScreenOn,
    ScreenOff,
    BootCompleted,
}

impl EventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventKind::SmsReceived => "sms_received",
            EventKind::CallMissed => "call_missed",
            EventKind::CallEnded => "call_ended",
            EventKind::CallIncoming => "call_incoming",
            EventKind::BatteryChanged => "battery_changed",
            EventKind::ChargerChanged => "charger_changed",
            EventKind::AppOpened => "app_opened",
            EventKind::AppClosed => "app_closed",
            EventKind::FileCreated => "file_created",
            EventKind::TemperatureChanged => "temperature_changed",
            EventKind::NetworkChanged => "network_changed",
            EventKind::WifiChanged => "wifi_changed",
            EventKind::Schedule => "schedule",
            EventKind::ScreenOn => "screen_on",
            EventKind::ScreenOff => "screen_off",
            EventKind::BootCompleted => "boot_completed",
        }
    }

    pub fn from_str(s: &str) -> Option<EventKind> {
        Some(match s {
            "sms_received" => EventKind::SmsReceived,
            "call_missed" => EventKind::CallMissed,
            "call_ended" => EventKind::CallEnded,
            "call_incoming" => EventKind::CallIncoming,
            "battery_changed" => EventKind::BatteryChanged,
            "charger_changed" => EventKind::ChargerChanged,
            "app_opened" => EventKind::AppOpened,
            "app_closed" => EventKind::AppClosed,
            "file_created" => EventKind::FileCreated,
            "temperature_changed" => EventKind::TemperatureChanged,
            "network_changed" => EventKind::NetworkChanged,
            "wifi_changed" => EventKind::WifiChanged,
            "schedule" => EventKind::Schedule,
            "screen_on" => EventKind::ScreenOn,
            "screen_off" => EventKind::ScreenOff,
            "boot_completed" => EventKind::BootCompleted,
            _ => return None,
        })
    }

    /// Todos los tipos, para la documentación de la herramienta MCP.
    pub fn all() -> &'static [EventKind] {
        &[
            EventKind::SmsReceived,
            EventKind::CallMissed,
            EventKind::CallEnded,
            EventKind::CallIncoming,
            EventKind::BatteryChanged,
            EventKind::ChargerChanged,
            EventKind::AppOpened,
            EventKind::AppClosed,
            EventKind::FileCreated,
            EventKind::TemperatureChanged,
            EventKind::NetworkChanged,
            EventKind::WifiChanged,
            EventKind::Schedule,
            EventKind::ScreenOn,
            EventKind::ScreenOff,
            EventKind::BootCompleted,
        ]
    }
}

/// Un evento del sistema: "ha pasado esto".
#[derive(Clone, Debug)]
pub struct Event {
    pub kind: EventKind,
    /// Epoch en segundos.
    pub ts: i64,
    /// Clave principal: paquete de la app, número del remitente, ruta…
    pub key: Option<String>,
    /// Valor numérico: % de batería, temperatura en °C, nivel de red…
    pub value: Option<f64>,
    /// Texto libre: cuerpo del SMS, texto de la notificación…
    pub text: Option<String>,
    /// Campos extra (`ssid`, `network`, `path`, `peer`…).
    pub extra: BTreeMap<String, String>,
}

impl Event {
    pub fn new(kind: EventKind, ts: i64) -> Event {
        Event {
            kind,
            ts,
            key: None,
            value: None,
            text: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn with_key(mut self, k: &str) -> Event {
        self.key = Some(k.to_string());
        self
    }

    pub fn with_value(mut self, v: f64) -> Event {
        self.value = Some(v);
        self
    }

    pub fn with_text(mut self, t: &str) -> Event {
        self.text = Some(t.to_string());
        self
    }

    pub fn with_extra(mut self, k: &str, v: &str) -> Event {
        self.extra.insert(k.to_string(), v.to_string());
        self
    }
}

/// Campo sobre el que se evalúa una condición.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Kind,
    Package,
    Ssid,
    Path,
    Peer,
    Text,
    BatteryPct,
    TemperatureC,
    NetworkType,
    Value,
    Extra,
}

impl Field {
    pub fn as_str(&self) -> &'static str {
        match self {
            Field::Kind => "kind",
            Field::Package => "package",
            Field::Ssid => "ssid",
            Field::Path => "path",
            Field::Peer => "peer",
            Field::Text => "text",
            Field::BatteryPct => "battery_pct",
            Field::TemperatureC => "temperature_c",
            Field::NetworkType => "network",
            Field::Value => "value",
            Field::Extra => "extra",
        }
    }

    pub fn from_str(s: &str) -> Option<Field> {
        Some(match s {
            "kind" => Field::Kind,
            "package" => Field::Package,
            "ssid" => Field::Ssid,
            "path" => Field::Path,
            "peer" => Field::Peer,
            "text" => Field::Text,
            "battery_pct" => Field::BatteryPct,
            "temperature_c" => Field::TemperatureC,
            "network" => Field::NetworkType,
            "value" => Field::Value,
            "extra" => Field::Extra,
            _ => return None,
        })
    }
}

/// Operador de comparación.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
    Prefix,
    Suffix,
    In,
    Exists,
}

impl Op {
    pub fn as_str(&self) -> &'static str {
        match self {
            Op::Eq => "eq",
            Op::Ne => "ne",
            Op::Lt => "lt",
            Op::Le => "le",
            Op::Gt => "gt",
            Op::Ge => "ge",
            Op::Contains => "contains",
            Op::Prefix => "prefix",
            Op::Suffix => "suffix",
            Op::In => "in",
            Op::Exists => "exists",
        }
    }

    pub fn from_str(s: &str) -> Option<Op> {
        Some(match s {
            "eq" => Op::Eq,
            "ne" => Op::Ne,
            "lt" => Op::Lt,
            "le" => Op::Le,
            "gt" => Op::Gt,
            "ge" => Op::Ge,
            "contains" => Op::Contains,
            "prefix" => Op::Prefix,
            "suffix" => Op::Suffix,
            "in" => Op::In,
            "exists" => Op::Exists,
            _ => return None,
        })
    }
}

/// Condición AND sobre un campo del evento.
#[derive(Clone, Debug)]
pub struct Condition {
    pub field: Field,
    pub op: Op,
    pub value: String,
    /// Nombre del campo extra cuando `field == Extra`.
    pub extra_key: Option<String>,
}

impl Condition {
    pub fn new(field: Field, op: Op, value: &str) -> Condition {
        Condition {
            field,
            op,
            value: value.to_string(),
            extra_key: None,
        }
    }

    pub fn extra(key: &str, op: Op, value: &str) -> Condition {
        Condition {
            field: Field::Extra,
            op,
            value: value.to_string(),
            extra_key: Some(key.to_string()),
        }
    }
}

/// Qué dispara la regla.
#[derive(Clone, Debug)]
pub enum Trigger {
    /// Un evento concreto del sistema.
    Event(EventKind),
    /// Cada N segundos (independiente de los eventos).
    Interval { every_secs: u64 },
    /// Expresión cron (5 campos) — la delega en `crate::cron`.
    Cron { expr: String },
}

impl Trigger {
    pub fn as_str(&self) -> &'static str {
        match self {
            Trigger::Event(_) => "event",
            Trigger::Interval { .. } => "interval",
            Trigger::Cron { .. } => "cron",
        }
    }
}

/// Acción a ejecutar cuando la regla se cumple.
#[derive(Clone, Debug)]
pub enum Action {
    /// Notificación local.
    Notify { title: String, body: String },
    /// Script de shell.
    Script { cmd: String, timeout_ms: u64 },
    /// Petición HTTP.
    Http {
        method: String,
        url: String,
        body: Option<String>,
        headers: Vec<(String, String)>,
        timeout_ms: u64,
    },
    /// Escribir (o anexar) un fichero.
    WriteFile {
        path: String,
        content: String,
        append: bool,
    },
    /// Cambiar un ajuste de Android (`settings put <ns> <k> <v>`).
    Setting {
        namespace: String,
        key: String,
        value: String,
    },
    /// Comando root.
    RootCommand { cmd: String },
    /// Broadcast de Android (con extras).
    Broadcast {
        action: String,
        extras: Vec<(String, String)>,
    },
}

impl Action {
    /// Nombre corto de la acción (para el journal y el informe).
    pub fn kind(&self) -> &'static str {
        match self {
            Action::Notify { .. } => "notify",
            Action::Script { .. } => "script",
            Action::Http { .. } => "http",
            Action::WriteFile { .. } => "write_file",
            Action::Setting { .. } => "setting",
            Action::RootCommand { .. } => "root_command",
            Action::Broadcast { .. } => "broadcast",
        }
    }

    /// Texto legible para logs.
    pub fn describe(&self) -> String {
        match self {
            Action::Notify { title, body } => format!("notificar «{}»: {}", title, body),
            Action::Script { cmd, .. } => format!("script: {}", cmd),
            Action::Http { method, url, .. } => format!("http {} {}", method, url),
            Action::WriteFile { path, append, .. } => {
                format!("escribir {} ({})", path, if *append { "anexar" } else { "sobrescribir" })
            }
            Action::Setting {
                namespace,
                key,
                value,
            } => format!("settings {} {}={}", namespace, key, value),
            Action::RootCommand { cmd } => format!("root: {}", cmd),
            Action::Broadcast { action, .. } => format!("broadcast {}", action),
        }
    }
}

/// Una regla completa.
#[derive(Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub trigger: Trigger,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    /// Segundos mínimos entre dos disparos de la misma regla.
    pub cooldown_secs: u64,
    /// Tope duro de disparos (0 = sin tope).
    pub max_fires: u32,
    /// Si una acción falla, no seguir con las demás.
    pub stop_on_error: bool,
    /// Permite acciones `RootCommand` en esta regla.
    pub allow_root: bool,
}

impl Default for Rule {
    fn default() -> Rule {
        Rule {
            id: String::new(),
            name: String::new(),
            enabled: true,
            trigger: Trigger::Event(EventKind::Schedule),
            conditions: Vec::new(),
            actions: Vec::new(),
            cooldown_secs: 60,
            max_fires: 0,
            stop_on_error: false,
            allow_root: true,
        }
    }
}

/// Resultado de ejecutar una acción.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub rule_id: String,
    pub rule_name: String,
    pub event_kind: String,
    pub action_index: usize,
    pub action: String,
    pub ok: bool,
    pub detail: String,
    pub ts: i64,
}

impl Outcome {
    pub fn to_json(&self) -> Value {
        json!({
            "ts": self.ts,
            "rule_id": self.rule_id,
            "rule_name": self.rule_name,
            "event": self.event_kind,
            "action_index": self.action_index,
            "action": self.action,
            "ok": self.ok,
            "detail": self.detail,
        })
    }
}

/// Quien ejecuta de verdad las acciones (el móvil, un test, un simulador…).
pub trait Effector {
    fn perform(&mut self, action: &Action) -> Result<String, String>;
}

/// Effector que no hace nada pero registra lo que haría (modo simulación).
pub struct NullEffector {
    pub performed: Vec<String>,
}

impl Default for NullEffector {
    fn default() -> Self {
        NullEffector { performed: Vec::new() }
    }
}

impl Effector for NullEffector {
    fn perform(&mut self, action: &Action) -> Result<String, String> {
        let d = action.describe();
        self.performed.push(d.clone());
        Ok(format!("simulado: {}", d))
    }
}

// ========================================================================= //
// Evaluar
// ========================================================================= //

/// Valor de un campo para un evento (numérico o textual).
fn field_value<'a>(ev: &'a Event, c: &Condition) -> Option<&'a str> {
    match c.field {
        Field::Kind => Some(ev.kind.as_str()),
        Field::Package => ev.key.as_deref(),
        Field::Ssid => ev.extra.get("ssid").map(|s| s.as_str()),
        Field::Path => ev.extra.get("path").map(|s| s.as_str()),
        Field::Peer => ev
            .extra
            .get("peer")
            .map(|s| s.as_str())
            .or(ev.key.as_deref()),
        Field::Text => ev.text.as_deref(),
        Field::BatteryPct | Field::TemperatureC | Field::Value => ev.value.map(|_| ""),
        Field::NetworkType => ev.extra.get("network").map(|s| s.as_str()),
        Field::Extra => c
            .extra_key
            .as_ref()
            .and_then(|k| ev.extra.get(k))
            .map(|s| s.as_str()),
    }
}

/// Compara dos valores: numéricamente si ambos son números, si no textual.
fn compare(a: Option<&str>, num_a: Option<f64>, op: Op, want: &str) -> bool {
    match op {
        Op::Exists => return a.is_some() || num_a.is_some(),
        _ => {}
    }
    // Camino numérico (batería, temperatura, valor…).
    if let Some(na) = num_a {
        if let Ok(nb) = want.trim().parse::<f64>() {
            return match op {
                Op::Eq => (na - nb).abs() < f64::EPSILON,
                Op::Ne => (na - nb).abs() >= f64::EPSILON,
                Op::Lt => na < nb,
                Op::Le => na <= nb,
                Op::Gt => na > nb,
                Op::Ge => na >= nb,
                _ => false,
            };
        }
        // Un `contains` sobre número no tiene sentido: se compara el texto.
        let s = want.trim();
        return match op {
            Op::Eq => s == format!("{}", na),
            Op::Ne => s != format!("{}", na),
            Op::Contains => format!("{}", na).contains(s),
            Op::Prefix => format!("{}", na).starts_with(s),
            Op::Suffix => format!("{}", na).ends_with(s),
            Op::In => want.split(',').any(|w| w.trim() == format!("{}", na)),
            _ => false,
        };
    }
    let Some(a) = a else {
        // Sin valor: sólo `ne` y `exists` tienen sentido.
        return matches!(op, Op::Ne);
    };
    match op {
        Op::Eq => a == want,
        Op::Ne => a != want,
        Op::Lt | Op::Le | Op::Gt | Op::Ge => {
            let (x, y) = (a.parse::<f64>(), want.trim().parse::<f64>());
            match (x, y) {
                (Ok(x), Ok(y)) => match op {
                    Op::Lt => x < y,
                    Op::Le => x <= y,
                    Op::Gt => x > y,
                    Op::Ge => x >= y,
                    _ => false,
                },
                _ => false,
            }
        }
        Op::Contains => a.contains(want),
        Op::Prefix => a.starts_with(want),
        Op::Suffix => a.ends_with(want),
        Op::In => want.split(',').any(|w| w.trim() == a),
        Op::Exists => true,
    }
}

/// ¿Se cumple la condición?
pub fn condition_ok(ev: &Event, c: &Condition) -> bool {
    let num = match c.field {
        Field::BatteryPct | Field::TemperatureC | Field::Value => ev.value,
        _ => None,
    };
    compare(field_value(ev, c), num, c.op, &c.value)
}

/// ¿El evento dispara la regla? (tipo de evento + condiciones)
pub fn matches(rule: &Rule, ev: &Event) -> bool {
    let trig_ok = match &rule.trigger {
        Trigger::Event(k) => *k == ev.kind,
        Trigger::Interval { .. } | Trigger::Cron { .. } => ev.kind == EventKind::Schedule,
    };
    if !trig_ok {
        return false;
    }
    rule.conditions.iter().all(|c| condition_ok(ev, c))
}

/// ¿Toca disparar? (regla + cooldown + tope de disparos)
pub fn due(rule: &Rule, ev: &Event, now: i64, last_fire: Option<i64>, fires: u32) -> bool {
    if !rule.enabled {
        return false;
    }
    if rule.max_fires > 0 && fires >= rule.max_fires {
        return false;
    }
    if rule.cooldown_secs > 0 {
        if let Some(lf) = last_fire {
            if now.saturating_sub(lf) < rule.cooldown_secs as i64 {
                return false;
            }
        }
    }
    // Un evento con marca de tiempo posterior al "ahora" no dispara.
    if ev.ts > now {
        return false;
    }
    matches(rule, ev)
}

/// Ejecuta las acciones de la regla que se han disparado.
///
/// Devuelve un `Outcome` por acción (incluso si falla) para que el journal
/// sea completo: un fallo silencioso es la peor forma de automatizar.
pub fn run_rule(rule: &Rule, ev: &Event, eff: &mut dyn Effector, now: i64) -> Vec<Outcome> {
    // v16: el FILTRO del evento es parte de la regla: si el evento no casa
    // (ni el disparador ni las condiciones) la regla NO se ejecuta.  Sin
    // esta comprobacion, `run_rule` hacia lo mismo que `matches` pero al
    // reves: disparaba reglas cuyo evento no correspondia.
    if !matches(rule, ev) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, act) in rule.actions.iter().enumerate() {
        if matches!(act, Action::RootCommand { .. }) && !rule.allow_root {
            out.push(Outcome {
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                event_kind: ev.kind.as_str().to_string(),
                action_index: i,
                action: act.kind().to_string(),
                ok: false,
                detail: "acción root no permitida por la regla (allow_root=false)".to_string(),
                ts: now,
            });
            if rule.stop_on_error {
                break;
            }
            continue;
        }
        let res = eff.perform(act);
        let (ok, detail) = match res {
            Ok(d) => (true, d),
            Err(e) => (false, e),
        };
        out.push(Outcome {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            event_kind: ev.kind.as_str().to_string(),
            action_index: i,
            action: act.kind().to_string(),
            ok,
            detail,
            ts: now,
        });
        if !ok && rule.stop_on_error {
            break;
        }
    }
    out
}

// ========================================================================= //
// JSON
// ========================================================================= //

fn action_to_json(a: &Action) -> Value {
    match a {
        Action::Notify { title, body } => json!({"type":"notify","title":title,"body":body}),
        Action::Script { cmd, timeout_ms } => json!({"type":"script","cmd":cmd,"timeout_ms":timeout_ms}),
        Action::Http { method, url, body, headers, timeout_ms } => json!({
            "type":"http","method":method,"url":url,"body":body,
            "headers": headers.iter().map(|(k,v)| json!([k,v])).collect::<Vec<_>>(),
            "timeout_ms":timeout_ms
        }),
        Action::WriteFile { path, content, append } => {
            json!({"type":"write_file","path":path,"content":content,"append":append})
        }
        Action::Setting { namespace, key, value } => {
            json!({"type":"setting","namespace":namespace,"key":key,"value":value})
        }
        Action::RootCommand { cmd } => json!({"type":"root_command","cmd":cmd}),
        Action::Broadcast { action, extras } => json!({
            "type":"broadcast","action":action,
            "extras": extras.iter().map(|(k,v)| json!([k,v])).collect::<Vec<_>>()
        }),
    }
}

/// JSON → acción (con validación: si algo no cuadra, error legible).
pub fn action_from_json(v: &Value) -> Result<Action, String> {
    let t = v
        .get("type")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "acción sin 'type'".to_string())?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let u = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    let extras = |k: &str| -> Vec<(String, String)> {
        v.get(k)
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|e| {
                        let p = e.as_array()?;
                        Some((
                            p.first()?.as_str()?.to_string(),
                            p.get(1)?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok(match t {
        "notify" => Action::Notify {
            title: s("title"),
            body: s("body"),
        },
        "script" => Action::Script {
            cmd: s("cmd"),
            timeout_ms: if u("timeout_ms") == 0 { 15_000 } else { u("timeout_ms") },
        },
        "http" => Action::Http {
            method: {
                let m = s("method");
                if m.is_empty() { "GET".to_string() } else { m }
            },
            url: s("url"),
            body: v.get("body").and_then(|x| x.as_str()).map(|x| x.to_string()),
            headers: extras("headers"),
            timeout_ms: if u("timeout_ms") == 0 { 15_000 } else { u("timeout_ms") },
        },
        "write_file" => Action::WriteFile {
            path: s("path"),
            content: s("content"),
            append: v.get("append").and_then(|x| x.as_bool()).unwrap_or(false),
        },
        "setting" => Action::Setting {
            namespace: {
                let n = s("namespace");
                if n.is_empty() { "global".to_string() } else { n }
            },
            key: s("key"),
            value: s("value"),
        },
        "root_command" => Action::RootCommand { cmd: s("cmd") },
        "broadcast" => Action::Broadcast {
            action: s("action"),
            extras: extras("extras"),
        },
        otro => return Err(format!("tipo de acción desconocido: {}", otro)),
    })
}

fn condition_to_json(c: &Condition) -> Value {
    json!({
        "field": c.field.as_str(),
        "op": c.op.as_str(),
        "value": c.value,
        "extra_key": c.extra_key,
    })
}

fn condition_from_json(v: &Value) -> Result<Condition, String> {
    let f = v
        .get("field")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "condición sin 'field'".to_string())?;
    let o = v
        .get("op")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "condición sin 'op'".to_string())?;
    Ok(Condition {
        field: Field::from_str(f).ok_or_else(|| format!("campo desconocido: {}", f))?,
        op: Op::from_str(o).ok_or_else(|| format!("operador desconocido: {}", o))?,
        value: v
            .get("value")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        extra_key: v
            .get("extra_key")
            .and_then(|x| x.as_str())
            .map(|x| x.to_string()),
    })
}

/// Regla → JSON (formato estable, el que se guarda en disco).
pub fn rule_to_json(r: &Rule) -> Value {
    let trigger = match &r.trigger {
        Trigger::Event(k) => json!({"type":"event","kind":k.as_str()}),
        Trigger::Interval { every_secs } => json!({"type":"interval","every_secs":every_secs}),
        Trigger::Cron { expr } => json!({"type":"cron","expr":expr}),
    };
    json!({
        "id": r.id,
        "name": r.name,
        "enabled": r.enabled,
        "trigger": trigger,
        "conditions": r.conditions.iter().map(condition_to_json).collect::<Vec<_>>(),
        "actions": r.actions.iter().map(action_to_json).collect::<Vec<_>>(),
        "cooldown_secs": r.cooldown_secs,
        "max_fires": r.max_fires,
        "stop_on_error": r.stop_on_error,
        "allow_root": r.allow_root,
    })
}

/// JSON → regla (con validación: si algo no cuadra, error legible).
pub fn rule_from_json(v: &Value) -> Result<Rule, String> {
    let mut r = Rule {
        id: v
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        name: v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        enabled: v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
        cooldown_secs: v.get("cooldown_secs").and_then(|x| x.as_u64()).unwrap_or(60),
        max_fires: v.get("max_fires").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        stop_on_error: v.get("stop_on_error").and_then(|x| x.as_bool()).unwrap_or(false),
        allow_root: v.get("allow_root").and_then(|x| x.as_bool()).unwrap_or(true),
        ..Rule::default()
    };
    if r.id.is_empty() {
        r.id = slug(&r.name);
    }
    let t = v.get("trigger").ok_or_else(|| "regla sin 'trigger'".to_string())?;
    let tt = t
        .get("type")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "trigger sin 'type'".to_string())?;
    r.trigger = match tt {
        "event" => {
            let k = t
                .get("kind")
                .and_then(|x| x.as_str())
                .ok_or_else(|| "trigger event sin 'kind'".to_string())?;
            Trigger::Event(
                EventKind::from_str(k).ok_or_else(|| format!("evento desconocido: {}", k))?,
            )
        }
        "interval" => Trigger::Interval {
            every_secs: t.get("every_secs").and_then(|x| x.as_u64()).unwrap_or(60),
        },
        "cron" => Trigger::Cron {
            expr: t
                .get("expr")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        },
        o => return Err(format!("tipo de trigger desconocido: {}", o)),
    };
    if let Some(cs) = v.get("conditions").and_then(|x| x.as_array()) {
        for c in cs {
            r.conditions.push(condition_from_json(c)?);
        }
    }
    if let Some(as_) = v.get("actions").and_then(|x| x.as_array()) {
        for a in as_ {
            r.actions.push(action_from_json(a)?);
        }
    }
    Ok(r)
}

/// Convierte un nombre en un id utilizable como nombre de fichero.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "regla".to_string()
    } else {
        out
    }
}

// ========================================================================= //
// Persistencia
// ========================================================================= //

/// Carga las reglas de un fichero JSON (`{"rules":[...]}` o lista pelada).
pub fn load_rules(path: &str) -> Vec<Rule> {
    let Ok(txt) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
        return Vec::new();
    };
    let arr = v
        .get("rules")
        .and_then(|x| x.as_array())
        .cloned()
        .or_else(|| v.as_array().cloned())
        .unwrap_or_default();
    arr.iter()
        .filter_map(|r| rule_from_json(r).ok())
        .collect()
}

/// Guarda las reglas.
pub fn save_rules(path: &str, rules: &[Rule]) -> Result<(), String> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let v = json!({
        "version": 1,
        "rules": rules.iter().map(rule_to_json).collect::<Vec<_>>(),
    });
    std::fs::write(path, serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Añade resultados al journal (JSON Lines).
pub fn append_journal(path: &str, outcomes: &[Outcome]) -> Result<(), String> {
    if outcomes.is_empty() {
        return Ok(());
    }
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    use std::io::Write;
    for o in outcomes {
        let line = serde_json::to_string(&o.to_json()).map_err(|e| e.to_string())?;
        writeln!(f, "{}", line).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Lee las últimas `n` líneas del journal.
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

/// Estado de cooldowns: regla → epoch del último disparo.
pub type Cooldowns = BTreeMap<String, i64>;

pub fn load_cooldowns(path: &str) -> Cooldowns {
    let Ok(txt) = std::fs::read_to_string(path) else {
        return Cooldowns::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
        return Cooldowns::new();
    };
    v.as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, val)| val.as_i64().map(|i| (k.clone(), i)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn save_cooldowns(path: &str, c: &Cooldowns) -> Result<(), String> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let v = json!(c);
    std::fs::write(path, serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Nº de disparos accumulated por regla (para `max_fires`).
pub fn load_fire_counts(path: &str) -> BTreeMap<String, u32> {
    let Ok(txt) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
        return BTreeMap::new();
    };
    v.as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, val)| val.as_u64().map(|i| (k.clone(), i as u32)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn save_fire_counts(path: &str, c: &BTreeMap<String, u32>) -> Result<(), String> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let v = json!(c);
    std::fs::write(path, serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    fn ev_sms() -> Event {
        Event::new(EventKind::SmsReceived, 1000)
            .with_key("+34600111222")
            .with_text("¿estás en casa?")
            .with_extra("peer", "+34600111222")
    }

    #[test]
    fn los_eventos_se_identified_por_nombre() {
        for k in EventKind::all() {
            assert_eq!(EventKind::from_str(k.as_str()), Some(*k));
        }
        assert_eq!(EventKind::from_str("no_existe"), None);
    }

    #[test]
    fn una_regla_de_sms_dispara_con_sms() {
        let r = Rule {
            id: "sms".into(),
            name: "SMS".into(),
            trigger: Trigger::Event(EventKind::SmsReceived),
            ..Rule::default()
        };
        assert!(matches(&r, &ev_sms()));
        let otro = Event::new(EventKind::CallMissed, 1000);
        assert!(!matches(&r, &otro));
    }

    #[test]
    fn la_condicion_de_texto_filtra_por_contenido() {
        let r = Rule {
            trigger: Trigger::Event(EventKind::SmsReceived),
            conditions: vec![Condition::new(Field::Text, Op::Contains, "casa")],
            ..Rule::default()
        };
        assert!(matches(&r, &ev_sms()));
        let r2 = Rule {
            trigger: Trigger::Event(EventKind::SmsReceived),
            conditions: vec![Condition::new(Field::Text, Op::Contains, "oficina")],
            ..Rule::default()
        };
        assert!(!matches(&r2, &ev_sms()));
    }

    #[test]
    fn la_comparacion_numerica_usa_el_valor_del_evento() {
        let ev = Event::new(EventKind::BatteryChanged, 10).with_value(15.0);
        assert!(condition_ok(&ev, &Condition::new(Field::BatteryPct, Op::Lt, "20")));
        assert!(!condition_ok(&ev, &Condition::new(Field::BatteryPct, Op::Gt, "20")));
        assert!(condition_ok(&ev, &Condition::new(Field::BatteryPct, Op::Ge, "15")));
    }

    #[test]
    fn el_cooldown_impide_disparar_twice_seguidas() {
        let r = Rule {
            trigger: Trigger::Event(EventKind::BatteryChanged),
            cooldown_secs: 300,
            ..Rule::default()
        };
        let ev = Event::new(EventKind::BatteryChanged, 1000).with_value(10.0);
        assert!(due(&r, &ev, 1000, None, 0));
        assert!(!due(&r, &ev, 1100, Some(1000), 0));
        assert!(due(&r, &ev, 1400, Some(1000), 0));
    }

    #[test]
    fn max_fires_es_un_tope_duro() {
        let r = Rule {
            trigger: Trigger::Event(EventKind::Schedule),
            max_fires: 2,
            cooldown_secs: 0,
            ..Rule::default()
        };
        let ev = Event::new(EventKind::Schedule, 1000);
        assert!(due(&r, &ev, 1000, None, 0));
        assert!(due(&r, &ev, 1000, None, 1));
        assert!(!due(&r, &ev, 1000, None, 2));
    }

    #[test]
    fn una_regla_desactivada_no_dispara() {
        let r = Rule {
            enabled: false,
            trigger: Trigger::Event(EventKind::Schedule),
            ..Rule::default()
        };
        assert!(!due(&r, &Event::new(EventKind::Schedule, 1), 1, None, 0));
    }

    #[test]
    fn las_acciones_se_ejecutan_en_orden() {
        let r = Rule {
            id: "x".into(),
            name: "prueba".into(),
            trigger: Trigger::Event(EventKind::SmsReceived),
            actions: vec![
                Action::Notify {
                    title: "T".into(),
                    body: "B".into(),
                },
                Action::Setting {
                    namespace: "global".into(),
                    key: "k".into(),
                    value: "v".into(),
                },
            ],
            ..Rule::default()
        };
        let mut eff = NullEffector::default();
        let out = run_rule(&r, &ev_sms(), &mut eff, 2000);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|o| o.ok));
        assert_eq!(eff.performed.len(), 2);
        assert!(eff.performed[0].contains("notificar"));
    }

    #[test]
    fn stop_on_error_corta_el_resto() {
        struct Failing;
        impl Effector for Failing {
            fn perform(&mut self, action: &Action) -> Result<String, String> {
                match action {
                    Action::Script { .. } => Err("boom".into()),
                    _ => Ok("ok".into()),
                }
            }
        }
        let r = Rule {
            id: "x".into(),
            trigger: Trigger::Event(EventKind::SmsReceived),
            stop_on_error: true,
            actions: vec![
                Action::Script {
                    cmd: "false".into(),
                    timeout_ms: 1000,
                },
                Action::Notify {
                    title: "t".into(),
                    body: "b".into(),
                },
            ],
            ..Rule::default()
        };
        let out = run_rule(&r, &ev_sms(), &mut Failing, 1);
        assert_eq!(out.len(), 1);
        assert!(!out[0].ok);
    }

    #[test]
    fn allow_root_falso_bloquea_el_comando_root() {
        let r = Rule {
            id: "x".into(),
            allow_root: false,
            trigger: Trigger::Event(EventKind::SmsReceived),
            actions: vec![Action::RootCommand {
                cmd: "id".into(),
            }],
            ..Rule::default()
        };
        let out = run_rule(&r, &ev_sms(), &mut NullEffector::default(), 1);
        assert_eq!(out.len(), 1);
        assert!(!out[0].ok);
        assert!(out[0].detail.contains("allow_root"));
    }

    #[test]
    fn la_regla_va_y_vuelve_de_json() {
        let r = Rule {
            id: "sms-casa".into(),
            name: "SMS casa".into(),
            trigger: Trigger::Event(EventKind::SmsReceived),
            conditions: vec![Condition::extra("ssid", Op::Eq, "MiFibra")],
            actions: vec![Action::Broadcast {
                action: "com.x.y".into(),
                extras: vec![("k".into(), "v".into())],
            }],
            cooldown_secs: 30,
            allow_root: false,
            ..Rule::default()
        };
        let j = rule_to_json(&r);
        let back = rule_from_json(&j).expect("debe volver");
        assert_eq!(back.id, r.id);
        assert_eq!(back.cooldown_secs, 30);
        assert!(!back.allow_root);
        assert_eq!(back.conditions.len(), 1);
        assert_eq!(back.actions.len(), 1);
    }

    #[test]
    fn una_accion_desconocida_es_error_legible() {
        let v = json!({"type":"inventada"});
        assert!(action_from_json(&v).is_err());
    }

    #[test]
    fn el_slug_genera_ids_utiles() {
        assert_eq!(slug("SMS de casa"), "sms-de-casa");
        assert_eq!(slug("  "), "regla");
    }

    #[test]
    fn el_journal_guarda_y_se_lee() {
        let p = "/tmp/md_test_automation_journal.ndjson";
        let _ = std::fs::remove_file(p);
        let o = Outcome {
            rule_id: "r".into(),
            rule_name: "n".into(),
            event_kind: "sms_received".into(),
            action_index: 0,
            action: "notify".into(),
            ok: true,
            detail: "d".into(),
            ts: 5,
        };
        append_journal(p, &[o]).unwrap();
        let tail = journal_tail(p, 10);
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0]["rule_id"], json!("r"));
        let _ = std::fs::remove_file(p);
    }
}

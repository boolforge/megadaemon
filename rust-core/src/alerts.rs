//! MegaDaemon v13.4 — Sistema de alertas.
//!
//! Un demonio que trabaja solo necesita poder **avisar**: batería baja, thermal,
//! disco lleno, cola de tareas desbordada, cita médica mañana, pharmacological
//! recordatorio, app que se ha actualizado con cambios de manifiesto, etc.
//!
//! Reglas de diseño:
//!
//! * **deduplicación por clave**: la misma condición repetida no genera un
//!   aluvión de alertas; incrementa un contador y actualiza la última vez;
//! * **severidad** (`info`/`warn`/`crit`) y **ventana de silenciado**;
//! * **acuse**: el usuario (o el agente) puede acusar una alerta y queda
//!   registrada, no borrada;
//! * **entrega** en tres destinos: memoria, fichero NDJSON (persistente) y
//!   webhook opcional (`MEGADAEMON_ALERT_WEBHOOK`);
//! * **reglas evaluables**: `check_rules()` corre reglas reales contra el
//!   estado del dispositivo (energía, disco, cola, citas detectadas).

use serde_json::{json, Value};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// Severidad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warn,
    Crit,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Crit => "crit",
        }
    }
    pub fn parse(s: &str) -> Severity {
        match s {
            "crit" | "critical" | "grave" => Severity::Crit,
            "warn" | "warning" | "aviso" => Severity::Warn,
            _ => Severity::Info,
        }
    }
}

/// Una alerta emitida.
#[derive(Clone, Debug)]
pub struct Alert {
    pub id: String,
    pub rule: String,
    pub severity: Severity,
    pub title: String,
    pub body: String,
    pub source: String,
    pub dedup_key: String,
    pub count: u32,
    pub created_at: u64,
    pub last_seen: u64,
    pub acknowledged: bool,
    pub ack_at: Option<u64>,
}

fn store() -> &'static Mutex<Vec<Alert>> {
    static S: OnceLock<Mutex<Vec<Alert>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Vec::new()))
}

fn path() -> String {
    format!("{}/alerts.ndjson", crate::cockpit_server::state_dir())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ventana de silenciado por defecto (segundos) para la misma clave.
const DEDUP_WINDOW: u64 = 3600;

/// Emite una alerta. Devuelve el id (mismo id si ya existía la clave).
pub fn raise(rule: &str, severity: Severity, title: &str, body: &str, dedup_key: &str) -> String {
    let key = if dedup_key.is_empty() { rule } else { dedup_key };
    let ts = now();
    let mut s = crate::lock_ok(store());
    let existing = s
        .iter_mut()
        .find(|a| a.dedup_key == key && ts.saturating_sub(a.last_seen) < DEDUP_WINDOW);
    let (id, count, is_new) = match existing {
        Some(a) => {
            a.count += 1;
            a.last_seen = ts;
            a.title = title.to_string();
            a.body = body.to_string();
            a.severity = severity;
            (a.id.clone(), a.count, false)
        }
        None => {
            let id = format!("alert-{}-{}", ts, s.len() + 1);
            s.push(Alert {
                id: id.clone(),
                rule: rule.to_string(),
                severity,
                title: title.to_string(),
                body: body.to_string(),
                source: "megadaemon".to_string(),
                dedup_key: key.to_string(),
                count: 1,
                created_at: ts,
                last_seen: ts,
                acknowledged: false,
                ack_at: None,
            });
            (id, 1, true)
        }
    };
    drop(s);
    if is_new {
        append_disk(&json!({
            "id": id, "rule": rule, "severity": severity.as_str(),
            "title": title, "body": body, "dedup_key": key, "ts": ts,
        }));
        deliver(&json!({
            "id": id, "rule": rule, "severity": severity.as_str(),
            "title": title, "body": body, "ts": ts,
        }));
    }
    id
}

/// Acusa una alerta.
pub fn acknowledge(id: &str) -> bool {
    let mut s = crate::lock_ok(store());
    if let Some(a) = s.iter_mut().find(|a| a.id == id) {
        a.acknowledged = true;
        a.ack_at = Some(now());
        true
    } else {
        false
    }
}

/// Lista de alertas (opcionalmente filtrada por severidad mínima).
pub fn list(min_severity: Option<Severity>, include_ack: bool, limit: usize) -> Value {
    let s = crate::lock_ok(store());
    let mut items: Vec<Value> = s
        .iter()
        .filter(|a| include_ack || !a.acknowledged)
        .filter(|a| match min_severity {
            Some(min) => a.severity >= min,
            None => true,
        })
        .map(|a| {
            json!({
                "id": a.id, "rule": a.rule, "severity": a.severity.as_str(),
                "title": a.title, "body": a.body, "source": a.source,
                "count": a.count, "created_at": a.created_at, "last_seen": a.last_seen,
                "acknowledged": a.acknowledged, "ack_at": a.ack_at,
            })
        })
        .collect();
    items.sort_by(|x, y| {
        let sx = x["severity"].as_str().unwrap_or("info");
        let sy = y["severity"].as_str().unwrap_or("info");
        let rank = |s: &str| match s {
            "crit" => 2,
            "warn" => 1,
            _ => 0,
        };
        rank(sy).cmp(&rank(sx)).then(
            y["last_seen"]
                .as_u64()
                .unwrap_or(0)
                .cmp(&x["last_seen"].as_u64().unwrap_or(0)),
        )
    });
    let total = items.len();
    items.truncate(limit.clamp(1, 500));
    json!({ "ok": true, "total": total, "returned": items.len(), "alerts": items })
}

/// Estado del sistema de alertas.
pub fn status() -> Value {
    let s = crate::lock_ok(store());
    let crit = s.iter().filter(|a| a.severity == Severity::Crit && !a.acknowledged).count();
    let warn = s.iter().filter(|a| a.severity == Severity::Warn && !a.acknowledged).count();
    let info = s.iter().filter(|a| a.severity == Severity::Info && !a.acknowledged).count();
    json!({
        "ok": true,
        "active": crit + warn + info,
        "crit": crit, "warn": warn, "info": info,
        "total_emitted": s.len(),
        "dedup_window_s": DEDUP_WINDOW,
        "persistence": path(),
    })
}

/// Elimina las alertas acusadas (limpieza).
pub fn prune() -> usize {
    let mut s = crate::lock_ok(store());
    let before = s.len();
    s.retain(|a| !a.acknowledged);
    before - s.len()
}

/// Escribe en el fichero persistente.
fn append_disk(entry: &Value) {
    let _ = std::fs::create_dir_all(crate::cockpit_server::state_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path())
    {
        let _ = writeln!(f, "{}", entry);
    }
}

/// Entrega externa opcional (webhook). No bloquea la emisión.
fn deliver(entry: &Value) {
    let Ok(url) = std::env::var("MEGADAEMON_ALERT_WEBHOOK") else {
        return;
    };
    if url.trim().is_empty() {
        return;
    }
    let url = url.trim().to_string();
    let body = entry.to_string();
    std::thread::spawn(move || {
        let mut cmd = std::process::Command::new("/data/data/com.termux/files/usr/bin/curl");
        cmd.args(["-s", "-m", "10", "-X", "POST", "-H", "Content-Type: application/json", "-d", &body, &url]);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        let _ = cmd.status();
    });
}

/// Porcentaje de batería (0 si no se pudo leer).
fn batt(p: &crate::power_policy::PowerState) -> u8 {
    p.capacity_pct.unwrap_or(0)
}

/// Reglas evaluables contra el estado real del dispositivo.
pub fn check_rules() -> Value {
    let mut raised = vec![];

    // 1) Energía.
    let (pstate, pol) = crate::power_policy::current_policy();
    if batt(&pstate) <= 15 && !pstate.charging {
        raised.push(raise(
            "power.battery_low",
            Severity::Crit,
            "Batería muy baja",
            &format!(
                "Batería al {} % sin cargador: se.stop las tareas pesadas.",
                batt(&pstate)
            ),
            "power.battery_low",
        ));
    } else if batt(&pstate) <= 25 && !pstate.charging {
        raised.push(raise(
            "power.battery_low",
            Severity::Warn,
            "Batería baja",
            &format!("Batería al {} % sin cargador.", batt(&pstate)),
            "power.battery_low",
        ));
    }
    if pstate.thermal_hot {
        raised.push(raise(
            "power.thermal",
            Severity::Warn,
            "Temperatura alta",
            &format!("El sistema marca térmico alto: se reduce la carga."),
            "power.thermal",
        ));
    }

    // 2) Disco.
    let df = crate::android::root_exec("df -P /data | tail -1", 15_000, 2048);
    if let Some(pct) = df
        .stdout
        .split_whitespace()
        .last()
        .and_then(|s| s.trim_end_matches('%').parse::<u32>().ok())
    {
        if pct >= 95 {
            raised.push(raise(
                "storage.full",
                Severity::Crit,
                "Almacenamiento casi lleno",
                &format!("/data al {} % de uso.", pct),
                "storage.full",
            ));
        } else if pct >= 88 {
            raised.push(raise(
                "storage.full",
                Severity::Warn,
                "Almacenamiento alto",
                &format!("/data al {} % de uso.", pct),
                "storage.full",
            ));
        }
    }

    // 3) Cola de tareas desbordada o atascada.
    let st = crate::task_scheduler::stats();
    let failed = st["by_state"]["failed"].as_u64().unwrap_or(0);
    if failed > 0 {
        raised.push(raise(
            "tasks.failed",
            Severity::Warn,
            "Tareas fallidas",
            &format!("{} tareas en estado fallido.", failed),
            "tasks.failed",
        ));
    }
    let pending = st["by_state"]["pending"].as_u64().unwrap_or(0);
    if pending > 50 {
        raised.push(raise(
            "tasks.backlog",
            Severity::Warn,
            "Cola de tareas creciendo",
            &format!("{} tareas pendientes.", pending),
            "tasks.backlog",
        ));
    }

    // 4) Citas/recordatorios detectados por el módulo de mensajería.
    if let Some(cands) = crate::messaging::alert_candidates().get("candidates").cloned() {
        if let Some(arr) = cands.as_array() {
            for c in arr.iter().take(5) {
                raised.push(raise(
                    "health.appointment",
                    Severity::Info,
                    "Cita o recordatorio detectado",
                    &format!(
                        "{} ({})",
                        c["text"].as_str().unwrap_or("").chars().take(160).collect::<String>(),
                        c["when"].as_str().unwrap_or("sin fecha")
                    ),
                    &format!("health.appointment:{}", c["id"].as_str().unwrap_or("?")),
                ));
            }
        }
    }

    json!({
        "ok": true,
        "checked": 5,
        "raised": raised.len(),
        "ids": raised,
        "power": { "battery_pct": batt(&pstate), "charging": pstate.charging,
                   "thermal_hot": pstate.thermal_hot, "allow_heavy": pol.allow_heavy },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_severidad_se_parsea_y_se_ordena() {
        assert_eq!(Severity::parse("crit"), Severity::Crit);
        assert_eq!(Severity::parse("aviso"), Severity::Warn);
        assert_eq!(Severity::parse("cualquier cosa"), Severity::Info);
        assert!(Severity::Crit > Severity::Warn);
        assert!(Severity::Warn > Severity::Info);
    }

    #[test]
    fn deduplica_por_clave() {
        let a = raise("test.dedup", Severity::Info, "A", "b", "k1");
        let b = raise("test.dedup", Severity::Info, "A", "b", "k1");
        assert_eq!(a, b, "la misma clave debe devolver el mismo id");
        let s = crate::lock_ok(store());
        let al = s.iter().find(|x| x.id == a).unwrap();
        assert_eq!(al.count, 2, "el contador debeCREMENTar");
    }

    #[test]
    fn acuse_marca_y_filtra() {
        let id = raise("test.ack", Severity::Warn, "T", "B", "k-ack");
        assert!(list(None, false, 100)["alerts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == json!(id)));
        assert!(acknowledge(&id));
        assert!(!list(None, false, 100)["alerts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == json!(id)));
        assert!(list(None, true, 100)["alerts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == json!(id)));
    }

    #[test]
    fn el_filtro_por_severidad_respeta_el_minimo() {
        raise("test.sev.info", Severity::Info, "i", "b", "k-info");
        raise("test.sev.crit", Severity::Crit, "c", "b", "k-crit");
        let crit = list(Some(Severity::Crit), true, 100);
        let arr = crit["alerts"].as_array().unwrap();
        assert!(arr.iter().all(|a| a["severity"] == json!("crit")));
    }
}

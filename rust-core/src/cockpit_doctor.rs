//! MegaDaemon v14 — `cockpit_doctor`: diagnóstico del cockpit en el móvil.
//!
//! POR QUÉ EXISTE
//!   El usuario abría http://localhost:3000 / :8090 / :8080 en el Samsung y
//!   veía `ERR_CONNECTION_REFUSED`.  El daemon NO estaba escuchando, pero el
//!   motivo era invisible: el supervisor (Magisk) lo relanzaba en bucle y el
//!   log no decía por qué.  Este módulo convierte ese "no sé qué pasa" en un
//!   diagnóstico concreto y en español:
//!
//!     * binario presente y versión;
//!     * proceso del cockpit vivo;
//!     * puertos REALMENTE escuchando (con /health);
//!     * plantilla web (embebida o en disco);
//!     * token de sesión presente;
//!     * API key presente;
//!     * antigüedad de la última sincronización;
//!     * último error del log.
//!
//! DISEÑO (puro y testeable)
//!   El módulo NO toca el disco: todo lo que necesita del sistema entra por
//!   `DoctorEnv` (parámetro) y por el trait `Probe` (inyectado).  Así el mismo
//!   código que corre en el móvil se puede testear en un PC con una sonda
//!   falsa, y `cockpit_doctor` puede añadirse a `lib.rs` sin tocar el
//!   servidor HTTP.
//!
//! ESTADO NUEVO: `not_operational`
//!   El fallo que más confunde es "el proceso existe pero no sirve": el
//!   supervisor lo ve vivo, no lo relanza, y el usuario recibe
//!   ERR_CONNECTION_REFUSED.  `DoctorReport::health` distingue tres estados:
//!
//!     * `Ok`             -> algún puerto responde /health con 200;
//!     * `NotOperational` -> el proceso está vivo pero NADIE responde (el
//!                           arranque se quedó atascado antes del bind);
//!     * `Down`           -> no hay proceso ni hay puerto que conteste.
//!
//! Autor: agente "MegaDaemon-Cockpit".  No toca `cockpit_server.rs`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// ─────────────────────────────────────────────────────────────────────────────
// Entorno
// ─────────────────────────────────────────────────────────────────────────────

/// Todo lo que el doctor necesita saber del sistema.  Se pasa por parámetro
/// para que el módulo sea puro (y testeable sin dispositivo).
#[derive(Debug, Clone)]
pub struct DoctorEnv {
    /// Prefijo de instalación en el móvil (`/data/local/tmp/megadaemon`).
    pub prefix: String,
    /// Directorio de estado (`<prefix>/state` en el móvil).
    pub state_dir: String,
    /// Binario del daemon.
    pub binary: String,
    /// Supervisor de arranque (Magisk `service.d`).
    pub service_script: String,
    /// Candidatos a plantilla web, en orden de prioridad.
    pub template_candidates: Vec<String>,
    /// Puertos en los que el cockpit DEBERÍA estar escuchando.
    pub expected_ports: Vec<u16>,
    /// Log del supervisor (rotativo en /sdcard).
    pub log_path: String,
    /// Epoch actual (para calcular la antigüedad del sync).
    pub now_epoch: u64,
    /// Tamaño de la plantilla embebida en el binario (0 = no hay).
    pub embedded_template_bytes: usize,
    /// Patron de `pgrep` para el proceso del cockpit.
    pub process_pattern: String,
}

impl DoctorEnv {
    /// Entorno del móvil Android (Magisk, root).
    pub fn android() -> Self {
        let prefix = "/data/local/tmp/megadaemon".to_string();
        let state_dir = format!("{}/state", prefix);
        Self {
            binary: format!("{}/megadaemon-core", prefix),
            service_script: "/data/adb/service.d/99-megadaemon.sh".to_string(),
            template_candidates: vec![
                format!("{}/web/cockpit_template.html", prefix),
                "/data/user/0/com.agy/files/megadaemon/web/cockpit_template.html".to_string(),
                "/sdcard/megadaemon/web/cockpit_template.html".to_string(),
                "/sdcard/Download/megadaemon/web/cockpit_template.html".to_string(),
            ],
            expected_ports: vec![3000, 3030, 8090, 9090],
            log_path: "/sdcard/megadaemon.log".to_string(),
            now_epoch: now_epoch(),
            embedded_template_bytes: 0,
            process_pattern: "megadaemon-core cockpit".to_string(),
            prefix,
            state_dir,
        }
    }

    /// Entorno de escritorio (PC): el binario y la plantilla están en el repo.
    pub fn desktop(prefix: &str, state_dir: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
            state_dir: state_dir.to_string(),
            binary: format!("{}/megadaemon-core", prefix),
            service_script: format!("{}/99-megadaemon.sh", prefix),
            template_candidates: vec![
                format!("{}/web/cockpit_template.html", prefix),
                "web/cockpit_template.html".to_string(),
            ],
            expected_ports: vec![3000, 3030, 8090, 9090],
            log_path: format!("{}/megadaemon.log", prefix),
            now_epoch: now_epoch(),
            embedded_template_bytes: 0,
            process_pattern: "megadaemon-core cockpit".to_string(),
        }
    }
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Sondas (inyectables)
// ─────────────────────────────────────────────────────────────────────────────

/// Abstracción del sistema.  `FsProbe` es la implementación real; los tests
/// usan una sonda falsa.
pub trait Probe {
    fn exists(&self, path: &str) -> bool;
    fn read(&self, path: &str) -> Option<String>;
    fn mtime(&self, path: &str) -> Option<u64>;
    fn process_running(&self, pattern: &str) -> bool;
    /// `Some(codigo_http)` si el puerto contesta, `None` si no hay nadie.
    fn http_status(&self, port: u16, path: &str) -> Option<u16>;
    fn log_tail(&self, path: &str, lines: usize) -> Vec<String>;
    fn env(&self, key: &str) -> Option<String>;
}

/// Sonda real: disco, procesos y HTTP por TCP.
pub struct FsProbe;

impl Probe for FsProbe {
    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }
    fn read(&self, path: &str) -> Option<String> {
        fs::read_to_string(path).ok()
    }
    fn mtime(&self, path: &str) -> Option<u64> {
        let md = fs::metadata(path).ok()?;
        let t = md.modified().ok()?;
        t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
    }
    fn process_running(&self, pattern: &str) -> bool {
        Command::new("pgrep")
            .arg("-f")
            .arg(pattern)
            .output()
            .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false)
    }
    fn http_status(&self, port: u16, path: &str) -> Option<u16> {
        use std::io::{Read, Write};
        use std::net::TcpStream;
        use std::time::Duration;
        let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
        s.set_read_timeout(Some(Duration::from_millis(1500))).ok()?;
        s.set_write_timeout(Some(Duration::from_millis(1500))).ok()?;
        let req = format!(
            "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            path, port
        );
        s.write_all(req.as_bytes()).ok()?;
        let mut buf = [0u8; 256];
        let n = s.read(&mut buf).ok()?;
        let head = String::from_utf8_lossy(&buf[..n]);
        let mut it = head.split_whitespace();
        it.next(); // "HTTP/1.1"
        it.next()?.parse::<u16>().ok()
    }
    fn log_tail(&self, path: &str, lines: usize) -> Vec<String> {
        let content = fs::read_to_string(path).unwrap_or_default();
        let all: Vec<&str> = content.lines().collect();
        let start = all.len().saturating_sub(lines);
        all[start..].iter().map(|s| s.to_string()).collect()
    }
    fn env(&self, key: &str) -> Option<String> {
        std::env::var(key).ok().filter(|v| !v.trim().is_empty())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Informe
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    /// Algún puerto responde /health con 200.
    Ok,
    /// El proceso está vivo pero NADIE responde (fallo invisible clásico).
    NotOperational,
    /// No hay proceso ni puerto que conteste.
    Down,
}

impl Health {
    pub fn as_str(&self) -> &'static str {
        match self {
            Health::Ok => "ok",
            Health::NotOperational => "not_operational",
            Health::Down => "down",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortStatus {
    pub port: u16,
    pub http_status: Option<u16>,
    pub answers: bool,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub health: Health,
    pub ok: bool,
    pub binary_present: bool,
    pub binary_version: Option<String>,
    pub process_alive: bool,
    pub expected_ports: Vec<u16>,
    pub ports: Vec<PortStatus>,
    /// "embebida" | "disco" | "falta"
    pub template_source: String,
    pub template_path: Option<String>,
    pub token_present: bool,
    pub api_key_present: bool,
    pub sync_age_sec: Option<u64>,
    pub last_log_error: Option<String>,
    pub last_log_lines: Vec<String>,
    pub problems: Vec<String>,
    pub summary: String,
}

impl DoctorReport {
    pub fn to_json(&self) -> Value {
        json!({
            "health": self.health.as_str(),
            "ok": self.ok,
            "binary_present": self.binary_present,
            "binary_version": self.binary_version,
            "process_alive": self.process_alive,
            "expected_ports": self.expected_ports,
            "ports": self.ports.iter().map(|p| json!({
                "port": p.port,
                "http_status": p.http_status,
                "answers": p.answers,
                "healthy": p.healthy,
            })).collect::<Vec<_>>(),
            "template_source": self.template_source,
            "template_path": self.template_path,
            "token_present": self.token_present,
            "api_key_present": self.api_key_present,
            "sync_age_sec": self.sync_age_sec,
            "last_log_error": self.last_log_error,
            "last_log_lines": self.last_log_lines,
            "problems": self.problems,
            "summary": self.summary,
        })
    }

    /// Resumen legible en español (una línea por problema).
    pub fn summary_lines(&self) -> Vec<String> {
        let mut v = Vec::new();
        v.push(format!(
            "estado del cockpit: {}",
            match self.health {
                Health::Ok => "OPERATIVO",
                Health::NotOperational => "NO OPERATIVO (el proceso vive pero no sirve)",
                Health::Down => "CAÍDO (no hay proceso ni puerto que conteste)",
            }
        ));
        v.push(format!(
            "binario: {} ({})",
            if self.binary_present { "presente" } else { "FALTA" },
            self.binary_version.clone().unwrap_or_else(|| "sin versión".into())
        ));
        v.push(format!(
            "proceso: {}",
            if self.process_alive { "vivo" } else { "NO está corriendo" }
        ));
        for p in &self.ports {
            v.push(format!(
                "puerto {}: {}",
                p.port,
                match p.http_status {
                    Some(c) if c == 200 => "OK (/health 200)".to_string(),
                    Some(c) => format!("responde HTTP {} (no es /health 200)", c),
                    None => "nadie escucha".to_string(),
                }
            ));
        }
        v.push(format!(
            "plantilla web: {}{}",
            self.template_source,
            self.template_path
                .as_ref()
                .map(|p| format!(" ({})", p))
                .unwrap_or_default()
        ));
        v.push(format!(
            "token de sesión: {}",
            if self.token_present { "presente" } else { "FALTA" }
        ));
        v.push(format!(
            "API key remota: {}",
            if self.api_key_present {
                "presente"
            } else {
                "FALTA (expuesto a la LAN sin key)"
            }
        ));
        v.push(match self.sync_age_sec {
            Some(age) => format!("última sincronización: hace {} s", age),
            None => "última sincronización: sin datos".to_string(),
        });
        if let Some(e) = &self.last_log_error {
            v.push(format!("último error del log: {}", e));
        }
        for p in &self.problems {
            v.push(format!("- {}", p));
        }
        v
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Diagnóstico
// ─────────────────────────────────────────────────────────────────────────────

/// Diagnostica el cockpit.  PURO: no toca el sistema, sólo pregunta a `probe`.
pub fn diagnose(env: &DoctorEnv, probe: &dyn Probe) -> DoctorReport {
    let mut problems: Vec<String> = Vec::new();

    // ── binario ──
    let binary_present = probe.exists(&env.binary);
    // La versión del binario desplegado: sibling "VERSION" si existe; si no,
    // la versión del propio doctor (que es la del código que se está ejecutando).
    let binary_version = if binary_present {
        probe
            .read(&format!("{}/VERSION", env.prefix))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| Some(format!("doctor {}", env!("CARGO_PKG_VERSION"))))
    } else {
        None
    };
    if !binary_present {
        problems.push(format!(
            "no está el binario {}: el supervisor no puede arrancar nada",
            env.binary
        ));
    }

    // ── proceso ──
    let process_alive = probe.process_running(&env.process_pattern);
    if !process_alive {
        problems.push(
            "no hay ningún proceso 'megadaemon-core cockpit': el supervisor debería relanzarlo".to_string(),
        );
    }

    // ── puertos ──
    let mut ports = Vec::new();
    for port in &env.expected_ports {
        let code = probe.http_status(*port, "/health");
        ports.push(PortStatus {
            port: *port,
            http_status: code,
            answers: code.is_some(),
            healthy: code == Some(200),
        });
    }
    let healthy_ports: Vec<u16> = ports.iter().filter(|p| p.healthy).map(|p| p.port).collect();
    let answering: Vec<u16> = ports.iter().filter(|p| p.answers).map(|p| p.port).collect();

    // ── plantilla ──
    let disk_template = env
        .template_candidates
        .iter()
        .find(|p| probe.exists(p))
        .cloned();
    let (template_source, template_path) = match (&disk_template, env.embedded_template_bytes) {
        (Some(p), _) => ("disco".to_string(), Some(p.clone())),
        (None, n) if n > 0 => ("embebida".to_string(), None),
        _ => ("falta".to_string(), None),
    };
    if template_source == "falta" {
        problems.push(
            "no hay plantilla web ni embebida: la UI cairá en un placeholder".to_string(),
        );
    }

    // ── token y API key ──
    let token_path = format!("{}/cockpit_token", env.state_dir);
    let token_present = probe
        .read(&token_path)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !token_present {
        problems.push(format!(
            "no hay token de sesión en {}: la UI no podrá llamar a la API",
            token_path
        ));
    }
    let api_key_present = probe
        .env("MEGADAEMON_API_KEY")
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
        || probe
            .read(&format!("{}/megadaemon_api.key", env.state_dir))
            .map(|k| k.trim().len() >= 16)
            .unwrap_or(false);
    if !api_key_present {
        problems.push(
            "no hay API key remota: con el cockpit en 0.0.0.0 cualquiera de la LAN puede usarlo"
                .to_string(),
        );
    }

    // ── sincronización ──
    let meta_path = format!("{}/sync/meta.json", env.state_dir);
    let sync_age_sec = probe
        .read(&meta_path)
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("last_sync_epoch").and_then(|x| x.as_u64()))
        .map(|t| env.now_epoch.saturating_sub(t))
        .or_else(|| probe.mtime(&meta_path).map(|t| env.now_epoch.saturating_sub(t)));

    // ── log ──
    let last_log_lines = probe.log_tail(&env.log_path, 40);
    let last_log_error = last_log_lines
        .iter()
        .rev()
        .find(|l| {
            let u = l.to_uppercase();
            u.contains("ERROR") || u.contains("PANIC") || u.contains("AVISO") || u.contains("FALLO")
        })
        .cloned();

    // ── veredicto ──
    let health = if !healthy_ports.is_empty() {
        Health::Ok
    } else if process_alive || !answering.is_empty() {
        Health::NotOperational
    } else {
        Health::Down
    };
    match health {
        Health::Ok => {}
        Health::NotOperational => {
            problems.push(
                "el proceso está vivo pero /health no responde en ningún puerto: el arranque se \
                 ha quedado atascado ANTES del bind (suele ser `su -c` sin timeout) o el binario \
                 escucha en un puerto ya ocupado"
                    .to_string(),
            );
        }
        Health::Down => {
            problems.push(
                "no hay ningún puerto del cockpit escuchando: comprueba que el supervisor \
                 (/data/adb/service.d/99-megadaemon.sh) está instalado y vigente"
                    .to_string(),
            );
        }
    }

    let ok = health == Health::Ok;
    let mut report = DoctorReport {
        health,
        ok,
        binary_present,
        binary_version,
        process_alive,
        expected_ports: env.expected_ports.clone(),
        ports,
        template_source,
        template_path,
        token_present,
        api_key_present,
        sync_age_sec,
        last_log_error,
        last_log_lines,
        problems,
        summary: String::new(),
    };
    report.summary = report.summary_lines().join("\n");
    report
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI
// ─────────────────────────────────────────────────────────────────────────────

/// Subcomando `doctor`.  Devuelve 0 si el cockpit está operativo, 1 si no.
///
/// ```
/// megadaemon-core doctor            # resumen legible + JSON
/// megadaemon-core doctor --json     # sólo JSON
/// megadaemon-core doctor --prefix=/data/local/tmp/megadaemon --ports=3000,9090
/// ```
pub fn run_doctor_cli(args: &[String]) -> i32 {
    let mut env = DoctorEnv::android();
    let mut json_only = false;
    let mut ports: Option<Vec<u16>> = None;
    for a in args {
        if a == "--json" {
            json_only = true;
        } else if let Some(v) = a.strip_prefix("--prefix=") {
            env.prefix = v.to_string();
            env.binary = format!("{}/megadaemon-core", v);
            env.state_dir = format!("{}/state", v);
        } else if let Some(v) = a.strip_prefix("--state=") {
            env.state_dir = v.to_string();
        } else if let Some(v) = a.strip_prefix("--ports=") {
            ports = Some(
                v.split(',')
                    .filter_map(|p| p.trim().parse::<u16>().ok())
                    .collect::<Vec<u16>>(),
            );
        }
    }
    if let Some(p) = ports {
        if !p.is_empty() {
            env.expected_ports = p;
        }
    }
    let report = diagnose(&env, &FsProbe);
    if json_only {
        println!(
            "{}",
            serde_json::to_string_pretty(&report.to_json()).unwrap_or_else(|_| "{}".into())
        );
    } else {
        println!("{}", report.summary);
        println!();
        println!(
            "{}",
            serde_json::to_string_pretty(&report.to_json()).unwrap_or_else(|_| "{}".into())
        );
    }
    if report.ok {
        0
    } else {
        1
    }
}

/// Utilidad: mapa ordenado de "clave -> valor" para logs.
pub fn flat_map(report: &DoctorReport) -> BTreeMap<&'static str, String> {
    let mut m = BTreeMap::new();
    m.insert("health", report.health.as_str().to_string());
    m.insert("binary_present", report.binary_present.to_string());
    m.insert("process_alive", report.process_alive.to_string());
    m.insert("template_source", report.template_source.clone());
    m.insert("token_present", report.token_present.to_string());
    m.insert("api_key_present", report.api_key_present.to_string());
    m
}

#[cfg(test)]
mod unit {
    use super::*;

    struct Fake {
        files: BTreeMap<String, String>,
        proc: bool,
        http: BTreeMap<u16, Option<u16>>,
        envs: BTreeMap<String, String>,
    }
    impl Fake {
        fn new() -> Self {
            Fake {
                files: BTreeMap::new(),
                proc: false,
                http: BTreeMap::new(),
                envs: BTreeMap::new(),
            }
        }
    }
    impl Probe for Fake {
        fn exists(&self, path: &str) -> bool {
            self.files.contains_key(path)
        }
        fn read(&self, path: &str) -> Option<String> {
            self.files.get(path).cloned()
        }
        fn mtime(&self, path: &str) -> Option<u64> {
            self.files.get(path).map(|_| 1_000)
        }
        fn process_running(&self, _pattern: &str) -> bool {
            self.proc
        }
        fn http_status(&self, port: u16, _path: &str) -> Option<u16> {
            self.http.get(&port).copied().flatten()
        }
        fn log_tail(&self, _path: &str, _lines: usize) -> Vec<String> {
            vec!["[2026-11-19] ERROR: algo".to_string()]
        }
        fn env(&self, key: &str) -> Option<String> {
            self.envs.get(key).cloned()
        }
    }

    fn env_ok() -> DoctorEnv {
        DoctorEnv {
            prefix: "/p".into(),
            state_dir: "/p/state".into(),
            binary: "/p/megadaemon-core".into(),
            service_script: "/s.sh".into(),
            template_candidates: vec!["/p/web/cockpit_template.html".into()],
            expected_ports: vec![3000, 9090],
            log_path: "/p/log".into(),
            now_epoch: 2_000,
            embedded_template_bytes: 100,
            process_pattern: "x".into(),
        }
    }

    #[test]
    fn proceso_vivo_sin_http_es_not_operational() {
        let mut f = Fake::new();
        f.proc = true;
        f.files.insert("/p/megadaemon-core".into(), "bin".into());
        f.files.insert("/p/state/cockpit_token".into(), "tok".into());
        f.files.insert("/p/web/cockpit_template.html".into(), "<html>".into());
        let r = diagnose(&env_ok(), &f);
        assert_eq!(r.health, Health::NotOperational);
        assert!(!r.ok);
    }

    #[test]
    fn todo_verde_es_ok() {
        let mut f = Fake::new();
        f.proc = true;
        f.http.insert(3000, Some(200));
        f.files.insert("/p/megadaemon-core".into(), "bin".into());
        f.files.insert("/p/state/cockpit_token".into(), "tok".into());
        f.files.insert("/p/web/cockpit_template.html".into(), "<html>".into());
        let r = diagnose(&env_ok(), &f);
        assert_eq!(r.health, Health::Ok);
        assert!(r.ok);
        assert!(r.token_present);
    }
}

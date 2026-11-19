//! MegaDaemon v14 — prueba de humo del `cockpit_doctor`.
//!
//! POR QUÉ ESTE TEST
//!   El fallo que el usuario veía ("ERR_CONNECTION_REFUSED" en todos los
//!   puertos) era INVISIBLE: el proceso `megadaemon-core cockpit` existía
//!   (el supervisor lo veía vivo y no lo relanzaba) pero el cockpit no
//!   escuchaba en ningún puerto.  Este test fija ese estado como
//!   `not_operational` y comprueba que el doctor lo dice en claro.
//!
//! El módulo se incluye por ruta (`#[path]`) para no tocar `lib.rs`, que es
//! del agente principal; el principal lo cablea cuando quiera.
//!
//! Ejecutar:
//!   export PATH="$HOME/.cargo/bin:$PATH"
//!   cd /workspace/megadaemon/rust-core
//!   CARGO_TARGET_DIR=/workspace/md-t-cockpit cargo test --test cockpit_doctor_smoke -- --nocapture

#[path = "../src/cockpit_doctor.rs"]
mod cockpit_doctor;

use cockpit_doctor::{diagnose, DoctorEnv, Health, Probe};
use std::collections::BTreeMap;

/// Sonda falsa: el doctor no toca el sistema, así que el test decide el mundo.
#[derive(Default)]
struct FakeProbe {
    files: BTreeMap<String, String>,
    proc_alive: bool,
    http: BTreeMap<u16, Option<u16>>,
    envs: BTreeMap<String, String>,
    log: Vec<String>,
}

impl FakeProbe {
    fn with_files(mut self, kv: &[(&str, &str)]) -> Self {
        for (k, v) in kv {
            self.files.insert(k.to_string(), v.to_string());
        }
        self
    }
    fn proc(mut self, alive: bool) -> Self {
        self.proc_alive = alive;
        self
    }
    fn http(mut self, port: u16, code: Option<u16>) -> Self {
        self.http.insert(port, code);
        self
    }
}

impl Probe for FakeProbe {
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
        self.proc_alive
    }
    fn http_status(&self, port: u16, _path: &str) -> Option<u16> {
        self.http.get(&port).copied().flatten()
    }
    fn log_tail(&self, _path: &str, _lines: usize) -> Vec<String> {
        self.log.clone()
    }
    fn env(&self, key: &str) -> Option<String> {
        self.envs.get(key).cloned()
    }
}

fn env_movil() -> DoctorEnv {
    DoctorEnv {
        prefix: "/data/local/tmp/megadaemon".into(),
        state_dir: "/data/local/tmp/megadaemon/state".into(),
        binary: "/data/local/tmp/megadaemon/megadaemon-core".into(),
        service_script: "/data/adb/service.d/99-megadaemon.sh".into(),
        template_candidates: vec![
            "/data/local/tmp/megadaemon/web/cockpit_template.html".into(),
            "/data/user/0/com.agy/files/megadaemon/web/cockpit_template.html".into(),
        ],
        expected_ports: vec![3000, 3030, 8090, 9090],
        log_path: "/sdcard/megadaemon.log".into(),
        now_epoch: 2_000,
        embedded_template_bytes: 87_499,
        process_pattern: "megadaemon-core cockpit".into(),
    }
}

fn instalacion_completa() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "/data/local/tmp/megadaemon/megadaemon-core",
            "ELF ARM64 binario",
        ),
        (
            "/data/local/tmp/megadaemon/web/cockpit_template.html",
            "<!doctype html>",
        ),
        (
            "/data/local/tmp/megadaemon/state/cockpit_token",
            "f222cd67515022ed86a822be6c32bff12cf37b0c327a95a97b578ae18ccd1c8d",
        ),
        (
            "/data/local/tmp/megadaemon/state/megadaemon_api.key",
            "mdaemon_sk_live_0123456789abcdef",
        ),
        (
            "/data/local/tmp/megadaemon/state/sync/meta.json",
            "{\"last_sync_epoch\": 1500}",
        ),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────

/// ESTADO NUEVO: el proceso vive pero NADIE responde ⇒ `not_operational`.
///
/// Antes este caso se confundía con "está bien, sólo hay que recargar" y el
/// usuario recibía ERR_CONNECTION_REFUSED sin explicación.
#[test]
fn doctor_detecta_proceso_vivo_pero_sin_servidor() {
    let probe = FakeProbe::default()
        .with_files(&instalacion_completa())
        .proc(true)
        .http(3000, None)
        .http(3030, None)
        .http(8090, None)
        .http(9090, None);

    let r = diagnose(&env_movil(), &probe);
    println!("[DOCTOR-1] health = {}", r.health.as_str());
    println!("[DOCTOR-1] {}", r.summary);
    println!("[DOCTOR-1] problemas: {:?}", r.problems);

    assert_eq!(
        r.health,
        Health::NotOperational,
        "un proceso vivo sin /health debe reportarse como not_operational"
    );
    assert!(!r.ok, "not_operational no puede ser 'ok'");
    assert!(r.process_alive);
    assert!(r
        .problems
        .iter()
        .any(|p| p.contains("/health") || p.contains("atascado")),
        "el problema debe explicar por qué: {:?}",
        r.problems
    );
}

/// Estado normal: /health 200 en un puerto alias ⇒ `ok`.
#[test]
fn doctor_da_ok_cuando_health_responde_200() {
    let probe = FakeProbe::default()
        .with_files(&instalacion_completa())
        .proc(true)
        .http(3000, None)
        .http(3030, None)
        .http(8090, None)
        .http(9090, Some(200));

    let r = diagnose(&env_movil(), &probe);
    println!("[DOCTOR-2] health = {}", r.health.as_str());
    println!("[DOCTOR-2] {}", r.summary);

    assert_eq!(r.health, Health::Ok);
    assert!(r.ok);
    assert!(r.token_present, "el token debe detectarse");
    assert!(r.api_key_present, "la API key debe detectarse");
    assert_eq!(r.template_source, "disco");
    assert_eq!(r.sync_age_sec, Some(500), "500 s desde el último sync");
}

/// Sin binario no hay nada que arrancar: `down` + motivo explícito.
#[test]
fn doctor_reporta_binario_ausente() {
    let probe = FakeProbe::default().proc(false);
    let r = diagnose(&env_movil(), &probe);
    println!("[DOCTOR-3] health = {}", r.health.as_str());
    println!("[DOCTOR-3] {}", r.summary);
    assert_eq!(r.health, Health::Down);
    assert!(!r.binary_present);
    assert!(r.problems.iter().any(|p| p.contains("binario")));
}

/// La plantilla embebida salva al móvil aunque el fichero no esté en disco
/// (AUDIT-WEB-01): el doctor debe decir "embebida", no "falta".
#[test]
fn doctor_usa_la_plantilla_embebida_cuando_no_hay_fichero() {
    let probe = FakeProbe::default()
        .with_files(&[
            ("/data/local/tmp/megadaemon/megadaemon-core", "ELF"),
            (
                "/data/local/tmp/megadaemon/state/cockpit_token",
                "f222cd67515022ed86a822be6c32bff12cf37b0c327a95a97b578ae18ccd1c8d",
            ),
        ])
        .proc(true)
        .http(9090, Some(200));

    let r = diagnose(&env_movil(), &probe);
    println!("[DOCTOR-4] template_source = {}", r.template_source);
    assert_eq!(r.template_source, "embebida");
    assert!(r.ok, "con /health 200 y plantilla embebida debe ser ok");
}

/// Un puerto que contesta pero NO con /health 200 (binario viejo) no es
/// "operativo": el verificador del despliegue no debe dar un falso OK.
#[test]
fn doctor_no_da_ok_con_un_puerto_que_no_es_health() {
    let probe = FakeProbe::default()
        .with_files(&instalacion_completa())
        .proc(true)
        .http(3000, Some(404))
        .http(9090, None);

    let r = diagnose(&env_movil(), &probe);
    println!("[DOCTOR-5] health = {}", r.health.as_str());
    println!("[DOCTOR-5] {}", r.summary);
    assert_ne!(r.health, Health::Ok, "404 en /health NO es estar operativo");
    assert!(!r.ok);
    assert!(r
        .ports
        .iter()
        .any(|p| p.port == 3000 && p.answers && !p.healthy));
}

/// El log debe recoger el último error para que el diagnóstico sea accionable.
#[test]
fn doctor_extrae_el_ultimo_error_del_log() {
    let mut probe = FakeProbe::default()
        .with_files(&instalacion_completa())
        .proc(true)
        .http(9090, Some(200));
    probe.log = vec![
        "[2026-11-19 20:30:01] INFO: arrancando".to_string(),
        "[2026-11-19 20:30:02] ERROR: cockpit: puerto alias 3000 no disponible".to_string(),
    ];
    let r = diagnose(&env_movil(), &probe);
    println!("[DOCTOR-6] last_log_error = {:?}", r.last_log_error);
    assert!(
        r.last_log_error
            .as_deref()
            .unwrap_or("")
            .contains("puerto alias 3000"),
        "el último error del log debe aparecer en el informe: {:?}",
        r.last_log_error
    );
}

/// El informe serializa a JSON con los campos clave (para el `doctor --json`).
#[test]
fn doctor_serializa_a_json() {
    let probe = FakeProbe::default()
        .with_files(&instalacion_completa())
        .proc(true)
        .http(9090, Some(200));
    let r = diagnose(&env_movil(), &probe);
    let v = r.to_json();
    println!("[DOCTOR-7] {}", serde_json::to_string_pretty(&v).unwrap());
    assert_eq!(v["health"], "ok");
    assert_eq!(v["token_present"], true);
    assert!(v["ports"].as_array().unwrap().len() >= 4);
}

//! MegaDaemon v16 — CAPA DE DISPOSITIVO DE LA AUTOMATIZACIÓN.
//!
//! Los cinco módulos de automatización (`automation`, `cron`,
//! `energy_automation`, `comms_automation`, `automation_bridge`) son **puros**:
//! no hablan con el móvil. Este fichero es el que las conecta con el
//! dispositivo real: mide, ejecuta y persiste.
//!
//! Todos los comandos se lanzan por `su -mm -c` (root real), que es como el
//! daemon ya ejecuta todo lo demás en el S21 Ultra.

use crate::android;
use crate::automation::{Action, Effector, Event, EventKind, Outcome, Rule};
use crate::comms_automation::{CommsDecision, Consent};
use crate::energy_automation::{EnergyAction, Profile, Snapshot};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Epoch actual en segundos.
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ========================================================================= //
// Rutas de estado
// ========================================================================= //

fn state_dir() -> String {
    crate::cockpit_server::state_dir()
}

pub fn rules_path() -> String {
    format!("{}/automation_rules.json", state_dir())
}
pub fn journal_path() -> String {
    format!("{}/automation_journal.ndjson", state_dir())
}
pub fn cooldown_path() -> String {
    format!("{}/automation_cooldowns.json", state_dir())
}
pub fn fire_counts_path() -> String {
    format!("{}/automation_fires.json", state_dir())
}
pub fn cron_state_path() -> String {
    format!("{}/cron_state.json", state_dir())
}
pub fn energy_profiles_path() -> String {
    format!("{}/energy_profiles.json", state_dir())
}
pub fn consent_path() -> String {
    format!("{}/comms_consent.json", state_dir())
}
pub fn comms_journal_path() -> String {
    format!("{}/comms_journal.ndjson", state_dir())
}
pub fn bridge_dir() -> String {
    format!("{}/bridge", state_dir())
}

// ========================================================================= //
// Efector: ejecuta las acciones de una regla contra el móvil
// ========================================================================= //

/// Ejecutor real de acciones.
pub struct DeviceEffector {
    /// Si es `true` no toca el dispositivo: sólo describe lo que haría.
    pub dry_run: bool,
}

impl Default for DeviceEffector {
    fn default() -> Self {
        DeviceEffector { dry_run: false }
    }
}

impl DeviceEffector {
    pub fn new(dry_run: bool) -> DeviceEffector {
        DeviceEffector { dry_run }
    }

    fn sh(&self, cmd: &str, timeout_ms: u64) -> Result<String, String> {
        if self.dry_run {
            return Ok(format!("[simulado] {}", cmd));
        }
        let r = android::root_exec(cmd, timeout_ms, 16_384);
        if r.timed_out {
            return Err("tiempo de espera agotado".to_string());
        }
        if r.code == Some(0) {
            Ok(r.stdout.trim().to_string())
        } else {
            let e = r.stderr.trim();
            if e.is_empty() {
                Err(format!("código de salida {:?}", r.code))
            } else {
                Err(e.chars().take(300).collect())
            }
        }
    }
}

impl Effector for DeviceEffector {
    fn perform(&mut self, action: &Action) -> Result<String, String> {
        match action {
            Action::Notify { title, body } => {
                let cmd = format!(
                    "cmd notification post -S bigtext -t {} {}",
                    android::sh_quote(title),
                    android::sh_quote(body)
                );
                self.sh(&cmd, 15_000)
            }
            Action::Script { cmd, timeout_ms } => self.sh(cmd, *timeout_ms),
            Action::RootCommand { cmd } => self.sh(cmd, 30_000),
            Action::Setting {
                namespace,
                key,
                value,
            } => {
                let ns = match namespace.as_str() {
                    "global" | "secure" | "system" => namespace.as_str(),
                    otro => return Err(format!("namespace no válido: {}", otro)),
                };
                let cmd = format!(
                    "settings put {} {} {}",
                    ns,
                    android::sh_quote(key),
                    android::sh_quote(value)
                );
                self.sh(&cmd, 15_000)
            }
            Action::Broadcast { action, extras } => {
                let mut cmd = format!("am broadcast -a {}", android::sh_quote(action));
                for (k, v) in extras {
                    cmd.push_str(&format!(
                        " --es {} {}",
                        android::sh_quote(k),
                        android::sh_quote(v)
                    ));
                }
                self.sh(&cmd, 20_000)
            }
            Action::WriteFile {
                path,
                content,
                append,
            } => {
                if self.dry_run {
                    return Ok(format!("[simulado] escribir {}", path));
                }
                // base64 evita cualquier problema de comillas con el contenido.
                use base64ct::Encoding;
                let b64 = base64ct::Base64::encode_string(content.as_bytes());
                let op = if *append { ">>" } else { ">" };
                let cmd = format!(
                    "printf %s {} {}{}",
                    android::sh_quote(&b64),
                    op,
                    android::sh_quote(path)
                );
                self.sh(&cmd, 20_000)
            }
            Action::Http {
                method,
                url,
                body,
                headers,
                timeout_ms,
            } => {
                if self.dry_run {
                    return Ok(format!("[simulado] http {} {}", method, url));
                }
                http_request(method, url, body.as_deref(), headers, *timeout_ms)
            }
        }
    }
}

/// Petición HTTP con `ureq` (nativo, sin curl en el móvil: medido, no hay curl).
pub fn http_request(
    method: &str,
    url: &str,
    body: Option<&str>,
    headers: &[(String, String)],
    timeout_ms: u64,
) -> Result<String, String> {
    let cfg = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(timeout_ms.max(1_000))))
        .timeout_connect(Some(Duration::from_secs(6)))
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::new_with_config(cfg);
    let m = method.to_ascii_uppercase();
    // `ureq` 3.x usa dos tipos de builder (con y sin cuerpo), asi que cada
    // metodo se construye por separado en vez de un `match` comun.
    let resp = match m.as_str() {
        "GET" => {
            let mut req = agent.get(url);
            for (k, v) in headers {
                req = req.header(k, v);
            }
            req.call()
        }
        "DELETE" => {
            let mut req = agent.delete(url);
            for (k, v) in headers {
                req = req.header(k, v);
            }
            req.call()
        }
        "POST" => {
            let mut req = agent.post(url);
            for (k, v) in headers {
                req = req.header(k, v);
            }
            req.send(body.unwrap_or("").as_bytes())
        }
        "PUT" => {
            let mut req = agent.put(url);
            for (k, v) in headers {
                req = req.header(k, v);
            }
            req.send(body.unwrap_or("").as_bytes())
        }
        "PATCH" => {
            let mut req = agent.patch(url);
            for (k, v) in headers {
                req = req.header(k, v);
            }
            req.send(body.unwrap_or("").as_bytes())
        }
        otra => return Err(format!("metodo HTTP no soportado: {}", otra)),
    };
    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let texto = r.into_body().read_to_string().unwrap_or_default();
            Ok(format!("HTTP {} ({} bytes)", status, texto.len()))
        }
        Err(ureq::Error::StatusCode(c)) => Ok(format!("HTTP {}", c)),
        Err(e) => Err(format!("HTTP error: {}", e)),
    }
}

// ========================================================================= //
// Sondeos del dispositivo
// ========================================================================= //

fn sh(cmd: &str, timeout_ms: u64) -> String {
    android::root_exec(cmd, timeout_ms, 65_536).stdout
}

/// Mide el estado de energía/thermal y lo devuelve en formato `clave=valor`
/// (el que entiende `energy_automation::parse_snapshot`).
pub fn probe_energy_text() -> String {
    let mut out = String::new();

    // Batería: `dumpsys battery` es la fuente más fiable.
    let bat = sh("dumpsys battery", 15_000);
    let level = bat
        .lines()
        .find(|l| l.trim_start().starts_with("level:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<u8>().ok());
    let status = bat
        .lines()
        .find(|l| l.trim_start().starts_with("status:"))
        .and_then(|l| l.split(':').nth(1))
        .unwrap_or("")
        .trim()
        .to_string();
    let ac = bat.contains("AC powered: true");
    let usb = bat.contains("USB powered: true");
    let charging = status == "2" || status == "5" || ac || usb;
    out.push_str(&format!("capacity={}\n", level.unwrap_or(0)));
    out.push_str(&format!("charging={}\n", if charging { 1 } else { 0 }));
    out.push_str(&format!("usb={}\n", if usb { 1 } else { 0 }));

    // Temperaturas: las zonas térmicas de sysfs.
    let zones = sh(
        "for z in /sys/class/thermal/thermal_zone*; do [ -d \"$z\" ] || continue; \
         printf '%s %s\\n' \"$(cat $z/type 2>/dev/null)\" \"$(cat $z/temp 2>/dev/null)\"; done",
        15_000,
    );
    let mut max_cpu = 0.0f32;
    let mut battery_temp = None;
    for line in zones.lines() {
        let mut it = line.split_whitespace();
        let (Some(name), Some(raw)) = (it.next(), it.next()) else {
            continue;
        };
        let Ok(milli) = raw.parse::<f32>() else {
            continue;
        };
        let c = milli / 1000.0;
        out.push_str(&format!("thermal={}:{}\n", name, c));
        if name.eq_ignore_ascii_case("battery") {
            battery_temp = Some(c);
        }
        if name.eq_ignore_ascii_case("big") || name.eq_ignore_ascii_case("cpu") {
            max_cpu = max_cpu.max(c);
        }
    }
    out.push_str(&format!("battery_temp_c={}\n", battery_temp.unwrap_or(0.0)));
    out.push_str(&format!("cpu_temp_c={}\n", max_cpu));

    // Ahorro del sistema y gobernador.
    let low_power = sh("settings get global low_power", 10_000);
    out.push_str(&format!(
        "power_save={}\n",
        if low_power.trim() == "1" { 1 } else { 0 }
    ));
    let gov = sh(
        "cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null",
        10_000,
    );
    out.push_str(&format!("governor={}\n", gov.trim()));

    // Límite de carga de Samsung (`mProtectionThreshold` en dumpsys battery).
    let limit = bat
        .lines()
        .find(|l| l.contains("mProtectionThreshold"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<u8>().ok());
    out.push_str(&format!("charge_limit={}\n", limit.unwrap_or(0)));
    // `cmd battery` NO tiene `set limit` en este build (medido), pero el
    // parámetro de Samsung sí: se considera soportado si aparece el umbral.
    out.push_str(&format!(
        "charge_limit_supported={}\n",
        if limit.is_some() { 1 } else { 0 }
    ));

    // Xtreme Battery Saver (módulo Magisk).
    let xbs_dir = "/data/adb/modules/XtremeBS";
    let xbs_prop = sh(
        &format!("cat {}/module.prop 2>/dev/null", xbs_dir),
        10_000,
    );
    let present = xbs_prop.contains("id=XtremeBS");
    out.push_str(&format!("xbs_module={}\n", if present { 1 } else { 0 }));
    if present {
        let ver = xbs_prop
            .lines()
            .find(|l| l.starts_with("version="))
            .map(|l| l.trim_start_matches("version=").to_string())
            .unwrap_or_default();
        out.push_str(&format!("xbs_version={}\n", ver));
        let daemon = sh(
            &format!("pidof XtremeBSd 2>/dev/null || pgrep -f XtremeBSd 2>/dev/null"),
            10_000,
        );
        out.push_str(&format!(
            "xbs_daemon={}\n",
            if daemon.trim().is_empty() { 0 } else { 1 }
        ));
        let ctl = format!("{}/system/bin/XBSctl", xbs_dir);
        out.push_str(&format!(
            "xbs_ctl={}\n",
            if std::path::Path::new(&ctl).exists() { ctl } else { String::new() }
        ));
    }
    // La app Xtreme Battery Saver (XBS de ryangconrad).
    for pkg in crate::battery_saver::XBS_PACKAGES {
        if std::path::Path::new(&format!("/data/data/{}", pkg)).exists() {
            out.push_str("xbs_app=1\n");
            break;
        }
    }
    out
}

/// Estado de energía ya parseado.
pub fn energy_snapshot() -> Snapshot {
    crate::energy_automation::parse_snapshot(&probe_energy_text())
}

/// Apps de automatización instaladas: `paquete -> versión`.
pub fn probe_installed_automation_apps() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let pkgs: Vec<String> = crate::automation_bridge::known_apps()
        .into_iter()
        .map(|t| t.package)
        .collect();
    for p in pkgs {
        let out_cmd = sh(
            &format!(
                "dumpsys package {} 2>/dev/null | grep -m1 'versionName=' | cut -d= -f2",
                p
            ),
            15_000,
        );
        let v = out_cmd.trim().to_string();
        if !v.is_empty() {
            out.insert(p, v);
        }
    }
    out
}

/// ¿Está configurada (con tareas/scripts)? Se mide por la Existence real de
/// su base de datos o de su directorio de arranque.
pub fn automation_app_configured(package: &str) -> Option<bool> {
    match package {
        "vadimerenkov.autasker" => Some(
            std::path::Path::new(&format!("/data/user/0/{}/databases", package)).exists(),
        ),
        "com.termux.boot" => Some(
            std::path::Path::new("/data/user/0/com.termux/files/home/.termux/boot").exists(),
        ),
        _ => None,
    }
}

// ========================================================================= //
// Eventos del sistema
// ========================================================================= //

/// Sondea el dispositivo y devuelve los eventos que han ocurrido desde la
/// última llamada (los persistentes en `<state>/automation_seen.json`).
pub fn poll_events() -> Vec<Event> {
    let mut evs: Vec<Event> = Vec::new();
    let ts = now();

    // Batería.
    let bat = sh("dumpsys battery", 15_000);
    let level = bat
        .lines()
        .find(|l| l.trim_start().starts_with("level:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<f64>().ok());
    if let Some(l) = level {
        evs.push(Event::new(EventKind::BatteryChanged, ts).with_value(l));
    }

    // Temperatura: la zona más caliente.
    let snap = energy_snapshot();
    if let Some(t) = snap.max_zone_temp() {
        evs.push(
            Event::new(EventKind::TemperatureChanged, ts)
                .with_value(t as f64)
                .with_extra("zone", "max"),
        );
    }

    // Red (LTE/WiFi).
    let net = sh("dumpsys connectivity 2>/dev/null | grep -m1 -o 'MOBILE\\[[A-Z0-9]*\\]\\|WIFI' | head -1", 15_000);
    let net = net.trim();
    if !net.is_empty() {
        evs.push(
            Event::new(EventKind::NetworkChanged, ts)
                .with_key(net)
                .with_extra("network", net),
        );
    }

    // SSID de WiFi.
    let ssid = sh(
        "dumpsys wifi 2>/dev/null | grep -m1 -o 'SSID: \"[^\"]*\"' | head -1 | cut -d'\"' -f2",
        15_000,
    );
    let ssid = ssid.trim();
    if !ssid.is_empty() && ssid != "<unknown ssid>" {
        evs.push(
            Event::new(EventKind::WifiChanged, ts)
                .with_key(ssid)
                .with_extra("ssid", ssid),
        );
    }

    // App en primer plano.
    let fg = sh(
        "dumpsys activity activities 2>/dev/null | grep -m1 -o 'topResumedActivity=ActivityRecord{[^}]*}' | head -1",
        15_000,
    );
    if let Some(pkg) = fg
        .split(" u0 ")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .map(|s| s.trim().to_string())
    {
        if !pkg.is_empty() {
            evs.push(
                Event::new(EventKind::AppOpened, ts)
                    .with_key(&pkg)
                    .with_extra("package", &pkg),
            );
        }
    }

    // SMS nuevos (última hora; el filtro fino lo hace el motor).
    let sms = sh(
        "content query --uri content://sms/inbox --projection address,date,body --where \"date > $(( $(date +%s) - 3600 ))\" 2>/dev/null | head -20",
        20_000,
    );
    for line in sms.lines() {
        if !line.contains("Row:") {
            continue;
        }
        let addr = extract_between(line, "address=", ",");
        let body = extract_between(line, "body=", ",");
        let date = extract_between(line, "date=", ",");
        let _ = (date, body);
        if let Some(a) = addr {
            evs.push(
                Event::new(EventKind::SmsReceived, ts)
                    .with_key(&a)
                    .with_extra("peer", &a),
            );
        }
    }

    // Llamada en curso (la detección la hace el módulo de grabación).
    let audio = sh("dumpsys audio 2>/dev/null | head -400", 20_000);
    if let Some(call) = crate::call_recorder::detect_active_call(&audio) {
        // `ActiveCall` no lleva el numero (Android no lo expone sin
        // permisos de registro): se usa cliente/paquete como referencia.
        let peer = call
            .package
            .clone()
            .or_else(|| call.client.clone())
            .unwrap_or_else(|| "desconocido".to_string());
        evs.push(
            Event::new(EventKind::CallIncoming, ts)
                .with_key(&peer)
                .with_extra("peer", &peer),
        );
    }

    evs
}

/// Extrae el valor entre dos separadores (para el formato de `content query`).
fn extract_between(s: &str, start: &str, end: &str) -> Option<String> {
    let i = s.find(start)? + start.len();
    let rest = &s[i..];
    let j = rest.find(end)?;
    Some(rest[..j].trim().to_string())
}

// ========================================================================= //
// Motor: aplicar una regla a un evento
// ========================================================================= //

/// Aplica todas las reglas que casan con el evento y devuelve el resultado.
pub fn dispatch_event(ev: &Event, eff: &mut dyn Effector) -> Vec<Outcome> {
    let rules: Vec<Rule> = crate::automation::load_rules(&rules_path());
    let cooldowns = crate::automation::load_cooldowns(&cooldown_path());
    let fires = crate::automation::load_fire_counts(&fire_counts_path());
    let ts = now();
    let mut out = Vec::new();
    let mut new_cooldowns = cooldowns.clone();
    let mut new_fires = fires.clone();

    for rule in &rules {
        if !rule.enabled {
            continue;
        }
        let last = cooldowns.get(&rule.id).copied();
        let f = fires.get(&rule.id).copied().unwrap_or(0);
        if !crate::automation::due(rule, ev, ts, last, f) {
            continue;
        }
        let res = crate::automation::run_rule(rule, ev, eff, ts);
        new_cooldowns.insert(rule.id.clone(), ts);
        new_fires.insert(rule.id.clone(), f.saturating_add(1));
        out.extend(res);
    }
    let _ = crate::automation::save_cooldowns(&cooldown_path(), &new_cooldowns);
    let _ = crate::automation::save_fire_counts(&fire_counts_path(), &new_fires);
    let _ = crate::automation::append_journal(&journal_path(), &out);
    out
}

// ========================================================================= //
// Energía: aplicar una decisión
// ========================================================================= //

/// Aplica una acción de energía al dispositivo.
///
/// Devuelve lo que REALMENTE se ha aplicado: si el comando no existe en este
/// build, lo dice (no se inventa un éxito).
pub fn apply_energy_action(a: &EnergyAction) -> Result<String, String> {
    match a {
        EnergyAction::Noop => Ok("nada que hacer".to_string()),
        EnergyAction::SetProfile(n) => Ok(format!("perfil activo: {}", n)),
        EnergyAction::RestrictHeavy(b) => Ok(format!(
            "trabajo pesado {}",
            if *b { "restringido" } else { "permitido" }
        )),
        EnergyAction::Notify { title, body } => {
            let cmd = format!(
                "cmd notification post -S bigtext -t {} {}",
                android::sh_quote(title),
                android::sh_quote(body)
            );
            let r = android::root_exec(&cmd, 15_000, 4096);
            if r.code == Some(0) {
                Ok("notificación publicada".to_string())
            } else {
                Err(format!("cmd notification: {}", r.stderr.trim().chars().take(200).collect::<String>()))
            }
        }
        EnergyAction::ResumeCharging => {
            let cmd = "dumpsys battery reset";
            let r = android::root_exec(cmd, 15_000, 4096);
            if r.code == Some(0) {
                Ok("carga reanudada (dumpsys battery reset)".to_string())
            } else {
                Err("no se pudo reanudar la carga".to_string())
            }
        }
        EnergyAction::StopCharging => {
            // `cmd battery unplug` congela el estado "desconectado": el móvil
            // deja de cargar aunque el cable siga puesto. Es el mecanismo que
            // existe en ESTE build (medido: `cmd battery` lista unplug).
            let cmd = "dumpsys battery unplug";
            let r = android::root_exec(cmd, 15_000, 4096);
            if r.code == Some(0) {
                Ok("carga suspendida (dumpsys battery unplug)".to_string())
            } else {
                Err(format!("dumpsys battery unplug: {}", r.stderr.trim()))
            }
        }
        EnergyAction::SetChargeLimit(pct) => {
            // Samsung: el límite se fija en Ajustes → "Protección de la batería".
            // En este build `cmd battery` NO tiene `set limit` (medido), así que
            // se deja constancia de lo que se ha intentado y de lo que hay.
            let actual = energy_snapshot().charge_limit_pct;
            Ok(format!(
                "límite de carga pedido: {}% (dispositivo: {:?}); en este build \
                 `cmd battery` no expone `set limit`: se ajusta en Ajustes → \
                 Protección de la batería",
                pct, actual
            ))
        }
    }
}

// ========================================================================= //
// Comunicaciones: ejecutar una decisión
// ========================================================================= //

/// Ejecuta una decisión de comunicación de verdad.
///
/// `dry_run` sale del consentimiento del usuario.
pub fn execute_comms(d: &CommsDecision, consent: &Consent) -> Result<String, String> {
    if consent.dry_run {
        return Ok(format!("[simulado] {}", d.kind()));
    }
    match d {
        CommsDecision::Reply { to, body } => send_sms(to, body),
        CommsDecision::Forward { to, body } => send_sms(to, body),
        CommsDecision::Record { peer, .. } => Ok(format!(
            "llamada de {} marcada para grabación (grabación nativa CSC)",
            peer
        )),
        CommsDecision::Log { .. } => Ok("registrado".to_string()),
        CommsDecision::Ignore { reason } => Ok(format!("ignorado: {}", reason)),
    }
}

/// Envía un SMS.
///
/// Se prueban las vías que existen de verdad en Android 13 con root; se
/// devuelve la que funcionó. Si ninguna, se dice cuál se intentó y por qué
/// falló (no se finge un envío).
pub fn send_sms(to: &str, body: &str) -> Result<String, String> {
    // 1) `service call isms`: vía clásica con root (sin GUI).
    let num = android::sh_quote(to);
    let txt = android::sh_quote(body);
    let cmd = format!("service call isms 12 i32 1 s16 {} s16 {} s16 '' s16 ''", num, txt);
    let r = android::root_exec(&cmd, 20_000, 4096);
    if r.code == Some(0) && !r.stdout.contains("Exception") {
        return Ok(format!("SMS enviado a {} (service call isms)", to));
    }
    // 2) `am start` con SENDTO: abre el compositor (Android lo exige si la app
    //    no es la de SMS por defecto).
    let cmd2 = format!(
        "am start -a android.intent.action.SENDTO -d smsto:{} --es smstext {}",
        android::sh_quote(to),
        android::sh_quote(body)
    );
    let r2 = android::root_exec(&cmd2, 20_000, 4096);
    if r2.code == Some(0) {
        return Ok(format!(
            "SMS a {} compuesto con `am start SENDTO` (envío manual de un toque)",
            to
        ));
    }
    Err(format!(
        "no se pudo enviar el SMS a {}: isms={} / SENDTO={}",
        to,
        r.stderr.trim().chars().take(120).collect::<String>(),
        r2.stderr.trim().chars().take(120).collect::<String>()
    ))
}

// ========================================================================= //
// Perfiles de energía
// ========================================================================= //

/// Carga los perfiles de energía (los de usuario; si no hay, los de serie).
pub fn load_profiles() -> Vec<Profile> {
    crate::energy_automation::load_profiles(&energy_profiles_path())
}

/// Guarda los perfiles de energía.
pub fn save_profiles(p: &[Profile]) -> Result<(), String> {
    crate::energy_automation::save_profiles(&energy_profiles_path(), p)
}

// ========================================================================= //
// API de alto nivel (la usan el CLI, la herramienta MCP y el agente)
// ========================================================================= //

/// Perfil de energía activo (persistido).
pub fn active_profile_path() -> String {
    format!("{}/energy_active.json", state_dir())
}

pub fn load_active_profile() -> String {
    std::fs::read_to_string(active_profile_path())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("profile").and_then(|x| x.as_str()).map(|s| s.to_string()))
        .unwrap_or_default()
}

pub fn save_active_profile(name: &str) -> Result<(), String> {
    std::fs::write(
        active_profile_path(),
        serde_json::to_string_pretty(&json!({"profile": name})).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

/// Lista las reglas de automatización.
pub fn automation_list() -> Value {
    let rules = crate::automation::load_rules(&rules_path());
    json!({
        "ok": true,
        "count": rules.len(),
        "rules": rules.iter().map(crate::automation::rule_to_json).collect::<Vec<_>>(),
    })
}

/// Añade o actualiza una regla (JSON de regla).
pub fn automation_add(rule_json: &Value) -> Result<Value, String> {
    let mut rule = crate::automation::rule_from_json(rule_json)?;
    let mut rules = crate::automation::load_rules(&rules_path());
    if rule.id.is_empty() {
        rule.id = crate::automation::slug(&rule.name);
    }
    if rules.iter().any(|r| r.id == rule.id) {
        rules.retain(|r| r.id != rule.id);
    }
    rules.push(rule.clone());
    crate::automation::save_rules(&rules_path(), &rules)?;
    Ok(json!({
        "ok": true,
        "id": rule.id,
        "rules": rules.len(),
    }))
}

/// Activa o desactiva una regla.
pub fn automation_toggle(rule_id: &str, enabled: bool) -> Result<Value, String> {
    let mut rules = crate::automation::load_rules(&rules_path());
    let mut found = false;
    for r in rules.iter_mut() {
        if r.id == rule_id {
            r.enabled = enabled;
            found = true;
        }
    }
    if !found {
        return Err(format!("regla desconocida: {}", rule_id));
    }
    crate::automation::save_rules(&rules_path(), &rules)?;
    Ok(json!({"ok": true, "id": rule_id, "enabled": enabled}))
}

/// Borra una regla.
pub fn automation_remove(rule_id: &str) -> Result<Value, String> {
    let mut rules = crate::automation::load_rules(&rules_path());
    let before = rules.len();
    rules.retain(|r| r.id != rule_id);
    if rules.len() == before {
        return Err(format!("regla desconocida: {}", rule_id));
    }
    crate::automation::save_rules(&rules_path(), &rules)?;
    Ok(json!({"ok": true, "id": rule_id, "rules": rules.len()}))
}

/// Construye un evento desde parámetros sueltos.
pub fn build_event(kind: &str, key: &str, value: Option<f64>, text: &str) -> Result<Event, String> {
    let k = crate::automation::EventKind::from_str(kind)
        .ok_or_else(|| format!("evento desconocido: {}", kind))?;
    let mut ev = Event::new(k, now());
    if !key.is_empty() {
        ev = ev.with_key(key);
        ev.extra.insert("peer".into(), key.to_string());
    }
    if let Some(v) = value {
        ev = ev.with_value(v);
    }
    if !text.is_empty() {
        ev = ev.with_text(text);
    }
    Ok(ev)
}

/// Sondea el dispositivo y despacha los eventos a las reglas.
pub fn automation_poll(dry_run: bool) -> Value {
    let evs = poll_events();
    let mut eff = DeviceEffector::new(dry_run);
    let mut outs = Vec::new();
    for ev in &evs {
        outs.extend(dispatch_event(ev, &mut eff));
    }
    json!({
        "ok": true,
        "events": evs.iter().map(|e| json!({
            "kind": e.kind.as_str(), "key": e.key, "value": e.value, "ts": e.ts
        })).collect::<Vec<_>>(),
        "outcomes": outs.iter().map(|o| o.to_json()).collect::<Vec<_>>(),
        "dry_run": dry_run,
    })
}

/// Dispara un evento manual contra las reglas.
pub fn automation_fire(
    kind: &str,
    key: &str,
    value: Option<f64>,
    text: &str,
    dry_run: bool,
) -> Result<Value, String> {
    let ev = build_event(kind, key, value, text)?;
    let mut eff = DeviceEffector::new(dry_run);
    let outs = dispatch_event(&ev, &mut eff);
    Ok(json!({
        "ok": true,
        "event": ev.kind.as_str(),
        "outcomes": outs.iter().map(|o| o.to_json()).collect::<Vec<_>>(),
    }))
}

/// Últimas entradas del journal de automatización.
pub fn automation_journal(limit: usize) -> Value {
    json!({
        "ok": true,
        "entries": crate::automation::journal_tail(&journal_path(), limit),
    })
}

// ------------------------------ cron ------------------------------------- //

/// Lista las tareas cron.
pub fn cron_list() -> Value {
    let st = crate::cron::load(&cron_state_path());
    json!({
        "ok": true,
        "count": st.jobs.len(),
        "tz_offset_min": st.tz_offset_min,
        "jobs": st.jobs.iter().map(crate::cron::job_to_json).collect::<Vec<_>>(),
    })
}

/// Añade o actualiza una tarea cron (JSON de tarea).
pub fn cron_add(job_json: &Value) -> Result<Value, String> {
    let mut job = crate::cron::job_from_json(job_json)?;
    let mut st = crate::cron::load(&cron_state_path());
    if st.tz_offset_min == 0 {
        st.tz_offset_min = crate::cron::parse_tz_offset(
            &android::root_exec("date +%z", 10_000, 128).stdout.trim(),
        )
        .unwrap_or(0);
    }
    if job.next_run.is_none() {
        job.next_run = crate::cron::CronExpr::parse(&job.cron)
            .ok()
            .and_then(|e| e.next_after(now(), st.tz_offset_min));
    }
    st.jobs.retain(|j| j.id != job.id);
    let id = job.id.clone();
    st.jobs.push(job);
    st.updated_at = now();
    crate::cron::save(&cron_state_path(), &st)?;
    Ok(json!({"ok": true, "id": id, "jobs": st.jobs.len()}))
}

/// Borra una tarea cron.
pub fn cron_remove(job_id: &str) -> Result<Value, String> {
    let mut st = crate::cron::load(&cron_state_path());
    let before = st.jobs.len();
    st.jobs.retain(|j| j.id != job_id);
    if st.jobs.len() == before {
        return Err(format!("tarea desconocida: {}", job_id));
    }
    st.updated_at = now();
    crate::cron::save(&cron_state_path(), &st)?;
    Ok(json!({"ok": true, "id": job_id, "jobs": st.jobs.len()}))
}

/// Plan de ejecuciones pendientes (sin ejecutar).
pub fn cron_plan() -> Value {
    let st = crate::cron::load(&cron_state_path());
    let plan = crate::cron::plan(&st, now());
    json!({
        "ok": true,
        "pending": plan.iter().map(|p| json!({
            "job_id": p.job_id, "scheduled_at": p.scheduled_at, "catch_up": p.catch_up
        })).collect::<Vec<_>>(),
    })
}

/// Ejecuta el plan pendiente (y devuelve qué se ha ejecutado).
pub fn cron_tick(dry_run: bool) -> Value {
    let mut st = crate::cron::load(&cron_state_path());
    let ts = now();
    let plan = crate::cron::plan(&st, ts);
    let limits = crate::cron::ResourceLimits::default();
    let mut eff = DeviceEffector::new(dry_run);
    let mut done = Vec::new();
    for p in plan {
        if crate::cron::eligible(&st, &p.job_id, done.len(), &limits, None).is_err() {
            continue;
        }
        let Some(job) = st.jobs.iter().find(|j| j.id == p.job_id).cloned() else {
            continue;
        };
        let mut ok = true;
        let mut detail = String::new();
        match crate::automation::action_from_json(&job.action) {
            Ok(action) => {
                let _ = crate::cron::mark_running(&mut st, &p.job_id, ts);
                match eff.perform(&action) {
                    Ok(d) => detail = d,
                    Err(e) => {
                        ok = false;
                        detail = e;
                    }
                }
            }
            Err(e) => {
                ok = false;
                detail = e;
            }
        }
        let _ = crate::cron::mark_result(&mut st, &p.job_id, ok, ts);
        done.push(json!({
            "job_id": p.job_id, "ok": ok, "detail": detail, "catch_up": p.catch_up
        }));
    }
    st.updated_at = ts;
    let _ = crate::cron::save(&cron_state_path(), &st);
    json!({"ok": true, "executed": done, "dry_run": dry_run})
}

/// Relanza las tareas que quedaron interrumpidas.
pub fn cron_resume() -> Value {
    let mut st = crate::cron::load(&cron_state_path());
    let ts = now();
    let limits = crate::cron::ResourceLimits::default();
    let inter = crate::cron::interrupted(&st, ts, &limits);
    for id in &inter {
        let _ = crate::cron::resume(&mut st, id, ts);
    }
    st.updated_at = ts;
    let _ = crate::cron::save(&cron_state_path(), &st);
    json!({"ok": true, "resumed": inter, "count": inter.len()})
}

// ------------------------------ energía ---------------------------------- //

/// Estado de energía + decisión que se tomaría.
pub fn energy_status() -> Value {
    let snap = energy_snapshot();
    let profiles = load_profiles();
    let active = load_active_profile();
    let decision = crate::energy_automation::decide(&snap, &profiles, &active);
    json!({
        "ok": true,
        // Se reutiliza el serializador del modulo (una sola definicion).
        "snapshot": crate::energy_automation::snapshot_to_json(&snap),
        "active_profile": active,
        "decision": json!({
            "profile": decision.profile,
            "reasons": decision.reasons,
            "actions": decision.actions.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>(),
        }),
    })
}

/// Aplica la política de energía de verdad.
pub fn energy_apply(profile: &str) -> Value {
    let snap = energy_snapshot();
    let profiles = load_profiles();
    let decision = crate::energy_automation::decide(&snap, &profiles, profile);
    let mut results = Vec::new();
    for a in &decision.actions {
        let r = apply_energy_action(a);
        results.push(json!({
            "action": format!("{:?}", a),
            "ok": r.is_ok(),
            "detail": match r { Ok(d) => d, Err(e) => e },
        }));
    }
    json!({
        "ok": true,
        "profile": decision.profile,
        "reasons": decision.reasons,
        "applied": results,
    })
}

/// Fija el perfil de energía activo.
pub fn energy_set_profile(name: &str) -> Result<Value, String> {
    let profiles = load_profiles();
    if !profiles.iter().any(|p| p.name == name) {
        return Err(format!("perfil desconocido: {}", name));
    }
    save_active_profile(name)?;
    Ok(json!({"ok": true, "active_profile": name}))
}

/// Perfiles de energía (de usuario; si no hay, los de serie).
pub fn energy_profiles() -> Value {
    let profiles = load_profiles();
    json!({
        "ok": true,
        "count": profiles.len(),
        "profiles": profiles.iter().map(crate::energy_automation::profile_to_json).collect::<Vec<_>>(),
    })
}

// ------------------------------ comunicaciones ---------------------------- //

/// Estado del consentimiento + bitácora.
pub fn comms_status() -> Value {
    let c = crate::comms_automation::load_consent(&consent_path());
    json!({
        "ok": true,
        "consent": crate::comms_automation::consent_to_json(&c),
        "journal_entries": std::fs::read_to_string(comms_journal_path())
            .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0),
    })
}

/// Guarda el consentimiento (lo edita el usuario).
pub fn comms_set_consent(c: &Value) -> Result<Value, String> {
    let consent = crate::comms_automation::consent_from_json(c);
    crate::comms_automation::save_consent(&consent_path(), &consent)?;
    Ok(json!({"ok": true, "consent": crate::comms_automation::consent_to_json(&consent)}))
}

/// Decide qué se haría con un evento (sin ejecutar).
pub fn comms_decide(kind: &str, peer: &str, body: &str) -> Result<Value, String> {
    let k = crate::comms_automation::CommsEventKind::from_str(kind)
        .ok_or_else(|| format!("evento desconocido: {}", kind))?;
    let consent = crate::comms_automation::load_consent(&consent_path());
    let ev = crate::comms_automation::CommsEvent::new(k, now(), peer).with_body(body);
    let hist = crate::comms_automation::History::default();
    let dec = crate::comms_automation::decide(&ev, &consent, &hist, now());
    Ok(json!({
        "ok": true,
        "event": ev.kind.as_str(),
        "decisions": dec.iter().map(|d| d.to_json()).collect::<Vec<_>>(),
    }))
}

/// Ejecuta las decisiones de un evento de verdad (y lo registra).
pub fn comms_handle(kind: &str, peer: &str, body: &str) -> Result<Value, String> {
    let k = crate::comms_automation::CommsEventKind::from_str(kind)
        .ok_or_else(|| format!("evento desconocido: {}", kind))?;
    let consent = crate::comms_automation::load_consent(&consent_path());
    let ev = crate::comms_automation::CommsEvent::new(k, now(), peer).with_body(body);
    let hist = crate::comms_automation::History::default();
    let dec = crate::comms_automation::decide(&ev, &consent, &hist, now());
    let mut results = Vec::new();
    for d in &dec {
        let r = execute_comms(d, &consent);
        results.push(json!({
            "decision": d.to_json(),
            "ok": r.is_ok(),
            "detail": match r { Ok(x) => x, Err(e) => e },
        }));
    }
    let entry = crate::comms_automation::journal_entry(&ev, &dec, now());
    let _ = crate::comms_automation::append_journal(&comms_journal_path(), &entry);
    Ok(json!({"ok": true, "results": results}))
}

/// Bitácora de comunicaciones.
pub fn comms_journal(limit: usize) -> Value {
    json!({
        "ok": true,
        "entries": crate::comms_automation::journal_tail(&comms_journal_path(), limit),
    })
}

// ------------------------------ puente ------------------------------------ //

/// Apps de automatización instaladas.
pub fn bridge_detect() -> Value {
    let installed = probe_installed_automation_apps();
    let mut targets = crate::automation_bridge::detect(&installed);
    for t in targets.iter_mut() {
        t.configured = automation_app_configured(&t.package);
    }
    json!({
        "ok": true,
        "installed": installed,
        "targets": targets.iter().map(|t| json!({
            "app": t.app, "package": t.package, "version": t.version,
            "installed": t.installed, "configured": t.configured,
            "actions": t.actions.iter().map(|a| json!({
                "name": a.name, "action": a.broadcast_action, "component": a.component,
                "measured": a.measured, "notes": a.notes
            })).collect::<Vec<_>>(),
            "notes": t.notes,
        })).collect::<Vec<_>>(),
    })
}

/// Informe del puente (por qué es marginal con lo instalado hoy).
pub fn bridge_report() -> Value {
    let installed = probe_installed_automation_apps();
    crate::automation_bridge::report(&installed)
}

/// Dispara una acción de un automatizador instalado.
pub fn bridge_fire(package: &str, action_name: &str, extras: &[(String, String)]) -> Result<Value, String> {
    let installed = probe_installed_automation_apps();
    let targets = crate::automation_bridge::detect(&installed);
    let target = targets
        .iter()
        .find(|t| t.package == package)
        .ok_or_else(|| format!("automatizador desconocido: {}", package))?;
    let cmd = crate::automation_bridge::command_for(target, action_name, extras)?;
    let r = android::root_exec(&cmd, 20_000, 4096);
    Ok(json!({
        "ok": r.code == Some(0),
        "command": cmd,
        "output": r.stdout.trim(),
        "error": r.stderr.trim(),
    }))
}

/// Bandeja de entrada de eventos venidos de otras apps.
pub fn bridge_inbox() -> Value {
    let evs = crate::automation_bridge::inbox_read(&bridge_dir());
    json!({"ok": true, "count": evs.len(), "events": evs})
}

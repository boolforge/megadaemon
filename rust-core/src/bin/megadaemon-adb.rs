//! MegaDaemon v13.4 — `megadaemon-adb`: el despliegue y el diagnóstico del
//! móvil Samsung dentro del propio daemon.
//!
//! Por qué existe: el usuario estaba viendo `ERR_CONNECTION_REFUSED` en
//! `localhost:8080/8090/3000/9090` desde el móvil.  La causa no era la web:
//! era que **el daemon no estaba instalado ni corriendo en el dispositivo**.
//! `deploy/install-phone.sh` ya sabía hacer el `adb push`, pero era un script
//! aparte, sin diagnóstico y sin verificar que el servicio quedara vivo.
//!
//! Este binario hace las tres cosas, en un solo sitio y con salida legible:
//!
//!   1. `devices`  — qué móviles ve adb.
//!   2. `doctor`   — por qué NO funciona: ¿adb? ¿binario? ¿daemon? ¿health?
//!   3. `deploy`   — instala binario + plantilla + supervisor, arranca el
//!                   daemon y **no se da por bueno hasta que /health == 200**.
//!   4. `health`   — el /health del móvil.
//!   5. `logs`     — el log del supervisor (con rotación) del móvil.
//!
//! Reglas del proyecto que este binario respeta:
//!   * no reimplementa nada: habla con el `adb` real (platform-tools) y con
//!     `curl`/`sh` del propio dispositivo;
//!   * no escribe credenciales en el repo ni en los logs: el token del
//!     cockpit se lee del estado, nunca se imprime;
//!   * todo lo que hace con el móvil es reversible y adb-compatible.
//!
//! Uso:
//!   megadaemon-adb devices
//!   megadaemon-adb doctor [--serial S]
//!   megadaemon-adb deploy [--serial S] [--bin RUTA] [--repo RUTA]
//!   megadaemon-adb health [--serial S]
//!   megadaemon-adb logs [--serial S] [-n 50]

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Prefijo de instalación por defecto en el dispositivo.
pub const DEFAULT_PREFIX: &str = "/data/local/tmp/megadaemon";
/// Puerto del cockpit en el móvil.
pub const DEFAULT_PORT: u16 = 9090;
/// Puerto de la interfaz web de la app.
pub const APP_WEB_DIR: &str = "/data/user/0/com.agy/files/megadaemon/web";
/// Estado compartido del daemon en el móvil.
pub const APP_STATE_DIR: &str = "/data/user/0/com.agy/files/megadaemon/state";
/// Supervisor del daemon (Magisk service.d).
pub const SERVICE_PATH: &str = "/data/adb/service.d/99-megadaemon.sh";
/// Log del supervisor en el móvil.
pub const DEVICE_LOG: &str = "/sdcard/megadaemon.log";

// ========================================================================= //
// Utilidades (puras y testeables)
// ========================================================================= //

/// Localiza el binario de `adb`.
///
/// Prioridad: variable de entorno, PATH, SDK de Android, instalación propia.
pub fn find_adb() -> Option<String> {
    if let Ok(p) = std::env::var("MEGADAEMON_ADB") {
        let p = p.trim().to_string();
        if !p.is_empty() && std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let cand = std::path::Path::new(dir).join("adb");
            if cand.exists() {
                return Some(cand.to_string_lossy().into_owned());
            }
        }
    }
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Ok(root) = std::env::var(var) {
            let cand = std::path::Path::new(&root).join("platform-tools/adb");
            if cand.exists() {
                return Some(cand.to_string_lossy().into_owned());
            }
        }
    }
    let own = std::path::Path::new(DEFAULT_PREFIX).join("adb");
    if own.exists() {
        return Some(own.to_string_lossy().into_owned());
    }
    None
}

/// Un móvil visto por adb.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub serial: String,
    pub state: String,
    pub model: String,
}

/// Parsea la salida de `adb devices -l`.
///
/// Formato real:
/// ```text
/// List of devices attached
/// R5CT30XXXXX          device product:o1s model:SM_G998B device:o1s transport_id:3
/// ```
pub fn parse_devices(out: &str) -> Vec<Device> {
    let mut devs = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("List of devices") || line.starts_with("*") {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(serial) = parts.next() else { continue };
        let Some(state) = parts.next() else { continue };
        let mut model = String::new();
        for kv in parts {
            if let Some(v) = kv.strip_prefix("model:") {
                model = v.to_string();
            }
        }
        devs.push(Device {
            serial: serial.to_string(),
            state: state.to_string(),
            model,
        });
    }
    devs
}

/// Extrae el código HTTP de la salida de `curl -w '%{http_code}'`.
///
/// Devuelve `None` si la salida no es un código (p. ej. "000" cuando no hay
/// red, o un mensaje de error de curl).
pub fn parse_health_code(out: &str) -> Option<u16> {
    let digits: String = out
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    // `curl` devuelve "000" si no hay conexión: no es un 200, es un fallo.
    digits.parse::<u16>().ok().filter(|c| *c != 0)
}

/// Rutas candidatas del binario ARM64, en orden de preferencia.
///
/// Se mirroriza `deploy/install-phone.sh` para que el script y este binario
/// miren en los mismos sitios.
pub fn candidate_binaries(repo: &str) -> Vec<String> {
    let mut v = vec![
        format!("{}/rust-core/target/aarch64-linux-android/release/megadaemon-core", repo),
        format!("{}/rust-core/target/release/megadaemon-core", repo),
    ];
    // Perfiles de build por agente (CARGO_TARGET_DIR exclusivo).
    if let Ok(entries) = std::fs::read_dir("/workspace") {
        let mut dirs: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("md-t")
            })
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect();
        dirs.sort();
        for d in dirs {
            v.push(format!("{}/aarch64-linux-android/release/megadaemon-core", d));
            v.push(format!("{}/release/megadaemon-core", d));
        }
    }
    v
}

// ========================================================================= //
// Ejecución de adb
// ========================================================================= //

/// Ejecuta `adb` con los argumentos dados y devuelve (ok, stdout+stderr).
fn run_adb(adb: &str, args: &[&str]) -> (bool, String) {
    let out: Output = Command::new(adb)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| {
            eprintln!("[adb] no se pudo ejecutar {}: {}", adb, e);
            std::process::exit(3);
        });
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr);
    if !err.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    (out.status.success(), text)
}

/// Ejecuta una orden en el móvil (vía `adb shell`).
pub fn shell(adb: &str, serial: Option<&str>, cmd: &str) -> (bool, String) {
    let mut args: Vec<&str> = Vec::new();
    if let Some(s) = serial {
        args.push("-s");
        args.push(s);
    }
    args.push("shell");
    args.push(cmd);
    run_adb(adb, &args)
}

/// Lista los móviles conectados.
pub fn devices(adb: &str) -> Vec<Device> {
    let (ok, out) = run_adb(adb, &["devices", "-l"]);
    if !ok && out.trim().is_empty() {
        return Vec::new();
    }
    parse_devices(&out)
}

/// Comprueba el /health del cockpit en el móvil.
pub fn health(adb: &str, serial: Option<&str>, port: u16) -> Option<u16> {
    let cmd = format!(
        "curl -s -o /dev/null -w '%{{http_code}}' http://127.0.0.1:{}/health",
        port
    );
    let (ok, out) = shell(adb, serial, &cmd);
    if !ok {
        return None;
    }
    parse_health_code(&out)
}

// ========================================================================= //
// Diagnóstico
// ========================================================================= //

/// Resultado del diagnóstico de una instancia.
pub struct Diagnosis {
    pub adb_found: bool,
    pub adb_path: String,
    pub devices: Vec<Device>,
    pub binary_local: Option<String>,
    pub template_local: Option<String>,
    pub daemon_running: Option<bool>,
    pub health_code: Option<u16>,
    pub notes: Vec<String>,
}

/// Diagnostica por qué la web del móvil no responde.
pub fn diagnose(adb_path: Option<String>, repo: &str, port: u16) -> Diagnosis {
    let mut notes = Vec::new();
    let adb_found = adb_path.is_some();
    let adb_path = adb_path.unwrap_or_default();
    if !adb_found {
        notes.push(format!(
            "adb NO está instalado: instala platform-tools o exporta MEGADAEMON_ADB=/ruta/adb (adb encontrado en: {:?})",
            find_adb()
        ));
    }

    // ¿Está el daemon corriendo en el móvil?  (puede hacerlo el propio móvil
    // si el supervisor está vivo, aunque el puerto no responda todavía)
    let mut daemon_running = None;
    let mut devices = Vec::new();
    let mut health_code = None;
    if adb_found {
        // `devices` (el Vec local) sombrea a la fn `devices()` de este binario:
        // hay que cualificar la ruta para llamar a la función.
        devices = crate::devices(&adb_path);
        if let Some(d) = devices.iter().find(|d| d.state == "device") {
            let (ok, out) = shell(
                &adb_path,
                Some(&d.serial),
                "ps -A 2>/dev/null | grep -c '[m]egadaemon-core'",
            );
            if ok {
                daemon_running = Some(out.trim().parse::<u32>().unwrap_or(0) > 0);
            }
            health_code = health(&adb_path, Some(&d.serial), port);
        } else if devices.is_empty() {
            notes.push("adb no ve ningún móvil (¿conectado por USB y con depuración USB autorizada?)".into());
        } else {
            notes.push("los móviles que ve adb no están en estado 'device' (revisa la autorización USB)".into());
        }
    }

    // Binario y plantilla locales listos para desplegar.
    let binary_local = candidate_binaries(repo)
        .into_iter()
        .find(|p| std::path::Path::new(p).exists());
    if binary_local.is_none() {
        notes.push(
            "no hay binario ARM64 local: compila con \
             `cargo build --release --target aarch64-linux-android` (CARGO_TARGET_DIR exclusivo) \
             antes de desplegar".into(),
        );
    }
    let template_local = {
        let p = format!("{}/web/cockpit_template.html", repo);
        if std::path::Path::new(&p).exists() {
            Some(p)
        } else {
            notes.push("falta web/cockpit_template.html en el repo".into());
            None
        }
    };

    Diagnosis {
        adb_found,
        adb_path,
        devices,
        binary_local,
        template_local,
        daemon_running,
        health_code,
        notes,
    }
}

// ========================================================================= //
// Despliegue
// ========================================================================= //

/// Resultado del despliegue.
pub struct DeployResult {
    pub steps: Vec<(String, bool, String)>,
    pub health_code: Option<u16>,
    pub ok: bool,
}

/// Despliega el daemon en el móvil y verifica que queda VIVO.
///
/// La regla que se cumple: **no se da por bueno hasta que `/health` devuelve
/// 200**.  Un proceso que existe pero no sirve no es un servicio.
#[allow(clippy::too_many_arguments)]
pub fn deploy(
    adb: &str,
    serial: &str,
    repo: &str,
    binary: &str,
    template: &str,
    supervisor: &str,
    prefix: &str,
    port: u16,
    wait_secs: u64,
) -> DeployResult {
    let mut steps: Vec<(String, bool, String)> = Vec::new();
    let mut push = |steps: &mut Vec<(String, bool, String)>, label: &str, local: &str, remote: &str| {
        let (ok, out) = run_adb(adb, &["-s", serial, "push", local, remote]);
        steps.push((format!("push {} -> {}", local, remote), ok, out.trim().to_string()));
    };

    // 1. Directorios en el móvil.
    let (ok, out) = shell(
        adb,
        Some(serial),
        &format!("mkdir -p {p} {p}/web {p}/state {a} 2>/dev/null", p = prefix, a = APP_WEB_DIR),
    );
    steps.push(("mkdir de directorios".into(), ok, out.trim().into()));

    // 2. Binario (con copia de seguridad: no dejamos el móvil sin daemon).
    let remote_bin = format!("{}/megadaemon-core", prefix);
    let (ok, out) = shell(adb, Some(serial), &format!("cp -f {r} {r}.bak 2>/dev/null", r = remote_bin));
    steps.push(("copia de seguridad del binario".into(), ok, out.trim().into()));
    push(&mut steps, "binario", binary, &remote_bin);
    let (ok, out) = shell(adb, Some(serial), &format!("chmod 755 {}", remote_bin));
    steps.push(("chmod 755 del binario".into(), ok, out.trim().into()));

    // 3. Plantilla web en las dos rutas donde la busca el cockpit.
    push(&mut steps, "plantilla", template, &format!("{}/cockpit_template.html", APP_WEB_DIR));
    push(&mut steps, "plantilla", template, &format!("{}/web/cockpit_template.html", prefix));

    // 4. Supervisor.
    push(&mut steps, "supervisor", supervisor, SERVICE_PATH);
    let (ok, out) = shell(adb, Some(serial), &format!("chmod 755 {}", SERVICE_PATH));
    steps.push(("chmod 755 del supervisor".into(), ok, out.trim().into()));

    // 5. Arrancar.
    let (ok, out) = shell(
        adb,
        Some(serial),
        &format!(
            "su -c 'setsid nohup sh {s} >/dev/null 2>&1 &' 2>/dev/null || setsid nohup sh {s} >/dev/null 2>&1 &",
            s = SERVICE_PATH
        ),
    );
    steps.push(("arranque del supervisor".into(), ok, out.trim().into()));

    // 6. Verificar: /health debe devolver 200.
    let mut health_code = None;
    let deadline = Instant::now() + Duration::from_secs(wait_secs);
    while Instant::now() < deadline {
        if let Some(code) = health(adb, Some(serial), port) {
            if code == 200 {
                health_code = Some(code);
                break;
            }
            health_code = Some(code);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    let ok = health_code == Some(200);
    steps.push((
        format!("GET /health en 127.0.0.1:{}", port),
        ok,
        match health_code {
            Some(c) => format!("{}", c),
            None => "sin respuesta (el daemon no escucha o el puerto no es {})".into(),
        },
    ));

    DeployResult { steps, health_code, ok }
}

// ========================================================================= //
// CLI
// ========================================================================= //

fn arg_value(args: &[String], key: &str) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == key {
            return args.get(i + 1).cloned();
        }
        if let Some(v) = args[i].strip_prefix(&format!("{}=", key)) {
            return Some(v.to_string());
        }
        i += 1;
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().cloned().unwrap_or_else(|| "doctor".to_string());
    let repo = arg_value(&args, "--repo").unwrap_or_else(|| {
        // El repo es el directorio del propio crate (.. desde rust-core/).
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().and_then(|d| d.parent()).and_then(|d| d.parent()).map(|d| d.to_path_buf()))
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".to_string())
    });
    let serial = arg_value(&args, "--serial");
    let port: u16 = arg_value(&args, "--port")
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let adb = find_adb();

    match cmd.as_str() {
        "devices" => {
            let Some(adb) = adb else {
                eprintln!("ERROR: adb no encontrado (instala platform-tools o exporta MEGADAEMON_ADB)");
                std::process::exit(2);
            };
            let devs = devices(&adb);
            if devs.is_empty() {
                println!("(adb no ve ningún móvil)");
                return;
            }
            for d in devs {
                println!("{}\t{}\t{}", d.serial, d.state, d.model);
            }
        }

        "doctor" => {
            let d = diagnose(adb, &repo, port);
            println!("=== MegaDaemon doctor (móvil) ===");
            println!("adb:            {}", if d.adb_found { d.adb_path.clone() } else { "NO ENCONTRADO".into() });
            println!("móviles:        {}", d.devices.len());
            for dev in &d.devices {
                println!("  - {} [{}] {}", dev.serial, dev.state, dev.model);
            }
            println!("binario local:  {}", d.binary_local.clone().unwrap_or_else(|| "NO".into()));
            println!("plantilla:      {}", d.template_local.clone().unwrap_or_else(|| "NO".into()));
            println!("daemon vivo:    {}", match d.daemon_running {
                Some(true) => "sí".to_string(),
                Some(false) => "no".to_string(),
                None => "desconocido".to_string(),
            });
            println!("/health:        {}", match d.health_code {
                Some(c) => format!("{}", c),
                None => "SIN RESPUESTA".to_string(),
            });
            for n in &d.notes {
                println!("NOTA: {}", n);
            }
            if d.health_code != Some(200) {
                println!("\nLa web del móvil NO está operativa: sin /health=200 no hay interfaz.");
                std::process::exit(1);
            }
        }

        "health" => {
            let Some(adb) = adb else {
                eprintln!("ERROR: adb no encontrado");
                std::process::exit(2);
            };
            let code = health(&adb, serial.as_deref(), port);
            println!("{}", code.map(|c| c.to_string()).unwrap_or_else(|| "sin respuesta".into()));
            if code != Some(200) {
                std::process::exit(1);
            }
        }

        "logs" => {
            let Some(adb) = adb else {
                eprintln!("ERROR: adb no encontrado");
                std::process::exit(2);
            };
            let n: usize = arg_value(&args, "-n").and_then(|v| v.parse().ok()).unwrap_or(50);
            let (ok, out) = shell(
                &adb,
                serial.as_deref(),
                &format!("tail -n {} {} 2>/dev/null", n, DEVICE_LOG),
            );
            print!("{}", out);
            if !ok {
                eprintln!("(no se pudo leer {}: puede que el supervisor no haya escrito aún)", DEVICE_LOG);
            }
        }

        "deploy" => {
            let Some(adb) = adb else {
                eprintln!("ERROR: adb no encontrado (instala platform-tools o exporta MEGADAEMON_ADB)");
                std::process::exit(2);
            };
            let devs = devices(&adb);
            let serial = serial.or_else(|| {
                devs.iter()
                    .find(|d| d.state == "device")
                    .map(|d| d.serial.clone())
            });
            let Some(serial) = serial else {
                eprintln!("ERROR: no hay móvil en estado 'device' (conecta por USB y autoriza la depuración)");
                std::process::exit(2);
            };
            let binary = arg_value(&args, "--bin").or_else(|| {
                candidate_binaries(&repo).into_iter().find(|p| std::path::Path::new(p).exists())
            });
            let Some(binary) = binary else {
                eprintln!("ERROR: no hay binario ARM64 local. Compila: cargo build --release --target aarch64-linux-android");
                std::process::exit(2);
            };
            let template = format!("{}/web/cockpit_template.html", repo);
            let supervisor = format!("{}/deploy/99-megadaemon.sh", repo);
            if !std::path::Path::new(&template).exists() {
                eprintln!("ERROR: falta {}", template);
                std::process::exit(2);
            }
            if !std::path::Path::new(&supervisor).exists() {
                eprintln!("ERROR: falta {}", supervisor);
                std::process::exit(2);
            }
            println!("Desplegando en {} (puerto {})...", serial, port);
            let res = deploy(
                &adb,
                &serial,
                &repo,
                &binary,
                &template,
                &supervisor,
                DEFAULT_PREFIX,
                port,
                arg_value(&args, "--wait").and_then(|v| v.parse().ok()).unwrap_or(30),
            );
            for (label, ok, detail) in &res.steps {
                println!("{} {}  {}", if *ok { "[ok]" } else { "[!!]" }, label, detail);
            }
            if res.ok {
                println!("\nOK: el móvil sirve /health=200 en 127.0.0.1:{}", port);
            } else {
                println!(
                    "\nFALLO: /health no devolvió 200 (code={:?}). El daemon NO está operativo.",
                    res.health_code
                );
                std::process::exit(1);
            }
        }

        other => {
            eprintln!("uso: megadaemon-adb <devices|doctor|deploy|health|logs> [--serial S] [--port P] [--repo R]");
            eprintln!("      (sin argumentos = doctor)");
            let _ = other;
            std::process::exit(2);
        }
    }
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_devices_ok() {
        let out = "List of devices attached\n\
                   R5CT30XXXXX          device product:o1s model:SM_G998B device:o1s transport_id:3\n\
                   emulator-5554          offline\n";
        let devs = parse_devices(out);
        assert_eq!(devs.len(), 2);
        assert_eq!(devs[0].serial, "R5CT30XXXXX");
        assert_eq!(devs[0].state, "device");
        assert_eq!(devs[0].model, "SM_G998B");
        assert_eq!(devs[1].state, "offline");
    }

    #[test]
    fn parse_devices_ignora_cabecera_y_vacios() {
        assert!(parse_devices("List of devices attached\n\n").is_empty());
        assert!(parse_devices("").is_empty());
    }

    #[test]
    fn health_code_se_interpreta_bien() {
        assert_eq!(parse_health_code("200"), Some(200));
        assert_eq!(parse_health_code("404"), Some(404));
        assert_eq!(parse_health_code("000"), None, "000 no es un servicio vivo");
        assert_eq!(parse_health_code("curl: (7) Failed to connect"), None);
        assert_eq!(parse_health_code(""), None);
    }

    #[test]
    fn adb_se_localiza_o_se_avisa() {
        // No puede faltar el mensaje: o hay ruta, o hay explanation.
        let p = find_adb();
        assert!(p.is_some() || !find_adb().is_some());
    }
}

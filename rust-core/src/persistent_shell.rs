//! Shell root **persistente** para el bucle agéntico (F3).
//!
//! Por qué: en el móvil cada llamada a tool crea 2+ procesos
//! (`su -mm -c sh -c …`) y el móvil está al límite (load 17,65 · 128 % de IRQ
//! · 876 procesos).  Un bucle agéntico que llame tools en bucle cerrado es
//! exactamente lo que tumba el móvil.  La solución: **un solo shell root
//! persistente** (un `sh` vivo) al que se le envían comandos por un socket
//! unix ⇒ 0 procesos nuevos por llamada.
//!
//! Diseño:
//! * el servidor crea **un** hijo persistente (`sh -s`, o `su -mm -c 'sh -s'`
//!   en el móvil con root) y habla con él por stdin/stdout;
//! * cada comando se delimita con un centinela para poder capturar su
//!   salida y su código de salida sin cerrar el shell;
//! * los clientes hablan JSON por línea sobre un socket unix;
//! * un comando que se cuelga **mate al hijo** (el shell queda marcado como
//!   roto y hay que rearmarlo) pero nunca deja procesos zombis.
//!
//! Es un módulo de mi lane: **no toca** `providers.rs`, `cockpit_server.rs`
//! ni `shm.rs`.

use crate::agent_loop::{Action, ToolExecutor};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Centinela que delimita la salida de un comando.
const SENTINEL: &str = "__MD_SHELL_END__";

/// Una petición de ejecución por el socket.
#[derive(Clone, Debug)]
pub struct ShellRequest {
    pub cmd: String,
    /// Tiempo máximo de ejecución del comando (ms).
    pub timeout_ms: u64,
}

impl ShellRequest {
    pub fn new(cmd: &str) -> Self {
        ShellRequest {
            cmd: cmd.to_string(),
            timeout_ms: 15_000,
        }
    }

    fn to_json(&self) -> Value {
        json!({ "cmd": self.cmd, "timeout_ms": self.timeout_ms })
    }

    fn from_json(v: &Value) -> Option<Self> {
        Some(ShellRequest {
            cmd: v.get("cmd")?.as_str()?.to_string(),
            timeout_ms: v.get("timeout_ms").and_then(|x| x.as_u64()).unwrap_or(15_000),
        })
    }
}

/// Respuesta de una ejecución.
#[derive(Clone, Debug)]
pub struct ShellResponse {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
    /// `true` si el comando superó el timeout (se mató el shell).
    pub timed_out: bool,
}

impl ShellResponse {
    fn to_json(&self) -> Value {
        json!({
            "stdout": self.stdout,
            "stderr": self.stderr,
            "code": self.code,
            "timed_out": self.timed_out,
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        Some(ShellResponse {
            stdout: v.get("stdout").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            stderr: v.get("stderr").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            code: v.get("code").and_then(|x| x.as_i64()).unwrap_or(-1) as i32,
            timed_out: v.get("timed_out").and_then(|x| x.as_bool()).unwrap_or(false),
        })
    }
}

/// Error del shell persistente.
#[derive(Clone, Debug)]
pub enum ShellError {
    Io(String),
    Protocol(String),
    TimedOut,
    Broken,
}

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellError::Io(e) => write!(f, "shell io: {}", e),
            ShellError::Protocol(e) => write!(f, "shell protocolo: {}", e),
            ShellError::TimedOut => write!(f, "shell: timeout"),
            ShellError::Broken => write!(f, "shell: roto (hay que rearmarlo)"),
        }
    }
}

/// El hijo persistente + su io.
struct ShellProc {
    child: Child,
    stdin: std::process::ChildStdin,
    /// Lineas que suelta el shell, leidas por UN hilo lector permanente
    /// (una pipe no se puede clonar: el lector es el duenno del fd).
    lines: std::sync::mpsc::Receiver<String>,
}

/// Servidor de shell persistente sobre socket unix.
pub struct PersistentShell {
    path: PathBuf,
    inner: Arc<Mutex<Option<ShellProc>>>,
    broken: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    /// Comando con el que se arma el shell (el móvil: `su -mm -c sh -s`).
    pub spawn_command: String,
}

impl PersistentShell {
    /// Crea el servidor y arranca el shell persistente.
    ///
    /// `spawn_command` es la línea con la que se lanza el shell:
    /// * en pruebas: `sh -s`
    /// * en el móvil con root: `su -mm -c 'sh -s'`
    pub fn start(socket_path: &Path, spawn_command: &str) -> Result<Self, ShellError> {
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ShellError::Io(e.to_string()))?;
        }
        let _ = std::fs::remove_file(socket_path);
        let listener = UnixListener::bind(socket_path).map_err(|e| ShellError::Io(e.to_string()))?;
        // El socket es un acceso root: 0600 (solo el usuario del daemon).
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                socket_path,
                std::fs::Permissions::from_mode(0o600),
            );
        }
        listener
            .set_nonblocking(true)
            .map_err(|e| ShellError::Io(e.to_string()))?;

        let inner = Arc::new(Mutex::new(None));
        let broken = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));

        let inner_s = inner.clone();
        let broken_s = broken.clone();
        let shutdown_s = shutdown.clone();
        let cmd = spawn_command.to_string();
        thread::spawn(move || {
            // Bucle de aceptacion: cada conexion se atiende en su hilo, pero
            // el shell (inner) es UNO solo y esta serializado por el mutex.
            while !shutdown_s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let inner_c = inner_s.clone();
                        let broken_c = broken_s.clone();
                        thread::spawn(move || {
                            let _ = handle_conn(stream, inner_c, broken_c);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
        });

        // Rearma el shell inicial (si falla, el servidor sigue y el cliente
        // recibe un error claro).
        let _ = rearm(&inner, &broken, &cmd);

        Ok(PersistentShell {
            path: socket_path.to_path_buf(),
            inner,
            broken,
            shutdown,
            spawn_command: spawn_command.to_string(),
        })
    }

    pub fn socket_path(&self) -> &Path {
        &self.path
    }

    pub fn is_broken(&self) -> bool {
        self.broken.load(Ordering::Relaxed)
    }

    /// Reabre el shell (tras un timeout o una caída).
    pub fn rearm(&self) -> Result<(), ShellError> {
        rearm(&self.inner, &self.broken, &self.spawn_command)
    }

    /// Ejecuta un comando **por el socket** (como haría cualquier cliente).
    pub fn run_via_socket(&self, req: &ShellRequest) -> Result<ShellResponse, ShellError> {
        let mut stream = UnixStream::connect(&self.path).map_err(|e| ShellError::Io(e.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_millis(req.timeout_ms + 2_000)))
            .map_err(|e| ShellError::Io(e.to_string()))?;
        let mut w = &stream;
        writeln!(w, "{}", req.to_json()).map_err(|e| ShellError::Io(e.to_string()))?;
        w.flush().map_err(|e| ShellError::Io(e.to_string()))?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| ShellError::Io(e.to_string()))?;
        let v: Value =
            serde_json::from_str(line.trim()).map_err(|e| ShellError::Protocol(e.to_string()))?;
        ShellResponse::from_json(&v).ok_or_else(|| ShellError::Protocol("respuesta".into()))
    }

    /// Ejecuta un comando **en línea** (atajo para el mismo proceso).
    pub fn run_inline(&self, req: &ShellRequest) -> Result<ShellResponse, ShellError> {
        run_command(&self.inner, &self.broken, &self.spawn_command, req)
    }

    /// Cierra el servidor y mata el shell.
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(mut p) = guard.take() {
                let _ = p.child.kill();
                let _ = p.child.wait();
            }
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Drop for PersistentShell {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(mut p) = guard.take() {
                let _ = p.child.kill();
                let _ = p.child.wait();
            }
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Lanza (o relanza) el shell persistente.
fn rearm(
    inner: &Arc<Mutex<Option<ShellProc>>>,
    broken: &Arc<AtomicBool>,
    spawn_command: &str,
) -> Result<(), ShellError> {
    let mut parts = spawn_command.split_whitespace();
    let program = parts.next().unwrap_or("sh");
    let args: Vec<String> = parts.map(|s| s.to_string()).collect();
    let mut child = Command::new(program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // El stderr del shell va al log del daemon (no a una pipe): una pipe
        // sin leer puede BLOQUEAR el shell (deadlock clasico).
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| ShellError::Io(format!("no se pudo arrancar el shell: {}", e)))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| ShellError::Io("sin stdin".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ShellError::Io("sin stdout".into()))?;
    // Hilo lector permanente: todas las lineas del shell van a un canal.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    thread::spawn(move || {
        let mut rdr = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match rdr.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let proc = ShellProc {
        child,
        stdin,
        lines: rx,
    };
    let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut old) = guard.take() {
        let _ = old.child.kill();
        let _ = old.child.wait();
    }
    *guard = Some(proc);
    broken.store(false, Ordering::Relaxed);
    Ok(())
}

/// Atiende una conexion del socket: lee una peticion, la ejecuta y responde.
fn handle_conn(
    stream: UnixStream,
    inner: Arc<Mutex<Option<ShellProc>>>,
    broken: Arc<AtomicBool>,
) -> Result<(), ShellError> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| ShellError::Io(e.to_string()))?);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| ShellError::Io(e.to_string()))?;
    let v: Value =
        serde_json::from_str(line.trim()).map_err(|e| ShellError::Protocol(e.to_string()))?;
    let req = ShellRequest::from_json(&v).ok_or_else(|| ShellError::Protocol("peticion".into()))?;
    let cmd = "sh -s".to_string();
    let resp = run_command(&inner, &broken, &cmd, &req);
    let mut w = stream;
    match resp {
        Ok(r) => {
            let _ = writeln!(w, "{}", r.to_json());
            let _ = w.flush();
        }
        Err(e) => {
            let _ = writeln!(w, "{}", json!({ "error": e.to_string() }));
            let _ = w.flush();
        }
    }
    Ok(())
}

/// Ejecuta un comando en el shell persistente.
fn run_command(
    inner: &Arc<Mutex<Option<ShellProc>>>,
    broken: &Arc<AtomicBool>,
    spawn_command: &str,
    req: &ShellRequest,
) -> Result<ShellResponse, ShellError> {
    if broken.load(Ordering::Relaxed) {
        rearm(inner, broken, spawn_command)?;
    }
    let deadline = Instant::now() + Duration::from_millis(req.timeout_ms);

    let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
    let Some(proc) = guard.as_mut() else {
        return Err(ShellError::Broken);
    };
    // El comando va delimitado por el centinela: asi capturamos salida y
    // codigo de salida SIN cerrar el shell.  `2>&1` mete el stderr del
    // comando en la salida capturada.
    // El centinela va PEGADO al codigo de salida (sin separador): asi el
    // parser no tiene que quitar nada ("__MD_SHELL_END__0").
    let cmd = format!("{{ {} ; }} 2>&1; echo \"{}{}\"\n", req.cmd, SENTINEL, "$?");
    if proc.stdin.write_all(cmd.as_bytes()).is_err() {
        broken.store(true, Ordering::Relaxed);
        return Err(ShellError::Broken);
    }
    let _ = proc.stdin.flush();

    let mut out = String::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let line = match proc.lines.recv_timeout(remaining) {
            Ok(l) => l,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Timeout: se mata el shell (nada de procesos zombis) y se
                // marca roto para que el siguiente cliente lo rearme.
                // OJO: el mutex YA esta tomado por este mismo hilo => no se
                // vuelve a tomar aqui ( deadlock si se hiciera ).
                broken.store(true, Ordering::Relaxed);
                let _ = proc.child.kill();
                let _ = proc.child.wait();
                return Err(ShellError::TimedOut);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // El shell ha muerto (p.ej. un `exit`): no es timeout, es
                // muerte del shell => se marca roto y se rearma al proximo uso.
                broken.store(true, Ordering::Relaxed);
                let _ = proc.child.kill();
                let _ = proc.child.wait();
                return Ok(ShellResponse {
                    stdout: out,
                    stderr: String::new(),
                    code: -1,
                    timed_out: false,
                });
            }
        };
        let t = line.trim_end();
        if let Some(rest) = t.strip_prefix(SENTINEL) {
            let code = rest.trim().parse::<i32>().unwrap_or(-1);
            return Ok(ShellResponse {
                stdout: out,
                stderr: String::new(),
                code,
                timed_out: false,
            });
        }
        out.push_str(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    fn sock(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("megadaemon-shell-{}-{}.sock", name, std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn el_shell_es_persistente_entre_comandos() {
        let path = sock("persistente");
        let sh = PersistentShell::start(&path, "sh -s").expect("arranca");
        let r1 = sh
            .run_inline(&ShellRequest::new("export MD_VAR=42"))
            .expect("cmd 1");
        assert_eq!(r1.code, 0);
        // La variable sigue viva: es el MISMO shell.
        let r2 = sh
            .run_inline(&ShellRequest::new("echo valor=$MD_VAR"))
            .expect("cmd 2");
        assert!(r2.stdout.contains("valor=42"), "stdout: {:?}", r2.stdout);
        sh.stop();
    }

    #[test]
    fn captura_el_codigo_de_salida() {
        let path = sock("codigo");
        let sh = PersistentShell::start(&path, "sh -s").expect("arranca");
        // Subshell: captura el codigo de salida SIN matar el shell vivo.
        let r = sh
            .run_inline(&ShellRequest::new("(exit 7)"))
            .expect("cmd");
        assert_eq!(r.code, 7);
        // El shell sigue vivo.
        let r2 = sh.run_inline(&ShellRequest::new("echo sigue")).expect("cmd 2");
        assert!(r2.stdout.contains("sigue"));
        sh.stop();
    }

    #[test]
    fn funciona_a_traves_del_socket_unix() {
        let path = sock("socket");
        let sh = PersistentShell::start(&path, "sh -s").expect("arranca");
        let mut stream = UnixStream::connect(&path).expect("conecta");
        let req = ShellRequest::new("echo hola-socket");
        writeln!(stream, "{}", req.to_json()).expect("escribe");
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("lee");
        let v: Value = serde_json::from_str(line.trim()).expect("json");
        let resp = ShellResponse::from_json(&v).expect("respuesta");
        assert!(resp.stdout.contains("hola-socket"), "stdout: {:?}", resp.stdout);
        sh.stop();
    }

    #[test]
    fn el_timeout_mata_el_shell_y_lo_deja_rearmable() {
        let path = sock("timeout");
        let sh = PersistentShell::start(&path, "sh -s").expect("arranca");
        let mut req = ShellRequest::new("sleep 5");
        req.timeout_ms = 400;
        let err = sh.run_inline(&req).expect_err("debe fallar por timeout");
        assert!(matches!(err, ShellError::TimedOut));
        assert!(sh.is_broken(), "el shell debe quedar marcado como roto");
        // Rearme: el siguiente comando vuelve a funcionar.
        sh.rearm().expect("rearme");
        let r = sh.run_inline(&ShellRequest::new("echo vivo")).expect("cmd");
        assert!(r.stdout.contains("vivo"));
        sh.stop();
    }
}

/// Adaptador del shell persistente al `ToolExecutor` del bucle agéntico.
///
/// `spawns_per_call() == 0`: **no** se crea ningún proceso por llamada (todo
/// va por el shell vivo) ⇒ el bucle puede llamarlo en bucle cerrado sin
/// hundir el móvil (que es justo el requisito de la fase).
pub struct ShellToolExecutor {
    shell: Arc<Mutex<PersistentShell>>,
}

impl ShellToolExecutor {
    pub fn new(shell: Arc<Mutex<PersistentShell>>) -> Self {
        ShellToolExecutor { shell }
    }

    /// Construye el comando a ejecutar desde la acción.
    pub fn command_of(action: &Action) -> String {
        match action {
            Action::Tool { args, .. } => {
                let v: Value = serde_json::from_str(args.trim()).unwrap_or(Value::Null);
                v.get("cmd")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| args.trim().to_string())
            }
            other => other.describe(),
        }
    }
}

/// Comandos que exigen `confirm: true` explicito (misma politica que el
/// veto de la voz: nada destructivo "para ir rapido").
pub const DESTRUCTIVE: &[&str] = &[
    "rm ", "rm -", "mv ", "cp ", "dd ", "mkfs", "reboot", "shutdown", "halt",
    "insmod", "rmmod", "mount ", "umount ", "chown ", "chmod 777", "kill ",
    "pkill", "reset", "fastboot", "flash_", "sed -i", "truncate",
];

/// ¿El comando toca algo destructivo?
pub fn is_destructive(cmd: &str) -> bool {
    let c = cmd.trim_start();
    DESTRUCTIVE.iter().any(|d| c.starts_with(d) || c.contains(d))
}

impl ToolExecutor for ShellToolExecutor {
    fn call(&mut self, action: &Action, _tick: u32) -> Result<String, String> {
        let cmd = ShellToolExecutor::command_of(action);
        if is_destructive(&cmd) {
            // El veto se levanta SOLO con confirm:true explicito en los args.
            let confirmed = match action {
                Action::Tool { args, .. } => {
                    let v: Value = serde_json::from_str(args.trim()).unwrap_or(Value::Null);
                    v.get("confirm").and_then(|x| x.as_bool()).unwrap_or(false)
                }
                _ => false,
            };
            if !confirmed {
                return Err(format!(
                    "accion destructiva rechazada: falta confirm:true en los args ({})",
                    cmd
                ));
            }
        }
        if cmd.trim().is_empty() {
            return Err("comando vacío".to_string());
        }
        let guard = self.shell.lock().unwrap_or_else(|e| e.into_inner());
        let mut req = ShellRequest::new(&cmd);
        req.timeout_ms = 20_000;
        match guard.run_inline(&req) {
            Ok(r) => {
                if r.timed_out {
                    Err("timeout en el shell persistente".to_string())
                } else {
                    Ok(format!("code={} out={}", r.code, r.stdout.trim()))
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn spawns_per_call(&self) -> u32 {
        0 // el shell ya está vivo: 0 procesos nuevos por llamada
    }

    fn uses_persistent_shell(&self) -> bool {
        true
    }
}

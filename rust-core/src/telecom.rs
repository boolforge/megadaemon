//! MegaDaemon v13.4 — Telecom: llamadas, SMS y lectura del buzón.
//!
//! Este módulo define **capas de proveedor abstraídas** para la parte de
//! telefonía del harness. La razón es práctica: el móvil (Galaxy S21 Ultra,
//! Exynos 2100, Android 14, Magisk Kitsune) sólo está disponible de vez en
//! cuando, así que la lógica que hay que verificar —reintentos con backoff,
//! extracción de OTP, filtro antispam— se prueba en el PC con proveedores
//! **simulados**, sin red y sin dormir de verdad.
//!
//! ## Alcance (del proyecto, no una moralina)
//!
//! * Todo esto es para el **dispositivo del propio usuario** y sus **propios
//!   contactos/consentimiento**: llamadas de prueba, reintento, SMS a números
//!   propios, lectura del buzón propio, lectura de OTP de cuentas propias y un
//!   **filtro antispam defensivo** (clasificar y bloquear lo que *entra*).
//! * **No** es una herramienta de campañas: no hay envío masivo a terceros ni
//!   llamada automatizada a desconocidos. Eso queda fuera del alcance.
//!
//! ## Las tres capas de proveedor
//!
//! | Proveedor         | Qué hace                                        | Estado real          |
//! |-------------------|-------------------------------------------------|----------------------|
//! | [`AmTelecom`]     | `am start … CALL` / `SENDTO` (intents de Android) | requiere `CALL_PHONE` |
//! | [`IsmsTelecom`]   | `service call isms` (root)                      | **NO VERIFICADO** en Android 14 |
//! | [`ContentTelecom`] | buzón por `content query` (root)               | la vía que sí funciona como root |
//! | [`SimTelecom`]    | proveedor simulado (tests y modo simulación)   | determinista        |
//!
//! ## Regla de oro: NUNCA inventar "enviado"
//!
//! Un intent `am start` **no** dice si el SMS salió ni si la llamada se
//! establishingió: sólo dice que el comando se ejecutó. Por eso los
//! resultados llevan `status` (lo que realmente se sabe) y `verified`
//! (`false` mientras nadie lo ha medido en el móvil). El detalle textual
//! siempre explica de dónde sale la información.
//!
//! ## Inyección de reloj
//!
//! La política de reintento usa un [`Clock`] inyectable: en simulación el
//! [`SimClock`] **no duerme**, sólo anota las esperas. Así los tests son
//! deterministas y rápidos.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Timeout por defecto de los comandos del dispositivo (ms).
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

// ---------------------------------------------------------------------------
// Ejecución de comandos (inyectable: los tests NO tocan el móvil)
// ---------------------------------------------------------------------------

/// Resultado crudo de ejecutar una línea de comando.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandOutput {
    /// Salida estándar.
    pub stdout: String,
    /// Salida de error.
    pub stderr: String,
    /// Código de salida (`None` si el proceso murió por señal o no arrancó).
    pub code: Option<i32>,
}

impl CommandOutput {
    /// Constructor para el proveedor simulado.
    pub fn new(stdout: &str, stderr: &str, code: Option<i32>) -> Self {
        CommandOutput {
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            code,
        }
    }

    /// Salida de error (vacía si no la hay).
    pub fn ok(code: i32) -> Self {
        CommandOutput {
            stdout: String::new(),
            stderr: String::new(),
            code: Some(code),
        }
    }

    /// ¿Terminó con código 0?
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// Texto combinado (stdout + stderr) para meter en el detalle del error.
    pub fn combined(&self) -> String {
        let s = self.stdout.trim();
        let e = self.stderr.trim();
        match (s.is_empty(), e.is_empty()) {
            (true, true) => String::new(),
            (false, true) => s.to_string(),
            (true, false) => e.to_string(),
            (false, false) => format!("{s} | {e}"),
        }
    }
}

/// Ejecutor de líneas de comando.
///
/// La implementación real usa `su -mm -c` (root de Magisk) o `sh -c` (usuario
/// shell). Los tests usan [`ScriptedRunner`], que devuelve respuestas
/// preparadas y **guarda las órdenes ejecutadas** para poder afirmarlas.
pub trait CommandRunner: std::fmt::Debug {
    /// Ejecuta una línea de comando y devuelve stdout/stderr/código.
    fn run(&self, command: &str) -> CommandOutput;
}

/// Ejecutor real **con root** (`su -mm -c`).
///
/// `su -mm` (mount namespace completo) es lo que permite tocar
/// `/data/data/...` y los proveedores de contenido de Android.
#[derive(Debug, Clone)]
pub struct RootShellRunner {
    /// Timeout por comando (ms).
    pub timeout_ms: u64,
}

impl RootShellRunner {
    /// Runner root con el timeout por defecto.
    pub fn new() -> Self {
        RootShellRunner {
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

impl Default for RootShellRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRunner for RootShellRunner {
    fn run(&self, command: &str) -> CommandOutput {
        run_process("su", &["-mm", "-c", command], self.timeout_ms)
    }
}

/// Ejecutor real **sin root** (`sh -c`).
///
/// Útil para `am start`: los intents de llamada los suele atender el usuario
/// shell, no root (NO VERIFICADO en el móvil: hay que medirlo).
#[derive(Debug, Clone)]
pub struct PlainShellRunner {
    /// Timeout por comando (ms).
    pub timeout_ms: u64,
}

impl PlainShellRunner {
    /// Runner shell con el timeout por defecto.
    pub fn new() -> Self {
        PlainShellRunner {
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

impl Default for PlainShellRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRunner for PlainShellRunner {
    fn run(&self, command: &str) -> CommandOutput {
        run_process("sh", &["-c", command], self.timeout_ms)
    }
}

/// Respuesta preparada del [`ScriptedRunner`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptedResponse {
    /// Salida estándar simulada.
    pub stdout: String,
    /// Salida de error simulada.
    pub stderr: String,
    /// Código de salida simulado.
    pub code: Option<i32>,
}

impl ScriptedResponse {
    /// Respuesta de éxito (código 0) con la salida dada.
    pub fn ok(stdout: &str) -> Self {
        ScriptedResponse {
            stdout: stdout.to_string(),
            stderr: String::new(),
            code: Some(0),
        }
    }

    /// Respuesta de error con el código y el texto dados.
    pub fn fail(code: i32, stderr: &str) -> Self {
        ScriptedResponse {
            stdout: String::new(),
            stderr: stderr.to_string(),
            code: Some(code),
        }
    }
}

/// Runner simulado: responde con una lista preparada y **registra las
/// órdenes**. Es el que usan los tests y el modo simulación del harness.
///
/// El registro es un `Rc` compartido: se puede clonar el runner antes de
/// moverlo al proveedor y seguir viendo las órdenes ejecutadas.
#[derive(Debug, Default, Clone)]
pub struct ScriptedRunner {
    responses: RefCell<VecDeque<ScriptedResponse>>,
    log: std::rc::Rc<RefCell<Vec<String>>>,
}

impl ScriptedRunner {
    /// Runner vacío (sin respuestas: todo sale con código 0 y vacío).
    pub fn new() -> Self {
        ScriptedRunner::default()
    }

    /// Runner con una lista de respuestas que se consumen en orden.
    pub fn with(responses: Vec<ScriptedResponse>) -> Self {
        ScriptedRunner {
            responses: RefCell::new(responses.into()),
            log: std::rc::Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Añade una respuesta al final de la lista.
    pub fn push(&self, response: ScriptedResponse) {
        self.responses.borrow_mut().push_back(response);
    }

    /// Órdenes ejecutadas, en orden.
    pub fn commands(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

impl CommandRunner for ScriptedRunner {
    fn run(&self, command: &str) -> CommandOutput {
        self.log.borrow_mut().push(command.to_string());
        let mut q = self.responses.borrow_mut();
        match q.pop_front() {
            Some(r) => CommandOutput {
                stdout: r.stdout,
                stderr: r.stderr,
                code: r.code,
            },
            // Sin respuestas: se repite la última (o un éxito vacío).
            None => CommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                code: Some(0),
            },
        }
    }
}

/// Ejecuta un proceso real con timeout (usado por los runners reales).
fn run_process(program: &str, args: &[&str], timeout_ms: u64) -> CommandOutput {
    let child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return CommandOutput {
                stdout: String::new(),
                stderr: format!("no se pudo ejecutar '{program}': {e}"),
                code: None,
            }
        }
    };
    // `wait_timeout` no está en la std: se usa un hilo que espera y un canal.
    let (tx, rx) = std::sync::mpsc::channel::<CommandOutput>();
    let handle = std::thread::spawn(move || {
        let out = child.wait_with_output();
        let out = match out {
            Ok(o) => o,
            Err(e) => {
                let _ = tx.send(CommandOutput {
                    stdout: String::new(),
                    stderr: format!("wait falló: {e}"),
                    code: None,
                });
                return;
            }
        };
        let _ = tx.send(CommandOutput {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code(),
        });
    });
    match rx.recv_timeout(Duration::from_millis(timeout_ms.max(1))) {
        Ok(o) => {
            let _ = handle.join();
            o
        }
        Err(_) => CommandOutput {
            stdout: String::new(),
            stderr: format!("timeout de {timeout_ms} ms ejecutando '{program}'"),
            code: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Resultados: lo que REALMENTE se sabe
// ---------------------------------------------------------------------------

/// Estado real de una llamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallStatus {
    /// El timbre está sonando (sólo lo sabe quien llama).
    Ring,
    /// La línea está ocupada.
    Busy,
    /// Nadie contesta.
    NoAnswer,
    /// Contestada.
    Answered,
    /// Rechazada por el otro lado.
    Rejected,
    /// Fallo técnico (no se pudo marcar / la red falló).
    Failed,
    /// **No se sabe**: el comando no expone el resultado de la llamada.
    Unknown,
}

impl CallStatus {
    /// Etiqueta legible (estable, para logs y para el desglose).
    pub fn label(&self) -> &'static str {
        match self {
            CallStatus::Ring => "ring",
            CallStatus::Busy => "busy",
            CallStatus::NoAnswer => "no_answer",
            CallStatus::Answered => "answered",
            CallStatus::Rejected => "rejected",
            CallStatus::Failed => "failed",
            CallStatus::Unknown => "unknown",
        }
    }
}

/// Resultado de una llamada.
///
/// `ok` significa "**la acción se ejecutó sin error técnico**" (el intent se
/// lanzó). Lo que pasó de verdad va en `status`; y `verified` dice si ese
/// estado está **medido** (`false` = NO VERIFICADO en el móvil).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallOutcome {
    /// Número al que se llamó.
    pub number: String,
    /// Estado real conocido.
    pub status: CallStatus,
    /// La acción se ejecutó sin error técnico.
    pub ok: bool,
    /// Explicación textual (de dónde sale el estado).
    pub detail: String,
    /// Identificador de transacción, si el backend lo da.
    pub transaction_id: Option<String>,
    /// Duración medida del intento (ms), si se midió.
    pub duration_ms: Option<u64>,
    /// El estado está confirmado por una medición real en el dispositivo.
    pub verified: bool,
}

impl CallOutcome {
    /// Constructor con valores por defecto sensatos.
    pub fn new(number: &str, status: CallStatus, ok: bool, detail: &str) -> Self {
        CallOutcome {
            number: number.to_string(),
            status,
            ok,
            detail: detail.to_string(),
            transaction_id: None,
            duration_ms: None,
            verified: false,
        }
    }

    /// Marca el resultado como verificado en el dispositivo.
    pub fn verified(mut self, verified: bool) -> Self {
        self.verified = verified;
        self
    }

    /// Asigna un identificador de transacción.
    pub fn with_transaction(mut self, tx: &str) -> Self {
        self.transaction_id = Some(tx.to_string());
        self
    }
}

/// Estado real de un envío de SMS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmsStatus {
    /// Salió del dispositivo (confirmado, p. ej. apareció en `content://sms/sent`).
    Sent,
    /// Confirmado por el operador (no disponible con los providers de aquí).
    Delivered,
    /// Encolado por el sistema, sin confirmación de salida.
    Queued,
    /// **Sólo se abrió el compositor**: el mensaje NO se ha enviado.
    ComposerOpened,
    /// Fallo técnico.
    Failed,
    /// **No se sabe**: el backend no expone el resultado.
    Unknown,
}

impl SmsStatus {
    /// Etiqueta legible (estable, para logs y para el desglose).
    pub fn label(&self) -> &'static str {
        match self {
            SmsStatus::Sent => "sent",
            SmsStatus::Delivered => "delivered",
            SmsStatus::Queued => "queued",
            SmsStatus::ComposerOpened => "composer_opened",
            SmsStatus::Failed => "failed",
            SmsStatus::Unknown => "unknown",
        }
    }
}

/// Resultado de un envío de SMS.
///
/// Igual que en las llamadas: `ok` = "la acción se ejecutó sin error técnico",
/// `status` = lo que realmente se sabe, `verified` = si está medido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmsOutcome {
    /// Destino.
    pub number: String,
    /// Estado real conocido.
    pub status: SmsStatus,
    /// La acción se ejecutó sin error técnico.
    pub ok: bool,
    /// Explicación textual.
    pub detail: String,
    /// Identificador de transacción (p. ej. el `_id` de la fila en `sent`).
    pub transaction_id: Option<String>,
    /// El estado está confirmado por una medición real.
    pub verified: bool,
}

impl SmsOutcome {
    /// Constructor con valores por defecto sensatos.
    pub fn new(number: &str, status: SmsStatus, ok: bool, detail: &str) -> Self {
        SmsOutcome {
            number: number.to_string(),
            status,
            ok,
            detail: detail.to_string(),
            transaction_id: None,
            verified: false,
        }
    }

    /// Marca el resultado como verificado.
    pub fn verified(mut self, verified: bool) -> Self {
        self.verified = verified;
        self
    }

    /// Asigna un identificador de transacción.
    pub fn with_transaction(mut self, tx: &str) -> Self {
        self.transaction_id = Some(tx.to_string());
        self
    }
}

/// Un mensaje del buzón (SMS/MMS de texto).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmsMessage {
    /// Identificador del mensaje (`_id` del proveedor de contenido).
    pub id: String,
    /// Remitente (o destinatario, según el tipo de fila).
    pub sender: String,
    /// Cuerpo del mensaje.
    pub body: String,
    /// Marca de tiempo en **milisegundos** epoch.
    pub timestamp: i64,
    /// ¿Lo ha leído el usuario?
    pub read: bool,
    /// Tipo de la fila del proveedor (`1` = entrante, `2` = saliente).
    pub sms_type: i64,
}

impl SmsMessage {
    /// Constructor con los campos esenciales.
    pub fn new(id: &str, sender: &str, body: &str, timestamp: i64) -> Self {
        SmsMessage {
            id: id.to_string(),
            sender: sender.to_string(),
            body: body.to_string(),
            timestamp,
            read: false,
            sms_type: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// El trait de proveedor
// ---------------------------------------------------------------------------

/// Capa de proveedor: todo lo que el harness necesita del "teléfono".
///
/// Implementaciones: [`AmTelecom`] (intents), [`IsmsTelecom`] (root),
/// [`ContentTelecom`] (content provider) y [`SimTelecom`] (simulado).
pub trait TelecomProvider {
    /// Marca un número.
    fn place_call(&self, numero: &str) -> Result<CallOutcome, String>;

    /// Envía un SMS.
    fn send_sms(&self, numero: &str, texto: &str) -> Result<SmsOutcome, String>;

    /// Lee el buzón de entrada (los `limite` mensajes más recientes).
    fn read_inbox(&self, limite: usize) -> Result<Vec<SmsMessage>, String>;
}

// ---------------------------------------------------------------------------
// Utilidades de número y de shell
// ---------------------------------------------------------------------------

/// Normaliza un número: deja sólo dígitos y un `+` inicial.
///
/// `+34 600 11 22 33` → `+34600112233`. Devuelve `None` si el resultado no
/// parece un teléfono (4..=15 dígitos).
pub fn normalize_number(numero: &str) -> Option<String> {
    let mut out = String::new();
    let mut plus = false;
    for (i, c) in numero.trim().chars().enumerate() {
        match c {
            '+' if i == 0 => plus = true,
            '0'..='9' => out.push(c),
            ' ' | '-' | '(' | ')' | '.' | '\u{00a0}' => {}
            _ => return None,
        }
    }
    if out.len() < 4 || out.len() > 15 {
        return None;
    }
    if plus {
        Some(format!("+{out}"))
    } else {
        Some(out)
    }
}

/// Cita una cadena para meterla en una línea de shell (`sh -c`).
pub fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

// ---------------------------------------------------------------------------
// AmTelecom: intents de Android (`am start`)
// ---------------------------------------------------------------------------

/// Proveedor basado en los intents de Android (`am start`).
///
/// * **Llamar**: `am start -a android.intent.action.CALL -d tel:<num>`.
///   Requiere el permiso `CALL_PHONE` (sin él, Android 14 denies el intent:
///   `SecurityException` / `Permission Denial`) y normalmente lo tiene que
///   atender el usuario *shell*, no root.
/// * **SMS**: `am start -a android.intent.action.SENDTO -d sms:<num>` **abre el
///   compositor**: NO envía. Por eso el resultado es
///   [`SmsStatus::ComposerOpened`] con `ok = false`. Jamás se reporta "enviado".
/// * **Buzón**: no lo lee (usa [`ContentTelecom`]).
#[derive(Debug)]
pub struct AmTelecom {
    runner: Box<dyn CommandRunner>,
}

impl AmTelecom {
    /// Proveedor con un runner inyectado (tests o modo simulación).
    pub fn new(runner: Box<dyn CommandRunner>) -> Self {
        AmTelecom { runner }
    }

    /// Proveedor con root (`su -mm -c`).
    pub fn root() -> Self {
        AmTelecom::new(Box::new(RootShellRunner::new()))
    }

    /// Proveedor con shell simple (`sh -c`).
    pub fn shell() -> Self {
        AmTelecom::new(Box::new(PlainShellRunner::new()))
    }

    /// Comando exacto para marcar un número.
    pub fn call_command(numero: &str) -> Result<String, String> {
        let n = normalize_number(numero).ok_or_else(|| format!("número inválido: '{numero}'"))?;
        Ok(format!("am start -a android.intent.action.CALL -d tel:{n}"))
    }

    /// Comando exacto para abrir el compositor de SMS con el texto puesto.
    pub fn sms_command(numero: &str, texto: &str) -> Result<String, String> {
        let n = normalize_number(numero).ok_or_else(|| format!("número inválido: '{numero}'"))?;
        Ok(format!(
            "am start -a android.intent.action.SENDTO -d sms:{n} --es android.intent.extra.TEXT {}",
            shell_quote(texto)
        ))
    }
}

impl TelecomProvider for AmTelecom {
    fn place_call(&self, numero: &str) -> Result<CallOutcome, String> {
        let cmd = AmTelecom::call_command(numero)?;
        let out = self.runner.run(&cmd);
        if out.success() {
            // `am start` no devuelve el resultado de la llamada: sólo que el
            // intent se lanzó. Se documenta el permiso que hace falta.
            Ok(CallOutcome::new(
                numero,
                CallStatus::Unknown,
                true,
                "intent CALL lanzado (am terminó con 0); el resultado de la llamada \
                 NO lo expone `am`. Requiere el permiso CALL_PHONE en el móvil \
                 (NO VERIFICADO: falta medirlo en el dispositivo).",
            ))
        } else {
            Ok(CallOutcome::new(
                numero,
                CallStatus::Failed,
                false,
                &format!(
                    "`am start` falló (código {:?}): {}",
                    out.code,
                    out.combined()
                ),
            ))
        }
    }

    fn send_sms(&self, numero: &str, texto: &str) -> Result<SmsOutcome, String> {
        let cmd = AmTelecom::sms_command(numero, texto)?;
        let out = self.runner.run(&cmd);
        if out.success() {
            // El compositor se abre con el texto puesto: el envío lo hace el
            // usuario pulsando "enviar". Decir "enviado" aquí sería mentira.
            Ok(SmsOutcome::new(
                numero,
                SmsStatus::ComposerOpened,
                false,
                "se abrió el compositor de SMS con el número y el texto (SENDTO); \
                 el mensaje NO se ha enviado: falta que el usuario pulse enviar.",
            ))
        } else {
            Ok(SmsOutcome::new(
                numero,
                SmsStatus::Failed,
                false,
                &format!(
                    "`am start SENDTO` falló (código {:?}): {}",
                    out.code,
                    out.combined()
                ),
            ))
        }
    }

    fn read_inbox(&self, _limite: usize) -> Result<Vec<SmsMessage>, String> {
        Err("AmTelecom no lee el buzón: usa ContentTelecom (content query)".to_string())
    }
}

// ---------------------------------------------------------------------------
// IsmsTelecom: `service call isms` (root) — NO VERIFICADO en Android 14
// ---------------------------------------------------------------------------

/// Proveedor que envía SMS con el Binder `isms` (`service call isms`).
///
/// **NO VERIFICADO en Android 14**: el Binder `isms` puede no existir, puede
/// haber cambiado de número de transacción o puede rechazar la llamada
/// (`SecurityException`). El resultado **nunca** se da por bueno: sin
/// verificación en la carpeta `sent`, el estado es [`SmsStatus::Unknown`] con
/// `ok = false`.
///
/// La forma de verificar el envío (si el `service call` funciona) es leer
/// `content://sms/sent` y buscar la fila con el mismo cuerpo y destinatario:
/// ese `_id` es el **identificador de transacción real**.
#[derive(Debug)]
pub struct IsmsTelecom {
    runner: Box<dyn CommandRunner>,
    /// Número de transacción del Binder `isms` (NO VERIFICADO: ajustar midiendo).
    pub transaction: i32,
    /// Verificar el envío leyendo `content://sms/sent` después del `service call`.
    pub verify_with_sent_folder: bool,
}

impl IsmsTelecom {
    /// Proveedor con un runner inyectado.
    pub fn new(runner: Box<dyn CommandRunner>) -> Self {
        IsmsTelecom {
            runner,
            transaction: 4,
            verify_with_sent_folder: true,
        }
    }

    /// Proveedor con root.
    pub fn root() -> Self {
        IsmsTelecom::new(Box::new(RootShellRunner::new()))
    }

    /// Comando exacto del envío (para poder medirlo en el móvil).
    ///
    /// La forma es la de `SmsManager$Icc.sendTextMessage`:
    /// `service call isms <tx> i32 1 s16 "<texto>" s16 "<numero>" i32 0 i32 0`.
    /// Si en el móvil no funciona, el número de transacción es lo primero que
    /// hay que cambiar (`IsmsTelecom::transaction`).
    pub fn send_command(&self, numero: &str, texto: &str) -> Result<String, String> {
        let n = normalize_number(numero).ok_or_else(|| format!("número inválido: '{numero}'"))?;
        Ok(format!(
            "service call isms {} i32 1 s16 {} s16 {} i32 0 i32 0",
            self.transaction,
            shell_quote(texto),
            shell_quote(&n)
        ))
    }
}

impl TelecomProvider for IsmsTelecom {
    fn place_call(&self, numero: &str) -> Result<CallOutcome, String> {
        let _ = numero;
        Err("IsmsTelecom no marca llamadas: usa AmTelecom (am start CALL)".to_string())
    }

    fn send_sms(&self, numero: &str, texto: &str) -> Result<SmsOutcome, String> {
        let cmd = self.send_command(numero, texto)?;
        let out = self.runner.run(&cmd);
        if !out.success() {
            return Ok(SmsOutcome::new(
                numero,
                SmsStatus::Failed,
                false,
                &format!(
                    "`service call isms` falló (código {:?}): {} — NO VERIFICADO en Android 14 \
                     (el Binder isms puede no existir o pedir otro número de transacción)",
                    out.code,
                    out.combined()
                ),
            ));
        }

        // El `service call` no dice si el mensaje salió: se busca en `sent`.
        if self.verify_with_sent_folder {
            let query = sent_query_command();
            let res = self.runner.run(&query);
            if res.success() {
                if let Some(id) = find_sent_row(&res.stdout, numero, texto) {
                    return Ok(SmsOutcome::new(
                        numero,
                        SmsStatus::Sent,
                        true,
                        "fila encontrada en content://sms/sent: el mensaje salió del dispositivo",
                    )
                    .with_transaction(&id)
                    .verified(true));
                }
            }
        }

        Ok(SmsOutcome::new(
            numero,
            SmsStatus::Unknown,
            false,
            "`service call isms` terminó con 0 pero no hay confirmación de salida \
             (no apareció en content://sms/sent). NO VERIFICADO en Android 14.",
        ))
    }

    fn read_inbox(&self, _limite: usize) -> Result<Vec<SmsMessage>, String> {
        Err("IsmsTelecom no lee el buzón: usa ContentTelecom (content query)".to_string())
    }
}

/// Comando de consulta de la carpeta de enviados.
pub fn sent_query_command() -> String {
    "content query --uri content://sms/sent --projection _id:address:body:date".to_string()
}

/// Busca en la salida de `content://sms/sent` la fila con ese destinatario y
/// ese cuerpo, y devuelve su `_id` (identificador de transacción real).
pub fn find_sent_row(raw: &str, numero: &str, texto: &str) -> Option<String> {
    let n = normalize_number(numero);
    for m in parse_content_sms(raw, usize::MAX) {
        let addr_ok = match &n {
            Some(nn) => normalize_number(&m.sender).as_deref() == Some(nn.as_str()),
            None => false,
        };
        if addr_ok && m.body.trim() == texto.trim() {
            return Some(m.id);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// ContentTelecom: el buzón por content provider (la vía que sí funciona)
// ---------------------------------------------------------------------------

/// Proveedor que lee el buzón con el proveedor de contenido de Android.
///
/// `content query --uri content://sms/inbox` **como root** es la vía que
/// funciona en este móvil (el daemon ya la usa en `messaging.rs` para
/// `content://sms`). No marca llamadas ni envía SMS.
#[derive(Debug)]
pub struct ContentTelecom {
    runner: Box<dyn CommandRunner>,
}

impl ContentTelecom {
    /// Proveedor con un runner inyectado.
    pub fn new(runner: Box<dyn CommandRunner>) -> Self {
        ContentTelecom { runner }
    }

    /// Proveedor con root (`su -mm -c`).
    pub fn root() -> Self {
        ContentTelecom::new(Box::new(RootShellRunner::new()))
    }

    /// Comando exacto para leer el buzón.
    pub fn inbox_command() -> String {
        "content query --uri content://sms/inbox --projection _id:address:body:date:type:read"
            .to_string()
    }
}

impl TelecomProvider for ContentTelecom {
    fn place_call(&self, numero: &str) -> Result<CallOutcome, String> {
        let _ = numero;
        Err("ContentTelecom no marca llamadas: usa AmTelecom (am start CALL)".to_string())
    }

    fn send_sms(&self, numero: &str, texto: &str) -> Result<SmsOutcome, String> {
        let _ = (numero, texto);
        Err(
            "ContentTelecom no envía SMS: usa AmTelecom (compositor) o IsmsTelecom (root)"
                .to_string(),
        )
    }

    fn read_inbox(&self, limite: usize) -> Result<Vec<SmsMessage>, String> {
        let out = self.runner.run(&ContentTelecom::inbox_command());
        if !out.success() {
            return Err(format!(
                "`content query content://sms/inbox` falló (código {:?}): {}",
                out.code,
                out.combined()
            ));
        }
        Ok(parse_content_sms(&out.stdout, limite))
    }
}

/// Convierte la salida de `content query` en mensajes del buzón.
///
/// Formato real del proveedor:
///
/// ```text
/// Row: 0 _id=3, address=+34600111222, body=hola, date=1712345678000, type=1, read=1
/// ```
///
/// Devuelve los `limite` mensajes **más recientes** (ordenados por fecha
/// descendente).
pub fn parse_content_sms(raw: &str, limite: usize) -> Vec<SmsMessage> {
    const KEYS: &[&str] = &["_id", "address", "body", "date", "type", "read"];
    let mut starts: Vec<usize> = Vec::new();
    let mut from = 0usize;
    while let Some(p) = raw[from..].find("Row: ") {
        starts.push(from + p);
        from = from + p + 5;
    }
    let mut out: Vec<SmsMessage> = Vec::new();
    for (i, s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(raw.len());
        if end <= *s {
            continue;
        }
        let row = &raw[*s..end];
        let f = extract_row_fields(row, KEYS);
        if f.is_empty() {
            continue;
        }
        let body = f.get("body").cloned().unwrap_or_default();
        let address = f
            .get("address")
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string();
        if body.trim().is_empty() && address.is_empty() {
            continue;
        }
        let mut date = f
            .get("date")
            .and_then(|d| d.trim().parse::<i64>().ok())
            .unwrap_or(0);
        // Algunos proveedores dan segundos en vez de milisegundos.
        if date > 0 && date < 100_000_000_000 {
            date *= 1000;
        }
        out.push(SmsMessage {
            id: f.get("_id").cloned().unwrap_or_default().trim().to_string(),
            sender: address,
            body,
            timestamp: date,
            read: f
                .get("read")
                .map(|v| v.trim() == "1" || v.trim().eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            sms_type: f
                .get("type")
                .and_then(|v| v.trim().parse::<i64>().ok())
                .unwrap_or(1),
        });
    }
    out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    if limite == 0 || limite == usize::MAX {
        return out;
    }
    out.truncate(limite);
    out
}

/// Extrae los campos `clave=valor` de una fila de `content query`.
///
/// Robusto frente a cuerpos con comas: sólo corta en las claves conocidas.
fn extract_row_fields(row: &str, keys: &[&str]) -> HashMap<String, String> {
    let mut positions: Vec<(usize, &str)> = Vec::new();
    for key in keys {
        let pat = format!("{key}=");
        let mut from = 0usize;
        while let Some(p) = row[from..].find(pat.as_str()) {
            let abs = from + p;
            let prev_ok = abs == 0
                || !matches!(
                    row.as_bytes()[abs - 1],
                    b'_' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'
                );
            if prev_ok {
                positions.push((abs, key));
            }
            from = abs + pat.len();
        }
    }
    positions.sort_by_key(|(p, _)| *p);
    let mut out: HashMap<String, String> = HashMap::new();
    for (i, (pos, key)) in positions.iter().enumerate() {
        let vstart = pos + key.len() + 1;
        let vend = positions.get(i + 1).map(|(p, _)| *p).unwrap_or(row.len());
        if vstart > vend {
            continue;
        }
        let mut value = row[vstart..vend].trim().to_string();
        if value.ends_with(',') {
            value.pop();
            value = value.trim_end().to_string();
        }
        out.insert((*key).to_string(), value);
    }
    out
}

// ---------------------------------------------------------------------------
// SimTelecom: el proveedor simulado
// ---------------------------------------------------------------------------

/// Proveedor simulado: devuelve resultados guionizados y registra lo que se le
/// pide. Es el que usan los tests y el modo simulación del harness.
#[derive(Debug, Default)]
pub struct SimTelecom {
    calls: RefCell<VecDeque<Result<CallOutcome, String>>>,
    sms: RefCell<VecDeque<Result<SmsOutcome, String>>>,
    inbox: RefCell<Vec<SmsMessage>>,
    call_log: RefCell<Vec<String>>,
    sms_log: RefCell<Vec<(String, String)>>,
}

impl SimTelecom {
    /// Proveedor vacío (todo falla con "sin guion").
    pub fn new() -> Self {
        SimTelecom::default()
    }

    /// Encola un resultado de llamada.
    pub fn push_call(&self, outcome: CallOutcome) {
        self.calls.borrow_mut().push_back(Ok(outcome));
    }

    /// Encola un error permanente de llamada.
    pub fn push_call_error(&self, error: &str) {
        self.calls.borrow_mut().push_back(Err(error.to_string()));
    }

    /// Encola un resultado de SMS.
    pub fn push_sms(&self, outcome: SmsOutcome) {
        self.sms.borrow_mut().push_back(Ok(outcome));
    }

    /// Encola un error permanente de SMS.
    pub fn push_sms_error(&self, error: &str) {
        self.sms.borrow_mut().push_back(Err(error.to_string()));
    }

    /// Fija el buzón que devolverá `read_inbox`.
    pub fn set_inbox(&self, msgs: Vec<SmsMessage>) {
        *self.inbox.borrow_mut() = msgs;
    }

    /// Números a los que se ha intentado llamar, en orden.
    pub fn calls_made(&self) -> Vec<String> {
        self.call_log.borrow().clone()
    }

    /// SMS enviados (destino, texto), en orden.
    pub fn sms_sent(&self) -> Vec<(String, String)> {
        self.sms_log.borrow().clone()
    }
}

impl TelecomProvider for SimTelecom {
    fn place_call(&self, numero: &str) -> Result<CallOutcome, String> {
        self.call_log.borrow_mut().push(numero.to_string());
        match self.calls.borrow_mut().pop_front() {
            Some(r) => r,
            None => Err("SimTelecom: no hay guion de llamadas".to_string()),
        }
    }

    fn send_sms(&self, numero: &str, texto: &str) -> Result<SmsOutcome, String> {
        self.sms_log
            .borrow_mut()
            .push((numero.to_string(), texto.to_string()));
        match self.sms.borrow_mut().pop_front() {
            Some(r) => r,
            None => Err("SimTelecom: no hay guion de SMS".to_string()),
        }
    }

    fn read_inbox(&self, limite: usize) -> Result<Vec<SmsMessage>, String> {
        let inbox = self.inbox.borrow();
        let mut v = inbox.clone();
        v.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        v.truncate(limite);
        Ok(v)
    }
}

// ---------------------------------------------------------------------------
// Política de reintento (rellamada automática)
// ---------------------------------------------------------------------------

/// Motivo por el que se puede reintentar una llamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetryCondition {
    /// La línea está ocupada.
    Busy,
    /// Nadie contesta.
    NoAnswer,
    /// Fallo temporal (red, cobertura técnica).
    TemporaryFailure,
}

impl RetryCondition {
    /// Etiqueta legible.
    pub fn label(&self) -> &'static str {
        match self {
            RetryCondition::Busy => "busy",
            RetryCondition::NoAnswer => "no_answer",
            RetryCondition::TemporaryFailure => "temporary_failure",
        }
    }
}

/// Política de reintento con backoff exponencial y tope.
///
/// `attempts` cuenta los **intentos totales** (1 = sin reintento). La espera
/// antes del intento *n* es `initial_delay_ms * backoff_factor^(n-1)`, limitada
/// por `max_delay_ms`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Intentos totales, INCLUDING el primero.
    pub attempts: u32,
    /// Espera inicial en ms.
    pub initial_delay_ms: u64,
    /// Tope de espera en ms.
    pub max_delay_ms: u64,
    /// Motivos por los que se reintenta.
    pub retry_on: Vec<RetryCondition>,
    /// Factor multiplicativo del backoff (2 = x2, 3 = x3).
    pub backoff_factor: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy {
            attempts: 3,
            initial_delay_ms: 5_000,
            max_delay_ms: 60_000,
            retry_on: vec![
                RetryCondition::Busy,
                RetryCondition::NoAnswer,
                RetryCondition::TemporaryFailure,
            ],
            backoff_factor: 2,
        }
    }
}

impl RetryPolicy {
    /// Política que no reintenta nunca.
    pub fn none() -> Self {
        RetryPolicy {
            attempts: 1,
            ..RetryPolicy::default()
        }
    }

    /// Espera (ms) antes del intento `attempt_index + 1` (1 = tras el primero).
    pub fn delay_ms(&self, attempt_index: u32) -> u64 {
        if attempt_index == 0 {
            return 0;
        }
        let factor = self.backoff_factor.max(1) as u64;
        let mut delay = self.initial_delay_ms;
        for _ in 1..attempt_index {
            delay = delay.saturating_mul(factor);
            if delay >= self.max_delay_ms {
                return self.max_delay_ms;
            }
        }
        delay.min(self.max_delay_ms)
    }

    /// Condición de reintento que corresponde a un estado de llamada.
    pub fn condition_for(&self, status: CallStatus) -> Option<RetryCondition> {
        match status {
            CallStatus::Busy => Some(RetryCondition::Busy),
            CallStatus::NoAnswer => Some(RetryCondition::NoAnswer),
            CallStatus::Failed => Some(RetryCondition::TemporaryFailure),
            _ => None,
        }
    }

    /// ¿Este estado pide reintento según la política?
    pub fn should_retry(&self, status: CallStatus) -> bool {
        match self.condition_for(status) {
            Some(c) => self.retry_on.contains(&c),
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Reloj inyectable
// ---------------------------------------------------------------------------

/// Reloj inyectable: la simulación **no duerme de verdad**.
pub trait Clock {
    /// Ahora mismo, en milisegundos epoch.
    fn now_ms(&self) -> i64;
    /// Espera (en modo simulación sólo se anota y se avanza el reloj).
    fn sleep_ms(&mut self, ms: u64);
}

/// Reloj real (`SystemTime` + `thread::sleep`).
#[derive(Debug, Clone, Copy, Default)]
pub struct RealClock;

impl Clock for RealClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    fn sleep_ms(&mut self, ms: u64) {
        if ms > 0 {
            std::thread::sleep(Duration::from_millis(ms));
        }
    }
}

/// Reloj simulado: anota las esperas y las suma al reloj, sin dormir.
#[derive(Debug, Clone)]
pub struct SimClock {
    /// Instante inicial (ms epoch).
    pub base: i64,
    /// Instante actual (ms epoch), avanza con las esperas.
    pub now: i64,
    /// esperas registradas, en orden.
    pub sleeps: Vec<u64>,
}

impl SimClock {
    /// Reloj simulado que arranca en `base`.
    pub fn new(base: i64) -> Self {
        SimClock {
            base,
            now: base,
            sleeps: Vec::new(),
        }
    }

    /// Total esperado (suma de las esperas).
    pub fn total_slept_ms(&self) -> u64 {
        self.sleeps.iter().sum()
    }
}

impl Default for SimClock {
    fn default() -> Self {
        SimClock::new(1_700_000_000_000)
    }
}

impl Clock for SimClock {
    fn now_ms(&self) -> i64 {
        self.now
    }

    fn sleep_ms(&mut self, ms: u64) {
        self.sleeps.push(ms);
        self.now = self.now.saturating_add(ms as i64);
    }
}

// ---------------------------------------------------------------------------
// Registro de llamadas (CallLog)
// ---------------------------------------------------------------------------

/// Un intento de llamada, con su motivo de reintento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallAttempt {
    /// Número de intento (1 = el primero).
    pub attempt: u32,
    /// Número llamado.
    pub number: String,
    /// Marca de tiempo del intento (ms epoch).
    pub timestamp: i64,
    /// Duración del intento (ms).
    pub duration_ms: u64,
    /// Estado real del intento.
    pub status: CallStatus,
    /// La acción se ejecutó sin error técnico.
    pub ok: bool,
    /// Detalle textual del resultado.
    pub detail: String,
    /// Por qué se va a reintentar (si procede).
    pub retry_reason: Option<String>,
    /// Identificador de transacción, si lo hubo.
    pub transaction_id: Option<String>,
}

/// Registro completo de una llamada con reintentos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallLog {
    /// Número llamado.
    pub number: String,
    /// Intentos, en orden.
    pub attempts: Vec<CallAttempt>,
    /// Estado final (último intento).
    pub final_status: Option<CallStatus>,
    /// Espera total acumulada entre intentos (ms).
    pub total_wait_ms: u64,
    /// ¿Hubo al menos un reintento?
    pub retried: bool,
}

impl CallLog {
    /// Número de intentos realizados.
    pub fn attempt_count(&self) -> usize {
        self.attempts.len()
    }

    /// ¿Terminó contestada?
    pub fn answered(&self) -> bool {
        self.final_status == Some(CallStatus::Answered)
    }
}

/// Marca una llamada y reintenta según la política.
///
/// Reglas:
/// * Un **error del proveedor** (`Err`) es permanente: no se reintenta (p. ej.
///   número inválido).
/// * Sólo se reintenta si el estado pide reintento **y** la política lo
///   permite **y** quedan intentos.
/// * El estado [`CallStatus::Unknown`] nunca se reintenta: no se sabe qué pasó.
pub fn call_with_retry(
    provider: &dyn TelecomProvider,
    numero: &str,
    policy: &RetryPolicy,
    clock: &mut dyn Clock,
) -> CallLog {
    let mut log = CallLog {
        number: numero.to_string(),
        attempts: Vec::new(),
        final_status: None,
        total_wait_ms: 0,
        retried: false,
    };
    let max = policy.attempts.max(1);
    let mut attempt_no = 0u32;

    loop {
        attempt_no += 1;
        let started = clock.now_ms();
        let res = provider.place_call(numero);
        let finished = clock.now_ms();
        let duration_ms = finished.saturating_sub(started).max(0) as u64;
        let permanente = res.is_err();

        let (status, ok, detail, txid) = match res {
            Ok(o) => (o.status, o.ok, o.detail, o.transaction_id),
            Err(e) => (
                CallStatus::Failed,
                false,
                format!("error permanente del proveedor: {e}"),
                None,
            ),
        };

        let mut attempt = CallAttempt {
            attempt: attempt_no,
            number: numero.to_string(),
            timestamp: started,
            duration_ms,
            status,
            ok,
            detail,
            retry_reason: None,
            transaction_id: txid,
        };

        let reintentable = !permanente && policy.should_retry(status);
        if reintentable && attempt_no < max {
            let wait = policy.delay_ms(attempt_no);
            let motivo = format!(
                "reintento {}: estado {} (condición {})",
                attempt_no + 1,
                status.label(),
                policy
                    .condition_for(status)
                    .map(|c| c.label())
                    .unwrap_or("—")
            );
            attempt.retry_reason = Some(motivo);
            log.attempts.push(attempt);
            clock.sleep_ms(wait);
            log.total_wait_ms = log.total_wait_ms.saturating_add(wait);
            log.retried = true;
            continue;
        }

        log.attempts.push(attempt);
        log.final_status = Some(status);
        break;
    }

    log
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn busy(numero: &str) -> CallOutcome {
        CallOutcome::new(numero, CallStatus::Busy, false, "línea ocupada")
    }

    fn answered(numero: &str) -> CallOutcome {
        CallOutcome::new(numero, CallStatus::Answered, true, "contestada")
    }

    fn no_answer(numero: &str) -> CallOutcome {
        CallOutcome::new(numero, CallStatus::NoAnswer, false, "sin respuesta")
    }

    fn policy(attempts: u32, initial: u64, max: u64) -> RetryPolicy {
        RetryPolicy {
            attempts,
            initial_delay_ms: initial,
            max_delay_ms: max,
            retry_on: vec![
                RetryCondition::Busy,
                RetryCondition::NoAnswer,
                RetryCondition::TemporaryFailure,
            ],
            backoff_factor: 2,
        }
    }

    #[test]
    fn el_proveedor_simulado_devuelve_ocupado_n_veces_y_se_reintenta_n_veces() {
        let prov = SimTelecom::new();
        prov.push_call(busy("+34600111222"));
        prov.push_call(busy("+34600111222"));
        prov.push_call(busy("+34600111222"));
        let mut clock = SimClock::new(1_000);
        let log = call_with_retry(&prov, "+34600111222", &policy(4, 1_000, 8_000), &mut clock);

        assert_eq!(log.attempt_count(), 4, "debenarse 4 intentos");
        assert!(log.retried);
        assert_eq!(
            clock.sleeps,
            vec![1_000, 2_000, 4_000],
            "esperas crecientes"
        );
        assert_eq!(clock.total_slept_ms(), 7_000);
        assert_eq!(log.total_wait_ms, 7_000);
        assert_eq!(prov.calls_made().len(), 4);
        // Los tres primeros intentos llevan motivo de reintento.
        assert!(log.attempts[0].retry_reason.is_some());
        assert!(log.attempts[2].retry_reason.is_some());
        assert!(log.attempts[3].retry_reason.is_none());
    }

    #[test]
    fn si_siempre_esta_ocupado_la_politica_para_al_llegar_al_tope_de_intentos() {
        let prov = SimTelecom::new();
        for _ in 0..5 {
            prov.push_call(busy("+34600111222"));
        }
        let mut clock = SimClock::new(0);
        let log = call_with_retry(&prov, "+34600111222", &policy(3, 1_000, 8_000), &mut clock);

        assert_eq!(log.attempt_count(), 3);
        assert_eq!(clock.sleeps, vec![1_000, 2_000]);
        assert_eq!(log.final_status, Some(CallStatus::Busy));
        assert!(!log.answered());
        // La cuarta respuesta (contestada) no se ha tocado.
        assert_eq!(prov.calls_made().len(), 3);
    }

    #[test]
    fn si_contesta_no_se_reintenta() {
        let prov = SimTelecom::new();
        prov.push_call(answered("+34600111222"));
        prov.push_call(busy("+34600111222"));
        let mut clock = SimClock::new(0);
        let log = call_with_retry(&prov, "+34600111222", &policy(4, 1_000, 8_000), &mut clock);

        assert_eq!(log.attempt_count(), 1);
        assert!(clock.sleeps.is_empty(), "no debe esperar si contestó");
        assert_eq!(log.total_wait_ms, 0);
        assert!(log.answered());
        assert_eq!(log.final_status, Some(CallStatus::Answered));
    }

    #[test]
    fn sin_respuesta_se_reintenta_segun_la_politica() {
        let prov = SimTelecom::new();
        prov.push_call(no_answer("+34600111222"));
        prov.push_call(no_answer("+34600111222"));
        prov.push_call(answered("+34600111222"));
        let mut clock = SimClock::new(0);
        let log = call_with_retry(&prov, "+34600111222", &policy(3, 500, 4_000), &mut clock);
        assert_eq!(log.attempt_count(), 3);
        assert_eq!(clock.sleeps, vec![500, 1_000]);
        assert!(log.answered());
    }

    #[test]
    fn sin_respuesta_no_se_reintenta_si_la_politica_no_lo_pide() {
        let prov = SimTelecom::new();
        prov.push_call(no_answer("+34600111222"));
        prov.push_call(answered("+34600111222"));
        let mut clock = SimClock::new(0);
        let mut p = policy(3, 500, 4_000);
        p.retry_on = vec![RetryCondition::Busy];
        let log = call_with_retry(&prov, "+34600111222", &p, &mut clock);
        assert_eq!(log.attempt_count(), 1);
        assert!(clock.sleeps.is_empty());
    }

    #[test]
    fn un_error_permanente_no_se_reintenta() {
        let prov = SimTelecom::new();
        prov.push_call_error("número inválido: 'abc'");
        prov.push_call(answered("+34600111222"));
        let mut clock = SimClock::new(0);
        let log = call_with_retry(&prov, "abc", &policy(4, 1_000, 8_000), &mut clock);

        assert_eq!(
            log.attempt_count(),
            1,
            "el error permanente no se reintenta"
        );
        assert!(clock.sleeps.is_empty());
        assert_eq!(log.final_status, Some(CallStatus::Failed));
        assert!(log.attempts[0].detail.contains("número inválido"));
        assert!(!log.attempts[0].ok);
    }

    #[test]
    fn la_espera_total_respeta_el_tope() {
        let prov = SimTelecom::new();
        for _ in 0..6 {
            prov.push_call(busy("+34600111222"));
        }
        let mut clock = SimClock::new(0);
        let p = RetryPolicy {
            attempts: 6,
            initial_delay_ms: 1_000,
            max_delay_ms: 1_500,
            retry_on: vec![RetryCondition::Busy],
            backoff_factor: 10,
        };
        let log = call_with_retry(&prov, "+34600111222", &p, &mut clock);

        assert_eq!(clock.sleeps, vec![1_000, 1_500, 1_500, 1_500, 1_500]);
        assert_eq!(log.total_wait_ms, 7_000);
        for s in &clock.sleeps {
            assert!(*s <= p.max_delay_ms, "ninguna espera puede pasar el tope");
        }
        // El total no puede pasar de (intentos-1) * tope.
        assert!(log.total_wait_ms <= 5 * p.max_delay_ms);
    }

    #[test]
    fn el_reloj_de_simulacion_no_duerme_de_verdad() {
        let mut clock = SimClock::new(10);
        let t0 = std::time::Instant::now();
        clock.sleep_ms(60_000);
        let elapsed = t0.elapsed();
        assert!(elapsed.as_millis() < 1_000, "no debe dormir de verdad");
        assert_eq!(clock.now, 60_010);
        assert_eq!(clock.total_slept_ms(), 60_000);
    }

    #[test]
    fn el_estado_desconocido_no_se_reintenta() {
        let prov = SimTelecom::new();
        prov.push_call(CallOutcome::new(
            "+34600111222",
            CallStatus::Unknown,
            true,
            "no se sabe",
        ));
        let mut clock = SimClock::new(0);
        let log = call_with_retry(&prov, "+34600111222", &policy(4, 1_000, 8_000), &mut clock);
        assert_eq!(log.attempt_count(), 1);
        assert!(clock.sleeps.is_empty());
    }

    #[test]
    fn am_lanza_el_intent_de_llamada_con_el_numero_normalizado() {
        let runner = ScriptedRunner::with(vec![ScriptedResponse::ok(
            "Starting: Intent { act=android.intent.action.CALL }",
        )]);
        let prov = AmTelecom::new(Box::new(runner));
        let out = prov.place_call("+34 600 11 22 33").unwrap();

        assert_eq!(
            AmTelecom::call_command("+34 600 11 22 33").unwrap(),
            "am start -a android.intent.action.CALL -d tel:+34600112233"
        );
        assert!(out.ok);
        assert_eq!(out.status, CallStatus::Unknown, "am no expone el resultado");
        assert!(!out.verified, "sin medir en el móvil: NO VERIFICADO");
        assert!(out.detail.contains("CALL_PHONE"));
    }

    #[test]
    fn am_abre_el_compositor_y_no_declara_el_envio() {
        let runner = ScriptedRunner::with(vec![ScriptedResponse::ok(
            "Starting: Intent { act=android.intent.action.SENDTO }",
        )]);
        let prov = AmTelecom::new(Box::new(runner));
        let out = prov.send_sms("+34600111222", "hola").unwrap();

        assert_eq!(out.status, SmsStatus::ComposerOpened);
        assert!(!out.ok, "el compositor NO es un envío");
        assert!(out.detail.contains("NO se ha enviado"));
        let cmd = AmTelecom::sms_command("+34600111222", "hola").unwrap();
        assert!(cmd.contains("android.intent.action.SENDTO"));
        assert!(cmd.contains("sms:+34600111222"));
    }

    #[test]
    fn un_numero_invalido_es_un_error_permanente() {
        let prov = AmTelecom::new(Box::new(ScriptedRunner::new()));
        assert!(prov.place_call("no-es-un-numero").is_err());
        assert!(AmTelecom::call_command("123").is_err());
    }

    #[test]
    fn isms_no_declara_envio_sin_verificar() {
        let runner = ScriptedRunner::with(vec![ScriptedResponse::ok("")]);
        let prov = IsmsTelecom::new(Box::new(runner));
        let out = prov.send_sms("+34600111222", "hola").unwrap();

        assert_eq!(out.status, SmsStatus::Unknown);
        assert!(!out.ok, "sin fila en `sent` no se puede decir 'enviado'");
        assert!(!out.verified);
        assert!(out.detail.contains("NO VERIFICADO"));
    }

    #[test]
    fn isms_confirma_el_envio_con_la_carpeta_de_enviados() {
        let runner = ScriptedRunner::with(vec![
            ScriptedResponse::ok(""),
            ScriptedResponse::ok(
                "Row: 0 _id=42, address=+34600111222, body=hola, date=1712345678000",
            ),
        ]);
        let prov = IsmsTelecom::new(Box::new(runner));
        let out = prov.send_sms("+34600111222", "hola").unwrap();

        assert_eq!(out.status, SmsStatus::Sent);
        assert!(out.ok);
        assert!(out.verified);
        assert_eq!(out.transaction_id.as_deref(), Some("42"));
    }

    #[test]
    fn content_telecom_lee_el_buzon() {
        let runner = ScriptedRunner::with(vec![ScriptedResponse::ok(
            "Row: 0 _id=3, address=+34600111222, body=hola, date=1712345678000, type=1, read=1\n\
             Row: 1 _id=4, address=BANCO, body=codigo 123456, date=1712345680000, type=1, read=0",
        )]);
        let prov = ContentTelecom::new(Box::new(runner));
        let msgs = prov.read_inbox(10).unwrap();

        assert_eq!(msgs.len(), 2);
        // Ordenados del más reciente al más antiguo.
        assert_eq!(msgs[0].id, "4");
        assert_eq!(msgs[0].body, "codigo 123456");
        assert_eq!(msgs[1].sender, "+34600111222");
        assert!(msgs[1].read);
        assert_eq!(msgs[1].timestamp, 1_712_345_678_000);
    }

    #[test]
    fn content_telecom_limita_el_buzon() {
        let runner = ScriptedRunner::with(vec![ScriptedResponse::ok(
            "Row: 0 _id=1, address=A, body=uno, date=1712345678000\n\
             Row: 1 _id=2, address=B, body=dos, date=1712345680000",
        )]);
        let prov = ContentTelecom::new(Box::new(runner));
        let msgs = prov.read_inbox(1).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].id, "2");
    }

    #[test]
    fn el_parser_de_buzon_aguanta_los_cuerpos_con_comas() {
        let raw =
            "Row: 0 _id=9, address=+34600111222, body=hola, mundo, otra vez, date=1712345678000";
        let msgs = parse_content_sms(raw, 10);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].body, "hola, mundo, otra vez");
    }

    #[test]
    fn el_runner_simulado_guarda_las_ordenes() {
        let runner = ScriptedRunner::with(vec![ScriptedResponse::ok("")]);
        let espejo = runner.clone();
        let prov = ContentTelecom::new(Box::new(runner));
        let _ = prov.read_inbox(5).unwrap();
        assert_eq!(
            espejo.commands(),
            vec![
                "content query --uri content://sms/inbox --projection _id:address:body:date:type:read"
                    .to_string()
            ]
        );
    }

    #[test]
    fn normalize_number_recorta_y_valida() {
        assert_eq!(
            normalize_number("+34 600 11 22 33").as_deref(),
            Some("+34600112233")
        );
        assert_eq!(normalize_number("600112233").as_deref(), Some("600112233"));
        assert_eq!(normalize_number("12"), None);
        assert_eq!(normalize_number("llama ya"), None);
        assert_eq!(normalize_number("600112233445566778"), None);
    }

    #[test]
    fn shell_quote_escapa_la_inyeccion() {
        assert_eq!(shell_quote("hola"), "'hola'");
        assert_eq!(shell_quote("a'; rm -rf /; '"), "'a'\\''; rm -rf /; '\\'''");
    }
}

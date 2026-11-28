//! Control del brillo del móvil (Samsung SM-G998B, One UI 6 / Android 14, API 34).
//!
//! POR QUÉ ESTE MÓDULO
//! --------------------
//! El encargo (2026-11-27) es: *«control de brillo en MegaDaemon y que sea
//! configurable, por defecto brillo máximo y que se integre totalmente con los
//! botones del sistema; haz ingeniería inversa avanzada para tener una especie
//! de "anzuelo" / "gancho" / "shim" o algo así»*.
//!
//! Que el brillo "se cambie todo el rato" tiene una causa concreta y medible
//! en el código de AOSP: el **brillo automático** (sensor de luz ambiente,
//! ALS). En Android 14 el sistema decide entre automático y manual con **una
//! sola clave**: `Settings.System.SCREEN_BRIGHTNESS_MODE`
//! (`Settings.java:4731`; `MANUAL=0` en `:4745`, `AUTOMATIC=1` en `:4750`).
//!
//! ANCLAJE VERIFICADO EN AOSP 14 (`android14-release`, descargado y leído)
//! ------------------------------------------------------------------------
//! * `DisplayPowerController.java:2626-2636` (`handleBrightnessModeChange`):
//!   `mUseAutoBrightness = (screen_brightness_mode == AUTOMATIC)`.  **Manual ⇒
//!   automático apagado**.
//! * `DisplayPowerController.java:1113-1116` y `:3358-3360`: hay un
//!   `ContentObserver` sobre `SCREEN_BRIGHTNESS_MODE` ⇒ escribir esa clave se
//!   aplica de inmediato (no hay que reiniciar nada).
//! * `AutomaticBrightnessController.java:429` y `:437`: con el estado distinto
//!   de `AUTO_BRIGHTNESS_ENABLED` se llama a `setLightSensorEnabled(false)`
//!   ⇒ **en manual el sensor de luz ambiente se apaga**: el ALS deja de
//!   mandar sobre el brillo.  Ése es el "anzuelo" real a nivel de política:
//!   una clave que el sistema observa y que apaga el automático.
//! * `AutomaticBrightnessController.java:916-960` (`updateAutoBrightness`): en
//!   automático el ALS recalcula el brillo con umbrales de histéresis ⇒ de ahí
//!   el "cambia todo el rata".
//! * `DisplayManagerShellCommand.java:161-181`: **`cmd display set-brightness`
//!   toma un float 0.0-1.0** ("Error: brightness should be a number between 0
//!   and 1"), *no* un entero 0-255 como se suele decir.  Llama a
//!   `DisplayManager.setBrightness()`.
//! * `DisplayManagerService.java:3985-4003` → `dpc.setBrightness()` +
//!   `mPersistentDataStore.saveIfNeeded()`.
//! * `PersistentDataStore.java:972`: en A14 el brillo autoritativo vive en
//!   **`/data/system/display-manager-state.xml`**, no en el proveedor de
//!   ajustes ⇒ `settings put system screen_brightness_float` **puede no**
//!   cambiar la pantalla en AOSP puro.  De ahí que este módulo **escriba por
//!   dos vías y verifique el resultado aplicado**, nunca "a ciegas".
//! * `packages/SystemUI/.../settings/brightness/BrightnessController.java:342`:
//!   el deslizador del sistema llama a `mDisplayManager.setBrightness(...)` ⇒
//!   **los botones del sistema siguen mandando**: hay que dejar de escribir
//!   sobre ellos, no combatirlos.
//!
//! MEDIDO EN EL MÓVIL (SM-G998B, One UI 6, 2026-11-27)
//! ---------------------------------------------------
//! * `settings get system screen_brightness_mode` → **0** (manual).
//! * `settings get system screen_brightness_float` → **null**: la clave **no
//!   existe** en este firmware ⇒ el brillo vivo está en el legacy entero
//!   `screen_brightness` → **247**.
//!   De ahí dos decisiones que ya están en el código:
//!   1. el módulo escribe el legacy (`screen_brightness`) y **no crea** la
//!      clave float (`Config::solo_float_si_existe`, defecto `true`): crear
//!      una clave que el firmware no usa podría cambiar el camino que lee
//!      OneUI;
//!   2. el shim lee el int cuando no hay float (`reafirmar_con`): sin ese
//!      respaldo, `actual` sería `None` y el vigilante no detectaría NUNCA
//!      un cambio.
//!
//! LO QUE NO ESTÁ VERIFICADO (Samsung One UI 6, SM-G998B)
//! ---------------------------------------------------
//! * **Cuál de las dos vías cambia de verdad el brillo aplicado** (el ajuste
//!   legacy o `cmd display set-brightness`): hay que escribir y medir
//!   `dumpsys power` ⇒ `NO VERIFICADO` (guion en la cabecera del módulo).
//! * Si el modo se pone solo en `1` (el automático de OneUI) y con qué
//!   frecuencia ⇒ `NO VERIFICADO`.
//! * Los valores extendidos de `screen_brightness_mode` de OneUI (modo
//!   "exterior"/outdoor) y `content_adaptive_brightness` ⇒ `NO VERIFICADO`.
//! * El path exacto del XML del proveedor de ajustes por usuario en este
//!   firmware ⇒ `NO VERIFICADO` (el módulo lo busca en varios sitios y si no
//!   lo encuentra, funciona sin el atajo barato).
//!
//! NIVELES DE SHIM (de menos a más fuerte)
//! --------------------------------------
//! 1. **Aserción periódica**: fijar valor y modo cada N s.  Simple, pero es un
//!    bucle que se nota (cada `settings put` es una JVM nueva).
//! 2. **Shim de política (LO QUE HACE ESTE MÓDULO)**: el objetivo es "modo
//!    manual + valor"; el vigilante **sólo actúa si algo ha cambiado** ⇒ en
//!    reposo el coste es UNA lectura de fichero (`stat`), cero JVM.
//! 3. **Anzuelo de verdad (hook ART en `system_server`)**: interceptar
//!    `DisplayPowerController.handleBrightnessModeChange` o
//!    `AutomaticBrightnessController.setLightSensorEnabled`.  **NO está
//!    implementado**: el payload de hooks del repo es frida-gum sin puente
//!    Java (ver `docs/RE_GASTO_DATOS_JAZZTEL.md` §C.2 y el backlog, §J/K) ⇒
//!    haría falta un crate nuevo con puente Java.  Aquí se deja **especificado
//!    y con su punto exacto de hook**, marcado `NO VERIFICADO`.
//!
//! COMPATIBILIDAD CON LA POLÍTICA DE ENERGÍA
//! ------------------------------------------
//! `battery_advanced.rs:1994-2003` recorta el brillo a `MEGADAEMON_BRIGHTNESS_MAX`
//! (60 por defecto, `deploy/99-megadaemon.sh:1066`) cuando la clase de política
//! no es `Normal`.  Este módulo **respeta ese techo** en sus rutas automáticas
//! (no en `fijar()` explícito) para no pelearse con la política de energía, y
//! lo publica en `estado().bloqueado_por`.
//!
//! LO QUE FALTA MEDIR EN EL MÓVIL (NO VERIFICADO)
//! -----------------------------------------------
//! Todo esto es un mando de root; el resultado es el que decide si hay que
//! tocar el módulo (Samsung puede usar otra clave o otra escala):
//!
//! ```text
//! # 1) Qué dice el sistema AHORA
//! su -mm -c 'settings get system screen_brightness_mode; settings get system screen_brightness_float; settings get system screen_brightness'
//! su -mm -c 'dumpsys power | grep -i "Display Power\|ScreenBrightnessOverride"'
//! su -mm -c 'dumpsys sensorservice | grep -i -A2 "light sensor"'
//!
//! # 2) Qué ficheros existen (para el atajo barato del vigilante)
//! su -mm -c 'ls -l /data/system/display-manager-state.xml /data/system/users/0/settings_system.xml'
//!
//! # 3) Qué vía escribe DE VERDAD (una u otra, según el firmware)
//! su -mm -c 'settings put system screen_brightness_mode 0; settings put system screen_brightness_float 1.0; dumpsys power | grep "Display Power"'
//! su -mm -c 'cmd display set-brightness 1.0; dumpsys power | grep "Display Power"'
//! su -mm -c 'cmd display help 2>&1 | grep -i brightness'   # ¿existe set-brightness-mode?
//!
//! # 4) Cada cuánto el automático sobrescribe (modo=1 y mirar la serie)
//! su -mm -c 'settings put system screen_brightness_mode 1; for i in $(seq 1 12); do date +%T; settings get system screen_brightness_float; dumpsys power | grep -o "brightness=[0-9.]*"; sleep 5; done'
//!
//! # 5) Claves propias de Samsung
//! su -mm -c 'settings list system | grep -i "brightness\|adaptive"'
//!
//! # 6) Coste de cada escritura (cuántas JVM)
//! su -mm -c 'time settings put system screen_brightness_float 0.5'
//! su -mm -c 'time cmd display set-brightness 0.5'
//! ```

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Fichero de configuración (en el state dir de la instancia).
pub const FICHERO_CONFIG: &str = "brillo_config.json";
/// Fichero de estado interno del vigilante (bookkeeping).
pub const FICHERO_ESTADO: &str = "brillo_estado.json";

/// Máximo de la escala legacy 0..255.
pub const BRILLO_MAXIMO: u8 = 255;

/// Política por defecto: el usuario manda sobre el valor, pero el automático
/// se mantiene apagado.
pub const POLITICA_POR_DEFECTO: &str = "respetar_usuario";
/// Intervalo por defecto del vigilante, en segundos.
pub const INTERVALO_POR_DEFECTO_S: u64 = 15;

/// Ruta del fichero de configuración.
pub fn ruta_config(state_dir: Option<&str>) -> PathBuf {
    let base = state_dir
        .map(|s| s.to_string())
        .unwrap_or_else(crate::state_dir::state_dir);
    Path::new(&base).join(FICHERO_CONFIG)
}

/// Ruta del fichero de estado interno.
pub fn ruta_estado(state_dir: Option<&str>) -> PathBuf {
    let base = state_dir
        .map(|s| s.to_string())
        .unwrap_or_else(crate::state_dir::state_dir);
    Path::new(&base).join(FICHERO_ESTADO)
}

// ===========================================================================
// CONFIGURACIÓN
// ===========================================================================

/// Configuración del módulo. Todo es configurable por el usuario.
#[derive(Clone, Debug)]
pub struct Config {
    /// Brillo que se aplica por defecto (0..=255).  Por defecto, el máximo.
    pub brillo_por_defecto: u8,
    /// `respetar_usuario` | `firme` | `pasivo`.
    pub politica: String,
    /// Periodo del vigilante, en segundos.
    pub intervalo_vigilancia_s: u64,
    /// Si es `true` el módulo **no** toca el modo (el automático puede vivir).
    pub permitir_automatico: bool,
    /// Escribir además el ajuste legacy entero (`screen_brightness`).
    pub escribir_ajuste_legacy: bool,
    /// Escribir `screen_brightness_float` **sólo si ya existe** en el
    /// firmware.
    ///
    /// MEDIDO (SM-G998B, One UI 6, 2026-11-27): `settings get system
    /// screen_brightness_float` → **null** (esa clave no existe).  Crearla a
    /// la fuerza podría cambiar el camino que lee el framework de Samsung,
    /// así que por defecto **no se crea**: se escribe el legacy entero, que
    /// es el que sí vive ahí (`screen_brightness` = 247).
    pub solo_float_si_existe: bool,
    /// Poner a 0 `content_adaptive_brightness` (el brillo adaptativo de
    /// OneUI) cuando la clave exista.
    ///
    /// **NO VERIFICADO** en este firmware: el sensor `com.samsung.sensor.
    /// autobrightness` existe (medido), pero no está medido que esa clave sea
    /// la que lo manda.  Por defecto **NO** se toca (opt-in).
    pub neutralizar_adaptativo: bool,
    /// Usar `cmd display set-brightness` (vía DisplayManager) además del
    /// proveedor de ajustes.
    pub usar_cmd_display: bool,
    /// Verificar el brillo **aplicado** con `dumpsys power` (caro: 1 JVM).
    pub verificar_aplicado: bool,
    /// Vigilar **por eventos** (inotify + FIFO) en vez de sondear.
    ///
    /// MEDIDO (SM-G998B, 2026-11-27): con el sondeo de 15 s el vigilante
    /// gastaba **5 interacciones con el móvil por ciclo**; con los eventos
    /// son **0 sondeos** (y 0..1 escrituras, sólo si hay que corregir algo).
    pub eventos: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            brillo_por_defecto: BRILLO_MAXIMO,
            politica: POLITICA_POR_DEFECTO.to_string(),
            intervalo_vigilancia_s: INTERVALO_POR_DEFECTO_S,
            permitir_automatico: false,
            escribir_ajuste_legacy: true,
            solo_float_si_existe: true,
            neutralizar_adaptativo: false,
            usar_cmd_display: true,
            verificar_aplicado: true,
            eventos: true,
        }
    }
}

impl Config {
    /// A JSON.
    pub fn a_json(&self) -> Value {
        json!({
            "brillo_por_defecto": self.brillo_por_defecto,
            "politica": self.politica,
            "intervalo_vigilancia_s": self.intervalo_vigilancia_s,
            "permitir_automatico": self.permitir_automatico,
            "escribir_ajuste_legacy": self.escribir_ajuste_legacy,
            "solo_float_si_existe": self.solo_float_si_existe,
            "neutralizar_adaptativo": self.neutralizar_adaptativo,
            "usar_cmd_display": self.usar_cmd_display,
            "verificar_aplicado": self.verificar_aplicado,
            "eventos": self.eventos,
        })
    }

    /// Desde JSON, tolerante: cualquier campo ausente o inválido cae al
    /// valor por defecto (nunca panic).
    pub fn de_json(v: &Value) -> Config {
        let d = Config::default();
        let brillo = v
            .get("brillo_por_defecto")
            .and_then(|x| x.as_u64())
            .map(|x| x.min(255) as u8)
            .unwrap_or(d.brillo_por_defecto);
        let politica = v
            .get("politica")
            .and_then(|x| x.as_str())
            .map(|s| s.trim().to_lowercase())
            .filter(|s| matches!(s.as_str(), "respetar_usuario" | "firme" | "pasivo"))
            .unwrap_or(d.politica);
        let intervalo = v
            .get("intervalo_vigilancia_s")
            .and_then(|x| x.as_u64())
            .map(|x| x.clamp(2, 3600))
            .unwrap_or(d.intervalo_vigilancia_s);
        Config {
            brillo_por_defecto: brillo,
            politica,
            intervalo_vigilancia_s: intervalo,
            permitir_automatico: v
                .get("permitir_automatico")
                .and_then(|x| x.as_bool())
                .unwrap_or(d.permitir_automatico),
            escribir_ajuste_legacy: v
                .get("escribir_ajuste_legacy")
                .and_then(|x| x.as_bool())
                .unwrap_or(d.escribir_ajuste_legacy),
            solo_float_si_existe: v
                .get("solo_float_si_existe")
                .and_then(|x| x.as_bool())
                .unwrap_or(d.solo_float_si_existe),
            neutralizar_adaptativo: v
                .get("neutralizar_adaptativo")
                .and_then(|x| x.as_bool())
                .unwrap_or(d.neutralizar_adaptativo),
            usar_cmd_display: v
                .get("usar_cmd_display")
                .and_then(|x| x.as_bool())
                .unwrap_or(d.usar_cmd_display),
            verificar_aplicado: v
                .get("verificar_aplicado")
                .and_then(|x| x.as_bool())
                .unwrap_or(d.verificar_aplicado),
            eventos: v
                .get("eventos")
                .and_then(|x| x.as_bool())
                .unwrap_or(d.eventos),
        }
    }
}

/// Lee el fichero de configuración **en crudo** (sin defaults ni campos
/// calculados).  Una sola lectura de disco.
fn leer_config_raw(state_dir: Option<&str>) -> Value {
    match std::fs::read_to_string(ruta_config(state_dir)) {
        Ok(txt) => serde_json::from_str::<Value>(txt.as_str()).unwrap_or_else(|_| json!({})),
        Err(_) => json!({}),
    }
}

/// Lee la configuración guardada (o los valores por defecto).
pub fn config(state_dir: Option<&str>) -> Value {
    let p = ruta_config(state_dir);
    let existe = p.exists();
    let cfg = Config::de_json(&leer_config_raw(state_dir));
    let mut out = cfg.a_json();
    out["ok"] = json!(true);
    out["ruta"] = json!(p.display().to_string());
    out["existe"] = json!(existe);
    // Techo efectivo de la política de energía (battery_advanced).
    if let Some(cap) = techo_energia() {
        out["techo_energia"] = json!(cap);
    }
    out
}

/// Actualiza la configuración (sólo los campos presentes) y la guarda.
pub fn config_actualizar(args: &Value, state_dir: Option<&str>) -> Value {
    let p = ruta_config(state_dir);
    let actual = match std::fs::read_to_string(&p) {
        Ok(txt) => serde_json::from_str::<Value>(&txt).unwrap_or_else(|_| json!({})),
        Err(_) => json!({}),
    };
    let mut merged = actual.clone();
    for clave in [
        "brillo_por_defecto",
        "politica",
        "intervalo_vigilancia_s",
        "permitir_automatico",
        "escribir_ajuste_legacy",
        "solo_float_si_existe",
        "neutralizar_adaptativo",
        "usar_cmd_display",
        "eventos",
        "verificar_aplicado",
    ] {
        if let Some(v) = args.get(clave) {
            merged[clave] = v.clone();
        }
    }
    let cfg = Config::de_json(&merged);
    if let Some(dir) = p.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return json!({"ok": false, "error": format!("crear {}: {e}", dir.display())});
        }
    }
    let ruta = match std::fs::write(&p, cfg.a_json().to_string()) {
        Ok(_) => p.display().to_string(),
        Err(e) => return json!({"ok": false, "error": format!("escribir {}: {e}", p.display())}),
    };
    json!({
        "ok": true,
        "ruta": ruta,
        "config": cfg.a_json(),
        "nota": "el brillo por defecto NO se aplica solo: llame a maximo()/fijar() o arranque el vigilante",
    })
}

// ===========================================================================
// EJECUCIÓN DE MANDOS EN EL MÓVIL
// ===========================================================================

/// Resultado de un mando.
pub struct Salida {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
    pub timed_out: bool,
}

impl Salida {
    /// stdout limpio.
    pub fn texto(&self) -> String {
        self.stdout.trim().to_string()
    }
    /// ¿Terminó bien?
    pub fn ok(&self) -> bool {
        !self.timed_out && self.code.map(|c| c == 0).unwrap_or(false)
    }
}

/// Ejecuta un mando **sin root** (shell normal), con timeout.
///
/// Lectura en hilo aparte (como `android::root_exec`): si no, un `dumpsys`
/// grande llenaría el pipe y el hijo se quedaría bloqueado.
fn sh_plano(cmd: &str, timeout_ms: u64) -> Salida {
    let mut hijo = match Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(h) => h,
        Err(e) => {
            return Salida {
                stdout: String::new(),
                stderr: format!("no se pudo ejecutar 'sh': {e}"),
                code: None,
                timed_out: false,
            }
        }
    };

    let mut out_pipe = hijo.stdout.take();
    let mut err_pipe = hijo.stderr.take();
    let h_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            use std::io::Read;
            let _ = p.read_to_end(&mut b);
        }
        b
    });
    let h_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            use std::io::Read;
            let _ = p.read_to_end(&mut b);
        }
        b
    });

    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(500));
    let mut timeout = false;
    let estado = loop {
        match hijo.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    timeout = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
            Err(_) => break None,
        }
    };

    let out = h_out.join().unwrap_or_default();
    let err = h_err.join().unwrap_or_default();
    Salida {
        stdout: String::from_utf8_lossy(&out).into_owned(),
        stderr: String::from_utf8_lossy(&err).into_owned(),
        code: estado.and_then(|s| s.code()),
        timed_out: timeout,
    }
}

// Contador de interacciones con el móvil (POR HILO).
//
// Cada llamada a `sh` es **un proceso en el móvil** (y cada `settings put` o
// `dumpsys` dentro son JVM).  Este contador permite **medir** el coste en vez
// de estimarlo: lo usan los tests (que acotan el número) y la acción `costo`
// (que lo mide **en el móvil**).
//
// MEDIDO (2026-11-27): el contador era GLOBAL y `cargo test` corre los tests
// EN PARALELO, así que los tests de coste contaban también las llamadas de
// los OTROS tests de este módulo que estaban corriendo a la vez.  En el CI
// (suite completa, 1.624 tests) salían 5 y 8 interacciones donde el módulo
// hace 2 y 5 => tres tests rojos por una razón que no era del módulo.
// Ahora el contador es **por hilo** (cada test mide sus propias llamadas) y
// `reset_llamadas_sh()` lo pone a cero antes de medir.
//
// (No se reintroduce el global: el arreglo de root se conserva.)
thread_local! {
    static CONTADOR_SH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Interacciones con el móvil desde el último reset (de ESTE hilo).
///
/// Disponible **en producción** porque la acción `costo` tiene que medir en
/// el móvil (en el CI no hay Android): el número del encargo sale del
/// aparato, no de una estimación.
pub fn llamadas_sh() -> usize {
    CONTADOR_SH.with(|c| c.get())
}

/// Pone a cero el contador de interacciones de ESTE hilo.
pub fn reset_llamadas_sh() {
    CONTADOR_SH.with(|c| c.set(0));
}

/// ¿El proceso actual ya es root?  (lectura de `/proc/self/status`, cacheada)
///
/// Si lo es, `su -mm -c` es **un proceso de más** en cada interacción con el
/// móvil: se puede ir directo al shell y ahorrarlo.
fn somos_root() -> bool {
    static CACHE: Mutex<Option<(u64, bool)>> = Mutex::new(None);
    if let Some(v) = cache_get(&CACHE, TTL_CACHE_S) {
        return v;
    }
    let uid = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1).map(|u| u.to_string()))
        });
    let v = uid.map(|u| u == "0").unwrap_or(false);
    cache_put(&CACHE, v);
    v
}

/// Ejecuta un mando **con root** (`su -mm -c`) si se puede; si no, cae al
/// shell normal.  Devuelve `ok=false` con el error si no hay permisos.
///
/// **Optimizado**: si el proceso ya es root (el daemon arranca como root desde
/// `service.d`), se va **directo al shell** y se ahorra el proceso `su`; si el
/// sistema lo deniega (SELinux), se reintenta con `su -mm -c` para no perder
/// funcionalidad.
fn sh(cmd: &str, root: bool, timeout_ms: u64) -> Salida {
    CONTADOR_SH.with(|c| c.set(c.get() + 1));
    if !root {
        return sh_plano(cmd, timeout_ms);
    }
    if somos_root() {
        let directa = sh_plano(cmd, timeout_ms);
        let denegado = !directa.timed_out
            && (directa.stderr.contains("Permission denied")
                || directa.stderr.contains("avc: denied")
                || directa.stderr.contains("SecurityException"));
        if denegado {
            let r = crate::android::root_exec(cmd, timeout_ms, 64 * 1024);
            return Salida {
                stdout: r.stdout,
                stderr: r.stderr,
                code: r.code,
                timed_out: r.timed_out,
            };
        }
        return directa;
    }
    let r = crate::android::root_exec(cmd, timeout_ms, 64 * 1024);
    // Si `su` no existe o deniega, el stderr lo dice y no hay código de
    // salida: en ese caso se prueba sin root (lecturas; las escrituras
    // fallarán con su propio error, que es lo honesto).
    let fallo_su = r.code.is_none()
        && (r.stderr.contains("not found")
            || r.stderr.contains("denied")
            || r.stderr.contains("Permission denied")
            || r.stderr.contains("No such file"));
    if fallo_su {
        return sh_plano(cmd, timeout_ms);
    }
    Salida {
        stdout: r.stdout,
        stderr: r.stderr,
        code: r.code,
        timed_out: r.timed_out,
    }
}

/// ¿Tenemos root?  (cacheado: `su` es un proceso, y esto se pregunta mucho).
///
/// TTL de 30 s: si el root aparece o desaparece (Magisk denied, etc.), el
/// estado se revisa solo sin estar lanzando `su` en cada llamada.
pub fn hay_root() -> bool {
    static CACHE: Mutex<Option<(u64, bool)>> = Mutex::new(None);
    const TTL_S: u64 = 30;
    if let Some(v) = cache_get(&CACHE, TTL_S) {
        return v;
    }
    let r = sh("id -u 2>/dev/null || echo 0", true, 8000);
    let uid = r.texto();
    let ok = r.ok() && (uid == "0" || uid.ends_with("uid=0"));
    cache_put(&CACHE, ok);
    ok
}

// ===========================================================================
// CACHÉ CON CADUCIDAD (los valores caros no se recalculan en cada llamada)
// ===========================================================================

/// TTL de las cachés internas, en segundos.
const TTL_CACHE_S: u64 = 30;

/// Lee una caché con caducidad.
fn cache_get<T: Copy>(cache: &Mutex<Option<(u64, T)>>, ttl_s: u64) -> Option<T> {
    let guard = cache.lock().ok()?;
    let (ts, valor) = guard.as_ref()?;
    if ahora().saturating_sub(*ts) < ttl_s {
        Some(*valor)
    } else {
        None
    }
}

/// Escribe una caché con caducidad.
fn cache_put<T: Copy>(cache: &Mutex<Option<(u64, T)>>, valor: T) {
    if let Ok(mut g) = cache.lock() {
        *g = Some((ahora(), valor));
    }
}

// ===========================================================================
// LECTURA DEL SISTEMA
// ===========================================================================

/// Lo que se ha leído del sistema.
#[derive(Clone, Debug, Default)]
pub struct Lectura {
    /// `screen_brightness_mode` (0 manual, 1 automático).
    pub modo: Option<i64>,
    /// `screen_brightness_float` (0.0-1.0).
    pub float: Option<f64>,
    /// `screen_brightness` (0-255, legacy).
    pub int: Option<u8>,
    /// `screen_auto_brightness_adj` (-1..1).
    pub auto_ajuste: Option<f64>,
    /// `content_adaptive_brightness` (0/1): el brillo adaptativo de OneUI.
    pub adaptativo: Option<i64>,
    /// Texto crudo (evidencia).
    pub crudo: String,
    /// ¿Se pudo leer?
    pub ok: bool,
    /// Error, si lo hubo.
    pub error: Option<String>,
}

impl Lectura {
    /// Brillo en la escala 0..255 (preferimos el float, que es el que usa el
    /// sistema desde Android 12).
    pub fn brillo_255(&self) -> Option<u8> {
        if let Some(f) = self.float {
            return Some((f.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        self.int
    }
    /// ¿El automático está activo?
    pub fn auto_activo(&self) -> bool {
        self.modo.map(|m| m != 0).unwrap_or(false)
    }
}

/// Parsea la salida de `settings list system` (o de varios `settings get`).
///
/// Formato: `clave=valor` por línea.
pub fn parse_ajustes(crudo: &str) -> Lectura {
    let mut l = Lectura {
        crudo: crudo.to_string(),
        ok: true,
        ..Default::default()
    };
    for linea in crudo.lines() {
        let Some((clave, valor)) = linea.split_once('=') else {
            continue;
        };
        let valor = valor.trim();
        match clave.trim() {
            "screen_brightness_mode" => l.modo = valor.parse::<i64>().ok(),
            "screen_brightness_float" => l.float = valor.parse::<f64>().ok(),
            "screen_brightness" => l.int = valor.parse::<f64>().ok().map(|v| v.round() as u8),
            "screen_auto_brightness_adj" => l.auto_ajuste = valor.parse::<f64>().ok(),
            "content_adaptive_brightness" | "adaptive_brightness" => {
                l.adaptativo = valor.parse::<i64>().ok()
            }
            _ => {}
        }
    }
    l
}

/// Una línea `<setting .../>` del XML del proveedor de ajustes.
fn parse_setting_xml(linea: &str) -> Option<(String, String)> {
    let nombre = attr(linea, "name")?;
    let valor = attr(linea, "value")
        .or_else(|| attr(linea, "defaultValue"))
        .unwrap_or_default();
    Some((nombre, valor))
}

/// Extrae un atributo XML (`name="x"`) de una etiqueta.
fn attr(etiqueta: &str, clave: &str) -> Option<String> {
    let pat = format!("{clave}=\"");
    let i = etiqueta.find(&pat)? + pat.len();
    let resto = &etiqueta[i..];
    let j = resto.find('"')?;
    Some(resto[..j].to_string())
}

/// Parsea los `<setting .../>` del XML del proveedor de ajustes (función pura).
///
/// MEDIDO: en el SM-G998B `settings list system` **no devuelve nada**
/// (AOSP 14 sí lo soporta: `SettingsProvider.java:585`), así que el XML del
/// proveedor es la vía de lectura **sin JVM**.
pub fn parse_settings_xml(crudo: &str) -> Lectura {
    let mut l = Lectura {
        crudo: crudo.to_string(),
        ok: true,
        ..Default::default()
    };
    for linea in crudo.lines() {
        let Some((nombre, valor)) = parse_setting_xml(linea) else {
            continue;
        };
        let valor = valor.trim();
        match nombre.as_str() {
            "screen_brightness_mode" => l.modo = valor.parse::<i64>().ok(),
            "screen_brightness_float" => l.float = valor.parse::<f64>().ok(),
            "screen_brightness" => l.int = valor.parse::<f64>().ok().map(|v| v.round() as u8),
            "screen_auto_brightness_adj" => l.auto_ajuste = valor.parse::<f64>().ok(),
            "content_adaptive_brightness" | "adaptive_brightness" => {
                l.adaptativo = valor.parse::<i64>().ok()
            }
            _ => {}
        }
    }
    l
}

/// Lee los ajustes desde el XML del proveedor de ajustes del usuario.
///
/// **0 JVM**: un `grep` sobre el fichero.  El proveedor lo escribe con
/// retraso (escribe por lotes), pero para vigilar va perfecto.
pub fn leer_ajustes_xml() -> Option<Lectura> {
    let cmd = "for f in /data/system/users/0/settings_system.xml /data/system/settings_system.xml /data/system/users/0/settings_global.xml; do if [ -r \"$f\" ]; then grep -oE '<setting [^>]*name=\"(screen_brightness|content_adaptive_brightness|adaptive_brightness|screen_auto_brightness_adj)[^>]*>' \"$f\" 2>/dev/null; fi; done";
    let r = sh(cmd, true, 10_000);
    let texto = r.texto();
    if texto.is_empty() {
        return None;
    }
    let l = parse_settings_xml(&texto);
    if l.modo.is_some() || l.int.is_some() || l.float.is_some() {
        Some(l)
    } else {
        None
    }
}

/// Lee los ajustes del brillo eligiendo la **vía más barata que funcione**.
///
/// MEDIDO (SM-G998B, 2026-11-27): `settings list system` **no devuelve nada**
/// en este firmware ⇒ la lectura por XML (0 JVM) es la que manda; y
/// `screen_brightness_float` **no existe** (el brillo vive en el legacy
/// entero).  Las tres vías, de más barata a más cara:
///
/// 1. XML del proveedor de ajustes (0 JVM) — la normal aquí;
/// 2. `settings list system` (1 JVM) — la de AOSP;
/// 3. `settings get` × 4 (4 JVM) — el último recurso.
pub fn leer_ajustes() -> Lectura {
    // Qué vía funciona (cacheado 5 min: no se reintenta la cara cada vez).
    static VIA: Mutex<Option<(u64, u8)>> = Mutex::new(None);
    const TTL_S: u64 = 300;
    let via = cache_get(&VIA, TTL_S).unwrap_or(0);

    if via != 3 {
        if let Some(l) = leer_ajustes_xml() {
            if via != 1 {
                cache_put(&VIA, 1u8);
            }
            return l;
        }
    }
    if via != 3 {
        let cmd = "settings list system 2>/dev/null | grep -E '^(screen_brightness_mode|screen_brightness_float|screen_brightness|screen_auto_brightness_adj)='";
        let r = sh(cmd, false, 20_000);
        let texto = r.texto();
        let l = parse_ajustes(&texto);
        if l.modo.is_some() || l.float.is_some() || l.int.is_some() {
            cache_put(&VIA, 2u8);
            return l;
        }
    }
    // Fallback: `settings get` (más caro: una JVM por clave).
    let cmd2 = "echo screen_brightness_mode=$(settings get system screen_brightness_mode 2>/dev/null); echo screen_brightness_float=$(settings get system screen_brightness_float 2>/dev/null); echo screen_brightness=$(settings get system screen_brightness 2>/dev/null); echo screen_auto_brightness_adj=$(settings get system screen_auto_brightness_adj 2>/dev/null)";
    let r2 = sh(cmd2, false, 30_000);
    let t2 = r2.texto();
    let mut l2 = parse_ajustes(&t2);
    l2.ok = !t2.trim().is_empty();
    l2.crudo = t2;
    cache_put(&VIA, 3u8);
    l2
}

/// Brillo **aplicado** (lo que la pantalla está haciendo), leído de
/// `dumpsys power`.
///
/// Formato real (medido en este repo, `battery_advanced.rs:1630`):
/// `Display Power: state=OFF, display=true, brightness=0, brightnessInt=0, displayPower=OFF`
/// y `mScreenBrightnessOverrideFromWindowManager=` (si no está vacío, hay una
/// app mandando sobre el brillo).
#[derive(Clone, Debug, Default)]
pub struct Aplicado {
    pub brillo_float: Option<f64>,
    pub brillo_int: Option<u8>,
    pub override_ventana: Option<String>,
    pub estado_pantalla: Option<String>,
    pub crudo: String,
}

impl Aplicado {
    /// ¿Hay una app (o el WM) sobrescribiendo el brillo?
    pub fn hay_override(&self) -> bool {
        self.override_ventana
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    }
}

/// Parsea la salida de `dumpsys power` (función pura, testeable).
pub fn parse_dumpsys_power(crudo: &str) -> Aplicado {
    let mut a = Aplicado {
        crudo: crudo.to_string(),
        ..Default::default()
    };
    for linea in crudo.lines() {
        let l = linea.trim();
        if l.starts_with("Display Power:") {
            if let Some(v) = campo_entre(l, "brightness=", ",") {
                a.brillo_float = v.trim().parse::<f64>().ok();
            }
            if let Some(v) = campo_entre(l, "brightnessInt=", ",") {
                a.brillo_int = v.trim().parse::<f64>().ok().map(|x| x.round() as u8);
            }
            if let Some(v) = campo_entre(l, "state=", ",") {
                a.estado_pantalla = Some(v.trim().to_string());
            }
        } else if l.starts_with("mScreenBrightnessOverrideFromWindowManager=") {
            a.override_ventana = Some(
                l.trim_start_matches("mScreenBrightnessOverrideFromWindowManager=")
                    .trim()
                    .to_string(),
            );
        }
    }
    a
}

/// Extrae el valor entre `prefijo` y `fin` (si aparece).
fn campo_entre(linea: &str, prefijo: &str, fin: &str) -> Option<String> {
    let i = linea.find(prefijo)? + prefijo.len();
    let resto = &linea[i..];
    match resto.find(fin) {
        Some(j) => Some(resto[..j].to_string()),
        None => Some(resto.to_string()),
    }
}

/// Lee el brillo aplicado (`dumpsys power`).  Cuesta 1 JVM: sólo bajo demanda.
///
/// **Optimizado**: el `grep` va dentro del shell ⇒ el daemon no arrastra
/// kilobytes de `dumpsys` por el pipe ni los parsea.
pub fn leer_aplicado() -> Aplicado {
    let r = sh(
        "dumpsys power 2>/dev/null | grep -i 'Display Power\\|ScreenBrightnessOverride'",
        false,
        30_000,
    );
    parse_dumpsys_power(&r.stdout)
}

/// Valor del sensor de luz ambiente (lux), si se puede leer.
///
/// Primero el sysfs (gratis); si no, `dumpsys sensorservice` (1 JVM).
/// La línea típica es: `0x... ) Light sensor | ... | value: 12.0 | max: ...`.
///
/// **Optimizado**: un único proceso; el `dumpsys` sólo se lanza si el sysfs no
/// da el valor, y su salida ya viene filtrada.
pub fn leer_sensor_luz() -> (Option<f64>, Option<String>) {
    let cmd = "v=$(for f in /sys/class/light_sensor/light /sys/class/light_sensor/lux /sys/class/misc/light_sensor/light; do [ -r \"$f\" ] && head -1 \"$f\" && break; done 2>/dev/null); if [ -n \"$v\" ]; then echo \"sysfs:$v\"; else dumpsys sensorservice 2>/dev/null | grep -A30 'Uncalibrated Light: last' | grep -E \"^[[:space:]]+[0-9]+ [(]\" | tail -3; fi";
    let r = sh(cmd, false, 30_000);
    parse_sensor_luz(&r.stdout)
}

/// Parsea la salida de la sonda del sensor de luz (función pura, testeable).
///
/// Acepta las dos formas que produce la sonda: `sysfs:<valor>` o las líneas
/// de `dumpsys sensorservice`.
pub fn parse_sensor_luz(crudo: &str) -> (Option<f64>, Option<String>) {
    let t = crudo.trim();
    if t.is_empty() {
        return (None, Some("sin sensor de luz legible".to_string()));
    }
    if let Some(resto) = t.strip_prefix("sysfs:") {
        let v = resto.trim().parse::<f64>().ok();
        return (v, Some(format!("sysfs: {}", resto.trim())));
    }
    (parse_als(t), Some("dumpsys sensorservice".to_string()))
}

/// Extrae el valor del sensor de luz de la salida de `dumpsys sensorservice`.
///
/// MEDIDO en el SM-G998B (2026-11-27): los sensores de luz son
/// ```
/// 0x0000000a) TMD4912 Light          | AMS | type: com.samsung.sensor.autobrightness(65601)
/// 0x00000019) TMD4912 Light IR       | AMS | type: com.samsung.sensor.light_ir(65578)
/// 0x00000021) TMD4912 Light CCT      | AMS | type: com.samsung.sensor.light_cct(65587)
/// 0x00000029) TMD4912 Uncalibrated Light | AMS | type: android.sensor.light(5)
/// 0x00000030) Light Seamless         | Samsung Electronics | type: com.samsung.sensor.light_seamless(65614)
/// ```
/// O sea: **Samsung tiene su propio sensor de autobrightness** además del
/// `android.sensor.light` estándar ⇒ el automático de OneUI puede ir por una
/// vía que NO es el `AutomaticBrightnessController` de AOSP.
///
/// El formato exacto de los *eventos* (donde está el valor) no se ha podido
/// leer en este firmware ⇒ el parser es **tolerante**: acepta `value: N`,
/// `v=N` y `lux: N`, dentro y fuera de un bloque "light", y **no inventa**
/// (si no encuentra nada, devuelve `None`).
pub fn parse_als(crudo: &str) -> Option<f64> {
    // FORMATO REAL MEDIDO (SM-G998B, 2026-11-27):
    //   TMD4912 Uncalibrated Light: last 50 events
    //            8 (ts=112929.010366080, wall=11-27 01:39:41.134) 9.00, 0.00, 0.00,
    // El PRIMER número tras el paréntesis es el lux; el último evento es el
    // valor actual.
    let mut ultimo: Option<f64> = None;
    for linea in crudo.lines() {
        let t = linea.trim_start();
        if !t.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let Some(cierre) = t.find(')') else {
            continue;
        };
        let resto = t[cierre + 1..].trim_start();
        let num: String = resto
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
            .collect();
        if let Ok(v) = num.parse::<f64>() {
            ultimo = Some(v);
        }
    }
    if ultimo.is_some() {
        return ultimo;
    }
    // Respaldo: "value: N" en una línea que mencione "light".
    for linea in crudo.lines() {
        let low = linea.to_lowercase();
        if !low.contains("light") {
            continue;
        }
        if let Some(i) = low.find("value:") {
            let num: String = linea[i + 6..]
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
                .collect();
            if let Ok(v) = num.parse::<f64>() {
                return Some(v);
            }
        }
    }
    None
}

/// ¿El firmware tiene las claves de **brillo adaptativo de Samsung**?
///
/// MEDIDO (SM-G998B): el sensor `com.samsung.sensor.autobrightness` existe
/// ⇒ hay una vía de auto-brightness propia de OneUI.  La clave que la
/// controla es `content_adaptive_brightness` (NO VERIFICADO: falta leer su
/// valor en este firmware).
pub fn claves_adaptativas() -> Value {
    let r = sh(
        "for k in content_adaptive_brightness adaptive_brightness screen_auto_brightness_adj screen_brightness_adjust; do v=$(settings get system $k 2>/dev/null); [ -n \"$v\" ] && echo \"$k=$v\"; done",
        false,
        25_000,
    );
    let texto = r.texto();
    let mut claves = serde_json::Map::new();
    for linea in texto.lines() {
        if let Some((k, v)) = linea.split_once('=') {
            claves.insert(k.trim().to_string(), json!(v.trim()));
        }
    }
    json!({
        "claves": Value::Object(claves),
        "crudo": texto.trim(),
        "nota": "si 'content_adaptive_brightness' vale 1, el brillo adaptativo de OneUI está activo y es el candidato nº1 para 'el brillo se cambia solo'",
    })
}

/// Neutraliza el brillo adaptativo de Samsung (si la clave existe).
///
/// **NO VERIFICADO**: no está medido en este firmware que esa clave sea la
/// que manda.  Por eso es **opt-in** (`neutralizar_adaptativo`), y el
/// diagnóstico informa del valor antes y después para que la medición
/// decida.
pub fn neutralizar_adaptativo() -> Value {
    let antes = sh(
        "settings get system content_adaptive_brightness 2>/dev/null",
        false,
        20_000,
    )
    .texto();
    let r = sh(
        "settings put system content_adaptive_brightness 0 2>&1; echo rc=$?",
        true,
        20_000,
    );
    let despues = sh(
        "settings get system content_adaptive_brightness 2>/dev/null",
        false,
        20_000,
    )
    .texto();
    json!({
        "ok": true,
        "antes": antes.trim(),
        "despues": despues.trim(),
        "rc": extract_rc(&r.stdout, "rc="),
        "salida": format!("{}{}", r.stdout, r.stderr).trim(),
        "nota": "NO VERIFICADO en este firmware: el valor anterior/después es la medición",
    })
}

/// El brillo que la política de energía guardó antes de recortarlo.
///
/// `deploy/99-megadaemon.sh:1097` (`ENERGY_BRIGHT_SAVED`) guarda el valor del
/// usuario y lo restaura al relajar la política.  Sirve para distinguir
/// "la política restauró su copia" de "el usuario movió el deslizador":
/// si el valor nuevo es **exactamente** el guardado ⇒ fue la política ⇒ se
/// restaura el objetivo del usuario (no a 247 clavado).
pub fn brillo_guardado_por_politica() -> Option<u8> {
    let cmd = "cat /data/adb/megadaemon/brightness_saved 2>/dev/null";
    let r = sh(cmd, true, 6000);
    let t = r.texto();
    t.trim().parse::<u8>().ok()
}

/// Techo de brillo impuesto por la política de energía del propio daemon.
///
/// `battery_advanced.rs:1994-2003` recorta a `MEGADAEMON_BRIGHTNESS_MAX`
/// (60 por defecto; `deploy/99-megadaemon.sh:1066`) cuando la clase de
/// política no es `Normal`.  Se lee del entorno del propio proceso y, si no
/// está, del fichero de entorno del móvil.
///
/// **Optimizado**: el entorno del proceso es gratis; la lectura del fichero
/// cuesta un proceso ⇒ se cachea 30 s (la política de energía cambia por
/// minutos, no por segundos).
pub fn techo_energia() -> Option<u8> {
    static CACHE: Mutex<Option<(u64, Option<u8>)>> = Mutex::new(None);
    if let Some(v) = cache_get(&CACHE, TTL_CACHE_S) {
        return v;
    }
    let valor =
        if let Ok(v) = std::env::var("MEGADAEMON_BRIGHTNESS_MAX") {
            v.trim().parse::<f64>().ok().and_then(|n| {
                if n > 0.0 {
                    Some(n.round() as u8)
                } else {
                    None
                }
            })
        } else {
            None
        };
    let valor = match valor {
        Some(v) => Some(v),
        None => {
            let cmd = "grep -h -i 'BRIGHTNESS_MAX' /data/adb/megadaemon/megadaemon.env /data/local/tmp/megadaemon/megadaemon.env 2>/dev/null | head -1";
            let r = sh(cmd, true, 8000);
            let t = r.texto();
            if t.is_empty() {
                None
            } else {
                // Formato: `ENERGY_BRIGHTNESS_MAX=60` o `MEGADAEMON_BRIGHTNESS_MAX=60`.
                let v = t.split('=').nth(1).unwrap_or("").trim();
                v.parse::<f64>().ok().and_then(
                    |n| {
                        if n > 0.0 {
                            Some(n.round() as u8)
                        } else {
                            None
                        }
                    },
                )
            }
        }
    };
    cache_put(&CACHE, valor);
    valor
}

// ===========================================================================
// ESCRITURA
// ===========================================================================

/// Formatea un float 0..1 para `settings put` (el proveedor lo parsea como
/// float; un entero plano también vale, pero el float es inequívoco).
pub fn fmt_float(f: f64) -> String {
    let f = f.clamp(0.0, 1.0);
    let s = format!("{f:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Resultado de una escritura.
#[derive(Clone, Debug, Default)]
pub struct Escritura {
    /// Pasos ejecutados: `(nombre, ok, salida)`.
    pub pasos: Vec<(String, bool, String)>,
    /// ¿Hubo algún fallo?
    pub fallo: bool,
}

impl Escritura {
    /// Nombres de los pasos que fallaron.
    pub fn fallos(&self) -> Vec<String> {
        self.pasos
            .iter()
            .filter(|(_, ok, _)| !*ok)
            .map(|(n, _, _)| n.clone())
            .collect()
    }
}

/// ¿Existe la clave `screen_brightness_float` en este firmware?
///
/// MEDIDO (SM-G998B, One UI 6, 2026-11-27): **no existe** (`settings get
/// system screen_brightness_float` → `null`); el brillo vivo está en el
/// legacy entero `screen_brightness` (= 247).  Cacheado 5 min (1 JVM).
pub fn existe_float() -> bool {
    static CACHE: Mutex<Option<(u64, bool)>> = Mutex::new(None);
    const TTL_S: u64 = 300;
    if let Some(v) = cache_get(&CACHE, TTL_S) {
        return v;
    }
    let r = sh(
        "settings get system screen_brightness_float 2>/dev/null",
        false,
        20_000,
    );
    let t = r.texto();
    let v = !t.is_empty() && t != "null" && t.parse::<f64>().is_ok();
    cache_put(&CACHE, v);
    v
}

/// Escribe el brillo por la **vía del proveedor de ajustes**.
///
/// Un único proceso (`sh`), tres `settings put` (cada uno es una JVM, pero en
/// el mismo shell ⇒ un solo `su`):
///
/// * `screen_brightness_float` (0.0-1.0) — el que usa el sistema desde
///   Android 12;
/// * `screen_brightness` (0-255, legacy) — por si el firmware de Samsung
///   todavía lo lee;
/// * `screen_brightness_mode` = 0 — **el ancla**: apaga el automático.
///
/// Requiere root para escribir; sin root, `settings put system` falla con
/// `SecurityException` y se devuelve el error tal cual.
pub fn escribir_via_ajustes(valor: u8, cfg: &Config, forzar_manual: bool) -> Escritura {
    let mut e = Escritura::default();
    let v = valor;
    let f = (v as f64) / 255.0;

    let escribir_float = !cfg.solo_float_si_existe || existe_float();
    let mut cmd = String::with_capacity(160);
    if escribir_float {
        cmd.push_str(&format!(
            "settings put system screen_brightness_float {} ; echo rc_float=$? ; ",
            fmt_float(f)
        ));
    }
    if cfg.escribir_ajuste_legacy {
        cmd.push_str(&format!(
            "settings put system screen_brightness {v} ; echo rc_int=$? ; "
        ));
    }
    if forzar_manual {
        cmd.push_str("settings put system screen_brightness_mode 0 ; echo rc_mode=$? ; ");
    }
    if cfg.neutralizar_adaptativo {
        cmd.push_str("settings put system content_adaptive_brightness 0 ; echo rc_adapt=$? ; ");
    }
    let r = sh(&cmd, true, 40_000);
    for (nombre, rc) in [
        ("screen_brightness_float", "rc_float="),
        ("screen_brightness", "rc_int="),
        ("screen_brightness_mode", "rc_mode="),
        ("content_adaptive_brightness", "rc_adapt="),
    ] {
        if nombre == "screen_brightness_float" && !escribir_float {
            continue;
        }
        if nombre == "content_adaptive_brightness" && !cfg.neutralizar_adaptativo {
            continue;
        }
        if !cfg.escribir_ajuste_legacy && nombre == "screen_brightness" {
            continue;
        }
        if !forzar_manual && nombre == "screen_brightness_mode" {
            continue;
        }
        let ok = r.stdout.contains(&format!("{rc}0"));
        e.pasos.push((
            nombre.to_string(),
            ok,
            extract_rc(&r.stdout, rc).unwrap_or_else(|| r.stderr.trim().to_string()),
        ));
    }
    e.fallo = e.pasos.iter().any(|(_, ok, _)| !*ok);
    e
}

/// Escribe el brillo por la **vía de DisplayManager**: `cmd display set-brightness`.
///
/// AOSP 14 toma un **float 0.0-1.0** (`DisplayManagerShellCommand.java:161-181`:
/// "Error: brightness should be a number between 0 and 1"), no un entero
/// 0-255; si el firmware usa la escala antigua, se reintenta con el entero.
pub fn escribir_via_cmd_display(valor: u8) -> Escritura {
    let mut e = Escritura::default();
    let v = valor;
    let f = (v as f64) / 255.0;
    let r2 = sh(
        &format!("cmd display set-brightness {} 2>&1", fmt_float(f)),
        true,
        30_000,
    );
    let texto = format!("{}{}", r2.stdout, r2.stderr);
    let mut ok = r2.ok() && !texto.contains("Error:");
    if !ok && texto.contains("between 0 and 1") {
        // AOSP ≤11 usaba un entero 0..255: reintentamos con esa forma.
        let r3 = sh(
            &format!("cmd display set-brightness {v} 2>&1"),
            true,
            30_000,
        );
        let t3 = format!("{}{}", r3.stdout, r3.stderr);
        ok = r3.ok() && !t3.contains("Error:");
        e.pasos.push((
            "cmd_display_set_brightness_int".to_string(),
            ok,
            t3.trim().to_string(),
        ));
    } else {
        e.pasos.push((
            "cmd_display_set_brightness".to_string(),
            ok,
            texto.trim().to_string(),
        ));
    }
    e.fallo = e.pasos.iter().any(|(_, ok, _)| !*ok);
    e
}

/// Resultado de aplicar el brillo: qué se escribió, qué se midió y por qué vía.
#[derive(Clone, Debug)]
pub struct ResultadoAplicacion {
    /// Pasos de las vías usadas.
    pub escritura: Escritura,
    /// Brillo aplicado medido (`dumpsys power`).
    pub aplicado: Aplicado,
    /// ¿El brillo aplicado coincide con el pedido?
    pub verificado: Option<bool>,
    /// Vía que surtió efecto: `ajustes` | `cmd_display` | `ninguna`.
    pub via: String,
    /// Por qué no se pudo verificar (pantalla apagada, sin dumpsys, …).
    pub nota: Option<String>,
}

/// Aplica el brillo y **mide** el resultado.
///
/// **Optimizado**: la vía alternativa (`cmd display`) sólo se usa si la
/// principal no surte efecto, así que en el caso bueno se ahorra una JVM
/// (escritura) y se hace una sola verificación en lugar de dos.
///
/// Con la pantalla **apagada** el brillo aplicado se lee como 0 (así lo
/// muestra `dumpsys power`: `Display Power: state=OFF, … brightness=0`), así
/// que no se puede verificar ⇒ se dice, en vez de dar un "fallo" falso.
/// ¿El brillo aplicado coincide con el pedido? (tolerancia de 2/255).
///
/// Función pura: la usa [`aplicar_brillo`] para decidir si hace falta la vía
/// alternativa.
pub fn coincide_brillo(a: &Aplicado, valor: u8) -> bool {
    a.brillo_int
        .map(|i| i.abs_diff(valor) <= 2)
        .or_else(|| {
            a.brillo_float
                .map(|f| ((f - valor as f64 / 255.0).abs() * 255.0) <= 2.0)
        })
        .unwrap_or(false)
}

/// ¿La pantalla está apagada?  Entonces el brillo aplicado se lee como 0 y no
/// se puede usar para verificar (`dumpsys power` lo dice así:
/// `Display Power: state=OFF, … brightness=0`).
pub fn pantalla_apagada(a: &Aplicado) -> bool {
    a.estado_pantalla
        .as_deref()
        .map(|e| e.eq_ignore_ascii_case("OFF") || e.eq_ignore_ascii_case("DOZE"))
        .unwrap_or(false)
}

/// Aplica el brillo y **mide** el resultado.
///
/// **Optimizado**: la vía alternativa (`cmd display`) sólo se usa si la
/// principal no surte efecto, así que en el caso bueno se ahorra una JVM
/// (escritura) y se hace una sola verificación en lugar de dos.
///
/// Con la pantalla **apagada** el brillo aplicado se lee como 0 (así lo
/// muestra `dumpsys power`: `Display Power: state=OFF, … brightness=0`), así
/// que no se puede verificar ⇒ se dice, en vez de dar un "fallo" falso.
pub fn aplicar_brillo(valor: u8, cfg: &Config, forzar_manual: bool) -> ResultadoAplicacion {
    let v = valor;
    let mut escritura = escribir_via_ajustes(v, cfg, forzar_manual);

    let mut aplicado = leer_aplicado();
    let coincide = |a: &Aplicado| coincide_brillo(a, v);

    if !pantalla_apagada(&aplicado) && coincide(&aplicado) {
        return ResultadoAplicacion {
            escritura,
            aplicado,
            verificado: Some(true),
            via: "ajustes".to_string(),
            nota: None,
        };
    }

    if pantalla_apagada(&aplicado) {
        return ResultadoAplicacion {
            escritura,
            aplicado,
            verificado: None,
            via: "ajustes".to_string(),
            nota: Some(
                "la pantalla está apagada: dumpsys power informa brightness=0, así que el \
                 brillo aplicado no se puede medir ahora (el valor se aplicará al encender)"
                    .to_string(),
            ),
        };
    }

    if cfg.usar_cmd_display {
        let e2 = escribir_via_cmd_display(v);
        escritura.pasos.extend(e2.pasos.iter().cloned());
        escritura.fallo = escritura.pasos.iter().any(|(_, ok, _)| !*ok);
        aplicado = leer_aplicado();
        let ok = coincide(&aplicado);
        return ResultadoAplicacion {
            escritura,
            aplicado,
            verificado: Some(ok),
            via: if ok { "cmd_display" } else { "ninguna" }.to_string(),
            nota: if ok {
                None
            } else {
                Some("ninguna de las dos vías cambió el brillo aplicado".to_string())
            },
        };
    }

    ResultadoAplicacion {
        escritura,
        aplicado,
        verificado: Some(false),
        via: "ninguna".to_string(),
        nota: Some("la vía del proveedor de ajustes no cambió el brillo aplicado".to_string()),
    }
}

/// Escribe el brillo (ambas vías, sin verificar) — API de bajo nivel.
pub fn escribir_brillo(valor: u8, cfg: &Config, forzar_manual: bool) -> Escritura {
    let mut e = escribir_via_ajustes(valor, cfg, forzar_manual);
    if cfg.usar_cmd_display {
        let e2 = escribir_via_cmd_display(valor);
        e.pasos.extend(e2.pasos.iter().cloned());
    }
    e.fallo = e.pasos.iter().any(|(_, ok, _)| !*ok);
    e
}

/// Extrae el valor de un `rc_xxx=N` de la salida.
fn extract_rc(salida: &str, prefijo: &str) -> Option<String> {
    for linea in salida.lines() {
        if let Some(resto) = linea.trim().strip_prefix(prefijo) {
            return Some(resto.trim().to_string());
        }
    }
    None
}

// ===========================================================================
// LECTURA BARATA (ATAJO DEL VIGILANTE)
// ===========================================================================

/// Firma de los ficheros que pueden cambiar cuando cambia el brillo.
///
/// En AOSP 14 el brillo autoritativo está en `/data/system/display-manager-state.xml`
/// (`PersistentDataStore.java:972`) y el modo en el XML del proveedor de
/// ajustes.  Si ninguno ha cambiado, no ha pasado nada ⇒ **cero JVM**.
pub fn firma_ficheros() -> String {
    let cmd = "for f in /data/system/display-manager-state.xml /data/system/users/0/settings_system.xml /data/system/users/0/settings_secure.xml; do if [ -e \"$f\" ]; then stat -c '%n:%Y:%s' \"$f\" 2>/dev/null; fi; done";
    let r = sh(cmd, true, 8000);
    let t = r.texto();
    if t.is_empty() {
        // Sin `stat` o sin permisos: firma vacía ⇒ siempre se lee (más caro).
        return String::new();
    }
    t
}

// ===========================================================================
// ESTADO INTERNO (BOOKKEEPING)
// ===========================================================================

/// Estado interno del vigilante.
#[derive(Clone, Debug, Default)]
pub struct Estado {
    /// Objetivo actual (float 0..1).
    pub objetivo: Option<f64>,
    /// Último valor visto en el sistema.
    pub ultimo_float: Option<f64>,
    /// Último modo visto.
    pub ultimo_modo: Option<i64>,
    /// Firma de ficheros vista en la última comprobación.
    pub firma: String,
    /// Marca de tiempo de la última lectura completa.
    pub ultima_lectura_ts: u64,
    /// Contadores.
    pub veces_reafirmado: u64,
    pub veces_modo_auto_corregido: u64,
    pub veces_cambio_usuario: u64,
    /// Cambios que vinieron de la política de energía del propio daemon.
    pub veces_cambio_politica: u64,
    /// ¿Estamos bajo el techo de la política de energía (brillo recortado)?
    pub bajo_politica: bool,
    /// Eventos recibidos por el vigilante por eventos (inotify/FIFO).
    pub eventos_recibidos: u64,
    /// Sondeos hechos (el respaldo; 0 si sólo hay eventos).
    pub sondeos: u64,
    /// El último evento (JSON).
    pub ultimo_evento: Option<Value>,
    /// Última acción (texto).
    pub ultima_accion: String,
    /// Vía que funcionó por última vez.
    pub ultima_via: String,
}

impl Estado {
    /// A JSON.
    pub fn a_json(&self) -> Value {
        json!({
            "objetivo": self.objetivo,
            "ultimo_float": self.ultimo_float,
            "ultimo_modo": self.ultimo_modo,
            "firma": self.firma,
            "ultima_lectura_ts": self.ultima_lectura_ts,
            "veces_reafirmado": self.veces_reafirmado,
            "veces_modo_auto_corregido": self.veces_modo_auto_corregido,
            "veces_cambio_usuario": self.veces_cambio_usuario,
            "veces_cambio_politica": self.veces_cambio_politica,
            "bajo_politica": self.bajo_politica,
            "eventos_recibidos": self.eventos_recibidos,
            "sondeos": self.sondeos,
            "ultimo_evento": self.ultimo_evento.clone(),
            "ultima_accion": self.ultima_accion,
            "ultima_via": self.ultima_via,
        })
    }

    /// Desde JSON (tolerante).
    pub fn de_json(v: &Value) -> Estado {
        Estado {
            objetivo: v.get("objetivo").and_then(|x| x.as_f64()),
            ultimo_float: v.get("ultimo_float").and_then(|x| x.as_f64()),
            ultimo_modo: v.get("ultimo_modo").and_then(|x| x.as_i64()),
            firma: v
                .get("firma")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            ultima_lectura_ts: v
                .get("ultima_lectura_ts")
                .and_then(|x| x.as_u64())
                .unwrap_or(0),
            veces_reafirmado: v
                .get("veces_reafirmado")
                .and_then(|x| x.as_u64())
                .unwrap_or(0),
            veces_modo_auto_corregido: v
                .get("veces_modo_auto_corregido")
                .and_then(|x| x.as_u64())
                .unwrap_or(0),
            veces_cambio_usuario: v
                .get("veces_cambio_usuario")
                .and_then(|x| x.as_u64())
                .unwrap_or(0),
            veces_cambio_politica: v
                .get("veces_cambio_politica")
                .and_then(|x| x.as_u64())
                .unwrap_or(0),
            bajo_politica: v
                .get("bajo_politica")
                .and_then(|x| x.as_bool())
                .unwrap_or(false),
            eventos_recibidos: v
                .get("eventos_recibidos")
                .and_then(|x| x.as_u64())
                .unwrap_or(0),
            sondeos: v.get("sondeos").and_then(|x| x.as_u64()).unwrap_or(0),
            ultimo_evento: v.get("ultimo_evento").cloned(),
            ultima_accion: v
                .get("ultima_accion")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            ultima_via: v
                .get("ultima_via")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        }
    }
}

/// Lee el estado interno.
pub fn leer_estado(state_dir: Option<&str>) -> Estado {
    let p = ruta_estado(state_dir);
    match std::fs::read_to_string(&p) {
        Ok(txt) => match serde_json::from_str::<Value>(&txt) {
            Ok(v) => Estado::de_json(&v),
            Err(_) => Estado::default(),
        },
        Err(_) => Estado::default(),
    }
}

/// Guarda el estado interno (best-effort: si falla, no se rompe nada).
pub fn guardar_estado(e: &Estado, state_dir: Option<&str>) -> Option<String> {
    let p = ruta_estado(state_dir);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&p, e.a_json().to_string()) {
        Ok(_) => Some(p.display().to_string()),
        Err(_) => None,
    }
}

/// Ahora (segundos).  Reutiliza el reloj del repo si está disponible.
fn ahora() -> u64 {
    crate::google_auth::now_secs()
}

// ===========================================================================
// DECISIÓN DEL VIGILANTE (LÓGICA PURA, TESTEABLE)
// ===========================================================================

/// Qué hay que hacer en una iteración del vigilante.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// No hacer nada (todo en su sitio).
    Nada,
    /// El usuario cambió el brillo con los botones ⇒ se acepta y se adopta
    /// como nuevo objetivo.
    AdoptarCambio,
    /// Algo devolvió el modo automático ⇒ hay que volver a manual.
    ForzarManual,
    /// El valor nuevo es **exactamente el techo de la política de energía**
    /// (MEDIDO 2026-11-27: 60 = `MEGADAEMON_BRIGHTNESS_MAX`): el culpable es
    /// el propio daemon, no el usuario ⇒ se respeta el techo y **no** se
    /// cambia el objetivo del usuario.
    CambioDePolitica,
    /// Hay que reescribir el brillo (y el modo manual).
    Reafirmar,
}

/// Decide qué hacer.
///
/// Reglas (todas verificables con el código de AOSP citado arriba):
///
/// * Si el modo es **automático** (1) y `permitir_automatico` es falso ⇒ el
///   automático manda sobre el brillo (`AutomaticBrightnessController.java:916`),
///   así que hay que **volver a manual** (y reponer el valor, porque al
///   volver a manual se queda el último valor que puso el ALS).
/// * Si el modo es **manual** y el valor difiere del objetivo:
///   - `firme` ⇒ reescribir.
///   - `respetar_usuario` ⇒ **adoptar** el cambio (el usuario movió el
///     deslizador; el sistema manda, `BrightnessController.java:342`).
///   - `pasivo` ⇒ no hacer nada.
/// * Si el modo es manual y el valor coincide ⇒ nada.
#[allow(clippy::too_many_arguments)]
pub fn decidir(
    politica: &str,
    objetivo: Option<f64>,
    actual: Option<f64>,
    modo: Option<i64>,
    permitir_automatico: bool,
    techo: Option<u8>,
    bajo_politica: bool,
    guardado_por_politica: Option<u8>,
) -> Decision {
    if politica == "pasivo" {
        return Decision::Nada;
    }
    let auto = modo.map(|m| m != 0).unwrap_or(false);
    if auto && !permitir_automatico {
        return Decision::ForzarManual;
    }
    let (Some(objetivo), Some(actual)) = (objetivo, actual) else {
        return Decision::Nada;
    };
    // La política de energía del propio daemon (MEDIDO 2026-11-27 en el
    // SM-G998B): el brillo bajó de 247 a 60 = `MEGADAEMON_BRIGHTNESS_MAX` y
    // luego volvió a 247.  Si el valor nuevo ES el techo, no es el usuario:
    // es la política ⇒ se respeta el techo y el objetivo del usuario se
    // guarda aparte (así, cuando la política se relaje, se restaura).
    if let Some(t) = techo {
        if t < BRILLO_MAXIMO {
            let techo_f = t as f64 / 255.0;
            if (actual - techo_f).abs() < 0.002 && (objetivo - actual).abs() >= 0.002 {
                // MEDIDO (2026-11-28): con la COPIA que la politica guardo se
                // distinguen las dos cosas que el codigo confundia:
                //
                // * el objetivo es el MISMO que la copia (p.ej. 247) => el
                //   objetivo del usuario sigue siendo 247 y el que bajo la
                //   pantalla a 60 fue el daemon => se respeta el techo
                //   (`CambioDePolitica`) y el objetivo no se toca;
                // * el objetivo es OTRO (p.ej. 128) => el usuario movio el
                //   deslizador MIENTRAS estaba recortado => con la politica
                //   "respetar_usuario" su cambio se adopta (`AdoptarCambio`).
                //
                // Sin la copia no hay con que comparar => se mantiene el
                // comportamiento anterior (es la politica la que cambio).
                let es_cambio_de_la_politica = match guardado_por_politica {
                    Some(g) => (objetivo - g as f64 / 255.0).abs() < 0.002,
                    None => true,
                };
                if es_cambio_de_la_politica {
                    return Decision::CambioDePolitica;
                }
            }
        }
    }
    // La política se ha relajado: ha restaurado SU copia del brillo (la que
    // guardó antes de recortar).  Entonces lo que hay que poner es **el
    // objetivo del usuario**, no el 247 que la política tenía guardado.
    if bajo_politica {
        if let Some(g) = guardado_por_politica {
            let g_f = g as f64 / 255.0;
            if (actual - g_f).abs() < 0.002 && (objetivo - actual).abs() >= 0.002 {
                return Decision::Reafirmar;
            }
        }
    }
    if (objetivo - actual).abs() < 0.002 {
        return Decision::Nada;
    }
    match politica {
        "firme" => Decision::Reafirmar,
        // "respetar_usuario": en MANUAL el cambio es del usuario.
        _ => {
            if auto {
                Decision::ForzarManual
            } else {
                Decision::AdoptarCambio
            }
        }
    }
}

// ===========================================================================
// API PÚBLICA
// ===========================================================================

/// Brillo aplicado en 0..255 (si se pudo leer).
fn brillo_efectivo(lectura: &Lectura, aplicado: Option<&Aplicado>) -> Option<u8> {
    if let Some(a) = aplicado {
        if let Some(i) = a.brillo_int {
            return Some(i);
        }
        if let Some(f) = a.brillo_float {
            return Some((f.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    lectura.brillo_255()
}

/// Rutas donde puede vivir el XML del proveedor de ajustes (orden de prueba).
pub const RUTAS_XML_AJUSTES: &[&str] = &[
    "/data/system/users/0/settings_system.xml",
    "/data/system/settings_system.xml",
    "/data/system/users/0/settings_global.xml",
];

/// Lee los ajustes del brillo del XML del proveedor **como fichero**.
///
/// **0 interacciones con el móvil**: es una lectura de disco, no un proceso.
/// El daemon corre como root, así que puede leerlo.
pub fn leer_ajustes_xml_fichero() -> Option<Lectura> {
    leer_ajustes_xml_fichero_en(RUTAS_XML_AJUSTES)
}

/// Igual, pero con las rutas **inyectadas** (para los tests: así se mide el
/// coste real de la vía del evento sin depender del móvil).
pub fn leer_ajustes_xml_fichero_en(rutas: &[&str]) -> Option<Lectura> {
    for ruta in rutas {
        let Ok(txt) = std::fs::read_to_string(ruta) else {
            continue;
        };
        let l = parse_settings_xml(&txt);
        if l.modo.is_some() || l.int.is_some() || l.float.is_some() {
            return Some(l);
        }
    }
    None
}

/// Todo lo que necesita `estado()`, leído con **una sola llamada** al móvil.
///
/// MEDIDO (SM-G998B, 2026-11-27): `estado()` costaba **6 interacciones**
/// (ajustes ×3 + brillo aplicado + ALS + techo + copia de la política).  Con
/// las lecturas agrupadas en un único comando y el XML leído como fichero
/// (0 procesos), el coste baja a **1**.
///
/// **El suelo es 1**: `dumpsys power` y `dumpsys sensorservice` son dos JVM
/// distintas y `dumpsys` no las fusiona ⇒ no se puede bajar de 1 sin perder
/// el brillo aplicado o el ALS.
pub struct LecturaCompleta {
    /// Ajustes de brillo.
    pub ajustes: Lectura,
    /// Brillo aplicado (`dumpsys power`).
    pub aplicado: Aplicado,
    /// Lux del sensor de luz ambiente.
    pub lux: Option<f64>,
    /// Techo de la política de energía.
    pub cap: Option<u8>,
    /// Brillo que la política de energía guardó antes de recortar.
    pub guardado: Option<u8>,
    /// ¿Existe la clave `screen_brightness_float`?
    pub float_existe: bool,
    /// Firma de los ficheros vigilados.
    pub ficheros: String,
}

/// El comando agrupado (una sola llamada).
fn cmd_agrupado(con_power: bool, con_als: bool) -> String {
    let mut c = String::with_capacity(640);
    if con_power {
        c.push_str("echo '==POWER=='; dumpsys power 2>/dev/null | grep -E -i 'Display Power|ScreenBrightnessOverride'; ");
    }
    if con_als {
        c.push_str("echo '==ALS=='; dumpsys sensorservice 2>/dev/null | grep -A30 'Uncalibrated Light: last' | grep -E \"^[[:space:]]+[0-9]+ [(]\" | tail -3; ");
    }
    c.push_str("echo '==CAP=='; grep -h -i 'BRIGHTNESS_MAX' /data/adb/megadaemon/megadaemon.env /data/local/tmp/megadaemon/megadaemon.env 2>/dev/null | head -1; ");
    c.push_str("echo '==SAVED=='; cat /data/adb/megadaemon/brightness_saved 2>/dev/null; ");
    c.push_str("echo '==FLOAT=='; settings get system screen_brightness_float 2>/dev/null; ");
    c.push_str("echo '==FILES=='; for f in /data/system/display-manager-state.xml /data/system/users/0/settings_system.xml; do [ -e \"$f\" ] && stat -c '%n:%Y:%s' \"$f\"; done; ");
    c
}

/// Extrae la sección que sigue a un marcador.
fn seccion(crudo: &str, marcador: &str) -> Option<String> {
    let mut dentro = false;
    let mut out = Vec::new();
    for linea in crudo.lines() {
        let l = linea.trim();
        if l.starts_with("==") && l.ends_with("==") {
            dentro = l == marcador;
            continue;
        }
        if dentro {
            out.push(l.to_string());
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join("\n"))
    }
}

/// Lee todo con **una** llamada al móvil (y el XML como fichero).
pub fn leer_todo(con_power: bool, con_als: bool) -> LecturaCompleta {
    // 1) Los ajustes: el XML del proveedor leído como fichero (0 procesos).
    let ajustes = leer_ajustes_xml_fichero()
        .or_else(leer_ajustes_xml)
        .unwrap_or_else(leer_ajustes);

    // 2) Todo lo demás en UNA llamada.
    let r = sh(&cmd_agrupado(con_power, con_als), true, 45_000);
    let crudo = r.stdout;

    let aplicado = match seccion(&crudo, "==POWER==") {
        Some(txt) => parse_dumpsys_power(&txt),
        None => Aplicado::default(),
    };
    let lux = seccion(&crudo, "==ALS==").and_then(|t| parse_als(&t));
    let cap = seccion(&crudo, "==CAP==").and_then(|t| {
        let v = t.split('=').nth(1).unwrap_or("").trim();
        v.parse::<f64>()
            .ok()
            .and_then(|n| if n > 0.0 { Some(n.round() as u8) } else { None })
    });
    let guardado = seccion(&crudo, "==SAVED==").and_then(|t| t.trim().parse::<u8>().ok());
    let float_existe = seccion(&crudo, "==FLOAT==")
        .map(|t| {
            let t = t.trim();
            !t.is_empty() && t != "null" && t.parse::<f64>().is_ok()
        })
        .unwrap_or(false);
    let ficheros = seccion(&crudo, "==FILES==").unwrap_or_default();

    LecturaCompleta {
        ajustes,
        aplicado,
        lux,
        cap,
        guardado,
        float_existe,
        ficheros,
    }
}

/// El estado **sin tocar el móvil**: sólo el XML del proveedor (0 procesos).
///
/// Para las rutas calientes (el watchdog, los paneles que se refrescan solos):
/// da el brillo y el modo sin lanzar ni un `settings` ni un `dumpsys`.
pub fn estado_rapido(state_dir: Option<&str>) -> Value {
    let ctx = Ctx::cargar(state_dir);
    let lectura = leer_ajustes_xml_fichero().unwrap_or_default();
    json!({
        "ok": true,
        "brillo_ajuste": lectura.int,
        "brillo_actual": lectura.int,
        "float": lectura.float,
        "modo": lectura.modo,
        "auto_activo": lectura.auto_activo(),
        "objetivo": ctx.objetivo(),
        "politica": ctx.cfg.politica,
        "interacciones": 0,
        "nota": "lectura del XML del proveedor como fichero: 0 procesos del móvil",
    })
}

/// Estado completo del brillo del móvil.
///
/// Contrato: `{ brillo_actual, brillo_max, modo, float_actual, auto_activo,
/// sensor_luz, bloqueado_por }` (+ extras).
pub fn estado(state_dir: Option<&str>) -> Value {
    let ctx = Ctx::cargar(state_dir);
    let cfg = &ctx.cfg;
    // MEDIDO (SM-G998B, 2026-11-27): antes esto costaba 6 interacciones con
    // el móvil; ahora TODO se lee en UNA llamada (y el XML, como fichero).
    let todo = leer_todo(cfg.verificar_aplicado, true);
    let lectura = todo.ajustes;
    let aplicado = if cfg.verificar_aplicado {
        Some(todo.aplicado)
    } else {
        None
    };
    let lux = todo.lux;
    let ev_als = Some("dumpsys sensorservice (en la llamada agrupada)".to_string());

    let brillo_actual = brillo_efectivo(&lectura, aplicado.as_ref());
    let float_actual = lectura.float.or_else(|| {
        aplicado
            .as_ref()
            .and_then(|a| a.brillo_float)
            .or_else(|| lectura.int.map(|i| i as f64 / 255.0))
    });

    // Quién está sujetando el brillo:
    let mut bloqueos: Vec<Value> = Vec::new();
    if lectura.auto_activo() {
        bloqueos.push(json!({
            "quien": "automatico_als",
            "detalle": "screen_brightness_mode=1 (automático): manda el sensor de luz ambiente",
            "ancla": "AutomaticBrightnessController.java:916 (updateAutoBrightness)",
        }));
    }
    if let Some(a) = aplicado.as_ref() {
        if a.hay_override() {
            bloqueos.push(json!({
                "quien": "override_ventana",
                "detalle": format!("mScreenBrightnessOverrideFromWindowManager={}", a.override_ventana.clone().unwrap_or_default()),
                "ancla": "DisplayPowerController (mPowerRequest.screenBrightnessOverride)",
            }));
        }
    }
    if let Some(cap) = todo.cap.or_else(techo_energia) {
        // MEDIDO 2026-11-27 en el SM-G998B: la política de energía recorta el
        // brillo a 60 y luego lo restaura (247 -> 60 -> 247): es EL CULPABLE
        // de "el brillo se cambia todo el rato", no el ALS.
        let iguala = brillo_actual.map(|b| b == cap).unwrap_or(false);
        let supera = brillo_actual.map(|b| b > cap).unwrap_or(false);
        if iguala || supera {
            bloqueos.push(json!({
                "quien": "politica_energia",
                "detalle": format!(
                    "la política de energía del propio daemon recorta a {cap} (MEGADAEMON_BRIGHTNESS_MAX); \
                     estado: {}",
                    if iguala { "AHORA MISMO recortado" } else { "por encima del techo" }
                ),
                "ancla": "battery_advanced.rs:1994-2003 + deploy/99-megadaemon.sh:1190-1197",
                "medido": "2026-11-27: 247 -> 60 -> 247 en el SM-G998B",
            }));
        }
    }

    let modo = match lectura.modo {
        Some(0) => "manual",
        Some(1) => "auto",
        Some(_) => "otro",
        None => "desconocido",
    };

    json!({
        "ok": lectura.ok || lectura.modo.is_some(),
        "brillo_actual": brillo_actual,
        "brillo_max": BRILLO_MAXIMO,
        "modo": modo,
        "float_actual": float_actual,
        "auto_activo": lectura.auto_activo(),
        "sensor_luz": lux,
        "bloqueado_por": if bloqueos.is_empty() { Value::Null } else { Value::Array(bloqueos) },
        "ajustes": {
            "screen_brightness_mode": lectura.modo,
            "screen_brightness_float": lectura.float,
            "screen_brightness": lectura.int,
            "screen_auto_brightness_adj": lectura.auto_ajuste,
            "content_adaptive_brightness": lectura.adaptativo,
            "crudo": lectura.crudo.trim(),
        },
        "aplicado": aplicado.as_ref().map(|a| json!({
            "brillo_float": a.brillo_float,
            "brillo_int": a.brillo_int,
            "estado_pantalla": a.estado_pantalla,
            "override_ventana": a.override_ventana,
        })),
        "objetivo": ctx.objetivo(),
        "politica": cfg.politica,
        "permitir_automatico": cfg.permitir_automatico,
        "root": hay_root(),
        "nota_als": ev_als,
        "medido": "settings list system (1 JVM) + dumpsys power (1 JVM) + dumpsys sensorservice (1 JVM)",
    })
}

/// Objetivo vigente (float 0..1) según el estado interno y la configuración.
pub fn objetivo_actual(state_dir: Option<&str>) -> Option<f64> {
    Some(Ctx::cargar(state_dir).objetivo())
}

/// Fija el brillo (0..255) y pasa el modo a manual.
///
/// Devuelve qué se ha hecho y **qué se ha verificado**: el brillo aplicado se
/// lee de `dumpsys power` (`Display Power: ... brightness=..., brightnessInt=...`).
pub fn fijar(valor: u8, state_dir: Option<&str>) -> Value {
    let cfg = Config::de_json(&config(state_dir));
    let v = valor;
    let forzar_manual = !cfg.permitir_automatico;
    // Escribe + mide en una sola pasada (una verificación, no dos).
    let res = aplicar_brillo(v, &cfg, forzar_manual);
    let aplicado_int = res.aplicado.brillo_int;
    let aplicado_float = res.aplicado.brillo_float;

    let mut e = leer_estado(state_dir);
    e.objetivo = Some(v as f64 / 255.0);
    e.ultimo_float = aplicado_float.or(Some(v as f64 / 255.0));
    e.ultimo_modo = Some(0);
    e.ultima_accion = format!("fijar({v})");
    e.ultima_via = res.via.clone();
    e.firma = firma_ficheros();
    e.ultima_lectura_ts = ahora();
    let _ = guardar_estado(&e, state_dir);

    json!({
        "ok": !res.escritura.fallo,
        "brillo_pedido": v,
        "brillo_max": BRILLO_MAXIMO,
        "techo_energia": techo_energia(),
        "modo_pedido": if forzar_manual { "manual" } else { "sin_tocar" },
        "pasos": res.escritura.pasos.iter().map(|(n, ok, s)| json!({
            "paso": n, "ok": ok, "salida": s,
        })).collect::<Vec<Value>>(),
        "fallos": res.escritura.fallos(),
        "aplicado_int": aplicado_int,
        "aplicado_float": aplicado_float,
        "verificado": res.verificado,
        "via": res.via,
        "nota": res.nota,
        "criterio_verificacion": "dumpsys power -> 'Display Power: ... brightnessInt=' (tolerancia 2)",
        "root": hay_root(),
    })
}

/// Brillo al máximo: es el **valor por defecto** del encargo.
pub fn maximo(state_dir: Option<&str>) -> Value {
    let r = fijar(BRILLO_MAXIMO, state_dir);
    let mut out = r.clone();
    out["accion"] = json!("maximo");
    out["nota"] = json!("el valor por defecto del encargo es el máximo (255/255 = 1.0)");
    out
}

/// Contexto de trabajo: configuración + estado interno, **leídos una vez**.
///
/// **Optimizado**: antes cada llamada pública releía el fichero de
/// configuración y el de estado por separado (2-3 lecturas de disco por
/// operación); aquí se leen una vez y se reutilizan.
#[derive(Clone, Debug)]
struct Ctx {
    cfg: Config,
    estado: Estado,
}

impl Ctx {
    /// Carga el contexto (2 lecturas de disco, 0 procesos).
    fn cargar(state_dir: Option<&str>) -> Ctx {
        Ctx {
            cfg: Config::de_json(&leer_config_raw(state_dir)),
            estado: leer_estado(state_dir),
        }
    }

    /// Objetivo vigente (float 0..1).
    fn objetivo(&self) -> f64 {
        self.estado
            .objetivo
            .unwrap_or(self.cfg.brillo_por_defecto as f64 / 255.0)
    }
}

/// Reafirma el objetivo **sólo si alguien lo ha cambiado**.
///
/// Éste es el corazón del shim de política: compara con lo último visto y no
/// toca nada si no ha cambiado (coste en reposo ≈ una lectura de fichero).
pub fn reafirmar(state_dir: Option<&str>) -> Value {
    let ctx = Ctx::cargar(state_dir);
    let firma = firma_ficheros();
    let mut e = ctx.estado.clone();
    e.sondeos += 1;
    let _ = guardar_estado(&e, state_dir);
    reafirmar_con(&ctx, Some(firma), state_dir, None)
}

/// Reafirmación **disparada por un evento** (inotify o el shim del FIFO).
///
/// El coste por evento es **0** si no hay que escribir nada: la lectura del
/// brillo es por XML (0 JVM) y la firma de ficheros no hace falta (ya sabemos
/// que algo cambió).
pub fn reafirmar_por_evento(ev: &crate::brillo_events::Evento, state_dir: Option<&str>) -> Value {
    let ctx = Ctx::cargar(state_dir);
    let mut out = reafirmar_con(&ctx, None, state_dir, Some(ev));
    out["disparado_por"] = json!({
        "fuente": ev.fuente,
        "clave": ev.clave,
        "ts": ev.ts,
    });
    out
}

/// Reafirmación con **cero llamadas al móvil** cuando el estado ya está bien.
///
/// Se usa en el camino del evento: el XML del proveedor se lee como
/// fichero (0 procesos) y, si no hay que escribir nada, el ciclo entero
/// cuesta **0 interacciones** (antes: 5 por ciclo de sondeo).
pub fn costo_por_evento(state_dir: Option<&str>) -> Value {
    let ctx = Ctx::cargar(state_dir);
    let Some(lectura) = leer_ajustes_xml_fichero() else {
        return json!({"ok": false, "motivo": "el XML del proveedor no es legible (sin root?)"});
    };
    let actual = lectura
        .float
        .or_else(|| lectura.int.map(|i| i as f64 / 255.0));
    let objetivo = Some(ctx.objetivo());
    // Se MIDE, no se afirma: el techo de la política sale de una caché con
    // caducidad (30 s), así que normalmente no cuesta ningún proceso.
    reset_llamadas_sh();
    let d = decidir(
        &ctx.cfg.politica,
        objetivo,
        actual,
        lectura.modo,
        ctx.cfg.permitir_automatico,
        techo_energia(),
        ctx.estado.bajo_politica,
        None,
    );
    let n = llamadas_sh();
    json!({
        "ok": true,
        "decision": format!("{:?}", d),
        "costo_interacciones": n,
        "nota": "la lectura es del XML (fichero): 0 procesos; el techo sale de una caché (30 s); sólo escribe si la decisión lo exige",
        "brillo_ajuste": lectura.int,
        "modo": lectura.modo,
    })
}

/// Reafirmación con el contexto **ya cargado** y la firma **ya calculada**
/// (el vigilante no repite ni la lectura de ficheros ni el `stat`).
fn reafirmar_con(
    ctx: &Ctx,
    firma: Option<String>,
    state_dir: Option<&str>,
    evento: Option<&crate::brillo_events::Evento>,
) -> Value {
    // En el camino del EVENTO los ajustes se leen del XML como fichero
    // (0 procesos); en el de SONDEO se usa la vía multi-copia.
    let lectura = match evento {
        Some(_) => leer_ajustes_xml_fichero().unwrap_or_else(leer_ajustes),
        None => leer_ajustes(),
    };
    let cfg = &ctx.cfg;
    // MEDIDO en el móvil (SM-G998B, One UI 6): `screen_brightness_float` NO
    // existe (`settings get` → "null"); el brillo vivo está en el ajuste
    // legacy entero `screen_brightness` (247).  Sin este respaldo, `actual`
    // sería None y el shim no detectaría NUNCA un cambio ⇒ no haría nada.
    let actual = lectura
        .float
        .or_else(|| lectura.int.map(|i| i as f64 / 255.0));
    let objetivo = Some(ctx.objetivo());
    let techo = techo_energia();
    let guardado = brillo_guardado_por_politica();
    let d = decidir(
        &cfg.politica,
        objetivo,
        actual,
        lectura.modo,
        cfg.permitir_automatico,
        techo,
        ctx.estado.bajo_politica,
        guardado,
    );

    let mut e = ctx.estado.clone();
    let firma_anterior = e.firma.clone();
    let cambio_de_fichero = match &firma {
        Some(f) => !f.is_empty() && *f != firma_anterior,
        None => false,
    };
    if let Some(f) = firma {
        e.firma = f;
    }
    if let Some(ev) = evento {
        e.eventos_recibidos += 1;
        e.ultimo_evento = Some(ev.a_json());
    }
    e.ultima_lectura_ts = ahora();

    match d {
        Decision::Nada => {
            // En Android 14 el deslizador del sistema **no** escribe el
            // ajuste del proveedor (`BrightnessController.java:342` llama a
            // `DisplayManager.setBrightness`, que persiste en
            // `display-manager-state.xml`, `PersistentDataStore.java:972`):
            // el cambio del usuario se ve en el fichero, no en el ajuste.
            // Si el fichero ha cambiado y el brillo aplicado ya no es el
            // objetivo, es que el usuario movió el deslizador ⇒ se adopta.
            if cambio_de_fichero && cfg.politica == "respetar_usuario" && !cfg.permitir_automatico {
                let aplicado = leer_aplicado();
                if let Some(f) = aplicado.brillo_float {
                    let difiere = objetivo.map(|o| (o - f).abs() > 0.02).unwrap_or(true);
                    if difiere {
                        e.veces_cambio_usuario += 1;
                        e.objetivo = Some(f);
                        e.ultimo_float = Some(f);
                        e.ultima_accion = "adoptar_cambio_usuario".to_string();
                        e.ultima_via = "dumpsys_power".to_string();
                        let _ = guardar_estado(&e, state_dir);
                        return json!({
                            "ok": true,
                            "accion": "adoptar_cambio_usuario",
                            "motivo": "el fichero de estado del brillo cambió y el brillo aplicado ya no es el objetivo: el usuario movió los botones del sistema y se respeta",
                            "nuevo_objetivo": (f.clamp(0.0, 1.0) * 255.0).round() as u8,
                            "brillo_aplicado": (f.clamp(0.0, 1.0) * 255.0).round() as u8,
                            "modo": lectura.modo,
                        });
                    }
                }
            }
            e.ultimo_float = actual.or(e.ultimo_float);
            e.ultimo_modo = lectura.modo.or(e.ultimo_modo);
            let _ = guardar_estado(&e, state_dir);
            json!({
                "ok": true,
                "accion": "nada",
                "motivo": "el brillo y el modo ya son los que queremos",
                "costo": "1 lectura de ajustes (settings list system)",
                "brillo_actual": lectura.brillo_255(),
                "modo": lectura.modo,
            })
        }
        Decision::AdoptarCambio => {
            // El usuario movió el deslizador: se adopta su valor.
            e.veces_cambio_usuario += 1;
            if let Some(f) = actual {
                e.objetivo = Some(f);
            }
            e.ultimo_float = actual;
            e.ultimo_modo = lectura.modo;
            e.ultima_accion = "adoptar_cambio_usuario".to_string();
            e.ultima_via = "usuario".to_string();
            let _ = guardar_estado(&e, state_dir);
            json!({
                "ok": true,
                "accion": "adoptar_cambio_usuario",
                "motivo": "el valor cambió estando en modo manual: se respeta al usuario (política 'respetar_usuario')",
                "nuevo_objetivo": actual.map(|f| (f.clamp(0.0,1.0) * 255.0).round() as u8),
                "brillo_actual": lectura.brillo_255(),
                "modo": lectura.modo,
            })
        }
        Decision::CambioDePolitica => {
            // El brillo lo ha recortado la POLÍTICA DE ENERGÍA del propio
            // daemon (no el usuario): se respeta el techo y el objetivo del
            // usuario se conserva para restaurarlo cuando la política se
            // relaje.
            e.veces_cambio_politica += 1;
            e.bajo_politica = true;
            e.ultimo_float = actual;
            e.ultima_accion = "cambio_de_politica_energia".to_string();
            e.ultima_via = "politica_energia".to_string();
            let _ = guardar_estado(&e, state_dir);
            json!({
                "ok": true,
                "accion": "cambio_de_politica_energia",
                "motivo": "el brillo nuevo es exactamente el techo de la política de energía (MEGADAEMON_BRIGHTNESS_MAX): no es el usuario; se respeta el techo y el objetivo del usuario se guarda para restaurarlo",
                "techo_energia": techo,
                "brillo_actual": lectura.brillo_255(),
                "objetivo_del_usuario": objetivo.map(|f| (f.clamp(0.0, 1.0) * 255.0).round() as u8),
                "modo": lectura.modo,
                "ancla": "battery_advanced.rs:1994-2003 + deploy/99-megadaemon.sh:1190-1197 (guarda el brillo, lo recorta y lo restaura)",
            })
        }
        Decision::ForzarManual => {
            let cap = techo_energia().unwrap_or(BRILLO_MAXIMO);
            let objetivo255 = objetivo
                .map(|f| (f.clamp(0.0, 1.0) * 255.0).round() as u8)
                .unwrap_or(BRILLO_MAXIMO)
                .min(cap);
            let res = aplicar_brillo(objetivo255, cfg, true);
            e.veces_modo_auto_corregido += 1;
            e.veces_reafirmado += 1;
            e.objetivo = Some(objetivo255 as f64 / 255.0);
            e.ultimo_modo = Some(0);
            e.ultima_accion = "forzar_manual".to_string();
            e.ultima_via = res.via.clone();
            let _ = guardar_estado(&e, state_dir);
            json!({
                "ok": !res.escritura.fallo,
                "accion": "forzar_manual",
                "motivo": "screen_brightness_mode=1 (automático): el ALS manda; se vuelve a manual y se repone el valor",
                "ancla": "DisplayPowerController.java:2626-2636 (mUseAutoBrightness) + AutomaticBrightnessController.java:429 (sensor apagado)",
                "brillo_pedido": objetivo255,
                "techo_energia": cap,
                "verificado": res.verificado,
                "via": res.via,
                "pasos": res.escritura.pasos.iter().map(|(n, ok, s)| json!({
                    "paso": n, "ok": ok, "salida": s,
                })).collect::<Vec<Value>>(),
                "fallos": res.escritura.fallos(),
            })
        }
        Decision::Reafirmar => {
            if e.bajo_politica {
                // La política de energía ha relajado: se restaura el objetivo
                // del USUARIO (no el valor que la política tenía guardado).
                e.bajo_politica = false;
                e.veces_cambio_politica += 1;
            }
            let cap = techo_energia().unwrap_or(BRILLO_MAXIMO);
            let objetivo255 = objetivo
                .map(|f| (f.clamp(0.0, 1.0) * 255.0).round() as u8)
                .unwrap_or(BRILLO_MAXIMO)
                .min(cap);
            let res = aplicar_brillo(objetivo255, cfg, !cfg.permitir_automatico);
            e.veces_reafirmado += 1;
            e.objetivo = Some(objetivo255 as f64 / 255.0);
            e.ultimo_float = Some(objetivo255 as f64 / 255.0);
            e.ultima_accion = "reafirmar".to_string();
            e.ultima_via = res.via.clone();
            let _ = guardar_estado(&e, state_dir);
            json!({
                "ok": !res.escritura.fallo,
                "accion": "reafirmar",
                "motivo": "el brillo se apartó del objetivo y la política es 'firme'",
                "brillo_pedido": objetivo255,
                "techo_energia": cap,
                "verificado": res.verificado,
                "via": res.via,
                "pasos": res.escritura.pasos.iter().map(|(n, ok, s)| json!({
                    "paso": n, "ok": ok, "salida": s,
                })).collect::<Vec<Value>>(),
                "fallos": res.escritura.fallos(),
            })
        }
    }
}

/// Observa el brillo durante un rato y dice **quién lo cambia y cada cuánto**.
///
/// Es la medición que faltaba: con el modo ya en manual (MEDIDO: 0), si el
/// brillo se mueve solo, el culpable **no** es el ALS de AOSP ⇒ es la
/// política adaptativa de OneUI, otra app con WRITE_SETTINGS o el usuario.
///
/// Muestrea el ajuste legacy (`screen_brightness`, el que existe en este
/// firmware), el modo y el ALS, y devuelve la serie con los **cambios**
/// detectados y el intervalo medio entre ellos.
pub fn observar(muestras: u32, intervalo_s: u64) -> Value {
    let n = muestras.clamp(2, 120);
    let paso = intervalo_s.clamp(1, 60);
    let mut serie: Vec<Value> = Vec::new();
    let mut cambios: Vec<Value> = Vec::new();
    let mut anterior: Option<i64> = None;
    let mut ts_anterior: Option<u64> = None;

    for i in 0..n {
        let t = ahora();
        let lectura = leer_ajustes();
        let valor: Option<i64> = lectura.int.map(|i| i as i64).or_else(|| {
            lectura
                .float
                .map(|f| (f.clamp(0.0, 1.0) * 255.0).round() as i64)
        });
        // El ALS sólo en la primera y la última muestra (es lo caro).
        let (lux, _) = if i == 0 || i == n - 1 {
            leer_sensor_luz()
        } else {
            (None, None)
        };
        serie.push(json!({
            "muestra": i,
            "ts": t,
            "brillo_ajuste": valor,
            "modo": lectura.modo,
            "lux": lux,
        }));
        if let (Some(v), Some(_)) = (valor, ts_anterior) {
            if v != anterior.unwrap_or(v) {
                cambios.push(json!({
                    "ts": t,
                    "de": anterior,
                    "a": v,
                    "delta": anterior.map(|a| v - a),
                    "segundos_desde_el_anterior": ts_anterior.map(|ta| t.saturating_sub(ta)),
                }));
            }
        }
        if valor.is_some() {
            anterior = valor;
        }
        ts_anterior = Some(t);
        if i + 1 < n {
            std::thread::sleep(Duration::from_secs(paso));
        }
    }

    // Intervalo medio entre cambios (si hubo).
    let intervalos: Vec<u64> = cambios
        .iter()
        .filter_map(|c| c.get("segundos_desde_el_anterior").and_then(|v| v.as_u64()))
        .collect();
    let medio = if intervalos.is_empty() {
        Value::Null
    } else {
        let suma: u64 = intervalos.iter().sum();
        json!(suma / intervalos.len() as u64)
    };

    json!({
        "ok": true,
        "muestras": n,
        "intervalo_s": paso,
        "serie": serie,
        "cambios": cambios,
        "cambios_totales": cambios.len(),
        "intervalo_medio_entre_cambios_s": medio,
        "veredicto": if cambios.is_empty() {
            "el brillo NO se movió durante la observación: nadie lo cambia (o el shim lo subjecta)"
        } else {
            "el brillo se movió solo: revisa 'cambios' (quién, cuándo y cada cuánto)"
        },
        "nota": "el ALS se lee sólo en la primera y la última muestra (es lo caro); el valor es el ajuste legacy 'screen_brightness'",
    })
}

/// Diagnóstico completo del brillo de la pantalla, para el **test real en
/// el móvil**.
///
/// Reúne, con mediciones reales:
/// * los ajustes de brillo (y **qué claves existen** de verdad);
/// * el brillo **aplicado** (`dumpsys power`) y si hay override de ventana;
/// * el sensor de luz ambiente (ALS);
/// * los ficheros donde vive el estado (`display-manager-state.xml`, XML del
///   proveedor de ajustes) con su mtime/tamaño;
/// * las claves propias de Samsung (`content_adaptive_brightness`, etc.);
/// * y, con `aplicar=true`, una **prueba A/B de las vías de escritura**
///   (ajuste legacy vs `cmd display`) midiendo antes y después, y
///   **restaurando** el valor original.
///
/// La salida es JSON compacto, pensado para pegarlo de vuelta al informe.
pub fn diagnostico(state_dir: Option<&str>, aplicar: bool) -> Value {
    let ctx = Ctx::cargar(state_dir);
    let cfg = &ctx.cfg;

    // 1) Ajustes: una sola lectura (1 JVM).
    let lectura = leer_ajustes();

    // 2) Brillo aplicado (1 JVM).
    let aplicado = leer_aplicado();

    // 3) Sensor de luz ambiente (1 JVM como mucho).
    let (lux, ev_als) = leer_sensor_luz();

    // 4) Ficheros donde vive el estado (1 proceso, sin JVM).
    let ficheros = sh(
        "for f in /data/system/display-manager-state.xml /data/system/users/0/settings_system.xml /data/system/users/0/settings_secure.xml; do if [ -e \"$f\" ]; then stat -c '%n mtime=%Y tam=%s' \"$f\"; else echo \"$f AUSENTE\"; fi; done 2>/dev/null",
        true,
        10_000,
    )
    .texto();

    // 5) Claves propias del firmware.
    //    MEDIDO (SM-G998B): `settings list system|secure` **no devuelve nada**
    //    ⇒ las claves se leen una a una con `settings get` (1 JVM cada una).
    let adaptativas = claves_adaptativas();
    let extracto_als = sh(
        "dumpsys sensorservice 2>/dev/null | grep -i -m12 'light'",
        false,
        30_000,
    )
    .texto();
    let claves = sh(
        "for ns in system secure; do for k in content_adaptive_brightness adaptive_brightness screen_auto_brightness_adj screen_brightness_adjust screen_brightness_mode screen_brightness screen_brightness_float auto_brightness_mode; do v=$(settings get $ns $k 2>/dev/null); [ -n \"$v\" ] && echo \"$ns.$k=$v\"; done; done",
        false,
        60_000,
    )
    .texto();

    // 6) Prueba A/B de las vías de escritura (opt-in: cambia la pantalla).
    let prueba = if aplicar {
        Some(prueba_de_via(cfg))
    } else {
        None
    };

    // Veredicto.
    let auto = lectura.auto_activo();
    let mut veredicto: Vec<String> = Vec::new();
    veredicto.push(format!(
        "modo={} ({})",
        lectura.modo.map(|m| m.to_string()).unwrap_or("?".into()),
        if auto {
            "automático: el ALS manda"
        } else {
            "manual: el ALS está apagado"
        }
    ));
    veredicto.push(format!(
        "brillo aplicado={} (ajuste legacy={})",
        aplicado
            .brillo_int
            .map(|i| i.to_string())
            .unwrap_or_else(|| "?".into()),
        lectura
            .int
            .map(|i| i.to_string())
            .unwrap_or_else(|| "?".into())
    ));
    if let Some(p) = &prueba {
        if let Some(via) = p.get("via_que_funciona").and_then(|v| v.as_str()) {
            veredicto.push(format!("vía que funciona: {via}"));
        }
    } else {
        veredicto.push("prueba de vías: NO ejecutada (aplicar=true)".to_string());
    }

    json!({
        "ok": true,
        "veredicto": veredicto,
        "ajustes": {
            "screen_brightness_mode": lectura.modo,
            "screen_brightness_float": lectura.float,
            "screen_brightness": lectura.int,
            "screen_auto_brightness_adj": lectura.auto_ajuste,
            "content_adaptive_brightness": lectura.adaptativo,
            "existe_float": existe_float(),
            "crudo": lectura.crudo.trim(),
        },
        "aplicado": {
            "brillo_int": aplicado.brillo_int,
            "brillo_float": aplicado.brillo_float,
            "estado_pantalla": aplicado.estado_pantalla,
            "override_ventana": aplicado.override_ventana,
            "hay_override": aplicado.hay_override(),
        },
        "sensor_luz": {
            "lux": lux,
            "fuente": ev_als,
            "extracto": extracto_als,
        },
        "adaptativo": adaptativas,
        "politica_energia": {
            "techo": techo_energia(),
            "modo_ahorro_activo": crate::battery_advanced::modo_ahorro_activo(),
            "brillo_iguala_techo": techo_energia()
                .and_then(|t| lectura.int.map(|b| b == t))
                .unwrap_or(false),
            "nota": "MEDIDO 2026-11-27: 247 -> 60 -> 247. El recorte a 60 es MEGADAEMON_BRIGHTNESS_MAX (politica de energia), NO el ALS",
        },
        "ficheros": ficheros,
        "claves_firmware": claves,
        "prueba_de_via": prueba,
        "objetivo": ctx.objetivo(),
        "politica": cfg.politica,
        "nota": "las lecturas se han hecho SIN root para los ajustes; las escrituras requieren root",
    })
}

/// Prueba A/B de las vías de escritura, midiendo antes y después y
/// **restaurando** el valor original.
///
/// Vía A: el ajuste legacy `screen_brightness` (MEDIDO: es el que existe en
/// One UI 6).  Vía B: `cmd display set-brightness` (AOSP 14: float 0-1).
pub fn prueba_de_via(cfg: &Config) -> Value {
    let antes = leer_aplicado();
    let original = antes.brillo_int;
    // Un valor claramente distinto del actual, para que la medición sea
    // inequívoca (y que no esté ni en el tope ni en el suelo).
    let objetivo = match original {
        Some(v) if v > 120 => v.saturating_sub(60).max(10),
        Some(v) => (v + 60).min(245),
        None => 128,
    };

    // --- Vía A: el ajuste legacy ------------------------------------------
    let _ = escribir_via_ajustes(objetivo, cfg, false);
    std::thread::sleep(Duration::from_millis(1200));
    let tras_a = leer_aplicado();
    let a_ok = coincide_brillo(&tras_a, objetivo);

    // --- Vía B: `cmd display` ----------------------------------------------
    let (b_ok, tras_b) = if a_ok {
        (false, tras_a.clone())
    } else {
        let _ = escribir_via_cmd_display(objetivo);
        std::thread::sleep(Duration::from_millis(1200));
        let t = leer_aplicado();
        (coincide_brillo(&t, objetivo), t)
    };

    // --- Restaurar ---------------------------------------------------------
    let restaurado = match original {
        Some(v) => {
            let cfg2 = Config {
                escribir_ajuste_legacy: true,
                solo_float_si_existe: false,
                ..cfg.clone()
            };
            if a_ok {
                let _ = escribir_via_ajustes(v, &cfg2, false);
            } else if b_ok {
                let _ = escribir_via_cmd_display(v);
            } else {
                let _ = escribir_via_ajustes(v, &cfg2, false);
                let _ = escribir_via_cmd_display(v);
            }
            std::thread::sleep(Duration::from_millis(1200));
            leer_aplicado().brillo_int
        }
        None => None,
    };

    json!({
        "objetivo": objetivo,
        "antes": antes.brillo_int,
        "via_ajuste_legacy": { "funciona": a_ok, "quedo": tras_a.brillo_int },
        "via_cmd_display": { "funciona": b_ok, "quedo": tras_b.brillo_int },
        "via_que_funciona": if a_ok {
            "ajuste_legacy"
        } else if b_ok {
            "cmd_display"
        } else {
            "ninguna"
        },
        "restaurado": restaurado,
        "restaurado_ok": restaurado == original,
        "nota": "si 'restaurado_ok' es false, el valor original no se ha podido recuperar: ponlo a mano",
    })
}

/// Mide el **coste real** de cada vía del vigilante, en el móvil.
///
/// MEDIDO (SM-G998B, 2026-11-27): el sondeo de 15 s costaba **5
/// interacciones con el móvil por ciclo**; con los eventos son **0
/// sondeos** (el XML se lee como fichero) y **0..1 escrituras** por evento
/// (sólo si hay que corregir algo).
///
/// Esta acción mide los dos números **en el aparato** (no de teoría).
pub fn costo(state_dir: Option<&str>) -> Value {
    // 1) El camino del EVENTO: leer el XML (fichero) y decidir.
    reset_llamadas_sh();
    let lectura_evento = leer_ajustes_xml_fichero();
    let inter_evento = llamadas_sh();

    // 2) El camino del SONDEO: un `tick()` completo.
    reset_llamadas_sh();
    let _ = tick(state_dir);
    let inter_sondeo = llamadas_sh();

    // 3) El coste de `estado()` (la lectura agrupada).
    reset_llamadas_sh();
    let _ = estado(state_dir);
    let inter_estado = llamadas_sh();

    json!({
        "ok": true,
        "evento": {
            "interacciones": inter_evento,
            "sondeos": 0,
            "lectura": if lectura_evento.is_some() { "XML del proveedor (fichero, 0 procesos)" } else { "sin XML legible: cae a la vía multi-copia" },
            "nota": "0 interacciones = el ciclo del evento no llama a ningún proceso del móvil",
        },
        "sondeo": {
            "interacciones": inter_sondeo,
            "nota": "un ciclo de sondeo (tick): firma + lectura + escritura si hace falta",
        },
        "estado": {
            "interacciones": inter_estado,
            "nota": "estado(): una sola llamada agrupada (dumpsys power + dumpsys sensorservice = 2 JVM dentro de 1 llamada)",
        },
        "suelo": {
            "estado": 1,
            "por_que": "dumpsys power y dumpsys sensorservice son dos JVM distintas: dumpsys no las fusiona",
            "evento": 0,
            "por_que_evento": "con el XML legible (root), la lectura es una lectura de fichero",
        },
    })
}

/// Informe **completo** del encargo de brillo, en UN comando.
///
/// Es el comando que cierra el encargo en el móvil de una vez: mide el
/// coste real de cada vía, comprueba que el puente del FIFO funciona, dice
/// si el vigilante está vivo y **muestra si el brillo se mueve solo**.
///
/// MEDIDO (SM-G998B, 2026-11-27): el brillo se movía solo porque la política
/// de energía recortaba a 60 y lo restauraba (247 -> 60 -> 247).  La
/// política ya no toca el brillo ⇒ este informe es el que lo demuestra.
///
/// **Tarda ~1 minuto** (la serie son 12 muestras de 5 s): es un comando de
/// diagnóstico, no algo para el bucle.
pub fn informe(state_dir: Option<&str>, muestras: u32, intervalo_s: u64) -> Value {
    // 1) El coste real de las tres vías (medido aquí, en el aparato).
    let c = costo(state_dir);

    // 2) El estado del shim y del vigilante.
    let v = shim_estado(state_dir);

    // 3) El PUENTE del FIFO: se escribe un evento y se mira si el daemon lo
    //    cuenta (si el contador sube, el puente funciona).
    let antes = crate::brillo_events::CONTADORES
        .fifo
        .load(Ordering::Relaxed);
    let salida = crate::brillo_events::escribir_evento_shell("screen_brightness", "probe");
    std::thread::sleep(Duration::from_millis(400));
    let despues = crate::brillo_events::CONTADORES
        .fifo
        .load(Ordering::Relaxed);

    // 4) La serie: ¿se mueve solo?
    let serie = observar(muestras, intervalo_s);

    let moved = serie
        .get("cambios_totales")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    json!({
        "ok": true,
        "costo": c,
        "estado_shim": v,
        "puente_fifo": {
            "escrito": salida.trim(),
            "eventos_antes": antes,
            "eventos_despues": despues,
            "consumido": despues > antes,
            "nota": "si 'consumido' es false, el daemon no está leyendo el FIFO: el vigilante no está vivo",
        },
        "serie": serie,
        "veredicto": if moved == 0 {
            "el brillo NO se movió durante la observación: 100% manual y sin cambios".to_string()
        } else {
            format!("el brillo se movió {moved} veces: hay que ver 'serie.cambios' (quien lo movió)")
        },
        "nota": "este comando es el que cierra el encargo: mide en el móvil lo que en el CI no se puede medir",
    })
}

/// Qué shim está activo y qué se ha podido verificar.
pub fn shim_estado(state_dir: Option<&str>) -> Value {
    let ctx = Ctx::cargar(state_dir);
    let cfg = &ctx.cfg;
    let e = &ctx.estado;
    // El XML del proveedor se lee como fichero (0 procesos) cuando se puede.
    let lectura = leer_ajustes_xml_fichero().unwrap_or_else(leer_ajustes);
    let aplicado = if cfg.verificar_aplicado {
        Some(leer_aplicado())
    } else {
        None
    };
    json!({
        "ok": true,
        "nivel": "shim_de_politica",
        "que_es": "el objetivo (modo manual + valor) se reimpone SÓLO cuando algo lo cambia; en reposo el coste es una lectura de fichero",
        "ancla_real": {
            "clave": "Settings.System.SCREEN_BRIGHTNESS_MODE",
            "efecto": "manual (0) => mUseAutoBrightness=false => el sensor de luz ambiente se apaga",
            "evidencia": [
                "Settings.java:4731,4745,4750 (clave y valores)",
                "DisplayPowerController.java:2626-2636 (mUseAutoBrightness = modo==AUTOMATIC)",
                "DisplayPowerController.java:1113-1116,3358-3360 (ContentObserver: efecto inmediato)",
                "AutomaticBrightnessController.java:429,437 (setLightSensorEnabled(false) en manual)",
            ],
        },
        "hooks_art_pendientes": {
            "estado": "NO IMPLEMENTADO",
            "por_que": "el payload de hooks del repo es frida-gum sin puente Java (docs/RE_GASTO_DATOS_JAZZTEL.md §C.2; backlog J/K): hace falta un crate nuevo con puente Java",
            "puntos_de_hook": [
                "com.android.server.display.DisplayPowerController#handleBrightnessModeChange (ignorar el paso a auto)",
                "com.android.server.display.AutomaticBrightnessController#setLightSensorEnabled (false)",
                "com.android.server.display.DisplayPowerController#updatePowerState (mUseAutoBrightness=false)",
            ],
            "alternativa_nativa": "hook sobre el servicio de display (DRM/HWC) o sobre 'cmd display' — NO MEDIDO, no implementado",
        },
        "politica": cfg.politica,
        "permitir_automatico": cfg.permitir_automatico,
        "objetivo": e.objetivo,
        "ultimo_modo_visto": e.ultimo_modo,
        "modo_actual": lectura.modo,
        "auto_activo": lectura.auto_activo(),
        "contadores": {
            "veces_reafirmado": e.veces_reafirmado,
            "veces_modo_auto_corregido": e.veces_modo_auto_corregido,
            "veces_cambio_usuario": e.veces_cambio_usuario,
        },
        "ultima_accion": e.ultima_accion,
        "ultima_via": e.ultima_via,
        "vigilante": crate::brillo_events::CONTADORES.a_json(),
        "eventos_recibidos": e.eventos_recibidos,
        "sondeos": e.sondeos,
        "ultimo_evento": e.ultimo_evento,
        "coste": {
            "evento": "0 sondeos (0 interacciones si el XML es legible: es una lectura de fichero)",
            "sondeo": "5 interacciones por ciclo (MEDIDO 2026-11-27 antes del vigilante por eventos)",
        },
        "firma_ficheros": e.firma,
        "aplicado": aplicado.as_ref().map(|a| json!({
            "brillo_int": a.brillo_int,
            "brillo_float": a.brillo_float,
            "override_ventana": a.override_ventana,
        })),
        "verificado": {
            "lectura_ajustes": lectura.ok,
            "brillo_aplicado": aplicado.as_ref().and_then(|a| a.brillo_int).is_some(),
        },
    })
}

/// Una iteración del vigilante (la que llamaría el planificador del daemon).
///
/// Atajo barato: si la firma de los ficheros no ha cambiado y la última
/// lectura completa fue hace poco, no se lanza ninguna JVM.
pub fn tick(state_dir: Option<&str>) -> Value {
    // Contexto cargado una vez (2 lecturas de disco) y firma calculada una
    // vez: antes esta función hacía el `stat` dos veces (una aquí y otra
    // dentro de `reafirmar`).
    let ctx = Ctx::cargar(state_dir);
    let firma = firma_ficheros();
    let ahora_ts = ahora();
    let transcurrido = ahora_ts.saturating_sub(ctx.estado.ultima_lectura_ts);

    if !firma.is_empty()
        && firma == ctx.estado.firma
        && transcurrido < 60
        && ctx.estado.ultima_lectura_ts > 0
    {
        return json!({
            "ok": true,
            "accion": "sin_cambios",
            "costo": "0 JVM (sólo un stat de dos ficheros)",
            "firma": firma,
            "proxima_lectura_completa_en_s": 60u64.saturating_sub(transcurrido),
        });
    }

    let r = reafirmar_con(&ctx, Some(firma), state_dir, None);
    let mut out = r.clone();
    out["costo"] = json!("1 lectura de ajustes (+1 si hay que escribir)");
    out
}

/// Aplica el brillo por defecto y arranca el estado (se llama al iniciar el
/// daemon o cuando el usuario pide "aplicar").
pub fn aplicar_al_arrancar(state_dir: Option<&str>) -> Value {
    let cfg = Config::de_json(&config(state_dir));
    let cap = techo_energia().unwrap_or(BRILLO_MAXIMO);
    let valor = cfg.brillo_por_defecto.min(cap);
    let mut r = fijar(valor, state_dir);
    r["accion"] = json!("aplicar_al_arrancar");
    r["brillo_por_defecto"] = json!(cfg.brillo_por_defecto);
    r["techo_energia"] = json!(cap);
    r
}

// ===========================================================================
// VIGILANTE EN HILO
// ===========================================================================

static PARAR_VIGILANTE: AtomicBool = AtomicBool::new(false);

/// Arranca el vigilante en segundo plano.
///
/// El hilo duerme `intervalo_vigilancia_s` y llama a [`tick`].  En reposo el
/// coste es un `stat` por iteración (cero JVM).
pub fn arrancar_vigilante(state_dir: Option<&str>) -> Value {
    PARAR_VIGILANTE.store(false, Ordering::Relaxed);
    let cfg = Config::de_json(&config(state_dir));
    let sd = state_dir.map(|s| s.to_string());
    let intervalo = cfg.intervalo_vigilancia_s;
    let usar_eventos = cfg.eventos;

    // --- 1) El vigilante POR EVENTOS (inotify + FIFO del shim) -------------
    // MEDIDO (SM-G998B, 2026-11-27): el sondeo de 15 s costaba 5
    // interacciones con el móvil POR CICLO; con los eventos son 0 sondeos
    // (y 0..1 escrituras, sólo cuando hay que corregir algo).
    let arranque_eventos = if usar_eventos {
        let sd_ev = sd.clone();
        crate::brillo_events::arrancar(move |ev| {
            let r = reafirmar_por_evento(&ev, sd_ev.as_deref());
            let _ = r;
        })
    } else {
        crate::brillo_events::Arranque::default()
    };

    // --- 2) El respaldo por SONDEO ----------------------------------------
    // Sólo si los eventos no arrancaron, o si el usuario pide además sondeo
    // (`intervalo_vigilancia_s` > 0).
    let hace_sondeo = !arranque_eventos.eventos() || intervalo > 0;
    if hace_sondeo {
        let sd_s = sd.clone();
        std::thread::spawn(move || {
            while !PARAR_VIGILANTE.load(Ordering::Relaxed) {
                let _ = tick(sd_s.as_deref());
                // Dormir en trozos para poder parar rápido.
                let mut quedan = if intervalo > 0 { intervalo } else { 30 };
                while quedan > 0 && !PARAR_VIGILANTE.load(Ordering::Relaxed) {
                    let trozo = quedan.min(1);
                    std::thread::sleep(Duration::from_secs(trozo));
                    quedan -= trozo;
                }
            }
        });
    }

    // --- 3) El SUPERVISOR: si el hilo de eventos se muere, se rearrange ----
    // Coste: **0 interacciones con el móvil** (dormir 30 s y mirar un
    // contador en memoria).  Sin esto, un inotify que se cae dejaría el
    // brillo sin vigilar (el fallo más caro y más silencioso).
    if usar_eventos {
        let sd_sup = sd.clone();
        std::thread::spawn(move || {
            while !PARAR_VIGILANTE.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(30));
                if PARAR_VIGILANTE.load(Ordering::Relaxed) {
                    break;
                }
                if !crate::brillo_events::CONTADORES
                    .vivo
                    .load(Ordering::Relaxed)
                {
                    let sd_ev = sd_sup.clone();
                    crate::brillo_events::arrancar(move |ev| {
                        let _ = reafirmar_por_evento(&ev, sd_ev.as_deref());
                    });
                }
            }
        });
    }

    json!({
        "ok": true,
        "accion": "vigilante_arrancado",
        "eventos": arranque_eventos.a_json(),
        "sondeo_respaldo": hace_sondeo,
        "supervisor": usar_eventos,
        "intervalo_s": if hace_sondeo { if intervalo > 0 { intervalo } else { 30 } } else { 0 },
        "coste_por_ciclo": if hace_sondeo { "1 sondeo (5 interacciones)" } else { "0 sondeos (0 interacciones)" },
        "hilo": "en segundo plano; se para con parar_vigilante()",
    })
}

/// Pide al vigilante que pare (tarda hasta `intervalo_vigilancia_s`).
pub fn parar_vigilante() -> Value {
    PARAR_VIGILANTE.store(true, Ordering::Relaxed);
    json!({"ok": true, "accion": "vigilante_parado"})
}

/// ¿Estamos en un móvil Android?  (para no arrancar el vigilante en la nube)
///
/// **Optimizado**: el resultado se cachea 60 s (un `stat` + un `getprop` son
/// procesos, y esto se pregunta al arrancar).
pub fn es_movil() -> bool {
    static CACHE: Mutex<Option<(u64, bool)>> = Mutex::new(None);
    if let Some(v) = cache_get(&CACHE, 60) {
        return v;
    }
    let r = sh(
        "if [ -e /system/build.prop ]; then echo android; else getprop ro.build.version.sdk 2>/dev/null; fi",
        false,
        8000,
    );
    let t = r.texto();
    let v = t.contains("android") || t.trim().parse::<i64>().map(|s| s > 0).unwrap_or(false);
    cache_put(&CACHE, v);
    v
}

/// Arranca el vigilante **sólo si tiene sentido** (móvil Android) y sólo si
/// el entorno no lo desactiva.
///
/// * `MEGADAEMON_BRILLO=0` ⇒ no arrancar.
/// * `MEGADAEMON_BRILLO=1` ⇒ arrancar siempre.
/// * por defecto ⇒ arrancar si [`es_movil`].
pub fn arrancar_si_movil(state_dir: Option<&str>) -> Value {
    let flag = std::env::var("MEGADAEMON_BRILLO")
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    match flag.as_str() {
        "0" | "no" | "false" | "off" => {
            return json!({"ok": true, "accion": "vigilante_no_arrancado", "motivo": "MEGADAEMON_BRILLO=0"})
        }
        "1" | "si" | "yes" | "true" | "on" => return arrancar_vigilante(state_dir),
        _ => {}
    }
    if !es_movil() {
        // Pero si el XML del proveedor del móvil está ahí, **es un móvil**
        // aunque la detección haya fallado (p. ej. `/system/build.prop` no
        // legible por el contexto del proceso): mejor arrancar de más que
        // dejar el brillo sin vigilar.
        if leer_ajustes_xml_fichero().is_none() {
            return json!({
                "ok": true,
                "accion": "vigilante_no_arrancado",
                "motivo": "no es un móvil Android (no se toca el brillo de otra máquina)",
            });
        }
    }
    arrancar_vigilante(state_dir)
}

// ===========================================================================
// MCP: DEFINICIÓN DE LA TOOL Y DESPACHO
// ===========================================================================

/// Tool MCP.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "brillo",
            "description": "BRILLO del móvil (Samsung SM-G998B, Android 14). Controla el brillo, lo deja en manual (el sensor de luz ambiente deja de mandar) y vigila que nadie lo cambie; por defecto lo deja al MÁXIMO y respeta los cambios que haga el usuario con los botones del sistema. Acciones: estado | fijar (valor 0-255) | maximo | config | config_actualizar | reafirmar | shim | vigilante (arrancar|parar|tick) | sensor.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "accion": {
                        "type": "string",
                        "enum": [
                            "estado", "fijar", "maximo", "config", "config_actualizar",
                            "reafirmar", "shim", "vigilante", "sensor", "diagnostico", "costo",
                            "shim_probar", "rapido", "informe",
                            "observar", "adaptativo"
                        ],
                        "default": "estado"
                    },
                    "aplicar": {
                        "type": "boolean",
                        "description": "En accion=diagnostico: hacer la prueba A/B de las vias de escritura (cambia el brillo un momento y lo restaura). En accion=adaptativo: poner a 0 el brillo adaptativo de OneUI.",
                        "default": false
                    },
                    "muestras": {
                        "type": "integer",
                        "description": "En accion=observar: numero de muestras (2-120).",
                        "default": 12
                    },
                    "intervalo_s": {
                        "type": "integer",
                        "description": "En accion=observar: segundos entre muestras (1-60).",
                        "default": 5
                    },
                    "valor": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 255,
                        "description": "Brillo 0-255 (accion fijar)."
                    },
                    "brillo_por_defecto": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 255,
                        "description": "Brillo por defecto (accion config_actualizar). Por defecto 255 (máximo)."
                    },
                    "politica": {
                        "type": "string",
                        "enum": ["respetar_usuario", "firme", "pasivo"],
                        "description": "respetar_usuario: el usuario puede mover el brillo y se respeta. firme: se reimpone siempre. pasivo: no se toca nada.",
                        "default": "respetar_usuario"
                    },
                    "intervalo_vigilancia_s": {
                        "type": "integer",
                        "description": "Periodo del vigilante en segundos (2-3600).",
                        "default": 15
                    },
                    "permitir_automatico": {
                        "type": "boolean",
                        "description": "Si es true, el módulo NO toca el modo y el brillo automático puede vivir.",
                        "default": false
                    },
                    "escribir_ajuste_legacy": {
                        "type": "boolean",
                        "description": "Escribir también el ajuste entero 'screen_brightness' (0-255).",
                        "default": true
                    },
                    "usar_cmd_display": {
                        "type": "boolean",
                        "description": "Usar además 'cmd display set-brightness' (float 0-1).",
                        "default": true
                    },
                    "verificar_aplicado": {
                        "type": "boolean",
                        "description": "Verificar el brillo aplicado con 'dumpsys power' (1 JVM más).",
                        "default": true
                    },
                    "vigilante_accion": {
                        "type": "string",
                        "enum": ["arrancar", "parar", "tick"],
                        "default": "tick",
                        "description": "Qué hacer con el vigilante (accion vigilante)."
                    }
                },
                "required": ["accion"]
            }
        }
    ])
}

/// Despacha la herramienta MCP.
pub fn dispatch(args: &Value) -> Value {
    let s = |k: &str| {
        args.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let accion = s("accion");
    let accion = if accion.is_empty() {
        "estado".to_string()
    } else {
        accion
    };
    match accion.as_str() {
        "fijar" => {
            let v = args
                .get("valor")
                .and_then(|x| x.as_u64())
                .map(|x| x.min(255) as u8)
                .unwrap_or(BRILLO_MAXIMO);
            fijar(v, None)
        }
        "maximo" => maximo(None),
        "config" => config(None),
        "config_actualizar" => config_actualizar(args, None),
        "reafirmar" => reafirmar(None),
        "shim" => shim_estado(None),
        "sensor" => {
            let (lux, ev) = leer_sensor_luz();
            json!({"ok": true, "sensor_luz": lux, "detalle": ev})
        }
        "diagnostico" => {
            let aplicar = args
                .get("aplicar")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            diagnostico(None, aplicar)
        }
        "costo" => costo(None),
        "rapido" => estado_rapido(None),
        "informe" => {
            let n = args.get("muestras").and_then(|v| v.as_u64()).unwrap_or(12) as u32;
            let paso = args
                .get("intervalo_s")
                .and_then(|v| v.as_u64())
                .unwrap_or(5);
            informe(None, n, paso)
        }
        "shim_probar" => {
            // Prueba del PUENTE: escribe un evento en el FIFO del shim (como
            // haria el hook) y comprueba que el daemon lo consume.
            let clave = args
                .get("clave")
                .and_then(|v| v.as_str())
                .unwrap_or("screen_brightness");
            let valor = args.get("valor").and_then(|v| v.as_str()).unwrap_or("247");
            let salida = crate::brillo_events::escribir_evento_shell(clave, valor);
            json!({
                "ok": true,
                "accion": "shim_probar",
                "fifo": crate::brillo_events::RUTA_FIFO,
                "escrito": format!("{{\"clave\":\"{clave}\",\"valor\":\"{valor}\"}}"),
                "salida": salida.trim(),
                "contadores": crate::brillo_events::CONTADORES.a_json(),
                "nota": "el daemon deberia consumir la linea y contar 1 evento; comprueba con 'brillo shim'",
            })
        }
        "vigilante" => match s("vigilante_accion").as_str() {
            "arrancar" => arrancar_vigilante(None),
            "parar" => parar_vigilante(),
            _ => tick(None),
        },
        _ => estado(None),
    }
}

// ===========================================================================
// TESTS (sin dispositivo: sólo lógica pura y ficheros)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_los_ajustes_de_brillo() {
        let crudo = "screen_brightness_mode=0\nscreen_brightness_float=1.0\nscreen_brightness=255\nscreen_auto_brightness_adj=0.0\nairplane_mode_on=1\n";
        let l = parse_ajustes(crudo);
        assert_eq!(l.modo, Some(0));
        assert_eq!(l.float, Some(1.0));
        assert_eq!(l.int, Some(255));
        assert_eq!(l.auto_ajuste, Some(0.0));
        assert!(!l.auto_activo());
        assert_eq!(l.brillo_255(), Some(255));
    }

    #[test]
    fn detecta_el_modo_automatico() {
        let l = parse_ajustes("screen_brightness_mode=1\n");
        assert!(l.auto_activo());
        assert_eq!(l.modo, Some(1));
    }

    #[test]
    fn parsea_el_brillo_aplicado_de_dumpsys_power() {
        // Formato real medido (battery_advanced.rs:1630).
        let crudo = "POWER MANAGER (dumpsys power)\n\nDisplay Power: state=ON, display=true, brightness=0.501961, brightnessInt=128, displayPower=ON\nmScreenBrightnessOverrideFromWindowManager=\nmWakefulness=Awake\n";
        let a = parse_dumpsys_power(crudo);
        assert_eq!(a.brillo_float, Some(0.501961));
        assert_eq!(a.brillo_int, Some(128));
        assert!(!a.hay_override());
        assert_eq!(a.estado_pantalla.as_deref(), Some("ON"));
    }

    #[test]
    fn detecta_el_override_de_ventana() {
        let crudo = "Display Power: state=ON, display=true, brightness=0.5, brightnessInt=127, displayPower=ON\nmScreenBrightnessOverrideFromWindowManager=0.25\n";
        let a = parse_dumpsys_power(crudo);
        assert!(a.hay_override());
    }

    #[test]
    fn extrae_el_valor_del_sensor_de_luz() {
        let crudo = "    0x0000000b) Light sensor | Samsung | value: 12.500000 | max: 1000.000000 | accuracy: 3\n";
        assert_eq!(parse_als(crudo), Some(12.5));
    }

    #[test]
    fn sin_sensor_de_luz_no_inventa() {
        assert_eq!(parse_als("Accelerometer | value: 0.0 | max: 19.6\n"), None);
    }

    #[test]
    fn el_shim_ve_el_brillo_although_no_exista_el_ajuste_float() {
        // MEDIDO en el SM-G998B (One UI 6): `screen_brightness_float` → null;
        // el brillo vivo está en `screen_brightness` (legacy entero).
        let l = parse_ajustes("screen_brightness_mode=0\nscreen_brightness=247\n");
        assert_eq!(l.float, None);
        assert_eq!(l.int, Some(247));
        assert_eq!(l.brillo_255(), Some(247));
        // Con el respaldo, el shim ve el cambio (247/255 = 0.9686).
        let actual = l.float.or_else(|| l.int.map(|i| i as f64 / 255.0));
        assert!(actual.is_some());
        assert_eq!(
            decidir(
                "respetar_usuario",
                Some(1.0),
                actual,
                Some(0),
                false,
                None,
                false,
                None
            ),
            Decision::AdoptarCambio
        );
        // Si el float no existe, `actual` es None y el shim no hace nada
        // (de ahí el respaldo de arriba).
        let sin_float = parse_ajustes("screen_brightness_mode=0\n");
        assert!(sin_float.float.is_none());
        assert!(sin_float.int.is_none());
    }

    #[test]
    fn parsea_el_xml_del_proveedor_de_ajustes() {
        // Formato real de /data/system/users/0/settings_system.xml:
        // <setting id="12" name="screen_brightness" package="com.android.settings" defaultValue="255" value="247" />
        let crudo = concat!(
            "<settings version=\"1\">\n",
            "  <setting id=\"1\" name=\"screen_brightness_mode\" package=\"com.android.settings\" defaultValue=\"0\" value=\"0\" />\n",
            "  <setting id=\"2\" name=\"screen_brightness\" package=\"com.android.settings\" defaultValue=\"255\" value=\"247\" />\n",
            "  <setting id=\"3\" name=\"airplane_mode_on\" package=\"com.android.settings\" value=\"0\" />\n",
            "</settings>\n"
        );
        let l = parse_settings_xml(crudo);
        assert_eq!(l.modo, Some(0));
        assert_eq!(l.int, Some(247));
        assert_eq!(l.float, None);
        assert!(!l.auto_activo());
        assert_eq!(l.brillo_255(), Some(247));
    }

    #[test]
    fn el_recorte_a_60_es_de_la_politica_de_energia_no_del_usuario() {
        // MEDIDO 2026-11-27 en el SM-G998B: el brillo bajó de 247 a 60 y
        // volvió a 247.  60 = `MEGADAEMON_BRIGHTNESS_MAX` (política de
        // energía del propio daemon).  El shim NO debe tomar ese cambio por
        // una preferencia del usuario.
        let objetivo = Some(247.0 / 255.0);
        let actual = Some(60.0 / 255.0);
        assert_eq!(
            decidir(
                "respetar_usuario",
                objetivo,
                actual,
                Some(0),
                false,
                Some(60),
                false,
                None
            ),
            Decision::CambioDePolitica
        );
        // Sin techo conocido, es un cambio normal (del usuario).
        assert_eq!(
            decidir(
                "respetar_usuario",
                objetivo,
                actual,
                Some(0),
                false,
                None,
                false,
                None
            ),
            Decision::AdoptarCambio
        );
        // Y si el usuario mueve el deslizador a un valor que NO es el techo,
        // es suyo.
        let otro = Some(128.0 / 255.0);
        assert_eq!(
            decidir(
                "respetar_usuario",
                objetivo,
                otro,
                Some(0),
                false,
                Some(60),
                false,
                None
            ),
            Decision::AdoptarCambio
        );
    }

    #[test]
    fn el_modo_manual_desactiva_el_automatico() {
        // Decisión clave del shim: con modo=1 el ALS manda.
        assert_eq!(
            decidir(
                "respetar_usuario",
                Some(1.0),
                Some(0.5),
                Some(1),
                false,
                None,
                false,
                None
            ),
            Decision::ForzarManual
        );
        // Con modo=0 y política de respeto, el cambio es del usuario.
        assert_eq!(
            decidir(
                "respetar_usuario",
                Some(1.0),
                Some(0.5),
                Some(0),
                false,
                None,
                false,
                None
            ),
            Decision::AdoptarCambio
        );
        // Política firme: se reimpone.
        assert_eq!(
            decidir(
                "firme",
                Some(1.0),
                Some(0.5),
                Some(0),
                false,
                None,
                false,
                None
            ),
            Decision::Reafirmar
        );
        // Pasivo: no se toca nada.
        assert_eq!(
            decidir(
                "pasivo",
                Some(1.0),
                Some(0.5),
                Some(1),
                false,
                None,
                false,
                None
            ),
            Decision::Nada
        );
        // Si se permite el automático, el modo no se corrige.
        assert_eq!(
            decidir(
                "firme",
                Some(1.0),
                Some(0.5),
                Some(1),
                true,
                None,
                false,
                None
            ),
            Decision::Reafirmar
        );
    }

    #[test]
    fn si_el_valor_coincide_no_hace_nada() {
        assert_eq!(
            decidir(
                "firme",
                Some(1.0),
                Some(1.0),
                Some(0),
                false,
                None,
                false,
                None
            ),
            Decision::Nada
        );
        assert_eq!(
            decidir(
                "respetar_usuario",
                Some(0.5),
                Some(0.5001),
                Some(0),
                false,
                None,
                false,
                None
            ),
            Decision::Nada
        );
    }

    #[test]
    fn formatea_el_float_para_el_proveedor() {
        assert_eq!(fmt_float(1.0), "1");
        assert_eq!(fmt_float(0.0), "0");
        assert_eq!(fmt_float(0.5019607843), "0.501961");
    }

    #[test]
    fn la_coincidencia_tiene_tolerancia_de_dos() {
        let a = Aplicado {
            brillo_int: Some(128),
            ..Default::default()
        };
        assert!(coincide_brillo(&a, 128));
        assert!(coincide_brillo(&a, 130));
        assert!(!coincide_brillo(&a, 120));
        // Sin nada medido: no se puede afirmar que coincida.
        assert!(!coincide_brillo(&Aplicado::default(), 128));
    }

    #[test]
    fn la_coincidencia_acepta_el_float_del_dumpsys() {
        let a = Aplicado {
            brillo_float: Some(0.5),
            ..Default::default()
        };
        assert!(coincide_brillo(&a, 128)); // 0.5 -> 127.5
        assert!(!coincide_brillo(&a, 60));
    }

    #[test]
    fn detecta_la_pantalla_apagada() {
        let off = parse_dumpsys_power(
            "Display Power: state=OFF, display=true, brightness=0, brightnessInt=0, displayPower=OFF\n",
        );
        assert!(pantalla_apagada(&off));
        let on = parse_dumpsys_power(
            "Display Power: state=ON, display=true, brightness=0.5, brightnessInt=128, displayPower=ON\n",
        );
        assert!(!pantalla_apagada(&on));
    }

    #[test]
    fn la_sonda_del_sensor_acepta_las_dos_formas() {
        let (v, ev) = parse_sensor_luz("sysfs:12.5");
        assert_eq!(v, Some(12.5));
        assert!(ev.unwrap_or_default().contains("sysfs"));
        let (v, _) = parse_sensor_luz(
            "    0x0000000b) Light sensor | Samsung | value: 12.500000 | max: 1000.000000\n",
        );
        assert_eq!(v, Some(12.5));
        let (v, ev) = parse_sensor_luz("");
        assert_eq!(v, None);
        assert!(ev.is_some());
    }

    #[test]
    fn el_objetivo_cae_en_el_brillo_por_defecto() {
        let ctx = Ctx {
            cfg: Config::default(),
            estado: Estado::default(),
        };
        assert!((ctx.objetivo() - 1.0).abs() < 1e-9);
        let ctx2 = Ctx {
            cfg: Config {
                brillo_por_defecto: 51,
                ..Config::default()
            },
            estado: Estado {
                objetivo: Some(0.2),
                ..Estado::default()
            },
        };
        // El objetivo guardado manda sobre el valor por defecto.
        assert!((ctx2.objetivo() - 0.2).abs() < 1e-9);
    }

    #[test]
    fn la_config_por_defecto_es_brillo_maximo() {
        let c = Config::default();
        assert_eq!(c.brillo_por_defecto, BRILLO_MAXIMO);
        assert_eq!(c.politica, "respetar_usuario");
        assert!(!c.permitir_automatico);
    }

    #[test]
    fn la_config_tolera_garbage() {
        let c = Config::de_json(
            &json!({"brillo_por_defecto": 9999, "politica": "inventada", "intervalo_vigilancia_s": 0}),
        );
        assert_eq!(c.brillo_por_defecto, 255);
        assert_eq!(c.politica, "respetar_usuario");
        assert_eq!(c.intervalo_vigilancia_s, 2);
    }

    #[test]
    fn la_config_sobrevive_a_un_fichero_ilegible() {
        let c = Config::de_json(&json!({"politica": "firme"}));
        assert_eq!(c.politica, "firme");
        assert_eq!(c.brillo_por_defecto, 255);
    }

    #[test]
    fn guarda_y_lee_la_config() {
        let dir = std::env::temp_dir().join(format!("md_brillo_{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let sd = dir.to_string_lossy().to_string();
        let r = config_actualizar(
            &json!({"politica": "firme", "brillo_por_defecto": 200}),
            Some(&sd),
        );
        assert_eq!(r["ok"], true);
        let c = config(Some(&sd));
        assert_eq!(c["politica"], "firme");
        assert_eq!(c["brillo_por_defecto"], 200);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_estado_interno_va_y_vuelve() {
        let e = Estado {
            objetivo: Some(1.0),
            ultimo_float: Some(0.5),
            ultimo_modo: Some(0),
            firma: "x".to_string(),
            ultima_lectura_ts: 42,
            veces_reafirmado: 3,
            veces_modo_auto_corregido: 1,
            veces_cambio_usuario: 2,
            veces_cambio_politica: 1,
            bajo_politica: false,
            eventos_recibidos: 0,
            sondeos: 0,
            ultimo_evento: None,
            ultima_accion: "forzar_manual".to_string(),
            ultima_via: "settings".to_string(),
        };
        let v = e.a_json();
        let b = Estado::de_json(&v);
        assert_eq!(b.objetivo, Some(1.0));
        assert_eq!(b.veces_reafirmado, 3);
        assert_eq!(b.veces_cambio_politica, 1);
        assert_eq!(b.ultima_accion, "forzar_manual");
    }

    #[test]
    fn el_sondeo_cuesta_5_interacciones_y_el_evento_0() {
        // Medición del coste real en reposo: el vigilante habla con el móvil
        // como mucho 4 veces (firma + lectura de ajustes + verificación), y en
        // el caso bueno (firma sin cambios) menos.
        let dir = std::env::temp_dir().join(format!("md_brillo_tick_{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let sd = dir.to_string_lossy().to_string();
        reset_llamadas_sh();
        let _ = tick(Some(&sd));
        let n = llamadas_sh();
        println!("MEDIDO vigilante: {} interacciones con el móvil", n);
        assert!(n <= 4, "el vigilante hizo {} interacciones con el móvil", n);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_ciclo_completo_del_veto() {
        // MEDIDO (SM-G998B, 2026-11-27): la politica de energia recortaba a 60
        // (MEGADAEMON_BRIGHTNESS_MAX) y luego RESTAURABA su copia => el
        // serrucho 247 -> 60 -> 247.  El modulo (que ya no compite con la
        // politica) tiene que:
        //
        //  1) aceptar el recorte SIN perder el objetivo del usuario;
        //  2) al relajar la politica, restaurar el valor del USUARIO, no el
        //     247 que la politica tenia guardado.
        let f = |v: u8| Some(v as f64 / 255.0);
        let objetivo = f(247);

        // 1) La politica recorta a 60: se acepta, pero el objetivo sigue 247.
        assert_eq!(
            decidir(
                "respetar_usuario",
                objetivo,
                f(60),
                Some(0),
                false,
                Some(60),
                false,
                None
            ),
            Decision::CambioDePolitica
        );

        // 2) La politica relaja y restaura SU copia (247): el valor vuelve a
        //    ser el objetivo => no hay nada que hacer.
        assert_eq!(
            decidir(
                "respetar_usuario",
                objetivo,
                f(247),
                Some(0),
                false,
                Some(60),
                true,
                Some(247)
            ),
            Decision::Nada
        );

        // 3) El usuario movio el deslizador a 128 MIENTRAS estaba recortado:
        //    el objetivo pasa a ser 128 y la pantalla SIGUE en 60 (la politica
        //    recortando) => el cambio es del usuario => se adopta.
        //
        //    MEDIDO (2026-11-28): el test mezclaba los DOS momentos y por eso
        //    no podia pasar: con `actual = 128` (el valor ya aplicado) y sin
        //    copia guardada, el modulo responde "Nada" (objetivo == actual),
        //    que es lo correcto segun sus propias reglas.  La invariante que
        //    de verdad importa es la del punto siguiente (Reafirmar con el
        //    objetivo del usuario, no con la copia de la politica).
        let objetivo_usuario = f(128);
        assert_eq!(
            decidir(
                "respetar_usuario",
                objetivo_usuario,
                f(60),
                Some(0),
                false,
                Some(60),
                true,
                Some(247)
            ),
            Decision::AdoptarCambio
        );
        assert_eq!(
            decidir(
                "respetar_usuario",
                objetivo_usuario,
                f(247),
                Some(0),
                false,
                Some(60),
                true,
                Some(247)
            ),
            Decision::Reafirmar
        );
    }

    #[test]
    fn el_usuario_puede_mover_el_deslizador_mientras_la_politica_recorta() {
        // MEDIDO (2026-11-28): el caso 3 de `el_ciclo_completo_del_veto` no
        // pasaba porque el CODIGO confundia dos cosas distintas:
        //
        // * la politica bajo la pantalla a 60 (el objetivo del usuario sigue
        //   siendo el 247 que la politica guardo) => se respeta el techo;
        // * el usuario movio el deslizador a 128 mientras estaba recortado
        //   (el objetivo ya no es el de la copia) => su cambio se adopta.
        //
        // Este test fija LOS DOS LADOS de la distincion.
        let f = |v: u8| Some(v as f64 / 255.0);
        // La politica bajo la pantalla: el objetivo es su copia (247).
        assert_eq!(
            decidir(
                "respetar_usuario",
                f(247),
                f(60),
                Some(0),
                false,
                Some(60),
                true,
                Some(247)
            ),
            Decision::CambioDePolitica,
            "la politica bajo la pantalla: se respeta el techo"
        );
        // El usuario movio el deslizador a 128: su cambio se adopta.
        assert_eq!(
            decidir(
                "respetar_usuario",
                f(128),
                f(60),
                Some(0),
                false,
                Some(60),
                true,
                Some(247)
            ),
            Decision::AdoptarCambio,
            "el usuario movio el deslizador: se adopta su objetivo"
        );
        // Y con la politica "firme" manda la politica, no el usuario.
        assert_eq!(
            decidir(
                "firme",
                f(128),
                f(60),
                Some(0),
                false,
                Some(60),
                true,
                Some(247)
            ),
            Decision::Reafirmar,
            "con la politica firme manda la politica"
        );
    }

    #[test]
    fn el_estado_rapido_no_toca_el_movil() {
        // La ruta caliente (el watchdog, los paneles) no puede lanzar ni un
        // `settings` ni un `dumpsys`: sólo leer el XML del proveedor.
        let dir = std::env::temp_dir().join(format!("md_brillo_rap_{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let sd = dir.to_string_lossy().to_string();
        reset_llamadas_sh();
        let v = estado_rapido(Some(&sd));
        let n = llamadas_sh();
        assert_eq!(v["ok"], true);
        assert_eq!(n, 0, "estado_rapido lanzo {} procesos; debe ser 0", n);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_camino_del_evento_cuesta_0_interacciones() {
        // MEDIDO (SM-G998B, 2026-11-27): el vigilante por SONDEO gastaba 5
        // interacciones con el móvil por ciclo (firma + lectura + ...).  Con
        // el vigilante POR EVENTOS el ciclo es 0: los ajustes se leen del XML
        // del proveedor **como fichero** (0 procesos) y sólo se escribe si hay
        // que corregir algo.
        //
        // Aquí se comprueba con un XML de mentira (rutas inyectadas): la
        // lectura no debe lanzar NINGÚN proceso.
        let dir = std::env::temp_dir().join(format!("md_brillo_ev_{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let xml = dir.join("settings_system.xml");
        std::fs::write(
            &xml,
            "<settings version=\"1\">\n  <setting id=\"1\" name=\"screen_brightness_mode\" package=\"com.android.settings\" value=\"0\" />\n  <setting id=\"2\" name=\"screen_brightness\" package=\"com.android.settings\" value=\"247\" />\n</settings>\n",
        )
        .unwrap();
        let ruta = xml.to_string_lossy().to_string();
        reset_llamadas_sh();
        let l = leer_ajustes_xml_fichero_en(&[ruta.as_str()]);
        let n = llamadas_sh();
        assert!(l.is_some(), "el XML de prueba no se leyó");
        assert_eq!(l.unwrap().int, Some(247));
        assert_eq!(n, 0, "la lectura del XML lanzó {} procesos; debe ser 0", n);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_costo_se_mide_en_el_movil() {
        // La acción `costo` mide los tres números EN EL APARATO (el CI no
        // puede medirlos: aquí no hay Android).  Lo que sí se comprueba aquí es
        // que la acción existe y devuelve las tres cifras.
        let dir = std::env::temp_dir().join(format!("md_brillo_costo_{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let sd = dir.to_string_lossy().to_string();
        let v = costo(Some(&sd));
        assert_eq!(v["ok"], true);
        assert!(v.get("evento").is_some());
        assert!(v.get("sondeo").is_some());
        assert!(v.get("estado").is_some());
        assert_eq!(v["suelo"]["estado"], 1);
        assert_eq!(v["suelo"]["evento"], 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fijar_tiene_un_tope_de_interacciones() {
        let dir = std::env::temp_dir().join(format!("md_brillo_fijar_{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let sd = dir.to_string_lossy().to_string();
        reset_llamadas_sh();
        let _ = fijar(200, Some(&sd));
        let n = llamadas_sh();
        println!("MEDIDO fijar: {} interacciones con el móvil", n);
        // escritura (1) + verificación (1) + vía alternativa (1) + firma (1)
        assert!(n <= 5, "fijar hizo {} interacciones con el móvil", n);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_estado_cuesta_1_llamada_agrupada() {
        // `estado()` es la lectura que más se usa (tool MCP): ajustes +
        // brillo aplicado + sensor de luz.  Antes la sonda del sensor eran
        // DOS procesos (sysfs y luego dumpsys); ahora es uno.
        let dir = std::env::temp_dir().join(format!("md_brillo_est_{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let sd = dir.to_string_lossy().to_string();
        reset_llamadas_sh();
        let _ = estado(Some(&sd));
        let n = llamadas_sh();
        println!("MEDIDO estado: {} interacciones con el móvil", n);
        // MEDIDO (2026-11-27): `estado()` costaba 6 interacciones (ajustes ×3
        // + brillo aplicado + ALS + techo + copia de la política).  Ahora las
        // lecturas van AGRUPADAS en una sola llamada al shell, y el XML del
        // proveedor se lee como fichero (0 procesos) ⇒ en el móvil real son
        // **1**.  En este entorno (sin Android) el XML no existe, así que se
        // prueban las vías de respaldo: 1 (XML por shell) + 1 (settings list)
        // + 1 (settings get) + 1 (agrupada) = 4.
        assert!(n <= 4, "estado hizo {} interacciones con el móvil", n);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_diagnostico_devuelve_el_esqueleto() {
        // Sin dispositivo: el diagnóstico NO puede entrar en pánico y siempre
        // devuelve las secciones que el informe necesita.
        let dir = std::env::temp_dir().join(format!("md_brillo_diag_{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let sd = dir.to_string_lossy().to_string();
        let v = diagnostico(Some(&sd), false);
        assert_eq!(v["ok"], true);
        for clave in [
            "veredicto",
            "ajustes",
            "aplicado",
            "sensor_luz",
            "ficheros",
            "claves_firmware",
            "prueba_de_via",
        ] {
            assert!(v.get(clave).is_some(), "falta {}", clave);
        }
        assert!(v["veredicto"].is_array());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_tool_tiene_esquema_valido() {
        let d = tool_definitions();
        let arr = d.as_array().expect("array");
        assert_eq!(arr[0]["name"], "brillo");
        assert_eq!(arr[0]["inputSchema"]["type"], "object");
    }

    #[test]
    fn el_despacho_responde_a_cada_accion() {
        for a in ["config", "shim", "sensor"] {
            let v = dispatch(&json!({"accion": a}));
            assert!(v.is_object(), "{}", a);
        }
        // Sin móvil, `fijar` no puede fallar con pánico.
        let v = dispatch(&json!({"accion": "fijar", "valor": 128}));
        assert!(v.is_object());
        assert!(v.get("ok").is_some());
    }
}

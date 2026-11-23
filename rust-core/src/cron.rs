//! MegaDaemon v16 — PLANIFICADOR CRON PERSISTENTE.
//!
//! El móvil no tiene `crond`, ni `at`, ni `busybox` (medido en el S21 Ultra:
//! `crond=NO`, `at=NO`, `crontab=NO`, `busybox=NO`). Todo lo que se programaba
//! antes se hacía con scripts en `service.d`, que sólo se ejecutan al arrancar
//! y que no sobreviven a un reinicio del proceso.
//!
//! Este módulo es un **planificador cron en Rust puro** con estado persistente:
//!
//! * expresiones cron de 5 campos (minuto hora día-del-mes mes día-de-semana),
//!   con `*`, `*/n`, `a-b`, `a-b/n`, listas y `?`;
//! * **persistencia**: las tareas sobreviven al reinicio del daemon (fichero
//!   JSON con `last_run`, `next_run`, `running_since`…);
//! * **re-ejecución y reanudación**: si el daemon muere con una tarea en
//!   curso, al volver la detecta como *interrumpida* y la relanza; los intentos
//!   fallidos se reintentan con espera creciente;
//! * **catch-up**: si el móvil estuvo dormido o apagado, decide qué hacer con
//!   las ejecuciones perdidas (`skip`, `run_once`, `run_all`);
//! * **límites de recursos**: tope de tareas simultáneas, tiempo máximo de
//!   ejecución, veto de tareas pesadas con batería baja.
//!
//! Como el resto de la capa de automatización, es **puro**: la ejecución real
//! la hace `crate::automation_runtime`.

use serde_json::{json, Value};

/// Expresión cron compilada.
#[derive(Clone, Debug)]
pub struct CronExpr {
    pub expr: String,
    minute: [bool; 60],
    hour: [bool; 24],
    dom: [bool; 32],
    month: [bool; 13],
    dow: [bool; 7],
    dom_restricted: bool,
    dow_restricted: bool,
}

// ========================================================================= //
// Calendario (sin dependencias: aritmética civil <-> epoch)
// ========================================================================= //

/// Días desde 1970-01-01 → (año, mes, día). Algoritmo de Hinnant.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (año, mes, día) → días desde 1970-01-01.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Epoch → (año, mes, día, hora, minuto) en hora local con el offset dado.
pub fn civil_from_epoch(epoch: i64, tz_offset_min: i64) -> (i64, u32, u32, u32, u32) {
    let local = epoch + tz_offset_min * 60;
    let days = local.div_euclid(86_400);
    let secs = local.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    (
        y,
        m,
        d,
        (secs / 3600) as u32,
        ((secs % 3600) / 60) as u32,
    )
}

/// Día de la semana (0 = domingo).
fn weekday(days: i64) -> u32 {
    (((days + 4) % 7 + 7) % 7) as u32
}

/// Interpreta `+0200`, `-0500`, `+02:00` como minutos de offset.
pub fn parse_tz_offset(s: &str) -> Option<i64> {
    let t: String = s.trim().chars().filter(|c| c.is_ascii_digit() || *c == '+' || *c == '-').collect();
    if t.len() < 5 {
        return None;
    }
    let sign = if t.starts_with('-') { -1 } else { 1 };
    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return None;
    }
    let h: i64 = digits[0..2].parse().ok()?;
    let m: i64 = digits[2..4].parse().ok()?;
    Some(sign * (h * 60 + m))
}

// ========================================================================= //
// Expresión cron
// ========================================================================= //

fn parse_field(spec: &str, min: u32, max: u32, names: Option<&[(&str, u32)]>) -> Result<(Vec<u32>, bool), String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("campo cron vacío".to_string());
    }
    let mut out: Vec<u32> = Vec::new();
    let mut restricted = true;
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("elemento vacío en '{}'", spec));
        }
        if part == "*" || part == "?" {
            restricted = false;
            for v in min..=max {
                if !out.contains(&v) {
                    out.push(v);
                }
            }
            continue;
        }
        // Rango con paso: a-b/n o a/n
        let (range_part, step) = match part.split_once('/') {
            Some((r, s)) => {
                let st: u32 = s.parse().map_err(|_| format!("paso inválido en '{}'", part))?;
                if st == 0 {
                    return Err(format!("paso 0 en '{}'", part));
                }
                (r, st)
            }
            None => (part, 1),
        };
        let (lo, hi) = if range_part == "*" {
            (min, max)
        } else if let Some((a, b)) = range_part.split_once('-') {
            (parse_value(a, min, max, names)?, parse_value(b, min, max, names)?)
        } else {
            let v = parse_value(range_part, min, max, names)?;
            if step > 1 {
                (v, max)
            } else {
                (v, v)
            }
        };
        if lo > hi {
            return Err(format!("rango invertido en '{}'", part));
        }
        let mut v = lo;
        while v <= hi {
            if !out.contains(&v) {
                out.push(v);
            }
            v += step;
        }
    }
    if out.is_empty() {
        return Err(format!("campo '{}' no matchea nada", spec));
    }
    Ok((out, restricted))
}

fn parse_value(s: &str, min: u32, max: u32, names: Option<&[(&str, u32)]>) -> Result<u32, String> {
    let s = s.trim();
    if let Some(ns) = names {
        for (n, v) in ns {
            if n.eq_ignore_ascii_case(s) {
                return Ok(*v);
            }
        }
    }
    let v: u32 = s
        .parse()
        .map_err(|_| format!("valor inválido en cron: '{}'", s))?;
    // El domingo puede escribirse 7.
    let v = if max == 6 && v == 7 { 0 } else { v };
    if v < min || v > max {
        return Err(format!("valor fuera de rango en cron: '{}' ({}..{})", s, min, max));
    }
    Ok(v)
}

const MONTH_NAMES: &[(&str, u32)] = &[
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

const DOW_NAMES: &[(&str, u32)] = &[
    ("sun", 0),
    ("mon", 1),
    ("tue", 2),
    ("wed", 3),
    ("thu", 4),
    ("fri", 5),
    ("sat", 6),
];

impl CronExpr {
    /// Compila una expresión cron de 5 campos.
    pub fn parse(expr: &str) -> Result<CronExpr, String> {
        let campos: Vec<&str> = expr.split_whitespace().collect();
        if campos.len() != 5 {
            return Err(format!(
                "se esperaban 5 campos cron (min hora dom mes dow) y llegaron {}: '{}'",
                campos.len(),
                expr
            ));
        }
        let (minutes, _) = parse_field(campos[0], 0, 59, None)?;
        let (hours, _) = parse_field(campos[1], 0, 23, None)?;
        let (doms, dom_r) = parse_field(campos[2], 1, 31, None)?;
        let (months, _) = parse_field(campos[3], 1, 12, Some(MONTH_NAMES))?;
        let (dows, dow_r) = parse_field(campos[4], 0, 6, Some(DOW_NAMES))?;

        let mut c = CronExpr {
            expr: expr.to_string(),
            minute: [false; 60],
            hour: [false; 24],
            dom: [false; 32],
            month: [false; 13],
            dow: [false; 7],
            dom_restricted: dom_r,
            dow_restricted: dow_r,
        };
        for v in minutes {
            c.minute[v as usize] = true;
        }
        for v in hours {
            c.hour[v as usize] = true;
        }
        for v in doms {
            c.dom[v as usize] = true;
        }
        for v in months {
            c.month[v as usize] = true;
        }
        for v in dows {
            c.dow[v as usize] = true;
        }
        Ok(c)
    }

    /// ¿El epoch (en hora local `tz`) cae en esta expresión?
    pub fn matches(&self, epoch: i64, tz: i64) -> bool {
        let (y, mo, d, h, mi) = civil_from_epoch(epoch, tz);
        self.month[mo as usize]
            && self.hour[h as usize]
            && self.minute[mi as usize]
            && self.day_matches(d, days_from_civil(y, mo, d))
    }

    fn day_matches(&self, day: u32, days: i64) -> bool {
        let dom_ok = self.dom[day as usize];
        let dow_ok = self.dow[weekday(days) as usize];
        match (self.dom_restricted, self.dow_restricted) {
            (true, true) => dom_ok || dow_ok, // semántica estándar de cron
            (true, false) => dom_ok,
            (false, true) => dow_ok,
            (false, false) => true,
        }
    }

    /// Próxima ejecución **estrictamente posterior** a `from`.
    pub fn next_after(&self, from: i64, tz: i64) -> Option<i64> {
        // Empezamos en el siguiente minuto exacto.
        let t0 = from - from.rem_euclid(60) + 60;
        let (y0, m0, d0, hh0, mm0) = civil_from_epoch(t0, tz);
        let day0 = days_from_civil(y0, m0, d0);
        for day_off in 0..(366 * 5) {
            let day = day0 + day_off;
            let (_y, mo, d) = civil_from_days(day);
            if !self.month[mo as usize] {
                continue;
            }
            if !self.day_matches(d, day) {
                continue;
            }
            for h in 0..24 {
                if !self.hour[h] {
                    continue;
                }
                for mi in 0..60 {
                    if !self.minute[mi] {
                        continue;
                    }
                    if day_off == 0
                        && (h < hh0 as usize || (h == hh0 as usize && mi < mm0 as usize))
                    {
                        continue;
                    }
                    return Some(day * 86_400 + (h as i64) * 3600 + (mi as i64) * 60 - tz * 60);
                }
            }
        }
        None
    }
}

// ========================================================================= //
// Tareas
// ========================================================================= //

/// Qué hacer con las ejecuciones perdidas mientras el móvil estaba dormido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatchUp {
    /// No ejecutar lo perdido.
    Skip,
    /// Ejecutar una vez (la última perdida).
    RunOnce,
    /// Ejecutar todas (con tope).
    RunAll,
}

impl CatchUp {
    pub fn as_str(&self) -> &'static str {
        match self {
            CatchUp::Skip => "skip",
            CatchUp::RunOnce => "run_once",
            CatchUp::RunAll => "run_all",
        }
    }
    pub fn from_str(s: &str) -> Option<CatchUp> {
        Some(match s {
            "skip" => CatchUp::Skip,
            "run_once" => CatchUp::RunOnce,
            "run_all" => CatchUp::RunAll,
            _ => return None,
        })
    }
}

/// Una tarea programada.
#[derive(Clone, Debug)]
pub struct Job {
    pub id: String,
    pub name: String,
    /// Expresión cron de 5 campos.
    pub cron: String,
    /// Qué hacer cuando salta (lo ejecuta la capa de runtime).
    pub action: Value,
    pub enabled: bool,
    pub catch_up: CatchUp,
    /// Reintentos tras un fallo.
    pub max_attempts: u32,
    /// Espera entre reintentos (se duplica en cada intento).
    pub retry_backoff_secs: u64,
    /// Tiempo máximo de ejecución.
    pub timeout_ms: u64,
    /// Tarea pesada: no se lanza con batería baja (política de energía).
    pub heavy: bool,
    pub last_run: Option<i64>,
    pub next_run: Option<i64>,
    /// Epoch en el que se marcó "en curso" (para detectar interrupciones).
    pub running_since: Option<i64>,
    pub failures: u32,
}

impl Default for Job {
    fn default() -> Job {
        Job {
            id: String::new(),
            name: String::new(),
            cron: "0 * * * *".to_string(),
            action: json!({}),
            enabled: true,
            catch_up: CatchUp::Skip,
            max_attempts: 3,
            retry_backoff_secs: 60,
            timeout_ms: 300_000,
            heavy: false,
            last_run: None,
            next_run: None,
            running_since: None,
            failures: 0,
        }
    }
}

/// Estado completo del planificador (lo que se persiste).
#[derive(Clone, Debug)]
pub struct SchedulerState {
    pub jobs: Vec<Job>,
    /// Tareas simultáneas como máximo.
    pub max_concurrent: usize,
    /// Offset de zona horaria en minutos.
    pub tz_offset_min: i64,
    /// Tope de ejecuciones perdidas que se recuperan con `run_all`.
    pub max_catchup: usize,
    pub updated_at: i64,
}

impl Default for SchedulerState {
    fn default() -> Self {
        SchedulerState {
            jobs: Vec::new(),
            max_concurrent: 2,
            tz_offset_min: 0,
            max_catchup: 10,
            updated_at: 0,
        }
    }
}

/// Límites de recursos para decidir si una tarea puede lanzarse.
#[derive(Clone, Copy, Debug)]
pub struct ResourceLimits {
    pub max_concurrent: usize,
    /// Tiempo máximo de ejecución tolerated (para detectar zombis).
    pub max_runtime_secs: u64,
    /// Permite tareas pesadas.
    pub allow_heavy: bool,
    /// Batería mínima (%) para lanzar tareas pesadas.
    pub min_battery_pct: u8,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        ResourceLimits {
            max_concurrent: 2,
            max_runtime_secs: 900,
            allow_heavy: true,
            min_battery_pct: 30,
        }
    }
}

/// Una ejecución planificada.
#[derive(Clone, Debug, PartialEq)]
pub struct RunPlan {
    pub job_id: String,
    /// Epoch en el que la tarea *debía* ejecutarse.
    pub scheduled_at: i64,
    /// true si se está recuperando una ejecución perdida.
    pub catch_up: bool,
    /// Intento (1 = primero).
    pub attempt: u32,
}

// ========================================================================= //
// Motor
// ========================================================================= //

/// Calcula las ejecuciones pendientes de un estado, sin mutarlo.
pub fn plan(state: &SchedulerState, now: i64) -> Vec<RunPlan> {
    let mut out: Vec<RunPlan> = Vec::new();
    for job in &state.jobs {
        if !job.enabled {
            continue;
        }
        let Ok(expr) = CronExpr::parse(&job.cron) else {
            continue;
        };
        // Punto de partida: el `next_run` guardado o el próximo a partir de ahora.
        let mut cursor = job.next_run.unwrap_or_else(|| now);
        if cursor > now {
            continue;
        }
        let mut missed: Vec<i64> = Vec::new();
        let mut guard = 0;
        while cursor <= now && guard < 1000 {
            missed.push(cursor);
            guard += 1;
            match expr.next_after(cursor, state.tz_offset_min) {
                Some(n) => cursor = n,
                None => break,
            }
        }
        if missed.is_empty() {
            continue;
        }
        let keep: Vec<i64> = match job.catch_up {
            CatchUp::Skip => Vec::new(),
            CatchUp::RunOnce => missed.last().copied().into_iter().collect(),
            CatchUp::RunAll => {
                let start = missed.len().saturating_sub(state.max_catchup);
                missed[start..].to_vec()
            }
        };
        for m in keep {
            out.push(RunPlan {
                job_id: job.id.clone(),
                scheduled_at: m,
                catch_up: m < now - 60,
                attempt: 1,
            });
        }
    }
    out.sort_by_key(|p| p.scheduled_at);
    out
}

/// Marca una tarea como "en curso".
pub fn mark_running(state: &mut SchedulerState, job_id: &str, now: i64) -> Result<(), String> {
    let j = state
        .jobs
        .iter_mut()
        .find(|j| j.id == job_id)
        .ok_or_else(|| format!("tarea desconocida: {}", job_id))?;
    j.running_since = Some(now);
    Ok(())
}

/// Registra el resultado de una ejecución y calcula el siguiente disparo.
pub fn mark_result(state: &mut SchedulerState, job_id: &str, ok: bool, now: i64) -> Result<(), String> {
    let j = state
        .jobs
        .iter_mut()
        .find(|j| j.id == job_id)
        .ok_or_else(|| format!("tarea desconocida: {}", job_id))?;
    j.running_since = None;
    j.last_run = Some(now);
    if ok {
        j.failures = 0;
    } else {
        j.failures = j.failures.saturating_add(1);
    }
    // Reintento: si falló y le quedan intentos, se reprograma pronto.
    if !ok && j.failures < j.max_attempts {
        let backoff = j.retry_backoff_secs.saturating_mul(j.failures as u64);
        j.next_run = Some(now + backoff as i64);
        return Ok(());
    }
    j.next_run = CronExpr::parse(&j.cron)
        .ok()
        .and_then(|e| e.next_after(now, state.tz_offset_min));
    Ok(())
}

/// Tareas que quedaron "en curso" más tiempo del permitido (el daemon murió).
pub fn interrupted(state: &SchedulerState, now: i64, limits: &ResourceLimits) -> Vec<String> {
    let mut out = Vec::new();
    for j in &state.jobs {
        if let Some(since) = j.running_since {
            if now.saturating_sub(since) > limits.max_runtime_secs as i64 {
                out.push(j.id.clone());
            }
        }
    }
    out
}

/// Relanza una tarea interrumpida (la marca como pendiente otra vez).
pub fn resume(state: &mut SchedulerState, job_id: &str, now: i64) -> Result<(), String> {
    let j = state
        .jobs
        .iter_mut()
        .find(|j| j.id == job_id)
        .ok_or_else(|| format!("tarea desconocida: {}", job_id))?;
    j.running_since = None;
    j.next_run = Some(now);
    Ok(())
}

/// ¿Se puede lanzar ya esta tarea?
pub fn eligible(
    state: &SchedulerState,
    job_id: &str,
    running: usize,
    limits: &ResourceLimits,
    battery_pct: Option<u8>,
) -> Result<(), String> {
    let j = state
        .jobs
        .iter()
        .find(|j| j.id == job_id)
        .ok_or_else(|| format!("tarea desconocida: {}", job_id))?;
    if running >= limits.max_concurrent.max(1) {
        return Err(format!("ya hay {} tareas en curso (tope {})", running, limits.max_concurrent));
    }
    if j.heavy && !limits.allow_heavy {
        return Err("la política de energía no permite tareas pesadas".to_string());
    }
    if j.heavy {
        if let Some(b) = battery_pct {
            if b < limits.min_battery_pct {
                return Err(format!(
                    "batería al {}% (mínimo {}% para tareas pesadas)",
                    b, limits.min_battery_pct
                ));
            }
        }
    }
    Ok(())
}

// ========================================================================= //
// Persistencia
// ========================================================================= //

pub fn job_to_json(j: &Job) -> Value {
    json!({
        "id": j.id,
        "name": j.name,
        "cron": j.cron,
        "action": j.action,
        "enabled": j.enabled,
        "catch_up": j.catch_up.as_str(),
        "max_attempts": j.max_attempts,
        "retry_backoff_secs": j.retry_backoff_secs,
        "timeout_ms": j.timeout_ms,
        "heavy": j.heavy,
        "last_run": j.last_run,
        "next_run": j.next_run,
        "running_since": j.running_since,
        "failures": j.failures,
    })
}

pub fn job_from_json(v: &Value) -> Result<Job, String> {
    let mut j = Job {
        id: v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        name: v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        cron: v.get("cron").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        action: v.get("action").cloned().unwrap_or(json!({})),
        enabled: v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
        catch_up: v
            .get("catch_up")
            .and_then(|x| x.as_str())
            .and_then(CatchUp::from_str)
            .unwrap_or(CatchUp::Skip),
        max_attempts: v.get("max_attempts").and_then(|x| x.as_u64()).unwrap_or(3) as u32,
        retry_backoff_secs: v.get("retry_backoff_secs").and_then(|x| x.as_u64()).unwrap_or(60),
        timeout_ms: v.get("timeout_ms").and_then(|x| x.as_u64()).unwrap_or(300_000),
        heavy: v.get("heavy").and_then(|x| x.as_bool()).unwrap_or(false),
        last_run: v.get("last_run").and_then(|x| x.as_i64()),
        next_run: v.get("next_run").and_then(|x| x.as_i64()),
        running_since: v.get("running_since").and_then(|x| x.as_i64()),
        failures: v.get("failures").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
    };
    if j.id.is_empty() {
        // Se reutiliza el `slug` del motor de reglas (no se duplica).
        j.id = crate::automation::slug(&j.name);
    }
    if j.cron.trim().is_empty() {
        return Err("tarea sin expresión cron".to_string());
    }
    CronExpr::parse(&j.cron)?; // valida al cargar: no se persiste una tarea rota
    Ok(j)
}

pub fn state_to_json(s: &SchedulerState) -> Value {
    json!({
        "version": 1,
        "max_concurrent": s.max_concurrent,
        "tz_offset_min": s.tz_offset_min,
        "max_catchup": s.max_catchup,
        "updated_at": s.updated_at,
        "jobs": s.jobs.iter().map(job_to_json).collect::<Vec<_>>(),
    })
}

pub fn state_from_json(v: &Value) -> SchedulerState {
    SchedulerState {
        jobs: v
            .get("jobs")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|j| job_from_json(j).ok()).collect())
            .unwrap_or_default(),
        max_concurrent: v.get("max_concurrent").and_then(|x| x.as_u64()).unwrap_or(2) as usize,
        tz_offset_min: v.get("tz_offset_min").and_then(|x| x.as_i64()).unwrap_or(0),
        max_catchup: v.get("max_catchup").and_then(|x| x.as_u64()).unwrap_or(10) as usize,
        updated_at: v.get("updated_at").and_then(|x| x.as_i64()).unwrap_or(0),
    }
}

/// Carga el estado (si no existe, uno nuevo).
pub fn load(path: &str) -> SchedulerState {
    let Ok(txt) = std::fs::read_to_string(path) else {
        return SchedulerState::default();
    };
    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
        return SchedulerState::default();
    };
    state_from_json(&v)
}

/// Guarda el estado.
pub fn save(path: &str, s: &SchedulerState) -> Result<(), String> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(path, serde_json::to_string_pretty(&state_to_json(s)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

// ========================================================================= //
// Tests
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_calendario_civil_es_coherente() {
        // 1970-01-01 fue jueves.
        assert_eq!(civil_from_epoch(0, 0), (1970, 1, 1, 0, 0));
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        // 2026-11-22 00:00:00 UTC = 20779 días desde el epoch.
        assert_eq!(days_from_civil(2026, 11, 22), 20_779);
        assert_eq!(civil_from_days(20_779), (2026, 11, 22));
    }

    #[test]
    fn el_dia_de_la_semana_es_correcto() {
        // 1970-01-01 = jueves (4 con domingo=0).
        assert_eq!(weekday(0), 4);
        assert_eq!(weekday(4), 1); // 1970-01-05 = lunes
    }

    #[test]
    fn el_offset_de_zona_se_interpreta() {
        assert_eq!(parse_tz_offset("+0200"), Some(120));
        assert_eq!(parse_tz_offset("-0500"), Some(-300));
        assert_eq!(parse_tz_offset("+02:00"), Some(120));
        assert_eq!(parse_tz_offset("basura"), None);
    }

    #[test]
    fn una_expresion_cron_se_compila() {
        assert!(CronExpr::parse("*/5 * * * *").is_ok());
        assert!(CronExpr::parse("0 3 * * 1-5").is_ok());
        assert!(CronExpr::parse("0 0 1 jan *").is_ok());
        assert!(CronExpr::parse("sin campos").is_err());
        assert!(CronExpr::parse("0 0 * * * *").is_err()); // 6 campos
        assert!(CronExpr::parse("99 * * * *").is_err()); // fuera de rango
    }

    #[test]
    fn la_expresion_encuentra_la_siguiente_ejecucion() {
        // Epoch de 2026-11-22 12:00:00 UTC.
        let base = 20_779 * 86_400 + 12 * 3600;
        let e = CronExpr::parse("30 13 * * *").unwrap();
        let n = e.next_after(base, 0).unwrap();
        assert_eq!(civil_from_epoch(n, 0), (2026, 11, 22, 13, 30));
    }

    #[test]
    fn el_futuro_se_calcula_con_zona_horaria() {
        // Mismo instante, misma expresión, hora local +2: la ejecución local es
        // una hora antes en UTC (10:00 UTC = 12:00 local, aún no son las 13:00
        // locales de ese día).
        let base = 20_779 * 86_400 + 10 * 3600;
        let e = CronExpr::parse("0 13 * * *").unwrap();
        let utc = e.next_after(base, 0).unwrap();
        let cet = e.next_after(base, 120).unwrap();
        assert_eq!(cet, utc - 7200);
    }

    #[test]
    fn matches_usa_el_dia_de_la_semana() {
        // 2026-11-22 es domingo (verificado con el calendario civil).
        let e = CronExpr::parse("0 0 * * 0").unwrap();
        let d = 20_779 * 86_400;
        assert!(e.matches(d, 0));
        let e2 = CronExpr::parse("0 0 * * 1").unwrap();
        assert!(!e2.matches(d, 0));
    }

    #[test]
    fn dom_y_dow_se_combinan_con_or() {
        // dom restringido y dow restringido: vale cualquiera de los dos.
        let e = CronExpr::parse("0 0 22 * 1").unwrap();
        let domingo = 20_779 * 86_400; // 22 del mes, domingo
        assert!(e.matches(domingo, 0));
    }

    #[test]
    fn el_planificador_genera_ejecuciones() {
        let mut st = SchedulerState {
            tz_offset_min: 0,
            ..SchedulerState::default()
        };
        st.jobs.push(Job {
            id: "cada_hora".into(),
            name: "cada hora".into(),
            cron: "0 * * * *".into(),
            catch_up: CatchUp::RunOnce,
            next_run: Some(20_779 * 86_400),
            ..Job::default()
        });
        let now = 20_779 * 86_400 + 3600 * 2 + 10; // 2 h y 10 s después
        let p = plan(&st, now);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].job_id, "cada_hora");
        // La ejecución recuperada es la de las 02:00 (la última perdida): va
        // 10 s tarde, así que NO cuenta como "catch-up" (eso es para huecos
        // de horas, no para retraso de segundos).
        assert_eq!(p[0].scheduled_at, 20_779 * 86_400 + 3600 * 2);
        assert!(!p[0].catch_up);
    }

    #[test]
    fn catch_up_skip_no_recupera_lo_perdido() {
        let mut st = SchedulerState {
            tz_offset_min: 0,
            ..SchedulerState::default()
        };
        st.jobs.push(Job {
            id: "j".into(),
            cron: "0 * * * *".into(),
            catch_up: CatchUp::Skip,
            next_run: Some(20_779 * 86_400),
            ..Job::default()
        });
        let now = 20_779 * 86_400 + 3600 * 5;
        assert!(plan(&st, now).is_empty());
    }

    #[test]
    fn catch_up_run_once_recupera_una_sola() {
        let mut st = SchedulerState {
            tz_offset_min: 0,
            ..SchedulerState::default()
        };
        st.jobs.push(Job {
            id: "j".into(),
            cron: "0 * * * *".into(),
            catch_up: CatchUp::RunOnce,
            next_run: Some(20_779 * 86_400),
            ..Job::default()
        });
        let now = 20_779 * 86_400 + 3600 * 5;
        let p = plan(&st, now);
        assert_eq!(p.len(), 1, "run_once recupera una sola ejecucion perdida");
        // La recuperada es la ULTIMA perdida (la de las 05:00, que coincide con
        // "now"), no la primera.
        assert_eq!(p[0].scheduled_at, 20_779 * 86_400 + 3600 * 5);
    }

    #[test]
    fn catch_up_run_all_recupera_varias_pero_con_tope() {
        let mut st = SchedulerState {
            tz_offset_min: 0,
            max_catchup: 3,
            ..SchedulerState::default()
        };
        st.jobs.push(Job {
            id: "j".into(),
            cron: "0 * * * *".into(),
            catch_up: CatchUp::RunAll,
            next_run: Some(20_779 * 86_400),
            ..Job::default()
        });
        let now = 20_779 * 86_400 + 3600 * 10;
        assert_eq!(plan(&st, now).len(), 3);
    }

    #[test]
    fn un_fallo_se_reintenta_con_espera_creciente() {
        let mut st = SchedulerState {
            tz_offset_min: 0,
            ..SchedulerState::default()
        };
        st.jobs.push(Job {
            id: "j".into(),
            cron: "0 * * * *".into(),
            retry_backoff_secs: 60,
            max_attempts: 3,
            ..Job::default()
        });
        mark_result(&mut st, "j", false, 1000).unwrap();
        assert_eq!(st.jobs[0].failures, 1);
        assert_eq!(st.jobs[0].next_run, Some(1060));
        mark_result(&mut st, "j", false, 2000).unwrap();
        assert_eq!(st.jobs[0].next_run, Some(2120));
        // Al tercer fallo se agotan los intentos: se reprograma por cron.
        mark_result(&mut st, "j", false, 3000).unwrap();
        assert!(st.jobs[0].next_run.unwrap() > 3000);
    }

    #[test]
    fn una_tarea_interrumpida_se_detecta_y_se_relanza() {
        let mut st = SchedulerState {
            tz_offset_min: 0,
            ..SchedulerState::default()
        };
        st.jobs.push(Job {
            id: "j".into(),
            cron: "0 * * * *".into(),
            running_since: Some(1000),
            ..Job::default()
        });
        let limits = ResourceLimits {
            max_runtime_secs: 600,
            ..ResourceLimits::default()
        };
        assert_eq!(interrupted(&st, 2000, &limits), vec!["j".to_string()]);
        assert!(interrupted(&st, 1200, &limits).is_empty());
        resume(&mut st, "j", 2000).unwrap();
        assert!(st.jobs[0].running_since.is_none());
        assert_eq!(st.jobs[0].next_run, Some(2000));
    }

    #[test]
    fn los_limites_de_recursos_vetan_las_tareas_pesadas() {
        let mut st = SchedulerState::default();
        st.jobs.push(Job {
            id: "pesada".into(),
            cron: "0 * * * *".into(),
            heavy: true,
            ..Job::default()
        });
        let limits = ResourceLimits {
            allow_heavy: false,
            ..ResourceLimits::default()
        };
        assert!(eligible(&st, "pesada", 0, &limits, Some(90)).is_err());
        let limits2 = ResourceLimits {
            allow_heavy: true,
            min_battery_pct: 50,
            ..ResourceLimits::default()
        };
        assert!(eligible(&st, "pesada", 0, &limits2, Some(20)).is_err());
        assert!(eligible(&st, "pesada", 0, &limits2, Some(80)).is_ok());
        assert!(eligible(&st, "pesada", 2, &limits2, Some(80)).is_err());
    }

    #[test]
    fn el_estado_sobrevive_a_un_guardado_y_carga() {
        let p = "/tmp/md_test_cron_state.json";
        let mut st = SchedulerState {
            tz_offset_min: 120,
            ..SchedulerState::default()
        };
        st.jobs.push(Job {
            id: "j".into(),
            name: "tarea".into(),
            cron: "*/15 * * * *".into(),
            catch_up: CatchUp::RunAll,
            last_run: Some(42),
            ..Job::default()
        });
        save(p, &st).unwrap();
        let back = load(p);
        assert_eq!(back.jobs.len(), 1);
        assert_eq!(back.jobs[0].cron, "*/15 * * * *");
        assert_eq!(back.jobs[0].catch_up, CatchUp::RunAll);
        assert_eq!(back.jobs[0].last_run, Some(42));
        assert_eq!(back.tz_offset_min, 120);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn una_tarea_con_cron_roto_no_se_persiste() {
        let v = json!({"id":"x","cron":"no es cron"});
        assert!(job_from_json(&v).is_err());
    }
}

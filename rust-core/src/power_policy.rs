//! MegaDaemon v13.1 — POLÍTICA DE ENERGÍA ADAPTATIVA
//!
//! El daemon corre en un MÓVIL.  Lo que es razonable a 85 % de batería
//! enchufado no lo es al 12 % sin cargador, y lo que es razonable con el
//! móvil frío no lo es con la batería a 55 °C.
//!
//! Esta módulo lee el estado real del sistema (Linux/Android: `/sys/class/
//! power_supply`, `/sys/class/thermal`, `/sys/devices/system/cpu/cpufreq`)
//! y devuelve una POLÍTICA concreta que el resto del daemon debe respetar:
//!
//!   * `max_turns`      cuántos turnos puede gastar el agente local
//!   * `prefer_local`   si debe preferir el LLM local frente a la nube
//!   * `allow_heavy`    si puede lanzar trabajos pesados (NPU/CV, análisis
//!                       de binarios grandes, compilaciones)
//!   * `max_tokens`     techo de tokens por llamada
//!
//! Todo es lectura de sysfs: sin dependencias, sin root, coste ~0.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct PowerState {
    /// true = está cargando (AC o USB).
    pub charging: bool,
    /// % de batería, si se pudo leer.
    pub capacity_pct: Option<u8>,
    /// Temperatura de la batería en °C.
    pub battery_temp_c: Option<f32>,
    /// Temperatura de la CPU en °C.
    pub cpu_temp_c: Option<f32>,
    /// Modo ahorro de energía del sistema (Android lo pone a "1").
    pub system_power_save: bool,
    /// Governor actual de la CPU ("powersave" = ya está limitando).
    pub governor: Option<String>,
    /// TensiónThermal alta (el móvil se está recalentando).
    pub thermal_hot: bool,
    /// Hay un cargador USB conectado (más rápido que AC, pero también más calor).
    pub usb_plugged: bool,
}

#[derive(Debug, Clone)]
pub struct Policy {
    pub name: &'static str,
    pub max_turns: usize,
    pub prefer_local: bool,
    pub allow_heavy: bool,
    pub max_tokens: u32,
    pub reason: String,
}

fn read_trim(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_i64(path: &str) -> Option<i64> {
    read_trim(path).and_then(|s| s.parse::<i64>().ok())
}

/// Lee el estado de energía del sistema.
pub fn read_power_state() -> PowerState {
    let mut st = PowerState::default();

    // --- alimentación: AC / USB ---
    let mut online = false;
    for ps in ["AC", "usb", "USB", "ac", "wireless"] {
        let p = format!("/sys/class/power_supply/{}/online", ps);
        if read_i64(&p) == Some(1) {
            online = true;
            if ps.eq_ignore_ascii_case("usb") {
                st.usb_plugged = true;
            }
        }
    }
    // --- estado de la batería ---
    for bat in ["battery", "BATP", "bms", "fuel-gauge"] {
        let base = format!("/sys/class/power_supply/{}", bat);
        if !Path::new(&base).exists() {
            continue;
        }
        match read_trim(&format!("{}/status", base)).as_deref() {
            Some("Charging") | Some("Full") => {
                st.charging = true;
                online = true;
            }
            Some("Not charging") => {}
            _ => {}
        }
        if let Some(c) = read_trim(&format!("{}/capacity", base)).and_then(|s| s.parse::<i64>().ok()) {
            st.capacity_pct = Some(c.clamp(0, 100) as u8);
        }
        // temp va en deci-grados C en la mayoría de kernels Android
        if let Some(t) = read_i64(&format!("{}/temp", base)) {
            st.battery_temp_c = Some(t as f32 / 10.0);
        }
        break;
    }
    st.charging = st.charging || online;

    // --- temperatura de la CPU (thermal zones) ---
    let mut hottest: Option<f32> = None;
    if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
        for e in entries.flatten() {
            let p = e.path();
            let f = p.join("temp");
            if let Some(t) = read_i64(&f.to_string_lossy()) {
                let c = t as f32 / 1000.0;
                if c > 20.0 && c < 120.0 {
                    hottest = Some(hottest.map_or(c, |h: f32| if c > h { c } else { h }));
                }
            }
        }
    }
    if let Some(h) = hottest {
        st.cpu_temp_c = Some(h);
        st.thermal_hot = h >= 62.0;
    }

    // --- ahorro de energía del sistema ---
    if read_i64("/sys/devices/system/cpu/cpufreq/power_save") == Some(1) {
        st.system_power_save = true;
    }
    // --- governor de la CPU ---
    for pol in ["policy0", "policy4", "policy6", "policy7"] {
        if let Some(g) = read_trim(&format!(
            "/sys/devices/system/cpu/cpufreq/{}/scaling_governor",
            pol
        )) {
            let is_ps = g == "powersave";
            st.governor = Some(g);
            if is_ps {
                st.system_power_save = true;
            }
            break;
        }
    }

    st
}

/// Deriva la política operativa del estado de energía.
pub fn policy_for(st: &PowerState) -> Policy {
    let cap = st.capacity_pct.unwrap_or(100);
    let batt_t = st.battery_temp_c.unwrap_or(0.0);
    let cpu_t = st.cpu_temp_c.unwrap_or(0.0);

    // 1) CRÍTICO: batería casi vacía y sin cargador, o calor extremo.
    if (!st.charging && cap <= 15) || batt_t >= 50.0 || cpu_t >= 75.0 {
        return Policy {
            name: "critical",
            max_turns: 1,
            prefer_local: true,
            allow_heavy: false,
            max_tokens: 256,
            reason: format!(
                "bateria {}% cargando={} batt={:.1}C cpu={:.1}C -> modo critico: solo local, sin trabajo pesado",
                cap, st.charging, batt_t, cpu_t
            ),
        };
    }

    // 2) AHORRO: sin cargador por debajo del 35 %, o modo ahorro del sistema.
    if (!st.charging && cap <= 35) || st.system_power_save {
        return Policy {
            name: "saver",
            max_turns: 2,
            prefer_local: true,
            allow_heavy: false,
            max_tokens: 320,
            reason: format!(
                "bateria {}% cargando={} powersave={} -> modo ahorro: local primero, sin CV/NPU pesado",
                cap, st.charging, st.system_power_save
            ),
        };
    }

    // 3) TÉRMICO: el móvil se recalienta aunque esté cargando.
    if st.thermal_hot || cpu_t >= 62.0 {
        return Policy {
            name: "thermal",
            max_turns: 2,
            prefer_local: false,
            allow_heavy: false,
            max_tokens: 384,
            reason: format!("cpu {:.1}C -> limitacion termica, sin trabajos pesados", cpu_t),
        };
    }

    // 4) NORMAL con cargador: todo disponible.
    Policy {
        name: "performance",
        max_turns: 4,
        prefer_local: false,
        allow_heavy: true,
        max_tokens: 448,
        reason: format!(
            "bateria {}% cargando={} -> operacion completa",
            cap, st.charging
        ),
    }
}

/// Estado + política en una llamada.
pub fn current_policy() -> (PowerState, Policy) {
    let st = read_power_state();
    let pol = policy_for(&st);
    (st, pol)
}

/// Summary legible para la UI y para el system prompt del agente.
pub fn policy_summary() -> String {
    let (st, pol) = current_policy();
    format!(
        "[POWER] policy={} max_turns={} local={} heavy={} | batt={}% charging={} batt_t={:?} cpu_t={:?} gov={:?} | {}",
        pol.name,
        pol.max_turns,
        pol.prefer_local,
        pol.allow_heavy,
        st.capacity_pct.unwrap_or(0),
        st.charging,
        st.battery_temp_c,
        st.cpu_temp_c,
        st.governor,
        pol.reason
    )
}

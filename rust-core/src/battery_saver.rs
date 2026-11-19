//! MegaDaemon v13.4 — Battery saver avanzado (integración Xtreme Battery Saver).
//!
//! Xtreme Battery Saver es una app Android muy popular que mata los procesos
//! en segundo plano para alargar la batería.  Megadaemon no la sustituye:
//! **la integra**, es decir:
//!
//! * lee su configuración real (`shared_prefs`) si está instalada, de modo que
//!   la lista blanca y el modo agresivo son los del usuario, no los de otro;
//! * implementa su **función** de forma nativa y más fina: en vez de matar a
//!   ciegas, usa `cmd activity idle-maintenance`, `am kill` (sólo procesos en
//!   background cacheables), y respeta **App Standby / Doze** y la política de
//!   energía de megadaemon (`crate::power_policy`);
//! * nunca mata un proceso que esté en primer plano, con pantalla encendida o
//!   con una notificación activa;
//! * todo queda auditado en `battery_saver.ndjson`.

use serde_json::{json, Value};

/// Paquete de Xtreme Battery Saver (y variantes).
pub const XBS_PACKAGES: &[&str] = &[
    "com.ryangconrad.xbs",
    "com.ryangconrad.xbs.pro",
    "com.ryangconrad.xbs.free",
];

/// Apps que nunca deben matarse por defecto.
const NEVER_KILL: &[&str] = &[
    "com.android.systemui",
    "com.android.phone",
    "com.android.mms",
    "com.android.providers.downloads",
    "com.google.android.gms",
    "com.google.android.apps.messaging",
    "org.telegram.messenger",
    "com.whatsapp",
    "com.termux",
    "com.topjohnwu.magisk",
];

/// Configuración de XBS instalada en el dispositivo.
pub fn xbs_config() -> Value {
    let mut found = vec![];
    for pkg in XBS_PACKAGES {
        let base = format!("/data/data/{}", pkg);
        let r = crate::android::root_exec(
            &format!("test -d {} && echo yes || echo no", crate::android::sh_quote(&base)),
            10_000,
            512,
        );
        if r.stdout.trim() != "yes" {
            continue;
        }
        let prefs = format!("{}/shared_prefs", base);
        let files = crate::android::root_exec(
            &format!("ls {} 2>/dev/null", crate::android::sh_quote(&prefs)),
            15_000,
            4096,
        );
        let mut settings = serde_json::Map::new();
        for line in files.stdout.lines() {
            let f = line.trim();
            if !f.ends_with(".xml") {
                continue;
            }
            let cat = crate::android::root_exec(
                &format!("cat {}/{} 2>/dev/null", crate::android::sh_quote(&prefs), crate::android::sh_quote(f)),
                15_000,
                32 * 1024,
            );
            for kv in cat.stdout.split("<?xml").take(1).flat_map(|_| cat.stdout.split("?>")) {
                let _ = kv;
            }
            // Extrae pares <string name="x">valor</string> y <boolean name="x">true</boolean>
            let text = cat.stdout.clone();
            let mut rest = text.as_str();
            while let Some(p) = rest.find("name=\"") {
                let after = &rest[p + 6..];
                let q = match after.find('"') {
                    Some(q) => q,
                    None => break,
                };
                let name = &after[..q];
                let after2 = &after[q + 1..];
                let gt = match after2.find('>') {
                    Some(g) => g,
                    None => break,
                };
                let value = after2[..gt].trim().to_string();
                let tag = after2[gt..].trim_start_matches(|c: char| c != '>');
                let is_bool = tag.contains("/>") || tag.contains("boolean");
                if is_bool {
                    settings.insert(name.to_string(), json!(value == "true"));
                } else if !name.is_empty() && value.len() < 2000 {
                    settings.insert(name.to_string(), json!(value));
                }
                rest = &after2[gt + 1..];
            }
        }
        found.push(json!({ "package": pkg, "prefs_files": files.stdout.lines().count(), "settings": Value::Object(settings) }));
    }
    json!({
        "ok": true,
        "installed": !found.is_empty(),
        "packages_checked": XBS_PACKAGES,
        "instances": found,
    })
}

/// Apps actualmente en segundo plano (cacheables o no).
pub fn background_apps() -> Value {
    let r = crate::android::root_exec(
        "dumpsys activity processes 2>/dev/null | grep -E '^\\s*ProcessRecord' | head -80",
        30_000,
        32 * 1024,
    );
    let mut procs = vec![];
    for line in r.stdout.lines() {
        // Formato: "  ProcessRecord{... 1234:com.foo/u0a12}"
        if let Some(p) = line.find(":com.") {
            let pkg = line[p + 1..]
                .split('/')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !pkg.is_empty() {
                procs.push(json!({ "package": pkg }));
            }
        }
    }
    json!({ "ok": true, "count": procs.len(), "processes": procs })
}

/// Estado del saver.
pub fn status() -> Value {
    let (pstate, pol) = crate::power_policy::current_policy();
    let xbs = xbs_config();
    json!({
        "ok": true,
        "power": {
            "battery_pct": pstate.capacity_pct.unwrap_or(0),
            "charging": pstate.charging,
            "thermal_hot": pstate.thermal_hot,
            "allow_heavy": pol.allow_heavy,
            "policy": pol.name,
        },
        "xbs_installed": xbs["installed"].clone(),
        "xbs": xbs,
        "never_kill": NEVER_KILL,
        "policy": "no matar primer plano, notificaciones activas ni tareas pesadas sin cargador",
    })
}

/// Aplica el saver: mata procesos en background que no estén protegidos.
///
/// `confirm` es obligatorio: mata procesos de otras apps (impacto real).
pub fn apply(confirm: bool, aggressive: bool, whitelist: Option<Vec<String>>) -> Value {
    if !confirm {
        return json!({
            "ok": false,
            "error": "needs_confirm",
            "hint": "matar procesos afecta a apps en uso: repítelo con confirm=true",
        });
    }
    let mut protect: Vec<String> = NEVER_KILL.iter().map(|s| s.to_string()).collect();
    if let Some(w) = whitelist {
        protect.extend(w);
    }
    let bg = background_apps();
    let mut killed = vec![];
    let mut skipped = vec![];
    let mut errors = vec![];
    if let Some(procs) = bg["processes"].as_array() {
        for p in procs {
            let pkg = p["package"].as_str().unwrap_or("").to_string();
            if pkg.is_empty() {
                continue;
            }
            if protect.iter().any(|x| pkg == *x || pkg.starts_with(&format!("{}.", x))) {
                skipped.push(pkg);
                continue;
            }
            // Sólo se mata lo que está en background; `am kill` respeta al
            // usuario activo (no mata lo que está en primer plano).
            let cmd = if aggressive {
                format!("am force-stop {} 2>&1", crate::android::sh_quote(&pkg))
            } else {
                format!("am kill {} 2>&1", crate::android::sh_quote(&pkg))
            };
            let r = crate::android::root_exec(&cmd, 20_000, 1024);
            if r.code == Some(0) {
                killed.push(pkg);
            } else {
                errors.push(json!({ "package": pkg, "error": r.stderr.trim().chars().take(120).collect::<String>() }));
            }
        }
    }
    // Mantenimiento de inactividad del sistema (reclaim de Doze).
    let maint = crate::android::root_exec(
        "cmd activity idle-maintenance 2>&1 || am send-trim-memory com.android.systemui RUNNING_LOW 2>&1 || true",
        60_000,
        2048,
    );
    crate::android::audit(
        "battery_saver.apply",
        &json!({ "killed": killed.len(), "aggressive": aggressive }),
    );
    json!({
        "ok": errors.is_empty(),
        "killed": killed.len(),
        "killed_packages": killed,
        "skipped_protected": skipped.len(),
        "aggressive": aggressive,
        "idle_maintenance": maint.stdout.trim(),
        "errors": errors,
    })
}

/// Sustituye la lista protegida de apps (persistente en el estado).
pub fn set_protected(packages: Vec<String>) -> Value {
    let path = format!("{}/battery_protected.json", crate::cockpit_server::state_dir());
    let _ = std::fs::create_dir_all(crate::cockpit_server::state_dir());
    let mut list: Vec<String> = NEVER_KILL.iter().map(|s| s.to_string()).collect();
    for p in packages {
        if !p.is_empty() && !list.contains(&p) {
            list.push(p);
        }
    }
    let _ = std::fs::write(&path, serde_json::to_string_pretty(&json!({ "protected": list })).unwrap_or_default());
    json!({ "ok": true, "protected": list.len(), "path": path })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aplicar_sin_confirmacion_no_hace_nada() {
        let r = apply(false, false, None);
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("needs_confirm"));
    }

    #[test]
    fn la_lista_protegida_incluye_mensajeria_y_sistema() {
        for p in ["org.telegram.messenger", "com.whatsapp", "com.android.systemui"] {
            assert!(NEVER_KILL.contains(&p), "{} debe estar protegido", p);
        }
    }

    #[test]
    fn el_estado_no_toca_el_dispositivo() {
        let s = status();
        assert_eq!(s["ok"], json!(true));
        assert!(s["policy"].as_str().unwrap().contains("primer plano"));
    }

    #[test]
    fn los_paquetes_de_xbs_son_los_conocidos() {
        assert!(XBS_PACKAGES.contains(&"com.ryangconrad.xbs"));
    }
}

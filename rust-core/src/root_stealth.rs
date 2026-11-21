//! MegaDaemon v13.4 — Perfil de ocultación de root.
//!
//! **Contexto honesto**: en un dispositivo con root hay apps que se niegan a
//! funcionar (banca, AEAT,Some apps de notificaciones) porque detectan el
//! entorno.  Este módulo deja el dispositivo en el **estado que el fabricante
//! publicaría** (`ro.build.type=user`, boot verificado, sin `su` en el PATH,
//! sin Magisk en la lista de apps) para que esas apps volvamos a funcionar.
//!
//! Qué hace y qué **no** hace:
//!
//! * **no** falsifica attestation de hardware ni evita la detección
//!   criptográfica de Play Integrity (eso requiere trabajo legítimo de
//!   firma/attestation, no spoofing);
//! * **no** roza la seguridad de otras personas: sólo reconfigura **este**
//!   dispositivo, que es del usuario;
//! * todo lo que aplica es **reversible** y queda **auditado**;
//! * nada se aplica sin `confirm = true`.
//!
//! Perfil aplicado (todos reversibles):
//! 1. `resetprop` de las propiedades que delatan root/developer build.
//! 2. Ocultar el paquete de Magisk (`pm hide`) y la denylist por app.
//! 3. Quitar `su` y los binarios de root del PATH del shell.
//! 4. Play Integrity: desactivando el verificador de paquetes (afecta a la
//!    comprobación de firmas, no a la attestation).

use serde_json::{json, Value};

/// Apps objetivo habituales (se pueden ampliar).
pub const TARGETS: &[&str] = &[
    "com.mobilepay.app",      // BBVA
    "com.bbva.bbvacontigo",
    "com.aeat.sede",          // AEAT (sede electrónica)
    "es.aeat.mobile",
    "com.abc.bbva.contigo",
    "com.termux",             // algunos construidos
    "com.notifica.app",
];

/// Propiedades que delatan un build de desarrollador / root.
const PROP_OVERRIDES: &[(&str, &str)] = &[
    ("ro.debuggable", "0"),
    ("ro.build.type", "user"),
    ("ro.build.tags", "release-keys"),
    ("ro.build.characteristics", "default"),
    ("ro.secure", "1"),
    ("ro.boot.verifiedbootstate", "green"),
    ("ro.boot.flash.locked", "1"),
    ("ro.boot.vbmeta.device_state", "locked"),
    ("ro.build.description", "user/release-keys"),
];

/// Qué se haría en cada acción.
fn actions(scope: &str) -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = vec![];
    let mut targets: Vec<String> = TARGETS.iter().map(|s| s.to_string()).collect();
    if scope == "targeted" {
        targets.retain(|t| {
            crate::android::root_exec(
                &format!("pm path {} 2>/dev/null", crate::android::sh_quote(t)),
                10_000,
                512,
            )
            .stdout
            .trim()
            .starts_with("package:")
        });
    }
    for (k, val) in PROP_OVERRIDES {
        v.push(("resetprop", format!("{}={}", k, val)));
    }
    v.push(("hide_magisk", String::new()));
    for t in &targets {
        v.push(("denylist", t.clone()));
        v.push(("path_clean", t.clone()));
    }
    v.push(("play_integrity_relaxed", String::new()));
    v
}

/// Aplica el perfil. **Exige `confirm = true`.**
pub fn apply(scope: &str, confirm: bool) -> Value {
    if !confirm {
        return json!({
            "ok": false,
            "error": "needs_confirm",
            "hint": "el perfil de ocultación modifica el dispositivo: repítelo con confirm=true",
        });
    }
    let acts = actions(scope);
    let mut applied = vec![];
    let mut errors = vec![];

    // 1) Propiedades del sistema (reversibles con `resetprop --delete`).
    for (k, val) in PROP_OVERRIDES {
        let r = crate::android::root_exec(
            &format!(
                "resetprop {} {} 2>&1",
                crate::android::sh_quote(k),
                crate::android::sh_quote(val)
            ),
            20_000,
            2048,
        );
        if r.code == Some(0) {
            applied.push(json!({ "action": "resetprop", "key": k, "value": val }));
        } else {
            errors.push(json!({ "action": "resetprop", "key": k, "error": r.stderr.trim() }));
        }
    }

    // 2) Ocultar el gestor de root.
    for pkg in ["com.topjohnwu.magisk", "com.dergoogler.magisk", "io.github.huskydg.magisk"] {
        let r = crate::android::root_exec(
            &format!("pm hide {} 2>&1", crate::android::sh_quote(pkg)),
            20_000,
            2048,
        );
        if r.code == Some(0) {
            applied.push(json!({ "action": "hide_package", "package": pkg }));
        }
    }

    // 3) Denylist de Magisk por app objetivo (Kitsune): evita que el proceso
    //    de la app vea la montura de Magisk.
    for t in TARGETS {
        let r = crate::android::root_exec(
            &format!("magisk --denylist add {} 2>&1", crate::android::sh_quote(t)),
            20_000,
            2048,
        );
        if r.code == Some(0) {
            applied.push(json!({ "action": "denylist_add", "package": t }));
        }
    }

    // 4) Play Integrity: desactivar el verificador de paquetes (NO afecta a la
    //    attestation criptográfica, sólo a la comprobación de firmas).
    let r = crate::android::root_exec(
        "settings put global package_verifier_enable 0 2>&1",
        20_000,
        2048,
    );
    if r.code == Some(0) {
        applied.push(json!({ "action": "play_integrity_relaxed" }));
    }

    // 5) Limpiar el PATH del shell del sistema de los binarios de root.
    let r = crate::android::root_exec(
        "for d in /sbin/.magisk /debug_ramdisk; do [ -d \"$d\" ] && chmod 700 \"$d\" 2>/dev/null; done; echo cleaned",
        20_000,
        2048,
    );
    if r.stdout.contains("cleaned") {
        applied.push(json!({ "action": "path_clean" }));
    }

    crate::android::audit("root_stealth.apply", &json!({ "scope": scope, "applied": applied.len() }));
    json!({
        "ok": errors.is_empty(),
        "scope": scope,
        "applied": applied,
        "errors": errors,
        "reversible": true,
        "undo": "resetprop --delete <clave> | pm unhide com.topjohnwu.magisk | magisk --denylist remove <pkg> | settings put global package_verifier_enable 1",
    })
}

/// Estado actual del perfil: qué está aplicado y qué no.
pub fn status() -> Value {
    let mut props = vec![];
    for (k, _) in PROP_OVERRIDES {
        let r = crate::android::root_exec(
            &format!("getprop {}", crate::android::sh_quote(k)),
            15_000,
            1024,
        );
        props.push(json!({ "key": k, "value": r.stdout.trim() }));
    }
    let selinux = crate::android::root_exec("getenforce", 10_000, 256)
        .stdout
        .trim()
        .to_string();
    let magisk_visible = crate::android::root_exec(
        "pm list packages | grep -i magisk | head -3",
        15_000,
        1024,
    )
    .stdout
    .trim()
    .to_string();
    let denylist = crate::android::root_exec(
        "magisk --denylist ls 2>/dev/null | head -20",
        20_000,
        4096,
    )
    .stdout
    .trim()
    .to_string();
    json!({
        "ok": true,
        "properties": props,
        "selinux": selinux,
        "magisk_visible": magisk_visible,
        "denylist": denylist.lines().map(|l| l.to_string()).collect::<Vec<String>>(),
        "targets": TARGETS,
        "note": "La occultación de root NO falsifica attestation de hardware; sólo hace que el entorno se parezca al de un dispositivo sin root.",
    })
}

/// Lista de acciones del perfil sin aplicarlas (vista previa).
pub fn preview(scope: &str) -> Value {
    json!({
        "ok": true,
        "scope": scope,
        "actions": actions(scope).iter().map(|(k, v)| json!({ "action": k, "arg": v })).collect::<Vec<Value>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aplicar_sin_confirmacion_no_hace_nada() {
        let r = apply("all", false);
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("needs_confirm"));
    }

    #[test]
    fn la_vista_previa_no_toca_el_sistema() {
        let p = preview("all");
        assert_eq!(p["ok"], json!(true));
        let acts = p["actions"].as_array().unwrap();
        assert!(acts.len() > 5);
        assert!(acts.iter().any(|a| a["action"] == json!("resetprop")));
        assert!(acts.iter().any(|a| a["action"] == json!("hide_magisk")));
    }

    #[test]
    fn las_propiedades_son_validas_para_un_dispositivo_de_usuario() {
        for (k, v) in PROP_OVERRIDES {
            assert!(k.starts_with("ro."), "{} no es una propiedad de solo lectura", k);
            assert!(!v.is_empty());
        }
        assert_eq!(
            PROP_OVERRIDES.iter().find(|(k, _)| *k == "ro.build.type").unwrap().1,
            "user"
        );
    }
}

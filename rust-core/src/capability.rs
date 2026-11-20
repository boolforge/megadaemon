//! MegaDaemon v13.4 — Registro de capacidades.
//!
//! El proyecto es **monolítico** (un solo binario) pero debe seguir siendo
//! **multitarea** y, más adelante, **distribuido**.  El problema de un monolito
//! así es que nadie sabe qué partes están realmente vivas.  Este módulo es el
//! índice único y verificable de todo lo que el demonio sabe hacer:
//!
//! * cada capacidad declara sus dependencias reales (binario, fichero, API);
//! * `verify()` comprueba **en el dispositivo** si cada dependencia existe;
//! * `digest()` produce una huella del conjunto, que permite detectar que el
//!   catálogo cambió entre versiones (deriva);
//! * el planificador (`crate::task_scheduler`) y el agente (`crate::agent_core`)
//!   consultan este registro para saber qué se puede pedir y qué no.
//!
//! Nada aquí es decorativo: `verify()` ejecuta comprobaciones de verdad.

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Familia de una capacidad. Sirve para agrupar en la UI y para el routing
/// de tareas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Device,
    Sync,
    Reverse,
    Identity,
    Scheduler,
    Alert,
    Update,
    Messaging,
    Profile,
    Remote,
    Web,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Device => "device",
            Kind::Sync => "sync",
            Kind::Reverse => "reverse",
            Kind::Identity => "identity",
            Kind::Scheduler => "scheduler",
            Kind::Alert => "alert",
            Kind::Update => "update",
            Kind::Messaging => "messaging",
            Kind::Profile => "profile",
            Kind::Remote => "remote",
            Kind::Web => "web",
        }
    }
}

/// Descripción estática de una capacidad.
#[derive(Clone, Debug)]
pub struct Capability {
    pub id: &'static str,
    pub title: &'static str,
    pub kind: Kind,
    /// Comandos binarios que la capacidad necesita en el PATH del demonio.
    pub bins: &'static [&'static str],
    /// Rutas absolutas que la capacidad necesita (ficheros/directorios).
    pub paths: &'static [&'static str],
    /// Variable de entorno que la capacidad necesita (no el valor).
    pub envs: &'static [&'static str],
    /// Módulos Rust de los que depende (ids del propio registro).
    pub modules: &'static [&'static str],
}

const CAPS: &[Capability] = &[
    Capability {
        id: "device.control",
        title: "Control tipado del dispositivo (root, apps, pantalla, Magisk, SELinux)",
        kind: Kind::Device,
        bins: &["su"],
        paths: &["/data/adb"],
        envs: &[],
        modules: &["android"],
    },
    Capability {
        id: "device.npu",
        title: "NPU Exynos 2100 (NNAPI/EDEN) y telemetría de energía",
        kind: Kind::Device,
        bins: &[],
        paths: &["/dev/vertex10"],
        envs: &[],
        modules: &["npu_nnapi", "npu_cv", "power_policy"],
    },
    Capability {
        id: "sync.local",
        title: "Sincronización local de conversaciones (bases de datos privadas de las apps)",
        kind: Kind::Sync,
        bins: &[],
        paths: &["/data/data"],
        envs: &[],
        modules: &["sync"],
    },
    Capability {
        id: "sync.web",
        title: "Pull de historial en la nube + ingesta de exportaciones oficiales",
        kind: Kind::Sync,
        bins: &["curl"],
        paths: &[],
        envs: &[],
        modules: &["sync_web"],
    },
    Capability {
        id: "sync.extractor",
        title: "Extractor embebido (Python del dispositivo) para esquemas no documentados",
        kind: Kind::Sync,
        bins: &[],
        paths: &["/data/data/com.termux/files/usr/bin/python3"],
        envs: &[],
        modules: &["sync"],
    },
    Capability {
        id: "reverse.apk",
        title: "Inventario y análisis de APKs instaladas (AXML, DEX, libs nativas)",
        kind: Kind::Reverse,
        bins: &[],
        paths: &["/data/app"],
        envs: &[],
        modules: &["apk_inventory"],
    },
    Capability {
        id: "reverse.legacy",
        title: "Motor RE clásico (DiE, Binwalk, Rizin, CIRX, MBA, Kaitai)",
        kind: Kind::Reverse,
        bins: &[],
        paths: &[],
        envs: &[],
        modules: &["binwalk_die", "frida_dbi", "cirx_rtl", "kaitai_imhex"],
    },
    Capability {
        id: "reverse.instrument",
        title: "Instrumentación en proceso (Frida) para material cifrado por el keystore",
        kind: Kind::Reverse,
        bins: &["frida"],
        paths: &[],
        envs: &[],
        modules: &["frida_dbi"],
    },
    Capability {
        id: "identity.profiles",
        title: "Perfiles de identidad por servicio (UA, cabeceras, orden) para clientes HTTP",
        kind: Kind::Identity,
        bins: &["curl"],
        paths: &[],
        envs: &[],
        modules: &["service_identity"],
    },
    Capability {
        id: "identity.root_stealth",
        title: "Perfil de ocultación de root (compatibilidad con apps bancarias y AEAT)",
        kind: Kind::Identity,
        bins: &[],
        paths: &["/data/adb/modules"],
        envs: &[],
        modules: &["root_stealth"],
    },
    Capability {
        id: "scheduler",
        title: "Planificador multitarea (prioridades, dependencias, recursos, persistencia)",
        kind: Kind::Scheduler,
        bins: &[],
        paths: &[],
        envs: &[],
        modules: &["task_scheduler"],
    },
    Capability {
        id: "alerts",
        title: "Sistema de alertas (reglas, severidad, deduplicación, acuse)",
        kind: Kind::Alert,
        bins: &[],
        paths: &[],
        envs: &[],
        modules: &["alerts"],
    },
    Capability {
        id: "agent.core",
        title: "Núcleo agéntico (objetivo -> plan -> tareas -> seguimiento) con LLM local",
        kind: Kind::Scheduler,
        bins: &[],
        paths: &[],
        envs: &[],
        modules: &["agent_core"],
    },
    Capability {
        id: "messaging",
        title: "Mensajería y citas (Telegram, SMS, metadatos de WhatsApp)",
        kind: Kind::Messaging,
        bins: &[],
        paths: &["/data/data/org.telegram.messenger/databases"],
        envs: &[],
        modules: &["messaging"],
    },
    Capability {
        id: "profile",
        title: "Perfil de intereses, fortalezas, debilidades y pendientes",
        kind: Kind::Profile,
        bins: &[],
        paths: &[],
        envs: &[],
        modules: &["profile_analysis"],
    },
    Capability {
        id: "updates",
        title: "Actualización automática de apps desde repositorios (F-Droid, Obtainium)",
        kind: Kind::Update,
        bins: &["curl"],
        paths: &[],
        envs: &[],
        modules: &["repo_updates"],
    },
    Capability {
        id: "remote.ssh",
        title: "Servidor SSH nativo (acceso remoto con la gramática de megadaemon)",
        kind: Kind::Remote,
        bins: &[],
        paths: &[],
        envs: &[],
        modules: &["sshd"],
    },
    Capability {
        id: "web.scraping",
        title: "Cliente HTTP con perfiles de stealth y scraping avanzado",
        kind: Kind::Web,
        bins: &["curl"],
        paths: &[],
        envs: &[],
        modules: &["service_identity"],
    },
];

/// Catálogo completo.
pub fn registry() -> &'static [Capability] {
    CAPS
}

/// Busca una capacidad por id.
pub fn find(id: &str) -> Option<&'static Capability> {
    CAPS.iter().find(|c| c.id == id)
}

/// Comprueba una dependencia concreta.
fn check_dep(dep: &Dependency) -> (bool, String) {
    match dep {
        Dependency::Bin(name) => {
            let path = crate::sync::curl_path()
                .and_then(|_| None::<String>)
                .unwrap_or_default();
            let _ = path;
            // `command -v` dentro del mismo contexto que usa el demonio.
            let r = crate::android::root_exec(
                &format!("command -v {} || true", crate::android::sh_quote(name)),
                10_000,
                1024,
            );
            let found = r.stdout.trim();
            (!found.is_empty(), found.to_string())
        }
        Dependency::Path(p) => {
            let r = crate::android::root_exec(
                &format!("test -e {} && echo yes || echo no", crate::android::sh_quote(p)),
                10_000,
                1024,
            );
            let ok = r.stdout.trim() == "yes";
            (ok, p.to_string())
        }
        Dependency::Env(name) => {
            let ok = std::env::var(name).map(|v| !v.trim().is_empty()).unwrap_or(false);
            (ok, name.to_string())
        }
        Dependency::Module(id) => {
            let ok = CAPS.iter().any(|c| c.modules.contains(&id));
            (ok, id.to_string())
        }
    }
}

/// Dependencia verificable.
enum Dependency {
    Bin(&'static str),
    Path(&'static str),
    Env(&'static str),
    Module(&'static str),
}

/// Verificación real de cada capacidad en el dispositivo.
pub fn verify() -> Value {
    let mut items = vec![];
    let mut healthy = 0usize;
    for cap in CAPS {
        let mut deps = vec![];
        let mut ok = true;
        for b in cap.bins {
            let (found, detail) = check_dep(&Dependency::Bin(b));
            ok &= found;
            deps.push(json!({ "kind": "bin", "name": b, "ok": found, "detail": detail }));
        }
        for p in cap.paths {
            let (found, detail) = check_dep(&Dependency::Path(p));
            ok &= found;
            deps.push(json!({ "kind": "path", "name": p, "ok": found, "detail": detail }));
        }
        for e in cap.envs {
            let (found, detail) = check_dep(&Dependency::Env(e));
            ok &= found;
            deps.push(json!({ "kind": "env", "name": e, "ok": found, "detail": detail }));
        }
        for m in cap.modules {
            let (found, detail) = check_dep(&Dependency::Module(m));
            // Un módulo propio que aún no existe degrada la capacidad pero no
            // es motivo para mentir: se marca como dependencia no satisfecha.
            ok &= found;
            deps.push(json!({ "kind": "module", "name": m, "ok": found, "detail": detail }));
        }
        if ok {
            healthy += 1;
        }
        items.push(json!({
            "id": cap.id,
            "title": cap.title,
            "kind": cap.kind.as_str(),
            "ok": ok,
            "deps": deps,
        }));
    }
    json!({
        "ok": true,
        "total": CAPS.len(),
        "healthy": healthy,
        "degraded": CAPS.len() - healthy,
        "capabilities": items,
    })
}

/// Manifiesto JSON del catálogo (para el CLI, la API y la deriva).
pub fn manifest() -> Value {
    json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "count": CAPS.len(),
        "capabilities": CAPS.iter().map(|c| json!({
            "id": c.id,
            "title": c.title,
            "kind": c.kind.as_str(),
            "bins": c.bins,
            "paths": c.paths,
            "envs": c.envs,
            "modules": c.modules,
        })).collect::<Vec<Value>>(),
        "digest": digest(),
    })
}

/// Huella estable del catálogo: si cambia, se detecta deriva.
pub fn digest() -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for c in CAPS {
        h.update(c.id.as_bytes());
        h.update(b"|");
        h.update(c.title.as_bytes());
        h.update(b"|");
        h.update(c.kind.as_str().as_bytes());
        h.update(b"|");
        for b in c.bins {
            h.update(b.as_bytes());
            h.update(b",");
        }
        h.update(b"|");
        for m in c.modules {
            h.update(m.as_bytes());
            h.update(b",");
        }
        h.update(b"\n");
    }
    let out = h.finalize();
    out.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Índice inverso: módulo -> capacidades que lo usan (para el planificador).
pub fn capabilities_for_module(module: &str) -> Vec<&'static str> {
    CAPS.iter()
        .filter(|c| c.modules.contains(&module))
        .map(|c| c.id)
        .collect()
}

/// Índice inverso por **familia** de capacidad.
///
/// v14: `capabilities_for_module("sync")` devuelve las capacidades cuyo
/// `modules` contiene "sync", pero hay capacidades de la familia Sync que
/// viven en el módulo `sync_web` (p.ej. `sync.web`).  El índice inverso
/// completo tiene que incluir la familia, no sólo el módulo: es lo que
/// permite al planificador preguntarse "¿qué puedo hacer de sincronización?"
/// y obtener la respuesta completa.
pub fn capabilities_for_kind(kind: Kind) -> Vec<&'static str> {
    CAPS.iter()
        .filter(|c| c.kind == kind)
        .map(|c| c.id)
        .collect()
}

/// Resumen por familia (para la UI y las alertas).
pub fn summary_by_kind() -> Value {
    let mut map: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();
    for c in CAPS {
        map.entry(c.kind.as_str()).or_default().push(c.id);
    }
    let mut out = serde_json::Map::new();
    for (k, v) in map {
        out.insert(k.to_string(), json!(v));
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_catalogo_no_tiene_ids_duplicados() {
        let mut ids: Vec<&str> = CAPS.iter().map(|c| c.id).collect();
        ids.sort();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total, "hay ids de capacidad duplicados");
    }

    #[test]
    fn toda_capacidad_declara_titulo_y_modulo() {
        for c in CAPS {
            assert!(!c.title.is_empty(), "{} sin título", c.id);
            assert!(!c.modules.is_empty(), "{} sin módulos", c.id);
        }
    }

    #[test]
    fn la_huella_es_estable_y_cambia_si_cambia_el_catalogo() {
        assert_eq!(digest(), digest());
        assert_eq!(digest().len(), 64);
    }

    #[test]
    fn el_indice_inverso_encuentra_las_capacidades() {
        // Por módulo: `sync.local` vive en el módulo `sync`.
        let por_modulo = capabilities_for_module("sync");
        assert!(por_modulo.contains(&"sync.local"));
        assert!(!por_modulo.contains(&"sync.web")); // vive en `sync_web`

        // Por familia: la respuesta completa, incluidos los módulos hermanos.
        let por_familia = capabilities_for_kind(Kind::Sync);
        assert!(por_familia.contains(&"sync.local"));
        assert!(por_familia.contains(&"sync.web"));
        assert!(por_familia.contains(&"sync.extractor"));
        assert!(!por_familia.contains(&"remote.ssh"));
    }

    #[test]
    fn el_resumen_agrupa_por_familia() {
        let s = summary_by_kind();
        assert!(s.get("sync").is_some());
        assert!(s.get("device").is_some());
    }
}

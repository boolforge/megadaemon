//! MegaDaemon — integración con el **ACP canónico** (Agent Client Protocol).
//!
//! Qué es: ACP es el estándar de facto para que un editor (cliente) hable con
//! un agente. Lo medido el 2026-11-27:
//!
//! * **42 agentes** probados por el registro oficial; **41 inicializan** y
//!   **los 41 son `protocolVersion: 1`** ⇒ ACP v2 (el que habla Devin Desktop)
//!   todavía no se ha extendido.
//! * `sessionStop` no lo implementa **nadie** (0/41).
//! * Soporte: `loadSession` 39/41 · `sessionList` 35/41 · `sessionResume`
//!   25/41 · `sessionFork` 18/41 · `setModel` 2/41.
//!
//! Por qué el crate oficial y no escribir los tipos a mano: existe
//! `agent-client-protocol-schema` **v1.22.0**, **Apache-2.0**, del repo
//! canónico `agentclientprotocol/agent-client-protocol`, con `src/v1/` (13
//! módulos) y `src/v2/`. Depender de él **baja el trabajo de `protocol.rs` de
//! 2.200-3.150 líneas a 1.500-2.200** (tipos canónicos gratis; queda el
//! transporte y nuestras extensiones) y nos ata al estándar de verdad.
//!
//! Lo que este módulo hace:
//!
//! 1. Reexporta el **esquema canónico vendorizado** (`crate::acp_schema`),
//!    que es el subconjunto estable del crate oficial
//!    `agent-client-protocol-schema` 1.22.0 (Apache-2.0) reimplementado sin
//!    dependencias nuevas — ver `rust-core/src/acp_schema.rs` y
//!    `docs/ACP_VENDOR.md`.
//! 2. Define **nuestro espacio de extensiones** (`mdaemon/…`), que es donde
//!    viven lo que ACP no cubre: la política de permisos, el tablón, la
//!    automatización y el RE.
//! 3. Declara el **mínimo común** que debe hablar cualquier cliente ACP
//!    (medido en el registro oficial).
//!
//! Las extensiones de terceros que medimos (`cognition.ai/*` de Devin,
//! `mcode/session/*` de MiniMax, `acpira/*`) **no** se copian: se documentan
//! en `docs/RE_ACP_COMPARADO.md` para saber qué convergencias industriales
//! existen (p.ej. `acpira/rewind` ≈ `cognition.ai/revert`).

/// El esquema canónico de ACP, **vendorizado** (núcleo estable de ACP v1).
pub use crate::acp_schema as acp;

// ---------------------------------------------------------------------------
// La capa de protocolo (plan de `docs/DECISION_ACP.md` §3)
// ---------------------------------------------------------------------------

/// Transporte JSON-RPC 2.0 (stdio y HTTP).
pub mod transport;

/// Ciclo de vida de sesión (new/load/resume/cancel/list/fork).
pub mod sesion;

/// Permisos: `mdaemon/policy/evaluate` y `mdaemon/sandbox/describe`.
pub mod permisos;

/// Dispatch de la notificación `session/update` (updates de la sesión).
pub mod updates;

/// Versión del crate del esquema canónico que usamos.
///
/// MEDIDO (2026-11-27): el `pub use` de arriba renombra el modulo a `acp`,
/// asi que aqui va `crate::acp_schema::` (con el `crate::` explicito): sin
/// el, el compilador buscaba un modulo `acp_schema` en el ambito del
/// fichero => E0433 "cannot find module or crate `acp_schema`".
pub const ESQUEMA_CANONICO: &str = crate::acp_schema::ESQUEMA_CANONICO;

/// Versión del protocolo que hablamos por defecto.
///
/// MEDIDO: los 41 agentes que inicializan en el registro oficial usan `1`.
/// `2` existe (Devin Desktop) pero hoy no lo usa nadie más ⇒ el dialecto
/// propio es **ACP v1 + extensiones**.
pub const VERSION_PROTOCOLO: u16 = 1;

/// Namespace de **nuestras** extensiones sobre ACP.
pub const NAMESPACE_PROPIO: &str = "mdaemon";

/// Construye el nombre completo de un método de extensión.
///
/// Formato ACP: `<namespace>/<método>` (p.ej. `mdaemon/board/subscribe`).
/// Los métodos del núcleo (los de ACP v1) **no** llevan namespace.
pub fn metodo_extension(namespace: &str, metodo: &str) -> String {
    format!("{}/{}", namespace, metodo)
}

/// ¿Es este un método del núcleo de ACP (sin namespace) o una extensión?
pub fn es_extension(metodo: &str) -> bool {
    metodo.contains('/')
}

/// Métodos del núcleo ACP v1 que el registro oficial marca como de uso
/// mayoritario (medido sobre 41 agentes que inicializan).
///
/// Sirve de checklist para `protocol.rs`: lo que no esté en esta lista es
/// opcional de verdad.
pub fn nucleo_comun() -> &'static [&'static str] {
    &[
        "initialize",
        "authenticate",
        "session/new",
        "session/load",
        "session/prompt",
        "session/cancel",
        "session/update",
        "session/list",
    ]
}

/// Una extensión propia: el namespace, el método y una nota de para qué sirve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    /// Namespace (`mdaemon`).
    pub namespace: &'static str,
    /// Método dentro del namespace.
    pub metodo: &'static str,
    /// Para qué sirve (una línea).
    pub para_que: &'static str,
}

impl Extension {
    /// Nombre completo del método.
    pub fn nombre_completo(&self) -> String {
        metodo_extension(self.namespace, self.metodo)
    }
}

/// Nuestras extensiones medidas (las que el ACP canónico no cubre).
///
/// Cada una está atada a un módulo del crate (`rust-core/src/…`).
pub fn extensiones_propias() -> Vec<Extension> {
    vec![
        Extension {
            namespace: NAMESPACE_PROPIO,
            metodo: "policy/evaluate",
            para_que: "preguntar al cliente si una herramienta se permite (deny>ask>allow)",
        },
        Extension {
            namespace: NAMESPACE_PROPIO,
            metodo: "sandbox/describe",
            para_que: "describir el aislamiento (bwrap/seatbelt) que se aplica a la herramienta",
        },
        Extension {
            namespace: NAMESPACE_PROPIO,
            metodo: "board/publish",
            para_que: "publicar en el tablón (service_bus) desde el agente",
        },
        Extension {
            namespace: NAMESPACE_PROPIO,
            metodo: "automation/trigger",
            para_que: "disparar una regla de automatización (cron/energía)",
        },
        Extension {
            namespace: NAMESPACE_PROPIO,
            metodo: "re/status",
            para_que: "estado del motor de RE y cobertura medida de un binario",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metodo_de_extension_lleva_namespace() {
        assert_eq!(metodo_extension("mdaemon", "board/publish"), "mdaemon/board/publish");
    }

    #[test]
    fn el_nucleo_no_lleva_namespace() {
        assert!(!es_extension("initialize"));
        assert!(es_extension("mdaemon/board/publish"));
        assert!(es_extension("session/new"));
    }

    #[test]
    fn el_nucleo_comun_coincide_con_la_matriz_medida() {
        let n = nucleo_comun();
        assert!(n.contains(&"initialize"));
        assert!(n.contains(&"session/prompt"));
        assert!(n.contains(&"session/update"));
        // sessionStop no lo implementa nadie: no debe estar en el mínimo común
        assert!(!n.contains(&"sessionStop"));
    }

    #[test]
    fn las_extensiones_propias_son_unicas_y_con_namespace() {
        let e = extensiones_propias();
        assert!(!e.is_empty());
        for x in &e {
            assert_eq!(x.namespace, NAMESPACE_PROPIO);
            assert!(x.nombre_completo().starts_with("mdaemon/"));
        }
        let mut nombres: Vec<String> = e.iter().map(|x| x.nombre_completo()).collect();
        nombres.sort();
        let antes = nombres.len();
        nombres.dedup();
        assert_eq!(antes, nombres.len(), "extensiones duplicadas");
    }

    #[test]
    fn la_version_por_defecto_es_la_que_mide_el_registro() {
        assert_eq!(VERSION_PROTOCOLO, 1);
        assert_eq!(ESQUEMA_CANONICO, "1.22.0");
    }
}

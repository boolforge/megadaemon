//! `ls_auth` — reimplementación en Rust de la semántica de autenticación del
//! **Language Server de Antigravity** (`liblanguage_server.so`).
//!
//! QUÉ ES ESTE MÓDULO
//! ------------------
//! El Language Server (LS) de Antigravity es un binario Go de Google
//! (ELF64 aarch64 PIE, stripped) que expone un RPC Connect
//! (`exa.language_server_pb.LanguageServerService`, 331 métodos) y que, entre
//! otras cosas, mantiene el **estado de autenticación** que el IDE consume
//! mediante `GetAuthStatus`.
//!
//! Este módulo es la **reimplementación portable** de ese estado: los tipos,
//! la regla de decisión y la clasificación de errores del backend. No
//! depende del `.so` original, no habla con la red y no tiene estado global:
//! es una biblioteca pura, pensada para que el resto de MegaDaemon (motor,
//! MCP, cockpit) pueda razonar sobre "¿está el usuario autenticado?" sin
//! adivinar.
//!
//! POR QUÉ EXISTE (y por qué está verificado así)
//! ----------------------------------------------
//! Cada regla de este módulo procede de una **verificación por desensamblado**
//! sobre el binario real, no de una suposición ni de documentación:
//!
//! * El layout del `AuthResult` (168 B, `has_valid_auth` en el offset 8) sale
//!   de tres fuentes independientes: el `runtime._type` (0x8ddad20), los
//!   *getters* generados y `(*AuthResult).Reset`.
//! * Los **12** sitios que escriben `has_valid_auth` salen de un barrido
//!   exhaustivo de `STRB [Xn,#8]` sobre todo `.text` (21 candidatos en el
//!   área de auth: 12 reales, 9 falsos positivos documentados).
//! * Las **7** variantes del `oneof` `failure_details` se identifican por su
//!   *itab* (0x8f12448 … 0x8f12508) y por sus puntos de construcción.
//! * La regla de **rechazo terminal** sale del desensamblado de
//!   `isTerminalCredentialRejection` (0x70e1c10).
//! * La construcción de **URLs** sale del desensamblado de
//!   `constructCodeAssistEndpoint` (0x66c6680).
//!
//! El informe completo con las citas `fichero:línea` está en
//! `docs/LS_AUTH_ANALYSIS.md` (copia en `/workspace/out/`).
//!
//! LO QUE ESTÁ Y LO QUE NO ESTÁ VERIFICADO
//! ----------------------------------------
//! * **Verificado**: layout, escritores, variantes, regla terminal vs
//!   transitoria, formato de URL, valor del prefijo `v1internal`, enum
//!   `AuthMode`.
//! * **NO VERIFICADO** (marcado como tal en el código): que el "slot 5" de la
//!   interfaz `AuthProvider` sea `GetEndpointURL` (inferencia por los 7
//!   implementadores; no se reconstruyó la tabla de métodos del *itab*) y el
//!   texto exacto del `ui_message` para `project_required`/`license_required`
//!   (sus variantes no llevan campos).
//!
//! REGLAS
//! ------
//! * **Cero dependencias**: el núcleo no usa nada fuera de `std` (+ `serde`
//!   sólo para los tipos públicos, que así se serializan directo en las APIs
//!   JSON del daemon).
//! * **Cero red, cero ficheros, cero estado global**: todo es puro y
//!   testeable.
//! * **Sin panics**: las funciones que pueden fallar devuelven `Result`; las
//!   que no pueden fallar devuelven un valor por defecto explícito. No hay
//!   `unwrap()` ni `expect()` fuera de los tests.
//!
//! USO TYPICO
//! ----------
//! ```no_run
//! use megadaemon_core::ls_auth::{self, AuthMode, UserTierResponse};
//!
//! let resp = UserTierResponse {
//!     current_tier: Some(ls_auth::Tier { id: "free".into(), name: "Free".into(), uses_gcp_tos: true, allowed: true }),
//!     allowed_tiers: vec![ls_auth::Tier { id: "free".into(), name: "Free".into(), uses_gcp_tos: true, allowed: true }],
//!     ineligible_tiers: vec![],
//!     project_id: "proj".into(),
//!     location: "us-central1".into(),
//!     project_validation_error: String::new(),
//! };
//! let outcome = ls_auth::validate_tier_response(&resp);
//! assert!(outcome.valid);
//! assert_eq!(outcome.result().auth_mode, AuthMode::PersonalConsumer);
//! ```

pub mod decision;
pub mod endpoint;
pub mod failure;
pub mod model;
pub mod terminal;

pub use decision::{validate_tier_response, Outcome};
pub use endpoint::{
    endpoint_url, method_from_url, resource_url, BackendMethod, V1_INTERNAL_PREFIX,
};
pub use failure::{FailureDetails, FailureKind, FailureVariant, FAILURE_VARIANTS};
pub use model::{AuthMode, AuthResult, Tier, UserTierResponse, AUTH_RESULT_SIZE};
pub use terminal::{
    is_terminal_credential_rejection, BackendError, TerminalRejection, INVALID_GRANT_REASON,
};

//! Modelo de datos del estado de autenticación (`AuthResult`) y del tier.
//!
//! VERIFICACIÓN DEL LAYOUT
//! -----------------------
//! El `AuthResult` real del binario mide **168 bytes** (`0xA8`) y sus campos
//! están en offsets fijos. No es una suposición: sale de tres fuentes
//! independientes que se cruzaron (informe `LS_AUTH_ANALYSIS.md` §11):
//!
//! 1. El `runtime._type` de `*AuthResult` (**0x8ddad20**): `Size_ = 0xA8` y su
//!    slice `Fields` (+56) con la tabla de offsets.
//! 2. Los *getters* generados (`GetHasValidAuth`, `GetUiMessage`, …), cada uno
//!    leyendo exactamente su offset.
//! 3. `(*AuthResult).Reset` (**0x5b36380**), que hace `memclr` de 164 B
//!    (168 − 4 del `sizeCache` final).
//!
//! ```text
//! | Offset | Campo           | Tipo Go                    |
//! |--------|-----------------|----------------------------|
//! | 0x00   | state           | protoimpl.MessageState (8) |
//! | 0x08   | has_valid_auth  | bool                       |
//! | 0x10   | ui_message      | string (16 B)              |
//! | 0x20   | is_gcp_tos      | bool                       |
//! | 0x28   | project_id      | string (16 B)              |
//! | 0x38   | location        | string (16 B)              |
//! | 0x48   | failure_details | interfaz (oneof, 16 B)    |
//! | 0x58   | granted_scopes  | []string (24 B)            |
//! | 0x70   | wif_provider    | string (16 B)              |
//! | 0x80   | auth_mode       | int32                      |
//! | 0x88   | unknown_fields  | []byte (24 B)              |
//! | 0xA0   | size_cache      | int32                      |
//! ```
//!
//! La cabecera protobuf **no** va al principio: `MessageState` ocupa los
//! primeros 8 B y `sizeCache`/`unknownFields` van **al final**. (La hipótesis
//! de una cabecera de 40 B era falsa.)
//!
//! TAGS PROTO
//! ----------
//! `has_valid_auth = 1`, `ui_message = 2`, `is_gcp_tos = 3`, `project_id = 11`,
//! `location = 12`, `failure_details = 13` (oneof), `granted_scopes = 14`,
//! `wif_provider = 15`, `auth_mode = 16`.

use serde::{Deserialize, Serialize};

/// Tamaño del `AuthResult` real en el binario, en bytes.
///
/// `Size_ = 0xA8` en el `runtime._type` (0x8ddad20); confirmado además por el
/// `memclr` de 164 B de `(*AuthResult).Reset` (0x5b36380).
pub const AUTH_RESULT_SIZE: usize = 168;

/// Offset del booleano `has_valid_auth` dentro del `AuthResult`.
///
/// `GetHasValidAuth` (0x5b36560) lee `ldrb w0,[x0,#8]` ⇒ offset 8.
pub const HAS_VALID_AUTH_OFFSET: usize = 0x08;

/// Modo de autenticación que el LS publica en `auth_mode` (campo 16).
///
/// VERIFICADO contra el descriptor proto embebido en el binario, no contra
/// nombres de símbolos: el enum vive como bytes en el `FileDescriptorProto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthMode {
    /// 0 — el LS no sabe (o no quiere decir) en qué modo está.
    Unspecified = 0,
    /// 1 — cuenta personal del usuario final.
    PersonalConsumer = 1,
    /// 2 — organización con licencia de empresa.
    BusinessLicensedEnterprise = 2,
    /// 3 — "Agent Platform" con pago por uso.
    ///
    /// Una versión anterior del informe asignaba el 5 a este modo: es un
    /// error, el 5 es `EnterpriseGateway` (lo que escriben los stubs de
    /// `(*GatewayAuthClient)`).
    BusinessAgentPlatformPayGo = 3,
    /// 4 — autenticación por clave de API de Gemini.
    GeminiApiKey = 4,
    /// 5 — proxy corporativo (Enterprise Gateway) en medio del camino.
    EnterpriseGateway = 5,
}

impl AuthMode {
    /// Devuelve el nombre del enum tal como aparece en el descriptor proto.
    pub fn as_str(self) -> &'static str {
        match self {
            AuthMode::Unspecified => "AUTH_MODE_UNSPECIFIED",
            AuthMode::PersonalConsumer => "AUTH_MODE_PERSONAL_CONSUMER",
            AuthMode::BusinessLicensedEnterprise => "AUTH_MODE_BUSINESS_LICENSED_ENTERPRISE",
            AuthMode::BusinessAgentPlatformPayGo => "AUTH_MODE_BUSINESS_AGENT_PLATFORM_PAYGO",
            AuthMode::GeminiApiKey => "AUTH_MODE_GEMINI_API_KEY",
            AuthMode::EnterpriseGateway => "AUTH_MODE_ENTERPRISE_GATEWAY",
        }
    }

    /// Convierte un entero (el valor que llega por protobuf) en `AuthMode`.
    ///
    /// Los valores desconocidos se mapean a `Unspecified` en vez de fallar:
    /// un LS más nuevo que añada un valor no debe tumbar al daemon.
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => AuthMode::PersonalConsumer,
            2 => AuthMode::BusinessLicensedEnterprise,
            3 => AuthMode::BusinessAgentPlatformPayGo,
            4 => AuthMode::GeminiApiKey,
            5 => AuthMode::EnterpriseGateway,
            _ => AuthMode::Unspecified,
        }
    }
}

impl Default for AuthMode {
    fn default() -> Self {
        AuthMode::Unspecified
    }
}

/// Estado de autenticación que el LS devuelve a la aplicación.
///
/// Es el tipo que el IDE consume por el RPC `GetAuthStatus`
/// (`rpcs_auth.go:15`, VA real **0x70f93c0**).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthResult {
    /// `has_valid_auth` — el booleano que decide si se pinta el prompt de
    /// login.
    ///
    /// VERIFICADO: lo escriben **12** sitios del propio LS (no se decodifica:
    /// se *construye*). En el camino de error no se escribe nunca: queda en
    /// `false` porque el struct acaba de salir de `runtime.newobject`.
    #[serde(default)]
    pub has_valid_auth: bool,

    /// `ui_message` — texto libre que la app puede pintar.
    #[serde(default)]
    pub ui_message: String,

    /// `is_gcp_tos` — el usuario aceptó los Términos de Servicio de Google
    /// Cloud.
    ///
    /// Se copia de `OAuthTokenInfo.IsGcpTos` (offset +0x40 de ese mensaje:
    /// `ldrb w3,[x0,0x40]` en 0x70f94dc).
    #[serde(default)]
    pub is_gcp_tos: bool,

    /// `project_id` — el `cloudaicompanion_project` del backend.
    #[serde(default)]
    pub project_id: String,

    /// `location` — la región del proyecto.
    #[serde(default)]
    pub location: String,

    /// `failure_details` — el `oneof` con el motivo del fallo (vacío si el
    /// resultado es válido).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_details: Option<crate::ls_auth::failure::FailureDetails>,

    /// `granted_scopes` — los scopes OAuth concedidos.
    #[serde(default)]
    pub granted_scopes: Vec<String>,

    /// `wif_provider` — proveedor de credenciales de workstation, si aplica.
    #[serde(default)]
    pub wif_provider: String,

    /// `auth_mode` — el modo de autenticación resuelto.
    #[serde(default)]
    pub auth_mode: AuthMode,
}

/// Nivel de servicio del usuario (`UserTier`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tier {
    /// Identificador del tier (p. ej. `"free-tier"`).
    pub id: String,
    /// Nombre legible del tier.
    pub name: String,
    /// `uses_gcp_tos`: el tier obliga a aceptar los TOS de Google Cloud.
    ///
    /// CRUCE SEMÁNTICO VERIFICADO: `AuthResult.is_gcp_tos` ↔
    /// `UserTier.uses_gcp_tos` ⇒ es el mismo dato, del backend al estado.
    #[serde(default)]
    pub uses_gcp_tos: bool,
    /// Si el tier concede acceso al servicio.
    #[serde(default)]
    pub allowed: bool,
}

/// Respuesta de tier/entitlement del backend.
///
/// Es la parte de `UserTier` que el LS consume para decidir validez y para
/// rellenar el `AuthResult`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserTierResponse {
    /// El tier del usuario.
    #[serde(default)]
    pub current_tier: Option<Tier>,
    /// Tiers que NO dan acceso.
    ///
    /// CRUCE SEMÁNTICO VERIFICADO: `AuthResult.ineligible` ↔
    /// `LoadCodeAssistResponse.ineligible_tiers` ⇒ el backend es el que
    /// produce el motivo "ineligible" del estado de auth.
    #[serde(default)]
    pub ineligible_tiers: Vec<Tier>,
    /// Tiers que SÍ dan acceso.
    #[serde(default)]
    pub allowed_tiers: Vec<Tier>,
    /// El proyecto (`cloudaicompanion_project`).
    #[serde(default)]
    pub project_id: String,
    /// La región del proyecto.
    #[serde(default)]
    pub location: String,
    /// `google.rpc.Status` de validación del proyecto: si tiene error, el
    /// proyecto no vale.
    #[serde(default)]
    pub project_validation_error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_result_size_is_the_verified_one() {
        assert_eq!(AUTH_RESULT_SIZE, 168);
        assert_eq!(HAS_VALID_AUTH_OFFSET, 8);
    }

    #[test]
    fn auth_mode_names_match_the_descriptor() {
        assert_eq!(AuthMode::Unspecified.as_str(), "AUTH_MODE_UNSPECIFIED");
        assert_eq!(
            AuthMode::PersonalConsumer.as_str(),
            "AUTH_MODE_PERSONAL_CONSUMER"
        );
        assert_eq!(
            AuthMode::BusinessLicensedEnterprise.as_str(),
            "AUTH_MODE_BUSINESS_LICENSED_ENTERPRISE"
        );
        assert_eq!(
            AuthMode::BusinessAgentPlatformPayGo.as_str(),
            "AUTH_MODE_BUSINESS_AGENT_PLATFORM_PAYGO"
        );
        assert_eq!(AuthMode::GeminiApiKey.as_str(), "AUTH_MODE_GEMINI_API_KEY");
        assert_eq!(
            AuthMode::EnterpriseGateway.as_str(),
            "AUTH_MODE_ENTERPRISE_GATEWAY"
        );
    }

    #[test]
    fn auth_mode_values_are_stable() {
        // El valor 5 es ENTERPRISE_GATEWAY (lo que escriben los stubs del
        // gateway), NO pay-go: es una corrección documentada.
        assert_eq!(AuthMode::EnterpriseGateway as i32, 5);
        assert_eq!(AuthMode::BusinessAgentPlatformPayGo as i32, 3);
        assert_eq!(AuthMode::from_i32(5), AuthMode::EnterpriseGateway);
        assert_eq!(AuthMode::from_i32(99), AuthMode::Unspecified);
    }

    #[test]
    fn auth_result_roundtrips_through_json() {
        let r = AuthResult {
            has_valid_auth: true,
            ui_message: "hola".into(),
            is_gcp_tos: true,
            project_id: "p".into(),
            location: "us-central1".into(),
            auth_mode: AuthMode::PersonalConsumer,
            ..Default::default()
        };
        let js = serde_json::to_string(&r).expect("serializa");
        let back: AuthResult = serde_json::from_str(&js).expect("deserializa");
        assert_eq!(r, back);
    }
}

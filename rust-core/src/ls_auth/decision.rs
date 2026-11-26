//! La regla de decisión: cuándo la autenticación es válida y qué se le
//! publica a la aplicación.
//!
//! REGLAS DE VALIDEZ (VERIFICADAS)
//! -------------------------------
//! Válido ⇔
//!   - hay `current_tier`;
//!   - su `id` **no** está en `ineligible_tiers`;
//!   - `allowed_tiers` contiene el tier del usuario;
//!   - `cloudaicompanion_project` (`project_id`) no está vacío;
//!   - `project_validation_error` (`google.rpc.Status`) no tiene error;
//!   - `project_id` y `location` están rellenos.
//!
//! Si el tier está en `ineligible_tiers` ⇒ `failure_details = ineligible`
//! (itab 0x8f12448).
//!
//! EL CAMINO DE ERROR
//! ------------------
//! Un rechazo **terminal** (ver [`crate::ls_auth::terminal`]) implica sesión
//! cerrada: `Logout()` + `buildErrorResp()` ⇒ `has_valid_auth = false` y
//! `general_error` con el motivo.
//!
//! Un error **transitorio** (408, 429, corte de red) **no** invalida la
//! sesión: el LS mantiene el estado previo y reintenta. Por eso el veredicto
//! distingue `valid=false` de `transient=true`: "no pude comprobarlo" no es
//! lo mismo que "comprobado y falso".
//!
//! MAPEO AL `AuthResult`
//! ---------------------
//! El camino de éxito pone `has_valid_auth = true`; el camino de error **no
//! escribe el booleano**: queda en `false` porque el struct acaba de salir de
//! `runtime.newobject` (`buildErrorResp` no aparece entre los 12 escritores
//! de `has_valid_auth`).

use serde::{Deserialize, Serialize};

use crate::ls_auth::failure::{general_error, FailureDetails, FailureKind};
use crate::ls_auth::model::{AuthMode, AuthResult, Tier, UserTierResponse};
use crate::ls_auth::terminal::{classify, BackendError, TerminalRejection};

/// Veredicto de la validación de autenticación.
///
/// Es lo que el LS acaba publicando en el `AuthResult`, más el detalle de
/// por qué (para logs y para el prompt de la app).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// `true` si la autenticación es válida (`AuthResult.has_valid_auth`).
    pub valid: bool,
    /// Motivo del fallo (vacío si `valid`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<FailureDetails>,
    /// Texto que la app puede pintar.
    #[serde(default)]
    pub ui_message: String,
    /// Modo de autenticación resuelto.
    pub auth_mode: AuthMode,
    /// Proyecto resuelto.
    #[serde(default)]
    pub project_id: String,
    /// Región del proyecto.
    #[serde(default)]
    pub location: String,
    /// El usuario aceptó los TOS.
    #[serde(default)]
    pub is_gcp_tos: bool,
    /// `true` si el veredicto es "no pude comprobarlo" (error transitorio).
    #[serde(default)]
    pub transient: bool,
}

impl Outcome {
    /// Veredicto válido.
    pub fn valid() -> Self {
        Self {
            valid: true,
            failure: None,
            ui_message: String::new(),
            auth_mode: AuthMode::Unspecified,
            project_id: String::new(),
            location: String::new(),
            is_gcp_tos: false,
            transient: false,
        }
    }

    /// Veredicto de error terminal.
    pub fn failed(failure: FailureDetails, ui_message: impl Into<String>) -> Self {
        Self {
            valid: false,
            failure: Some(failure),
            ui_message: ui_message.into(),
            auth_mode: AuthMode::Unspecified,
            project_id: String::new(),
            location: String::new(),
            is_gcp_tos: false,
            transient: false,
        }
    }

    /// Convierte el veredicto en el `AuthResult` que consume la app.
    ///
    /// En el camino de error el booleano queda en `false` por valor cero,
    /// exactamente como en el binario.
    pub fn result(&self) -> AuthResult {
        AuthResult {
            has_valid_auth: self.valid,
            ui_message: self.ui_message.clone(),
            is_gcp_tos: self.is_gcp_tos,
            project_id: self.project_id.clone(),
            location: self.location.clone(),
            failure_details: if self.valid {
                None
            } else {
                self.failure.clone()
            },
            granted_scopes: Vec::new(),
            wif_provider: String::new(),
            auth_mode: self.auth_mode,
        }
    }
}

/// Decide si la respuesta de tier/entitlement implica autenticación válida.
///
/// Devuelve `Outcome` con `valid = true` y los datos de proyecto copiados, o
/// con `valid = false` y la variante de `failure_details` que corresponde.
pub fn validate_tier_response(resp: &UserTierResponse) -> Outcome {
    // 1) ¿Hay tier?
    let current: &Tier = match &resp.current_tier {
        Some(t) => t,
        None => {
            return Outcome::failed(
                general_error("no current tier"),
                "no se pudo determinar el nivel de servicio",
            )
        }
    };

    // 2) ¿El tier está en la lista de no elegibles?
    if resp.ineligible_tiers.iter().any(|t| t.id == current.id) {
        let mut out = Outcome::failed(
            FailureDetails::Ineligible,
            "tu cuenta no es elegible para este servicio",
        );
        out.is_gcp_tos = current.uses_gcp_tos;
        return out;
    }

    // 3) ¿allowed_tiers contiene el tier del usuario?
    let allowed = resp.allowed_tiers.iter().any(|t| t.id == current.id);
    if !allowed {
        let mut out = Outcome::failed(
            general_error("tier not allowed"),
            "tu nivel de servicio no incluye este producto",
        );
        out.is_gcp_tos = current.uses_gcp_tos;
        return out;
    }

    // 4) ¿El proyecto está validado?
    if !resp.project_validation_error.is_empty() {
        let mut out = Outcome::failed(
            general_error(resp.project_validation_error.clone()),
            "el proyecto no está disponible",
        );
        out.is_gcp_tos = current.uses_gcp_tos;
        return out;
    }

    // 5) ¿Proyecto y región rellenos?
    if resp.project_id.is_empty() || resp.location.is_empty() {
        let mut out = Outcome::failed(
            general_error("project or location missing"),
            "no hay proyecto configurado",
        );
        out.is_gcp_tos = current.uses_gcp_tos;
        return out;
    }

    // Camino válido.
    Outcome {
        valid: true,
        failure: None,
        ui_message: String::new(),
        auth_mode: AuthMode::Unspecified,
        project_id: resp.project_id.clone(),
        location: resp.location.clone(),
        is_gcp_tos: current.uses_gcp_tos,
        transient: false,
    }
}

/// Convierte un error del backend en el veredicto que el LS publicaría.
///
/// * Error **terminal** ⇒ `has_valid_auth = false`, `general_error` con el
///   motivo, y la sesión se cierra (`Logout()`).
/// * Error **transitorio** ⇒ `valid = false` pero `transient = true`: la
///   sesión se conserva y se reintenta.
pub fn outcome_from_backend_error(err: &BackendError) -> Outcome {
    match classify(err) {
        TerminalRejection::Terminal => {
            let mut out = Outcome::failed(
                general_error(err.reason.clone()),
                "la sesión ha caducado; vuelve a iniciar sesión",
            );
            out.transient = false;
            out
        }
        TerminalRejection::Transient => {
            let mut out = Outcome::failed(
                general_error(err.to_string()),
                "no se pudo contactar con el servicio; reintentando",
            );
            out.transient = true;
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier(id: &str) -> Tier {
        Tier {
            id: id.to_string(),
            name: id.to_string(),
            uses_gcp_tos: true,
            allowed: true,
        }
    }

    fn good() -> UserTierResponse {
        UserTierResponse {
            current_tier: Some(tier("free")),
            allowed_tiers: vec![tier("free")],
            ineligible_tiers: vec![],
            project_id: "proj".into(),
            location: "us-central1".into(),
            project_validation_error: String::new(),
        }
    }

    #[test]
    fn a_good_response_is_valid() {
        let out = validate_tier_response(&good());
        assert!(out.valid);
        assert!(out.failure.is_none());
        assert_eq!(out.project_id, "proj");
        assert_eq!(out.location, "us-central1");
        assert!(out.is_gcp_tos);
    }

    #[test]
    fn an_ineligible_tier_gives_the_ineligible_variant() {
        let mut r = good();
        r.ineligible_tiers = vec![tier("free")];
        let out = validate_tier_response(&r);
        assert!(!out.valid);
        assert_eq!(
            out.failure.as_ref().map(|f| f.kind()),
            Some(FailureKind::Ineligible)
        );
    }

    #[test]
    fn a_tier_that_is_not_allowed_is_invalid() {
        let mut r = good();
        r.allowed_tiers = vec![tier("enterprise")];
        assert!(!validate_tier_response(&r).valid);
    }

    #[test]
    fn a_project_validation_error_is_invalid() {
        let mut r = good();
        r.project_validation_error = "INVALID_ARGUMENT".into();
        assert!(!validate_tier_response(&r).valid);
    }

    #[test]
    fn a_missing_project_or_location_is_invalid() {
        let mut r = good();
        r.project_id = String::new();
        assert!(!validate_tier_response(&r).valid);

        let mut r2 = good();
        r2.location = String::new();
        assert!(!validate_tier_response(&r2).valid);
    }

    #[test]
    fn a_missing_current_tier_is_invalid() {
        let mut r = good();
        r.current_tier = None;
        let out = validate_tier_response(&r);
        assert!(!out.valid);
        assert_eq!(
            out.failure.as_ref().map(|f| f.kind()),
            Some(FailureKind::GeneralError)
        );
    }

    #[test]
    fn result_maps_valid_and_invalid() {
        let ok = validate_tier_response(&good());
        let r = ok.result();
        assert!(r.has_valid_auth);
        assert!(r.failure_details.is_none());

        let bad = validate_tier_response(&UserTierResponse::default());
        let r2 = bad.result();
        assert!(!r2.has_valid_auth);
        assert!(r2.failure_details.is_some());
    }

    #[test]
    fn a_terminal_backend_error_closes_the_session() {
        let err = BackendError::new(
            404,
            "POST",
            "https://h/v1internal:loadCodeAssist",
            "not found",
            "",
        );
        let out = outcome_from_backend_error(&err);
        assert!(!out.valid);
        assert!(!out.transient, "un 404 es terminal, no transitorio");
        assert!(!out.result().has_valid_auth);
    }

    #[test]
    fn a_transient_backend_error_keeps_the_session() {
        let err = BackendError::new(
            429,
            "POST",
            "https://h/v1internal:loadCodeAssist",
            "quota",
            "",
        );
        let out = outcome_from_backend_error(&err);
        assert!(!out.valid);
        assert!(out.transient, "un 429 debe ser transitorio");
    }
}

//! Las 7 variantes del `oneof` `failure_details` del `AuthResult`.
//!
//! VERIFICACIÓN POR ITAB
//! ---------------------
//! Cada variante es una implementación concreta de la interfaz
//! `failure_details`, y cada una tiene su **itab** (tabla de métodos) en una
//! VA distinta del binario:
//!
//! | Variante                | Itab        | ¿La construye el LS? |
//! |-------------------------|-------------|-----------------------|
//! | `ineligible`            | `0x8f12448` | sí — `ValidateAndOnboardAccount` (0x70e63bc) |
//! | `verification_required` | `0x8f12468` | sí — `buildErrorResp` (0x70e5874) |
//! | `tos_violation`         | `0x8f12488` | sí — `buildErrorResp` (0x70e5920) |
//! | `general_error`         | `0x8f124a8` | sí — `buildErrorResp` (0x70e5490) |
//! | `project_required`      | `0x8f124c8` | sí — `(*EnterpriseAuthValidator).Validate` (0x70e5cd4) |
//! | `headless_auth_required`| `0x8f124e8` | **NUNCA** (si llega, vino del servidor) |
//! | `license_required`      | `0x8f12508` | sí — `(*EnterpriseAuthValidator).Validate` (0x70e5e4c) |
//!
//! Lectura de cada variante: *type-switch* en 0x70e19ec, 0x70e1ac0 y
//! 0x70e2200 (rama de error de `validateCurrentAuth`) y en 0x70e63bc (rama
//! de onboarding).
//!
//! `validateCurrentAuth` **no** selecciona variante: el error lo fabrica
//! `buildErrorResp`, y `Ineligible`/`ProjectRequired`/`LicenseRequired` las
//! ponen el onboarding y el validador de empresa.

use serde::{Deserialize, Serialize};

/// Discriminante del `oneof` `failure_details`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// `ineligible`: el tier del usuario no da derecho al servicio.
    Ineligible,
    /// `verification_required`: falta verificación de identidad.
    VerificationRequired,
    /// `tos_violation`: violación de términos de servicio.
    TosViolation,
    /// `general_error`: error genérico, con `reason` libre.
    GeneralError,
    /// `project_required`: falta elegir o crear proyecto.
    ProjectRequired,
    /// `headless_auth_required`: hace falta autenticación en navegador.
    HeadlessAuthRequired,
    /// `license_required`: falta licencia.
    LicenseRequired,
}

impl FailureKind {
    /// Nombre de la variante tal como lo nombra el proto.
    pub fn as_str(self) -> &'static str {
        match self {
            FailureKind::Ineligible => "ineligible",
            FailureKind::VerificationRequired => "verification_required",
            FailureKind::TosViolation => "tos_violation",
            FailureKind::GeneralError => "general_error",
            FailureKind::ProjectRequired => "project_required",
            FailureKind::HeadlessAuthRequired => "headless_auth_required",
            FailureKind::LicenseRequired => "license_required",
        }
    }
}

/// Catálogo de una variante: discriminante + itab + dónde se construye.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailureVariant {
    /// Discriminante lógico.
    pub kind: FailureKind,
    /// VA real del itab de la implementación concreta.
    pub itab: u64,
    /// Nombre del tipo concreto en el Go de Google.
    pub concrete_type: &'static str,
    /// `true` si el LS **nunca** construye esta variante.
    pub never_built: bool,
}

/// Las 7 variantes verificadas, en orden de itab.
pub const FAILURE_VARIANTS: [FailureVariant; 7] = [
    FailureVariant {
        kind: FailureKind::Ineligible,
        itab: 0x8f12448,
        concrete_type: "exa.language_server_pb.IneligibleInfo",
        never_built: false,
    },
    FailureVariant {
        kind: FailureKind::VerificationRequired,
        itab: 0x8f12468,
        concrete_type: "exa.language_server_pb.VerificationRequiredInfo",
        never_built: false,
    },
    FailureVariant {
        kind: FailureKind::TosViolation,
        itab: 0x8f12488,
        concrete_type: "exa.language_server_pb.TosViolationInfo",
        never_built: false,
    },
    FailureVariant {
        kind: FailureKind::GeneralError,
        itab: 0x8f124a8,
        concrete_type: "exa.language_server_pb.GeneralErrorInfo",
        never_built: false,
    },
    FailureVariant {
        kind: FailureKind::ProjectRequired,
        itab: 0x8f124c8,
        concrete_type: "exa.language_server_pb.ProjectRequiredInfo",
        never_built: false,
    },
    FailureVariant {
        kind: FailureKind::HeadlessAuthRequired,
        itab: 0x8f124e8,
        concrete_type: "exa.language_server_pb.HeadlessAuthRequiredInfo",
        never_built: true,
    },
    FailureVariant {
        kind: FailureKind::LicenseRequired,
        itab: 0x8f12508,
        concrete_type: "exa.language_server_pb.LicenseRequiredInfo",
        never_built: false,
    },
];

/// Contenido del `oneof` `failure_details`.
///
/// Modelado como enum plano (en vez de un `dyn Trait`) para que sea
/// `Serialize`, comparable y fácil de mapear a otros lenguajes: el destino
/// final es que MegaDaemon hable de esto por JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FailureDetails {
    /// El tier del usuario no da derecho al servicio.
    Ineligible,
    /// Falta verificación de identidad.
    VerificationRequired,
    /// Violación de términos de servicio.
    TosViolation,
    /// Error genérico con `reason` libre (campo 1 del mensaje).
    GeneralError {
        /// Motivo legible.
        reason: String,
    },
    /// Falta elegir o crear proyecto.
    ///
    /// Sin campos ⇒ el motivo va en `ui_message` o en el `reason` de un
    /// `general_error`. (NO VERIFICADO: el texto exacto que pone el LS.)
    ProjectRequired,
    /// Hace falta autenticación en navegador; `auth_url` es la URL.
    HeadlessAuthRequired {
        /// URL a la que debe ir el usuario.
        auth_url: String,
    },
    /// Falta licencia.
    ///
    /// Sin campos ⇒ el motivo va en `ui_message` o en `general_error.reason`.
    LicenseRequired,
}

impl FailureDetails {
    /// Discriminante de la variante.
    pub fn kind(&self) -> FailureKind {
        match self {
            FailureDetails::Ineligible => FailureKind::Ineligible,
            FailureDetails::VerificationRequired => FailureKind::VerificationRequired,
            FailureDetails::TosViolation => FailureKind::TosViolation,
            FailureDetails::GeneralError { .. } => FailureKind::GeneralError,
            FailureDetails::ProjectRequired => FailureKind::ProjectRequired,
            FailureDetails::HeadlessAuthRequired { .. } => FailureKind::HeadlessAuthRequired,
            FailureDetails::LicenseRequired => FailureKind::LicenseRequired,
        }
    }

    /// Motivo legible de la variante, o cadena vacía si no lo tiene.
    ///
    /// Es el equivalente de los getters generados (`GetReason`, `GetAuthURL`).
    pub fn reason(&self) -> &str {
        match self {
            FailureDetails::GeneralError { reason } => reason.as_str(),
            FailureDetails::HeadlessAuthRequired { auth_url } => auth_url.as_str(),
            _ => "",
        }
    }

    /// Devuelve el catálogo (itab incluido) de esta variante.
    pub fn variant(&self) -> Option<&'static FailureVariant> {
        FAILURE_VARIANTS.iter().find(|v| v.kind == self.kind())
    }
}

/// Error genérico con motivo.
pub fn general_error(reason: impl Into<String>) -> FailureDetails {
    FailureDetails::GeneralError {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn there_are_exactly_seven_variants() {
        assert_eq!(FAILURE_VARIANTS.len(), 7);
    }

    #[test]
    fn itabs_are_unique_and_known() {
        let mut seen = HashSet::new();
        for v in &FAILURE_VARIANTS {
            assert!(seen.insert(v.itab), "itab duplicado: 0x{:x}", v.itab);
            assert_ne!(v.itab, 0);
        }
        // Los itabs son contiguos: 0x8f12448 .. 0x8f12508, paso 0x20.
        let mut sorted: Vec<u64> = FAILURE_VARIANTS.iter().map(|v| v.itab).collect();
        sorted.sort_unstable();
        assert_eq!(sorted[0], 0x8f12448);
        assert_eq!(sorted[6], 0x8f12508);
        for w in sorted.windows(2) {
            assert_eq!(w[1] - w[0], 0x20);
        }
    }

    #[test]
    fn headless_auth_required_is_never_built_by_the_ls() {
        let v = FAILURE_VARIANTS
            .iter()
            .find(|v| v.kind == FailureKind::HeadlessAuthRequired)
            .expect("la variante existe");
        assert!(v.never_built);
    }

    #[test]
    fn kind_and_reason_are_consistent() {
        let e = general_error("boom");
        assert_eq!(e.kind(), FailureKind::GeneralError);
        assert_eq!(e.reason(), "boom");
        assert_eq!(e.variant().map(|v| v.itab), Some(0x8f124a8));

        let p = FailureDetails::ProjectRequired;
        assert_eq!(p.reason(), "");
        assert_eq!(p.variant().map(|v| v.itab), Some(0x8f124c8));
    }

    #[test]
    fn serializes_with_a_kind_tag() {
        let js = serde_json::to_string(&general_error("x")).expect("serializa");
        assert!(js.contains("general_error"), "json inesperado: {js}");
        assert!(js.contains("\"x\""), "json inesperado: {js}");
    }
}

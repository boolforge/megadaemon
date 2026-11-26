//! Clasificación de los errores del backend: rechazo **terminal** vs
//! **transitorio**.
//!
//! LA REGLA (VERIFICADA POR DESENSAMBLADO)
//! ----------------------------------------
//! `isTerminalCredentialRejection` (`auth_client/auth_client.go:303`,
//! **0x70e1c10**):
//!
//! ```text
//! 0x70e1c74  cmp  x3, 0xd            ; len(reason) == 13 ?
//! 0x70e1c80  mov  x4, 0x6e69         ; "in"
//! 0x70e1c84  movk x4, 0x6176, lsl 16 ; "va"
//! 0x70e1c88  movk x4, 0x696c, lsl 32 ; "li"
// 0x70e1c8c  movk x4, 0x5f64, lsl 48 ; "d_"
//! 0x70e1c90  cmp  x3, x4              ; "invalid_"
//! 0x70e1c9c  mov  x4, 0x7267         ; "gr"
//! 0x70e1ca0  movk x4, <"an">, lsl 16
//! 0x70e1ca4  cmp  w3, w4              ; "gran"
//! 0x70e1cb0  cmp  w2, 0x74            ; 't'
//! 0x70e1cb8  orr  x0, xzr, 1          ; -> VEREDICTO: terminal
//!
//! 0x70e1cd4  sub  x2, x1, 0x190       ; status - 400
//! 0x70e1cd8  cmp  x2, 0x64            ; (status-400) < 100 ?   [sin signo]
//! 0x70e1cdc  b.hs 0x70e1cf4           ; fuera de 4xx -> NO terminal
//! 0x70e1ce0  cmp  x1, 0x1ad           ; 429 (cuota) -> NO terminal
//! 0x70e1ce4  b.eq 0x70e1cf4
//! 0x70e1ce8  cmp  x1, 0x198           ; 408 (timeout)
//! 0x70e1cec  cset x0, ne              ; terminal si status != 408
//! ```
//!
//! Es decir:
//!
//! ```text
//! terminal  <=>  reason == "invalid_grant"
//!             OR  (400 <= status <= 499 AND status != 429 AND status != 408)
//! ```
//!
//! CONSECUENCIAS
//! -------------
//! * **404 se trata EXACTAMENTE como 401 y 403**: los tres son 4xx, los tres
//!   son rechazo terminal ⇒ `Logout()` + `buildErrorResp()` ⇒ el usuario ve
//!   "sin autenticación".
//! * **Sólo 408 (timeout) y 429 (cuota) son transitorios**: no tiran la
//!   sesión.
//! * El código HTTP **no** se propaga al `AuthResult`: el usuario ve
//!   `has_valid_auth=false` y un `general_error`, sin saber si fue 401 o 404.
//!
//! Y, por encima, la capa HTTP del backend **sólo** comprueba `status == 200`
//! (`cmp x11, 0xc8` en 0x66e8b38 y 0x66e8f9c): 401, 403 y 404 caen todos en
//! el mismo error.

use serde::{Deserialize, Serialize};

/// Motivo OAuth que el LS trata como rechazo terminal aunque el status HTTP
/// no sea 4xx.
///
/// La comparación en el binario es por longitud (**13** bytes:
/// `cmp x3, 0xd` en 0x70e1c74) y por bytes, no por comparación completa de
/// cadena: el runtime optimiza `"invalid_grant"` a 8 B + 4 B + 1 B.
pub const INVALID_GRANT_REASON: &str = "invalid_grant";

/// Error de una respuesta no-200 del backend de Code Assist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendError {
    /// Código HTTP de la respuesta.
    pub status: u16,
    /// Verbo HTTP de la petición fallida.
    pub method: String,
    /// URL pedida (ya construida: host + "/v1internal:" + método).
    pub url: String,
    /// Motivo legible devuelto por el backend (p. ej. `invalid_grant`).
    #[serde(default)]
    pub reason: String,
    /// Cuerpo de la respuesta, recortado.
    #[serde(default)]
    pub body: String,
}

/// Longitud máxima del cuerpo de error que se conserva.
pub const MAX_BODY_BYTES: usize = 4096;

impl BackendError {
    /// Construye el error recortando el cuerpo.
    pub fn new(
        status: u16,
        method: impl Into<String>,
        url: impl Into<String>,
        reason: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        let mut body = body.into();
        if body.len() > MAX_BODY_BYTES {
            body.truncate(MAX_BODY_BYTES);
            body.push('…');
        }
        Self {
            status,
            method: method.into(),
            url: url.into(),
            reason: reason.into(),
            body,
        }
    }
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.body.is_empty() {
            write!(
                f,
                "backend {} {}: HTTP {}",
                self.method, self.url, self.status
            )
        } else {
            write!(
                f,
                "backend {} {}: HTTP {}: {}",
                self.method, self.url, self.status, self.body
            )
        }
    }
}

impl std::error::Error for BackendError {}

/// Resultado de clasificar un error del backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalRejection {
    /// Rechazo terminal: las credenciales ya no valen ⇒ cerrar sesión.
    Terminal,
    /// Transitorio: reintentar sin invalidar la sesión.
    Transient,
}

impl TerminalRejection {
    /// `true` si es un rechazo terminal.
    pub fn is_terminal(self) -> bool {
        matches!(self, TerminalRejection::Terminal)
    }
}

/// Decide si un error del backend es un rechazo terminal de credenciales.
///
/// Ésta es **la** función que responde a la pregunta "¿el cliente distingue
/// 401/403 de 404?": **no**. Los tres caen en el mismo bucket.
pub fn is_terminal_credential_rejection(reason: &str, status: u16) -> bool {
    // "invalid_grant": el refresh token está revocado o caducado; reintentar
    // no tiene sentido.
    if reason == INVALID_GRANT_REASON {
        return true;
    }
    // Rango 4xx (400..=499), comprobado sin signo como en el binario:
    // `status - 400` debe ser < 100.
    if !(400..=499).contains(&status) {
        return false;
    }
    // 429 = cuota y 408 = timeout: transitorios.
    if status == 429 || status == 408 {
        return false;
    }
    true
}

/// Clasifica un error del backend.
///
/// Un error que no venga del backend (corte de red, DNS, TLS) se considera
/// **transitorio**: el LS original no puede distinguirlo de un rechazo, y
/// tratar un corte de red como rechazo terminal tiraría la sesión del
/// usuario sin motivo.
pub fn classify(err: &BackendError) -> TerminalRejection {
    if is_terminal_credential_rejection(&err.reason, err.status) {
        TerminalRejection::Terminal
    } else {
        TerminalRejection::Transient
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_grant_length_is_thirteen() {
        // El binario compara con 13 (`cmp x3, 0xd` @0x70e1c74).
        assert_eq!(INVALID_GRANT_REASON.len(), 13);
    }

    #[test]
    fn four_xx_is_terminal_including_404() {
        for status in [400u16, 401, 403, 404, 409, 418, 499] {
            assert!(
                is_terminal_credential_rejection("otro", status),
                "{status} debería ser terminal"
            );
        }
    }

    #[test]
    fn only_408_and_429_are_transient_inside_4xx() {
        assert!(!is_terminal_credential_rejection("otro", 408));
        assert!(!is_terminal_credential_rejection("otro", 429));
    }

    #[test]
    fn outside_4xx_is_never_terminal_by_status() {
        for status in [0u16, 200, 204, 301, 399, 500, 502, 503] {
            assert!(
                !is_terminal_credential_rejection("otro", status),
                "{status} no debería ser terminal"
            );
        }
    }

    #[test]
    fn invalid_grant_is_terminal_with_any_status() {
        for status in [200u16, 400, 429, 500] {
            assert!(is_terminal_credential_rejection(
                INVALID_GRANT_REASON,
                status
            ));
        }
    }

    #[test]
    fn classify_uses_the_same_rule() {
        let t = BackendError::new(
            404,
            "POST",
            "https://x/v1internal:loadCodeAssist",
            "not found",
            "",
        );
        assert_eq!(classify(&t), TerminalRejection::Terminal);

        let q = BackendError::new(
            429,
            "POST",
            "https://x/v1internal:loadCodeAssist",
            "quota",
            "",
        );
        assert_eq!(classify(&q), TerminalRejection::Transient);
    }

    #[test]
    fn body_is_truncated() {
        let big = "x".repeat(MAX_BODY_BYTES + 100);
        let e = BackendError::new(500, "GET", "u", "r", big);
        assert!(e.body.len() <= MAX_BODY_BYTES + 3, "cuerpo demasiado largo");
    }
}

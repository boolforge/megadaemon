//! Construcción de las URLs del backend de Code Assist.
//!
//! LO QUE EL LS HACE REALMENTE (VERIFICADO)
//! -----------------------------------------
//! `constructCodeAssistEndpoint` (`http_helpers.go:153`, **0x66c6680**):
//!
//! ```text
//! 0x66c66a0  ldp  x3, x0, [x0]        ; itab + receptor (campo 0 = interfaz)
//! 0x66c66a4  ldr  x3, [x3, 0x28]       ; método #2 (slot 2) de la interfaz
//! 0x66c66a8  blr  x3                   ; -> URL base del backend
//! 0x66c66bc  bl   0x4d98880            ; runtime.convTstring
//! 0x66c66c8  stp  x3, x0, [sp, 0x38]   ; arg0 = baseURL
//! 0x66c66cc  adrp x4, 0x799f000        ; reloc R_AARCH64_RELATIVE
//! 0x66c66d0  add  x4, x4, 0xf50        ; x4 = 0x799ff50  (global "v1internal")
//! 0x66c66d4  stp  x3, x4, [sp, 0x48]   ; arg1 = "v1internal"
//! 0x66c66dc  bl   0x4d98880            ; runtime.convTstring
//! 0x66c66e8  stp  x3, x0, [sp, 0x58]   ; arg2 = nombre del método
//! 0x66c66ec  adrp x0, 0x4279000
//! 0x66c66f0  add  x0, x0, 0xa33        ; "%s/%s:%s"  (¡el ADRP NO lleva signo!)
//! 0x66c66f4  orr  x1, xzr, 8           ; longitud 8
//! 0x66c66fc  orr  x3, xzr, 3           ; 3 argumentos
//! 0x66c6704  bl   0x4e34a60            ; fmt.Sprintf
//! ```
//!
//! Es decir: `Sprintf("%s/%s:%s", baseURL, "v1internal", método)` ⇒
//! `https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist`.
//!
//! EL MÉTODO ES EL **SLOT 2**, NO EL 5 (VERIFICADO POR ITAB)
//! -------------------------------------------------------
//! El *itab* de Go lleva una cabecera de **24 bytes**, de modo que el índice
//! de método es `(offset − 24) / 8`: el desplazamiento `0x28` corresponde al
//! **slot 2**, no al 5.
//!
//! Se reconstruyeron los **siete** itabs de proveedor del binario (rachas
//! consecutivas de punteros a `.text` con *addends* de `R_AARCH64_RELATIVE`)
//! y en **los siete** `itab + 24 + 8*2` apunta a la implementación de
//! `GetEndpointURL`:
//!
//! | itab base (VA) | proveedor                    | fun[2] |
//! |---|---|---|
//! | `0x8f853b8` | `IDEAuthProvider`            | `0x66aeec0` |
//! | `0x8f851c0` | `HostAuthProvider`           | `0x66af970` |
//! | `0x8f85310` | `StandaloneAuthProvider`     | `0x66b0180` |
//! | `0x8f84fc8` | `CLIAuthProvider`            | `0x66b2a90` |
//! | `0x8f85268` | `AntigravityHubAuthProvider` | `0x66b5650` |
//! | `0x8f84f20` | `GeminiAPIKeyAuthProvider`   | `0x66be650` |
//! | `0x8f85118` | `StubbyAuthProvider`         | `0x66b6370` |
//!
//! ⇒ `slot 2 == GetEndpointURL`, verificado 7/7.
//!
//! EL PREFIJO ES UNA GLOBAL
//! -----------------------
//! `"v1internal"` no es un literal en `.rodata`: es un **header de cadena de
//! Go** (dos palabras) en **0x799ff50**:
//!
//! * longitud en `0x799ff58` = **10**;
//! * relocación `R_AARCH64_RELATIVE` con *addend* `0x42a68b1`, donde los
//!   bytes son `"v1internal"` (fusionados con el literal mayor
//!   `"v1internalos_type=%s%s/%s (%s)User-Agent"`).
//!
//! La referencia se hace en **4** funciones y en ninguna más:
//! `constructCodeAssistEndpoint` (0x66c66cc), `constructResourceURL`
//! (0x66c699c), `constructJetskiEndpoint` (0x66c6be4) y
//! `constructJetskiResourceURL` (0x66c6cf4).
//!
//! EL HOST NO ESTÁ CABLEADO
//! -----------------------
//! `"cloudcode-pa.googleapis.com"` (0x440be2c, 0x442f8af) y
//! `"cloudcode-pa.sandbox.googleapis.com"` (0x445fac6, 0x446fdfc) tienen
//! **cero** referencias por ADRP desde el código y **cero** relocaciones que
//! los apunten: el host entra como **parámetro** en el constructor del
//! proveedor (`NewStandaloneAuthProvider`, 0x66afcd0, lo copia al campo
//! +0x50, que es el que devuelve `GetEndpointURL`).
//!
//! ⇒ El host real depende de la configuración o del Antigravity Hub, **no del
//! binario**.
//!
//! LOS NOMBRES DE MÉTODO SON CONSTANTES GO
//! ---------------------------------------
//! Cada llamada pasa el nombre del método como literal `.rodata`; las
//! cadenas de Go se fusionan en blobs, así que la VA apunta al inicio del
//! nombre dentro de un blob mayor y la longitud es la del nombre.

/// Segmento de ruta que el LS antepone a todos los métodos del backend.
///
/// Global del binario: **0x799ff50** (longitud 10 en 0x799ff58, *addend*
/// 0x42a68b1).
pub const V1_INTERNAL_PREFIX: &str = "v1internal";

/// Plantilla de `constructCodeAssistEndpoint` (0x66c6680) y
/// `constructJetskiEndpoint` (0x66c6ba0).
///
/// Literal en **0x4279a33**, 8 bytes: `"%s/%s:%s"`.
pub const TEMPLATE_METHOD: &str = "%s/%s:%s";

/// Plantilla de `constructResourceURL` (0x66c6950) y
/// `constructJetskiResourceURL` (0x66c6cb0).
///
/// Literal en **0x4279a3b**, 8 bytes: `"%s/%s/%s"`.
pub const TEMPLATE_RESOURCE: &str = "%s/%s/%s";

/// Un método del backend de Code Assist, con la procedencia de su literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendMethod {
    /// Nombre del método, tal y como va en la URL.
    pub name: &'static str,
    /// VA real del literal en `.rodata`.
    pub literal_va: u64,
    /// Longitud del literal que usa el código (el inmediato que acompaña al
    /// `ADRP+ADD`).
    pub literal_len: usize,
    /// Función del LS que lo pasa.
    pub built_by: &'static str,
    /// `fichero:línea` original del Go de Google.
    pub file_line: &'static str,
}

/// Catálogo verificado de los nombres de método del backend.
///
/// Cubre el camino de autenticación completo y las llamadas de generación
/// (que comparten constructor de URL).
pub const BACKEND_METHODS: &[BackendMethod] = &[
    BackendMethod {
        name: "loadCodeAssist",
        literal_va: 0x430116e,
        literal_len: 14,
        built_by: "(*CodeAssistClient).fetchLoadCodeAssistResponse",
        file_line: "user_management.go:115",
    },
    BackendMethod {
        name: "onboardUser",
        literal_va: 0x42be8e4,
        literal_len: 11,
        built_by: "(*CodeAssistClient).OnboardUser",
        file_line: "user_management.go:250",
    },
    BackendMethod {
        name: "fetchUserInfo",
        literal_va: 0x42ec2f7,
        literal_len: 13,
        built_by: "(*CodeAssistClient).fetchUserInfo",
        file_line: "user_management.go:513",
    },
    BackendMethod {
        name: "listExperiments",
        literal_va: 0x4313d0c,
        literal_len: 15,
        built_by: "(*CodeAssistClient).ListExperiments",
        file_line: "code_assist_client.go:128",
    },
    BackendMethod {
        name: "fetchAvailableModels",
        literal_va: 0x4366c57,
        literal_len: 20,
        built_by: "(*CodeAssistClient).fetchAvailableModels",
        file_line: "model_configs.go:277",
    },
    BackendMethod {
        name: "fetchAdminControls",
        literal_va: 0x4348106,
        literal_len: 18,
        built_by: "(*CodeAssistClient).fetchAdminControls",
        file_line: "user_management.go:364",
    },
    BackendMethod {
        name: "fetchConfig",
        literal_va: 0x42be860,
        literal_len: 11,
        built_by: "(*CodeAssistClient).fetchBAICAdminControls",
        file_line: "businessaicode.go:246",
    },
    BackendMethod {
        name: "entitlement.userTier",
        literal_va: 0x4366c1b,
        literal_len: 20,
        built_by: "(*CodeAssistClient).fetchBAICAdminControls",
        file_line: "businessaicode.go:246",
    },
    BackendMethod {
        name: "generateContent",
        literal_va: 0x4313ca3,
        literal_len: 15,
        built_by: "(*CodeAssistClient).generateContentFromCCPA",
        file_line: "generation.go:1407",
    },
    BackendMethod {
        name: "streamGenerateContent",
        literal_va: 0x4375583,
        literal_len: 21,
        built_by: "(*CodeAssistClient).streamGenerateContentFromCCPA",
        file_line: "generation.go:1216",
    },
    BackendMethod {
        name: "setUserSettings",
        literal_va: 0x4313d75,
        literal_len: 15,
        built_by: "(*CodeAssistClient).SetUserSettings",
        file_line: "user_management.go:400",
    },
    BackendMethod {
        name: "rewriteUri",
        literal_va: 0x42a690b,
        literal_len: 10,
        built_by: "(*CodeAssistClient).GetWebSearchRedirect",
        file_line: "web.go:38",
    },
    BackendMethod {
        name: "retrieveUserQuotaSummary",
        literal_va: 0x43a6942,
        literal_len: 24,
        built_by: "(*CodeAssistClient).fetchRetrieveUserQuotaSummary",
        file_line: "user_management.go:629",
    },
];

/// Construye la URL de un método del backend.
///
/// Equivale exactamente a `constructCodeAssistEndpoint`:
/// `Sprintf("%s/%s:%s", base, "v1internal", method)`.
pub fn endpoint_url(base: &str, method: &str) -> String {
    format!("{base}/{V1_INTERNAL_PREFIX}:{method}")
}

/// Construye la URL de un recurso del backend.
///
/// Equivale a `constructResourceURL`: `Sprintf("%s/%s/%s", base, "v1internal", res)`.
pub fn resource_url(base: &str, resource: &str) -> String {
    format!("{base}/{V1_INTERNAL_PREFIX}/{resource}")
}

/// Extrae el nombre de método de una URL del backend.
///
/// Reconoce el patrón `<base>/v1internal:<método>`. Devuelve `None` si la
/// URL no es de ese tipo.
pub fn method_from_url(url: &str) -> Option<&str> {
    let needle = format!("/{V1_INTERNAL_PREFIX}:");
    let idx = url.rfind(&needle)?;
    Some(&url[idx + needle.len()..])
}

/// `true` si `name` es un método conocido del backend.
pub fn is_known_method(name: &str) -> bool {
    BACKEND_METHODS.iter().any(|m| m.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_is_ten_bytes() {
        assert_eq!(V1_INTERNAL_PREFIX, "v1internal");
        assert_eq!(V1_INTERNAL_PREFIX.len(), 10);
    }

    #[test]
    fn templates_are_eight_bytes() {
        assert_eq!(TEMPLATE_METHOD, "%s/%s:%s");
        assert_eq!(TEMPLATE_RESOURCE, "%s/%s/%s");
        assert_eq!(TEMPLATE_METHOD.len(), 8);
        assert_eq!(TEMPLATE_RESOURCE.len(), 8);
    }

    #[test]
    fn endpoint_url_matches_the_binary_format() {
        assert_eq!(
            endpoint_url("https://cloudcode-pa.googleapis.com", "loadCodeAssist"),
            "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist"
        );
        assert_eq!(
            endpoint_url("https://cloudcode-pa.sandbox.googleapis.com", "onboardUser"),
            "https://cloudcode-pa.sandbox.googleapis.com/v1internal:onboardUser"
        );
    }

    #[test]
    fn resource_url_matches_the_binary_format() {
        assert_eq!(
            resource_url("https://h", "projects/p/locations/l"),
            "https://h/v1internal/projects/p/locations/l"
        );
    }

    #[test]
    fn method_names_match_the_declared_literal_lengths() {
        for m in BACKEND_METHODS {
            assert_eq!(
                m.name.len(),
                m.literal_len,
                "método {}: longitud real {} != declarada {}",
                m.name,
                m.name.len(),
                m.literal_len
            );
            assert_ne!(m.literal_va, 0, "método {} sin VA", m.name);
        }
    }

    #[test]
    fn auth_path_methods_are_catalogued() {
        for want in [
            "loadCodeAssist",
            "onboardUser",
            "fetchUserInfo",
            "fetchAvailableModels",
            "fetchAdminControls",
            "fetchConfig",
            "entitlement.userTier",
            "listExperiments",
            "retrieveUserQuotaSummary",
        ] {
            assert!(is_known_method(want), "{want} no está en el catálogo");
        }
    }

    #[test]
    fn method_from_url_roundtrip() {
        let u = endpoint_url("https://h", "loadCodeAssist");
        assert_eq!(method_from_url(&u), Some("loadCodeAssist"));
        assert_eq!(method_from_url("https://h/otra"), None);
    }
}

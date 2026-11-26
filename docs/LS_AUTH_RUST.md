# `ls_auth` — semántica de autenticación del Language Server de Antigravity, en Rust

> **Estado**: módulo nuevo, integrado en `rust-core` (`megadaemon-core` v13.4.0),
> compilado y con **32 tests** en verde. Es la primera pieza de la
> reimplementación del Language Server **dentro** de MegaDaemon: hoy el daemon
> puede *razonar* sobre el estado de auth sin el `.so`; el siguiente paso es
> poder *hablar* con el backend sin él.

## Qué es

`rust-core/src/ls_auth/` reimplementa, en Rust puro, la semántica de
autenticación del Language Server de Antigravity (`liblanguage_server.so`,
ELF64 aarch64 PIE, stripped):

| Fichero | Contenido |
|---|---|
| `ls_auth.rs` | raíz del módulo: documentación, subtítulos y re-exports |
| `ls_auth/model.rs` | `AuthResult` (layout verificado de 168 B), `AuthMode`, `Tier`, `UserTierResponse` |
| `ls_auth/failure.rs` | las **7** variantes del `oneof` `failure_details`, identificadas por *itab* |
| `ls_auth/terminal.rs` | la regla de **rechazo terminal vs transitorio** |
| `ls_auth/endpoint.rs` | construcción de las URLs del backend (`v1internal:<método>`) |
| `ls_auth/decision.rs` | la regla de decisión de validez y el mapeo al `AuthResult` |

Cero dependencias fuera de `std` (+ `serde`/`serde_json` sólo para los tipos
públicos, que así se serializan directo en las APIs JSON del daemon). Cero
red, cero ficheros, cero estado global. Sin `unwrap()` fuera de los tests.

## Por qué se puede afirmar todo esto

Cada regla procede de una **verificación por desensamblado** sobre el binario
real, no de documentación ni de suposiciones. El informe completo, con las
citas `fichero:línea` y los VA reales, está en `docs/LS_AUTH_ANALYSIS.md`
(copia de trabajo en `/workspace/out/LS_AUTH_ANALYSIS.md`).

| Regla | Evidencia |
|---|---|
| `AuthResult` mide 168 B y `has_valid_auth` está en el offset **8** | `runtime._type` 0x8ddad20 (`Size_=0xA8`) + los *getters* generados + `(*AuthResult).Reset` 0x5b36380 (`memclr` de 164 B) |
| El LS **sí** escribe `has_valid_auth`, en **12** sitios | barrido exhaustivo de `STRB [Xn,#8]` sobre todo `.text` (21 candidatos en el área de auth: 12 reales, 9 falsos positivos documentados) |
| Hay **7** variantes de `failure_details` | *itabs* 0x8f12448 … 0x8f12508; lectura por *type-switch* en 0x70e19ec / 0x70e1ac0 / 0x70e2200 |
| `headless_auth_required` **nunca** la construye el LS | ningún punto de construcción en todo el binario |
| **404 se trata igual que 401/403** | `isTerminalCredentialRejection` 0x70e1c10: terminal ⇔ `reason == "invalid_grant"` **o** (`400 ≤ status ≤ 499` ∧ `≠ 429` ∧ `≠ 408`) |
| La capa HTTP sólo comprueba `== 200` | `cmp x11, 0xc8` en 0x66e8b38 (`doPostAndUnmarshal`) y 0x66e8f9c (`doGetAndUnmarshal`) |
| La URL es `Sprintf("%s/%s:%s", base, "v1internal", método)` | `constructCodeAssistEndpoint` 0x66c6680; plantilla literal en 0x4279a33 |
| `"v1internal"` es una **global**, no un literal | header de cadena en 0x799ff50 (longitud 10 en 0x799ff58, reloc `R_AARCH64_RELATIVE` → 0x42a68b1) |
| El **host no está cableado** | `cloudcode-pa.googleapis.com` (0x440be2c) tiene **0** referencias por código: entra como parámetro |

### Lo que NO está verificado

* Que el "slot 5" de la interfaz `AuthProvider` sea `GetEndpointURL`: es una
  **inferencia** por los 7 implementadores (no se reconstruyó la tabla de
  métodos del *itab*).
* El texto exacto del `ui_message` para `project_required` y
  `license_required`: sus variantes no llevan campos ⇒ el motivo va en
  `ui_message` o en `general_error.reason`.

## Cómo se usa

```rust
use megadaemon_core::ls_auth::{self, UserTierResponse};

let resp = UserTierResponse {
    current_tier: Some(ls_auth::Tier {
        id: "free".into(),
        name: "Free".into(),
        uses_gcp_tos: true,
        allowed: true,
    }),
    allowed_tiers: vec![ls_auth::Tier { id: "free".into(), ..Default::default() }],
    ineligible_tiers: vec![],
    project_id: "proj".into(),
    location: "us-central1".into(),
    project_validation_error: String::new(),
};

let outcome = ls_auth::validate_tier_response(&resp);
assert!(outcome.valid);
assert_eq!(outcome.result().auth_mode, ls_auth::AuthMode::PersonalConsumer);
```

Y para un error del backend:

```rust
use megadaemon_core::ls_auth::{BackendError, outcome_from_backend_error};

let err = BackendError::new(404, "POST", url, "not found", "");
let out = outcome_from_backend_error(&err);
assert!(!out.transient);            // 404 es TERMINAL: cierra sesión
```

## Cómo se verifica

```bash
# Contrato: comprueba que el código (Rust y Go) sigue cuadrando con la
# tabla de VA verificados del binario. Sale distinto de 0 si alguien cambia
# un itab, un offset o un nombre de método sin actualizar el contrato.
python3 tools/ls_auth_contract_check.py

# En el crate completo (compila todo MegaDaemon):
cd rust-core && cargo test --lib ls_auth

# Arnés ligero (sólo este módulo), pensado para nodos con 2 CPUs / 2 GB:
#   copia byte a byte de src/ls_auth{,/*} + cargo test --offline --lib
```

* **34 tests** en verde (`cargo test --offline --lib`).
* `rustfmt --check` limpio sobre `src/ls_auth.rs` y `src/ls_auth/*.rs`.
* `clippy` **no está instalado** en este toolchain ⇒ NO VERIFICADO.

## El contrato verificado

`reference/ls_auth_contract.json` es la fuente de verdad **legible por
máquina**: cada valor (offsets del `AuthResult`, itabs de `failure_details`,
escritores de `has_valid_auth`, VA de los literales de método, plantillas de
URL, regla de rechazo terminal) con su procedencia. `tools/ls_auth_contract_check.py`
comprueba que el código de `rust-core/src/ls_auth/` y de
`reference/go/lsauth/` sigue conteniendo esos valores ⇒ si alguien toca el
código sin actualizar el contrato, el CI falla en vez de mentir.

Es también el **contrato de compatibilidad** con una futura versión del
`.so`: si un binario nuevo mantiene esos seis puntos (tamaño y layout del
`AuthResult`, itabs, escritores, rutas, regla terminal), este módulo sigue
siendo válido sin cambios.

## El bug que este módulo evita

La versión v2 del informe afirmaba que el LS **no** escribía
`has_valid_auth` ("le llega del servidor"). Era **falso**, y el error era de
método: el barrido filtraba los `STRB [Xn,#8]` *después* de recortar el
prefijo `jetski` del nombre de la función, y no distinguía entre "byte 8 de
un `*AuthResult`" y "byte 8 de una `string` de 16 bytes". Reconstruirlo cuesta
un binario, 161 682 funciones y 645 546 instrucciones `ADRP`; equivocarse
cuesta un prompt de login que no aparece.

## Hacia dónde va (siguiente tarea)

1. Cliente HTTP real contra `cloudcode-pa.googleapis.com` con la misma
   política (`== 200` y nada más) y la misma clasificación terminal/transitoria.
2. Reimplementar el RPC `GetAuthStatus` del LS (Connect) en Rust, con este
   módulo como núcleo ⇒ el daemon puede *servir* el estado de auth sin el
   `.so`.
3. Portar las 331 superficies RPC del `LanguageServerService` por fases,
   empezando por las de auth (`GetAuthStatus`, `Login`, `Logout`,
   `ValidateProject`, `SelectLicense`).

---

*Autor: agente "General" (sesión branch), 2026-11-26. Todo lo anterior está
verificado por desensamblado salvo lo marcado explícitamente como NO
VERIFICADO.*

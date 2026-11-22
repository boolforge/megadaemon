//! ===================================================================
//! audit_scout_voice.rs — Auditoría adversarial del parche de seguridad
//! de la VOZ (AUDIT-VOICE-01..05), sobre el árbol tal y como está.
//!
//! Reglas de la ronda:
//!   * sólo tests: no se toca `src/`.
//!   * cada hallazgo se reproduce con un test que FALLA (o, si es
//!     estático, se documenta con fichero:línea y se dice por qué no se
//!     puede reproducir de forma hermética).
//!   * nada de secretos reales: los tokens de prueba son sintéticos.
//!
//! Contrato auditado (rust-core/src/voice_runtime.rs):
//!   * AUDIT-VOICE-01: allow-list `ALLOWED` (línea 370) + `is_allowed`
//!     (396) sustituye a la deny-list `VETOED` (341) / `is_vetoed` (387).
//!   * AUDIT-VOICE-02: `SECRET_PREFIXES` (411) + `looks_like_secret` (434)
//!     + `redact` (458) + `redact_value` (474) preservando `HASH_FIELDS`.
//!   * AUDIT-VOICE-03: `broadcast` (641) redacta la línea ANTES de los
//!     canales 1-3 (journal, sync, tablón) con `flock` (`append_locked`, 612).
//!   * AUDIT-VOICE-04: escritura bajo `flock`.
//!   * AUDIT-VOICE-05: `handle("act")` con `action:"do"` exige `confirm` (990).
//! ===================================================================

use megadaemon_core::voice_runtime as vr;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

/// `MEGADAEMON_STATE_DIR` es global al proceso: los tests que lo tocan se
/// serializan para que no se pisen (mismo patrón que `audit_scout_auth.rs`).
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Token sintético con la forma de un secreto real (nunca es real).
const SEC: &str = "ghp_AAAABBBBCCCCDDDDEEEEFFFFGGGGHHHHIIIIJJJJKKKKLLLL";

/// Prepara un directorio de estado limpio con el tablón del bus creado.
fn state_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("md-audit-voice-{}-{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("state dir");
    std::env::set_var("MEGADAEMON_STATE_DIR", dir.to_string_lossy().to_string());
    // El bus necesita su tablón: `Mesh::open` lo crea.
    let _ = megadaemon_core::mesh::Mesh::open(&dir.to_string_lossy());
    dir
}

// ------------------------------------------------------------------------ //
// VOICE-01 (P0) — el redactor sólo examina tokens delimitados por ESPACIOS
// y exige que el prefijo de credencial esté al INICIO del token.
// Consecuencia: un secreto pegado a otra cosa (`key=`, comillas, query
// string) NO se redacta y sale tal cual al journal, al sync y al tablón.
// ------------------------------------------------------------------------ //
#[test]
fn voice01_redact_no_deja_el_secreto_cuando_va_pegado() {
    let casos = [
        format!("token={}", SEC),                     // prefijo pegado
        format!("\"{}\"", SEC),                       // entre comillas
        format!("https://x/cb?access_token={}", SEC), // query string
        format!("Authorization: {}", SEC),            // cabecera
        format!("Bearer {}", SEC),                    // esquema
    ];
    let mut fugados = vec![];
    for c in &casos {
        let out = vr::redact(c);
        println!("[VOICE-01] entrada={:?}\n           salida={:?}", c, out);
        if out.contains(SEC) {
            fugados.push(c.clone());
        }
    }
    assert!(
        fugados.is_empty(),
        "[VOICE-01] P0: el redactor deja el secreto intacto en {} caso(s): {:?}\n\
         Criterio de aceptación: `redact()` no debe dejar ninguna subcadena de la\n\
         credencial en la salida, sea cual sea el carácter que la preceda.",
        fugados.len(),
        fugados
    );
}

// ------------------------------------------------------------------------ //
// VOICE-02 (P0) — el mensaje que la voz deposita en el BUZÓN del peer
// (estado compartido => tablero => repo) NO pasa por el redactor:
// `speak()` sólo redacta la COPIA que devuelve (voice_runtime.rs:958),
// el mensaje que se escribe va en claro (`webui::node_message`, webui.rs:873).
// ------------------------------------------------------------------------ //
#[test]
fn voice02_el_mensaje_al_buzon_del_peer_no_lleva_secretos() {
    let _g = lock_env();
    let dir = state_dir("buzon");
    let r = vr::speak("peer-audit", &format!("hola, mi clave es {}", SEC));
    println!("[VOICE-02] speak() devolvió: {}", r);
    let inbox = dir.join("inbox").join("peer-audit.ndjson");
    let contenido = fs::read_to_string(&inbox).unwrap_or_else(|e| {
        panic!(
            "[VOICE-02] no se pudo leer el buzón {}: {}",
            inbox.display(),
            e
        )
    });
    println!(
        "[VOICE-02] buzón ({} bytes): {:?}",
        contenido.len(),
        &contenido[..contenido.len().min(300)]
    );
    assert!(
        !contenido.contains(SEC),
        "[VOICE-02] P0: la credencial quedó en CLARO en el buzón compartido {}\n\
         (ese buzón se replica al repo, que es público).\n\
         Criterio de aceptación: lo que se escribe en `inbox/<peer>.ndjson` debe\n\
         pasar por `redact_value`/`redact` igual que la línea de broadcast.",
        inbox.display()
    );
}

// ------------------------------------------------------------------------ //
// VOICE-03 (control, verde) — la allow-list es la puerta real del ejecutor:
// fuera de `ALLOWED` no se ejecuta nada.  Comprueba también la
// normalización (mayúsculas y espacios) que preguntaba la ronda.
// ------------------------------------------------------------------------ //
#[test]
fn voice03_la_allow_list_rechaza_lo_destructivo_y_normaliza_el_nombre() {
    for tool in [
        "fs_write",
        "root_exec",
        "restart",
        "reboot",
        "git_ops",
        "battery_saver",
        "adb_shell",
        "signing_vault",
        "megadaemon_rizin",
    ] {
        assert!(
            !vr::is_allowed(tool),
            "[VOICE-03] la tool {} NO debería estar en la allow-list de la voz",
            tool
        );
    }
    for tool in [
        "mesh_claim",
        "swarm_chat",
        "system_overview",
        "MESH_CLAIM",
        " mesh_claim ",
    ] {
        assert!(
            vr::is_allowed(tool),
            "[VOICE-03] la tool {} debería estar permitida (normalización trim+lowercase)",
            tool
        );
    }
    println!("[VOICE-03] allow-list verificada (normalización trim+lowercase confirmada)");
}

// ------------------------------------------------------------------------ //
// VOICE-04 (control, verde) — AUDIT-VOICE-05: `do` exige `confirm: true`.
// ------------------------------------------------------------------------ //
#[test]
fn voice04_la_accion_do_exige_confirmacion() {
    let r = vr::handle(
        "act",
        &json!({"action": "do", "tool": "system_overview", "args": {}}),
    );
    println!("[VOICE-04] handle(act, do sin confirm) -> {}", r);
    assert_eq!(
        r.get("ok").and_then(|v| v.as_bool()),
        Some(false),
        "[VOICE-04] `do` sin confirm debe rechazarse"
    );
    assert_eq!(
        r.get("error").and_then(|v| v.as_str()),
        Some("confirmation_required"),
        "[VOICE-04] el error debe ser `confirmation_required` (contrato de la voz)"
    );
}

// ------------------------------------------------------------------------ //
// VOICE-05 (P2) — la allow-list anuncia tools que NO existen en el registro
// de MCP: `mesh_snapshot` no está en `mcp_ext::tool_definitions()` ni es una
// de las ramas especiales del ejecutor => entrada muerta.
// ------------------------------------------------------------------------ //
#[test]
fn voice05_la_allow_list_no_contiene_tools_inexistentes() {
    // Contrato copiado de voice_runtime.rs:370-384 (constante privada).
    let allowed = [
        "mesh_claim",
        "mesh_ask",
        "swarm_chat",
        "system_overview",
        "capabilities",
        "task_queue",
        "alerts",
        "agent",
        "apk_inventory",
        "dex_decompile",
        "mesh_snapshot",
    ];
    // Las dos primeras las ejecuta el propio ejecutor (ramas especiales).
    let especiales = ["mesh_claim", "mesh_ask"];
    // OJO: `tool_definitions()` devuelve un ARRAY JSON de tools (no un
    // objeto con la clave "tools"): leerlo como objeto daria un registro
    // vacio y el test seria un falso positivo.
    // El catalogo CANONICO es `mcp_server::tool_definitions()` (es lo que
    // sirve `tools/list`: 51 tools).  `mcp_ext::tool_definitions()` es solo
    // una parte => usarlo aqui daria falsos positivos.
    let defs = megadaemon_core::mcp_server::tool_definitions();
    let registro: Vec<String> = defs
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !registro.is_empty(),
        "[VOICE-05] el registro MCP salio vacio: la forma de tool_definitions() cambio"
    );
    let mut muertas = vec![];
    for a in allowed {
        let existe = registro.iter().any(|r| r == a) || especiales.contains(&a);
        if !existe {
            muertas.push(a);
        }
    }
    println!(
        "[VOICE-05] registro MCP: {} tools; allow-list de la voz: {} entradas; muertas: {:?}",
        registro.len(),
        allowed.len(),
        muertas
    );
    assert!(
        muertas.is_empty(),
        "[VOICE-05] P2: la allow-list declara {} tool(s) que no existen ni en el\n\
         registro MCP ni como rama del ejecutor: {:?}\n\
         Criterio de aceptación: toda entrada de la allow-list debe ser una tool\n\
         real del registro o una rama explícita del ejecutor.",
        muertas.len(),
        muertas
    );
}

// ------------------------------------------------------------------------ //
// VOICE-06 (P2) — `redact()` destruye el texto: parte por ESPACIOS y lo
// re-une con un único espacio.  Un texto multilínea (un stack trace, un
// JSON) sale en una sola línea, y un secreto partido por un salto de línea
// queda sólo a medias redactado.
// ------------------------------------------------------------------------ //
#[test]
fn voice06_redact_no_debe_destruir_el_texto() {
    let entrada = "primera linea\nsegunda linea";
    let out = vr::redact(entrada);
    println!("[VOICE-06] entrada={:?} salida={:?}", entrada, out);
    assert!(
        out.contains('\n'),
        "[VOICE-06] P2: `redact()` ha aplanado el texto (se comió los saltos de línea)\n\
         Criterio de aceptación: la redacción debe conservar la estructura del texto\n\
         (y redactar por tokens, no por líneas)."
    );
    // Y el caso de seguridad: un secreto partido por un salto de línea.
    let partido = format!("ghp_AAAA\n{}", &SEC[7..]);
    let out2 = vr::redact(&partido);
    println!("[VOICE-06] partido={:?} salida={:?}", partido, out2);
    assert!(
        !out2.contains("ghp_AAAA") || !out2.contains(&SEC[7..]),
        "[VOICE-06] P2: un secreto partido por un salto de línea queda a medias en la salida"
    );
}

// ------------------------------------------------------------------------ //
// VOICE-07 (P2) — `redact_value` redacta los VALORES pero no las CLAVES:
// una credencial usada como nombre de campo sale tal cual.
// ------------------------------------------------------------------------ //
#[test]
fn voice07_redact_value_redacta_las_claves() {
    let v = json!({ SEC: "valor", "detail": "texto" });
    let out = vr::redact_value(&v);
    println!("[VOICE-07] entrada={} salida={}", v, out);
    assert!(
        !out.to_string().contains(SEC),
        "[VOICE-07] P2: la credencial sobrevive como CLAVE del objeto JSON\n\
         Criterio de aceptación: `redact_value` debe aplicarse a claves y valores."
    );
}

// ------------------------------------------------------------------------ //
// VOICE-08 (control, verde) — los hashes se preservan POR CAMPO (sha256,
// hash, digest, checksum, md5) y el redactor es simétrico en los formatos
// con prefijo de credencial bien conocido.
// ------------------------------------------------------------------------ //
#[test]
fn voice08_los_hashes_se_preservan_por_campo_y_los_formatos_conocidos_se_redactan() {
    let sha = "a".repeat(64);
    let v = json!({"sha256": sha, "detail": format!("firma {}", SEC)});
    let out = vr::redact_value(&v);
    println!("[VOICE-08] salida={}", out);
    assert_eq!(
        out["sha256"].as_str().unwrap(),
        sha,
        "[VOICE-08] el campo sha256 debe preservarse (el broadcast lo necesita)"
    );
    assert!(
        !out["detail"].as_str().unwrap().contains(SEC),
        "[VOICE-08] el campo detail debe quedar redactado"
    );
    // Formatos con prefijo al inicio del token: sí se redactan.
    for s in [
        format!("clave {}", SEC),
        format!("Bearer {}", SEC),
        SEC.to_string(),
    ] {
        assert!(
            !vr::redact(&s).contains(SEC),
            "[VOICE-08] debería redactarse: {:?}",
            s
        );
    }
}

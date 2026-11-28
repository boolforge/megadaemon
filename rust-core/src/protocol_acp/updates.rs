//! **Dispatch de `session/update`** (la notificación ACP v1).
//!
//! MEDIDO (2026-11-28, `docs/DECISION_ACP.md` §3.2): el TIPO `SessionUpdate`
//! estaba en el esquema canónico vendorizado (`crate::acp_schema`), pero el
//! **dispatch** (emitir los updates hacia el cliente) NO estaba cableado:
//! el agente hacía su trabajo y el cliente ACP no se enteraba de nada
//! hasta el final del turno.
//!
//! Lo que añade este módulo:
//!
//! * [`params`] / [`notificacion`] / [`enmarcar`]: la notificación
//!   canónica `{"method":"session/update","params":{"sessionId":…,
//!   "update":…}}` y su línea NDJSON lista para el cable.  El **framing**
//!   lo pone el transporte (una línea por mensaje): no se duplica.
//! * Los constructores ([`texto_usuario`], [`texto_agente`],
//!   [`pensamiento`], [`tool_call`], [`tool_call_update`]): los 7
//!   `SessionUpdate` del esquema, con los `ContentBlock` ya construidos.
//! * [`registrar`]: el update entra en el **historial** de la sesión, que
//!   es lo que `session/load` devuelve ⇒ el cliente que carga una sesión
//!   ve también lo que pasó en ella.
//!
//! Puro salvo el registro de sesiones (que es un `Mutex` global medido:
//! el daemon es un proceso largo y `session/cancel` o `session/resume`
//! son llamadas sueltas).

use serde_json::{json, Value};

use super::super::acp_schema::{ContentBlock, ContentChunk, SessionUpdate, ToolCall, ToolCallUpdate};
use super::sesion;
use super::transport;

/// El método canónico de la notificación.
pub const METODO: &str = "session/update";

/// Los `params` de la notificación: `{sessionId, update}`.
///
/// MEDIDO (esquema canónico ACP v1): la notificación lleva el id de la
/// sesión y el update; el update es un objeto con su discriminante
/// (`sessionUpdate`).
pub fn params(session_id: &str, update: &SessionUpdate) -> Value {
    json!({ "sessionId": session_id, "update": update })
}

/// La notificación JSON-RPC (sin `id`: es una notificación).
pub fn notificacion(session_id: &str, update: &SessionUpdate) -> Value {
    let m = transport::notificacion(METODO, Some(params(session_id, update)));
    serde_json::to_value(&m).unwrap_or(Value::Null)
}

/// La LÍNEA NDJSON lista para el cable (una línea por mensaje).
pub fn enmarcar(session_id: &str, update: &SessionUpdate) -> String {
    let m = transport::notificacion(METODO, Some(params(session_id, update)));
    transport::enmarcar(&m)
}

/// Un fragmento de texto del USUARIO.
pub fn texto_usuario(texto: &str) -> SessionUpdate {
    SessionUpdate::UserMessageChunk(ContentChunk {
        content: ContentBlock::from(texto),
        meta: None,
    })
}

/// Un fragmento de texto del AGENTE (la respuesta en streaming).
pub fn texto_agente(texto: &str) -> SessionUpdate {
    SessionUpdate::AgentMessageChunk(ContentChunk {
        content: ContentBlock::from(texto),
        meta: None,
    })
}

/// Un fragmento del razonamiento interno del agente.
pub fn pensamiento(texto: &str) -> SessionUpdate {
    SessionUpdate::AgentThoughtChunk(ContentChunk {
        content: ContentBlock::from(texto),
        meta: None,
    })
}

/// Una llamada a herramienta.
pub fn tool_call(tc: ToolCall) -> SessionUpdate {
    SessionUpdate::ToolCall(tc)
}

/// La actualización de una llamada a herramienta (progreso, resultado).
pub fn tool_call_update(u: ToolCallUpdate) -> SessionUpdate {
    SessionUpdate::ToolCallUpdate(u)
}

/// Emite un update: lo mete en el historial de la sesión y devuelve la
/// línea NDJSON para el cable.
///
/// MEDIDO (2026-11-28): el historial de la sesión es lo que `session/load`
/// devuelve; si el update no se guarda, el cliente que recarga la sesión
/// pierde el progreso del turno.
///
/// **OJO**: el registro es un `Mutex` **global** y NO es reentrante ⇒ esta
/// función NO se puede llamar desde dentro de `sesion::con_global` (el
/// `Mutex` se bloquearía a sí mismo).  Quien ya tenga el registro cogido
/// usa `Registro::registrar_update` directamente.
pub fn registrar(session_id: &str, update: &SessionUpdate) -> Result<String, String> {
    let v = params(session_id, update);
    sesion::con_global(|r| r.registrar_update(session_id, &v))?;
    Ok(enmarcar(session_id, update))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_notificacion_lleva_el_id_de_sesion_y_el_update() {
        let n = notificacion("ses-1", &texto_agente("hola"));
        assert_eq!(n.get("method").and_then(|m| m.as_str()), Some("session/update"));
        assert!(n.get("id").is_none(), "una notificación no lleva id");
        assert_eq!(
            n.pointer("/params/sessionId").and_then(|v| v.as_str()),
            Some("ses-1")
        );
        assert_eq!(
            n.pointer("/params/update/sessionUpdate").and_then(|v| v.as_str()),
            Some("agent_message_chunk")
        );
    }

    #[test]
    fn el_enmarque_es_una_sola_linea() {
        let l = enmarcar("ses-1", &texto_usuario("hola"));
        assert_eq!(l.matches('\n').count(), 1, "NDJSON: una línea por mensaje");
        assert!(l.ends_with('\n'));
        assert!(!l[..l.len() - 1].contains('\n'));
    }

    #[test]
    fn los_constructores_cubren_los_updates_canónicos() {
        let casos: Vec<(SessionUpdate, &str)> = vec![
            (texto_usuario("u"), "user_message_chunk"),
            (texto_agente("a"), "agent_message_chunk"),
            (pensamiento("p"), "agent_thought_chunk"),
        ];
        for (u, tag) in casos {
            let j = serde_json::to_value(&u).unwrap();
            assert_eq!(j.get("sessionUpdate").and_then(|v| v.as_str()), Some(tag));
        }
    }

    #[test]
    fn el_update_registrado_queda_en_el_historial() {
        let id = "ses-test-1";
        sesion::con_global(|r| {
            r.nueva("/tmp", None);
            r.registrar_update(id, &json!({"sessionUpdate": "agent_message_chunk"}))
                .ok();
        });
        // La sesión se crea con un id propio; se registra sobre el real.
        let lista = sesion::con_global(|r| r.listar());
        assert!(!lista.is_empty(), "hay sesiones");
        let propio = lista[0]["sessionId"].as_str().unwrap_or("").to_string();
        let ok = sesion::con_global(|r| {
            r.registrar_update(&propio, &json!({"sessionUpdate": "agent_message_chunk"}))
        });
        assert!(ok.is_ok(), "registrar en una sesión existente: {:?}", ok);
        let historial = sesion::con_global(|r| r.historial(&propio).len());
        assert!(historial >= 1, "el update queda en el historial");
    }

    #[test]
    fn registrar_en_una_sesion_inexistente_falla() {
        let e = registrar("no-existe-xyz", &texto_agente("hola")).unwrap_err();
        assert!(e.contains("no-existe-xyz"), "el error dice qué sesión: {}", e);
    }
}

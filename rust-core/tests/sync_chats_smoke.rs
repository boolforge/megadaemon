//! Arnés de pruebas del motor de chats (`sync_chats`), por la API **pública**.
//!
//! Qué comprueba (todo determinista, sin red y sin móvil):
//!
//! * el catálogo: los cuatro chats + Grok + los servicios de IA que ya
//!   existían (sin duplicarlos);
//! * el modelo unificado: `origin` y `access` siempre presentes;
//! * el merge idempotente por `servicio::conversation_id::message_id`;
//! * el cursor incremental;
//! * los cuatro adaptadores con su fixture: SQLite de Telegram, JSON de
//!   Discord, `dumpsys` de notificaciones y el **caso cifrado** (que debe
//!   devolver `cifrado_no_descifrado` y **ningún** mensaje inventado).
//!
//! NOTA: este arnés necesita `pub mod sync_chats;` en `src/lib.rs` (la línea
//! que añade el módulo).  Con ella, el binario `md-sync-chats` también
//! compila.

use megadaemon_core::sync_chats::{
    classify_db, conversation_key, cursor_for, is_newer_than_cursor, list_services, merge_all,
    message_key, notifications_conversations, parse_discord_export, parse_grok_export, pull,
    service_names, signal_db_state, telegram_from_path, AccessLevel, ChatMessage, Conversation,
    Cursor, DbState, Origin,
};
use rusqlite::Connection;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn temp_path(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("md_sync_chats_smoke_{}_{}.db", tag, std::process::id()));
    p
}

/// Fixture de Telegram con el esquema real medido.
fn telegram_fixture(path: &PathBuf) {
    let conn = Connection::open(path).expect("debe abrir la BD de prueba");
    conn.execute_batch(
        "CREATE TABLE messages (uid INTEGER PRIMARY KEY, chat_id INTEGER, from_id INTEGER,
             date INTEGER, message TEXT);
         CREATE TABLE chats (chat_id INTEGER, title TEXT);
         CREATE TABLE users (user_id INTEGER, first_name TEXT);
         CREATE TABLE info (key TEXT, value TEXT);
         INSERT INTO info VALUES ('user_id', '7');
         INSERT INTO users VALUES (7, 'Yo');
         INSERT INTO users VALUES (9, 'Ana');
         INSERT INTO chats VALUES (42, 'Ana');
         INSERT INTO messages VALUES (1, 42, 9, 1700000000, 'hola');
         INSERT INTO messages VALUES (2, 42, 7, 1700000060, 'hola Ana');
         INSERT INTO messages VALUES (3, 42, 9, 1700000120, 'que tal');",
    )
    .expect("debe crear el esquema");
}

const DUMPSYS: &str = r#"NotificationRecord(0x1: pkg=com.whatsapp user=android uid=1)
    extras={
      android.title=String (Ana)
      android.text=String (hola)
    }
    postTime=11-20 12:34:56.789
"#;

const DISCORD_EXPORT: &str = r#"{"channels":[{"id":"111","name":"general","guild":{"name":"MD"},
  "messages":[{"id":"1001","timestamp":"2024-01-02T03:04:05.000+00:00","content":"hola",
  "author":{"username":"ana"}}]}]}"#;

#[test]
fn el_catalogo_tiene_los_cuatro_chats_grok_y_los_servicios_de_ia() {
    let names = service_names();
    for s in ["whatsapp", "telegram", "signal", "discord", "grok"] {
        assert!(names.contains(&s), "falta {}", s);
    }
    // Los servicios de IA que ya existían NO se duplican: están, una vez.
    for s in ["chatgpt", "claude", "gemini", "deepseek", "kimi", "mistral"] {
        assert_eq!(names.iter().filter(|n| **n == s).count(), 1, "{}", s);
    }
    let value = list_services();
    assert_eq!(value["ok"], json!(true));
    assert!(value["services"].as_array().unwrap().len() >= 13);
}

#[test]
fn telegram_lee_el_fixture_y_el_cursor_es_incremental() {
    let path = temp_path("tg");
    let _ = std::fs::remove_file(&path);
    telegram_fixture(&path);

    let rep = telegram_from_path(&path, &Cursor::default(), 100);
    assert_eq!(rep.status, "ok");
    assert_eq!(rep.access, AccessLevel::Completo);
    assert_eq!(rep.origin, Origin::LocalDb);
    assert_eq!(rep.messages(), 3);
    assert_eq!(rep.conversations[0].conversation_id, "chat_42");
    assert_eq!(rep.conversations[0].title, "Ana");

    // Con cursor en el último mensaje no baja nada nuevo.
    let cursor = Cursor {
        service: "telegram".into(),
        last_ts: 1700000120,
        last_id: "3".into(),
        ..Default::default()
    };
    let rep2 = telegram_from_path(&path, &cursor, 100);
    assert_eq!(rep2.messages(), 0);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn una_bd_cifrada_no_devuelve_mensajes_inventados() {
    let cifrado = vec![0xABu8; 48];
    assert_eq!(classify_db(&cifrado), DbState::Encrypted);
    let rep = signal_db_state(&cifrado, "signal.db");
    assert_eq!(rep.access, AccessLevel::CifradoNoDescifrado);
    assert_eq!(rep.conversations.len(), 0);
    assert_eq!(rep.messages(), 0);
    assert!(!rep.notes.is_empty());
}

#[test]
fn las_notificaciones_son_texto_real_de_origen_notificaciones() {
    let convs = notifications_conversations("whatsapp", "com.whatsapp", DUMPSYS, &Cursor::default());
    assert_eq!(convs.len(), 1);
    assert_eq!(convs[0].origin, Origin::Notifications);
    assert_eq!(convs[0].access, AccessLevel::Metadatos);
    assert_eq!(convs[0].messages[0].kind, "notificacion");
    assert!(convs[0].messages[0].text.contains("hola"));
}

#[test]
fn la_exportacion_de_discord_y_de_grok_se_normalizan() {
    let d = parse_discord_export(DISCORD_EXPORT, &Cursor::default(), 100);
    assert_eq!(d.status, "ok");
    assert_eq!(d.origin, Origin::Export);
    assert_eq!(d.access, AccessLevel::Completo);
    assert_eq!(d.conversations[0].messages.len(), 1);
    assert_eq!(d.conversations[0].messages[0].author, "ana");

    let g = parse_grok_export(
        r#"{"conversations":[{"id":"c1","title":"T","messages":[{"role":"user","content":"hola"}]}]}"#,
        &Cursor::default(),
        10,
    );
    assert_eq!(g.status, "ok");
    assert_eq!(g.conversations[0].messages[0].text, "hola");
}

#[test]
fn el_merge_es_idempotente_por_mensaje() {
    let conv = Conversation {
        service: "telegram".into(),
        conversation_id: "chat_42".into(),
        title: "Ana".into(),
        messages: vec![ChatMessage {
            id: "1".into(),
            ts: 10,
            author: "peer".into(),
            text: "hola".into(),
            kind: "texto".into(),
            verified: true,
        }],
        origin: Origin::LocalDb,
        access: AccessLevel::Completo,
        updated_at: 10,
        source: String::new(),
        note: String::new(),
    };
    let mut store: BTreeMap<String, Conversation> = BTreeMap::new();
    let s1 = merge_all(&mut store, vec![conv.clone()]);
    assert_eq!(s1.added, 1);
    assert_eq!(s1.new_messages, 1);
    let s2 = merge_all(&mut store, vec![conv.clone()]);
    assert_eq!(s2.added, 0);
    assert_eq!(s2.new_messages, 0);
    assert_eq!(store.len(), 1);
    assert_eq!(store[&conversation_key("telegram", "chat_42")].messages.len(), 1);
    assert_eq!(
        message_key("telegram", "chat_42", "1"),
        "telegram::chat_42::1"
    );
}

#[test]
fn el_cursor_filtra_lo_nuevo() {
    let cursor = Cursor {
        last_ts: 100,
        last_id: "5".into(),
        ..Default::default()
    };
    assert!(is_newer_than_cursor(101, "1", &cursor));
    assert!(!is_newer_than_cursor(100, "5", &cursor));
    assert!(is_newer_than_cursor(100, "6", &cursor));
}

#[test]
fn sin_movil_el_pull_responde_el_estado_real_y_no_inventa() {
    // En el sandbox no hay `su`: el pull debe decirlo, no rellenar.
    let value = pull("telegram");
    let status = value["status"].as_str().unwrap_or("");
    assert!(
        status == "sin_acceso" || status == "parcial" || status == "ok",
        "estado inesperado: {}",
        status
    );
    if status == "sin_acceso" {
        assert_eq!(value["messages"], json!(0));
        assert!(!value["errors"].as_array().unwrap().is_empty()
            || !value["notes"].as_array().unwrap().is_empty());
    }
}

#[test]
fn el_cursor_por_defecto_es_vacio() {
    let c = cursor_for("telegram");
    assert_eq!(c.last_ts, 0);
    assert_eq!(c.last_id, "");
}

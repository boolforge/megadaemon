//! W0/W3 · Arnés de pruebas del grabador de llamadas (`call_recorder`).
//!
//! `src/lib.rs` es intocable sin acuerdo del bus, así que el módulo se
//! incluye aquí por ruta (como `tests/signing_vault.rs` o
//! `tests/device_inventory_smoke.rs`). En cuanto se añada
//! `pub mod call_recorder;` a `src/lib.rs`, este arnés puede borrarse sin
//! tocar los tests.
//!
//! Lo que se prueba aquí es el **comportamiento observable**: qué fuente se
//! detecta, qué hace el plan de captura y qué se detecta de un `dumpsys`
//! real. El módulo tiene 11 tests propios; este arnés añade la comprobación
//! de extremo a extremo con la forma exacta de la salida de `dumpsys audio`.

#[path = "../src/call_recorder.rs"]
mod call_recorder;

use call_recorder::{
    detect_active_call, detect_media_projection, detect_source, list_recordings, plan_capture,
    plan_video_capture, CallSession, CallSource, RecorderConfig,
};

#[test]
fn se_reconocen_las_fuentes_de_llamada() {
    assert!(matches!(detect_source("com.whatsapp", ""), CallSource::WhatsApp));
    assert!(matches!(
        detect_source("org.telegram.messenger", ""),
        CallSource::Telegram
    ));
    assert!(matches!(
        detect_source("org.thoughtcrime.securesms", ""),
        CallSource::Signal
    ));
    assert!(matches!(detect_source("com.discord", ""), CallSource::Discord));
    assert!(matches!(detect_source("", "com.android.server.telecom"),
        CallSource::System));
    // Desconocido: se conserva el paquete crudo, no se inventa.
    match detect_source("com.empresa.rara", "") {
        CallSource::Unknown(s) => assert_eq!(s, "com.empresa.rara"),
        _ => panic!("debe ser Unknown"),
    }
}

#[test]
fn el_plan_de_captura_es_puro_y_distingue_la_fuente() {
    let cfg = RecorderConfig::default();
    let sys = plan_capture(&cfg, CallSource::System);
    let wa = plan_capture(&cfg, CallSource::WhatsApp);
    assert_ne!(sys.method, wa.method);
    assert!(!sys.commands.is_empty());
    assert!(!wa.commands.is_empty());
    // El plan de vídeo tiene que ser distinto del de audio.
    let vid = plan_video_capture(&cfg, CallSource::WhatsApp);
    assert_ne!(vid.method, wa.method);
    // Determinista: dos llamadas dan lo mismo.
    assert_eq!(plan_capture(&cfg, CallSource::System).commands, sys.commands);
}

#[test]
fn se_detecta_una_llamada_en_curso_en_un_dumpsys_real() {
    let dump = r#"
  Audio Focus stack entries:
  mCallState: 2
  mode: MODE_IN_COMMUNICATION
  state: IN_CALL
  Communication client: com.whatsapp
  package: com.whatsapp
"#;
    let call = detect_active_call(dump).expect("debe parsear el dump");
    assert!(call.active);
    assert_eq!(call.state, "IN_CALL");
    assert_eq!(call.package.as_deref(), Some("com.whatsapp"));
    assert_eq!(call.client.as_deref(), Some("com.whatsapp"));
}

#[test]
fn un_dumpsys_sin_llamada_no_inventa_una() {
    let dump = "mCallState: 0\nmode: MODE_NORMAL\nstate: IDLE\n";
    let call = detect_active_call(dump).expect("dump válido");
    assert!(!call.active);
    assert_eq!(call.state, "IDLE");
    // Y un texto que no es un dump devuelve None.
    assert!(detect_active_call("hola").is_none());
    assert!(detect_active_call("").is_none());
}

#[test]
fn la_proyeccion_de_media_se_detecta() {
    let dump = "media projection: packageName=com.screenrecorder uid=10123";
    assert!(detect_media_projection(dump));
    assert!(!detect_media_projection("nada aquí"));
}

#[test]
fn la_sesion_se_puede_cerrar_y_serializar() {
    let mut s = CallSession::new("id-1", "whatsapp", "2026-11-20T00:00:00Z", "voice");
    assert_eq!(s.state, "recording");
    s.finalize("2026-11-20T00:01:00Z", 60);
    assert_eq!(s.state, "stopped");
    assert_eq!(s.duration_s, 60);
    let j = s.to_json();
    assert_eq!(j["source"], "whatsapp");
    assert_eq!(j["duration_s"], 60);
}

#[test]
fn listar_grabaciones_devuelve_lista_vacia_si_no_hay_directorio() {
    let l = list_recordings("/data/local/tmp/no-existe-esta-ruta-xyz");
    assert!(l.is_empty() || l.len() == 0);
}

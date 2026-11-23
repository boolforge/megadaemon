//! Arnés de pruebas de la automatización de comunicaciones
//! (`src/comms_automation.rs`).
//!
//! Usa la biblioteca (`use megadaemon_core::…;`): los módulos ya están
//! cableados en `src/lib.rs`.

use megadaemon_core::comms_automation::{
    consent_from_json, consent_to_json, decide, CommsDecision, CommsEvent, CommsEventKind,
    Consent, History,
};

fn sms(num: &str, cuerpo: &str) -> CommsEvent {
    CommsEvent::new(CommsEventKind::SmsReceived, 100, num).with_body(cuerpo)
}

#[test]
fn por_defecto_todo_esta_autorizado_y_se_ejecuta() {
    let c = Consent::default();
    assert!(c.enabled);
    assert!(c.auto_reply);
    assert!(c.forwarding);
    assert!(c.outgoing_allowed);
    assert!(c.allow_record_calls);
    assert!(!c.dry_run, "por defecto NO es simulación: se ejecuta de verdad");
}

#[test]
fn un_sms_produce_respuesta_y_reenvio() {
    let mut c = Consent::default();
    c.forward_to = Some("+34600999888".into());
    let d = decide(&sms("+34600111222", "urgente"), &c, &History::default(), 100);
    let reply = d.iter().find_map(|x| match x {
        CommsDecision::Reply { to, body } => Some((to.clone(), body.clone())),
        _ => None,
    });
    assert_eq!(reply.unwrap().0, "+34600111222");
    let fwd = d.iter().find_map(|x| match x {
        CommsDecision::Forward { to, body } => Some((to.clone(), body.clone())),
        _ => None,
    });
    assert_eq!(fwd.unwrap(), ("+34600999888".to_string(), "urgente".to_string()));
    // Y siempre queda registro.
    assert!(d.iter().any(|x| matches!(x, CommsDecision::Log { .. })));
}

#[test]
fn una_llamada_se_graba_porque_esta_autorizado() {
    let ev = CommsEvent::new(CommsEventKind::CallIncoming, 100, "+34600111222");
    let d = decide(&ev, &Consent::default(), &History::default(), 100);
    assert!(d.iter().any(|x| matches!(x, CommsDecision::Record { .. })));
}

#[test]
fn si_el_usuario_lo_desactiva_ya_no_se_graba() {
    let mut c = Consent::default();
    c.allow_record_calls = false;
    let ev = CommsEvent::new(CommsEventKind::CallIncoming, 100, "+34600111222");
    let d = decide(&ev, &c, &History::default(), 100);
    assert!(!d.iter().any(|x| matches!(x, CommsDecision::Record { .. })));
}

#[test]
fn la_lista_negra_del_usuario_se_respeta() {
    let mut c = Consent::default();
    c.blocked_peers = vec!["+34600111222".into()];
    let d = decide(&sms("+34600111222", "hola"), &c, &History::default(), 100);
    assert!(!d.iter().any(|x| matches!(x, CommsDecision::Reply { .. })));
}

#[test]
fn el_tope_por_hora_y_el_enfriamiento_se_cumplen() {
    let mut c = Consent::default();
    c.max_replies_per_hour = 1;
    c.cooldown_secs = 600;
    let h = History {
        replies: vec![100],
        last_reply: vec![("+34600111222".into(), 100)],
    };
    let d = decide(&sms("+34600111222", "hola"), &c, &h, 200);
    assert!(!d.iter().any(|x| matches!(x, CommsDecision::Reply { .. })));
    assert!(d.iter().any(|x| matches!(x, CommsDecision::Ignore { .. })));
}

#[test]
fn el_consentimiento_va_y_vuelve_de_json() {
    let mut c = Consent::default();
    c.reply_template = "luego te leo".into();
    c.forward_to = Some("+34600999888".into());
    c.dry_run = true;
    let back = consent_from_json(&consent_to_json(&c));
    assert_eq!(back, c);
}

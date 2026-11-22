//! Arnés de pruebas del motor de disparadores y reglas (`src/automation.rs`).
//!
//! Los módulos ya están cableados en `src/lib.rs`, así que este arnés usa la
//! biblioteca (`use megadaemon_core::…;`) en vez de copiar el módulo con
//! `#[path]`: al copiarlo, `crate::` pasaría a ser la raíz de este crate y
//! romperían las referencias internas (el mismo error que suffers
//! `tests/root_hide_app_intel.rs` antes de arreglarlo).
//!
//! Los tests unitarios del módulo viven dentro de él (`#[cfg(test)] mod tests`
//! en `src/automation.rs`) y se ejecutan con `cargo test --lib`. Aquí se
//! añaden los de integración: persistencia real en disco, ciclo completo
//! regla → evento → acción → journal.

use megadaemon_core::automation::{
    matches, run_rule, Action, Condition, Effector, Event, EventKind, Field, Op, Outcome, Rule,
    Trigger,
};
use serde_json::json;

/// Efector que anota lo ejecutado y puede fallar a propósito.
struct Grabador {
    log: Vec<String>,
    fallar_en: Option<usize>,
    n: usize,
}

impl Grabador {
    fn nuevo() -> Grabador {
        Grabador {
            log: Vec::new(),
            fallar_en: None,
            n: 0,
        }
    }
}

impl Effector for Grabador {
    fn perform(&mut self, action: &Action) -> Result<String, String> {
        let i = self.n;
        self.n += 1;
        self.log.push(action.describe());
        if self.fallar_en == Some(i) {
            return Err("fallo simulado".to_string());
        }
        Ok(format!("ok #{}", i))
    }
}

fn regla_sms() -> Rule {
    Rule {
        id: "sms-casa".into(),
        name: "SMS de casa".into(),
        trigger: Trigger::Event(EventKind::SmsReceived),
        conditions: vec![Condition::new(Field::Text, Op::Contains, "casa")],
        actions: vec![
            Action::Notify {
                title: "Casa".into(),
                body: "Han escrito".into(),
            },
            Action::Setting {
                namespace: "global".into(),
                key: "modo".into(),
                value: "casa".into(),
            },
        ],
        cooldown_secs: 60,
        ..Rule::default()
    }
}

#[test]
fn el_ciclo_completo_regla_evento_accion_y_journal() {
    let dir = std::env::temp_dir().join("md_test_automation_ciclo");
    let _ = std::fs::remove_dir_all(&dir);
    let reglas = dir.join("rules.json");
    let journal = dir.join("journal.ndjson");

    let mut r = regla_sms();
    r.id = "sms-casa".into();
    megadaemon_core::automation::save_rules(rutas(&reglas), &[r.clone()]).unwrap();

    // La regla se ha guardado y se ha vuelto a leer.
    let cargadas = megadaemon_core::automation::load_rules(rutas(&reglas));
    assert_eq!(cargadas.len(), 1);
    assert_eq!(cargadas[0].id, "sms-casa");

    // El evento casa; la regla dispara.
    let ev = Event::new(EventKind::SmsReceived, 1000)
        .with_key("+34600111222")
        .with_text("¿estás en casa?")
        .with_extra("peer", "+34600111222");
    assert!(matches(&r, &ev));

    let mut eff = Grabador::nuevo();
    let out: Vec<Outcome> = run_rule(&r, &ev, &mut eff, 1000);
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|o| o.ok));
    assert_eq!(eff.log.len(), 2);

    // El journal deja rastro de las dos acciones.
    megadaemon_core::automation::append_journal(rutas(&journal), &out).unwrap();
    let tail = megadaemon_core::automation::journal_tail(rutas(&journal), 10);
    assert_eq!(tail.len(), 2);
    assert_eq!(tail[0]["rule_id"], json!("sms-casa"));
    assert_eq!(tail[0]["event"], json!("sms_received"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn un_evento_que_no_casa_no_dispara_nada() {
    let r = regla_sms();
    let ev = Event::new(EventKind::SmsReceived, 1000).with_text("voy de camino");
    assert!(!matches(&r, &ev));
    let mut eff = Grabador::nuevo();
    assert!(run_rule(&r, &ev, &mut eff, 1000).is_empty());
    assert!(eff.log.is_empty());
}

#[test]
fn un_fallo_se_registra_pero_no_impide_el_resto() {
    let mut r = regla_sms();
    r.stop_on_error = false;
    let ev = Event::new(EventKind::SmsReceived, 1000).with_text("casa");
    let mut eff = Grabador {
        log: Vec::new(),
        fallar_en: Some(0),
        n: 0,
    };
    let out = run_rule(&r, &ev, &mut eff, 1);
    assert_eq!(out.len(), 2);
    assert!(!out[0].ok);
    assert!(out[1].ok);
}

#[test]
fn las_reglas_sobreviven_a_un_guardado_y_carga() {
    let dir = std::env::temp_dir().join("md_test_automation_persistencia");
    let _ = std::fs::remove_dir_all(&dir);
    let f = dir.join("rules.json");
    let mut r = regla_sms();
    r.max_fires = 7;
    r.allow_root = false;
    megadaemon_core::automation::save_rules(rutas(&f), &[r]).unwrap();
    let back = megadaemon_core::automation::load_rules(rutas(&f));
    assert_eq!(back[0].max_fires, 7);
    assert!(!back[0].allow_root);
    assert_eq!(back[0].conditions.len(), 1);
    assert_eq!(back[0].actions.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `Path` → `&str` para las funciones que trabajan con rutas.
fn rutas(p: &std::path::Path) -> &str {
    p.to_str().unwrap()
}

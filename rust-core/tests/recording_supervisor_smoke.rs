//! Arnés del **supervisor de grabación** (`call_recorder::RecordingSupervisor`).
//!
//! El supervisor gobierna el grabador **nativo** de Samsung (el único que
//! reproduce el aviso legal) y **no** graba por su cuenta. El recorrido es
//! *armar → grabar → marcar → cerrar*.
//!
//! Lo que se comprueba aquí, sobre todo, es la **guarda legal**: sin el valor
//! de feature *con disclaimer* el supervisor **se niega a armar** (en España,
//! grabar a un tercero en silencio es delito: art. 197 CP, y además RGPD).

#[path = "../src/call_recorder.rs"]
mod call_recorder;

use call_recorder::*;
use std::path::PathBuf;

fn tmpdir(nombre: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sup-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("directorio temporal");
    d
}

#[test]
fn armar_exige_un_valor_que_avise() {
    let mut s = RecordingSupervisor::new();
    assert_eq!(s.state, SupervisorState::Idle);

    // Grabación automática y silenciosa: PROHIBIDA.
    let e = s.armar("THL", RECORDING_VALUE_SILENT).unwrap_err();
    assert!(e.contains("art. 197"), "el error debe citar la ley: {e}");
    assert_eq!(s.state, SupervisorState::Failed);

    // Un valor que no avisa: tampoco.
    assert!(s.armar("THL", "AlgoDistinto").is_err());

    // Con disclaimer: adelante.
    s.armar("THL", RECORDING_VALUE_WITH_DISCLAIMER).unwrap();
    assert_eq!(s.state, SupervisorState::Armed);
    assert!(s.aviso_reproducido, "con disclaimer el aviso es inherente");
}

#[test]
fn el_csc_debe_tener_grabacion_nativa() {
    let mut s = RecordingSupervisor::new();
    let e = s.armar("AMO", RECORDING_VALUE_WITH_DISCLAIMER).unwrap_err();
    assert!(e.contains("AMO"), "{e}");
    assert!(e.contains("THL"), "el error debe decir qué CSC sirven: {e}");
}

#[test]
fn el_flujo_completo_marca_y_cierra() {
    let d = tmpdir("flujo");
    let f = d.join("grabacion.omr");
    std::fs::write(&f, b"contenido de audio de prueba").unwrap();

    let mut s = RecordingSupervisor::new();
    s.armar("THL", RECORDING_VALUE_WITH_DISCLAIMER).unwrap();
    s.iniciar_llamada("ses-001", "2026-11-21T10:00:00Z").unwrap();
    assert_eq!(s.state, SupervisorState::Recording);

    s.marcar(f.to_str().unwrap(), "2026-11-21T10:02:00Z").unwrap();
    assert_eq!(s.state, SupervisorState::Marked);
    assert_eq!(s.size_bytes, 28);
    assert_eq!(s.sha256.len(), 64, "sha256 en hex: 64 caracteres");

    s.cerrar().unwrap();
    assert_eq!(s.state, SupervisorState::Closed);
    assert!(s.listo_para_boveda());

    let j = s.to_json();
    assert_eq!(j["estado"], "closed");
    assert_eq!(j["size_bytes"], 28);
    assert_eq!(j["aviso"], true);
    assert_eq!(j["csc"], "THL");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn una_grabacion_vacia_o_ausente_no_se_marca() {
    let d = tmpdir("vacia");
    let f = d.join("vacio.omr");
    std::fs::write(&f, b"").unwrap();
    let mut s = RecordingSupervisor::new();
    s.armar("INS", RECORDING_VALUE_WITH_DISCLAIMER).unwrap();
    s.iniciar_llamada("s", "t").unwrap();
    let e = s.marcar(f.to_str().unwrap(), "t").unwrap_err();
    assert!(e.contains("vacío"), "{e}");
    assert!(s.marcar("/no/existe/xyz.omr", "t").is_err());
    assert_eq!(s.state, SupervisorState::Recording, "un fallo no debe cerrar la sesión");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn las_fases_no_se_pueden_saltar() {
    let mut s = RecordingSupervisor::new();
    assert!(s.iniciar_llamada("s", "t").is_err(), "no se puede llamar sin armar");
    assert!(s.marcar("/x", "t").is_err());
    assert!(s.cerrar().is_err());

    s.armar("ILO", RECORDING_VALUE_WITH_DISCLAIMER).unwrap();
    assert!(s.marcar("/x", "t").is_err(), "no se puede marcar sin llamada");
    assert!(s.cerrar().is_err(), "no se puede cerrar sin marcar");
    assert!(s.iniciar_llamada("", "t").is_err(), "la sesión necesita id");
}

#[test]
fn el_preflight_avisa_del_bloqueo_legal() {
    let v = supervisor_preflight("AMO", RECORDING_VALUE_SILENT);
    assert!(v.iter().any(|l| l.contains("BLOQUEO")), "{v:?}");
    assert!(v.iter().any(|l| l.contains("art. 197")));

    let v2 = supervisor_preflight("THL", RECORDING_VALUE_WITH_DISCLAIMER);
    assert!(!v2.iter().any(|l| l.contains("BLOQUEO")), "{v2:?}");
    assert!(v2.iter().any(|l| l.contains("InCallUI")));
    assert!(supervisor_requisitos().len() >= 5);
}

#[test]
fn el_valor_de_aviso_se_reconoce() {
    assert!(recording_value_avisa(RECORDING_VALUE_WITH_DISCLAIMER));
    assert!(recording_value_avisa(RECORDING_VALUE_WITH_DISCLAIMER_INDIA));
    assert!(!recording_value_avisa(RECORDING_VALUE_SILENT));
    assert!(recording_value_es_silenciosa(RECORDING_VALUE_SILENT));
    assert!(!recording_value_es_silenciosa(RECORDING_VALUE_WITH_DISCLAIMER));
    assert!(csc_permite_grabacion_con_aviso("THL", RECORDING_VALUE_WITH_DISCLAIMER).is_ok());
    assert!(csc_permite_grabacion_con_aviso("", RECORDING_VALUE_WITH_DISCLAIMER).is_err());
}

#[test]
fn los_estados_tienen_nombre_estable() {
    assert_eq!(SupervisorState::Idle.as_str(), "idle");
    assert_eq!(SupervisorState::Armed.as_str(), "armed");
    assert_eq!(SupervisorState::Recording.as_str(), "recording");
    assert_eq!(SupervisorState::Marked.as_str(), "marked");
    assert_eq!(SupervisorState::Closed.as_str(), "closed");
    assert_eq!(SupervisorState::Failed.as_str(), "failed");
}

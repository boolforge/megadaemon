//! Arnés de pruebas del camino de captura de audio y de las banderas de CSC.
//!
//! `src/lib.rs` es intocable en esta tarea: el módulo se incluye con
//! `#[path]` para poder compilar este arnés de forma aislada.

use megadaemon_core::call_recorder;

use megadaemon_core::rom_migration;

use megadaemon_core::call_recorder::*;
use megadaemon_core::rom_migration::{preconfig_install_plan, preconfig_state};

#[test]
fn las_fuentes_de_captura_declaran_bien_el_permiso() {
    assert!(!CaptureSource::Mic.requires_capture_audio_output());
    assert!(!CaptureSource::RemoteSubmix.requires_capture_audio_output());
    for s in [CaptureSource::VoiceCall, CaptureSource::VoiceUplink, CaptureSource::VoiceDownlink] {
        assert!(s.requires_capture_audio_output(), "{}", s.as_str());
        assert!(s.captures_call_audio());
    }
    // Valores de android.media.MediaRecorder.AudioSource
    assert_eq!(CaptureSource::Mic.android_source(), 1);
    assert_eq!(CaptureSource::VoiceUplink.android_source(), 2);
    assert_eq!(CaptureSource::VoiceDownlink.android_source(), 3);
    assert_eq!(CaptureSource::VoiceCall.android_source(), 4);
    assert_eq!(CaptureSource::RemoteSubmix.android_source(), 8);
}

#[test]
fn la_captura_de_llamada_se_lanza_bajo_el_uid_de_sistema() {
    let plan = capture_launch_plan(CaptureSource::VoiceCall, "/data/local/tmp/md-rec");
    assert!(plan.needs_root);
    assert_eq!(plan.uid, SYSTEM_UID);
    assert!(plan.command.contains("su 1000"), "{}", plan.command);
    assert!(plan.command.contains("--source 4"));
    assert!(plan.notes.iter().any(|n| n.contains("silencio")));
}

#[test]
fn el_microfono_no_necesita_root_y_avisa_de_sus_limites() {
    let plan = capture_launch_plan(CaptureSource::Mic, "/data/local/tmp/md-rec");
    assert!(!plan.needs_root);
    assert_eq!(plan.uid, 0);
    assert!(!plan.command.contains("su "));
    assert!(plan.notes.iter().any(|n| n.contains("bloqueada")));
}

#[test]
fn la_politica_de_audio_sin_mixport_de_voz_no_necesita_parche() {
    // Política real medida en el S21 Ultra (extracto del fichero original).
    let xml = r#"<mixPort name="primary-out" role="source">
          <mixPort name="primary-in" role="sink">
          <devicePort tagName="Voice Call Mic" type="AUDIO_DEVICE_IN_VOICE_CALL" role="source">
          <route type="mix" sink="primary-in" sources="Built-In Mic,Voice Call Mic"/>"#;
    let f = audio_policy_facts("/vendor/etc/audio_policy_configuration.xml", xml);
    assert_eq!(f.mix_ports, vec!["primary-out", "primary-in"]);
    assert!(f.has_voice_call_device_port);
    assert!(!f.has_voice_call_source, "esta politica no declara fuentes VOICE_*");
    assert!(!f.has_call_capture_mix_port);
    assert!(!f.needs_policy_patch());
}

#[test]
fn una_politica_con_fuente_de_voz_sin_mixport_si_necesita_parche() {
    // Fuente de voz declarada, pero SIN mixport dedicado a la llamada:
    // habría que AÑADIR un mixport de captura (parche de política).
    let xml = r#"<mixPort name="primary-in" role="sink">
                 <inputProfile><inputStream tagName="VOICE_CALL" source="AUDIO_SOURCE_VOICE_CALL"/>"#;
    let f = audio_policy_facts("/vendor/etc/audio_policy_configuration.xml", xml);
    assert!(f.has_voice_call_source);
    assert!(!f.has_call_capture_mix_port);
    assert!(f.needs_policy_patch());
}

#[test]
fn una_politica_con_mixport_de_llamada_no_necesita_parche() {
    let xml = r#"<mixPort name="call_recorder" role="sink">
                 <inputProfile><inputStream tagName="VOICE_CALL" source="AUDIO_SOURCE_VOICE_CALL"/>"#;
    let f = audio_policy_facts("/vendor/etc/audio_policy_configuration.xml", xml);
    assert!(f.has_call_capture_mix_port);
    assert!(!f.needs_policy_patch());
}

#[test]
fn los_flags_de_csc_se_parsean_en_los_dos_formatos() {
    let kv = "CscFeature_VoiceCall_ConfigRecording=1;CscFeature_VoiceCall_SupportRTT=0";
    let f = parse_csc_features(kv);
    assert_eq!(voice_call_config_recording(&f), Some(true));

    let xml = r#"<CscFeature name="CscFeature_VoiceCall_ConfigRecording" value="0"/>"#;
    let f2 = parse_csc_features(xml);
    assert_eq!(voice_call_config_recording(&f2), Some(false));

    let f3 = parse_csc_features("otra_cosa=1");
    assert_eq!(voice_call_config_recording(&f3), None);
}

#[test]
fn las_rutas_de_csc_apuntan_a_donde_estan_en_el_samsung() {
    let c = csc_feature_candidates("AMO");
    assert!(c.iter().any(|p| p.contains("/prism/etc/carriers/single/AMO/contents.db")));
    assert!(c.iter().any(|p| p.contains("/optics/configs/carriers/single/AMO/conf")));
    assert!(c.iter().all(|p| !p.contains("{csc}")));
}

#[test]
fn el_plan_de_preconfig_es_jerarquico_y_termina_en_la_alternativa_real() {
    let s = preconfig_state(false, false);
    assert!(!s.usable());
    let plan = preconfig_install_plan(&s);
    assert!(plan[0].contains("firmware Samsung"));
    assert!(plan.last().unwrap().contains("AT+PRECONFG"));
    let s2 = preconfig_state(false, true);
    let plan2 = preconfig_install_plan(&s2);
    assert!(plan2[0].contains("install-existing"));
}


#[test]
fn el_envoltorio_omc_se_distingue_del_texto_y_del_gzip() {
    // Medido: el cscfeature.xml de AMO empieza por 2f2729b7... y no es gzip
    let bytes = [0x2fu8, 0x27, 0x29, 0xb7, 0xda, 0xc1, 0xac, 0x37, 0x01, 0x02];
    match omc_state(&bytes) {
        OmcState::Obfuscated { magic, .. } => assert!(magic.starts_with("2f2729b7")),
        otro => panic!("esperaba ofuscado, {:?}", otro),
    }
    assert_eq!(omc_state(b"<CscFeature name=\"CscFeature_VoiceCall_ConfigRecording\" value=\"1\"/>"), OmcState::Plaintext);
    assert_eq!(omc_state(&[0x1f, 0x8b, 0x08, 0x00]), OmcState::Gzip);
}

#[test]
fn el_veredicto_de_audio_es_honesto_y_prioriza_el_csc() {
    let f = CallAudioFacts {
        hal_has_voice_path: false,
        apps_with_capture_permission: vec![
            "YourPhone_P1_5".to_string(),
            "LinkToWindowsService".to_string(),
        ],
        csc_allows_native: false,
        system_uid_available: true,
    };
    let c = f.conclusion();
    assert!(c.contains("cambiarlo"), "{}", c);
    assert!(c.contains("modem"), "{}", c);
    assert!(c.contains("YourPhone_P1_5"), "{}", c);
}

#[test]
fn las_rutas_de_csc_incluyen_el_cscfeature_xml_ofuscado() {
    let c = csc_feature_candidates("AMO");
    assert!(c[0].ends_with("/conf/system/cscfeature.xml"), "{:?}", c);
}

#[test]
fn los_pcm_de_voz_son_los_wdma() {
    assert_eq!(ALSA_WDMA_DEVICES[0], "hw:0,12");
    assert_eq!(ALSA_WDMA_DEVICES.len(), 8);
    let p = alsa_voice_capture_plan("hw:0,12", 5);
    assert!(p.command.contains("md_alsa"));
    assert!(p.command.contains("hw:0,12"));
    assert!(p.notes.iter().any(|n| n.contains("silencio")));
}

#[test]
fn el_riesgo_del_cambio_de_csc_es_explicito() {
    let r = csc_switch_risk(SIM_OPERATOR_NUMERIC, "ILO");
    assert!(r.contains("ILO"));
    assert!(r.contains("21403"));
    assert!(r.contains("reversible"));
    let v = csc_switch_verify_plan("ILO");
    assert!(v[0].contains("ro.csc.sales_code"));
    assert!(v.iter().any(|x| x.contains("CscFeature_VoiceCall_ConfigRecording")));
}

#[test]
fn los_csc_de_grabacion_disponibles_estan_medidos() {
    assert!(RECORDING_CSC_AVAILABLE.contains(&"THL"));
    assert!(RECORDING_CSC_AVAILABLE.contains(&"ILO"));
    assert!(RECORDING_CSC_AVAILABLE.contains(&"XXV"));
    assert!(!RECORDING_CSC_AVAILABLE.contains(&"INS"), "INS no esta en este firmware");
}

//! Arnés de pruebas del gestor de grabaciones (estilo BCR-GUI).

use megadaemon_core::drive_client;
use megadaemon_core::recording_vault;

use megadaemon_core::recording_vault::*;

fn árbol(dir: &str, name: &str, contenido: &[u8]) {
    let p = std::path::Path::new(dir).join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, contenido).unwrap();
}

#[test]
fn se_reconocen_los_ficheros_de_audio_y_de_texto() {
    assert!(is_recording_file("grabacion_20231114_101500.m4a"));
    assert!(is_recording_file("llamada.MP4"));
    assert!(!is_recording_file("notas.txt"));
    assert!(is_transcript_file("grabacion.srt"));
    assert!(!is_transcript_file("grabacion.m4a"));
}

#[test]
fn el_inventario_asocia_la_transcripcion_y_calcula_el_hash() {
    let dir = std::env::temp_dir().join(format!("md-rec-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = dir.to_string_lossy().to_string();
    árbol(&d, "2023-11-14/grabacion_101500.m4a", b"AUDIO DE PRUEBA");
    árbol(&d, "2023-11-14/grabacion_101500.srt", b"1\nhola\n");
    let items = scan_recordings(&d).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].size, 15);
    assert_eq!(items[0].transcript.as_deref(), Some("1\nhola\n"));
    // el sha256 es el del contenido real
    assert_eq!(items[0].sha256, sha256_file(&items[0].path).unwrap());
    assert_eq!(items[0].sha256.len(), 64);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_pack_del_vault_usa_carpetas_por_dia() {
    let dir = std::env::temp_dir().join(format!("md-rec2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = dir.to_string_lossy().to_string();
    árbol(&d, "g.m4a", b"AUDIO");
    árbol(&d, "g.txt", b"texto");
    let items = scan_recordings(&d).unwrap();
    let pack = vault_pack(&items);
    assert!(pack.iter().any(|e| e.dest.ends_with(".m4a")), "{:?}", pack);
    assert!(pack.iter().any(|e| e.dest.ends_with(".txt")), "{:?}", pack);
    assert!(pack.iter().all(|e| e.dest.starts_with("grabaciones/")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn la_fecha_se_convierte_a_dia_utc() {
    // 2023-11-14T22:13:20Z
    let items = vec![RecordingMeta {
        path: "/x.m4a".into(),
        name: "x.m4a".into(),
        size: 1,
        sha256: "a".repeat(64),
        modified: Some(1_700_000_000),
        transcript: None,
    }];
    let pack = vault_pack(&items);
    assert!(pack[0].dest.contains("2023-11-14"), "{}", pack[0].dest);
}

#[test]
fn el_plan_de_transcripcion_dice_que_enviar() {
    let r = RecordingMeta {
        path: "/data/media/0/grab.m4a".into(),
        name: "grab.m4a".into(),
        size: 10,
        sha256: "b".repeat(64),
        modified: None,
        transcript: None,
    };
    let p = transcription_plan(
        &r,
        &TranscriptionBackend::OpenAiCompat {
            endpoint: "https://api.example/v1".into(),
            model: "whisper-1".into(),
        },
    );
    assert!(p.iter().any(|l| l.contains("/v1/audio/transcriptions")), "{:?}", p);
    assert!(p.iter().any(|l| l.contains("whisper-1")));
    assert!(p.iter().any(|l| l.contains("/data/media/0/grab.m4a")));
    let n = transcription_plan(&r, &TranscriptionBackend::None);
    assert!(n[0].contains("Sin transcripción"));
}

#[test]
fn el_plan_de_subida_cubre_el_fichero_entero() {
    let items = vec![RecordingMeta {
        path: "/g.m4a".into(),
        name: "g.m4a".into(),
        size: 3_000_000,
        sha256: "c".repeat(64),
        modified: None,
        transcript: None,
    }];
    let plan = drive_upload_plan(&items, "FOLDER", 256 * 1024);
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].size, 3_000_000);
    assert!(plan[0].chunks.len() > 1);
    assert_eq!(plan[0].chunks[0].start, 0);
    assert_eq!(plan[0].chunks.last().unwrap().end, 2_999_999);
}

#[test]
fn el_informe_del_vault_cuenta_y_fija_el_manifiesto() {
    let mk = |h: &str, size: u64, tr: bool| RecordingMeta {
        path: format!("/{h}.m4a"),
        name: format!("{h}.m4a"),
        size,
        sha256: h.repeat(64),
        modified: None,
        transcript: if tr { Some("x".into()) } else { None },
    };
    let items = vec![mk("a", 10, true), mk("b", 20, false)];
    let r = vault_report(&items);
    assert_eq!(r.count, 2);
    assert_eq!(r.total_bytes, 30);
    assert_eq!(r.with_transcript, 1);
    assert_eq!(r.manifest_sha256.len(), 64);
    // el manifiesto depende del orden (hash de los hashes)
    let rotado: Vec<RecordingMeta> = items[1..].iter().chain(items[..1].iter()).cloned().collect();
    let r2 = vault_report(&rotado);
    assert_ne!(r.manifest_sha256, r2.manifest_sha256);
}

//! Arnés de pruebas del cliente de Drive en streaming.

#[path = "../src/drive_client.rs"]
mod drive_client;

use drive_client::*;

fn creds() -> DriveCredentials {
    DriveCredentials {
        access_token: "ya29.TOKEN-DE-PRUEBA".to_string(),
    }
}

#[test]
fn el_plan_de_trozos_cubre_el_fichero_sin_huecos() {
    for (size, chunk) in [(0u64, 1u64), (1, 1), (1024, 256 * 1024), (5_000_000, 1_000_000), (3_000_000_000, 8 * 1024 * 1024)] {
        let plan = chunk_plan(size, chunk);
        if size == 0 {
            assert!(plan.is_empty());
            continue;
        }
        assert_eq!(plan[0].start, 0);
        assert_eq!(plan.last().unwrap().end, size - 1);
        let mut expect = 0u64;
        for r in &plan {
            assert_eq!(r.start, expect, "sin huecos");
            expect = r.end + 1;
        }
        // todos menos el ultimo son multiplos de 256 KiB (lo exige Google)
        for r in plan.iter().take(plan.len().saturating_sub(1)) {
            assert_eq!(r.len() % DRIVE_CHUNK_GRANULARITY, 0, "trozo no alineado");
        }
    }
}

#[test]
fn la_sesion_de_subida_lleva_metadatos_y_autenticacion() {
    let r = open_upload_session(&creds(), "backup.tar", 1234, &DriveTarget::folder("FOLDER"));
    assert_eq!(r.method, "POST");
    assert!(r.url.contains("uploadType=resumable"));
    assert!(r.headers.iter().any(|(k, v)| k == "Authorization" && v.starts_with("Bearer ")));
    assert!(r.headers.iter().any(|(k, v)| k == "X-Upload-Content-Length" && v == "1234"));
    let body = r.body.unwrap();
    assert!(body.contains("\"name\":\"backup.tar\""));
    assert!(body.contains("\"parents\":[\"FOLDER\"]"));
    assert!(body.contains("application/octet-stream"));
}

#[test]
fn cada_trozo_envia_content_range_correcto() {
    let plan = chunk_plan(700_000, 256 * 1024);
    let last = plan.last().unwrap();
    let r = upload_chunk_request(&creds(), "https://session/uri", last, 700_000);
    let cr = r.headers.iter().find(|(k, _)| k == "Content-Range").unwrap();
    assert_eq!(cr.1, format!("bytes {}-{}/700000", last.start, last.end));
    assert_eq!(r.method, "PUT");
    assert_eq!(r.url, "https://session/uri");
}

#[test]
fn la_descarga_pide_alt_media_y_la_carpeta_se_crea_con_padre() {
    let d = download_request(&creds(), "FILEID");
    assert!(d.url.contains("/drive/v3/files/FILEID"));
    assert!(d.url.contains("alt=media"));
    let f = create_folder_request(&creds(), "backups", &DriveTarget::folder("PARENT"));
    let fb = f.body.unwrap();
    assert!(fb.contains("application/vnd.google-apps.folder"));
    assert!(fb.contains("\"parents\":[\"PARENT\"]"));
}

#[test]
fn el_comando_curl_no_escribe_ningun_fichero_intermedio() {
    let r = upload_chunk_request(&creds(), "https://session/uri", &DriveChunkRange { start: 0, end: 1023 }, 2048);
    let cmd = r.curl_command(Some("/data/media/0/foto.mp4"));
    assert!(cmd.contains("/data/media/0/foto.mp4"));
    assert!(!cmd.contains("> /"));           // no redirige a ningun sitio
    assert!(cmd.contains("Content-Range: bytes 0-1023/2048"));
    assert!(cmd.contains("-X PUT"));
}

#[test]
fn el_hash_se_verifica_y_falla_si_no_cuadra() {
    let dir = std::env::temp_dir().join(format!("md-drive-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("datos.bin");
    std::fs::write(&path, b"contenido de prueba").unwrap();
    let good = sha256_chunk(b"contenido de prueba");
    let p = path.to_string_lossy().to_string();
    assert_eq!(verify_file_sha256(&p, &good).unwrap(), good);
    let err = verify_file_sha256(&p, &"0".repeat(64)).unwrap_err();
    assert!(err.contains("no cuadra"), "{}", err);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn la_descarga_escribe_y_verifica_el_hash_en_la_misma_pasada() {
    let dir = std::env::temp_dir().join(format!("md-dl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("descarga.bin");
    let datos = vec![7u8; 300_000];
    let r = download_to_file(&datos[..], &path).unwrap();
    assert_eq!(r.bytes, 300_000);
    assert_eq!(r.sha256, sha256_chunk(&datos));
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 300_000);
    let (n, h2) = sha256_reader(&datos[..]).unwrap();
    assert_eq!(n, 300_000);
    assert_eq!(h2, r.sha256);
    // el hash calculado coincide con el del fichero ya escrito
    assert_eq!(verify_file_sha256(&path.to_string_lossy(), &r.sha256).is_ok(), true);
    let _ = std::fs::remove_dir_all(&dir);
}

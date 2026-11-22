//! Smoke del respaldo de PERFIL: plan, exclusiones, streaming y verificación.
//!
//! **Determinista**: todo ocurre sobre un directorio temporal que imita la
//! estructura de `/data` del móvil. Ni red, ni móvil, ni root.

use megadaemon_core::backup_profile as bp;
use megadaemon_core::backup_stream as bs;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "md_profile_test_{}_{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(path: &Path, data: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, data).unwrap();
}

/// Monta un `/data` de mentira con lo que importa.
fn fake_device(root: &Path) {
    // Una app con shared_prefs, files y caché.
    write(
        &root.join("data/user/0/com.ejemplo.app/shared_prefs/app.xml"),
        b"<prefs/>",
    );
    write(
        &root.join("data/user/0/com.ejemplo.app/files/dato.txt"),
        b"hola",
    );
    write(
        &root.join("data/user/0/com.ejemplo.app/cache/regenerable.bin"),
        &[0u8; 4096],
    );
    // Otra app con un .p12 del usuario.
    write(
        &root.join("data/user/0/com.banco.app/files/certs/cliente.p12"),
        b"P12",
    );
    // Configuración del sistema.
    write(&root.join("data/system/packages.xml"), b"<packages/>");
    write(&root.join("data/system/blobstore/blob1"), &[0u8; 2048]);
    // Keychain y keystore.
    write(&root.join("data/misc/keychain/pins"), b"pins");
    write(&root.join("data/misc/keystore/persistent.sqlite"), b"sqlite");
    // Chrome.
    write(
        &root.join("data/user/0/com.android.chrome/app_chrome/Default/Bookmarks"),
        b"{}",
    );
    write(
        &root.join("data/user/0/com.android.chrome/app_chrome/Default/Login Data"),
        b"login",
    );
    write(
        &root.join("data/user/0/com.android.chrome/app_chrome/Default/Cookies"),
        b"cookies",
    );
    write(
        &root.join("data/user/0/com.android.chrome/app_chrome/Default/History"),
        b"history",
    );
    write(
        &root.join("data/user/0/com.android.chrome/app_chrome/Default/Preferences"),
        b"prefs",
    );
    write(
        &root.join("data/user/0/com.android.chrome/app_chrome/Default/Extensions/abc/ext.js"),
        b"ext",
    );
    // Firefox.
    write(
        &root.join("data/user/0/org.mozilla.fenix/files/mozilla/abc.default/places.sqlite"),
        b"places",
    );
    write(
        &root.join("data/user/0/org.mozilla.fenix/files/mozilla/abc.default/prefs.js"),
        b"user_pref",
    );
    // KDE Connect.
    write(
        &root.join("data/user/0/org.kde.kdeconnect_tp/shared_prefs/kdeconnect.plist"),
        b"plist",
    );
}

#[test]
fn el_plan_entra_lo_que_debe_y_explica_el_por_que() {
    let d = tmp("plan");
    fake_device(&d);
    let policy = bp::ProfilePolicy::for_profile(bp::BackupProfile::P1Profile);
    let plan = bp::plan(d.to_str().unwrap(), &policy);

    // Las preferencias de la app están.
    assert!(plan
        .included()
        .any(|e| e.rel_path == "profile/apps/com.ejemplo.app/shared_prefs/app.xml"));
    // La caché NO (es regenerable) y se dice por qué.
    let cache = plan
        .entries
        .iter()
        .find(|e| e.path.ends_with("cache/regenerable.bin"))
        .expect("la caché debe aparecer en el plan (aunque fuera)");
    assert!(!cache.included);
    assert!(cache.reason.contains("caché"));
    // El blobstore de /data/system queda fuera con su motivo.
    let blob = plan
        .entries
        .iter()
        .find(|e| e.path.ends_with("blobstore/blob1"))
        .expect("el blobstore debe aparecer en el plan");
    assert!(!blob.included);
    assert!(blob.reason.contains("blobstore"));
    // Los certificados del usuario sí.
    assert!(plan
        .included()
        .any(|e| e.rel_path == "profile/certs/user/com.banco.app/cliente.p12"));
    // El keychain y el keystore también.
    assert!(plan.included().any(|e| e.rel_path.contains("certs/keychain")));
    assert!(plan.included().any(|e| e.rel_path.contains("certs/keystore")));
    // Chrome: marcadores, contraseñas, cookies, historial, preferencias y extensiones.
    for f in [
        "Bookmarks",
        "Login Data",
        "Cookies",
        "History",
        "Preferences",
    ] {
        assert!(
            plan.included()
                .any(|e| e.rel_path.contains(&format!("browser/com.android.chrome/app_chrome/Default/{f}"))),
            "falta {f}"
        );
    }
    assert!(plan
        .included()
        .any(|e| e.rel_path.contains("browser/com.android.chrome/app_chrome/Default/Extensions/abc/ext.js")));
    // Firefox (perfil de nombre aleatorio).
    assert!(plan
        .included()
        .any(|e| e.rel_path.contains("browser/org.mozilla.fenix/abc.default/places.sqlite")));
    // KDE Connect.
    assert!(plan.included().any(|e| e.rel_path.contains("kdeconnect")));

    // El informe JSON cuadra con el plan.
    let j = plan.to_json();
    assert_eq!(j["profile"], "P1");
    assert!(j["included_bytes"].as_u64().unwrap() > 0);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn la_lista_blanca_de_paquetes_filtra() {
    let d = tmp("wl");
    fake_device(&d);
    let mut policy = bp::ProfilePolicy::for_profile(bp::BackupProfile::P1Profile);
    policy.include_packages = vec!["com.ejemplo.app".to_string()];
    let plan = bp::plan(d.to_str().unwrap(), &policy);
    assert!(plan
        .included()
        .any(|e| e.package == "com.ejemplo.app"));
    // Los navegadores no están en la lista blanca ⇒ fuera, con motivo.
    let chrome = plan
        .entries
        .iter()
        .find(|e| e.package == "com.android.chrome")
        .expect("chrome debe aparecer");
    assert!(!chrome.included);
    assert!(chrome.reason.contains("lista blanca"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_exclusion_por_patron_es_configurable() {
    let d = tmp("pat");
    fake_device(&d);
    let mut policy = bp::ProfilePolicy::for_profile(bp::BackupProfile::P1Profile);
    policy.exclude_patterns.push("/shared_prefs/".to_string());
    let plan = bp::plan(d.to_str().unwrap(), &policy);
    let sp = plan
        .entries
        .iter()
        .find(|e| e.rel_path.ends_with("shared_prefs/app.xml"))
        .expect("debe aparecer");
    assert!(!sp.included);
    assert!(sp.reason.contains("patrón"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn el_stream_genera_un_tar_reconstruible_y_verificable() {
    let d = tmp("stream");
    fake_device(&d);
    let out = tmp("stream-out");
    let policy = bp::ProfilePolicy::for_profile(bp::BackupProfile::P1Profile);
    let plan = bp::plan(d.to_str().unwrap(), &policy);

    // Destino local (sin red).
    let sink = Arc::new(bs::FileSink::new(&out, "perfil").unwrap());
    // El total se calcula como el TAR exacto (así lo exige Drive).
    let files = bp::hash_files(&plan).unwrap();
    let manifest_json = serde_json::to_string(&files).unwrap();
    let sizes: Vec<u64> = files.iter().map(|f| f.size).collect();
    let total = bp::tar_total_bytes(manifest_json.len() as u64, &sizes);

    let mut pipeline =
        bs::Pipeline::new(sink, "perfil", total, 256 * 1024, 2, BTreeMap::new()).unwrap();
    let mut manifest = bp::stream_to_drive(&plan, &mut pipeline, 256 * 1024).unwrap();
    let summary = pipeline.finish().unwrap();
    assert_eq!(summary.bytes, total);
    assert_eq!(manifest.total_bytes, total);

    // Se reensambla y se comprueba que están todos los ficheros con su hash.
    manifest.parts = vec![
        bp::PartRecord {
            stream: 0,
            name: "perfil.part00".into(),
            file_id: "x".into(),
            bytes: 0,
        },
        bp::PartRecord {
            stream: 1,
            name: "perfil.part01".into(),
            file_id: "y".into(),
            bytes: 0,
        },
    ];
    let tar = bp::reassemble(&out, &manifest).unwrap();
    assert_eq!(tar.len() as u64, total);
    let extracted = bp::untar(&tar).unwrap();
    for f in &manifest.files {
        let found = extracted.iter().find(|(p, _)| *p == f.path);
        assert!(found.is_some(), "falta {} en el TAR", f.path);
        assert_eq!(found.unwrap().1, f.sha256, "hash distinto de {}", f.path);
    }
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn la_verificacion_del_stream_detecta_una_corrupcion() {
    let d = tmp("verify");
    fake_device(&d);
    let out = tmp("verify-out");
    let policy = bp::ProfilePolicy::for_profile(bp::BackupProfile::P1Profile);
    let plan = bp::plan(d.to_str().unwrap(), &policy);
    let sink = Arc::new(bs::FileSink::new(&out, "perfil").unwrap());
    let files = bp::hash_files(&plan).unwrap();
    let manifest_json = serde_json::to_string(&files).unwrap();
    let sizes: Vec<u64> = files.iter().map(|f| f.size).collect();
    let total = bp::tar_total_bytes(manifest_json.len() as u64, &sizes);
    let mut pipeline =
        bs::Pipeline::new(sink, "perfil", total, 256 * 1024, 1, BTreeMap::new()).unwrap();
    let mut manifest = bp::stream_to_drive(&plan, &mut pipeline, 256 * 1024).unwrap();
    let _ = pipeline.finish().unwrap();
    manifest.parts = vec![bp::PartRecord {
        stream: 0,
        name: "perfil.part00".into(),
        file_id: "x".into(),
        bytes: 0,
    }];
    // Correcto: todo cuadra.
    let v = bp::verify_stream(&out, &manifest).unwrap();
    assert!(v.ok, "debería cuadrar: {:?}", v.chunks_bad);
    // Corrupto: se toca un byte del tramo.
    let part = out.join("part00");
    let mut data = std::fs::read(&part).unwrap();
    data[0] ^= 0xFF;
    std::fs::write(&part, &data).unwrap();
    let v2 = bp::verify_stream(&out, &manifest).unwrap();
    assert!(!v2.ok);
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&out);
}

use std::sync::Arc;

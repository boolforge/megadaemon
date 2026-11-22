//! Tests del motor de **backup nativo de apps** (`md-backup`).
//!
//! Todos son deterministas, **sin red** y **sin dispositivo**: se usa un doble
//! de móvil (un directorio + un `pm` falso en el `PATH`).

use megadaemon_core::backup_apps::{
    self, CreateOptions, Governor, InventoryOptions, RestoreOptions, SegmentSink,
};
use megadaemon_core::backup_crypto::sha256_hex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// El doble de móvil (raíz + `pm` falso). Compartido por todos los tests.
struct FakePhone {
    root: PathBuf,
    base: PathBuf,
}

fn phone() -> &'static FakePhone {
    static PHONE: OnceLock<FakePhone> = OnceLock::new();
    PHONE.get_or_init(|| {
        let base = std::env::temp_dir().join(format!("md-backup-smoke-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("root");
        let bin = base.join("bin");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&bin).unwrap();

        // `pm` falso: responde como el del S21 Ultra (formato medido
        // `package:<apk>=<pkg> uid:<uid>`).
        let pm = format!(
            r#"#!/bin/sh
ROOT="{root}"
if [ "$1" = "list" ] && [ "$2" = "packages" ]; then
  case "$3" in
    -s) exit 0 ;;
    -d) exit 0 ;;
    -f) echo "package:$ROOT/data/app/com.test.app/base.apk=com.test.app uid:10123"
        echo "package:$ROOT/data/app/com.test.other/base.apk=com.test.other uid:10124"
        exit 0 ;;
    -u) echo "package:com.test.app uid:10123"
        echo "package:com.test.other uid:10124"
        exit 0 ;;
  esac
fi
if [ "$1" = "path" ]; then
  case "$2" in
    com.test.app) echo "package:$ROOT/data/app/com.test.app/base.apk"; exit 0 ;;
    com.test.other) echo "package:$ROOT/data/app/com.test.other/base.apk"; exit 0 ;;
  esac
fi
exit 1
"#,
            root = root.display()
        );
        let pm_path = bin.join("pm");
        fs::write(&pm_path, pm).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = fs::metadata(&pm_path).unwrap().permissions();
            p.set_mode(0o755);
            fs::set_permissions(&pm_path, p).unwrap();
        }

        // Datos de las apps.
        let write = |rel: &str, body: &[u8]| {
            let p = root.join(rel.trim_start_matches('/'));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        };
        write("/data/user/0/com.test.app/databases/app.db", b"contenido-db-1");
        write("/data/user/0/com.test.app/shared_prefs/prefs.xml", b"<prefs/>");
        write("/data/user/0/com.test.app/cache/tmp.bin", b"basura-cache");
        write("/data/user/0/com.test.app/code_cache/x.bin", b"basura-code-cache");
        write("/data/user/0/com.test.app/no_backup/y.bin", b"basura-no-backup");
        write("/data/user/0/com.test.other/files/other.txt", b"contenido-other");
        write("/data/app/com.test.app/base.apk", b"APK-FALSO-1");
        write("/data/app/com.test.other/base.apk", b"APK-FALSO-2");

        // El `PATH` apunta al `pm` falso (proceso entero: todos los tests
        // comparten el mismo doble).
        let old = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{}:{}", bin.display(), old));

        FakePhone { root, base }
    })
}

/// Cuenta los ficheros de un árbol (para probar que el inventario no escribe).
fn count_files(dir: &Path) -> usize {
    let mut n = 0usize;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(rd) = fs::read_dir(&p) else {
            continue;
        };
        for e in rd.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                stack.push(e.path());
            } else {
                n += 1;
            }
        }
    }
    n
}

/// Un directorio de trabajo limpio por test.
fn work_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "md-backup-out-{}-{}-{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn base_options(out: &Path) -> CreateOptions {
    let ph = phone();
    CreateOptions {
        root: ph.root.clone(),
        out: out.to_path_buf(),
        packages: vec!["com.test.app".to_string()],
        segment_bytes: 64 * 1024,
        passphrase: Some("clave-de-prueba".to_string()),
        key_out: Some(out.with_extension("keys.json")),
        include_apk: true,
        include_data: true,
        include_external: false,
        allow_live_framework: false,
        include_media: false,
        exclude_patterns: Vec::new(),
        plan: None,
        dry_run: false,
        gov: Governor {
            enabled: false,
            ..Governor::default()
        },
    }
}


#[test]
fn el_ultimo_segmento_que_rota_no_panea() {
    // BUG REAL (medido el 2026-11-22 en el móvil): si el ÚLTIMO fichero hace
    // rotar el segmento (o sea: el tar queda sin fichero en curso), el
    // `finish()`aba con `unwrap()` sobre un `None` ⇒ **panic** ⇒ el backup
    // entero se perdía (con los segmentos ya subir a Drive por el
    // pipelining). Aquí se fuerza ese caso: segmento de 1 KiB y ficheros
    // que lo rebasan.
    let out = work_dir("rota");
    let ph = phone();
    let opts = CreateOptions {
        root: ph.root.clone(),
        out: out.clone(),
        packages: vec!["com.test.app".to_string()],
        segment_bytes: 1024,
        passphrase: Some("clave-de-prueba".to_string()),
        key_out: Some(out.with_extension("keys.json")),
        include_apk: true,
        include_data: true,
        include_external: false,
        allow_live_framework: false,
        include_media: false,
        exclude_patterns: Vec::new(),
        plan: None,
        dry_run: false,
        gov: Governor {
            enabled: false,
            ..Governor::default()
        },
    };
    let mut sink = backup_apps::NullSink;
    // Antes del fix esto entraba en panic.
    let rep = backup_apps::create(&opts, &mut sink).expect("create no debe panear");
    assert!(rep.files >= 3);
    // Y el contenedor tiene que ser verificable.
    let key = backup_apps::read_key_file(&out.with_extension("keys.json")).unwrap();
    let v = backup_apps::verify(&out, Some(&key)).unwrap();
    assert!(v.ok, "verify falló: {:?}", v);
    let _ = fs::remove_dir_all(&out);
}

// ---------------------------------------------------------------------------
// Inventario
// ---------------------------------------------------------------------------

#[test]
fn el_inventario_no_escribe_nada_en_disco() {
    let ph = phone();
    let before = count_files(&ph.root);
    let opts = InventoryOptions {
        root: ph.root.clone(),
        measure: true,
        ..Default::default()
    };
    let apps = backup_apps::inventory(&opts).unwrap();
    let after = count_files(&ph.root);
    assert_eq!(before, after, "el inventario escribió algo en el móvil");
    assert!(apps.iter().any(|a| a.package == "com.test.app"));
    let app = apps.iter().find(|a| a.package == "com.test.app").unwrap();
    // La medición excluye lo regenerable.
    let measured = app.data_bytes.unwrap();
    let solo_datos = "contenido-db-1".len() + "<prefs/>".len();
    assert_eq!(measured, solo_datos as u64, "se midió la basura regenerable");
    assert_eq!(app.file_count.unwrap(), 2);
}

#[test]
fn lo_regenerable_se_reconoce() {
    assert!(backup_apps::is_regenerable("/data/user/0/x/cache/f.bin"));
    assert!(backup_apps::is_regenerable("/data/user/0/x/code_cache/f.bin"));
    assert!(backup_apps::is_regenerable("/data/user/0/x/no_backup/f.bin"));
    assert!(backup_apps::is_regenerable("/data/dalvik-cache/f.dm"));
    assert!(!backup_apps::is_regenerable("/data/user/0/x/databases/app.db"));
}

#[test]
fn los_formatos_de_pm_se_parsean() {
    let texto = "package:/data/app/~~abc==/com.x-y==/base.apk=com.x uid:10342";
    let v = backup_apps::parse_pm_packages_f_u(texto);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].0, "com.x");
    assert_eq!(v[0].1, "/data/app/~~abc==/com.x-y==/base.apk");
    assert_eq!(v[0].2, 10342);
    let u = backup_apps::parse_pm_packages_u("package:com.x uid:1042");
    assert_eq!(u[0].0, "com.x");
    assert_eq!(u[0].1, 1042);
}

// ---------------------------------------------------------------------------
// create / verify
// ---------------------------------------------------------------------------

#[test]
fn el_contenedor_se_crea_y_se_verifica() {
    let out = work_dir("crea");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    let rep = backup_apps::create(&opts, &mut sink).unwrap();
    assert!(rep.files >= 3, "ficheros respaldados: {}", rep.files);
    assert!(out.join("container.json").exists());
    assert!(out.join("manifest.json").exists());
    assert!(out.join("segments").exists());

    let key = backup_apps::read_key_file(&out.with_extension("keys.json")).unwrap();
    let v = backup_apps::verify(&out, Some(&key)).unwrap();
    assert!(v.ok, "verify falló: {:?}", v);
    assert!(v.entries >= 3);
    assert!(v.mismatches.is_empty());
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn el_manifiesto_lleva_el_sha256_de_cada_fichero() {
    let out = work_dir("sha");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    backup_apps::create(&opts, &mut sink).unwrap();
    let manifest = backup_apps::read_manifest(&out).unwrap();
    let e = manifest
        .entries
        .iter()
        .find(|e| e.path.ends_with("databases/app.db"))
        .expect("no está el fichero de datos");
    assert_eq!(e.sha256, sha256_hex(b"contenido-db-1"));
    assert_eq!(e.size, 13);
    // La cabecera guarda el hash del manifiesto.
    let h = backup_apps::read_header(&out).unwrap();
    assert_eq!(h.manifest_sha256, manifest.hash());
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn lo_regenerable_no_se_copia() {
    let out = work_dir("regen");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    backup_apps::create(&opts, &mut sink).unwrap();
    let manifest = backup_apps::read_manifest(&out).unwrap();
    assert!(!manifest.entries.iter().any(|e| e.path.contains("/cache/")));
    assert!(!manifest.entries.iter().any(|e| e.path.contains("/code_cache/")));
    assert!(!manifest.entries.iter().any(|e| e.path.contains("/no_backup/")));
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn una_corrupcion_de_un_byte_se_detecta() {
    let out = work_dir("corrupto");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    backup_apps::create(&opts, &mut sink).unwrap();
    let key = backup_apps::read_key_file(&out.with_extension("keys.json")).unwrap();

    // Se corrompe un byte del primer segmento.
    let segs = fs::read_dir(out.join("segments")).unwrap();
    let seg = segs
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|e| e == "mds").unwrap_or(false))
        .expect("no hay segmentos");
    let mut data = fs::read(&seg).unwrap();
    let mid = data.len() / 2;
    data[mid] ^= 0xff;
    fs::write(&seg, &data).unwrap();

    let v = backup_apps::verify(&out, Some(&key)).unwrap();
    assert!(!v.ok, "la corrupción no se detectó");
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn sin_clave_no_se_abre_el_segmento() {
    let out = work_dir("sinclave");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    backup_apps::create(&opts, &mut sink).unwrap();
    let segs = fs::read_dir(out.join("segments")).unwrap();
    let seg = segs
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|e| e == "mds").unwrap_or(false))
        .expect("no hay segmentos");
    let data = fs::read(&seg).unwrap();
    let e = backup_apps::open_segment(&data, None).unwrap_err();
    assert!(e.contains("clave"), "mensaje: {e}");
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn la_clave_se_guarda_con_0600() {
    let out = work_dir("perm");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    backup_apps::create(&opts, &mut sink).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let md = fs::metadata(out.with_extension("keys.json")).unwrap();
        assert_eq!(md.permissions().mode() & 0o777, 0o600);
    }
    let _ = fs::remove_dir_all(&out);
}

// ---------------------------------------------------------------------------
// restore
// ---------------------------------------------------------------------------

#[test]
fn el_restore_por_defecto_es_simulacion_y_no_escribe_nada() {
    let out = work_dir("simul");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    backup_apps::create(&opts, &mut sink).unwrap();
    let key = backup_apps::read_key_file(&out.with_extension("keys.json")).unwrap();
    let dest = work_dir("dest-simul");
    let r = backup_apps::restore(&RestoreOptions {
        dir: out.clone(),
        dest: dest.clone(),
        packages: vec![],
        confirm: false,
        overwrite: false,
        key: Some(key),
    })
    .unwrap();
    assert!(r.dry_run);
    assert!(r.written == 0);
    assert_eq!(count_files(&dest), 0, "el simulacro escribió algo");
    let _ = fs::remove_dir_all(&out);
    let _ = fs::remove_dir_all(&dest);
}

#[test]
fn el_restore_con_confirm_escribe_y_el_sha256_cuadra() {
    let out = work_dir("restaura");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    backup_apps::create(&opts, &mut sink).unwrap();
    let key = backup_apps::read_key_file(&out.with_extension("keys.json")).unwrap();
    let dest = work_dir("dest");
    let r = backup_apps::restore(&RestoreOptions {
        dir: out.clone(),
        dest: dest.clone(),
        packages: vec![],
        confirm: true,
        overwrite: true,
        key: Some(key),
    })
    .unwrap();
    assert!(!r.dry_run);
    assert!(r.written >= 3, "escritos: {}", r.written);
    let p = dest.join("data/user/0/com.test.app/databases/app.db");
    assert!(p.exists(), "no se restauró el fichero de datos");
    assert_eq!(fs::read(&p).unwrap(), b"contenido-db-1");
    let _ = fs::remove_dir_all(&out);
    let _ = fs::remove_dir_all(&dest);
}

#[test]
fn el_restore_no_sobrescribe_sin_overwrite() {
    let out = work_dir("no-sobrescribe");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    backup_apps::create(&opts, &mut sink).unwrap();
    let key = backup_apps::read_key_file(&out.with_extension("keys.json")).unwrap();
    let dest = work_dir("dest2");
    let p = dest.join("data/user/0/com.test.app/databases/app.db");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, b"contenido-original").unwrap();
    let r = backup_apps::restore(&RestoreOptions {
        dir: out.clone(),
        dest: dest.clone(),
        packages: vec![],
        confirm: true,
        overwrite: false,
        key: Some(key),
    })
    .unwrap();
    assert!(r.skipped >= 1);
    assert_eq!(fs::read(&p).unwrap(), b"contenido-original");
    let _ = fs::remove_dir_all(&out);
    let _ = fs::remove_dir_all(&dest);
}

// ---------------------------------------------------------------------------
// Pipelining
// ---------------------------------------------------------------------------

/// Un consumidor que **apunta** lo que recibe y en qué momento.
#[derive(Clone)]
struct RecordingSink {
    received: Arc<Mutex<Vec<(u32, u64)>>>,
    /// En el momento de la entrega, ¿existía ya el manifiesto? (false ⇒ la
    /// entrega es durante la recogida, no al final).
    manifest_present_at_handoff: Arc<Mutex<Vec<bool>>>,
}

impl SegmentSink for RecordingSink {
    fn submit(&mut self, seg: &backup_apps::SealedSegment) -> Result<(), String> {
        self.received
            .lock()
            .unwrap()
            .push((seg.index, seg.info.plain_len));
        Ok(())
    }
    fn finish(
        &mut self,
        _h: &backup_apps::ContainerHeader,
        _m: &backup_apps::Manifest,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn el_pipelining_entrega_cada_segmento_antes_de_terminar() {
    let out = work_dir("pipelining");
    let mut opts = base_options(&out);
    // Segmentos pequeños ⇒ varios segmentos ⇒ varias entregas.
    opts.segment_bytes = 1024;
    let received = Arc::new(Mutex::new(Vec::new()));
    let present = Arc::new(Mutex::new(Vec::new()));
    let mut sink = RecordingSink {
        received: received.clone(),
        manifest_present_at_handoff: present.clone(),
    };
    backup_apps::create(&opts, &mut sink).unwrap();
    let r = received.lock().unwrap();
    assert!(r.len() >= 2, "segmentos entregados: {}", r.len());
    // Los índices van en orden y cada entrega corresponde a un segmento real.
    for (i, (idx, _len)) in r.iter().enumerate() {
        assert_eq!(*idx as usize, i, "los segmentos no se entregaron en orden");
    }
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn el_manifiesto_se_escribe_al_final() {
    // El manifiesto es el índice: no puede existir mientras se recoge.
    let out = work_dir("final");
    let mut opts = base_options(&out);
    opts.segment_bytes = 1024;
    let present = Arc::new(Mutex::new(Vec::<bool>::new()));
    let seen = Arc::new(Mutex::new(Vec::<bool>::new()));
    struct Probe {
        out: PathBuf,
        seen: Arc<Mutex<Vec<bool>>>,
    }
    impl SegmentSink for Probe {
        fn submit(&mut self, _seg: &backup_apps::SealedSegment) -> Result<(), String> {
            self.seen.lock().unwrap().push(self.out.join("manifest.json").exists());
            Ok(())
        }
        fn finish(
            &mut self,
            _h: &backup_apps::ContainerHeader,
            _m: &backup_apps::Manifest,
        ) -> Result<(), String> {
            Ok(())
        }
    }
    let mut probe = Probe {
        out: out.clone(),
        seen: seen.clone(),
    };
    backup_apps::create(&opts, &mut probe).unwrap();
    let s = seen.lock().unwrap();
    assert!(!s.is_empty());
    assert!(
        s.iter().all(|x| !*x),
        "el manifiesto existía antes de terminar la recogida"
    );
    let _ = fs::remove_dir_all(&out);
}

// ---------------------------------------------------------------------------
// Reanudación
// ---------------------------------------------------------------------------

/// Consumidor que falla en el segundo segmento (simula un corte).
struct FailingSink {
    fail_at: u32,
}

impl SegmentSink for FailingSink {
    fn submit(&mut self, seg: &backup_apps::SealedSegment) -> Result<(), String> {
        if seg.index >= self.fail_at {
            return Err("corte simulado".to_string());
        }
        Ok(())
    }
    fn finish(
        &mut self,
        _h: &backup_apps::ContainerHeader,
        _m: &backup_apps::Manifest,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn tras_un_corte_el_backup_continua_y_verifica() {
    let out = work_dir("reanuda");
    let opts = base_options(&out);
    let mut sink = FailingSink { fail_at: 1 };
    let e = backup_apps::create(&opts, &mut sink).unwrap_err();
    assert!(e.contains("corte simulado"), "mensaje: {e}");
    // El primer segmento se quedó escrito y journaleado.
    let segs: Vec<_> = fs::read_dir(out.join("segments"))
        .unwrap()
        .filter_map(|x| x.ok())
        .map(|x| x.path())
        .collect();
    assert!(!segs.is_empty(), "no quedó ningún segmento");

    // Segunda pasada: continúa y termina.
    let mut sink2 = backup_apps::NullSink;
    let rep = backup_apps::create(&opts, &mut sink2).unwrap();
    assert!(rep.files >= 3);
    let key = backup_apps::read_key_file(&out.with_extension("keys.json")).unwrap();
    let v = backup_apps::verify(&out, Some(&key)).unwrap();
    assert!(v.ok, "verify falló tras reanudar: {:?}", v);
    let _ = fs::remove_dir_all(&out);
}

// ---------------------------------------------------------------------------
// Anti-congelación
// ---------------------------------------------------------------------------

#[test]
fn el_gobierno_descansa_cuando_la_carga_se_dispara() {
    let gov = Governor {
        enabled: true,
        load_high: 1.0,
        load_resume: 0.5,
        max_pause_s: 5,
        sleep_ms: 0,
        ..Governor::default()
    };
    // Fuente de carga falsa: siempre alta ⇒ el gobierno tiene que esperar.
    let t0 = std::time::Instant::now();
    let info = gov.tick_with("20.00 19.00 19.00 1/100 1", || "20.00 19.00 19.00 1/100 1".to_string());
    assert!(info.paused, "no descansó con carga 20");
    assert!(info.pause_ms >= 1000, "esperó {} ms", info.pause_ms);
    assert!(t0.elapsed().as_millis() >= 900);
    assert!(info.reason.contains("carga"), "motivo: {}", info.reason);
}

#[test]
fn el_gobierno_no_descansa_si_la_carga_es_normal() {
    let gov = Governor {
        enabled: true,
        load_high: 8.0,
        sleep_ms: 0,
        ..Governor::default()
    };
    let info = gov.tick_with("0.50 0.60 0.70 1/100 1", || "0.50 0.60 0.70 1/100 1".to_string());
    assert!(!info.paused);
}

#[test]
fn el_gobierno_nunca_compite_con_el_daemon() {
    // El daemon va a nice 0: el backup debe quedar SIEMPRE por debajo.
    let gov = Governor {
        enabled: true,
        nice: 0,
        ..Governor::default()
    };
    let note = gov.apply();
    assert!(note.contains("nice=10"), "nota: {note}");
    let gov2 = Governor {
        enabled: true,
        nice: 19,
        ..Governor::default()
    };
    assert!(gov2.apply().contains("nice=19"));
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

#[test]
fn list_los_contenedores() {
    let out = work_dir("list");
    let opts = base_options(&out);
    let mut sink = backup_apps::NullSink;
    backup_apps::create(&opts, &mut sink).unwrap();
    let items = backup_apps::list(&out).unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].complete);
    assert!(items[0].files >= 3);
    assert_eq!(items[0].packages, vec!["com.test.app".to_string()]);
    let _ = fs::remove_dir_all(&out);
}

// ---------------------------------------------------------------------------
// Paridad: MCP y agent_coding
// ---------------------------------------------------------------------------

#[test]
fn la_tool_mcp_phone_backup_exige_confirm() {
    use megadaemon_core::mcp_ext;
    let v = mcp_ext::dispatch(
        "phone_backup",
        &serde_json::json!({ "action": "create", "out": "/tmp/no-debe-escribirse" }),
    )
    .expect("la tool no existe");
    assert_eq!(v["isError"], serde_json::json!(true));
    let txt = v["content"][0]["text"].as_str().unwrap_or("");
    assert!(txt.contains("confirm=true"), "texto: {txt}");
}

#[test]
fn la_tool_mcp_phone_backup_inventaria() {
    use megadaemon_core::mcp_ext;
    let ph = phone();
    let v = mcp_ext::dispatch(
        "phone_backup",
        &serde_json::json!({
            "action": "inventory",
            "root": ph.root.to_string_lossy(),
            "measure": true,
        }),
    )
    .expect("la tool no existe");
    assert!(v.get("isError").is_none());
    let txt = v["content"][0]["text"].as_str().unwrap_or("");
    let j: serde_json::Value = serde_json::from_str(txt).unwrap_or_default();
    assert_eq!(j["ok"], serde_json::json!(true));
    assert!(j["packages"].as_u64().unwrap_or(0) >= 2);
}

#[test]
fn megadaemon_tool_bridges_a_phone_backup() {
    use megadaemon_core::agent_coding;
    let ph = phone();
    let res = agent_coding::dispatch_tool(
        "megadaemon_tool",
        &serde_json::json!({
            "tool_name": "phone_backup",
            "arguments": {
                "action": "inventory",
                "root": ph.root.to_string_lossy(),
            }
        }),
        std::path::Path::new("/tmp"),
        true,
    );
    assert!(res.ok, "megadaemon_tool falló: {}", res.output);
    assert!(res.output.contains("com.test.app"), "salida: {}", res.output);
}

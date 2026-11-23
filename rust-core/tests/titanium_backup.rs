//! Tests del lector de backups «tar por paquete» **sobre fixtures reales**.
//!
//! # De dónde salen los fixtures
//!
//! * `tests/data/titanium/com.megadaemon.testfixture.tar` — **bytes reales**
//!   producidos **en el Samsung S21 Ultra del usuario** con el **script real
//!   de la app de backup instalada** (`com.machiav3lli.backup`,
//!   `files/plugin/tar.sh`), verbatim:
//!   `tar -c -f <archive> -C <datadir> -X <tar_BACKUP_EXCLUDE> .`
//!   El árbol de origen es **sintético** (`com.megadaemon.testfixture`): ni
//!   un byte de datos personales.
//!   * sha256: `c420010470898e28c1a754393ddfea2a8dd771efe19bd05fd47680f846947911`
//!   * 11 264 B · 5 entradas · `lib/` excluido por el `tar_BACKUP_EXCLUDE`
//!     real de la app.
//! * `tests/data/titanium/com.megadaemon.testfixture.dat` — el **mismo** TAR
//!   dentro de un ZIP con el layout **medido** de Swift Backup (los `.dat`
//!   reales de Drive son un ZIP que envuelve un `<pkg>.tar`).
//!   * **sintético**: este ZIP **no** venía de Drive, se generó aquí a
//!     propósito (mismo TAR dentro, una sola entrada
//!     `com.megadaemon.testfixture.tar`, deflate nivel 9, fecha fija
//!     1980-01-01) ⇒ es **reproducible byte a byte**.
//!   * sha256: `ef31ba76100aff824f377254232a354debb5bea066771be7614d4a6fa61ff816`
//!   * 4 845 B
//!
//! Los hashes están **congelados** aquí: si el fixture cambia, el test falla
//! (no se reescribe el hash en silencio).

use megadaemon_core::titanium_backup as tb;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/titanium")
}

fn fixture_tar() -> PathBuf {
    data_dir().join("com.megadaemon.testfixture.tar")
}

fn fixture_dat() -> PathBuf {
    data_dir().join("com.megadaemon.testfixture.dat")
}

#[test]
fn el_fixture_real_es_un_tar_ustar() {
    let bytes = std::fs::read(fixture_tar()).expect("debe existir el fixture");
    assert_eq!(bytes.len(), 11_264, "tamaño congelado del fixture real");
    assert_eq!(
        tb::sha256_hex(&bytes),
        "c420010470898e28c1a754393ddfea2a8dd771efe19bd05fd47680f846947911"
    );
    assert_eq!(tb::detectar_formato(&bytes), tb::FormatoContenedor::Tar);
    // El magic ustar va en el offset 257 de la cabecera de la 1ª entrada.
    // MEDIDO 2026-11-23 (CI): el fixture REAL (el script de la app en el S21
    // Ultra) escribe el magic **GNU** ("ustar  \0"), no el POSIX ("ustar\0"):
    // el `tar` de Android es el de toybox/BSD.  El codigo de produccion lo
    // acepta ambos (`MAGIC_USTAR` | `MAGIC_USTAR_GNU`); aqui se afirma el que
    // trae el fixture de verdad (su sha256 esta congelado justo encima).
    assert_eq!(&bytes[257..263], tb::MAGIC_USTAR_GNU);
}

#[test]
fn lista_las_entradas_del_tar_real() {
    let bytes = std::fs::read(fixture_tar()).unwrap();
    let entradas = tb::leer_tar(&bytes).expect("el TAR real debe leerse");
    let nombres: Vec<&str> = entradas.iter().map(|e| e.nombre.as_str()).collect();
    assert!(
        nombres.contains(&"./databases/fixture.db"),
        "debe ver la base de datos: {nombres:?}"
    );
    assert!(
        nombres.contains(&"./shared_prefs/com.megadaemon.testfixture_preferences.xml"),
        "debe ver las shared_prefs: {nombres:?}"
    );
    assert!(
        nombres.contains(&"./files/leeme.txt"),
        "debe ver los ficheros: {nombres:?}"
    );
    // El `lib/` NO debe estar: lo excluye el `tar_BACKUP_EXCLUDE` real.
    assert!(
        !nombres.iter().any(|n| n.starts_with("./lib")),
        "lib/ debe estar excluido por el exclude real de la app: {nombres:?}"
    );
}

#[test]
fn el_contenedor_zip_con_tar_se_reconoce() {
    let bytes = std::fs::read(fixture_dat()).unwrap();
    assert_eq!(
        tb::sha256_hex(&bytes),
        "ef31ba76100aff824f377254232a354debb5bea066771be7614d4a6fa61ff816"
    );
    assert_eq!(
        tb::detectar_formato(&bytes),
        tb::FormatoContenedor::ZipConTar
    );
    let tar = tb::tar_del_contenedor(&bytes).expect("debe sacar el tar del zip");
    assert_eq!(tar.len(), 11_264);
    let entradas = tb::leer_tar(&tar).unwrap();
    assert!(!entradas.is_empty());
}

#[test]
fn inventaria_el_paquete_del_fixture() {
    let p = tb::inventariar_fichero(&fixture_tar()).expect("debe inventariar");
    assert_eq!(p.paquete, "com.megadaemon.testfixture");
    assert_eq!(p.bytes, 11_264);
    assert_eq!(p.formato, tb::FormatoContenedor::Tar);
    assert!(p.tiene_shared_prefs, "debe detectar las shared_prefs");
    assert!(p.tiene_databases, "debe detectar las bases de datos");
    assert_eq!(
        p.sha256,
        "c420010470898e28c1a754393ddfea2a8dd771efe19bd05fd47680f846947911"
    );
}

#[test]
fn verifica_el_fixture_real() {
    let v = tb::verificar_fichero(&fixture_tar()).expect("debe verificar");
    assert!(v.ok, "errores: {:?}", v.errores);
    assert_eq!(v.formato, tb::FormatoContenedor::Tar);
    assert_eq!(v.bytes, 11_264);
    assert!(v.entradas >= 3, "entradas: {}", v.entradas);
}

#[test]
fn extrae_el_fixture_real_y_su_contenido_cuadra() {
    let dir = std::env::temp_dir().join("megadaemon_titanium_fixture_test");
    let _ = std::fs::remove_dir_all(&dir);
    let dest = dir.join("out");
    let r = tb::extraer_paquete(&fixture_tar(), &dest).expect("debe extraer");
    assert!(r.ok);
    assert!(r.ficheros.iter().any(|f| f == "databases/fixture.db"));
    assert!(r
        .ficheros
        .iter()
        .any(|f| f == "shared_prefs/com.megadaemon.testfixture_preferences.xml"));
    // El contenido extraído es exactamente el del fixture.
    let original = std::fs::read(fixture_tar()).unwrap();
    let tar = tb::tar_del_contenedor(&original).unwrap();
    let entradas = tb::leer_tar(&tar).unwrap();
    for e in &entradas {
        if !e.es_fichero() {
            continue;
        }
        let limpio = tb::sanear_ruta(&e.nombre).unwrap();
        let leido = std::fs::read(dest.join(&limpio)).expect("debe existir el extraído");
        let esperado = &tar[e.offset..e.offset + e.tamano as usize];
        assert_eq!(leido, esperado, "contenido distinto en {}", e.nombre);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn extrae_del_contenedor_zip() {
    let dir = std::env::temp_dir().join("megadaemon_titanium_fixture_zip_test");
    let _ = std::fs::remove_dir_all(&dir);
    let dest = dir.join("out");
    let r = tb::extraer_paquete(&fixture_dat(), &dest).expect("debe extraer");
    assert!(r.ok);
    assert!(dest.join("databases/fixture.db").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn un_backup_corrupto_no_pasa_la_verificacion() {
    let dir = std::env::temp_dir().join("megadaemon_titanium_corrupto_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut bytes = std::fs::read(fixture_tar()).unwrap();
    // Se rompe el nombre de la primera entrada => el checksum del TAR falla.
    bytes[0] = b'X';
    let roto = dir.join("com.roto.tar");
    std::fs::write(&roto, &bytes).unwrap();
    let v = tb::verificar_fichero(&roto).expect("debe devolver informe");
    assert!(!v.ok, "un TAR corrupto NO puede dar ok");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_indice_sqlite_se_lee_con_el_esquema_medido() {
    // Se crea un índice con la tabla `Backup` tal y como se midió en el móvil.
    let dir = std::env::temp_dir().join("megadaemon_titanium_indice_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("main.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE \"Backup\" (
                `backupVersionCode` INTEGER NOT NULL, `packageName` TEXT NOT NULL,
                `packageLabel` TEXT NOT NULL, `versionName` TEXT DEFAULT '-',
                `versionCode` INTEGER NOT NULL, `profileId` INTEGER NOT NULL,
                `sourceDir` TEXT, `splitSourceDirs` TEXT NOT NULL, `isSystem` INTEGER NOT NULL,
                `backupDate` TEXT NOT NULL, `hasApk` INTEGER NOT NULL, `hasAppData` INTEGER NOT NULL,
                `hasDevicesProtectedData` INTEGER NOT NULL, `hasExternalData` INTEGER NOT NULL,
                `hasObbData` INTEGER NOT NULL, `hasMediaData` INTEGER NOT NULL,
                `compressionType` TEXT, `cipherType` TEXT, `iv` BLOB, `cpuArch` TEXT,
                `permissions` TEXT NOT NULL, `size` INTEGER NOT NULL, `note` TEXT NOT NULL,
                `persistent` INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(`packageName`, `backupDate`));",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO Backup (backupVersionCode, packageName, packageLabel, versionName,
                versionCode, profileId, splitSourceDirs, isSystem, backupDate, hasApk, hasAppData,
                hasDevicesProtectedData, hasExternalData, hasObbData, hasMediaData,
                compressionType, cipherType, permissions, size, note)
             VALUES (1, 'com.megadaemon.testfixture', 'Fixture', '5.0.4', 2305, 0, '', 0,
                '2026-11-22T15:02:00', 1, 1, 0, 0, 0, 0, 'none', 'none', '', 11264, '')",
            [],
        )
        .unwrap();
    }
    let filas = tb::leer_indice_sqlite(&db).expect("debe leer el índice");
    assert_eq!(filas.len(), 1);
    assert_eq!(filas[0].package_name, "com.megadaemon.testfixture");
    assert_eq!(filas[0].version_name, "5.0.4");
    assert!(filas[0].has_apk);
    assert!(filas[0].has_app_data);
    assert_eq!(filas[0].size, 11264);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn un_índice_sin_tabla_backup_da_vacio_y_no_error() {
    let dir = std::env::temp_dir().join("megadaemon_titanium_indice_vacio_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("vacio.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE otra (x INTEGER);").unwrap();
    }
    let filas = tb::leer_indice_sqlite(&db).unwrap();
    assert!(filas.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_escribe_fuera_del_destino() {
    let dir = std::env::temp_dir().join("megadaemon_titanium_traversal_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Un TAR con una entrada que intenta salir del destino.
    let mut t = vec![0u8; 512];
    let nombre = b"../escapado.txt";
    t[0..nombre.len()].copy_from_slice(nombre);
    t[100..108].copy_from_slice(b"0000600\0");
    t[108..116].copy_from_slice(b"0000000\0");
    t[116..124].copy_from_slice(b"0000000\0");
    t[124..136].copy_from_slice(b"00000000001\0");
    t[136..148].copy_from_slice(b"00000000000\0");
    t[148..156].copy_from_slice(b"        ");
    t[156] = b'0';
    t[257..263].copy_from_slice(tb::MAGIC_USTAR);
    let mut suma: i64 = 0;
    for (i, &b) in t.iter().enumerate() {
        suma += if (148..156).contains(&i) { 32 } else { b as i64 };
    }
    let chk = format!("{:06o}\0 ", suma);
    t[148..156].copy_from_slice(chk.as_bytes());
    let mut tar = t;
    tar.extend_from_slice(b"x");
    tar.extend_from_slice(&[0u8; 507]);
    tar.extend_from_slice(&[0u8; 1024]);
    let f = dir.join("com.evil.tar");
    std::fs::write(&f, &tar).unwrap();
    let dest = dir.join("out");
    let r = tb::extraer_paquete(&f, &dest).unwrap();
    assert!(r.ficheros.is_empty(), "no debe escribir nada: {:?}", r.ficheros);
    assert!(!dir.join("escapado.txt").exists(), "no debe salir del destino");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_lector_tar_rechaza_lo_que_no_es_tar() {
    assert!(tb::leer_tar(b"esto no es un tar de ninguna manera").is_err());
    assert!(tb::leer_tar(&[]).is_err());
}

#[test]
fn inventario_de_una_carpeta_inexistente_da_error_exacto() {
    let e = tb::inventariar_directorio(Path::new("/no/existe/este/directorio"))
        .expect_err("debe dar error");
    assert!(e.contains("no se pudo leer"), "{e}");
}

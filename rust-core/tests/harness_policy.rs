//! MegaDaemon v13.4 — SUITE DE POLÍTICA DEL HARNESS
//!
//! Cada test reproduce un defecto REAL de la capa MCP/dispositivo.
//! Si el defecto vuelve, el test falla.
//!
//! Ejecutar:  cargo test --test harness_policy
//!
//! Contexto de la evidencia:
//!  * H-01 (precedencia de operadores): la condición de confirmación de
//!    `fs_write` era
//!        path.starts_with("/dev/block") || path.starts_with("/system/") && chmod.is_empty()
//!    En Rust `&&` liga más fuerte que `||`, así que eso se leía como
//!    `A || (B && C)`.  Consecuencia: escribir en `/system/...` CON un
//!    `chmod` presente saltaba la confirmación y llegaba al sistema sin
//!    `confirm_destructive`.  El usuario perdía el control de un
//!    `confirm` que la propia política declaraba obligatorio.
//!  * H-02 (contrato muerto): el error `needs_confirm` de `fs_write`
//!    decía "repítelo con confirm_destructive=true", pero `fs_write` ni
//!    siquiera tenía ese parámetro, ni lo declaraba su esquema MCP, ni lo
//!    leía el despacho.  La instrucción era IMPOSIBLE de satisfacer: el
//!    cliente se quedaba atrapado en un bucle de error sin salida.
//!  * H-03 (ruta sin barra final): `starts_with("/system/")` no casa con
//!    `/system` exactamente, que también es una escritura de sistema.
//!
//! Todos los tests hablan con `android::dispatch` (firma estable) y con el
//! catálogo MCP publicado, nunca con funciones internas: así la suite
//! compila y falla en rojo ANTES del arreglo, y pasa en verde DESPUÉS.

use megadaemon_core::android;
use megadaemon_core::mcp_server;
use serde_json::{json, Value};
use std::sync::Once;

static ESTADO: Once = Once::new();

/// Redirige el directorio de estado a una ruta temporal propia del proceso
/// de test, para que ni la auditoría ni los ficheros transitorios de
/// `fs_write` toquen el estado real del daemon.
fn estado_de_prueba() {
    ESTADO.call_once(|| {
        let dir = format!("/tmp/megadaemon_harness_policy_{}", std::process::id());
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("MEGADAEMON_STATE_DIR", dir);
    });
}

fn fs_write(args: Value) -> Value {
    android::dispatch("fs_write", &args)
}

fn error_es(v: &Value, codigo: &str) -> bool {
    v.get("error").and_then(|e| e.as_str()) == Some(codigo)
}

// ========================================================================= //
// A. Política de escritura: /system y /dev/block exigen confirmación
// ========================================================================= //

#[test]
fn h01_escribir_en_system_con_chmod_exige_confirmacion() {
    estado_de_prueba();
    // El defecto: con `chmod` presente la condición se evaluaba como
    // `false || (true && false)` y la escritura salía sin confirmar.
    let r = fs_write(json!({
        "path": "/system/app/megadaemon_harness_probe.apk",
        "content": "no deberia escribirse",
        "chmod": "644",
        "confirm_destructive": false
    }));
    assert!(
        error_es(&r, "needs_confirm"),
        "escribir en /system con chmod debe exigir confirm_destructive, devolvio: {}",
        r
    );
    assert_eq!(r["ok"], json!(false), "{}", r);
}

#[test]
fn h03_escribir_en_system_sin_barra_final_exige_confirmacion() {
    estado_de_prueba();
    let r = fs_write(json!({
        "path": "/system",
        "content": "x",
        "confirm_destructive": false
    }));
    // Limpieza defensiva: si el test falla (es decir, si la política NO
    // bloquea), el intento de escritura puede haber dejado un fichero
    // llamado `/system` en la raíz.  En un Android real `/system` es un
    // directorio de sólo lectura y esto no hace nada.
    let _ = std::fs::remove_file("/system");
    assert!(
        error_es(&r, "needs_confirm"),
        "`/system` (sin barra) tambien es una escritura de sistema, devolvio: {}",
        r
    );
}

#[test]
fn escribir_en_dev_block_exige_confirmacion() {
    estado_de_prueba();
    let r = fs_write(json!({
        "path": "/dev/block/sda",
        "content": "x",
        "confirm_destructive": false
    }));
    assert!(
        error_es(&r, "needs_confirm"),
        "escribir en /dev/block debe exigir confirm_destructive, devolvio: {}",
        r
    );
}

#[test]
fn una_escritura_normal_no_pide_confirmacion() {
    estado_de_prueba();
    let r = fs_write(json!({
        "path": "/tmp/megadaemon_harness_policy_probe.txt",
        "content": "hola",
        "confirm_destructive": false
    }));
    assert!(
        !error_es(&r, "needs_confirm"),
        "una escritura corriente no debe pedir confirmación, devolvio: {}",
        r
    );
}

#[test]
fn confirmar_habilita_la_escritura_en_system() {
    estado_de_prueba();
    // Con `confirm_destructive: true` la política debe abrirse.  La ruta
    // elegida (`/system`) es un directorio, así que la escritura falla
    // aunque se autorice: el test NO toca nada en un dispositivo real.
    let r = fs_write(json!({
        "path": "/system",
        "content": "x",
        "confirm_destructive": true
    }));
    assert!(
        !error_es(&r, "needs_confirm"),
        "la confirmación explícita debe abrir la política, devolvio: {}",
        r
    );
}

// ========================================================================= //
// B. El contrato del esquema MCP y el despacho no puede divergir
// ========================================================================= //

#[test]
fn h02_el_esquema_de_fs_write_declara_confirm_destructive() {
    estado_de_prueba();
    let defs = android::tool_definitions();
    let arr = defs.as_array().expect("el catálogo debe ser un array");
    let fs_write = arr
        .iter()
        .find(|d| d["name"].as_str() == Some("fs_write"))
        .expect("fs_write debe estar en el catálogo");
    let props = fs_write["inputSchema"]["properties"]
        .as_object()
        .expect("fs_write debe declarar propiedades");
    assert!(
        props.contains_key("confirm_destructive"),
        "el esquema de fs_write debe declarar `confirm_destructive`: sin él, \
         el error needs_confirm pide un parámetro que el cliente no puede enviar"
    );
    assert_eq!(
        props["confirm_destructive"]["type"].as_str(),
        Some("boolean"),
        "confirm_destructive debe ser booleano"
    );
}

#[test]
fn toda_herramienta_que_pide_confirmacion_la_declara_en_su_esquema() {
    // Invariante genérico: si la descripción de una herramienta menciona
    // `confirm_destructive`, el parámetro tiene que existir en su esquema.
    // Sin esta comprobación, un `needs_confirm` puede volver a ser un
    // contrato muerto (H-02) en cualquier herramienta futura.
    let mut catálogos = vec![mcp_server::tool_definitions()];
    catálogos.push(android::tool_definitions());
    for cat in catálogos {
        for d in cat.as_array().unwrap() {
            let name = d["name"].as_str().unwrap_or("");
            let descr = d["description"].as_str().unwrap_or("");
            if descr.contains("confirm_destructive") {
                let props = d["inputSchema"]["properties"].as_object();
                assert!(
                    props.map(|p| p.contains_key("confirm_destructive")).unwrap_or(false),
                    "{}: la descripción menciona confirm_destructive pero el esquema no lo declara",
                    name
                );
            }
        }
    }
}

// ========================================================================= //
// C. El catálogo MCP anuncia exactamente lo que existe
// ========================================================================= //

#[test]
fn el_catalogo_mcp_no_tiene_nombres_duplicados() {
    let defs = mcp_server::tool_definitions();
    let arr = defs.as_array().expect("el catálogo debe ser un array");
    let mut nombres: Vec<&str> = arr.iter().filter_map(|d| d["name"].as_str()).collect();
    let total = nombres.len();
    nombres.sort_unstable();
    nombres.dedup();
    assert_eq!(
        nombres.len(),
        total,
        "hay herramientas MCP con el mismo nombre: un cliente no sabe cuál ejecutar"
    );
}

#[test]
fn el_catalogo_mcp_es_la_union_de_los_subcatalogos() {
    // El MCP fusiona CINCO grupos (`mcp_server::tool_definitions`):
    //   core (10) + android + sync + adb_tool + mcp_ext.
    // Este test enumera los cinco: la invariante que protege es que el
    // catálogo publicado es EXACTAMENTE la unión de sus grupos (una tool
    // que se pierde al fusionar, o una tool inventada fuera de un grupo,
    // hacen fallar el test).  Antes sólo se enumeraban tres grupos
    // (core parcial + android + sync) y el conteo fijo "26": al añadir
    // la voz (megadaemon_voice/catalog), el catálogo unificado (R-47) y
    // los módulos de v14 (adb_tool, mcp_ext) el test quedó obsoleto y
    // bloqueaba el gate del CI con un fallo que no era de ningún defecto.
    let nucleo = [
        "minimax_chat",
        // MEDIDO (2026-11-28): `megadaemon_chat` esta en el catalogo core
        // (`mcp_server.rs`) desde hace tiempo, pero la lista del test se
        // quedo obsoleta => el gate del CI fallaba sin que hubiera ningun
        // defecto (el mismo caso que el comentario de arriba).
        "megadaemon_chat",
        "megadaemon_voice",
        "megadaemon_catalog",
        "swarm_chat",
        "megadaemon_die_binwalk",
        "megadaemon_rizin",
        "megadaemon_cirx_analyze",
        "megadaemon_mba_simplify",
        "megadaemon_npu_cv",
        "signing_vault",
    ];
    let mut esperado: Vec<String> = nucleo.iter().map(|s| s.to_string()).collect();
    for sub in [
        android::tool_definitions(),
        megadaemon_core::sync::tool_definitions(),
        megadaemon_core::adb_tool::tool_definitions(),
        megadaemon_core::mcp_ext::tool_definitions(),
        megadaemon_core::antigravity_model::tool_definitions(),
        megadaemon_core::antigravity_env::tool_definitions(),
        // v16 (2026-11-23, rama del MITM): el FICHERO FALSO del language
        // server tambien aporta su tool al catalogo fusionado.  El test
        // enumeraba seis grupos y se quedaba UNO por detras => el gate
        // del CI fallaba sin que hubiera ningun defecto.
        megadaemon_core::antigravity_fake_so::tool_definitions(),
    ] {
        for d in sub.as_array().unwrap() {
            esperado.push(d["name"].as_str().unwrap().to_string());
        }
    }
    esperado.sort();
    let mut real: Vec<String> = mcp_server::tool_definitions()
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["name"].as_str().map(|s| s.to_string()))
        .collect();
    real.sort();
    assert_eq!(
        real, esperado,
        "el catálogo fusionado debe ser exactamente la unión de sus subcatálogos"
    );
    // Suelo (no un número exacto): el conteo exacto se rompía con cada
    // tool nueva; lo que no puede pasar es que el catálogo pierda
    // herramientas por el camino.
    assert!(
        real.len() >= 45,
        "el catálogo MCP ha perdido herramientas: {} (esperado >= 45)",
        real.len()
    );
}

#[test]
fn toda_herramienta_anunciada_tiene_su_brazo_en_el_despacho() {
    // Se comprueba contra el código fuente: cada herramienta que el
    // catálogo anuncia tiene que tener su brazo en `android::dispatch`.
    // Es la guarda que habría detectado que `repo_updates` se anunciaba
    // sin estar cableado, o al revés.
    let src = include_str!("../src/android.rs");
    let ini = src
        .find("pub fn dispatch(name: &str, args: &Value) -> Value {")
        .expect("no se encuentra android::dispatch");
    let resto = &src[ini..];
    let fin = resto.find("\n}\n").map(|p| p + 3).unwrap_or(resto.len());
    let cuerpo = &resto[..fin];
    let mut brazos: Vec<String> = Vec::new();
    for linea in cuerpo.lines() {
        let l = linea.trim();
        if let Some(cola) = l.strip_prefix('"') {
            if let Some(p) = cola.find("\" =>") {
                brazos.push(cola[..p].to_string());
            }
        }
    }
    for d in android::tool_definitions().as_array().unwrap() {
        let name = d["name"].as_str().unwrap();
        assert!(
            brazos.iter().any(|b| b == name),
            "{} se anuncia en el catálogo pero android::dispatch no lo atiende",
            name
        );
    }
}

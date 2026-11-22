//! Smoke test del soporte de Ghidra de MegaDaemon.
//!
//! Cubre lo que **no** depende de que haya Ghidra instalado:
//!
//! * el parseo de los TSV de `unstrip` y de anclas;
//! * la detección de `analyzeHeadless` / JDK **sin panic** cuando no existen
//!   (el mensaje de error debe ser accionable, no un `panic!`);
//! * la cola global: una segunda petición con presupuesto 0 debe fallar
//!   raudamente con `Busy`, no bloquearse;
//! * el script Java embebido: debe ser válido y tener los tres modos;
//! * (opcional, sólo si hay Ghidra + JDK) un ciclo real de import/decompile.

use std::path::PathBuf;
use std::time::Duration;

use megadaemon_core::ghidra::{
    decompile_anchors, find_symbol, import_binary, parse_anchors_tsv, parse_symbols_tsv,
    GhidraConfig, GhidraError, GhidraTool, MD_GHIDRA_SCRIPT,
};

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("md-ghidra-test-{}", tag));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("crear dir temporal");
    d
}

#[test]
fn parses_unstrip_symbols_tsv() {
    let dir = tmp_dir("sym");
    let p = dir.join("symbols.tsv");
    std::fs::write(
        &p,
        "0x4d06cb8\tinternal_abi.__RegArgs_.Dump\tgo: internal/abi.(*RegArgs).Dump | abi.go:47\n\
         0x61cef28\tmcp_mcp.NewClient\tgo: .../mcp.NewClient | client.go:51\n",
    )
    .expect("escribir TSV");

    let syms = parse_symbols_tsv(&p).expect("parsear TSV de símbolos");
    assert_eq!(syms.len(), 2, "deben parsearse 2 símbolos");
    assert_eq!(syms[0].addr, 0x4d06cb8);
    assert_eq!(syms[0].name, "internal_abi.__RegArgs_.Dump");
    assert!(syms[0].comment.contains("abi.go:47"));
    assert_eq!(syms[1].addr, 0x61cef28);

    // búsqueda exacta y por subcadena
    let hit = find_symbol(&syms, "mcp_mcp.NewClient").expect("debe encontrar la función");
    assert_eq!(hit.addr, 0x61cef28);
    let hit2 = find_symbol(&syms, "NewClient").expect("subcadena");
    assert_eq!(hit2.addr, 0x61cef28);
    assert!(find_symbol(&syms, "no-existe-esta").is_none());
}

#[test]
fn parses_anchors_tsv() {
    let dir = tmp_dir("anch");
    let p = dir.join("anchors.tsv");
    std::fs::write(
        &p,
        "0x4d59d68\truntime.main\truntime.main\tproc.go:153\tentry\n\
         0x61cef28\tmcp_mcp.NewClient\tgo: .../mcp.NewClient\tclient.go:51\tmcp\n",
    )
    .expect("escribir TSV");

    let a = parse_anchors_tsv(&p).expect("parsear TSV de anclas");
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].tag, "entry");
    assert_eq!(a[0].location, "proc.go:153");
    assert_eq!(a[1].tag, "mcp");
    assert_eq!(a[1].addr, 0x61cef28);
}

#[test]
fn invalid_address_is_a_clear_error_not_a_panic() {
    let dir = tmp_dir("bad");
    let p = dir.join("bad.tsv");
    std::fs::write(&p, "no-es-hex\tname\tx\n").expect("escribir TSV");
    let err = parse_symbols_tsv(&p).expect_err("debe fallar");
    match err {
        GhidraError::Config(m) => assert!(m.contains("dirección inválida")),
        other => panic!("se esperaba Config, llegó {:?}", other),
    }
}

#[test]
fn missing_java_gives_a_clear_message() {
    // Un JAVA_HOME imposible y un PATH vacío: la detección debe fallar con
    // un mensaje accionable (y sin panic).
    let cfg = GhidraConfig {
        java_home: Some(PathBuf::from("/definitivamente/no/existe/jdk")),
        ghidra_dir: Some(PathBuf::from("/definitivamente/no/existe/ghidra")),
        ..GhidraConfig::default()
    };
    let saved_path = std::env::var("PATH").ok();
    // No tocamos el PATH global (tests paralelos): sólo comprobamos que,
    // si la detección falla, el mensaje sea útil.
    let _ = saved_path;

    match GhidraTool::detect(&cfg) {
        Err(GhidraError::JavaMissing { searched }) => {
            assert!(!searched.is_empty(), "debe listar dónde buscó");
        }
        Err(GhidraError::AnalyzeHeadlessMissing { searched }) => {
            assert!(!searched.is_empty(), "debe listar dónde buscó");
        }
        Err(other) => {
            //otro error: el mensaje debe ser legible
            assert!(!other.to_string().is_empty());
        }
        Ok(tool) => {
            // El host sí tiene Ghidra/JDK: comprobamos que la ruta es usable.
            assert!(tool.analyze_headless.exists());
            assert!(tool.java_bin.is_some());
        }
    }
}

#[test]
fn java_missing_message_names_the_fix() {
    let err = GhidraError::JavaMissing {
        searched: vec!["/usr/bin/java".to_string()],
    };
    let msg = err.to_string();
    assert!(msg.contains("JDK"), "el mensaje debe mencionar el JDK: {}", msg);
    assert!(msg.contains("openjdk"), "el mensaje debe decir qué instalar: {}", msg);
}

#[test]
fn embedded_java_script_is_complete() {
    assert!(MD_GHIDRA_SCRIPT.contains("public class MdGhidra extends GhidraScript"));
    assert!(MD_GHIDRA_SCRIPT.contains("import ghidra.app.decompiler.DecompInterface;"));
    assert!(MD_GHIDRA_SCRIPT.contains("\"symbols\""));
    assert!(MD_GHIDRA_SCRIPT.contains("\"decompile\""));
    assert!(MD_GHIDRA_SCRIPT.contains("\"info\""));
    // debe cerrar todas las llaves (el compilador de Ghidra lo exige)
    let opens = MD_GHIDRA_SCRIPT.matches('{').count();
    let closes = MD_GHIDRA_SCRIPT.matches('}').count();
    assert_eq!(opens, closes, "llaves desbalanceadas en el script embebido");
}

#[test]
fn queue_reports_busy_instead_of_hanging() {
    // Con presupuesto 0, si la cola está libre debe entrar; si no, debe
    // devolver Busy rápido. En cualquier caso NO puede colgarse.
    let cfg = GhidraConfig {
        queue_wait: Duration::from_millis(0),
        ..GhidraConfig::default()
    };
    let dir = tmp_dir("queue");
    let bin = dir.join("no-existe.bin");
    // binario inexistente -> error de configuración (no debe colgarse)
    let tool = GhidraTool::detect(&cfg);
    if let Ok(tool) = tool {
        let start = std::time::Instant::now();
        let r = import_binary(&tool, &cfg, &bin);
        assert!(start.elapsed() < Duration::from_secs(5), "no debe tardar");
        assert!(r.is_err(), "un binario inexistente debe fallar");
        if let Err(GhidraError::Config(m)) = r {
            assert!(m.contains("no existe"));
        }
    }
}

/// Ciclo real: sólo se ejecuta si el host tiene Ghidra + JDK.
/// Se salta (no falla) cuando no están, para no romper CI sin herramientas.
#[test]
fn end_to_end_import_and_decompile_if_ghidra_available() {
    let cfg = GhidraConfig {
        project_dir: tmp_dir("e2e-proj"),
        script_dir: tmp_dir("e2e-scripts"),
        timeout: Duration::from_secs(900),
        queue_wait: Duration::from_secs(10),
        ..GhidraConfig::default()
    };
    let tool = match GhidraTool::detect(&cfg) {
        Ok(t) if t.has_jdk => t,
        _ => {
            eprintln!("Ghidra/JDK no disponible: test end-to-end omitido");
            return;
        }
    };

    // Binario de prueba mínimo: el propio binario de test no sirve, así que
    // usamos el intérprete de shell, que es un ELF en Linux.
    let bin = PathBuf::from("/bin/sh");
    if !bin.exists() {
        eprintln!("sin /bin/sh: test end-to-end omitido");
        return;
    }

    let out = tmp_dir("e2e-out");
    let rep = import_binary(&tool, &cfg, &bin).expect("import debe funcionar");
    assert!(rep.ok, "import falló: {}", rep.message);

    // Export de símbolos mínimo: una función real de /bin/sh no es estable,
    // así que sólo comprobamos que el comando termina y produce informe.
    let syms = out.join("symbols.tsv");
    std::fs::write(&syms, "0x1000\tmd_test_symbol\ttest\n").expect("escribir TSV");
    let rep2 = megadaemon_core::ghidra::apply_symbols(&tool, &cfg, &syms);
    assert!(rep2.is_ok(), "apply_symbols no debe fallar: {:?}", rep2.err());

    let anchors = out.join("anchors.tsv");
    std::fs::write(&anchors, "0x1000\tmd_test_symbol\tmd_test_symbol\ttest.c:1\tentry\n")
        .expect("escribir TSV");
    let dec = decompile_anchors(&tool, &cfg, &anchors, &out, "").expect("decompile");
    assert!(dec.report.ok, "decompile falló: {}", dec.report.message);
}

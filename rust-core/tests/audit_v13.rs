//! MegaDaemon v13.1 — SUITE DE AUDITORÍA ADVERSARIA
//!
//! Cada test reproduce un defecto REAL encontrado en v13.0 con evidencia
//! medible.  Si un defecto vuelve, el test falla.
//!
//! Ejecutar:  cargo test --test audit_v13
//!
//! Contexto de la evidencia:
//!  * E1 (bench_fidelity.py): con la huella `<ENV ...>` en el turno de
//!    usuario, Qwen2.5-0.5B cae de 8/12 a 3/12 enrutado y de 8/12 a 0/12 en
//!    corrección semántica; SmolLM2-360M cae de 8/12 a 5/12.
//!  * E2 (bench_models.py): el fallo dominante en los 4 modelos medidos no
//!    era elegir mal la herramienta, sino COPIAR la línea "OPS" como orden
//!    (2-7 de 19 respuestas) y escribir "| $ grep" dentro de un pipeline
//!    (10-16 de 19).
//!  * El bucle completo showed: "R: $ free -m" (una orden, no una respuesta),
//!    respuestas truncadas que se ejecutaban como degeneración, y citas
//!    [SRC:n] que no resolvían a ninguna observación.

use megadaemon_core::seere_v11 as S;

// ========================================================================= //
// A. La huella del host NO puede volver al prompt por descuido
// ========================================================================= //

#[test]
fn a1_huella_ausente_del_prompt_por_defecto() {
    let cfg = S::SeereConfig::default();
    let msgs = S::seere_messages("¿cuántos núcleos tiene esta máquina?", &cfg, "es");
    let user = msgs[1]["content"].as_str().unwrap();
    assert!(
        !user.contains("<ENV"),
        "la huella del host vuelve al prompt del modelo: {}",
        user
    );
    assert!(!user.contains("uid="), "la huella vuelve al prompt: {}", user);
}

#[test]
fn a2_huella_es_opcional_y_explicita() {
    let cfg = S::SeereConfig {
        env_in_prompt: true,
        ..Default::default()
    };
    let msgs = S::seere_messages("hola", &cfg, "es");
    let user = msgs[1]["content"].as_str().unwrap();
    assert!(user.contains("<ENV"), "la opción --env-in-prompt debe funcionar");
}

#[test]
fn a3_system_prompt_es_estatico() {
    // El prefijo de KV sólo se reutiliza si el system prompt NO cambia.
    let a = S::sys_prompt("REACT", "es");
    let b = S::sys_prompt("REACT", "es");
    assert_eq!(a, b);
    assert!(!a.contains("uid="), "el system prompt lleva la huella");
    assert!(!a.contains("/data/user"), "el system prompt lleva rutas de host");
}

// ========================================================================= //
// B. Guardián anti-eco: la lista de herramientas NO es una orden
// ========================================================================= //

#[test]
fn b1_eco_de_ops_se_descarta() {
    let p = S::parse_model_output("A: $ OPS  $ shell | ? web search | W wiki search | C read file");
    assert_eq!(p.out_type, "echo", "un eco de OPS debe clasificarse como echo");
    assert!(p.actions.is_empty(), "el eco no debe producir acciones ejecutables");
    assert!(p.echo);
}

#[test]
fn b2_eco_simple_se_descarta() {
    for raw in ["A: $ shell", "A: W wiki search", "A: ? web search", "A: C read file"] {
        let p = S::parse_model_output(raw);
        assert!(
            p.actions.is_empty(),
            "{} no debe generar acciones ejecutables: {:?}",
            raw,
            p.actions
        );
    }
}

#[test]
fn b3_acciones_legitimas_no_se_tocan() {
    let p = S::parse_model_output("A: $ ls /data/local/tmp");
    assert_eq!(p.out_type, "action");
    assert_eq!(p.actions.len(), 1);
    assert_eq!(p.actions[0].0, "ROOT_EXEC");
    assert_eq!(p.actions[0].1, "ls /data/local/tmp");
    assert!(!p.echo);
}

#[test]
fn b4_shm_info_conserva_argumento_vacio() {
    // "S i" es SHM_INFO y NO tiene argumento: el guardián no debe descartarlo.
    let p = S::parse_model_output("A: S i");
    assert_eq!(p.out_type, "action");
    assert_eq!(p.actions.len(), 1);
    assert_eq!(p.actions[0].0, "SHM_INFO");
    assert_eq!(p.actions[0].1, "");
}

// ========================================================================= //
// C. Multi-acción y pipelines
// ========================================================================= //

#[test]
fn c1_cadena_de_acciones_separte() {
    let p = S::parse_model_output("A: $ cat x | W finite automaton | C /etc/hosts#0");
    assert_eq!(p.actions.len(), 3, "esperaba 3 acciones: {:?}", p.actions);
    assert_eq!(p.actions[0].0, "ROOT_EXEC");
    assert_eq!(p.actions[1].0, "WIKI_SEARCH");
    assert_eq!(p.actions[2].0, "CHUNK_READ");
}

#[test]
fn c2_pipeline_con_opcode_repetido_se_normaliza() {
    // El modelo escribe "cat x | $ grep y": es UN pipeline, no dos acciones.
    let p = S::parse_model_output("A: $ cat /tmp/x | $ grep error");
    assert_eq!(p.actions.len(), 1, "un pipeline no son dos acciones: {:?}", p.actions);
    assert_eq!(p.actions[0].1, "cat /tmp/x | grep error");
}

#[test]
fn c3_respuesta_al_final_de_un_pipeline_se_separa() {
    let p = S::parse_model_output("A: $ date | R: son las 16:00");
    assert_eq!(p.actions.len(), 1);
    assert_eq!(p.actions[0].0, "ROOT_EXEC");
    assert_eq!(p.actions[0].1, "date");
    assert_eq!(p.response.as_deref(), Some("son las 16:00"));
}

// ========================================================================= //
// D. Fundamento: la cita tiene que RESOLVER
// ========================================================================= //

#[test]
fn d1_cita_sin_observacion_no_pasa() {
    let ids: Vec<String> = vec![];
    let (ok, miss) = S::verify_grounding("UID is 0 [SRC:1]", &[], "uid=0", "hello", Some(&ids));
    assert!(!ok, "una cita sin observación no puede pasar: {:?}", miss);
}

#[test]
fn d2_cita_con_observacion_pasa() {
    let ids: Vec<String> = vec!["1".to_string()];
    let (ok, _) = S::verify_grounding(
        "UID is 0 [SRC:1]",
        &["<OBS id=\"1\">uid=0</OBS>".to_string()],
        "",
        "hello",
        Some(&ids),
    );
    assert!(ok);
}

#[test]
fn d3_sin_observaciones_un_numero_es_inventado() {
    let ids: Vec<String> = vec![];
    let (ok, miss) = S::verify_grounding(
        "Esta maquina tiene 5 nucleos",
        &[],
        "uid=0 gid=0",
        "cuantos nucleos",
        Some(&ids),
    );
    assert!(!ok, "sin observaciones, el 5 es inventado: {:?}", miss);
}

#[test]
fn d4_matching_por_limite_de_palabra() {
    // "4" NO puede estar soportado por "pid=1414".
    let ids: Vec<String> = vec![];
    let (ok, _) = S::verify_grounding("La memoria es 4 GB", &[], "uid=0 gid=0 pid=1414", "ram", Some(&ids));
    assert!(!ok, "'4' no puede casar dentro de pid=1414");
}

#[test]
fn d5_numero_con_unidad_es_dato() {
    let ids: Vec<String> = vec![];
    let (ok, miss) = S::verify_grounding("La memoria es 5.10.144 MB", &[], "uid=0", "ram", Some(&ids));
    assert!(!ok, "un tamaño con unidad es dato aunque no haya OBS: {:?}", miss);
}

#[test]
fn d6_sin_falsos_positivos_para_fechas() {
    // El número está en la PREGUNTA: no se exige soporte.
    let (ok, _) = S::verify_grounding("The release was in 2024", &[], "", "the 2024 release", None);
    assert!(ok);
}

// ========================================================================= //
// E. Cliente HTTP: endpoint configurable, sin slot fijado
// ========================================================================= //

#[test]
fn e1_endpoint_por_defecto() {
    let ep = S::parse_endpoint("http://127.0.0.1:8080");
    assert_eq!(ep.host, "127.0.0.1");
    assert_eq!(ep.port, 8080);
    assert_eq!(ep.path, "/v1/chat/completions");
    assert!(!ep.tls);
}

#[test]
fn e2_endpoint_con_puerto_y_ruta() {
    let ep = S::parse_endpoint("http://192.168.1.5:9099/v1/chat/completions");
    assert_eq!(ep.host, "192.168.1.5");
    assert_eq!(ep.port, 9099);
    assert_eq!(ep.path, "/v1/chat/completions");
}

#[test]
fn e3_endpoint_https_se_marca() {
    let ep = S::parse_endpoint("https://api.example.com/v1/chat/completions");
    assert!(ep.tls);
    assert_eq!(ep.port, 443);
}

// ========================================================================= //
// F. La respuesta que es una ORDEN
// ========================================================================= //

#[test]
fn f1_respuesta_que_es_una_orden_se_detecta() {
    let (tool, arg) = S::lead_opcode("$ free -m | grep Mem").expect("debería detectar la orden");
    assert_eq!(tool, "ROOT_EXEC");
    assert_eq!(arg, "free -m | grep Mem");
}

#[test]
fn f2_una_respuesta_normal_no_es_una_orden() {
    assert!(S::lead_opcode("La memoria es de 4 GB").is_none());
    assert!(S::lead_opcode("Cores: 5").is_none());
    assert!(S::lead_opcode("hola").is_none());
}

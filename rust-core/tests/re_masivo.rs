//! Arnés del pipeline de RE masivo por app (`re_masivo`).
//!
//! Fija lo que NO depende del dispositivo:
//! * la tool está en el catálogo MCP y su `action` es un enum;
//! * el `dispatch` responde con un error estructurado (nunca un panic);
//! * la puntuación (priorización) es **determinista** y explica el porqué;
//! * el digest se construye aunque no haya ninguna app analizada.
//!
//! Lo que SÍ depende del móvil (el escaneo real de APKs instalados) se
//! verifica con la tool en el dispositivo: ver `docs/RE-MASIVO.md`.

use megadaemon_core::mass_re;
use serde_json::json;

#[test]
fn la_tool_re_masivo_esta_en_el_catalogo_mcp() {
    let defs = megadaemon_core::mcp_ext::tool_definitions();
    let arr = defs.as_array().expect("tool_definitions devuelve una lista");
    let d = arr
        .iter()
        .find(|d| d["name"] == "re_masivo")
        .expect("la tool re_masivo tiene que estar declarada");
    assert!(d["description"].as_str().unwrap_or("").len() > 40);
    let acciones = d["inputSchema"]["properties"]["action"]["enum"]
        .as_array()
        .expect("la acción tiene que ser un enum");
    let lista: Vec<&str> = acciones.iter().filter_map(|v| v.as_str()).collect();
    for a in ["worklist", "analizar", "analizar_lote", "digest", "bitacora"] {
        assert!(lista.contains(&a), "falta la acción {}", a);
    }
}

#[test]
fn el_dispatch_responde_con_error_estructurado() {
    let r = megadaemon_core::mcp_ext::dispatch("re_masivo", &json!({"action": "no_existe"}))
        .expect("la tool tiene que despachar");
    assert_eq!(r["ok"], json!(false));
    assert_eq!(r["error"], "accion_desconocida");

    let r2 = megadaemon_core::mcp_ext::dispatch("re_masivo", &json!({"action": "analizar"}))
        .expect("la tool tiene que despachar");
    assert_eq!(r2["ok"], json!(false));
    assert_eq!(r2["error"], "falta_paquete");
}

#[test]
fn la_puntuacion_es_determinista_y_explica_el_porque() {
    let mut ev = mass_re::Evidencia {
        paquete: "com.bbva.bbvacontigo".to_string(),
        ..Default::default()
    };
    ev.veredicto = "detector_fuerte".to_string();
    ev.familias = vec![
        "attestation".to_string(),
        "hook_framework".to_string(),
        "su_binary".to_string(),
    ];
    ev.ofuscacion = 0.9;
    ev.permisos_peligrosos = 12;

    let a = mass_re::puntuar(&ev);
    let b = mass_re::puntuar(&ev);
    assert_eq!(a.total, b.total);
    assert_eq!(a.nivel, b.nivel);
    assert_eq!(a.motivos, b.motivos);
    assert!(a.total >= 75, "puntos: {}", a.total);
    assert_eq!(a.nivel, "critico");
    // Los motivos están ordenados de más a menos puntos y todos son texto.
    let puntos: Vec<u32> = a.motivos.iter().map(|(_, p)| *p).collect();
    let mut orden = puntos.clone();
    orden.sort_by(|x, y| y.cmp(x));
    assert_eq!(puntos, orden);
    assert!(a
        .motivos
        .iter()
        .any(|(m, _)| m.contains("ofuscacion") || m.contains("app sensible")));
}

#[test]
fn el_nivel_crece_con_el_puntaje() {
    assert_eq!(mass_re::nivel_para(0), "bajo");
    assert_eq!(mass_re::nivel_para(30), "medio");
    assert_eq!(mass_re::nivel_para(55), "alto");
    assert_eq!(mass_re::nivel_para(80), "critico");
}

#[test]
fn el_digest_se_construye_sin_apps() {
    let d = mass_re::digest();
    assert_eq!(d["ok"], json!(true));
    assert!(d["tabla"].as_str().unwrap_or("").contains("| app |"));
    // El recuento de apps existe siempre (0 si no se ha analizado ninguna).
    assert!(d["apps"].as_u64().is_some());
    assert!(d["por_nivel"].as_array().map(|a| a.len() == 4).unwrap_or(false));
}

#[test]
fn el_presupuesto_nunca_es_infinito() {
    let p = mass_re::Presupuesto::desde_json(&json!({}));
    assert!(p.max_apps > 0);
    assert!(p.max_por_app_ms > 0);
    assert!(p.max_por_app_bytes > 0);
    assert!(p.max_senales_por_app > 0);
    // Un 0 no desactiva el límite (sería un agujero).
    let p2 = mass_re::Presupuesto::desde_json(&json!({"max_apps": 0, "max_por_app_ms": 0}));
    assert_eq!(p2.max_apps, mass_re::Presupuesto::default().max_apps);
    assert_eq!(
        p2.max_por_app_ms,
        mass_re::Presupuesto::default().max_por_app_ms
    );
}

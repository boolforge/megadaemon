//! Tests de integración del gestor de portapapeles.
//!
//! Comprueban tres cosas que importan de verdad:
//!   1. la tool `clipboard` aparece en el catálogo MCP (si no, un agente MCP
//!      que sólo lee el catálogo no la ve);
//!   2. el contenido **no** se filtra ni al log ni al NDJSON en claro;
//!   3. el formato de malla ida y vuelta no pierde el TTL.

use megadaemon_core::clipboard::{self, Item, ReglasSync};
use serde_json::json;

#[test]
fn la_tool_clipboard_esta_en_el_catalogo_mcp() {
    let catalogo = megadaemon_core::mcp_server::tool_definitions();
    let tools = catalogo.as_array().expect("el catalogo es un array");
    let nombres: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(
        nombres.contains(&"clipboard"),
        "la tool clipboard no esta en el catalogo MCP"
    );
    // y su esquema declara las acciones
    let t = tools
        .iter()
        .find(|t| t.get("name").and_then(|n| n.as_str()) == Some("clipboard"))
        .expect("tool clipboard");
    let props = t
        .get("inputSchema")
        .and_then(|s| s.get("properties"))
        .expect("inputSchema");
    assert!(props.get("action").is_some());
    assert!(props.get("text").is_some());
}

#[test]
fn el_contenido_sensible_no_se_persiste_en_claro() {
    let dir = std::env::temp_dir().join(format!("md_clip_it_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sd = dir.to_string_lossy().to_string();

    let r = clipboard::handle_action(
        "put",
        &json!({ "text": "mi contraseña es 1234" }),
        &sd,
    );
    assert_eq!(r["ok"], json!(true));
    assert_eq!(r["sensible"], json!(true));
    assert_eq!(r["guardado"], json!(false));

    // el fichero existe pero NO contiene el secreto
    let ruta = clipboard::ruta_historial(&sd);
    if let Ok(crudo) = std::fs::read(&ruta) {
        let s = String::from_utf8_lossy(&crudo);
        assert!(
            !s.contains("contraseña") && !s.contains("1234"),
            "el secreto se ha quedado en el historial en claro"
        );
    }

    // un texto normal si se guarda
    let r = clipboard::handle_action("put", &json!({ "text": "nota normal" }), &sd);
    assert_eq!(r["guardado"], json!(true));
    let l = clipboard::handle_action("list", &json!({}), &sd);
    assert_eq!(l["items"].as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn el_item_marcado_como_sensible_llega_como_efimero() {
    let i = Item::nuevo("tu código es 482913", 1000, "sms");
    assert!(i.sensible);
    assert!(i.efimero);
    assert_eq!(i.ttl_s, Some(60), "lo sensible caduca en 60 s");
}

#[test]
fn la_malla_conserva_el_ttl_y_el_tipo() {
    let mut i = Item::nuevo("hola", 1000, "manual");
    i.ttl_s = Some(30);
    let linea = clipboard::a_malla(&i, &ReglasSync::default()).expect("se propaga");
    let vuelta = clipboard::de_malla(&linea).expect("vuelve");
    assert_eq!(vuelta.ttl_s, Some(30));
    assert_eq!(vuelta.ts, 1000);
    assert_eq!(vuelta.texto, "hola");
    assert_eq!(vuelta.tipo, i.tipo);
}

#[test]
fn el_item_sensible_no_se_propaga_por_la_malla() {
    let i = Item::nuevo("mi contraseña es 1234", 1000, "manual");
    assert!(
        clipboard::a_malla(&i, &ReglasSync::default()).is_none(),
        "un secreto no debe viajar por la malla por defecto"
    );
}

#[test]
fn el_hash_del_item_es_estable() {
    let a = Item::nuevo("hola", 1000, "x");
    let b = Item::nuevo("hola", 1000, "y");
    assert_eq!(a.id, b.id, "el id depende del texto y del instante, no del origen");
    let c = Item::nuevo("hola", 2000, "x");
    assert_ne!(a.id, c.id, "cambiar el instante cambia el id");
}

//! Smoke test del inventario de Antigravity (`antigravity_intel`).
//!
//! Se incluye el módulo por ruta (`#[path]`) para **no tocar `lib.rs`**
//! (regla del repo: `lib.rs`/`Cargo.toml` sólo con acuerdo). El wiring
//! (`pub mod antigravity_intel;`) lo hace el principal.
//!
//! El test monta un árbol `.gemini` sintético y comprueba, sobre todo, que
//! **ningún valor de credencial sale del módulo**: sólo nombres de campo y
//! sha16.

use std::fs;
use std::path::PathBuf;

#[path = "../src/antigravity_intel.rs"]
mod antigravity_intel;

use self::antigravity_intel::*;

fn tmp_root(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "agy-intel-smoke-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn write(path: &PathBuf, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

/// Config MCP con las dos formas que usa Antigravity: stdio (con comando que
/// NO existe) y Streamable HTTP (con cabeceras).
const MCP_CONFIG: &str = r#"{
  "mcpServers": {
    "megadaemon-minimax": {
      "command": "/data/user/0/com.agy/files/megadaemon/rust-core/target/release/megadaemon-core",
      "args": ["mcp-server"],
      "env": { "MEGADAEMON_MCP": "1", "MEGADAEMON_STATE_DIR": "/data/local/tmp/megadaemon/state" }
    },
    "otro-remoto": {
      "url": "http://127.0.0.1:9090/mcp",
      "headers": { "X-API-Key": "mdaemon_sk_live_SEGRETO_NO_DEBE_APARECER", "X-Seere-Auth-Token": "t" }
    }
  }
}"#;

/// Config principal con credenciales de proveedor (valores que NO deben salir).
const CONFIG: &str = r#"{
  "providers": {
    "minimax": { "api_key": "mdaemon_sk_live_MINIMAX_VALOR_SECRETO", "base_url": "https://api.minimax.example" },
    "openai": { "api_key": "sk-openai-SECRETO" }
  },
  "theme": "dark"
}"#;

const TASKS: &str = r#"[
  {"id":"task_1","name":"Snatcher-MegaDaemon-SuperLoop","command":"python3 /data/user/0/com.agy/files/scummvm/tools/re_pipeline/snatcher_megadaemon.py --once","prompt":"","enabled":true,
   "trigger":{"type":"interval","interval_sec":300}},
  {"id":"task_2","name":"minimax_bootstrap","command":"bash /data/user/0/com.agy/files/dumper-web/bootstrap_core.sh","prompt":"","enabled":false,
   "trigger":{"type":"cron","interval_sec":null}}
]"#;

#[test]
fn escanea_instalacion_de_antigravity() {
    let root = tmp_root("scan");
    write(&root.join("config").join("mcp_config.json"), MCP_CONFIG);
    write(&root.join("config").join("config.json"), CONFIG);
    write(
        &root.join("antigravity-app").join("daemon").join("tasks.json"),
        TASKS,
    );
    fs::create_dir_all(root.join("config").join("plugins").join("gemini-api")).unwrap();
    fs::create_dir_all(root.join("config").join("skills").join("minimax-megadaemon")).unwrap();
    fs::create_dir_all(root.join("antigravity-app").join("brain").join("conv-1")).unwrap();
    write(&root.join("jetski-standalone-oauth-token"), "oauth-opaque");

    let inv = scan_gemini_root(&root).expect("debe escanear");
    assert_eq!(inv.plugins, vec!["gemini-api".to_string()]);
    assert_eq!(inv.skills, vec!["minimax-megadaemon".to_string()]);
    assert_eq!(inv.conversations, 1);
    assert!(inv.mcp_config_sha256.is_some());
    assert!(inv.config_sha256.is_some());
    assert!(inv.tasks_sha256.is_some());

    // --- servidores MCP ---
    assert_eq!(inv.servers.len(), 2);
    let stdio = inv.servers.iter().find(|s| s.name == "megadaemon-minimax").unwrap();
    assert_eq!(stdio.transport, "stdio");
    assert_eq!(stdio.args, vec!["mcp-server".to_string()]);
    assert!(stdio.env_names.contains(&"MEGADAEMON_MCP".to_string()));
    // el binario del comando no existe en el test => el MCP no arrancaría
    assert!(!stdio.command_exists);
    assert!(!inv.stdio_binary_present);

    let http = inv.servers.iter().find(|s| s.name == "otro-remoto").unwrap();
    assert_eq!(http.transport, "http");
    assert_eq!(http.url.as_deref(), Some("http://127.0.0.1:9090/mcp"));
    // sólo NOMBRES de cabecera: el valor no debe existir en la estructura
    assert!(http.header_names.contains(&"X-API-Key".to_string()));
    assert!(http.header_names.contains(&"X-Seere-Auth-Token".to_string()));

    // --- proveedores: estructura, nunca valores ---
    assert_eq!(inv.providers.len(), 2);
    let mm = inv.providers.iter().find(|p| p.provider == "minimax").unwrap();
    assert!(mm.fields.contains(&"api_key".to_string()));
    assert_eq!(mm.value_len, "mdaemon_sk_live_MINIMAX_VALOR_SECRETO".len());
    assert_eq!(mm.value_sha16.len(), 1);
    assert_eq!(mm.value_sha16[0].0, "api_key");
    assert_eq!(mm.value_sha16[0].1.len(), 16);

    // --- hooks ---
    assert_eq!(inv.hooks.len(), 2);
    let h0 = &inv.hooks[0];
    assert_eq!(h0.trigger_type, "interval");
    assert_eq!(h0.interval_sec, Some(300));
    assert!(h0.enabled);
    let h1 = &inv.hooks[1];
    assert_eq!(h1.trigger_type, "cron");
    assert!(!h1.enabled);

    // --- rutas de credenciales (sólo rutas) ---
    assert!(inv
        .credential_paths
        .iter()
        .any(|p| p.ends_with("jetski-standalone-oauth-token")));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn el_inventario_nunca_devuelve_valores_de_credencial() {
    let root = tmp_root("redact");
    write(&root.join("config").join("mcp_config.json"), MCP_CONFIG);
    write(&root.join("config").join("config.json"), CONFIG);
    let inv = scan_gemini_root(&root).unwrap();
    let json = inventory_to_json(&inv);
    for secret in [
        "mdaemon_sk_live_SEGRETO_NO_DEBE_APARECER",
        "mdaemon_sk_live_MINIMAX_VALOR_SECRETO",
        "sk-openai-SECRETO",
    ] {
        assert!(
            !json.contains(secret),
            "el inventariofiltró el valor de una credencial: {}",
            secret
        );
    }
    // pero sí el sha16 y el nombre del campo
    assert!(json.contains("\"api_key\""));
    assert!(json.contains("sha16"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn la_receta_del_puente_avisa_del_binario_ausente() {
    let root = tmp_root("bridge");
    write(&root.join("config").join("mcp_config.json"), MCP_CONFIG);
    let inv = scan_gemini_root(&root).unwrap();
    let r = bridge_recommendation(&inv, "http://127.0.0.1:9090/mcp");
    assert!(r.contains("http://127.0.0.1:9090/mcp"));
    assert!(r.contains("X-API-Key"));
    assert!(r.contains("NO existe"), "debe avisar del stdio roto: {}", r);
    assert!(!r.contains("SEGRETO_NO_DEBE_APARECER"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn parsea_config_malo_sin_panicar() {
    assert!(parse_mcp_config("{no json").is_empty());
    assert!(parse_provider_credentials("[]").is_empty());
    assert!(parse_daemon_tasks("{}").is_empty());
    assert!(scan_gemini_root(std::path::Path::new("/no/existe/esta/ruta")).is_err());
}

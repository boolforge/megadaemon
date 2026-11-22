//! **Antigravity usa SIEMPRE MegaDaemon**: el modelo personalizado (BYOM)
//! de la IDE agéntica apunta al gateway canónico del daemon.
//!
//! POR QUÉ ESTE MÓDULO
//! -------------------
//! La GUI de Antigravity (`com.agy`) tiene un catálogo de modelos **cerrado**:
//! sus ids son ofuscados (`MODEL_PLACEHOLDER_M526`, `MODEL_PLACEHOLDER_M402`,
//! ...) y no se pueden apuntar a un proveedor externo.  La vía limpia es el
//! **BYOM** ("Bring Your Own Model"), que la GUI ofrece en el selector de
//! modelo ("Add Custom Model").
//!
//! MEDIDO 2026-11-22 (S21 Ultra del usuario, `liblanguage_server.so` de
//! 181.358.800 B y `web/generated/main.js` de 9.902.890 B):
//!
//! | Qué | Evidencia |
//! |---|---|
//! | El estado de los modelos personalizados es `appState.customModelsConfig.customModels` | `main.js`: `this.customModelsProvider = zi(this.rootProvider, b => ({customModels: b.appState?.customModelsConfig?.customModels ?? {}}))` |
//! | El modelo personalizado se declara con `modelId=GOOGLE_GEMINI_INTERNAL_BYOM` (=326) y `apiProvider=API_PROVIDER_GOOGLE_EVERGREEN` (=30) | `main.js`: `p(I.ModelInfoSchema,{modelId:I.Model.GOOGLE_GEMINI_INTERNAL_BYOM,apiProvider:I.APIProvider.API_PROVIDER_GOOGLE_EVERGREEN,chatModelName:k,modelName:m,maxTokens:...,modelFeatures:L})` |
//! | `chatModelName` es la **etiqueta** y `modelName` la **URL base** del modelo | mismo sitio: `customModelUrl: L.modelName` |
//! | El servidor escribe el estado con `JetboxWriteState` | `main.js`: `this.lsClient.jetboxWriteState(p(mi,{appState:p(Wh,{...})}))` |
//! | Los métodos RPC viven en el MISMO servicio Connect que los demás | `liblanguage_server.so`: `language_server_go_proto._LanguageServerService_JetboxWriteState_Handler` |
//! | El language server habla el **protocolo Gemini** (`genai` SDK), no el de OpenAI | `liblanguage_server.so`: `genai.Models.generateContent` y las rutas `/v1internal:generateContent` / `/v1internal:streamGenerateContent` |
//!
//! ⇒ La cadena completa es:
//! `GUI → JetboxWriteState(appState.customModelsConfig.customModels)` →
//! `language server → POST {base}/v1internal:generateContent` → **el gateway
//! de MegaDaemon** → el proveedor que elija el router.
//!
//! QUÉ HACE ESTE MÓDULO
//! --------------------
//! * Construye **exactamente** el `ModelInfo` que la GUI construye (mismos
//!   nombres de campo, mismos enums) ⇒ lo que se registra aquí es lo que la
//!   GUI pinta.
//! * Lo registra en el language server (`JetboxWriteState`) y lo **lee**
//!   (`JetboxReadAppState`) para poder verificarlo sin JSON de la GUI.
//! * Escribe la **clave** del modelo en `mcp_config.json`
//!   (`providers.<etiqueta>.api_key`), que es donde Antigravity guarda las
//!   credenciales de proveedor (MEDIDO: ese bloque existe en el móvil).
//!
//! REGLAS
//! ------
//! * Nada de secretos en el repo ni en los logs: la clave se **inyecta** en
//!   el momento de escribir la configuración, nunca se guarda en el código.
//! * Todo el parseo es **puro y testeable**; la red vive en el puente.

use crate::antigravity_bridge::{AntigravityBridge, BridgeResult};
use serde_json::{json, Map, Value};

/// `Model.GOOGLE_GEMINI_INTERNAL_BYOM` (=326) — el id del modelo propio.
pub const MODEL_ID_BYOM: &str = "GOOGLE_GEMINI_INTERNAL_BYOM";
/// `APIProvider.API_PROVIDER_GOOGLE_EVERGREEN` (=30) — el proveedor del BYOM.
pub const API_PROVIDER_EVERGREEN: &str = "API_PROVIDER_GOOGLE_EVERGREEN";
/// Método RPC que escribe el estado de la IDE.
pub const RPC_WRITE_STATE: &str = "JetboxWriteState";
/// Método RPC que lee el estado de la IDE.
pub const RPC_READ_APP_STATE: &str = "JetboxReadAppState";
/// Método RPC que **stream**ea el estado (lo usa la GUI para enterarse).
pub const RPC_SUBSCRIBE_STATE: &str = "JetboxSubscribeToState";
/// Fichero de estado de la IDE (proto-text) donde acaba persistido.
pub const AGY_STATE: &str =
    "/data/user/0/com.agy/files/.gemini/antigravity-app/antigravity_state.pbtxt";
/// Configuración MCP + credenciales de proveedor de Antigravity.
pub const AGY_MCP_CONFIG: &str = "/data/user/0/com.agy/files/.gemini/config/mcp_config.json";
/// Etiqueta por defecto del modelo: **`megadaemon`**.
pub const DEFAULT_LABEL: &str = "megadaemon";
/// URL base por defecto: el **gateway canónico** del daemon en el móvil.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:9090";
/// Tope de tokens por defecto del modelo personalizado.
pub const DEFAULT_MAX_TOKENS: u32 = 32_768;

// ---------------------------------------------------------------------------
// Descripción del modelo
// ---------------------------------------------------------------------------

/// Cómo se declara un modelo personalizado en Antigravity.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpec {
    /// Etiqueta que ve el usuario en el selector (`chatModelName`).
    pub label: String,
    /// URL base del modelo (`modelName`): el gateway de MegaDaemon.
    pub base_url: String,
    /// Tope de tokens (`maxTokens`).
    pub max_tokens: u32,
    /// ¿Admite imágenes?
    pub supports_images: bool,
    /// ¿Admite tool calls? (el gateway sí: es lo que hace que Antigravity
    /// pueda usar las tools de MegaDaemon).
    pub supports_tool_calls: bool,
    /// ¿Devuelve razonamiento?
    pub supports_thinking: bool,
}

impl Default for ModelSpec {
    fn default() -> Self {
        Self {
            label: DEFAULT_LABEL.to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            max_tokens: DEFAULT_MAX_TOKENS,
            supports_images: false,
            supports_tool_calls: true,
            supports_thinking: false,
        }
    }
}

impl ModelSpec {
    /// Constructor rápido con la etiqueta y la URL base.
    pub fn new(label: &str, base_url: &str) -> Self {
        Self {
            label: label.to_string(),
            base_url: base_url.to_string(),
            ..Default::default()
        }
    }
}

/// Construye el `ModelInfo` **exactamente** como lo construye la GUI.
///
/// Los nombres de campo son los de la GUI (`modelId`, `apiProvider`,
/// `chatModelName`, `modelName`, `maxTokens`, `modelFeatures`), porque es el
/// mismo proto el que lee y escribe el language server.
pub fn model_info(spec: &ModelSpec) -> Value {
    json!({
        "modelId": MODEL_ID_BYOM,
        "apiProvider": API_PROVIDER_EVERGREEN,
        "chatModelName": spec.label,
        "modelName": spec.base_url,
        "maxTokens": spec.max_tokens,
        "modelFeatures": {
            "zeroShotCapable": true,
            "supportsImages": spec.supports_images,
            "supportsToolCalls": spec.supports_tool_calls,
            "supportsThinking": spec.supports_thinking,
        }
    })
}

/// Construye el cuerpo de `JetboxWriteState` con el mapa de modelos.
///
/// El mapa se **reemplaza** por completo (se lee el actual, se modifica y se
/// escribe el entero) para no dejar modelos a medias.
pub fn app_state_payload(models: &[(String, Value)]) -> Value {
    let mut map = Map::new();
    for (k, v) in models {
        map.insert(k.clone(), v.clone());
    }
    json!({
        "appState": {
            "customModelsConfig": { "customModels": Value::Object(map) }
        }
    })
}

/// Extrae el mapa de modelos personalizados de un `appState`.
pub fn custom_models_from_app_state(state: &Value) -> Vec<(String, Value)> {
    let src = state
        .pointer("/customModelsConfig/customModels")
        .or_else(|| state.pointer("/appState/customModelsConfig/customModels"));
    let mut out = Vec::new();
    if let Some(Value::Object(m)) = src {
        for (k, v) in m {
            out.push((k.clone(), v.clone()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Resumen legible (sin secretos) de un `ModelInfo`.
pub fn summary(info: &Value) -> Value {
    json!({
        "label": info.get("chatModelName").and_then(|v| v.as_str()).unwrap_or(""),
        "model_id": info.get("modelId").and_then(|v| v.as_str()).unwrap_or(""),
        "api_provider": info.get("apiProvider").and_then(|v| v.as_str()).unwrap_or(""),
        "base_url": info.get("modelName").and_then(|v| v.as_str()).unwrap_or(""),
        "max_tokens": info.get("maxTokens").and_then(|v| v.as_u64()).unwrap_or(0),
        "supports_tool_calls": info
            .pointer("/modelFeatures/supportsToolCalls")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "supports_images": info
            .pointer("/modelFeatures/supportsImages")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    })
}

// ---------------------------------------------------------------------------
// IO contra el language server
// ---------------------------------------------------------------------------

/// Registra (o sustituye) un modelo personalizado en Antigravity.
///
/// Es idempotente: si la etiqueta ya existe, se sustituye su definición.
pub fn register(bridge: &AntigravityBridge, spec: &ModelSpec) -> BridgeResult<Value> {
    let mut modelos = current_models(bridge)?;
    let info = model_info(spec);
    modelos.retain(|(k, _)| k != &spec.label);
    modelos.push((spec.label.clone(), info));
    modelos.sort_by(|a, b| a.0.cmp(&b.0));
    bridge.rpc(RPC_WRITE_STATE, &app_state_payload(&modelos))
}

/// Los modelos personalizados que hay **ahora mismo** en la IDE.
pub fn current_models(bridge: &AntigravityBridge) -> BridgeResult<Vec<(String, Value)>> {
    let state = bridge.rpc(RPC_READ_APP_STATE, &json!({}))?;
    Ok(custom_models_from_app_state(&state))
}

/// Quita un modelo personalizado por su etiqueta.
pub fn remove(bridge: &AntigravityBridge, label: &str) -> BridgeResult<Value> {
    let mut modelos = current_models(bridge)?;
    modelos.retain(|(k, _)| k != label);
    bridge.rpc(RPC_WRITE_STATE, &app_state_payload(&modelos))
}

/// ¿Está el modelo `label` registrado con la URL base `base_url`?
pub fn is_registered(bridge: &AntigravityBridge, label: &str, base_url: &str) -> bool {
    current_models(bridge)
        .map(|ms| {
            ms.iter().any(|(k, v)| {
                k == label
                    && v.get("modelName").and_then(|x| x.as_str()).unwrap_or("") == base_url
            })
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Credenciales de proveedor (mcp_config.json)
// ---------------------------------------------------------------------------

/// Devuelve la clave del proveedor `label` del `mcp_config.json` de Antigravity.
pub fn provider_key(config_raw: &str, label: &str) -> Option<String> {
    let v: Value = serde_json::from_str(config_raw).ok()?;
    v.get("providers")?
        .get(label)?
        .get("api_key")
        .or_else(|| v.get("providers")?.get(label)?.get("apiKey"))?
        .as_str()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// Escribe la clave del proveedor `label` en el `mcp_config.json` de Antigravity.
///
/// Devuelve el JSON nuevo (el original se conserva íntegro: el resto de
/// servidores MCP y de proveedores no se tocan).
pub fn patch_provider_key(
    config_raw: &str,
    label: &str,
    api_key: &str,
) -> Result<String, String> {
    let mut v: Value = serde_json::from_str(config_raw)
        .map_err(|e| format!("mcp_config.json no es JSON válido: {e}"))?;
    if !v.is_object() {
        return Err("mcp_config.json no es un objeto".to_string());
    }
    let root = v.as_object_mut().expect("comprobado arriba");
    let providers = root
        .entry("providers".to_string())
        .or_insert_with(|| json!({}));
    if !providers.is_object() {
        *providers = json!({});
    }
    providers
        .as_object_mut()
        .expect("objecto")
        .insert(
            label.to_string(),
            json!({ "api_key": api_key }),
        );
    serde_json::to_string_pretty(&v).map_err(|e| format!("no se puede serializar: {e}"))
}

/// Pide a la GUI el "modelo actual" y lo deja como `megadaemon`.
///
/// **No reinicia nada**: sólo escribe el estado.  Si el language server no
/// está vivo, el error lo dice claro (no se toca el proceso).
pub fn select_as_default(bridge: &AntigravityBridge, model: &str) -> BridgeResult<Value> {
    bridge.rpc(
        "SetUserSettings",
        &json!({ "lastSelectedAgentModel": model }),
    )
}

// ---------------------------------------------------------------------------
// Tool MCP (paridad con el CLI)
// ---------------------------------------------------------------------------

/// Definición de la tool MCP.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "antigravity_model",
            "description": "Modelos personalizados (BYOM) de Antigravity: el 'modelo' de la IDE es SIEMPRE el gateway del MegaDaemon canónico. Acciones: list (los que hay ahora), register (registra 'megadaemon' -> http://127.0.0.1:9090), remove (quita uno) y provider_key (escribe la credencial del proveedor en mcp_config.json). Registrar o quitar MODIFICA el estado de la IDE: exige confirm=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "register", "remove", "provider_key"], "default": "list" },
                    "label": { "type": "string", "default": "megadaemon", "description": "Etiqueta del modelo (chatModelName)." },
                    "base_url": { "type": "string", "default": "http://127.0.0.1:9090", "description": "URL base del modelo (modelName): el gateway de MegaDaemon." },
                    "api_key": { "type": "string", "description": "Credencial del proveedor (sólo para provider_key)." },
                    "config_path": { "type": "string", "description": "Ruta de mcp_config.json (por defecto la de Antigravity en el móvil)." },
                    "confirm": { "type": "boolean", "default": false }
                }
            }
        }
    ])
}

fn wrap(v: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }],
        "structuredContent": v,
    })
}

/// Despacha la tool MCP.  `None` = el nombre no es de este módulo.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    if name != "antigravity_model" {
        return None;
    }
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let accion = {
        let a = s("action");
        if a.trim().is_empty() {
            "list".to_string()
        } else {
            a
        }
    };
    let confirm = args
        .get("confirm")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let label = {
        let l = s("label");
        if l.trim().is_empty() {
            DEFAULT_LABEL.to_string()
        } else {
            l
        }
    };
    let base_url = {
        let u = s("base_url");
        if u.trim().is_empty() {
            DEFAULT_BASE_URL.to_string()
        } else {
            u
        }
    };

    // Las escrituras modifican el estado de la IDE: exigen confirmación.
    if matches!(accion.as_str(), "register" | "remove" | "provider_key") && !confirm {
        return Some(wrap(json!({
            "ok": false,
            "error": "confirm_required",
            "hint": "esta acción modifica el estado de Antigravity: repítela con confirm=true",
        })));
    }

    match accion.as_str() {
        "register" => {
            let cfg = crate::antigravity_bridge::BridgeConfig::default();
            let b = match AntigravityBridge::new(&cfg) {
                Ok(b) => b,
                Err(e) => return Some(wrap(json!({ "ok": false, "error": e.to_string() }))),
            };
            let spec = ModelSpec::new(&label, &base_url);
            match register(&b, &spec) {
                Ok(_) => {
                    let modelos = current_models(&b).unwrap_or_default();
                    Some(wrap(json!({
                        "ok": true,
                        "registrado": summary(&model_info(&spec)),
                        "modelos": modelos.iter().map(|(k, i)| json!({"label": k, "info": summary(i)})).collect::<Vec<_>>(),
                    })))
                }
                Err(e) => Some(wrap(json!({ "ok": false, "error": e.to_string() }))),
            }
        }
        "remove" => {
            let cfg = crate::antigravity_bridge::BridgeConfig::default();
            let b = match AntigravityBridge::new(&cfg) {
                Ok(b) => b,
                Err(e) => return Some(wrap(json!({ "ok": false, "error": e.to_string() }))),
            };
            match remove(&b, &label) {
                Ok(_) => Some(wrap(json!({ "ok": true, "quitado": label }))),
                Err(e) => Some(wrap(json!({ "ok": false, "error": e.to_string() }))),
            }
        }
        "provider_key" => {
            let ruta = {
                let p = s("config_path");
                if p.trim().is_empty() {
                    AGY_MCP_CONFIG.to_string()
                } else {
                    p
                }
            };
            let key = s("api_key");
            if key.trim().is_empty() {
                return Some(wrap(json!({
                    "ok": false,
                    "error": "falta api_key",
                    "hint": "la credencial se inyecta aquí: nunca se guarda en el repo",
                })));
            }
            let crudo = match std::fs::read_to_string(&ruta) {
                Ok(c) => c,
                Err(e) => {
                    return Some(wrap(json!({ "ok": false, "error": format!("no se pudo leer {ruta}: {e}") })))
                }
            };
            let nuevo = match patch_provider_key(&crudo, &label, &key) {
                Ok(n) => n,
                Err(e) => return Some(wrap(json!({ "ok": false, "error": e }))),
            };
            // Respaldo antes de escribir (el fichero es de la app).
            let bak = format!("{ruta}.bak-megadaemon");
            let _ = std::fs::write(&bak, &crudo);
            match std::fs::write(&ruta, &nuevo) {
                Ok(()) => Some(wrap(json!({
                    "ok": true,
                    "ruta": ruta,
                    "respaldo": bak,
                    "proveedor": label,
                    "clave_escrita": true,
                }))),
                Err(e) => Some(wrap(json!({ "ok": false, "error": format!("escribir {ruta}: {e}") }))),
            }
        }
        _ => {
            let cfg = crate::antigravity_bridge::BridgeConfig::default();
            let b = match AntigravityBridge::new(&cfg) {
                Ok(b) => b,
                Err(e) => return Some(wrap(json!({ "ok": false, "error": e.to_string() }))),
            };
            match current_models(&b) {
                Ok(modelos) => Some(wrap(json!({
                    "ok": true,
                    "n": modelos.len(),
                    "modelos": modelos.iter().map(|(k, i)| json!({"label": k, "info": summary(i)})).collect::<Vec<_>>(),
                }))),
                Err(e) => Some(wrap(json!({ "ok": false, "error": e.to_string() }))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_model_info_usa_los_mismos_campos_que_la_gui() {
        let spec = ModelSpec::new("megadaemon", "http://127.0.0.1:9090");
        let v = model_info(&spec);
        assert_eq!(v["modelId"], MODEL_ID_BYOM);
        assert_eq!(v["apiProvider"], API_PROVIDER_EVERGREEN);
        assert_eq!(v["chatModelName"], "megadaemon");
        assert_eq!(v["modelName"], "http://127.0.0.1:9090");
        assert_eq!(v["maxTokens"], DEFAULT_MAX_TOKENS);
        assert_eq!(v["modelFeatures"]["supportsToolCalls"], true);
        assert_eq!(v["modelFeatures"]["supportsImages"], false);
        assert_eq!(v["modelFeatures"]["zeroShotCapable"], true);
    }

    #[test]
    fn el_payload_lleva_el_mapa_bajo_custom_models_config() {
        let modelos = vec![(
            "megadaemon".to_string(),
            model_info(&ModelSpec::default()),
        )];
        let p = app_state_payload(&modelos);
        assert!(p["appState"]["customModelsConfig"]["customModels"]["megadaemon"].is_object());
    }

    #[test]
    fn lee_los_modelos_del_app_state() {
        let state = json!({
            "customModelsConfig": {"customModels": {
                "megadaemon": {"chatModelName": "megadaemon", "modelName": "http://x"},
                "otro": {"chatModelName": "otro"}
            }}
        });
        let ms = custom_models_from_app_state(&state);
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[0].0, "megadaemon");
        // También con la envoltura "appState" (lo que devuelve el RPC).
        let envuelto = json!({"appState": state});
        assert_eq!(custom_models_from_app_state(&envuelto).len(), 2);
    }

    #[test]
    fn app_state_vacio_no_peta() {
        assert!(custom_models_from_app_state(&json!({})).is_empty());
        assert!(custom_models_from_app_state(&Value::Null).is_empty());
    }

    #[test]
    fn el_resumen_no_trae_secretos() {
        let s = summary(&model_info(&ModelSpec::default()));
        assert_eq!(s["label"], "megadaemon");
        assert_eq!(s["base_url"], DEFAULT_BASE_URL);
        assert!(s.get("api_key").is_none());
    }

    #[test]
    fn la_clave_de_proveedor_se_lee_y_se_escribe() {
        let crudo = r#"{"mcpServers":{"x":{"url":"u"}},"providers":{"minimax":{"api_key":"K"}}}"#;
        assert_eq!(provider_key(crudo, "minimax").as_deref(), Some("K"));
        assert!(provider_key(crudo, "otro").is_none());
        let nuevo = patch_provider_key(crudo, "megadaemon", "NUEVA").unwrap();
        assert_eq!(provider_key(&nuevo, "megadaemon").as_deref(), Some("NUEVA"));
        // No se rompe lo que había.
        assert_eq!(provider_key(&nuevo, "minimax").as_deref(), Some("K"));
        assert!(nuevo.contains("\"mcpServers\""));
    }

    #[test]
    fn un_mcp_config_roto_no_peta() {
        assert!(patch_provider_key("{no es json", "x", "y").is_err());
    }
}

//! **Opciones de modelo**: el catálogo del daemon + el catálogo nativo que
//! propone el cliente.
//!
//! ## Qué capacidad se implementa
//!
//! La lista de opciones de modelo que el cliente puede ofrecer al usuario
//! (modelo, etiqueta, proveedor, API que habla, grupo y disponibilidad) y las
//! cuatro operaciones sobre ella: **listar**, **enriquecer** con el catálogo
//! del cliente, **filtrar por proveedor** y **elegir la opción por defecto**.
//!
//! ## Fuente MEDIDA (se documenta, no se transcribe)
//!
//! * `docs/RE_ACP.md:339` — §6.1, fila 13: `cognition.ai/clientProvidedModels`
//!   ⇒ "El cliente enriquece las opciones de modelo con su catálogo nativo".
//! * `docs/RE_ACP.md:340` — §6.1, fila 14: `cognition.ai/groupedSessionConfigOptions`
//!   ⇒ "Opciones de modelo agrupadas en `SessionConfigSelectGroup`".
//! * `docs/RE_ACP_COMPARADO.md:305` — §5: Devin y los demás productos tienen
//!   extensiones propias (`cognition.ai/*`); `docs/RE_ACP_COMPARADO.md:358-362`
//!   — §6.5: las extensiones van en `_meta` con prefijo de producto ⇒ las
//!   **nuestras** van en el namespace `mdaemon`
//!   (`rust-core/src/protocol_acp.rs:73`).
//!
//! ## La implementación es NUESTRA
//!
//! Procedimiento **AST → Rust** del repo (`docs/EXTENSIONES_PROPIETARIAS.md`
//! §4, líneas 51-78): OUR Rust, OUR nombres, OUR estructura; la equivalencia
//! que se busca es de **comportamiento observable** (mismas entradas
//! aceptadas/rechazadas, misma salida, mismos efectos secundarios), NO el
//! mismo texto ni los mismos nombres.
//!
//! ## De dónde sale lo técnico: el catálogo POR MODELO del repo
//!
//! `rust-core/src/model_catalog.rs` es el catálogo por modelo (patrón 5 de
//! `docs/PATRONES_CLI_RUST_DEFINICION.md:165`, regla de `:198`: "los umbrales
//! por modelo se leen del catálogo, no están hardcodeados"): la **familia** (el
//! grupo), el **formateador de tools** (la API que habla) y el **umbral de
//! compactación** son **datos por modelo**, no constantes del llamador.  Este
//! módulo **consume** esa API pública (`familia`, `tool_formatter`,
//! `umbral_compactacion`) y **no reimplementa** nada de ella: si mañana el
//! catálogo cambia, las opciones de modelo cambian con él.
//!
//! ## Integración posterior (la hace el root, NO este fichero)
//!
//! 1. `protocol_acp::extensiones_propias()` ⇒ registrar
//!    `model_options::extensiones()` con el namespace `mdaemon`.
//! 2. `initialize` ⇒ anadir `model_options::initialize_meta(opciones)` en el
//!    `_meta` (bajo la clave del namespace propio).
//! 3. La lista de opciones sale de la bóveda de proveedores: el root mapea
//!    cada entrada a [`DaemonProvider`] (id, nombre, vendor, endpoint, modelo,
//!    habilitado) y llama a [`list_options`].

use crate::model_catalog::{familia, tool_formatter, umbral_compactacion, ToolFormatter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

/// De dónde sale una opción de modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionOrigin {
    /// La opción la trae el daemon (su bóveda de proveedores): el daemon la
    /// puede servir de verdad.
    Daemon,
    /// La opción la propuso el cliente (su catálogo nativo): la muestra el
    /// cliente y el daemon no tiene credenciales para ella.
    Client,
}

impl OptionOrigin {
    /// Identificador estable (para el JSON y los logs).
    pub fn id(&self) -> &'static str {
        match self {
            OptionOrigin::Daemon => "daemon",
            OptionOrigin::Client => "client",
        }
    }
}

/// Una opción de modelo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelOption {
    /// El identificador del modelo (tal cual lo pide el cliente).
    pub model: String,
    /// La etiqueta que se muestra.
    pub label: String,
    /// El proveedor (vendor o id del proveedor de la bóveda).
    pub provider: String,
    /// La API que habla (lo decide el catálogo por modelo).
    pub api: ToolFormatter,
    /// El grupo al que pertenece (las opciones se muestran agrupadas).
    pub group: String,
    /// ¿Está disponible?
    pub available: bool,
    /// De dónde sale la opción.
    pub origin: OptionOrigin,
    /// Umbral de compactación del modelo (del catálogo).
    pub threshold_chars: usize,
}

/// Un proveedor del daemon con los datos **mínimos** que hacen falta para
/// construir una opción de modelo.
///
/// No depende de `providers.rs` a propósito: así el módulo se compila y se
/// testea aislado (sin bóveda, sin disco) y el root sólo tiene que mapear
/// cuatro campos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonProvider {
    /// Id del proveedor en la bóveda.
    pub id: String,
    /// Nombre legible.
    pub name: String,
    /// Quien da el servicio (`openai`, `anthropic`, `google`…).
    pub vendor: String,
    /// El endpoint (decide el dialecto con el que se hablan las tools).
    pub endpoint: String,
    /// El modelo que sirve este proveedor.
    pub model: String,
    /// ¿Está habilitado?
    pub enabled: bool,
}

/// Un modelo propuesto por el cliente (una entrada de su catálogo nativo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientModel {
    /// El identificador del modelo (obligatorio).
    pub model: String,
    /// La etiqueta que muestra el cliente (opcional).
    pub label: Option<String>,
    /// El proveedor (opcional).
    pub provider: Option<String>,
    /// El grupo (opcional).
    pub group: Option<String>,
}

/// Los errores TIPADOS de las opciones de modelo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelOptionError {
    /// Ese modelo no está en el catálogo.
    UnknownModel {
        /// El modelo pedido.
        model: String,
    },
    /// El catálogo que propuso el cliente está mal formado.
    MalformedClientCatalog {
        /// Por qué (qué entrada y qué le pasa).
        reason: String,
    },
}

impl fmt::Display for ModelOptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelOptionError::UnknownModel { model } => {
                write!(f, "modelo desconocido: {model}")
            }
            ModelOptionError::MalformedClientCatalog { reason } => {
                write!(f, "catálogo del cliente mal formado: {reason}")
            }
        }
    }
}

impl std::error::Error for ModelOptionError {}

/// Las opciones del daemon: una por proveedor con modelo.
///
/// * la **API que habla** sale de `model_catalog::tool_formatter` (el endpoint
///   manda sobre el nombre del modelo);
/// * el **grupo** sale de `model_catalog::familia` (los modelos se agrupan por
///   prefijo: `gemini-*`, `claude-*`…);
/// * el **umbral** sale de `model_catalog::umbral_compactacion`.
pub fn list_options(providers: &[DaemonProvider]) -> Vec<ModelOption> {
    providers
        .iter()
        .filter(|p| !p.model.trim().is_empty())
        .map(|p| {
            let model = p.model.trim().to_string();
            let label = if p.name.trim().is_empty() {
                model.clone()
            } else {
                p.name.trim().to_string()
            };
            let provider = if p.vendor.trim().is_empty() {
                p.id.trim().to_string()
            } else {
                p.vendor.trim().to_string()
            };
            ModelOption {
                api: tool_formatter(&model, &p.endpoint),
                group: familia(&model),
                threshold_chars: umbral_compactacion(&model),
                label,
                provider,
                available: p.enabled,
                origin: OptionOrigin::Daemon,
                model,
            }
        })
        .collect()
}

/// Valida y parsea el catálogo que propone el cliente.
///
/// Acepta una lista de objetos (`[{"model": "…"}]`) o un objeto con la clave
/// `models`.  Cada entrada **debe** traer `model` con texto no vacío; el
/// resto es opcional.  Un modelo repetido es un **error**: el cliente quiere
/// decir algo y no lo está diciendo ⇒ se le dice, en vez de fusionar en
/// silencio (un catálogo mal formado no se "arregla" por el camino).
pub fn parse_client_catalog(value: &Value) -> Result<Vec<ClientModel>, ModelOptionError> {
    let items: Vec<&Value> = match value {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) => match o.get("models") {
            Some(Value::Array(a)) => a.iter().collect(),
            Some(_) => {
                return Err(ModelOptionError::MalformedClientCatalog {
                    reason: "'models' no es una lista".to_string(),
                })
            }
            None => {
                return Err(ModelOptionError::MalformedClientCatalog {
                    reason: "falta la lista 'models'".to_string(),
                })
            }
        },
        _ => {
            return Err(ModelOptionError::MalformedClientCatalog {
                reason: "el catálogo del cliente no es una lista".to_string(),
            })
        }
    };
    let mut out: Vec<ClientModel> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| ModelOptionError::MalformedClientCatalog {
                reason: format!("la entrada {i} no es un objeto"),
            })?;
        let model = obj
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if model.is_empty() {
            return Err(ModelOptionError::MalformedClientCatalog {
                reason: format!("la entrada {i} no tiene nombre de modelo"),
            });
        }
        if out.iter().any(|m| m.model == model) {
            return Err(ModelOptionError::MalformedClientCatalog {
                reason: format!("modelo duplicado: {model}"),
            });
        }
        let texto = |k: &str| -> Option<String> {
            obj.get(k)
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };
        out.push(ClientModel {
            model,
            label: texto("label"),
            provider: texto("provider"),
            group: texto("group"),
        });
    }
    Ok(out)
}

/// Enriquece las opciones del daemon con el catálogo del cliente.
///
/// **Regla de fusión (nuestra, documentada)**:
///
/// * si el modelo **ya está** en el catálogo del daemon ⇒ el **daemon gana**
///   en `api`, `group`, `threshold_chars` y `available` (son datos técnicos
///   medidos: el daemon es quien sabe qué API habla y cuándo compacta); el
///   **cliente gana** sólo en `label`, y sólo si aporta alguna (la etiqueta
///   es de quien la muestra);
/// * si el modelo es **nuevo** ⇒ se añade con el dialecto genérico del
///   catálogo (`tool_formatter` sin endpoint) y el umbral del catálogo, con
///   `origin = Client` (el daemon no tiene credenciales para él: lo sirve el
///   cliente);
/// * el orden es estable: primero las del daemon, luego las nuevas del
///   cliente; un modelo **no** aparece dos veces.
pub fn enrich(daemon: &[ModelOption], client: &[ClientModel]) -> Vec<ModelOption> {
    let mut out: Vec<ModelOption> = daemon.to_vec();
    for cm in client {
        if let Some(existente) = out.iter_mut().find(|o| o.model == cm.model) {
            if let Some(label) = &cm.label {
                existente.label = label.clone();
            }
            continue;
        }
        out.push(ModelOption {
            label: cm.label.clone().unwrap_or_else(|| cm.model.clone()),
            provider: cm.provider.clone().unwrap_or_else(|| "client".to_string()),
            api: tool_formatter(&cm.model, ""),
            group: cm.group.clone().unwrap_or_else(|| familia(&cm.model)),
            available: true,
            origin: OptionOrigin::Client,
            threshold_chars: umbral_compactacion(&cm.model),
            model: cm.model.clone(),
        });
    }
    out
}

/// Filtra las opciones por proveedor (sin distinguir mayúsculas).
pub fn filter_by_provider(options: &[ModelOption], provider: &str) -> Vec<ModelOption> {
    let p = provider.trim().to_ascii_lowercase();
    options
        .iter()
        .filter(|o| o.provider.to_ascii_lowercase() == p)
        .cloned()
        .collect()
}

/// Busca una opción por nombre de modelo.
pub fn find<'a>(
    options: &'a [ModelOption],
    model: &str,
) -> Result<&'a ModelOption, ModelOptionError> {
    options
        .iter()
        .find(|o| o.model == model)
        .ok_or_else(|| ModelOptionError::UnknownModel {
            model: model.to_string(),
        })
}

/// Elige la opción por defecto.
///
/// **Regla (nuestra)**: entre las opciones **disponibles**, primero las del
/// **daemon** (las puede servir de verdad); entre ellas, las que hablan una
/// API con **tools estructuradas** (`api != Texto`), porque mandar las tools
/// como texto es gastarlas para que el motor las ignore; y a igualdad, la de
/// **mayor umbral** (más ventana viva antes de compactar).  El orden de
/// entrada decide el último empate (determinista).
pub fn default_option(options: &[ModelOption]) -> Option<ModelOption> {
    let mut best: Option<&ModelOption> = None;
    for option in options.iter().filter(|o| o.available) {
        let better = match best {
            None => true,
            Some(actual) => score(option) > score(actual),
        };
        if better {
            best = Some(option);
        }
    }
    best.cloned()
}

/// La puntuación de la opción por defecto (mayor = mejor).
fn score(option: &ModelOption) -> (u8, u8, usize) {
    let origen = match option.origin {
        OptionOrigin::Daemon => 1u8,
        OptionOrigin::Client => 0u8,
    };
    let con_tools = match option.api {
        ToolFormatter::Texto => 0u8,
        _ => 1u8,
    };
    (origen, con_tools, option.threshold_chars)
}

/// Una opción en JSON (la forma que viaja en el `initialize`).
pub fn option_to_json(option: &ModelOption) -> Value {
    json!({
        "model": option.model,
        "label": option.label,
        "provider": option.provider,
        "api": option.api.as_str(),
        "group": option.group,
        "available": option.available,
        "origin": option.origin.id(),
        "thresholdChars": option.threshold_chars,
    })
}

/// Las opciones agrupadas (la forma `SessionConfigSelectGroup`: un grupo con
/// sus opciones).  El orden de los grupos es el de aparición (estable).
pub fn grouped_to_json(options: &[ModelOption]) -> Vec<Value> {
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for option in options {
        let render = option_to_json(option);
        match groups.iter_mut().find(|(g, _)| *g == option.group) {
            Some((_, lista)) => lista.push(render),
            None => groups.push((option.group.clone(), vec![render])),
        }
    }
    groups
        .into_iter()
        .map(|(group, lista)| json!({ "group": group, "options": lista }))
        .collect()
}

/// El bloque que acaba en el `initialize` del ACP (dentro del `_meta` del
/// namespace propio).
///
/// * `modelOptions`: la lista plana (la que consume un selector simple);
/// * `groupedModelOptions`: la misma lista agrupada por grupo;
/// * `defaultModel`: el modelo por defecto (vacío si no hay ninguna opción
///   disponible).
pub fn initialize_meta(options: &[ModelOption]) -> Value {
    json!({
        "modelOptions": options.iter().map(option_to_json).collect::<Vec<Value>>(),
        "groupedModelOptions": grouped_to_json(options),
        "defaultModel": default_option(options).map(|o| o.model).unwrap_or_default(),
    })
}

/// Las extensiones propias que el root registra con el namespace `mdaemon`.
pub fn extensiones() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "models/options/list",
            "listar las opciones de modelo con su API, su grupo y su umbral de compactación",
        ),
        (
            "models/options/enrich",
            "fusionar el catálogo nativo del cliente con las opciones de modelo del daemon",
        ),
        (
            "models/options/filter",
            "filtrar las opciones de modelo por proveedor",
        ),
        (
            "models/options/default",
            "elegir la opción de modelo por defecto",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proveedor(
        id: &str,
        vendor: &str,
        endpoint: &str,
        model: &str,
        enabled: bool,
    ) -> DaemonProvider {
        DaemonProvider {
            id: id.to_string(),
            name: format!("{id} ({model})"),
            vendor: vendor.to_string(),
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            enabled,
        }
    }

    fn proveedores() -> Vec<DaemonProvider> {
        vec![
            proveedor(
                "google",
                "google",
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                "gemini-3.8-flash",
                true,
            ),
            proveedor(
                "pollinations",
                "pollinations",
                "https://text.pollinations.ai/openai",
                "openai-fast",
                true,
            ),
            proveedor(
                "local",
                "local",
                "http://127.0.0.1:8081/v1/chat/completions",
                "local-seere",
                false,
            ),
        ]
    }

    fn cliente(modelos: &[(&str, Option<&str>)]) -> Vec<ClientModel> {
        modelos
            .iter()
            .map(|(m, l)| ClientModel {
                model: m.to_string(),
                label: l.map(|s| s.to_string()),
                provider: None,
                group: None,
            })
            .collect()
    }

    #[test]
    fn el_listado_devuelve_una_opcion_por_proveedor_con_modelo() {
        let opciones = list_options(&proveedores());
        assert_eq!(opciones.len(), 3);
        assert_eq!(opciones[0].model, "gemini-3.8-flash");
        assert_eq!(opciones[0].provider, "google");
        assert_eq!(opciones[0].label, "google (gemini-3.8-flash)");
        assert_eq!(opciones[0].group, "gemini");
        assert!(opciones[0].available);
        assert_eq!(opciones[0].origin, OptionOrigin::Daemon);
        // El proveedor apagado sale, pero no disponible.
        assert!(!opciones[2].available);
    }

    #[test]
    fn la_api_que_habla_la_sale_del_catalogo_por_modelo() {
        let opciones = list_options(&proveedores());
        // Endpoint de Google ⇒ dialecto nativo, aunque el modelo no se llame
        // "gemini" (el catálogo manda de la API).
        assert_eq!(opciones[0].api, ToolFormatter::GeminiNativo);
        // Endpoint genérico + familia openai ⇒ JSON de OpenAI.
        assert_eq!(opciones[1].api, ToolFormatter::OpenAiJson);
        // Motor local ⇒ las tools van como texto.
        assert_eq!(opciones[2].api, ToolFormatter::Texto);
        // El umbral también sale del catálogo (no de una constante del módulo).
        assert_eq!(
            opciones[0].threshold_chars,
            umbral_compactacion("gemini-3.8-flash")
        );
        assert!(opciones[2].threshold_chars < opciones[0].threshold_chars);
    }

    #[test]
    fn el_enriquecimiento_fusiona_sin_duplicar() {
        let opciones = list_options(&proveedores());
        let cliente = cliente(&[
            ("gemini-3.8-flash", None),
            ("modelo-del-cliente", Some("Cliente bueno")),
        ]);
        let fusion = enrich(&opciones, &cliente);
        assert_eq!(fusion.len(), 4, "3 del daemon + 1 nuevo del cliente");
        let modelos: Vec<&str> = fusion.iter().map(|o| o.model.as_str()).collect();
        assert_eq!(
            modelos.iter().filter(|m| **m == "gemini-3.8-flash").count(),
            1,
            "no se duplica"
        );
        // El nuevo entra con los valores por defecto del catálogo.
        let nuevo = fusion
            .iter()
            .find(|o| o.model == "modelo-del-cliente")
            .unwrap();
        assert_eq!(nuevo.origin, OptionOrigin::Client);
        assert_eq!(nuevo.label, "Cliente bueno");
        assert!(nuevo.available);
        assert_eq!(nuevo.group, familia("modelo-del-cliente"));
    }

    #[test]
    fn en_el_conflicto_gana_el_daemon_en_lo_tecnico_y_el_cliente_en_la_etiqueta() {
        let opciones = list_options(&proveedores());
        let antes = opciones[0].clone();
        let cliente = cliente(&[("gemini-3.8-flash", Some("Gemini del cliente"))]);
        let fusion = enrich(&opciones, &cliente);
        let despues = &fusion[0];
        // Técnico: el daemon manda.
        assert_eq!(despues.api, antes.api);
        assert_eq!(despues.group, antes.group);
        assert_eq!(despues.threshold_chars, antes.threshold_chars);
        assert_eq!(despues.provider, antes.provider);
        assert_eq!(despues.available, antes.available);
        assert_eq!(despues.origin, OptionOrigin::Daemon);
        // Presentación: el cliente manda.
        assert_eq!(despues.label, "Gemini del cliente");
    }

    #[test]
    fn el_filtro_por_proveedor_solo_devuelve_las_suyas() {
        let opciones = list_options(&proveedores());
        let google = filter_by_provider(&opciones, "google");
        assert_eq!(google.len(), 1);
        assert_eq!(google[0].model, "gemini-3.8-flash");
        // Sin distinguir mayúsculas.
        assert_eq!(filter_by_provider(&opciones, "GOOGLE").len(), 1);
        assert!(filter_by_provider(&opciones, "microsoft").is_empty());
    }

    #[test]
    fn la_opcion_por_defecto_prefiere_el_daemon_con_tools_estructuradas() {
        let opciones = list_options(&proveedores());
        // El local está apagado, así que no puede ganar.
        let por_defecto = default_option(&opciones).expect("hay opciones disponibles");
        assert_eq!(por_defecto.model, "gemini-3.8-flash");
        assert_eq!(por_defecto.origin, OptionOrigin::Daemon);
        assert_ne!(por_defecto.api, ToolFormatter::Texto);
    }

    #[test]
    fn la_opcion_por_defecto_cae_al_cliente_si_el_daemon_no_tiene_nada_usable() {
        let opciones = list_options(&vec![proveedor(
            "local",
            "local",
            "http://127.0.0.1:8081/v1/chat/completions",
            "local-seere",
            false,
        )]);
        assert!(
            default_option(&opciones).is_none(),
            "no hay nada disponible"
        );
        let fusion = enrich(&opciones, &cliente(&[("gpt-propio", None)]));
        let por_defecto = default_option(&fusion).expect("el cliente aporta una");
        assert_eq!(por_defecto.model, "gpt-propio");
        assert_eq!(por_defecto.origin, OptionOrigin::Client);
    }

    #[test]
    fn un_modelo_desconocido_es_un_error_tipado() {
        let opciones = list_options(&proveedores());
        match find(&opciones, "no-existe") {
            Err(ModelOptionError::UnknownModel { model }) => assert_eq!(model, "no-existe"),
            otro => panic!("se esperaba UnknownModel, llegó {otro:?}"),
        }
        assert!(find(&opciones, "gemini-3.8-flash").is_ok());
    }

    #[test]
    fn un_catalogo_del_cliente_mal_formado_es_un_error_tipado() {
        // Sin nombre de modelo.
        match parse_client_catalog(&json!([{ "label": "sin nombre" }])) {
            Err(ModelOptionError::MalformedClientCatalog { reason }) => {
                assert!(reason.contains("nombre"), "el error lo dice: {reason}")
            }
            otro => panic!("se esperaba MalformedClientCatalog, llegó {otro:?}"),
        }
        // Duplicado.
        match parse_client_catalog(&json!([
            { "model": "x" },
            { "model": "x" }
        ])) {
            Err(ModelOptionError::MalformedClientCatalog { reason }) => {
                assert!(reason.contains("duplicado"), "el error lo dice: {reason}")
            }
            otro => panic!("se esperaba MalformedClientCatalog, llegó {otro:?}"),
        }
        // Forma que no es lista.
        match parse_client_catalog(&json!({ "models": "x" })) {
            Err(ModelOptionError::MalformedClientCatalog { .. }) => {}
            otro => panic!("se esperaba MalformedClientCatalog, llegó {otro:?}"),
        }
        // Un catálogo bien formado se parsea.
        let ok = parse_client_catalog(&json!({ "models": [{ "model": "x", "label": "X" }] }))
            .expect("catálogo válido");
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].model, "x");
        assert_eq!(ok[0].label.as_deref(), Some("X"));
    }

    #[test]
    fn la_serializacion_tiene_la_forma_medida() {
        let opciones = list_options(&proveedores());
        let meta = initialize_meta(&opciones);
        assert_eq!(meta["modelOptions"].as_array().unwrap().len(), 3);
        let primera = &meta["modelOptions"][0];
        assert_eq!(primera["model"], json!("gemini-3.8-flash"));
        assert_eq!(primera["api"], json!("gemini_nativo"));
        assert_eq!(primera["group"], json!("gemini"));
        assert_eq!(primera["available"], json!(true));
        assert_eq!(primera["origin"], json!("daemon"));
        assert_eq!(primera["thresholdChars"], json!(12_000));
        assert_eq!(meta["defaultModel"], json!("gemini-3.8-flash"));
        // Agrupadas: un grupo por familia.
        let grupos = meta["groupedModelOptions"].as_array().unwrap();
        assert_eq!(grupos.len(), 3);
        assert_eq!(grupos[0]["group"], json!("gemini"));
        assert_eq!(grupos[0]["options"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn la_agrupacion_junta_las_opciones_del_mismo_grupo() {
        let opciones = list_options(&vec![
            proveedor(
                "a",
                "google",
                "https://generativelanguage.googleapis.com/v1beta",
                "gemini-a",
                true,
            ),
            proveedor(
                "b",
                "google",
                "https://generativelanguage.googleapis.com/v1beta",
                "gemini-b",
                true,
            ),
        ]);
        let grupos = grouped_to_json(&opciones);
        assert_eq!(grupos.len(), 1, "las dos son del mismo grupo");
        assert_eq!(grupos[0]["group"], json!("gemini"));
        assert_eq!(grupos[0]["options"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn las_extensiones_propias_no_se_repiten() {
        let ext = extensiones();
        assert_eq!(ext.len(), 4);
        let mut ids: Vec<&str> = ext.iter().map(|(m, _)| *m).collect();
        ids.sort();
        let antes = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), antes, "los métodos se repiten");
        for (metodo, _) in &ext {
            assert!(metodo.starts_with("models/"), "namespace propio: {metodo}");
        }
    }
}

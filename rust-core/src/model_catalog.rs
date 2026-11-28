//! Catálogo **POR MODELO**: formateador de tools y umbral de compactación.
//!
//! MEDIDO (2026-11-28, `docs/RE_DEVIN_CLI_RUST.md` §3): en el CLI de
//! referencia los **umbrales de compactación** y el **`tool_formatter_type`**
//! son **POR MODELO**, no globales: el formateador decide cómo se le
//! mandan las tools a ese modelo y el umbral cuándo se compacta su
//! conversación.
//!
//! En MegaDaemon los dos estaban **hardcodeados**:
//!
//! * el presupuesto de la cascada era uno solo para todos los modelos
//!   (`providers::PROMPT_MAX_CHARS`), y
//! * la forma de las tools se decidía dentro de `providers.rs`, mirando el
//!   endpoint (y el camino nativo de Gemini se escribió aparte, con su
//!   propio `if`).
//!
//! Aquí los dos son **DATOS**: una tabla por familia de modelo, con la
//! API que habla como fuente de verdad y el umbral por modelo.  Todo es
//! **puro** (sin red, sin estado global) ⇒ se testea offline.
//!
//! Patrón 5 del repo (`docs/PATRONES_CLI_RUST_DEFINICION.md`, §b y §c):
//! "Los umbrales por modelo se leen del catálogo, no están hardcodeados".

use serde::{Deserialize, Serialize};

/// Cómo se le mandan las **tools** a un modelo.
///
/// Cada familia de API habla un dialecto distinto: mandarle el JSON de
/// OpenAI a Gemini es un 400, y no darle tools a un motor que no las
/// entiende es gastar tokens explicándolas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolFormatter {
    /// JSON de tool-calling de OpenAI: `tools: [{type:"function", function:{…}}]`.
    OpenAiJson,
    /// Nativo de Gemini: `tools: [{functionDeclarations:[…]}]`.
    GeminiNativo,
    /// Anthropic: `tools: [{name, description, input_schema}]`.
    AnthropicTools,
    /// El motor no entiende tools estructuradas: van como TEXTO en el prompt.
    Texto,
}

impl ToolFormatter {
    /// Nombre estable (para logs, API y tests).
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolFormatter::OpenAiJson => "openai_json",
            ToolFormatter::GeminiNativo => "gemini_nativo",
            ToolFormatter::AnthropicTools => "anthropic_tools",
            ToolFormatter::Texto => "texto",
        }
    }
}

/// Perfil de un modelo: los dos datos que el patrón 5 dice que son
/// **por modelo** y no globales.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerfilModelo {
    /// El modelo consultado (tal cual lo pidió el cliente).
    pub modelo: String,
    /// La familia a la que pertenece (`gemini`, `claude`, `qwen`…).
    pub familia: String,
    /// Cómo se le mandan las tools.
    pub formatter: ToolFormatter,
    /// Cuándo se compacta su conversación (en caracteres de la vista viva).
    pub umbral_compactacion_chars: usize,
}

/// Umbral de compactación por defecto.
///
/// MEDIDO (2026-11-27, móvil): `pollinations-free` (API clásica, sin key)
/// acepta **16.000 caracteres** contando mensajes **y** tools, y el
/// presupuesto de la cascada son 12.000 (`providers::PROMPT_MAX_CHARS`):
/// es el número que cabe en el proveedor **más estricto** ⇒ es el umbral
/// que sirve para todos mientras el catálogo no diga otra cosa.
pub const UMBRAL_POR_DEFECTO: usize = 12_000;

/// Tabla de umbrales por familia de modelo.
///
/// MEDIDO: el valor por defecto es el de arriba (12.000).  Las familias
/// **offline** usan la mitad porque el motor local (SEERE, GBNF) trabaja
/// con una ventana mucho más pequeña que un modelo de nube: mandar 12.000
/// caracteres al motor local es gastar contexto sin que lo entienda.
const UMBRALES: &[(&str, usize)] = &[
    ("local", 6_000),
    ("qwen", 8_000),
    ("llama", 8_000),
    ("gemini", 12_000),
    ("claude", 12_000),
    ("openai", 12_000),
    ("grok", 12_000),
];

/// La familia de un modelo (por prefijo del nombre).
///
/// MEDIDO (catálogo de proveedores): los modelos que usa MegaDaemon se
/// agrupan por prefijo (`gemini-*`, `claude-*`, `qwen*`, `llama*`…).
pub fn familia(modelo: &str) -> String {
    let m = modelo.trim().to_ascii_lowercase();
    let f = if m.starts_with("gemini") {
        "gemini"
    } else if m.starts_with("claude") {
        "claude"
    } else if m.starts_with("gpt") || m.starts_with("o1") || m.starts_with("o3") {
        "openai"
    } else if m.starts_with("qwen") {
        "qwen"
    } else if m.starts_with("llama") {
        "llama"
    } else if m.starts_with("grok") {
        "grok"
    } else if m.starts_with("local") || m.starts_with("seere") {
        "local"
    } else {
        "otro"
    };
    f.to_string()
}

/// El formateador que le corresponde a un **endpoint**.
///
/// La API que habla es la fuente de verdad: aunque el modelo se llame
/// `gemini`, si el endpoint es el de Google hay que usar el dialecto
/// nativo, y si es un proxy OpenAI-compatible hay que usar el JSON de
/// OpenAI.
pub fn formatter_de_endpoint(endpoint: &str) -> ToolFormatter {
    let e = endpoint.to_ascii_lowercase();
    if e.contains("generativelanguage.googleapis.com") {
        ToolFormatter::GeminiNativo
    } else if e.contains("api.anthropic.com") {
        ToolFormatter::AnthropicTools
    } else {
        ToolFormatter::OpenAiJson
    }
}

/// El formateador de tools de un modelo **en un endpoint**.
///
/// 1. Si el endpoint es de una API con dialecto propio (Gemini, Anthropic),
///    manda la API.
/// 2. Si no, manda la familia del modelo: los motores **offline** (`local`,
///    `seere`) no entienden tools estructuradas ⇒ van como texto.
pub fn tool_formatter(modelo: &str, endpoint: &str) -> ToolFormatter {
    let por_endpoint = formatter_de_endpoint(endpoint);
    if por_endpoint != ToolFormatter::OpenAiJson {
        return por_endpoint;
    }
    match familia(modelo).as_str() {
        "local" => ToolFormatter::Texto,
        _ => ToolFormatter::OpenAiJson,
    }
}

/// El umbral de compactación de un modelo (en caracteres).
///
/// Sale del catálogo, no de una constante hardcodeada en el llamador.
pub fn umbral_compactacion(modelo: &str) -> usize {
    let f = familia(modelo);
    UMBRALES
        .iter()
        .find(|(nombre, _)| *nombre == f)
        .map(|(_, v)| *v)
        .unwrap_or(UMBRAL_POR_DEFECTO)
}

/// El perfil completo de un modelo en un endpoint.
pub fn perfil(modelo: &str, endpoint: &str) -> PerfilModelo {
    PerfilModelo {
        modelo: modelo.to_string(),
        familia: familia(modelo),
        formatter: tool_formatter(modelo, endpoint),
        umbral_compactacion_chars: umbral_compactacion(modelo),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_familia_se_saca_del_prefijo_del_modelo() {
        assert_eq!(familia("gemini-3.8-flash"), "gemini");
        assert_eq!(familia("claude-sonnet-4"), "claude");
        assert_eq!(familia("gpt-oss-120b"), "openai");
        assert_eq!(familia("qwen3.6-35b-a3b"), "qwen");
        assert_eq!(familia("llama-3.1-8b"), "llama");
        assert_eq!(familia("local-seere"), "local");
        assert_eq!(familia("  GEMINI-2.5-PRO "), "gemini");
        assert_eq!(familia("modelo-raro"), "otro");
    }

    #[test]
    fn la_api_manda_en_el_formateador() {
        // MEDIDO (2026-11-28): con el endpoint OpenAI-compat, Google
        // devolvia HTTP 400 por la ESTRUCTURA => el dialecto nativo no es
        // negociable.
        assert_eq!(
            formatter_de_endpoint(
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
            ),
            ToolFormatter::GeminiNativo
        );
        assert_eq!(
            formatter_de_endpoint("https://api.anthropic.com/v1/messages"),
            ToolFormatter::AnthropicTools
        );
        assert_eq!(
            formatter_de_endpoint("https://api.groq.com/openai/v1/chat/completions"),
            ToolFormatter::OpenAiJson
        );
    }

    #[test]
    fn el_formateador_depende_del_modelo_cuando_el_endpoint_es_generico() {
        // El motor local no entiende tools estructuradas.
        assert_eq!(
            tool_formatter("local-seere", "http://127.0.0.1:8081/v1/chat/completions"),
            ToolFormatter::Texto
        );
        assert_eq!(
            tool_formatter("gpt-oss-120b", "https://api.groq.com/openai/v1/chat/completions"),
            ToolFormatter::OpenAiJson
        );
        // Pero si el endpoint es de Gemini, manda la API.
        assert_eq!(
            tool_formatter("gpt-oss-120b", "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"),
            ToolFormatter::GeminiNativo
        );
    }

    #[test]
    fn el_umbral_de_compactacion_sale_del_catalogo() {
        // MEDIDO: el defecto es el presupuesto que cabe en el proveedor
        // mas estricto (16.000 con margen => 12.000).
        assert_eq!(umbral_compactacion("gemini-3.8-flash"), 12_000);
        assert_eq!(umbral_compactacion("claude-sonnet-4"), 12_000);
        assert_eq!(umbral_compactacion("modelo-raro"), UMBRAL_POR_DEFECTO);
        // Los motores offline/workshop tienen menos ventana.
        assert!(umbral_compactacion("local-seere") < UMBRAL_POR_DEFECTO);
    }

    #[test]
    fn el_umbral_por_defecto_cabe_en_el_proveedor_mas_estricto() {
        // Si el umbral por defecto se pasara del limite del proveedor
        // estricto, la compactacion llegaria tarde.
        assert!(UMBRAL_POR_DEFECTO <= crate::providers::PROMPT_LIMITE_ESTRICTO);
    }

    #[test]
    fn el_perfil_reune_los_dos_datos_por_modelo() {
        let p = perfil(
            "gemini-3.8-flash",
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        );
        assert_eq!(p.familia, "gemini");
        assert_eq!(p.formatter, ToolFormatter::GeminiNativo);
        assert_eq!(p.umbral_compactacion_chars, 12_000);
        assert_eq!(p.formatter.as_str(), "gemini_nativo");
    }

    #[test]
    fn el_proveedor_anthropic_del_catalogo_habla_el_dialecto_de_anthropic() {
        // MEDIDO (2026-11-28): el catálogo de proveedores tiene a Anthropic
        // como proveedor de primera clase (endpoint `api.anthropic.com`),
        // así que el formateador del catálogo le tiene que asignar SU
        // dialecto.  Si esto se rompe, el catálogo y el código dejan de
        // hablar el mismo idioma (y el proveedor responde HTTP 400).
        let anthropic = crate::provider_catalog::catalog_v13_1()
            .into_iter()
            .find(|p| p.id == "anthropic")
            .expect("el catálogo tiene a anthropic");
        assert_eq!(
            formatter_de_endpoint(&anthropic.endpoint),
            ToolFormatter::AnthropicTools,
            "el endpoint de Anthropic habla su dialecto"
        );
        assert_eq!(
            tool_formatter(&anthropic.model, &anthropic.endpoint),
            ToolFormatter::AnthropicTools
        );
    }

    /// Quita los comentarios del código (mismo criterio que `contexto.rs`).
    ///
    /// Sin esto, la propia documentación del módulo daría falsos positivos
    /// al buscar una llamada en el código.
    fn sin_comentarios(src: &str) -> String {
        let c: Vec<char> = src.chars().collect();
        let mut out = String::with_capacity(src.len());
        let mut i = 0usize;
        while i < c.len() {
            let ch = c[i];
            // Comentario de línea.
            if ch == '/' && i + 1 < c.len() && c[i + 1] == '/' {
                i += 2;
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            // Comentario de bloque (anidado).
            if ch == '/' && i + 1 < c.len() && c[i + 1] == '*' {
                let mut nivel = 1usize;
                i += 2;
                while i < c.len() && nivel > 0 {
                    if c[i] == '/' && i + 1 < c.len() && c[i + 1] == '*' {
                        nivel += 1;
                        i += 2;
                        continue;
                    }
                    if c[i] == '*' && i + 1 < c.len() && c[i + 1] == '/' {
                        nivel -= 1;
                        i += 2;
                        continue;
                    }
                    if c[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
                continue;
            }
            // Cadena de texto o literal de carácter: se copian enteros.
            if ch == '"' || ch == '\'' {
                let delim = ch;
                out.push(ch);
                i += 1;
                while i < c.len() {
                    let d = c[i];
                    out.push(d);
                    i += 1;
                    if d == '\\' && i < c.len() {
                        out.push(c[i]);
                        i += 1;
                        continue;
                    }
                    if d == delim {
                        break;
                    }
                }
                continue;
            }
            out.push(ch);
            i += 1;
        }
        out
    }

    #[test]
    fn la_tool_de_compactacion_toma_el_umbral_del_catalogo() {
        // El umbral por modelo llega al llamador por la tool MCP
        // `compaction_epoch` (`mcp_ext.rs`).  Ese módulo no se puede aislar
        // (arrastra medio crate), así que el **cableado** se comprueba sobre
        // el código, como hace `contexto.rs` con el suyo.
        //
        // Las palabras se construyen en runtime para que este test no las
        // lleve escritas en el fichero (si no, el grep se encontraría a sí
        // mismo).
        let llamadas: Vec<String> = vec![["model_catalog::umbral_", "compactacion"].concat()];
        let codigo = sin_comentarios(include_str!("mcp_ext.rs"));
        for llamada in &llamadas {
            assert!(
                codigo.contains(llamada.as_str()),
                "la tool de compactación debe leer el umbral del CATÁLOGO (\'{}\' no aparece en el código de mcp_ext.rs)",
                llamada
            );
        }
    }

    #[test]
    fn el_formatter_tiene_nombre_estable() {
        assert_eq!(ToolFormatter::OpenAiJson.as_str(), "openai_json");
        assert_eq!(ToolFormatter::AnthropicTools.as_str(), "anthropic_tools");
        assert_eq!(ToolFormatter::Texto.as_str(), "texto");
    }
}

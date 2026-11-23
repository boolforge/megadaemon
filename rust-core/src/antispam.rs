//! MegaDaemon v13.4 — Filtro antispam **entrante** (SMS y llamadas).
//!
//! Esto es **defensivo**: clasifica y bloquea **lo que entra** en el buzón y
//! en el registro de llamadas del propio usuario. No envía nada a terceros.
//!
//! ## Lo que decide
//!
//! | Decisión    | Qué significa                                              |
//! |-------------|------------------------------------------------------------|
//! | `Accept`    | mensaje normal: se deja pasar                              |
//! | `Quarantine`| dudoso: se aparta (carpeta de cuarentena) pero no se tira  |
//! | `Block`     | fraude/spam: se bloquea                                    |
//!
//! ## La puntuación NO es una caja negra
//!
//! [`SpamScore`] lleva el **desglose**: cada [`Signal`] dice **qué regla
//! saltó**, **cuánto pesó** y **por qué** (con el detalle: qué palabra clave,
//! qué URL, qué contacto se parecía). El total es la suma de los pesos.
//!
//! ## Reglas
//!
//! * remitente en la **lista negra** (peso alto) o en la **lista blanca**;
//! * **número corto** o **prefijo premium**;
//! * **plantilla de phishing** ("premio", "ganaste", "haz clic", "verifica tu
//!   cuenta", "WhatsApp"…) y **urgencia con amenazas** ("urgente",
//!   "suspender", "última hora");
//! * **enlaces acortados** (bit.ly, tinyurl…);
//! * **frecuencia anómala**: muchos mensajes del mismo remitente en poco
//!   tiempo;
//! * **número confundible**: Levenshtein contra los contactos del usuario
//!   (0/O, 1/l, 5/S, rn/m…);
//! * **coherencia baja**: mensaje o transcripción de voz con muy poca
//!   variedad de palabras.
//!
//! ## Listas persistentes
//!
//! Viven en `<state_dir>/antispam_lists.json` con permisos **0600**, y cada
//! entrada guarda **el motivo** de la decisión (para poder revisarla después).
//!
//! ## Dependencias
//!
//! Este módulo es **autocontenido** (no depende de `telecom` ni de `otp`):
//! normaliza y compara números por su cuenta para poder cablearlo y
//! probarlo solo.

use serde::{Deserialize, Serialize};
use std::fs;

// ---------------------------------------------------------------------------
// Puntuación
// ---------------------------------------------------------------------------

/// Una señal que ha saltado en el análisis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    /// Nombre de la regla (estable, para logs y para filtros).
    pub rule: String,
    /// Peso que aporta al total (puede ser negativo: la lista blanca resta).
    pub weight: f32,
    /// Detalle de **por qué** saltó.
    pub detail: String,
}

impl Signal {
    /// Constructor de una señal.
    pub fn new(rule: &str, weight: f32, detail: &str) -> Self {
        Signal {
            rule: rule.to_string(),
            weight,
            detail: detail.to_string(),
        }
    }
}

/// Puntuación de spam con su **desglose**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpamScore {
    /// Total: la suma de los pesos de las señales.
    pub total: f32,
    /// Señales que han saltado.
    pub signals: Vec<Signal>,
}

impl SpamScore {
    /// Puntuación vacía.
    pub fn empty() -> Self {
        SpamScore {
            total: 0.0,
            signals: Vec::new(),
        }
    }

    /// Añade una señal y suma su peso al total.
    pub fn push(&mut self, signal: Signal) {
        self.total += signal.weight;
        self.signals.push(signal);
    }

    /// ¿Ha saltado alguna señal con este nombre de regla?
    pub fn has_rule(&self, rule: &str) -> bool {
        self.signals.iter().any(|s| s.rule == rule)
    }

    /// Peso acumulado de una regla.
    pub fn rule_weight(&self, rule: &str) -> f32 {
        self.signals
            .iter()
            .filter(|s| s.rule == rule)
            .map(|s| s.weight)
            .sum()
    }

    /// Texto con el desglose, línea a línea.
    pub fn explain(&self) -> String {
        let mut out = format!("total {:.2}\n", self.total);
        for s in &self.signals {
            out.push_str(&format!(
                "  {:+.2}  {:<22} {}\n",
                s.weight, s.rule, s.detail
            ));
        }
        out
    }
}

/// Decisión sobre un mensaje o una llamada entrante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// Se deja pasar.
    Accept,
    /// Se aparta (cuarentena), sin tirarlo.
    Quarantine,
    /// Se bloquea.
    Block,
}

impl Decision {
    /// Etiqueta estable (para logs y para el MCP).
    pub fn label(&self) -> &'static str {
        match self {
            Decision::Accept => "accept",
            Decision::Quarantine => "quarantine",
            Decision::Block => "block",
        }
    }

    /// ¿Se permite que pase al usuario?
    pub fn allowed(&self) -> bool {
        matches!(self, Decision::Accept)
    }
}

/// Puntuación + decisión.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpamVerdict {
    /// Puntuación con desglose.
    pub score: SpamScore,
    /// Decisión tomada.
    pub decision: Decision,
}

// ---------------------------------------------------------------------------
// Pesos y umbrales
// ---------------------------------------------------------------------------

/// Peso de cada regla (configurable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalWeights {
    /// Remitente en la lista negra.
    pub blacklist: f32,
    /// Remitente en la lista blanca (peso negativo).
    pub whitelist: f32,
    /// Número corto (centro de mensajes / premium corto).
    pub short_code: f32,
    /// Prefijo de tarificación especial.
    pub premium_prefix: f32,
    /// Cada palabra clave de plantilla de phishing.
    pub phishing_keyword: f32,
    /// Tope del bloque de "plantilla de phishing".
    pub phishing_cap: f32,
    /// Cada palabra de urgencia/amenaza.
    pub urgency_threat: f32,
    /// Tope del bloque de urgencia.
    pub urgency_cap: f32,
    /// Enlace acortado.
    pub shortened_url: f32,
    /// Cualquier URL.
    pub generic_url: f32,
    /// Tope del bloque de URLs.
    pub url_cap: f32,
    /// Frecuencia anómala.
    pub frequency: f32,
    /// Número confundible con un contacto.
    pub confusable: f32,
    /// Coherencia baja (voz o mensaje).
    pub low_coherence: f32,
}

impl Default for SignalWeights {
    fn default() -> Self {
        SignalWeights {
            blacklist: 1.0,
            whitelist: -1.0,
            short_code: 0.30,
            premium_prefix: 0.20,
            phishing_keyword: 0.18,
            phishing_cap: 0.45,
            urgency_threat: 0.35,
            urgency_cap: 0.45,
            shortened_url: 0.45,
            generic_url: 0.10,
            url_cap: 0.20,
            frequency: 0.40,
            confusable: 0.55,
            low_coherence: 0.30,
        }
    }
}

/// Umbrales y parámetros del filtro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpamConfig {
    /// A partir de este total: cuarentena.
    pub quarantine_threshold: f32,
    /// A partir de este total: bloqueo.
    pub block_threshold: f32,
    /// Ventana de la frecuencia anómala (ms).
    pub frequency_window_ms: i64,
    /// Mensajes del mismo remitente en la ventana para considerarlo anómalo.
    pub frequency_threshold: usize,
    /// Distancia máxima de Levenshtein para considerarlo "confundible".
    pub confusable_max_distance: usize,
    /// Pesos por regla.
    pub weights: SignalWeights,
}

impl Default for SpamConfig {
    fn default() -> Self {
        SpamConfig {
            quarantine_threshold: 0.35,
            block_threshold: 0.80,
            frequency_window_ms: 10 * 60 * 1000,
            frequency_threshold: 3,
            confusable_max_distance: 2,
            weights: SignalWeights::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Mensajes y llamadas entrantes
// ---------------------------------------------------------------------------

/// Un SMS entrante.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboundSms {
    /// Remitente.
    pub sender: String,
    /// Cuerpo del mensaje.
    pub body: String,
    /// Marca de tiempo (ms epoch).
    pub timestamp: i64,
    /// Identificador del mensaje (opcional).
    pub message_id: String,
}

impl InboundSms {
    /// Constructor con los campos esenciales.
    pub fn new(sender: &str, body: &str, timestamp: i64) -> Self {
        InboundSms {
            sender: sender.to_string(),
            body: body.to_string(),
            timestamp,
            message_id: String::new(),
        }
    }
}

/// Una llamada entrante (con transcripción opcional).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboundCall {
    /// Número que llama.
    pub number: String,
    /// Transcripción de la voz, si la hay.
    pub transcript: Option<String>,
    /// Marca de tiempo (ms epoch).
    pub timestamp: i64,
    /// Duración de la llamada (ms), si se sabe.
    pub duration_ms: Option<u64>,
}

impl InboundCall {
    /// Constructor con los campos esenciales.
    pub fn new(number: &str, transcript: Option<&str>, timestamp: i64) -> Self {
        InboundCall {
            number: number.to_string(),
            transcript: transcript.map(|t| t.to_string()),
            timestamp,
            duration_ms: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Listas persistentes
// ---------------------------------------------------------------------------

/// Una entrada de lista, con su motivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListEntry {
    /// Número (tal como se guardó).
    pub number: String,
    /// Motivo de la entrada (obligatorio: para poder revisarla).
    pub reason: String,
    /// Cuándo se añadió (ms epoch).
    pub added_at: i64,
}

/// Listas negra y blanca del usuario.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SpamLists {
    /// Lista negra.
    pub blacklist: Vec<ListEntry>,
    /// Lista blanca.
    pub whitelist: Vec<ListEntry>,
}

/// Nombre del fichero de listas dentro del `state_dir`.
pub const SPAM_LISTS_FILE: &str = "antispam_lists.json";

impl SpamLists {
    /// Ruta del fichero de listas.
    pub fn path(state_dir: &str) -> String {
        format!("{state_dir}/{SPAM_LISTS_FILE}")
    }

    /// Carga las listas del `state_dir` (si no hay fichero, listas vacías).
    ///
    /// Si el fichero está corrupto, se devuelven listas vacías (mejor un
    /// filtro sin listas que un daemon que no arranca).
    pub fn load(state_dir: &str) -> SpamLists {
        let path = SpamLists::path(state_dir);
        match fs::read_to_string(&path) {
            Ok(txt) => serde_json::from_str(&txt).unwrap_or_default(),
            Err(_) => SpamLists::default(),
        }
    }

    /// Guarda las listas en el `state_dir` con permisos **0600**.
    ///
    /// MEDIDO (2026-11-23, auditoría de calidad): antes usaba
    /// `fs::write` + `set_mode_600` (que además ignoraba el error del
    /// chmod) ⇒ el fichero se creaba con el modo del umask (0644) y sólo
    /// después se restrictivo: hay una ventana en la que las listas (números
    /// bloqueados, reglas del filtro) están legibles por cualquiera.  Ahora
    /// el temporal se crea CON el modo 0600 y se renombra.
    pub fn save(&self, state_dir: &str) -> Result<(), String> {
        fs::create_dir_all(state_dir).map_err(|e| format!("mkdir {state_dir}: {e}"))?;
        let path = SpamLists::path(state_dir);
        let txt = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serializando las listas: {e}"))?;
        crate::util_fs::write_atomic(std::path::Path::new(&path), txt.as_bytes(), 0o600)
            .map_err(|e| format!("escribiendo {path}: {e}"))
    }

    /// Añade un número a la lista negra (sólo en memoria).
    ///
    /// Para que además se guarde, usa [`SpamLists::add_blacklist_in`].
    pub fn add_blacklist(&mut self, number: &str, reason: &str) {
        let key = national_key(number);
        self.blacklist.retain(|e| national_key(&e.number) != key);
        self.blacklist.push(ListEntry {
            number: number.to_string(),
            reason: reason.to_string(),
            added_at: now_ms(),
        });
    }

    /// Quita un número de la lista negra (sólo en memoria).
    pub fn remove_blacklist(&mut self, number: &str) -> bool {
        let key = national_key(number);
        let before = self.blacklist.len();
        self.blacklist.retain(|e| national_key(&e.number) != key);
        self.blacklist.len() != before
    }

    /// Añade un número a la lista blanca (sólo en memoria).
    ///
    /// Para que además se guarde, usa [`SpamLists::add_whitelist_in`].
    pub fn add_whitelist(&mut self, number: &str, reason: &str) {
        let key = national_key(number);
        self.whitelist.retain(|e| national_key(&e.number) != key);
        self.whitelist.push(ListEntry {
            number: number.to_string(),
            reason: reason.to_string(),
            added_at: now_ms(),
        });
    }

    /// Quita un número de la lista blanca (sólo en memoria).
    pub fn remove_whitelist(&mut self, number: &str) -> bool {
        let key = national_key(number);
        let before = self.whitelist.len();
        self.whitelist.retain(|e| national_key(&e.number) != key);
        self.whitelist.len() != before
    }

    /// ¿Está este número en la lista negra?
    pub fn is_blacklisted(&self, number: &str) -> bool {
        let key = national_key(number);
        self.blacklist
            .iter()
            .any(|e| national_key(&e.number) == key)
    }

    /// ¿Está este número en la lista blanca?
    pub fn is_whitelisted(&self, number: &str) -> bool {
        let key = national_key(number);
        self.whitelist
            .iter()
            .any(|e| national_key(&e.number) == key)
    }

    /// Motivo de la entrada (si está en alguna lista).
    pub fn reason_for(&self, number: &str) -> Option<String> {
        let key = national_key(number);
        self.blacklist
            .iter()
            .chain(self.whitelist.iter())
            .find(|e| national_key(&e.number) == key)
            .map(|e| e.reason.clone())
    }

    /// Añade a la lista negra indicando el `state_dir` (persiste).
    pub fn add_blacklist_in(
        &mut self,
        state_dir: &str,
        number: &str,
        reason: &str,
    ) -> Result<(), String> {
        self.add_blacklist(number, reason);
        self.save(state_dir)
    }

    /// Añade a la lista blanca indicando el `state_dir` (persiste).
    pub fn add_whitelist_in(
        &mut self,
        state_dir: &str,
        number: &str,
        reason: &str,
    ) -> Result<(), String> {
        self.add_whitelist(number, reason);
        self.save(state_dir)
    }

    /// Quita de la lista negra y guarda.
    pub fn remove_blacklist_in(&mut self, state_dir: &str, number: &str) -> bool {
        let removed = self.remove_blacklist(number);
        if removed {
            let _ = self.save(state_dir);
        }
        removed
    }

    /// Quita de la lista blanca y guarda.
    pub fn remove_whitelist_in(&mut self, state_dir: &str, number: &str) -> bool {
        let removed = self.remove_whitelist(number);
        if removed {
            let _ = self.save(state_dir);
        }
        removed
    }
}

/// Nota: el guardado usa `util_fs::write_atomic` (tmp + 0600 + rename), así
/// que ya no hace falta un `set_mode_600` aparte: el fichero nace con el
/// modo correcto y no hay ventana en la que el contenido esté legible.

/// Ahora mismo, en ms epoch.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Contexto de análisis
// ---------------------------------------------------------------------------

/// Todo lo que el escáner necesita saber aparte del mensaje.
#[derive(Debug, Clone, Default)]
pub struct ScanContext {
    /// Listas negra/blanca.
    pub lists: SpamLists,
    /// Contactos del usuario (para detectar números confundibles).
    pub contacts: Vec<String>,
    /// Mensajes recientes (para detectar frecuencia anómala).
    pub history: Vec<InboundSms>,
    /// Umbrales y pesos.
    pub config: SpamConfig,
}

impl ScanContext {
    /// Contexto vacío con la configuración por defecto.
    pub fn new() -> Self {
        ScanContext::default()
    }

    /// Contexto con unas listas dadas.
    pub fn with_lists(lists: SpamLists) -> Self {
        ScanContext {
            lists,
            ..ScanContext::default()
        }
    }

    /// Añade un contacto del usuario.
    pub fn add_contact(&mut self, number: &str) {
        self.contacts.push(number.to_string());
    }

    /// Añade un mensaje al historial de frecuencia.
    pub fn add_history(&mut self, sms: InboundSms) {
        self.history.push(sms);
    }
}

// ---------------------------------------------------------------------------
// Reglas: palabras clave, URLs, números, coherencia
// ---------------------------------------------------------------------------

/// Plantillas de phishing (texto normalizado, sin acentos).
pub const PHISHING_KEYWORDS: &[&str] = &[
    "premio",
    "ganaste",
    "ganado",
    "ganador",
    "haz clic",
    "haz click",
    "clic aqui",
    "click here",
    "pulsa aqui",
    "pulse aqui",
    "verifica tu cuenta",
    "verifique su cuenta",
    "verifica tu numero",
    "whatsapp",
    "telegram",
    "regalo",
    "you have won",
    "claim your",
    "your account has been",
    "limited time",
    "datos bancarios",
    "numero de tarjeta",
    "cvv",
    "contrasena",
    "transferencia inmediata",
];

/// Palabras de urgencia con amenaza.
pub const URGENCY_KEYWORDS: &[&str] = &[
    "urgente",
    "actua ahora",
    "actuar ahora",
    "ultima hora",
    "suspender",
    "suspendida",
    "suspendido",
    "24 horas",
    "se cancelara",
    "se cancelara",
    "bloqueara",
    "inmediatamente",
];

/// Dominios de acortadores de enlaces ( phishing: el destino no se ve ).
pub const SHORTENED_HOSTS: &[&str] = &[
    "bit.ly",
    "tinyurl.com",
    "t.co",
    "goo.gl",
    "ow.ly",
    "is.gd",
    "v.gd",
    "cutt.ly",
    "rb.gy",
    "rebrand.ly",
    "buff.ly",
    "tiny.cc",
    "t.ly",
    "shorturl.at",
    "linktr.ee",
    "qrco.de",
    "shorte.st",
];

/// Prefijos de tarificación especial en España.
pub const PREMIUM_PREFIXES: &[&str] = &[
    "803", "805", "806", "807", "809", "900", "902", "903", "904", "905", "906",
];

/// Normaliza texto a ASCII "plano" (minúsculas, sin acentos, sin espacios
/// raros, dígitos de ancho completo a ASCII).
///
/// Es una copia **intencionada** de la normalización de `otp.rs`: este módulo
/// se mantiene autocontenido para poder cablearlo y probarlo solo.
pub fn normalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{ff10}'..='\u{ff19}' => out.push((b'0' + (c as u32 - 0xff10) as u8) as char),
            // Letras de ancho completo → ASCII.
            '\u{ff21}'..='\u{ff3a}' => out.push((b'A' + (c as u32 - 0xff21) as u8) as char),
            '\u{ff41}'..='\u{ff5a}' => out.push((b'a' + (c as u32 - 0xff41) as u8) as char),
            '\u{0660}'..='\u{0669}' => out.push((b'0' + (c as u32 - 0x0660) as u8) as char),
            '\u{06f0}'..='\u{06f9}' => out.push((b'0' + (c as u32 - 0x06f0) as u8) as char),
            '\u{00a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => {
                out.push(' ')
            }
            '\u{200b}'..='\u{200d}' | '\u{feff}' => {}
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' | 'Å' => {
                out.push('a')
            }
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => out.push('e'),
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => out.push('i'),
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => out.push('o'),
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => out.push('u'),
            'ñ' | 'Ñ' => out.push('n'),
            'ç' | 'Ç' => out.push('c'),
            'A'..='Z' => out.push(c.to_ascii_lowercase()),
            _ => out.push(c),
        }
    }
    out
}

/// Busca una lista de palabras en un texto normalizado.
///
/// Devuelve las palabras encontradas (en orden de aparición). Las palabras
/// cortas no se enganchan dentro de otras (`code` no casa con `barcode`).
fn find_words(text: &str, words: &[&str]) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut found: Vec<String> = Vec::new();
    for w in words {
        let wc: Vec<char> = w.chars().collect();
        if wc.is_empty() {
            continue;
        }
        let mut from = 0usize;
        while from + wc.len() <= chars.len() {
            if chars[from..from + wc.len()] == wc[..] {
                let before_ok = from == 0 || !is_word_char(chars[from - 1]);
                let after = chars.get(from + wc.len()).copied();
                let after_ok = match after {
                    None => true,
                    Some(c) => {
                        if c.is_ascii_digit() {
                            false
                        } else if wc.len() <= 4 && c.is_alphabetic() {
                            false
                        } else {
                            true
                        }
                    }
                };
                if before_ok && after_ok {
                    let mut s = String::new();
                    for c in &wc {
                        s.push(*c);
                    }
                    if !found.contains(&s) {
                        found.push(s);
                    }
                }
            }
            from += 1;
        }
    }
    found
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// Extrae los **hosts** de las URLs de un texto (con o sin `http://`).
pub fn find_url_hosts(text: &str) -> Vec<String> {
    let mut hosts = Vec::new();
    for raw in text.split_whitespace() {
        let token: String = raw
            .trim_matches(|c: char| {
                matches!(
                    c,
                    '(' | ')' | '"' | '\'' | '<' | '>' | ',' | ';' | '!' | '?' | '*'
                )
            })
            .trim_end_matches(['.', ':'])
            .to_string();
        let mut t = token.as_str();
        for pref in ["https://", "http://", "HTTPS://", "HTTP://"] {
            if let Some(rest) = t.strip_prefix(pref) {
                t = rest;
                break;
            }
        }
        let t = t.strip_prefix("www.").unwrap_or(t);
        if !t.contains('.') || !t.contains('/') && t.matches('.').count() == 0 {
            continue;
        }
        let host = t.split('/').next().unwrap_or("").to_lowercase();
        if host.is_empty() || host.starts_with('.') || host.ends_with('.') {
            continue;
        }
        if !host.contains('.') {
            continue;
        }
        if host.chars().any(|c| c.is_whitespace()) {
            continue;
        }
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    hosts
}

/// ¿Es un acortador de enlaces?
pub fn is_shortened_host(host: &str) -> bool {
    let h = host.trim_start_matches("www.").to_lowercase();
    SHORTENED_HOSTS
        .iter()
        .any(|s| h == *s || h.ends_with(&format!(".{s}")))
}

/// Clave nacional de un número (para comparar listas y contactos).
///
/// `+34600112233` → `600112233`; los números cortos se quedan como están.
pub fn national_key(number: &str) -> String {
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() > 9 {
        digits[digits.len() - 9..].to_string()
    } else {
        digits
    }
}

/// Longitud de la parte nacional de un número (para detectar números cortos).
pub fn national_length(number: &str) -> usize {
    national_key(number).len()
}

/// Distancia de Levenshtein entre dos cadenas.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    if ac.is_empty() {
        return bc.len();
    }
    if bc.is_empty() {
        return ac.len();
    }
    let mut prev: Vec<usize> = (0..=bc.len()).collect();
    let mut cur = vec![0usize; bc.len() + 1];
    for i in 1..=ac.len() {
        cur[0] = i;
        for j in 1..=bc.len() {
            let cost = if ac[i - 1] == bc[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[bc.len()]
}

/// Canoniza los caracteres confundibles a una clase común.
///
/// `0/O`→`0`, `1/l/I`→`1`, `5/S`→`5`, `2/Z`→`2`, `8/B`→`8`, `6/G`→`6` y
/// `rn/m`→`m` (el "rn" tecleado en lugar de "m").
pub fn canonicalize_confusables(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if (c == 'r' || c == 'R')
            && chars
                .get(i + 1)
                .map(|n| *n == 'n' || *n == 'N')
                .unwrap_or(false)
        {
            out.push('m');
            i += 2;
            continue;
        }
        let canon = match c {
            '0' | 'o' | 'O' => '0',
            '1' | 'l' | 'L' | 'i' | 'I' | '|' => '1',
            '5' | 's' | 'S' => '5',
            '2' | 'z' | 'Z' => '2',
            '8' | 'b' | 'B' => '8',
            '6' | 'g' | 'G' => '6',
            '9' | 'q' | 'Q' => '9',
            'm' | 'M' => 'm',
            other => other,
        };
        out.push(canon);
        i += 1;
    }
    out
}

/// Resultado de comparar un número con los contactos del usuario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfusableMatch {
    /// Contacto con el que se parece.
    pub contact: String,
    /// Distancia mínima encontrada.
    pub distance: usize,
    /// ¿La coincidencia usó la canonización de confundibles?
    pub canonical: bool,
}

/// Dígitos de un número (descarta todo lo que no sea dígito).
fn digits_only(text: &str) -> String {
    text.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Parte nacional de una cadena **sólo de dígitos** (los 9 últimos).
fn national_from_digits(digits: &str) -> String {
    if digits.len() > 9 {
        digits[digits.len() - 9..].to_string()
    } else {
        digits.to_string()
    }
}

/// Busca el contacto más parecido a un número.
///
/// Se comparan los números **crudos** (distancia normal) y los **canónicos**
/// (confundibles), y se queda la mejor.
///
/// Importante: la canonización se hace **antes** de recortar la parte
/// nacional, porque un número con una letra confundible ("+3460011122O") tiene
/// menos dígitos y se alinearía mal con el contacto real.
pub fn confusable_match(
    number: &str,
    contacts: &[String],
    max_distance: usize,
) -> Option<ConfusableMatch> {
    let raw_key = national_from_digits(&digits_only(number));
    let canon_key = national_from_digits(&digits_only(&canonicalize_confusables(number)));
    let mut best: Option<ConfusableMatch> = None;
    for c in contacts {
        let c_raw = national_from_digits(&digits_only(c));
        if c_raw.is_empty() {
            continue;
        }
        let c_canon = national_from_digits(&digits_only(&canonicalize_confusables(c)));
        let d_raw = levenshtein(&raw_key, &c_raw);
        let d_canon = levenshtein(&canon_key, &c_canon);
        let (distance, canonical) = if d_canon < d_raw {
            (d_canon, true)
        } else if d_canon == d_raw {
            (d_raw, canon_key != raw_key || c_canon != c_raw)
        } else {
            (d_raw, false)
        };
        if distance > max_distance {
            continue;
        }
        if best.as_ref().map(|b| distance < b.distance).unwrap_or(true) {
            best = Some(ConfusableMatch {
                contact: c.clone(),
                distance,
                canonical,
            });
        }
    }
    best
}

/// Coherencia de un texto:CCN palabras distintas / palabras totales.
///
/// 1.0 = todas distintas; 0.0 = se repite la misma palabra.
pub fn coherence(text: &str) -> f32 {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return 1.0;
    }
    let mut unicas: Vec<&String> = Vec::new();
    for w in &words {
        if !unicas.contains(&w) {
            unicas.push(w);
        }
    }
    unicas.len() as f32 / words.len() as f32
}

/// Cuenta cuántas palabras distintas tiene un texto.
pub fn word_count(text: &str) -> usize {
    text.split_whitespace()
        .filter(|w| w.chars().any(|c| c.is_alphanumeric()))
        .count()
}

// ---------------------------------------------------------------------------
// Escaneo
// ---------------------------------------------------------------------------

/// Analiza un SMS entrante y devuelve la puntuación con su desglose.
pub fn scan_sms(sms: &InboundSms, ctx: &ScanContext) -> SpamScore {
    let mut score = SpamScore::empty();
    scan_into(&mut score, &sms.sender, &sms.body, sms.timestamp, ctx);
    score
}

/// Analiza una llamada entrante y devuelve la puntuación con su desglose.
pub fn scan_call(call: &InboundCall, ctx: &ScanContext) -> SpamScore {
    let mut score = SpamScore::empty();
    let body = call.transcript.clone().unwrap_or_default();
    scan_into(&mut score, &call.number, &body, call.timestamp, ctx);
    score
}

/// Decide qué hacer con un SMS ya puntuado.
pub fn decide_sms(sms: &InboundSms, score: &SpamScore, ctx: &ScanContext) -> Decision {
    decide(&sms.sender, score, ctx)
}

/// Decide qué hacer con una llamada ya puntuada.
pub fn decide_call(call: &InboundCall, score: &SpamScore, ctx: &ScanContext) -> Decision {
    decide(&call.number, score, ctx)
}

/// Puntuación + decisión para un SMS.
pub fn classify_sms(sms: &InboundSms, ctx: &ScanContext) -> SpamVerdict {
    let score = scan_sms(sms, ctx);
    let decision = decide_sms(sms, &score, ctx);
    SpamVerdict { score, decision }
}

/// Puntuación + decisión para una llamada.
pub fn classify_call(call: &InboundCall, ctx: &ScanContext) -> SpamVerdict {
    let score = scan_call(call, ctx);
    let decision = decide_call(call, &score, ctx);
    SpamVerdict { score, decision }
}

/// Aplica todas las reglas a un remitente y un texto.
fn scan_into(score: &mut SpamScore, sender: &str, body: &str, timestamp: i64, ctx: &ScanContext) {
    let w = &ctx.config.weights;

    // 1. Listas (la blanca tiene prioridad: se decide al final, pero se
    //    puntúa aquí para que el desglose lo muestre).
    if ctx.lists.is_whitelisted(sender) {
        let reason = ctx
            .lists
            .reason_for(sender)
            .unwrap_or_else(|| "en la lista blanca".to_string());
        score.push(Signal::new(
            "whitelist",
            w.whitelist,
            &format!("remitente en la lista blanca ({reason}): la lista blanca tiene prioridad"),
        ));
    }
    if ctx.lists.is_blacklisted(sender) {
        let reason = ctx
            .lists
            .reason_for(sender)
            .unwrap_or_else(|| "en la lista negra".to_string());
        score.push(Signal::new(
            "blacklist",
            w.blacklist,
            &format!("remitente en la lista negra ({reason})"),
        ));
    }

    // 2. Forma del número: corto / premium.
    let digits: String = sender.chars().filter(|c| c.is_ascii_digit()).collect();
    let alphanumeric = sender.chars().any(|c| c.is_alphabetic());
    if !alphanumeric && !digits.is_empty() {
        let national = national_key(sender);
        if national.len() <= 5 {
            score.push(Signal::new(
                "short_code",
                w.short_code,
                &format!("remitente corto ({national}): centro de mensajes o número corto"),
            ));
        }
        for p in PREMIUM_PREFIXES {
            if national.starts_with(p) && national.len() > 5 {
                score.push(Signal::new(
                    "premium_prefix",
                    w.premium_prefix,
                    &format!("prefijo de tarificación especial {p}"),
                ));
                break;
            }
        }
    }

    let norm = normalize_text(body);

    // 3. Plantillas de phishing (con tope).
    let phishing = find_words(&norm, PHISHING_KEYWORDS);
    let mut phishing_weight = 0.0f32;
    for (i, p) in phishing.iter().enumerate() {
        if phishing_weight + w.phishing_keyword > w.phishing_cap {
            break;
        }
        let detalle = if i + 1 < phishing.len() {
            format!(
                "plantilla de phishing: '{p}' (+{} más omitidas por tope)",
                phishing.len() - i - 1
            )
        } else {
            format!("plantilla de phishing: '{p}'")
        };
        score.push(Signal::new(
            "phishing_template",
            w.phishing_keyword,
            &detalle,
        ));
        phishing_weight += w.phishing_keyword;
    }

    // 4. Urgencia con amenazas (con tope).
    let urgency = find_words(&norm, URGENCY_KEYWORDS);
    let mut urgency_weight = 0.0f32;
    for (i, u) in urgency.iter().enumerate() {
        if urgency_weight + w.urgency_threat > w.urgency_cap {
            break;
        }
        let detalle = if i + 1 < urgency.len() {
            format!(
                "urgencia/amenaza: '{u}' (+{} más omitidas por tope)",
                urgency.len() - i - 1
            )
        } else {
            format!("urgencia/amenaza: '{u}'")
        };
        score.push(Signal::new("urgency_threat", w.urgency_threat, &detalle));
        urgency_weight += w.urgency_threat;
    }

    // 5. URLs (acortados y genéricas).
    let hosts = find_url_hosts(&norm);
    let mut url_weight = 0.0f32;
    for h in &hosts {
        if is_shortened_host(h) && url_weight + w.shortened_url <= w.url_cap + w.shortened_url {
            score.push(Signal::new(
                "shortened_url",
                w.shortened_url,
                &format!("enlace acortado ({h}): el destino real no se ve"),
            ));
            url_weight += w.shortened_url;
            continue;
        }
        if url_weight + w.generic_url <= w.url_cap {
            score.push(Signal::new(
                "generic_url",
                w.generic_url,
                &format!("el mensaje lleva una URL ({h})"),
            ));
            url_weight += w.generic_url;
        }
    }

    // 6. Frecuencia anómala del mismo remitente.
    let key = national_key(sender);
    let window = ctx.config.frequency_window_ms;
    let count = ctx
        .history
        .iter()
        .filter(|h| {
            national_key(&h.sender) == key
                && (h.timestamp - timestamp).abs() <= window
                && h.timestamp != timestamp
        })
        .count();
    if count + 1 >= ctx.config.frequency_threshold {
        score.push(Signal::new(
            "frequency",
            w.frequency,
            &format!(
                "{count} mensajes del mismo remitente en {} min: frecuencia anómala",
                window / 60_000
            ),
        ));
    }

    // 7. Número confundible con un contacto del usuario.
    //    (distancia 0 = es un contacto conocido: no es una señal de spam).
    if !ctx.contacts.is_empty() {
        if let Some(m) = confusable_match(sender, &ctx.contacts, ctx.config.confusable_max_distance)
        {
            if m.distance >= 1 {
                let detalle = if m.canonical {
                    format!(
                        "el número se parece a '{}' (distancia {}, con confundibles 0/O 1/l 5/S rn/m)",
                        m.contact, m.distance
                    )
                } else {
                    format!(
                        "el número se parece a '{}' (distancia {})",
                        m.contact, m.distance
                    )
                };
                score.push(Signal::new("confusable_number", w.confusable, &detalle));
            }
        }
    }

    // 8. Coherencia baja (mensaje o transcripción con muy poca variedad).
    let wc = word_count(&norm);
    if wc >= 4 {
        let coh = coherence(&norm);
        if coh < 0.35 {
            score.push(Signal::new(
                "low_coherence",
                w.low_coherence,
                &format!("coherencia baja ({coh:.2}): {wc} palabras con muy poca variedad"),
            ));
        }
    }
}

/// Decide según la puntuación y las listas.
///
/// * Lista blanca → `Accept` **siempre** (tiene prioridad).
/// * Lista negra → `Block` **siempre**.
/// * Si no, por umbrales: `>= block_threshold` bloquea, `>= quarantine_threshold`
///   aparta, y por debajo acepta.
pub fn decide(sender: &str, score: &SpamScore, ctx: &ScanContext) -> Decision {
    if ctx.lists.is_whitelisted(sender) {
        return Decision::Accept;
    }
    if ctx.lists.is_blacklisted(sender) {
        return Decision::Block;
    }
    if score.total >= ctx.config.block_threshold {
        Decision::Block
    } else if score.total >= ctx.config.quarantine_threshold {
        Decision::Quarantine
    } else {
        Decision::Accept
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_712_345_678_000;

    fn ctx() -> ScanContext {
        ScanContext::new()
    }

    #[test]
    fn un_sms_de_phishing_con_enlace_acortado_bloquea() {
        let c = ctx();
        let sms = InboundSms::new(
            "PROMO1",
            "¡Felicidades! Has ganado un premio de 500 €. Haz clic aquí http://bit.ly/3xY para reclamarlo. WhatsApp 600112233",
            T0,
        );
        let v = classify_sms(&sms, &c);
        assert_eq!(
            v.decision,
            Decision::Block,
            "desglose:\n{}",
            v.score.explain()
        );
        assert!(v.score.has_rule("shortened_url"));
        assert!(v.score.has_rule("phishing_template"));
        assert!(v.score.total >= c.config.block_threshold);
    }

    #[test]
    fn un_sms_de_mi_banco_sin_url_se_acepta() {
        let c = ctx();
        let sms = InboundSms::new(
            "BANCO",
            "Banco: su código es 123456. No comparta este código con nadie.",
            T0,
        );
        let v = classify_sms(&sms, &c);
        assert_eq!(
            v.decision,
            Decision::Accept,
            "desglose:\n{}",
            v.score.explain()
        );
        assert!(v.score.total < c.config.quarantine_threshold);
    }

    #[test]
    fn un_numero_cercano_a_un_contacto_va_a_cuarentena() {
        let mut c = ctx();
        c.add_contact("+34600111222");
        let sms = InboundSms::new("+3460011122O", "Hola, ¿me confirmas la cita?", T0);
        let v = classify_sms(&sms, &c);
        assert_eq!(
            v.decision,
            Decision::Quarantine,
            "desglose:\n{}",
            v.score.explain()
        );
        assert!(v.score.has_rule("confusable_number"));
        let m = confusable_match("+3460011122O", &["+34600111222".to_string()], 2).unwrap();
        assert_eq!(m.distance, 1);
        assert!(m.canonical, "el último carácter es una O en vez de un 0");
    }

    #[test]
    fn la_lista_blanca_tiene_prioridad() {
        let mut c = ctx();
        c.lists.whitelist.push(ListEntry {
            number: "+34600111222".to_string(),
            reason: "es mi banco".to_string(),
            added_at: T0,
        });
        let sms = InboundSms::new(
            "+34600111222",
            "Has ganado un premio, haz clic en http://bit.ly/3xY",
            T0,
        );
        let v = classify_sms(&sms, &c);
        assert_eq!(
            v.decision,
            Decision::Accept,
            "desglose:\n{}",
            v.score.explain()
        );
        assert!(v.score.has_rule("whitelist"));
    }

    #[test]
    fn la_lista_negra_bloquea() {
        let mut c = ctx();
        c.lists.blacklist.push(ListEntry {
            number: "+34600111222".to_string(),
            reason: "spam de premios".to_string(),
            added_at: T0,
        });
        let sms = InboundSms::new("+34600111222", "Hola", T0);
        let v = classify_sms(&sms, &c);
        assert_eq!(v.decision, Decision::Block);
        assert!(v.score.has_rule("blacklist"));
        assert_eq!(
            c.lists.reason_for("+34600111222").as_deref(),
            Some("spam de premios")
        );
    }

    #[test]
    fn un_numero_corto_por_si_solo_no_es_spam() {
        let c = ctx();
        let sms = InboundSms::new("40312", "Consulta disponibilidad en el panel", T0);
        let v = classify_sms(&sms, &c);
        assert!(v.score.has_rule("short_code"));
        assert_eq!(
            v.decision,
            Decision::Accept,
            "desglose:\n{}",
            v.score.explain()
        );
    }

    #[test]
    fn la_frecuencia_anomala_sube_la_puntuacion() {
        let mut c = ctx();
        for i in 0..3 {
            c.add_history(InboundSms::new("+34600111222", "mensaje", T0 + i * 60_000));
        }
        let sms = InboundSms::new("+34600111222", "mensaje", T0 + 4 * 60_000);
        let v = classify_sms(&sms, &c);
        assert!(
            v.score.has_rule("frequency"),
            "desglose:\n{}",
            v.score.explain()
        );
        assert_eq!(v.decision, Decision::Quarantine);
    }

    #[test]
    fn la_urgencia_con_amenazas_sube_la_puntuacion() {
        let c = ctx();
        let sms = InboundSms::new(
            "+34600111222",
            "URGENTE: su cuenta será suspendida en 24 horas. Llame ahora mismo.",
            T0,
        );
        let v = classify_sms(&sms, &c);
        assert!(
            v.score.has_rule("urgency_threat"),
            "desglose:\n{}",
            v.score.explain()
        );
        assert_eq!(v.decision, Decision::Quarantine);
    }

    #[test]
    fn un_mensaje_normal_de_un_contacto_se_acepta() {
        let mut c = ctx();
        c.add_contact("+34600111222");
        let sms = InboundSms::new(
            "+34600111222",
            "Hola, ¿nos vemos mañana a las 18:00 en la plaza?",
            T0,
        );
        let v = classify_sms(&sms, &c);
        assert_eq!(
            v.decision,
            Decision::Accept,
            "desglose:\n{}",
            v.score.explain()
        );
        assert_eq!(v.score.signals.len(), 0, "no debería saltar ninguna señal");
    }

    #[test]
    fn un_prefijo_premium_con_palabras_clave_sube_a_cuarentena() {
        let c = ctx();
        let sms = InboundSms::new(
            "806123456",
            "Ha ganado un premio, llame ahora para reclamarlo",
            T0,
        );
        let v = classify_sms(&sms, &c);
        assert!(
            v.score.has_rule("premium_prefix"),
            "desglose:\n{}",
            v.score.explain()
        );
        assert_eq!(v.decision, Decision::Quarantine);
    }

    #[test]
    fn una_voz_con_coherencia_baja_sube_la_puntuacion() {
        let c = ctx();
        let call = InboundCall::new(
            "+34600111222",
            Some("urgente urgente urgente premio premio premio premio"),
            T0,
        );
        let v = classify_call(&call, &c);
        assert!(
            v.score.has_rule("low_coherence"),
            "desglose:\n{}",
            v.score.explain()
        );
        assert!(v.score.has_rule("phishing_template"));
        assert_ne!(v.decision, Decision::Accept);
    }

    #[test]
    fn una_voz_normal_no_salta_la_regla_de_coherencia() {
        let c = ctx();
        let call = InboundCall::new(
            "+34600111222",
            Some("Hola, te llamo para confirmar la cita de mañana a las diez"),
            T0,
        );
        let v = classify_call(&call, &c);
        assert!(!v.score.has_rule("low_coherence"));
        assert_eq!(v.decision, Decision::Accept);
    }

    #[test]
    fn el_desglose_muestra_las_señales() {
        let c = ctx();
        let sms = InboundSms::new(
            "PROMO1",
            "Has ganado un premio, haz clic aquí http://bit.ly/3xY",
            T0,
        );
        let v = classify_sms(&sms, &c);
        let texto = v.score.explain();
        assert!(texto.contains("phishing_template"), "{texto}");
        assert!(texto.contains("shortened_url"), "{texto}");
        assert!(texto.contains("total"), "{texto}");
        for s in &v.score.signals {
            assert!(!s.detail.is_empty(), "ninguna señal puede ir sin detalle");
        }
    }

    #[test]
    fn las_listas_se_guardan_y_se_cargan_con_permiso_600() {
        let dir = std::env::temp_dir().join(format!("md-antispam-test-{}", std::process::id()));
        let dir_s = dir.to_string_lossy().to_string();
        let _ = fs::remove_dir_all(&dir);

        let mut lists = SpamLists::default();
        lists
            .add_blacklist_in(&dir_s, "+34600111222", "spam de premios")
            .unwrap();
        lists
            .add_whitelist_in(&dir_s, "+34600555666", "mi banco")
            .unwrap();
        assert!(lists.is_blacklisted("34600111222"));
        assert!(lists.is_whitelisted("+34600555666"));

        let ruta = SpamLists::path(&dir_s);
        assert!(
            std::path::Path::new(&ruta).is_file(),
            "debe existir el fichero de listas"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = fs::metadata(&ruta).unwrap().permissions().mode() & 0o777;
            assert_eq!(modo, 0o600, "las listas deben ser 0600");
        }

        let cargadas = SpamLists::load(&dir_s);
        assert_eq!(cargadas.blacklist.len(), 1);
        assert_eq!(cargadas.whitelist.len(), 1);
        assert_eq!(cargadas.blacklist[0].reason, "spam de premios");

        // Y se pueden quitar.
        let mut cargadas = cargadas;
        assert!(cargadas.remove_blacklist("+34600111222"));
        assert!(!cargadas.is_blacklisted("34600111222"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_distancia_de_levenshtein_es_la_esperada() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
    }

    #[test]
    fn los_confundibles_se_canonizan() {
        assert_eq!(canonicalize_confusables("0O"), "00");
        assert_eq!(canonicalize_confusables("1lI"), "111");
        assert_eq!(canonicalize_confusables("5S"), "55");
        assert_eq!(canonicalize_confusables("rn"), "m");
        assert_eq!(canonicalize_confusables("m"), "m");
    }

    #[test]
    fn la_clave_nacional_compara_numeros_con_y_sin_prefijo() {
        assert_eq!(national_key("+34600112233"), "600112233");
        assert_eq!(national_key("600112233"), "600112233");
        assert_eq!(national_key("+34 600 11 22 33"), "600112233");
        assert_eq!(national_key("40312"), "40312");
    }

    #[test]
    fn se_detectan_los_hosts_de_las_urls() {
        let hosts = find_url_hosts("Mira http://bit.ly/3xY y también https://www.mi-bank.es/login");
        assert!(hosts.contains(&"bit.ly".to_string()), "{hosts:?}");
        assert!(hosts.contains(&"mi-bank.es".to_string()), "{hosts:?}");
        assert!(is_shortened_host("bit.ly"));
        assert!(!is_shortened_host("mi-bank.es"));
    }
}

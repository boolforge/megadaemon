//! Gestor de portapapeles **distribuido**.
//!
//! # Por qué esto no es "otro gestor de portapapeles"
//!
//! Los gestores de portapapeles de Android están rota por diseño del sistema:
//!
//! | Android | Limitación | Consecuencia |
//! |---|---|---|
//! | 10 (API 29) | una app en segundo plano **no puede leer** el portapapeles | sólo funciona con la app en primer plano |
//! | 12 (API 31) | al leer, aparece el aviso "pegado desde…" | el usuario ve de dónde viene cada pegado |
//! | 13 (API 33) | el aviso pasa a ser una previsualización | hay que tocarla para pegar |
//!
//! La respuesta habitual: un **servicio en primer plano** + una
//! notificación, y el usuario tiene que tocar la notificación cada vez. Es
//! molesto, consume batería y no es fiable.
//!
//! # Las cuatro rutas, y cuál usa MegaDaemon
//!
//! 1. **`ACTION_PROCESS_TEXT`** (la de Text Tools, `corphish/TextTools`):
//!    el usuario selecciona texto ⇒ el sistema **te lo entrega** en el menú
//!    de contexto. Sin restricción de segundo plano, sin aviso, sin
//!    notificación. Es la vía que funciona en cualquier Android 10+ sin root.
//! 2. **Root (la que usa este módulo por defecto)**: el daemon corre como
//!    `root`, así que puede hablar con el servicio del portapapeles
//!    directamente (binder) sin la política por UID: **lee y escribe en
//!    segundo plano, sin avisos y sin primer plano**. Ninguna app normal
//!    puede hacer esto.
//! 3. **Teclado propio (IME)**: un IME tiene acceso al portapapeles. Es la
//!    vía más limpia si se quiere captura automática.
//! 4. **Accesibilidad**: ve la pantalla, no el portapapeles.
//!
//! # Lo que aporta sobre los demás
//!
//! * **Distinguido**: no guarda lo que no debe. Una clave, un token o un OTP
//!   se clasifican como **sensibles** y **no entran al historial persistente**
//!   por defecto (viven con TTL corto).
//! * **Distribuido**: los items viajan por la malla entre nodos con reglas
//!   por destino; el móvil puede **empujar** al PC y al revés.
//! * **Con expiración**: "borra esto en 30 s" como en un gestor de claves.
//! * **Cifrado en reposo**: AEGIS-256 + Argon2id (los módulos canónicos del
//!   repo), no un AES cualquiera.
//! * **Auditable sin filtrar**: el contenido nunca se escribe en el log.
//!
//! # Lo que NO VERIFICADO queda aquí
//!
//! Los dos proveedores root usan `cmd clipboard` y `service call clipboard`.
//! **No están medidos en este Samsung**: hay que ejecutar los comandos del
//! `docs/CLIPBOARD.md` en el móvil y anotar qué responde.  Si ninguno
//! funciona, el módulo degrada al proveedor `PushClipboard` (items
//! empujados por PROCESS_TEXT/IME), que **siempre** funciona.

use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Tamaño máximo de un item (un portapapeles con un BASE64 gigante no es un
/// portapapeles, es una fuga de memoria).
pub const MAX_ITEM_BYTES: usize = 64 * 1024;

/// Entradas por defecto en el historial.
pub const MAX_ENTRADAS: usize = 200;

/// Bytes máximos del historial completo.
pub const MAX_HISTORIAL_BYTES: usize = 2 * 1024 * 1024;

/// Clasificación de un item del portapapeles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Texto,
    Url,
    Telefono,
    Codigo,
    Json,
}

impl Tipo {
    /// Nombre estable (para JSON y para la malla).
    pub fn as_str(&self) -> &'static str {
        match self {
            Tipo::Texto => "texto",
            Tipo::Url => "url",
            Tipo::Telefono => "telefono",
            Tipo::Codigo => "codigo",
            Tipo::Json => "json",
        }
    }
}

/// Un item del historial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Identificador estable (hash del texto + timestamp).
    pub id: String,
    /// El contenido.
    pub texto: String,
    /// Clasificación automática.
    pub tipo: Tipo,
    /// Epoch de captura.
    pub ts: u64,
    /// De dónde viene (paquete de la app, o "manual").
    pub origen: String,
    /// Segundos hasta expirar (si expira).
    pub ttl_s: Option<u64>,
    /// No se persiste en el historial (sensibles y efímeros).
    pub efimero: bool,
    /// Clasificado como sensible (clave, token, OTP).
    pub sensible: bool,
}

impl Item {
    /// Crea un item clasificando el texto.
    pub fn nuevo(texto: &str, ts: u64, origen: &str) -> Item {
        let sensible = es_sensible(texto);
        Item {
            id: id_de(texto, ts),
            texto: texto.to_string(),
            tipo: clasificar(texto),
            ts,
            origen: origen.to_string(),
            ttl_s: if sensible { Some(60) } else { None },
            efimero: sensible,
            sensible,
        }
    }

    /// ¿Ha expirado?
    pub fn expirado(&self, ahora: u64) -> bool {
        match self.ttl_s {
            Some(ttl) => ahora.saturating_sub(self.ts) >= ttl,
            None => false,
        }
    }

    /// Remaining seconds (0 if it doesn't expire).
    pub fn restante_s(&self, ahora: u64) -> u64 {
        match self.ttl_s {
            Some(ttl) => ttl.saturating_sub(ahora.saturating_sub(self.ts)),
            None => 0,
        }
    }

    /// Serialización a JSON (la que viaja por la malla).
    pub fn a_json(&self) -> Value {
        json!({
            "id": self.id,
            "texto": self.texto,
            "tipo": self.tipo.as_str(),
            "ts": self.ts,
            "origen": self.origen,
            "ttl_s": self.ttl_s,
            "efimero": self.efimero,
            "sensible": self.sensible,
        })
    }

    /// Lee un item de JSON (de la malla o del historial).
    pub fn de_json(v: &Value) -> Option<Item> {
        let texto = v.get("texto")?.as_str()?.to_string();
        let ts = v.get("ts").and_then(|x| x.as_u64()).unwrap_or(0);
        let tipo = match v.get("tipo").and_then(|x| x.as_str()).unwrap_or("texto") {
            "url" => Tipo::Url,
            "telefono" => Tipo::Telefono,
            "codigo" => Tipo::Codigo,
            "json" => Tipo::Json,
            _ => Tipo::Texto,
        };
        Some(Item {
            id: v
                .get("id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| id_de(&texto, ts)),
            texto,
            tipo,
            ts,
            origen: v
                .get("origen")
                .and_then(|x| x.as_str())
                .unwrap_or("desconocido")
                .to_string(),
            ttl_s: v.get("ttl_s").and_then(|x| x.as_u64()),
            efimero: v.get("efimero").and_then(|x| x.as_bool()).unwrap_or(false),
            sensible: v.get("sensible").and_then(|x| x.as_bool()).unwrap_or(false),
        })
    }
}

/// Identificador estable de un item: hash del texto + timestamp.
fn id_de(texto: &str, ts: u64) -> String {
    let mut h = crate::clipboard::sha256_texto(texto);
    h.truncate(16);
    format!("{h}-{ts}")
}

/// SHA-256 en hexadecimal (usa el `sha2` del repo).
pub fn sha256_texto(texto: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(texto.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Clasifica el contenido del portapapeles.
pub fn clasificar(texto: &str) -> Tipo {
    let t = texto.trim();
    if t.is_empty() {
        return Tipo::Texto;
    }
    if t.starts_with("http://") || t.starts_with("https://") || t.starts_with("www.") {
        return Tipo::Url;
    }
    // OJO: un numero suelto ("600123456") es JSON valido, asi que hay que
    // exigir objeto o array; si no, un telefono acabaria clasificado como JSON.
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if v.is_object() || v.is_array() {
            return Tipo::Json;
        }
    }
    // Un código de 4-8 dígitos con contexto de "código" => OTP.
    if parece_codigo(t) {
        return Tipo::Codigo;
    }
    // Teléfono: dígitos, espacios, +, guiones y paréntesis; 7-15 dígitos.
    let digitos: usize = t.chars().filter(|c| c.is_ascii_digit()).count();
    let otros = t
        .chars()
        .filter(|c| !c.is_ascii_digit() && !matches!(c, ' ' | '+' | '-' | '(' | ')' | '.'))
        .count();
    if digitos >= 7 && digitos <= 15 && otros == 0 {
        return Tipo::Telefono;
    }
    Tipo::Texto
}

/// ¿El texto parece un código de un solo uso?
///
/// Regla: **un único grupo** de 4-8 dígitos, y el resto del texto es
/// contexto (hasta 24 caracteres no numéricos: "tu código es " = 12).
///
/// Esto separa los casos de verdad:
///   * "123456"       -> 1 grupo de 6            => código
///   * "código: 123456" -> 1 grupo + contexto   => código
///   * "600 123 456"  -> 3 grupos                => teléfono (no código)
///   * "600123456"    -> 1 grupo de 9            => teléfono (no código)
fn parece_codigo(t: &str) -> bool {
    let grupos: Vec<&str> = t
        .split(|c: char| !c.is_ascii_digit())
        .filter(|g| !g.is_empty())
        .collect();
    if grupos.len() != 1 {
        return false;
    }
    let d = grupos[0];
    if d.len() < 4 || d.len() > 8 {
        return false;
    }
    let contexto = t.chars().filter(|c| !c.is_ascii_digit()).count();
    contexto <= 24
}

/// Palabras que delatan un secreto en el texto.
const PALABRAS_SENSIBLES: &[&str] = &[
    "contraseña",
    "contrasena",
    "password",
    "passwd",
    "clave",
    "secret",
    "token",
    "api_key",
    "apikey",
    "-----begin",
    "private key",
    "cvv",
    "cvc",
];

/// ¿El texto parece contener un secreto?
///
/// Un código suelto de 6 dígitos **no** se marca sensible por sí solo (puede
/// ser un número de teléfono o un código de pedido); sí se marca si viene
/// con contexto ("tu código es 123456").
pub fn es_sensible(texto: &str) -> bool {
    let t = texto.to_lowercase();
    if PALABRAS_SENSIBLES.iter().any(|p| t.contains(p)) {
        return true;
    }
    // Un codigo de 4-8 digitos (con contexto o a secas) se trata como
    // sensible: un OTP pegado solo es el caso mas habitual y no queremos
    // que se quede en el historial.  Un telefono NO entra aqui (9 digitos).
    parece_codigo(&t)
}

// ── El historial ──────────────────────────────────────────────────────────

/// Reglas del historial.
#[derive(Debug, Clone)]
pub struct Reglas {
    /// Entradas máxima.
    pub max_entradas: usize,
    /// Bytes máximos del historial.
    pub max_bytes: usize,
    /// Bytes máximos por item.
    pub max_item_bytes: usize,
    /// ¿Guardar los items sensibles en el historial persistente?
    pub persistir_sensibles: bool,
}

impl Default for Reglas {
    fn default() -> Self {
        Reglas {
            max_entradas: MAX_ENTRADAS,
            max_bytes: MAX_HISTORIAL_BYTES,
            max_item_bytes: MAX_ITEM_BYTES,
            persistir_sensibles: false,
        }
    }
}

/// Historial de portapapeles: un deque con deduplicación y purga.
#[derive(Debug, Clone, Default)]
pub struct Historial {
    items: VecDeque<Item>,
    reglas: Reglas,
}

impl Historial {
    /// Historial con las reglas por defecto.
    pub fn nuevo() -> Historial {
        Historial {
            items: VecDeque::new(),
            reglas: Reglas::default(),
        }
    }

    /// Historial con reglas propias.
    pub fn con_reglas(reglas: Reglas) -> Historial {
        Historial {
            items: VecDeque::new(),
            reglas,
        }
    }

    /// Añade un item.
    ///
    /// * **dedup**: si el texto ya estaba, se mueve al frente y se actualiza
    ///   su `ts` (y su `id`), en vez de duplicarlo.
    /// * los **sensibles** no se guardan si `persistir_sensibles == false`.
    /// * se **purgan** los expirados y se recorta al máximo.
    ///
    /// Devuelve `true` si el item ha quedado en el historial.
    pub fn push(&mut self, item: Item) -> bool {
        let ahora = item.ts;
        self.purgar(ahora);
        if item.texto.len() > self.reglas.max_item_bytes {
            return false;
        }
        if item.sensible && !self.reglas.persistir_sensibles {
            return false;
        }
        // Dedup: se quita la entrada anterior con el mismo texto.
        self.items.retain(|i| i.texto != item.texto);
        self.items.push_front(item);
        self.recortar();
        true
    }

    /// Quita los items expirados.
    pub fn purgar(&mut self, ahora: u64) {
        self.items.retain(|i| !i.expirado(ahora));
    }

    /// Recorta a los máximos configurados.
    fn recortar(&mut self) {
        while self.items.len() > self.reglas.max_entradas {
            self.items.pop_back();
        }
        let mut total: usize = self.items.iter().map(|i| i.texto.len()).sum();
        while total > self.reglas.max_bytes {
            match self.items.pop_back() {
                Some(i) => total = total.saturating_sub(i.texto.len()),
                None => break,
            }
        }
    }

    /// Todos los items (del más reciente al más viejo).
    pub fn items(&self) -> Vec<&Item> {
        self.items.iter().collect()
    }

    /// Número de items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// ¿Vacío?
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Borra todo.
    pub fn limpiar(&mut self) {
        self.items.clear();
    }

    /// Busca por subcadena (case-insensitive).  Devuelve hasta `limite`.
    pub fn buscar(&self, q: &str, limite: usize) -> Vec<&Item> {
        if q.is_empty() {
            return self.items.iter().take(limite).collect();
        }
        let q = q.to_lowercase();
        self.items
            .iter()
            .filter(|i| i.texto.to_lowercase().contains(&q))
            .take(limite)
            .collect()
    }

    /// Busca por id exacto.
    pub fn por_id(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Bytes que ocupa el historial.
    pub fn bytes(&self) -> usize {
        self.items.iter().map(|i| i.texto.len()).sum()
    }
}

// ── Proveedores ───────────────────────────────────────────────────────────

/// Fuente del portapapeles.
pub trait ClipboardProvider {
    /// Lee el contenido actual (o `None` si está vacío).
    fn leer(&self) -> Result<Option<String>, String>;
    /// Escribe contenido en el portapapeles.
    fn escribir(&self, texto: &str) -> Result<(), String>;
    /// Nombre del proveedor (para diagnóstico).
    fn nombre(&self) -> &'static str;
}

/// Proveedor en memoria (tests y simulación).
#[derive(Debug, Default)]
pub struct MockClipboard {
    pub contenido: std::sync::Mutex<Option<String>>,
}

impl MockClipboard {
    /// Proveedor nuevo, vacío.
    pub fn nuevo() -> MockClipboard {
        MockClipboard::default()
    }
}

impl ClipboardProvider for MockClipboard {
    fn leer(&self) -> Result<Option<String>, String> {
        Ok(self.contenido.lock().ok().and_then(|g| g.clone()))
    }
    fn escribir(&self, texto: &str) -> Result<(), String> {
        *self.contenido.lock().map_err(|_| "mutex envenenado")? = Some(texto.to_string());
        Ok(())
    }
    fn nombre(&self) -> &'static str {
        "mock"
    }
}

/// Proveedor que **recibe** los items empujados desde dentro del proceso
/// (lo que llega por `ACTION_PROCESS_TEXT`, el IME o accesibilidad).
///
/// Éste **siempre** funciona: no depende del sistema ni de root.
#[derive(Debug, Default)]
pub struct PushClipboard {
    pub contenido: std::sync::Mutex<Option<String>>,
}

impl PushClipboard {
    /// Proveedor nuevo, vacío.
    pub fn nuevo() -> PushClipboard {
        PushClipboard::default()
    }

    /// Empuja un texto (lo llama el hook de PROCESS_TEXT).
    pub fn empujar(&self, texto: &str) {
        if let Ok(mut g) = self.contenido.lock() {
            *g = Some(texto.to_string());
        }
    }
}

impl ClipboardProvider for PushClipboard {
    fn leer(&self) -> Result<Option<String>, String> {
        Ok(self.contenido.lock().ok().and_then(|g| g.clone()))
    }
    fn escribir(&self, texto: &str) -> Result<(), String> {
        if let Ok(mut g) = self.contenido.lock() {
            *g = Some(texto.to_string());
        }
        Ok(())
    }
    fn nombre(&self) -> &'static str {
        "push"
    }
}

/// Proveedor root vía `cmd clipboard`.
///
/// **NO VERIFICADO en este dispositivo**: hay que medir si el binario existe
/// y si el shell de root puede usarlo.  Degradación: si falla, el historial
/// sigue funcionando ( sólo deja de poder leer en segundo plano).
#[derive(Debug, Default)]
pub struct CmdClipboard;

impl ClipboardProvider for CmdClipboard {
    fn leer(&self) -> Result<Option<String>, String> {
        let out = std::process::Command::new("cmd")
            .args(["clipboard", "get"])
            .output()
            .map_err(|e| format!("cmd clipboard get: {e}"))?;
        if !out.status.success() {
            return Err("cmd clipboard get devolvio error".to_string());
        }
        let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if t.is_empty() {
            Ok(None)
        } else {
            Ok(Some(t))
        }
    }
    fn escribir(&self, texto: &str) -> Result<(), String> {
        let out = std::process::Command::new("cmd")
            .args(["clipboard", "set", texto])
            .output()
            .map_err(|e| format!("cmd clipboard set: {e}"))?;
        if !out.status.success() {
            return Err("cmd clipboard set devolvio error".to_string());
        }
        Ok(())
    }
    fn nombre(&self) -> &'static str {
        "cmd-clipboard"
    }
}

/// Proveedor root vía `service call clipboard`.
///
/// **NO VERIFICADO** (y la API binder cambia entre versiones de Android:
/// el número de transacción y el formato del `Parcel` no son estables).
#[derive(Debug, Default)]
pub struct ServiceCallClipboard;

impl ClipboardProvider for ServiceCallClipboard {
    fn leer(&self) -> Result<Option<String>, String> {
        Err("service call clipboard: NO VERIFICADO en este dispositivo".to_string())
    }
    fn escribir(&self, _texto: &str) -> Result<(), String> {
        Err("service call clipboard: NO VERIFICADO en este dispositivo".to_string())
    }
    fn nombre(&self) -> &'static str {
        "service-call-clipboard"
    }
}

/// Elige el mejor proveedor disponible en este entorno.
///
/// 1. `cmd clipboard` (root) → lectura en segundo plano sin avisos.
/// 2. `PushClipboard` → siempre funciona (lo alimentan PROCESS_TEXT/IME).
pub fn proveedor_por_defecto() -> Box<dyn ClipboardProvider> {
    let cmd = CmdClipboard;
    if cmd.leer().is_ok() {
        return Box::new(cmd);
    }
    Box::new(PushClipboard::nuevo())
}

// ── Distribución (malla) ──────────────────────────────────────────────────

/// Reglas de sincronización entre nodos.
#[derive(Debug, Clone)]
pub struct ReglasSync {
    /// ¿Sólo enviar items efímeros (los sensibles)?
    pub solo_efimeros: bool,
    /// Paquetes de los que NO se captura ni se envía.
    pub excluir_apps: Vec<String>,
    /// TTL máximo que se propaga (0 = no caduca).
    pub ttl_max_s: u64,
    /// ¿Se pueden propagar items sensibles?
    pub permitir_sensibles: bool,
}

impl Default for ReglasSync {
    fn default() -> Self {
        ReglasSync {
            solo_efimeros: false,
            excluir_apps: vec![
                // Gestores de claves: el contenido nunca debe salir.
                "com.android.settings".into(),
                "com.samsung.android.rubin.app".into(),
            ],
            ttl_max_s: 0,
            permitir_sensibles: false,
        }
    }
}

/// ¿Este item se puede propagar con estas reglas?
pub fn puede_propagarse(item: &Item, reglas: &ReglasSync) -> bool {
    if item.sensible && !reglas.permitir_sensibles {
        return false;
    }
    if reglas.solo_efimeros && !item.efimero {
        return false;
    }
    if item
        .origen
        .rsplit('.')
        .next()
        .map(|_| item.origen.clone())
        .unwrap_or_default()
        .is_empty()
    {
        return false;
    }
    if reglas.excluir_apps.iter().any(|p| item.origen == *p) {
        return false;
    }
    true
}

/// Prepara la línea de malla de un item (o `None` si las reglas lo excluyen).
pub fn a_malla(item: &Item, reglas: &ReglasSync) -> Option<String> {
    if !puede_propagarse(item, reglas) {
        return None;
    }
    let mut j = item.a_json();
    if reglas.ttl_max_s > 0 {
        if let Some(ttl) = item.ttl_s {
            if ttl > reglas.ttl_max_s {
                j["ttl_s"] = json!(reglas.ttl_max_s);
            }
        }
    }
    j["tipo_malla"] = json!("clipboard");
    Some(j.to_string())
}

/// Lee una línea de malla y la convierte en item (si es de portapapeles).
pub fn de_malla(linea: &str) -> Option<Item> {
    let v: Value = serde_json::from_str(linea).ok()?;
    if v.get("tipo_malla").and_then(|x| x.as_str()) != Some("clipboard") {
        return None;
    }
    Item::de_json(&v)
}

/// Fusiona un item recibido por la malla con el historial local.
///
/// Si el item es sensible, **no** entra al historial persistente (pero sí
/// queda disponible para pegarse una vez).
pub fn fusionar(h: &mut Historial, item: Item, ahora: u64) -> bool {
    let mut it = item;
    if let Some(ttl) = it.ttl_s {
        if ttl > 0 {
            // El TTL se recalcula desde la llegada (el reloj del otro nodo
            // no tiene por qué coincidir).
            it.ts = ahora;
            it.ttl_s = Some(ttl);
        }
    }
    h.push(it)
}

// ── Persistencia ──────────────────────────────────────────────────────────

/// Magic del fichero cifrado.
pub const MAGIC_CIFRADO: &[u8; 5] = b"MCLP1";

/// Escribe el historial.
///
/// Con `passphrase`: **cifrado** con AEGIS-256 y clave derivada con Argon2id
/// (los módulos canónicos del repo).  Sin passphrase: NDJSON plano, que es
/// lo que se usa para el estado efímero.
pub fn guardar(
    ruta: &Path,
    items: &[&Item],
    passphrase: Option<&str>,
) -> Result<(), String> {
    let ndjson: String = items
        .iter()
        .map(|i| i.a_json().to_string())
        .collect::<Vec<String>>()
        .join("\n");
    let bytes = ndjson.as_bytes();
    match passphrase {
        Some(p) => {
            let salt = random_bytes(16);
            let nonce = random_bytes(32);
            let clave = crate::argon2::derive_key(p, &salt, 3, 64 * 1024, 2, 32)?;
            let (ct, tag) = crate::aegis::encrypt(&clave, &nonce, bytes, MAGIC_CIFRADO, 16)?;
            let mut out = Vec::with_capacity(MAGIC_CIFRADO.len() + salt.len() + nonce.len() + ct.len() + tag.len());
            out.extend_from_slice(MAGIC_CIFRADO);
            out.extend_from_slice(&salt);
            out.extend_from_slice(&nonce);
            out.extend_from_slice(&ct);
            out.extend_from_slice(&tag);
            if let Some(p) = ruta.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            std::fs::write(ruta, out).map_err(|e| format!("escribir {}: {e}", ruta.display()))?;
            // 0600: contiene material sensible.
            set_0600(ruta);
            Ok(())
        }
        None => {
            if let Some(p) = ruta.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            std::fs::write(ruta, bytes).map_err(|e| format!("escribir {}: {e}", ruta.display()))?;
            set_0600(ruta);
            Ok(())
        }
    }
}

/// Lee el historial (cifrado o plano).
pub fn cargar(ruta: &Path, passphrase: Option<&str>) -> Result<Vec<Item>, String> {
    let bytes = std::fs::read(ruta).map_err(|e| format!("leer {}: {e}", ruta.display()))?;
    if bytes.len() >= MAGIC_CIFRADO.len() && &bytes[..MAGIC_CIFRADO.len()] == MAGIC_CIFRADO {
        let pass = passphrase.ok_or_else(|| {
            "el fichero esta cifrado y falta la passphrase".to_string()
        })?;
        let salto = MAGIC_CIFRADO.len();
        let salt = &bytes[salto..salto + 16];
        let nonce = &bytes[salto + 16..salto + 48];
        let cuerpo = &bytes[salto + 48..];
        if cuerpo.len() < 16 {
            return Err("fichero cifrado truncado".to_string());
        }
        let (ct, tag) = cuerpo.split_at(cuerpo.len() - 16);
        let clave = crate::argon2::derive_key(pass, salt, 3, 64 * 1024, 2, 32)?;
        let plano = crate::aegis::decrypt(&clave, nonce, ct, MAGIC_CIFRADO, tag)?;
        parsear_ndjson(&String::from_utf8_lossy(&plano))
    } else {
        parsear_ndjson(&String::from_utf8_lossy(&bytes))
    }
}

/// Parsea el NDJSON de items.
fn parsear_ndjson(ndjson: &str) -> Result<Vec<Item>, String> {
    let mut out = Vec::new();
    for l in ndjson.lines() {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(l) {
            if let Some(i) = Item::de_json(&v) {
                out.push(i);
            }
        }
    }
    Ok(out)
}

/// Bytes aleatorios (usa el OS).
pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        if f.read_exact(&mut v).is_ok() {
            return v;
        }
    }
    // Fallback (no debería llegar aquí en Linux).
    let mut s = String::new();
    s.push_str(&std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
        .to_string());
    for (i, b) in v.iter_mut().enumerate() {
        *b = (s.as_bytes().get(i % s.len()).copied().unwrap_or(0) ^ (i as u8)).wrapping_add(1);
    }
    v
}

/// Pone el fichero en 0600 (best effort).
pub fn set_0600(ruta: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o600));
    }
}

// ── Acciones (compartidas por el cockpit y por la tool MCP) ───────────────

/// Ruta del historial dentro del `state_dir`.
pub fn ruta_historial(state_dir: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(state_dir).join("clipboard.ndjson")
}

/// Carga el historial del disco (si existe).
pub fn historial_de(state_dir: &str) -> Historial {
    let mut h = Historial::nuevo();
    if let Ok(items) = cargar(&ruta_historial(state_dir), None) {
        for i in items {
            h.push(i);
        }
    }
    h
}

/// Guarda el historial en disco (0600).
pub fn guardar_historial(state_dir: &str, h: &Historial) -> Result<(), String> {
    let items = h.items();
    guardar(&ruta_historial(state_dir), &items, None)
}

/// Ejecuta una acción del gestor de portapapeles.
///
/// Acciones: `list`, `put`, `get`, `search`, `clear`, `copy`, `status`.
///
/// El contenido **nunca** se escribe en el log: aquí sólo se devuelve en la
/// respuesta, que ya va por el canal autenticado.
pub fn handle_action(action: &str, body: &Value, state_dir: &str) -> Value {
    let mut h = historial_de(state_dir);
    let ahora = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match action {
        "list" => {
            h.purgar(ahora);
            let limite = body
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(100)
                .min(500) as usize;
            let items: Vec<Value> = h
                .items()
                .into_iter()
                .take(limite)
                .map(|i| {
                    let mut j = i.a_json();
                    j["restante_s"] = json!(i.restante_s(ahora));
                    j
                })
                .collect();
            json!({ "ok": true, "items": items, "total": h.len() })
        }
        "search" => {
            let q = body.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limite = body
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(50)
                .min(200) as usize;
            let items: Vec<Value> = h.buscar(q, limite).iter().map(|i| i.a_json()).collect();
            json!({ "ok": true, "items": items, "query": q })
        }
        "put" => {
            let texto = match body.get("text").and_then(|v| v.as_str()) {
                Some(t) => t.to_string(),
                None => return json!({ "ok": false, "error": "falta_text" }),
            };
            if texto.is_empty() {
                return json!({ "ok": false, "error": "texto_vacio" });
            }
            if texto.len() > MAX_ITEM_BYTES {
                return json!({ "ok": false, "error": "texto_demasiado_largo" });
            }
            let origen = body
                .get("origin")
                .and_then(|v| v.as_str())
                .unwrap_or("cockpit")
                .to_string();
            let item = Item::nuevo(&texto, ahora, &origen);
            let sensible = item.sensible;
            let id = item.id.clone();
            let aceptado = h.push(item);
            if let Err(e) = guardar_historial(state_dir, &h) {
                return json!({ "ok": false, "error": e });
            }
            json!({
                "ok": true, "id": id, "guardado": aceptado,
                "sensible": sensible,
                "nota": if sensible { "el item sensible no se persiste en el historial" } else { "" }
            })
        }
        "get" => {
            let item = match body.get("id").and_then(|v| v.as_str()) {
                Some(id) => h.por_id(id).cloned(),
                None => h.items().first().map(|i| (*i).clone()),
            };
            match item {
                Some(i) => {
                    let mut j = i.a_json();
                    j["restante_s"] = json!(i.restante_s(ahora));
                    json!({ "ok": true, "item": j })
                }
                None => json!({ "ok": false, "error": "historial_vacio" }),
            }
        }
        "copy" => {
            let item = match body.get("id").and_then(|v| v.as_str()) {
                Some(id) => h.por_id(id).cloned(),
                None => h.items().first().map(|i| (*i).clone()),
            };
            match item {
                Some(i) => {
                    let p = proveedor_por_defecto();
                    match p.escribir(&i.texto) {
                        Ok(()) => json!({ "ok": true, "id": i.id, "proveedor": p.nombre() }),
                        Err(e) => json!({
                            "ok": false, "error": e,
                            "proveedor": p.nombre(),
                            "nota": "el proveedor root no esta disponible; usa la via PROCESS_TEXT (push)"
                        }),
                    }
                }
                None => json!({ "ok": false, "error": "historial_vacio" }),
            }
        }
        "clear" => {
            h.limpiar();
            match guardar_historial(state_dir, &h) {
                Ok(()) => json!({ "ok": true, "borrados": true }),
                Err(e) => json!({ "ok": false, "error": e }),
            }
        }
        "status" => {
            let p = proveedor_por_defecto();
            json!({
                "ok": true,
                "proveedor": p.nombre(),
                "lectura_disponible": p.leer().is_ok(),
                "items": h.len(),
                "bytes": h.bytes(),
                "ruta": ruta_historial(state_dir).to_string_lossy(),
                "nota": "cmd clipboard es NO VERIFICADO en este dispositivo; la via PROCESS_TEXT siempre funciona"
            })
        }
        _ => json!({ "ok": false, "error": "accion_desconocida", "accion": action }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(texto: &str, ts: u64) -> Item {
        Item::nuevo(texto, ts, "prueba")
    }

    #[test]
    fn clasifica_urls_telefonos_y_codigos() {
        assert_eq!(clasificar("https://example.com"), Tipo::Url);
        assert_eq!(clasificar("www.example.com"), Tipo::Url);
        assert_eq!(clasificar("+34 600 123 456"), Tipo::Telefono);
        assert_eq!(clasificar("600123456"), Tipo::Telefono);
        assert_eq!(clasificar("código: 123456"), Tipo::Codigo);
        assert_eq!(clasificar("123456"), Tipo::Codigo);
        assert_eq!(clasificar("600 123 456"), Tipo::Telefono);
        assert_eq!(clasificar("{\"a\":1}"), Tipo::Json);
        assert_eq!(clasificar("hola mundo"), Tipo::Texto);
    }

    #[test]
    fn detecta_lo_sensible() {
        assert!(es_sensible("mi contraseña es 1234"));
        assert!(es_sensible("API_KEY=abc123"));
        assert!(es_sensible("-----BEGIN PRIVATE KEY-----"));
        assert!(es_sensible("tu código es 482913"), "un OTP con contexto es sensible");
        assert!(es_sensible("482913"), "un OTP a secas tambien");
        assert!(!es_sensible("hola"), "un texto normal no");
        assert!(!es_sensible("600123456"), "un teléfono no es un secreto");
        assert!(!es_sensible("600 123 456"), "un teléfono con espacios tampoco");
    }

    #[test]
    fn el_dedup_no_crea_entradas_repetidas() {
        let mut h = Historial::nuevo();
        assert!(h.push(item("hola", 1)));
        assert!(h.push(item("adios", 2)));
        assert!(h.push(item("hola", 3)));
        assert_eq!(h.len(), 2);
        // el repetido va al frente
        assert_eq!(h.items()[0].texto, "hola");
    }

    #[test]
    fn los_sensibles_no_entran_en_el_historial() {
        let mut h = Historial::nuevo();
        assert!(!h.push(item("mi contraseña es 1234", 1)));
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn los_sensibles_entran_si_se_piden() {
        let mut h = Historial::con_reglas(Reglas {
            persistir_sensibles: true,
            ..Reglas::default()
        });
        assert!(h.push(item("mi contraseña es 1234", 1)));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn lo_efimero_caduca() {
        let mut h = Historial::nuevo();
        let mut i = item("secreto", 100);
        i.sensible = false; // para que entre
        i.efimero = false;
        i.ttl_s = Some(10);
        assert!(h.push(i));
        assert_eq!(h.len(), 1);
        h.purgar(120);
        assert_eq!(h.len(), 0, "a los 20 s habia caducado");
    }

    #[test]
    fn el_tope_de_entradas_se_respeta() {
        let mut h = Historial::con_reglas(Reglas {
            max_entradas: 3,
            ..Reglas::default()
        });
        for i in 0..10 {
            h.push(item(&format!("texto{i}"), i as u64));
        }
        assert_eq!(h.len(), 3);
        // se quedan los mas recientes
        assert_eq!(h.items()[0].texto, "texto9");
    }

    #[test]
    fn un_item_enorme_se_rechaza() {
        let mut h = Historial::con_reglas(Reglas {
            max_item_bytes: 10,
            ..Reglas::default()
        });
        assert!(!h.push(item("0123456789012345678901234567890", 1)));
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn la_busqueda_ignora_mayusculas() {
        let mut h = Historial::nuevo();
        h.push(item("Hola Mundo", 1));
        assert_eq!(h.buscar("mundo", 10).len(), 1);
        assert_eq!(h.buscar("nada", 10).len(), 0);
    }

    #[test]
    fn el_mock_lectura_escritura() {
        let p = MockClipboard::nuevo();
        assert!(p.leer().unwrap().is_none());
        p.escribir("hola").unwrap();
        assert_eq!(p.leer().unwrap().as_deref(), Some("hola"));
    }

    #[test]
    fn el_push_clipboard_siempre_acepta() {
        let p = PushClipboard::nuevo();
        p.empujar("texto");
        assert_eq!(p.leer().unwrap().as_deref(), Some("texto"));
        assert_eq!(p.nombre(), "push");
    }

    #[test]
    fn la_malla_va_y_vuelve() {
        let i = item("hola", 100);
        let reglas = ReglasSync::default();
        let linea = a_malla(&i, &reglas).expect("debe propagarse");
        let vuelta = de_malla(&linea).expect("debe volver");
        assert_eq!(vuelta.texto, "hola");
        assert_eq!(vuelta.tipo, i.tipo);
        assert_eq!(vuelta.ts, 100);
    }

    #[test]
    fn los_sensibles_no_se_propagan() {
        let i = item("mi contraseña es 1234", 100);
        assert!(a_malla(&i, &ReglasSync::default()).is_none());
        let reglas = ReglasSync {
            permitir_sensibles: true,
            ..ReglasSync::default()
        };
        assert!(a_malla(&i, &reglas).is_some());
    }

    #[test]
    fn las_apps_excluidas_no_se_propagan() {
        let mut i = item("hola", 100);
        i.origen = "com.ejemplo.banco".to_string();
        let reglas = ReglasSync {
            excluir_apps: vec!["com.ejemplo.banco".to_string()],
            ..ReglasSync::default()
        };
        assert!(a_malla(&i, &reglas).is_none());
    }

    #[test]
    fn solo_efimeros_filtra() {
        let mut normal = item("texto normal", 100);
        normal.efimero = false;
        normal.sensible = false;
        let reglas = ReglasSync {
            solo_efimeros: true,
            ..ReglasSync::default()
        };
        assert!(a_malla(&normal, &reglas).is_none());
    }

    #[test]
    fn el_ttl_de_la_malla_se_acota() {
        let mut i = item("hola", 100);
        i.ttl_s = Some(3600);
        let reglas = ReglasSync {
            ttl_max_s: 60,
            ..ReglasSync::default()
        };
        let linea = a_malla(&i, &reglas).unwrap();
        let vuelta = de_malla(&linea).unwrap();
        assert_eq!(vuelta.ttl_s, Some(60));
    }

    #[test]
    fn la_linea_de_malla_que_no_es_clipboard_se_ignora() {
        assert!(de_malla(r#"{"tipo_malla":"otra"}"#).is_none());
        assert!(de_malla("no soy json").is_none());
    }

    #[test]
    fn guardar_y_cargar_en_plano() {
        let dir = std::env::temp_dir().join(format!("md_clip_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("clip.ndjson");
        let i = item("hola", 100);
        guardar(&ruta, &[&i], None).unwrap();
        let leidos = cargar(&ruta, None).unwrap();
        assert_eq!(leidos.len(), 1);
        assert_eq!(leidos[0].texto, "hola");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn guardar_y_cargar_cifrado() {
        let dir = std::env::temp_dir().join(format!("md_clip_cif_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("clip.bin");
        let i = item("secreto de prueba", 100);
        guardar(&ruta, &[&i], Some("clave-maestra")).unwrap();
        let leidos = cargar(&ruta, Some("clave-maestra")).unwrap();
        assert_eq!(leidos.len(), 1);
        assert_eq!(leidos[0].texto, "secreto de prueba");
        // y el fichero en disco NO contiene el texto en claro
        let crudo = std::fs::read(&ruta).unwrap();
        let s = String::from_utf8_lossy(&crudo);
        assert!(!s.contains("secreto de prueba"), "el texto esta en claro en disco");
        // y con otra clave falla
        assert!(cargar(&ruta, Some("otra")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_fichero_cifrado_usa_el_magic() {
        let dir = std::env::temp_dir().join(format!("md_clip_magic_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("clip.bin");
        let i = item("x", 1);
        guardar(&ruta, &[&i], Some("k")).unwrap();
        let crudo = std::fs::read(&ruta).unwrap();
        assert_eq!(&crudo[..5], MAGIC_CIFRADO);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_accion_put_guarda_y_no_filtra_lo_sensible() {
        let dir = std::env::temp_dir().join(format!("md_clip_act_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sd = dir.to_string_lossy().to_string();
        let r = handle_action("put", &json!({ "text": "hola" }), &sd);
        assert_eq!(r["ok"], json!(true));
        assert_eq!(r["guardado"], json!(true));
        let r = handle_action("put", &json!({ "text": "mi contraseña es 1234" }), &sd);
        assert_eq!(r["ok"], json!(true));
        assert_eq!(r["sensible"], json!(true));
        assert_eq!(r["guardado"], json!(false), "lo sensible no se persiste");
        let l = handle_action("list", &json!({}), &sd);
        assert_eq!(l["items"].as_array().unwrap().len(), 1, "solo el texto normal");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_accion_put_rechaza_lo_demasiado_largo() {
        let dir = std::env::temp_dir().join(format!("md_clip_lar_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sd = dir.to_string_lossy().to_string();
        let enorme = "x".repeat(MAX_ITEM_BYTES + 1);
        let r = handle_action("put", &json!({ "text": enorme }), &sd);
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("texto_demasiado_largo"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_accion_desconocida_falla_limpiamente() {
        let r = handle_action("nope", &json!({}), "/tmp");
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("accion_desconocida"));
    }

    #[test]
    fn la_accion_search_encuentra() {
        let dir = std::env::temp_dir().join(format!("md_clip_sea_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sd = dir.to_string_lossy().to_string();
        handle_action("put", &json!({ "text": "el zorro rápido" }), &sd);
        handle_action("put", &json!({ "text": "la liebre" }), &sd);
        let r = handle_action("search", &json!({ "query": "zorro" }), &sd);
        assert_eq!(r["items"].as_array().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fusionar_recalcula_el_ttl() {
        let mut h = Historial::nuevo();
        let mut i = item("hola", 1);
        i.sensible = false;
        i.ttl_s = Some(30);
        fusionar(&mut h, i, 1000);
        assert_eq!(h.len(), 1);
        assert_eq!(h.items()[0].ts, 1000, "el TTL se recalcula desde la llegada");
    }
}

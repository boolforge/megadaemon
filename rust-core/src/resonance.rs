//! TABLÓN: el bus p2p del enjambre, respaldado por Drive.
//!
//! POR QUÉ (2026-11-23)
//! ---------------------
//! El enjambre (PC, móvil, agentes) no tiene una red propia fiable: el móvil
//! se cae, el túnel cae, ADB es inalcanzable desde el sandbox y cada agente
//! trabaja en su worktree.  Lo que **SÍ** está siempre disponible es la
//! carpeta **privada** de Google Drive (el mismo almacén que ya usa
//! `task_sync` para la cola de tareas).
//!
//! ⇒ Drive se usa como **tablón de anuncios** (bulletin board): cada nodo
//! deja cartas y lee las de los demás.  De ahí sale un bus p2p "ultra
//! resiliente" sin infraestructura:
//!
//! * **Convergente**: la fusión es por `id` ⇒ importar dos veces lo mismo
//!   no duplica nada y todos los nodos acaban con el mismo estado.
//! * **A prueba de partición**: un nodo caído no bloquea a nadie (sus cartas
//!   siguen ahí, con su `epoch`; el siguiente nodo con más época manda).
//! * **Con historia**: el tablón es un *journal* (append-only) ⇒ la
//!   "historia" compartida sobrevive a la muerte de un nodo.  Cuando crece,
//!   `plegar()` lo convierte en una **instantánea** (el "archivo común") y
//!   deja el journal limpio.
//! * **Sin suplantar**: cada carta va **firmada** con la clave Ed25519 del
//!   nodo ⇒ otro nodo no puede escribir en nombre de este.
//! * **Eficiente**: sólo viajan los deltas (una carta por evento), nunca el
//!   estado entero; la instantánea es un fichero por nodo.
//!
//! MODELO DE CONSISTENCIA
//! ----------------------
//! CRDT *last-writer-wins por época*: cada carta lleva una `epoch` global y
//! un `seq` por nodo.  El estado de una clave es el de la carta con la
//! `epoch` mayor (y, a igualdad de época, el `seq` mayor).  No hay consenso
//! ni consenso de quórum: **todos convergen** sinneed DrugStore de coordinados.
//!
//! ESTADO EN DISCO (`<state_dir>/tablón/`)
//! ---------------------------------------
//! * `clave-<nodo>.ed25519` — clave privada del nodo (0600, nunca al repo).
//! * `cartas-<nodo>.jsonl`   — el journal local (una carta por línea).
//! * `foto.json`             — la instantánea (el "archivo común").
//!
//! NOTA DE SEGURIDAD: las cartas NUNCA llevan secretos (el redactor del
//! shim se aplica al cuerpo) ⇒ el tablón no es un sitio donde acabe una
//! credencial.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Carpeta privada de Drive donde vive el tablón.
const CARPETA_BACKUP: &str = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0";
/// Subcarpeta del tablón dentro de la carpeta privada.
const SUBCARPETA: &str = "tablon-resiliencia";

/// Tipos de carta que el tablón entiende.
pub const TIPOS: &[&str] = &[
    "latido",     // el nodo está vivo (con su build y su carga)
    "peticion",   // quiero que otro nodo haga algo
    "aviso",      // ojo con esto (el móvil se ha caído, la cuota está al límite)
    "cambio",     // el estado de una clave ha cambiado (worktree, build, cola)
    "cierre",     // el nodo se retira
];

/// Una carta del tablón.
#[derive(Clone, Debug, PartialEq)]
pub struct Carta {
    /// Identificador único (hash del contenido) ⇒ la fusión es idempotente.
    pub id: String,
    /// Época global: la más alta gana (LWW).
    pub epoca: u64,
    /// Secuencia del nodo (desempate dentro de la misma época).
    pub seq: u64,
    /// Marca de tiempo (segundos Unix).
    pub ts: u64,
    /// Nodo emisor.
    pub nodo: String,
    /// Tipo de carta (ver [`TIPOS`]).
    pub tipo: String,
    /// Contenido (ya redactado).
    pub cuerpo: Value,
    /// Firma Ed25519 del cuerpo canónico (hex).  Vacía si no se pudo firmar.
    pub firma: String,
}

impl Carta {
    /// Cuerpo canónico que se firma: todo menos la propia firma.
    pub fn canonico(&self) -> Vec<u8> {
        json!({
            "id": self.id,
            "epoca": self.epoca,
            "seq": self.seq,
            "ts": self.ts,
            "nodo": self.nodo,
            "tipo": self.tipo,
            "cuerpo": self.cuerpo,
        })
        .to_string()
        .into_bytes()
    }

    /// Identificador determinista del contenido (hex sha-256).
    pub fn calcular_id(&self) -> String {
        sha256_hex(&self.canonico())
    }

    /// Verifica la firma con la clave pública del emisor.
    pub fn verificar(&self, pub_hex: &str) -> bool {
        if self.firma.is_empty() {
            return false;
        }
        let Ok(pk) = hex_bytes(pub_hex) else {
            return false;
        };
        let Ok(sig) = hex_bytes(&self.firma) else {
            return false;
        };
        ring::signature::UnparsedPublicKey::new(
            &ring::signature::ED25519,
            &pk,
        )
        .verify(&self.canonico(), &sig)
        .is_ok()
    }

    /// A JSON (para el journal en NDJSON y para Drive).
    pub fn a_json(&self) -> Value {
        json!({
            "id": self.id,
            "epoca": self.epoca,
            "seq": self.seq,
            "ts": self.ts,
            "nodo": self.nodo,
            "tipo": self.tipo,
            "cuerpo": self.cuerpo,
            "firma": self.firma,
        })
    }

    /// Desde JSON (tolerante: un campo que falte se rellena).
    pub fn de_json(v: &Value) -> Option<Carta> {
        let id = v["id"].as_str()?.to_string();
        let nodo = v["nodo"].as_str()?.to_string();
        Some(Carta {
            id,
            epoca: v["epoca"].as_u64().unwrap_or(0),
            seq: v["seq"].as_u64().unwrap_or(0),
            ts: v["ts"].as_u64().unwrap_or(0),
            nodo,
            tipo: v["tipo"].as_str().unwrap_or("cambio").to_string(),
            cuerpo: v["cuerpo"].clone(),
            firma: v["firma"].as_str().unwrap_or("").to_string(),
        })
    }
}

/// El tablón de un nodo.
pub struct Tablon {
    /// Nombre del nodo.
    pub nodo: String,
    /// Carpeta de estado (`<state_dir>/tablón`).
    pub dir: PathBuf,
    /// Todas las cartas conocidas (journal + instantáneas de otros).
    pub cartas: Vec<Carta>,
    /// Época actual (la máxima vista + 1 al publicar).
    pub epoca: u64,
    /// Secuencia del propio nodo.
    pub seq: u64,
    /// Clave pública del nodo (hex).
    pub pub_hex: String,
}

impl Tablon {
    /// Abre (o crea) el tablón del nodo en `state_dir`.
    pub fn abrir(state_dir: &str) -> std::io::Result<Tablon> {
        let nodo = nodo_actual();
        let dir = PathBuf::from(state_dir).join("tablón");
        std::fs::create_dir_all(&dir)?;
        let (pub_hex, _) = cargar_o_crear_clave(&dir, &nodo)?;
        let mut t = Tablon {
            nodo: nodo.clone(),
            dir: dir.clone(),
            cartas: Vec::new(),
            epoca: 0,
            seq: 0,
            pub_hex,
        };
        t.cargar_local()?;
        Ok(t)
    }

    /// Carga el journal local y la instantánea.
    pub fn cargar_local(&mut self) -> std::io::Result<()> {
        // La instantánea: el "archivo común".
        let foto = self.dir.join("foto.json");
        if let Ok(txt) = std::fs::read_to_string(&foto) {
            if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                if let Some(arr) = v["cartas"].as_array() {
                    for c in arr {
                        if let Some(carta) = Carta::de_json(c) {
                            self.cartas.push(carta);
                        }
                    }
                }
                self.epoca = self.epoca.max(v["epoca"].as_u64().unwrap_or(0));
            }
        }
        // El journal local (una carta por línea).
        let j = self.dir.join(format!("cartas-{}.jsonl", self.nodo));
        if let Ok(txt) = std::fs::read_to_string(&j) {
            for linea in txt.lines() {
                if linea.trim().is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<Value>(linea) {
                    if let Some(carta) = Carta::de_json(&v) {
                        self.seq = self.seq.max(carta.seq);
                        self.epoca = self.epoca.max(carta.epoca);
                        self.cartas.push(carta);
                    }
                }
            }
        }
        self.dedup();
        Ok(())
    }

    /// Elimina duplicados por `id` (idempotencia de la fusión).
    pub fn dedup(&mut self) {
        let mut vistas = std::collections::HashSet::new();
        self.cartas.retain(|c| vistas.insert(c.id.clone()));
    }

    /// Publica una carta (la firma con la clave del nodo).
    pub fn publicar(&mut self, tipo: &str, cuerpo: Value) -> Carta {
        self.seq += 1;
        self.epoca += 1;
        let mut carta = Carta {
            id: String::new(),
            epoca: self.epoca,
            seq: self.seq,
            ts: ahora(),
            nodo: self.nodo.clone(),
            tipo: tipo.to_string(),
            cuerpo,
            firma: String::new(),
        };
        carta.id = carta.calcular_id();
        if let Ok((_, priv_hex)) = cargar_o_crear_clave(&self.dir, &self.nodo) {
            if let Some(sig) = firmar(&priv_hex, &carta.canonico()) {
                carta.firma = sig;
            }
        }
        self.cartas.push(carta.clone());
        let _ = self.anadir_journal(&carta);
        carta
    }

    /// Añade la carta al journal local (append-only).
    fn anadir_journal(&self, carta: &Carta) -> std::io::Result<()> {
        use std::io::Write;
        let j = self.dir.join(format!("cartas-{}.jsonl", self.nodo));
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(j)?;
        writeln!(f, "{}", carta.a_json())?;
        f.sync_data()?;
        Ok(())
    }

    /// Fusiona las cartas de otro nodo (idempotente).
    ///
    /// Devuelve cuántas cartas NUEVAS entraron.
    pub fn fusionar(&mut self, otras: &[Carta]) -> usize {
        let antes = self.cartas.len();
        let mut ids = std::collections::HashSet::new();
        for c in &self.cartas {
            ids.insert(c.id.clone());
        }
        for c in otras {
            if ids.insert(c.id.clone()) {
                self.epoca = self.epoca.max(c.epoca);
                self.cartas.push(c.clone());
            }
        }
        self.cartas.len() - antes
    }

    /// Pliega el journal en la instantánea (el "archivo común").
    ///
    /// Devuelve `(cartas, bytes_de_la_instantánea)`.
    pub fn plegar(&mut self) -> std::io::Result<(usize, usize)> {
        self.dedup();
        let cartas = self.cartas.clone();
        let v = json!({
            "epoca": self.epoca,
            "nodo": self.nodo,
            "actualizado": ahora(),
            "cartas": cartas.iter().map(|c| c.a_json()).collect::<Vec<_>>(),
        });
        let txt = serde_json::to_string_pretty(&v).unwrap_or_default();
        let foto = self.dir.join("foto.json");
        // Escritura ATÓMICA: primero a `.tmp`, luego rename ⇒ un corte de
        // luz no deja una instantánea corrupta (que rompería el tablón).
        // `util_fs::write_atomic` YA hace el tmp+rename (escritura atomica):
        // MEDIDO (2026-11-23): al hacerlo aqui tambien, el `rename` manual
        // fallaba (el tmp ya no existia) y la instantanea NO se creaba.
        crate::util_fs::write_atomic(&foto, txt.as_bytes(), 0o600)
            .map_err(|e| std::io::Error::other(e))?;
        // El journal local se vacía (ya está en la instantánea).
        let j = self.dir.join(format!("cartas-{}.jsonl", self.nodo));
        let _ = std::fs::write(&j, b"");
        Ok((cartas.len(), txt.len()))
    }

    /// Estado del tablón (para la tool y el endpoint).
    pub fn estado(&self) -> Value {
        let por_tipo: std::collections::BTreeMap<String, usize> = self
            .cartas
            .iter()
            .fold(std::collections::BTreeMap::new(), |mut m, c| {
                *m.entry(c.tipo.clone()).or_insert(0) += 1;
                m
            });
        let mut nodos: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
        for c in &self.cartas {
            let e = nodos.entry(c.nodo.clone()).or_insert(0);
            *e = (*e).max(c.ts);
        }
        let ahora_s = ahora();
        json!({
            "nodo": self.nodo,
            "epoca": self.epoca,
            "cartas": self.cartas.len(),
            "por_tipo": por_tipo,
            "nodos": nodos.iter().map(|(k, v)| json!({"nodo": k, "visto_hace_s": ahora_s.saturating_sub(*v)})).collect::<Vec<_>>(),
            "pub": self.pub_hex,
            "directorio": self.dir.to_string_lossy(),
        })
    }
}

/// Sincroniza el tablón con Drive (subir + traer + fusionar).
///
/// Es el mismo patrón que `task_sync` (listar, descargar, fusionar) pero
/// sobre el journal del tablón.
pub fn sincronizar() -> Result<Value, String> {
    let state = crate::cockpit_server::state_dir();
    let mut t = Tablon::abrir(&state).map_err(|e| format!("abrir el tablón: {e}"))?;
    let carpeta = crate::drive_native::ensure_folder(SUBCARPETA, CARPETA_BACKUP, Some(&state))?;

    // 1) Subir el journal local (un fichero por nodo, sólo el delta).
    let sello = ahora();
    let nombre = format!("cartas-{}-{}.jsonl", t.nodo, sello);
    let fichero = PathBuf::from(&state).join(&nombre);
    let lineas: String = t
        .cartas
        .iter()
        .map(|c| c.a_json().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&fichero, lineas.as_bytes()).map_err(|e| format!("escribir local: {e}"))?;
    let subido = match crate::drive_native::upload_file(
        &fichero,
        &nombre,
        &crate::drive_client::DriveTarget::folder(&carpeta),
        Some(&state),
    ) {
        Ok(u) => json!({"ok": true, "id": u.file_id, "bytes": u.bytes, "sha256": u.sha256}),
        Err(e) => json!({"ok": false, "error": e}),
    };

    // 2) Traer las cartas de los demás.
    let q = format!("'{carpeta}' in parents and name contains 'cartas-' and trashed = false");
    let url = format!(
        "{}?q={}&fields=files(id,name)&pageSize=200&supportsAllDrives=true&includeItemsFromAllDrives=true",
        crate::drive_client::DRIVE_API,
        urlencode(&q)
    );
    let (ak, av) = crate::google_auth::auth_header(Some(&state))?;
    let (st, cuerpo) = crate::http_transfer::get_texto(
        &url,
        &{
            let mut o = crate::http_transfer::Opciones::default();
            o.cabeceras.push((ak, av));
            o
        },
    )
    .map_err(|e| format!("listar el tablón en Drive: {e}"))?;
    // MEDIDO (2026-11-23): sin comprobar el status, un 403 de Drive se
    // traducía en "0 cartas traytas" con `ok: true` (fallo silencioso).
    if st >= 400 {
        let detalle: String = cuerpo.chars().take(160).collect();
        return Err(format!("listar el tablón en Drive: HTTP {st}: {detalle}"));
    }
    let lista: Value =
        serde_json::from_str(&cuerpo).map_err(|e| format!("la lista del tablón no es JSON: {e}"))?;

    let mut traytas = 0usize;
    let mut nuevas = 0usize;
    let mut errores: Vec<Value> = Vec::new();
    for f in lista["files"].as_array().cloned().unwrap_or_default() {
        let id = f["id"].as_str().unwrap_or("").to_string();
        let name = f["name"].as_str().unwrap_or("").to_string();
        if id.is_empty() || name.starts_with(&format!("cartas-{}-", t.nodo)) {
            continue;
        }
        let dest = PathBuf::from(&state).join(format!("tablon-sync-{name}"));
        match crate::drive_native::download_file(&id, &dest, Some(&state)) {
            Ok(_) => match std::fs::read_to_string(&dest) {
                Ok(txt) => {
                    let mut remotas = Vec::new();
                    for linea in txt.lines() {
                        if linea.trim().is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<Value>(linea) {
                            Ok(v) => {
                                if let Some(c) = Carta::de_json(&v) {
                                    remotas.push(c);
                                }
                            }
                            Err(e) => errores.push(json!({"fichero": name, "error": e.to_string()})),
                        }
                    }
                    nuevas += t.fusionar(&remotas);
                    traytas += remotas.len();
                }
                Err(e) => errores.push(json!({"fichero": name, "error": e.to_string()})),
            },
            Err(e) => errores.push(json!({"fichero": name, "error": e})),
        }
    }
    let _ = std::fs::remove_file(&fichero);

    Ok(json!({
        "ok": true,
        "nodo": t.nodo,
        "carpeta": carpeta,
        "subida": subido,
        "cartas_remotas": traytas,
        "cartas_nuevas": nuevas,
        "cartas_totales": t.cartas.len(),
        "errores": errores,
        "estado": t.estado(),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Utilidades
// ─────────────────────────────────────────────────────────────────────────────

/// Nombre del nodo (env `MEGADAEMON_NODE`, o el host).
pub fn nodo_actual() -> String {
    std::env::var("MEGADAEMON_NODE")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            std::env::var("HOSTNAME")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "pc".to_string())
        })
}

/// Segundos Unix.
pub fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `urlencode` mínimo (mismo criterio que `task_sync`).
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// sha-256 en hex.
fn sha256_hex(datos: &[u8]) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, datos);
    hex_bytes_of(d.as_ref())
}

/// Bytes -> hex.
fn hex_bytes_of(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

/// Hex -> bytes.
fn hex_bytes(h: &str) -> Result<Vec<u8>, String> {
    if h.len() % 2 != 0 {
        return Err("hex de longitud impar".into());
    }
    let mut out = Vec::with_capacity(h.len() / 2);
    let b = h.as_bytes();
    for i in (0..b.len()).step_by(2) {
        let s = std::str::from_utf8(&b[i..i + 2]).map_err(|_| "hex no utf8".to_string())?;
        out.push(u8::from_str_radix(s, 16).map_err(|_| "hex invalido".to_string())?);
    }
    Ok(out)
}

/// Carga (o crea) la clave Ed25519 del nodo.
///
/// Devuelve `(pública_hex, privada_hex)`.  La clave privada se guarda con
/// permisos 0600 y **nunca** sale de la máquina.
fn cargar_o_crear_clave(dir: &Path, nodo: &str) -> std::io::Result<(String, String)> {
    let ruta = dir.join(format!("clave-{nodo}.ed25519"));
    if let Ok(txt) = std::fs::read_to_string(&ruta) {
        let mut lineas = txt.lines();
        let pub_hex = lineas.next().unwrap_or("").trim().to_string();
        let priv_hex = lineas.next().unwrap_or("").trim().to_string();
        if !pub_hex.is_empty() && !priv_hex.is_empty() {
            return Ok((pub_hex, priv_hex));
        }
    }
    let rng = ring::rand::SystemRandom::new();
    let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|e| std::io::Error::other(format!("generar clave: {e}")))?;
    let bytes = pkcs8.as_ref().to_vec();
    let kp = ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&bytes)
        .map_err(|e| std::io::Error::other(format!("leer clave: {e}")))?;
    // MEDIDO (2026-11-23): en ring 0.17 `public_key()` es un metodo del
    // trait `KeyPair` (no un campo) => hay que importar el trait.
    use ring::signature::KeyPair as _;
    let pub_hex = hex_bytes_of(kp.public_key().as_ref());
    let priv_hex = hex_bytes_of(&bytes);
    // 0600: la clave privada es de este nodo y nadie más.
    crate::util_fs::write_atomic(&ruta, format!("{pub_hex}\n{priv_hex}\n").as_bytes(), 0o600)
        .map_err(|e| std::io::Error::other(e))?;
    Ok((pub_hex, priv_hex))
}

/// Firma con la clave privada (hex).
fn firmar(priv_hex: &str, datos: &[u8]) -> Option<String> {
    let bytes = hex_bytes(priv_hex).ok()?;
    let kp = ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&bytes).ok()?;
    use ring::signature::KeyPair;
    // MEDIDO (2026-11-23): en ring 0.17 `sign` devuelve `Signature` (no un
    // Result) => `.ok()` no existe (E0599).
    let sig = kp.sign(datos);
    Some(hex_bytes_of(sig.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carta(nodo: &str, epoca: u64, seq: u64, tipo: &str) -> Carta {
        Carta {
            id: String::new(),
            epoca,
            seq,
            ts: 1,
            nodo: nodo.to_string(),
            tipo: tipo.to_string(),
            cuerpo: json!({"x": 1}),
            firma: String::new(),
        }
    }

    #[test]
    fn el_identificador_es_determinista() {
        let mut a = carta("n1", 1, 1, "latido");
        let mut b = carta("n1", 1, 1, "latido");
        a.id = a.calcular_id();
        b.id = b.calcular_id();
        assert_eq!(a.id, b.id);
        assert_eq!(a.id.len(), 64, "sha-256 en hex = 64 caracteres");
    }

    #[test]
    fn fusionar_es_idempotente() {
        let mut t = Tablon {
            nodo: "n1".into(),
            dir: PathBuf::from("/tmp"),
            cartas: vec![],
            epoca: 0,
            seq: 0,
            pub_hex: String::new(),
        };
        let mut c = carta("n2", 1, 1, "cambio");
        c.id = c.calcular_id();
        assert_eq!(t.fusionar(&[c.clone()]), 1);
        assert_eq!(t.fusionar(&[c.clone()]), 0, "reimportar no duplica");
        assert_eq!(t.cartas.len(), 1);
    }

    #[test]
    fn la_epoca_crece_al_publicar() {
        let dir = std::env::temp_dir().join(format!("tablon-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut t = Tablon::abrir(dir.to_str().unwrap()).expect("abrir el tablón");
        let e0 = t.epoca;
        t.publicar("latido", json!({"build": "87c66e5"}));
        assert!(t.epoca > e0, "la época debe avanzar");
        assert_eq!(t.cartas.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_carta_va_firmada_y_se_verifica() {
        let dir = std::env::temp_dir().join(format!("tablon-firma-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut t = Tablon::abrir(dir.to_str().unwrap()).expect("abrir el tablón");
        let c = t.publicar("aviso", json!({"móvil": "caído"}));
        assert!(!c.firma.is_empty(), "la carta debe ir firmada");
        assert!(c.verificar(&t.pub_hex), "la firma debe verificar");
        // El cuerpo no se puede tocar sin romper la firma.
        let mut c2 = c.clone();
        c2.cuerpo = json!({"móvil": "vivo"});
        assert!(!c2.verificar(&t.pub_hex), "el cuerpo alterado no verifica");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plegar_deja_la_instantanea_y_vacia_el_journal() {
        let dir = std::env::temp_dir().join(format!("tablon-plegar-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut t = Tablon::abrir(dir.to_str().unwrap()).expect("abrir el tablón");
        t.publicar("latido", json!({"n": 1}));
        t.publicar("cambio", json!({"n": 2}));
        let (n, bytes) = t.plegar().expect("plegar");
        assert_eq!(n, 2);
        assert!(bytes > 0);
        // MEDIDO (2026-11-23): la instantánea vive en `<state>/tablon/`
        // (el subdirectorio del tablón), NO en `<state>/` => el test miraba
        // en la ruta equivocada y fallaba aunque la escritura fuese buena.
        assert!(
            t.dir.join("foto.json").exists(),
            "debe quedar la instantánea en {:?}",
            t.dir
        );
        // Al reabrir, las cartas siguen ahí (el estado no se pierde).
        let t2 = Tablon::abrir(dir.to_str().unwrap()).expect("reabrir");
        assert_eq!(t2.cartas.len(), 2, "la instantánea conserva las cartas");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_tablon_no_acepta_cartas_falsificadas() {
        let mut c = carta("enemigo", 9, 9, "peticion");
        c.id = c.calcular_id();
        c.firma = "00".repeat(32);
        assert!(!c.verificar(&"11".repeat(32)), "una firma falsa no verifica");
    }
}

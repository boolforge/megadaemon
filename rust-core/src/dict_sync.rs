//! Diccionarios del usuario: sincronización entre nodos y corrección de erratas.
//!
//! # Qué resuelve
//!
//! El teclado del móvil guarda las palabras que el usuario teclea (el
//! "diccionario personal" de AOSP: `content://user_dictionary/words`).  En
//! varios dispositivos (móvil, tablet, PC) son diccionarios **distintos**:
//! lo que se aprendes en el móvil no aparece en el otro lado.
//!
//! Además, el usuario escribe erratas ("habia" por "había", "qeu" por
//! "que", "segun" por "según").  Con un diccionario propio se pueden
//! **detectar y corregir** casi automáticamente, porque las erratas
//! tipográficas están a distancia de Levenshtein 1-2 de la palabra correcta.
//!
//! # Alcance
//!
//! Sólo el diccionario **del propio usuario** y la corrección de **su**
//! texto.  Nada de diccionarios de terceros ni de escritura por otros.
//!
//! # Medido / no medido
//!
//! * El esquema del content provider (`word`, `shortcut`, `timestamp`) es el
//!   estándar de AOSP; el parser es **tolerante** (no falla si el móvil
//!   devuelve columnas distintas) y el comando exacto para medirlo en el
//!   móvil está en `docs/DICCIONARIOS.md`.
//! * La aplicación del corrector sobre texto real depende de cómo el
//!   teclado entregue el texto: aquí se ofrece como **API** y como CLI, no
//!   como gancho al teclado (eso requiere un IME propio o accesibilidad).

use std::collections::{BTreeMap, BTreeSet};

/// Una entrada del diccionario personal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// La palabra aprendida.
    pub palabra: String,
    /// Atajos associated (el usuario puede tener "qeu" -> "que").
    pub atajos: Vec<String>,
    /// Epoch de la última modificación.
    pub ts: u64,
    /// De dónde viene: "teclado", "sincronizado:<nodo>", "importado:<fichero>".
    pub origen: String,
}

impl Entrada {
    /// Entrada nueva con la marca de tiempo actual.
    pub fn nueva(palabra: &str, ts: u64, origen: &str) -> Entrada {
        Entrada {
            palabra: palabra.trim().to_string(),
            atajos: Vec::new(),
            ts,
            origen: origen.to_string(),
        }
    }
}

/// Epoch actual en segundos.
pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Parseo del content provider ──────────────────────────────────────────

/// Extrae los campos `clave=valor` de una fila de `content query`.
///
/// Robusto frente a cuerpos con comas y saltos de línea: sólo corta en las
/// claves conocidas, nunca en una coma del texto.
fn extract_fields(row: &str, keys: &[&str]) -> BTreeMap<String, String> {
    let mut posiciones: Vec<(usize, &str)> = Vec::new();
    for key in keys {
        let pat = format!("{key}=");
        let mut from = 0usize;
        while let Some(p) = row[from..].find(pat.as_str()) {
            let abs = from + p;
            let previo_ok = abs == 0
                || !matches!(
                    row.as_bytes()[abs - 1],
                    b'_' | b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'
                );
            if previo_ok {
                posiciones.push((abs, key));
            }
            from = abs + pat.len();
        }
    }
    posiciones.sort_by_key(|p| p.0);
    let mut out = BTreeMap::new();
    for (i, (abs, key)) in posiciones.iter().enumerate() {
        let fin = posiciones
            .get(i + 1)
            .map(|(p, _)| *p)
            .unwrap_or(row.len());
        // `content query` separa con ", " => el valor arrastra la coma.
        let mut valor = row[abs + key.len() + 1..fin].trim().to_string();
        while valor.ends_with(',') {
            valor.pop();
            valor = valor.trim_end().to_string();
        }
        out.insert((*key).to_string(), valor);
    }
    out
}

/// Parsea la salida de `content query --uri content://user_dictionary/words`.
///
/// Acepta también un volcado JSON (`[{"word":..,"shortcut":..}]`), que es
/// lo que exportan otras herramientas.  Es **tolerante**: una fila que no
/// se entiende se salta, no rompe el resto.
pub fn parse_user_dictionary(stdout: &str) -> Vec<Entrada> {
    let texto = stdout.trim();
    if texto.is_empty() {
        return Vec::new();
    }
    // Formato JSON primero.
    if texto.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(texto) {
            if let Some(arr) = v.as_array() {
                return arr
                    .iter()
                    .filter_map(|e| {
                        let palabra = e.get("word").and_then(|x| x.as_str())?.trim().to_string();
                        if palabra.is_empty() {
                            return None;
                        }
                        let atajos = e
                            .get("shortcut")
                            .and_then(|x| x.as_str())
                            .map(|s| {
                                s.split(',')
                                    .map(|x| x.trim().to_string())
                                    .filter(|x| !x.is_empty())
                                    .collect::<Vec<String>>()
                            })
                            .unwrap_or_default();
                        let ts = e
                            .get("timestamp")
                            .and_then(|x| x.as_u64())
                            .unwrap_or(0);
                        Some(Entrada {
                            palabra,
                            atajos,
                            ts,
                            origen: "teclado".to_string(),
                        })
                    })
                    .collect();
            }
        }
    }
    // Formato `content query`: filas separadas por líneas, campos clave=valor.
    let claves = ["word", "shortcut", "timestamp"];
    let mut out = Vec::new();
    for fila in texto.lines() {
        let fila = fila.trim();
        if fila.is_empty() || !fila.contains("word=") {
            continue;
        }
        let f = extract_fields(fila, &claves);
        let palabra = match f.get("word") {
            Some(w) if !w.trim().is_empty() => w.trim().to_string(),
            _ => continue,
        };
        let atajos = f
            .get("shortcut")
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();
        let ts = f
            .get("timestamp")
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);
        out.push(Entrada {
            palabra,
            atajos,
            ts,
            origen: "teclado".to_string(),
        });
    }
    out
}

// ── Sincronización ───────────────────────────────────────────────────────

/// Fusiona dos conjuntos de entradas de forma **idempotente**.
///
/// Clave: la palabra (normalizada).  Gana la entrada con el `ts` mayor; si
/// empatan, gana la entrante (para que la sincronización propague).  Los
/// atajos se unen (conjunto ordenado, sin duplicados).
pub fn merge(existentes: &[Entrada], entrantes: &[Entrada]) -> Vec<Entrada> {
    let mut mapa: BTreeMap<String, Entrada> = BTreeMap::new();
    for e in existentes.iter().chain(entrantes.iter()) {
        let clave = normalizar(&e.palabra);
        if clave.is_empty() {
            continue;
        }
        match mapa.get_mut(&clave) {
            Some(prev) => {
                let atajos: BTreeSet<String> = prev.atajos.iter().chain(e.atajos.iter()).cloned().collect();
                prev.atajos = atajos.into_iter().collect();
                if e.ts >= prev.ts {
                    let origen = e.origen.clone();
                    let atajos = prev.atajos.clone();
                    *prev = e.clone();
                    prev.atajos = atajos;
                    prev.origen = origen;
                }
            }
            None => {
                mapa.insert(clave, e.clone());
            }
        }
    }
    mapa.into_values().collect()
}

/// Formato de serialización de una entrada (NDJSON, una por línea).
fn entry_to_json(e: &Entrada) -> serde_json::Value {
    serde_json::json!({
        "palabra": e.palabra,
        "atajos": e.atajos,
        "ts": e.ts,
        "origen": e.origen,
    })
}

/// Serializa las entradas a NDJSON (append-only, una entrada por línea).
pub fn to_ndjson(entradas: &[Entrada]) -> String {
    let mut s = String::new();
    for e in entradas {
        s.push_str(&entry_to_json(e).to_string());
        s.push('\n');
    }
    s
}

/// Lee NDJSON de entradas (tolerante: las líneas corruptas se saltan).
pub fn from_ndjson(ndjson: &str) -> Vec<Entrada> {
    let mut out = Vec::new();
    for linea in ndjson.lines() {
        let l = linea.trim();
        if l.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(l) {
            if let Some(palabra) = v.get("palabra").and_then(|x| x.as_str()) {
                let atajos = v
                    .get("atajos")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s.as_str().map(|x| x.to_string()))
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                out.push(Entrada {
                    palabra: palabra.to_string(),
                    atajos,
                    ts: v.get("ts").and_then(|x| x.as_u64()).unwrap_or(0),
                    origen: v
                        .get("origen")
                        .and_then(|x| x.as_str())
                        .unwrap_or("desconocido")
                        .to_string(),
                });
            }
        }
    }
    out
}

/// Manifiesto del paquete de diccionarios.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifiesto {
    /// Número de entradas.
    pub entradas: usize,
    /// SHA-256 del NDJSON (sobre los bytes exactos, con el salto final).
    pub sha256: String,
    /// Tamaño del NDJSON en bytes.
    pub bytes: usize,
}

/// Calcula el manifiesto del NDJSON.
pub fn manifiesto(ndjson: &str) -> Manifiesto {
    Manifiesto {
        entradas: ndjson.lines().filter(|l| !l.trim().is_empty()).count(),
        sha256: sha256_hex(ndjson.as_bytes()),
        bytes: ndjson.len(),
    }
}

/// Verifica que el NDJSON corresponde al manifiesto.
pub fn verificar(ndjson: &str, m: &Manifiesto) -> Result<(), String> {
    let real = manifiesto(ndjson);
    if real.sha256 != m.sha256 {
        return Err(format!(
            "el sha256 no cuadra: manifiesto {} real {}",
            m.sha256, real.sha256
        ));
    }
    if real.entradas != m.entradas {
        return Err(format!(
            "el numero de entradas no cuadra: manifiesto {} real {}",
            m.entradas, real.entradas
        ));
    }
    Ok(())
}

/// SHA-256 (hex).  El repo ya tiene `sha2`; se usa para no duplicar.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

// ── Normalización y confusables ──────────────────────────────────────────

/// Normaliza una palabra para compararla: minúsculas, sin acentos, sin
/// signos de puntuación y sin espacios sobrantes.
///
/// Se usa para la clave de comparación, NO para escribir: el usuario quiere
/// que "HABÍA" y "habia" sean la MISMA palabra.
pub fn normalizar(palabra: &str) -> String {
    let base: String = palabra
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-')
        .collect();
    let sin_acentos: String = base
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            _ => c,
        })
        .collect();
    sin_acentos
}

/// Normaliza **confusables** (caracteres que se parecen): 0↔o, 1↔l/i,
/// 5↔s, rn↔m, vv↔w.  Sirve para sugerir correcciones cuando el error es
/// de "tecla equivocada" y no de "letra cambiada".
pub fn confusables(palabra: &str) -> String {
    let n = normalizar(palabra);
    let mut s = String::with_capacity(n.len());
    let chars: Vec<char> = n.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == 'r' && i + 1 < chars.len() && chars[i + 1] == 'n' {
            s.push('m');
            i += 2;
            continue;
        }
        if c == 'v' && i + 1 < chars.len() && chars[i + 1] == 'v' {
            s.push('w');
            i += 2;
            continue;
        }
        s.push(match c {
            '0' => 'o',
            '1' => 'l',
            '5' => 's',
            '8' => 'b',
            '2' => 'z',
            _ => c,
        });
        i += 1;
    }
    s
}

/// Distancia de Levenshtein con recorrido en anchura (2 filas).
pub fn levenshtein(a: &str, b: &str) -> usize {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    if av.is_empty() {
        return bv.len();
    }
    if bv.is_empty() {
        return av.len();
    }
    let mut prev: Vec<usize> = (0..=bv.len()).collect();
    let mut cur = vec![0usize; bv.len() + 1];
    for (i, ca) in av.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in bv.iter().enumerate() {
            let coste = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + coste);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[bv.len()]
}

/// Distancia de Damerau-Levenshtein (transposiciones).  Sirve para el caso
/// típico "qeu" en vez de "que".
pub fn damerau(a: &str, b: &str) -> usize {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    if av.is_empty() {
        return bv.len();
    }
    if bv.is_empty() {
        return av.len();
    }
    let n = av.len();
    let m = bv.len();
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=m {
        d[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let coste = if av[i - 1] == bv[j - 1] { 0 } else { 1 };
            let mut v = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + coste);
            if i > 1 && j > 1 && av[i - 1] == bv[j - 2] && av[i - 2] == bv[j - 1] {
                v = v.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = v;
        }
    }
    d[n][m]
}

// ── Corrección de erratas ────────────────────────────────────────────────

/// Una corrección propuesta.
#[derive(Debug, Clone, PartialEq)]
pub struct Sugerencia {
    /// La palabra tal como está escrita.
    pub original: String,
    /// La palabra corregida.
    pub corregida: String,
    /// Confianza 0..1.
    pub confianza: f32,
    /// Por qué se propone (para que sea auditable, no una caja negra).
    pub motivo: String,
}

/// ¿Son las dos palabras un intercambio de DOS CARACTERES ADYACENTES?
///
/// "qeu" vs "que" => sí.  "casa" vs "caso" => no (es una sustitución).
pub fn es_transposicion(a: &str, b: &str) -> bool {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    if av.len() != bv.len() || av.len() < 2 {
        return false;
    }
    if av == bv {
        return false;
    }
    // Primer índice donde difieren: el intercambio debe estar ahí.
    let mut k = 0usize;
    while k < av.len() && av[k] == bv[k] {
        k += 1;
    }
    if k == av.len() {
        return false; // son iguales
    }
    if k + 1 >= av.len() {
        return false; // no hay par que intercambiar
    }
    av[k] == bv[k + 1] && av[k + 1] == bv[k] && av[k + 2..] == bv[k + 2..]
}

/// Confianza de una corrección según la distancia y el tipo de error.
fn confianza_para(original: &str, candidata: &str) -> Option<(f32, String)> {
    let o = normalizar(original);
    let c = normalizar(candidata);
    if o.is_empty() || c.is_empty() || o == c {
        return None;
    }
    let lo = o.chars().count();
    let lc = c.chars().count();

    // 1) Transposición REAL: dos caracteres adyacentes intercambiados
    //    ("qeu" -> "que").  Muy alta confianza.
    //    OJO: Damerau NO sirve aquí porque incluye la sustitución, y
    //    "casa"/"caso" saldría como transposición (¡y son dos palabras
    //    reales!).  Hay que comprobar el intercambio adyacente explícito.
    if lo >= 3 && lc >= 3 && lo == lc && es_transposicion(&o, &c) {
        return Some((0.92, "transposicion".to_string()));
    }
    // 2) Confusables: "rn"->"m", "0"->"o".  Alta.
    if confusables(&o) == c {
        return Some((0.88, "caracteres_confusables".to_string()));
    }
    // 3) Caracteres repetidos: "aaa" -> "a".
    if o.chars().collect::<Vec<char>>().windows(3).any(|w| w[0] == w[1] && w[1] == w[2])
        && lo > 3
        && levenshtein(&o, &c) == 1
    {
        return Some((0.75, "caracteres_repetidos".to_string()));
    }
    // 4) Distancia 1 (una letra de más, de menos o cambiada).
    //    La confianza BAJA con la longitud: "casa"->"caso" son dos palabras
    //    reales y autocorregirlo sería un error; "administracion"->
    //    "administración" no tiene competitors.
    let d = levenshtein(&o, &c);
    if d == 1 {
        let conf = match lo {
            0..=3 => 0.40,
            4 => 0.55,
            5 => 0.75,
            _ => 0.95,
        };
        return Some((conf, "un_caracter".to_string()));
    }
    // 5) Distancia 2 sólo con palabras largas (si no, hay falsos positivos).
    if d == 2 && lo >= 6 && lc >= 6 {
        return Some((0.78, "dos_caracteres".to_string()));
    }
    None
}

/// Propone correcciones para las palabras de un texto.
///
/// `diccionario` es la lista de palabras que el usuario ha aprendido
/// (normalizadas internamente).  Devuelve las sugerencias **ordenadas por
/// confianza descendente** y sin duplicados.
pub fn sugerir(texto: &str, diccionario: &[String]) -> Vec<Sugerencia> {
    if diccionario.is_empty() {
        return Vec::new();
    }
    let dict_norm: Vec<String> = diccionario
        .iter()
        .map(|w| normalizar(w))
        .filter(|w| !w.is_empty())
        .collect();
    let mut out: Vec<Sugerencia> = Vec::new();
    for palabra in tokenizar(texto) {
        let n = normalizar(&palabra);
        if n.is_empty() || n.chars().count() < 3 {
            continue;
        }
        if dict_norm.iter().any(|d| *d == n) {
            continue; // ya está en el diccionario: no se toca
        }
        let mut mejor: Option<Sugerencia> = None;
        for d in &dict_norm {
            if d.chars().count() < 3 {
                continue;
            }
            if let Some((conf, motivo)) = confianza_para(&palabra, d) {
                let s = Sugerencia {
                    original: palabra.clone(),
                    corregida: d.clone(),
                    confianza: conf,
                    motivo,
                };
                let mejor_es_mayor = mejor
                    .as_ref()
                    .map(|b| s.confianza > b.confianza)
                    .unwrap_or(true);
                if mejor_es_mayor {
                    mejor = Some(s);
                }
            }
        }
        if let Some(s) = mejor {
            if !out.iter().any(|x| x.original == s.original) {
                out.push(s);
            }
        }
    }
    out.sort_by(|a, b| b.confianza.partial_cmp(&a.confianza).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Divide el texto en palabras conservando la puntuación como separadores.
pub fn tokenizar(texto: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut actual = String::new();
    for c in texto.chars() {
        if c.is_alphanumeric() || c == '\'' || c == '-' {
            actual.push(c);
        } else if !actual.is_empty() {
            out.push(std::mem::take(&mut actual));
        }
    }
    if !actual.is_empty() {
        out.push(actual);
    }
    out
}

/// Aplica las sugerencias por encima de `umbral`.
///
/// Devuelve el texto corregido y las sugerencias **aplicadas** (para poder
/// deshacer o revisar).
pub fn aplicar(texto: &str, sugerencias: &[Sugerencia], umbral: f32) -> (String, Vec<Sugerencia>) {
    let aplicadas: Vec<Sugerencia> = sugerencias
        .iter()
        .filter(|s| s.confianza >= umbral)
        .cloned()
        .collect();
    if aplicadas.is_empty() {
        return (texto.to_string(), Vec::new());
    }
    let mut fuera = texto.to_string();
    for s in &aplicadas {
        fuera = reemplazar_palabra(&fuera, &s.original, &s.corregida);
    }
    (fuera, aplicadas)
}

/// Sustituye una palabra **completa** (por límites de palabra), sin regex.
///
/// "qeu" NO debe cambiar dentro de "alqueusar": los límites son
/// alfanuméricos, igual que en el tokenizador.
pub fn reemplazar_palabra(texto: &str, original: &str, corregida: &str) -> String {
    if original.is_empty() {
        return texto.to_string();
    }
    let es_palabra = |c: char| c.is_alphanumeric() || c == '\'' || c == '-';
    let objetivo: Vec<char> = original.chars().collect();
    let fuente: Vec<char> = texto.chars().collect();
    let mut out = String::with_capacity(texto.len());
    let mut i = 0usize;
    while i < fuente.len() {
        let cabe = i + objetivo.len() <= fuente.len()
            && fuente[i..i + objetivo.len()] == objetivo[..];
        let antes_ok = i == 0 || !es_palabra(fuente[i - 1]);
        let despues_ok = i + objetivo.len() >= fuente.len()
            || !es_palabra(fuente[i + objetivo.len()]);
        if cabe && antes_ok && despues_ok {
            out.push_str(corregida);
            i += objetivo.len();
        } else {
            out.push(fuente[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_el_volcado_del_content_provider() {
        let salida = "Row: 0 word=habla, shortcut=habia, timestamp=1712345678\nRow: 1 word=megadaemon, shortcut=, timestamp=1712345680\n";
        let e = parse_user_dictionary(salida);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].palabra, "habla");
        assert_eq!(e[0].atajos, vec!["habia".to_string()]);
        assert_eq!(e[0].ts, 1712345678);
        assert_eq!(e[1].palabra, "megadaemon");
        assert!(e[1].atajos.is_empty());
    }

    #[test]
    fn parsea_el_volcado_json() {
        let salida = r#"[{"word":"habla","shortcut":"habia","timestamp":1712345678}]"#;
        let e = parse_user_dictionary(salida);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].palabra, "habla");
        assert_eq!(e[0].atajos, vec!["habia".to_string()]);
    }

    #[test]
    fn parseo_tolerante_ante_ruido() {
        let salida = "garbage\nRow: 0 word=ok, shortcut=, timestamp=1\n";
        let e = parse_user_dictionary(salida);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].palabra, "ok");
    }

    #[test]
    fn el_merge_es_idempotente() {
        let a = vec![Entrada::nueva("habla", 10, "teclado")];
        let b = vec![Entrada::nueva("habla", 10, "sincronizado:pc")];
        let m1 = merge(&a, &b);
        let m2 = merge(&m1, &b);
        assert_eq!(m1.len(), 1);
        assert_eq!(m2.len(), 1);
        assert_eq!(m1, m2);
    }

    #[test]
    fn el_merge_gana_el_ts_mayor() {
        let a = vec![Entrada::nueva("habla", 10, "viejo")];
        let b = vec![Entrada::nueva("habla", 20, "nuevo")];
        let m = merge(&a, &b);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].ts, 20);
    }

    #[test]
    fn ndjson_y_manifiesto_cuadran() {
        let e = vec![
            Entrada::nueva("habla", 1, "teclado"),
            Entrada::nueva("megadaemon", 2, "teclado"),
        ];
        let nd = to_ndjson(&e);
        let m = manifiesto(&nd);
        assert_eq!(m.entradas, 2);
        assert_eq!(m.sha256.len(), 64);
        assert!(verificar(&nd, &m).is_ok());
        // y detecta la manipulacion
        let manipulado = nd.replace("habla", "habIz");
        assert!(verificar(&manipulado, &m).is_err());
    }

    #[test]
    fn el_round_trip_ndjson_conserva_los_datos() {
        let mut e = Entrada::nueva("habla", 7, "teclado");
        e.atajos.push("habia".to_string());
        let nd = to_ndjson(std::slice::from_ref(&e));
        let leida = from_ndjson(&nd);
        assert_eq!(leida.len(), 1);
        assert_eq!(leida[0].palabra, e.palabra);
        assert_eq!(leida[0].atajos, e.atajos);
        assert_eq!(leida[0].ts, e.ts);
        assert_eq!(leida[0].origen, e.origen);
    }

    #[test]
    fn normalizacion_quita_acentos_y_mayusculas() {
        assert_eq!(normalizar("HABÍA"), "habia");
        assert_eq!(normalizar("  Móvil  "), "movil");
        assert_eq!(normalizar("ñandú"), "nandu");
    }

    #[test]
    fn los_confusables_se_reconocen() {
        assert_eq!(confusables("rn"), "m");
        assert_eq!(confusables("0"), "o");
        assert_eq!(confusables("l1"), "ll");
    }

    #[test]
    fn la_transposicion_es_un_intercambio_real() {
        assert!(es_transposicion("qeu", "que"));
        assert!(es_transposicion("hte", "the"));
        assert!(!es_transposicion("casa", "caso"), "una sustitución no es transposición");
        assert!(!es_transposicion("abc", "abc"));
        // "ab"/"ba" SÍ es un intercambio, pero con 2 letras el modelo de
        // confianza no propone nada (exige 3+): "ab" y "ba" son palabras.
        assert!(es_transposicion("ab", "ba"));
        assert!(sugerir("ba", &["ab".to_string()]).is_empty());
    }

    #[test]
    fn detecta_una_transposicion() {
        let s = sugerir("qeu", &["que".to_string()]);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].corregida, "que");
        assert!(s[0].confianza > 0.9, "confianza baja: {:?}", s[0]);
        assert_eq!(s[0].motivo, "transposicion");
    }

    #[test]
    fn detecta_una_letra_de_mas() {
        let s = sugerir("holaa", &["hola".to_string()]);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].corregida, "hola");
    }

    #[test]
    fn no_propone_si_la_palabra_ya_esta() {
        let s = sugerir("hola", &["hola".to_string()]);
        assert!(s.is_empty());
    }

    #[test]
    fn no_propone_para_palabras_muy_cortas() {
        let s = sugerir("ab", &["abc".to_string()]);
        assert!(s.is_empty());
    }

    #[test]
    fn la_confianza_depende_de_la_longitud() {
        let s = sugerir("casa", &["caso".to_string()]);
        assert_eq!(s.len(), 1);
        // "casa"/"caso": distancia 1 pero palabra de 4 letras => confianza
        // media: se SUGIERE pero NO se autocorrige (umbral 0,9).
        assert!(s[0].confianza < 0.8, "no debería ser alta: {:?}", s[0]);
        assert!(s[0].confianza > 0.4, "pero debe ser una sugerencia: {:?}", s[0]);
    }

    #[test]
    fn detecta_confusables() {
        let s = sugerir("rnate", &["mate".to_string()]);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].corregida, "mate");
    }

    #[test]
    fn el_tokenizador_respeta_la_puntuacion() {
        let t = tokenizar("Hola, mundo! qeu?");
        assert_eq!(t, vec!["Hola", "mundo", "qeu"]);
    }

    #[test]
    fn la_aplicacion_sustituye_palabras_completas() {
        let s = sugerir("qeu", &["que".to_string()]);
        let (txt, ap) = aplicar("qeu y qeu", &s, 0.9);
        assert_eq!(txt, "que y que");
        assert_eq!(ap.len(), 1);
    }

    #[test]
    fn no_aplica_por_debajo_del_umbral() {
        let s = vec![Sugerencia {
            original: "qeu".into(),
            corregida: "que".into(),
            confianza: 0.5,
            motivo: "prueba".into(),
        }];
        let (txt, ap) = aplicar("qeu", &s, 0.9);
        assert_eq!(txt, "qeu");
        assert!(ap.is_empty());
    }

    #[test]
    fn la_distancia_levenshtein_es_correcta() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }
}

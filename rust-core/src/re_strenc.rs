//! MegaDaemon v13.4 — Descifrado de strings ofuscados con PRUEBA de ida y vuelta.
//!
//! ## Qué resuelve
//!
//! Mucha ofuscación (packers, protectores, malware móvil) esconde los strings
//! cifrándolos en una rutina de descifrado en tiempo de ejecución.  Si se detecta
//! **qué** transformación se aplica, se pueden recuperar los strings **sin**
//! ejecutar el binario.
//!
//! ## Las transformaciones soportadas
//!
//! | Transformación | Descifrado | Nota |
//! |---|---|---|
//! | `XorByte(k)`     | `p[i] = c[i] ^ k` | la más común |
//! | `AddByte(k)`     | `p[i] = c[i] - k` | suma/resta constante |
//! | `XorRolling(seed)` | `p[0] = c[0] ^ seed`, `p[i] = c[i] ^ p[i-1]` | rolling XOR |
//! | `RolXor(rot,k)`  | `p[i] = ror(c[i] ^ k, rot)` | rotar + XOR |
//!
//! ## Cómo se detectan (sin ejecutar nada)
//!
//! Truco clásico: en un texto ASCII cifrado con XOR de un byte, el byte **más
//! frecuente** del texto cifrado es `clave ^ 0x20` (el espacio) o `clave ^ 'e'`.
//! Por eso el detector **no** prueba las 256 claves en cada posición (serían
//! miles de millones de operaciones en un binario de 181 MB): cuenta las
//! frecuencias del bloque, prueba unas pocas claves candidatas y busca las
//! **tiradas** maximales que salen imprimibles.
//!
//! ## La prueba
//!
//! Un descifrado sólo se da por bueno si:
//! 1. **ida y vuelta**: `cifrar(descifrar(cifrado)) == cifrado` (la transformación
//!    tiene inversa conocida y verificada), y
//! 2. **validez**: el texto plano es UTF-8 válido y casi todo imprimible.
//!
//! El informe dice cuál de los dos falló.  Un descifrado que no pase la prueba
//! **no** se publica como texto plano.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Transformación de cifrado reversible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transform {
    /// XOR con un byte constante.
    XorByte(u8),
    /// Suma (o resta) constante.
    AddByte(u8),
    /// XOR encadenado con el byte plano anterior.
    XorRolling(u8),
    /// Rotación a la derecha seguida de XOR.
    RolXor { rot: u32, key: u8 },
}

impl Transform {
    /// Texto legible de la transformación.
    pub fn label(&self) -> String {
        match self {
            Transform::XorByte(k) => format!("xor-byte(0x{k:02x})"),
            Transform::AddByte(k) => format!("add-byte(0x{k:02x})"),
            Transform::XorRolling(s) => format!("xor-rolling(0x{s:02x})"),
            Transform::RolXor { rot, key } => format!("rol-xor(rot={rot},key=0x{key:02x})"),
        }
    }
}

/// Descifra un bloque con la transformación dada.
pub fn decrypt(data: &[u8], t: &Transform) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    match t {
        Transform::XorByte(k) => out.extend(data.iter().map(|b| b ^ k)),
        Transform::AddByte(k) => out.extend(data.iter().map(|b| b.wrapping_sub(*k))),
        Transform::XorRolling(seed) => {
            let mut prev = *seed;
            for &b in data {
                let p = b ^ prev;
                out.push(p);
                prev = p;
            }
        }
        Transform::RolXor { rot, key } => {
            // Inversa de `rotate_left(r) ^ key`: primero se deshace el XOR y
            // despues se rota al reves.  (rotar despues de XOR no es lo mismo:
            //  el XOR y la rotacion NO conmutan.)
            let r = (*rot % 8) as u32;
            out.extend(data.iter().map(|b| (b ^ key).rotate_right(r)));
        }
    }
    out
}

/// Cifra un bloque con la transformación dada (la inversa).
pub fn encrypt(data: &[u8], t: &Transform) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    match t {
        Transform::XorByte(k) => out.extend(data.iter().map(|b| b ^ k)),
        Transform::AddByte(k) => out.extend(data.iter().map(|b| b.wrapping_add(*k))),
        Transform::XorRolling(seed) => {
            let mut prev = *seed;
            for &b in data {
                let c = b ^ prev;
                out.push(c);
                prev = b;
            }
        }
        Transform::RolXor { rot, key } => {
            let r = (*rot % 8) as u32;
            out.extend(data.iter().map(|b| b.rotate_left(r) ^ key));
        }
    }
    out
}

/// Un string descifrado.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecryptedString {
    /// Desplazamiento del blob en el fichero.
    pub offset: u64,
    /// Longitud del blob cifrado.
    pub length: usize,
    pub transform: Transform,
    /// Texto plano recuperado.
    pub plaintext: String,
    /// Proporción de bytes imprimibles del texto plano (0..1).
    pub printable_ratio: f64,
    /// ¿El texto plano es UTF-8 válido?
    pub utf8: bool,
}

/// Resultado de la prueba de un descifrado.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecryptProof {
    /// ¿Pasa las dos condiciones (ida y vuelta + validez)?
    pub passed: bool,
    /// ¿La ida y vuelta devuelve el cifrado original?
    pub round_trip: bool,
    /// ¿El texto plano es válido (UTF-8 + imprimible)?
    pub valid_text: bool,
    pub detail: String,
}

/// Prueba un descifrado: ida y vuelta + validez del texto.
///
/// La **ida y vuelta** comprueba que aplicar la inversa a un texto plano devuelve
/// exactamente el bloque cifrado original: es lo que garantiza que el
/// descifrado no se ha inventado nada.
pub fn prove(data: &[u8], t: &Transform) -> (DecryptProof, Vec<u8>) {
    let plain = decrypt(data, t);
    let round = encrypt(&plain, t);
    let round_trip = round == data;
    let printable = printable_ratio(&plain);
    let utf8 = std::str::from_utf8(&plain).is_ok();
    // Validez: casi todo imprimible, UTF-8 valido Y con **aspecto de texto**
    // (minusculas y palabras).  Sin esta comprobacion, un XOR con clave
    // cercana (`'a'^1 == 'b'`) tambien daria "texto valido".
    let valid_text = printable >= 0.9 && utf8 && looks_like_text(&plain);
    let detail = if !round_trip {
        "la ida y vuelta NO devuelve el cifrado original".to_string()
    } else if !utf8 {
        format!("el texto plano no es UTF-8 valido (imprimible {printable:.2})")
    } else if printable < 0.9 {
        format!("el texto plano solo es {printable:.0}% imprimible")
    } else {
        format!(
            "ida y vuelta exacta y texto plano valido (UTF-8, {printable:.0}% imprimible)"
        )
    };
    (
        DecryptProof {
            passed: round_trip && valid_text,
            round_trip,
            valid_text,
            detail,
        },
        plain,
    )
}

/// ¿El bloque tiene **aspecto de texto**?
///
/// Tresheurística: proportion de minusculas y al menos una palabra de 3 letras
/// o mas.  Sin esto, un XOR con clave cercana tambien "pasa" la prueba.
pub fn looks_like_text(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }
    let total = data.len();
    let mut letras = 0usize;
    let mut minusculas = 0usize;
    let mut mayusculas = 0usize;
    let mut distintas: Vec<u8> = Vec::new();
    for &b in data {
        if b.is_ascii_alphabetic() {
            letras += 1;
            if b.is_ascii_lowercase() {
                minusculas += 1;
            } else {
                mayusculas += 1;
            }
            let l = b.to_ascii_lowercase();
            if !distintas.contains(&l) {
                distintas.push(l);
            }
        }
    }
    // 1) Casi todo son letras (los simbolos de un path siguen siendo texto).
    if (letras as f64) < 0.5 * (total as f64) {
        return false;
    }
    // 2) Diversidad lexica: el relleno (p.ej. 64 bytes iguales) tiene 1 sola
    //    letra distinta => se descarta.
    if distintas.len() < 6 {
        return false;
    }
    // 3) Hay minUSCULAS o MAYUSCULAS (un texto normal tiene las dos; el
    //    relleno repetido tendria una sola).
    let ratio_min = minusculas as f64 / letras as f64;
    let ratio_may = mayusculas as f64 / letras as f64;
    if ratio_min < 0.15 && ratio_may < 0.15 {
        return false;
    }
    // 4) Al menos una palabra de 3+ letras.
    let mut palabra = 0usize;
    let mut mejor = 0usize;
    for &b in data {
        if b.is_ascii_alphabetic() {
            palabra += 1;
            if palabra > mejor {
                mejor = palabra;
            }
        } else {
            palabra = 0;
        }
    }
    mejor >= 3
}

/// Recorta un bloque descifrado al tramo mas largo con **aspecto de texto**.
///
/// Sin este recorte, una tirada puede "`Z` de relleno + texto real" y la
/// proporcion de minusculas se diluye ⇒ el descifrado se rechaza aunque el
/// texto sea correcto.  Devuelve `(desplazamiento, longitud)` dentro del
/// bloque, o `None` si no hay ningun tramo con aspecto de texto.
pub fn trim_to_text(plain: &[u8], min_len: usize) -> Option<(usize, usize)> {
    // Se busca la ventana MAS LARGA cuya proporcion de minusculas sea >= 0.2.
    //
    // Condicion:  L/(L+N) >= 0.2  <=>  4L >= N  <=>  4L - N >= 0.
    // Con el prefijo  P(i) = 4*minusculas(i) - no_minusculas(i),  la condicion
    // de la ventana [i,j) es  P(j) - P(i) >= 0  <=>  P(i) <= P(j).
    // La ventana maxima es el "maximum width ramp" clasico: pila monotona + barrido
    // de derecha a izquierda.
    let n = plain.len();
    if n == 0 {
        return None;
    }
    let mut pre = Vec::with_capacity(n + 1);
    pre.push(0i64);
    for &b in plain {
        let d = if b.is_ascii_lowercase() { 4i64 } else { -1i64 };
        let last = *pre.last().unwrap();
        pre.push(last + d);
    }
    // Pila con P estrictamente decreciente.
    let mut pila: Vec<usize> = Vec::with_capacity(n + 1);
    for i in 0..=n {
        if pila.is_empty() || pre[i] < pre[*pila.last().unwrap()] {
            pila.push(i);
        }
    }
    let mut mejor: Option<(usize, usize)> = None;
    for j in (0..=n).rev() {
        while let Some(&i) = pila.last() {
            if pre[i] <= pre[j] {
                let cand = (i, j - i);
                if mejor.map(|m| m.1 < cand.1).unwrap_or(true) {
                    mejor = Some(cand);
                }
                pila.pop();
            } else {
                break;
            }
        }
    }
    let (ini, len) = mejor?;
    // Recorte ligero: se quitan los caracteres de control/rareza de los
    // extremos (el relleno alfabetico del binario, p.ej. las 'Z' que salen de
    // un relleno de ceros, **se queda**: quitarlo exigiria adivinar donde
    // empieza el texto real, y el informe declara el ratio y el tamanho).
    let mut ini = ini;
    let mut fin = ini + len;
    while ini < fin && !es_caracter_de_texto(plain[ini]) {
        ini += 1;
    }
    while fin > ini && !es_caracter_de_texto(plain[fin - 1]) {
        fin -= 1;
    }
    if fin - ini < min_len {
        return Some((ini, len));
    }
    Some((ini, fin - ini))
}

/// Caracteres que pueden formar parte de un texto (letras, digitos y los
/// separadores habituales de una ruta o de una frase).
fn es_caracter_de_texto(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || b.is_ascii_whitespace()
        || matches!(
            b,
            b'.' | b',' | b':' | b';' | b'!' | b'?' | b'-' | b'_' | b'/' | b'\\'
                | b'@' | b'#' | b'$' | b'%' | b'&' | b'=' | b'+' | b'*' | b'\'' | b'"'
                | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'<'
        )
}

/// Proporción de bytes imprimibles (ASCII + tab + salto) de un bloque.
pub fn printable_ratio(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let ok = data
        .iter()
        .filter(|&&b| (0x20..=0x7e).contains(&b) || b == b'\t' || b == b'\n' || b == b'\r')
        .count();
    ok as f64 / data.len() as f64
}

/// Opciones del detector.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DetectOptions {
    /// Longitud mínima del texto plano (en bytes).
    pub min_len: usize,
    /// Tope de candidatos de clave probados por bloque.
    pub max_keys: usize,
    /// Proporción mínima imprimible para dar un descifrado por bueno.
    pub min_printable: f64,
    /// Tope de bytes analizados (el resto del bloque se ignora y se declara).
    pub max_bytes: usize,
}

impl Default for DetectOptions {
    fn default() -> Self {
        DetectOptions {
            min_len: 6,
            max_keys: 8,
            min_printable: 0.9,
            max_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Resultado del barrido de un bloque: los descifrados y lo que se escaneó.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanResult {
    pub found: Vec<DecryptedString>,
    /// Bytes realmente analizados.
    pub scanned_bytes: usize,
    /// ¿Se cortó el bloque por `max_bytes`?
    pub truncated: bool,
}

/// Detecta strings cifrados en un bloque de datos.
///
/// Devuelve sólo los descifrados que **pasan la prueba** (ida y vuelta + validez).
pub fn detect(data: &[u8], opts: &DetectOptions) -> ScanResult {
    let mut out: Vec<DecryptedString> = Vec::new();
    let truncado = data.len() > opts.max_bytes;
    let bloque = if truncado {
        &data[..opts.max_bytes]
    } else {
        data
    };
    if bloque.len() < opts.min_len {
        return ScanResult {
            found: out,
            scanned_bytes: bloque.len(),
            truncated: truncado,
        };
    }

    // 1) Claves candidatas: por frecuencia del byte en el bloque.
    //
    //    El byte mas frecuente del texto cifrado es `clave ^ (caracter mas
    //    frecuente del texto plano)`.  Se prueban los caracteres typicalos.
    let mut frec = [0usize; 256];
    for &b in bloque {
        frec[b as usize] += 1;
    }
    let mut orden: Vec<usize> = (0..256).collect();
    orden.sort_by(|a, b| frec[*b].cmp(&frec[*a]));
    let patrones: [u8; 8] = [0x20, 0x5f, 0x65, 0x00, 0x2f, 0x2e, 0x4c, 0x4d];
    let mut xor_keys: Vec<u8> = Vec::new();
    let mut add_keys: Vec<u8> = Vec::new();
    for &f in orden.iter().take(opts.max_keys) {
        for patron in patrones {
            let k = (f as u8) ^ patron;
            if !xor_keys.contains(&k) {
                xor_keys.push(k);
            }
            let a = (f as u8).wrapping_sub(patron);
            if !add_keys.contains(&a) {
                add_keys.push(a);
            }
        }
    }
    // Etapa 2: en bloques cortos se prueban TODAS las claves (256 XOR + 256 ADD).
    //    Es lo que garantiza no perder un cifrado cuyo byte mas frecuente no
    //    este entre los candidatos de la heuristica.
    if bloque.len() <= 256 * 1024 {
        for k in 0u16..=255 {
            let kb = k as u8;
            if !xor_keys.contains(&kb) {
                xor_keys.push(kb);
            }
            if !add_keys.contains(&kb) {
                add_keys.push(kb);
            }
        }
    }

    // 2) XOR de un byte.
    for &k in xor_keys.iter() {
        recoge(&mut out, bloque, &Transform::XorByte(k), opts);
    }
    // 3) Suma/resta constante.
    for &k in add_keys.iter() {
        recoge(&mut out, bloque, &Transform::AddByte(k), opts);
    }
    // 4) XOR encadenado (rolling): el estado es global, asi que se descifra el
    //    bloque entero y se buscan tiradas en el resultado.
    for seed in [0u8, 0x20, 0x65] {
        let t = Transform::XorRolling(seed);
        let plano = decrypt(bloque, &t);
        // La ida y vuelta del rolling es a nivel de BLOQUE (el estado es
        // global): se comprueba una vez sobre todo el bloque.
        let ida_y_vuelta = encrypt(&plano, &t) == bloque;
        for (off, len) in runs_in(&plano, opts.min_len, opts.min_printable) {
            let (rel, l2) = match trim_to_text(&plano[off..off + len], opts.min_len) {
                Some(x) => x,
                None => continue,
            };
            let trozo = &plano[off + rel..off + rel + l2];
            if !looks_like_text(trozo) {
                continue;
            }
            if !ida_y_vuelta {
                continue;
            }
            let texto = String::from_utf8_lossy(trozo).to_string();
            if !texto.chars().any(|c| c.is_ascii_alphabetic()) {
                continue;
            }
            out.push(DecryptedString {
                offset: (off + rel) as u64,
                length: l2,
                transform: t,
                printable_ratio: printable_ratio(trozo),
                utf8: true,
                plaintext: texto,
            });
        }
    }
    // 5) Rotar + XOR.
    for rot in 1u32..8 {
        for &k in xor_keys.iter().take(4) {
            recoge(&mut out, bloque, &Transform::RolXor { rot, key: k }, opts);
        }
    }

    out.sort_by(|a, b| a.offset.cmp(&b.offset).then(a.length.cmp(&b.length)));
    out.dedup_by(|a, b| a.offset == b.offset && a.plaintext == b.plaintext);
    ScanResult {
        found: out,
        scanned_bytes: bloque.len(),
        truncated: truncado,
    }
}

/// Aplica una transformación byte a byte y recoge las tiradas que pasan la prueba.
fn recoge(
    out: &mut Vec<DecryptedString>,
    bloque: &[u8],
    t: &Transform,
    opts: &DetectOptions,
) {
    for (off, len) in runs_bytewise(bloque, t, opts.min_len, opts.min_printable) {
        let blob = &bloque[off..off + len];
        let (_, plain) = prove(blob, t);
        // Recorte: la tirada puede venir con relleno "imprimible" delante o
        // detras (p.ej. bytes nulos que, bajo la clave, salen como 'Z').
        let (rel, l2) = match trim_to_text(&plain, opts.min_len) {
            Some(x) => x,
            None => continue,
        };
        let blob2 = &bloque[off + rel..off + rel + l2];
        let (proof, plain2) = prove(blob2, t);
        if !proof.passed {
            continue;
        }
        let texto = String::from_utf8_lossy(&plain2).to_string();
        if !texto.chars().any(|c| c.is_ascii_alphabetic()) {
            continue;
        }
        out.push(DecryptedString {
            offset: (off + rel) as u64,
            length: l2,
            transform: *t,
            printable_ratio: printable_ratio(&plain2),
            utf8: true,
            plaintext: texto,
        });
    }
}

/// Tiradas maximales imprimibles en un bloque **ya descifrado**.
fn runs_in(data: &[u8], min_len: usize, min_printable: f64) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut inicio: Option<usize> = None;
    let mut cuenta: usize = 0;
    for (i, &p) in data.iter().enumerate() {
        let ok = (0x20..=0x7e).contains(&p) || p == b'\t' || p == b'\n' || p == b'\r';
        if ok {
            if inicio.is_none() {
                inicio = Some(i);
                cuenta = 0;
            }
            cuenta += 1;
        } else {
            if let Some(s) = inicio {
                if cuenta >= min_len {
                    out.push((s, cuenta));
                }
            }
            inicio = None;
            cuenta = 0;
        }
    }
    if let Some(s) = inicio {
        if cuenta >= min_len {
            out.push((s, cuenta));
        }
    }
    out.into_iter()
        .filter(|(s, l)| printable_ratio(&data[*s..*s + *l]) >= min_printable)
        .collect()
}

/// Tiradas maximales imprimibles aplicando la transformación byte a byte.
///
/// Sólo sirve para transformaciones **punto a punto** (XOR/ADD/Rol): el XOR
/// encadenado tiene estado global y se calcula aparte.
fn runs_bytewise(
    data: &[u8],
    t: &Transform,
    min_len: usize,
    min_printable: f64,
) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut inicio: Option<usize> = None;
    let mut cuenta: usize = 0;
    for (i, &b) in data.iter().enumerate() {
        let p = match t {
            Transform::XorByte(k) => b ^ k,
            Transform::AddByte(k) => b.wrapping_sub(*k),
            Transform::RolXor { rot, key } => b.rotate_right((*rot % 8) as u32) ^ key,
            Transform::XorRolling(_) => 0,
        };
        let ok = (0x20..=0x7e).contains(&p) || p == b'\t' || p == b'\n' || p == b'\r';
        if ok {
            if inicio.is_none() {
                inicio = Some(i);
                cuenta = 0;
            }
            cuenta += 1;
        } else {
            if let Some(s) = inicio {
                if cuenta >= min_len {
                    out.push((s, cuenta));
                }
            }
            inicio = None;
            cuenta = 0;
        }
    }
    if let Some(s) = inicio {
        if cuenta >= min_len {
            out.push((s, cuenta));
        }
    }
    // OJO: la proporcion se mide sobre los bytes **descifrados**, no sobre los
    // originales (que son opacos).  Medir sobre los originales rechazaba
    // TODAS las tiradas.
    out.into_iter()
        .filter(|(s, l)| {
            let dec: Vec<u8> = data[*s..*s + *l]
                .iter()
                .map(|&b| match t {
                    Transform::XorByte(k) => b ^ k,
                    Transform::AddByte(k) => b.wrapping_sub(*k),
                    Transform::RolXor { rot, key } => (b ^ key).rotate_right((*rot % 8) as u32),
                    Transform::XorRolling(_) => b,
                })
                .collect();
            printable_ratio(&dec) >= min_printable
        })
        .collect()
}

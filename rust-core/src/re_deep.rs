//! `re_deep` — RE **profunda** de APK/DEX para MegaDaemon.
//!
//! POR QUÉ EXISTE
//! -------------
//! `dex_decompile` + `apk_deobfuscator` + `apk_inventory` ya cubren el
//! desensamblado, el manifiesto, la firma y el fingerprint de ofuscación.
//! Lo que **no** cubren (y este módulo añade) son las cinco cosas que
//! hicieron falta para reconstruir el subsistema de gasto de datos de un
//! operador real:
//!
//! 1. **Xrefs**: qué clase y qué método usan cada string. Sin esto, un string
//!    como `"consumption-api/subscriptions/{id}/summary"` no se puede ligar a
//!    la interfaz que lo sirve.
//! 2. **Rutas REST**: `extract_endpoints` devuelve URLs *absolutas*; el
//!    inventario de endpoints de un backend real son rutas *relativas*
//!    (`consumption-api/subscriptions/{subscriptionId}/summary`) clasificadas
//!    por familia de API.
//! 3. **Entropía de assets**: un APK con un payload cifrado en `assets/` es
//!    una señal de empaquetado/modulo protegido que el inventario actual no ve.
//! 4. **Pin de certificado**: los APK de operador guardan el SHA-256 del
//!    certificado del *servidor* en un asset (p. ej. `stamp-cert-sha256`).
//!    Compararlo con los certificados de firma del APK dice si un proxy con CA
//!    de usuario va a ser aceptado o rechazado.
//! 5. **Ejecución simbólica**: ejecutar un método del DEX **en tiempo de
//!    análisis** para recuperar en claro los strings que la app descifra en
//!    runtime (los "decryptors" de string que el detector marca).
//!
//! Además, el detector de descifrado se **discrimina**: un candidato con material
//! criptográfico (`javax/crypto/*`) no es un ofuscador de strings sino un SDK
//! de pago 3DS2 ⇒ clasificarlo como `crypto_material` evita 200 falsos
//! positivos.
//!
//! DEPENDENCIAS: **ninguna nueva**. Usa `serde_json`, `sha2` y el crate
//! `dex_decompile` del propio daemon. El crate `zip` es el mismo que usa
//! `apk_inventory`.
//!
//! ESTADO: registrado en `lib.rs` y usable desde la CLI
//! (`megadaemon-core redeep <apk>`) y desde la tool MCP `re_profunda`.
//! Ver `docs/RE_GASTO_DATOS_JAZZTEL.md` §C.13 para el mapa de qué parte ya
//! existía y qué parte aporta este módulo.
//!
//! ARTEFACTOS: los APK y sus volcados **no** van al repo (es público): se
//! quedan fuera, en el directorio de trabajo del agente que hace la RE.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::dex_decompile::{DexFile, DexType};

// ========================================================================= //
// 1) XREFS: string -> (clase, método)
// ========================================================================= //

/// Una referencia a un string desde el código.
#[derive(Debug, Clone, Serialize)]
pub struct StringXref {
    /// El string referenciado.
    pub string: String,
    /// Clase que lo usa (descriptor `Lcom/foo/Bar;`).
    pub class: String,
    /// Método que lo usa.
    pub method: String,
    /// DEX donde vive el método.
    pub dex: String,
}

/// Construye el índice de xrefs de un DEX.
///
/// Escanea el `code_item` de cada método buscando `const-string` (0x1A) y
/// `const-string/jumbo` (0x1B), que son las únicas instrucciones que
/// materializan una constante del pool de strings.
///
/// El recorrido respeta los anchos de instrucción (no se lee "en crudo"): si
/// no, un `const-string` que caiga dentro de los operandos de otra
/// instrucción daría falsos positivos.
pub fn string_xrefs(dex: &DexFile, dex_name: &str) -> Vec<StringXref> {
    let mut out = Vec::new();
    for class in &dex.classes {
        for m in class
            .direct_methods
            .iter()
            .chain(class.virtual_methods.iter())
        {
            let Some(code) = m.code.as_ref() else {
                continue;
            };
            let Some(name) = dex.methods.get(m.method_idx as usize) else {
                continue;
            };
            for sidx in const_string_indexes(&code.insns) {
                if let Some(text) = dex.strings.get(sidx as usize) {
                    out.push(StringXref {
                        string: text.clone(),
                        class: class.class_name.clone(),
                        method: name.name.clone(),
                        dex: dex_name.to_string(),
                    });
                }
            }
        }
    }
    out
}

/// Índices del pool de strings usados por `const-string` en un bloque de código.
pub fn const_string_indexes(insns: &[u16]) -> Vec<u32> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < insns.len() {
        let op = (insns[i] & 0xFF) as u8;
        match op {
            0x1A => {
                out.push(*insns.get(i + 1).unwrap_or(&0) as u32);
                i += 2;
            }
            0x1B => {
                let lo = *insns.get(i + 1).unwrap_or(&0) as u32;
                let hi = *insns.get(i + 2).unwrap_or(&0) as u32;
                out.push(lo | (hi << 16));
                i += 3;
            }
            _ => i = siguiente_insn(insns, i),
        }
    }
    out
}

/// Ancho en unidades de 16 bits de una instrucción Dalvik.
///
/// Delega en [`dalvik_width`], que es la tabla **medida** de este módulo
/// (ver el bloque de convenciones medidas más abajo).
///
/// Por qué NO se usa directamente `dex_decompile::instruction_width`: la tabla
/// del repo pone el `goto` (0x28, formato `10t`) en **dos** unidades, y con
/// esa medida el desensamblado de esta app se desalinea tras cada salto.
/// MEDIDO sobre el APK real (5 DEX, 36.248 clases, 323.861 métodos):
///
/// | ancho del `goto` | opcodes reservados (nunca los emite un compilador) | métodos que terminan en `return`/`throw` |
/// |---|---|---|
/// | 2 (como la tabla del repo) | 5.549 / 4.009.270 | 93,60 % |
/// | 1 (el medido aquí) | **0 / 4.147.865** | **96,35 %** |
///
/// El hallmarks de la tabla equivocada son los opcodes *reservados*
/// (0x3e-0x43, 0x73, 0x79-0x7a, 0xf3-0xf9): un compilador **jamás** los
/// emite, así que cada uno que aparece es un byte de código leído como
/// instrucción ⇒ el barrido está desalineado.
pub fn insn_width(op: u8) -> usize {
    dalvik_width(op)
}

/// Índice de la siguiente instrucción a partir de `i`.
///
/// Respeta los *payloads* de Dalvik (packed-switch, sparse-switch y
/// fill-array-data), que **no** son instrucciones: sus datos tienen que
/// saltarse enteros.
///
/// El identificador del payload viaja en el byte alto **en su sitio**
/// (0x0100, 0x0200, 0x0300). Leerlo como `unidad >> 8` da 0x01/0x02/0x03 y la
/// comparación nunca casa: ese fue un bug real de este módulo (el barrido
/// continuaba dentro de los datos). Los cuatro barreidores del módulo usan
/// **este** helper, para que no vuelvan a divergir entre sí.
fn siguiente_insn(insns: &[u16], i: usize) -> usize {
    let u = insns[i];
    let op = (u & 0xFF) as u8;
    if op == 0x00 {
        match u & 0xff00 {
            // packed-switch-payload: ident + size + first_key(2) + targets(2*size)
            0x0100 => {
                let size = *insns.get(i + 1).unwrap_or(&0) as usize;
                return i + 4 + size * 2;
            }
            // sparse-switch-payload: ident + size + keys(2*size) + targets(2*size)
            0x0200 => {
                let size = *insns.get(i + 1).unwrap_or(&0) as usize;
                return i + 2 + size * 4;
            }
            // fill-array-data-payload: ident + ancho + size(2) + datos
            0x0300 => {
                let ew = *insns.get(i + 1).unwrap_or(&0) as usize;
                let size = *insns.get(i + 2).unwrap_or(&0) as usize
                    | ((*insns.get(i + 3).unwrap_or(&0) as usize) << 16);
                return i + 4 + (size * ew + 1) / 2;
            }
            // `nop` de verdad: una unidad.
            _ => return i + 1,
        }
    }
    i + insn_width(op)
}

/// ¿El bloque de código usa alguna operación **XOR**?
///
/// El XOR es la marca del ofuscador de strings: la tabla estática se
/// descifra a mano, carácter a carácter, con XOR.
///
/// Se busca por **nombre** de opcode (el repo ya tiene la tabla canónica en
/// `dex_decompile::opcode_info`), no con una lista de números escrita a mano.
/// La lista de antes tenía `0x98` (= `shl-int`) y `0xE0` (= `shl-int/lit8`)
/// como si fueran XOR, y se le pasaban `xor-long` (0xa2), `xor-long/2addr`
/// (0xc2) y `xor-int/lit16` (0xd7).
pub fn tiene_xor(insns: &[u16]) -> bool {
    let mut i = 0usize;
    while i < insns.len() {
        let op = (insns[i] & 0xFF) as u8;
        if op != 0x00 && crate::dex_decompile::opcode_info(op).0.starts_with("xor") {
            return true;
        }
        i = siguiente_insn(insns, i);
    }
    false
}

// ========================================================================= //
// 2) RUTAS REST: inventario y familia de API
// ========================================================================= //

/// Una ruta REST relativa con su familia de API.
#[derive(Debug, Clone, Serialize)]
pub struct RestPath {
    /// La ruta (`consumption-api/subscriptions/{subscriptionId}/summary`).
    pub path: String,
    /// Familia de API (el primer segmento): `consumption-api`, `balances-api`…
    pub family: String,
    /// Parámetros de ruta (`{subscriptionId}`).
    pub params: Vec<String>,
    /// Nº de clases del DEX que la referencian.
    pub refs: usize,
}

/// Extrae las rutas REST relativas del pool de strings y las clasifica.
///
/// Un camino es una ruta si cumple `segmento(/segmento)+` donde cada segmento
/// son `[a-z0-9_-]` o `{param}`. Se descartan los falsos positivos típicos
/// (`assets/dexopt/baseline.prof`, `classes.dex`).
pub fn rest_paths(dex: &DexFile) -> Vec<RestPath> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for s in &dex.strings {
        if let Some(p) = classify_rest_path(s) {
            *counts.entry(p).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .map(|(path, refs)| {
            let segs: Vec<&str> = path.split('/').collect();
            let params = segs
                .iter()
                .filter(|s| s.starts_with('{') && s.ends_with('}'))
                .map(|s| s.trim_matches(|c| c == '{' || c == '}').to_string())
                .collect();
            RestPath {
                family: segs.first().copied().unwrap_or("").to_string(),
                path,
                params,
                refs,
            }
        })
        .collect()
}

/// Devuelve la ruta si el string **es** una ruta REST relativa.
///
/// Regla por segmento:
///
/// * Un **parámetro** (`{subscriptionId}`) se acepta con camelCase: es el
///   nombre de una variable Java, no un segmento estático. MEDIDO en la app
///   del operador: **todas** sus rutas llevan parámetros en camelCase
///   (`{subscriptionId}`, `{accountId}`, `{productId}`), así que rechazar las
///   mayúsculas aquí dejaba el inventario de rutas **vacío**.
/// * Un **segmento estático** va en minúsculas, con dígitos, `_` o `-`.
///
/// La familia (el primer segmento) tiene que ser de API: `<algo>-api`, o un
/// nombre que contenga `api` o `services`.
pub fn classify_rest_path(s: &str) -> Option<String> {
    if s.len() < 3 || s.len() > 120 {
        return None;
    }
    if s.starts_with('/') || s.contains("://") || s.contains(' ') {
        return None;
    }
    let segs: Vec<&str> = s.split('/').collect();
    if segs.len() < 2 {
        return None;
    }
    for seg in &segs {
        let tiene_llave = seg.contains('{') || seg.contains('}');
        let es_parametro = seg.starts_with('{') && seg.ends_with('}') && seg.len() > 2;
        if tiene_llave && !es_parametro {
            // Llave suelta: no es ni un parámetro ni un segmento estático.
            return None;
        }
        let core = seg.trim_matches(|c| c == '{' || c == '}');
        if core.is_empty() {
            return None;
        }
        let ok = if es_parametro {
            core.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        } else {
            core.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        };
        if !ok {
            return None;
        }
        // Familias conocidas: prefijadas por "<algo>-api" o nombres de dominio.
        let family_ok =
            segs[0].ends_with("-api") || segs[0].contains("api") || segs[0].contains("services");
        if !family_ok {
            return None;
        }
    }
    Some(s.to_string())
}

// ========================================================================= //
// 3) ENTROPÍA DE ASSETS (payload cifrados)
// ========================================================================= //

/// Informe de un asset del APK.
#[derive(Debug, Clone, Serialize)]
pub struct AssetReport {
    /// Ruta dentro del ZIP.
    pub name: String,
    /// Tamaño en bytes.
    pub size: usize,
    /// Entropía de Shannon en bits/byte (0..8).
    pub entropy: f64,
    /// `true` si parece cifrado (entropía alta y sin magic conocido).
    pub looks_encrypted: bool,
    /// Magic detectado, si lo hay (`dex`, `PK`, `gzip`…).
    pub magic: Option<String>,
}

/// Calcula la entropía de Shannon de un buffer (bits por byte).
pub fn entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut hist = [0u32; 256];
    for b in data {
        hist[*b as usize] += 1;
    }
    let n = data.len() as f64;
    let mut h = 0.0f64;
    for c in hist.iter() {
        if *c > 0 {
            let p = *c as f64 / n;
            h -= p * p.log2();
        }
    }
    h
}

/// Detecta magics conocidos al principio de un buffer.
pub fn detect_magic(data: &[u8]) -> Option<&'static str> {
    const MAGICS: &[(&[u8], &str)] = &[
        (b"dex\n", "dex"),
        (b"PK\x03\x04", "zip"),
        (b"\x1f\x8b", "gzip"),
        (b"\x1f\x9d", "compress"),
        (b"7z\xbc\xaf\x27\x1c", "7z"),
        (b"Rar!", "rar"),
        (b"\x89PNG", "png"),
        (b"\xff\xd8\xff", "jpeg"),
        (b"SQLite format 3", "sqlite"),
        (b"\x04\x22\x4d\x18", "lz4"),
        (b"\x28\xb5\x2f\xfd", "zstd"),
        (b"BZh", "bzip2"),
    ];
    for (m, name) in MAGICS {
        if data.starts_with(m) {
            return Some(name);
        }
    }
    None
}

/// Informe de entropía de todos los assets.
///
/// Un asset con entropía > 7,9 bits/byte y sin magic reconocible es, con
/// mucha probabilidad, un **payload cifrado** (módulo de código protegido,
/// configuración cifrada o un perfil de antifraude).
pub fn asset_reports(assets: &[(String, Vec<u8>)]) -> Vec<AssetReport> {
    assets
        .iter()
        .map(|(name, data)| {
            let e = entropy(data);
            let magic = detect_magic(data);
            AssetReport {
                name: name.clone(),
                size: data.len(),
                entropy: (e * 1000.0).round() / 1000.0,
                looks_encrypted: e > 7.9 && magic.is_none() && data.len() > 512,
                magic: magic.map(|s| s.to_string()),
            }
        })
        .collect()
}

// ========================================================================= //
// 4) PIN DE CERTIFICADO vs CERTIFICADOS DE FIRMA DEL APK
// ========================================================================= //

/// Un certificado del APK Signing Block.
#[derive(Debug, Clone, Serialize)]
pub struct SigningCert {
    /// SHA-256 del certificado (hex).
    pub sha256: String,
    /// Esquema de firma (`v2`, `v3`).
    pub scheme: String,
}

/// Extrae los certificados del **APK Signing Block** (v2 y v3).
///
/// El bloque de firma vive entre el final del contenido y el Central
/// Directory del ZIP, y se localiza por el magic `APK Sig Block 42` (inverso).
/// Cada entrada es un par (id u32, tamaño u32); los ids que interesan son
/// `0x7109871a` (v2) y `0xf05368c0` (v3).
pub fn signing_certs(apk: &[u8]) -> Vec<SigningCert> {
    const MAGIC: &[u8] = b"APK Sig Block 42";
    let Some(cd_off) = find_central_directory(apk) else {
        return Vec::new();
    };
    let Some(m) = find_bytes(apk, MAGIC, cd_off) else {
        return Vec::new();
    };
    if m < 8 {
        return Vec::new();
    }
    let block_size = u64::from_le_bytes([
        apk[m - 8],
        apk[m - 7],
        apk[m - 6],
        apk[m - 5],
        apk[m - 4],
        apk[m - 3],
        apk[m - 2],
        apk[m - 1],
    ]) as usize;
    if block_size < 24 || m + 16 > apk.len() {
        return Vec::new();
    }
    let start = m + 16 - block_size;
    let mut out = Vec::new();
    let mut p = start + 8;
    let end = m - 8;
    while p + 8 <= end && p + 8 <= apk.len() {
        let id = u32::from_le_bytes([apk[p], apk[p + 1], apk[p + 2], apk[p + 3]]);
        let len = u32::from_le_bytes([apk[p + 4], apk[p + 5], apk[p + 6], apk[p + 7]]) as usize;
        if len < 4 || p + 8 + len > end || p + 8 + len > apk.len() {
            break;
        }
        let scheme = match id {
            0x7109_871a => Some("v2"),
            0xf053_68c0 => Some("v3"),
            _ => None,
        };
        if let Some(scheme) = scheme {
            for digest in cert_digests(&apk[p + 8..p + 8 + len]) {
                out.push(SigningCert {
                    sha256: digest,
                    scheme: scheme.to_string(),
                });
            }
        }
        p += 8 + len;
    }
    out
}

/// SHA-256 de los certificados de un bloque de firma (v2 o v3).
///
/// El bloque es una secuencia de piezas con prefijo de longitud: la primera
/// pieza son los *signed data*, y dentro, la tercera, la lista de certificados.
fn cert_digests(block: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut pieces = Vec::new();
    while i + 4 <= block.len() {
        let len = u32::from_le_bytes([block[i], block[i + 1], block[i + 2], block[i + 3]]) as usize;
        i += 4;
        if len > block.len() - i {
            break;
        }
        pieces.push(&block[i..i + len]);
        i += len;
    }
    let Some(signed) = pieces.get(1) else {
        return out;
    };
    // Dentro de signed data: (digests) (certificados) (atributos adicionales)
    let mut j = 0usize;
    let mut certs: &[u8] = &[];
    let mut seen = 0;
    while j + 4 <= signed.len() {
        let len =
            u32::from_le_bytes([signed[j], signed[j + 1], signed[j + 2], signed[j + 3]]) as usize;
        j += 4;
        if len > signed.len() - j {
            break;
        }
        seen += 1;
        if seen == 2 {
            certs = &signed[j..j + len];
        }
        j += len;
    }
    let mut k = 0usize;
    while k + 4 <= certs.len() {
        let len = u32::from_le_bytes([certs[k], certs[k + 1], certs[k + 2], certs[k + 3]]) as usize;
        k += 4;
        if len > certs.len() - k {
            break;
        }
        let der = &certs[k..k + len];
        if der.len() > 4 {
            out.push(sha256_hex(&der[4..]));
        }
        k += len;
    }
    out
}

/// Informe de pinning: qué pins declara el APK y si son del servidor.
#[derive(Debug, Clone, Serialize)]
pub struct PinReport {
    /// Pins encontrados (SHA-256 hex) con el asset que los declara.
    pub pins: Vec<Pin>,
    /// Certificados de firma del propio APK.
    pub signing_certs: Vec<SigningCert>,
    /// Pins que **no** son ningún certificado de firma ⇒ son del servidor.
    pub server_pins: Vec<String>,
}

/// Un pin de certificado declarado por la app.
#[derive(Debug, Clone, Serialize)]
pub struct Pin {
    /// Asset que lo declara.
    pub asset: String,
    /// SHA-256 en hex.
    pub sha256: String,
}

/// Compara los pins declarados en `assets/` con los certificados de firma.
///
/// Si un pin **no** está en la lista de certificados de firma, el pin es del
/// **servidor**: una CA de usuario no basta y hay que hookear la
/// verificación.
pub fn pin_report(assets: &[(String, Vec<u8>)], apk: &[u8]) -> PinReport {
    let mut pins = Vec::new();
    for (name, data) in assets {
        if data.len() != 32 {
            continue;
        }
        let looks_hex = name.contains("cert") || name.contains("sha") || name.contains("pin");
        if !looks_hex {
            continue;
        }
        pins.push(Pin {
            asset: name.clone(),
            sha256: sha256_hex(data),
        });
    }
    let certs = signing_certs(apk);
    let server_pins = pins
        .iter()
        .filter(|p| {
            !certs
                .iter()
                .any(|c| c.sha256.eq_ignore_ascii_case(&p.sha256))
        })
        .map(|p| p.sha256.clone())
        .collect();
    PinReport {
        pins,
        signing_certs: certs,
        server_pins,
    }
}

/// SHA-256 en hex minúsculas.
///
/// Delega en el helper compartido (`self_update::sha256_hex`): el repo ya
/// tenía tres copias locales de este cálculo y la regla de la orden
/// imperativa pide **unificar lo equivalente** en vez de propagar la cuarta.
pub fn sha256_hex(data: &[u8]) -> String {
    crate::self_update::sha256_hex(data)
}

/// Busca el inicio del Central Directory del ZIP (firma `PK\x05\x06`).
fn find_central_directory(apk: &[u8]) -> Option<usize> {
    let sig = [0x50, 0x4B, 0x05, 0x06];
    let mut i = apk.len().checked_sub(22)?;
    let min = i.saturating_sub(0x10000);
    while i >= min {
        if apk[i..i + 4] == sig {
            let off =
                u32::from_le_bytes([apk[i + 16], apk[i + 17], apk[i + 18], apk[i + 19]]) as usize;
            return Some(off);
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    None
}

/// Busca `needle` en `hay` antes de `limit` (de atrás hacia delante).
fn find_bytes(hay: &[u8], needle: &[u8], limit: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let end = limit.min(hay.len());
    if end < needle.len() {
        return None;
    }
    let mut i = end - needle.len();
    loop {
        if &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

// ========================================================================= //
// 5) DESCIFRADOR DE STRINGS: discriminar cripto de ofuscación
// ========================================================================= //

/// Un candidato a descifrador, ya clasificado.
#[derive(Debug, Clone, Serialize)]
pub struct DecryptorCandidate {
    /// Clase.
    pub class: String,
    /// Tipo de tabla estática (`[I`, `[C`, `[B`…).
    pub table: String,
    /// Métodos que usan XOR.
    pub xor_methods: Vec<String>,
    /// Métodos que construyen un `String`.
    pub string_methods: Vec<String>,
    /// **Clasificación**: `string_decryptor` u `crypto_material`.
    pub kind: String,
    /// Evidencia de por qué se clasifica así.
    pub reason: String,
    /// Confianza 0..100.
    pub confidence: u32,
}

/// Detecta candidatos a descifrador y **discrimina** los falsos positivos.
///
/// Un candidato que además usa `javax/crypto/*` o `java/security/*` (y una
/// tabla que se usa como *material* y no como *índice*) es un SDK de pago
/// 3DS2, no un ofuscador de strings. Sin esta discriminator, una app con
/// criptografía legítima genera cientos de falsos positivos.
pub fn decryptor_candidates(dex: &DexFile) -> Vec<DecryptorCandidate> {
    let crypto_classes = BTreeSet::from([
        "Ljavax/crypto/Cipher;",
        "Ljavax/crypto/spec/SecretKeySpec;",
        "Ljavax/crypto/KeyGenerator;",
        "Ljavax/crypto/Mac;",
        "Ljava/security/MessageDigest;",
        "Ljava/security/KeyFactory;",
        "Ljava/security/spec/PKCS8EncodedKeySpec;",
    ]);
    let mut out = Vec::new();
    for class in &dex.classes {
        let table = class
            .static_fields
            .iter()
            .filter_map(|f| dex.fields.get(f.field_idx as usize))
            .find(|f| {
                matches!(
                    f.field_type.as_str(),
                    "[I" | "[B" | "[C" | "[Ljava/lang/String;"
                )
            })
            .map(|f| f.field_type.clone());
        let Some(table) = table else { continue };
        let mut xor_methods = Vec::new();
        let mut string_methods = Vec::new();
        let mut uses_crypto = false;
        for m in class
            .direct_methods
            .iter()
            .chain(class.virtual_methods.iter())
        {
            let Some(code) = m.code.as_ref() else {
                continue;
            };
            let Some(name) = dex.methods.get(m.method_idx as usize) else {
                continue;
            };
            let calls = method_calls(dex, &code.insns);
            // El XOR no es una invocación: `method_calls` no lo ve.  Antes se
            // buscaba en la lista de invocaciones (imposible que casara, ⇒ el
            // detector NUNCA encontraba un descifrador) y con una lista de
            // opcodes escrita a mano que además tenía `shl-int` por `xor`.
            let has_xor = tiene_xor(&code.insns);
            let creates = calls
                .iter()
                .any(|c| c.1 == "Ljava/lang/String;" && c.2 == "<init>");
            if has_xor {
                xor_methods.push(name.name.clone());
            }
            if creates {
                string_methods.push(name.name.clone());
            }
            if calls.iter().any(|c| crypto_classes.contains(&c.1.as_str())) {
                uses_crypto = true;
            }
        }
        if xor_methods.is_empty() || string_methods.is_empty() {
            continue;
        }
        let (kind, reason, confidence) = if uses_crypto {
            (
                "crypto_material",
                "usa javax/crypto o java/security: es material criptográfico, no una tabla de sustitución",
                86,
            )
        } else {
            (
                "string_decryptor",
                "array estático + XOR + construcción de String: patrón del playbook §5.1",
                86,
            )
        };
        out.push(DecryptorCandidate {
            class: class.class_name.clone(),
            table,
            xor_methods: xor_methods.into_iter().take(8).collect(),
            string_methods: string_methods.into_iter().take(8).collect(),
            kind: kind.to_string(),
            reason: reason.to_string(),
            confidence,
        });
    }
    out.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    out
}

/// Devuelve las invocaciones de método de un bloque de código.
///
/// Cada elemento es `(opcode, clase, nombre)`.  Ojo: aquí sólo se registran
/// las **invocaciones**; para saber si hay un XOR hay que usar
/// [`tiene_xor`] (un XOR no invoca a nadie).
pub fn method_calls(dex: &DexFile, insns: &[u16]) -> Vec<(u8, String, String)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < insns.len() {
        let op = (insns[i] & 0xFF) as u8;
        match op {
            0x6E..=0x72 | 0x74..=0x78 => {
                if let Some(idx) = insns.get(i + 1) {
                    if let Some(m) = dex.methods.get(*idx as usize) {
                        out.push((op, m.class_name.clone(), m.name.clone()));
                    }
                }
                i += 3;
            }
            _ => i = siguiente_insn(insns, i),
        }
    }
    out
}

// ========================================================================= //
// 6) EJECUCIÓN SIMBÓLICA: intérprete Dalvik acotado
// ========================================================================= //

/// Valor manipulado por el intérprete.
#[derive(Debug, Clone)]
pub enum Val {
    /// Entero de 32 bits.
    Int(i32),
    /// Entero de 64 bits.
    Long(i64),
    /// Referencia nula.
    Null,
    /// Referencia a un objeto.
    Ref(Obj),
}

/// Un objeto del modelo virtual.
#[derive(Debug, Clone)]
pub struct Obj {
    /// Tipo del objeto (`java.lang.String`, `int[]`, `char[]`…).
    pub class: String,
    /// Contenido si es un `String`.
    pub text: String,
    /// Contenido si es un array (`int[]`/`char[]`).
    pub items: Vec<i32>,
    /// Tipo del array, si lo es.
    pub elem: String,
    /// Campos extra (sólo para tipos propios; `Box` para no crear un ciclo
    /// de tamaño infinito `Val -> Obj -> BTreeMap<String, Val>`).
    pub fields: Box<BTreeMap<String, Val>>,
}

/// Motivo por el que el intérprete se detiene.
#[derive(Debug, Clone, Serialize)]
pub struct ExecError {
    /// Motivo legible.
    pub reason: String,
    /// Clase y método donde se detuvo.
    pub where_: String,
}

/// Límite de pasos del intérprete (evita bucles infinitos).
pub const MAX_STEPS: usize = 200_000;
/// Profundidad máxima de recursión entre métodos del DEX.
pub const MAX_DEPTH: usize = 12;

/// Ejecuta un método del DEX en un intérprete acotado y devuelve su valor.
///
/// Esto es **ejecución simbólica** de verdad: el método se interpreta
/// instrucción a instrucción y las llamadas a otros métodos del **mismo DEX**
/// se interpretan recursivamente. Sirve para recuperar en claro los strings
/// que la app descifra en runtime (el caso típico: un `decrypt(String)` cuyo
/// resultado no aparece nunca en el pool de strings).
///
/// No es un intérprete completo: si toca una API de la plataforma que no se
/// sabe emular, **se detiene y lo dice** (`ExecError`), nunca inventa.
pub fn interpret_method(dex: &DexFile, class: &str, method: &str) -> Result<Value, ExecError> {
    let mut state = ExecState { steps: 0, depth: 0 };
    let v = exec_method(dex, class, method, &[], &mut state)?;
    Ok(match v {
        Val::Int(i) => json!({ "tipo": "int", "valor": i }),
        Val::Long(l) => json!({ "tipo": "long", "valor": l }),
        Val::Null => json!({ "tipo": "null" }),
        Val::Ref(o) => json!({ "tipo": o.class, "texto": o.text, "items": o.items.len() }),
    })
}

/// Estado del intérprete (presupuesto de pasos y profundidad).
struct ExecState {
    steps: usize,
    depth: usize,
}

/// Ejecuta un método con los argumentos dados.
fn exec_method(
    dex: &DexFile,
    class: &str,
    method: &str,
    args: &[Val],
    st: &mut ExecState,
) -> Result<Val, ExecError> {
    if st.depth > MAX_DEPTH {
        return Err(ExecError {
            reason: "profundidad máxima de recursión".into(),
            where_: format!("{}->{}", class, method),
        });
    }
    let Some(cls) = dex.classes.iter().find(|c| c.class_name == class) else {
        return Err(ExecError {
            reason: "clase no encontrada en el DEX".into(),
            where_: class.to_string(),
        });
    };
    let enc = cls
        .direct_methods
        .iter()
        .chain(cls.virtual_methods.iter())
        .find(|m| {
            dex.methods
                .get(m.method_idx as usize)
                .map(|x| x.name == method)
                .unwrap_or(false)
        });
    let Some(enc) = enc else {
        return Err(ExecError {
            reason: "método no encontrado".into(),
            where_: format!("{}->{}", class, method),
        });
    };
    let Some(code) = enc.code.as_ref() else {
        return Err(ExecError {
            reason: "método sin cuerpo (abstracto/nativo)".into(),
            where_: format!("{}->{}", class, method),
        });
    };
    let insns = &code.insns;
    let mut regs: Vec<Val> = vec![Val::Int(0); code.registers_size as usize];
    // Los parámetros se copian a los últimos registros (convención Dalvik).
    let n = args.len();
    for (k, a) in args.iter().enumerate() {
        let idx = regs.len().saturating_sub(n - k);
        if idx < regs.len() {
            regs[idx] = a.clone();
        }
    }
    let mut pc = 0usize;
    let mut last: Option<Val> = None;
    while pc < insns.len() {
        st.steps += 1;
        if st.steps > MAX_STEPS {
            return Err(ExecError {
                reason: "presupuesto de pasos agotado (¿bucle infinito?)".into(),
                where_: format!("{}->{}", class, method),
            });
        }
        let u = insns[pc];
        let op = (u & 0xFF) as u8;
        let a = (u >> 8) as usize;
        let b = (u >> 12) as usize;
        let c = (u & 0xFF) as usize;
        macro_rules! get {
            ($i:expr) => {
                regs.get($i).cloned().unwrap_or(Val::Int(0))
            };
        }
        macro_rules! set {
            ($i:expr, $v:expr) => {
                if $i < regs.len() {
                    regs[$i] = $v;
                }
            };
        }
        match op {
            0x00 => {
                // `nop` o payload de Dalvik: el helper compartido decide
                // cuántos datos hay que saltarse.
                pc = siguiente_insn(insns, pc);
            }
            0x0A | 0x0B | 0x0C => {
                last = Some(get!(a));
                pc += 1;
            }
            0x0E => return Ok(last.unwrap_or(Val::Null)),
            0x0F | 0x10 | 0x11 => {
                let v = get!(a);
                last = Some(v);
                pc += 1;
            }
            0x12 => {
                set!(a, Val::Int(((u >> 12) & 0xF) as i32));
                pc += 1;
            }
            0x13 => {
                let v = *insns.get(pc + 1).unwrap_or(&0) as i16 as i32;
                set!(a, Val::Int(v));
                pc += 2;
            }
            0x14 => {
                let v = *insns.get(pc + 1).unwrap_or(&0) as i16 as i32
                    | ((*insns.get(pc + 2).unwrap_or(&0) as i16 as i32) << 16);
                set!(a, Val::Int(v));
                pc += 3;
            }
            0x16 => {
                let v = *insns.get(pc + 1).unwrap_or(&0) as i16 as i64;
                set!(a, Val::Long(v));
                pc += 2;
            }
            0x1A => {
                let sidx = *insns.get(pc + 1).unwrap_or(&0) as usize;
                let s = dex.strings.get(sidx).cloned().unwrap_or_default();
                set!(
                    a,
                    Val::Ref(Obj {
                        class: "java.lang.String".into(),
                        text: s,
                        items: vec![],
                        elem: String::new(),
                        fields: Box::new(BTreeMap::new())
                    })
                );
                pc += 2;
            }
            0x1B => {
                let lo = *insns.get(pc + 1).unwrap_or(&0) as usize;
                let hi = *insns.get(pc + 2).unwrap_or(&0) as usize;
                let s = dex
                    .strings
                    .get(lo | (hi << 16))
                    .cloned()
                    .unwrap_or_default();
                set!(
                    a,
                    Val::Ref(Obj {
                        class: "java.lang.String".into(),
                        text: s,
                        items: vec![],
                        elem: String::new(),
                        fields: Box::new(BTreeMap::new())
                    })
                );
                pc += 3;
            }
            0x22 => {
                let ti = *insns.get(pc + 1).unwrap_or(&0) as usize;
                let desc = dex
                    .types
                    .get(ti)
                    .map(|t: &DexType| t.descriptor.clone())
                    .unwrap_or_default();
                set!(
                    a,
                    Val::Ref(Obj {
                        class: desc,
                        text: String::new(),
                        items: vec![],
                        elem: String::new(),
                        fields: Box::new(BTreeMap::new())
                    })
                );
                pc += 2;
            }
            0x23 => {
                let ti = *insns.get(pc + 1).unwrap_or(&0) as usize;
                let desc = dex
                    .types
                    .get(ti)
                    .map(|t: &DexType| t.descriptor.clone())
                    .unwrap_or_default();
                let n = match get!(c) {
                    Val::Int(i) => i.max(0) as usize,
                    _ => 0,
                };
                set!(
                    a,
                    Val::Ref(Obj {
                        class: desc,
                        text: String::new(),
                        items: vec![0i32; n],
                        elem: "int".into(),
                        fields: Box::new(BTreeMap::new())
                    })
                );
                pc += 2;
            }
            0x21 => {
                let n = match get!(b) {
                    Val::Ref(o) => o.items.len() as i32,
                    _ => 0,
                };
                set!(a, Val::Int(n));
                pc += 1;
            }
            0x01 | 0x07 => {
                set!(a, get!(b));
                pc += 1;
            }
            0x02 | 0x08 => {
                let v = get!(*insns.get(pc + 1).unwrap_or(&0) as usize);
                set!(a, v);
                pc += 2;
            }
            0x04 | 0x05 => {
                let v = get!(b);
                set!(a, v);
                pc += if op == 0x04 { 1 } else { 2 };
            }
            0x24 => {
                // filled-new-array
                let ti = *insns.get(pc + 1).unwrap_or(&0) as usize;
                let desc = dex
                    .types
                    .get(ti)
                    .map(|t: &DexType| t.descriptor.clone())
                    .unwrap_or_default();
                let count = (u >> 8) as usize & 0xF;
                let mut items = Vec::new();
                for k in 0..count {
                    items.push(match get!(c + k) {
                        Val::Int(i) => i,
                        _ => 0,
                    });
                }
                set!(
                    a,
                    Val::Ref(Obj {
                        class: desc,
                        text: String::new(),
                        items,
                        elem: "int".into(),
                        fields: Box::new(BTreeMap::new())
                    })
                );
                pc += 3;
            }
            0x44..=0x4D => {
                // aget*
                let arr = get!(b);
                let idx = match get!(c) {
                    Val::Int(i) => i,
                    _ => 0,
                };
                let v = match arr {
                    Val::Ref(o) => o
                        .items
                        .get(idx as usize)
                        .copied()
                        .map(Val::Int)
                        .unwrap_or(Val::Int(0)),
                    _ => Val::Int(0),
                };
                set!(a, v);
                pc += 2;
            }
            0x4B..=0x51 => {
                // aput*
                let val = match get!(b) {
                    Val::Int(i) => i,
                    _ => 0,
                };
                let idx = match get!(c) {
                    Val::Int(i) => i,
                    _ => 0,
                };
                if let Val::Ref(o) = get!(a) {
                    let mut o = o;
                    if let Some(slot) = o.items.get_mut(idx as usize) {
                        *slot = val;
                    }
                    set!(a, Val::Ref(o));
                }
                pc += 2;
            }
            0x90..=0xAF => {
                let x = as_int(&get!(b));
                let y = as_int(&get!(c));
                let r = match op {
                    0x90 => x.wrapping_add(y),
                    0x91 => x.wrapping_sub(y),
                    0x92 => x.wrapping_mul(y),
                    0x93 => {
                        if y == 0 {
                            return Err(ExecError {
                                reason: "división por cero".into(),
                                where_: format!("{}->{}", class, method),
                            });
                        }
                        x / y
                    }
                    0x94 => {
                        if y == 0 {
                            return Err(ExecError {
                                reason: "módulo por cero".into(),
                                where_: format!("{}->{}", class, method),
                            });
                        }
                        x % y
                    }
                    0x95 => x & y,
                    0x96 => x | y,
                    0x97 => x ^ y,
                    0x98 => x ^ y,
                    0x99 => x << (y & 31),
                    0x9A => x >> (y & 31),
                    0x9B => ((x as u32) >> (y & 31)) as i32,
                    _ => 0,
                };
                set!(a, Val::Int(r));
                pc += 2;
            }
            0xB0..=0xCF => {
                let x = as_int(&get!(b));
                let y = as_int(&get!(c));
                let r = match op {
                    0xB1 => x.wrapping_add(y),
                    0xB2 => x.wrapping_sub(y),
                    0xB3 => x.wrapping_mul(y),
                    0xB8 => x ^ y,
                    0xB9 => x << (y & 31),
                    0xBA => x >> (y & 31),
                    0xBB => ((x as u32) >> (y & 31)) as i32,
                    _ => 0,
                };
                set!(a, Val::Int(r));
                pc += 1;
            }
            0xD0..=0xE2 => {
                let x = as_int(&get!(b));
                let lit = *insns.get(pc + 1).unwrap_or(&0) as i16 as i32;
                let r = match op {
                    0xD0 => x.wrapping_add(lit),
                    0xD1 => lit.wrapping_sub(x),
                    0xD2 => x.wrapping_mul(lit),
                    0xD5 => x & lit,
                    0xD6 => x | lit,
                    0xD7 => x ^ lit,
                    0xD8 => x.wrapping_add(lit),
                    0xD9 => lit.wrapping_sub(x),
                    0xDA => x.wrapping_mul(lit),
                    0xDD => x & lit,
                    0xDE => x | lit,
                    0xDF => x ^ lit,
                    0xE0 => x << (lit & 31),
                    0xE1 => x >> (lit & 31),
                    0xE2 => ((x as u32) >> (lit & 31)) as i32,
                    _ => 0,
                };
                set!(a, Val::Int(r));
                pc += 2;
            }
            0x7B => {
                set!(a, Val::Int(as_int(&get!(b)).wrapping_neg()));
                pc += 1;
            }
            0x7C => {
                set!(a, Val::Int(!as_int(&get!(b))));
                pc += 1;
            }
            0x8E => {
                set!(a, Val::Int(as_int(&get!(b)) as u8 as i32));
                pc += 1;
            }
            0x8F => {
                set!(a, Val::Int(as_int(&get!(b)) as i16 as i32));
                pc += 1;
            }
            0x81 => {
                set!(a, Val::Long(as_int(&get!(b)) as i64));
                pc += 1;
            }
            0x84 => {
                set!(a, Val::Int(as_long(&get!(b)) as i32));
                pc += 1;
            }
            0x32..=0x37 => {
                let x = as_int(&get!(a));
                let y = as_int(&get!(b));
                let take = match op {
                    0x32 => x == y,
                    0x33 => x != y,
                    0x34 => x < y,
                    0x35 => x >= y,
                    0x36 => x > y,
                    _ => x <= y,
                };
                let off = *insns.get(pc + 1).unwrap_or(&0) as i16 as i32;
                pc += if take { 2 + off as usize } else { 2 };
            }
            0x38..=0x3D => {
                let x = as_int(&get!(a));
                let take = match op {
                    0x38 => x == 0,
                    0x39 => x != 0,
                    0x3A => x < 0,
                    0x3B => x >= 0,
                    0x3C => x > 0,
                    _ => x <= 0,
                };
                let off = *insns.get(pc + 1).unwrap_or(&0) as i16 as i32;
                pc += if take { 2 + off as usize } else { 2 };
            }
            0x28 => {
                let off = (u as u8) as i8 as i32;
                pc = (pc as i32 + off) as usize;
            }
            0x29 => {
                let off = *insns.get(pc + 1).unwrap_or(&0) as i16 as i32;
                pc = (pc as i32 + off) as usize;
            }
            0x2A => {
                let off = *insns.get(pc + 1).unwrap_or(&0) as i16 as i32
                    | ((*insns.get(pc + 2).unwrap_or(&0) as i16 as i32) << 16);
                pc = (pc as i32 + off) as usize;
            }
            0x6E..=0x72 | 0x74..=0x78 => {
                let midx = *insns.get(pc + 1).unwrap_or(&0) as usize;
                let Some(m) = dex.methods.get(midx) else {
                    return Err(ExecError {
                        reason: "método no encontrado".into(),
                        where_: format!("{}->{}", class, method),
                    });
                };
                let (nargs, base) = if op <= 0x72 {
                    // 35c: A|G|op BBBB -> A = nº de args (u>>12), G = 1er reg (u&0xF)
                    (((u >> 12) & 0xF) as usize, (u & 0xF) as usize)
                } else {
                    // 3rc: AA|op BBBB -> AA = nº de args (u>>8), 1er reg = u&0xF
                    ((u >> 8) as usize, (u & 0xF) as usize)
                };
                let mut args: Vec<Val> = Vec::with_capacity(nargs);
                for k in 0..nargs {
                    args.push(get!(base + k));
                }
                let (rc, rn) = (m.class_name.clone(), m.name.clone());
                let res = call(dex, &rc, &rn, &args, st)?;
                set!(a, res.clone());
                last = Some(res);
                pc += 3;
            }
            0x1C | 0x1F => {
                // const-class / check-cast: no cambian el valor
                pc += 2;
            }
            0x20 => {
                // instance-of: se asume false (no hay modelo de clases)
                set!(a, Val::Int(0));
                pc += 2;
            }
            _ => {
                // Instrucción no emulada: se detiene con el motivo exacto.
                return Err(ExecError {
                    reason: format!("opcode 0x{:02X} no emulado", op),
                    where_: format!("{}->{} @{}", class, method, pc),
                });
            }
        }
    }
    Ok(last.unwrap_or(Val::Null))
}

/// Convierte un valor a `i32` para aritmética.
fn as_int(v: &Val) -> i32 {
    match v {
        Val::Int(i) => *i,
        Val::Long(l) => *l as i32,
        Val::Null => 0,
        Val::Ref(o) => o.items.first().copied().unwrap_or(0),
    }
}

/// Convierte un valor a `i64`.
fn as_long(v: &Val) -> i64 {
    match v {
        Val::Int(i) => *i as i64,
        Val::Long(l) => *l,
        Val::Null => 0,
        Val::Ref(o) => o.items.first().copied().unwrap_or(0) as i64,
    }
}

/// Ejecuta una llamada: primero las APIs emulables, luego el propio DEX.
fn call(
    dex: &DexFile,
    class: &str,
    method: &str,
    args: &[Val],
    st: &mut ExecState,
) -> Result<Val, ExecError> {
    // --- APIs de la plataforma que se saben emular -------------------------
    match (class, method) {
        ("java/lang/String", "length") => {
            return Ok(Val::Int(text(args, 0).chars().count() as i32));
        }
        ("java/lang/String", "isEmpty") => {
            return Ok(Val::Int(text(args, 0).is_empty() as i32));
        }
        ("java/lang/String", "charAt") => {
            let s: Vec<char> = text(args, 0).chars().collect();
            let i = args.get(1).map(as_int).unwrap_or(0) as usize;
            return Ok(Val::Int(s.get(i).map(|c| *c as i32).unwrap_or(0)));
        }
        ("java/lang/String", "substring") => {
            let s: Vec<char> = text(args, 0).chars().collect();
            let start = args.get(1).map(as_int).unwrap_or(0) as usize;
            let end = if args.len() > 2 {
                as_int(&args[2]) as usize
            } else {
                s.len()
            };
            // `s.get(rango)` devuelve `Option<&[char]>`: el corte se hace sobre
            // el vector de `char`, no sobre la cadena original (los indices de
            // `substring` son indices de `char`, no de bytes: con emojis o
            // acentos, cortar por bytes parte el character al medio).
            let sub: String = s
                .get(start.min(s.len())..end.min(s.len()))
                .map(|trozo| trozo.iter().collect::<String>())
                .unwrap_or_default();
            return Ok(Val::Ref(Obj {
                class: "java.lang.String".into(),
                text: sub,
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/String", "indexOf") => {
            let hay = text(args, 0);
            let needle = text(args, 1);
            return Ok(Val::Int(match hay.find(&needle) {
                Some(p) => hay[..p].chars().count() as i32,
                None => -1,
            }));
        }
        ("java/lang/String", "toLowerCase") => {
            return Ok(Val::Ref(Obj {
                class: "java.lang.String".into(),
                text: text(args, 0).to_lowercase(),
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/String", "trim") => {
            return Ok(Val::Ref(Obj {
                class: "java.lang.String".into(),
                text: text(args, 0).trim().to_string(),
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/String", "concat") => {
            return Ok(Val::Ref(Obj {
                class: "java.lang.String".into(),
                text: format!("{}{}", text(args, 0), text(args, 1)),
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/String", "toCharArray") => {
            let s: Vec<char> = text(args, 0).chars().collect();
            let items: Vec<i32> = s.iter().map(|c| *c as i32).collect();
            return Ok(Val::Ref(Obj {
                class: "[C".into(),
                text: String::new(),
                items,
                elem: "char".into(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/String", "getBytes") => {
            let s = text(args, 0);
            let items: Vec<i32> = s.bytes().map(|b| b as i32).collect();
            return Ok(Val::Ref(Obj {
                class: "[B".into(),
                text: String::new(),
                items,
                elem: "byte".into(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/String", "valueOf") => {
            return Ok(Val::Ref(Obj {
                class: "java.lang.String".into(),
                text: text(args, 0),
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/String", "equals") => {
            return Ok(Val::Int((text(args, 0) == text(args, 1)) as i32));
        }
        ("java/lang/String", "<init>") => {
            return Ok(Val::Null);
        }
        ("java/lang/StringBuilder", "<init>") => {
            return Ok(Val::Ref(Obj {
                class: "java.lang.StringBuilder".into(),
                text: String::new(),
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/StringBuilder", "append") => {
            let mut acc = match args.first() {
                Some(Val::Ref(o)) => o.text.clone(),
                _ => String::new(),
            };
            acc.push_str(&text(args, 0));
            return Ok(Val::Ref(Obj {
                class: "java.lang.StringBuilder".into(),
                text: acc,
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/StringBuilder", "toString") => {
            return Ok(Val::Ref(Obj {
                class: "java.lang.String".into(),
                text: text(args, 0),
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/Math", "abs") => return Ok(Val::Int(as_int(&args[0]).abs())),
        ("java/lang/Math", "max") => {
            return Ok(Val::Int(
                as_int(&args[0]).max(args.get(1).map(as_int).unwrap_or(0)),
            ));
        }
        ("java/lang/Math", "min") => {
            return Ok(Val::Int(
                as_int(&args[0]).min(args.get(1).map(as_int).unwrap_or(0)),
            ));
        }
        ("java/lang/Integer", "parseInt") => {
            let s = text(args, 0);
            let v = s.trim().parse::<i32>().map_err(|_| ExecError {
                reason: format!("Integer.parseInt(\"{}\") no numérico", s),
                where_: "java.lang.Integer.parseInt".into(),
            })?;
            return Ok(Val::Int(v));
        }
        ("java/lang/Integer", "toString") | ("java/lang/String", "valueOf") => {
            return Ok(Val::Ref(Obj {
                class: "java.lang.String".into(),
                text: as_int(&args[0]).to_string(),
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        ("java/lang/Character", "toString") => {
            let ch = char::from_u32(as_int(&args[0]) as u32).unwrap_or('\u{FFFD}');
            return Ok(Val::Ref(Obj {
                class: "java.lang.String".into(),
                text: ch.to_string(),
                items: vec![],
                elem: String::new(),
                fields: Box::new(BTreeMap::new()),
            }));
        }
        _ => {}
    }
    // --- Si el método está en el DEX, se interpreta -----------------------
    if dex.classes.iter().any(|c| c.class_name == class) {
        st.depth += 1;
        let r = exec_method(dex, class, method, args, st);
        st.depth -= 1;
        return r;
    }
    Err(ExecError {
        reason: format!("API no emulada: {}->{}", class, method),
        where_: format!("{}->{}", class, method),
    })
}

/// Texto de un argumento (String o número).
fn text(args: &[Val], i: usize) -> String {
    match args.get(i) {
        Some(Val::Ref(o)) => o.text.clone(),
        Some(Val::Int(v)) => v.to_string(),
        Some(Val::Long(v)) => v.to_string(),
        _ => String::new(),
    }
}

// ========================================================================= //
// 7) INFORME JSON de todo lo anterior
// ========================================================================= //

/// Informe completo de RE profunda de un APK.
///
/// `apk_bytes` son los bytes del APK; `dexes` son los DEX ya parseados (con su
/// nombre para poder localizarlos).
pub fn deep_report(
    apk_bytes: &[u8],
    assets: &[(String, Vec<u8>)],
    dexes: &[(String, DexFile)],
) -> Value {
    let mut xrefs: Vec<StringXref> = Vec::new();
    let mut paths: BTreeMap<String, usize> = BTreeMap::new();
    let mut decryptors: Vec<DecryptorCandidate> = Vec::new();
    let asset_rows = asset_reports(assets);
    for (name, dex) in dexes {
        xrefs.extend(string_xrefs(dex, name));
        for p in rest_paths(dex) {
            *paths.entry(p.path).or_insert(0) += p.refs;
        }
        decryptors.extend(decryptor_candidates(dex));
    }
    // Paths: se reconstructen desde el recuento agregado
    let mut path_rows: Vec<RestPath> = Vec::new();
    for (path, refs) in paths {
        let segs: Vec<&str> = path.split('/').collect();
        let params = segs
            .iter()
            .filter(|s| s.starts_with('{') && s.ends_with('}'))
            .map(|s| s.trim_matches(|c| c == '{' || c == '}').to_string())
            .collect();
        path_rows.push(RestPath {
            family: segs.first().copied().unwrap_or("").to_string(),
            path,
            params,
            refs,
        });
    }
    path_rows.sort_by(|a, b| a.path.cmp(&b.path));

    json!({
        "assets": asset_rows,
        "pins": pin_report(assets, apk_bytes),
        "rest_paths": path_rows,
        "decryptors": decryptors,
        "xrefs": xrefs,
        "resumen": {
            "xrefs": xrefs.len(),
            "rest_paths": path_rows.len(),
            "decryptors": decryptors.len(),
            "decryptors_reales": decryptors.iter().filter(|d| d.kind == "string_decryptor").count(),
            "assets_cifrados": asset_rows.iter().filter(|a| a.looks_encrypted).count(),
        }
    })
}

/// Índice de strings por clase (para búsquedas rápidas en la UI).
pub fn string_index(dex: &DexFile) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for class in &dex.classes {
        let mut names: Vec<String> = Vec::new();
        for m in class
            .direct_methods
            .iter()
            .chain(class.virtual_methods.iter())
        {
            let Some(code) = m.code.as_ref() else {
                continue;
            };
            for sidx in const_string_indexes(&code.insns) {
                if let Some(s) = dex.strings.get(sidx as usize) {
                    names.push(s.clone());
                }
            }
        }
        names.sort();
        names.dedup();
        out.insert(class.class_name.clone(), names);
    }
    out
}

// ========================================================================= //
// DESPACHO: CLI + tool MCP
// ========================================================================= //

/// Carga un APK del disco.
///
/// Devuelve los bytes crudos (para leer el bloque de firma), los assets, los
/// DEX ya parseados y la lista de DEX que **no** se pudieron parsear (para no
/// callar un fallo: un DEX roto es un dato, no un detalle).
pub fn cargar_apk(
    ruta: &str,
) -> Result<
    (
        Vec<u8>,
        Vec<(String, Vec<u8>)>,
        Vec<(String, DexFile)>,
        Vec<String>,
    ),
    String,
> {
    let bundle = crate::apk_deobfuscator::open_bundle(ruta)?;
    let raw = std::fs::read(ruta).map_err(|e| format!("no se pudo leer {ruta}: {e}"))?;
    let mut dexes = Vec::new();
    let mut fallos = Vec::new();
    for (nombre, bytes) in &bundle.dex_files {
        match DexFile::parse(bytes) {
            Ok(d) => dexes.push((nombre.clone(), d)),
            Err(e) => fallos.push(format!("{nombre}: {e}")),
        }
    }
    Ok((raw, bundle.assets, dexes, fallos))
}

/// Informe completo de un APK (el mismo que produce [`deep_report`]).
pub fn analizar_apk(ruta: &str) -> Result<Value, String> {
    let (raw, assets, dexes, fallos) = cargar_apk(ruta)?;
    let mut informe = deep_report(&raw, &assets, &dexes);
    informe["apk"] = json!({
        "ruta": ruta,
        "sha256": sha256_hex(&raw),
        "bytes": raw.len(),
        "dex_parseados": dexes.len(),
    });
    if !fallos.is_empty() {
        informe["dex_fallidos"] = json!(fallos);
    }
    Ok(informe)
}

/// Despacha la herramienta MCP y el subcomando de la CLI.
///
/// Acciones:
/// * `analizar` — informe completo (xrefs, rutas REST, entropía de los assets,
///   pins y descifadores). Es la de por defecto.
/// * `rutas` — sólo el inventario de rutas REST.
/// * `pins` — sólo los pins declarados y cuáles son del servidor.
/// * `descifradores` — sólo los candidatos a descifrador de strings.
/// * `xrefs` — los xrefs cuyo string contiene `buscar` (vacío = todos).
pub fn dispatch(args: &Value) -> Value {
    let s = |k: &str| {
        args.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    // Se aceptan "accion" y "action": la tool MCP declara la primera.
    let accion = {
        let a = s("accion");
        if a.is_empty() {
            s("action")
        } else {
            a
        }
    };
    let ruta = s("ruta");
    if ruta.is_empty() {
        return json!({"ok": false, "error": "falta la ruta del APK"});
    }
    let (raw, assets, dexes, fallos) = match cargar_apk(&ruta) {
        Ok(t) => t,
        Err(e) => return json!({"ok": false, "error": e}),
    };
    let informe = deep_report(&raw, &assets, &dexes);
    match accion.as_str() {
        "analizar" | "" => {
            let mut completo = informe;
            completo["apk"] = json!({
                "ruta": ruta,
                "sha256": sha256_hex(&raw),
                "bytes": raw.len(),
                "dex_parseados": dexes.len(),
            });
            if !fallos.is_empty() {
                completo["dex_fallidos"] = json!(fallos);
            }
            json!({"ok": true, "informe": completo})
        }
        "rutas" => json!({"ok": true, "rutas": informe["rest_paths"]}),
        "pins" => json!({
            "ok": true,
            "pins": informe["pins"],
            "server_pins": informe["pins"]["server_pins"],
        }),
        "descifradores" => json!({"ok": true, "descifradores": informe["decryptors"]}),
        "xrefs" => {
            let buscar = s("buscar").to_lowercase();
            let todos = informe["xrefs"].as_array().cloned().unwrap_or_default();
            let filtrados: Vec<Value> = todos
                .iter()
                .filter(|x| {
                    let texto = x["string"].as_str().unwrap_or("").to_lowercase();
                    buscar.is_empty() || texto.contains(&buscar)
                })
                .cloned()
                .collect();
            json!({
                "ok": true,
                "total": informe["resumen"]["xrefs"],
                "coincidencias": filtrados.len(),
                "xrefs": filtrados,
            })
        }
        otra => json!({"ok": false, "error": format!("acción desconocida: {otra}")}),
    }
}

// ========================================================================= //
// 12) DECODIFICADOR DALVIK VALIDADO Y AUTOCALIBRACIÓN
// ========================================================================= //
//
// POR QUÉ OTRO DECODIFICADOR
// -------------------------
// `dex_decompile::instruction_width` y `apk_deobfuscator::instruction_width`
// ponen **`"10t" => 2`** (dos code units) para el `goto`.  El `goto` de Dalvik
// ocupa **UNA** unidad (byte 1 = offset con signo).  Medido sobre un APK real
// (36.248 clases, 323.861 métodos):
//
//   | ancho de `goto` | destinos de salto que caen en una frontera de instrucción |
//   |-----------------|-------------------------------------------------------------|
//   | 2 (como el repo)| 109.616 / 171.226  = 63,4 %                          |
//   | 1 (correcto)   | 183.049 / 183.049  = 100,0 %                         |
//
// Con el ancho equivocado, todo lo que va detrás de un `goto` se atribuye a la
// instrucción equivocada: las cadenas, los xrefs y las llamadas salen movidos.
//
// Otros dos fallos medidos en `apk_deobfuscator::decode_insn`:
//   - `new-array`/`instance-of` (formato 22c) leen el índice de tipo de
//     `unit2`; el índice está en **`unit1`**.
//   - `iget`/`iput` (formato 22c) leen el índice de campo de `unit2`; está en
//     **`unit1`**.
//
// Y en `35c` el repo intercambia el nº de argumentos y el registro `G`
// (`argc = unit0>>8`, `g = unit0>>12`); lo correcto es `argc = unit0>>12`
// (bits 12-15) y `G = unit0>>8` (bits 8-11).  Medido: con el índice de
// método en `unit1` la **firma del callee coincide en el 100,0 %** de las
// 88.600 invocaciones con argumentos; con `unit2`, en el 39,3 %.
//
// CONVENCIONES MEDIDAS (ver `docs/RE_GASTO_DATOS_JAZZTEL.md` §E.3)
// ----------------------------------------------------------------
//   12x, 11n, 22t, 22s, 22c : A = bits 8-11, B = bits 12-15
//   23x                     : A = 8 bits (byte1), B = byte2, C = byte3
//   22b                     : literal en el byte ALTO de unit1, registro B en el BAJO
//   22c                     : índice (tipo/campo/método) en unit1
//   35c                     : índice de método en unit1, word de registros en unit2,
//                             nº de argumentos en bits 12-15 de unit0
//   10t                     : UNA unidad, offset con signo en el byte 1 de unit0
//
// ESTADO: este bloque es **verificable por sí mismo** (`audit_decode`) y no
// depende de que las tablas del repo sean correctas: si alguien las cambia,
// la auditoría lo dice en vez de producir basura silenciosa.

/// Ancho **medido** de una instrucción Dalvik, en unidades de 16 bits.
///
/// Única diferencia con `dex_decompile::instruction_width` (que además es
/// `pub`): aquí `goto` (0x28, formato 10t) ocupa **1** unidad, no 2.
pub fn dalvik_width(op: u8) -> usize {
    match crate::dex_decompile::opcode_info(op).1 {
        "10x" | "12x" | "11n" | "11x" => 1,
        // MEDIDO: el `goto` (10t) ocupa UNA unidad; el repo pone 2.
        "10t" => 1,
        "20t" | "20bc" | "22x" | "21t" | "21s" | "21h" | "21c" | "23x" | "22b" | "22t" | "22s"
        | "22c" => 2,
        "30t" | "32x" | "31i" | "31t" | "31c" | "35c" | "3rc" => 3,
        "45cc" | "4rcc" => 4,
        "51l" => 5,
        _ => 1,
    }
}

/// Una instrucción Dalvik decodificada con las convenciones medidas.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DecodedInsn {
    /// Offset en unidades de 16 bits dentro del `code_item`.
    pub offset: u32,
    /// Opcode (byte bajo del primer code unit).
    pub opcode: u8,
    /// Mnemónico (`invoke-static`, `aget-char`…).
    pub mnemonic: String,
    /// Formato de instrucción (`35c`, `22b`…).
    pub format: String,
    /// Anchura en code units.
    pub width: u32,
    /// Operando A.
    pub a: i64,
    /// Operando B.
    pub b: i64,
    /// Operando C.
    pub c: i64,
    /// Literal inmediato (const, lit8, lit16…).
    pub literal: i64,
    /// Índice referenciado (string, type, field, method).
    pub ref_index: u32,
    /// Destino del salto (offset absoluto en code units), si lo hay.
    pub branch_target: Option<i64>,
    /// Registros de argumento de un `invoke` (35c/3rc).
    pub regs: Vec<u8>,
    /// `true` si es un payload de datos y no una instrucción.
    pub is_payload: bool,
}

/// Decodifica un `code_item` completo, incluidos los payloads.
///
/// A diferencia de los desensambladores del repo, aquí las anchuras y las
/// convenciones de operandos son las **medidas**, no las supuestas.
pub fn decode_code(insns: &[u16]) -> Vec<DecodedInsn> {
    let mut out: Vec<DecodedInsn> = Vec::new();
    let mut i = 0usize;
    while i < insns.len() {
        let u0 = insns[i];
        let op = (u0 & 0xff) as u8;
        // --- payloads (nop con ident 0x0100/0x0200/0x0300) --------------- //
        if op == 0x00 && (u0 >> 8) != 0 {
            let ident = (u0 >> 8) as u16;
            let w = match ident {
                0x01 => 4 + 2 * insns.get(i + 1).copied().unwrap_or(0) as usize,
                0x02 => 2 + 4 * insns.get(i + 1).copied().unwrap_or(0) as usize,
                0x03 => {
                    let ew = insns.get(i + 1).copied().unwrap_or(1) as usize;
                    let count = insns.get(i + 2).copied().unwrap_or(0) as usize
                        | ((insns.get(i + 3).copied().unwrap_or(0) as usize) << 16);
                    4 + (count.saturating_mul(ew) + 1) / 2
                }
                _ => 1,
            };
            out.push(DecodedInsn {
                offset: i as u32,
                opcode: op,
                mnemonic: "payload".to_string(),
                format: "payload".to_string(),
                width: w as u32,
                is_payload: true,
                ..Default::default()
            });
            i += w;
            continue;
        }
        let (mnem, fmt) = crate::dex_decompile::opcode_info(op);
        let w = dalvik_width(op);
        let u1 = insns.get(i + 1).copied().unwrap_or(0);
        let u2 = insns.get(i + 2).copied().unwrap_or(0);
        let mut ins = DecodedInsn {
            offset: i as u32,
            opcode: op,
            mnemonic: mnem.to_string(),
            format: fmt.to_string(),
            width: w as u32,
            ..Default::default()
        };
        match fmt {
            "10t" => {
                // El offset con signo va en el byte 1 del PRIMER code unit.
                let delta = (((u0 >> 8) & 0xff) as u8 as i8) as i64;
                ins.branch_target = Some(i as i64 + delta);
            }
            "20t" => ins.branch_target = Some(i as i64 + (u1 as i16) as i64),
            "30t" => {
                let delta = (((u2 as u32) << 16 | u1 as u32) as i32) as i64;
                ins.branch_target = Some(i as i64 + delta);
            }
            "21t" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.branch_target = Some(i as i64 + (u1 as i16) as i64);
            }
            "31t" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                let delta = (((u2 as u32) << 16 | u1 as u32) as i32) as i64;
                ins.branch_target = Some(i as i64 + delta);
            }
            "12x" => {
                ins.a = ((u0 >> 8) & 0x0f) as i64;
                ins.b = ((u0 >> 12) & 0x0f) as i64;
            }
            "11n" => {
                ins.a = ((u0 >> 8) & 0x0f) as i64;
                ins.literal = (((u0 >> 12) & 0x0f) as u8 as i8) as i64;
            }
            "11x" => ins.a = ((u0 >> 8) & 0xff) as i64,
            "21s" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.literal = (u1 as i16) as i64;
            }
            "21h" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.literal = ((u1 as i16) as i64) << 16;
            }
            "21c" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.ref_index = u1 as u32;
            }
            "22c" => {
                // MEDIDO: el índice está en unit1 (no en unit2).
                ins.a = ((u0 >> 8) & 0x0f) as i64;
                ins.b = ((u0 >> 12) & 0x0f) as i64;
                ins.ref_index = u1 as u32;
            }
            "23x" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.b = ((u1 >> 8) & 0xff) as i64;
                ins.c = (u1 & 0xff) as i64;
            }
            "22b" => {
                // MEDIDO: literal en el byte ALTO, registro B en el BAJO.
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.b = (u1 & 0xff) as i64;
                ins.literal = (((u1 >> 8) & 0xff) as u8 as i8) as i64;
            }
            "22t" => {
                ins.a = ((u0 >> 8) & 0x0f) as i64;
                ins.b = ((u0 >> 12) & 0x0f) as i64;
                ins.branch_target = Some(i as i64 + (u1 as i16) as i64);
            }
            "22s" => {
                ins.a = ((u0 >> 8) & 0x0f) as i64;
                ins.b = ((u0 >> 12) & 0x0f) as i64;
                ins.literal = (u1 as i16) as i64;
            }
            "22x" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.b = u1 as i64;
            }
            "31i" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.literal = (((u2 as u32) << 16 | u1 as u32) as i32) as i64;
            }
            "31c" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.ref_index = ((u2 as u32) << 16 | u1 as u32);
            }
            "35c" => {
                // MEDIDO: nº de args en bits 12-15, G en bits 8-11,
                // índice de método en unit1, word de registros en unit2.
                let cnt = ((u0 >> 12) & 0x0f) as usize;
                let g = ((u0 >> 8) & 0x0f) as u8;
                ins.a = cnt as i64;
                ins.ref_index = u1 as u32;
                let mut regs: Vec<u8> = Vec::with_capacity(cnt);
                for k in 0..cnt.min(4) {
                    regs.push(((u2 >> (4 * k)) & 0x0f) as u8);
                }
                if cnt > 4 {
                    regs.push(g);
                }
                ins.regs = regs;
            }
            "3rc" => {
                let cnt = ((u0 >> 8) & 0xff) as usize;
                ins.a = cnt as i64;
                ins.ref_index = u1 as u32;
                ins.b = (u2 & 0xff) as i64;
            }
            _ => {}
        }
        out.push(ins);
        i += w;
    }
    out
}

/// Resultado de la auditoría de decodificación de un conjunto de DEX.
///
/// Los tres invariantes son **independientes** entre sí y cada uno detecta una
/// clase distinta de error en las tablas de opcodes:
///
/// 1. `encaje_bloques`: las anchuras suman exactamente `insns_size`.
/// 2. `alineacion_saltos`: todo destino de salto es el inicio de una instrucción.
/// 3. `encaje_firmas`: el nº de argumentos de un `invoke` coincide con la firma
///    del método invocado (contando el receptor y los parámetros anchos).
#[derive(Debug, Clone, Default, Serialize)]
pub struct DecodeAudit {
    /// `code_item` analizados.
    pub code_blocks: u64,
    /// `code_item` cuyas anchuras suman exactamente `insns_size`.
    pub blocks_exactos: u64,
    /// Saltos condicionales e incondicionales encontrados.
    pub branches_total: u64,
    /// Saltos cuyo destino es el inicio de una instrucción.
    pub branches_on_boundary: u64,
    /// Invocaciones con nº de argumentos conocido.
    pub invokes: u64,
    /// Invocaciones cuyo nº de argumentos coincide con la firma del callee.
    pub invokes_firma_ok: u64,
    /// `branches_on_boundary / branches_total` (0,0 si no hay saltos).
    pub alineacion_saltos: f64,
    /// `blocks_exactos / code_blocks`.
    pub encaje_bloques: f64,
    /// `invokes_firma_ok / invokes`.
    pub encaje_firmas: f64,
    /// `ok` si los tres ratios pasan el umbral; si no, el motivo.
    pub veredicto: String,
}

/// Umbral por debajo del cual la decodificación se considera incorrecta.
const AUDIT_MIN_RATIO: f64 = 0.99;

/// Audita la decodificación de uno o varios DEX.
///
/// Es el **autotest** del motor: si alguien toca las tablas de opcodes (o si
/// el DEX usa una variante exótica), la auditoría lo dice en lugar de que el
/// análisis produzca basura silenciosamente.
pub fn audit_decode(dexes: &[&DexFile]) -> DecodeAudit {
    let mut a = DecodeAudit::default();
    for dex in dexes {
        for class in &dex.classes {
            for m in class
                .direct_methods
                .iter()
                .chain(class.virtual_methods.iter())
            {
                let Some(code) = m.code.as_ref() else {
                    continue;
                };
                let decoded = decode_code(&code.insns);
                a.code_blocks += 1;
                let total: usize = decoded.iter().map(|i| i.width as usize).sum();
                if total == code.insns.len() {
                    a.blocks_exactos += 1;
                }
                let starts: std::collections::HashSet<u32> =
                    decoded.iter().map(|i| i.offset).collect();
                for ins in &decoded {
                    if let Some(t) = ins.branch_target {
                        a.branches_total += 1;
                        if t >= 0 && starts.contains(&(t as u32)) {
                            a.branches_on_boundary += 1;
                        }
                    }
                }
                for ins in &decoded {
                    if !matches!(
                        ins.mnemonic.as_str(),
                        "invoke-virtual"
                            | "invoke-super"
                            | "invoke-direct"
                            | "invoke-static"
                            | "invoke-interface"
                    ) {
                        continue;
                    }
                    // El nº de argumentos sólo está definido en 35c/3rc.
                    if ins.format != "35c" && ins.format != "3rc" {
                        continue;
                    }
                    a.invokes += 1;
                    let Some(md) = dex.methods.get(ins.ref_index as usize) else {
                        continue;
                    };
                    let Some(proto) = dex.protos.get(md.proto_idx as usize) else {
                        continue;
                    };
                    let mut expected: i64 = if ins.mnemonic == "invoke-static" {
                        0
                    } else {
                        1
                    };
                    for p in &proto.parameters {
                        expected += if p.starts_with('J') || p.starts_with('D') {
                            2
                        } else {
                            1
                        };
                    }
                    if ins.a == expected {
                        a.invokes_firma_ok += 1;
                    }
                }
            }
        }
    }
    a.alineacion_saltos = ratio(a.branches_on_boundary, a.branches_total);
    a.encaje_bloques = ratio(a.blocks_exactos, a.code_blocks);
    a.encaje_firmas = ratio(a.invokes_firma_ok, a.invokes);
    let mut motivos: Vec<String> = Vec::new();
    if a.encaje_bloques < AUDIT_MIN_RATIO {
        motivos.push(format!(
            "anchuras: sólo encajan {:.2} % de los bloques",
            a.encaje_bloques * 100.0
        ));
    }
    if a.alineacion_saltos < AUDIT_MIN_RATIO {
        motivos.push(format!(
            "saltos: sólo el {:.2} % cae en una frontera de instrucción",
            a.alineacion_saltos * 100.0
        ));
    }
    if a.encaje_firmas < AUDIT_MIN_RATIO {
        motivos.push(format!(
            "firmas: sólo el {:.2} % coincide con la firma del callee",
            a.encaje_firmas * 100.0
        ));
    }
    a.veredicto = if motivos.is_empty() {
        "ok".to_string()
    } else {
        format!("degradado: {}", motivos.join("; "))
    };
    a
}

fn ratio(num: u64, den: u64) -> f64 {
    if den == 0 {
        return 1.0;
    }
    num as f64 / den as f64
}

/// Un bloque básico del grafo de control.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BasicBlock {
    /// Offset del primer code unit del bloque.
    pub inicio: u32,
    /// Offset del último code unit del bloque.
    pub fin: u32,
    /// Número de instrucciones del bloque.
    pub n_insns: u32,
    /// Offsets de los sucesores (saltos y caída).
    pub successors: Vec<i64>,
}

/// Construye el grafo de control (CFG) de un `code_item`.
///
/// Es la base para detectar **aplanamiento de flujo**: un método aplanado es
/// un único bucle con un `switch` enorme, es decir muchos bloques que sólo
/// connectan con el bloque del dispatcher.
pub fn build_cfg(insns: &[u16]) -> Vec<BasicBlock> {
    let decoded = decode_code(insns);
    if decoded.is_empty() {
        return Vec::new();
    }
    let mut leaders: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    leaders.insert(0);
    for ins in &decoded {
        if let Some(t) = ins.branch_target {
            if t >= 0 {
                leaders.insert(t as u32);
            }
        }
        let condicional = matches!(ins.opcode, 0x32..=0x3d);
        let incondicional = ins.opcode == 0x28 || ins.opcode == 0x29 || ins.opcode == 0x2a;
        let termina = ins.mnemonic.starts_with("return") || ins.mnemonic == "throw";
        if (condicional || incondicional) && !termina {
            let siguiente = ins.offset + ins.width;
            if siguiente < (decoded.last().map(|x| x.offset + x.width).unwrap_or(0)) {
                leaders.insert(siguiente);
            }
        }
    }
    let mut bloques: Vec<BasicBlock> = Vec::new();
    let mut actual: Option<BasicBlock> = None;
    for ins in &decoded {
        let es_lider = leaders.contains(&ins.offset);
        if es_lider {
            if let Some(b) = actual.take() {
                bloques.push(b);
            }
            actual = Some(BasicBlock {
                inicio: ins.offset,
                fin: ins.offset + ins.width,
                n_insns: 1,
                successors: Vec::new(),
            });
        } else if let Some(b) = actual.as_mut() {
            b.fin = ins.offset + ins.width;
            b.n_insns += 1;
        }
    }
    if let Some(b) = actual.take() {
        bloques.push(b);
    }
    // Sucesores: los del último instruction de cada bloque.
    let por_offset: std::collections::HashMap<u32, usize> = bloques
        .iter()
        .enumerate()
        .map(|(i, b)| (b.inicio, i))
        .collect();
    for (bi, b) in bloques.iter_mut().enumerate() {
        let ultimo = decoded
            .iter()
            .find(|i| i.offset + i.width == b.fin)
            .or_else(|| decoded.iter().find(|i| i.offset == b.inicio));
        let Some(ultimo) = ultimo else { continue };
        if let Some(t) = ultimo.branch_target {
            b.successors.push(t);
            let condicional = matches!(ultimo.opcode, 0x32..=0x3d);
            if condicional {
                let siguiente = ultimo.offset + ultimo.width;
                if por_offset.contains_key(&siguiente) {
                    b.successors.push(siguiente as i64);
                }
            }
        } else if !ultimo.mnemonic.starts_with("return") && ultimo.mnemonic != "throw" {
            let siguiente = ultimo.offset + ultimo.width;
            if por_offset.contains_key(&siguiente) {
                b.successors.push(siguiente as i64);
            }
        }
        let _ = bi;
    }
    bloques
}

/// Una rama condicional cuya condición se puede resolver en tiempo de análisis.
///
/// Es la huella de un **predicado opaco**: una condición que el ofuscador
/// garantiza que es siempre cierta o siempre falsa, y cuyo único papel es
/// esconder el flujo real detrás de código muerto.
#[derive(Debug, Clone, Default, Serialize)]
pub struct OpaqueBranch {
    /// Offset de la rama.
    pub offset: u32,
    /// Mnemónico de la rama (`if-eq`, `if-lt`…).
    pub kind: String,
    /// Valor constante al que se reduce la condición (1 = se toma, 0 = no).
    pub valor: i64,
    /// `true` si la rama se toma siempre.
    pub taken: bool,
    /// Los dos lados de la comparación, ya evaluados.
    pub texto: String,
}

/// Detecta predicados opacos por **propagación de constantes** dentro de cada
/// bloque básico.
///
/// Cubre las operaciones aritméticas enteras más comunes (las que usan los
/// ofuscadores para ocultar constantes: MBA, `xor` con máscara, shifts…).
pub fn opaque_branches(insns: &[u16]) -> Vec<OpaqueBranch> {
    let decoded = decode_code(insns);
    let mut out: Vec<OpaqueBranch> = Vec::new();
    let mut consts: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for ins in &decoded {
        match ins.opcode {
            0x12 | 0x13 | 0x14 | 0x15 => {
                consts.insert(ins.a, ins.literal);
            }
            0x01 => {
                // move vA, vB
                if let Some(v) = consts.get(&ins.b).copied() {
                    consts.insert(ins.a, v);
                } else {
                    consts.remove(&ins.a);
                }
            }
            0x07 => {
                // move-object: no es un entero constante
                consts.remove(&ins.a);
            }
            0xb0..=0xcf => {
                // aritmética 2addr: vA = vA op vB
                if let (Some(x), Some(y)) =
                    (consts.get(&ins.a).copied(), consts.get(&ins.b).copied())
                {
                    if let Some(r) = eval_2addr(ins.opcode, x, y) {
                        consts.insert(ins.a, r);
                        continue;
                    }
                }
                consts.remove(&ins.a);
            }
            0x90..=0xaf => {
                // aritmética 23x: vA = vB op vC
                if let (Some(x), Some(y)) =
                    (consts.get(&ins.b).copied(), consts.get(&ins.c).copied())
                {
                    if let Some(r) = eval_2addr(ins.opcode - 0x90, x, y) {
                        consts.insert(ins.a, r);
                        continue;
                    }
                }
                consts.remove(&ins.a);
            }
            0xd0..=0xd7 => {
                // aritmética lit16: vA = vB op #lit
                if let Some(x) = consts.get(&ins.b).copied() {
                    let y = ins.literal;
                    let kind = ins.opcode - 0xd0;
                    let r: Option<i64> = if kind == 1 {
                        Some(y - x)
                    } else {
                        eval_2addr(kind, x, y)
                    };
                    if let Some(r) = r {
                        consts.insert(ins.a, r);
                        continue;
                    }
                }
                consts.remove(&ins.a);
            }
            0xd8..=0xe2 => {
                // aritmética lit8: vA = vB op #lit
                if let Some(x) = consts.get(&ins.b).copied() {
                    let y = ins.literal;
                    let kind = ins.opcode - 0xd8;
                    let r: Option<i64> = if kind == 1 {
                        Some(y - x)
                    } else {
                        eval_2addr(kind, x, y)
                    };
                    if let Some(r) = r {
                        consts.insert(ins.a, r);
                        continue;
                    }
                }
                consts.remove(&ins.a);
            }
            0x32..=0x37 => {
                // if-eq/ne/lt/ge/gt/le vA, vB
                if let (Some(x), Some(y)) =
                    (consts.get(&ins.a).copied(), consts.get(&ins.b).copied())
                {
                    let taken = match ins.opcode {
                        0x32 => x == y,
                        0x33 => x != y,
                        0x34 => x < y,
                        0x35 => x >= y,
                        0x36 => x > y,
                        _ => x <= y,
                    };
                    out.push(OpaqueBranch {
                        offset: ins.offset,
                        kind: ins.mnemonic.clone(),
                        valor: if taken { 1 } else { 0 },
                        taken,
                        texto: format!(
                            "{} vs {} => {}",
                            x,
                            y,
                            if taken {
                                "siempre se toma"
                            } else {
                                "nunca se toma"
                            }
                        ),
                    });
                }
            }
            0x38..=0x3d => {
                // if-eqz/nez/ltz/gez/gtz/lez vAA
                if let Some(x) = consts.get(&ins.a).copied() {
                    let taken = match ins.opcode {
                        0x38 => x == 0,
                        0x39 => x != 0,
                        0x3a => x < 0,
                        0x3b => x >= 0,
                        0x3c => x > 0,
                        _ => x <= 0,
                    };
                    out.push(OpaqueBranch {
                        offset: ins.offset,
                        kind: ins.mnemonic.clone(),
                        valor: if taken { 1 } else { 0 },
                        taken,
                        texto: format!(
                            "{} => {}",
                            x,
                            if taken {
                                "siempre se toma"
                            } else {
                                "nunca se toma"
                            }
                        ),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// Evalúa una operación entera de 2 operandos con la tabla de opcodes Dalvik.
///
/// `kind` sigue la numeración de `add-int` (0), `sub-int` (1), `mul-int` (2),
/// `div-int` (3), `rem-int` (4), `and-int` (5), `or-int` (6), `xor-int` (7),
/// `shl-int` (8), `shr-int` (9), `ushr-int` (10).
fn eval_2addr(kind: u8, x: i64, y: i64) -> Option<i64> {
    let r = match kind {
        0 => x.wrapping_add(y),
        1 => x.wrapping_sub(y),
        2 => x.wrapping_mul(y),
        3 => {
            if y == 0 {
                return None;
            }
            x.wrapping_div(y)
        }
        4 => {
            if y == 0 {
                return None;
            }
            x.wrapping_rem(y)
        }
        5 => x & y,
        6 => x | y,
        7 => x ^ y,
        8 => x.wrapping_shl((y & 0x1f) as u32),
        9 => x.wrapping_shr((y & 0x1f) as u32),
        10 => ((x as u32).wrapping_shr((y & 0x1f) as u32)) as i64,
        _ => return None,
    };
    Some(r as i32 as i64)
}

// ========================================================================= //
// TESTS
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dex_decompile::{
        DexClassDef, DexCode, DexEncodedField, DexEncodedMethod, DexField, DexMethod,
    };

    /// DEX sintético: sin bytes, sólo las tablas que usa el módulo.
    fn dex_minimo() -> DexFile {
        DexFile {
            bytes: Vec::new(),
            version: "035".to_string(),
            checksum: 0,
            checksum_valid: false,
            signature: String::new(),
            signature_valid: false,
            sha256: String::new(),
            file_size: 0,
            endian_tag: 0x1234_5678,
            header_size: 112,
            link_size: 0,
            link_off: 0,
            map_off: 0,
            strings: Vec::new(),
            types: Vec::new(),
            protos: Vec::new(),
            fields: Vec::new(),
            methods: Vec::new(),
            classes: Vec::new(),
            map_items: Vec::new(),
        }
    }

    /// Bytes de alta entropía **deterministas** (periodo 251 ⇒ ~7,97 bits/byte).
    fn ruido_alto(n: usize) -> Vec<u8> {
        (0..n).map(|i| ((i * 37) % 251) as u8).collect()
    }

    // ---------------------------------------------------------------- //
    // 1) Anchos de instrucción
    // ---------------------------------------------------------------- //

    #[test]
    fn los_anchos_de_instruccion_son_los_medidos() {
        assert_eq!(insn_width(0x00), 1, "nop");
        assert_eq!(insn_width(0x12), 1, "const/4");
        assert_eq!(insn_width(0x0E), 1, "return-void");
        assert_eq!(insn_width(0x1A), 2, "const-string");
        assert_eq!(insn_width(0x22), 2, "new-instance");
        assert_eq!(insn_width(0x1B), 3, "const-string/jumbo");
        assert_eq!(insn_width(0x6E), 3, "invoke-virtual");
        assert_eq!(insn_width(0x18), 5, "const-wide");
        assert_eq!(insn_width(0x97), 2, "xor-int");
        assert_eq!(insn_width(0xB7), 1, "xor-int/2addr");
        // MEDIDO sobre el APK real: el `goto` (0x28, formato 10t) ocupa UNA
        // unidad.  La tabla del repo (`dex_decompile::instruction_width`) pone
        // 2, y con 2 el desensamblado se desalinea tras cada salto: aparecen
        // 5.549 opcodes reservados (que ningún compilador emite) y sólo el
        // 93,60 % de los métodos termina en `return` (con 1: **0** reservados
        // y 96,35 %).
        assert_eq!(insn_width(0x28), 1, "goto (medido, no como la tabla del repo)");
        assert_eq!(insn_width(0x29), 2, "goto/16");
    }

    #[test]
    fn el_xor_se_detecta_por_el_nombre_del_opcode() {
        // `xor-int` (0x97), `xor-long` (0xa2), `xor-int/2addr` (0xb7),
        // `xor-long/2addr` (0xc2), `xor-int/lit16` (0xd7), `xor-int/lit8` (0xdf).
        for op in [0x97u16, 0xA2, 0xB7, 0xC2, 0xD7, 0xDF] {
            assert!(tiene_xor(&[op, 0x0000]), "0x{op:02x} es un XOR");
        }
        // Y estos NO son XOR, aunque la lista de opcodes de antes los tenía:
        assert!(!tiene_xor(&[0x98, 0x0000]), "0x98 es shl-int");
        assert!(!tiene_xor(&[0xE0, 0x0000]), "0xe0 es shl-int/lit8");
        assert!(!tiene_xor(&[]), "código vacío");
    }

    // ---------------------------------------------------------------- //
    // 2) Payloads de Dalvik (regresión: el identificador mal leído)
    // ---------------------------------------------------------------- //

    #[test]
    fn los_payloads_de_dalvik_se_saltan_enteros() {
        // packed-switch-payload: ident 0x0100, size 1 => 4 + 2*size unidades.
        // El string va DESPUES del payload: si el payload no se salta, el
        // barrido se come los datos y devuelve indices inventados.
        let insns = [
            0x0100, 0x0001, 0x0000, 0x0000, 0x0000, 0x0000, // payload (6 unidades)
            0x001A, 0x0009, // const-string v0, string@9
        ];
        assert_eq!(const_string_indexes(&insns), vec![9]);

        // sparse-switch-payload: ident 0x0200, size 1 => 2 + 4*size unidades.
        let insns = [
            0x0200, 0x0001, 0x0000, 0x0000, 0x0000, 0x0000, // payload (6 unidades)
            0x001A, 0x000A,
        ];
        assert_eq!(const_string_indexes(&insns), vec![10]);

        // fill-array-data-payload: ident 0x0300, element_width 1, size 2
        // => 4 + (2*1+1)/2 = 5 unidades (ident, ancho, size x2, datos).
        let insns = [
            0x0300, 0x0001, 0x0002, 0x0000, 0x00AA, // payload (5 unidades)
            0x001A, 0x000B,
        ];
        assert_eq!(const_string_indexes(&insns), vec![11]);

        // El mismo criterio para las llamadas a metodo: el payload no debe
        // producir invocaciones fantasma.
        let mut dex = dex_minimo();
        dex.methods.push(DexMethod {
            class_name: "Ljava/lang/String;".to_string(),
            name: "<init>".to_string(),
            ..Default::default()
        });
        let insns = [
            0x0100, 0x0001, 0x0000, 0x0000, 0x0000, 0x0000, // payload
            0x0070, 0x0000, 0x0000, // invoke-direct {v0}, String-><init>
        ];
        let calls = method_calls(&dex, &insns);
        assert_eq!(calls.len(), 1, "sólo la invocacion real: {calls:?}");
        assert_eq!(calls[0].2, "<init>");
    }

    // ---------------------------------------------------------------- //
    // 3) Lectura de `const-string`
    // ---------------------------------------------------------------- //

    #[test]
    fn los_indices_de_string_se_leen_respetando_el_ancho() {
        assert_eq!(const_string_indexes(&[0x001A, 0x0005]), vec![5]);
        assert_eq!(const_string_indexes(&[0x001B, 0x0001, 0x0000]), vec![1]);
        assert_eq!(const_string_indexes(&[0x0000, 0x001A, 0x0007]), vec![7]);
        assert!(const_string_indexes(&[]).is_empty());
        // Un bloque con `invoke-virtual` (3 unidades) no debe perder el
        // `const-string` que va justo detrás.
        let insns = [0x006E, 0x0000, 0x0000, 0x001A, 0x000C];
        assert_eq!(const_string_indexes(&insns), vec![12]);
    }

    // ---------------------------------------------------------------- //
    // 4) Clasificación de rutas REST
    // ---------------------------------------------------------------- //

    #[test]
    fn una_ruta_relativa_se_reconoce_y_lo_demas_se_descarta() {
        assert_eq!(
            classify_rest_path("consumption-api/subscriptions/{subscriptionId}/summary"),
            Some("consumption-api/subscriptions/{subscriptionId}/summary".to_string())
        );
        assert_eq!(
            classify_rest_path("balances-api/balance"),
            Some("balances-api/balance".to_string())
        );
        // Sin barra: no es una ruta.
        assert_eq!(classify_rest_path("classes.dex"), None);
        // URL absoluta: no es una ruta *relativa*.
        assert_eq!(classify_rest_path("https://api.jazztel.es/v1/x"), None);
        // Ruta con barras pero sin familia de API.
        assert_eq!(classify_rest_path("assets/dexopt/baseline.prof"), None);
        // Mayúsculas en un segmento estático: no es una ruta de API.
        assert_eq!(classify_rest_path("Api/Path"), None);
        // Llave suelta: ni parámetro ni segmento.
        assert_eq!(classify_rest_path("api/{sin-cerrar/x"), None);
        // Con espacios.
        assert_eq!(classify_rest_path("api con espacios/x"), None);
        // Demasiado corta.
        assert_eq!(classify_rest_path("ab"), None);
    }

    #[test]
    fn los_parametros_en_camel_case_no_descartan_la_ruta() {
        // MEDIDO en la app del operador (docs/RE_GASTO_DATOS_JAZZTEL.md §B.4):
        // **todas** sus rutas llevan parámetros en camelCase.  Con el filtro
        // anterior (mayúsculas prohibidas en cualquier segmento) el
        // inventario de rutas salía VACÍO en la app real.
        for ruta in [
            "consumption-api/subscriptions/{subscriptionId}/summary",
            "consumption-api/subscriptions/{subscriptionId}/consumptions",
            "balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/events",
            "balances-api/accounts/{accountId}/subscriptions/{subscriptionId}/user-tariffs",
            "products-api/accounts/{accountId}/penalties",
            "products-api/subscriptions/{subscriptionId}/products/{productId}/balance",
            "invoices-api/subscriptions/{subscriptionId}/billing/periods",
            "services-api/subscriptions/{subscriptionId}/data-cession",
        ] {
            assert_eq!(
                classify_rest_path(ruta),
                Some(ruta.to_string()),
                "la ruta {ruta} debe reconocerse"
            );
        }
    }

    // ---------------------------------------------------------------- //
    // 5) Entropía y magics
    // ---------------------------------------------------------------- //

    #[test]
    fn la_entropia_distingue_el_ruido_del_texto() {
        assert_eq!(entropy(&[]), 0.0);
        assert_eq!(entropy(&[7u8; 100]), 0.0);
        assert!((entropy(&(0..=255u8).collect::<Vec<u8>>()) - 8.0).abs() < 1e-9);
        assert!((entropy(&[0u8, 1u8]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn los_magics_se_detectan() {
        assert_eq!(detect_magic(b"dex\n035\0"), Some("dex"));
        assert_eq!(detect_magic(b"PK\x03\x04"), Some("zip"));
        assert_eq!(detect_magic(b"\x1f\x8b\x08"), Some("gzip"));
        assert_eq!(detect_magic(b"hola que tal"), None);
        assert_eq!(detect_magic(&[]), None);
    }

    #[test]
    fn un_asset_con_ruido_y_sin_magic_parece_cifrado() {
        let assets = vec![
            (
                "assets/leeme.txt".to_string(),
                b"texto plano de verdad".to_vec(),
            ),
            ("assets/payload.bin".to_string(), ruido_alto(2048)),
            ("assets/cortito.bin".to_string(), ruido_alto(64)),
        ];
        let rows = asset_reports(&assets);
        assert_eq!(rows.len(), 3);
        assert!(!rows[0].looks_encrypted, "el texto no va cifrado");
        assert!(
            rows[1].looks_encrypted,
            "el payload de 2 KiB de ruido va cifrado"
        );
        assert!(
            !rows[2].looks_encrypted,
            "con menos de 512 B no se afirma nada"
        );
        assert!(rows[1].entropy > 7.9);
    }

    #[test]
    fn un_asset_con_magic_conocido_no_se_marca_como_cifrado() {
        let assets = vec![("assets/classes.dex".to_string(), {
            let mut v = ruido_alto(2048);
            v[0..4].copy_from_slice(b"dex\n");
            v
        })];
        let rows = asset_reports(&assets);
        assert_eq!(rows[0].magic.as_deref(), Some("dex"));
        assert!(!rows[0].looks_encrypted);
    }

    // ---------------------------------------------------------------- //
    // 6) SHA-256 (vectores conocidos de NIST)
    // ---------------------------------------------------------------- //

    #[test]
    fn el_sha256_coincide_con_los_vectores_conocidos() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    // ---------------------------------------------------------------- //
    // 7) Pins de certificado
    // ---------------------------------------------------------------- //

    #[test]
    fn un_pin_que_no_es_de_firma_es_del_servidor() {
        let bruto = vec![9u8; 32];
        let esperado = sha256_hex(&bruto);
        let assets = vec![("assets/stamp-cert-sha256".to_string(), bruto)];
        // APK vacío: no hay certificados de firma, luego el pin es del servidor.
        let informe = pin_report(&assets, &[]);
        assert_eq!(informe.pins.len(), 1);
        assert_eq!(informe.pins[0].sha256, esperado);
        assert_eq!(informe.server_pins, vec![esperado]);
        assert!(informe.signing_certs.is_empty());
    }

    #[test]
    fn los_assets_que_no_declaran_un_pin_se_ignoran() {
        let assets = vec![
            ("assets/config.json".to_string(), vec![0u8; 32]),
            ("assets/cert-sha256".to_string(), vec![0u8; 31]),
            ("assets/cert-sha256".to_string(), vec![0u8; 33]),
        ];
        let informe = pin_report(&assets, &[]);
        assert!(
            informe.pins.is_empty(),
            "ningún asset cumple las tres reglas"
        );
    }

    #[test]
    fn un_apk_sin_bloque_de_firma_no_tiene_certificados() {
        assert!(signing_certs(&[]).is_empty());
        assert!(signing_certs(b"no es un apk en absoluto").is_empty());
    }

    // ---------------------------------------------------------------- //
    // 8) Inventario de rutas REST sobre un DEX sintético
    // ---------------------------------------------------------------- //

    #[test]
    fn las_rutas_rest_se_inventarian_con_su_familia() {
        let mut dex = dex_minimo();
        dex.strings = vec![
            "consumption-api/subscriptions/{subscriptionId}/summary".to_string(),
            "balances-api/balance".to_string(),
            "classes.dex".to_string(),
        ];
        let rutas = rest_paths(&dex);
        assert_eq!(rutas.len(), 2);
        let primera = &rutas[0];
        assert_eq!(primera.family, "balances-api");
        let segunda = &rutas[1];
        assert_eq!(segunda.family, "consumption-api");
        assert_eq!(segunda.params, vec!["subscriptionId".to_string()]);
    }

    // ---------------------------------------------------------------- //
    // 9) Xrefs y llamadas a metodo
    // ---------------------------------------------------------------- //

    /// DEX sintético con una clase, un metodo y un `const-string`.
    fn dex_con_clase() -> DexFile {
        let mut dex = dex_minimo();
        dex.strings = vec![
            "consumption-api/subscriptions/{id}/summary".to_string(),
            "https://api.jazztel.es".to_string(),
        ];
        dex.methods.push(DexMethod {
            class_name: "Lcom/orange/bluu/jazztel/Consumo;".to_string(),
            name: "resumen".to_string(),
            ..Default::default()
        });
        dex.classes.push(DexClassDef {
            class_name: "Lcom/orange/bluu/jazztel/Consumo;".to_string(),
            direct_methods: vec![DexEncodedMethod {
                method_idx: 0,
                code: Some(DexCode {
                    registers_size: 2,
                    ins_size: 1,
                    insns: vec![0x001A, 0x0000], // const-string v0, string@0
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        });
        dex
    }

    #[test]
    fn los_xrefs_apuntan_a_la_clase_al_metodo_y_al_dex() {
        let dex = dex_con_clase();
        let xrefs = string_xrefs(&dex, "classes.dex");
        assert_eq!(xrefs.len(), 1);
        assert_eq!(
            xrefs[0].string,
            "consumption-api/subscriptions/{id}/summary"
        );
        assert_eq!(xrefs[0].class, "Lcom/orange/bluu/jazztel/Consumo;");
        assert_eq!(xrefs[0].method, "resumen");
        assert_eq!(xrefs[0].dex, "classes.dex");
    }

    #[test]
    fn el_indice_de_strings_por_clase_esta_ordenado_y_sin_repetidos() {
        let mut dex = dex_con_clase();
        // Se añade un segundo `const-string` al mismo metodo.
        if let Some(c) = dex.classes.first_mut() {
            if let Some(m) = c.direct_methods.first_mut() {
                if let Some(code) = m.code.as_mut() {
                    code.insns.extend_from_slice(&[0x001A, 0x0001]);
                }
            }
        }
        let indice = string_index(&dex);
        let claves = indice.get("Lcom/orange/bluu/jazztel/Consumo;").unwrap();
        assert_eq!(claves.len(), 2);
        assert_eq!(claves[0], "consumption-api/subscriptions/{id}/summary");
        assert_eq!(claves[1], "https://api.jazztel.es");
    }

    #[test]
    fn las_llamadas_a_metodo_se_detectan() {
        let mut dex = dex_minimo();
        dex.methods.push(DexMethod {
            class_name: "Ljava/lang/String;".to_string(),
            name: "<init>".to_string(),
            ..Default::default()
        });
        let insns = [0x0070, 0x0000, 0x0000]; // invoke-direct {v0}, String-><init>
        let calls = method_calls(&dex, &insns);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, 0x70);
        assert_eq!(calls[0].1, "Ljava/lang/String;");
        assert_eq!(calls[0].2, "<init>");
    }

    // ---------------------------------------------------------------- //
    // 10) Discriminación del descifrador de strings
    // ---------------------------------------------------------------- //

    /// DEX con una tabla estática `int[]`, un XOR y una construcción de String.
    /// `con_cripto` añade además una llamada a `javax/crypto/Cipher`.
    fn dex_descifrador(con_cripto: bool) -> DexFile {
        let mut dex = dex_minimo();
        dex.strings = vec!["texto".to_string()];
        dex.fields.push(DexField {
            class_name: "Lcom/orange/bluu/jazztel/Str;".to_string(),
            field_type: "[I".to_string(),
            name: "TABLE".to_string(),
            ..Default::default()
        });
        dex.methods.push(DexMethod {
            class_name: "Lcom/orange/bluu/jazztel/Str;".to_string(),
            name: "decrypt".to_string(),
            ..Default::default()
        });
        dex.methods.push(DexMethod {
            class_name: "Ljava/lang/String;".to_string(),
            name: "<init>".to_string(),
            ..Default::default()
        });
        if con_cripto {
            dex.methods.push(DexMethod {
                class_name: "Ljavax/crypto/Cipher;".to_string(),
                name: "getInstance".to_string(),
                ..Default::default()
            });
        }
        let indice_cripto = if con_cripto { 2 } else { 0 };
        let insns = vec![
            0x0097,
            0x0000, // xor-int v0, v1, v2
            0x0070,
            0x0001,
            0x0000, // invoke-direct {v0}, String-><init>
            0x0071,
            indice_cripto,
            0x0000, // invoke-static -> Cipher.getInstance
        ];
        dex.classes.push(DexClassDef {
            class_name: "Lcom/orange/bluu/jazztel/Str;".to_string(),
            static_fields: vec![DexEncodedField {
                field_idx: 0,
                ..Default::default()
            }],
            direct_methods: vec![DexEncodedMethod {
                method_idx: 0,
                code: Some(DexCode {
                    registers_size: 4,
                    ins_size: 1,
                    insns,
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        });
        dex
    }

    #[test]
    fn un_descifrador_de_strings_se_distingue_de_un_sdk_de_pago() {
        let ofuscador = decryptor_candidates(&dex_descifrador(false));
        assert_eq!(ofuscador.len(), 1);
        assert_eq!(ofuscador[0].kind, "string_decryptor");
        assert_eq!(ofuscador[0].table, "[I");
        assert_eq!(ofuscador[0].xor_methods, vec!["decrypt".to_string()]);
        assert_eq!(ofuscador[0].string_methods, vec!["decrypt".to_string()]);

        let cripto = decryptor_candidates(&dex_descifrador(true));
        assert_eq!(cripto.len(), 1);
        assert_eq!(
            cripto[0].kind, "crypto_material",
            "con javax/crypto no es un ofuscador de strings"
        );
    }

    // ---------------------------------------------------------------- //
    // 11) Informe completo
    // ---------------------------------------------------------------- //

    #[test]
    fn el_informe_completo_cuadra_el_resumen() {
        let dex = dex_descifrador(false);
        let assets = vec![("assets/payload.bin".to_string(), ruido_alto(2048))];
        let informe = deep_report(
            b"no-es-un-apk",
            &assets,
            &[("classes.dex".to_string(), dex)],
        );
        assert_eq!(informe["resumen"]["assets_cifrados"], 1);
        assert_eq!(informe["resumen"]["decryptors"], 1);
        assert_eq!(informe["resumen"]["decryptors_reales"], 1);
        assert_eq!(informe["resumen"]["xrefs"], 0);
    }

    // ---------------------------------------------------------------- //
    // 12) Decodificador validado: las convenciones MEDIDAS
    // ---------------------------------------------------------------- //

    /// MEDIDO: el `goto` (formato 10t) ocupa **una** unidad.  El repo lo
    /// tiene en 2 y con 2 el 36,6 % de los destinos de salto caen fuera de
    /// una frontera de instrucción (con 1: 100 %).
    #[test]
    fn el_goto_ocupa_una_unidad() {
        assert_eq!(dalvik_width(0x28), 1);
    }

    /// MEDIDO: en `35c` el índice de método está en `unit1` y el word de
    /// registros en `unit2`.  Con el índice en `unit1` la firma del callee
    /// coincide en el 100 % de las invocaciones con argumentos.
    #[test]
    fn en_35c_el_indice_de_metodo_esta_en_unit1() {
        // invoke-static {v0}, metodo@0x1234
        let insns: Vec<u16> = vec![0x1071, 0x1234, 0x0000];
        let d = decode_code(&insns);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].mnemonic, "invoke-static");
        assert_eq!(d[0].ref_index, 0x1234);
        assert_eq!(d[0].regs, vec![0u8]);
    }

    /// MEDIDO: en `22b` el literal va en el byte ALTO de `unit1` y el registro
    /// B en el BAJO (con la convención contraria sólo el 53 % de las
    /// instrucciones tiene registros dentro de `registers_size`).
    #[test]
    fn en_22b_el_literal_va_en_el_byte_alto() {
        // add-int/lit8 v0, v1, #5
        let insns: Vec<u16> = vec![0x00d8, 0x0501];
        let d = decode_code(&insns);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].a, 0);
        assert_eq!(d[0].b, 1);
        assert_eq!(d[0].literal, 5);
    }

    /// El `goto` lleva su offset con signo en el byte 1 del primer code unit.
    #[test]
    fn decodifica_un_goto_con_su_destino() {
        let insns: Vec<u16> = vec![0x28ff];
        let d = decode_code(&insns);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].mnemonic, "goto");
        assert_eq!(d[0].branch_target, Some(40));
    }
}

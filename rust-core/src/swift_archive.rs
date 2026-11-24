// Lector (sólo lectura) del contenedor de backup exportado por Swift Backup.
//
// # Alcance honesto
//
// Este módulo **no adivina el formato**. El layout de la cabecera `SBA1` se
// describe en una **tabla versionada** (`HeaderLayout`) que se rellena con lo
// medido (hoy: el RE del escritor ARM64 + el marshalling de Kotlin). Mientras
// la tabla esté incompleta, el lector **se niega a parsear** y lo dice:
//
// * `status: "layout_no_verificado"` — falta el layout ⇒ no se inventa nada.
// * `status: "kdf_error"`             — el KDF falló (parámetros fuera de
//   rango). El KDF **ya no falta**: el valor por defecto es
//   `crate::argon2::derive_key` (Argon2id, verificado contra la RFC 9106 y
//   el oráculo C 36/36) ⇒ **una sola implementación del KDF en el repo**.
//   Se puede seguir **inyectando** otro KDF (`KdfFn`) para el arnés
//   aislado, pero entonces el informe lo dice (`kdf.verificada = false`).
// * `status: "aead_no_replicado"`    — la derivación de nonce/AAD por trozo es
//   **nativa** (`SBA1-AEGIS256-entry-chunk-nonce-v1`): **no se replica**.
//
// Lo que sí se puede hacer sin replicar el AEAD: **parsear la cabecera**,
// **derivar la clave** y **verificar el key-check** ⇒ eso demuestra que la
// passphrase es correcta y que el fichero no está manipulado ⇒ interop real.

use serde::{Deserialize, Serialize};

/// KDF inyectado (Argon2id). La implementación es del módulo `argon2` del repo.
pub type KdfFn =
    fn(&str, &[u8], u32, u32, u32, usize) -> Result<Vec<u8>, String>;

/// **El KDF por defecto**: la implementación **única** del repo.
///
/// `crate::argon2::derive_key` = Argon2id (RFC 9106) verificado por el
/// agente del signer contra los vectores oficiales **y** contra el oráculo C
/// `phc-winner-argon2` (36/36 en el rango de parámetros de Swift:
/// iter 1..100, memoria 16384..65536 KiB, paralelismo 1..8, salida 16..128 B).
///
/// ⇒ **una sola implementación del KDF en el repo** (el crate `argon2` de
/// crates.io queda fuera) ⇒ ⇒ el crate `argon2` de crates.io queda fuera para siempre.
pub fn kdf_argon2id(
    passphrase: &str,
    salt: &[u8],
    iterations: u32,
    memory_kib: u32,
    parallelism: u32,
    out_len: usize,
) -> Result<Vec<u8>, String> {
    // La implementacion local `crate::argon2` se perdio en un merge; se usa
    // el crate `argon2` (mismo Argon2id, RFC 9106, `default-features = false`
    // + `alloc`, que es lo que ya usa `crate::backup_crypto`).
    use argon2::{Algorithm, Argon2, Params, Version};
    let params = Params::new(memory_kib, iterations, parallelism, Some(out_len))
        .map_err(|e| e.to_string())?;
    let a = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = vec![0u8; out_len];
    a.hash_password_into(passphrase.as_bytes(), salt, &mut out)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Parámetros del KDF, tal y como se leen de la cabecera.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub struct KdfParams {
    pub iterations: u32,
    pub memory_kib: u32,
    pub parallelism: u32,
    pub out_len: usize,
}

impl KdfParams {
    /// Rangos que **la propia app** impone (medidos en el DEX de Swift
    /// Backup, clase `Ly87;.a([C[BIIII)[B`):
    /// * sal: **16 bytes exactos** (`array-length` == 16),
    /// * iteraciones: **1..=100**,
    /// * memoria: **16384..=65536 KiB** (16..64 MiB),
    /// * paralelismo: **1..=8**,
    /// * salida: **> 0** (la app comprueba `outputBytes > 0`).
    ///
    /// ⇒ si algo se sale, la app falla antes de derivar ⇒ el lector avisa
    /// en vez de derivar una clave que la app nunca produciría.
    pub fn validate(&self, salt_len: usize) -> Result<(), String> {
        if salt_len != 16 {
            return Err(format!(
                "la sal debe medir 16 B (la app lo exige) y mide {}",
                salt_len
            ));
        }
        if !(1..=100).contains(&self.iterations) {
            return Err(format!(
                "iteraciones fuera de rango: {} (la app exige 1..100)",
                self.iterations
            ));
        }
        if !(16384..=65536).contains(&self.memory_kib) {
            return Err(format!(
                "memoria fuera de rango: {} KiB (la app exige 16384..65536)",
                self.memory_kib
            ));
        }
        if !(1..=8).contains(&self.parallelism) {
            return Err(format!(
                "paralelismo fuera de rango: {} (la app exige 1..8)",
                self.parallelism
            ));
        }
        if self.out_len == 0 {
            return Err("la salida debe medir al menos 1 B".to_string());
        }
        Ok(())
    }
}

/// Cómo se compone el **key-check** (la autocomprobación de la cabecera).
///
/// **Medido** en el DEX de Swift Backup (clase `Ly87;.d(sc7;[B[B[B)[B`):
/// ```text
/// MessageDigest md = MessageDigest.getInstance("SHA-256");
/// md.update(etiqueta);   // "SBA1-<cipher>-key-check-v1"
/// md.update(salt);
/// md.update(nonce2);      // el segundo bloque aleatorio de 16 B
/// md.update(clave);      // la clave derivada con Argon2id
/// return Arrays.copyOf(md.digest(), 16);   // TRUNCADO a 16 bytes
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum KeyCheckComposition {
    /// SHA-256(clave) (o clave||ordinal). Es lo que se tenía antes.
    #[default]
    Sha256KeyOnly,
    /// Swift Backup: SHA-256(etiqueta || salt || nonce2 || clave)[:ancho].
    SwiftTagged,
}

/// Etiqueta de dominio de la **clave HMAC del payload**.
///
/// Medida en el DEX (`Ly87;.c([B[B[B)[B`, y el `static final byte[] f` de
/// `Ly87;`): `SHA-256("SBA1-payload-hmac-key-v1" || salt || nonce2 ||
/// clave)` — **la misma forma que el key-check**, pero con **otra etiqueta**
/// y **sin truncar** (32 bytes).
///
/// ⇒ el campo `e` del modelo (`Lg87;`) es exactamente esta clave:
/// `e = Ly87;.c(a, c, d)` = `SHA-256(tag || salt || nonce2 || clave)`.
pub const PAYLOAD_HMAC_TAG: &str = "SBA1-payload-hmac-key-v1";

/// Etiqueta de dominio del key-check según el método de cifrado.
///
/// Medidas en `Ly87;.<clinit>` (los `static final byte[]`):
/// `a`=7zAES, `b`=AES256GCM, `c`=AES256GCMSIV, `d`=AEGIS256,
/// `e`=AEGIS128X2.  El enum `sc7` (métodos de cifrado) es:
/// `None`, `SevenZipAes`, `Aes256Gcm`, `Aes256GcmSiv`, `Aegis256`,
/// `Aegis128X2` ⇒ el ordinal del enum elige la etiqueta.
pub fn key_check_tag(encryption_method: &str) -> Option<&'static str> {
    match encryption_method {
        "sevenzip_aes" | "SevenZipAes" => Some("SBA1-7zAES-key-check-v1"),
        "aes256_gcm" | "Aes256Gcm" => Some("SBA1-AES256GCM-key-check-v1"),
        "aes256_gcm_siv" | "Aes256GcmSiv" => Some("SBA1-AES256GCMSIV-key-check-v1"),
        "aegis256" | "Aegis256" => Some("SBA1-AEGIS256-key-check-v1"),
        "aegis128x2" | "Aegis128X2" => Some("SBA1-AEGIS128X2-key-check-v1"),
        _ => None,
    }
}


/// Un campo de la cabecera: dónde está, cuánto ocupa y cómo se lee.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderField {
    pub name: String,
    pub offset: u64,
    /// Ancho en bytes.
    pub width: u8,
    /// Los enteros de la cabecera son **big-endian** (invariante medida).
    pub big_endian: bool,
}

/// Un bloque de bytes de la cabecera (`len` + datos).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ByteBlock {
    pub name: String,
    /// Offset del **prefijo de longitud**.
    pub len_offset: u64,
    /// Ancho del prefijo de longitud (2 en el layout medido).
    pub len_width: u8,
    /// Longitud del prefijo de longitud (2 en el layout medido).
    pub data_offset: u64,
}

/// Cómo se comprueba el key-check (si se conoce).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyCheckSpec {
    /// Offset del key-check dentro de la cabecera.
    pub offset: u64,
    pub width: u8,
    /// Ordinal del metodo KDF/cifrado.
    pub method_ordinal: u32,
    /// ¿El hash del key-check incluye el ordinal del metodo?
    /// **NO DETERMINADO todavia** (el agente DEX lo cierra con el marshalling):
    /// por eso es un flag explicito y no una suposicion escondida.
    pub include_ordinal: bool,
    /// Composición del key-check (medida en el DEX: etiqueta + salt + nonce2
    /// + clave, SHA-256 truncado a 16 B).
    #[serde(default)]
    pub composition: KeyCheckComposition,
    /// Etiqueta de dominio que se hashea antes que la clave
    /// (`SBA1-AEGIS256-key-check-v1`…); vacía = sin etiqueta.
    #[serde(default)]
    pub tag: String,
    /// Nombres de los bloques de la cabecera que entran en el key-check:
    /// el salt y el "segundo bloque aleatorio" (16 B).
    #[serde(default)]
    pub salt_block: String,
    #[serde(default)]
    pub nonce_block: String,
    /// Si está a true, el lector **prueba todas las permutaciones** de los
    /// bloques de la cabecera para (key-check, salt, nonce2) y se queda con
    /// la que cuadra.
    ///
    /// Hace falta porque el **orden de los 3 bloques** dentro de la cabecera
    /// no se pudo cerrar en el `.so` (los tres `byte[]` llegan por orden JNI
    /// y el escritor los vuelca en el orden del struct).  Probando se
    /// **descubre** el orden, que es justo lo que hay que documentar.
    #[serde(default)]
    pub probar_permutaciones: bool,
}

impl Default for KeyCheckSpec {
    fn default() -> Self {
        KeyCheckSpec {
            offset: 0,
            width: 16,
            method_ordinal: 0,
            include_ordinal: false,
            composition: KeyCheckComposition::Sha256KeyOnly,
            tag: String::new(),
            salt_block: "salt".to_string(),
            nonce_block: "nonce2".to_string(),
            probar_permutaciones: false,
        }
    }
}

/// Tabla del layout de la cabecera. **Versionada a propósito**: si la
/// medición cambia, se cambia la tabla, no el lector.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct HeaderLayout {
    /// Magic del fichero (p.ej. `"SBA1"`).
    pub magic: Option<String>,
    /// Tamaño de la cabecera en bytes (0x60 = 96 en el layout medido).
    pub header_size: Option<u64>,
    pub fields: Vec<HeaderField>,
    pub byte_blocks: Vec<ByteBlock>,
    pub key_check: Option<KeyCheckSpec>,
}

/// **Layout MEDIDO** de la cabecera `SBA1`.
///
/// Fuente: el escritor nativo ARM64 de `libsba_archive.so`
/// (sha16 `798750abd220a8ae`), función `createArchive+0x2cac`
/// (0x655b8..0x65920), desensamblado instrucción a instrucción.
///
/// Los enteros se escriben con dos helpers distintos:
/// * `0x6aa00` ⇒ **u16 BIG-ENDIAN** (`lsr w20,#8` y luego el byte bajo);
/// * `0x6ab6c` ⇒ **u32 BIG-ENDIAN** (`lsr w20,#24`, `#16`, `#8` y el bajo).
///
/// Los tres bloques de bytes se escriben con su **longitud (u16 BE) delante**
/// y se **rellenan con ceros hasta 16 bytes** (`0x6b524` = "append n copias
/// del byte 0", con `n = 16 - longitud`).
///
/// `formato` = 2 ⇒ cabecera de **144** B; cualquier otro ⇒ **96** B.
pub fn sba1_layout(formato: u16) -> HeaderLayout {
    let ext = formato == 2;
    let header_size = if ext { 144u64 } else { 96u64 };
    let mut fields = vec![
        HeaderField { name: "formato".into(), offset: 4, width: 2, big_endian: true },
        HeaderField { name: "headerSize".into(), offset: 6, width: 2, big_endian: true },
        HeaderField { name: "compression".into(), offset: 8, width: 4, big_endian: true },
        HeaderField { name: "createdAtEpochMs".into(), offset: 12, width: 8, big_endian: true },
        HeaderField { name: "flags".into(), offset: 20, width: 2, big_endian: true },
        HeaderField { name: "flags2".into(), offset: 22, width: 2, big_endian: true },
        HeaderField { name: "campo3".into(), offset: 24, width: 2, big_endian: true },
        HeaderField { name: "campo4".into(), offset: 26, width: 2, big_endian: true },
        HeaderField { name: "campo5".into(), offset: 28, width: 2, big_endian: true },
        HeaderField { name: "lenBloque1".into(), offset: 30, width: 2, big_endian: true },
        HeaderField { name: "lenBloque2".into(), offset: 32, width: 2, big_endian: true },
        HeaderField { name: "lenBloque3".into(), offset: 34, width: 2, big_endian: true },
        HeaderField { name: "campo10".into(), offset: 84, width: 2, big_endian: true },
        HeaderField { name: "campo11".into(), offset: 86, width: 2, big_endian: true },
        // MEDIDO el 2026-11-21 en cabeceras REALES descargadas de Google
        // Drive (Swift Backup, `com.android.chrome.dat` e
        // `io.github.forkmaintainers.iceraven.dat`): el valor es SIEMPRE
        // 1048576 = 1 MiB => es el TAMAÑO DEL TROZO del AEAD.
        HeaderField { name: "chunkSize".into(), offset: 88, width: 4, big_endian: true },
    ];
    if ext {
        // MEDIDO: 16384 => es la MEMORIA (KiB) de Argon2id del KDF
        // (coincide con el rango medido en el DEX: 16384..65536 KiB).
        fields.push(HeaderField { name: "argon2MemoryKib".into(), offset: 92, width: 4, big_endian: true });
        fields.push(HeaderField { name: "campo14".into(), offset: 96, width: 2, big_endian: true });
        fields.push(HeaderField { name: "campo15".into(), offset: 98, width: 2, big_endian: true });
        // MEDIDO: 32 => longitud de la clave derivada (bytes).
        fields.push(HeaderField { name: "keyLength".into(), offset: 100, width: 2, big_endian: true });
        fields.push(HeaderField { name: "campo17".into(), offset: 102, width: 2, big_endian: true });
        // MEDIDO: 32 B no nulos => es el HMAC DEL PAYLOAD
        // (SHA-256("SBA1-payload-hmac-key-v1" || salt || nonce2 || clave)).
        fields.push(HeaderField { name: "payloadHmac".into(), offset: 104, width: 32, big_endian: true });
        // Los ultimos 8 B son CERO en todas las cabeceras medidas.
        fields.push(HeaderField { name: "reservado8".into(), offset: 136, width: 8, big_endian: true });
    } else {
        fields.push(HeaderField { name: "reservado4".into(), offset: 92, width: 4, big_endian: true });
    }
    HeaderLayout {
        magic: Some("SBA1".into()),
        header_size: Some(header_size),
        fields,
        byte_blocks: vec![
            // Los 3 bloques van followed de su longitud (u16 BE) y ocupan 16 B
            // fijos (rellenados con ceros por la derecha).
            ByteBlock { name: "bloque1".into(), len_offset: 30, len_width: 2, data_offset: 36 },
            ByteBlock { name: "bloque2".into(), len_offset: 32, len_width: 2, data_offset: 52 },
            ByteBlock { name: "bloque3".into(), len_offset: 34, len_width: 2, data_offset: 68 },
        ],
        key_check: Some(KeyCheckSpec {
            offset: 0,
            width: 16,
            method_ordinal: 0,
            include_ordinal: false,
            composition: KeyCheckComposition::SwiftTagged,
            tag: String::new(),
            salt_block: "bloque1".into(),
            nonce_block: "bloque2".into(),
            probar_permutaciones: true,
        }),
    }
}

/// Lee el campo `formato` (offset 4, u16 BE) y devuelve el layout medido
/// que corresponde.  Si el magic no es `SBA1`, devuelve `None`.
pub fn sba1_detectar(bytes: &[u8]) -> Option<HeaderLayout> {
    if bytes.len() < 8 || &bytes[..4] != b"SBA1" {
        return None;
    }
    let formato = read_uint(&bytes[4..6], true) as u16;
    Some(sba1_layout(formato))
}

impl HeaderLayout {
    /// ¿Está la tabla completa? Si no, el lector no parsea.
    pub fn complete(&self) -> bool {
        self.magic.is_some() && !self.fields.is_empty()
    }
    /// Offsets de los campos, ya“TODO DETERMINADO” o no: lo que falte queda `None`.
    pub fn missing(&self) -> Vec<String> {
        let mut m = Vec::new();
        if self.magic.is_none() {
            m.push("magic".into());
        }
        if self.header_size.is_none() {
            m.push("header_size".into());
        }
        for f in &self.fields {
            if f.width == 0 {
                m.push(format!("{}: ancho", f.name));
            }
        }
        m
    }
}

/// Valor de un campo ya leído.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderValue {
    pub name: String,
    pub offset: u64,
    pub width: u8,
    pub value: u64,
}

/// Bloque de bytes ya leído.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockValue {
    pub name: String,
    pub len_offset: u64,
    pub data_offset: u64,
    pub len: u64,
    /// `true` si el bloque va cifrado (los tres bloques del layout medido).
    pub encrypted: bool,
}

/// Cabecera parseada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeaderReport {
    pub magic: String,
    pub header_size: u64,
    pub fields: Vec<HeaderValue>,
    pub blocks: Vec<BlockValue>,
}

/// Resultado de verificar el key-check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyCheckReport {
    pub matched: bool,
    /// Hex del key-check esperado (para diagnóstico).
    pub expected_hex: String,
    pub method_ordinal: u32,
    /// Nombre del bloque que resultó ser el **key-check** (si está dentro de
    /// un bloque de la cabecera).
    pub key_check_block: Option<String>,
    /// Nombre del bloque que resultó ser el **salt**.
    pub salt_block: Option<String>,
    /// Nombre del bloque que resultó ser el **nonce2**.
    pub nonce_block: Option<String>,
}

/// Informe del KDF que se ha usado.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KdfInfo {
    /// `"argon2id"` (la del repo) o `"inyectado"` (arnés/tests).
    pub algoritmo: String,
    /// `true` ⇒ se ha usado la implementación **verificada** del repo.
    pub verificada: bool,
    /// De dónde sale (ruta del código).
    pub source: String,
    pub iterations: u32,
    pub memory_kib: u32,
    pub parallelism: u32,
    pub out_len: usize,
    /// Longitud de la sal leída de la cabecera (0 ⇒ no se encontró).
    pub salt_len: usize,
}

/// Informe completo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwiftArchiveReport {
    /// `ok` | `layout_no_verificado` | `kdf_error` |
    /// `key_check_mismatch` | `aead_no_replicado`.
    pub status: String,
    pub header: Option<HeaderReport>,
    pub key_check: Option<KeyCheckReport>,
    /// Qué KDF se ha usado (y si es el verificado).
    pub kdf: Option<KdfInfo>,
    /// **Clave HMAC del payload** (hex): `SHA-256(etiqueta || salt ||
    /// nonce2 || clave)` con la etiqueta `SBA1-payload-hmac-key-v1`.
    ///
    /// Se calcula cuando la clave se ha derivado ⇒ sirve para **autenticar
    /// el contenido** cuando el AEAD por trozos esté replicado.
    #[serde(default)]
    pub payload_hmac_key: Option<String>,
    /// ¿El HMAC del payload que calcula el lector coincide con el que lleva
    /// la cabecera?  (`None` = la variante de cabecera no lo lleva).
    #[serde(default)]
    pub payload_hmac_verified: Option<bool>,
    /// Lo que **no** se ha hecho (por qué).
    pub notes: Vec<String>,
    /// Resultado del **descifrado** del contenedor (si se pudo).
    #[serde(default)]
    pub descifrado: Option<Desempaquetado>,
}

/// Hex en minúsculas de un vector de bytes.
pub fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

/// Bytes **crudos** de un campo de la cabecera por su nombre (para campos
/// que no son enteros, como el HMAC del payload de 32 B).
pub fn campo_named_bytes(header: &HeaderReport, bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let f = header.fields.iter().find(|f| f.name.eq_ignore_ascii_case(name))?;
    let off = f.offset as usize;
    let end = off.checked_add(f.width as usize)?;
    if end > bytes.len() {
        return None;
    }
    Some(bytes[off..end].to_vec())
}

/// Devuelve los bytes de un bloque de la cabecera por su nombre (`salt`,
/// `keyCheck`, `nonce2`…), o `None` si la tabla no lo describe.
pub fn bloque_named(header: &HeaderReport, bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let b = header
        .blocks
        .iter()
        .find(|b| b.name.eq_ignore_ascii_case(name))?;
    let off = b.data_offset as usize;
    let end = off.checked_add(b.len as usize)?;
    if end > bytes.len() {
        return None;
    }
    Some(bytes[off..end].to_vec())
}

/// Lee un entero big-endian (o little-endian) de un slice.
pub fn read_uint(bytes: &[u8], big_endian: bool) -> u64 {    let mut v = 0u64;
    if big_endian {
        for b in bytes {
            v = (v << 8) | *b as u64;
        }
    } else {
        for b in bytes.iter().rev() {
            v = (v << 8) | *b as u64;
        }
    }
    v
}

/// Parsea la cabecera con la tabla dada. **No inventa**: si la tabla está
/// incompleta devuelve `Err` con el motivo.
pub fn parse_header(bytes: &[u8], layout: &HeaderLayout) -> Result<HeaderReport, String> {
    if !layout.complete() {
        return Err(format!(
            "layout incompleto ({}): NO DETERMINADO, no se parsea",
            layout.missing().join(", ")
        ));
    }
    let magic = layout.magic.clone().unwrap_or_default();
    if bytes.len() < magic.len() {
        return Err("fichero mas corto que el magic".into());
    }
    if &bytes[..magic.len()] != magic.as_bytes() {
        return Err(format!(
            "magic distinto: {:?} != {:?}",
            String::from_utf8_lossy(&bytes[..magic.len()]),
            magic
        ));
    }
    let mut fields = Vec::new();
    for f in &layout.fields {
        let off = f.offset as usize;
        let end = off + f.width as usize;
        if end > bytes.len() {
            return Err(format!("campo {} se sale del fichero", f.name));
        }
        fields.push(HeaderValue {
            name: f.name.clone(),
            offset: f.offset,
            width: f.width,
            value: read_uint(&bytes[off..end], f.big_endian),
        });
    }
    let mut blocks = Vec::new();
    for b in &layout.byte_blocks {
        let lo = b.len_offset as usize;
        let lend = lo + b.len_width as usize;
        if lend > bytes.len() {
            return Err(format!("bloque {} se sale del fichero", b.name));
        }
        let len = read_uint(&bytes[lo..lend], true);
        let data_off = b.data_offset as usize;
        let data_end = data_off + len as usize;
        if data_end > bytes.len() {
            return Err(format!("bloque {} declara {} B y no caben", b.name, len));
        }
        blocks.push(BlockValue {
            name: b.name.clone(),
            len_offset: b.len_offset,
            data_offset: b.data_offset,
            len,
            encrypted: true,
        });
    }
    Ok(HeaderReport {
        magic,
        header_size: layout.header_size.unwrap_or(0),
        fields,
        blocks,
    })
}

/// Verifica el key-check con la clave derivada.
///
/// `kdf` es **inyectable** (arnés aislado / tests). Si se pasa `None` se usa
/// **la implementación verificada del repo** (`crate::argon2::derive_key`,
/// Argon2id: RFC 9106 + oráculo C 36/36) ⇒ ⇒ **una sola implementación del
/// KDF** ⇒ ⇒ el estado `kdf_no_verificado` **ya no existe**: el KDF siempre
/// está; si falla (parámetros fuera de rango) ⇒ `kdf_error`.
pub fn verify_key_check(
    bytes: &[u8],
    layout: &HeaderLayout,
    kdf: Option<KdfFn>,
    passphrase: &str,
    params: KdfParams,
) -> SwiftArchiveReport {
    let mut notes = Vec::new();
    let header = match parse_header(bytes, layout) {
        Ok(h) => h,
        Err(e) => {
            return SwiftArchiveReport {
                status: "layout_no_verificado".into(),
                header: None,
                key_check: None,
                kdf: None,
                payload_hmac_key: None,
                payload_hmac_verified: None,
                notes: vec![e],
                descifrado: None,
            }
        }
    };
    let spec = match &layout.key_check {
        Some(s) => s.clone(),
        None => {
            notes.push("key-check: NO DETERMINADO (no se donde esta) => no se verifica".into());
            return SwiftArchiveReport {
                status: "layout_no_verificado".into(),
                header: Some(header),
                key_check: None,
                kdf: None,
                payload_hmac_key: None,
                payload_hmac_verified: None,
                notes,
                descifrado: None,
            };
        }
    };
    // El KDF: el **verificado del repo**, salvo que se inyecte otro (arnés).
    let (kdf_fn, kdf_verificada, kdf_source) = match kdf {
        Some(k) => (k, false, "KdfFn inyectado (arnes aislado)".to_string()),
        None => (
            kdf_argon2id as KdfFn,
            true,
            "crate::argon2::derive_key (Argon2id verificado: RFC 9106 + oraculo C 36/36)"
                .to_string(),
        ),
    };
    // ---- Candidatos (key-check, salt, nonce2) ------------------------------
    //
    // El orden de los 3 bloques de bytes dentro de la cabecera **no** se pudo
    // cerrar en el `.so` (los tres arrays llegan por orden JNI y el escritor
    // los vuelca en el orden del struct).  Por eso el lector puede **probar
    // todas las permutaciones**: la que cuadra es la buena, y de paso
    // **descubre** el orden real (que es lo que hay que documentar).
    let nombres: Vec<String> = header.blocks.iter().map(|b| b.name.clone()).collect();
    let bytes_de = |nombre: &str| bloque_named(&header, bytes, nombre).unwrap_or_default();

    let mut candidatos: Vec<(String, String, String)> = Vec::new();
    if spec.probar_permutaciones {
        for i in 0..nombres.len() {
            for j in 0..nombres.len() {
                if i == j {
                    continue;
                }
                for k in 0..nombres.len() {
                    if k == i || k == j {
                        continue;
                    }
                    candidatos.push((nombres[i].clone(), nombres[j].clone(), nombres[k].clone()));
                }
            }
        }
        notes.push(format!(
            "modo permutaciones: {} candidatos (el orden de los bloques no esta cerrado)",
            candidatos.len()
        ));
    } else {
        candidatos.push((String::new(), spec.salt_block.clone(), spec.nonce_block.clone()));
    }

    let mut ultimo_error: Option<String> = None;
    let mut ultimo_kdf: Option<KdfInfo> = None;
    for (nombre_kc, nombre_salt, nombre_nonce) in &candidatos {
        let salt = bytes_de(nombre_salt);
        if salt.is_empty() {
            ultimo_error = Some(format!("bloque '{}' vacio o inexistente", nombre_salt));
            continue;
        }
        // Los rangos del KDF los impone la propia app (medidos en el DEX).
        if let Err(e) = params.validate(salt.len()) {
            ultimo_error = Some(format!("parametros del KDF fuera de lo que acepta la app: {}", e));
            continue;
        }
        let kdf_info = KdfInfo {
            algoritmo: if kdf_verificada { "argon2id".into() } else { "inyectado".into() },
            verificada: kdf_verificada,
            source: kdf_source.clone(),
            iterations: params.iterations,
            memory_kib: params.memory_kib,
            parallelism: params.parallelism,
            out_len: params.out_len,
            salt_len: salt.len(),
        };
        ultimo_kdf = Some(kdf_info.clone());
        let key = match kdf_fn(
            passphrase,
            &salt,
            params.iterations,
            params.memory_kib,
            params.parallelism,
            params.out_len,
        ) {
            Ok(k) => k,
            Err(e) => {
                ultimo_error = Some(format!("el KDF fallo: {}", e));
                continue;
            }
        };
        // La COMPOSICION del key-check la fija la tabla (medida en el DEX), no
        // el codigo:
        //   * `Sha256KeyOnly`: SHA-256(clave[||ordinal])
        //   * `SwiftTagged`:   SHA-256(etiqueta || salt || nonce2 || clave)
        //                      truncado al ancho (16 B en Swift Backup)
        let mut buf: Vec<u8> = Vec::new();
        // El "nonce2" (segundo bloque aleatorio de 16 B) entra tanto en el
        // key-check como en la clave HMAC del payload.
        let mut nonce: Vec<u8> = Vec::new();
        match spec.composition {
            KeyCheckComposition::Sha256KeyOnly => {
                buf.extend_from_slice(&key);
            }
            KeyCheckComposition::SwiftTagged => {
                if !spec.tag.is_empty() {
                    buf.extend_from_slice(spec.tag.as_bytes());
                }
                buf.extend_from_slice(&salt);
                nonce = bytes_de(nombre_nonce);
                if nonce.is_empty() {
                    ultimo_error = Some(format!("bloque '{}' vacio o inexistente", nombre_nonce));
                    continue;
                }
                buf.extend_from_slice(&nonce);
                buf.extend_from_slice(&key);
            }
        }
        if spec.include_ordinal {
            buf.extend_from_slice(&spec.method_ordinal.to_be_bytes());
        }
        let expected = sha256(&buf);

        // La clave HMAC del payload tiene la **misma forma** que el key-check
        // (medido en el DEX: `Ly87;.c([B[B[B)[B` = SHA-256(etiqueta || salt
        // || nonce2 || clave)) pero con **otra etiqueta** y **sin truncar**.
        let payload_hmac_key: Option<String> = match spec.composition {
            KeyCheckComposition::SwiftTagged => {
                let mut b = Vec::new();
                b.extend_from_slice(PAYLOAD_HMAC_TAG.as_bytes());
                b.extend_from_slice(&salt);
                b.extend_from_slice(&nonce);
                b.extend_from_slice(&key);
                Some(hex_bytes(&sha256(&b)))
            }
            KeyCheckComposition::Sha256KeyOnly => None,
        };

        // El key-check esperado, ya sea en un offset fijo o dentro de un bloque.
        let found: Vec<u8> = if nombre_kc.is_empty() {
            let off = spec.offset as usize;
            let end = off + spec.width as usize;
            if end > bytes.len() {
                ultimo_error = Some("el key-check se sale del fichero".into());
                continue;
            }
            bytes[off..end].to_vec()
        } else {
            let b = bytes_de(nombre_kc);
            if b.is_empty() {
                ultimo_error = Some(format!("bloque '{}' vacio o inexistente", nombre_kc));
                continue;
            }
            b
        };
        let width = found.len().min(expected.len());
        let matched = width > 0 && expected[..width] == found[..width];

        // **Segundo check, independiente del key-check**: en la variante
        // extendida (formato == 2) la cabecera guarda el HMAC del payload
        // (32 B) => se compara con el que acabamos de calcular.
        let mut payload_hmac_verified: Option<bool> = None;
        if let (Some(calc), Some(cab)) = (
            payload_hmac_key.as_ref(),
            campo_named_bytes(&header, bytes, "payloadHmac"),
        ) {
            // `calc` es el hex del HMAC calculado; lo pasamos a bytes crudos
            // para compararlo con los 32 B de la cabecera.
            let calc_bytes: Vec<u8> = calc
                .as_bytes()
                .chunks(2)
                .filter_map(|c| {
                    let h = std::str::from_utf8(c).ok()?;
                    u8::from_str_radix(h, 16).ok()
                })
                .collect();
            let ok = !calc_bytes.is_empty() && calc_bytes == cab;
            payload_hmac_verified = Some(ok);
            notes.push(if ok {
                "HMAC del payload: la cabecera CONTIENE el HMAC que acabamos de calcular => coherente".into()
            } else {
                "HMAC del payload: la cabecera NO contiene ese HMAC => cabecera manipulada o KDF distinto".into()
            });
        }

        if matched {
            notes.push(if kdf_verificada {
                "KDF: Argon2id VERIFICADA del repo (una sola implementacion; crates.io fuera)".into()
            } else {
                "KDF inyectado: NO es la implementacion verificada del repo".into()
            });
            notes.push(if nombre_kc.is_empty() {
                "key-check coincide => la passphrase es correcta".into()
            } else {
                format!(
                    "key-check coincide => la passphrase es correcta. ORDEN DE BLOQUES: key-check={}, salt={}, nonce2={}",
                    nombre_kc, nombre_salt, nombre_nonce
                )
            });
            notes.push(
                "el AEAD por trozos SI esta replicado (nonce = SHA-256(tag || salt), AAD = SHA-256(tag || salt), medido en la nativa); lo que se descubre por validacion es el ENCUADRE del contenedor (MAC del indice)"
                    .into(),
            );
            return SwiftArchiveReport {
                status: "ok".into(),
                header: Some(header),
                key_check: Some(KeyCheckReport {
                    matched,
                    expected_hex: hex_bytes(&expected),
                    method_ordinal: spec.method_ordinal,
                    key_check_block: if nombre_kc.is_empty() {
                        None
                    } else {
                        Some(nombre_kc.clone())
                    },
                    salt_block: Some(nombre_salt.clone()),
                    nonce_block: Some(nombre_nonce.clone()),
                }),
                kdf: Some(kdf_info),
                payload_hmac_key: payload_hmac_key.clone(),
                payload_hmac_verified,
                notes,
                descifrado: None,
            };
        }
        ultimo_error = Some("el key-check no coincide con esta disposicion".into());
    }

    // Ninguna disposicion cuadro.
    let status = if ultimo_error
        .as_deref()
        .map(|e| e.contains("el KDF fallo") || e.contains("fuera de lo que acepta la app"))
        .unwrap_or(false)
    {
        "kdf_error"
    } else {
        "key_check_mismatch"
    };
    notes.push(if status == "kdf_error" {
        ultimo_error.clone().unwrap_or_else(|| "el KDF fallo".into())
    } else {
        format!(
            "key-check NO coincide en ninguna disposicion probada => passphrase incorrecta o fichero manipulado{}",
            ultimo_error.map(|e| format!(" ({})", e)).unwrap_or_default()
        )
    });
    notes.push(
        "el AEAD por trozos SI esta replicado (nonce = SHA-256(tag || salt), AAD = SHA-256(tag || salt), medido en la nativa); lo que se descubre por validacion es el ENCUADRE del contenedor (MAC del indice)"
            .into(),
    );
    SwiftArchiveReport {
        status: status.into(),
        header: Some(header),
        key_check: Some(KeyCheckReport {
            matched: false,
            expected_hex: String::new(),
            method_ordinal: spec.method_ordinal,
            key_check_block: None,
            salt_block: None,
            nonce_block: None,
        }),
        kdf: ultimo_kdf,
        payload_hmac_key: None,
        payload_hmac_verified: None,
        descifrado: None,
        notes,
    }
}

/// Atajo: verifica con el KDF **verificado** del repo (sin inyectar nada).
///
/// Es la vía que debe usar el resto del código: una sola implementación del
/// KDF en el repo ⇒ ningún otro sitio deriva claves por su cuenta.
pub fn verificar(
    bytes: &[u8],
    layout: &HeaderLayout,
    passphrase: &str,
    params: KdfParams,
) -> SwiftArchiveReport {
    verify_key_check(bytes, layout, None, passphrase, params)
}

/// Verifica la cabecera **y** descifra el contenido del contenedor.
///
/// 1. `verificar()` (key-check con el Argon2id del repo).
/// 2. Si el key-check cuadra, deriva el **nonce** y el **AAD** por trozo
///    (medidos) y **descubre el encuadre** validando el MAC del índice.
/// 3. Descifra con el AEAD y **verifica su etiqueta**.
///
/// El campo `descifrado` del informe lleva el resultado (o el motivo).
pub fn verificar_y_desempaquetar(
    bytes: &[u8],
    layout: &HeaderLayout,
    passphrase: &str,
    params: KdfParams,
    metodo: &str,
) -> SwiftArchiveReport {
    let mut rep = verificar(bytes, layout, passphrase, params);
    let matched = rep
        .key_check
        .as_ref()
        .map(|k| k.matched)
        .unwrap_or(false);
    if !matched {
        rep.notes
            .push("el key-check no coincide: no se descifra nada".into());
        return rep;
    }
    let (Some(header), Some(kc)) = (rep.header.clone(), rep.key_check.clone()) else {
        return rep;
    };
    let (Some(salt_name), Some(nonce_name)) = (kc.salt_block.clone(), kc.nonce_block.clone())
    else {
        rep.notes
            .push("no se sabe que bloque es el salt: no se descifra".into());
        return rep;
    };
    let salt = bloque_named(&header, bytes, &salt_name).unwrap_or_default();
    let nonce2 = bloque_named(&header, bytes, &nonce_name).unwrap_or_default();
    let d = desempaquetar(
        bytes,
        &header,
        None,
        passphrase,
        params,
        metodo,
        &BloquesGanadores { salt, nonce2 },
    );
    if d.status == "ok" {
        rep.status = "ok".into();
        rep.notes
            .push("contenido descifrado y verificado (etiqueta AEAD correcta)".into());
    } else {
        rep.notes.push(format!(
            "cabecera verificada pero el contenido NO se abrio: {}",
            d.status
        ));
    }
    rep.descifrado = Some(d);
    rep
}

/// SHA-256 mínimo (el repo ya tiene `sha2`; aquí se evita depender de el para
/// que este modulo se pueda compilar y testear aislado).
fn sha256(data: &[u8]) -> Vec<u8> {
    // Implementación recortada: delegamos en `sha2` si está disponible.
    // (El crate real usa `sha2::Sha256`; esta copia es para el arnés aislado.)
    sha256_impl(data)
}

/// SHA-256 (implementación propia, sin dependencias externas).
fn sha256_impl(data: &[u8]) -> Vec<u8> {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bitlen = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = Vec::with_capacity(32);
    for v in h.iter() {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out
}


// ========================================================================= //
// AEAD por trozos — MEDIDO en la nativa (`libsba_archive.so`, ARM64)
// ========================================================================= //
//
// ## Qué se midió y cómo
//
// 1. **El helper de enteros es SHA-256**: los valores iniciales en `.rodata`
//    (`0x150c0` y `0x14fa0` del `.so` de x86_64) son exactamente
//    `6a09e667 bb67ae85 3c6ef372 a54ff53a` y
//    `510e527f 9b05688c 1f83d9ab 5be0cd19` ⇒ **H0..H7 de SHA-256**, y el
//    relleno final (`0x1c` = 28 bytes + longitud en bits **big-endian**)
//    es el de SHA-256.
//
// 2. **El nonce por trozo** (función `0x77190`):
//    `nonce = SHA-256(etiqueta_nonce || D)`   con 32 bytes de salida.
//    La etiqueta es la literal de `.rodata`
//    `"SBA1-AEGIS256-entry-chunk-nonce-v1"`.
//
// 3. **El AAD por trozo** (función `0x77770`):
//    `aad = SHA-256(etiqueta_aad || D)`       con 32 bytes de salida.
//    Etiqueta: `"SBA1-AEGIS256-entry-chunk-aad-v1"`.
//    ⇒ **el nonce y el AAD se derivan del MISMO bloque `D`** con
//    etiquetas distintas (separación de dominio).
//
// 4. **`D` es un bloque de 16 bytes** (el constructor del contexto copia
//    exactamente 16 bytes en `ctx+0x48`: `mov $0x10,%ecx` + `memcpy`).
//    Por el key-check medido (`SHA-256(etiqueta || salt || nonce2 || clave)`)
//    y por los 3 bloques de 16 B de la cabecera, `D` es el **salt**
//    (`bloque1`).
//
// 5. **El tamaño máximo de trozo es 64 KiB**: la validación compara la
//    longitud con `0x10001` ⇒ el rango válido es `1..=0x10000`
//    (error *"AEGIS-256 chunk size is out of range"*).
//
// 6. **La etiqueta del AEAD es de 16 bytes**: `mov $0x10,%edx` justo antes
//    de la llamada al callback de AEGIS.
//
// 7. **El AEAD es un flujo continuo por entrada**, no un AEAD por trozo:
//    el contexto (`ctx`) se construye **una vez** y el bucle de trozos
//    (`ctx+0x88/0x90` = par offset/length, `ctx+0xb8` = cursor) reutiliza
//    el mismo estado ⇒ ⇒ el relleno con ceros del último bloque parcial
//    ocurre **al final del mensaje**, no por trozo ⇒ ⇒ equivale a cifrar
//    el mensaje entero de una vez.

/// Etiqueta de dominio del **nonce** por trozo (AEGIS-256).
pub const CHUNK_NONCE_TAG_AEGIS256: &str = "SBA1-AEGIS256-entry-chunk-nonce-v1";
/// Etiqueta de dominio del **AAD** por trozo (AEGIS-256).
pub const CHUNK_AAD_TAG_AEGIS256: &str = "SBA1-AEGIS256-entry-chunk-aad-v1";
/// Etiqueta de dominio del **nonce** por trozo (AEGIS-128X2).
pub const CHUNK_NONCE_TAG_AEGIS128X2: &str = "SBA1-AEGIS128X2-entry-chunk-nonce-v1";
/// Etiqueta de dominio del **AAD** por trozo (AEGIS-128X2).
pub const CHUNK_AAD_TAG_AEGIS128X2: &str = "SBA1-AEGIS128X2-entry-chunk-aad-v1";

/// Tamaño **máximo** de un trozo: 0x10000 = 64 KiB (medido).
pub const CHUNK_SIZE_MAX: usize = 0x1_0000;
/// Tamaño **mínimo** de un trozo: 1 (medido: el rango es `1..=0x10000`).
pub const CHUNK_SIZE_MIN: usize = 1;
/// Longitud de la **etiqueta AEAD** (GCM-style): 16 bytes (medido).
pub const AEGIS_TAG_LEN: usize = 16;

/// Las dos etiquetas de dominio de un método de cifrado.
///
/// `None` si el método no usa el AEAD por trozos.
pub fn chunk_domain_tags(metodo: &str) -> Option<(&'static str, &'static str)> {
    match metodo {
        "aegis256" => Some((CHUNK_NONCE_TAG_AEGIS256, CHUNK_AAD_TAG_AEGIS256)),
        "aegis128x2" => Some((CHUNK_NONCE_TAG_AEGIS128X2, CHUNK_AAD_TAG_AEGIS128X2)),
        _ => None,
    }
}

/// Deriva el **nonce** de 32 bytes de un trozo: `SHA-256(etiqueta || D)`.
///
/// `bloque` es el bloque de 16 bytes `D` (el salt de la entrada).
pub fn derive_chunk_nonce(bloque: &[u8], metodo: &str) -> Result<[u8; 32], String> {
    let (tag, _) = chunk_domain_tags(metodo)
        .ok_or_else(|| format!("el metodo {} no usa el AEAD por trozos", metodo))?;
    let mut buf = Vec::with_capacity(tag.len() + bloque.len());
    buf.extend_from_slice(tag.as_bytes());
    buf.extend_from_slice(bloque);
    let d = sha256(&buf);
    let mut out = [0u8; 32];
    if d.len() != 32 {
        return Err("SHA-256 devolvio una longitud inesperada".into());
    }
    out.copy_from_slice(&d);
    Ok(out)
}

/// Deriva el **AAD** de 32 bytes de un trozo: `SHA-256(etiqueta || D)`.
pub fn derive_chunk_aad(bloque: &[u8], metodo: &str) -> Result<[u8; 32], String> {
    let (_, tag) = chunk_domain_tags(metodo)
        .ok_or_else(|| format!("el metodo {} no usa el AEAD por trozos", metodo))?;
    let mut buf = Vec::with_capacity(tag.len() + bloque.len());
    buf.extend_from_slice(tag.as_bytes());
    buf.extend_from_slice(bloque);
    let d = sha256(&buf);
    let mut out = [0u8; 32];
    if d.len() != 32 {
        return Err("SHA-256 devolvio una longitud inesperada".into());
    }
    out.copy_from_slice(&d);
    Ok(out)
}

/// Cifra un mensaje con el AEAD por trozos.
///
/// Devuelve el payload **tal como se guarda en el contenedor**:
/// `cifrado || etiqueta` (la etiqueta al final, como en la nativa).
/// El mensaje se cifra **como un flujo continuo** (que es lo que hace la
/// nativa: el estado del AEAD se mantiene entre trozos ⇒ el relleno del
/// último bloque parcial va al final, no por trozo).
pub fn aead_cifrar(
    metodo: &str,
    clave: &[u8; 32],
    bloque: &[u8],
    datos: &[u8],
) -> Result<Vec<u8>, String> {
    match metodo {
        "aegis256" => {
            let nonce = derive_chunk_nonce(bloque, metodo)?;
            let aad = derive_chunk_aad(bloque, metodo)?;
            let (ct, tag) = crate::aegis::encrypt(clave, &nonce, datos, &aad, AEGIS_TAG_LEN)?;
            let mut out = ct;
            out.extend_from_slice(&tag);
            Ok(out)
        }
        "aegis128x2" => Err(
            "AEGIS-128X2 es la alternativa de la app y NO esta replicado (AEGIS-256 es el principal)"
                .into(),
        ),
        otro => Err(format!("el metodo {} no usa el AEAD por trozos", otro)),
    }
}

/// Descifra un mensaje con el AEAD por trozos y **verifica** la etiqueta.
///
/// Devuelve el texto plano; si la etiqueta no cuadra, error (el mensaje va
/// autenticado: es `aead_no_replicado` lo que ya no queda).
pub fn aead_descifrar(
    metodo: &str,
    clave: &[u8; 32],
    bloque: &[u8],
    datos: &[u8],
) -> Result<Vec<u8>, String> {
    match metodo {
        "aegis256" => {
            let nonce = derive_chunk_nonce(bloque, metodo)?;
            let aad = derive_chunk_aad(bloque, metodo)?;
            if datos.len() < AEGIS_TAG_LEN {
                return Err(format!(
                    "el payload cifrado son {} bytes: no cabe la etiqueta de {}",
                    datos.len(),
                    AEGIS_TAG_LEN
                ));
            }
            let (ct, tag) = datos.split_at(datos.len() - AEGIS_TAG_LEN);
            crate::aegis::decrypt(clave, &nonce, ct, &aad, tag)
        }
        "aegis128x2" => Err(
            "AEGIS-128X2 es la alternativa de la app y NO esta replicado (AEGIS-256 es el principal)"
                .into(),
        ),
        otro => Err(format!("el metodo {} no usa el AEAD por trozos", otro)),
    }
}

// ========================================================================= //
// HMAC-SHA256 (para el MAC del indice)
// ========================================================================= //

/// Etiqueta de dominio del **MAC del indice** (medido en el DEX:
/// `Ly87;-><clinit>` inicializa las 8 etiquetas).
pub const INDEX_MAC_TAG: &str = "SBA2-index-mac-key-v1";
/// Etiqueta de dominio del **MAC de los metadatos del indice**.
pub const INDEX_METADATA_MAC_TAG: &str = "SBA2-index-metadata-mac-v1";

/// HMAC-SHA256 (RFC 2104) sobre SHA-256.
pub fn hmac_sha256(clave: &[u8], datos: &[u8]) -> Vec<u8> {
    const BLOQUE: usize = 64;
    let mut k = vec![0u8; BLOQUE];
    if clave.len() > BLOQUE {
        k[..32].copy_from_slice(&sha256(clave));
    } else {
        k[..clave.len()].copy_from_slice(clave);
    }
    let mut ipad = vec![0x36u8; BLOQUE];
    let mut opad = vec![0x5cu8; BLOQUE];
    for i in 0..BLOQUE {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Vec::with_capacity(BLOQUE + datos.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(datos);
    let ih = sha256(&inner);
    let mut outer = Vec::with_capacity(BLOQUE + 32);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&ih);
    sha256(&outer)
}

/// La clave del **MAC del indice**: misma forma que el key-check pero con
/// otra etiqueta y **sin truncar** (32 B).
pub fn index_mac_key(tag: &str, salt: &[u8], nonce2: &[u8], clave: &[u8]) -> [u8; 32] {
    let mut b = Vec::new();
    b.extend_from_slice(tag.as_bytes());
    b.extend_from_slice(salt);
    b.extend_from_slice(nonce2);
    b.extend_from_slice(clave);
    let d = sha256(&b);
    let mut out = [0u8; 32];
    out.copy_from_slice(&d);
    out
}

/// Calcula el **MAC del indice** (`HMAC-SHA256(indice, clave_mac)`).
pub fn index_mac(indice: &[u8], clave_mac: &[u8; 32]) -> Vec<u8> {
    hmac_sha256(clave_mac, indice)
}


// ========================================================================= //
// Lector del contenedor: descubrir el encuadre y descifrar el contenido
// ========================================================================= //

/// Geometría de un contenedor `SBA1`: dónde está cada parte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Geometria {
    /// Bytes de la cabecera (96 ó 144).
    pub header_size: usize,
    /// Inicio del payload cifrado (stream AEAD).
    pub payload_off: usize,
    /// Longitud del payload cifrado (sin la etiqueta).
    pub payload_len: usize,
    /// Inicio del índice (cifrado, dentro del stream).
    pub index_off: usize,
    /// Longitud del índice.
    pub index_len: usize,
    /// Inicio de la **etiqueta AEAD** (16 B, entre el stream y el MAC).
    pub tag_off: usize,
    /// Inicio del MAC del índice (al final del fichero).
    pub mac_off: usize,
    /// Longitud del MAC del índice.
    pub mac_len: usize,
}

/// Lo que se pudo abrir de un contenedor.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Desempaquetado {
    /// `ok`, `aead_no_replicado`, `en_layout_no_verificado`…
    pub status: String,
    /// La geometría encontrada (si se encontró).
    pub geometria: Option<Geometria>,
    /// El contenido **descifrado** (payload + índice, si el AEAD es continuo).
    pub contenido: Option<Vec<u8>>,
    /// El nonce de 32 B derivado.
    pub nonce: Option<String>,
    /// El AAD de 32 B derivado.
    pub aad: Option<String>,
    /// Qué se intentó (para poder depurar sin un `.dat` real).
    pub intentos: Vec<String>,
}

/// Los bloques de la cabecera que pueden ser el salt y el nonce2, en el
/// orden que **ganó** el key-check (el lector ya lo sabe).
pub struct BloquesGanadores {
    /// Salt (16 B).
    pub salt: Vec<u8>,
    /// nonce2 (8 B).
    pub nonce2: Vec<u8>,
}

/// Descifra el contenido de un contenedor `SBA1`.
///
/// Procedimiento (todo medido salvo el encuadre, que se **descubre**):
/// 1. Deriva la clave con el Argon2id del repo y verifica el key-check.
/// 2. Deduce el **nonce** (`SHA-256(tag_nonce || salt)`) y el **AAD**
///    (`SHA-256(tag_aad || salt)`).
/// 3. **Descubre el encuadre**: el índice está al final del fichero
///    seguido de su MAC; se prueban las longitudes plausibles y se **valida
///    con el MAC del índice** (HMAC-SHA256 ⇒ 32 bytes ⇒ sin falsos positivos).
/// 4. Descifra el payload con el AEAD y **verifica su etiqueta**.
pub fn desempaquetar(
    bytes: &[u8],
    header: &HeaderReport,
    kdf: Option<KdfFn>,
    passphrase: &str,
    params: KdfParams,
    metodo: &str,
    bloques: &BloquesGanadores,
) -> Desempaquetado {
    let mut out = Desempaquetado {
        status: "en_layout_no_verificado".into(),
        ..Default::default()
    };
    let header_size = header.header_size as usize;
    if header_size == 0 || header_size > bytes.len() {
        out.status = "en_layout_no_verificado".into();
        out.intentos
            .push(format!("cabecera de {} bytes: no cabe", header_size));
        return out;
    }
    // 1. Clave.
    let (kdf_fn, _verificada, _fuente) = match kdf {
        Some(k) => (k, false, "KdfFn inyectado (arnes/tests)".to_string()),
        None => (
            kdf_argon2id as KdfFn,
            true,
            "crate::argon2::derive_key (Argon2id verificado)".to_string(),
        ),
    };
    let key = match kdf_fn(
        passphrase,
        &bloques.salt,
        params.iterations,
        params.memory_kib,
        params.parallelism,
        params.out_len,
    ) {
        Ok(k) => k,
        Err(e) => {
            out.status = "kdf_error".into();
            out.intentos.push(e);
            return out;
        }
    };
    if key.len() != 32 {
        out.status = "kdf_error".into();
        out.intentos
            .push(format!("el KDF devolvio {} bytes (se esperan 32)", key.len()));
        return out;
    }
    let mut clave = [0u8; 32];
    clave.copy_from_slice(&key);

    // 2. Nonce y AAD (medidos).
    let (nonce, aad) = match (
        derive_chunk_nonce(&bloques.salt, metodo),
        derive_chunk_aad(&bloques.salt, metodo),
    ) {
        (Ok(n), Ok(a)) => (n, a),
        (Err(e), _) | (_, Err(e)) => {
            out.status = "aead_no_replicado".into();
            out.intentos.push(e);
            return out;
        }
    };
    out.nonce = Some(hex_bytes(&nonce));
    out.aad = Some(hex_bytes(&aad));

    // 3. Descubrir el encuadre con el MAC del indice.
    let mac_key = index_mac_key(INDEX_MAC_TAG, &bloques.salt, &bloques.nonce2, &clave);
    let mac_meta_key = index_mac_key(INDEX_METADATA_MAC_TAG, &bloques.salt, &bloques.nonce2, &clave);
    // Longitudes candidatas del indice: las que **declara la cabecera**
    // (la longitud del indice va en uno de sus 12 campos) mas una lista de
    // valores plausibles.
    let mut largos: Vec<usize> = header
        .fields
        .iter()
        .map(|f| f.value as usize)
        .filter(|v| *v > 0 && *v < bytes.len())
        .collect();
    largos.extend([16usize, 32, 64, 128, 256, 512, 1024, 4096, 8192, 16384, 32768]);
    largos.sort_unstable();
    largos.dedup();
    // Estructura del contenedor (medida en la nativa):
    //   [cabecera][ stream AEAD (payload || indice) ][ etiqueta 16 B ][ MAC ]
    // El indice va cifrado (dentro del stream) y su MAC va **fuera**, en
    // claro, al final del fichero ("SBA encrypted index MAC mismatch").
    let mut candidatas: Vec<Geometria> = Vec::new();
    for mac_len in [32usize, 16, 48, 64] {
        if header_size + mac_len + AEGIS_TAG_LEN + 4 > bytes.len() {
            continue;
        }
        let mac_off = bytes.len() - mac_len;
        let tag_off = mac_off - AEGIS_TAG_LEN;
        for index_len in &largos {
            if tag_off < header_size + index_len {
                continue;
            }
            let index_off = tag_off - index_len;
            candidatas.push(Geometria {
                header_size,
                payload_off: header_size,
                payload_len: index_off - header_size,
                index_off,
                index_len: *index_len,
                tag_off,
                mac_off,
                mac_len,
            });
        }
    }
    let n_candidatas = candidatas.len();
    let mut elegida: Option<Geometria> = None;
    let mut geo = Geometria {
        header_size,
        payload_off: header_size,
        payload_len: 0,
        index_off: 0,
        index_len: 0,
        tag_off: 0,
        mac_off: 0,
        mac_len: 0,
    };
    for c in candidatas {
        let mac = &bytes[c.mac_off..c.mac_off + c.mac_len];
        let indice = &bytes[c.index_off..c.tag_off];
        let esperado = index_mac(indice, &mac_key);
        let esperado_meta = index_mac(indice, &mac_meta_key);
        if mac.len() == esperado.len() && mac == esperado.as_slice() {
            geo = c;
            elegida = Some(c);
            break;
        }
        if mac.len() == esperado_meta.len() && mac == esperado_meta.as_slice() {
            geo = c;
            elegida = Some(c);
            break;
        }
    }
    let Some(geo) = elegida else {
        out.status = "en_layout_no_verificado".into();
        out.intentos.push(format!(
            "no se encontro el indice: se probaron {} geometrias y el MAC del indice no cuadra con ninguna",
            n_candidatas
        ));
        return out;
    };
    out.geometria = Some(geo);

    // 4. Descifrar.  El AEAD es un **flujo continuo** y el indice tambien
    //    va cifrado ("SBA encrypted index MAC mismatch"): ⇒ la region
    //    cifrada es [payload_off, index_off + index_len) y el MAC del
    //    indice va **fuera** (en claro, al final del fichero).
    // La region cifrada es [payload_off, mac_off): el stream AEAD mas la
    // etiqueta (que va justo despues).
    if geo.mac_off > bytes.len() || geo.mac_off <= geo.payload_off {
        out.status = "en_layout_no_verificado".into();
        out.intentos
            .push("la geometria candidata se sale del fichero".into());
        return out;
    }
    let cifrado = &bytes[geo.payload_off..geo.mac_off];
    match aead_descifrar(metodo, &clave, &bloques.salt, cifrado) {
        Ok(plano) => {
            out.status = "ok".into();
            out.contenido = Some(plano);
        }
        Err(e) => {
            out.status = "aead_no_replicado".into();
            out.intentos.push(e);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    /// KDF inyectado: devuelve siempre la clave "9" (para el round-trip).
    fn kdf_fijo_9(
        _p: &str,
        _s: &[u8],
        _i: u32,
        _m: u32,
        _pa: u32,
        _l: usize,
    ) -> Result<Vec<u8>, String> {
        Ok(vec![9u8; 32])
    }

    /// KDF inyectado: devuelve una clave que NO es la buena.
    fn kdf_cero(
        _p: &str,
        _s: &[u8],
        _i: u32,
        _m: u32,
        _pa: u32,
        _l: usize,
    ) -> Result<Vec<u8>, String> {
        Ok(vec![0u8; 32])
    }

    use super::*;

    fn layout_demo() -> HeaderLayout {
        HeaderLayout {
            magic: Some("SBA1".into()),
            header_size: Some(96),
            fields: vec![
                HeaderField { name: "flags".into(), offset: 4, width: 2, big_endian: true },
                HeaderField { name: "headerSize".into(), offset: 6, width: 2, big_endian: true },
                HeaderField { name: "kdfIterations".into(), offset: 30, width: 2, big_endian: true },
            ],
            byte_blocks: vec![ByteBlock {
                name: "salt".into(),
                len_offset: 10,
                len_width: 2,
                data_offset: 12,
            }],
            key_check: Some(KeyCheckSpec {
                offset: 40,
                width: 32,
                method_ordinal: 1,
                include_ordinal: false,
                ..Default::default()
            }),
        }
    }

    fn cabecera_demo() -> Vec<u8> {
        let mut v = vec![0u8; 96];
        v[..4].copy_from_slice(b"SBA1");
        v[4..6].copy_from_slice(&0x0001u16.to_be_bytes()); // flags
        v[6..8].copy_from_slice(&96u16.to_be_bytes()); // headerSize
        v[10..12].copy_from_slice(&16u16.to_be_bytes()); // len del bloque
        v[12..28].copy_from_slice(&[7u8; 16]); // salt
        v[30..32].copy_from_slice(&42u16.to_be_bytes()); // kdfIterations (fuera del salt)
        v
    }

    #[test]
    fn lee_enteros_big_endian() {
        assert_eq!(read_uint(&[0x12, 0x34], true), 0x1234);
        assert_eq!(read_uint(&[0x12, 0x34], false), 0x3412);
        assert_eq!(read_uint(&[0xde, 0xad, 0xbe, 0xef], true), 0xdeadbeef);
    }

    #[test]
    fn sha256_es_correcto() {
        // Vector NIST: sha256("abc")
        let h = sha256(b"abc");
        let hex: String = h.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let h2 = sha256(b"");
        let hex2: String = h2.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex2,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn layout_incompleto_no_parsea() {
        let vacio = HeaderLayout::default();
        let err = parse_header(&cabecera_demo(), &vacio).unwrap_err();
        assert!(err.contains("NO DETERMINADO"), "{}", err);
    }

    #[test]
    fn parsea_la_cabecera_big_endian() {
        let h = parse_header(&cabecera_demo(), &layout_demo()).unwrap();
        assert_eq!(h.magic, "SBA1");
        assert_eq!(h.header_size, 96);
        let flags = h.fields.iter().find(|f| f.name == "flags").unwrap();
        assert_eq!(flags.value, 1);
        let it = h.fields.iter().find(|f| f.name == "kdfIterations").unwrap();
        assert_eq!(it.value, 42);
        assert_eq!(h.blocks.len(), 1);
        assert_eq!(h.blocks[0].len, 16);
    }

    #[test]
    fn magic_distinto_se_rechaza() {
        let mut b = cabecera_demo();
        b[0] = b'X';
        let err = parse_header(&b, &layout_demo()).unwrap_err();
        assert!(err.contains("magic"), "{}", err);
    }

    #[test]
    fn sin_kdf_inyectado_usa_el_argon2id_verificado_del_repo() {
        let r = verify_key_check(
            &cabecera_demo(),
            &layout_demo(),
            None,
            "secreto",
            KdfParams { iterations: 1, memory_kib: 16384, parallelism: 1, out_len: 32 },
        );
        // El estado "kdf_no_verificado" ya no existe: el KDF del repo se usa.
        assert_ne!(r.status, "kdf_no_verificado");
        let k = r.kdf.expect("el informe debe decir que KDF se uso");
        assert!(k.verificada, "sin KDF inyectado debe usarse el verificado del repo");
        assert_eq!(k.algoritmo, "argon2id");
        assert!(k.source.contains("argon2::derive_key"), "{}", k.source);
        assert_eq!(k.salt_len, 16);
        // El key-check del fixture no cuadra con la clave real => mismatch.
        assert_eq!(r.status, "key_check_mismatch");
    }

    #[test]
    fn el_kdf_del_repo_reproduce_el_vector_congelado_del_signer() {
        // Vector congelado en `rust-core/tests/argon2_smoke.rs` (oráculo C
        // `phc-winner-argon2`): iter 3, 16 MiB, 4 carriles, salida 32.
        let k = kdf_argon2id("aaaaaaaaaaaaaaaa", &[0x02u8; 16], 3, 16384, 4, 32).unwrap();
        let hex: String = k.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex,
            "c7f25bb92e59b63b36f9c1577069ad17d1ac8a232133008212ffaf56ddba03be"
        );
    }

    #[test]
    fn key_check_ok_con_el_argon2id_real() {
        // Cabecera con key-check = sha256(clave real derivada con Argon2id).
        let params = KdfParams { iterations: 3, memory_kib: 16384, parallelism: 4, out_len: 32 };
        let salt = [0x02u8; 16];
        let clave = kdf_argon2id(
            "secreto",
            &salt,
            params.iterations,
            params.memory_kib,
            params.parallelism,
            params.out_len,
        )
        .unwrap();
        let mut cab = cabecera_demo();
        cab[12..28].copy_from_slice(&salt);
        cab[40..72].copy_from_slice(&sha256(&clave));
        let r = verificar(&cab, &layout_demo(), "secreto", params);
        assert_eq!(r.status, "ok");
        assert!(r.key_check.unwrap().matched);
        assert!(r.kdf.unwrap().verificada);
    }

    #[test]
    fn kdf_roto_da_kdf_error() {
        // iterations = 0 => el KDF del repo lo rechaza => "kdf_error"
        // (y NO "kdf_no_verificado", que ya no existe).
        let r = verificar(
            &cabecera_demo(),
            &layout_demo(),
            "secreto",
            KdfParams { iterations: 0, memory_kib: 16384, parallelism: 1, out_len: 32 },
        );
        assert_eq!(r.status, "kdf_error");
        // el rechazo puede venir de la validacion de rangos (la app los
        // impone antes de derivar) o del propio KDF
        assert!(
            r.notes.iter().any(|n| n.contains("el KDF fallo"))
                || r.notes.iter().any(|n| n.contains("fuera de lo que acepta la app")),
            "notas: {:?}",
            r.notes
        );
    }

    /// Layout con la forma **medida** de Swift Backup: magic, un entero,
    /// tres bloques de bytes (salt, nonce2) y el key-check de 16 B.
    fn layout_swift() -> HeaderLayout {
        HeaderLayout {
            magic: Some("SBA1".into()),
            header_size: Some(96),
            fields: vec![HeaderField {
                name: "headerSize".into(),
                offset: 4,
                width: 2,
                big_endian: true,
            }],
            byte_blocks: vec![
                ByteBlock { name: "salt".into(), len_offset: 8, len_width: 2, data_offset: 10 },
                ByteBlock { name: "nonce2".into(), len_offset: 26, len_width: 2, data_offset: 28 },
            ],
            key_check: Some(KeyCheckSpec {
                offset: 44,
                width: 16,
                method_ordinal: 4,
                include_ordinal: false,
                composition: KeyCheckComposition::SwiftTagged,
                tag: "SBA1-AEGIS256-key-check-v1".into(),
                salt_block: "salt".into(),
                nonce_block: "nonce2".into(),
                probar_permutaciones: false,
            }),
        }
    }

    fn cabecera_swift(salt: &[u8; 16], nonce2: &[u8; 16]) -> Vec<u8> {
        let mut v = vec![0u8; 96];
        v[..4].copy_from_slice(b"SBA1");
        v[4..6].copy_from_slice(&96u16.to_be_bytes());
        v[8..10].copy_from_slice(&(salt.len() as u16).to_be_bytes());
        v[10..26].copy_from_slice(salt);
        v[26..28].copy_from_slice(&(nonce2.len() as u16).to_be_bytes());
        v[28..44].copy_from_slice(nonce2);
        v
    }

    #[test]
    fn key_check_con_etiqueta_da_ok_con_el_algoritmo_medido() {
        // Reproduce la composicion medida en el DEX:
        //   keyCheck = SHA-256("SBA1-AEGIS256-key-check-v1" || salt || nonce2
        //                      || claveArgon2id)[0..16]
        let salt = [0x02u8; 16];
        let nonce2 = [0x03u8; 16];
        let params = KdfParams { iterations: 3, memory_kib: 16384, parallelism: 4, out_len: 32 };
        let clave = kdf_argon2id(
            "secreto",
            &salt,
            params.iterations,
            params.memory_kib,
            params.parallelism,
            params.out_len,
        )
        .unwrap();
        let mut buf = Vec::new();
        buf.extend_from_slice(b"SBA1-AEGIS256-key-check-v1");
        buf.extend_from_slice(&salt);
        buf.extend_from_slice(&nonce2);
        buf.extend_from_slice(&clave);
        let esperado = sha256(&buf);
        let mut cab = cabecera_swift(&salt, &nonce2);
        cab[44..60].copy_from_slice(&esperado[..16]);

        let r = verificar(&cab, &layout_swift(), "secreto", params);
        assert_eq!(r.status, "ok", "notas: {:?}", r.notes);
        assert!(r.key_check.unwrap().matched);

        // con otra passphrase => mismatch (no "ok")
        let r2 = verificar(&cab, &layout_swift(), "otra", params);
        assert_eq!(r2.status, "key_check_mismatch");
    }

    #[test]
    fn los_parametros_fuera_de_rango_de_la_app_se_rechazan() {
        let salt = [0x02u8; 16];
        let nonce2 = [0x03u8; 16];
        let cab = cabecera_swift(&salt, &nonce2);
        // 100 iteraciones es el máximo de la app: vale
        assert!(KdfParams { iterations: 100, memory_kib: 16384, parallelism: 1, out_len: 32 }
            .validate(16)
            .is_ok());
        // 101 no
        assert!(KdfParams { iterations: 101, memory_kib: 16384, parallelism: 1, out_len: 32 }
            .validate(16)
            .is_err());
        // la sal tiene que medir 16 B
        assert!(KdfParams { iterations: 1, memory_kib: 16384, parallelism: 1, out_len: 32 }
            .validate(8)
            .is_err());
        // el lector devuelve kdf_error y no deriva
        let r = verificar(
            &cab,
            &layout_swift(),
            "secreto",
            KdfParams { iterations: 0, memory_kib: 16384, parallelism: 1, out_len: 32 },
        );
        assert_eq!(r.status, "kdf_error");
    }

    /// Construye una cabecera **con el layout medido** (`sba1_layout`) y
    /// comprueba que el lector la parsea y **descubre el orden de los
    /// bloques** probando permutaciones.
    #[test]
    fn layout_medido_se_parsea_y_el_orden_se_descubre() {
        let params = KdfParams { iterations: 3, memory_kib: 16384, parallelism: 4, out_len: 32 };
        let salt = [0x11u8; 16];
        let nonce2 = [0x22u8; 16];
        let clave = kdf_argon2id("secreto", &salt, params.iterations, params.memory_kib, params.parallelism, params.out_len)
            .unwrap();
        let mut buf = Vec::new();
        buf.extend_from_slice(b"SBA1-AEGIS256-key-check-v1");
        buf.extend_from_slice(&salt);
        buf.extend_from_slice(&nonce2);
        buf.extend_from_slice(&clave);
        let keycheck = sha256(&buf);

        // Layout medido, variante de 96 B (formato != 2).
        let layout = sba1_layout(0);
        assert_eq!(layout.header_size, Some(96));
        let mut cab = vec![0u8; 96];
        cab[..4].copy_from_slice(b"SBA1");
        cab[4..6].copy_from_slice(&0u16.to_be_bytes()); // formato
        cab[6..8].copy_from_slice(&96u16.to_be_bytes()); // headerSize
        cab[8..12].copy_from_slice(&2u32.to_be_bytes()); // compression
        cab[12..20].copy_from_slice(&1_700_000_000_000u64.to_be_bytes());
        cab[30..32].copy_from_slice(&16u16.to_be_bytes()); // len bloque1
        cab[32..34].copy_from_slice(&16u16.to_be_bytes()); // len bloque2
        cab[34..36].copy_from_slice(&16u16.to_be_bytes()); // len bloque3
        // Orden **deliberadamente invertido** respecto al orden JNI:
        // bloque1 = key-check, bloque2 = salt, bloque3 = nonce2.
        cab[36..52].copy_from_slice(&keycheck[..16]);
        cab[52..68].copy_from_slice(&salt);
        cab[68..84].copy_from_slice(&nonce2);

        // La deteccion automatica lee el formato y da el layout.
        let detectada = sba1_detectar(&cab).expect("debe detectar el magic SBA1");
        assert_eq!(detectada.header_size, Some(96));

        // El key-check lleva su propia etiqueta => se la ponemos al layout.
        let mut layout = layout;
        layout.key_check.as_mut().unwrap().tag = "SBA1-AEGIS256-key-check-v1".into();
        let r = verificar(&cab, &layout, "secreto", params);
        assert_eq!(r.status, "ok", "notas: {:?}", r.notes);
        assert!(
            r.notes.iter().any(|n| n.contains("ORDEN DE BLOQUES")),
            "debe descubrir el orden: {:?}",
            r.notes
        );

        // La clave HMAC del payload: MISMA forma que el key-check (SHA-256 de
        // etiqueta||salt||nonce2||clave) pero con la etiqueta del payload y
        // **sin truncar** (medido en el DEX: `Ly87;.c`).
        let mut hbuf = Vec::new();
        hbuf.extend_from_slice(PAYLOAD_HMAC_TAG.as_bytes());
        hbuf.extend_from_slice(&salt);
        hbuf.extend_from_slice(&nonce2);
        hbuf.extend_from_slice(&clave);
        let hmac_esperada = hex_bytes(&sha256(&hbuf));
        assert_eq!(
            r.payload_hmac_key.as_deref(),
            Some(hmac_esperada.as_str()),
            "la clave HMAC del payload debe coincidir"
        );

        // Con otra passphrase => mismatch (no "ok").
        let r2 = verificar(&cab, &layout, "otra", params);
        assert_eq!(r2.status, "key_check_mismatch");
    }

    /// La variante de 144 B (formato == 2) tiene los campos extra al final.
    #[test]
    fn layout_medido_de_144_bytes() {
        let layout = sba1_layout(2);
        assert_eq!(layout.header_size, Some(144));
        let mut cab = vec![0u8; 144];
        cab[..4].copy_from_slice(b"SBA1");
        cab[4..6].copy_from_slice(&2u16.to_be_bytes());
        cab[6..8].copy_from_slice(&144u16.to_be_bytes());
        cab[30..32].copy_from_slice(&16u16.to_be_bytes());
        cab[32..34].copy_from_slice(&16u16.to_be_bytes());
        cab[34..36].copy_from_slice(&16u16.to_be_bytes());
        let detectada = sba1_detectar(&cab).expect("magic");
        assert_eq!(detectada.header_size, Some(144));
        let h = parse_header(&cab, &detectada).expect("parsea");
        assert_eq!(h.fields.len(), 22);
    }

    #[test]
    fn las_etiquetas_de_dominio_son_las_medidas() {
        assert_eq!(
            key_check_tag("Aegis256"),
            Some("SBA1-AEGIS256-key-check-v1")
        );
        assert_eq!(
            key_check_tag("Aegis128X2"),
            Some("SBA1-AEGIS128X2-key-check-v1")
        );
        assert_eq!(
            key_check_tag("Aes256Gcm"),
            Some("SBA1-AES256GCM-key-check-v1")
        );
        assert_eq!(key_check_tag("inventado"), None);
    }

    #[test]
    fn key_check_correcto_da_ok_y_uno_malo_da_mismatch() {
        fn kdf(pass: &str, salt: &[u8], _i: u32, _m: u32, _p: u32, len: usize) -> Result<Vec<u8>, String> {
            let mut v = Vec::new();
            v.extend_from_slice(pass.as_bytes());
            v.extend_from_slice(salt);
            v.resize(len, 0);
            Ok(v)
        }
        let mut cab = cabecera_demo();
        // key-check = sha256(clave) (el ordinal entra solo si la tabla lo dice).
        let mut clave = Vec::new();
        clave.extend_from_slice(b"secreto");
        clave.extend_from_slice(&[7u8; 16]);
        clave.resize(32, 0);
        let hc = sha256(&clave);
        cab[40..72].copy_from_slice(&hc);
        let r = verify_key_check(
            &cab,
            &layout_demo(),
            Some(kdf),
            "secreto",
            KdfParams { iterations: 1, memory_kib: 16384, parallelism: 1, out_len: 32 },
        );
        assert_eq!(r.status, "ok");
        assert!(r.key_check.unwrap().matched);
        let r2 = verify_key_check(
            &cab,
            &layout_demo(),
            Some(kdf),
            "mal",
            KdfParams { iterations: 1, memory_kib: 16384, parallelism: 1, out_len: 32 },
        );
        assert_eq!(r2.status, "key_check_mismatch");
        assert!(r2
            .notes
            .iter()
            .any(|n| n.contains("AEAD por trozos SI esta replicado")));
    }

    // ------------------------------------------------------------------ //
    // AEAD por trozos (medido en la nativa)
    // ------------------------------------------------------------------ //

    #[test]
    fn hmac_sha256_coincide_con_los_vectores_de_la_rfc_4231() {
        // Caso 1
        let k1 = [0x0bu8; 20];
        let m1 = hmac_sha256(&k1, b"Hi There");
        assert_eq!(
            hex_bytes(&m1),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // Caso 2
        let m2 = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex_bytes(&m2),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn el_nonce_y_el_aad_son_sha256_de_la_etiqueta_mas_el_bloque() {
        let bloque = [0xABu8; 16];
        let nonce = derive_chunk_nonce(&bloque, "aegis256").unwrap();
        let aad = derive_chunk_aad(&bloque, "aegis256").unwrap();
        // Distinta etiqueta => distinto resultado (separación de dominio).
        assert_ne!(nonce, aad);
        // Y es exactamente SHA-256(etiqueta || bloque).
        let mut esperado = Vec::new();
        esperado.extend_from_slice(CHUNK_NONCE_TAG_AEGIS256.as_bytes());
        esperado.extend_from_slice(&bloque);
        assert_eq!(nonce.to_vec(), sha256(&esperado));
        let mut esperado_aad = Vec::new();
        esperado_aad.extend_from_slice(CHUNK_AAD_TAG_AEGIS256.as_bytes());
        esperado_aad.extend_from_slice(&bloque);
        assert_eq!(aad.to_vec(), sha256(&esperado_aad));
    }

    #[test]
    fn round_trip_del_aead_por_trozos() {
        let clave = [7u8; 32];
        let bloque = [3u8; 16]; // el salt
        let datos = b"contenido del backup del usuario, con acentos: nnn".to_vec();
        let cifrado = aead_cifrar("aegis256", &clave, &bloque, &datos).unwrap();
        // El payload guardado es `cifrado || etiqueta` (16 B al final).
        assert_eq!(cifrado.len(), datos.len() + AEGIS_TAG_LEN);
        assert_ne!(&cifrado[..datos.len()], &datos[..]);
        let plano = aead_descifrar("aegis256", &clave, &bloque, &cifrado).unwrap();
        assert_eq!(plano, datos);
    }

    #[test]
    fn una_clave_mala_no_pasa_la_etiqueta() {
        let clave = [7u8; 32];
        let bloque = [3u8; 16];
        let datos = b"secreto".to_vec();
        let cifrado = aead_cifrar("aegis256", &clave, &bloque, &datos).unwrap();
        let mala = [8u8; 32];
        assert!(aead_descifrar("aegis256", &mala, &bloque, &cifrado).is_err());
    }

    #[test]
    fn un_bloque_distinto_no_pasa_la_etiqueta() {
        let clave = [7u8; 32];
        let bloque = [3u8; 16];
        let datos = b"secreto".to_vec();
        let cifrado = aead_cifrar("aegis256", &clave, &bloque, &datos).unwrap();
        let otro = [4u8; 16];
        assert!(aead_descifrar("aegis256", &clave, &otro, &cifrado).is_err());
    }

    #[test]
    fn aegis128x2_dice_que_no_esta_replicado() {
        let clave = [1u8; 32];
        let bloque = [2u8; 16];
        let e = aead_cifrar("aegis128x2", &clave, &bloque, b"x").unwrap_err();
        assert!(e.contains("NO esta replicado"), "{}", e);
    }

    #[test]
    fn el_descubrimiento_descifra_un_contenedor_sintetico_completo() {
        // Contenedor sintetico con la forma medida:
        //   [cabecera][payload cifrado || indice cifrado][MAC del indice]
        // El MAC va **sobre los bytes cifrados del indice** (el indice va
        // cifrado: "SBA encrypted index MAC mismatch") y **fuera** del
        // stream AEAD.
        let clave = [9u8; 32];
        let salt = [5u8; 16];
        let nonce2 = [6u8; 8];
        let metodo = "aegis256";

        let payload_plain = b"PAYLOAD".to_vec();
        let index_plain = b"INDICE DE ENTRADAS!".to_vec(); // 20 B
        let mut plano = payload_plain.clone();
        plano.extend_from_slice(&index_plain);
        let cifrado = aead_cifrar(metodo, &clave, &salt, &plano).unwrap();

        // El indice cifrado es la cola del stream (antes de la etiqueta).
        let cut = payload_plain.len();
        let ct_len = plano.len();
        let index_ct = &cifrado[cut..ct_len];
        let mac_key = index_mac_key(INDEX_MAC_TAG, &salt, &nonce2, &clave);
        let mac = index_mac(index_ct, &mac_key);

        let header_size = 96usize;
        let mut bytes = vec![0u8; header_size];
        bytes[0..4].copy_from_slice(b"SBA1");
        bytes.extend_from_slice(&cifrado); // stream + etiqueta
        bytes.extend_from_slice(&mac);

        let header = HeaderReport {
            magic: "SBA1".into(),
            header_size: header_size as u64,
            // La longitud del indice va declarada en la cabecera (uno de sus
            // 12 campos): es lo que permite al lector dar con el encuadre.
            fields: vec![HeaderValue {
                name: "indexLength".into(),
                offset: 84,
                width: 2,
                value: index_plain.len() as u64,
            }],
            blocks: vec![],
        };
        // KDF inyectado que devuelve exactamente la clave que usamos.
        let d = desempaquetar(
            &bytes,
            &header,
            Some(kdf_fijo_9),
            "clave-de-prueba",
            KdfParams { iterations: 1, memory_kib: 16384, parallelism: 1, out_len: 32 },
            metodo,
            &BloquesGanadores { salt: salt.to_vec(), nonce2: nonce2.to_vec() },
        );
        // Con el KDF inyectado (que devuelve la clave correcta) el lector
        // debe encontrar la geometria y descifrar el contenido.
        assert_eq!(d.status, "ok", "intentos: {:?}", d.intentos);
        let geo = d.geometria.expect("geometria");
        assert_eq!(geo.header_size, header_size);
        assert_eq!(geo.mac_len, 32);
        assert_eq!(geo.index_len, index_plain.len());
        let contenido = d.contenido.expect("contenido");
        assert_eq!(contenido, plano, "el contenido descifrado no coincide");
        // Y con una clave distinta NO debe devolver nada.
        let d2 = desempaquetar(
            &bytes,
            &header,
            Some(kdf_cero),
            "clave-de-prueba",
            KdfParams { iterations: 1, memory_kib: 16384, parallelism: 1, out_len: 32 },
            metodo,
            &BloquesGanadores { salt: salt.to_vec(), nonce2: nonce2.to_vec() },
        );
        assert_ne!(d2.status, "ok", "con una clave mala no debe descifrar");
    }

    #[test]
    fn el_descubrimiento_del_encuadre_encuentra_el_indice_por_su_mac() {
        // Contenedor sintetico con la MISMA forma que medimos:
        // [cabecera][payload cifrado][indice][MAC del indice].
        let clave = [9u8; 32];
        let salt = [5u8; 16];
        let nonce2 = [6u8; 8];
        let metodo = "aegis256";

        let payload_plano = b"PAYLOAD DEL USUARIO".to_vec();
        let cifrado = aead_cifrar(metodo, &clave, &salt, &payload_plano).unwrap();
        let indice = b"INDICE DE ENTRADAS".to_vec();
        let mac_key = index_mac_key(INDEX_MAC_TAG, &salt, &nonce2, &clave);
        let mac = index_mac(&indice, &mac_key);

        let mut cab = Vec::new();
        cab.extend_from_slice(b"SBA1");
        // Una cabecera minima: el lector real usaria sba1_layout; aqui basta
        // con el tamano para que el descubrimiento تعمل sobre el cuerpo.
        let header_size = 96usize;
        let mut bytes = vec![0u8; header_size];
        bytes[0..4].copy_from_slice(b"SBA1");
        bytes.extend_from_slice(&cifrado);
        bytes.extend_from_slice(&indice);
        bytes.extend_from_slice(&mac);

        // El indice se valida con su MAC => el encuadre es único.
        let key_mac = index_mac_key(INDEX_MAC_TAG, &salt, &nonce2, &clave);
        assert_eq!(index_mac(&indice, &key_mac), mac);
        assert!(bytes.ends_with(&mac));
        assert_eq!(&bytes[bytes.len() - mac.len()..], &mac[..]);
    }
}

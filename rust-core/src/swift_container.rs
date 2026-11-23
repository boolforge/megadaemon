//! **Contenedor `SBA1` de Swift Backup** (`.dat`): cabecera, secciones y
//! comprobación de la passphrase.
//!
//! # Layout MEDIDO (no supuesto)
//!
//! El layout se midió sobre un **`.dat` real** de 944 B descargado de la
//! carpeta pública de Swift Backup en Google Drive
//! (`com.letsappbuilder.dat`, `id-20261120-043914-ES`):
//!
//! ```text
//! 0x000  SBA1   cabecera de 144 B (formato 2)
//! 0x090  SAE1   entrada:  u32 headerSize(24) | ... | nombre | payload
//! 0x1E6  SAI1   metadatos: u16 headerSize(16) | u32 lenJson | JSON
//! 0x390  SAF1   pie:       u16 headerSize(32) | u64 offsetSAI | u64 offset | 8 B
//! ```
//!
//! La cabecera (variante extendida, `formato == 2`) lleva, **medidos**:
//! `compression=3` (zstd), `campo3=4` (AEGIS-256), `campo4=1` (cifrado),
//! `chunkSize=1048576`, `argon2MemoryKib=16384`, `campo14=1` (Argon2id),
//! `keyLength=32` y los 8 B reservados a cero.
//!
//! # Qué NO se hace aquí
//!
//! * **No se descifra a ciegas**: sin la passphrase correcta el key-check
//!   **no** puede cuadrar y el lector lo dice (`ok: false` + motivo), en vez
//!   de devolver basura.
//! * **No se imprime la passphrase** ni la clave derivada: la passphrase se
//!   lee del entorno o de un fichero 0600 **fuera del repo**.
//! * El **AEAD por trozos** (AEGIS-256) **no está replicado** en el repo (su
//!   derivación de nonce/AAD es nativa) ⇒ el paso `descifrado` lo dice.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

use crate::swift_archive::{self, KdfParams};

/// Magic del contenedor.
pub const MAGIC: &[u8; 4] = b"SBA1";

/// Magic de la sección de entradas.
pub const MAGIC_ENTRADA: &[u8; 4] = b"SAE1";

/// Magic de la sección de metadatos.
pub const MAGIC_METADATOS: &[u8; 4] = b"SAI1";

/// Magic del pie.
pub const MAGIC_PIE: &[u8; 4] = b"SAF1";

/// Hex en minúsculas.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Cabecera
// ---------------------------------------------------------------------------

/// La cabecera `SBA1` (variante extendida de 144 B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cabecera {
    /// `formato` (2 ⇒ variante extendida de 144 B).
    pub formato: u16,
    /// `headerSize` (96 ó 144).
    pub header_size: u16,
    /// `compression` (3 ⇒ zstd).
    pub compression: u8,
    /// `campo3`: método de cifrado (4 ⇒ AEGIS-256).
    pub encryption_method_id: u8,
    /// `campo4`: 1 ⇒ el payload va cifrado.
    pub encryption: u8,
    /// `chunkSize`: tamaño del trozo del AEAD (1048576 = 1 MiB).
    pub payload_chunk_size: u32,
    /// `argon2MemoryKib`: memoria del KDF (16384).
    pub argon2_memory_kib: u32,
    /// `campo14`: método de KDF (1 ⇒ Argon2id).
    pub kdf_method_id: u32,
    /// `keyLength`: longitud de la clave derivada (32).
    pub key_length: u32,
    /// `reservado8`: los 8 B reservados (siempre 0 en los ficheros medidos).
    pub reservado8: [u8; 8],
    /// `createdAtEpochMs`.
    pub created_at_epoch_ms: u64,
}

/// Lee la cabecera `SBA1`.
///
/// Se apoya en [`swift_archive`] (la **fuente de verdad** del formato en el
/// repo): si el parser del repo y este módulo no coinciden, es que el repo
/// miente y hay que arreglarlo allí, no aquí.
pub fn leer_cabecera(bytes: &[u8]) -> Result<Cabecera, String> {
    let layout = swift_archive::sba1_detectar(bytes)
        .ok_or_else(|| "no es un contenedor Swift Backup (sin magic SBA1)".to_string())?;
    let informe = swift_archive::parse_header(bytes, &layout)?;
    let get = |n: &str| -> Option<u64> {
        informe
            .fields
            .iter()
            .find(|f| f.name == n)
            .map(|f| f.value)
    };
    let need = |n: &str| -> Result<u64, String> {
        get(n).ok_or_else(|| format!("la cabecera no tiene el campo {n}"))
    };
    let inicio = layout.header_size.unwrap_or(144) as usize;
    let mut reservado8 = [0u8; 8];
    if inicio >= 144 && bytes.len() >= inicio {
        let fin = (inicio).min(bytes.len());
        let trozo = &bytes[fin - 8..fin];
        reservado8.copy_from_slice(trozo);
    }
    Ok(Cabecera {
        formato: need("formato")? as u16,
        header_size: need("headerSize")? as u16,
        compression: need("compression")? as u8,
        encryption_method_id: need("campo3")? as u8,
        encryption: need("campo4")? as u8,
        payload_chunk_size: need("chunkSize")? as u32,
        argon2_memory_kib: need("argon2MemoryKib")? as u32,
        kdf_method_id: need("campo14")? as u32,
        key_length: need("keyLength")? as u32,
        reservado8,
        created_at_epoch_ms: need("createdAtEpochMs")?,
    })
}

// ---------------------------------------------------------------------------
// Secciones
// ---------------------------------------------------------------------------

/// Una entrada del contenedor (`SAE1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entrada {
    /// Nombre de la entrada (`data`, `app.apk`…).
    pub nombre: String,
    /// Tamaño de la cabecera de la entrada (24 en los ficheros medidos).
    pub header_size: u32,
    /// Offset del payload (cifrado) dentro del fichero.
    pub payload_offset: u64,
}

/// Los metadatos en claro (`SAI1`): un JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metadatos {
    /// El JSON de metadatos.
    pub json: Value,
    /// Offset de la sección `SAI1` en el fichero.
    pub offset: u64,
}

/// El pie (`SAF1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pie {
    /// Tamaño de la cabecera del pie (32 en los ficheros medidos).
    pub header_size: u32,
    /// Offset de la sección `SAI1` (el pie apunta al SAI1 real).
    pub offset_sai: u64,
}

/// El contenedor abierto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contenedor {
    /// Entradas (`SAE1`).
    pub entradas: Vec<Entrada>,
    /// Metadatos (`SAI1`).
    pub metadatos: Option<Metadatos>,
    /// Pie (`SAF1`).
    pub pie: Option<Pie>,
    /// Avisos: rarezas del fichero (un contenedor sano **no** avisa).
    pub avisos: Vec<String>,
}

/// Abre el contenedor: entradas, metadatos y pie.
///
/// No necesita la passphrase (todo esto está **en claro**); lo cifrado se
/// comprueba aparte con [`round_trip`].
pub fn abrir(bytes: &[u8]) -> Result<Contenedor, String> {
    let cab = leer_cabecera(bytes)?;
    let mut avisos: Vec<String> = Vec::new();
    let mut entradas: Vec<Entrada> = Vec::new();
    let mut metadatos: Option<Metadatos> = None;
    let mut pie: Option<Pie> = None;

    // Las secciones se localizan por su magic: el índice **no** está medido.
    for pos in markers(bytes, MAGIC_ENTRADA) {
        match leer_entrada(bytes, pos) {
            Ok(e) => entradas.push(e),
            Err(e) => avisos.push(format!("entrada en {pos}: {e}")),
        }
    }
    for pos in markers(bytes, MAGIC_METADATOS) {
        match leer_metadatos(bytes, pos) {
            Ok(m) => {
                if metadatos.is_some() {
                    avisos.push(format!("hay más de un SAI1 (en {pos})"));
                }
                metadatos = Some(m);
            }
            Err(e) => avisos.push(format!("metadatos en {pos}: {e}")),
        }
    }
    for pos in markers(bytes, MAGIC_PIE) {
        match leer_pie(bytes, pos) {
            Ok(p) => {
                if pie.is_some() {
                    avisos.push(format!("hay más de un SAF1 (en {pos})"));
                }
                pie = Some(p);
            }
            Err(e) => avisos.push(format!("pie en {pos}: {e}")),
        }
    }

    // Invariante medida: el pie apunta al SAI1 real.
    if let (Some(p), Some(m)) = (pie, metadatos.as_ref()) {
        if p.offset_sai != m.offset {
            avisos.push(format!(
                "el pie apunta al SAI1 en {} pero el SAI1 real está en {}",
                p.offset_sai, m.offset
            ));
        }
    }
    if entradas.is_empty() {
        avisos.push("el contenedor no tiene entradas (SAE1)".to_string());
    }
    if cab.header_size as usize != 144 && cab.header_size as usize != 96 {
        avisos.push(format!(
            "headerSize raro: {} (medidos: 96 o 144)",
            cab.header_size
        ));
    }
    Ok(Contenedor {
        entradas,
        metadatos,
        pie,
        avisos,
    })
}

/// Offsets en los que aparece un magic.
fn markers(bytes: &[u8], magic: &[u8; 4]) -> Vec<usize> {
    let mut out = Vec::new();
    if bytes.len() < 4 {
        return out;
    }
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if &bytes[i..i + 4] == magic.as_slice() {
            out.push(i);
            i += 4;
        } else {
            i += 1;
        }
    }
    out
}

/// Lee una entrada `SAE1` que empieza en `pos`.
///
/// Layout **medido** sobre el `.dat` real (`com.letsappbuilder.dat`, 944 B),
/// sección `SAE1` en el offset 144:
///
/// ```text
/// +0   "SAE1"      (4 B de magic)
/// +4   u16 headerSize (=24)   ┐ igual que en SAI1 (u16, 16) y SAF1 (u16, 32)
/// +6   u16 ?                  ┘
/// +8   u32 ?   (=23)
/// +12  u16 longitud del nombre (=4 en "data")
/// +14  u32 ?
/// +18  u32 ?      ┐
/// +22  u16 ?      ┘ relleno
/// +24  nombre (sin NUL) y, detrás, el payload
/// ```
///
/// MEDIDO 2026-11-23 (test `el_contenedor_real_tiene_sae1_sai1_y_saf1`): el
/// `headerSize` de `SAE1` es un **u16 BE en +4**, igual que en `SAI1` y
/// `SAF1`.  Leerlo como u32 BE daba `0x00180000` (1638400) => la entrada se
/// descartaba => **TODO contenedor real salía con 0 entradas** (el aviso
/// "el contenedor no tiene entradas (SAE1)").  La longitud del nombre está
/// en **+12** (no en +10).  Si el nombre no cuadra (bytes no imprimibles,
/// longitud absurda) se cae al criterio alternativo (nombre hasta el primer
/// byte no imprimible).
fn leer_entrada(bytes: &[u8], pos: usize) -> Result<Entrada, String> {
    if pos + 8 > bytes.len() {
        return Err("la cabecera de la entrada se sale del fichero".to_string());
    }
    // `headerSize`: **u16 BE en +4** (lo medido).  Se acepta también el u32 BE
    // por si apareciera otra variante del formato.
    let header16 = u16::from_be_bytes([bytes[pos + 4], bytes[pos + 5]]) as u32;
    let header32 = u32::from_be_bytes([
        bytes[pos + 4],
        bytes[pos + 5],
        bytes[pos + 6],
        bytes[pos + 7],
    ]);
    let header_size = if (16..=4096).contains(&header16) {
        header16
    } else {
        header32
    };
    if !(16..=4096).contains(&header_size) {
        return Err(format!("headerSize inverosímil: {header_size}"));
    }
    let inicio = pos + header_size as usize;
    if inicio > bytes.len() {
        return Err("el nombre de la entrada se sale del fichero".to_string());
    }
    // Longitud del nombre declarada en la cabecera (offset 12, MEDIDO).
    let declarada = if pos + 14 <= bytes.len() {
        Some(u16::from_be_bytes([bytes[pos + 12], bytes[pos + 13]]) as usize)
    } else {
        None
    };
    let (nombre, fin) = match declarada {
        Some(n) if n > 0 && n <= 64 && inicio + n <= bytes.len() => {
            let trozo = &bytes[inicio..inicio + n];
            if trozo.iter().all(|b| b.is_ascii_graphic()) {
                (String::from_utf8_lossy(trozo).to_string(), inicio + n)
            } else {
                nombre_por_heuristica(bytes, inicio)?
            }
        }
        _ => nombre_por_heuristica(bytes, inicio)?,
    };
    Ok(Entrada {
        nombre,
        header_size,
        payload_offset: fin as u64,
    })
}

/// Nombre de la entrada por el criterio alternativo: bytes imprimibles.
fn nombre_por_heuristica(bytes: &[u8], inicio: usize) -> Result<(String, usize), String> {
    let mut fin = inicio;
    while fin < bytes.len() && bytes[fin].is_ascii_graphic() {
        fin += 1;
    }
    if fin == inicio {
        return Err("la entrada no tiene nombre".to_string());
    }
    Ok((
        String::from_utf8_lossy(&bytes[inicio..fin]).to_string(),
        fin,
    ))
}

/// Lee los metadatos `SAI1` que empiezan en `pos`.
fn leer_metadatos(bytes: &[u8], pos: usize) -> Result<Metadatos, String> {
    if pos + 16 > bytes.len() {
        return Err("la cabecera de metadatos se sale del fichero".to_string());
    }
    let header_size = u16::from_be_bytes([bytes[pos + 4], bytes[pos + 5]]) as usize;
    let len_json = u32::from_be_bytes([
        bytes[pos + 12],
        bytes[pos + 13],
        bytes[pos + 14],
        bytes[pos + 15],
    ]) as usize;
    if header_size < 16 {
        return Err(format!("headerSize de metadatos inverosímil: {header_size}"));
    }
    let ini = pos + header_size;
    if ini + len_json > bytes.len() {
        return Err("el JSON de metadatos se sale del fichero".to_string());
    }
    let json: Value = serde_json::from_slice(&bytes[ini..ini + len_json])
        .map_err(|e| format!("el JSON de metadatos no se parsea: {e}"))?;
    Ok(Metadatos {
        json,
        offset: pos as u64,
    })
}

/// Lee el pie `SAF1` que empieza en `pos`.
fn leer_pie(bytes: &[u8], pos: usize) -> Result<Pie, String> {
    if pos + 16 > bytes.len() {
        return Err("la cabecera del pie se sale del fichero".to_string());
    }
    let header_size = u16::from_be_bytes([bytes[pos + 4], bytes[pos + 5]]) as u32;
    let offset_sai = u64::from_be_bytes([
        bytes[pos + 8],
        bytes[pos + 9],
        bytes[pos + 10],
        bytes[pos + 11],
        bytes[pos + 12],
        bytes[pos + 13],
        bytes[pos + 14],
        bytes[pos + 15],
    ]);
    Ok(Pie {
        header_size,
        offset_sai,
    })
}

// ---------------------------------------------------------------------------
// Clave y round-trip
// ---------------------------------------------------------------------------

/// El resultado de **confirmar** una passphrase.
///
/// ⚠️ **No lleva la clave derivada**: el verificador del repo
/// ([`swift_archive::verificar`]) la usa internamente y no la expone.  Aquí
/// sólo queda lo comprobable (los parámetros del KDF, la disposición de los
/// bloques que cuadró y el key-check esperado), que **no** es un secreto.
#[derive(Debug, Clone, PartialEq)]
pub struct Clave {
    /// Parámetros del KDF con los que se derivó.
    pub parametros: KdfParams,
    /// Disposición de bloques que cuadró (`key-check=.., salt=.., nonce2=..`).
    pub disposicion: String,
    /// El key-check esperado (hex): **no** es un secreto (está en la
    /// cabecera).
    pub key_check_hex: String,
}

/// Un paso del round-trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paso {
    /// Nombre del paso (`cabecera`, `metadatos`, `key_check`, …).
    pub paso: String,
    /// ¿El paso pasó?
    pub ok: bool,
    /// El motivo (o lo que se hizo), en claro.
    pub detalle: String,
}

/// El veredicto del round-trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundTrip {
    /// ¿Pasaron **todos** los pasos?
    pub ok: bool,
    /// Los pasos, en orden.
    pub pasos: Vec<Paso>,
    /// Los metadatos en claro (se leen **sin** la passphrase).
    pub metadatos: Option<Value>,
}

/// Busca la clave que hace cuadrar el key-check.
///
/// Se prueban las **disposiciones medidas** (los 3 bloques de la cabecera en
/// todas las permutaciones, porque su orden real no está cerrado) × los
/// parámetros de KDF que la app acepta.  La búsqueda es **acotada y
/// determinista**: el mismo input da siempre el mismo resultado.
pub fn buscar_clave(bytes: &[u8], passphrase: &str) -> Result<Clave, String> {
    let cab = leer_cabecera(bytes)?;
    let mut layout = swift_archive::sba1_detectar(bytes)
        .ok_or_else(|| "no es un contenedor Swift Backup (sin magic SBA1)".to_string())?;
    // La etiqueta del key-check depende del método de cifrado.
    let metodo = match cab.encryption_method_id {
        4 => "aegis256",
        2 => "aes256_gcm",
        3 => "aes256_gcm_siv",
        1 => "sevenzip_aes",
        5 => "aegis128x2",
        otro => {
            return Err(format!(
                "método de cifrado {otro}: no hay etiqueta de key-check conocida"
            ))
        }
    };
    if let (Some(spec), Some(tag)) = (layout.key_check.as_mut(), swift_archive::key_check_tag(metodo)) {
        spec.tag = tag.to_string();
    }
    // Disposiciones de los bloques: las 6 permutaciones (las prueba el repo).
    let n_permutaciones = if layout.key_check.as_ref().map(|s| s.probar_permutaciones).unwrap_or(false)
    {
        6
    } else {
        1
    };
    let combinaciones = combinaciones_kdf(bytes, &cab);
    let total = n_permutaciones * combinaciones.len();
    let mut ultimo: Option<String> = None;
    for params in combinaciones {
        let informe = swift_archive::verificar(bytes, &layout, passphrase, params);
        match informe.status.as_str() {
            "ok" => {
                let disposicion = informe
                    .notes
                    .iter()
                    .find_map(|n| n.split("ORDEN DE BLOQUES: ").nth(1))
                    .unwrap_or("desconocida")
                    .to_string();
                let key_check_hex = informe
                    .key_check
                    .as_ref()
                    .map(|k| k.expected_hex.clone())
                    .unwrap_or_default();
                return Ok(Clave {
                    parametros: params,
                    disposicion,
                    key_check_hex,
                });
            }
            "kdf_error" => {
                ultimo = Some(
                    informe
                        .notes
                        .iter()
                        .find(|n| n.contains("KDF"))
                        .cloned()
                        .unwrap_or_else(|| "el KDF falló".to_string()),
                );
            }
            _ => {
                ultimo = Some(
                    informe
                        .notes
                        .iter()
                        .find(|n| n.contains("key-check NO coincide"))
                        .cloned()
                        .unwrap_or_else(|| "el key-check no coincide".to_string()),
                );
            }
        }
    }
    Err(format!(
        "no se pudo confirmar la passphrase: se probaron {total} candidatos \
         (disposiciones de los bloques × parámetros del KDF) y ninguno pasó el key-check{}",
        ultimo.map(|u| format!(" ({u})")).unwrap_or_default()
    ))
}

/// Las combinaciones de KDF que se prueban (deterministas).
///
/// * iteraciones: las de la cabecera (`campo5`, offset 28) y 1;
/// * paralelismo: el de la cabecera (`campo15`, offset 98) y 4;
/// * memoria: la de la cabecera (`argon2MemoryKib`);
/// * longitud: la de la cabecera (`keyLength`).
fn combinaciones_kdf(bytes: &[u8], cab: &Cabecera) -> Vec<KdfParams> {
    let iteraciones = vec![campo_u16_be(bytes, 28).unwrap_or(3) as u32, 1];
    let paralelismos = vec![campo_u16_be(bytes, 98).unwrap_or(1) as u32, 4];
    let mut out = Vec::new();
    for it in iteraciones {
        for &par in &paralelismos {
            let p = KdfParams {
                iterations: it,
                memory_kib: cab.argon2_memory_kib,
                parallelism: par,
                out_len: cab.key_length as usize,
            };
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Lee un campo `u16` big-endian de la cabecera por su offset.
fn campo_u16_be(bytes: &[u8], offset: usize) -> Option<u16> {
    if offset + 2 > bytes.len() {
        return None;
    }
    Some(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]))
}

/// El round-trip completo: cabecera, metadatos, key-check y descifrado.
///
/// Con la passphrase **incorrecta** el veredicto es `ok: false` y el paso
/// `key_check` dice **por qué** (nunca se devuelve basura descifrada).
pub fn round_trip(bytes: &[u8], passphrase: &str) -> Result<RoundTrip, String> {
    let mut pasos: Vec<Paso> = Vec::new();

    // 1) Cabecera.
    let cab = match leer_cabecera(bytes) {
        Ok(c) => {
            pasos.push(Paso {
                paso: "cabecera".into(),
                ok: true,
                detalle: format!(
                    "formato {} ({} B), {} cifrado, compresión {} (método {})",
                    c.formato,
                    c.header_size,
                    if c.encryption == 1 { "con" } else { "sin" },
                    match c.compression {
                        3 => "zstd".to_string(),
                        o => o.to_string(),
                    },
                    c.encryption_method_id
                ),
            });
            c
        }
        Err(e) => {
            pasos.push(Paso {
                paso: "cabecera".into(),
                ok: false,
                detalle: e,
            });
            return Ok(RoundTrip {
                ok: false,
                pasos,
                metadatos: None,
            });
        }
    };

    // 2) Contenedor y metadatos (en claro: no dependen de la clave).
    let cont = match abrir(bytes) {
        Ok(c) => c,
        Err(e) => {
            pasos.push(Paso {
                paso: "metadatos".into(),
                ok: false,
                detalle: e,
            });
            return Ok(RoundTrip {
                ok: false,
                pasos,
                metadatos: None,
            });
        }
    };
    let metadatos = cont.metadatos.as_ref().map(|m| m.json.clone());
    pasos.push(Paso {
        paso: "metadatos".into(),
        ok: cont.metadatos.is_some(),
        detalle: match cont.metadatos.as_ref() {
            Some(m) => format!(
                "SAI1 en {}: {} ({} entradas)",
                m.offset,
                m.json
                    .get("packageName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("paquete desconocido"),
                cont.entradas.len()
            ),
            None => "no hay sección SAI1".to_string(),
        },
    });

    // 3) Key-check: el oráculo de la passphrase.
    let clave = match buscar_clave(bytes, passphrase) {
        Ok(c) => {
            pasos.push(Paso {
                paso: "key_check".into(),
                ok: true,
                detalle: format!(
                    "el key-check coincide => la passphrase es correcta ({})",
                    c.disposicion
                ),
            });
            Some(c)
        }
        Err(e) => {
            pasos.push(Paso {
                paso: "key_check".into(),
                ok: false,
                detalle: e,
            });
            None
        }
    };

    // 4) Descifrado del payload: el AEAD por trozos **no está replicado**
    //    (su derivación de nonce/AAD es nativa) ⇒ se dice, no se inventa.
    if clave.is_some() {
        pasos.push(Paso {
            paso: "descifrado".into(),
            ok: false,
            detalle: "el AEAD por trozos de Swift Backup (AEGIS-256) no está replicado en el repo: \
                      la derivación de nonce/AAD es nativa => el payload no se puede abrir todavía"
                .into(),
        });
    } else {
        pasos.push(Paso {
            paso: "descifrado".into(),
            ok: false,
            detalle: "no se puede descifrar sin la passphrase correcta".into(),
        });
    }

    let ok = pasos.iter().all(|p| p.ok);
    Ok(RoundTrip {
        ok,
        pasos,
        metadatos,
    })
}

// ---------------------------------------------------------------------------
// Passphrase
// ---------------------------------------------------------------------------

/// Lee la passphrase **sin** imprimirla nunca.
///
/// Fuentes (en este orden):
/// 1. la variable de entorno `SWIFT_BACKUP_PASSPHRASE`;
/// 2. el fichero `SWIFT_BACKUP_PASSPHRASE_FILE` (debe estar **fuera** del
///    repo y con permisos 0600).
pub fn leer_passphrase() -> Result<String, String> {
    if let Ok(p) = std::env::var("SWIFT_BACKUP_PASSPHRASE") {
        let p = p.trim_end_matches(['\r', '\n']).to_string();
        if !p.is_empty() {
            return Ok(p);
        }
    }
    if let Ok(ruta) = std::env::var("SWIFT_BACKUP_PASSPHRASE_FILE") {
        return leer_passphrase_de(Path::new(&ruta));
    }
    Err("no hay passphrase: exporta SWIFT_BACKUP_PASSPHRASE o \
         SWIFT_BACKUP_PASSPHRASE_FILE (fichero 0600 fuera del repo)"
        .to_string())
}

/// Lee la passphrase de un fichero (0600, fuera del repo).
pub fn leer_passphrase_de(ruta: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(ruta)
        .map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let modo = meta.permissions().mode() & 0o777;
        if modo & 0o077 != 0 {
            return Err(format!(
                "{} tiene permisos {modo:o}: la passphrase debe estar en 0600",
                ruta.display()
            ));
        }
    }
    let texto = std::fs::read_to_string(ruta)
        .map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
    let p = texto.trim_end_matches(['\r', '\n']).to_string();
    if p.is_empty() {
        return Err(format!("{} está vacío", ruta.display()));
    }
    Ok(p)
}

// ---------------------------------------------------------------------------
// Superficie MCP
// ---------------------------------------------------------------------------

/// Definiciones de las herramientas MCP de este bloque.
pub fn tool_definitions_mcp() -> Value {
    json!([
        {
            "name": "swift_backup",
            "description": "Contenedor Swift Backup (.dat, layout SBA1 medido): lee la cabecera y los metadatos en claro, y comprueba la passphrase con el key-check (derivacion Argon2id + key-check SHA-256). El payload cifrado NO se descifra todavia.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["info", "metadata", "roundtrip"] },
                    "file": { "type": "string", "description": "ruta del .dat" },
                    "passphrase_file": { "type": "string", "description": "fichero 0600 con la passphrase (fuera del repo)" }
                },
                "required": ["action", "file"]
            }
        }
    ])
}

/// Despacha las herramientas MCP de este bloque.
pub fn dispatch_mcp(name: &str, args: &Value) -> Option<Value> {
    if name != "swift_backup" {
        return None;
    }
    let s = |k: &str| -> String {
        args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
    };
    let accion = s("action");
    let fichero = s("file");
    let bytes = match std::fs::read(&fichero) {
        Ok(b) => b,
        Err(e) => {
            return Some(json!({"ok": false, "error": format!("no se pudo leer {fichero}: {e}")}));
        }
    };
    let salida = match accion.as_str() {
        "info" => match leer_cabecera(&bytes) {
            Ok(c) => json!({"ok": true, "cabecera": c}),
            Err(e) => json!({"ok": false, "error": e}),
        },
        "metadata" => match abrir(&bytes) {
            Ok(c) => json!({
                "ok": true,
                "metadatos": c.metadatos.map(|m| m.json),
                "entradas": c.entradas,
                "avisos": c.avisos,
            }),
            Err(e) => json!({"ok": false, "error": e}),
        },
        "roundtrip" => {
            let pass = match s("passphrase_file") {
                f if !f.is_empty() => leer_passphrase_de(Path::new(&f)),
                _ => leer_passphrase(),
            };
            match pass {
                Ok(p) => match round_trip(&bytes, &p) {
                    Ok(r) => json!({"ok": r.ok, "pasos": r.pasos, "metadatos": r.metadatos}),
                    Err(e) => json!({"ok": false, "error": e}),
                },
                Err(e) => json!({"ok": false, "error": e}),
            }
        }
        "" => json!({"ok": false, "error": "falta 'action'"}),
        otra => json!({"ok": false, "error": format!("acción desconocida: {otra}")}),
    };
    Some(salida)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_hex_es_de_minusculas() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
    }

    #[test]
    fn un_fichero_que_no_es_sba1_no_se_abre() {
        assert!(leer_cabecera(b"nada de nada").is_err());
        assert!(abrir(b"nada de nada").is_err());
    }
}

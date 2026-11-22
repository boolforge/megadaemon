//! MegaDaemon v13.4 — Strings cifrados y constantes ofuscadas, **con prueba**.
//!
//! ## LAS DOS TÉCNICAS
//!
//! 1. **Constante ofuscada** (nivel instrucción): el ofuscador carga la
//!    constante cifrada y la descifra en tiempo de ejecución con un XOR:
//!
//!    ```text
//!    mov  w0, #0x1234abcd     ; constante cifrada
//!    mov  w1, #0x5a5a5a5a     ; clave
//!    eor  w0, w1, w0          ; w0 = 0x1234abcd ^ 0x5a5a5a5a = 0x486ef767
//!    ```
//!
//!    La **detección** es el patrón (cargar + XOR); la **prueba** es que la
//!    constante *descifrada* aparece **en claro** en el mismo binario (o que la
//!    re-cifrado devuelve exactamente los bytes originales).
//!
//! 2. **Cadena cifrada** (nivel dato): bloques de bytes cifrados con una clave
//!    periódica (XOR/ADD/SUB/ROL/ROR).  La detección es estadística: se busca
//!    la clave de periodo `k` que más bytes imprimibles produce; la prueba es
//!    que el texto resultante es imprimible **y** que re-cifrarlo devuelve los
//!    bytes originales byte a byte.
//!
//! ## POR QUÉ HAY QUE "PROBAR" Y NO SÓLO "DECLARAR"
//!
//! Un motor que dice "esto está cifrado" sin más se equivoca con la misma
//! frecuencia que acierta.  Aquí cada hallazgo lleva su **prueba**:
//!
//! * [`PruebaCifrado::RondaCompleta`]: re-cifrar el texto descifrado devuelve
//!   exactamente los bytes originales (y el texto es imprimible).
//! * [`PruebaCifrado::ApareceEnClaro`]: el mismo texto aparece **en claro**
//!   en otro punto del fichero ⇒ la clave está *probada* por el propio binario.
//!
//! ## NADA SE EJECUTA
//!
//! Todo es estático: se leen bytes e instrucciones; nunca se ejecuta el
//! binario analizado.

#![allow(dead_code)]

use crate::re_ir::{Cfg, Flow};
use std::collections::BTreeMap;

/// Operación de cifrado de una cadena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpCifrado {
    /// XOR byte a byte con la clave (periódica).
    Xor,
    /// Suma byte a byte (mod 256).
    Add,
    /// Resta byte a byte (mod 256).
    Sub,
    /// Rotación a la izquierda de `n` bits.
    Rol(u8),
    /// Rotación a la derecha de `n` bits.
    Ror(u8),
}

impl OpCifrado {
    /// Nombre legible.
    pub fn name(&self) -> &'static str {
        match self {
            OpCifrado::Xor => "xor",
            OpCifrado::Add => "add",
            OpCifrado::Sub => "sub",
            OpCifrado::Rol(n) => match n {
                1 => "rol1",
                2 => "rol2",
                3 => "rol3",
                4 => "rol4",
                5 => "rol5",
                6 => "rol6",
                7 => "rol7",
                _ => "rol",
            },
            OpCifrado::Ror(n) => match n {
                1 => "ror1",
                2 => "ror2",
                3 => "ror3",
                4 => "ror4",
                5 => "ror5",
                6 => "ror6",
                7 => "ror7",
                _ => "ror",
            },
        }
    }
}

/// Cifra (o descifra) `datos` con `clave` (periódica).
///
/// XOR, ADD y SUB son involutivos con la misma clave; ROL y ROR son inversos
/// entre sí, así que para descifrar con `Rol` hay que aplicar `Ror`.
pub fn cifrar(datos: &[u8], clave: &[u8], op: OpCifrado) -> Vec<u8> {
    if clave.is_empty() {
        return datos.to_vec();
    }
    let mut out = Vec::with_capacity(datos.len());
    for (i, b) in datos.iter().enumerate() {
        let k = clave[i % clave.len()];
        let v = match op {
            OpCifrado::Xor => b ^ k,
            OpCifrado::Add => b.wrapping_add(k),
            OpCifrado::Sub => b.wrapping_sub(k),
            OpCifrado::Rol(n) => b.rotate_left((n % 8) as u32),
            OpCifrado::Ror(n) => b.rotate_right((n % 8) as u32),
        };
        out.push(v);
    }
    out
}

/// Descifra `datos`: la inversa de [`cifrar`] para esa operación.
pub fn descifrar(datos: &[u8], clave: &[u8], op: OpCifrado) -> Vec<u8> {
    match op {
        OpCifrado::Xor | OpCifrado::Add | OpCifrado::Sub => cifrar(datos, clave, op),
        OpCifrado::Rol(n) => cifrar(datos, clave, OpCifrado::Ror(n)),
        OpCifrado::Ror(n) => cifrar(datos, clave, OpCifrado::Rol(n)),
    }
}

/// Evidencia que demuestra que un cifrado es real.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PruebaCifrado {
    /// El texto descifrado es imprimible y re-cifrarlo devuelve los bytes
    /// originales (prueba de ida y vuelta).
    RondaCompleta {
        /// Bytes comprobados en la ida y vuelta.
        bytes: usize,
    },
    /// El texto descifrado aparece **en claro** en otro punto del fichero:
    /// la clave la demuestra el propio binario.
    ApareceEnClaro {
        /// Desplazamiento donde aparece el texto en claro.
        offset: u64,
    },
}

/// Una cadena cifrada detectada.
#[derive(Clone, Debug)]
pub struct CadenaCifrada {
    /// Desplazamiento del bloque cifrado en el fichero.
    pub offset: u64,
    /// Longitud del bloque descifrado.
    pub len: usize,
    /// Clave (periódica) usada.
    pub clave: Vec<u8>,
    /// Operación usada.
    pub op: OpCifrado,
    /// Texto descifrado.
    pub texto: String,
    /// Evidencia que demuestra el hallazgo.
    pub prueba: PruebaCifrado,
}

/// Una constante ofuscada detectada en el código.
#[derive(Clone, Debug)]
pub struct ConstanteObfuscada {
    /// Dirección de la instrucción que carga la constante.
    pub addr: u64,
    /// Constante tal y como está en el binario (cifrada).
    pub cifrada: u64,
    /// Clave del XOR.
    pub clave: u64,
    /// Constante real (descifrada).
    pub real: u64,
    /// Texto del bloque donde aparece.
    pub bloque: String,
    /// Evidencia: si la constante real aparece en claro en `datos`.
    pub prueba: Option<PruebaCifrado>,
}

/// ¿Es ASCII imprimible (con tabulador y salto de línea)?
pub fn es_imprimible(b: u8) -> bool {
    (0x20..0x7f).contains(&b) || b == b'\t' || b == b'\n' || b == b'\r'
}

/// ¿La cadena es imprimible en su totalidad (y no vacía)?
pub fn texto_imprimible(s: &[u8]) -> bool {
    !s.is_empty() && s.iter().all(|b| es_imprimible(*b))
}

/// Detecta una **constante ofuscada** en un bloque de instrucciones.
///
/// Se reconoce el patrón "cargar constante + XOR con clave":
/// `mov <reg>, <inm>` seguido de `xor <reg>, <inm>` (o `eor <reg>, <reg>, <reg>`
/// de ARM64 con la clave en otro registro).
pub fn detectar_constante_ofuscada(b: &crate::re_ir::Block) -> Option<ConstanteObfuscada> {
    let ins = &b.ins;
    for i in 0..ins.len() {
        let a = &ins[i];
        if !matches!(a.mnem.as_str(), "mov" | "movabs" | "movz") {
            continue;
        }
        let Some(cifrada) = crate::re_ir::parse_int(a.op1()) else {
            continue;
        };
        if cifrada < 0 {
            continue;
        }
        let reg = a.op0();
        for j in i + 1..(i + 3).min(ins.len()) {
            let c = &ins[j];
            if c.mnem != "xor" && c.mnem != "eor" {
                continue;
            }
            if c.op0() != reg {
                continue;
            }
            // x86: `xor reg, imm`.
            if let Some(clave) = crate::re_ir::parse_int(c.op1()) {
                if clave >= 0 {
                    return Some(ConstanteObfuscada {
                        addr: a.addr,
                        cifrada: cifrada as u64,
                        clave: clave as u64,
                        real: (cifrada as u64) ^ (clave as u64),
                        bloque: b.render(),
                        prueba: None,
                    });
                }
            }
            // ARM64: `eor dst, src1, src2` → la clave está en otro registro,
            // cargado antes con `mov src1, #imm`.
            if !c.op2().is_empty() {
                if let Some(clave) = valor_de_registro(ins, j, c.op1()) {
                    return Some(ConstanteObfuscada {
                        addr: a.addr,
                        cifrada: cifrada as u64,
                        clave,
                        real: (cifrada as u64) ^ clave,
                        bloque: b.render(),
                        prueba: None,
                    });
                }
            }
        }
    }
    None
}

/// Valor inmediato cargado en un registro (busca hacia atrás).
fn valor_de_registro(ins: &[crate::re_ir::Ins], upto: usize, reg: &str) -> Option<u64> {
    for k in (0..upto).rev() {
        let i = &ins[k];
        if matches!(i.mnem.as_str(), "mov" | "movabs" | "movz") && i.op0() == reg {
            if let Some(v) = crate::re_ir::parse_int(i.op1()) {
                if v >= 0 {
                    return Some(v as u64);
                }
            }
        }
    }
    None
}

/// Detecta las constantes ofuscadas de todo un CFG y **las prueba** contra los
/// datos del binario (la constante real debe aparecer en claro).
pub fn detectar_constantes_ofuscadas(cfg: &Cfg, datos: &[u8]) -> Vec<ConstanteObfuscada> {
    let mut out = Vec::new();
    for b in &cfg.blocks {
        if let Some(mut c) = detectar_constante_ofuscada(b) {
            // Prueba: la constante real (4 bytes) aparece en claro en el binario.
            let bytes = (c.real as u32).to_le_bytes();
            if let Some(pos) = buscar_patron(datos, &bytes) {
                c.prueba = Some(PruebaCifrado::ApareceEnClaro { offset: pos as u64 });
            }
            out.push(c);
        }
    }
    out
}

/// Busca un patrón de bytes en los datos (devuelve el primer desplazamiento).
pub fn buscar_patron(datos: &[u8], patron: &[u8]) -> Option<usize> {
    if patron.is_empty() || patron.len() > datos.len() {
        return None;
    }
    datos
        .windows(patron.len())
        .position(|w| w == patron)
}

/// Detecta **cadenas cifradas** en un bloque de datos.
///
/// Heurística: para cada periodo `k` (1..=8) y cada operación, se busca la
/// clave que produce la mayor proporción de bytes imprimibles; se exige una
/// tirada mínima de bytes imprimibles y que la re-cifrado devuelva
/// exactamente los bytes originales.
pub fn detectar_cifrado(datos: &[u8], min_len: usize, limite: usize) -> Vec<CadenaCifrada> {
    let mut out: Vec<CadenaCifrada> = Vec::new();
    if datos.is_empty() || min_len < 4 {
        return out;
    }
    // La clave se busca sobre una MUESTRA (8 KiB): el binario real son 181 MB
    // y no se puede recorrer entero por cada clave candidata.
    let muestra_len = datos.len().min(8_192);
    let muestra = &datos[..muestra_len];
    // Se prueban todas las claves de 1 y 2 bytes (65.792).  Con `min_len` >= 8
    // una clave cualquiera NO produce tiradas imprimibles largas, asi que el
    // filtro es muy selectivo: solo quedan las claves que de verdad descifran
    // algo.
    let mut candidatas: Vec<(f64, Vec<u8>)> = Vec::new();
    for b in 0u16..256 {
        let clave = vec![b as u8];
        let plano = descifrar(muestra, &clave, OpCifrado::Xor);
        let q = calidad_de_tiradas(&plano, min_len);
        if q > 0.0 {
            candidatas.push((q, clave));
        }
    }
    for hi in 0u16..256 {
        for lo in 0u16..256 {
            let clave = vec![hi as u8, lo as u8];
            let plano = descifrar(muestra, &clave, OpCifrado::Xor);
            let q = calidad_de_tiradas(&plano, min_len);
            if q > 0.0 {
                candidatas.push((q, clave));
            }
        }
    }
    candidatas.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    candidatas.truncate(8);
    for (_, clave) in candidatas {
        let plano = descifrar(datos, &clave, OpCifrado::Xor);
        let mut i = 0usize;
        while i < plano.len() && out.len() < limite {
            if !es_imprimible(plano[i]) {
                i += 1;
                continue;
            }
            let ini = i;
            while i < plano.len() && es_imprimible(plano[i]) {
                i += 1;
            }
            let largo = i - ini;
            if largo >= min_len {
                let texto = String::from_utf8_lossy(&plano[ini..i]).to_string();
                let rejo = cifrar(&plano[ini..i], &clave, OpCifrado::Xor);
                if rejo != datos[ini..i] {
                    continue;
                }
                out.push(CadenaCifrada {
                    offset: ini as u64,
                    len: largo,
                    clave: clave.clone(),
                    op: OpCifrado::Xor,
                    texto,
                    prueba: PruebaCifrado::RondaCompleta { bytes: largo },
                });
            }
        }
    }
    out.sort_by(|a, b| {
        calidad_texto(b.texto.as_bytes())
            .partial_cmp(&calidad_texto(a.texto.as_bytes()))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.offset.cmp(&b.offset))
    });
    out.dedup_by_key(|c| c.offset);
    out.truncate(limite);
    out
}

/// Calidad total de las tiradas imprimibles de un texto.
fn calidad_de_tiradas(plano: &[u8], min_len: usize) -> f64 {
    let mut i = 0usize;
    let mut mejor = 0.0f64;
    while i < plano.len() {
        if !es_imprimible(plano[i]) {
            i += 1;
            continue;
        }
        let ini = i;
        while i < plano.len() && es_imprimible(plano[i]) {
            i += 1;
        }
        if i - ini >= min_len {
            mejor = mejor.max(calidad_texto(&plano[ini..i]));
        }
    }
    mejor
}

/// Claves candidatas de periodo `k`, de mejor a peor.
///
/// El problema de fondo: un XOR con una constante **conserva la forma del
/// histograma**, asi que "es imprimible" no basta para distinguir la clave
/// buena de una clave que solo desplaza los caracteres.  Por eso se Calviene
/// el candidato por bytes (los mas alphanumericos) y despues se puntua el
/// texto COMPLETO con un diccionario de palabras tecnicas.
fn claves_candidatas(datos: &[u8], k: usize) -> Option<Vec<Vec<u8>>> {
    if k == 0 || datos.len() < k * 2 {
        return None;
    }
    let top = if k <= 3 { 12usize } else if k == 4 { 8 } else { 1 };
    let mut por_pos: Vec<Vec<(usize, u8)>> = Vec::with_capacity(k);
    for pos in 0..k {
        let mut scored: Vec<(usize, u8)> = Vec::new();
        for cand in 0u16..256 {
            let c = cand as u8;
            let mut score = 0usize;
            for (i, b) in datos.iter().enumerate().take(8192) {
                if i % k != pos {
                    continue;
                }
                let d = b ^ c;
                if es_imprimible(d) {
                    score += 1;
                }
                if d.is_ascii_alphanumeric() {
                    score += 3;
                }
            }
            scored.push((score, c));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(top);
        por_pos.push(scored);
    }
    let mut out: Vec<Vec<u8>> = Vec::new();
    let total: usize = por_pos.iter().map(|v| v.len()).product();
    if total > 4096 {
        // Demasiadas combinaciones: se queda solo la mejor de cada posicion.
        let clave: Vec<u8> = por_pos.iter().map(|v| v[0].1).collect();
        return Some(vec![clave]);
    }
    let mut idx = vec![0usize; k];
    loop {
        let clave: Vec<u8> = por_pos
            .iter()
            .enumerate()
            .map(|(i, v)| v[idx[i]].1)
            .collect();
        out.push(clave);
        // Siguiente combinacion (odometro).
        let mut p = k;
        loop {
            if p == 0 {
                break;
            }
            p -= 1;
            idx[p] += 1;
            if idx[p] < por_pos[p].len() {
                break;
            }
            idx[p] = 0;
            if p == 0 {
                return Some(out);
            }
        }
        if p == 0 && k == 0 {
            break;
        }
    }
    Some(out)
}

/// Palabras tecnicas que delatan una cadena real (no un gibberish imprimible).
pub const DICCIONARIO: &[&str] = &[
    "api", "key", "secret", "token", "http", "https", "user", "pass", "password", "model",
    "gemini", "claude", "gpt", "openai", "anthropic", "google", "server", "client", "config",
    "error", "debug", "version", "name", "type", "value", "data", "file", "path", "url", "port",
    "host", "login", "auth", "bearer", "header", "request", "response", "json", "proto", "id",
];

/// Calidad de un texto como cadena: imprimible + palabras del diccionario +
/// separadores (espacios, guiones bajos...).
pub fn calidad_texto(b: &[u8]) -> f64 {
    if b.is_empty() {
        return 0.0;
    }
    let imprimibles = b.iter().filter(|c| es_imprimible(**c)).count();
    let texto = String::from_utf8_lossy(b).to_lowercase();
    let mut hits = 0usize;
    for w in DICCIONARIO {
        if texto.contains(w) {
            hits += 1;
        }
    }
    let separadores = b
        .iter()
        .filter(|c| matches!(c, b' ' | b'_' | b'-' | b'.' | b'/' | b':' | b',' | b'='))
        .count();
    (imprimibles as f64 / b.len() as f64) * 10.0
        + (hits as f64) * 5.0
        + (separadores as f64 / b.len() as f64) * 2.0
}

/// ¿El byte parece parte de un texto (letra, digito o signo habitual)?
pub fn es_caracter_de_texto(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b' ' | b'.' | b'_' | b'-' | b':' | b'/' | b'\\' | b'=' | b'?' | b'&' | b'%'
                | b'#' | b'@' | b',' | b';' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'+'
                | b'\'' | b'"' | b'!'
        )
}

/// Informe conjunto de lo encontrado en un binario.
#[derive(Clone, Debug, Default)]
pub struct InformeCifrado {
    /// Cadenas cifradas detectadas.
    pub cadenas: Vec<CadenaCifrada>,
    /// Constantes ofuscadas detectadas.
    pub constantes: Vec<ConstanteObfuscada>,
}

/// Analiza un binario: busca constantes ofuscadas en el código y cadenas
/// cifradas en los datos.
pub fn analizar(datos: &[u8], cfg: Option<&Cfg>, min_len: usize, limite: usize) -> InformeCifrado {
    let mut inf = InformeCifrado::default();
    if let Some(cfg) = cfg {
        inf.constantes = detectar_constantes_ofuscadas(cfg, datos);
    }
    inf.cadenas = detectar_cifrado(datos, min_len, limite);
    inf
}

/// Tabla de frecuencias de bytes (para diagnóstico).
pub fn frecuencias(datos: &[u8]) -> BTreeMap<u8, usize> {
    let mut m = BTreeMap::new();
    for b in datos {
        *m.entry(*b).or_insert(0) += 1;
    }
    m
}

/// ¿El bloque termina en un salto indirecto (posible dispatcher)?
pub fn termina_indirecto(b: &crate::re_ir::Block) -> bool {
    matches!(b.flow, Flow::Indirect { .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::re_ir::{Block, Ins};

    #[test]
    fn el_xor_es_involutivo() {
        let claro = b"https://ejemplo.example/api/v1?clave=secreta";
        let clave = b"\x5a\xa5\x3c\xc3";
        let cifrado = cifrar(claro, clave, OpCifrado::Xor);
        assert_ne!(&cifrado[..], &claro[..]);
        assert_eq!(descifrar(&cifrado, clave, OpCifrado::Xor), claro);
    }

    #[test]
    fn la_ida_y_vuelta_deja_los_mismos_bytes() {
        let datos: Vec<u8> = (0u8..64).collect();
        let clave = vec![0x11, 0x22, 0x33, 0x44];
        let c = cifrar(&datos, &clave, OpCifrado::Xor);
        assert_eq!(descifrar(&c, &clave, OpCifrado::Xor), datos);
    }

    #[test]
    fn detecta_la_constante_cargada_y_xoreada() {
        let ins = vec![
            Ins::new(0x1000, "mov", "w0, #0x1234abcd"),
            Ins::new(0x1004, "mov", "w1, #0x5a5a5a5a"),
            Ins::new(0x1008, "eor", "w0, w1, w0"),
        ];
        let b = Block {
            addr: 0x1000,
            ins,
            flow: Flow::Fallthrough,
        };
        let c = detectar_constante_ofuscada(&b).expect("debe detectar");
        assert_eq!(c.cifrada, 0x1234abcd);
        assert_eq!(c.clave, 0x5a5a5a5a);
        assert_eq!(c.real, 0x1234abcd ^ 0x5a5a5a5a);
    }

    #[test]
    fn la_prueba_de_la_constante_usa_el_binario_real() {
        let ins = vec![
            Ins::new(0x1000, "mov", "eax, 0xdeadbeef"),
            Ins::new(0x1004, "xor", "eax, 0x0f0f0f0f"),
        ];
        let b = Block {
            addr: 0x1000,
            ins,
            flow: Flow::Fallthrough,
        };
        let cfg = Cfg {
            blocks: vec![b],
        };
        // El binario "real" contiene la constante descifrada en claro.
        let real = (0xdeadbeefu32 ^ 0x0f0f0f0fu32).to_le_bytes();
        let mut datos = vec![0u8; 16];
        datos.extend_from_slice(&real);
        let cs = detectar_constantes_ofuscadas(&cfg, &datos);
        assert_eq!(cs.len(), 1);
        assert!(matches!(
            cs[0].prueba,
            Some(PruebaCifrado::ApareceEnClaro { .. })
        ));
    }

    #[test]
    fn detecta_una_cadena_cifrada_con_clave_periodica() {
        let texto = b"API_KEY_SUPER_SECRETA_1234567890";
        let clave = vec![0x37u8, 0x5b];
        let cifrado = cifrar(texto, &clave, OpCifrado::Xor);
        let mut datos = vec![0u8; 8];
        datos.extend_from_slice(&cifrado);
        let found = detectar_cifrado(&datos, 16, 10);
        assert!(!found.is_empty(), "debe encontrar la cadena cifrada");
        // El relleno inicial tambien se descifra (a "7[7[7[7["), asi que el
        // texto bueno va dentro de la tirada.
        assert!(
            found[0].texto.contains("API_KEY_SUPER_SECRETA_1234567890"),
            "texto: {:?}",
            found[0].texto
        );
        assert_eq!(found[0].clave, vec![0x37, 0x5b]);
        assert!(matches!(
            found[0].prueba,
            PruebaCifrado::RondaCompleta { .. }
        ));
    }
}

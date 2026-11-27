//! Motor genérico de deofuscación de APK/DEX — **capas 4 a 7**.
//!
//! Este módulo implementa el tramo del motor descrito en
//! `re_deep_brief/parte_e.md` §E.4.1 que **no** estaba cubierto por
//! `re_deep.rs` (capas 1-3: ingesta, decodificador auditado y CFG):
//!
//! * **Capa 4 — folding de constantes / MBA**: propagación de constantes
//!   intra-bloque, detección de ramas con condición constante (predicados
//!   opacos) y consulta del veredicto MBA al motor `re_mba` (ROBDD).
//! * **Capa 5 — descifrado de strings en masa**:
////!   5a. detección de stubs (`find_decrypt_stubs`),
//!   5b. enumeración de *call sites* en **todos** los DEX (`find_call_sites`),
//!   5c. extracción de la clave (ejecutando el `<clinit>` de la clase con el
//!        intérprete, que es donde los empaquetadores construyen la tabla),
//!   5d. ejecución del stub con un intérprete Dalvik acotado (`Interpreter`),
//!   5e. *fallback* de cifrado de bloque con **clave al final del blob**
//!        (AES/ECB/PKCS5, implementado sin dependencias nuevas).
//! * **Capa 6 — huella de empaquetador / protector / anti-análisis**:
//!   loaders dinámicos, entropía de assets, cifrado de strings, anti-debug,
//!   RASP y, para nativo, UPX / OLLVM / Tigress / VMProtect.
//! * **Capa 7 — salida**: informe JSON, tabla de strings (CSV/Markdown) y
//!   hooks Frida para el modo dinámico.
//!
//! ## Estado de lo verificado
//!
//! * Las anchuras de instrucción usan [`crate::re_deep::dalvik_width`], que es
//!   la tabla **medida** del repo (el `goto` ocupa **una** unidad).
//! * Los operandos se leen con la convención **medida** (§E.3.3): índice de
//!   `22c`/`35c` en `unit1`, literal de `22b` en el byte alto, palabra de
//!   registros de `35c` en `unit2` con el primer argumento en el nibble bajo.
//! * La tabla de opcodes del repo (`dex_decompile::opcode_info`) se ha
//!   contrastado contra un APK real: `int-to-char` = `0x8e`, `aget-char` =
//!   `0x49`, `aput-char` = `0x50` (MEDIDO, ver `re_obf` tests).
//! * El AES/ECB de la capa 5e se validó antes en Python contra los **tres**
//!   vectores FIPS-197 (AES-128/192/256) y se porta aquí sin cambios de
//!   algoritmo.
//!
//! ## Límites honestos (§E.4.3)
//!
//! * No se descifra una clave que dependa del hardware (TEE, `attest`).
//! * No se devirtualiza un protector comercial sin ejecutar el binario.
//! * El intérprete es **acotado**: si llama a una API que no se emula, lo dice
//!   (`ExecError`) en vez de inventar el resultado.
//! * La reescritura del DEX con los strings en claro (F5) **no** está aquí:
//!   escribir un DEX válido es un encargo aparte.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::cell::RefCell;
use std::rc::Rc;

use crate::dex_decompile::{DexCode, DexFile};
use crate::re_deep::{decode_code, DecodedInsn};

// ========================================================================= //
// 0. VALORES
// ========================================================================= //

/// Valor manipulado por el plegado (capa 4) y por el intérprete (capa 5d).
///
/// `Unknown` es un valor de primera clase: el motor nunca inventa un dato,
/// marca que no lo ha podido derivar.
// `Value` NO implementa `Serialize`: los arrays usan `Rc` (copy-on-write)
// y serde no serializa `Rc` sin la feature "rc" (y el crate del daemon no
// la activa). Los informes usan los tipos planos (`Stub`, `CallSite`,
// `StringEntry`), que sí son serializables.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// No se ha podido derivar el valor.
    Unknown,
    /// Referencia nula.
    Null,
    /// Entero de 32 bits con signo ( Dalvik `int`).
    Int(i64),
    /// Booleano.
    Bool(bool),
    /// Cadena de texto.
    Str(String),
    /// `byte[]`.
    Bytes(Rc<RefCell<Vec<u8>>>),
    /// `char[]`.
    Chars(Rc<RefCell<Vec<u16>>>),
    /// `int[]`.
    Ints(Rc<RefCell<Vec<i64>>>),
    /// Arreglo de objetos (`Object[]`, `String[]`…).
    Objects(Rc<RefCell<Vec<Value>>>),
    /// Objeto simple con campos (p. ej. `SecretKeySpec`).
    Object {
        /// Descriptor de la clase (`Ljavax/crypto/spec/SecretKeySpec;`).
        class: String,
        /// Campos por nombre.
        fields: BTreeMap<String, Value>,
    },
}

impl Value {
    /// Entero equivalente, si lo hay.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Bool(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    /// Texto equivalente, si lo hay.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// `true` si el motor no ha podido derivar el valor.
    pub fn is_unknown(&self) -> bool {
        matches!(self, Value::Unknown)
    }

    /// Representación textual corta (para informes y tablas).
    pub fn to_text(&self) -> String {
        match self {
            Value::Unknown => "?".to_string(),
            Value::Null => "null".to_string(),
            Value::Int(i) => i.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Str(s) => s.clone(),
            Value::Bytes(v) => format!("<{} bytes>", v.borrow().len()),
            Value::Chars(v) => format!("<{} chars>", v.borrow().len()),
            Value::Ints(v) => format!("<{} ints>", v.borrow().len()),
            Value::Objects(v) => format!("<{} objetos>", v.borrow().len()),
            Value::Object { class, .. } => format!("<{class}>"),
        }
    }
}

/// Trunca a 32 bits con signo (semántica `int` de Dalvik).
fn wrap32(x: i64) -> i64 {
    x as i32 as i64
}

/// Convierte un operando a registro sin desbordar.
fn as_reg(x: i64) -> u16 {
    if x < 0 {
        0
    } else if x > 0xffff {
        0xffff
    } else {
        x as u16
    }
}

// ========================================================================= //
// 1. DECODIFICACIÓN LOCAL (extiende la del repo a los formatos que faltan)
// ========================================================================= //

/// Decodificación completa de un `code_item`.
///
/// Parte de [`crate::re_deep::decode_code`] (que ya usa las convenciones
/// medidas) y **parchea los formatos que su tabla no cubre**: `32x`
/// (`move/16`), `3rc` (invocaciones por rango: la lista de registros no venia
/// rellena), `45cc`/`4rcc` (invocaciones polimórficas) y sus índices de
/// método.
fn decode_all(insns: &[u16]) -> Vec<DecodedInsn> {
    let mut out = decode_code(insns);
    for ins in out.iter_mut() {
        let i = ins.offset as usize;
        let u0 = insns.get(i).copied().unwrap_or(0);
        let u1 = insns.get(i + 1).copied().unwrap_or(0);
        let u2 = insns.get(i + 2).copied().unwrap_or(0);
        let u3 = insns.get(i + 3).copied().unwrap_or(0);
        match ins.format.as_str() {
            "32x" => {
                ins.a = ((u0 >> 8) & 0xff) as i64;
                ins.b = u1 as i64;
            }
            "3rc" => {
                ins.ref_index = u1 as u32;
                let start = (u2 & 0xff) as i64;
                let cnt = ins.a.max(0) as usize;
                ins.regs = (0..cnt).map(|k| (start + k as i64) as u8).collect();
            }
            "45cc" => {
                ins.ref_index = u1 as u32;
                let cnt = ((u0 >> 12) & 0x0f) as usize;
                let mut regs: Vec<u8> = Vec::with_capacity(cnt);
                for k in 0..cnt.min(8) {
                    regs.push(((u2 >> (4 * k)) & 0x0f) as u8);
                }
                for k in 0..cnt.saturating_sub(8).min(8) {
                    regs.push(((u3 >> (4 * k)) & 0x0f) as u8);
                }
                ins.regs = regs;
            }
            "4rcc" => {
                ins.ref_index = u1 as u32;
                let start = (u2 & 0xff) as i64;
                let cnt = ins.a.max(0) as usize;
                ins.regs = (0..cnt).map(|k| (start + k as i64) as u8).collect();
            }
            _ => {}
        }
    }
    out
}

/// Conjunto de destinos de salto (para partir en bloques básicos).
pub fn branch_targets(decoded: &[DecodedInsn]) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for ins in decoded {
        if let Some(t) = ins.branch_target {
            if t >= 0 {
                out.insert(t as u32);
            }
        }
    }
    out
}

// ========================================================================= //
// 2. CAPA 4 — FOLDING DE CONSTANTES / MBA
// ========================================================================= //

/// Instrucción analizada por el plegador.
#[derive(Debug, Clone)]
pub struct FoldedInsn {
    /// Offset en unidades de 16 bits.
    pub offset: u32,
    /// Mnemónico Dalvik.
    pub mnemonic: String,
    /// Registro destino (si la instrucción escribe en uno).
    pub register: Option<u16>,
    /// Valor constante deducido (`None` si no se pudo).
    pub value: Option<Value>,
    /// Expresión deducida en texto.
    pub expression: String,
    /// `true` si la instrucción es una rama con condición constante.
    pub opaque: bool,
}

/// Resumen del plegado de un `code_item`.
#[derive(Debug, Clone, Default)]
pub struct FoldSummary {
    /// Instrucciones analizadas (sin payloads).
    pub instructions: usize,
    /// Instrucciones con valor constante deducido.
    pub folded: usize,
    /// Ramas con condición constante (predicados opacos).
    pub opaque_branches: usize,
    /// Detalle por instrucción.
    pub items: Vec<FoldedInsn>,
}

/// Family of integer operations handled by the folder.
fn arith_kind(m: &str) -> Option<&'static str> {
    let base = m.split('/').next().unwrap_or(m);
    match base {
        "add-int" => Some("add-int"),
        "sub-int" => Some("sub-int"),
        "mul-int" => Some("mul-int"),
        "div-int" => Some("div-int"),
        "rem-int" => Some("rem-int"),
        "and-int" => Some("and-int"),
        "or-int" => Some("or-int"),
        "xor-int" => Some("xor-int"),
        "shl-int" => Some("shl-int"),
        "shr-int" => Some("shr-int"),
        "ushr-int" => Some("ushr-int"),
        "rsub-int" => Some("rsub-int"),
        _ => None,
    }
}

/// Evalúa una operación binaria entera con semántica de 32 bits.
fn eval_bin(op: &str, x: Option<i64>, y: Option<i64>) -> Option<i64> {
    let (x, y) = match (x, y) {
        (Some(a), Some(b)) => (a, b),
        _ => return None,
    };
    let r = match op {
        "add-int" => x.wrapping_add(y),
        "sub-int" => x.wrapping_sub(y),
        "mul-int" => x.wrapping_mul(y),
        "div-int" => {
            if y == 0 {
                return None;
            }
            x.wrapping_div(y)
        }
        "rem-int" => {
            if y == 0 {
                return None;
            }
            x.wrapping_rem(y)
        }
        "and-int" => x & y,
        "or-int" => x | y,
        "xor-int" => x ^ y,
        "shl-int" => x.wrapping_shl((y as u32) & 31),
        "shr-int" => x.wrapping_shr((y as u32) & 31),
        "ushr-int" => ((x as u32).wrapping_shr((y as u32) & 31)) as i64,
        "rsub-int" => y.wrapping_sub(x),
        _ => return None,
    };
    Some(wrap32(r))
}

/// Compara dos enteros según la mnemónico de la rama.
fn compare(op: &str, a: i64, b: i64) -> bool {
    match op {
        "if-eq" => a == b,
        "if-ne" => a != b,
        "if-lt" => a < b,
        "if-ge" => a >= b,
        "if-gt" => a > b,
        "if-le" => a <= b,
        _ => false,
    }
}

/// Plegador de constantes: una pasada lineal, conservadora.
struct Folder<'a> {
    dex: Option<&'a DexFile>,
    regs: HashMap<u16, Value>,
}

impl<'a> Folder<'a> {
    fn string_at(&self, index: u32) -> Option<String> {
        self.dex
            .and_then(|d| d.strings.get(index as usize))
            .cloned()
    }

    fn type_at(&self, index: u32) -> Option<String> {
        self.dex
            .and_then(|d| d.types.get(index as usize))
            .map(|t| t.descriptor.clone())
    }
}

/// Fold de a single instruction (returns value and destination register).
fn fold_one(ins: &DecodedInsn, f: &mut Folder<'_>) -> (Option<Value>, Option<u16>, bool) {
    let m = ins.mnemonic.as_str();
    let dst = Some(as_reg(ins.a));
    let geti = |r: i64| -> Option<i64> {
        f.regs.get(&as_reg(r)).and_then(|v| v.as_int())
    };
    let getv = |r: i64| -> Option<Value> { f.regs.get(&as_reg(r)).cloned() };

    // --- constantes ---------------------------------------------------- //
    match m {
        "const/4" | "const/16" | "const" | "const/high16" => {
            return (Some(Value::Int(ins.literal)), dst, false);
        }
        "const-string" => {
            if let Some(s) = f.string_at(ins.ref_index) {
                return (Some(Value::Str(s)), dst, false);
            }
            return (None, dst, false);
        }
        _ => {}
    }

    // --- movimientos --------------------------------------------------- //
    if m.starts_with("move")
        && m != "move-result"
        && m != "move-result-object"
        && m != "move-exception"
    {
        if let Some(v) = getv(ins.b) {
            if !v.is_unknown() {
                return (Some(v), dst, false);
            }
        }
        return (None, dst, false);
    }

    // --- aritmética ---------------------------------------------------- //
    if let Some(kind) = arith_kind(m) {
        let v = if m.contains("/2addr") {
            eval_bin(kind, geti(ins.a), geti(ins.b))
        } else if m.contains("/lit8") || m.contains("/lit16") {
            if kind == "rsub-int" {
                match geti(ins.b) {
                    Some(b) => Some(wrap32(ins.literal.wrapping_sub(b))),
                    None => None,
                }
            } else {
                eval_bin(kind, geti(ins.b), Some(ins.literal))
            }
        } else {
            eval_bin(kind, geti(ins.b), geti(ins.c))
        };
        return (v.map(Value::Int), dst, false);
    }

    // --- unarios ------------------------------------------------------- //
    match m {
        "neg-int" => {
            return (geti(ins.b).map(|b| Value::Int(wrap32(b.wrapping_neg()))), dst, false);
        }
        "not-int" => {
            return (geti(ins.b).map(|b| Value::Int(wrap32(!b))), dst, false);
        }
        "int-to-byte" => {
            return (
                geti(ins.b).map(|b| Value::Int(b as i8 as i64)),
                dst,
                false,
            );
        }
        "int-to-char" => {
            return (
                geti(ins.b).map(|b| Value::Int(b as u16 as i64)),
                dst,
                false,
            );
        }
        "int-to-short" => {
            return (
                geti(ins.b).map(|b| Value::Int(b as i16 as i64)),
                dst,
                false,
            );
        }
        "array-length" => {
            let len = match getv(ins.b) {
                Some(Value::Bytes(v)) => Some(v.borrow().len() as i64),
                Some(Value::Chars(v)) => Some(v.borrow().len() as i64),
                Some(Value::Ints(v)) => Some(v.borrow().len() as i64),
                Some(Value::Objects(v)) => Some(v.borrow().len() as i64),
                _ => None,
            };
            return (len.map(Value::Int), dst, false);
        }
        _ => {}
    }

    // --- arreglos ------------------------------------------------------ //
    if m == "new-array" {
        let size = match geti(ins.b) {
            Some(n) if n >= 0 => n as usize,
            _ => return (None, dst, false),
        };
        let ty = f.type_at(ins.ref_index).unwrap_or_default();
        let v = match ty.as_str() {
            "[C" => Value::Chars(Rc::new(RefCell::new(vec![0u16; size]))),
            "[B" => Value::Bytes(Rc::new(RefCell::new(vec![0u8; size]))),
            "[I" => Value::Ints(Rc::new(RefCell::new(vec![0i64; size]))),
            "[S" => Value::Ints(Rc::new(RefCell::new(vec![0i64; size]))),
            "[J" => Value::Ints(Rc::new(RefCell::new(vec![0i64; size]))),
            _ => Value::Objects(Rc::new(RefCell::new(vec![Value::Unknown; size]))),
        };
        return (Some(v), dst, false);
    }
    if m.starts_with("aget") {
        // `aget vAA, vBB, vCC` → vAA = vCC[vBB] (array = vCC, índice = vBB).
        let arr = getv(ins.c);
        let idx = geti(ins.b);
        if let (Some(arr), Some(idx)) = (arr, idx) {
            let v = array_get(&arr, idx, m);
            if let Some(v) = v {
                return (Some(v), dst, false);
            }
        }
        return (None, dst, false);
    }

    // --- ramas (predicados opacos) ------------------------------------- //
    if let Some(op) = branch_op(m) {
        let (lhs, rhs) = if m.ends_with("z") {
            (geti(ins.a), Some(0))
        } else {
            (geti(ins.a), geti(ins.b))
        };
        if let (Some(a), Some(b)) = (lhs, rhs) {
            let taken = compare(op, a, b);
            return (Some(Value::Bool(taken)), None, true);
        }
        return (None, None, false);
    }

    (None, None, false)
}

/// Nombre de la comparación de una rama (`if-eq`…), si lo es.
fn branch_op(m: &str) -> Option<&'static str> {
    match m {
        "if-eq" => Some("if-eq"),
        "if-ne" => Some("if-ne"),
        "if-lt" => Some("if-lt"),
        "if-ge" => Some("if-ge"),
        "if-gt" => Some("if-gt"),
        "if-le" => Some("if-le"),
        "if-eqz" => Some("if-eq"),
        "if-nez" => Some("if-ne"),
        "if-ltz" => Some("if-lt"),
        "if-gez" => Some("if-ge"),
        "if-gtz" => Some("if-gt"),
        "if-lez" => Some("if-le"),
        _ => None,
    }
}

/// Lee un elemento de un arreglo Dalvik.
fn array_get(arr: &Value, idx: i64, mnemonic: &str) -> Option<Value> {
    if idx < 0 {
        return None;
    }
    let i = idx as usize;
    match arr {
        Value::Chars(v) => v.borrow().get(i).map(|c| Value::Int(*c as i64)),
        Value::Bytes(v) => v.borrow().get(i).map(|b| {
            if mnemonic == "aget-byte" {
                Value::Int(*b as i8 as i64)
            } else {
                Value::Int(*b as i64)
            }
        }),
        Value::Ints(v) => v.borrow().get(i).map(|x| Value::Int(*x)),
        Value::Objects(v) => v.borrow().get(i).cloned(),
        _ => None,
    }
}

/// Escribe un elemento en un arreglo Dalvik (in place sobre el valor).
fn array_set(arr: &mut Value, idx: i64, value: Value, mnemonic: &str) -> bool {
    if idx < 0 {
        return false;
    }
    let i = idx as usize;
    match arr {
        Value::Chars(v) => {
            let n = value.as_int().unwrap_or(0) as u16;
            let mut v = v.borrow_mut();
            if i < v.len() {
                v[i] = n;
                true
            } else {
                false
            }
        }
        Value::Bytes(v) => {
            let n = value.as_int().unwrap_or(0) as u8;
            let mut v = v.borrow_mut();
            if i < v.len() {
                v[i] = n;
                true
            } else {
                false
            }
        }
        Value::Ints(v) => {
            let n = value.as_int().unwrap_or(0);
            let mut v = v.borrow_mut();
            if i < v.len() {
                v[i] = n;
                true
            } else {
                false
            }
        }
        Value::Objects(v) => {
            let mut v = v.borrow_mut();
            if i < v.len() {
                v[i] = value;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Plega las constantes de un `code_item` (sin resolver `const-string`).
pub fn fold_constants(insns: &[u16]) -> FoldSummary {
    fold_internal(None, insns)
}

/// Plega las constantes de un `code_item` resolviendo `const-string`.
pub fn fold_constants_dex(dex: &DexFile, insns: &[u16]) -> FoldSummary {
    fold_internal(Some(dex), insns)
}

/// Implementación común del plegador.
fn fold_internal(dex: Option<&DexFile>, insns: &[u16]) -> FoldSummary {
    let decoded = decode_all(insns);
    // Sólo se invalidan las constantes en los **saltos hacia atrás**: ahí el
    // estado puede venir de otro camino. En los saltos hacia delante el
    // estado fluye por el camino de caída, que es el caso habitual.
    let mut back_targets: BTreeSet<u32> = BTreeSet::new();
    for ins in &decoded {
        if let Some(t) = ins.branch_target {
            if t >= 0 && (t as u32) < ins.offset {
                back_targets.insert(t as u32);
            }
        }
    }
    let mut folder = Folder {
        dex,
        regs: HashMap::new(),
    };
    let mut summary = FoldSummary::default();

    for ins in &decoded {
        if ins.is_payload {
            continue;
        }
        if back_targets.contains(&ins.offset) {
            folder.regs.clear();
        }
        let (value, reg, opaque) = fold_one(ins, &mut folder);
        if let Some(v) = &value {
            if !v.is_unknown() {
                if let Some(r) = reg {
                    folder.regs.insert(r, v.clone());
                }
                summary.folded += 1;
            }
        }
        if opaque {
            summary.opaque_branches += 1;
        }
        let expression = match (&value, reg) {
            (Some(v), Some(r)) => format!("v{r} = {}", v.to_text()),
            (Some(v), None) => format!("{}", v.to_text()),
            (None, Some(r)) => format!("v{r} = ?"),
            (None, None) => String::new(),
        };
        summary.items.push(FoldedInsn {
            offset: ins.offset,
            mnemonic: ins.mnemonic.clone(),
            register: reg,
            value,
            expression,
            opaque,
        });
        summary.instructions += 1;
        // Tras una rama el estado puede divergir: se invalida.
        if ins.mnemonic.starts_with("if-") || ins.mnemonic.starts_with("goto") {
            folder.regs.clear();
        }
    }
    summary
}

/// Veredicto del motor MBA sobre una expresión textual.
#[derive(Debug, Clone, Serialize)]
pub struct MbaVerdict {
    /// `opaca`, `no-opaca`, `constante` o `error`.
    pub verdict: String,
    /// `true` si la expresión se ha demostrado equivalente a cero.
    pub always_zero: bool,
    /// Detalle legible.
    pub detail: String,
}

/// Consulta el veredicto de predicado opaco al motor `re_mba` (ROBDD).
///
/// Es el puente entre la capa 4 textual (jadx-string-decrypt) y el motor
/// algebraico ya existente en el repo.
pub fn mba_verdict(expression: &str) -> MbaVerdict {
    match crate::re_mba::parse(expression) {
        Err(e) => MbaVerdict {
            verdict: "error".to_string(),
            always_zero: false,
            detail: format!("no se pudo parsear: {e:?}"),
        },
        Ok(expr) => {
            let (verdict, proof) = crate::re_mba::analyze_opaque_predicate(&expr);
            let always_zero = verdict == crate::re_mba::OpaqueVerdict::AlwaysZero;
            let name = match verdict {
                crate::re_mba::OpaqueVerdict::AlwaysZero => "opaca",
                crate::re_mba::OpaqueVerdict::AlwaysNonZero => "no-opaca",
                crate::re_mba::OpaqueVerdict::AlwaysConst(_) => "constante",
                crate::re_mba::OpaqueVerdict::Unknown => "indeciso",
            };
            MbaVerdict {
                verdict: name.to_string(),
                always_zero,
                detail: format!("{verdict:?} · {}", proof.detail),
            }
        }
    }
}

// ========================================================================= //
// 3. CAPA 5e — AES/ECB/PKCS5 (sin dependencias nuevas)
// ========================================================================= //

/// Multiplicación en GF(2^8).
fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut r = 0u8;
    while b != 0 {
        if b & 1 != 0 {
            r ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    r
}

/// Multiplicación por `x` en GF(2^8).
fn xtime(a: u8) -> u8 {
    let v = (a as u16) << 1;
    if v & 0x100 != 0 {
        (v ^ 0x1b) as u8
    } else {
        v as u8
    }
}

/// Construye la S-box de AES (inverso multiplicativo + transformación afín).
fn aes_sbox() -> [u8; 256] {
    let mut inv = [0u8; 256];
    for a in 1u16..256 {
        for b in 1u16..256 {
            if gf_mul(a as u8, b as u8) == 1 {
                inv[a as usize] = b as u8;
                break;
            }
        }
    }
    let mut sbox = [0u8; 256];
    for i in 0..256usize {
        let mut c = inv[i];
        let mut s = c;
        for _ in 0..4 {
            c = (c << 1) | (c >> 7);
            s ^= c;
        }
        sbox[i] = s ^ 0x63;
    }
    sbox
}

/// Expansión de clave de AES (128/192/256).
fn aes_key_expansion(key: &[u8]) -> Option<(Vec<u8>, usize)> {
    if key.len() % 4 != 0 {
        return None;
    }
    let nk = key.len() / 4;
    if !(4..=8).contains(&nk) {
        return None;
    }
    let nr = nk + 6;
    let total = 4 * (nr + 1);
    let mut w = vec![0u8; total * 4];
    w[..key.len()].copy_from_slice(key);
    let sbox = aes_sbox();
    let mut rcon = 1u8;
    for i in nk..total {
        let mut t = [
            w[4 * (i - 1)],
            w[4 * (i - 1) + 1],
            w[4 * (i - 1) + 2],
            w[4 * (i - 1) + 3],
        ];
        if i % nk == 0 {
            t.rotate_left(1);
            for x in t.iter_mut() {
                *x = sbox[*x as usize];
            }
            t[0] ^= rcon;
            rcon = xtime(rcon);
        } else if nk > 6 && i % nk == 4 {
            for x in t.iter_mut() {
                *x = sbox[*x as usize];
            }
        }
        for j in 0..4 {
            w[4 * i + j] = w[4 * (i - nk) + j] ^ t[j];
        }
    }
    Some((w, nr))
}

/// `AddRoundKey` del descifrado.
fn aes_add_round_key(s: &mut [u8; 16], w: &[u8], round: usize) {
    for c in 0..4 {
        for r in 0..4 {
            s[4 * c + r] ^= w[round * 16 + 4 * c + r];
        }
    }
}

/// `InvShiftRows`.
fn aes_inv_shift_rows(s: &mut [u8; 16]) {
    for r in 1..4usize {
        let row: [u8; 4] = [s[r], s[4 + r], s[8 + r], s[12 + r]];
        for c in 0..4usize {
            s[4 * c + r] = row[(c + 4 - r) % 4];
        }
    }
}

/// `InvMixColumns`.
fn aes_inv_mix_columns(s: &mut [u8; 16]) {
    for c in 0..4usize {
        let a = [s[4 * c], s[4 * c + 1], s[4 * c + 2], s[4 * c + 3]];
        s[4 * c] = gf_mul(0x0e, a[0]) ^ gf_mul(0x0b, a[1]) ^ gf_mul(0x0d, a[2]) ^ gf_mul(0x09, a[3]);
        s[4 * c + 1] =
            gf_mul(0x09, a[0]) ^ gf_mul(0x0e, a[1]) ^ gf_mul(0x0b, a[2]) ^ gf_mul(0x0d, a[3]);
        s[4 * c + 2] =
            gf_mul(0x0d, a[0]) ^ gf_mul(0x09, a[1]) ^ gf_mul(0x0e, a[2]) ^ gf_mul(0x0b, a[3]);
        s[4 * c + 3] =
            gf_mul(0x0b, a[0]) ^ gf_mul(0x0d, a[1]) ^ gf_mul(0x09, a[2]) ^ gf_mul(0x0e, a[3]);
    }
}

/// Descifra un bloque AES de 16 bytes en modo ECB (InvCipher, FIPS-197).
fn aes_decrypt_block(block: &[u8], w: &[u8], nr: usize, inv_sbox: &[u8; 256]) -> [u8; 16] {
    let mut s = [0u8; 16];
    s.copy_from_slice(&block[..16]);
    aes_add_round_key(&mut s, w, nr);
    let mut round = nr as i64 - 1;
    while round > 0 {
        aes_inv_shift_rows(&mut s);
        for b in s.iter_mut() {
            *b = inv_sbox[*b as usize];
        }
        aes_add_round_key(&mut s, w, round as usize);
        aes_inv_mix_columns(&mut s);
        round -= 1;
    }
    aes_inv_shift_rows(&mut s);
    for b in s.iter_mut() {
        *b = inv_sbox[*b as usize];
    }
    aes_add_round_key(&mut s, w, 0);
    s
}

/// Descifra en modo ECB (clave de 16, 24 o 32 bytes).
pub fn aes_ecb_decrypt(data: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    if data.len() % 16 != 0 || data.is_empty() {
        return None;
    }
    let (w, nr) = aes_key_expansion(key)?;
    let sbox = aes_sbox();
    let mut inv_sbox = [0u8; 256];
    for (i, v) in sbox.iter().enumerate() {
        inv_sbox[*v as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(16) {
        out.extend_from_slice(&aes_decrypt_block(chunk, &w, nr, &inv_sbox));
    }
    Some(out)
}

/// `ShiftRows` del cifrado (rotación a la izquierda de la fila `r`).
fn aes_shift_rows(s: &mut [u8; 16]) {
    for r in 1..4usize {
        let row: [u8; 4] = [s[r], s[4 + r], s[8 + r], s[12 + r]];
        for c in 0..4usize {
            s[4 * c + r] = row[(c + r) % 4];
        }
    }
}

/// `MixColumns` del cifrado.
fn aes_mix_columns(s: &mut [u8; 16]) {
    for c in 0..4usize {
        let a = [s[4 * c], s[4 * c + 1], s[4 * c + 2], s[4 * c + 3]];
        s[4 * c] = gf_mul(0x02, a[0]) ^ gf_mul(0x03, a[1]) ^ a[2] ^ a[3];
        s[4 * c + 1] = a[0] ^ gf_mul(0x02, a[1]) ^ gf_mul(0x03, a[2]) ^ a[3];
        s[4 * c + 2] = a[0] ^ a[1] ^ gf_mul(0x02, a[2]) ^ gf_mul(0x03, a[3]);
        s[4 * c + 3] = gf_mul(0x03, a[0]) ^ a[1] ^ a[2] ^ gf_mul(0x02, a[3]);
    }
}

/// Cifra un bloque AES de 16 bytes en modo ECB.
///
/// El motor sólo necesita *descifrar*, pero el cifrador da la simetría para
/// verificar la implementación (ida y vuelta) y para construir casos de
/// prueba con relleno PKCS#5 válido.
pub fn aes_ecb_encrypt(data: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    if data.len() % 16 != 0 || data.is_empty() {
        return None;
    }
    let (w, nr) = aes_key_expansion(key)?;
    let sbox = aes_sbox();
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(16) {
        let mut s = [0u8; 16];
        s.copy_from_slice(&chunk[..16]);
        aes_add_round_key(&mut s, &w, 0);
        for round in 1..nr {
            for b in s.iter_mut() {
                *b = sbox[*b as usize];
            }
            aes_shift_rows(&mut s);
            aes_mix_columns(&mut s);
            aes_add_round_key(&mut s, &w, round);
        }
        for b in s.iter_mut() {
            *b = sbox[*b as usize];
        }
        aes_shift_rows(&mut s);
        aes_add_round_key(&mut s, &w, nr);
        out.extend_from_slice(&s);
    }
    Some(out)
}

/// Quita el relleno PKCS#5/PKCS#7.
pub fn pkcs5_unpad(data: &[u8]) -> Option<&[u8]> {
    if data.is_empty() {
        return None;
    }
    let pad = *data.last().unwrap() as usize;
    if pad == 0 || pad > 16 || pad > data.len() {
        return None;
    }
    if data[data.len() - pad..].iter().any(|b| *b as usize != pad) {
        return None;
    }
    Some(&data[..data.len() - pad])
}

/// Descodifica base64 (alfabeto estándar y URL-safe, con o sin `=`).
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::new();
    let mut padding = 0usize;
    for ch in text.chars() {
        if ch == '=' {
            padding += 1;
            continue;
        }
        if ch.is_ascii_whitespace() {
            continue;
        }
        if padding > 0 {
            return None;
        }
        let v = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            _ => return None,
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// *Fallback* de la capa 5e: cifrado de bloque con **clave al final**.
///
/// Devuelve el texto en claro y cómo se obtuvo.
pub fn aes_fallback(blob: &[u8]) -> Option<(String, String)> {
    // El blob puede venir en base64 o en crudo.
    let mut candidatos: Vec<Vec<u8>> = Vec::new();
    if let Ok(text) = std::str::from_utf8(blob) {
        if let Some(dec) = base64_decode(text.trim()) {
            candidatos.push(dec);
        }
    }
    candidatos.push(blob.to_vec());

    for data in candidatos {
        for klen in [16usize, 24, 32] {
            if data.len() <= klen {
                continue;
            }
            let split = data.len() - klen;
            let (cifrado, key) = data.split_at(split);
            let Some(plain) = aes_ecb_decrypt(cifrado, key) else {
                continue;
            };
            let Some(sin_relleno) = pkcs5_unpad(&plain) else {
                continue;
            };
            let Ok(texto) = String::from_utf8(sin_relleno.to_vec()) else {
                continue;
            };
            if texto.chars().any(|c| c.is_control() && c != '\n' && c != '\t' && c != '\r') {
                continue;
            }
            return Some((
                texto,
                format!("AES/ECB/PKCS5 con clave de {klen} bytes al final del blob"),
            ));
        }
    }
    None
}

// ========================================================================= //
// 4. CAPA 5a — DETECCIÓN DE STUBS
// ========================================================================= //

/// Candidato a descifrador de strings.
#[derive(Debug, Clone, Serialize)]
pub struct Stub {
    /// Clase que contiene el descifrador.
    pub class: String,
    /// Nombre del método.
    pub method: String,
    /// Tipo de retorno (`Ljava/lang/String;`).
    pub return_type: String,
    /// `true` si el método es estático.
    pub is_static: bool,
    /// Parámetros (descriptores).
    pub params: Vec<String>,
    /// Tipo de la tabla que usa (`[C`, `[B`…), si se ha detectado.
    pub table_type: Option<String>,
    /// `true` si usa `javax/crypto` o `java/security`.
    pub uses_crypto: bool,
    /// `true` si usa base64.
    pub uses_base64: bool,
    /// `true` si usa XOR.
    pub uses_xor: bool,
    /// Clasificación: `string_decryptor` o `crypto_material`.
    pub kind: String,
    /// Confianza 0..100.
    pub confidence: u32,
    /// Evidencia textual.
    pub evidence: Vec<String>,
}

/// Detecta métodos que devuelven un `String` construido a partir de datos
/// cifrados (capa 5a).
///
/// No se limita a un protector: se apoya en **evidencia** (tabla `char[]`
/// o `byte[]`, XOR, `javax/crypto`, base64 y construcción de `String`).
pub fn find_decrypt_stubs(dex: &DexFile) -> Vec<Stub> {
    let mut out: Vec<Stub> = Vec::new();
    for class in &dex.classes {
        for em in class.direct_methods.iter().chain(class.virtual_methods.iter()) {
            let Some(code) = em.code.as_ref() else {
                continue;
            };
            let Some(m) = dex.methods.get(em.method_idx as usize) else {
                continue;
            };
            let proto = dex.protos.get(m.proto_idx as usize);
            let ret = proto.map(|p| p.return_type.clone()).unwrap_or_default();
            if ret != "Ljava/lang/String;" {
                continue;
            }
            let params = proto.map(|p| p.parameters.clone()).unwrap_or_default();
            let decoded = decode_all(&code.insns);
            let mut uses_xor = false;
            let mut uses_crypto = false;
            let mut uses_base64 = false;
            let mut creates_string = false;
            let mut table_type: Option<String> = None;

            for ins in &decoded {
                match ins.mnemonic.as_str() {
                    "xor-int" | "xor-int/2addr" | "xor-int/lit8" | "xor-int/lit16" => {
                        uses_xor = true;
                    }
                    "new-array" => {
                        let d = dex
                            .types
                            .get(ins.ref_index as usize)
                            .map(|t| t.descriptor.clone())
                            .unwrap_or_default();
                        if d == "[C" || d == "[B" {
                            table_type = Some(d);
                        }
                    }
                    "aget-char" | "aput-char" | "aget-byte" | "aput-byte" => {
                        if table_type.is_none() {
                            table_type = Some("[C".to_string());
                        }
                    }
                    _ => {}
                }
                if ins.mnemonic.starts_with("invoke-") {
                    if let Some(callee) = dex.methods.get(ins.ref_index as usize) {
                        match callee.class_name.as_str() {
                            "Ljavax/crypto/Cipher;" | "Ljavax/crypto/spec/SecretKeySpec;"
                            | "Ljavax/crypto/KeyGenerator;" | "Ljava/security/MessageDigest;"
                            | "Ljavax/crypto/Mac;" => uses_crypto = true,
                            "Landroid/util/Base64;" | "Ljava/util/Base64;" => uses_base64 = true,
                            "Ljava/lang/String;" if callee.name == "<init>" => {
                                creates_string = true;
                            }
                            _ => {}
                        }
                    }
                }
            }

            let mut confidence = 25u32;
            let mut evidence: Vec<String> = Vec::new();
            if let Some(t) = &table_type {
                confidence += 25;
                evidence.push(format!("tabla {t}"));
            }
            if uses_xor {
                confidence += 15;
                evidence.push("xor".to_string());
            }
            if uses_crypto {
                confidence += 15;
                evidence.push("javax/crypto".to_string());
            }
            if uses_base64 {
                confidence += 10;
                evidence.push("base64".to_string());
            }
            if creates_string {
                confidence += 10;
                evidence.push("construye String".to_string());
            }
            if confidence < 50 {
                continue;
            }
            let kind = if uses_crypto && table_type.is_none() {
                "crypto_material"
            } else {
                "string_decryptor"
            };
            out.push(Stub {
                class: class.class_name.clone(),
                method: m.name.clone(),
                return_type: ret,
                is_static: em.access_flags & 0x8 != 0,
                params,
                table_type,
                uses_crypto,
                uses_base64,
                uses_xor,
                kind: kind.to_string(),
                confidence: confidence.min(95),
                evidence,
            });
        }
    }
    out.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    out
}

// ========================================================================= //
// 5. CAPA 5b — SITIOS DE LLAMADA
// ========================================================================= //

/// Un sitio donde se invoca a un descifrador.
#[derive(Debug, Clone, Serialize)]
pub struct CallSite {
    /// DEX donde está el llamante.
    pub dex: String,
    /// Clase que llama.
    pub caller_class: String,
    /// Método que llama.
    pub caller_method: String,
    /// Offset de la invocación (unidades de 16 bits).
    pub offset: u32,
    /// Clase del descifrador invocado.
    pub callee_class: String,
    /// Método del descifrador invocado.
    pub callee_method: String,
    /// Texto del argumento (el blob cifrado), si se pudo resolver.
    pub argument: Option<String>,
    /// Registro del primer argumento.
    pub argument_register: Option<u16>,
    /// Offset de la instrucción que define el argumento.
    pub argument_offset: Option<u32>,
}

/// Enumera los sitios de llamada de un método en **todos** los DEX (capa 5b).
///
/// Es el paso que exige mirar en *todos* los ficheros: un descifrador suele
/// vivir en `classes2.dex` y se llama desde `classes.dex`.
///
/// **Coste**: construye el índice de invocaciones completo (una pasada por
/// todos los métodos). Para muchos stubs, usa [`build_call_index`] una sola
/// vez y [`call_sites_for`].
pub fn find_call_sites(
    dexes: &[(&str, &DexFile)],
    class: &str,
    method: &str,
) -> Vec<CallSite> {
    let index = build_call_index(dexes);
    call_sites_for(&index, dexes, class, method)
}

/// Índice de invocaciones: `(dex, índice de método)` → sitios de llamada.
///
/// Se construye en **una sola pasada** por todos los métodos de todos los
/// DEX: es lo que hace el motor escalable (sin esto, buscar los sitios de
/// N stubs cuesta N pasadas completas por el código).
pub type CallIndex = BTreeMap<(usize, u32), Vec<CallSite>>;

/// Construye el índice de invocaciones de todos los DEX.
pub fn build_call_index(dexes: &[(&str, &DexFile)]) -> CallIndex {
    let mut index: CallIndex = BTreeMap::new();
    for (dex_idx, (dex_name, dex)) in dexes.iter().enumerate() {
        // Qué métodos son estáticos (para saber si el primer registro es el
        // receptor o el primer argumento).
        let mut static_flags: HashMap<u32, bool> = HashMap::new();
        for cls in &dex.classes {
            for em in cls.direct_methods.iter().chain(cls.virtual_methods.iter()) {
                static_flags.insert(em.method_idx, em.access_flags & 0x8 != 0);
            }
        }
        for cls in &dex.classes {
            for em in cls.direct_methods.iter().chain(cls.virtual_methods.iter()) {
                let Some(code) = em.code.as_ref() else {
                    continue;
                };
                let Some(caller) = dex.methods.get(em.method_idx as usize) else {
                    continue;
                };
                let decoded = decode_all(&code.insns);
                for (idx, ins) in decoded.iter().enumerate() {
                    if !ins.mnemonic.starts_with("invoke-") {
                        continue;
                    }
                    let Some(callee) = dex.methods.get(ins.ref_index as usize) else {
                        continue;
                    };
                    // Primer argumento: en `invoke-static` es el registro 0 de
                    // la lista; en una instancia, el registro 1 (el 0 es el
                    // receptor).
                    let arg_reg: Option<u16> = if ins.regs.is_empty() {
                        None
                    } else if static_flags.get(&ins.ref_index).copied().unwrap_or(false) {
                        Some(ins.regs[0] as u16)
                    } else {
                        ins.regs.get(1).map(|r| *r as u16)
                    };
                    let mut argument = None;
                    let mut argument_offset = None;
                    if let Some(reg) = arg_reg {
                        for prev in decoded[..idx].iter().rev() {
                            if prev.mnemonic == "const-string" && as_reg(prev.a) == reg {
                                argument = dex.strings.get(prev.ref_index as usize).cloned();
                                argument_offset = Some(prev.offset);
                                break;
                            }
                        }
                    }
                    index.entry((dex_idx, ins.ref_index)).or_default().push(CallSite {
                        dex: dex_name.to_string(),
                        caller_class: cls.class_name.clone(),
                        caller_method: caller.name.clone(),
                        offset: ins.offset,
                        callee_class: callee.class_name.clone(),
                        callee_method: callee.name.clone(),
                        argument,
                        argument_register: arg_reg,
                        argument_offset,
                    });
                }
            }
        }
    }
    index
}

/// Sitios de llamada de un método, a partir del índice ya construido.
pub fn call_sites_for(
    index: &CallIndex,
    dexes: &[(&str, &DexFile)],
    class: &str,
    method: &str,
) -> Vec<CallSite> {
    let mut out: Vec<CallSite> = Vec::new();
    for (dex_idx, (_, dex)) in dexes.iter().enumerate() {
        for (m_idx, m) in dex.methods.iter().enumerate() {
            if m.class_name != class || m.name != method {
                continue;
            }
            if let Some(sites) = index.get(&(dex_idx, m_idx as u32)) {
                out.extend(sites.iter().cloned());
            }
        }
    }
    out
}

// ========================================================================= //
// 6. CAPA 5d — INTÉRPRETE DALVIK ACOTADO
// ========================================================================= //

/// Error de ejecución con el punto exacto del fallo.
#[derive(Debug, Clone, Serialize)]
pub struct ExecError {
    /// Qué pasó.
    pub message: String,
    /// Clase del método.
    pub class: String,
    /// Método.
    pub method: String,
    /// Offset de la instrucción.
    pub offset: u32,
}

impl ExecError {
    fn new(class: &str, method: &str, offset: u32, message: impl Into<String>) -> Self {
        ExecError {
            message: message.into(),
            class: class.to_string(),
            method: method.to_string(),
            offset,
        }
    }
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}->{} en 0x{:x}: {}",
            self.class, self.method, self.offset, self.message
        )
    }
}

impl std::error::Error for ExecError {}

/// Método localizado en los DEX.
#[derive(Debug, Clone)]
struct MethodInfo {
    dex_idx: usize,
    class_name: String,
    name: String,
    proto: String,
    is_static: bool,
    params: Vec<String>,
    code: DexCode,
}

/// Resultado de un paso de ejecución.
enum Step {
    /// Continúa en el offset indicado.
    Next(usize),
    /// Salta a un offset.
    Jump(i64),
    /// Devuelve un valor.
    Return(Value),
    /// Bifurca: se exploran las dos ramas.
    Fork(i64, i64),
}

/// Intérprete Dalvik acotado, con estado estático persistente.
pub struct Interpreter<'a> {
    dexes: &'a [&'a DexFile],
    /// Valores de campos estáticos (`clase.nombre` → valor).
    pub statics: BTreeMap<String, Value>,
    steps: u32,
    max_steps: u32,
    depth: u32,
    max_depth: u32,
    /// Notas del motor (qué API se emuló, qué se encontró…).
    pub notes: Vec<String>,
    /// Cuántas veces se ejecutó cada offset (para diagnosticar bucles y
    /// puntos calientes del código interpretado).
    pub visits: HashMap<u32, u64>,
    /// Ramas simultáneas máximas (controla la explosión combinatoria de las
    /// bifurcaciones con predicados indeterminados).
    pub max_frames: usize,
    /// Ramas creadas **en total** (techo del árbol de bifurcaciones).
    pub max_frames_total: u64,
    /// Devolver el primer resultado encontrado (modo rápido, el habitual
    /// para descifrar strings: cualquier texto en claro sirve).
    pub stop_on_first_result: bool,
    /// Registrar la traza de ejecución (diagnóstico).
    pub trace: bool,
    /// Traza de ejecución (hasta 400 entradas).
    pub trace_log: Vec<String>,
    /// Trazar **cada paso** (si no, sólo bifurcaciones y retornos).
    pub trace_steps: bool,
    /// Pasos máximos de una **rama** antes de descartarla (evita que una
    /// maraña de predicados opacos acapare el presupuesto).
    pub max_steps_per_branch: u32,
    /// Frames creados en total (diagnóstico).
    pub frames_created: u64,
    /// Caché de métodos localizados por (clase, nombre).
    ///
    /// Sin esta caché, cada llamada re-recorre todos los DEX y todas sus
    /// clases: con 5 DEX y 36.000 clases, una clave que se calcula 32.767
    /// veces (el bucle de construcción de la tabla de Jazztel) son miles de
    /// millones de comparaciones.
    cache: HashMap<(String, String), Option<MethodInfo>>,
    /// Caché de métodos localizados por (dex, índice de método).
    cache_idx: HashMap<(usize, u32), Option<MethodInfo>>,
}

/// Ramas vivas simultáneas por defecto.
///
/// HOLGADO a propósito: un bucle legítimo (p. ej. la construcción de la
/// tabla `char[32767]` de Jazztel, 32.767 iteraciones) encola una rama por
/// iteración, así que con un tope bajo el bucle se corta a mitad y la clave
/// queda incompleta. El coste real lo acota `max_frames_total`.
const MAX_FRAMES: usize = 4096;

/// Visitas máximas admitidas **por offset**.
///
/// Red de seguridad para bucles sin salida: si un offset se repite más de
/// este número de veces, la rama se corta.
///
/// El podado **estructural** (`bucle_cerrado`) es el que detecta el código
/// muerto del ofuscador; este límite sólo atrapa lo que se escape a aquél,
/// por eso es holgado (un bucle legítimo puede repetir un offset cientos de
/// miles de veces: la tabla `char[32767]` de Jazztel son 32.767 iteraciones
/// por offset, y el mismo bucle puede ejecutarse en varias ramas).
const MAX_VISITS_PER_OFFSET: u64 = 2_000_000;

impl<'a> Interpreter<'a> {
    /// Crea un intérprete sobre un conjunto de DEX.
    pub fn new(dexes: &'a [&'a DexFile]) -> Self {
        Interpreter {
            dexes,
            statics: BTreeMap::new(),
            steps: 0,
            max_steps: 200_000,
            depth: 0,
            max_depth: 12,
            notes: Vec::new(),
            visits: HashMap::new(),
            max_frames: MAX_FRAMES,
            max_frames_total: 200_000,
            stop_on_first_result: true,
            trace: false,
            trace_steps: false,
            max_steps_per_branch: 3_000_000,
            trace_log: Vec::new(),
            frames_created: 0,
            cache: HashMap::new(),
            cache_idx: HashMap::new(),
        }
    }

    /// Ajusta los límites de ejecución.
    pub fn with_limits(mut self, max_steps: u32, max_depth: u32) -> Self {
        self.max_steps = max_steps.max(1);
        self.max_depth = max_depth.max(1);
        self
    }

    /// Localiza un método por clase y nombre (con caché).
    fn find_method(&mut self, class: &str, method: &str) -> Option<MethodInfo> {
        let key = (class.to_string(), method.to_string());
        if let Some(hit) = self.cache.get(&key) {
            return hit.clone();
        }
        let found = self.find_method_uncached(class, method);
        self.cache.insert(key, found.clone());
        found
    }

    /// Localiza un método por clase y nombre (sin caché).
    fn find_method_uncached(&self, class: &str, method: &str) -> Option<MethodInfo> {
        for (dex_idx, dex) in self.dexes.iter().enumerate() {
            for cls in &dex.classes {
                for em in cls.direct_methods.iter().chain(cls.virtual_methods.iter()) {
                    let Some(m) = dex.methods.get(em.method_idx as usize) else {
                        continue;
                    };
                    if cls.class_name == class && m.name == method {
                        let Some(code) = em.code.clone() else {
                            continue;
                        };
                        let proto = dex.protos.get(m.proto_idx as usize);
                        return Some(MethodInfo {
                            dex_idx,
                            class_name: cls.class_name.clone(),
                            name: m.name.clone(),
                            proto: m.proto.clone(),
                            is_static: em.access_flags & 0x8 != 0,
                            params: proto.map(|p| p.parameters.clone()).unwrap_or_default(),
                            code,
                        });
                    }
                }
            }
        }
        None
    }

    /// Localiza un método por índice dentro de un DEX.
    ///
    /// **Multi-DEX**: el índice de método es local a cada DEX, pero una
    /// referencia *sí* puede apuntar a un método implementado en otro DEX
    /// (así seMEDIDO en el APK de Jazztel: `Lpx;->D` se referencia en
    /// `classes.dex` y se implementa en `classes4.dex`). Por eso, si el
    /// índice no resuelve, se busca por `(clase, nombre)` en todos los DEX.
    fn locate(&self, dex_idx: usize, method_idx: u32) -> Option<MethodInfo> {
        let dex = self.dexes.get(dex_idx)?;
        let m = dex.methods.get(method_idx as usize)?;
        let class_name = m.class_name.clone();
        let name = m.name.clone();
        // (a) Por índice, en el DEX del llamante.
        if let Some(info) = self.find_in_dex(dex_idx, Some(method_idx), &class_name, &name) {
            return Some(info);
        }
        // (b) Por (clase, nombre), en cualquier DEX.
        for i in 0..self.dexes.len() {
            if i == dex_idx {
                continue;
            }
            if let Some(info) = self.find_in_dex(i, None, &class_name, &name) {
                return Some(info);
            }
        }
        None
    }

    /// Busca la implementación de un método dentro de un DEX.
    fn find_in_dex(
        &self,
        dex_idx: usize,
        method_idx: Option<u32>,
        class_name: &str,
        name: &str,
    ) -> Option<MethodInfo> {
        let dex = self.dexes.get(dex_idx)?;
        for cls in &dex.classes {
            if cls.class_name != class_name {
                continue;
            }
            for em in cls.direct_methods.iter().chain(cls.virtual_methods.iter()) {
                let coincide = match method_idx {
                    Some(idx) => em.method_idx == idx,
                    None => dex
                        .methods
                        .get(em.method_idx as usize)
                        .map(|m| m.name == name)
                        .unwrap_or(false),
                };
                if !coincide {
                    continue;
                }
                let Some(code) = em.code.clone() else {
                    continue;
                };
                let m = dex.methods.get(em.method_idx as usize);
                let proto = m.and_then(|m| dex.protos.get(m.proto_idx as usize));
                return Some(MethodInfo {
                    dex_idx,
                    class_name: cls.class_name.clone(),
                    name: m.map(|m| m.name.clone()).unwrap_or_else(|| name.to_string()),
                    proto: m.map(|m| m.proto.clone()).unwrap_or_default(),
                    is_static: em.access_flags & 0x8 != 0,
                    params: proto.map(|p| p.parameters.clone()).unwrap_or_default(),
                    code,
                });
            }
        }
        None
    }

    /// Ejecuta el inicializador estático de una clase (`<clinit>`).
    ///
    /// Es el paso 5c: muchos empaquetadores construyen la clave (la tabla
    /// `char[]`) en el `<clinit>`,填 en un runtime, no en el DEX.
    pub fn call_static_init(&mut self, class: &str) -> Result<(), ExecError> {
        let Some(info) = self.find_method(class, "<clinit>") else {
            return Ok(());
        };
        self.call(&info.class_name.clone(), "<clinit>", &[])?;
        Ok(())
    }

    /// Ejecuta un `code_item` suelto, sin DEX.
    ///
    /// Útil para verificar el intérprete sobre código sintético y para el
    /// modo "intérprete suelto" (sin contexto de DEX).
    pub fn exec_raw(
        &mut self,
        insns: &[u16],
        registers_size: u16,
        args: &[Value],
    ) -> Result<Value, ExecError> {
        let info = MethodInfo {
            dex_idx: 0,
            class_name: "<raw>".to_string(),
            name: "<raw>".to_string(),
            proto: "()V".to_string(),
            is_static: true,
            params: args.iter().map(|_| "I".to_string()).collect(),
            code: DexCode {
                registers_size,
                ins_size: args.len() as u16,
                outs_size: 0,
                tries_size: 0,
                debug_info_off: 0,
                insns: insns.to_vec(),
                tries: Vec::new(),
                debug: None,
            },
        };
        self.exec_method(&info, args)
    }

    /// Ejecuta un método con argumentos.
    pub fn call(
        &mut self,
        class: &str,
        method: &str,
        args: &[Value],
    ) -> Result<Value, ExecError> {
        let info = self
            .find_method(class, method)
            .ok_or_else(|| ExecError::new(class, method, 0, "método no encontrado"))?;
        if self.depth >= self.max_depth {
            return Err(ExecError::new(
                class,
                method,
                0,
                format!("profundidad máxima ({}) alcanzada", self.max_depth),
            ));
        }
        self.depth += 1;
        let r = self.exec_method(&info, args);
        self.depth -= 1;
        r
    }

    /// Ejecuta el cuerpo de un método.
    fn exec_method(&mut self, info: &MethodInfo, args: &[Value]) -> Result<Value, ExecError> {
        let decoded = decode_all(&info.code.insns);
        let mut index: HashMap<u32, usize> = HashMap::new();
        for (i, ins) in decoded.iter().enumerate() {
            index.insert(ins.offset, i);
        }
        // **DFS** (pila LIFO): el motor baja por la rama "tomada" hasta
        // llegar a un `return`. En el código ofuscado, la rama que lleva al
        // código real suele ser la "tomada" (la "caída" es el predicado
        // opaco), así que DFS + parada en el primer resultado es lo rápido.
        // Código muerto: offsets desde los que no se puede volver.
        let con_return = returns_alcanzables(&decoded);
        let mut frames: Vec<Frame> = vec![self.build_frame(info, args)];
        let mut results: Vec<Value> = Vec::new();
        let mut errors: Vec<ExecError> = Vec::new();

        while let Some(mut fr) = frames.pop() {
            let mut pasos_rama = 0u32;
            loop {
                self.steps += 1;
                pasos_rama += 1;
                if pasos_rama > self.max_steps_per_branch {
                    errors.push(ExecError::new(
                        &info.class_name,
                        &info.name,
                        fr.pc as u32,
                        "rama descartada: demasiados pasos sin converger",
                    ));
                    break;
                }
                let visitas = self.visits.entry(fr.pc as u32).or_insert(0);
                if *visitas >= MAX_VISITS_PER_OFFSET {
                    errors.push(ExecError::new(
                        &info.class_name,
                        &info.name,
                        fr.pc as u32,
                        format!("bucle sin salida: 0x{:x} repetido {} veces", fr.pc, visitas),
                    ));
                    break;
                }
                *visitas += 1;
                if self.steps > self.max_steps {
                    errors.push(ExecError::new(
                        &info.class_name,
                        &info.name,
                        fr.pc as u32,
                        format!("límite de pasos ({}) alcanzado", self.max_steps),
                    ));
                    break;
                }
                let Some(idx) = index.get(&(fr.pc as u32)).copied() else {
                    errors.push(ExecError::new(
                        &info.class_name,
                        &info.name,
                        fr.pc as u32,
                        "salto a una dirección que no es instrucción",
                    ));
                    break;
                };
                let ins = &decoded[idx];
                if ins.is_payload {
                    fr.pc += ins.width as usize;
                    continue;
                }
                if !con_return.contains(&ins.offset) {
                    errors.push(ExecError::new(
                        &info.class_name,
                        &info.name,
                        ins.offset,
                        format!("código muerto: 0x{:x} no puede alcanzar un return", ins.offset),
                    ));
                    break;
                }
                if self.trace && self.trace_steps {
                    if self.trace_log.len() >= 200 {
                        self.trace_log.remove(0);
                    }
                    let regs: Vec<String> = fr
                        .regs
                        .iter()
                        .map(|v| match v {
                            Value::Int(i) => i.to_string(),
                            Value::Str(s) => format!("\"{s}\""),
                            Value::Unknown => "?".to_string(),
                            Value::Null => "null".to_string(),
                            o => o.to_text(),
                        })
                        .collect();
                    self.trace_log.push(format!(
                        "0x{:04x} {:<16} regs=[{}]",
                        ins.offset, ins.mnemonic, regs.join(", ")
                    ));
                }
                match self.step(info, &decoded, &con_return, ins, &mut fr) {
                    Ok(Step::Next(n)) => fr.pc = n,
                    Ok(Step::Jump(t)) => fr.pc = t as usize,
                    Ok(Step::Return(v)) => {
                        if self.trace {
                            if self.trace_log.len() >= 200 {
                                self.trace_log.remove(0);
                            }
                            self.trace_log.push(format!("RETURN en 0x{:04x} = {:?}", ins.offset, v));
                        }
                        results.push(v);
                        if self.stop_on_first_result {
                            frames.clear();
                        }
                        break;
                    }
                    Ok(Step::Fork(taken, not_taken)) => {
                        if self.trace {
                            if self.trace_log.len() >= 200 {
                                self.trace_log.remove(0);
                            }
                            self.trace_log.push(format!(
                                "BIFURCA en 0x{:04x}: tomada=0x{:x} caida=0x{:x}",
                                ins.offset, taken, not_taken
                            ));
                        }
                        if frames.len() < self.max_frames
                            && self.frames_created < self.max_frames_total
                        {
                            let mut other = fr.clone();
                            other.pc = not_taken as usize;
                            frames.push(other);
                            self.frames_created += 1;
                        }
                        fr.pc = taken as usize;
                    }
                    Err(e) => {
                        errors.push(e);
                        break;
                    }
                }
            }
        }

        if let Some(v) = results.into_iter().next() {
            Ok(v)
        } else if !errors.is_empty() {
            // Se agregan los mensajes de todas las ramas fallidas: el motor
            // bifurca, y cada rama puede fallar por un motivo distinto (una
            // rama opaca que no se puede decidir, una API no emulada…).
            let mut msgs: Vec<String> = Vec::new();
            for e in &errors {
                let m = format!("{} (0x{:x})", e.message, e.offset);
                if !msgs.contains(&m) {
                    msgs.push(m);
                }
            }
            Err(ExecError {
                message: msgs.join(" | "),
                class: errors[0].class.clone(),
                method: errors[0].method.clone(),
                offset: errors[0].offset,
            })
        } else {
            Err(ExecError::new(
                &info.class_name,
                &info.name,
                0,
                "el método no devolvió ningún valor",
            ))
        }
    }

    /// Construye el frame inicial (registros + argumentos).
    ///
    /// Convención **medida**: los parámetros de entrada ocupan los **últimos**
    /// registros del frame (`registers_size - n .. registers_size`), y el
    /// receptor (`this`) ocupa el primero de ese bloque.
    fn build_frame(&self, info: &MethodInfo, args: &[Value]) -> Frame {
        let total = info.code.registers_size.max(1) as usize;
        let mut regs = vec![Value::Unknown; total];
        if info.is_static {
            let n = args.len().min(total);
            for k in 0..n {
                regs[total - n + k] = args[k].clone();
            }
        } else {
            // Bloque de entrada = `this` + parámetros.
            let n = (info.params.len() + 1).min(total).min(args.len().max(1));
            if n > 0 {
                regs[total - n] = args.first().cloned().unwrap_or(Value::Unknown);
            }
            for k in 1..n {
                regs[total - n + k] = args.get(k).cloned().unwrap_or(Value::Unknown);
            }
        }
        Frame {
            pc: 0,
            regs,
            pending: Value::Unknown,
            dex_idx: info.dex_idx,
        }
    }

    /// Ejecuta una instrucción.
    fn step(
        &mut self,
        info: &MethodInfo,
        decoded: &[DecodedInsn],
        con_return: &BTreeSet<u32>,
        ins: &DecodedInsn,
        fr: &mut Frame,
    ) -> Result<Step, ExecError> {
        let op = ins.opcode;
        let next = ins.offset as usize + ins.width.max(1) as usize;
        let m = ins.mnemonic.as_str();
        let err = |msg: &str| ExecError::new(&info.class_name, &info.name, ins.offset, msg);

        match op {
            0x00 => Ok(Step::Next(next)),
            // --- movimientos ------------------------------------------- //
            0x01..=0x09 => {
                let v = rd(fr, as_reg(ins.b));
                wr(fr, as_reg(ins.a), v);
                Ok(Step::Next(next))
            }
            0x0a..=0x0c => {
                let v = fr.pending.clone();
                wr(fr, as_reg(ins.a), v);
                Ok(Step::Next(next))
            }
            0x0d => Ok(Step::Next(next)),
            0x0e => Ok(Step::Return(Value::Null)),
            0x0f | 0x10 | 0x11 => Ok(Step::Return(rd(fr, as_reg(ins.a)))),
            // --- constantes -------------------------------------------- //
            0x12..=0x15 => {
                wr(fr, as_reg(ins.a), Value::Int(ins.literal));
                Ok(Step::Next(next))
            }
            0x1a => {
                let s = self
                    .dexes
                    .get(fr.dex_idx)
                    .and_then(|d| d.strings.get(ins.ref_index as usize))
                    .cloned()
                    .unwrap_or_default();
                wr(fr, as_reg(ins.a), Value::Str(s));
                Ok(Step::Next(next))
            }
            0x1b => {
                let s = self
                    .dexes
                    .get(fr.dex_idx)
                    .and_then(|d| d.strings.get(ins.ref_index as usize))
                    .cloned()
                    .unwrap_or_default();
                wr(fr, as_reg(ins.a), Value::Str(s));
                Ok(Step::Next(next))
            }
            0x1c => {
                let d = self
                    .dexes
                    .get(fr.dex_idx)
                    .and_then(|d| d.types.get(ins.ref_index as usize))
                    .map(|t| t.descriptor.clone())
                    .unwrap_or_default();
                wr(fr, as_reg(ins.a), Value::Str(d));
                Ok(Step::Next(next))
            }
            0x1d | 0x1e | 0x1f => Ok(Step::Next(next)),
            0x20 => {
                wr(fr, as_reg(ins.a), Value::Int(0));
                Ok(Step::Next(next))
            }
            0x21 => {
                let len = match rd(fr, as_reg(ins.b)) {
                    Value::Bytes(v) => Some(v.borrow().len()),
                    Value::Chars(v) => Some(v.borrow().len()),
                    Value::Ints(v) => Some(v.borrow().len()),
                    Value::Objects(v) => Some(v.borrow().len()),
                    _ => None,
                };
                match len {
                    Some(n) => {
                        wr(fr, as_reg(ins.a), Value::Int(n as i64));
                        Ok(Step::Next(next))
                    }
                    None => Err(err("array-length sobre un valor que no es arreglo")),
                }
            }
            0x22 => {
                let d = self
                    .dexes
                    .get(fr.dex_idx)
                    .and_then(|d| d.types.get(ins.ref_index as usize))
                    .map(|t| t.descriptor.clone())
                    .unwrap_or_default();
                wr(
                    fr,
                    as_reg(ins.a),
                    Value::Object {
                        class: d,
                        fields: BTreeMap::new(),
                    },
                );
                Ok(Step::Next(next))
            }
            0x23 => {
                let size = rd(fr, as_reg(ins.b)).as_int().unwrap_or(0);
                if size < 0 || size > 1_000_000 {
                    return Err(err("new-array con tamaño inverosímil"));
                }
                let ty = self
                    .dexes
                    .get(fr.dex_idx)
                    .and_then(|d| d.types.get(ins.ref_index as usize))
                    .map(|t| t.descriptor.clone())
                    .unwrap_or_default();
                let v = match ty.as_str() {
                    "[C" => Value::Chars(Rc::new(RefCell::new(vec![0u16; size as usize]))),
                    "[B" => Value::Bytes(Rc::new(RefCell::new(vec![0u8; size as usize]))),
                    _ => Value::Ints(Rc::new(RefCell::new(vec![0i64; size as usize]))),
                };
                wr(fr, as_reg(ins.a), v);
                Ok(Step::Next(next))
            }
            0x26 => {
                let target = ins.branch_target.unwrap_or(0) as usize;
                self.fill_array_data(info, target, as_reg(ins.a), fr)?;
                Ok(Step::Next(next))
            }
            0x27 => Err(err("throw: el stub lanza una excepción")),
            0x28 | 0x29 | 0x2a => {
                let target = ins.branch_target.unwrap_or(next as i64);
                // Salto hacia atrás: si el bloque no tiene salida, es un
                // bucle muerto del ofuscador → se poda en el acto.
                if target < next as i64 && bucle_cerrado(decoded, target as u32, ins.offset) {
                    return Err(err(&format!(
                        "bucle sin salida desde 0x{:x} (código muerto)",
                        target
                    )));
                }
                Ok(Step::Jump(target))
            }
            0x2b | 0x2c => {
                let target = ins.branch_target.unwrap_or(0) as usize;
                let key = rd(fr, as_reg(ins.a)).as_int().unwrap_or(0);
                match self.switch_target(info, decoded, target, key, op == 0x2b) {
                    Some(t) => Ok(Step::Jump(t)),
                    None => Ok(Step::Next(next)),
                }
            }
            0x2d..=0x31 => {
                // cmp*: el resultado es 0/1 pero no se modela la aritmética
                // de punto flotante.
                wr(fr, as_reg(ins.a), Value::Int(0));
                Ok(Step::Next(next))
            }
            0x32..=0x37 => {
                let a = rd(fr, as_reg(ins.a)).as_int();
                let b = rd(fr, as_reg(ins.b)).as_int();
                self.branch(info, &decoded, &con_return, ins, fr, a, b, next, false)
            }
            0x38..=0x3d => {
                let a = rd(fr, as_reg(ins.a)).as_int();
                self.branch(info, &decoded, &con_return, ins, fr, a, Some(0), next, true)
            }
            0x44..=0x4a => {
                // Semántica Dalvik MEDIDA: `aget-char vAA, vBB, vCC` →
                // vAA = vCC[vBB]. El ARRAY es el último operando (vCC) y el
                // índice el intermedio (vBB).
                let arr = rd(fr, as_reg(ins.c));
                let idx = rd(fr, as_reg(ins.b)).as_int();
                match array_get(&arr, idx.unwrap_or(-1), m) {
                    Some(v) => {
                        wr(fr, as_reg(ins.a), v);
                        Ok(Step::Next(next))
                    }
                    None => Err(err("aget sobre un valor que no es arreglo")),
                }
            }
            0x4b..=0x51 => {
                // `aput-char vAA, vBB, vCC` → vCC[vBB] = vAA.
                let value = rd(fr, as_reg(ins.a));
                let mut arr = rd(fr, as_reg(ins.c));
                let idx = rd(fr, as_reg(ins.b)).as_int();
                if array_set(&mut arr, idx.unwrap_or(-1), value, m) {
                    wr(fr, as_reg(ins.c), arr);
                    Ok(Step::Next(next))
                } else {
                    Err(err("aput sobre un valor que no es arreglo"))
                }
            }
            0x52..=0x5f => {
                let field = self
                    .dexes
                    .get(fr.dex_idx)
                    .and_then(|d| d.fields.get(ins.ref_index as usize))
                    .map(|f| f.name.clone());
                match field {
                    Some(name) => {
                        let mut obj = rd(fr, as_reg(ins.b));
                        if op <= 0x58 {
                            // iget*: lectura.
                            let v = match &obj {
                                Value::Object { fields, .. } => {
                                    fields.get(&name).cloned().unwrap_or(Value::Unknown)
                                }
                                _ => Value::Unknown,
                            };
                            wr(fr, as_reg(ins.a), v);
                        } else {
                            // iput*: escritura en el objeto.
                            let value = rd(fr, as_reg(ins.a));
                            if let Value::Object { fields, .. } = &mut obj {
                                fields.insert(name, value);
                            }
                            wr(fr, as_reg(ins.b), obj);
                        }
                        Ok(Step::Next(next))
                    }
                    None => Err(err("campo no encontrado")),
                }
            }
            0x60..=0x6d => {
                let key = match self.static_key(fr.dex_idx, ins.ref_index) {
                    Some(k) => k,
                    None => return Err(err("campo estático no encontrado")),
                };
                if op <= 0x66 {
                    // sget*: lectura.
                    let v = self.statics.get(&key).cloned().unwrap_or(Value::Unknown);
                    wr(fr, as_reg(ins.a), v);
                } else {
                    // sput*: escritura (el valor va en `a`).
                    let v = rd(fr, as_reg(ins.a));
                    self.statics.insert(key, v);
                }
                Ok(Step::Next(next))
            }
            0x6e..=0x72 | 0x74..=0x78 => {
                let args: Vec<Value> = ins.regs.iter().map(|r| rd(fr, *r as u16)).collect();
                let callee = self.locate(fr.dex_idx, ins.ref_index);
                let (cls, name) = match &callee {
                    Some(c) => (c.class_name.clone(), c.name.clone()),
                    None => {
                        let m = self
                            .dexes
                            .get(fr.dex_idx)
                            .and_then(|d| d.methods.get(ins.ref_index as usize));
                        match m {
                            Some(m) => (m.class_name.clone(), m.name.clone()),
                            None => return Err(err("método invocado no encontrado")),
                        }
                    }
                };
                let is_static = callee
                    .as_ref()
                    .map(|c| c.is_static)
                    .unwrap_or_else(|| api_is_static(&cls, &name));
                // 1) API de plataforma emulada.
                if let Some(result) = self.emulate_api(&cls, &name, &args, is_static) {
                    let v = result?;
                    fr.pending = v.clone();
                    // Un **constructor** deja su resultado en el receptor:
                    // en Dalvik no hace falta `move-result` (MEDIDO en
                    // Jazztel: `new-instance v8, String;` +
                    // `invoke-direct {v8, v0}, String;-><init>([C)` +
                    // `return-object v8`).
                    if name == "<init>" && !ins.regs.is_empty() {
                        wr(fr, ins.regs[0] as u16, v);
                    }
                    Ok(Step::Next(next))
                } else if let Some(c) = callee {
                    // 2) Método del DEX: se ejecuta en línea.
                    let v = self.call(&c.class_name.clone(), &c.name.clone(), &args)?;
                    fr.pending = v.clone();
                    if name == "<init>" && !ins.regs.is_empty() {
                        wr(fr, ins.regs[0] as u16, v);
                    }
                    Ok(Step::Next(next))
                } else {
                    Err(err(&format!("no se puede emular {cls}->{name}")))
                }
            }
            0x7b..=0x8f => {
                let v = match op {
                    0x7b => rd(fr, as_reg(ins.b)).as_int().map(|x| wrap32(x.wrapping_neg())),
                    0x7c => rd(fr, as_reg(ins.b)).as_int().map(|x| wrap32(!x)),
                    0x8d => rd(fr, as_reg(ins.b)).as_int().map(|x| x as i8 as i64),
                    0x8e => rd(fr, as_reg(ins.b)).as_int().map(|x| x as u16 as i64),
                    0x8f => rd(fr, as_reg(ins.b)).as_int().map(|x| x as i16 as i64),
                    _ => None,
                };
                // Operando desconocido → resultado desconocido (no es un
                // error: el predicado se resolverá por bifurcación).
                let v = v.map(Value::Int).unwrap_or(Value::Unknown);
                wr(fr, as_reg(ins.a), v);
                Ok(Step::Next(next))
            }
            0x90..=0x9a => {
                let x = rd(fr, as_reg(ins.b)).as_int();
                let y = rd(fr, as_reg(ins.c)).as_int();
                let v = eval_bin(arith_kind(m).unwrap_or("add-int"), x, y)
                    .map(Value::Int)
                    .unwrap_or(Value::Unknown);
                wr(fr, as_reg(ins.a), v);
                Ok(Step::Next(next))
            }
            0xb0..=0xba => {
                let x = rd(fr, as_reg(ins.a)).as_int();
                let y = rd(fr, as_reg(ins.b)).as_int();
                let v = eval_bin(arith_kind(m).unwrap_or("add-int"), x, y)
                    .map(Value::Int)
                    .unwrap_or(Value::Unknown);
                wr(fr, as_reg(ins.a), v);
                Ok(Step::Next(next))
            }
            0xd0..=0xd7 => {
                let x = rd(fr, as_reg(ins.b)).as_int();
                let v = eval_bin(arith_kind(m).unwrap_or("add-int"), x, Some(ins.literal))
                    .map(Value::Int)
                    .unwrap_or(Value::Unknown);
                wr(fr, as_reg(ins.a), v);
                Ok(Step::Next(next))
            }
            0xd8..=0xe2 => {
                let x = rd(fr, as_reg(ins.b)).as_int();
                let v = eval_bin(arith_kind(m).unwrap_or("add-int"), x, Some(ins.literal))
                    .map(Value::Int)
                    .unwrap_or(Value::Unknown);
                wr(fr, as_reg(ins.a), v);
                Ok(Step::Next(next))
            }
            _ => Ok(Step::Next(next)),
        }
    }

    /// Resuelve una rama condicional.
    fn branch(
        &mut self,
        info: &MethodInfo,
        decoded: &[DecodedInsn],
        con_return: &BTreeSet<u32>,
        ins: &DecodedInsn,
        fr: &mut Frame,
        a: Option<i64>,
        b: Option<i64>,
        next: usize,
        against_zero: bool,
    ) -> Result<Step, ExecError> {
        let m = ins.mnemonic.as_str();
        let Some(op) = branch_op(m) else {
            return Ok(Step::Next(next));
        };
        let target = ins.branch_target.unwrap_or(next as i64);
        match (a, b) {
            (Some(x), Some(y)) => {
                let taken = compare(op, x, y);
                let _ = against_zero;
                if taken {
                    if target < next as i64 && bucle_cerrado(decoded, target as u32, ins.offset) {
                        return Err(ExecError::new(
                            &info.class_name,
                            &info.name,
                            ins.offset,
                            format!("bucle sin salida desde 0x{:x} (código muerto)", target),
                        ));
                    }
                    Ok(Step::Jump(target))
                } else {
                    Ok(Step::Next(next))
                }
            }
            _ => {
                // Condición indeterminada: se decide con el CFG, no por
                // gusto.  Un camino que **no puede alcanzar ningún return**
                // es código muerto del ofuscador (bloque opaco) y se
                // descarta; si sólo uno de los dos es viable, ése es el
                // bueno.  MEDIDO en Jazztel: en `0x26` la caída es el
                // bloque muerto `0x28..0xa1` y el salto hacia atrás (`0x0f`)
                // es el bucle real.
                let caida_viva = con_return.contains(&(next as u32));
                let tomada_viva = con_return.contains(&(target as u32));
                if tomada_viva && !caida_viva {
                    Ok(Step::Jump(target))
                } else if caida_viva && !tomada_viva {
                    Ok(Step::Next(next))
                } else if caida_viva {
                    Ok(Step::Fork(target, next as i64))
                } else {
                    Err(ExecError::new(
                        &info.class_name,
                        &info.name,
                        ins.offset,
                        "ninguna rama puede devolver un valor (código muerto)",
                    ))
                }
            }
        }
    }

    /// Destino de un `switch` (empaquetado o disperso).
    fn switch_target(
        &self,
        info: &MethodInfo,
        decoded: &[DecodedInsn],
        payload: usize,
        key: i64,
        packed: bool,
    ) -> Option<i64> {
        let insns = &info.code.insns;
        let ident = insns.get(payload).map(|u| *u >> 8).unwrap_or(0);
        let size = insns.get(payload + 1).copied().unwrap_or(0) as usize;
        let switch_at = decoded
            .iter()
            .find(|d| d.branch_target == Some(payload as i64))
            .map(|d| d.offset as i64)
            .unwrap_or(0);
        if packed && ident == 0x01 {
            let first = read_i32(insns, payload + 2);
            for k in 0..size {
                let off = read_i32(insns, payload + 4 + 2 * k);
                if first + k as i64 == key {
                    return Some(switch_at + off);
                }
            }
        } else if !packed && ident == 0x02 {
            for k in 0..size {
                let kk = read_i32(insns, payload + 2 + 2 * k);
                if kk == key {
                    let off = read_i32(insns, payload + 2 + 2 * size + 2 * k);
                    return Some(switch_at + off);
                }
            }
        }
        None
    }

    /// Rellena un arreglo con los datos de un payload `fill-array-data`.
    fn fill_array_data(
        &mut self,
        info: &MethodInfo,
        payload: usize,
        reg: u16,
        fr: &mut Frame,
    ) -> Result<(), ExecError> {
        let insns = &info.code.insns;
        let ew = insns.get(payload + 1).copied().unwrap_or(0) as usize;
        let count = read_i32(insns, payload + 2).max(0) as usize;
        let base = payload + 4;
        let mut arr = rd(fr, reg);
        for k in 0..count {
            let unit = insns.get(base + k).copied().unwrap_or(0);
            let value = match ew {
                1 => (unit & 0xff) as i64,
                2 => (unit & 0xffff) as i64,
                _ => unit as i64,
            };
            array_set(&mut arr, k as i64, Value::Int(value), "aput");
        }
        wr(fr, reg, arr);
        Ok(())
    }

    /// Clave de un campo estático (`clase.nombre`).
    fn static_key(&self, dex_idx: usize, field_idx: u32) -> Option<String> {
        let dex = self.dexes.get(dex_idx)?;
        let f = dex.fields.get(field_idx as usize)?;
        Some(format!("{}.{}", f.class_name, f.name))
    }

    /// Emula las APIs de plataforma más usadas por los descifradores.
    ///
    /// `args[0]` es el **receptor** en los métodos de instancia y el primer
    /// argumento real en los estáticos; `p(i)` da el argumento `i` real en
    /// ambos casos.
    fn emulate_api(
        &mut self,
        class: &str,
        method: &str,
        args: &[Value],
        is_static: bool,
    ) -> Option<Result<Value, ExecError>> {
        let recv = if is_static {
            Value::Unknown
        } else {
            args.first().cloned().unwrap_or(Value::Unknown)
        };
        let p = |i: usize| -> Value {
            let idx = if is_static { i } else { i + 1 };
            args.get(idx).cloned().unwrap_or(Value::Unknown)
        };
        match class {
            "Ljava/lang/String;" => Some(match method {
                "length" => Ok(Value::Int(match &recv {
                    Value::Str(t) => t.chars().count() as i64,
                    _ => 0,
                })),
                "charAt" => Ok(match (&recv, p(0).as_int()) {
                    (Value::Str(t), Some(i)) => {
                        t.chars().nth(i as usize).map(|c| Value::Int(c as u32 as i64))
                            .unwrap_or(Value::Unknown)
                    }
                    _ => Value::Unknown,
                }),
                "substring" => Ok(match (&recv, p(0).as_int()) {
                    (Value::Str(t), Some(i)) => {
                        let start = i.max(0) as usize;
                        let end = match p(1).as_int() {
                            Some(e) => e.max(0) as usize,
                            None => t.chars().count(),
                        };
                        Value::Str(t.chars().skip(start).take(end.saturating_sub(start)).collect())
                    }
                    _ => Value::Unknown,
                }),
                "toCharArray" => Ok(match &recv {
                    Value::Str(t) => Value::Chars(Rc::new(RefCell::new(t.chars().map(|c| c as u32 as u16).collect()))),
                    _ => Value::Unknown,
                }),
                "getBytes" => Ok(match &recv {
                    Value::Str(t) => Value::Bytes(Rc::new(RefCell::new(t.as_bytes().to_vec()))),
                    _ => Value::Unknown,
                }),
                "equals" => Ok(Value::Bool(
                    recv.as_text().unwrap_or("") == p(0).as_text().unwrap_or(""),
                )),
                "<init>" => Ok(value_to_string(&p(0))),
                "valueOf" => Ok(Value::Str(p(0).to_text())),
                "hashCode" => Ok(Value::Int(recv.to_text().len() as i64)),
                _ => Err(ExecError::new(class, method, 0, "API de String no emulada")),
            }),
            "Ljava/lang/StringBuilder;" | "Ljava/lang/StringBuffer;" => Some(match method {
                "<init>" => Ok(Value::Str(String::new())),
                "append" => Ok(Value::Str(format!(
                    "{}{}",
                    recv.as_text().unwrap_or(""),
                    p(0).to_text()
                ))),
                "toString" => Ok(Value::Str(recv.to_text())),
                "length" => Ok(Value::Int(recv.to_text().chars().count() as i64)),
                _ => Err(ExecError::new(class, method, 0, "API no emulada")),
            }),
            "Ljava/lang/Integer;" | "Ljava/lang/Long;" => Some(match method {
                "parseInt" | "parseLong" => Ok(
                    match p(0).as_text().and_then(|t| t.trim().parse::<i64>().ok()) {
                        Some(v) => Value::Int(v),
                        None => Value::Unknown,
                    },
                ),
                "valueOf" => Ok(Value::Int(p(0).as_int().unwrap_or(0))),
                "toString" => Ok(Value::Str(p(0).to_text())),
                "intValue" | "longValue" => Ok(Value::Int(recv.as_int().unwrap_or(0))),
                _ => Err(ExecError::new(class, method, 0, "API no emulada")),
            }),
            "Ljava/lang/Math;" => Some(match method {
                "abs" => Ok(Value::Int(p(0).as_int().unwrap_or(0).abs())),
                "max" => Ok(Value::Int(
                    p(0).as_int().unwrap_or(0).max(p(1).as_int().unwrap_or(0)),
                )),
                "min" => Ok(Value::Int(
                    p(0).as_int().unwrap_or(0).min(p(1).as_int().unwrap_or(0)),
                )),
                _ => Err(ExecError::new(class, method, 0, "API no emulada")),
            }),
            "Ljavax/crypto/spec/SecretKeySpec;" => Some(match method {
                "<init>" => {
                    let mut fields = BTreeMap::new();
                    if let Value::Bytes(b) = &p(0) {
                        fields.insert("key".to_string(), Value::Bytes(b.clone()));
                    }
                    fields.insert(
                        "alg".to_string(),
                        Value::Str(p(1).as_text().unwrap_or("AES").to_string()),
                    );
                    Ok(Value::Object {
                        class: class.to_string(),
                        fields,
                    })
                }
                _ => Err(ExecError::new(class, method, 0, "API no emulada")),
            }),
            "Ljavax/crypto/Cipher;" => Some(match method {
                "getInstance" => {
                    let mut fields = BTreeMap::new();
                    fields.insert(
                        "alg".to_string(),
                        Value::Str(p(0).as_text().unwrap_or("").to_string()),
                    );
                    Ok(Value::Object {
                        class: class.to_string(),
                        fields,
                    })
                }
                "init" => {
                    // init(int modo, Key clave): la clave se guarda para el
                    // `doFinal` siguiente.
                    if let Value::Object { fields, .. } = &p(1) {
                        if let Some(k) = fields.get("key") {
                            self.statics.insert("__cipher_key__".to_string(), k.clone());
                        }
                    }
                    Ok(Value::Null)
                }
                "doFinal" => {
                    let data: Vec<u8> = match &p(0) {
                        Value::Bytes(b) => b.borrow().clone(),
                        _ => Vec::new(),
                    };
                    let key: Vec<u8> = self
                        .statics
                        .get("__cipher_key__")
                        .and_then(|v| match v {
                            Value::Bytes(b) => Some(b.borrow().clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    match aes_ecb_decrypt(&data, &key) {
                        Some(plain) => {
                            let out = pkcs5_unpad(&plain).map(|p| p.to_vec()).unwrap_or(plain);
                            Ok(Value::Bytes(Rc::new(RefCell::new(out))))
                        }
                        None => Err(ExecError::new(class, method, 0, "AES/ECB falló")),
                    }
                }
                _ => Err(ExecError::new(class, method, 0, "API no emulada")),
            }),
            "Landroid/util/Base64;" | "Ljava/util/Base64;" => Some(match method {
                "decode" => Ok(match base64_decode(p(0).as_text().unwrap_or("")) {
                    Some(b) => Value::Bytes(Rc::new(RefCell::new(b))),
                    None => Value::Unknown,
                }),
                "encodeToString" => Ok(Value::Str(p(0).to_text())),
                _ => Err(ExecError::new(class, method, 0, "API no emulada")),
            }),
            _ => None,
        }
    }
}

/// Convierte un valor Dalvik en su representación de texto.
fn value_to_string(v: &Value) -> Value {
    match v {
        Value::Str(s) => Value::Str(s.clone()),
        Value::Chars(c) => Value::Str(c.borrow().iter().map(|x| *x as u32).filter_map(char::from_u32).collect()),
        Value::Bytes(b) => Value::Str(String::from_utf8_lossy(&b.borrow()).to_string()),
        other => Value::Str(other.to_text()),
    }
}

/// `true` si el método de una API de plataforma es estático.
fn api_is_static(class: &str, method: &str) -> bool {
    match class {
        "Ljava/lang/String;" => method == "valueOf",
        "Ljava/lang/Integer;" | "Ljava/lang/Long;" => {
            matches!(method, "parseInt" | "parseLong" | "valueOf" | "toString")
        }
        "Ljava/lang/Math;" => true,
        "Ljavax/crypto/Cipher;" => method == "getInstance",
        "Landroid/util/Base64;" | "Ljava/util/Base64;" => {
            matches!(method, "decode" | "encodeToString")
        }
        _ => false,
    }
}

/// Frame de ejecución.
#[derive(Clone)]
struct Frame {
    pc: usize,
    regs: Vec<Value>,
    pending: Value,
    dex_idx: usize,
}

/// `true` si el bloque `[inicio, fin]` **no tiene salida**: ningún salto
/// lleva fuera del rango y no hay ningún `return` dentro.
///
/// Los empaquetadores meten bucles cerrados (código muerto) para agotar el
/// presupuesto de cualquier emulador. Detectarlo por estructura (y no por
/// tiempo) es lo que permite podarlos en el acto.
fn bucle_cerrado(decoded: &[DecodedInsn], inicio: u32, fin: u32) -> bool {
    for ins in decoded {
        if ins.offset < inicio || ins.offset > fin {
            continue;
        }
        let m = ins.mnemonic.as_str();
        if m.starts_with("return") {
            return false; // hay salida
        }
        if let Some(t) = ins.branch_target {
            if t < 0 {
                continue;
            }
            let t = t as u32;
            if t < inicio || t > fin {
                return false; // hay salida
            }
        }
    }
    true
}

/// Offsets desde los que **sí se puede alcanzar un `return`**.
///
/// Análisis de CFG por encima del `code_item`. Sirve para podar el código
/// muerto del ofuscador: los bloques opacos (anillos de saltos que se
/// auto-bifurcan) no conductive a ningún `return` ⇒ se descartan de entrada
/// en vez de gastar el presupuesto explorándolos.
fn returns_alcanzables(decoded: &[DecodedInsn]) -> BTreeSet<u32> {
    // 1) Mapa offset -> índice (para saltar en O(1)).
    let mut pos: HashMap<u32, usize> = HashMap::new();
    for (i, ins) in decoded.iter().enumerate() {
        pos.insert(ins.offset, i);
    }
    // 2) Predecesores de cada offset (para propagar hacia atrás).
    let mut pred: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut pila: Vec<u32> = Vec::new();
    for (i, ins) in decoded.iter().enumerate() {
        if ins.is_payload {
            continue;
        }
        let m = ins.mnemonic.as_str();
        if m.starts_with("return") {
            pila.push(ins.offset);
            continue;
        }
        if let Some(t) = ins.branch_target {
            if t >= 0 {
                pred.entry(t as u32).or_default().push(ins.offset);
            }
        }
        if !m.starts_with("goto") && !m.starts_with("throw") {
            // El fall-through salta los payloads (datos de switch): la
            // siguiente instrucción *real* es la que sigue.
            let siguiente = decoded
                .get(i + 1..)
                .and_then(|rest| {
                    rest.iter()
                        .enumerate()
                        .find(|(_, d)| !d.is_payload)
                        .map(|(k, d)| d.offset)
                });
            if let Some(off) = siguiente {
                pred.entry(off).or_default().push(ins.offset);
            }
        }
    }
    // 3) BFS inverso: desde cada `return` hacia atrás.
    let mut alcanzables: BTreeSet<u32> = BTreeSet::new();
    while let Some(off) = pila.pop() {
        if !alcanzables.insert(off) {
            continue;
        }
        if let Some(ps) = pred.get(&off) {
            for p in ps {
                if !alcanzables.contains(p) {
                    pila.push(*p);
                }
            }
        }
    }
    alcanzables
}

/// Lee un registro (fuera de rango → `Unknown`, nunca `panic`).
fn rd(fr: &Frame, r: u16) -> Value {
    fr.regs.get(r as usize).cloned().unwrap_or(Value::Unknown)
}

/// Escribe un registro (crece el frame si hace falta).
fn wr(fr: &mut Frame, r: u16, v: Value) {
    let i = r as usize;
    if i >= fr.regs.len() {
        fr.regs.resize(i + 1, Value::Unknown);
    }
    fr.regs[i] = v;
}

/// Lee un entero de 32 bits (dos code units, little endian).
fn read_i32(insns: &[u16], at: usize) -> i64 {
    let lo = insns.get(at).copied().unwrap_or(0) as u32;
    let hi = insns.get(at + 1).copied().unwrap_or(0) as u32;
    (lo | (hi << 16)) as i32 as i64
}

// ========================================================================= //
// 7. CAPA 5 — TABLA DE STRINGS
// ========================================================================= //

/// String descifrado.
#[derive(Debug, Clone, Serialize)]
pub struct StringEntry {
    /// Texto cifrado (el blob del DEX).
    pub blob: String,
    /// Texto en claro.
    pub plaintext: String,
    /// Clase del descifrador.
    pub stub_class: String,
    /// Método del descifrador.
    pub stub_method: String,
    /// DEX donde está el llamante.
    pub dex: String,
    /// Clase del llamante.
    pub caller_class: String,
    /// Método del llamante.
    pub caller_method: String,
    /// Cómo se obtuvo (`intérprete`, `AES/ECB`, …).
    pub method: String,
    /// Confianza 0..100.
    pub confidence: u32,
}

/// Tabla de strings descifrados.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StringTable {
    /// Strings descifrados.
    pub entries: Vec<StringEntry>,
    /// Stubs usados.
    pub stubs: Vec<Stub>,
    /// Blobs que no se pudieron descifrar.
    pub unresolved: Vec<String>,
}

/// Opciones del motor.
#[derive(Debug, Clone, Serialize)]
pub struct DeobfOptions {
    /// Pasos máximos del intérprete.
    pub max_steps: u32,
    /// Profundidad máxima de llamadas.
    pub max_depth: u32,
    /// Usar el *fallback* AES/ECB.
    pub use_aes_fallback: bool,
    /// Máximo de strings en la tabla.
    pub max_entries: usize,
    /// Máximo de stubs a descifrar (se ordenan por número de sitios de
    /// llamada: los más usados primero).
    pub max_stubs: usize,
}

impl Default for DeobfOptions {
    fn default() -> Self {
        DeobfOptions {
            max_steps: 200_000,
            max_depth: 12,
            use_aes_fallback: true,
            max_entries: 5_000,
            max_stubs: 40,
        }
    }
}

/// Descifra todas las strings que se pueden (capa 5 completa).
///
/// Construye el índice de invocaciones (una pasada) y luego itera.
pub fn decrypt_strings(
    dexes: &[(&str, &DexFile)],
    stubs: &[Stub],
    opts: &DeobfOptions,
) -> StringTable {
    let index = build_call_index(dexes);
    decrypt_strings_with_index(dexes, &index, stubs, opts)
}

/// Descifra todas las strings usando un índice de invocaciones ya construido.
pub fn decrypt_strings_with_index(
    dexes: &[(&str, &DexFile)],
    index: &CallIndex,
    stubs: &[Stub],
    opts: &DeobfOptions,
) -> StringTable {
    let mut table = StringTable::default();
    table.stubs = stubs.to_vec();
    let refs: Vec<&DexFile> = dexes.iter().map(|(_, d)| *d).collect();

    for stub in stubs {
        let sites = call_sites_for(index, dexes, &stub.class, &stub.method);
        if sites.is_empty() {
            continue;
        }
        // 5c: la clave suele construirse en el `<clinit>` de la clase. Se
        // ejecuta **una vez por stub** (como en la JVM), no por sitio.
        let mut interp = Interpreter::new(&refs).with_limits(opts.max_steps, opts.max_depth);
        let _ = interp.call_static_init(&stub.class);
        for site in sites {
            if table.entries.len() >= opts.max_entries {
                return table;
            }
            let Some(blob) = site.argument.clone() else {
                continue;
            };
            if blob.is_empty() {
                continue;
            }
            // En un método de instancia, el primer argumento es el receptor.
            let call_args: Vec<Value> = if stub.is_static {
                vec![Value::Str(blob.clone())]
            } else {
                vec![
                    Value::Object {
                        class: stub.class.clone(),
                        fields: BTreeMap::new(),
                    },
                    Value::Str(blob.clone()),
                ]
            };
            let mut plaintext: Option<(String, String, u32)> = None;
            if let Ok(Value::Str(clear)) = interp.call(&stub.class, &stub.method, &call_args) {
                if !clear.is_empty() {
                    plaintext = Some((clear, "intérprete".to_string(), 90));
                }
            }
            // 5e: fallback de cifrado de bloque con clave al final del blob.
            if plaintext.is_none() && opts.use_aes_fallback {
                if let Some((clear, how)) = aes_fallback(blob.as_bytes()) {
                    plaintext = Some((clear, how, 70));
                }
            }
            match plaintext {
                Some((clear, how, confidence)) => {
                    table.entries.push(StringEntry {
                        blob,
                        plaintext: clear,
                        stub_class: stub.class.clone(),
                        stub_method: stub.method.clone(),
                        dex: site.dex.clone(),
                        caller_class: site.caller_class.clone(),
                        caller_method: site.caller_method.clone(),
                        method: how,
                        confidence,
                    });
                }
                None => {
                    if !table.unresolved.contains(&blob) {
                        table.unresolved.push(blob);
                    }
                }
            }
        }
    }
    table
}

// ========================================================================= //
// 8. CAPA 6 — HUELLA DE EMPAQUETADOR / PROTECTOR / ANTI-ANÁLISIS
// ========================================================================= //

/// Hallazgo de la capa 6.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// Familia: `packer`, `crypto`, `anti-debug`, `loader`, `native`…
    pub category: String,
    /// Nombre del detector que lo dispara.
    pub detector: String,
    /// Severidad: `info`, `low`, `medium`, `high`.
    pub severity: String,
    /// Confianza 0..100.
    pub confidence: u32,
    /// Evidencia concreta.
    pub evidence: String,
}

/// Tabla de huellas de empaquetadores conocidos (clases y librerías).
const PACKER_MARKERS: &[&str] = &[
    "com/secshell/",
    "com/shell/",
    "com/bangcle/",
    "com/secneo/",
    "com/qihoo/util/",
    "com/tencent/StubShell",
    "com/tencent/midas",
    "com/aliyun/mobisec/",
    "com/nqshield/",
    "com/payegis/",
    "com/lm/",
    "com/tencent/tp2/",
    "com/papaback/",
    "com/mobisec/",
    "com/diql/",
];

/// Librerías nativas de protectores Android conocidas.
const PACKER_LIBS: &[&str] = &[
    "libjiagu",
    "libdexhelper",
    "libsecexe",
    "libchaosvmp",
    "libexecmain",
    "libshell",
    "libprotect",
    "libnesec",
    "libDexHelper",
    "libfakejni",
    "libbaiduprotect",
    "libBugly",
];

/// Huellas de APK (capa 6): loaders, cifrado, anti-análisis, assets.
pub fn fingerprint_apk(
    dexes: &[(&str, &DexFile)],
    assets: &[(String, Vec<u8>)],
    native_libs: &[(String, Vec<u8>)],
) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();

    // --- loaders dinámicos -------------------------------------------- //
    let mut loader_hits: Vec<String> = Vec::new();
    for (_, dex) in dexes {
        for s in &dex.strings {
            if s.contains("dalvik/system/DexClassLoader")
                || s.contains("dalvik/system/InMemoryDexClassLoader")
                || s.contains("dalvik/system/PathClassLoader")
                || s.contains("dalvik/system/DexFile;->loadDex")
            {
                loader_hits.push(s.clone());
            }
        }
    }
    if !loader_hits.is_empty() {
        out.push(Finding {
            category: "loader".to_string(),
            detector: "cargador_dinamico".to_string(),
            severity: "high".to_string(),
            confidence: 85,
            evidence: format!("{} referencias a cargadores de DEX", loader_hits.len()),
        });
    }

    // --- clases de empaquetadores conocidos ---------------------------- //
    let mut packer_classes: BTreeSet<String> = BTreeSet::new();
    for (_, dex) in dexes {
        for c in &dex.classes {
            for marker in PACKER_MARKERS {
                if c.class_name.contains(marker) {
                    packer_classes.insert(c.class_name.clone());
                }
            }
        }
    }
    if !packer_classes.is_empty() {
        let muestra: Vec<String> = packer_classes.iter().take(5).cloned().collect();
        out.push(Finding {
            category: "packer".to_string(),
            detector: "clases_de_empaquetador".to_string(),
            severity: "high".to_string(),
            confidence: 90,
            evidence: format!("{} clases ({})", packer_classes.len(), muestra.join(", ")),
        });
    }

    // --- librerías nativas de protectores ------------------------------ //
    let mut packer_libs: Vec<String> = Vec::new();
    for (name, _) in native_libs {
        let lower = name.to_lowercase();
        for lib in PACKER_LIBS {
            if lower.contains(&lib.to_lowercase()) {
                packer_libs.push(name.clone());
                break;
            }
        }
    }
    if !packer_libs.is_empty() {
        out.push(Finding {
            category: "packer".to_string(),
            detector: "libreria_nativa_de_protector".to_string(),
            severity: "high".to_string(),
            confidence: 85,
            evidence: packer_libs.join(", "),
        });
    }

    // --- cifrado de strings -------------------------------------------- //
    let mut crypto_users: Vec<String> = Vec::new();
    for (name, dex) in dexes {
        let n = dex
            .strings
            .iter()
            .filter(|s| s.contains("javax/crypto/Cipher;") || s.contains("javax/crypto/spec/"))
            .count();
        if n > 0 {
            crypto_users.push(format!("{name}:{n}"));
        }
    }
    if !crypto_users.is_empty() {
        out.push(Finding {
            category: "crypto".to_string(),
            detector: "cifrado_de_strings".to_string(),
            severity: "medium".to_string(),
            confidence: 70,
            evidence: crypto_users.join(", "),
        });
    }

    // --- anti-debug / RASP --------------------------------------------- //
    let mut antidbg: Vec<String> = Vec::new();
    for (_, dex) in dexes {
        for s in &dex.strings {
            if s.contains("android/os/Debug;")
                || s.contains("TracerPid")
                || s.contains("ptrace")
                || s.contains("/proc/self/status")
            {
                antidbg.push(s.clone());
            }
        }
    }
    if !antidbg.is_empty() {
        out.push(Finding {
            category: "anti-debug".to_string(),
            detector: "anti_debug".to_string(),
            severity: "high".to_string(),
            confidence: 75,
            evidence: format!("{} señales ({})", antidbg.len(), antidbg[0]),
        });
    }

    let mut root_checks: Vec<String> = Vec::new();
    for (_, dex) in dexes {
        for s in &dex.strings {
            if s.contains("/system/bin/su")
                || s.contains("/system/xbin/su")
                || s.contains("test-keys")
                || s.contains("magisk")
            {
                root_checks.push(s.clone());
            }
        }
    }
    if !root_checks.is_empty() {
        out.push(Finding {
            category: "anti-analisis".to_string(),
            detector: "deteccion_de_root".to_string(),
            severity: "medium".to_string(),
            confidence: 70,
            evidence: format!("{} señales ({})", root_checks.len(), root_checks[0]),
        });
    }

    // --- TLS pinning ---------------------------------------------------- //
    let mut pinning = false;
    for (_, dex) in dexes {
        if dex
            .strings
            .iter()
            .any(|s| s.contains("okhttp3/CertificatePinner") || s.contains("TrustManagerImpl"))
        {
            pinning = true;
        }
    }
    if pinning {
        out.push(Finding {
            category: "red".to_string(),
            detector: "tls_pinning".to_string(),
            severity: "medium".to_string(),
            confidence: 65,
            evidence: "CertificatePinner / TrustManager propio".to_string(),
        });
    }

    // --- entropía de assets --------------------------------------------- //
    for (name, data) in assets {
        if data.len() < 1024 {
            continue;
        }
        let e = crate::re_deep::entropy(data);
        if e > 7.2 {
            out.push(Finding {
                category: "assets".to_string(),
                detector: "asset_cifrado".to_string(),
                severity: "medium".to_string(),
                confidence: 60,
                evidence: format!("{name}: entropía {e:.2} en {} bytes", data.len()),
            });
        }
    }

    // --- nativo ---------------------------------------------------------- //
    for (name, data) in native_libs {
        for f in fingerprint_native(data, name) {
            out.push(f);
        }
    }

    out
}

/// Huellas de un binario nativo (capa 6): UPX, OLLVM, Tigress, VMProtect.
pub fn fingerprint_native(data: &[u8], name: &str) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    if data.is_empty() {
        return out;
    }
    let lower = name.to_lowercase();

    // --- UPX ------------------------------------------------------------ //
    let upx_sections = if data.starts_with(b"\x7fELF") {
        goblin::elf::Elf::parse(data)
            .ok()
            .map(|elf| {
                elf.section_headers
                    .iter()
                    .filter_map(|sh| elf.shdr_strtab.get_at(sh.sh_name))
                    .filter(|s| s.starts_with("UPX"))
                    .count()
            })
            .unwrap_or(0)
    } else {
        0
    };
    if upx_sections > 0 || find_bytes(data, b"UPX!").is_some() {
        out.push(Finding {
            category: "native".to_string(),
            detector: "upx".to_string(),
            severity: "high".to_string(),
            confidence: 90,
            evidence: format!("{name}: firma/secciones UPX ({upx_sections} secciones)"),
        });
    }

    // --- VMProtect / Themida ------------------------------------------- //
    let vmp_sections = if data.starts_with(b"\x7fELF") {
        goblin::elf::Elf::parse(data)
            .ok()
            .map(|elf| {
                elf.section_headers
                    .iter()
                    .filter_map(|sh| elf.shdr_strtab.get_at(sh.sh_name))
                    .filter(|s| s.starts_with(".vmp") || s.starts_with(".themida"))
                    .count()
            })
            .unwrap_or(0)
    } else {
        0
    };
    if vmp_sections > 0 {
        out.push(Finding {
            category: "native".to_string(),
            detector: "vmprotect".to_string(),
            severity: "high".to_string(),
            confidence: 85,
            evidence: format!("{name}: {vmp_sections} secciones .vmp*"),
        });
    }
    for marker in ["VMProtect", "Themida", "WinLicense", "Code Virtualizer"] {
        if find_bytes(data, marker.as_bytes()).is_some() {
            out.push(Finding {
                category: "native".to_string(),
                detector: "protector_comercial".to_string(),
                severity: "high".to_string(),
                confidence: 70,
                evidence: format!("{name}: cadena \"{marker}\""),
            });
            break;
        }
    }

    // --- Tigress -------------------------------------------------------- //
    for marker in [".tig", "Tigress", "tigress"] {
        if find_bytes(data, marker.as_bytes()).is_some() {
            out.push(Finding {
                category: "native".to_string(),
                detector: "tigress".to_string(),
                severity: "high".to_string(),
                confidence: 65,
                evidence: format!("{name}: marca \"{marker}\""),
            });
            break;
        }
    }

    // --- OLLVM ----------------------------------------------------------- //
    // Los marcadores de OLLVM aparecen en los nombres de las funciones
    // exportadas (`bcf`, `sub`, `upd`, `fla`) o en la tabla de símbolos.
    if data.starts_with(b"\x7fELF") {
        if let Ok(elf) = goblin::elf::Elf::parse(data) {
            let mut hits = 0usize;
            for sym in elf.syms.iter().chain(elf.dynsyms.iter()) {
                if let Some(name) = elf.strtab.get_at(sym.st_name) {
                    let n = name.to_lowercase();
                    if n.contains("bcf")
                        || n.contains("fla")
                        || n.contains("upd")
                        || n.contains("ollvm")
                        || n.contains("sub_")
                        || n.contains("const_c")
                    {
                        hits += 1;
                    }
                }
            }
            if hits >= 3 {
                out.push(Finding {
                    category: "native".to_string(),
                    detector: "ollvm".to_string(),
                    severity: "medium".to_string(),
                    confidence: 60,
                    evidence: format!("{name}: {hits} símbolos con patrón OLLVM"),
                });
            }
        }
    }

    // --- entropía global ------------------------------------------------ //
    let e = crate::re_deep::entropy(data);
    if e > 7.5 && data.len() > 4096 {
        out.push(Finding {
            category: "native".to_string(),
            detector: "binario_empaquetado".to_string(),
            severity: "medium".to_string(),
            confidence: 55,
            evidence: format!("{name}: entropía {e:.2}"),
        });
    }

    let _ = lower;
    out
}

/// Busca una cadena de bytes dentro de un binario.
fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

// ========================================================================= //
// 9. CAPA 7 — SALIDA
// ========================================================================= //

/// Informe completo del motor.
#[derive(Debug, Clone, Serialize)]
pub struct DeobfReport {
    /// Auditoría de la decodificación (gate del motor).
    pub audit: crate::re_deep::DecodeAudit,
    /// Stubs detectados.
    pub stubs: Vec<Stub>,
    /// Sitios de llamada (resumen por stub).
    pub call_sites: Vec<CallSite>,
    /// Strings descifrados.
    pub strings: StringTable,
    /// Hallazgos de la capa 6.
    pub findings: Vec<Finding>,
    /// Avisos y limitaciones.
    pub warnings: Vec<String>,
}

/// Ejecuta el motor completo sobre un conjunto de DEX.
pub fn analyze(
    dexes: &[(&str, &DexFile)],
    assets: &[(String, Vec<u8>)],
    native_libs: &[(String, Vec<u8>)],
    opts: &DeobfOptions,
) -> DeobfReport {
    let refs: Vec<&DexFile> = dexes.iter().map(|(_, d)| *d).collect();
    let audit = crate::re_deep::audit_decode(&refs);

    let mut stubs: Vec<Stub> = Vec::new();
    for (_, dex) in dexes {
        for s in find_decrypt_stubs(dex) {
            if !stubs
                .iter()
                .any(|e| e.class == s.class && e.method == s.method)
            {
                stubs.push(s);
            }
        }
    }
    stubs.sort_by(|a, b| b.confidence.cmp(&a.confidence));

    let mut call_sites: Vec<CallSite> = Vec::new();
    let index = build_call_index(dexes);
    for s in &stubs {
        call_sites.extend(call_sites_for(&index, dexes, &s.class, &s.method));
    }

    // Ordenar los stubs por número de sitios de llamada (los más usados
    // primero) y podar: descifrar 380 stubs con clave propia es carísimo.
    let mut conteo: HashMap<(String, String), usize> = HashMap::new();
    for c in &call_sites {
        *conteo
            .entry((c.callee_class.clone(), c.callee_method.clone()))
            .or_insert(0) += 1;
    }
    stubs.sort_by(|a, b| {
        let ca = conteo
            .get(&(a.class.clone(), a.method.clone()))
            .copied()
            .unwrap_or(0);
        let cb = conteo
            .get(&(b.class.clone(), b.method.clone()))
            .copied()
            .unwrap_or(0);
        cb.cmp(&ca).then_with(|| b.confidence.cmp(&a.confidence))
    });
    if stubs.len() > opts.max_stubs {
        stubs.truncate(opts.max_stubs);
        call_sites.clear();
        for s in &stubs {
            call_sites.extend(call_sites_for(&index, dexes, &s.class, &s.method));
        }
    }

    let strings = decrypt_strings_with_index(dexes, &index, &stubs, opts);
    let findings = fingerprint_apk(dexes, assets, native_libs);

    let mut warnings: Vec<String> = Vec::new();
    if audit.veredicto != "ok" {
        warnings.push(format!(
            "auditoría de decodificación degradada ({}): los resultados pueden estar desplazados",
            audit.veredicto
        ));
    }
    if stubs.is_empty() {
        warnings.push("no se ha detectado ningún stub de descifrado".to_string());
    }
    warnings.push(
        "el intérprete es acotado: las APIs de plataforma no emuladas selstán como error"
            .to_string(),
    );

    DeobfReport {
        audit,
        stubs,
        call_sites,
        strings,
        findings,
        warnings,
    }
}

/// Convierte el informe en JSON (capa 7).
pub fn report_json(report: &DeobfReport) -> Json {
    json!({
        "capa_4_folding": {
            "nota": "el folding se expone por instrucción (fold_constants)",
        },
        "capa_5_strings": {
            "stubs": report.stubs.len(),
            "sitios_de_llamada": report.call_sites.len(),
            "descifrados": report.strings.entries.len(),
            "sin_resolver": report.strings.unresolved.len(),
            "entradas": report.strings.entries,
            "stubs_detalle": report.stubs,
        },
        "capa_6_huellas": report.findings,
        "auditoria": {
            "veredicto": report.audit.veredicto,
            "encaje_bloques": report.audit.encaje_bloques,
            "alineacion_saltos": report.audit.alineacion_saltos,
            "encaje_firmas": report.audit.encaje_firmas,
        },
        "avisos": report.warnings,
    })
}

/// Tabla de strings en CSV (capa 7).
pub fn strings_csv(table: &StringTable) -> String {
    let mut out = String::from("blob,claro,descifrador,metodo,dex,llamante,confianza\n");
    for e in &table.entries {
        out.push_str(&format!(
            "\"{}\",\"{}\",\"{}->{}\",\"{}\",\"{}\",\"{}->{}\",{}\n",
            e.blob.replace('"', "\"\""),
            e.plaintext.replace('"', "\"\""),
            e.stub_class,
            e.stub_method,
            e.method,
            e.dex,
            e.caller_class,
            e.caller_method,
            e.confidence
        ));
    }
    out
}

/// Tabla de strings en Markdown (capa 7).
pub fn strings_markdown(table: &StringTable) -> String {
    let mut out = String::from("| Cifrado | Claro | Descifrador | Método | Confianza |\n");
    out.push_str("|---|---|---|---|---|\n");
    for e in &table.entries {
        out.push_str(&format!(
            "| `{}` | {} | `{}->{}` | {} | {} |\n",
            e.blob.replace('|', "\\|"),
            e.plaintext.replace('|', "\\|"),
            e.stub_class,
            e.stub_method,
            e.method,
            e.confidence
        ));
    }
    out
}

/// Hooks Frida para el modo dinámico (capa 7).
///
/// El truco de `android-security-handbook` §40: todo descifrador termina
/// construyendo un `String`, así que hookeando `String.<init>([B)` y
/// `String.<init>([C)` se capturan el 100 % de los descifradores.
pub fn frida_hooks() -> Vec<String> {
    vec![
        r#"// Hook de String.<init>([B) y ([C): captura TODOS los descifrados.
Java.perform(function () {
  var S = Java.use('java.lang.String');
  var log = function (tag, bytes) {
    var out = '';
    try {
      for (var i = 0; i < bytes.length; i++) {
        var c = bytes[i] & 0xff;
        out += (c >= 32 && c < 127) ? String.fromCharCode(c) : '.';
      }
    } catch (e) {}
    console.log('[deobf] ' + tag + ': ' + out);
  };
  S.$init.overload('[B').implementation = function (b) {
    log('String([B)', b);
    return this.$init(b);
  };
  S.$init.overload('[C').implementation = function (c) {
    log('String([C)', c);
    return this.$init(c);
  };
});
"#
        .to_string(),
        r#"// Hook de javax.crypto.Cipher.doFinal: captura el texto ya descifrado.
Java.perform(function () {
  var C = Java.use('javax.crypto.Cipher');
  C.doFinal.overload('[B').implementation = function (b) {
    var out = C.doFinal.overload('[B').call(this, b);
    var txt = '';
    try {
      for (var i = 0; i < out.length; i++) {
        var c = out[i] & 0xff;
        txt += (c >= 32 && c < 127) ? String.fromCharCode(c) : '.';
      }
    } catch (e) {}
    console.log('[deobf] doFinal: ' + txt);
    return out;
  };
});
"#
        .to_string(),
        r#"// Carga dinámica: cualquier DEX que se cargue en runtime.
Java.perform(function () {
  var D = Java.use('dalvik.system.DexClassLoader');
  D.$init.implementation = function (p, o, d, c) {
    console.log('[deobf] DexClassLoader: ' + p);
    return this.$init(p, o, d, c);
  };
});
"#
        .to_string(),
    ]
}

// ========================================================================= //
// TESTS
// ========================================================================= //

#[cfg(test)]
mod unit {
    use super::*;
    use crate::dex_decompile::{DexClassDef, DexEncodedMethod, DexMethod, DexProto, DexType};

    // ---------------------------------------------------------------- //
    // Utilidades para construir código Dalvik sintético.
    // ---------------------------------------------------------------- //

    /// `const/4 vA, #lit` (formato 11n).
    fn const4(a: u16, lit: i8) -> u16 {
        0x12 | ((a as u16 & 0xf) << 8) | (((lit as u16) & 0xf) << 12)
    }

    /// `const/16 vA, #lit` (formato 21s).
    fn const16(a: u16, lit: u16) -> [u16; 2] {
        [0x13 | ((a & 0xff) << 8), lit]
    }

    /// `new-array vA, vB, [T]` (formato 22c).
    fn new_array(a: u16, b: u16, type_idx: u16) -> [u16; 2] {
        [
            0x23 | ((a & 0xf) << 8) | ((b & 0xf) << 12),
            type_idx,
        ]
    }

    /// `aput-char vA, vB, vC` (formato 23x).
    fn aput_char(a: u16, b: u16, c: u16) -> [u16; 2] {
        [0x50 | ((a & 0xff) << 8), (b << 8) | c]
    }

    /// `invoke-static {vA…}, meth` (formato 35c, un argumento).
    fn invoke_static1(arg: u16, method_idx: u16) -> [u16; 3] {
        [0x1071, method_idx, arg & 0xf]
    }

    /// `return-object vA` (formato 11x).
    fn return_object(a: u16) -> u16 {
        0x11 | ((a & 0xff) << 8)
    }

    /// `return-void` (formato 10x).
    fn return_void() -> u16 {
        0x0e
    }

    // ---------------------------------------------------------------- //
    // Capa 4: folding
    // ---------------------------------------------------------------- //

    #[test]
    fn fold_calcula_aritmetica() {
        // v0 = 5; v1 = 3; v0 = v0 + v1; return v0
        let insns = vec![const4(0, 5), const4(1, 3), 0x10b0, 0x000f];
        let s = fold_constants(&insns);
        assert_eq!(s.instructions, 4);
        let add = s
            .items
            .iter()
            .find(|i| i.mnemonic == "add-int/2addr")
            .expect("no se ha encontrado el add");
        assert_eq!(add.value, Some(Value::Int(8)));
        assert!(s.folded >= 3);
    }

    #[test]
    fn fold_detecta_rama_opaca() {
        // v0 = 0; v0 = v0 ^ v0; if-eqz v0 -> rama constante
        let insns = vec![const4(0, 0), 0x00b7, 0x0038, 0x0000];
        let s = fold_constants(&insns);
        assert_eq!(s.opaque_branches, 1);
        let branch = s.items.iter().find(|i| i.opaque).expect("sin rama opaca");
        assert_eq!(branch.value, Some(Value::Bool(true)));
    }

    #[test]
    fn fold_resuelve_const_string_con_dex() {
        let dex = dex_de_prueba();
        // const-string v0, "hola" (21c)
        let insns = vec![0x1a, 0x0000];
        let s = fold_constants_dex(&dex, &insns);
        assert_eq!(s.items[0].value, Some(Value::Str("hola".to_string())));
    }

    #[test]
    fn mba_no_rompe_con_expresion_invalida() {
        let v = mba_verdict("(((");
        assert!(v.verdict == "error" || v.verdict == "indeciso" || v.verdict == "opaca");
    }

    // ---------------------------------------------------------------- //
    // Capa 5e: AES
    // ---------------------------------------------------------------- //

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn aes_fips197_descifra() {
        // Vectores FIPS-197 (C.1, C.2, C.3).
        let casos = [
            (
                "000102030405060708090a0b0c0d0e0f",
                "69c4e0d86a7b0430d8cdb78070b4c55a",
                "00112233445566778899aabbccddeeff",
            ),
            (
                "000102030405060708090a0b0c0d0e0f1011121314151617",
                "dda97ca4864cdfe06eaf70a0ec0d7191",
                "00112233445566778899aabbccddeeff",
            ),
            (
                "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
                "8ea2b7ca516745bfeafc49904b496089",
                "00112233445566778899aabbccddeeff",
            ),
        ];
        for (clave, cifrado, plano) in casos {
            let out = aes_ecb_decrypt(&hex(cifrado), &hex(clave)).expect("falló el descifrado");
            assert_eq!(out, hex(plano), "AES de {} bytes", clave.len() / 2);
        }
    }

    #[test]
    fn aes_ida_y_vuelta() {
        let clave = hex("000102030405060708090a0b0c0d0e0f");
        let datos = hex("00112233445566778899aabbccddeeff");
        let cifrado = aes_ecb_encrypt(&datos, &clave).expect("falló el cifrado");
        assert_eq!(cifrado, hex("69c4e0d86a7b0430d8cdb78070b4c55a"));
        let plano = aes_ecb_decrypt(&cifrado, &clave).expect("falló el descifrado");
        assert_eq!(plano, datos);
    }

    #[test]
    fn pkcs5_valida_relleno() {
        assert_eq!(pkcs5_unpad(&[1, 2, 3, 4, 4, 4]), Some(&[1u8, 2, 3][..]));
        assert!(pkcs5_unpad(&[1, 2, 3, 4, 5]).is_none());
        assert!(pkcs5_unpad(&[]).is_none());
    }

    #[test]
    fn aes_fallback_usa_clave_al_final_del_blob() {
        let clave = hex("000102030405060708090a0b0c0d0e0f");
        let texto = b"hola mundo en claro";
        let pad = 16 - (texto.len() % 16);
        let mut plano = texto.to_vec();
        plano.extend(std::iter::repeat(pad as u8).take(pad));
        let cifrado = aes_ecb_encrypt(&plano, &clave).expect("cifrado");
        let mut blob = cifrado.clone();
        blob.extend_from_slice(&clave);
        let (texto_plano, como) = aes_fallback(&blob).expect("falló el fallback");
        assert_eq!(texto_plano, "hola mundo en claro");
        assert!(como.contains("AES/ECB"));
    }

    #[test]
    fn base64_decodifica() {
        assert_eq!(
            base64_decode("aGVsbG8=").unwrap(),
            b"hello".to_vec()
        );
        assert!(base64_decode("no-es-base64!").is_none());
    }

    // ---------------------------------------------------------------- //
    // Decodificación local
    // ---------------------------------------------------------------- //

    #[test]
    fn decode_all_rellena_registros_de_rango() {
        // invoke-static/range {v0, v1}, metodo#5
        let insns = vec![0x0277, 0x0005, 0x0000];
        let d = decode_all(&insns);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].regs, vec![0u8, 1]);
        assert_eq!(d[0].ref_index, 5);
    }

    #[test]
    fn decode_all_usa_las_convenciones_medidas() {
        // const/4 v0, #5 → literal 5 (byte alto con signo)
        let d = decode_all(&[const4(0, 5)]);
        assert_eq!(d[0].a, 0);
        assert_eq!(d[0].literal, 5);
        // aput-char v3, v0, v2 → A=3, B=0, C=2
        let d = decode_all(&aput_char(3, 0, 2));
        assert_eq!((d[0].a, d[0].b, d[0].c), (3, 0, 2));
    }

    // ---------------------------------------------------------------- //
    // Capa 5d: intérprete
    // ---------------------------------------------------------------- //

    #[test]
    fn interprete_calcula_y_devuelve() {
        // v0 = 7; v1 = 5; v0 = v0 * v1; return v0
        let insns = vec![const4(0, 7), const4(1, 5), 0x10b2, 0x000f];
        let mut interp = Interpreter::new(&[]);
        let v = interp.exec_raw(&insns, 4, &[]).expect("falló la ejecución");
        assert_eq!(v, Value::Int(35));
    }

    #[test]
    fn interprete_ejecuta_bucle() {
        // v0 = 3; v1 = 1; v0 -= v1; if-nez v0 -> -3; return v0
        let insns = vec![const4(0, 3), const4(1, 1), 0x10b1, 0x0039, 0xfffd, 0x000f];
        let mut interp = Interpreter::new(&[]);
        let v = interp.exec_raw(&insns, 4, &[]).expect("falló la ejecución");
        assert_eq!(v, Value::Int(0));
    }

    #[test]
    fn interprete_bifurca_y_conserva_el_camino_bueno() {
        // if-eqz v0 (desconocido) -> bifurca; el camino tomado devuelve 5.
        // 0000 if-eqz v0,+3 | 0001 const/4 v0,9 | 0002 goto +2
        // 0003 const/4 v0,5 | 0004 return v0
        let insns = vec![0x0038, 0x0003, 0x9012, 0x0228, 0x5012, 0x000f];
        let mut interp = Interpreter::new(&[]);
        let v = interp.exec_raw(&insns, 4, &[]).expect("falló la ejecución");
        assert_eq!(v, Value::Int(5));
    }

    #[test]
    fn interprete_pasa_argumentos_en_los_registros_altos() {
        // return v1 con v0 = parámetro (registro alto con 2 registros).
        let insns = vec![0x010f];
        let mut interp = Interpreter::new(&[]);
        let v = interp
            .exec_raw(&insns, 2, &[Value::Int(0), Value::Int(77)])
            .expect("falló la ejecución");
        assert_eq!(v, Value::Int(77));
    }

    // ---------------------------------------------------------------- //
    // Capa 5a/5b: stubs y sitios de llamada
    // ---------------------------------------------------------------- //

    /// DEX sintético: una clase con un descifrador y otra que lo llama.
    fn dex_de_prueba() -> DexFile {
        let strings = vec![
            "hola".to_string(),
            "dec".to_string(),
            "<init>".to_string(),
            "call".to_string(),
        ];
        let types = vec![
            DexType {
                descriptor_idx: 0,
                descriptor: "Lcom/x/A;".to_string(),
            },
            DexType {
                descriptor_idx: 1,
                descriptor: "[C".to_string(),
            },
            DexType {
                descriptor_idx: 2,
                descriptor: "Ljava/lang/String;".to_string(),
            },
            DexType {
                descriptor_idx: 3,
                descriptor: "Lcom/x/B;".to_string(),
            },
        ];
        let protos = vec![
            DexProto {
                shorty_idx: 0,
                shorty: "Ljava/lang/String;".to_string(),
                return_type_idx: 2,
                return_type: "Ljava/lang/String;".to_string(),
                parameters: vec!["Ljava/lang/String;".to_string()],
            },
            DexProto {
                shorty_idx: 1,
                shorty: "V".to_string(),
                return_type_idx: 0,
                return_type: "V".to_string(),
                parameters: Vec::new(),
            },
        ];
        let methods = vec![
            DexMethod {
                class_idx: 0,
                proto_idx: 0,
                name_idx: 1,
                class_name: "Lcom/x/A;".to_string(),
                proto: "(Ljava/lang/String;)Ljava/lang/String;".to_string(),
                name: "dec".to_string(),
            },
            DexMethod {
                class_idx: 2,
                proto_idx: 0,
                name_idx: 2,
                class_name: "Ljava/lang/String;".to_string(),
                proto: "(Ljava/lang/String;)Ljava/lang/String;".to_string(),
                name: "<init>".to_string(),
            },
            DexMethod {
                class_idx: 3,
                proto_idx: 1,
                name_idx: 3,
                class_name: "Lcom/x/B;".to_string(),
                proto: "()V".to_string(),
                name: "call".to_string(),
            },
        ];

        // Descifrador: tabla char[] + construcción de String.
        let mut stub_insns: Vec<u16> = Vec::new();
        stub_insns.push(const4(1, 3));
        stub_insns.extend(new_array(0, 1, 1));
        stub_insns.push(const4(2, 0));
        stub_insns.extend(const16(3, 65));
        stub_insns.extend(aput_char(3, 0, 2));
        // new-instance v4, Ljava/lang/String;  (22c)
        stub_insns.extend([0x0422, 0x0002]);
        // invoke-direct {v4, v0}, Ljava/lang/String;-><init>([C)  (35c, 2 args)
        stub_insns.extend([0x2070, 0x0001, 0x0004]);
        stub_insns.push(return_object(4));

        // Llamante: const-string + invoke-static.
        let mut call_insns: Vec<u16> = Vec::new();
        call_insns.extend([0x1a, 0x0000]);
        call_insns.extend(invoke_static1(0, 0));
        call_insns.push(return_void());

        let classes = vec![
            DexClassDef {
                class_idx: 0,
                class_name: "Lcom/x/A;".to_string(),
                direct_methods: vec![DexEncodedMethod {
                    method_idx: 0,
                    access_flags: 0x9,
                    code_off: 1,
                    code: Some(DexCode {
                        registers_size: 4,
                        ins_size: 1,
                        outs_size: 0,
                        tries_size: 0,
                        debug_info_off: 0,
                        insns: stub_insns,
                        tries: Vec::new(),
                        debug: None,
                    }),
                }],
                ..Default::default()
            },
            DexClassDef {
                class_idx: 3,
                class_name: "Lcom/x/B;".to_string(),
                direct_methods: vec![DexEncodedMethod {
                    method_idx: 2,
                    access_flags: 0x9,
                    code_off: 1,
                    code: Some(DexCode {
                        registers_size: 2,
                        ins_size: 0,
                        outs_size: 0,
                        tries_size: 0,
                        debug_info_off: 0,
                        insns: call_insns,
                        tries: Vec::new(),
                        debug: None,
                    }),
                }],
                ..Default::default()
            },
        ];

        DexFile {
            bytes: Vec::new(),
            version: "035".to_string(),
            checksum: 0,
            checksum_valid: false,
            signature: String::new(),
            signature_valid: false,
            sha256: String::new(),
            file_size: 0,
            endian_tag: 0x12345678,
            header_size: 0x70,
            link_size: 0,
            link_off: 0,
            map_off: 0,
            strings,
            types,
            protos,
            fields: Vec::new(),
            methods,
            classes,
            map_items: Vec::new(),
        }
    }

    #[test]
    fn detecta_stub_descifrador() {
        let dex = dex_de_prueba();
        let stubs = find_decrypt_stubs(&dex);
        assert_eq!(stubs.len(), 1, "stubs: {stubs:?}");
        let s = &stubs[0];
        assert_eq!(s.class, "Lcom/x/A;");
        assert_eq!(s.method, "dec");
        assert_eq!(s.kind, "string_decryptor");
        assert!(s.confidence >= 50);
        assert_eq!(s.table_type.as_deref(), Some("[C"));
    }

    #[test]
    fn enumera_sitios_de_llamada_con_argumento() {
        let dex = dex_de_prueba();
        let sites = find_call_sites(&[("classes.dex", &dex)], "Lcom/x/A;", "dec");
        assert_eq!(sites.len(), 1, "sitios: {sites:?}");
        assert_eq!(sites[0].caller_class, "Lcom/x/B;");
        assert_eq!(sites[0].caller_method, "call");
        assert_eq!(sites[0].argument.as_deref(), Some("hola"));
    }

    // ---------------------------------------------------------------- //
    // Capa 6: huellas
    // ---------------------------------------------------------------- //

    #[test]
    fn detecta_upx_en_nativo() {
        // Binario que no es ELF: se evita goblin y se busca la marca "UPX!".
        let datos = b"\x00\x01\x02\x03 garbage UPX! mas basura";
        let f = fingerprint_native(datos, "libjni.so");
        assert!(f.iter().any(|x| x.detector == "upx"), "hallazgos: {f:?}");
    }

    #[test]
    fn detecta_cargador_dinamico() {
        let dex = dex_de_prueba();
        let f = fingerprint_apk(&[("classes.dex", &dex)], &[], &[]);
        // El DEX sintético no tiene cargadores: no debe haber falsos positivos.
        assert!(f.iter().all(|x| x.detector != "cargador_dinamico"));
    }

    // ---------------------------------------------------------------- //
    // Capa 7: salida
    // ---------------------------------------------------------------- //

    #[test]
    fn tabla_de_strings_en_csv_y_markdown() {
        let tabla = StringTable {
            entries: vec![StringEntry {
                blob: "aGk=".to_string(),
                plaintext: "hola".to_string(),
                stub_class: "Lcom/x/A;".to_string(),
                stub_method: "dec".to_string(),
                dex: "classes.dex".to_string(),
                caller_class: "Lcom/x/B;".to_string(),
                caller_method: "call".to_string(),
                method: "intérprete".to_string(),
                confidence: 90,
            }],
            stubs: Vec::new(),
            unresolved: Vec::new(),
        };
        let csv = strings_csv(&tabla);
        assert!(csv.contains("hola"));
        assert!(csv.starts_with("blob,claro"));
        let md = strings_markdown(&tabla);
        assert!(md.contains("hola"));
        assert!(md.contains("Lcom/x/A;->dec"));
    }

    #[test]
    fn genera_hooks_frida() {
        let hooks = frida_hooks();
        assert_eq!(hooks.len(), 3);
        assert!(hooks.iter().any(|h| h.contains("java.lang.String")));
        assert!(hooks.iter().any(|h| h.contains("doFinal")));
    }

    #[test]
    fn informe_json_tiene_las_tres_capas() {
        let dex = dex_de_prueba();
        let opts = DeobfOptions::default();
        let informe = analyze(&[("classes.dex", &dex)], &[], &[], &opts);
        let j = report_json(&informe);
        assert!(j.get("capa_5_strings").is_some());
        assert!(j.get("capa_6_huellas").is_some());
        assert!(j.get("auditoria").is_some());
        // El stub se detecta.
        assert_eq!(informe.stubs.len(), 1);
        // Y el sitio de llamada se encuentra.
        assert_eq!(informe.call_sites.len(), 1);
    }
}

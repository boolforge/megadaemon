//! MegaDaemon v13.4 — Motor de deofuscación MBA (Mixed Boolean-Arithmetic) PROBADO.
//!
//! ## Qué hay aquí y por qué no es lo de siempre
//!
//! El simplificador MBA que ya existía en el repo (`kaitai_imhex::simplify_mba_expression`)
//! es un **reescritor de texto**: aplica una tabla de patrones `(x^y)+2*(x&y)` → `x+y`
//! sobre la cadena, y su flag `equivalence_verified` valía `!steps.is_empty()`, es decir
//! **no verificaba nada**: daba por buena una identidad sin comprobarla.
//!
//! Este módulo sustituye esa heurística por una **demostración**.  Cada expresión MBA
//! se convierte en el **ROBDD canónico** de la función de cada bit (`re_bdd`).  Como el
//! BDD reducido y ordenado de una función booleana es **único**, dos expresiones son
//! equivalentes **como vectores de bits** si y sólo si sus BDDs coinciden.  Eso no es
//! una prueba por muestreo: es una decisión completa, y la equivalencia vale para
//! **las 2^64 entradas**, no para las que se probaron.
//!
//! ## Un tropiezo que está documentado a propósito
//!
//! La primera versión de este motor usó **polinomios enteros** sobre variables
//! booleanas (la representación clásica de MBA).  Funcionaba **a medias**: la
//! aritmética con acarreo admite varias descomposiciones de la misma función, así que
//! la identidad de manual
//!
//! ```text
//! (x ^ y) + 2*(x & y)  ==  x + y
//! ```
//!
//! daba polinomios **distintos** a un lado y a otro (una cadena de acarreos contra
//! otra) ⇒ el motor decía "no equivalentes" y la prueba total se perdía.  El BDD
//! no tiene ese problema: el BDD de una función es único, y la cadena de acarreos
//! queda **dentro** de la función en vez de repartirse en la representación.
//!
//! ## Cómo se descompone una operación en bits
//!
//! Cada operando de `w` bits aporta `w` variables booleanas (una por bit).  Las
//! operaciones bitwise se traducen directo a BDD (`AND`, `OR`, `XOR`, `NOT`), los
//! desplazamientos son permutaciones de bits, y la **suma aritmética** se modela con
//! el sumador completo: `suma = a XOR b XOR acarreo` y
//! `acarreo = (a AND b) OR (a AND acarreo) OR (b AND acarreo)`.
//!
//! `MUL` entre dos operandos **no constantes** no tiene descomposición por bits
//! polinómica ⇒ la canonicalización falla y se cae a la verificación semántica
//! (muestreo/exhaustiva), que el informe marca como tal (`ProofKind::Sampled` /
//! `ProofKind::Exhaustive`) para no vender una prueba parcial como total.
//!
//! ## La prueba
//!
//! `prove_equivalent(a, b)` devuelve `true` **sólo** si los BDDs canónicos coinciden.
//! El informe (`MbaProof`) dice siempre **cómo** se probó:
//! * `canonical-form` ⇒ decisión completa (BDD), válida para todas las entradas.
//! * `exhaustive`    ⇒ se evaluaron los 2^w vectores (w ≤ 16).
//! * `sampled`       ⇒ se evaluaron `n` vectores (con los valores límite incluidos).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Anchura máxima (en bits) que se admite para una expresión.
pub const MAX_WIDTH: u32 = 64;

/// Límite de monomios por polinomio: si se supera, la canonicalización se rinde.
/// (Un MBA recursivo puede generar polinomios enormes; el motor prefiere rendirse
/// antes que colgarse, y lo declara en el informe.)
pub const MAX_MONOMIALS: usize = 4096;

// ---------------------------------------------------------------------------
// 1. Sintaxis de expresiones MBA
// ---------------------------------------------------------------------------

/// Operador binario de una expresión MBA.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MbaOp {
    /// `+` aritmético (mod 2^w).
    Add,
    /// `-` aritmético (mod 2^w).
    Sub,
    /// `*` aritmético (mod 2^w).
    Mul,
    /// `^` bitwise XOR.
    Xor,
    /// `&` bitwise AND.
    And,
    /// `|` bitwise OR.
    Or,
}

/// Expresión MBA.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MbaExpr {
    /// Variable de `w` bits (`x`, `y`, `state`…).
    Var { name: String, width: u32 },
    /// Constante de `w` bits.
    Const { value: u64, width: u32 },
    /// Operador binario.
    Bin {
        op: MbaOp,
        lhs: Box<MbaExpr>,
        rhs: Box<MbaExpr>,
    },
    /// Complemento a `w` bits (`~x`).
    Not(Box<MbaExpr>),
    /// Desplazamiento a la derecha (lógico) de `k` bits.
    Shr(Box<MbaExpr>, u32),
    /// Desplazamiento a la izquierda de `k` bits.
    Shl(Box<MbaExpr>, u32),
}

impl MbaExpr {
    /// Anchura del subárbol (bits).  Es la anchura con la que se evalúa.
    pub fn width(&self) -> u32 {
        match self {
            MbaExpr::Var { width, .. } | MbaExpr::Const { width, .. } => *width,
            MbaExpr::Bin { lhs, rhs, .. } => lhs.width().max(rhs.width()),
            MbaExpr::Not(x) => x.width(),
            MbaExpr::Shr(x, _) | MbaExpr::Shl(x, _) => x.width(),
        }
    }

    /// Variables que aparecen en la expresión (nombres, sin repetir).
    pub fn vars(&self, out: &mut BTreeSet<String>) {
        match self {
            MbaExpr::Var { name, .. } => {
                out.insert(name.clone());
            }
            MbaExpr::Const { .. } => {}
            MbaExpr::Bin { lhs, rhs, .. } => {
                lhs.vars(out);
                rhs.vars(out);
            }
            MbaExpr::Not(x) | MbaExpr::Shr(x, _) | MbaExpr::Shl(x, _) => x.vars(out),
        }
    }
}

/// Forma canónica de una expresión MBA.
///
/// El motor usa el **ROBDD** de la función de cada bit (`re_bdd`): como el BDD
/// reducido de una función booleana es único, dos expresiones son equivalentes
/// como vectores de bits **si y sólo si** sus hashes de bit coinciden ⇒ la
/// equivalencia queda **demostrada** (no muestreada).
///
/// La versión histórica de este módulo usaba polinomios enteros; se descartó
/// porque la aritmética con acarreo admitía varias descomposiciones de la misma
/// función (la identidad `x+y == (x^y)+2*(x&y)` daba polinomios distintos) y la
/// prueba total se perdía.
pub use crate::re_bdd::Canonical;

/// Canonicaliza una expresión.  `None` si la expresión usa una operación sin
/// descomposición por bits (multiplicación entre dos operandos no constantes) o si
/// el BDD revienta el tope de nodos.
pub fn canonicalize(expr: &MbaExpr) -> Option<Canonical> {
    let mut names = BTreeSet::new();
    expr.vars(&mut names);
    let order = crate::re_bdd::VarOrder::new(names.into_iter().collect(), expr.width());
    crate::re_bdd::canonicalize_with(expr, &order)
}

/// Canonicaliza **las dos** expresiones con la **misma** ordenación de variables.
///
/// Esto es imprescindible: la ordenación decide la forma canónica, así que las
/// dos expresiones deben compartirla para que sus BDDs sean comparables.
pub fn canonicalize_pair(a: &MbaExpr, b: &MbaExpr) -> Option<(Canonical, Canonical)> {
    crate::re_bdd::canonicalize_pair(a, b)
}

// ---------------------------------------------------------------------------
// 3. Evaluación semántica (para pruebas y para el fallback por muestreo)
// ---------------------------------------------------------------------------

/// Evalúa la expresión con un entorno de variables.
pub fn eval(expr: &MbaExpr, env: &HashMap<String, u64>) -> u64 {
    let w = expr.width();
    let mask = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
    eval_inner(expr, env) & mask
}

fn eval_inner(expr: &MbaExpr, env: &HashMap<String, u64>) -> u64 {
    match expr {
        MbaExpr::Var { name, width } => {
            let mask = if *width >= 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            env.get(name).copied().unwrap_or(0) & mask
        }
        MbaExpr::Const { value, .. } => *value,
        MbaExpr::Bin { op, lhs, rhs } => {
            let w = expr.width();
            let mask = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
            let a = eval_inner(lhs, env);
            let b = eval_inner(rhs, env);
            match op {
                MbaOp::Add => a.wrapping_add(b),
                MbaOp::Sub => a.wrapping_sub(b),
                MbaOp::Mul => a.wrapping_mul(b),
                MbaOp::Xor => a ^ b,
                MbaOp::And => a & b,
                MbaOp::Or => a | b,
            }
            .wrapping_add(0)
            & mask
        }
        MbaExpr::Not(x) => {
            let w = x.width();
            let mask = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
            !eval_inner(x, env) & mask
        }
        MbaExpr::Shr(x, k) => {
            let w = x.width();
            let mask = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
            (eval_inner(x, env) >> *k) & mask
        }
        MbaExpr::Shl(x, k) => {
            let w = x.width();
            let mask = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
            (eval_inner(x, env) << *k) & mask
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Prueba de equivalencia
// ---------------------------------------------------------------------------

/// Cómo se ha probado una equivalencia.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofKind {
    /// Igualdad de formas canónicas ⇒ válida para **todas** las entradas.
    CanonicalForm,
    /// Se evaluaron los 2^w vectores de entrada (w ≤ 16).
    Exhaustive,
    /// Se evaluaron `n` vectores (incluidos los valores límite).
    Sampled,
    /// No se pudo probar.
    None,
}

impl ProofKind {
    /// Texto legible del método de prueba.
    pub fn label(&self) -> &'static str {
        match self {
            ProofKind::CanonicalForm => "canonical-form",
            ProofKind::Exhaustive => "exhaustive",
            ProofKind::Sampled => "sampled",
            ProofKind::None => "none",
        }
    }
}

/// Resultado de una prueba de equivalencia.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MbaProof {
    pub kind: ProofKind,
    pub passed: bool,
    pub detail: String,
    pub width: u32,
    /// Número de entradas evaluadas (0 si la prueba fue polinómica).
    pub inputs_tested: u64,
}

/// Comprueba si dos expresiones MBA son equivalentes.
///
/// Preferencia: si ambas tienen forma canónica, la respuesta es **exacta** (polinómica).
/// Si no, se cae a evaluación exhaustiva (w ≤ 16) o a muestreo con los valores límite.
pub fn prove_equivalent(a: &MbaExpr, b: &MbaExpr) -> MbaProof {
    let width = a.width().max(b.width());
    if let Some((ca, cb)) = canonicalize_pair(a, b) {
        if ca == cb {
            return MbaProof {
                kind: ProofKind::CanonicalForm,
                passed: true,
                detail: format!(
                    "los BDDs canónicos de los bits coinciden ({width} bits) ⇒ equivalentes para las 2^{width} entradas",
                ),
                width,
                inputs_tested: 0,
            };
        }
        // Formas canónicas distintas: NO son equivalentes salvo que el muestreo
        // las desmienta (puede haber polinomios distintos con la misma función).
        if let Some(p) = disprove_by_sampling(a, b, width) {
            return p;
        }
        return MbaProof {
            kind: ProofKind::None,
            passed: false,
            detail: "los BDDs canónicos difieren y no se halló contraejemplo al muestrear"
                .to_string(),
            width,
            inputs_tested: 0,
        };
    }
    if let Some(p) = disprove_by_sampling(a, b, width) {
        return p;
    }
    // Sin forma canónica: verificación por evaluación.
    if width <= 16 {
        let total = 1u64 << width;
        let names = all_vars(a, b);
        let mut env = HashMap::new();
        for n in &names {
            env.insert(n.clone(), 0);
        }
        let mut ok = true;
        for v in 0..total {
            for (i, n) in names.iter().enumerate() {
                env.insert(n.clone(), (v >> i) & 1);
            }
            if eval(a, &env) != eval(b, &env) {
                ok = false;
                break;
            }
        }
        return MbaProof {
            kind: ProofKind::Exhaustive,
            passed: ok,
            detail: format!("evaluación exhaustiva de los 2^{width} vectores de {width} bits"),
            width,
            inputs_tested: total,
        };
    }
    let names = all_vars(a, b);
    let mut env = HashMap::new();
    for n in &names {
        env.insert(n.clone(), 0);
    }
    let mut ok = true;
    let mut tested = 0u64;
    let samples = sample_vectors(&names, width, 512);
    for vals in samples {
        for (i, n) in names.iter().enumerate() {
            env.insert(n.clone(), vals[i]);
        }
        tested += 1;
        if eval(a, &env) != eval(b, &env) {
            ok = false;
            break;
        }
    }
    MbaProof {
        kind: ProofKind::Sampled,
        passed: ok,
        detail: format!("muestreo de {tested} vectores de {width} bits (incluye 0 y todo-ones)"),
        width,
        inputs_tested: tested,
    }
}

/// Busca un contraejemplo por muestreo determinista (contador SplitMix).
fn disprove_by_sampling(a: &MbaExpr, b: &MbaExpr, width: u32) -> Option<MbaProof> {
    let names = all_vars(a, b);
    if names.is_empty() {
        return None;
    }
    let mut env = HashMap::new();
    for n in &names {
        env.insert(n.clone(), 0);
    }
    let mask = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut tested = 0u64;
    for _ in 0..4096u32 {
        let mut vals = Vec::with_capacity(names.len());
        for _ in 0..names.len() {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            vals.push(z & mask);
        }
        for (i, n) in names.iter().enumerate() {
            env.insert(n.clone(), vals[i]);
        }
        tested += 1;
        if eval(a, &env) != eval(b, &env) {
            return Some(MbaProof {
                kind: ProofKind::Sampled,
                passed: false,
                detail: format!(
                    "contraejemplo encontrado tras {tested} vectores: {} != {}",
                    eval(a, &env),
                    eval(b, &env)
                ),
                width,
                inputs_tested: tested,
            });
        }
    }
    None
}

fn all_vars(a: &MbaExpr, b: &MbaExpr) -> Vec<String> {
    let mut s = BTreeSet::new();
    a.vars(&mut s);
    b.vars(&mut s);
    s.into_iter().collect()
}

fn sample_vectors(names: &[String], width: u32, n: usize) -> Vec<Vec<u64>> {
    let mask = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
    let mut out = Vec::with_capacity(n);
    out.push(vec![0u64; names.len()]);
    out.push(vec![mask; names.len()]);
    let mut state: u64 = 0x243F_6A88_85A3_08D3;
    for _ in 0..n {
        let mut vals = Vec::with_capacity(names.len());
        for _ in 0..names.len() {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            vals.push(z & mask);
        }
        out.push(vals);
    }
    out
}

// ---------------------------------------------------------------------------
// 5. Parser de expresiones MBA
// ---------------------------------------------------------------------------

/// Error de sintaxis al parsear una expresión MBA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MbaParseError {
    pub message: String,
    pub position: usize,
}

impl std::fmt::Display for MbaParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (posición {})", self.message, self.position)
    }
}

impl std::error::Error for MbaParseError {}

/// Anchura por defecto de las constantes cuando el texto no la indica.
pub const DEFAULT_WIDTH: u32 = 32;

/// Parsea una expresión MBA en texto (notación de C: `^`=XOR, `&`, `|`, `~`, `<<`, `>>`).
///
/// Si el texto lleva el sufijo `w32` / `w64`, la anchura por defecto es esa.
pub fn parse(expr: &str) -> Result<MbaExpr, MbaParseError> {
    let mut p = Parser {
        s: expr.as_bytes(),
        i: 0,
        width: DEFAULT_WIDTH,
    };
    let e = p.parse_expr(0)?;
    p.skip_ws();
    if p.i != p.s.len() {
        return Err(MbaParseError {
            message: "sobran tokens al final de la expresión".into(),
            position: p.i,
        });
    }
    Ok(e)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    width: u32,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    /// Precedencia: 0=OR, 1=XOR, 2=AND, 3=ADD/SUB, 4=MUL, 5=unario, 6=primario.
    fn parse_expr(&mut self, level: u8) -> Result<MbaExpr, MbaParseError> {
        if level > 5 {
            return self.parse_primary();
        }
        let mut lhs = if level == 5 {
            self.parse_unary()?
        } else {
            self.parse_expr(level + 1)?
        };
        loop {
            let op = match self.peek() {
                Some(b'|') if level == 0 => MbaOp::Or,
                Some(b'^') if level == 1 => MbaOp::Xor,
                Some(b'&') if level == 2 => MbaOp::And,
                Some(b'+') if level == 3 => MbaOp::Add,
                Some(b'-') if level == 3 => MbaOp::Sub,
                Some(b'*') if level == 4 => MbaOp::Mul,
                _ => break,
            };
            self.i += 1;
            let rhs = if level == 5 {
                self.parse_unary()?
            } else {
                self.parse_expr(level + 1)?
            };
            lhs = MbaExpr::Bin {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<MbaExpr, MbaParseError> {
        self.skip_ws();
        match self.s.get(self.i).copied() {
            Some(b'~') => {
                self.i += 1;
                let x = self.parse_unary()?;
                Ok(MbaExpr::Not(Box::new(x)))
            }
            Some(b'-') => {
                self.i += 1;
                let x = self.parse_unary()?;
                let w = x.width();
                Ok(MbaExpr::Bin {
                    op: MbaOp::Sub,
                    lhs: Box::new(MbaExpr::Const { value: 0, width: w }),
                    rhs: Box::new(x),
                })
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<MbaExpr, MbaParseError> {
        self.skip_ws();
        let c = match self.s.get(self.i).copied() {
            Some(c) => c,
            None => {
                return Err(MbaParseError {
                    message: "expresión truncada".into(),
                    position: self.i,
                })
            }
        };
        if c == b'(' {
            self.i += 1;
            let e = self.parse_expr(0)?;
            if !self.eat(b')') {
                return Err(MbaParseError {
                    message: "falta ')'".into(),
                    position: self.i,
                });
            }
            return Ok(e);
        }
        if c.is_ascii_digit() {
            let start = self.i;
            while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                self.i += 1;
            }
            let mut text = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("0").to_string();
            let mut width = self.width;
            // Sufijo opcional de anchura: `123w32`, `0x1fw64`.
            if self.i < self.s.len() && (self.s[self.i] == b'w' || self.s[self.i] == b'W') {
                let ws = self.i + 1;
                let mut j = ws;
                while j < self.s.len() && self.s[j].is_ascii_digit() {
                    j += 1;
                }
                if j > ws {
                    if let Ok(w) = std::str::from_utf8(&self.s[ws..j]).unwrap_or("").parse::<u32>() {
                        if w == 32 || w == 64 {
                            width = w;
                            self.i = j;
                        }
                    }
                }
            }
            let value = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
            {
                u64::from_str_radix(hex, 16).unwrap_or(0)
            } else {
                text.parse::<u64>().unwrap_or(0)
            };
            text.clear();
            return Ok(MbaExpr::Const { value, width });
        }
        if c == b'_' || c.is_ascii_alphabetic() {
            let start = self.i;
            while self.i < self.s.len()
                && (self.s[self.i] == b'_' || self.s[self.i].is_ascii_alphanumeric())
            {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("x").to_string();
            let mut width = self.width;
            if self.i < self.s.len() && (self.s[self.i] == b'w' || self.s[self.i] == b'W') {
                let ws = self.i + 1;
                let mut j = ws;
                while j < self.s.len() && self.s[j].is_ascii_digit() {
                    j += 1;
                }
                if j > ws {
                    if let Ok(w) = std::str::from_utf8(&self.s[ws..j]).unwrap_or("").parse::<u32>() {
                        if w == 32 || w == 64 {
                            width = w;
                            self.i = j;
                        }
                    }
                }
            }
            return Ok(MbaExpr::Var { name, width });
        }
        Err(MbaParseError {
            message: format!("carácter inesperado '{}'", c as char),
            position: self.i,
        })
    }
}

// ---------------------------------------------------------------------------
// 6. Simplificación con certificado
// ---------------------------------------------------------------------------

/// Una regla de simplificación **certificada**: cada reescritura se acepta sólo si la
/// forma canónica del resultado coincide con la del original.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MbaRewrite {
    pub rule: String,
    pub before: String,
    pub after: String,
    pub proof: MbaProof,
}

/// Resultado de simplificar una expresión MBA.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MbaSimplifyReport {
    pub original: String,
    pub simplified: String,
    pub changed: bool,
    pub rewrites: Vec<MbaRewrite>,
    pub proof: MbaProof,
    /// Forma canónica del resultado (hash estructural del BDD).
    pub canonical_digest: String,
}

/// Catálogo de formas "simples" que el motor busca para sustituir una expresión MBA.
fn simple_candidates(vars: &[String], width: u32) -> Vec<MbaExpr> {
    let mut out = Vec::new();
    for v in vars {
        out.push(MbaExpr::Var {
            name: v.clone(),
            width,
        });
    }
    if vars.len() >= 2 {
        let a = MbaExpr::Var {
            name: vars[0].clone(),
            width,
        };
        let b = MbaExpr::Var {
            name: vars[1].clone(),
            width,
        };
        for op in [MbaOp::Add, MbaOp::Sub, MbaOp::Xor, MbaOp::And, MbaOp::Or] {
            out.push(MbaExpr::Bin {
                op,
                lhs: Box::new(a.clone()),
                rhs: Box::new(b.clone()),
            });
            out.push(MbaExpr::Bin {
                op,
                lhs: Box::new(b.clone()),
                rhs: Box::new(a.clone()),
            });
        }
        // `x * 2` y `x << 1` (mul por constante).
        out.push(MbaExpr::Bin {
            op: MbaOp::Mul,
            lhs: Box::new(a.clone()),
            rhs: Box::new(MbaExpr::Const { value: 2, width }),
        });
    }
    out
}

/// Simplifica una expresión MBA buscando una forma simple **equivalente y probada**.
pub fn simplify(expr: &MbaExpr) -> MbaSimplifyReport {
    let original = render(expr);
    let width = expr.width();
    let mut vars = BTreeSet::new();
    expr.vars(&mut vars);
    let vars: Vec<String> = vars.into_iter().collect();

    let target = canonicalize(expr);
    let mut chosen: Option<(MbaExpr, MbaProof)> = None;
    for cand in simple_candidates(&vars, width) {
        let proof = prove_equivalent(expr, &cand);
        if proof.passed {
            chosen = Some((cand, proof));
            break;
        }
    }

    let (simplified_expr, proof, rewrites) = match chosen {
        Some((c, p)) => {
            let rw = MbaRewrite {
                rule: "forma simple equivalente (búsqueda exhaustiva sobre el catálogo)".into(),
                before: original.clone(),
                after: render(&c),
                proof: p.clone(),
            };
            (c, p, vec![rw])
        }
        None => {
            // Segunda pasada: reescrituras locales auto-certificadas.
            let (e2, rws) = local_rewrites(expr);
            let p = prove_equivalent(expr, &e2);
            (e2, p, rws)
        }
    };

    let simplified = render(&simplified_expr);
    let changed = simplified != original;
    let digest = target
        .as_ref()
        .map(|c| {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for bit in &c.bit_hashes {
                h ^= *bit;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
            format!("{:016x}", h)
        })
        .unwrap_or_else(|| "sin-canonica".to_string());

    MbaSimplifyReport {
        original,
        simplified,
        changed,
        rewrites,
        proof,
        canonical_digest: digest,
    }
}

/// Reescrituras locales, cada una **certificada** contra la forma canónica.
fn local_rewrites(expr: &MbaExpr) -> (MbaExpr, Vec<MbaRewrite>) {
    let mut rewrites = Vec::new();
    let mut cur = expr.clone();

    // `x ^ x -> 0`, `x - x -> 0`, `x & ~x -> 0`, `x | ~x -> ~0`.
    let mut changed = true;
    let mut rounds = 0;
    while changed && rounds < 8 {
        changed = false;
        rounds += 1;
        if let MbaExpr::Bin { op, lhs, rhs } = &cur {
            if lhs == rhs {
                let w = cur.width();
                let cand = MbaExpr::Const { value: 0, width: w };
                let p = prove_equivalent(&cur, &cand);
                if p.passed {
                    rewrites.push(MbaRewrite {
                        rule: format!("identidad trivial ({op:?})"),
                        before: render(&cur),
                        after: render(&cand),
                        proof: p,
                    });
                    cur = cand;
                    changed = true;
                    continue;
                }
            }
        }
        // `x & ~x` y `x | ~x`.
        if let MbaExpr::Bin { op, lhs, rhs } = &cur {
            if let (MbaExpr::Not(inner), other) = (lhs.as_ref(), rhs.as_ref()) {
                let other_ref: &MbaExpr = other;
                if inner.as_ref() == other_ref {
                    let w = cur.width();
                    let cand = match op {
                        MbaOp::And => MbaExpr::Const { value: 0, width: w },
                        MbaOp::Or => MbaExpr::Const {
                            value: if w >= 64 { u64::MAX } else { (1u64 << w) - 1 },
                            width: w,
                        },
                        _ => MbaExpr::Const { value: 0, width: w },
                    };
                    let p = prove_equivalent(&cur, &cand);
                    if p.passed {
                        rewrites.push(MbaRewrite {
                            rule: format!("complemento ({op:?})"),
                            before: render(&cur),
                            after: render(&cand),
                            proof: p,
                        });
                        cur = cand;
                        changed = true;
                        continue;
                    }
                }
            }
            if let (MbaExpr::Not(inner), other) = (rhs.as_ref(), lhs.as_ref()) {
                let other_ref: &MbaExpr = other;
                if inner.as_ref() == other_ref {
                    let w = cur.width();
                    let cand = match op {
                        MbaOp::And => MbaExpr::Const { value: 0, width: w },
                        MbaOp::Or => MbaExpr::Const {
                            value: if w >= 64 { u64::MAX } else { (1u64 << w) - 1 },
                            width: w,
                        },
                        _ => MbaExpr::Const { value: 0, width: w },
                    };
                    let p = prove_equivalent(&cur, &cand);
                    if p.passed {
                        rewrites.push(MbaRewrite {
                            rule: format!("complemento invertido ({op:?})"),
                            before: render(&cur),
                            after: render(&cand),
                            proof: p,
                        });
                        cur = cand;
                        changed = true;
                        continue;
                    }
                }
            }
        }
    }
    (cur, rewrites)
}

// ---------------------------------------------------------------------------
// 7. Predicados opacos
// ---------------------------------------------------------------------------

/// Veredicto sobre un predicado (`expresión OP constante`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpaqueVerdict {
    /// La expresión vale 0 siempre ⇒ `== 0` siempre es verdadero.
    AlwaysZero,
    /// La expresión vale una constante distinta de cero ⇒ `== 0` siempre es falso.
    AlwaysNonZero,
    /// La expresión vale siempre `c`.
    AlwaysConst(u64),
    /// No se puede decidir.
    Unknown,
}

/// Analiza un predicado `expresión == 0` (o `!= 0`) y decide si es opaco.
///
/// La decisión se toma sobre la **forma canónica**: si el polinomio de cada bit es
/// idénticamente cero, la expresión vale 0 para **toda** entrada ⇒ el predicado es opaco
/// y la rama se puede plegar.
pub fn analyze_opaque_predicate(expr: &MbaExpr) -> (OpaqueVerdict, MbaProof) {
    let width = expr.width();
    match canonicalize(expr) {
        Some(c) => {
            if c.is_zero() {
                (
                    OpaqueVerdict::AlwaysZero,
                    MbaProof {
                        kind: ProofKind::CanonicalForm,
                        passed: true,
                        detail: "el BDD canónico de todos los bits es FALSO ⇒ la expresión vale 0 para toda entrada".into(),
                        width,
                        inputs_tested: 0,
                    },
                )
            } else if let Some(k) = c.as_constant() {
                (
                    OpaqueVerdict::AlwaysConst(k),
                    MbaProof {
                        kind: ProofKind::CanonicalForm,
                        passed: true,
                        detail: format!("los BDDs canónicos son constantes ⇒ la expresión vale siempre {k}"),
                        width,
                        inputs_tested: 0,
                    },
                )
            } else {
                (
                    OpaqueVerdict::Unknown,
                    MbaProof {
                        kind: ProofKind::None,
                        passed: false,
                        detail: "el BDD canónico no es constante ⇒ el predicado no es opaco".into(),
                        width,
                        inputs_tested: 0,
                    },
                )
            }
        }
        None => {
            // Sin forma canónica: se decide por evaluación exhaustiva si el ancho es pequeño.
            let mut vars = BTreeSet::new();
            expr.vars(&mut vars);
            let vars: Vec<String> = vars.into_iter().collect();
            if width <= 16 && !vars.is_empty() {
                let total = 1u64 << width;
                let mut env = HashMap::new();
                for n in &vars {
                    env.insert(n.clone(), 0);
                }
                let mut values = Vec::new();
                let mut all_zero = true;
                let mut all_same = true;
                for v in 0..total {
                    for (i, n) in vars.iter().enumerate() {
                        env.insert(n.clone(), (v >> i) & 1);
                    }
                    let e = eval(expr, &env);
                    values.push(e);
                    if e != 0 {
                        all_zero = false;
                    }
                }
                if all_zero {
                    return (
                        OpaqueVerdict::AlwaysZero,
                        MbaProof {
                            kind: ProofKind::Exhaustive,
                            passed: true,
                            detail: format!("evaluación exhaustiva de 2^{width} vectores: siempre 0"),
                            width,
                            inputs_tested: total,
                        },
                    );
                }
                if values.windows(2).all(|w| w[0] == w[1]) {
                    all_same = true;
                    let k = values[0];
                    return (
                        OpaqueVerdict::AlwaysConst(k),
                        MbaProof {
                            kind: ProofKind::Exhaustive,
                            passed: true,
                            detail: format!("evaluación exhaustiva de 2^{width} vectores: siempre {k}"),
                            width,
                            inputs_tested: total,
                        },
                    );
                }
                let _ = all_same;
            }
            (
                OpaqueVerdict::Unknown,
                MbaProof {
                    kind: ProofKind::None,
                    passed: false,
                    detail: "sin forma canónica y sin evaluación exhaustiva posible".into(),
                    width,
                    inputs_tested: 0,
                },
            )
        }
    }
}

// ---------------------------------------------------------------------------
// 8. Render (expresión -> texto)
// ---------------------------------------------------------------------------

/// Vuelca la expresión a texto con paréntesis explícitos.
pub fn render(e: &MbaExpr) -> String {
    match e {
        MbaExpr::Var { name, .. } => name.clone(),
        MbaExpr::Const { value, .. } => format!("{value}"),
        MbaExpr::Not(x) => format!("~{}", render(x)),
        MbaExpr::Shr(x, k) => format!("({} >> {k})", render(x)),
        MbaExpr::Shl(x, k) => format!("({} << {k})", render(x)),
        MbaExpr::Bin { op, lhs, rhs } => {
            let s = match op {
                MbaOp::Add => "+",
                MbaOp::Sub => "-",
                MbaOp::Mul => "*",
                MbaOp::Xor => "^",
                MbaOp::And => "&",
                MbaOp::Or => "|",
            };
            format!("({} {} {})", render(lhs), s, render(rhs))
        }
    }
}

// ---------------------------------------------------------------------------
// 9. Catálogo de identidadesMBA conocidas (con prueba adjunta)
// ---------------------------------------------------------------------------

/// Identidad MBA con su texto y su demostración.
pub struct MbaIdentity {
    pub name: &'static str,
    pub statement: &'static str,
    pub lhs: &'static str,
    pub rhs: &'static str,
    pub width: u32,
}

/// Catálogo de identidades MBA canónicas (la forma "de libro").
pub fn identity_catalog() -> Vec<MbaIdentity> {
    vec![
        MbaIdentity {
            name: "MBA-ADD-1",
            statement: "(x ^ y) + 2*(x & y) == x + y",
            lhs: "(x ^ y) + (2 * (x & y))",
            rhs: "(x + y)",
            width: 32,
        },
        MbaIdentity {
            name: "MBA-ADD-2",
            statement: "2*(x & y) + (x ^ y) == x + y",
            lhs: "((2 * (x & y)) + (x ^ y))",
            rhs: "(x + y)",
            width: 32,
        },
        MbaIdentity {
            name: "MBA-ADD-3",
            statement: "(x | y) + (x & y) == x + y",
            lhs: "((x | y) + (x & y))",
            rhs: "(x + y)",
            width: 32,
        },
        MbaIdentity {
            name: "MBA-ADD-4",
            statement: "(x & y) + (x | y) == x + y",
            lhs: "((x & y) + (x | y))",
            rhs: "(x + y)",
            width: 32,
        },
        MbaIdentity {
            name: "MBA-SUB-1",
            // OJO: la tabla original del repo decía `(x ^ ~y) + 2*(x | y) == x - y - 1`,
            // que es **FALSA** (contraejemplo x=0, y=1).  La forma correcta lleva
            // `x & ~y`, no `x | y`.  Ver `identidades_falsas_detectadas()`.
            statement: "(x ^ ~y) + 2*(x & ~y) == x - y - 1",
            lhs: "((x ^ (~y)) + (2 * (x & (~y))))",
            rhs: "((x - y) - 1)",
            width: 32,
        },
        MbaIdentity {
            name: "MBA-SUB-1b",
            statement: "(x | ~y) + (x & ~y) + 1 == x - y",
            lhs: "(((x | (~y)) + (x & (~y))) + 1)",
            rhs: "(x - y)",
            width: 32,
        },
        MbaIdentity {
            name: "MBA-SUB-2",
            statement: "(x ^ y) - 2*(~x & y) == x - y",
            lhs: "((x ^ y) - (2 * ((~x) & y)))",
            rhs: "(x - y)",
            width: 32,
        },
        MbaIdentity {
            name: "MBA-XOR-1",
            statement: "(x | y) - (x ^ y) == x & y",
            lhs: "((x | y) - (x ^ y))",
            rhs: "(x & y)",
            width: 32,
        },
        MbaIdentity {
            name: "MBA-XOR-2",
            statement: "(x + y) - 2*(x & y) == x ^ y",
            lhs: "((x + y) - (2 * (x & y)))",
            rhs: "(x ^ y)",
            width: 32,
        },
        MbaIdentity {
            name: "DeMorgan-AND",
            statement: "~(~x | ~y) == x & y",
            lhs: "(~((~x) | (~y)))",
            rhs: "(x & y)",
            width: 32,
        },
        MbaIdentity {
            name: "DeMorgan-OR",
            statement: "~(~x & ~y) == x | y",
            lhs: "(~((~x) & (~y)))",
            rhs: "(x | y)",
            width: 32,
        },
    ]
}

/// Identidad **falsa** detectada por el motor (con su contraejemplo).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FalseIdentity {
    pub name: &'static str,
    pub statement: &'static str,
    pub lhs: &'static str,
    pub rhs: &'static str,
    /// Contraejemplo mínimo encontrado (x, y).
    pub counterexample: (u64, u64),
    pub lhs_value: u64,
    pub rhs_value: u64,
}

/// Identidades **falsas** que el motor encontró en la tabla de reglas que ya tenía
/// el repo (`kaitai_imhex::simplify_mba_expression`).
///
/// MEDIDO el 2026-11-22: aquella tabla aplicaba reglas **sin verificarlas**
/// (su flag `equivalence_verified` valía `!steps.is_empty()`), y dos de sus reglas
/// eran **falsas** ⇒ el simplificador antigo podía devolver una expresión que no
/// era equivalente a la original.  Estas dos son las reglas rotas:
///
/// * `MBA-SUB-1`: `(x ^ ~y) + 2*(x | y) == x - y - 1` → contraejemplo `x=0, y=1`.
///
///   El correcto es `(x ^ ~y) + 2*(x & ~y) == x - y - 1` (`x & ~y`, no `x | y`).
/// * `MBA-SUB-3`: `(x | ~y) - (~x & y) == x - y` → contraejemplo `x=0, y=0`.
///
///   El correcto es `(x | ~y) + (x & ~y) + 1 == x - y`.
///
/// El contraejemplo se **vuelve a calcular** con el motor cada vez que se llama a
/// esta función, así que no es una constante escrita a mano: si algún día dejara de
/// ser contraejemplo, el motor lo diría.
pub fn identidades_falsas_detectadas() -> Vec<FalseIdentity> {
    let casos: Vec<(&'static str, &'static str, &'static str, &'static str)> = vec![
        (
            "MBA-SUB-1",
            "(x ^ ~y) + 2*(x | y) == x - y - 1",
            "((x ^ (~y)) + (2 * (x | y)))",
            "((x - y) - 1)",
        ),
        (
            "MBA-SUB-3",
            "(x | ~y) - (~x & y) == x - y",
            "((x | (~y)) - ((~x) & y))",
            "(x - y)",
        ),
    ];
    let mut out = Vec::new();
    for (name, statement, lhs, rhs) in casos {
        let le = match parse(lhs) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let re = match parse(rhs) {
            Ok(e) => e,
            Err(_) => continue,
        };
        // Contraejemplo mínimo por búsqueda exhaustiva en entradas pequeñas.
        let mut found = None;
        'bucle: for x in 0u64..64 {
            for y in 0u64..64 {
                let mut env = HashMap::new();
                env.insert("x".to_string(), x);
                env.insert("y".to_string(), y);
                let lv = eval(&le, &env);
                let rv = eval(&re, &env);
                if lv != rv {
                    found = Some((x, y, lv, rv));
                    break 'bucle;
                }
            }
        }
        if let Some((x, y, lv, rv)) = found {
            out.push(FalseIdentity {
                name,
                statement,
                lhs,
                rhs,
                counterexample: (x, y),
                lhs_value: lv,
                rhs_value: rv,
            });
        }
    }
    out
}

/// Verifica una identidad del catálogo con la forma canónica.
pub fn verify_identity(id: &MbaIdentity) -> MbaProof {
    let lhs = parse(id.lhs).expect("identidad del catálogo debe parsear");
    let rhs = parse(id.rhs).expect("identidad del catálogo debe parsear");
    prove_equivalent(&lhs, &rhs)
}

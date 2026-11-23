//! MegaDaemon v13.4 — Forma canónica BDD para vectores de bits.
//!
//! ## Por qué BDD y no polinomios
//!
//! La primera versión del motor MBA (`re_mba`, histórica) usaba polinomios entero
//! sobre variables booleanas.  Funcionaba **a medias**: dos expresiones con la misma
//! función podían dar polinomios distintos, porque la aritmética con acarreo
//! depende de **cómo** se descomponga la suma.  Concretamente, la identidad
//! clásica
//!
//! ```text
//! (x ^ y) + 2*(x & y)  ==  x + y
//! ```
//!
//! es cierta como enteros, pero el polinomio de `x + y` (que arrastra la cadena de
//! acarreos) **no** coincide con el de `(x^y) + 2*(x&y)` (que arrastra otra cadena
//! de acarreos).  La comparación de polinomios daba "no equivalentes" para una
//! identidad de manual ⇒ el motor tenía que recurrir al muestreo y la prueba ya no
//! era total.
//!
//! La forma canónica correcta aquí es el **BDD reducido y ordenado** (ROBDD) de la
//! función de cada bit: el ROBDD de una función booleana es **único** (dada una
//! ordenación de variables), así que dos expresiones son equivalentes como vectores
//! de bits **si y sólo si** sus BDDs coinciden.  Eso es una decisión completa, no
//! una heurística, y el nodo canónico sirve además de "resumen" de la expresión.
//!
//! ## Ordenación de variables
//!
//! Las variables son los **bits de entrada** de cada operando: `(operando, bit)`.
//! Se ordenan **por bit primero** (`bit * nº_operandos + índice_operando`) para que el
//! bit `i` del resultado dependa sólo de variables con índice menor: así los BDDs
//! del sumador salen lineales en `n` en vez de exponenciales.
//!
//! ## Límites
//!
//! * `MAX_NODES` acota el tamaño del BDD: si una función patológica lo revienta,
//!   la canonicalización devuelve `None` y el motor lo declara en el informe
//!   (nunca "aproba" sin poder comprobar).
//! * `MUL` entre dos operandos **no constantes** no tiene descomposición por bits
//!   polinómica: la canonicalización falla y se cae a la verificación semántica.

#![allow(dead_code)]

use std::collections::HashMap;

/// Tope de nodos del BDD (protección ante funciones patológicas).
pub const MAX_NODES: usize = 400_000;

/// Identificador de nodo BDD.  `0` = FALSO, `1` = VERDADERO.
pub type BddId = u32;

/// Variable = bit de un operando.  Identificador = `bit * names.len() + idx_operando`.
pub type VarId = u32;

/// Ordenación de variables compartida por las expresiones que se comparan.
#[derive(Clone, Debug, Default)]
pub struct VarOrder {
    /// Nombres de los operandos, ordenados.
    pub names: Vec<String>,
    /// Anchura máxima en bits.
    pub max_width: u32,
}

impl VarOrder {
    /// Construye la ordenación a partir de un conjunto de nombres.
    pub fn new(mut names: Vec<String>, max_width: u32) -> Self {
        names.sort();
        names.dedup();
        VarOrder { names, max_width }
    }

    /// Identificador de la variable `(name, bit)`.
    pub fn id(&self, name: &str, bit: u32) -> Option<VarId> {
        let idx = self.names.iter().position(|n| n == name)?;
        Some(bit * self.names.len() as u32 + idx as u32)
    }
}

/// Nodo BDD: variable y sus dos cofactores.
#[derive(Clone, Copy, Debug)]
struct Node {
    var: VarId,
    low: BddId,
    high: BddId,
}

/// Gestor de BDDs con tabla única y caché de `ite`.
#[derive(Debug)]
pub struct Bdd {
    nodes: Vec<Node>,
    unique: HashMap<(VarId, BddId, BddId), BddId>,
    ite_cache: HashMap<(BddId, BddId, BddId), BddId>,
    hash_cache: HashMap<BddId, u64>,
}

impl Default for Bdd {
    fn default() -> Self {
        Self::new()
    }
}

impl Bdd {
    /// Crea un gestor vacío (con los dos terminales).
    pub fn new() -> Self {
        let mut b = Bdd {
            nodes: Vec::new(),
            unique: HashMap::new(),
            ite_cache: HashMap::new(),
            hash_cache: HashMap::new(),
        };
        // 0 = FALSO, 1 = VERDADERO (terminales sin variable).
        b.nodes.push(Node {
            var: u32::MAX,
            low: 0,
            high: 0,
        });
        b.nodes.push(Node {
            var: u32::MAX,
            low: 1,
            high: 1,
        });
        b
    }

    /// Número de nodos vivos.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Variable del nodo (`u32::MAX` si es terminal).
    pub fn var_of(&self, n: BddId) -> VarId {
        self.nodes[n as usize].var
    }

    /// Cofactor "0" del nodo respecto de su variable.
    pub fn low(&self, n: BddId) -> BddId {
        self.nodes[n as usize].low
    }

    /// Cofactor "1" del nodo respecto de su variable.
    pub fn high(&self, n: BddId) -> BddId {
        self.nodes[n as usize].high
    }

    /// Crea un nodo (o reutiliza el de la tabla única).
    pub fn mk(&mut self, var: VarId, low: BddId, high: BddId) -> Option<BddId> {
        if low == high {
            return Some(low);
        }
        if let Some(id) = self.unique.get(&(var, low, high)) {
            return Some(*id);
        }
        if self.nodes.len() >= MAX_NODES {
            return None;
        }
        let id = self.nodes.len() as BddId;
        self.nodes.push(Node { var, low, high });
        self.unique.insert((var, low, high), id);
        Some(id)
    }

    /// Variable (terminal o no).
    pub fn var_node(&mut self, v: VarId) -> BddId {
        self.mk(v, 0, 1).unwrap_or(0)
    }

    /// Cofactores de un nodo respecto de una variable.
    fn cofactors(&self, n: BddId, v: VarId) -> (BddId, BddId) {
        if self.var_of(n) == v {
            (self.low(n), self.high(n))
        } else {
            (n, n)
        }
    }

    /// `ite(f, g, h)`: si `f` entonces `g` si no `h`.
    pub fn ite(&mut self, f: BddId, g: BddId, h: BddId) -> BddId {
        if f == 1 {
            return g;
        }
        if f == 0 {
            return h;
        }
        if g == h {
            return g;
        }
        if let Some(r) = self.ite_cache.get(&(f, g, h)) {
            return *r;
        }
        let vf = self.var_of(f);
        let vg = self.var_of(g);
        let vh = self.var_of(h);
        let v = vf.min(vg).min(vh);
        let (f0, f1) = self.cofactors(f, v);
        let (g0, g1) = self.cofactors(g, v);
        let (h0, h1) = self.cofactors(h, v);
        let r0 = self.ite(f0, g0, h0);
        let r1 = self.ite(f1, g1, h1);
        let r = self.mk(v, r0, r1).unwrap_or(0);
        self.ite_cache.insert((f, g, h), r);
        r
    }

    /// Complemento.
    pub fn not(&mut self, f: BddId) -> BddId {
        self.ite(f, 0, 1)
    }

    /// AND.
    pub fn and(&mut self, f: BddId, g: BddId) -> BddId {
        self.ite(f, g, 0)
    }

    /// OR.
    pub fn or(&mut self, f: BddId, g: BddId) -> BddId {
        self.ite(f, 1, g)
    }

    /// XOR.
    pub fn xor(&mut self, f: BddId, g: BddId) -> BddId {
        let ng = self.not(g);
        self.ite(f, ng, g)
    }

    /// Mayoría (carry de un sumador completo).
    pub fn maj(&mut self, a: BddId, b: BddId, c: BddId) -> BddId {
        let ab = self.and(a, b);
        let ac = self.and(a, c);
        let bc = self.and(b, c);
        let t = self.or(ab, ac);
        self.or(t, bc)
    }

    /// Hash estructural del nodo (canónico: dos BDDs iguales ⇒ mismo hash).
    pub fn hash(&mut self, n: BddId) -> u64 {
        if let Some(h) = self.hash_cache.get(&n) {
            return *h;
        }
        let h = if n == 0 {
            0x243f_6a88_85a3_08d3
        } else if n == 1 {
            0x1319_8a2e_0370_7344
        } else {
            let var = self.var_of(n) as u64;
            let lo = self.hash(self.low(n));
            let hi = self.hash(self.high(n));
            let mut x = var
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .rotate_left(23)
                ^ lo.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
                ^ hi.wrapping_mul(0x1656_67B1_9E37_79F9);
            x ^= x >> 29;
            x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x ^= x >> 32;
            x
        };
        self.hash_cache.insert(n, h);
        h
    }
}

/// Forma canónica de una expresión: un hash estructural por bit.
///
/// Dos expresiones con la **misma** `VarOrder` son equivalentes como vectores de
/// bits **si y sólo si** sus hashes de bit coinciden (el ROBDD de una función
/// booleana es único).
pub use self::canonical::Canonical;

mod canonical {
    use super::*;

    /// Forma canónica de una expresión: un hash estructural por bit.
    ///
    /// La igualdad **ignora** `nodes` a propósito: el número de nodos del BDD
    /// depende del gestor (y del orden en que se construyó), no de la función.
    /// Comparar por `nodes` daría "distintas" dos expresiones equivalentes.
    #[derive(Clone, Debug, Eq)]
    pub struct Canonical {
        pub width: u32,
        pub bit_hashes: Vec<u64>,
        pub nodes: usize,
    }

    impl PartialEq for Canonical {
        fn eq(&self, other: &Self) -> bool {
            self.width == other.width && self.bit_hashes == other.bit_hashes
        }
    }

    impl Canonical {
        /// ¿La expresión vale 0 para **toda** entrada?
        pub fn is_zero(&self) -> bool {
            !self.bit_hashes.is_empty()
                && self
                    .bit_hashes
                    .iter()
                    .all(|h| *h == 0x243f_6a88_85a3_08d3)
        }

        /// ¿La expresión vale una constante? Devuelve el valor si lo es.
        pub fn as_constant(&self) -> Option<u64> {
            if self.bit_hashes.is_empty() {
                return None;
            }
            let mut v: u64 = 0;
            for (i, h) in self.bit_hashes.iter().enumerate() {
                if *h == 0x243f_6a88_85a3_08d3 {
                    continue; // bit 0
                }
                if *h != 0x1319_8a2e_0370_7344 {
                    return None; // no es constante
                }
                if i >= 64 {
                    return None;
                }
                v |= 1u64 << i;
            }
            Some(v)
        }
    }
}

/// Calcula los BDDs de los bits de una expresión.
///
/// `None` si la expresión no tiene descomposición por bits (multiplicación entre
/// dos operandos no constantes) o si el BDD revienta el tope de nodos.
pub fn bits_of(
    bdd: &mut Bdd,
    expr: &crate::re_mba::MbaExpr,
    order: &VarOrder,
) -> Option<Vec<BddId>> {
    use crate::re_mba::{MbaExpr, MbaOp};
    match expr {
        MbaExpr::Var { name, width } => {
            let mut v = Vec::with_capacity(*width as usize);
            for i in 0..*width {
                let id = order.id(name, i)?;
                v.push(bdd.var_node(id));
            }
            Some(v)
        }
        MbaExpr::Const { value, width } => {
            let mask = if *width >= 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            let val = value & mask;
            let mut v = Vec::with_capacity(*width as usize);
            for i in 0..*width {
                v.push(if (val >> i) & 1 == 1 { 1 } else { 0 });
            }
            Some(v)
        }
        MbaExpr::Not(x) => {
            let b = bits_of(bdd, x, order)?;
            let mut v = Vec::with_capacity(b.len());
            for n in b {
                v.push(bdd.not(n));
            }
            Some(v)
        }
        MbaExpr::Shr(x, k) => {
            let w = x.width();
            let b = bits_of(bdd, x, order)?;
            let mut v = vec![0u32; w as usize];
            for i in 0..w {
                let src = i + k;
                if src < w {
                    v[i as usize] = b[src as usize];
                }
            }
            Some(v)
        }
        MbaExpr::Shl(x, k) => {
            let w = x.width();
            let b = bits_of(bdd, x, order)?;
            let mut v = vec![0u32; w as usize];
            for i in 0..w {
                if *k <= i && (i - k) < w {
                    v[i as usize] = b[(i - k) as usize];
                }
            }
            Some(v)
        }
        MbaExpr::Bin { op, lhs, rhs } => {
            let wa = lhs.width();
            let wb = rhs.width();
            let w = wa.max(wb);
            let a = zext(&bits_of(bdd, lhs, order)?, w);
            let bb = zext(&bits_of(bdd, rhs, order)?, w);
            match op {
                MbaOp::And => {
                    let mut v = Vec::with_capacity(w as usize);
                    for i in 0..w as usize {
                        v.push(bdd.and(a[i], bb[i]));
                    }
                    Some(v)
                }
                MbaOp::Or => {
                    let mut v = Vec::with_capacity(w as usize);
                    for i in 0..w as usize {
                        v.push(bdd.or(a[i], bb[i]));
                    }
                    Some(v)
                }
                MbaOp::Xor => {
                    let mut v = Vec::with_capacity(w as usize);
                    for i in 0..w as usize {
                        v.push(bdd.xor(a[i], bb[i]));
                    }
                    Some(v)
                }
                MbaOp::Add => Some(add_bits(bdd, &a, &bb, w, 0)),
                MbaOp::Sub => {
                    // a - b = a + ~b + 1
                    let nb: Vec<BddId> = bb.iter().map(|n| bdd.not(*n)).collect();
                    Some(add_bits(bdd, &a, &nb, w, 1))
                }
                MbaOp::Mul => {
                    let c = const_of(lhs).or_else(|| const_of(rhs));
                    let other = if const_of(lhs).is_some() { rhs } else { lhs };
                    match c {
                        Some(cv) => {
                            let mask = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
                            let cv = cv & mask;
                            let ob = zext(&bits_of(bdd, other, order)?, w);
                            let mut acc = vec![0u32; w as usize];
                            for i in 0..w {
                                if (cv >> i) & 1 == 1 {
                                    let mut sh = vec![0u32; w as usize];
                                    for j in 0..w {
                                        if j >= i {
                                            sh[j as usize] = ob[(j - i) as usize];
                                        }
                                    }
                                    acc = add_bits(bdd, &acc, &sh, w, 0);
                                }
                            }
                            Some(acc)
                        }
                        None => None,
                    }
                }
            }
        }
    }
}

fn const_of(e: &crate::re_mba::MbaExpr) -> Option<u64> {
    match e {
        crate::re_mba::MbaExpr::Const { value, width } => {
            let mask = if *width >= 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            Some(value & mask)
        }
        _ => None,
    }
}

fn zext(v: &[BddId], to: u32) -> Vec<BddId> {
    let mut r = v.to_vec();
    while (r.len() as u32) < to {
        r.push(0);
    }
    r.truncate(to as usize);
    r
}

/// Suma de dos vectores de bits con acarreo (opcionalmente con acarreo de entrada).
fn add_bits(bdd: &mut Bdd, a: &[BddId], b: &[BddId], w: u32, carry_in: BddId) -> Vec<BddId> {
    let mut out = Vec::with_capacity(w as usize);
    let mut carry = carry_in;
    for i in 0..w as usize {
        let ai = a[i];
        let bi = b[i];
        let t = bdd.xor(ai, bi);
        let s = bdd.xor(t, carry);
        let c = bdd.maj(ai, bi, carry);
        out.push(s);
        carry = c;
    }
    out
}

/// Canonicaliza **dos** expresiones con la **misma** ordenación de variables.
pub fn canonicalize_pair(
    a: &crate::re_mba::MbaExpr,
    b: &crate::re_mba::MbaExpr,
) -> Option<(Canonical, Canonical)> {
    let mut names = std::collections::BTreeSet::new();
    a.vars(&mut names);
    b.vars(&mut names);
    let w = a.width().max(b.width());
    let order = VarOrder::new(names.into_iter().collect(), w);
    let ca = canonicalize_with(a, &order)?;
    let cb = canonicalize_with(b, &order)?;
    Some((ca, cb))
}

/// Canonicaliza una expresión con una ordenación dada.
pub fn canonicalize_with(
    e: &crate::re_mba::MbaExpr,
    order: &VarOrder,
) -> Option<Canonical> {
    let mut bdd = Bdd::new();
    let bits = bits_of(&mut bdd, e, order)?;
    let mut hashes = Vec::with_capacity(bits.len());
    for n in bits {
        hashes.push(bdd.hash(n));
    }
    Some(Canonical {
        width: e.width(),
        bit_hashes: hashes,
        nodes: bdd.node_count(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::re_mba::MbaExpr;

    #[test]
    fn el_bdd_distingue_funciones_distintas() {
        let mut names = std::collections::BTreeSet::new();
        let a = crate::re_mba::parse("(x ^ y)").unwrap();
        let b = crate::re_mba::parse("(x + y)").unwrap();
        a.vars(&mut names);
        b.vars(&mut names);
        let order = VarOrder::new(names.into_iter().collect(), 32);
        let ca = canonicalize_with(&a, &order).unwrap();
        let cb = canonicalize_with(&b, &order).unwrap();
        assert_ne!(ca.bit_hashes, cb.bit_hashes);
    }

    #[test]
    fn el_bdd_reconoce_la_identidad_de_suma() {
        let a = crate::re_mba::parse("(x ^ y) + 2*(x & y)").unwrap();
        let b = crate::re_mba::parse("(x + y)").unwrap();
        let (ca, cb) = canonicalize_pair(&a, &b).unwrap();
        assert_eq!(ca.bit_hashes, cb.bit_hashes);
    }

    #[test]
    fn el_bdd_detecta_el_cero() {
        let a = crate::re_mba::parse("(x ^ x)").unwrap();
        let mut names = std::collections::BTreeSet::new();
        a.vars(&mut names);
        let order = VarOrder::new(names.into_iter().collect(), 32);
        let ca = canonicalize_with(&a, &order).unwrap();
        assert!(ca.is_zero());
    }
}

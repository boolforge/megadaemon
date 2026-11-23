//! MegaDaemon v13.4 — IR mínimo de instrucciones para el análisis de flujo.
//!
//! ## POR QUÉ UN IR PROPIO
//!
//! Los módulos de desensamblado del repo (`re_disasm`) devuelven texto con
//! direcciones; para detectar *control-flow flattening* o *predicados opacos*
//! hace falta saber **qué bloque termina en qué** y **qué valor tiene cada
//! registro**.  En vez de atar el motor a Capstone (cuyo texto cambia entre
//! versiones) se define aquí un IR mínimo y estable:
//!
//! * [`Ins`]: una instrucción (dirección, mnemónico, operandos en texto).
//! * [`Flow`]: cómo termina un bloque (salto directo, condicional, indirecto…).
//! * [`Block`]: bloque básico con su terminador.
//! * [`Cfg`]: el grafo de control completo, con los sucesores ya calculados.
//!
//! El IR es **agnóstico delISA**: el texto de los operandos se normaliza
//! (minúsculas, sin espacios) para que las heurísticas funcionen igual con
//! `JMP 0x401000` (x86) que con `b 0x401000` (ARM64).
//!
//! ## POR QUÉ NO SE EJECUTA NADA
//!
//! Todo el análisis es estático: se leen bytes e instrucciones, nunca se
//! ejecuta el binario analizado.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

/// Una instrucción desensamblada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ins {
    /// Dirección de la instrucción.
    pub addr: u64,
    /// Mnemónico en minúsculas (`mov`, `jmp`, `cmp`…).
    pub mnem: String,
    /// Operandos en texto, ya normalizados (minúsculas, sin espacios sobrantes).
    pub ops: String,
}

impl Ins {
    /// Crea una instrucción (normaliza el texto).
    pub fn new(addr: u64, mnem: &str, ops: &str) -> Self {
        Ins {
            addr,
            mnem: mnem.trim().to_ascii_lowercase(),
            ops: normalize_ops(ops),
        }
    }

    /// Dirección del operando si es un inmediato hexadecimal (`0x401000`).
    pub fn target(&self) -> Option<u64> {
        let t = self.ops.split(',').next()?.trim();
        parse_int(t).and_then(|v| if v >= 0 { Some(v as u64) } else { None })
    }

    /// Primer operando (sin espacios).
    pub fn op0(&self) -> &str {
        self.ops.split(',').next().unwrap_or("").trim()
    }

    /// Segundo operando (sin espacios).
    pub fn op1(&self) -> &str {
        self.ops.split(',').nth(1).unwrap_or("").trim()
    }

    /// Tercer operando (sin espacios) — para las formas de tres operandos.
    pub fn op2(&self) -> &str {
        self.ops.split(',').nth(2).unwrap_or("").trim()
    }

    /// Valor inmediato del operando, si lo es (admite `0x`, decimal y `-`).
    pub fn imm(&self) -> Option<i64> {
        parse_int(self.op1())
    }

    /// ¿Es un salto (directo o indirecto)?
    pub fn is_jump(&self) -> bool {
        self.mnem.starts_with('j')
            || self.mnem == "b"
            || self.mnem == "br"
            || self.mnem.starts_with("b.")
            || self.mnem == "cbz"
            || self.mnem == "cbnz"
            || self.mnem == "tbz"
            || self.mnem == "tbnz"
    }

    /// ¿Es un salto condicional?
    pub fn is_cond_jump(&self) -> bool {
        if !self.is_jump() {
            return false;
        }
        let m = self.mnem.as_str();
        match m {
            "jmp" | "b" | "br" => false,
            _ => {
                if m.starts_with("b.") {
                    // `b.eq`, `b.ne`… son saltos condicionales de ARM64.
                    true
                } else {
                    matches!(
                        m,
                        "je" | "jne" | "jz" | "jnz" | "jg" | "jge" | "jl" | "jle" | "ja" | "jae"
                            | "jb" | "jbe" | "js" | "jns" | "jo" | "jno" | "jp" | "jnp" | "jc"
                            | "jnc" | "jeq" | "cbz" | "cbnz" | "tbz" | "tbnz"
                    )
                }
            }
        }
    }

    /// ¿Es una llamada?
    pub fn is_call(&self) -> bool {
        self.mnem == "call" || self.mnem == "bl" || self.mnem == "blx"
    }

    /// ¿Es un retorno?
    pub fn is_ret(&self) -> bool {
        self.mnem == "ret" || self.mnem.starts_with("ret")
    }
}

/// Normaliza una lista de operandos: minúsculas, sin espaciosAround de más.
pub fn normalize_ops(ops: &str) -> String {
    let s = ops.trim().to_ascii_lowercase();
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            let prev = out.chars().last();
            let mut j = i;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            let next = chars.get(j).copied();
            match prev {
                None => {}
                Some('(') => {}
                Some(',') => out.push(' '),
                _ => {
                    if !matches!(next, Some(',') | Some(')') | Some('(') | None) {
                        out.push(' ');
                    }
                }
            }
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.trim().to_string()
}

/// Parsea un inmediato (`0x10`, `10`, `-1`).
pub fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim().trim_start_matches('#');
    if t.is_empty() {
        return None;
    }
    if let Some(h) = t.strip_prefix("0x") {
        return i64::from_str_radix(h, 16).ok();
    }
    t.parse::<i64>().ok()
}

/// Cómo termina un bloque básico.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Cae al siguiente bloque.
    Fallthrough,
    /// Salto directo incondicional.
    Jump(u64),
    /// Salto condicional: si se cumple, va a `target`; si no, cae al siguiente.
    Cond {
        /// Dirección del destino de la rama verdadera.
        target: u64,
        /// Mnemónico de la condición (`je`, `jne`, `cbz`…).
        cc: String,
    },
    /// Salto indirecto (tabla de saltos, `jmp rax`, `br x3`…).
    Indirect {
        /// Destino si se conoce estáticamente (p. ej. `jmp [rip+tabla]`).
        target: Option<u64>,
    },
    /// Llamada: el flujo sigue en la instrucción siguiente.
    Call(u64),
    /// Retorno.
    Ret,
    /// Fin de bloque sin terminador reconocido.
    Unknown,
}

/// Bloque básico: instrucciones + terminador.
#[derive(Clone, Debug)]
pub struct Block {
    /// Dirección del bloque.
    pub addr: u64,
    /// Instrucciones del bloque (la última es la que termina el bloque).
    pub ins: Vec<Ins>,
    /// Terminador del bloque.
    pub flow: Flow,
}

impl Block {
    /// Última instrucción del bloque.
    pub fn last(&self) -> Option<&Ins> {
        self.ins.last()
    }

    /// Texto del bloque en una línea por instrucción.
    pub fn render(&self) -> String {
        self.ins
            .iter()
            .map(|i| format!("{:#x}: {} {}", i.addr, i.mnem, i.ops))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Grafo de control de control: bloques + mapa por dirección.
#[derive(Clone, Debug, Default)]
pub struct Cfg {
    /// Bloques en orden de dirección.
    pub blocks: Vec<Block>,
}

impl Cfg {
    /// Construye el CFG a partir de una lista lineal de instrucciones,
    /// partiendo en bloques donde hay un terminador.
    pub fn from_ins(ins: &[Ins]) -> Cfg {
        let mut blocks: Vec<Block> = Vec::new();
        let mut cur: Vec<Ins> = Vec::new();
        for i in ins {
            cur.push(i.clone());
            let flow = flow_of(i, &cur);
            let terminal = matches!(
                flow,
                Flow::Jump(_) | Flow::Cond { .. } | Flow::Indirect { .. } | Flow::Ret
            );
            if terminal {
                blocks.push(Block {
                    addr: cur[0].addr,
                    ins: std::mem::take(&mut cur),
                    flow,
                });
            }
        }
        if !cur.is_empty() {
            let flow = Flow::Fallthrough;
            blocks.push(Block {
                addr: cur[0].addr,
                ins: cur,
                flow,
            });
        }
        Cfg { blocks }
    }

    /// Construye el CFG a partir de un desensamblado (`(dirección, texto)`).
    ///
    /// El texto es el que devuelve Capstone/iced: `"mov w0, #0x1234"`.
    pub fn desde_disasm(instrs: &[(u64, String)]) -> Cfg {
        let ins: Vec<Ins> = instrs
            .iter()
            .map(|(addr, texto)| {
                let t = texto.trim();
                match t.find(' ') {
                    Some(i) => Ins::new(*addr, &t[..i], &t[i + 1..]),
                    None => Ins::new(*addr, t, ""),
                }
            })
            .collect();
        Cfg::from_ins(&ins)
    }

    /// Índice del bloque que empieza en `addr`.
    pub fn index_of(&self, addr: u64) -> Option<usize> {
        self.blocks.iter().position(|b| b.addr == addr)
    }

    /// Bloque que empieza en `addr`.
    pub fn block_at(&self, addr: u64) -> Option<&Block> {
        self.blocks.iter().find(|b| b.addr == addr)
    }

    /// Sucesores de un bloque (direcciones).
    pub fn succs(&self, b: &Block) -> Vec<u64> {
        let mut out = Vec::new();
        match b.flow {
            Flow::Fallthrough => {
                if let Some(next) = self.next_addr(b.addr) {
                    out.push(next);
                }
            }
            Flow::Jump(t) => out.push(t),
            Flow::Cond { target, .. } => {
                out.push(target);
                if let Some(next) = self.next_addr(b.addr) {
                    if next != target {
                        out.push(next);
                    }
                }
            }
            Flow::Indirect { target } => {
                if let Some(t) = target {
                    out.push(t);
                }
            }
            Flow::Call(_) => {
                if let Some(next) = self.next_addr(b.addr) {
                    out.push(next);
                }
            }
            Flow::Ret | Flow::Unknown => {}
        }
        out
    }

    /// Dirección del bloque siguiente en el orden lineal.
    pub fn next_addr(&self, addr: u64) -> Option<u64> {
        let mut addrs: Vec<u64> = self.blocks.iter().map(|b| b.addr).collect();
        addrs.sort_unstable();
        let pos = addrs.binary_search(&addr).ok()?;
        addrs.get(pos + 1).copied()
    }

    /// Grafo de adyacencia (dirección -> sucesores).
    pub fn edges(&self) -> BTreeMap<u64, Vec<u64>> {
        let mut m = BTreeMap::new();
        for b in &self.blocks {
            m.insert(b.addr, self.succs(b));
        }
        m
    }

    /// Grado de entrada de cada bloque (cuántos bloques lo apuntan).
    pub fn in_degree(&self) -> BTreeMap<u64, usize> {
        let mut m: BTreeMap<u64, usize> = BTreeMap::new();
        for b in &self.blocks {
            m.entry(b.addr).or_insert(0);
            for s in self.succs(b) {
                *m.entry(s).or_insert(0) += 1;
            }
        }
        m
    }

    /// Direcciones de todos los bloques.
    pub fn addrs(&self) -> Vec<u64> {
        let mut v: Vec<u64> = self.blocks.iter().map(|b| b.addr).collect();
        v.sort_unstable();
        v
    }

    /// Conjunto de direcciones alcanzables desde `addr`.
    pub fn reachable(&self, addr: u64) -> BTreeSet<u64> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![addr];
        while let Some(a) = stack.pop() {
            if !seen.insert(a) {
                continue;
            }
            if let Some(b) = self.block_at(a) {
                for s in self.succs(b) {
                    if !seen.contains(&s) {
                        stack.push(s);
                    }
                }
            }
        }
        seen
    }
}

/// Dispatcher detectado en un CFG real (a partir del desensamblado).
#[derive(Clone, Debug)]
pub struct DispatcherCfg {
    /// Dirección del bloque dispatcher.
    pub addr: u64,
    /// Texto del salto indirecto (`rax`, `[rip+0x1234]`…).
    pub destino_indirecto: String,
    /// Grado de entrada: cuántos bloques saltan al dispatcher.
    pub grado_entrada: usize,
    /// Variable de estado (registro) que se asigna antes de saltar.
    pub variable_estado: String,
    /// Casos: (valor del estado -> bloque destino).
    pub casos: Vec<(i64, u64)>,
}

/// Detecta el dispatcher de un CFG **real** (desensamblado).
///
/// Heurística: el dispatcher es el bloque con mayor grado de entrada; los
/// "setters" son los bloques que le asignan un estado (un `mov reg, inm`
/// justo antes del salto) y los casos se emparejan estado -> bloque.
pub fn detectar_dispatcher_cfg(cfg: &Cfg) -> Option<DispatcherCfg> {
    let in_deg = cfg.in_degree();
    let mut mejor: Option<(u64, usize)> = None;
    for b in &cfg.blocks {
        let g = *in_deg.get(&b.addr).unwrap_or(&0);
        if g < 3 {
            continue;
        }
        match mejor {
            Some((_, pg)) if pg >= g => {}
            _ => mejor = Some((b.addr, g)),
        }
    }
    let (addr, grado) = mejor?;
    // Variable de estado: el registro asignado en los bloques que van al
    // dispatcher (deben coincidir).
    let mut variable = String::new();
    let mut casos: Vec<(i64, u64)> = Vec::new();
    for b in &cfg.blocks {
        if b.addr == addr {
            continue;
        }
        if !cfg.succs(b).contains(&addr) {
            continue;
        }
        if let Some((reg, valor)) = estado_antes_del_salto(b) {
            if variable.is_empty() {
                variable = reg.clone();
            }
            if variable == reg {
                casos.push((valor, b.addr));
            }
        }
    }
    casos.sort_by_key(|(k, _)| *k);
    let destino_indirecto = cfg
        .block_at(addr)
        .and_then(|b| b.last().map(|i| i.ops.clone()))
        .unwrap_or_default();
    Some(DispatcherCfg {
        addr,
        destino_indirecto,
        grado_entrada: grado,
        variable_estado: variable,
        casos,
    })
}

/// Estado (`mov reg, inm`) asignado justo antes del salto final del bloque.
pub fn estado_antes_del_salto(b: &Block) -> Option<(String, i64)> {
    let n = b.ins.len();
    if n < 2 {
        return None;
    }
    for i in (0..n.saturating_sub(1)).rev().take(3) {
        let ins = &b.ins[i];
        if matches!(ins.mnem.as_str(), "mov" | "movabs" | "movz") {
            if let Some(v) = crate::re_ir::parse_int(ins.op1()) {
                return Some((ins.op0().to_string(), v));
            }
        }
    }
    None
}

/// Calcula el terminador de una instrucción.
fn flow_of(i: &Ins, _block: &[Ins]) -> Flow {
    if i.is_ret() {
        return Flow::Ret;
    }
    if i.is_call() {
        return Flow::Call(i.target().unwrap_or(0));
    }
    if i.is_jump() {
        match i.mnem.as_str() {
            "jmp" | "b" | "br" => {
                return match i.target() {
                    Some(t) => Flow::Jump(t),
                    None => Flow::Indirect { target: None },
                }
            }
            _ => {
                if let Some(t) = i.target() {
                    return Flow::Cond { target: t, cc: i.mnem.clone() };
                }
                return Flow::Indirect { target: None };
            }
        }
    }
    Flow::Fallthrough
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_cfg_se_parte_en_bloques() {
        let ins = vec![
            Ins::new(0x1000, "mov", "rax, 1"),
            Ins::new(0x1004, "je", "0x1020"),
            Ins::new(0x1008, "mov", "rbx, 2"),
            Ins::new(0x100c, "jmp", "0x1030"),
            Ins::new(0x1020, "nop", ""),
        ];
        let cfg = Cfg::from_ins(&ins);
        assert_eq!(cfg.blocks.len(), 3);
        assert_eq!(cfg.blocks[0].addr, 0x1000);
        assert!(matches!(cfg.blocks[0].flow, Flow::Cond { target: 0x1020, .. }));
        assert_eq!(cfg.succs(&cfg.blocks[0]), vec![0x1020, 0x1008]);
    }

    #[test]
    fn los_operandos_se_normalizan() {
        let i = Ins::new(0, "MOV", "  RAX ,  0X10 ");
        assert_eq!(i.mnem, "mov");
        assert_eq!(i.ops, "rax, 0x10");
        assert_eq!(i.imm(), Some(16));
    }
}

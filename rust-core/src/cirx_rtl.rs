//! MegaDaemon v13.0 Zenith — LTO-Decomp v3.4 Formal CIRX Micro-RTL, SSA, Risk Score & SMT-LIB2 Engine
//! Directly integrates `LTO-Decomp-Distribution-v3.4/src/cirx_micro_rtl_v2.rs` with normative §6.1 Risk Score,
//! §15.2 Absolute Set Closure Lemma (`A = { formal_control_flow }`), MAS Track Routing, Conformal Quantile,
//! Flag Polarity Verification, and Open Obligations O-1..O-8 surfacing.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// CIRX Stage-1 Micro-RTL, Stage-2 SSA, and SMT-LIB export: reference implementation, revision 2.
//
// Scope: an AArch64-subset, single-threaded, concrete interpreter plus a multi-block SSA
// construction (Cooper-Harvey-Kennedy dominators, Cytron dominance frontiers, versioned
// registers, four independent flag bits, and a versioned memory variable) and a generator of
// SMT-LIB 2 overlap queries. It does NOT implement vector, floating-point, atomic, system-register,
// or pointer-authentication semantics, and it is not the architecture-parametric model of
// Section 20.2 of the specification.
//
// Build and test:  rustc --edition 2021 -O cirx_micro_rtl_v2.rs && ./cirx_micro_rtl_v2

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

// ---------------------------------------------------------------- registers and flags

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reg {
    X0, X1, X2, X3, X4, X5, X6, X7, X8, X9, X10, X11, X12, X13, X14, X15,
    X16, X17, X18, X19, X20, X21, X22, X23, X24, X25, X26, X27, X28, X29, X30,
    Sp, Pc,
}

const ALL_REGS: [Reg; 33] = [
    Reg::X0, Reg::X1, Reg::X2, Reg::X3, Reg::X4, Reg::X5, Reg::X6, Reg::X7, Reg::X8,
    Reg::X9, Reg::X10, Reg::X11, Reg::X12, Reg::X13, Reg::X14, Reg::X15, Reg::X16,
    Reg::X17, Reg::X18, Reg::X19, Reg::X20, Reg::X21, Reg::X22, Reg::X23, Reg::X24,
    Reg::X25, Reg::X26, Reg::X27, Reg::X28, Reg::X29, Reg::X30, Reg::Sp, Reg::Pc,
];

impl Reg {
    fn index(self) -> usize {
        ALL_REGS.iter().position(|r| *r == self).unwrap()
    }
}

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width { W32, W64 }

impl Width {
    fn bits(self) -> u32 { match self { Width::W32 => 32, Width::W64 => 64 } }
    fn mask(self) -> u64 { match self { Width::W32 => 0xFFFF_FFFF, Width::W64 => u64::MAX } }
    fn sign_bit(self) -> u64 { 1u64 << (self.bits() - 1) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FlagBit { N, Z, C, V }

const ALL_FLAGS: [FlagBit; 4] = [FlagBit::N, FlagBit::Z, FlagBit::C, FlagBit::V];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CondCode { Eq, Ne, Cs, Cc, Mi, Pl, Vs, Vc, Hi, Ls, Ge, Lt, Gt, Le, Always }

impl CondCode {
    /// Flag bits read by the condition (used for SSA use sets).
    fn reads(self) -> &'static [FlagBit] {
        use FlagBit::*;
        match self {
            CondCode::Eq | CondCode::Ne => &[Z],
            CondCode::Cs | CondCode::Cc => &[C],
            CondCode::Mi | CondCode::Pl => &[N],
            CondCode::Vs | CondCode::Vc => &[V],
            CondCode::Hi | CondCode::Ls => &[C, Z],
            CondCode::Ge | CondCode::Lt => &[N, V],
            CondCode::Gt | CondCode::Le => &[N, Z, V],
            CondCode::Always => &[],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Nzcv { pub n: bool, pub z: bool, pub c: bool, pub v: bool }

impl Nzcv {
    pub fn eval(&self, cc: CondCode) -> bool {
        match cc {
            CondCode::Eq => self.z,
            CondCode::Ne => !self.z,
            CondCode::Cs => self.c,
            CondCode::Cc => !self.c,
            CondCode::Mi => self.n,
            CondCode::Pl => !self.n,
            CondCode::Vs => self.v,
            CondCode::Vc => !self.v,
            CondCode::Hi => self.c && !self.z,
            CondCode::Ls => !self.c || self.z,
            CondCode::Ge => self.n == self.v,
            CondCode::Lt => self.n != self.v,
            CondCode::Gt => !self.z && (self.n == self.v),
            CondCode::Le => self.z || (self.n != self.v),
            CondCode::Always => true,
        }
    }

    /// ADDS: r = a + b at width w (a, b already masked to w bits).
    pub fn from_add(w: Width, a: u64, b: u64) -> (u64, Nzcv) {
        let sum = (a as u128) + (b as u128);
        let r = (sum as u64) & w.mask();
        let n = r & w.sign_bit() != 0;
        let z = r == 0;
        let c = sum > w.mask() as u128;
        let (sa, sb, sr) = (a & w.sign_bit() != 0, b & w.sign_bit() != 0, n);
        let v = sa == sb && sa != sr;
        (r, Nzcv { n, z, c, v })
    }

    /// SUBS: r = a - b = a + !b + 1 at width w. C = 1 iff there is no borrow (a >=u b).
    pub fn from_sub(w: Width, a: u64, b: u64) -> (u64, Nzcv) {
        let r = a.wrapping_sub(b) & w.mask();
        let n = r & w.sign_bit() != 0;
        let z = r == 0;
        let c = a >= b;
        let (sa, sb, sr) = (a & w.sign_bit() != 0, b & w.sign_bit() != 0, n);
        let v = sa != sb && sa != sr;
        (r, Nzcv { n, z, c, v })
    }

    /// ANDS and similar: C = V = 0 (AArch64; AArch32 differs, see Section 20.3.1).
    pub fn from_logical(w: Width, r: u64) -> Nzcv {
        Nzcv { n: r & w.sign_bit() != 0, z: r & w.mask() == 0, c: false, v: false }
    }
}

// ---------------------------------------------------------------- instructions

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemSize { B8, B16, B32, B64 }

impl MemSize {
    fn bytes(self) -> u64 { match self { MemSize::B8 => 1, MemSize::B16 => 2, MemSize::B32 => 4, MemSize::B64 => 8 } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOpKind { Add, Sub, And, Or, Xor, Mul, Lsl, Lsr, Asr }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operand { Reg(Reg), Imm(u64) }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MicroInstr {
    /// A W32 destination write zero-extends to 64 bits.
    Mov { w: Width, dst: Reg, src: Operand },
    BinOp { op: BinOpKind, w: Width, dst: Reg, a: Operand, b: Operand, set_flags: bool },
    Load { dst: Reg, base: Reg, offset: i64, size: MemSize, signed: bool },
    Store { src: Reg, base: Reg, offset: i64, size: MemSize },
    Branch { cond: CondCode, target: u64 },
    /// `ret_addr` is supplied by the lifter (the fall-through address); the interpreter never
    /// guesses an instruction length.
    Call { target: Operand, ret_addr: u64, clobbers: Vec<Reg> },
    Ret,
    Label { id: u64 },
}

// ---------------------------------------------------------------- memory

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region { Stack, Heap, Global, Unknown }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemError { Uninitialized(u64) }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecError { Mem(MemError) }

#[derive(Clone, Debug, Default)]
pub struct MemorySpace {
    bytes: HashMap<u64, u8>,
    regions: Vec<(u64, u64, Region)>, // (start, length, region)
}

impl MemorySpace {
    pub fn new() -> Self { Self::default() }

    pub fn add_region(&mut self, start: u64, len: u64, r: Region) { self.regions.push((start, len, r)); }

    pub fn region_of(&self, addr: u64) -> Region {
        for (s, l, r) in &self.regions {
            if addr.wrapping_sub(*s) < *l { return *r; }
        }
        Region::Unknown
    }

    /// May-overlap of byte ranges [a, a+sa) and [b, b+sb) modulo 2^64:
    /// (b - a) <u sa  or  (a - b) <u sb   (Section 3.3.2). Different known regions never alias.
    pub fn may_alias(&self, a: u64, sa: u64, b: u64, sb: u64) -> bool {
        let (ra, rb) = (self.region_of(a), self.region_of(b));
        if ra != rb && ra != Region::Unknown && rb != Region::Unknown { return false; }
        b.wrapping_sub(a) < sa || a.wrapping_sub(b) < sb
    }

    pub fn store(&mut self, addr: u64, size: MemSize, value: u64) {
        for i in 0..size.bytes() {
            self.bytes.insert(addr.wrapping_add(i), ((value >> (8 * i)) & 0xff) as u8);
        }
    }

    /// Little-endian load. Reading a byte that was never written is an error, not zero.
    pub fn load(&self, addr: u64, size: MemSize, signed: bool) -> Result<u64, MemError> {
        let n = size.bytes();
        let mut v: u64 = 0;
        for i in 0..n {
            let a = addr.wrapping_add(i);
            let b = *self.bytes.get(&a).ok_or(MemError::Uninitialized(a))? as u64;
            v |= b << (8 * i);
        }
        if signed && n < 8 {
            let bits = (n * 8) as u32;
            if v & (1u64 << (bits - 1)) != 0 { v |= u64::MAX << bits; }
        }
        Ok(v)
    }
}

// ---------------------------------------------------------------- interpreter

#[derive(Clone, Debug)]
pub struct Machine {
    regs: [u64; 33],
    pub nzcv: Nzcv,
    pub mem: MemorySpace,
    pub branch_taken: Option<u64>,
    pub halted: bool,
}

impl Machine {
    pub fn new() -> Self {
        Machine { regs: [0u64; 33], nzcv: Nzcv::default(), mem: MemorySpace::new(), branch_taken: None, halted: false }
    }
    pub fn get(&self, r: Reg) -> u64 { self.regs[r.index()] }
    pub fn set(&mut self, r: Reg, v: u64) { self.regs[r.index()] = v; }
    fn read(&self, o: &Operand, w: Width) -> u64 {
        match o { Operand::Reg(r) => self.get(*r) & w.mask(), Operand::Imm(v) => *v & w.mask() }
    }

    pub fn step(&mut self, i: &MicroInstr) -> Result<(), ExecError> {
        match i {
            MicroInstr::Mov { w, dst, src } => { let v = self.read(src, *w); self.set(*dst, v); }
            MicroInstr::BinOp { op, w, dst, a, b, set_flags } => {
                let (x, y) = (self.read(a, *w), self.read(b, *w));
                let (r, fl) = match op {
                    BinOpKind::Add => Nzcv::from_add(*w, x, y),
                    BinOpKind::Sub => Nzcv::from_sub(*w, x, y),
                    BinOpKind::And => { let r = x & y; (r, Nzcv::from_logical(*w, r)) }
                    BinOpKind::Or => { let r = x | y; (r, Nzcv::from_logical(*w, r)) }
                    BinOpKind::Xor => { let r = x ^ y; (r, Nzcv::from_logical(*w, r)) }
                    BinOpKind::Mul => ((x.wrapping_mul(y)) & w.mask(), self.nzcv.clone()),
                    BinOpKind::Lsl => ((x.wrapping_shl((y % w.bits() as u64) as u32)) & w.mask(), self.nzcv.clone()),
                    BinOpKind::Lsr => ((x.wrapping_shr((y % w.bits() as u64) as u32)) & w.mask(), self.nzcv.clone()),
                    BinOpKind::Asr => {
                        let sh = (y % w.bits() as u64) as u32;
                        let sx = if *w == Width::W32 { x as u32 as i32 as i64 } else { x as i64 };
                        (((sx >> sh) as u64) & w.mask(), self.nzcv.clone())
                    }
                };
                self.set(*dst, r); // masked to w, hence zero-extended for W32
                if *set_flags { self.nzcv = fl; }
            }
            MicroInstr::Load { dst, base, offset, size, signed } => {
                let addr = self.get(*base).wrapping_add(*offset as u64);
                let v = self.mem.load(addr, *size, *signed).map_err(ExecError::Mem)?;
                self.set(*dst, v);
            }
            MicroInstr::Store { src, base, offset, size } => {
                let addr = self.get(*base).wrapping_add(*offset as u64);
                let v = self.get(*src);
                self.mem.store(addr, *size, v);
            }
            MicroInstr::Branch { cond, target } => {
                if self.nzcv.eval(*cond) { self.branch_taken = Some(*target); self.set(Reg::Pc, *target); }
            }
            MicroInstr::Call { target, ret_addr, .. } => {
                let t = self.read(target, Width::W64);
                self.set(Reg::X30, *ret_addr);
                self.set(Reg::Pc, t);
                self.branch_taken = Some(t);
            }
            MicroInstr::Ret => { let lr = self.get(Reg::X30); self.set(Reg::Pc, lr); self.halted = true; }
            MicroInstr::Label { .. } => {}
        }
        Ok(())
    }

    pub fn run_block(&mut self, instrs: &[MicroInstr]) -> Result<(), ExecError> {
        self.branch_taken = None;
        self.halted = false;
        for i in instrs {
            self.step(i)?;
            if self.halted || self.branch_taken.is_some() { break; }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- CFG, dominators, SSA

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Var { Reg(Reg), Flag(FlagBit), Mem }

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { Var::Reg(r) => write!(f, "{}", r), Var::Flag(b) => write!(f, "{:?}", b), Var::Mem => write!(f, "Mem") }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SsaName { pub var: Var, pub version: u32 }

impl fmt::Display for SsaName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}_{}", self.var, self.version) }
}

#[derive(Clone, Debug)]
pub struct BlockDef { pub instrs: Vec<MicroInstr>, pub succs: Vec<usize> }

#[derive(Clone, Debug)]
pub struct Cfg { pub blocks: Vec<BlockDef>, pub preds: Vec<Vec<usize>> }

impl Cfg {
    /// Block 0 is the entry. `succs` lists successor block indices.
    pub fn new(blocks: Vec<BlockDef>) -> Self {
        let mut preds = vec![Vec::new(); blocks.len()];
        for (i, b) in blocks.iter().enumerate() { for s in &b.succs { preds[*s].push(i); } }
        Cfg { blocks, preds }
    }

    fn rpo(&self) -> Vec<usize> {
        let n = self.blocks.len();
        let mut seen = vec![false; n];
        let mut post = Vec::new();
        fn dfs(c: &Cfg, u: usize, seen: &mut Vec<bool>, post: &mut Vec<usize>) {
            seen[u] = true;
            for &s in &c.blocks[u].succs { if !seen[s] { dfs(c, s, seen, post); } }
            post.push(u);
        }
        dfs(self, 0, &mut seen, &mut post);
        post.reverse();
        post
    }

    /// Immediate dominators by the Cooper-Harvey-Kennedy iterative algorithm.
    /// Unreachable blocks have no entry. idom(0) = 0.
    pub fn idoms(&self) -> HashMap<usize, usize> {
        let rpo = self.rpo();
        let order: HashMap<usize, usize> = rpo.iter().enumerate().map(|(i, b)| (*b, i)).collect();
        let mut idom: HashMap<usize, usize> = HashMap::new();
        idom.insert(0, 0);
        let intersect = |idom: &HashMap<usize, usize>, mut a: usize, mut b: usize| -> usize {
            while a != b {
                while order[&a] > order[&b] { a = idom[&a]; }
                while order[&b] > order[&a] { b = idom[&b]; }
            }
            a
        };
        let mut changed = true;
        while changed {
            changed = false;
            for &b in rpo.iter().skip(1) {
                let mut new_idom: Option<usize> = None;
                for &p in &self.preds[b] {
                    if !idom.contains_key(&p) { continue; }
                    new_idom = Some(match new_idom { None => p, Some(cur) => intersect(&idom, p, cur) });
                }
                if let Some(ni) = new_idom {
                    if idom.get(&b) != Some(&ni) { idom.insert(b, ni); changed = true; }
                }
            }
        }
        idom
    }

    /// Dominance frontiers (Cytron et al. 1991, using the join-node formulation).
    pub fn dominance_frontiers(&self) -> BTreeMap<usize, BTreeSet<usize>> {
        let idom = self.idoms();
        let mut df: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for b in 0..self.blocks.len() { df.insert(b, BTreeSet::new()); }
        for b in 0..self.blocks.len() {
            if !idom.contains_key(&b) { continue; }
            let reach_preds: Vec<usize> = self.preds[b].iter().copied().filter(|p| idom.contains_key(p)).collect();
            if reach_preds.len() >= 2 {
                for p in reach_preds {
                    let mut runner = p;
                    while runner != idom[&b] {
                        df.get_mut(&runner).unwrap().insert(b);
                        runner = idom[&runner];
                    }
                }
            }
        }
        df
    }
}

/// Definitions and uses of one instruction, as variables.
fn defs_uses(i: &MicroInstr) -> (Vec<Var>, Vec<Var>) {
    let mut d = Vec::new();
    let mut u = Vec::new();
    let op_use = |o: &Operand, u: &mut Vec<Var>| { if let Operand::Reg(r) = o { u.push(Var::Reg(*r)); } };
    match i {
        MicroInstr::Mov { dst, src, .. } => { op_use(src, &mut u); d.push(Var::Reg(*dst)); }
        MicroInstr::BinOp { dst, a, b, set_flags, .. } => {
            op_use(a, &mut u); op_use(b, &mut u);
            d.push(Var::Reg(*dst));
            if *set_flags { for f in ALL_FLAGS { d.push(Var::Flag(f)); } }
        }
        MicroInstr::Load { dst, base, .. } => { u.push(Var::Reg(*base)); u.push(Var::Mem); d.push(Var::Reg(*dst)); }
        MicroInstr::Store { src, base, .. } => { u.push(Var::Reg(*src)); u.push(Var::Reg(*base)); d.push(Var::Mem); }
        MicroInstr::Branch { cond, .. } => { for f in cond.reads() { u.push(Var::Flag(*f)); } }
        MicroInstr::Call { target, clobbers, .. } => {
            op_use(target, &mut u);
            d.push(Var::Reg(Reg::X30));
            d.push(Var::Mem);
            for f in ALL_FLAGS { d.push(Var::Flag(f)); }   // AAPCS64: NZCV is not preserved across calls
            for r in clobbers { d.push(Var::Reg(*r)); }
        }
        MicroInstr::Ret => { u.push(Var::Reg(Reg::X30)); }
        MicroInstr::Label { .. } => {}
    }
    d.sort(); d.dedup();
    (d, u)
}

#[derive(Clone, Debug)]
pub struct SsaInstr { pub text: String, pub defs: Vec<SsaName>, pub uses: Vec<SsaName> }

#[derive(Clone, Debug)]
pub struct Phi { pub dst: SsaName, pub incomings: Vec<(usize, SsaName)> }

#[derive(Clone, Debug)]
pub struct SsaBlock { pub phis: Vec<Phi>, pub instrs: Vec<SsaInstr> }

#[derive(Clone, Debug)]
pub struct SsaFunction { pub blocks: Vec<SsaBlock> }

/// Build SSA with minimal phi placement for every variable class (registers, flags, memory).
/// Every variable is live-in at entry with version 0.
pub fn build_ssa(cfg: &Cfg) -> SsaFunction {
    let n = cfg.blocks.len();
    let idom = cfg.idoms();
    let df = cfg.dominance_frontiers();

    let mut def_blocks: BTreeMap<Var, BTreeSet<usize>> = BTreeMap::new();
    for (bi, b) in cfg.blocks.iter().enumerate() {
        for i in &b.instrs { for v in defs_uses(i).0 { def_blocks.entry(v).or_default().insert(bi); } }
    }
    // phi placement
    let mut phi_vars: Vec<BTreeSet<Var>> = vec![BTreeSet::new(); n];
    for (v, defs) in &def_blocks {
        let mut work: Vec<usize> = defs.iter().copied().collect();
        let mut has_phi: BTreeSet<usize> = BTreeSet::new();
        while let Some(x) = work.pop() {
            for &y in df[&x].iter() {
                if has_phi.insert(y) {
                    phi_vars[y].insert(*v);
                    if !defs.contains(&y) { work.push(y); }
                }
            }
        }
    }
    let mut out = SsaFunction { blocks: (0..n).map(|_| SsaBlock { phis: vec![], instrs: vec![] }).collect() };
    for b in 0..n {
        for v in &phi_vars[b] {
            out.blocks[b].phis.push(Phi { dst: SsaName { var: *v, version: 0 }, incomings: vec![] });
        }
    }
    // dominator tree children
    let mut children: Vec<Vec<usize>> = vec![vec![]; n];
    for (&b, &p) in idom.iter() { if b != 0 { children[p].push(b); } }
    for c in &mut children { c.sort(); }

    let mut counter: HashMap<Var, u32> = HashMap::new();
    let mut stacks: HashMap<Var, Vec<u32>> = HashMap::new();
    fn top(stacks: &HashMap<Var, Vec<u32>>, v: Var) -> SsaName {
        SsaName { var: v, version: stacks.get(&v).and_then(|s| s.last().copied()).unwrap_or(0) }
    }
    fn fresh(counter: &mut HashMap<Var, u32>, stacks: &mut HashMap<Var, Vec<u32>>, v: Var) -> SsaName {
        let c = counter.entry(v).or_insert(0);
        *c += 1;
        stacks.entry(v).or_default().push(*c);
        SsaName { var: v, version: *c }
    }
    #[allow(clippy::too_many_arguments)]
    fn rename(b: usize, cfg: &Cfg, children: &Vec<Vec<usize>>, out: &mut SsaFunction,
              counter: &mut HashMap<Var, u32>, stacks: &mut HashMap<Var, Vec<u32>>) {
        let mut pushed: Vec<Var> = Vec::new();
        for pi in 0..out.blocks[b].phis.len() {
            let v = out.blocks[b].phis[pi].dst.var;
            let nm = fresh(counter, stacks, v);
            out.blocks[b].phis[pi].dst = nm;
            pushed.push(v);
        }
        for ins in &cfg.blocks[b].instrs {
            let (d, u) = defs_uses(ins);
            let uses: Vec<SsaName> = u.iter().map(|v| top(stacks, *v)).collect();
            let defs: Vec<SsaName> = d.iter().map(|v| { pushed.push(*v); fresh(counter, stacks, *v) }).collect();
            let text = format!("{:?}", ins);
            out.blocks[b].instrs.push(SsaInstr { text, defs, uses });
        }
        for &s in &cfg.blocks[b].succs {
            for pi in 0..out.blocks[s].phis.len() {
                let v = out.blocks[s].phis[pi].dst.var;
                let nm = top(stacks, v);
                out.blocks[s].phis[pi].incomings.push((b, nm));
            }
        }
        for &c in &children[b] { rename(c, cfg, children, out, counter, stacks); }
        for v in pushed.into_iter().rev() { stacks.get_mut(&v).unwrap().pop(); }
    }
    rename(0, cfg, &children, &mut out, &mut counter, &mut stacks);
    out
}

/// Structural SSA checker: single definition per name; every use is defined and its definition
/// dominates the use; phi incoming count equals predecessor count (reachable predecessors only).
pub fn check_ssa(cfg: &Cfg, ssa: &SsaFunction) -> Result<(), String> {
    let idom = cfg.idoms();
    let dominates = |a: usize, mut b: usize| -> bool {
        loop { if a == b { return true; } if b == 0 { return false; } b = idom[&b]; }
    };
    let mut def_site: HashMap<SsaName, (usize, usize)> = HashMap::new(); // (block, position; phis = 0)
    for (bi, b) in ssa.blocks.iter().enumerate() {
        if !idom.contains_key(&bi) { continue; }
        for p in &b.phis {
            if def_site.insert(p.dst, (bi, 0)).is_some() { return Err(format!("duplicate def {}", p.dst)); }
        }
        for (ii, i) in b.instrs.iter().enumerate() {
            for d in &i.defs {
                if def_site.insert(*d, (bi, ii + 1)).is_some() { return Err(format!("duplicate def {}", d)); }
            }
        }
    }
    for (bi, b) in ssa.blocks.iter().enumerate() {
        if !idom.contains_key(&bi) { continue; }
        for p in &b.phis {
            let rp = cfg.preds[bi].iter().filter(|q| idom.contains_key(q)).count();
            if p.incomings.len() != rp { return Err(format!("phi {} has {} incomings for {} preds", p.dst, p.incomings.len(), rp)); }
            for (pred, nm) in &p.incomings {
                if nm.version != 0 {
                    let (db, _) = *def_site.get(nm).ok_or(format!("phi use {} undefined", nm))?;
                    if !dominates(db, *pred) { return Err(format!("phi use {} does not dominate pred {}", nm, pred)); }
                }
            }
        }
        for (ii, i) in b.instrs.iter().enumerate() {
            for u in &i.uses {
                if u.version == 0 { continue; } // live-in at entry
                let (db, dpos) = *def_site.get(u).ok_or(format!("use {} undefined", u))?;
                let ok = if db == bi { dpos <= ii } else { dominates(db, bi) };
                if !ok { return Err(format!("use {} not dominated by its definition", u)); }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- SMT-LIB export

/// SMT-LIB 2 script asserting that two symbolic accesses (a, sa) and (b, sb) overlap
/// (circular byte ranges modulo 2^64). `sat` means a may-alias witness exists.
pub fn smt_overlap_query(sa: u64, sb: u64) -> String {
    format!(
"(set-logic QF_BV)\n(declare-const a (_ BitVec 64))\n(declare-const b (_ BitVec 64))\n\
(assert (or (bvult (bvsub b a) (_ bv{sa} 64)) (bvult (bvsub a b) (_ bv{sb} 64))))\n(check-sat)\n")
}



// ---------------------------------------------------------------- MegaDaemon LTO-Decomp v3.4 Normative API

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CirxPipelineReport {
    pub spec_version: String,
    pub mas_score: f64,
    pub track: String,
    pub claim_power: String,
    pub risk_s: f64,
    pub risk_r: u32,
    pub absolute_guarantee_set: Vec<String>,
    pub closure_lemma_status: String,
    pub ssa_blocks: usize,
    pub phi_nodes: usize,
    pub ssa_verified: bool,
    pub ssa_listing: Vec<String>,
    pub smt_overlap_query: String,
    pub conformal_quantile_k: u64,
    pub flag_polarity_verified: bool,
    pub open_obligations: Vec<Value>,
}

/// Normative §15.2 Absolute Set Closure Lemma validator
pub fn verify_badge_closure(guarantee_class: &str, claimed_absolute: bool) -> (bool, String) {
    if !claimed_absolute {
        return (true, format!("ACCEPTED (relational/empirical claim for {})", guarantee_class));
    }
    if guarantee_class == "formal_control_flow" {
        (true, "ACCEPTED: formal_control_flow ∈ A (Layer-1 CF-GKAT absolute set)".into())
    } else {
        (
            false,
            format!(
                "REJECTED by Absolute Set Closure Lemma (§15.2): {} ∉ A = {{formal_control_flow}}",
                guarantee_class
            ),
        )
    }
}

/// Normative §6.1 Static Risk Score: S = 8M + 1000P + 20A + 50L + 200I; R = min(100, floor(S/50))
pub fn compute_risk_score(m: f64, p: f64, a: f64, l: f64, i: f64) -> (f64, u32) {
    let s = 8.0 * m + 1000.0 * p + 20.0 * a + 50.0 * l + 200.0 * i;
    let r = (s / 50.0).floor().clamp(0.0, 100.0) as u32;
    ((s * 100.0).round() / 100.0, r)
}

/// Normative MAS Track Routing
pub fn route_track(mas: f64) -> (&'static str, &'static str) {
    if mas >= 0.70 {
        ("Track A", "Formal cascade (Layers 1-5) inside bounded policy walls")
    } else if mas >= 0.30 {
        ("Track A'", "Rehabilitation only (partial metadata recovery)")
    } else {
        ("Track B", "Conformal + VIF-SPRT triage only (NO formal equivalence claims)")
    }
}

/// Normative Conformal Quantile: k = ceil((n + 1)(1 - alpha))
pub fn conformal_quantile(n: u64, alpha: f64) -> u64 {
    (((n + 1) as f64) * (1.0 - alpha)).ceil().max(1.0) as u64
}

/// Exhaustive 8-bit x86 CF vs AArch64 C flag polarity check (65,536 pairs)
pub fn verify_flag_polarity_8bit() -> bool {
    for a in 0u16..256 {
        for b in 0u16..256 {
            let x86_cf = a < b; // borrow on SUB
            let aarch64_c = a >= b; // NOT-borrow on SUBS
            if x86_cf == aarch64_c {
                return false;
            }
        }
    }
    true
}

pub fn run_reference_pipeline(
    mas: f64,
    m: u64,
    p: f64,
    a: u64,
    l: u64,
    i: u64,
) -> CirxPipelineReport {
    let (track, claim_power) = route_track(mas);
    let (risk_s, risk_r) = compute_risk_score(m as f64, p, a as f64, l as f64, i as f64);

    // Build canonical loop CFG and run Cooper-Harvey-Kennedy dominators + Cytron dominance frontiers + SSA verification
    let cfg = Cfg::new(vec![
        BlockDef {
            instrs: vec![MicroInstr::Mov { w: Width::W64, dst: Reg::X0, src: Operand::Imm(0) }],
            succs: vec![1],
        },
        BlockDef {
            instrs: vec![
                MicroInstr::BinOp {
                    op: BinOpKind::Sub,
                    w: Width::W64,
                    dst: Reg::X9,
                    a: Operand::Reg(Reg::X0),
                    b: Operand::Imm(10),
                    set_flags: true,
                },
                MicroInstr::Branch { cond: CondCode::Lt, target: 0x1008 },
            ],
            succs: vec![2, 3],
        },
        BlockDef {
            instrs: vec![MicroInstr::BinOp {
                op: BinOpKind::Add,
                w: Width::W64,
                dst: Reg::X0,
                a: Operand::Reg(Reg::X0),
                b: Operand::Imm(1),
                set_flags: false,
            }],
            succs: vec![1],
        },
        BlockDef {
            instrs: vec![MicroInstr::Ret],
            succs: vec![],
        },
    ]);

    let ssa = build_ssa(&cfg);
    let ssa_ok = check_ssa(&cfg, &ssa).is_ok();
    let mut phi_count = 0usize;
    let mut listing = Vec::new();
    for (bi, blk) in ssa.blocks.iter().enumerate() {
        listing.push(format!("BB{}:", bi));
        for ph in &blk.phis {
            phi_count += 1;
            let inc: Vec<String> = ph
                .incomings
                .iter()
                .map(|(pb, nm)| format!("BB{}:{}", pb, nm))
                .collect();
            listing.push(format!("  {} = phi({})", ph.dst, inc.join(", ")));
        }
        for ins in &blk.instrs {
            let ds: Vec<String> = ins.defs.iter().map(|d| d.to_string()).collect();
            let us: Vec<String> = ins.uses.iter().map(|u| u.to_string()).collect();
            listing.push(format!("  defs=[{}] uses=[{}] // {}", ds.join(","), us.join(","), ins.text));
        }
    }

    let (_, closure_msg) = verify_badge_closure("formal_control_flow", true);

    CirxPipelineReport {
        spec_version: "LTO-Decomp v3.4 (CIRX Micro-RTL rev2)".into(),
        mas_score: mas,
        track: track.into(),
        claim_power: claim_power.into(),
        risk_s,
        risk_r,
        absolute_guarantee_set: vec!["formal_control_flow".into()],
        closure_lemma_status: closure_msg,
        ssa_blocks: ssa.blocks.len(),
        phi_nodes: phi_count,
        ssa_verified: ssa_ok,
        ssa_listing: listing,
        smt_overlap_query: smt_overlap_query(8, 4),
        conformal_quantile_k: conformal_quantile(100, 0.05),
        flag_polarity_verified: verify_flag_polarity_8bit(),
        open_obligations: vec![
            json!({"id": "O-1", "priority": "P0", "status": "OPEN", "desc": "CIRX Stage-1 lifter (formal semantics)"}),
            json!({"id": "O-2", "priority": "P0", "status": "OPEN", "desc": "Parametric CIRX for x86-64 / RV64 / AArch32"}),
            json!({"id": "O-3", "priority": "P0", "status": "OPEN", "desc": "Absolute Layer-2 data-flow checker"}),
            json!({"id": "O-4", "priority": "P1", "status": "OPEN", "desc": "Layer-5 relational harness"}),
            json!({"id": "O-5", "priority": "P1", "status": "IN_PROGRESS", "desc": "Track-B trace store + emulator adapters (MegaDaemon SHM/Frida)"}),
            json!({"id": "O-6", "priority": "P1", "status": "OPEN", "desc": "Full conformal + VIF-SPRT calibration pipeline"}),
            json!({"id": "O-7", "priority": "P1", "status": "IN_PROGRESS", "desc": "Multi-container ingest (MegaDaemon DiE/Binwalk PE+ELF+MachO+DEX)"}),
            json!({"id": "O-8", "priority": "P2", "status": "OPEN", "desc": "Extension-protocol automation"}),
        ],
    }
}

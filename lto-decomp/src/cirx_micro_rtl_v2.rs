//! CIRX Stage-1 Micro-RTL, Stage-2 SSA, and SMT-LIB export: reference implementation, revision 2.
//!
//! Scope: an AArch64-subset, single-threaded, concrete interpreter plus a multi-block SSA
//! construction (Cooper-Harvey-Kennedy dominators, Cytron dominance frontiers, versioned
//! registers, four independent flag bits, and a versioned memory variable) and a generator of
//! SMT-LIB 2 overlap queries. It does NOT implement vector, floating-point, atomic, system-register,
//! or pointer-authentication semantics, and it is not the architecture-parametric model of
//! Section 20.2 of the specification.
//!
//! Build and test:  rustc --edition 2021 -O cirx_micro_rtl_v2.rs && ./cirx_micro_rtl_v2

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

// ---------------------------------------------------------------- tests

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
}

fn edge_values(w: Width) -> Vec<u64> {
    let m = w.mask();
    let s = w.sign_bit();
    vec![0, 1, 2, m, m - 1, s, s - 1, s + 1, m >> 1, 0x55 & m, 0xAA & m]
}

fn t_cond_semantics() {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    for w in [Width::W32, Width::W64] {
        // equal operands and boundary values are enumerated exhaustively; random sampling alone
        // almost never produces a == b and let a mutation of the carry flag survive
        let mut pairs: Vec<(u64, u64)> = Vec::new();
        for x in edge_values(w) { for y in edge_values(w) { pairs.push((x, y)); } }
        for _ in 0..20000 { pairs.push((rng.next() & w.mask(), rng.next() & w.mask())); }
        for (a, b) in pairs {
            let (_, f) = Nzcv::from_sub(w, a, b);
            let sg = |x: u64| -> i64 { if w == Width::W32 { x as u32 as i32 as i64 } else { x as i64 } };
            assert_eq!(f.eval(CondCode::Eq), a == b);
            assert_eq!(f.eval(CondCode::Ne), a != b);
            assert_eq!(f.eval(CondCode::Cs), a >= b);
            assert_eq!(f.eval(CondCode::Cc), a < b);
            assert_eq!(f.eval(CondCode::Hi), a > b);
            assert_eq!(f.eval(CondCode::Ls), a <= b);
            assert_eq!(f.eval(CondCode::Ge), sg(a) >= sg(b));
            assert_eq!(f.eval(CondCode::Lt), sg(a) < sg(b));
            assert_eq!(f.eval(CondCode::Gt), sg(a) > sg(b));
            assert_eq!(f.eval(CondCode::Le), sg(a) <= sg(b));
            let (r, fa) = Nzcv::from_add(w, a, b);
            let (sa, sb) = (sg(a) as i128, sg(b) as i128);
            let lim = 1i128 << (w.bits() - 1);
            assert_eq!(fa.v, !(-lim <= sa + sb && sa + sb < lim));
            assert_eq!(fa.c, (a as u128 + b as u128) > w.mask() as u128);
            assert_eq!(r, a.wrapping_add(b) & w.mask());
        }
    }
    println!("[flags] edge-case and randomized SUBS/ADDS checks (W32 and W64) against integer predicates: OK");
}

fn t_memory() {
    let mut m = MemorySpace::new();
    m.add_region(0x7fff_0000, 0x1_0000, Region::Stack);
    m.add_region(0x0040_0000, 0x1000, Region::Global);
    m.store(0x7fff_0100, MemSize::B64, 0xDEAD_BEEF_CAFE_BABE);
    assert_eq!(m.load(0x7fff_0100, MemSize::B64, false), Ok(0xDEAD_BEEF_CAFE_BABE));
    // defect 1 of revision 1: signed 64-bit load panicked in debug builds
    m.store(0x7fff_0200, MemSize::B64, 0xFFFF_FFFF_FFFF_FFFE);
    assert_eq!(m.load(0x7fff_0200, MemSize::B64, true), Ok(0xFFFF_FFFF_FFFF_FFFE));
    m.store(0x7fff_0300, MemSize::B8, 0x80);
    assert_eq!(m.load(0x7fff_0300, MemSize::B8, true), Ok(0xFFFF_FFFF_FFFF_FF80));
    // defect 2: wrap-around overlap
    assert!(m.may_alias(0xFFFF_FFFF_FFFF_FFFE, 4, 0x0, 1));
    assert!(!m.may_alias(0xFFFF_FFFF_FFFF_FFFE, 2, 0x0, 1));
    // regions
    assert!(!m.may_alias(0x7fff_0100, 8, 0x0040_0000, 8));
    assert!(m.may_alias(0x7fff_0100, 8, 0x7fff_0104, 4));
    assert!(m.may_alias(0x7fff_0104, 4, 0x7fff_0100, 8));   // reverse direction (second disjunct)
    assert!(!m.may_alias(0x7fff_0100, 4, 0x7fff_0104, 4));  // adjacent, not overlapping
    assert!(!m.may_alias(0x7fff_0104, 4, 0x7fff_0100, 4));
    assert!(m.may_alias(0x0, 1, 0xFFFF_FFFF_FFFF_FFFE, 4)); // wrap-around, reverse direction
    // defect 3: uninitialized read is an error
    assert_eq!(m.load(0x7fff_5000, MemSize::B32, false), Err(MemError::Uninitialized(0x7fff_5000)));
    println!("[mem] signed B64 load, circular overlap, region separation, uninitialized-read error: OK");
}

fn t_exec() {
    let mut m = Machine::new();
    m.mem.add_region(0x7fff_0000, 0x1_0000, Region::Stack);
    m.set(Reg::Sp, 0x7fff_0100);
    let prog = vec![
        MicroInstr::Mov { w: Width::W64, dst: Reg::X0, src: Operand::Imm(40) },
        MicroInstr::BinOp { op: BinOpKind::Add, w: Width::W64, dst: Reg::X1, a: Operand::Reg(Reg::X0), b: Operand::Imm(2), set_flags: true },
        MicroInstr::Store { src: Reg::X1, base: Reg::Sp, offset: 0, size: MemSize::B64 },
        MicroInstr::Load { dst: Reg::X2, base: Reg::Sp, offset: 0, size: MemSize::B64, signed: false },
        MicroInstr::BinOp { op: BinOpKind::Sub, w: Width::W64, dst: Reg::X3, a: Operand::Reg(Reg::X2), b: Operand::Imm(42), set_flags: true },
        MicroInstr::Branch { cond: CondCode::Eq, target: 0x1000 },
        MicroInstr::Ret,
    ];
    m.run_block(&prog).unwrap();
    assert_eq!((m.get(Reg::X1), m.get(Reg::X3)), (42, 0));
    assert!(m.nzcv.z && m.branch_taken == Some(0x1000));
    // W32 destination write zero-extends
    let mut m2 = Machine::new();
    m2.set(Reg::X5, 0xAAAA_BBBB_FFFF_FFFF);
    m2.step(&MicroInstr::BinOp { op: BinOpKind::Add, w: Width::W32, dst: Reg::X5, a: Operand::Reg(Reg::X5), b: Operand::Imm(1), set_flags: true }).unwrap();
    assert_eq!(m2.get(Reg::X5), 0);
    assert!(m2.nzcv.z && m2.nzcv.c && !m2.nzcv.n && !m2.nzcv.v);
    // call writes the lifter-supplied return address
    let mut m3 = Machine::new();
    m3.step(&MicroInstr::Call { target: Operand::Imm(0x2000), ret_addr: 0x1008, clobbers: vec![Reg::X0] }).unwrap();
    assert_eq!((m3.get(Reg::X30), m3.get(Reg::Pc)), (0x1008, 0x2000));
    println!("[exec] 64-bit program, W32 zero-extension and flags, call with explicit return address: OK");
}

fn diamond() -> Cfg {
    // 0 -> {1,2}; 1 -> 3; 2 -> 3 ; X0 defined in 1 and 2, used in 3
    let def = |v: u64| MicroInstr::Mov { w: Width::W64, dst: Reg::X0, src: Operand::Imm(v) };
    Cfg::new(vec![
        BlockDef { instrs: vec![MicroInstr::BinOp { op: BinOpKind::Sub, w: Width::W64, dst: Reg::X9, a: Operand::Reg(Reg::X8), b: Operand::Imm(0), set_flags: true },
                                MicroInstr::Branch { cond: CondCode::Eq, target: 0 }], succs: vec![1, 2] },
        BlockDef { instrs: vec![def(1)], succs: vec![3] },
        BlockDef { instrs: vec![def(2)], succs: vec![3] },
        BlockDef { instrs: vec![MicroInstr::Mov { w: Width::W64, dst: Reg::X1, src: Operand::Reg(Reg::X0) }, MicroInstr::Ret], succs: vec![] },
    ])
}

fn loop_cfg() -> Cfg {
    // 0 -> 1; 1 -> {2,3}; 2 -> 1 (back edge); 3 exit. X0 incremented in 2.
    Cfg::new(vec![
        BlockDef { instrs: vec![MicroInstr::Mov { w: Width::W64, dst: Reg::X0, src: Operand::Imm(0) }], succs: vec![1] },
        BlockDef { instrs: vec![MicroInstr::BinOp { op: BinOpKind::Sub, w: Width::W64, dst: Reg::X9, a: Operand::Reg(Reg::X0), b: Operand::Imm(10), set_flags: true },
                                MicroInstr::Branch { cond: CondCode::Lt, target: 0 }], succs: vec![2, 3] },
        BlockDef { instrs: vec![MicroInstr::BinOp { op: BinOpKind::Add, w: Width::W64, dst: Reg::X0, a: Operand::Reg(Reg::X0), b: Operand::Imm(1), set_flags: false }], succs: vec![1] },
        BlockDef { instrs: vec![MicroInstr::Ret], succs: vec![] },
    ])
}

fn t_ssa() {
    let d = diamond();
    let df = d.dominance_frontiers();
    assert_eq!(df[&1], BTreeSet::from([3]));
    assert_eq!(df[&2], BTreeSet::from([3]));
    assert!(df[&0].is_empty() && df[&3].is_empty());
    let s = build_ssa(&d);
    check_ssa(&d, &s).unwrap();
    assert!(s.blocks[3].phis.iter().any(|p| p.dst.var == Var::Reg(Reg::X0) && p.incomings.len() == 2));
    assert!(!s.blocks[3].phis.iter().any(|p| p.dst.var == Var::Reg(Reg::X9)));
    let l = loop_cfg();
    let dfl = l.dominance_frontiers();
    assert_eq!(dfl[&2], BTreeSet::from([1]));
    assert_eq!(dfl[&1], BTreeSet::from([1]));
    let sl = build_ssa(&l);
    check_ssa(&l, &sl).unwrap();
    assert!(sl.blocks[1].phis.iter().any(|p| p.dst.var == Var::Reg(Reg::X0)));
    // straight-line: a call renames its clobbers (defect 4 of revision 1: the call was dropped)
    let sl2 = Cfg::new(vec![BlockDef { instrs: vec![
        MicroInstr::Mov { w: Width::W64, dst: Reg::X0, src: Operand::Imm(1) },
        MicroInstr::Call { target: Operand::Imm(0x2000), ret_addr: 0x10, clobbers: vec![Reg::X0] },
        MicroInstr::Mov { w: Width::W64, dst: Reg::X1, src: Operand::Reg(Reg::X0) },
    ], succs: vec![] }]);
    let s2 = build_ssa(&sl2);
    check_ssa(&sl2, &s2).unwrap();
    let mov_after = &s2.blocks[0].instrs[2];
    assert_eq!(mov_after.uses[0], SsaName { var: Var::Reg(Reg::X0), version: 2 }); // the call's X0, not the Mov's
    // flags are versioned independently of registers
    let c = &s2.blocks[0].instrs[1];
    assert!(c.defs.contains(&SsaName { var: Var::Flag(FlagBit::C), version: 1 }));
    println!("[ssa] diamond and loop dominance frontiers, minimal phi placement, call clobber renaming, flag versions, dominance-checked: OK");
    println!("[ssa] diamond block 3 phis: {:?}", s.blocks[3].phis.iter().map(|p| format!("{} = phi{:?}", p.dst, p.incomings.iter().map(|(b, n)| format!("B{}:{}", b, n)).collect::<Vec<_>>())).collect::<Vec<_>>());
}

fn t_smt() {
    let q = smt_overlap_query(8, 4);
    assert!(q.contains("(bvult (bvsub b a) (_ bv8 64))"));
    println!("[smt] overlap query generated:\n{}", q);
}

fn main() {
    println!("=== CIRX Micro-RTL reference implementation, revision 2 ===");
    t_cond_semantics();
    t_memory();
    t_exec();
    t_ssa();
    t_smt();
    println!("=== ALL CIRX TESTS PASSED ===");
}

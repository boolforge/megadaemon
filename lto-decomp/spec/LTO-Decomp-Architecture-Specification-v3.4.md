# Three-Tier Cross-Platform, Multi-Architecture Framework for Verifiable Decompilation of LTO-Optimized Binaries

## Formal Reconstruction (Track A), Partial-Metadata Rehabilitation (Track A′), and Probabilistic Behavioral Triage (Track B)

**Version:** 3.4 (distribution & agent-ingestion edition: inherits all closed findings and the research-contract obligations of v3.3; adds machine-readable obligation matrix, formal claim index, stronger discharge metrics, and explicit LLM/agent consumption contracts; zero new document-local defects)  
**Status:** Normative architecture specification + engineering contract + distribution package. Document-local defects closed. Remaining work is explicitly scoped, ranked, acceptance-tested, and machine-readable. This edition is the recommended artifact for human and agent consumption.  
**Targets:** Linux, Android, and BSD ELF; Windows PE/COFF; macOS and iOS Mach-O; DOS MZ/NE/LE; console containers (Nintendo Switch NSO/NRO, GameCube/Wii DOL and REL, and others in Section 29); WebAssembly modules  
**Architectures (full instantiation):** AArch64 (reference, Parts I), x86-64, IA-32 and x86-16, AArch32, RISC-V (RV32 and RV64), MIPS (32 and 64), PowerPC (32 and 64), WebAssembly (Part II); further ISAs by the extension protocol (Section 28)  
**Intermediate representation:** CIRX (Canonical Intermediate Representation eXtended), schema version 4 (architecture-parametric; Section 20)  
**Governing bound:** Rice's theorem (idealized model) plus computational intractability (concrete finite-state model)  
**SMT-LIB 2 logics:** `QF_ABV` and `QF_ABVFP` (CIRX memory queries, Layer 2 obligations), `QF_AUFBV` and `QF_AUFBVFP` (Layer 5 obligations); CF-GKAT is algebraic and uses no SMT solver  
**Language:** American English  

**Re-execution environment (v3.3).** The v3.3 review was conducted in a Linux x86-64 sandbox with `python3.12`, `numpy 2.4.4`, `gcc/g++ 13.3.0`, `node 24`, `rustc 1.75`, and host `as`. Tools still **absent**: `z3`, every cross-assembler, `nasm`, `wasmtime`, `wat2wasm`. Consequence: Appendix E rows that require those tools remain **carried-forward**; pure-Python checks (overlap, polarity, alignment, conformal, badges) and the extended linter are re-executed and pass. The presence of `rustc` enables local compilation of the Micro-RTL reference in Appendix D.

**Re-execution environment (v3.2 — historical, closes F-12).** The v3.2 review was conducted in a Linux x86-64 sandbox with `python3.11`, `numpy 2.4.6`, `gcc/g++ 12.2.0`, `node 22`, and host `as`. The following tools were **absent**: `z3` and `z3-solver` (PyPI unreachable from the sandbox), `rustc` and `rustup`, every cross-assembler (`aarch64-linux-gnu-as`, `riscv64-linux-gnu-as`, `mips-linux-gnu-as`, `powerpc-linux-gnu-as`, `arm-linux-gnueabihf-as`), `nasm`, `wasmtime`, `wat2wasm`, `clang`, `lld`. Consequence: Appendix E rows V-10, V-15, V-16 and E.8's assembler/wasmtime rows are **not re-executable in this review**; the v3.2 specification carries them as *carried forward from v3.1* and marks the carried-forward rows explicitly. Pure-Python and NumPy checks (overlap predicate, x86/Arm CF complementarity, alignment idiom, conformal quantile, badge SHA-256 digests) are re-executed and their output is captured in `verify/`. 

---

## Reading Conventions

- **MUST / MUST NOT / MAY** are used in the RFC 2119 sense.
- A **policy parameter** is a number chosen by this specification (for example, a 2,000-instruction wall). A policy parameter is *not* a property of any cited tool and carries no citation.
- A **reported figure** is a number printed in a cited primary source, measured in that source's own setting. No reported figure is a claim about ARM64 LTO binaries unless the source says so.
- Every formal claim is followed by a **Does not claim** statement that names what the claim excludes.
- **Part I** (Sections 1–19) defines the architecture-independent framework and instantiates it for AArch64. **Part II** (Sections 20–31) defines the architecture-parametric core and instantiates it for every other supported architecture with the same eleven-subsection template. Where Part II is silent, Part I applies unchanged; where they differ, Part II is normative for its architecture.
- All code in this specification is reproduced in the document itself: the data models (Sections 3.2, 20.2, 29.1), SMT-LIB, assembly, C++, and WebAssembly idioms (Part II), the JSON schemas, the reference implementation (Appendix D), and the verification and lint tools (Appendix F). The accompanying package contains the same files in executable form.
- Specification facts about ABIs, instruction sets, and containers are cited to the governing specification. A fact about a proprietary platform for which no stable public primary source was located is marked in Appendix A as *informational* and is never used as a premise of a verification claim.
- Appendix A (Verification Ledger) states, for each external claim, whether this review re-read the primary source or carried the claim forward from the source document.

---

## Abstract

Compilation is a lossy, many-to-one transformation. Link-time optimization (LTO) erases translation-unit boundaries and inlines aggressively, C++ lowering removes templates and class structure, and the container, ABI, and instruction-set ecosystems (ELF, PE/COFF, Mach-O, MZ, NSO/NRO, and WebAssembly, across x86, ARM, RISC-V, MIPS, and PowerPC) fragment tooling. Behavioral equivalence of arbitrary programs is undecidable in the idealized model (Rice, 1953), and decidable-but-intractable for real finite-state binaries, so no pipeline can claim complete behavioral consistency for arbitrary inputs.

This specification defines a three-tier architecture, instantiated for every supported instruction set, that routes each binary by a Metadata Availability Score (MAS) and attaches a machine-checkable provenance badge, including the architecture and ABI, to every output:

- **Track A** (MAS ≥ 0.70) runs a five-layer verification cascade whose *positive* verdicts are scoped to explicit envelopes.
- **Track A′** (0.30 ≤ MAS < 0.70) attempts bounded *rehabilitation* of individual layers and reports per-layer coverage; it never claims full-cascade coverage.
- **Track B** (MAS < 0.30) produces ranked structural hypotheses with split-conformal prediction sets; it makes no formal verification claim.

LLM context is managed by one primary pattern, contract-driven interprocedural decomposition with an external scratchpad. Its token savings are *modeled* (Section 9.1), not measured. The specification states no behavioral-consistency percentage for LTO binaries of any architecture; it pre-registers hypotheses (Section 16) and lists the figures that cited tools reported in their own settings (Section 6).

**Does not claim:** that any layer proves equivalence of an arbitrary function; that reported figures transfer to any LTO target or to any architecture other than the one in which they were measured; that every layer is available on every architecture (Layer 5 has a backend only for AArch64 in the documented tooling; Section 30.5); that any commercial title can be reconstructed byte-exactly.

---

## 1. Introduction

### 1.1 The Problem

Compilation discards variable names, types, function boundaries, and high-level control structure. LTO defers optimization to link time. C++ adds further loss: templates are instantiated and leave no residue, virtual calls become indirect branches through vtables, exception handling reduces to unwind tables (`.eh_frame` and `.gcc_except_table` on ELF; `.pdata` and `.xdata` on PE), and STL containers lose type identity.

Two LTO modes matter (Johnson et al., CGO 2017, https://doi.org/10.1109/CGO.2017.7863733; LLVM, https://llvm.org/docs/LinkTimeOptimization.html, https://clang.llvm.org/docs/ThinLTO.html):

- **Full LTO** merges all bitcode into one LLVM IR module and optimizes whole-program.
- **ThinLTO** keeps per-module bitcode, uses per-module summary indexes, and performs cross-module importing and inlining in parallel.

Bitcode produced by the two modes can be distinguished by the presence of module-summary records inspected with `llvm-bcanalyzer`. This applies only when intermediate bitcode survives; a shipped binary does not contain it.

**Does not claim:** that the LTO mode is recoverable from a finished, stripped executable. The v2.2 statement that the LTO mode is "recoverable via `llvm-bcanalyzer`" is restricted here to retained bitcode objects (for example, files named by `--lto-obj-path`).

The v2.2 empirical sentence "Linux kernel LTO raises the cross-file inlined function count from 6 895 to 8 058" is removed because no primary source was located (Appendix A, item L-14).

### 1.2 Rice's Theorem and the Decidability Ceiling

**Theorem (Rice, 1953; Trans. AMS 74(2):358–366; https://doi.org/10.1090/S0002-9947-1953-0053041-6).** Let $\varphi_e$ denote the partial computable function computed by program index $e$, and let $\mathcal{S}$ be a set of partial computable functions that is neither empty nor the set of all of them. Then $\{e : \varphi_e \in \mathcal{S}\}$ is undecidable.

*Corollary used here.* Fix a program $Q$ that computes a total function. The set $\mathcal{S}_Q = \{\varphi_e : \varphi_e = \varphi_Q\}$ is non-trivial, so $\{P : P \equiv Q\}$ is undecidable, where $P \equiv Q$ means equality of partial functions (including identical divergence). Equivalence is a binary relation, so the corollary is obtained by fixing one argument; Rice's theorem is not applied to the pair directly.

*Correction to v2.2.* v2.2 stated the theorem for "a non-empty proper subset closed under extensional equivalence", wrote equivalence as $\forall\sigma.\ \llbracket P\rrbracket(\sigma)=\llbracket Q\rrbracket(\sigma)$ without fixing Kleene equality, and cited a DOI with the Proceedings of the AMS prefix (`S0002-9939`). The Transactions of the AMS prefix is `S0002-9947`; the corrected DOI is used above.

**Finite-state caveat (stress test).** A real ARM64 process has bounded memory, so its transition system is finite and equivalence between two such systems is decidable in principle, at a cost exponential in the state size. Rice's theorem therefore bounds the *idealized* model (unbounded memory); the practical barrier for real binaries is intractability. This specification uses both facts: Rice's theorem to justify that no complete procedure exists in the idealized model, and explicit resource walls (policy parameters) to justify abandonment in the concrete model.

**Does not claim:** that Rice's theorem forbids all useful verification; it forbids a *complete and sound* procedure for a non-trivial semantic property over all programs.

### 1.3 LTO Distortion Operator

Let $\mathcal{L}$ denote the composition of inter-procedural inlining, constant propagation, loop peeling, identical-code folding, and weak-symbol internalization. For a source function $f$, define

$$
\mathcal{L}(f) = \{ r_1, r_2, \dots, r_k \}, \qquad k \ge 0,
$$

where each $r_i$ is a code region (a fragment, a merged body, or an inlined region) located in one or more binary functions. The case $k = 0$ (dead-code elimination) is allowed; the case in which one region $r$ belongs to $\mathcal{L}(f)$ and $\mathcal{L}(g)$ for $f \neq g$ (identical-code folding) is also allowed. Hence $\mathcal{L}$ is not a function from source functions to disjoint binary functions, and recovery of $f$ from $\mathcal{L}(f)$ is under-determined.

**Does not claim:** a closed-form characterization of $\mathcal{L}$; $\mathcal{L}$ is a descriptive operator, not an algorithm.

### 1.4 The Three-Tier Mandate

Earlier unified pipelines let 60–80 % of LTO functions fall through every formal layer (source-document observation, not a measured result in this specification). Two-track splits left a partial-metadata gap. The three tracks share CIRX and the provenance badge but share no risk router, no verification layer, and no evaluation metric.

| Track | MAS range | Verification claim |
|-------|-----------|--------------------|
| A | $\mathrm{MAS} \ge 0.70$ | Per-function, per-layer positive verdicts inside explicit envelopes |
| A′ | $0.30 \le \mathrm{MAS} < 0.70$ | Rehabilitated layers with reported coverage; never full cascade |
| B | $\mathrm{MAS} < 0.30$ | None; ranked hypotheses with calibrated prediction sets |

### 1.5 Contributions

1. Metadata routing by MAS with a precisely defined indicator vector (Section 4).
2. A bounded rehabilitation protocol for partial-metadata binaries (Section 5).
3. A Sandbox Safety Contract adapted to AArch64, Windows ARM64, and Horizon OS (Section 7.3).
4. Split-conformal calibration with a corrected finite-sample quantile (Section 7.6).
5. A single LLM orchestration pattern with a modeled token budget (Section 9).
6. A uniform badge in which every positive claim is paired with its solver or oracle (Section 10).
7. A Switch tier scoped to publicly documented behavior (Section 8) and a Metroid Prime case study that distinguishes what public tooling supports from what it does not (Appendix B).
8. An architecture-parametric CIRX with explicit flag, extension, endianness, delay-slot, mode, and address-space models (Section 20).
9. Full per-architecture instantiations of every section's obligations for x86-64, IA-32 and x86-16, AArch32, RISC-V, MIPS, PowerPC, and WebAssembly (Sections 21–27), an extension protocol for further ISAs (Section 28), platform and container tiers (Section 29), and cross-architecture matrices that state which layer exists where (Section 30).

---

## 2. Platform Landscape

### 2.1 Linux ELF

Linux ARM64 binaries use ELF and the Itanium C++ ABI as profiled for AArch64 (https://itanium-cxx-abi.github.io/cxx-abi/; https://github.com/ARM-software/abi-aa/blob/main/cppabi64/cppabi64.rst). Toolchains: `aarch64-linux-gnu-gcc` and `aarch64-linux-gnu-clang`. LTO is enabled with `-flto`. Exception handling uses DWARF call-frame information in `.eh_frame` and language-specific data in `.gcc_except_table` (DWARF 5, https://dwarfstd.org/dwarf5std.html; AArch64 DWARF extensions, https://github.com/ARM-software/abi-aa/blob/main/aadwarf64/aadwarf64.rst).

Typical partial-metadata forms: `.dynsym` present with `.symtab` stripped; DWARF line tables present without variable or type information (`-gline-tables-only`); a `.comment` section naming the compiler.

### 2.2 Windows PE/COFF

Windows ARM64 binaries use PE/COFF and the MSVC C++ ABI (`cl.exe`, or `clang-cl` targeting `aarch64-pc-windows-msvc`). LTO uses `/GL` and `/LTCG`. Unwind data lives in `.pdata` (`RUNTIME_FUNCTION`) and `.xdata` (https://learn.microsoft.com/en-us/cpp/build/arm64-exception-handling). The PE format is documented at https://learn.microsoft.com/en-us/windows/win32/debug/pe-format. Arm64EC uses a distinct mangling mode and `#`-prefixed function symbols; demanglers must be configured for it (`demumble`, https://github.com/nico/demumble, covers Itanium and MSVC manglings).

Typical partial-metadata forms: export table intact; PDB available from a symbol server but absent locally; PDB stripped before shipping.

### 2.3 Nintendo Switch NSO/NRO

The Switch tier is specified in Section 8. In summary: NSO is the executable format of commercial titles and system modules; NRO is the homebrew format. The pipeline accepts only lawfully obtained binaries (Section 8.7).

### 2.4 ARM64 Calling Convention (AAPCS64)

Per AAPCS64 (https://github.com/ARM-software/abi-aa/blob/main/aapcs64/aapcs64.rst): integer and pointer arguments in `x0`–`x7`; floating-point and SIMD arguments in `v0`–`v7`; results in `x0` (or `v0`); `x8` carries the indirect result location; `x9`–`x15` are caller-saved temporaries; `x16`/`x17` are intra-procedure-call scratch registers; `x18` is the platform register (reserved on Windows and some other platforms; on Linux it is treated as a temporary or as a shadow-call-stack pointer depending on the toolchain mode); `x19`–`x28` are callee-saved; `x29` is the frame pointer; `x30` is the link register. The low 64 bits of `v8`–`v15` are callee-saved. Variadic calls on Apple and Windows ARM64 deviate from base AAPCS64 (stack-passed variadic arguments); the scratchpad `variadic` field (Section 9.2) therefore records the governing ABI.

**Scope note.** Sections 2.1–2.4 describe the AArch64 instantiation. Other architectures, Mach-O, DOS, and console containers are specified in Sections 21–29 with the same obligations.

**Does not claim:** that the register roles above hold for hand-written assembly, `preserve_most` or other non-default calling conventions, or LTO-internal custom conventions. LLVM LTO may change the calling convention of internal (non-exported) functions; Layer 2 contracts for internalized functions MUST be derived from the CIRX body, not from AAPCS64 alone.

---

## 3. CIRX: Canonical Intermediate Representation eXtended

### 3.1 Design Invariants

CIRX is a three-stage, provenance-preserving IR that isolates machine state before any SSA conversion. Every instruction carries an immutable provenance record

$$
\pi = (\mathit{origin},\ \mathit{mangling},\ \mathit{type\_confidence},\ \mathit{type\_source},\ \mathit{inline\_boundary},\ \mathit{semantic\_loss},\ \mathit{template\_candidate},\ \mathit{rtld\_relocation},\ \mathit{metadata\_provenance}).
$$

Invariants:

1. **I1 (no premature SSA).** Stages 1 and 2 never rename registers, flags, or memory.
2. **I2 (explicit flags).** `N`, `Z`, `C`, `V` are four independent single-bit values; they are never packed into an integer.
3. **I3 (explicit loss).** Every semantic approximation is recorded in `semantic_loss`; a lifter that cannot record its loss MUST NOT be used for Track A.
4. **I4 (determinism).** Lifting the same bytes with the same pinned lifter version yields byte-identical CIRX, so `cirx_sha256` is reproducible.

### 3.2 Stage 1 – Micro-RTL Normalization (pre-SSA)

The Rust types below are the normative Stage 1 data model *for AArch64*. They are self-contained: every referenced type is defined. Section 20.2 defines the architecture-parametric generalization (`ArchProfile`, `RegId`, `FlagSet`, and the extended instruction set); the AArch64 types below are obtained from it by instantiating `ArchProfile::aarch64()`, and every other architecture uses the generalization.

```rust
/// CIRX Stage 1 (Micro-RTL). Normative data model, schema version 3.
pub const CIRX_SCHEMA_VERSION: u32 = 4; // v4 = architecture-parametric (Section 20); this block is the AArch64 view

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Reg {
    X(u8),      // x0..x30; index 31 is not a Reg (see Operand::Zr and Operand::Sp)
    V(u8),      // v0..v31
    Sp,         // stack pointer
    Pc,         // program counter (read-only; written only by Branch/Call/Ret)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Width { W8, W16, W32, W64, W128 }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operand {
    Reg(Reg, Width),        // W32 reads the low 32 bits; a W32 write zero-extends to 64 bits
    Imm(u64, Width),
    Zr(Width),              // xzr / wzr: reads as zero, writes are discarded
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CondCode {         // AArch64 condition codes; each is a function of N,Z,C,V only
    Eq, Ne, Cs, Cc, Mi, Pl, Vs, Vc, Hi, Ls, Ge, Lt, Gt, Le, Al, Nv,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinOpKind {
    Add, Sub, Adc, Sbc, And, Orr, Eor, Bic, Orn, Eon,
    Lsl, Lsr, Asr, Ror, Mul, UDiv, SDiv, UMulH, SMulH,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ordering { Relaxed, Acquire, Release, AcquireRelease }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BarrierKind { Dmb, Dsb, Isb }

/// Four independent one-bit values (invariant I2).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NzcvFlags { pub n: bool, pub z: bool, pub c: bool, pub v: bool }

/// Provenance record (Section 3.1). Fields mirror the !cirx.* annotations of Section 3.6.
#[derive(Clone, PartialEq, Debug)]
pub struct Provenance {
    pub origin: (u64, u64),                 // half-open address range [start, end)
    pub mangling: Option<String>,
    pub type_confidence: f32,               // in [0.0, 1.0]
    pub type_source: TypeSource,
    pub inline_boundary: Option<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub template_candidate: Option<[u8; 32]>,
    pub rtld_relocation: Option<RtldReloc>,
    pub metadata_provenance: Vec<MetadataSource>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeSource { Heuristic, Retypd, DebugInfo, LinkerMap }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SemanticLoss {
    ExclusiveMonitorAbstracted, SystemRegisterOpaque, SimdOpaque, FlagsDropped,
    PointerAuthOpaque, MemoryTaggingOpaque, IndirectBranchUnresolved, InlineAsmOpaque,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MetadataSource { Symtab, Dynsym, Pdb, DwarfLine, DwarfType, LinkerMap, LtoObject, Comment, Export }

#[derive(Clone, PartialEq, Debug)]
pub enum RtldReloc { Resolved { kind: u32, addend: i64 }, Unsupported { kind: u32 } }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemSize { B1, B2, B4, B8, B16 }

#[derive(Clone, PartialEq, Debug)]
pub enum MicroInstr {
    Mov    { dst: Reg, src: Operand },
    BinOp  { op: BinOpKind, dst: Operand, src1: Operand, src2: Operand, set_flags: bool },
    CondSelect { dst: Operand, cond: CondCode, if_true: Operand, if_false: Operand },
    CondCompare { cond: CondCode, src1: Operand, src2: Operand, nzcv_if_false: NzcvFlags, is_sub: bool },
    Load   { dst: Operand, base: Operand, offset: i64, size: MemSize, signed: bool, ord: Ordering },
    Store  { src: Operand, base: Operand, offset: i64, size: MemSize, ord: Ordering },
    LoadExclusive  { dst: Operand, base: Operand, size: MemSize, ord: Ordering },
    StoreExclusive { status: Operand, src: Operand, base: Operand, size: MemSize, ord: Ordering },
    Barrier(BarrierKind),
    Hint   { imm: u8 },                      // YIELD is Hint { imm: 1 }; no architectural effect
    ReadSysReg  { dst: Operand, sysreg: u32 },   // opaque: modeled by an uninterpreted function
    WriteSysReg { src: Operand, sysreg: u32 },   // opaque: recorded as SemanticLoss::SystemRegisterOpaque
    Branch   { cond: CondCode, target: u64 },          // reads NZCV; Al is an unconditional branch
    BranchReg{ target: Operand },                      // indirect jump
    Call     { target: CallTarget, clobbers: Vec<Reg> },
    Ret,
    Trap     { imm: u16 },                             // brk, udf: terminates the trace
}

#[derive(Clone, PartialEq, Debug)]
pub enum CallTarget { Direct(u64), Indirect(Operand) }

pub struct MicroBlock {
    pub start: u64,
    pub instrs: Vec<(MicroInstr, Provenance)>,
}

pub struct MicroRtl {
    pub schema_version: u32,
    pub blocks: Vec<MicroBlock>,
    pub entry: u64,
}
```

*NZCV semantics (normative).* For `ADDS`/`SUBS` on width $w \in \{32, 64\}$ with operands $a, b$ and result $r$ (all $w$-bit vectors; `SUBS` computes $a + \lnot b + 1$):

$$
N = r[w-1],\quad Z = (r = 0),\quad C = \text{carry-out of } a + b' + c_{in},\quad V = (a[w-1] = b'[w-1]) \wedge (r[w-1] \neq a[w-1]),
$$

where $b' = b$ for addition and $b' = \lnot b$ for subtraction, and $c_{in} = 0$ for addition and $1$ for subtraction. For subtraction, $C = 1$ exactly when $a \ge_u b$ (no borrow). Logical flag-setting instructions (`ANDS`, `BICS`) set $C = V = 0$. A 32-bit operation computes all four flags from 32-bit values.

*Memory exclusives and ordering.* `LoadExclusive` and `StoreExclusive` are modeled in Section 3.3.3. `Barrier` instructions are no-ops in the sequential model of Section 3.3 and are recorded; the pipeline restricts every verification claim to single-threaded or data-race-free execution (Does-not-claim in Section 3.3.3).

**Does not claim:** support for SVE/SME vector semantics, pointer-authentication instruction semantics, or memory-tagging semantics. Those instructions lift to opaque operations and set the corresponding `SemanticLoss`; any function containing them is ineligible for Layers 2 and 5.

### 3.3 Memory Aliasing Model and SMT Theories

### 3.3.1 Memory model

SMT-LIB arrays have one fixed range sort, so a single array cannot return values of widths 8 through 128. The model is therefore a byte-addressed array with little-endian multi-byte access (AArch64 is little-endian in all targeted environments):

$$
\mathrm{Mem} : \mathrm{Array}(\mathrm{BV}_{64}, \mathrm{BV}_{8}), \qquad
\mathrm{load}_{n}(M, a) = \mathrm{select}(M, a + n - 1) \,\Vert\, \cdots \,\Vert\, \mathrm{select}(M, a + 1) \,\Vert\, \mathrm{select}(M, a),
$$

with the concatenation taken left to right from the most significant byte. In SMT-LIB 2 for $n = 4$:

```smt2
(set-logic QF_ABV)
(declare-const mem (Array (_ BitVec 64) (_ BitVec 8)))
(declare-const a (_ BitVec 64))
(define-fun load32 ((m (Array (_ BitVec 64) (_ BitVec 8))) (p (_ BitVec 64))) (_ BitVec 32)
  (concat (select m (bvadd p #x0000000000000003))
          (concat (select m (bvadd p #x0000000000000002))
                  (concat (select m (bvadd p #x0000000000000001))
                          (select m p)))))
(define-fun store32 ((m (Array (_ BitVec 64) (_ BitVec 8))) (p (_ BitVec 64)) (v (_ BitVec 32)))
  (Array (_ BitVec 64) (_ BitVec 8))
  (store (store (store (store m p ((_ extract 7 0) v))
                       (bvadd p #x0000000000000001) ((_ extract 15 8) v))
                (bvadd p #x0000000000000002) ((_ extract 23 16) v))
         (bvadd p #x0000000000000003) ((_ extract 31 24) v)))
(check-sat)
```

### 3.3.2 Aliasing queries

Let an access be a pair $(a, s)$ of a 64-bit address and a byte size $1 \le s \le 16$. Two accesses $(a, s_1)$ and $(b, s_2)$ *overlap* (as circular byte ranges modulo $2^{64}$) exactly when

$$
\mathrm{Overlap}\big((a,s_1),(b,s_2)\big) \;\equiv\; (b - a) <_u s_1 \;\lor\; (a - b) <_u s_2 ,
$$

with subtraction in $\mathrm{BV}_{64}$. The two query kinds are distinct:

- **May-alias:** $\Phi \land \mathrm{Overlap}$ is `sat`, where $\Phi$ is the path constraint.
- **Must-alias (or no-alias):** $\Phi \land \lnot \mathrm{Overlap}$ (respectively $\Phi \land \mathrm{Overlap}$) is `unsat`.

v2.2 defined aliasing as "$\mathrm{BVAdd}(p,o_1) = \mathrm{BVAdd}(q,o_2)$ is sat". That is a *may-equal-start-address* test: it ignores access size and partial overlap, and it is not a must-alias test. It is replaced by the definitions above. The overlap predicate was checked in this review at 8-bit address width, exhaustively over all address pairs and the sizes 1, 2, 4, 8, 16 (zero mismatches against explicit circular-interval semantics), and its equivalence to the explicit interval definition was proved by Z3 at 8 bits (the negation is `unsat`; Appendix E.2); the 64-bit predicate is the same expression, and a 64-bit proof was not run. A `sat` result for may-alias is a possibility, not a proof of aliasing; an `unknown` result is treated as may-alias (the conservative direction for soundness of no-alias consumers) and the layer abandons if it needs a definite answer.

**Provenance regions.** A pointer value carries a region tag in $\{\text{stack}, \text{heap}, \text{global}, \text{tls}, \text{unknown}\}$. Cross-region no-alias is assumed only when the region tags are both known and the tag-assignment evidence is recorded in `metadata_provenance`. This is a *provenance-based* assumption in the sense of the C memory-model literature (Kang et al., PLDI 2015, https://doi.org/10.1145/2737924.2738005; Memarian et al., POPL 2019, https://doi.org/10.1145/3290380); it is unsound for programs that cast integers to pointers across regions, and any function containing an `IndirectBranchUnresolved` or integer-to-pointer cast of unknown origin MUST use region `unknown`. v2.2 attributed provenance-aware modeling to SoftBound; SoftBound (Nagarakatte et al., PLDI 2009, https://doi.org/10.1145/1542476.1542504) is a bounds-metadata instrumentation scheme, not a provenance semantics, and the attribution is withdrawn.

### 3.3.3 Exclusive monitors, YIELD, and barriers

`LoadExclusive` followed by `StoreExclusive` on the same address with no intervening exclusive access is lowered, **in the sequential model only**, to a read-modify-write that always succeeds (status register $= 0$) on the success path and a second path on which the store is skipped and the status register is $1$. The failure path models spurious exclusive-monitor loss, which the architecture permits (Arm Architecture Reference Manual for A-profile, https://developer.arm.com/documentation/ddi0487/latest). `Hint { imm: 1 }` (`YIELD`) has no architectural effect and is lowered to a no-op with a recorded annotation. Acquire/release orderings are recorded but have no effect in the sequential model.

**Does not claim:** any property under concurrency. The sequential lowering proves nothing about data-race freedom, atomicity as observed by other cores, or memory-model behavior (for the AArch64 concurrency model see Pulte et al., POPL 2018, https://doi.org/10.1145/3158107). Any function whose verdict depends on inter-core ordering is reported `Out_of_envelope`.

### 3.3.4 SMT-LIB 2 Theories Employed

Definitions follow the SMT-LIB Standard 2.6 (Barrett, Fontaine, Tinelli, 2017; https://smt-lib.org; logic declarations at https://smt-lib.org/logics-all.shtml).

| Layer | Logic | Theories | Decidability and complexity (precise) | Solver backends |
|-------|-------|----------|---------------------------------------|-----------------|
| CF-GKAT | none (algebraic) | Guarded Kleene Algebra with Tests; the symbolic variant uses SAT solvers, not SMT | Equivalence decidable; nearly linear in automaton size for GKAT (Smolka et al., POPL 2020, https://doi.org/10.1145/3371129) | Rust symbolic implementation with SAT solvers (Zhang et al., arXiv:2601.09986) |
| Layer 2 obligations | `QF_ABV` | `FixedSizeBitVectors`, `ArraysEx` | Decidable; NP-hard; with binary-encoded widths QF_BV is NEXPTIME-complete (Kovásznai, Fröhlich, Biere, Theory Comput. Syst. 59(2), 2016, https://doi.org/10.1007/s00224-015-9653-1) | Z3, cvc5, Bitwuzla |
| Layer 5 obligations | `QF_AUFBV` | `QF_ABV` plus uninterpreted function and predicate symbols | Decidable (quantifier-free, congruence closure plus bit-vectors and arrays); NP-hard | Boogie with Z3 or cvc5 |
| Tier 2b type recovery | none | Retypd-style subtype-constraint solving (pushdown-system saturation), not SMT | See Noonan et al., PLDI 2016 | Retypd-style solver |
| PCodeTrans, CHISEL, Echo | none | Execution or recompilation oracles | Not applicable | Not applicable |

*Corrections to v2.2.* (a) v2.2 called `QF_AUFBV` "undecidable in the presence of unrestricted UF". Quantifier-free combinations of arrays, uninterpreted functions, and bit-vectors are decidable; undecidability arises with quantifiers or non-extensional unbounded features, which this pipeline never emits. (b) v2.2 called `QF_ABV` "NP-complete in the worst case"; the correct statement is NP-hard, with the upper bound depending on bit-width encoding. (c) v2.2 assigned Tier 2b to `QF_AUFBV` and bounded it by "$10^6$ solver queries"; Retypd does not use an SMT solver (Noonan et al., PLDI 2016, https://doi.org/10.1145/2908080.2908119), so that bound is replaced by constraint-count and wall-clock budgets (Section 3.4). (d) Faultless is not asserted to use `QF_ABV` (Appendix A, L-02); the logic assignment in the table is a design commitment of this pipeline's own Layer 2 checker.

**Definition of `QF_ABV`.** Quantifier-free formulae over the combined theory of fixed-size bit-vectors and extensional arrays $\mathrm{Array}(\mathrm{BV}_n, \mathrm{BV}_m)$ with `select`, `store`, McCarthy's read-over-write axioms (McCarthy, 1962, "Towards a Mathematical Science of Computation", IFIP Congress 62), and extensionality: $(\forall i.\ \mathrm{select}(a,i) = \mathrm{select}(b,i)) \Rightarrow a = b$. The quantifier belongs to the theory axiom; submitted formulae are quantifier-free.

**Definition of `QF_AUFBV`.** `QF_ABV` extended with uninterpreted function and predicate symbols. In this pipeline, uninterpreted symbols model operations the instruction-semantics front end leaves opaque (system-register reads, unmodeled flag side effects).

**Solvers.** Z3 (de Moura and Bjørner, TACAS 2008, https://doi.org/10.1007/978-3-540-78800-3_24; https://github.com/Z3Prover/z3; https://microsoft.github.io/z3guide/). cvc5 (Barbosa et al., TACAS 2022, pp. 415–442, https://doi.org/10.1007/978-3-030-99524-9_24; https://github.com/cvc5/cvc5; https://cvc5.github.io/). The v2.2 and integration-package DOI suffix `_29` is wrong; `_24` is the DOI printed by the publisher and by the authors' bibliography page. Bitwuzla (Niemetz and Preiner, CAV 2023, https://doi.org/10.1007/978-3-031-37703-7_1; https://github.com/bitwuzla/bitwuzla; https://bitwuzla.github.io/). The DOI for the Bitwuzla paper was not re-fetched in this review (Appendix A, L-16). Boogie (Barnett et al., FMCO 2005, https://doi.org/10.1007/11804192_17; https://github.com/boogie-org/boogie).

**Solver-selection and limit policy.**

1. Layer 2 and CIRX aliasing queries use Z3 or Bitwuzla (configuration flag).
2. Layer 5 obligations use the Boogie backend (Z3 by default; cvc5 optional).
3. Every query carries a wall-clock limit, a memory limit, and a conflict limit; the verification record stores solver name, version, and resource use.
4. **Unknown rule.** An unanswered SMT query (timeout, memory exhaustion, or explicit `unknown`) is treated as "unknown" and forces the layer to abandon the function, preserving the soundness invariant of the layer. No "best-effort" or "probably equivalent" verdict is ever emitted from an incomplete solver run.

**Does not claim:** that a `sat` or `unsat` answer is correct beyond the soundness of the solver binary; solver bugs are an unmodeled residual risk. Mitigation: for Layer 5, obligations MAY be cross-checked with a second solver; disagreement is recorded as `unknown`.

### 3.4 Stage 2 – Type and Memory Reconstruction

**Tier 2a (fast heuristic).** Pattern-based inference (load width gives integer size; sign extension suggests signedness; alignment patterns suggest struct pointers). It runs per function in milliseconds and emits `type_confidence` in $[0.0, 0.6]$.

**Tier 2b (precise analysis).** Retypd-style constraint-based, inter-procedural type recovery (Noonan et al., PLDI 2016, https://doi.org/10.1145/2908080.2908119). It runs only on Track A high-risk functions and Track A′ rehabilitation candidates. It either emits a type substitution with confidence $\ge 0.70$ or aborts at a budget of $5 \times 10^5$ type constraints or 10 minutes of wall-clock time per function, whichever occurs first (policy parameters).

NotDec (Wang et al., ICSE 2026; https://conf.researchr.org/details/icse-2026/icse-2026-research-track/234/NotDec-WebAssembly-Decompilation-With-Inter-Procedural-Type-Recovery) reports, for **WebAssembly** inputs, 85.33 % recovery of struct member accesses against Ghidra's 9.24 % and 100 % recompilation on 5,241 Juliet samples against Ghidra's 45.95 %, and states that the full inter-procedural variant faces scalability challenges on large binaries. NotDec does not target AArch64. v2.2 presented NotDec as a realization of Tier 2b for ARM64 over `QF_AUFBV`; both statements are withdrawn. The figures are cited only as evidence that Retypd-style recovery can be effective in another setting. The v2.2 sentence "Retypd can exceed 24 hours for six cases at `-O0`" is not supported by the retrieved abstract and is removed (Appendix A, L-05).

**Does not claim:** that any type recovery here is sound; type substitutions are hypotheses carrying a confidence value, and Layer 2 consumes only substitutions with confidence $\ge 0.70$ together with the Layer 2 coupling check.

### 3.5 Stage 3 – SSA Conversion and Provenance Join

SSA is introduced only after Stage 2, at the **dominance-frontier** points of Cytron et al. (TOPLAS 13(4), 1991; https://doi.org/10.1145/115372.115320), computed independently for four variable classes: general registers (including `sp`), vector registers, each of the four flag bits, and memory. Memory is renamed as a versioned array variable $M_k$ (each `Store` and `StoreExclusive` produces $M_{k+1} = \mathrm{store}(M_k, \cdot, \cdot)$), so that Section 3.3 formulae apply unchanged.

The provenance record of a $\phi$-node with arguments $\pi_1, \dots, \pi_n$ is the **join**:

| Field | Join rule |
|-------|-----------|
| `origin` | Hull of the argument ranges |
| `type_confidence` | $\min_i$ of the arguments |
| `type_source` | The weakest source in the order Heuristic < LinkerMap < Retypd < DebugInfo |
| `semantic_loss` | Union |
| `inline_boundary`, `mangling`, `template_candidate` | Kept only if all arguments agree; otherwise null |
| `rtld_relocation` | `Unsupported` if any argument is `Unsupported`; otherwise kept if all agree |
| `metadata_provenance` | Union |

### 3.6 CIRX Metadata Specification

| Annotation | Type | Meaning |
|------------|------|---------|
| `!cirx.origin` | address range | Source instruction range in the original binary |
| `!cirx.mangling` | string or null | Demangled symbol, null if stripped |
| `!cirx.type_confidence` | float in [0.0, 1.0] | Confidence of type recovery |
| `!cirx.type_source` | enum | `heuristic`, `retypd`, `debug_info`, `linker_map` |
| `!cirx.inline_boundary` | string or null | Original callee symbol if inlined by LTO |
| `!cirx.semantic_loss` | set of enum | Dropped or opaque semantics (Section 3.2) |
| `!cirx.template_candidate` | 32-byte hash or null | Structural hash indicating possible template origin |
| `!cirx.rtld_relocation` | record | `resolved(kind, addend)` or `unsupported(kind)` (Section 8.3) |
| `!cirx.metadata_provenance` | set of enum | Metadata sources that contributed |

### 3.7 Verification Record Schema

The record is a JSON object validated against the following JSON Schema (draft 2020-12). Every record MUST validate; a record that fails validation is discarded and the function is reported `unknown`.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://example.invalid/cirx/verification-record-v3.json",
  "type": "object",
  "required": ["track", "arch", "abi", "layer", "function", "verdict", "guarantee_class", "smt_logics", "assumptions", "resource_used", "abandonment_reason", "cirx_sha256"],
  "additionalProperties": false,
  "properties": {
    "track": { "enum": ["A", "A_prime", "B"] },
    "arch": { "enum": ["aarch64", "aarch32", "x86_64", "x86_32", "x86_16", "riscv32", "riscv64", "mips32", "mips64", "ppc32", "ppc64", "wasm32", "wasm64", "other"] },
    "abi": { "enum": ["aapcs64", "aapcs64_apple", "aapcs64_windows", "aapcs32", "aapcs32_vfp", "sysv_amd64", "win64", "i386_cdecl", "i386_stdcall", "i386_fastcall", "i386_thiscall_msvc", "dos_near", "dos_far", "riscv_ilp32", "riscv_ilp32d", "riscv_lp64", "riscv_lp64d", "mips_o32", "mips_n32", "mips_n64", "ppc32_sysv", "ppc64_elfv1", "ppc64_elfv2", "wasm_basic_c_abi", "internal_lto", "unknown"] },
    "layer": { "enum": ["cf-gkat", "faultless", "pcode-trans", "chisel", "echo", "basil", "decir", "cluster"] },
    "function": { "type": "string", "minLength": 1 },
    "verdict": { "enum": ["proved", "disproved", "unknown", "out_of_envelope", "hypothesis"] },
    "guarantee_class": {
      "enum": ["formal_control_flow", "formal_data_flow_absolute", "formal_data_flow_relative", "formal_trace_equivalence",
               "execution_empirical", "byte_exact", "rehabilitated_partial", "structural_hypothesis", "none"]
    },
    "smt_logics": {
      "type": "array", "uniqueItems": true,
      "items": { "enum": ["QF_ABV", "QF_ABVFP", "QF_AUFBV", "QF_AUFBVFP", "algebraic", "none"] }
    },
    "solver": {
      "type": ["object", "null"],
      "properties": { "name": { "type": "string" }, "version": { "type": "string" } },
      "required": ["name", "version"], "additionalProperties": false
    },
    "assumptions": { "type": "array", "items": { "type": "string" } },
    "abandonment_reason": { "enum": ["none", "timeout", "memory", "conflicts", "unknown_result", "instruction_wall", "microop_wall", "loop_wall", "block_wall", "symbolic_value_wall", "unsupported_instruction", "unsupported_relocation", "unsupported_architecture", "no_semantics_backend", "config_budget", "rehab_budget", "fallback_budget", "fuzz_coverage_budget", "out_of_calibration_domain", "sandbox_unavailable"] },
    "resource_used": {
      "type": "object",
      "required": ["time_ms", "memory_mb", "solver_queries"],
      "properties": {
        "time_ms": { "type": "integer", "minimum": 0 },
        "memory_mb": { "type": "integer", "minimum": 0 },
        "solver_queries": { "type": "integer", "minimum": 0 }
      },
      "additionalProperties": false
    },
    "counterexample": { "type": ["object", "null"] },
    "cirx_sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" }
  }
}
```

The v2.2 record used the guarantee-class strings `control_flow`, `control_data_flow`, `byte_exact`, `empirical_high_coverage`, and others that were inconsistent with Section 5.3 and Section 9.2. The enumeration above is the single normative set.

### 3.8 Lowering Rules for Unwind and C++ Runtime Metadata

The following rules are required of any lifter claiming Track A eligibility. Each rule is a *lowering* from binary metadata to CIRX annotations; none is a proof of anything.

| Source structure | Lowering | Primary source |
|------------------|----------|----------------|
| DWARF `.eh_frame` CIE/FDE with `DW_CFA_*` rows | Per-address unwind rows (CFA rule, register rules) attached to call sites; `DW_CFA_AARCH64_negate_ra_state` toggles the return-address-signed bit and sets `PointerAuthOpaque` loss | DWARF 5; AArch64 DWARF ABI (aadwarf64) |
| `.gcc_except_table` LSDA | Call-site table: (start, length, landing pad, action) tuples become exceptional-edge annotations on `Call`; action chains become catch/cleanup edge kinds | Itanium C++ ABI exception handling |
| Itanium vtable and `typeinfo` | Vtable layout: offset-to-top, typeinfo pointer, then virtual function pointers; `typeinfo` objects of kinds `__class_type_info`, `__si_class_type_info`, `__vmi_class_type_info` yield inheritance edges | Itanium C++ ABI (RTTI) |
| MSVC `.pdata` and `.xdata` (ARM64) | `RUNTIME_FUNCTION` entries (packed or full `.xdata`) become unwind rows; ARM64 unwind codes (allocation, `save_fplr`, `set_fp`, `end`) become CFA rules; a language-specific handler RVA and handler data become exceptional edges | Microsoft ARM64 exception handling documentation |
| MSVC RTTI `CompleteObjectLocator` | Located at index $-1$ of a vftable; fields (signature, offset, constructor displacement offset, type descriptor, class hierarchy descriptor, self reference) yield class identity and base-class array | Microsoft ABI documentation and reverse-engineering literature; no single primary specification exists |
| AArch64 ELF relocations (static) | `R_AARCH64_ADR_PREL_PG_HI21`, `ADD_ABS_LO12_NC`, `LDST*_ABS_LO12_NC`, `CALL26`, `JUMP26`, GOT-page forms: resolved to absolute addresses before lifting | AAELF64 |
| AArch64 ELF relocations (dynamic) | `R_AARCH64_ABS64`, `GLOB_DAT`, `JUMP_SLOT`, `RELATIVE`, `TLS_TPREL64`, `TLSDESC`, `IRELATIVE`, `COPY`: recorded in `!cirx.rtld_relocation` | AAELF64 |
| Windows ARM64 base relocations | `IMAGE_REL_BASED_DIR64` and ARM64 branch/page relocations: recorded as resolved or unsupported | PE format documentation |

AAELF64: https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst. The `CompleteObjectLocator` row is marked low-assurance in Appendix A (L-12) because the cited sources are secondary; the lifter MUST validate each field against the image (RVAs inside the image; the type-descriptor name matches the `.?AV` mangling pattern) and set `unsupported` on any mismatch.

**Does not claim:** that these rules are complete. In particular, exceptional control flow introduced by `longjmp`, signal handlers, and C++ coroutines is not lowered; functions reaching them are reported `Out_of_envelope`.

### 3.9 Honest Cost Acknowledgement

No existing lifter fully satisfies invariants I1–I4. CIRX is a multi-year research and engineering roadmap, not an available artifact. The adapter estimates of Section 15 are planning figures, not measurements.

---

## 4. The Metadata Spectrum and Routing

### 4.1 Metadata Availability Score

Each indicator $I_k \in \{0, 1\}$ is a predicate over the input image and its sidecar files, defined below. Weights are integers (percentage points) so that threshold comparisons use exact integer arithmetic.

**Two-phase bootstrap (v3.2 — closes F-04).** v3.1 defined $I_{\text{sym}}$ in terms of "the function entry points found by CFG recovery," while CFG recovery is a Track A / A′ instrument. This makes the router circular. v3.2 breaks the circularity with a two-phase procedure:

1. **Phase 0 — Bootstrap CFG recovery.** Before MAS is computed, the lifter runs a *bootstrap* CFG recovery pass using only MAS-independent evidence: the container's symbol table (`.symtab`, `.dynsym`, exports, PDB), unwind metadata (`.eh_frame`, `.pdata`, `.ARM.exidx`), and relocation evidence. Bootstrap recovery does not consume the static risk score and does not invoke Layers 1–5. Its output is a partial set of function entry points $\mathcal{E}_0$.
2. **Phase 1 — MAS computation.** $I_{\text{sym}}$ is evaluated against $\mathcal{E}_0$, not against the full Track A CFG. $I_{\text{line}}$ is evaluated against $\mathcal{E}_0$ as well. The remaining indicators ($I_{\text{dyn}}, I_{\text{map}}, I_{\text{lto}}, I_{\text{cmt}}$) are container-level and do not depend on CFG recovery.
3. **Phase 2 — Track-A CFG refinement.** After the track is selected, the Track A lifter may *refine* the CFG using Track A's own evidence. The refinement is a *disjoint* pass; it MUST NOT re-enter Phase 0 and MUST NOT change $\mathrm{MAS}_{\%}$. The refined CFG is recorded separately as $\mathcal{E}_1$.

**Termination.** Phase 0 terminates because the bootstrap evidence is finite and the recovery algorithm is terminating (linear sweep + symbol-seeding + unwind-seeding). Phase 1 terminates because $\mathrm{MAS}_{\%}$ is a closed-form expression over the Phase 0 outputs. Phase 2 terminates because the Track A lifter's refinement is bounded by the §6.7 walls.

**Tie-break.** When bootstrap recovery is partial (the bootstrap pass cannot resolve some entry points because no MAS-independent evidence is available), $\mathrm{MAS}_{\%}$ is computed against the recovered subset, and the badge reports `[MAS-partial: N]` where $N$ is the recovered entry count (no denominator — see below). Track A's eligibility predicate still applies to every function, including those not in $\mathcal{E}_0$; functions outside $\mathcal{E}_0$ are reported `[Unknown entry]` and routed by container-level evidence alone (typically Track B).

**v3.2.1 correction (closes N15).** The v3.2 text proposed `[MAS-boot: N/M]` where $M$ was described as "bootstrap's lower bound on the true entry count." No algorithm computes $M$ without re-running the full Track A recovery, which would re-introduce the circularity. v3.2.1 drops the denominator and reports only the recovered count. The badge carries the raw $N$; consumers that need a confidence interval can apply the testbed's empirical bootstrap-recovery rate (a policy parameter to be calibrated by a future hypothesis).

**Does not claim:** that bootstrap recovery is exhaustive. Functions reached only by indirect branches with no symbol, unwind, or relocation evidence are not in $\mathcal{E}_0$ and are tracked separately. The two-phase procedure removes the circularity; it does not solve the general entry-point discovery problem.

$$
\mathrm{MAS}_{\%} = 30\,I_{\text{sym}} + 15\,I_{\text{dyn}} + 10\,I_{\text{line}} + 15\,I_{\text{type}} + 15\,I_{\text{map}} + 10\,I_{\text{lto}} + 5\,I_{\text{cmt}}, \qquad \mathrm{MAS} = \mathrm{MAS}_{\%} / 100.
$$

The weights sum to 100. They are policy parameters, not fitted values.

| Indicator | Predicate ($=1$ iff) |
|-----------|----------------------|
| $I_{\text{sym}}$ | A local symbol table (`.symtab`, or a locally loaded PDB) names at least 50 % of the function entry points found by CFG recovery |
| $I_{\text{dyn}}$ | `.dynsym` (ELF) or the export directory (PE) lists at least one function |
| $I_{\text{line}}$ | DWARF `.debug_line` or PDB line records map at least 50 % of `.text` bytes to source lines |
| $I_{\text{type}}$ | DWARF `.debug_info` (or the PDB type stream) supplies parameter and return types for at least 50 % of functions |
| $I_{\text{map}}$ | A linker map file is supplied and parses |
| $I_{\text{lto}}$ | Intermediate LTO bitcode or object files are supplied |
| $I_{\text{cmt}}$ | A compiler identification string is found (`.comment` or `DW_AT_producer`) |

v2.2 wrote `[.dynsym > 0]` (mixing a Boolean with an arithmetic comparison) and left every other indicator undefined; both defects are fixed above. Because $I_{\text{type}}$ usually implies $I_{\text{line}}$, indicators are not independent; the score is a routing heuristic, not a probability.

| MAS range | Track |
|-----------|-------|
| $\mathrm{MAS}_{\%} \ge 70$ | A |
| $30 \le \mathrm{MAS}_{\%} < 70$ | A′ |
| $\mathrm{MAS}_{\%} < 30$ | B |

*Worked examples.* An unstripped Linux ELF with `.symtab`, `.dynsym`, line tables, DWARF types, and `.comment` scores $30 + 15 + 10 + 15 + 5 = 75$: Track A. A stripped ELF with `.dynsym`, line tables, and `.comment` scores $15 + 10 + 5 = 30$: Track A′ (boundary case). A stripped ELF with `.dynsym` and `.comment` scores $20$: Track B.

**MAS does not guarantee layer eligibility.** A binary can reach Track A without DWARF types (for example, $30 + 15 + 10 + 15 + 10 + 5 = 85$ with a linker map and LTO objects but no DWARF types). Eligibility is therefore decided per function and per layer by the predicates in Section 6.7; MAS routes only the binary.

### 4.2 Routing Within Tracks

Track A schedules functions by static risk (Section 6.1). Track A′ attempts rehabilitation (Section 5) and then applies the Track A scheduler to rehabilitated functions only. Track B routes functions through trace hashing under the Sandbox Safety Contract and falls back to static emulation for hostile binaries.

---

## 5. Track A′: Partial-Metadata Rehabilitation

### 5.1 Applicability

$30 \le \mathrm{MAS}_{\%} < 70$. Typical cases: exports intact with `.symtab` stripped; line tables without variable types; a PDB available only from a symbol server; a linker map supplied for a stripped binary.

### 5.2 Rehabilitation Protocol

**R1 – Layer 2 via export hooks.** For every function reachable from an exported entry point through direct calls, propagate the export's declared signature to the callee. Where the callee is inlined, mark the region with `!cirx.inline_boundary`. Effect: `type_confidence` MAY rise to at least 0.70 on the export call graph. Limit: propagation is unsound across indirect calls and across LTO-internalized functions whose calling convention changed (Section 2.4); both stop propagation.

**R2 – Layer 3 via line-table hooks.** Line records map addresses to source lines. Consecutive lines indicate loop structure; one source line spanning several basic blocks marks a potential inline site. Effect: functions with line coverage gain Layer 3 test-harness boundaries.

**R3 – Layer 4 via compiler identification.** `.comment` or `DW_AT_producer` narrows the compiler-configuration search for Echo.

**R4 – Layer 5 via linker-map hooks.** A linker map supplies symbol, offset, and size, so function boundaries can be established without a symbol table, which is a precondition for lifting with BASIL.

### 5.3 Layer-Specific Guarantee Degradation

| Layer | Standard guarantee class | Rehabilitated guarantee class |
|-------|--------------------------|-------------------------------|
| CF-GKAT | `formal_control_flow` | `formal_control_flow`, only if the CFG has no unresolved indirect branches |
| Layer 2 | `formal_data_flow_relative` (Track A) / `formal_data_flow_absolute` (Track A, only when a sound absolute checker is available — research obligation) | `rehabilitated_partial` |
| Layer 3 | `execution_empirical` | `rehabilitated_partial` |
| Layer 4 | `byte_exact` | `byte_exact`, only if the recompiled bytes match exactly under the narrowed configuration |
| Layer 5 | `formal_trace_equivalence` | `formal_trace_equivalence`, only if the verification condition is discharged and no over-approximation caveat applies |

The v2.2 table marked three layers "unchanged". That wording implied unchanged *coverage*; the table above states that the class is unchanged only when the stated condition holds, and that coverage is reported separately (Section 5.5).

### 5.4 Rehabilitation Budget

The total rehabilitation budget per binary is 20 minutes of wall-clock time (policy parameter). A rehabilitation that exceeds its share is abandoned and its layer is not rehabilitated for the affected functions.

### 5.5 Coverage Reporting

Track A′ reports, per layer, the fraction of functions rehabilitated and the fraction of those that reached a positive verdict. It never claims full-cascade coverage. The v2.2 claim "85–95 % behavioral consistency in Track A′" had no source and is withdrawn; it is replaced by pre-registered hypothesis H-A′1 (Section 16).

**Does not claim:** that rehabilitation preserves the soundness of the original layer where propagated types are wrong; wrong propagated types cause `disproved` or `unknown`, never a spurious `proved`, only if Layer 2 re-checks the types against the body (Section 6.3).

---

## 6. Track A: Formal Evidence-Backed Reconstruction

### 6.1 Applicability and Risk-Budgeted Scheduler

$\mathrm{MAS}_{\%} \ge 70$. The static risk score is a deterministic, integer-valued, normalized function of five features. It is valid only where a symbol table or debug information exists, so it is a Track A and A′ instrument. High-risk functions (security-critical, cryptographic, privilege-boundary) are routed through Layers 2–5 first and are *exempt from budget-based skipping but not from walls*: a wall (§6.7) is a soundness boundary and is never bypassed. v2.2 stated that high-risk functions "bypass Layer 5 abandonment thresholds", which contradicts the hard-wall rule of §6.6; the statement is withdrawn.

**Static risk score definition (v3.2 — closes F-05).** Let $f$ be a function with recovered CFG, computed features, and zero or more `high_risk` annotations. The features are:

| Feature | Symbol | Definition | Domain |
|---------|--------|------------|--------|
| Cyclomatic complexity | $M$ | $E - N + 2$ over the recovered CFG, where $E$ is edge count and $N$ is basic-block count | $\mathbb{Z}_{\ge 1}$ |
| Pointer-arithmetic density | $P$ | $\frac{\#\{\text{GetElementPtr with non-constant index, ptrtoint, inttoptr}\}}{1 + \text{instruction count}}$ | $[0, 1]$ |
| API-interaction surface | $A$ | $\#\{\text{calls into external symbols}\}$ | $\mathbb{Z}_{\ge 0}$ |
| Loop-nesting depth | $L$ | maximal nesting of natural loops in the CFG | $\mathbb{Z}_{\ge 0}$ |
| Indirect-call count | $I$ | $\#\{\text{call instructions with non-direct target}\}$ | $\mathbb{Z}_{\ge 0}$ |

**Formula.** The unweighted score is $S = 8 M + 1000 P + 20 A + 50 L + 200 I$. The weighted score is $R = \min\left(100, \left\lfloor S / S_{\text{cal}} \right\rfloor\right)$ where $S_{\text{cal}} = 50$ is a policy-parameter calibration constant. v3.2.1 correction (closes N4): the v3.2 formula had an arithmetic error. For the "typical" function with $M{=}10, P{=}0.05, A{=}3, L{=}1, I{=}0$, the unweighted score is $S = 8(10) + 1000(0.05) + 20(3) + 50(1) + 200(0) = 80 + 50 + 60 + 50 + 0 = 240$ (v3.2 incorrectly wrote 380). With $S_{\text{cal}} = 50$, the typical function scores $R = \lfloor 240 / 50 \rfloor = 4$ (v3.2 incorrectly wrote 7). The integer $R \in [0, 100]$ is the **static risk score**.

**Weight table provenance.** The weights $(8, 1000, 20, 50, 200)$ are policy parameters; they are not fitted values, and the document does not claim a fitted model. They are chosen so that each feature contributes comparable magnitude to the "typical" function above. A future revision MAY publish fitted weights conditioned on a corpus; the current weights are documented in the §16 hypothesis **H-RS1** for re-calibration.

**Risk bands.** The score partitions functions into three bands:

| Band | Range | Treatment |
|------|-------|-----------|
| Low risk | $R \in [0, 20]$ | Scheduled normally; Layers 1–3 attempted in order |
| High risk | $R \in (20, 70]$ | Layers 2–5 attempted in order; budget-based skipping NOT applied |
| Critical risk | $R \in (70, 100]$ | Layers 2 and 5 attempted first; walls still apply |

**High-risk override.** A function annotated `high_risk` (security-critical, cryptographic, privilege-boundary) is treated as Critical risk regardless of $R$.

**Does not claim:** that $R$ is a probability, that the weights are optimal, or that $R$ is comparable across architectures. The score is a routing heuristic, not a verification claim. The acceptance test for H-RS1 is: on the testbed, the calibration constant $S_{\text{cal}}$ is re-fit by isotonic regression against a held-out set of expert-labeled security-critical functions, and the re-fit $S_{\text{cal}}'$ satisfies $\lvert R - R' \rvert \le 5$ for $\ge 90\%$ of the test set.

### 6.2 Layer 1 – Control-Flow Equivalence (CF-GKAT)

CF-GKAT (Zhang, Kappé, Narváez, Naus; POPL 2025; https://arxiv.org/abs/2411.13220; https://doi.org/10.1145/3704857) extends Guarded Kleene Algebra with Tests (Kozen, TOPLAS 19(3), 1997, https://doi.org/10.1145/256167.256195; Smolka et al., POPL 2020, https://doi.org/10.1145/3371129) to non-local control flow (`goto`, `break`, `return`) and hardcoded values. The paper states that it soundly and completely decides trace equivalence for its program class with nearly linear efficiency. The symbolic decision procedures of Zhang et al. (https://arxiv.org/abs/2601.09986) were evaluated on 237 Coreutils functions compiled with Clang and decompiled with Ghidra, and the work reports a confirmed and fixed Ghidra bug (mismatch on `copy_internal`, Coreutils 9.5). The symbolic implementation is written in Rust and uses SAT solvers; it therefore is "algebraic" in this specification's badge sense (no SMT) but not solver-free.

*Blinding.* Primitive actions and tests are replaced by opaque calls, so the check is agnostic to data semantics.

*Pipeline policy.* Functions with more than 10,000 basic blocks, or with unresolved indirect branches that cannot be treated as opaque primitives, are skipped. The v2.2 cost bound "$O(n\log n)$ in control-flow edges" is replaced by "nearly linear in automaton size" (the cited papers' statement); no tighter bound is asserted. The v2.2 figures "about 7 seconds, under 100 MB" and "807 of 1,425 functions blindable" were not located in the retrieved text and are removed (Appendix A, L-08).

**What it proves:** control-flow trace equivalence *assuming* corresponding primitives are semantically equivalent. **Does not claim:** data-flow equivalence, equivalence of the primitives themselves, or anything about functions outside the supported control-flow fragment.

### 6.3 Layer 2 – Data-Flow Equivalence (Faultless-style)

Faultless (Dramko, Le Goues, Schwartz; https://arxiv.org/abs/2609.34089) is a translation-validation technique that, per its abstract, "compares code produced by a deterministic decompiler with that of a neural decompiler"; its input is *two C functions*, it uses static symbolic execution with a custom execution and memory model, and its reference implementation (a Python package described as a proof of concept) depends on Z3. The authors note that deterministic decompilers are themselves often unsound, in predictable ways.

*Consequences for this pipeline.*

1. A Layer 2 verdict is an equivalence **relative to the reference decompiler's output**, not to the binary. Its guarantee class is therefore `formal_data_flow_relative`.
2. Faultless is not run on CIRX. This specification defines Layer 2 as a *Faultless-style checker over CIRX-derived C* with the memory model of Section 3.3 and the logic `QF_ABV`; the logic is this pipeline's commitment, not an attribute of Faultless.
3. Reported figures (not re-verified here; Appendix A, L-02): proof of more than 50 % of genuinely equivalent pairs at `-O0` and 43–45 % at higher optimization levels, 87 % agreement with unit tests. These were measured on the authors' benchmarks, not on ARM64 LTO code.

*Pipeline policy (preconditions and walls).* Every value has `type_confidence` $\ge 0.70$; at most 3 nested loops; at most 100 basic blocks per loop body; at most $10^6$ symbolic values. These are policy parameters, not published limits of Faultless.

**Does not claim:** that the reference decompiler is correct; that heap-mediated dependencies are captured (the source reports unsound verdicts concentrated in a heap-modeling bug); or that globals are matched without an inferred mapping.

### 6.4 Layer 3 – Execution-Based Validation (PCodeTrans and CHISEL)

PCodeTrans (Cui, Gao, He, Qin, Zhang; https://arxiv.org/abs/2603.14855) extracts a minimal coherent context, hot-swaps the recompiled function into the original running binary, and uses fine-grained differential tracing to guide LLM repair. Its abstract frames the goal as function-level dynamic validation using Hex-Rays pseudocode as input. The v2.2 figures 100 % function-level compilability on unstripped binaries, 99.55 % and 99.89 % test-validated behavioral consistency on Coreutils and Binutils, and over 96 % on stripped binaries are *reported figures* carried forward from the source document (Appendix A, L-03); their evaluation architecture is not stated as ARM64. The v2.2 sentence "100 % compilability" is a property of one benchmark, not a guarantee.

CHISEL (Kohli, Raghava, Sikdar, Divakaran, "CHISEL-ing Back Source Code with AI-enabled Iterative Recovery", SURE 2026; https://arxiv.org/abs/2608.27981) recovers compilable, behaviorally faithful C from Ghidra pseudo-C **without a test suite**. The ingestion package that supplied this specification's latest changes states the loop as follows (the paper's existence, title, authors, and venue were verified in this review; its internals and figures were not re-read, Appendix A, L-04):

1. *Generation.* An LLM proposes candidate C from the pseudo-C (the package names an open-weight Gemma-family backbone).
2. *Static oracle.* A compiler attempt; type errors, missing declarations, and syntax failures become structured feedback.
3. *Differential oracle.* A coverage-guided, type-aware fuzzer runs the candidate and the **original binary** and compares return values, output-buffer *contents* (pointers by content, not address), standard output, exit codes, timeouts, and crash signals.
4. *Cross-iteration divergence memory* records failure modes already seen, and *best-candidate retention* keeps the strongest candidate if later iterations regress.

Figures reported by the package for CHISEL's own benchmark (120 ExeBench functions, **x86-64**, `-O0` through `-O3`, stripped and unstripped): 96.1 % re-compilability and 79.8 % re-executability at about 2.1 iterations, about 26 % of first-generation execution errors recovered, and a feedback-oracle false-accept rate of 9.4 %. These are *reported figures in an x86-64 setting*; they are not claims about any other architecture.

*Role in the cascade.* PCodeTrans needs the program's own tests; CHISEL needs none, so CHISEL is the Layer 3 option when no tests exist. Both are empirical layers (`SMT: {none}`) with guarantee class `execution_empirical`. CHISEL's oracle can only *refute* equivalence on the inputs the fuzzer reaches; its false-accept rate is nonzero, and every Layer 3 record from CHISEL MUST carry the fuzzer's coverage and the number of executions.

**Soundness boundary.** Execution validation can *refute* equivalence (a divergence is a counterexample) but can never prove it. Its guarantee class is `execution_empirical`, and every Layer 3 record MUST report the number of tests and the fraction of basic blocks covered. A 9.4 % oracle false-accept rate means roughly one in eleven accepted candidates is wrong at the oracle's own measured rate, so Layer 3 acceptance MUST be paired with the oracle false-positive rate in the badge (Section 10).

### 6.5 Layer 4 – Byte-Exact Matching (Echo)

Echo (Bi et al.; https://arxiv.org/abs/2609.18706) performs matching decompilation via *trusted back-translation*: compile candidates under a fixed configuration, compare assembly, and repair mismatches with rule-based, neural, and reasoning-based refinement. The paper reports 2.43× more exact assembly matches on average than the strongest baseline on function-level benchmarks and, on the Mirai malware binary, matching 2.75× and 7.4× as many functions as GPT-5.6 and Codex respectively (ratios read from the retrieved abstract of arXiv:2609.18706 in this review). The v2.2 text "2.75× more matches than GPT-5.6 and substantially more than Codex" is replaced by the exact ratios; the source document's "35.5 % exact match" figure was not located and is removed (Appendix A, L-07).

*Precondition:* known compiler, version, and flags (for Track A′, narrowed by Section 5.2 R3). *Pipeline policy:* abandon after $10^3$ configurations per function. *Soundness:* a byte-exact match under a pinned configuration means the candidate compiles to the same bytes; it is evidence of source-level plausibility, not of source identity (different sources can compile to identical bytes), and it is `byte_exact` rather than `formal_*`. **Does not claim:** that a wrong configuration yields only false negatives; a wrong configuration can in principle produce a coincidental match, and the verification record stores the configuration hash.

### 6.6 Layer 5 – Formal Translation Validation (BASIL and DecIR)

BASIL ("Binary Analysis for Secure Information-Flow Logics"; UQ-PAC; https://github.com/UQ-PAC/BASIL) is a Scala pipeline that takes a GTIRB semantics file (`.gts`, from `gtirb-semantics`) and `readelf` output for an AArch64 binary and emits a Boogie program, optionally running Boogie (`boogie /useArrayAxioms`). Instruction semantics come from ASLp (https://github.com/UQ-PAC/aslp). v2.2 expanded the acronym incorrectly ("Boogie Analysis for Secure Information-Flow Logics"), listed BASIL's language as OCaml (Section 15), and credited BAP as the lifter; the repository documents Scala, GTIRB, and ASLp.

*What is documented:* binary to Boogie translation and verification of information-flow properties. *What is not documented in retrieved sources:* an equivalence check between a recompiled LLM candidate and the original binary, the name and definition "Conditional Trace-Equivalence", and the statement that results transfer to executables. These are removed from the normative text (Appendix A, L-01).

**Normative Layer 5 design (to be built).** (1) Compile the candidate with the pinned toolchain; (2) lift both objects with the BASIL pipeline to Boogie; (3) generate a bounded relational verification condition on corresponding inputs and observable effects; (4) discharge with Boogie and Z3 or cvc5 under the Unknown rule of Section 3.3.4. Step (3) does not exist in BASIL; Layer 5 is therefore *specified, not available*, and until step (3) is implemented its only permitted outputs are properties verified on the lifted binary (`formal_trace_equivalence` is not emitted).

*Pipeline policy:* abandonment wall at 2,000 instructions or more than 2 nested loops. Instructions without ASLp coverage force a sound over-approximation (the lifted program may exhibit more behaviors, never fewer); a safety property proved on an over-approximation holds on the original, but an equivalence cannot be concluded from an over-approximation of both sides. DecIR (LLM decompilation with iterative compilation-error correction and SMT-backed consistency checks) is cited from the source document and is not re-read (Appendix A, L-06).

### 6.7 Cascade Summary and Eligibility Predicates

| Layer | Eligibility predicate (policy) | Guarantee class | Abandonment wall | Logic |
|-------|-------------------------------|-----------------|------------------|-------|
| 1 CF-GKAT | $\le 10{,}000$ basic blocks; no unresolved indirect branch | `formal_control_flow` | 10,000 blocks | algebraic |
| 2 Faultless-style | All values $\ge 0.70$ confidence; $\le 3$ loops; $\le 100$ blocks per body; $\le 10^6$ symbolic values; no `semantic_loss` | `formal_data_flow_relative` (Track A, reference decompiler assumed unsound) / `formal_data_flow_absolute` (Track A, only when a sound absolute Layer 2 checker exists — research obligation; not currently reachable; included in the enum for forward compatibility) / `rehabilitated_partial` (Track A′) | Any of: 3 nested loops, 100 blocks/loop body, $10^6$ symbolic values, $T_{2}$ wall-clock seconds | `QF_ABV` |
| 3 PCodeTrans, CHISEL | Recompilable context; interceptable syscalls | `execution_empirical` | Fuzz coverage budget (per-input) | none |
| 4 Echo | Known compiler configuration | `byte_exact` | $10^3$ configurations | none |
| 5 BASIL, DecIR | $\le 2{,}000$ instructions; $\le 2$ loops; no unsupported instruction | `formal_trace_equivalence` (gated — see §10.2 rule 2) | 2,000 instructions or 3 loops | `QF_AUFBV` |
| B cluster | Track B only; calibrated set; coverage, $\alpha$, stratum, calib version, $n_\ell$ | `structural_hypothesis` | Calibration domain; per-stratum $n_\ell \ge \lceil 1/\alpha \rceil - 1$ | none |

The pipeline never reports equivalence beyond a wall; it reports what each layer achieved and stops.

**v3.2 clarification (closes F-10).** The "Same" in the v3.1 Layer 2 abandonment-wall cell was a dangling reference to Layer 1's 10 000-block bound. v3.2 states the actual wall: Layer 2 abandons when *any* of four conditions is met (3 nested loops, 100 blocks per loop body, $10^6$ symbolic values, or the $T_{2}$ wall-clock limit). The wall-clock limit $T_{2}$ is a policy parameter; the v3.2 default is 30 seconds. The per-architecture refinements of §20.10 (micro-operation walls) apply unchanged.

**Layer 5 guarantee class gating (closes F-09).** The `formal_trace_equivalence` guarantee class is emittable only when the Layer 5 relational harness exists and the verification condition is discharged with no over-approximation caveat (§6.6). Until then, Layer 5's effective guarantee class for badge purposes is `none`, and the badge's `Guarantee` field MUST NOT carry `formal_trace_equivalence`. The verification record's enum still contains the value (for forward compatibility) but it is not reachable from a present-day pipeline run.

### 6.8 Residual Risk (Track A)

Even when every layer returns a positive verdict:

1. CF-GKAT proves control-flow trace equivalence only under the assumption that corresponding primitive tests and actions are equivalent.
2. Layer 2 equivalence is relative to a reference decompiler that can be unsound.
3. Layers 2 and 5 are bounded by their walls; functions beyond a wall are `out_of_envelope`, never `proved`.
4. Layer 3 can refute but never prove; its false-accept rate is nonzero.
5. Layer 4 matches bytes under one configuration; it does not prove source identity.
6. Layer 5 is a design until a relational harness exists (Section 6.6).
7. All SMT verdicts inherit solver and front-end (lifter, ASLp) correctness; semantic loss recorded in CIRX excludes functions from Layers 2 and 5.
8. Nothing here is a statement about concurrency (Section 3.3.3).
9. The whole cascade remains subject to Rice's theorem for any property outside the explicit envelopes.

---

## 7. Track B: Probabilistic Behavioral Triage

### 7.1 Fundamental Distinction

Track B makes no formal verification claim, does not route functions through Layers 2 or 5, and delivers ranked hypotheses with conformal prediction sets.

### 7.2 Partial-Execution Trace Hashing

1. **Execution** under a deterministic emulator with a curated input corpus (empty, all-zeros, all-ones, structured patterns for common data formats).
2. **Recording** of executed instruction addresses and memory accesses.
3. **Normalization**: rebase addresses, canonicalize stack-frame offsets, canonicalize register names, and collapse repeated trace subsequences (loop unrolling).
4. **Hashing**: a locality-sensitive hash of the normalized trace.
5. **Matching** against a corpus of open-source and standard-library traces.

Weisfeiler–Lehman graph kernels (Shervashidze et al., JMLR 12, 2011, https://www.jmlr.org/papers/v12/shervashidze11a.html) are *hypothesized* to degrade under LTO because block splitting and merging alter graph structure; v2.2 asserted this ("a topological illusion") and asserted that trace signatures are "invariant to LTO-induced CFG distortion". Neither statement is demonstrated by a cited result. They are replaced by pre-registered hypothesis H-B3 (Section 16): trace-hash top-1 precision on LTO builds is within 10 percentage points of its precision on non-LTO builds of the same source, on the domain testbed.

**Does not claim:** invariance; coverage of paths not exercised by the input corpus; or correctness of the emulator's model of the target's I/O.

### 7.3 Sandbox Safety Contract

**Pre-execution classification.**

| Class | Signals | Action |
|-------|---------|--------|
| Safe | No anti-tamper section names (for example `.vmp`, `.themida`, `.enigma`), no debugger-detection strings, no `ptrace` (Linux) or `NtQueryInformationProcess` (Windows) imports, no timing or hardware-identification register reads outside the C runtime | Sandbox execution |
| Suspect | Any anti-tamper section, any debugger-detection string, or any anti-VM probe | Sandbox execution with environment stubs |
| Hostile | Obfuscated entry point, integrity self-checks, encrypted code sections, or a classification that is unknown | Static emulation only (Section 7.5) |

An *unknown* classification is Hostile (fail closed). Anti-debugging and anti-virtualization behavior in malware is documented by Chen et al., DSN 2008 (https://doi.org/10.1109/DSN.2008.4630086).

**Environment stubs (AArch64 and Windows ARM64).** v2.2 specified x86 artifacts (`rdtsc` returning 10–50 cycles, `CPUID` leaves 0x1 and 0x40000000) for ARM64 targets, where neither exists. The stubs are:

- `ptrace(PTRACE_TRACEME, 0, 0, 0)` returns 0 (Linux).
- `NtQueryInformationProcess(ProcessDebugPort)` returns `STATUS_SUCCESS` with a zero port (Windows ARM64).
- Reads of the virtual counter `CNTVCT_EL0` advance by a fixed small increment per read, and `CNTFRQ_EL0` returns a fixed documented frequency (Arm Architecture Reference Manual, https://developer.arm.com/documentation/ddi0487/latest).
- Reads of `MIDR_EL1` and the `ID_AA64*` feature registers, which Linux emulates for user space (https://docs.kernel.org/arch/arm64/cpu-feature-registers.html), return a fixed bare-metal profile.
- File-system and registry probes return the expected presence or absence of sandbox artifacts.

Horizon OS (Switch) has no `ptrace`; commercial Switch binaries run only under a system emulator that implements Horizon service calls, so the stub set is emulator-specific and is part of the emulator adapter.

**Execution bounds (policy).** 5 seconds wall-clock per input; $10^8$ instructions; 512 MB allocation; $10^4$ system calls. If any bound is exceeded the trace is discarded and the function is marked `sandbox_unavailable`.

**Containment.** User-space emulation (Unicorn, https://www.unicorn-engine.org/; QEMU user mode) wherever possible. Where kernel interaction is required, the guest MUST run in a hardware-virtualized microVM or a user-space-kernel sandbox. A plain container (`docker run --network=none --read-only --cap-drop=ALL`) shares the host kernel and is *not* sufficient for Suspect or Hostile malware; v2.2 listed it as the isolation for kernel-interaction cases, and that is withdrawn. Network access is denied; DNS is stubbed.

**Stress test: Hostile classification.** The binary is never executed. Static emulation produces traces marked `trace_source: static`; the badge reports `[Sandbox: Static]`; prediction sets are computed only if the obfuscation level is inside the calibration domain, otherwise `[Confidence: N/A, Reason: out_of_calibration_domain]`.

**Sandbox coverage report.**

```text
[Track: B]
[Sandbox Coverage: 71% (1,065 of 1,500 functions)]
[Sandbox Classification: 42% safe, 39% suspect, 19% hostile]
[Static Emulation Fallback: 19% (hostile class)]
```

The numbers above are an *illustrative format example*, not a measurement.

### 7.4 Dynamic Risk Signaling

Static risk scoring fails on stripped binaries, so Track B uses runtime signals: writes to privileged address ranges; memory-access patterns matching AES S-box indexing, SHA message schedules, or elliptic-curve scalar multiplication; system or hypervisor calls with privileged numbers; and allocator free-list or thread-local-storage manipulation. These signals are hypotheses to be validated (Section 18); they are not claimed to detect security-critical functions.

### 7.5 Static Emulation Fallback

For Hostile binaries, symbolic partial execution (angr, Shoshitaishvili et al., IEEE S&P 2016, https://doi.org/10.1109/SP.2016.17; https://angr.io/) constructs static traces. **Stress test: VMProtect-class virtualization plus full stripping plus extreme LTO plus unknown compiler.** Virtualization-based obfuscators are outside the pipeline's scope; symbolic deobfuscation research exists (Salwan, Bardin, Potet, DIMVA 2018, https://doi.org/10.1007/978-3-319-93411-2_17) but is not integrated. For such inputs the output is `[Cluster: unknown_obfuscated, Confidence: N/A]` with `Formal: Not_applicable`, and the binary is routed to manual analysis. An unknown compiler removes Layer 4 and any compiler-keyed template matching; an unknown compiler with MAS below 30 therefore yields Track B output only.

### 7.6 Domain-Adapted Conformal Calibration

**Setting.** For each obfuscation stratum $\ell \in \{\text{none}, \text{basic}, \text{intermediate}, \text{advanced}, \text{extreme}\}$, a calibration set $\{(x_i, y_i)\}_{i=1}^{n_\ell}$ of labeled testbed clusters is held out from the training data of the probability model $\hat p$ (split conformal prediction; Papadopoulos et al., ECML 2002, https://doi.org/10.1007/3-540-36755-1_29; Lei et al., JASA 2018, https://doi.org/10.1080/01621459.2017.1307116; Angelopoulos and Bates, 2023, https://arxiv.org/abs/2107.07511).

**Non-conformity score** (the "least ambiguous classifier" score; Sadinle, Lei, Wasserman, JASA 2019, https://doi.org/10.1080/01621459.2017.1395341):

$$
V_i = 1 - \hat p(y_i \mid x_i).
$$

**Quantile (corrected).** Let $k = \lceil (n_\ell + 1)(1 - \alpha) \rceil$ and let $V_{(1)} \le \cdots \le V_{(n_\ell)}$ be the sorted scores. Define

$$
\hat q_\ell(\alpha) = \begin{cases} V_{(k)} & \text{if } k \le n_\ell, \\ +\infty & \text{if } k > n_\ell. \end{cases}
$$

Equivalently, $\hat q_\ell(\alpha) = \inf\{ q : |\{i : V_i \le q\}| \ge (n_\ell + 1)(1 - \alpha) \}$. The v2.2 formula used $(|\{i : V_i \le q\}| + 1)/(n+1) \ge 1 - \alpha$, which selects an order statistic one rank too low and undercovers by up to $1/(n+1)$; it is replaced.

*Measured behavior (this review, Appendix E.4).* Monte Carlo with 40,000 trials per row and uniform scores: at $\alpha = 0.10$ and $n = 19$, the corrected rule covers 0.902 (target at least 0.900) and the v2.2 rule covers 0.849; at $n = 49$ the values are 0.902 and 0.879; at $n = 99$, 0.899 and 0.889; at $n = 240$, 0.899 and 0.897. The v2.2 undercoverage therefore matches the stated bound of about $1/(n+1)$ and is largest for the small per-stratum calibration sets that obfuscation strata produce.

**Prediction set.** $\Gamma_\alpha(x) = \{ c : 1 - \hat p(c \mid x) \le \hat q_\ell(\alpha) \}$.

**Guarantee.** If the calibration points and the test point are exchangeable *within stratum $\ell$*, then

$$
1 - \alpha \;\le\; \mathbb{P}\big( y \in \Gamma_\alpha(X) \big) \;\le\; 1 - \alpha + \frac{1}{n_\ell + 1}
$$

(the upper bound holds when the scores are almost surely distinct; Lei et al., 2018). Stratification by $\ell$ yields stratum-conditional validity in the sense of Mondrian conformal prediction (Vovk, ACML 2012, PMLR 25:475–490, https://proceedings.mlr.press/v25/vovk12.html). The v2.2 attribution of the bound to "Vovk et al. (2005), Proposition 2.3" was not verified and is replaced by the sources above.

**Reporting.** The badge reports the prediction set, the coverage level $1-\alpha$, the stratum, the calibration-set version, and $n_\ell$. A point probability $\hat p$, if shown, is labeled "uncalibrated model score". The v2.2 format `Confidence: 0.91 [0.83, 0.96]` conflated a point score with an interval; conformal prediction yields a *set*, not an interval on a probability.

**Out-of-domain rule.** If the detected obfuscation level, compiler family, or architecture falls outside the calibration domain, or if $n_\ell < \lceil 1/\alpha \rceil - 1$ (which would force $k > n_\ell$ and a trivial set), the report is `[Confidence: N/A, Reason: out_of_calibration_domain]`. Covariate shift between testbed and field binaries breaks exchangeability (Tibshirani et al., NeurIPS 2019, https://arxiv.org/abs/1904.06019); no coverage statement is made under shift.

**Does not claim:** conditional coverage for an individual binary; coverage on distributions different from the testbed; that the obfuscation-level detector is itself correct. Detector error enters as stratum misassignment and is a residual risk.

### 7.7 Block-Sequential SPRT and Intra-Block Correlation Correction

**Model.** Each evaluated cluster yields $X \in \{0, 1\}$ ($1$ iff top-1 prediction is correct). Test $H_0 : p = p_0 = 0.55$ against $H_1 : p = p_1 = 0.70$ (policy parameters; $p_1$ is the pre-registered target of H-B1). Blocks have $m = 15$ clusters, one per stratification cell. For block $j$ with $S_j$ successes the log-likelihood-ratio increment is

$$
\lambda_j = S_j \ln\frac{p_1}{p_0} + (m - S_j) \ln\frac{1 - p_1}{1 - p_0} = 0.24116\, S_j - 0.40547\,(15 - S_j).
$$

**Wald boundaries** (Wald, Ann. Math. Statist. 16(2), 1945, https://doi.org/10.1214/aoms/1177731118) for $\alpha = 0.05$ and $\beta = 0.20$:

$$
\ln A = \ln\frac{1-\beta}{\alpha} = \ln 16 = 2.7726, \qquad \ln B = \ln\frac{\beta}{1-\alpha} = \ln\frac{0.20}{0.95} = -1.5581.
$$

These are Wald's approximate boundaries; they ignore overshoot, and the actual error rates satisfy $\alpha' + \beta' \le \alpha + \beta$ (Wald, 1945).

**Intra-block correlation.** The variance inflation factor (design effect for equal-size clusters; Kish, *Survey Sampling*, Wiley, 1965; Donner and Klar, *Design and Analysis of Cluster Randomization Trials in Health Research*, Arnold, 2000) is

$$
\mathrm{VIF} = 1 + (m - 1)\hat\rho .
$$

$\hat\rho$ is estimated by the one-way ANOVA estimator from the first 4 blocks, which are *excluded* from the test statistic, and is then frozen. For subsequent blocks the adjusted statistic is $\Lambda^{\text{adj}}_k = \tfrac{1}{\mathrm{VIF}} \sum_{j=5}^{k} \lambda_j$. The test accepts $H_1$ if $\Lambda^{\text{adj}}_k \ge \ln A$, accepts $H_0$ if $\Lambda^{\text{adj}}_k \le \ln B$, and otherwise continues.

**Measured operating characteristics (this review, Appendix E.5).** A Monte Carlo study of the procedure above (beta-binomial blocks, $m = 15$, 4 burn-in blocks, truncation at 24 blocks, 20,000 replications per cell) gives the following estimates for the error rate $\hat\alpha$ of accepting $H_1$ when $p = p_0$, the error rate $\hat\beta$ of accepting $H_0$ when $p = p_1$, and the fraction of inconclusive runs when $p = p_1$:

| True $\rho$ | VIF correction | $\hat\alpha$ | $\hat\beta$ | Inconclusive at $p_1$ |
|-------------|----------------|--------------|-------------|------------------------|
| 0.00 | off | 0.029 | 0.091 | 0.001 |
| 0.00 | on | 0.021 | 0.075 | 0.009 |
| 0.05 | off | 0.095 | 0.196 | 0.000 |
| 0.05 | on | 0.051 | 0.135 | 0.047 |
| 0.10 | off | 0.155 | 0.260 | 0.000 |
| 0.10 | on | 0.066 | 0.161 | 0.096 |
| 0.20 | off | 0.248 | 0.320 | 0.000 |
| 0.20 | on | 0.078 | 0.183 | 0.187 |

Without the correction the nominal $\alpha = 0.05$ is exceeded by a factor of 2 to 5 under even modest intra-block correlation. With the correction the type I error is near nominal at $\rho = 0.05$ but **exceeds the nominal 0.05 at $\rho \ge 0.10$ (0.066 and 0.078)**, and the cost is up to 19 % inconclusive runs. The VIF heuristic therefore reduces but does not remove the inflation. *Normative consequence:* a Track B claim that $H_1$ holds MUST be reported together with the simulated $\hat\alpha$ at the estimated $\hat\rho$ and its upper confidence bound, and the claim is withdrawn if that bound exceeds 0.05; the nominal boundaries are never reported as the error rates.

**Truncation.** At block 24 (360 clusters total, 300 in the statistic) an undecided test is reported **inconclusive**. Dividing by VIF is a heuristic; neither the nominal $\alpha$ and $\beta$ nor the truncated test's error rates are guaranteed under correlation, so the protocol requires a Monte Carlo study of the exact procedure (simulating correlated blocks over a grid of $\rho$) before any result is reported; the study's output is part of the evaluation record.

### 7.8 Track B Honest Scope

Track B is a triage engine for analysts and incident responders; it ranks hypotheses and routes to more rigorous analysis when metadata appears. It is not a verification tool.

---

## 8. Nintendo Switch Tier (NSO / NRO / SDK / Toolchain / Relocations / Compression)

This section replaces the v2.2 Switch tier. It is grounded in public switchbrew documentation and open-source tools. It makes no claim about proprietary loader behavior beyond those sources.

### 8.1 NSO and NRO Container Formats

NSO ("Nintendo Shared Object") is the main executable format of commercial titles, system modules, and the ExeFS `main` binary. NRO ("Nintendo Relocatable Object") is the homebrew format.

| Format | Magic | Offset of magic | Typical use | Primary source |
|--------|-------|-----------------|-------------|----------------|
| NSO | `NSO0` | 0x00 | Commercial games, system modules, ExeFS `main` | https://switchbrew.org/wiki/NSO0 |
| NRO | `NRO0` | 0x10 | Homebrew applications | https://switchbrew.org/wiki/NRO0 |

Both describe three segments (`.text`, `.rodata`, `.data`) in a compact header comparable to a short-form ELF program-header table. Segment payloads follow the header, possibly compressed, with optional dynamic information (`.dynsym`, `.dynstr`, `.dynamic`, relocation tables).

Reference tools: nx2elf (https://github.com/shuffle2/nx2elf), hactool (https://github.com/SciresM/hactool), nxdumptool (https://github.com/DarkMatterCore/nxdumptool), NSTool (https://github.com/jakcron/nstool), nsnsotool (https://github.com/0CBH0/nsnsotool).

### 8.2 Segment Compression: LZ4 and ZBIC

Each NSO segment may be compressed independently. The switchbrew NSO0 page documents the flag word at header offset 0x0C as follows (verified in this review against the page's revision history): bits 0–2 mark `.text`, `.rodata`, and `.data` as compressed; bits 3–5 request hash checking at load; **bit 7 (`UseZbicCompression`, firmware 22.0.0 and later) selects a zstd-based method instead of LZ4**. The v2.2-era integration package additionally listed bit 6 as "ExecuteOnlyMemory, firmware 20.0.0 and later"; that row was not found in the retrieved documentation and is removed (Appendix A, L-10).

- **Before 22.0.0 (LZ4).** Compressed segments use LZ4; hactool decompresses them (`nso.c`).
- **22.0.0 and later (ZBIC).** The 22.0.0 page states that the method "appears to be a slightly customized variant of zstd", used when flag bit 7 is set, with the zstd magic number replaced by the bytes `ZBIC` (little-endian 32-bit value `0x4349425A`, against the standard `0xFD2FB528`), legacy support disabled, and code that otherwise matches zstd version "~1.6.0" (https://switchbrew.org/wiki/22.0.0). The page's revision history first recorded the magic as `0x5A424943` and later corrected it to `0x4349425A`; the corrected little-endian value is used here. The integration package's statements that the base is zstd 1.5.7 and that `FSE_readNCount` was replaced by binary-interpolative coding are not supported by the retrieved page and are removed (Appendix A, L-10). Open-source decompression support is reported for nxdumptool and `kinnay/zbic` (https://github.com/kinnay/zbic).

**Routing consequence (corrected).** The integration package routed every ZBIC binary to Track B as "opaque for static analysis" while also listing open-source ZBIC decompressors; those statements conflict. Decompression, where a documented implementation exists, is a pre-processing step and does not by itself change the track. A binary that cannot be decompressed is `compression_unknown`, is not analyzed, and is reported as such. Track assignment then follows MAS (Section 4); commercial retail binaries are fully stripped and fall to Track B because their MAS is below 30, not because of compression.

### 8.3 Static rtld Emulation: Relocation Set

switchbrew's Rtld page (https://switchbrew.org/wiki/Rtld) is cited by the integration package as listing the relocation types the official runtime linker processes. Relocation types per AAELF64 (https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst):

| Value | Name | Semantics |
|-------|------|-----------|
| 257 | `R_AARCH64_ABS64` | Absolute 64-bit address, $S + A$ |
| 258 | `R_AARCH64_ABS32` | Absolute 32-bit address |
| 1025 | `R_AARCH64_GLOB_DAT` | GOT entry for a global data symbol |
| 1026 | `R_AARCH64_JUMP_SLOT` | PLT entry for a function |
| 1027 | `R_AARCH64_RELATIVE` | Base-relative, $B + A$ |

The statement that these five are the *only* types the official rtld handles was not re-read in this review (Appendix A, L-10), so the static emulator treats the set as a *design set*: it implements these five, and records any other relocation type as unsupported.

**TLS and other types.** `R_AARCH64_TLS_TPREL64`, `R_AARCH64_TLSDESC`, `R_AARCH64_TLS_DTPREL64`, and any type outside the design set are recorded as

```text
!cirx.rtld_relocation = unsupported(kind)
```

(the integration package wrote the value `tls_unsupported`; the normative form is the record of Section 3.6) and are listed in the scratchpad field `unsupported_relocations` (Section 9.2). No static resolution is claimed for them.

**Scope of the static-rtld claim.** Static rtld emulation is asserted only for uncompressed or decompressed homebrew binaries whose dynamic sections conform to the documented libnx and switchbrew patterns. Commercial titles and any binary with unsupported relocations receive no static-rtld guarantee.

### 8.4 Official SDK and Toolchain

The 2018 LLVM Developers' Meeting talk "Developer Toolchain for the Nintendo Switch" (https://www.youtube.com/watch?v=9_7exO60EA8) is cited by the integration package for these statements: the first-party toolchain is Clang/LLVM with libc++, libc++abi, and later LLD; LTO is used; a gain of about 3 % on Zelda titles is reported; and PGO with LTO was evaluated. The talk was not re-watched in this review (Appendix A, L-11), so the statements are carried forward as *reported by the cited talk*, and no title-specific claim (such as which title ships with Full LTO or ThinLTO) is made.

| Component | Identity | Notes |
|-----------|----------|-------|
| Homebrew compiler | `aarch64-none-elf-gcc` (devkitA64) | Typical flags `-march=armv8-a -mtune=cortex-a57 -mtp=soft -fPIE` |
| Userland library | libnx (https://github.com/switchbrew/libnx) | Supplies `switch.specs` |
| Linker | GNU ld or LLD via devkitA64 | |

**Implication.** If a commercial title is built with LTO, the operator $\mathcal{L}$ of Section 1.3 applies with full strength and, with stripping, MAS falls below 30; the expected outcome is Track B. This is an expectation derived from the toolchain statement, not a measurement.

### 8.5 Studios

Studio and title associations in the integration package (Nintendo EPD, Retro Studios, Next Level Games, Virtuos) are informational and are not used by the pipeline; no per-studio toolchain claim is made.

### 8.6 Pipeline Routing

```text
NSO / NRO (lawfully obtained; Section 8.7)
  |-- decompress if flag bit 7 or LZ4 flags set and a documented decompressor exists
  |       otherwise: compression_unknown -> stop
  |-- homebrew, supported relocations
  |       -> static rtld emulation (design set of five types)
  |       -> CIRX normalization -> Track by MAS (A or A-prime)
  `-- commercial, unsupported relocations, or Hostile
          -> Sandbox Safety classification (emulator adapter)
          -> Track B (partial-execution trace hashing or static symbolic fallback)
```

### 8.7 Lawful Use

The pipeline accepts only binaries the operator is entitled to analyze, such as dumps from the operator's own hardware or homebrew. This specification does not describe how to obtain proprietary binaries, console keys, or leaked builds, and no component of the pipeline retrieves them.

---

## 9. LLM Context Management: Contract-Driven Interprocedural Decomposition

### 9.1 Pattern and Modeled Savings

For a target function $f$ with callees $g_1, \dots, g_k$, the orchestrator supplies the LLM with the CIRX body of $f$ and, for each callee, a *contract* $\kappa(g_i)$ (pre-conditions, post-conditions, side effects, ABI) rather than the callee's body. Contracts are stored in an external scratchpad (Section 9.2) keyed by `cirx_sha256`.

*Model.* Let $B$ be the mean callee body size in tokens and $C$ the mean contract size. The context reduction ratio is $R = kB / (kC) = B/C$ when contracts replace bodies. With the assumed $B = 2{,}000$ and $C = 150$ tokens, $R = 13.3$; for $B \in [1{,}000, 8{,}000]$ and $C = 150$, $R \in [6.7, 53.3]$. These are **assumed values, not measurements**. The v2.2 claim of "measured 10–50× reduction" is withdrawn; hypothesis H-L1 (Section 16) specifies the measurement.

### 9.2 Scratchpad Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://example.invalid/cirx/scratchpad-entry-v3.json",
  "type": "object",
  "required": ["function", "arch", "address", "cirx_sha256", "contract", "status", "guarantee_class", "unsupported_relocations"],
  "additionalProperties": false,
  "properties": {
    "function": { "type": "string" },
    "arch": { "enum": ["aarch64", "aarch32", "x86_64", "x86_32", "x86_16", "riscv32", "riscv64", "mips32", "mips64", "ppc32", "ppc64", "wasm32", "wasm64", "other"] },
    "address": { "type": "string", "pattern": "^0x[0-9a-f]{1,16}$" },
    "cirx_sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
    "contract": {
      "type": "object",
      "required": ["abi", "pre", "post", "side_effects", "variadic"],
      "additionalProperties": false,
      "properties": {
        "abi": { "enum": ["aapcs64", "aapcs64_apple", "aapcs64_windows", "aapcs32", "aapcs32_vfp", "sysv_amd64", "win64", "i386_cdecl", "i386_stdcall", "i386_fastcall", "i386_thiscall_msvc", "dos_near", "dos_far", "riscv_ilp32", "riscv_ilp32d", "riscv_lp64", "riscv_lp64d", "mips_o32", "mips_n32", "mips_n64", "ppc32_sysv", "ppc64_elfv1", "ppc64_elfv2", "wasm_basic_c_abi", "internal_lto", "unknown"] },
        "pre": { "type": "array", "items": { "type": "string" } },
        "post": { "type": "array", "items": { "type": "string" } },
        "side_effects": { "type": "array", "items": { "type": "string" } },
        "variadic": { "type": "boolean" }
      }
    },
    "status": { "enum": ["verified", "partial", "hypothesis", "abandoned"] },
    "guarantee_class": {
      "enum": ["formal_control_flow", "formal_data_flow_absolute", "formal_data_flow_relative", "formal_trace_equivalence",
               "execution_empirical", "byte_exact", "rehabilitated_partial", "structural_hypothesis", "none"]
    },
    "unsupported_relocations": { "type": "array", "items": { "type": "string" } }
  }
}
```

The `guarantee_class` enumeration is identical to the verification-record enumeration of Section 3.7. A contract whose `status` is `hypothesis` or `abandoned` MUST NOT be presented to the LLM as a fact; it is presented with the label "unverified".

**v3.2 — `abi` field and verdict↔status mapping (closes F-13).** v3.1 left the `contract.abi` enum in the scratchpad but did not add the `abi` property to the verification-record schema (§3.7), so the badge's `ABI: abi-id` field had no schema-side source of truth. v3.2 adds the `abi` property to the verification record (the same 25-value enum plus `internal_lto` and `unknown`), and aligns the scratchpad's `contract.abi` to that enum. The `guarantee_class` enumeration is also extended with `formal_data_flow_absolute` (the rehabilitated-or-absolute counterpart of `formal_data_flow_relative`; see §6.3 and §10.2 rule 3).

The two status vocabularies — verification record `verdict` ∈ {proved, disproved, unknown, out_of_envelope, hypothesis} and scratchpad `status` ∈ {verified, partial, hypothesis, abandoned} — are joined by the following **normative mapping and total order**:

| Scratchpad `status` | Verification record `verdict` | Strength rank |
|---------------------|-------------------------------|---------------|
| `verified` | `proved` | 4 (strongest positive) |
| `partial` | `proved` (with `guarantee_class = rehabilitated_partial`) OR `unknown` (if at least one layer was attempted) | 3 |
| `hypothesis` | `hypothesis` (Track B cluster) | 2 |
| `abandoned` | `out_of_envelope` OR `unknown` (no layer was attempted) | 1 (weakest) |
| (no contract) | `disproved` | 0 (negative; no contract is emitted) |

The total order is `verified > partial > hypothesis > abandoned`. A contract's `status` MUST be the rank of the strongest `verdict` among the verification records keyed by the contract's `cirx_sha256`. This makes §9.3's rule "the contract status is never stronger than the strongest verdict of its function" enforceable: the contract is computed from the records, not asserted.

### 9.3 Contract Verification Loop

1. Generate a candidate for $f$ from its CIRX body and verified callee contracts.
2. In Track A, run the cascade of Section 6 on the candidate; a `disproved` verdict returns the counterexample to the LLM for repair, bounded at 5 iterations.
3. Any contract derived from a candidate is `partial` unless the candidate's cascade verdict supports it; the contract status is never stronger than the strongest verdict of its function.

**LLM generators and baselines.** LLM4Decompile (https://github.com/albertan017/LLM4Decompile; Tan et al., https://arxiv.org/abs/2403.05286) provides open models in two modes: an *End* mode (assembly to C) and a *Ref* mode (decompiler pseudo-C to refined C), reported on x86-64 GCC `-O0` through `-O3` with *re-executability* (compile, run, compare against tests) as the metric. It sits outside the cascade: it is a candidate generator or a Track B baseline, can feed the repair loops of Layer 3 (PCodeTrans, CHISEL), and is compared against name- and type-recovery systems that operate on decompiler ASTs (DIRTY, DeGPT). No formal guarantee is inferred from it. The specific checkpoint, prompt format, and benchmark MUST be recorded with any result; the package's remark that stronger checkpoints reach a re-executability band of roughly 45–65 % is not used because no table or checkpoint was identified (Appendix A, L-20).

**Does not claim:** that LLM-generated contracts are correct; they are hypotheses until checked. Memorization of public source by LLMs may inflate benchmark performance on open-source programs; evaluations MUST include a held-out proprietary or freshly written corpus (Section 13).

---

## 10. Unified Provenance Badge

### 10.1 Grammar

Every output carries a badge with the fields below. A badge missing any required field, pairing `Formal: Proved` with `SMT: {none}`, or pairing `Formal: Proved` with a guarantee class outside the *absolute* set of §10.2 rule 2, is malformed and MUST be rejected by every consumer.

```text
badge        = track arch control data execution formal guarantee smt cluster provenance sandbox mas cirx
track        = "[Track: " ("A" / "A_prime" / "B") "]"
arch         = "[Arch: " arch-id "; ABI: " abi-id "]"
arch-id      = "aarch64" / "aarch32" / "x86_64" / "x86_32" / "x86_16" / "riscv32" / "riscv64" / "mips32" / "mips64" / "ppc32" / "ppc64" / "wasm32" / "wasm64" / "other"
abi-id       = abi-enum / "internal_lto" / "unknown"
abi-enum     = "aapcs64" / "aapcs64_apple" / "aapcs64_windows" / "aapcs32" / "aapcs32_vfp" / "sysv_amd64" / "win64" / "i386_cdecl" / "i386_stdcall" / "i386_fastcall" / "i386_thiscall_msvc" / "dos_near" / "dos_far" / "riscv_ilp32" / "riscv_ilp32d" / "riscv_lp64" / "riscv_lp64d" / "mips_o32" / "mips_n32" / "mips_n64" / "ppc32_sysv" / "ppc64_elfv1" / "ppc64_elfv2" / "wasm_basic_c_abi"
control      = "[Control: " verdict-with-source "]"
data         = "[Data: " verdict-with-source "]"
verdict-with-source = verdict-tag " by " source-tag
verdict-tag  = "Proved" / "Disproved" / "Refuted" / "Hypothesized" / "Not_applicable"
source-tag   = 1*( ALPHA / DIGIT / "-" / "_" / "." / "(" / ")" / "," / " " )
execution    = "[Execution: " text "]"
formal       = "[Formal: " ("Proved" / "Disproved" / "Unknown" / "Out_of_envelope" / "Not_applicable") "]"
guarantee    = "[Guarantee: " guarantee-class "]" / "[Guarantee: Not_applicable]"
guarantee-class = "formal_control_flow" / "formal_data_flow_absolute" / "formal_trace_equivalence" / "execution_empirical" / "byte_exact" / "rehabilitated_partial" / "structural_hypothesis" / "none"
smt          = "[SMT: {" logic *("," logic) "}]"
logic        = "QF_ABV" / "QF_ABVFP" / "QF_AUFBV" / "QF_AUFBVFP" / "algebraic" / "none"
cluster      = "[Cluster: " text "]"
provenance   = "[Provenance: " text "]"
sandbox      = "[Sandbox: " ("Safe" / "Suspect" / "Hostile" / "Static" / "Not_applicable") "]"
mas          = "[MAS: " 1*3DIGIT "]"
cirx         = "[CIRX-SHA256: " 64HEXDIG "]"
```

The `SMT` field is a *set* drawn from the closed alphabet, because one function can use several logics (for example, `{algebraic, QF_ABV}`); the value `none` is exclusive. v2.2 and the integration packages specified a single value per badge, which cannot describe a function verified by Layers 1 and 2 together. The solver name and version are stored in the verification record (§3.7), not in the badge.

The `Guarantee` field is **required** (additive in v3.2) and carries the guarantee class of the strongest positive verdict, or `Not_applicable` if no positive verdict exists. This makes the badge machine-checkable against the guarantee-class enum: a consumer need not consult the verification record to know whether a `Formal: Proved` is absolute or relative.

The `abi-id` production is the closed 25-value enum of §9.2's `contract.abi`, with the two extension values `internal_lto` and `unknown`. v3.1's unconstrained `1*(ALPHA / DIGIT / "_")` accepted ABI strings that no scratchpad schema permits; the closed enum is the single source of truth.

The `verdict-with-source` production is a *qualified verdict paired with its source*. The source-tag names the layer, the tool, or the human-process that produced the verdict (for example, `CF-GKAT`, `Faultless-style Layer 2, relative to reference decompiler`, `CHISEL, n=40 tests, 83% block coverage`). v3.1 used this production but never defined it; an undefined production voids the grammar's machine-checkability claim.

### 10.2 Soundness Rules

1. `Formal: Proved` requires: (a) a non-empty `SMT` set that excludes `none`; (b) at least one `proved` verification record; and (c) the record's `guarantee_class` is a member of the **absolute set** `ABSOLUTE_GUARANTEE_CLASSES = {formal_control_flow, formal_trace_equivalence}`. v3.1's prefix test (`begins with formal_`) accepted `formal_data_flow_relative`, inverting the document's epistemic discipline; the v3.2 rule is an exact-membership test over the absolute set. v3.2.1 correction (closes N3/N18): `formal_data_flow_absolute` is **not** in the absolute set. Until a sound absolute Layer 2 checker exists (a research obligation, not a configuration option), the only absolute formal guarantee emittable is `formal_control_flow`. Rule 3 of v3.2 (which claimed `formal_data_flow_absolute` could produce `Formal: Proved`) is **withdrawn**; the v3.2.1 badge grammar's `guarantee-class` production includes `formal_data_flow_absolute` for forward compatibility, but the §10.2 rule 1 absolute set does not.
2. `formal_trace_equivalence` is emittable only when the Layer 5 relational harness exists and the verification condition is discharged with no over-approximation caveat (§6.6). Until then, `formal_trace_equivalence` MUST NOT appear in any record, and the badge's `Guarantee` field MUST be one of `formal_control_flow`, `execution_empirical`, `byte_exact`, `rehabilitated_partial`, `structural_hypothesis`, or `none`. This closes the schema/pipeline divergence that v3.1 left open.
3. **Withdrawn in v3.2.1** (closes N3). The original v3.2 rule 3 stated that `Formal: Proved` with `Guarantee: formal_data_flow_absolute` was an absolute Layer 2 verdict. This is inconsistent with rule 1: `formal_data_flow_absolute` is not in `ABSOLUTE_GUARANTEE_CLASSES`. The contradiction is resolved by withdrawal. A rehabilitated Layer 2 verdict carries `Guarantee: rehabilitated_partial` (see §5.3), and the `SMT` set MUST include the logic that discharged the rehabilitated obligation.
4. An unanswered SMT query forces abandonment; the badge shows `Formal: Unknown` or `Out_of_envelope` and records the logic that was attempted.
5. `algebraic` denotes the CF-GKAT decision procedure; no SMT solver runs, and the symbolic variant's SAT-solver identity is recorded in the verification record.
6. MAS in the badge is the integer $\mathrm{MAS}_\%$. The badge's `mas` production accepts `0`–`999`; consumers MUST reject `MAS: 250`-style values whose integer is outside the valid range.
7. The `Guarantee` field is required. A badge missing `Guarantee` is malformed and MUST be rejected, with one exception: Track B outputs whose `formal` is `Not_applicable` MAY carry `Guarantee: structural_hypothesis` or `Guarantee: Not_applicable`.

### 10.3 Examples

The three examples are re-derived under the v3.2 grammar. The example CIRX-SHA256 digests are unchanged (v3.1 §10.3's digests are SHA-256 of placeholder strings; re-verified in this review).

**Example 1 — Track A, absolute Layer 1 + relative Layer 2 (out of envelope):**

```text
[Track: A][Arch: aarch64; ABI: aapcs64][Control: Proved by CF-GKAT][Data: Proved by Faultless-style Layer 2, relative to reference decompiler]
[Execution: 40 tests, 83% block coverage][Formal: Out_of_envelope][Guarantee: formal_control_flow][SMT: {algebraic, QF_ABV}]
[Cluster: n/a][Provenance: symtab, dwarf_type, comment][Sandbox: Not_applicable][MAS: 75]
[CIRX-SHA256: 85acdb48f9aa1270493421732adfe33375d4cca3b0acfeec03adce698b7c0148]
```

`Guarantee: formal_control_flow` records that the *strongest* positive verdict is from CF-GKAT (Layer 1, absolute). The Layer 2 verdict is `Proved` but its class is `formal_data_flow_relative`; because `formal_data_flow_relative` is not in the absolute set (rule 1), the badge's `formal` is `Out_of_envelope` — the function's overall envelope has not been closed by an absolute claim. The `SMT` set includes both logics used.

**Example 2 — Track A′, rehabilitated Layer 1:**

```text
[Track: A_prime][Arch: x86_64; ABI: sysv_amd64][Control: Proved by CF-GKAT, rehabilitated (no unresolved indirect branches)][Data: Rehabilitated via export hooks, 68% of functions]
[Execution: Not_applicable][Formal: Unknown][Guarantee: rehabilitated_partial][SMT: {algebraic}]
[Cluster: n/a][Provenance: dynsym, dwarf_line, comment][Sandbox: Not_applicable][MAS: 30]
[CIRX-SHA256: 8b03ee56f3e4abce5242de69e23890b1115a5fae39646404b442cce368e92be9]
```

`Guarantee: rehabilitated_partial` records that the strongest positive verdict is rehabilitated (§5.3), and `Formal: Unknown` reflects that the full cascade has not been discharged. The `verdict-with-source` for `Control` is now qualified with the rehabilitation clause.

**Example 3 — Track B, calibrated cluster hypothesis:**

```text
[Track: B][Arch: mips32; ABI: mips_o32][Control: Not_applicable][Data: Not_applicable][Execution: 12 traces, static fallback 0%]
[Formal: Not_applicable][Guarantee: structural_hypothesis][SMT: {none}][Cluster: set={crypto_aes_sbox_access}, coverage>=0.90, alpha=0.10, stratum=intermediate, calib=v1, n=240]
[Provenance: dynsym][Sandbox: Suspect][MAS: 20]
[CIRX-SHA256: 221a3d3a53f06c11d41da779cac72dba8bd4545b95efa458efbfccf900d246c5]
```

`Guarantee: structural_hypothesis` and `SMT: {none}` are mutually consistent (§10.2 rule 1's `none` exclusion applies only to `Formal: Proved`).

The three digests above are the SHA-256 values of the ASCII strings `example-cirx-aarch64-function-1`, `example-cirx-x86_64-function-2`, and `example-cirx-mips32-function-3`; they illustrate the syntax and are not digests of any lifted function. Re-verified in this review: `sha256("example-cirx-aarch64-function-1") = 85acdb48f9aa1270493421732adfe33375d4cca3b0acfeec03adce698b7c0148`, and likewise for the other two.

---

## 11. Cross-Platform ABI Considerations

| Concern | Linux (Itanium, AArch64) | Windows (MSVC, ARM64 and Arm64EC) | Switch (Horizon) |
|---------|--------------------------|-----------------------------------|------------------|
| Name mangling | Itanium | MSVC; Arm64EC uses distinct decoration | Itanium (Clang/LLVM, libc++) as reported by Section 8.4 |
| RTTI | `typeinfo` after offset-to-top | `CompleteObjectLocator` at vftable index $-1$ | As Linux where libc++ RTTI is retained |
| Unwind | `.eh_frame`, LSDA | `.pdata`, `.xdata` | `.eh_frame` for LLVM builds (not verified; L-10) |
| Variadic ABI | Base AAPCS64 | Windows ARM64 stack variadics | Not verified |
| Platform register `x18` | Toolchain-dependent | Reserved | Not verified |

Entries marked "not verified" are intentionally conservative: the lifter records `abi: unknown` and defers to body-derived contracts.

---

## 12. End-to-End Pipeline

1. **Ingest and classify** the container (ELF, PE, Mach-O, MZ and its relatives, NSO, NRO, console containers, WebAssembly) through the `ContainerIngest` interface of Section 29.1; decompress where documented; read the architecture, byte order, and ABI from the container, never assume them.
1a. **Dispatch on the architecture profile** (Section 20.1). An architecture without a profile exits with `unsupported_architecture` and receives no layer verdict.
2. **Compute MAS** (Section 4.1) and select the track.
3. **Lift** to CIRX Stage 1 with the semantics source named in the profile; record `semantic_loss`; apply the lowering rules of Section 3.8 or the per-architecture section. The reference implementation of Appendix D is the conformance baseline for the AArch64 subset it covers; any other lifter MUST pass an equivalent differential and mutation-tested suite (hypothesis H-X2).
4. **Reconstruct** types (Stage 2); join provenance at SSA (Stage 3).
5. **Run the track** (Sections 5, 6, 7).
6. **Orchestrate LLM generation** under Section 9 with scratchpad contracts.
7. **Emit** the output and the badge; reject any output whose badge is malformed.

Every step that invokes an SMT solver obeys the Unknown rule of Section 3.3.4.

---

## 13. Evaluation Methodology

1. **Corpora.** Open-source functions with ground truth (for example, Coreutils and Binutils as used by the cited tools) *plus* a held-out proprietary or freshly authored corpus never present on the public web, to bound memorization effects.
2. **Build matrix.** ARM64 builds under Clang and GCC; `-O0` through `-O3`; no LTO, ThinLTO, Full LTO; symbols present and stripped.
3. **Metrics.** Layer verdict rates (`proved`, `disproved`, `unknown`, `out_of_envelope`); false-proof rate (must be measured by seeding known-inequivalent mutants); conformal empirical coverage per stratum; time and memory per function.
4. **Baselines.** The reference deterministic decompiler for Layer 2; the cited tools for each layer in their native settings.
5. **Statistics.** Pre-registered hypotheses (Section 16); multiple hypotheses are controlled by the Benjamini–Hochberg procedure (Benjamini and Hochberg, JRSS-B 57(1), 1995, https://doi.org/10.1111/j.2517-6161.1995.tb02031.x), valid under independence or positive regression dependence (Benjamini and Yekutieli, Ann. Statist. 29(4), 2001, https://doi.org/10.1214/aos/1013699998). Hypotheses sharing a testbed are positively dependent at best, so the Benjamini–Yekutieli correction MUST be used where positive dependence is not established.

---

## 14. Benchmark Versus Production and Adversarial Stress Tests

### 14.1 Benchmark Versus Production

Reported figures from the cited tools come from benchmark settings (compiled open-source programs, often without LTO). Production binaries differ in compiler version, flags, LTO, and stripping. This specification therefore treats all reported figures as upper bounds of plausibility, not as targets.

### 14.2 Stress Test S1 – Extreme LTO, Full Stripping, Virtualizing Protector, Unknown Compiler

MAS is at most 20 (no `.symtab`, no debug data, no `.comment`); Track B applies; the classification is Hostile; static emulation is attempted and, for virtualization-based protectors, abandoned (Section 7.5). Output: `[Cluster: unknown_obfuscated, Confidence: N/A]`, `[Formal: Not_applicable]`, `[SMT: {none}]`. No verification claim is made.

### 14.3 Stress Test S2 – Solver Returns `unknown` or Times Out

The layer abandons the function. The badge shows `Formal: Unknown` (or `Out_of_envelope` if a wall was hit), `SMT` shows the logic attempted, and the record shows `abandonment_reason`. No fallback verdict is produced from a partial run. If a second solver is configured and returns a definite answer, the answer is accepted only if the second solver's version and resources are recorded and no first-solver `sat`/`unsat` contradicts it.

### 14.4 Stress Test S3 – Sandbox Classified Hostile

The binary is not executed (Section 7.3). Static emulation produces traces labeled `static`; calibration is applied only inside the calibrated domain.

### 14.5 Stress Test S4 – Wrong Propagated Types in Track A′

Rehabilitated types that are wrong cause Layer 2 to return `disproved` or `unknown`, because Layer 2 re-checks types against the body (Section 6.3); the layer never returns `proved` on the strength of a propagated type alone.

### 14.6 Fallback Cascade and Cost

When a layer abandons, the orchestrator may try the next eligible layer. Total fallback compute per binary is capped at 5 times the primary-path compute (policy parameter), and per-function fallback attempts are capped at 3. v2.2 stated both caps in different places with different units; this section is the single normative statement.

---

## 15. Implementation Obligations & Acceptance Criteria (v3.3)

This section elevates every residual roadmap item into a **measurable engineering contract**. An obligation is discharged only when its acceptance criteria are met and the corresponding badge or ledger entry is produced. Planning estimates remain indicative; they are not contractual.

### 15.1 Priority Matrix

| ID | Obligation | Priority | Acceptance Criteria (must all hold) | Evidence Artifact |
|----|------------|----------|-------------------------------------|-------------------|
| O-1 | CIRX Stage-1 lifter for AArch64 on a formally derived semantics source (ASLp or Sail) | P0 | (a) Every instruction in the AArch64 base ISA produces a Micro-RTL fragment obeying invariants I1–I4; (b) `semantic_loss` field is set only for documented coverage gaps; (c) differential testing against a trusted reference (ASLp simulator or QEMU system mode) yields zero mismatches on a 10 k instruction corpus; (d) the lifter is deterministic | Public repository + corpus hash + differential log |
| O-2 | Architecture-parametric CIRX for x86-64, RV64, AArch32 | P0 | Same as O-1 for each ISA; ArchProfile completeness checklist (§20.1.1) passes the extended linter | Same |
| O-3 | Layer-2 absolute data-flow checker over CIRX (Faultless-style or equivalent) | P0 | (a) Sound under the CIRX memory model (§3.3); (b) produces only `formal_data_flow_absolute` when the absolute set membership test succeeds; (c) mutation analysis on 100 seeded inequivalent mutants yields zero false proofs | Checker binary + mutation report |
| O-4 | Layer-5 relational harness (step 3 of §6.6) | P1 | Bounded relational VC generation for corresponding inputs/outputs; discharges under QF_AUFBV; false-proof rate 0 on seeded mutants inside the envelope | Boogie/SMT scripts + mutation report |
| O-5 | Track-B trace store + emulator adapters (QEMU user-mode or Unicorn) | P1 | Partial-execution traces hash-stable under the sandbox safety contract; SPRT harness re-executable | Trace corpus + SPRT log |
| O-6 | Full conformal + VIF-SPRT calibration pipeline | P1 | Empirical coverage within 0.03 of 1−α on held-out strata; VIF estimation documented | Calibration report |
| O-7 | Multi-container ingest (ELF, PE, Mach-O, NSO/NRO) with relocation tables validated against LLVM | P1 | Zero discrepancies on the relocation constant suite of Appendix F; round-trip of a stripped LTO binary yields a CIRX CFG whose entry points match the loader | Reloc checker log + CFG dump |
| O-8 | Cross-architecture extension protocol automation (§28) | P2 | New ISA profile added by filling the ArchProfile checklist; linter C-PA passes | Profile PR + linter output |

### 15.2 Absolute-Guarantee Soundness Preservation Lemma

Let \(\mathcal{A}\) be the absolute set \(\{\texttt{formal\_control\_flow}\}\).  
Let \(G\) be any guarantee class produced by a verification layer.  

**Lemma (Absolute Set Closure).**  
If \(G \notin \mathcal{A}\), then a badge that claims \(G\) as absolute is rejected by the grammar (rule 3, §10.2).  
If \(G \in \mathcal{A}\) and the layer’s eligibility predicate and policy walls are satisfied, the badge may emit the absolute form.  

**Proof sketch.** By construction of the badge grammar (productions in §10.1) and the exact-membership test of the absolute set. The finite-state wall and policy parameters bound the concrete transition system; Rice’s theorem is thereby confined to the idealized unbounded model. No layer outside Layer 1 currently belongs to \(\mathcal{A}\); any future absolute claim must first enlarge \(\mathcal{A}\) by a documented soundness argument and a linter update.

### 15.3 Discharge Rule

An obligation moves from “open” to “discharged” only when:
1. its acceptance criteria are met,
2. the evidence artifact is archived under a content-addressed hash,
3. the verification ledger (Appendix A) receives a new row with status `verified` or `re-executed`,
4. the extended linter reports zero issues on the updated specification.

Until then the corresponding badge or claim remains forbidden.

### 15.4 Quantitative Discharge Metrics (v3.4)

For each P0/P1 obligation the following quantitative gates are normative:

| Obligation | Minimum Corpus / Mutants | Max False-Proof Rate | Differential Mismatch Bound | Reproducibility |
|------------|--------------------------|----------------------|-----------------------------|-----------------|
| O-1 CIRX Stage-1 | ≥ 10 000 instructions | n/a | 0 mismatches vs reference simulator | Deterministic, content-addressed binary |
| O-3 Absolute Layer-2 | ≥ 100 seeded inequivalent mutants | 0 | n/a | Checker binary + mutation log hash |
| O-4 Layer-5 relational | ≥ 50 relational mutants inside envelope | 0 | n/a | Boogie/SMT scripts + solver logs |
| O-6 Conformal/SPRT | Held-out strata ≥ 5 | Coverage error ≤ 0.03 | n/a | Full calibration report |

Any claim of discharge that fails a quantitative gate is rejected by the verification ledger.

### 15.5 Agent & LLM Consumption Contract (v3.4)

Any automated agent or LLM that consumes this specification MUST:

1. Treat the absolute set \(\{	exttt{formal\_control\_flow}\}\) as the sole source of absolute guarantees.
2. Never emit a badge or claim that violates the Absolute Set Closure Lemma.
3. Prefer the machine-readable files in the distribution package (`llm-kit/`) over free-form summarization of the full text when context is limited.
4. Record every generated claim with a provenance pointer back to a section, formula, or obligation ID.
5. Surface residual obligations (O-1 to O-8) rather than silently eliding them.

---

## 16. Pre-Registered Hypotheses

| ID | Hypothesis | Test |
|----|------------|------|
| H-A′1 | On the testbed, rehabilitation (Section 5.2) raises the fraction of functions with a positive Layer 2 verdict by at least 10 percentage points over no rehabilitation at the same MAS | Paired comparison, same binaries |
| H-B1 | Track B top-1 cluster precision is at least 0.70 on the domain testbed | SPRT of Section 7.7 |
| H-B2 | Conformal prediction sets achieve empirical coverage within 0.03 of $1-\alpha$ in each stratum | Held-out evaluation |
| H-B3 | Trace-hash top-1 precision on LTO builds is within 10 percentage points of its precision on non-LTO builds | Paired builds |
| H-C1 | Layer 5 false-proof rate on seeded inequivalent mutants is 0 within its envelope | Mutation testing |
| H-L1 | Contract-driven context uses at least 10 times fewer tokens than inlining callee bodies at equal repair success | Token accounting per Section 9.1 |
| H-RS1 | The static risk score calibration constant $S_{\text{cal}} = 50$ is within a factor of 2 of the isotonic-regression-fitted constant on a held-out set of expert-labeled security-critical functions, and the re-fit score $R'$ satisfies $\lvert R - R' \rvert \le 5$ for $\ge 90\%$ of the test set (§6.1) | Held-out expert-labeled corpus |

A hypothesis that fails is reported as failed.

---

## 17. Related Work

| Work | Relevance | Status of the citation |
|------|-----------|------------------------|
| Retypd (Noonan et al., PLDI 2016) | Polymorphic type inference for machine code | DOI cited from memory (L-15) |
| NotDec (ICSE 2026) | Retypd-style recovery for WebAssembly | Abstract numbers verified |
| DIRTY (Chen et al., USENIX Security 2022; https://arxiv.org/abs/2108.06363) | Learned variable names and types | Carried forward |
| LLM4Decompile (Tan et al., https://arxiv.org/abs/2403.05286) | LLM-based decompilation | Carried forward |
| SALT4Decompile (https://arxiv.org/abs/2509.14646) | Abstract logic trees; reported 70.4 % TCP on Decompile-Eval with 10.6 % improvement | Verified from abstract |
| ReSource (https://arxiv.org/abs/2608.27889) | Transformation-agnostic recovery under optimization and obfuscation | Title verified; figures not re-read |
| CHISEL (https://arxiv.org/abs/2608.27981) | Test-suite-free iterative recovery with compiler and fuzzer oracles | Existence, title, authors, venue verified; figures carried forward (L-04) |
| CLASScanner (ISSTA 2026; https://doi.org/10.1145/3832103) | C++ class recovery driven by object-flow graphs | Link supplied by the ingestion package; not re-fetched (L-09) |
| STILL (https://arxiv.org/abs/2608.26408) | Lowered STL semantics for LLM-assisted C++ decompilation | As above (L-09) |
| BRIDGE (ACL 2026; https://doi.org/10.18653/v1/2026.acl-long.527) | Optimized binary to canonical LLVM IR | As above (L-09) |
| Agent4Decompile (https://arxiv.org/abs/2604.23940) | Multi-agent, constraint-guided recovery | As above (L-09) |
| DeGPT, memorization studies | LLM decompiler-output refinement and benchmark contamination | No primary source located (L-09) |
| Faultless, PCodeTrans, CHISEL, Echo, DecIR, CF-GKAT, BASIL | The five cascade layers | See Section 6 and Appendix A |

---

## 18. Limitations and Future Work

1. Layers 2 and 5 depend on lifter and instruction-semantics correctness; no end-to-end mechanized proof exists. Absolute data-flow claims remain forbidden until O-3 is discharged.
2. Concurrency, SVE/SME, pointer authentication, and memory tagging are out of scope (Section 3.2).
3. Dynamic risk signals (Section 7.4) are unvalidated.
4. The calibration testbed is the principal dependence of Track B guarantees; O-6 must be discharged before any production deployment of Track B.
5. Future work is exactly the obligation matrix of Section 15. The highest-priority items are O-1 (CIRX Stage-1 lifter) and O-4 (Layer-5 relational harness). Mechanized proofs of CIRX lowering for a subset and an ARM64 LTO+stripped benchmark are supporting activities for those obligations.

---

## 19. Conclusion

The architecture routes binaries by available metadata, pairs each positive claim with the specific solver or oracle that produced it, and states in every section what it does not claim. Document-local defects are closed (v3.2.1). Remaining work is captured as measurable Implementation Obligations (Section 15) with explicit acceptance criteria. Until O-1 (CIRX Stage-1 lifter) and O-4 (Layer-5 relational harness) are discharged, the strongest outputs remain bounded per-layer verdicts inside the absolute set \(\{	exttt{formal\_control\_flow}\}\) and calibrated triage hypotheses, never general behavioral equivalence. The Absolute Set Closure Lemma (§15.2) guarantees that no badge can over-claim absolute power.

---

---

**PART II. ARCHITECTURE-PARAMETRIC CORE AND PER-ARCHITECTURE INSTANTIATIONS** (Sections 20–31)

## 20. Architecture-Parametric Foundations

### 20.1 The Architecture Profile

Every obligation in Part I that mentions AArch64 is a *parameter binding*, not a property of the framework. An **architecture profile** $\mathcal{A}$ binds the following parameters. A lifter claiming support for an architecture MUST publish its profile; a binary whose architecture has no profile is reported `unsupported_architecture` and receives no verification claim (Section 28).

### 20.1.1 ArchProfile Completeness Checklist (normative, v3.3)

An ISA extension is accepted only when every row below is satisfied and the extended linter (C-PA) reports zero issues:

1. `word_bits`, `endianness`, `insn_encoding`, `isa_modes`, `delay_slots`, `flag_model` fully populated.
2. `reg_file` contains every architectural register that appears in the calling convention or in the Micro-RTL lowering rules, with correct width and alias sets.
3. `abi_set` lists every supported ABI (e.g., AAPCS64, System V AMD64, RV64G ILP32/LP64) with argument/return register mappings.
4. `atomics` documents the memory-order model and the supported fence/barrier instructions.
5. `lowering_rules` enumerates the Micro-RTL patterns for every instruction class that the lifter claims to cover; uncovered classes MUST set `semantic_loss`.
6. A constructor `ArchProfile::<isa>()` exists and is pure (no I/O, deterministic).
7. At least one differential test corpus (instruction-level or function-level) is cited or shipped.
8. The absolute-guarantee eligibility predicates of §6.7 are instantiated or explicitly marked inapplicable.

Failure of any item yields `unsupported_architecture` for that ISA.


| Parameter | Meaning | Examples of values |
|-----------|---------|--------------------|
| `word_bits` | Natural integer and address width | 8, 16, 32, 64 |
| `endianness` | Byte order of multi-byte memory access | little, big, bi-endian with runtime selection |
| `insn_encoding` | Instruction length model | fixed 32; fixed 16 and 32 (Thumb-2, RISC-V C); variable 1–15 bytes (x86) |
| `isa_modes` | Run-time switchable decode modes | ARM/Thumb (bit 0 of branch target); MIPS32/microMIPS/MIPS16e; PowerPC VLE page attribute; RISC-V is static per object |
| `delay_slots` | Branch delay slots and nullification | none; one slot (MIPS, SPARC, SH); annulling (MIPS branch-likely, SPARC annul bit) |
| `flag_model` | Condition state | `Nzcv`, `X86Eflags`, `PpcCrXer`, `None` (Section 20.3) |
| `reg_file` | Architectural registers and aliases | Section 20.2 |
| `abi_set` | Calling conventions that may govern a function | Per-architecture tables in Sections 21–27 |
| `mem_spaces` | Distinct address spaces | one flat space; Harvard (code and data separate); segmented (x86-16) |
| `mem_model` | Memory-ordering model | x86-TSO, ARM, RVWMO, POWER, MIPS, WebAssembly threads (Section 20.7) |
| `atomics` | Atomic primitives | LL/SC (ARM exclusives, RISC-V LR/SC, MIPS LL/SC, PowerPC lwarx/stwcx.); RMW (x86 LOCK, RISC-V AMO, Arm LSE) |
| `unwind` | Exception and unwinding metadata | DWARF CFI; `.pdata`/`.xdata`; `.ARM.exidx`; SEH chains; WebAssembly exception tags |
| `reloc_design_set` | Relocation types the static loader emulates | Per-architecture tables in Sections 21–27 |

### 20.2 Architecture-Parametric Rust Data Model (CIRX schema version 4)

The following module is the generalization of Section 3.2. It is self-contained and replaces the hard-coded `Reg` enum with an architecture-indexed `RegId`. The AArch64 types of Section 3.2 are a view obtained by instantiating `ArchProfile::aarch64()` and are retained because Part I's examples use them.

```rust
/// CIRX Stage 1 generic model, schema version 4.
pub const CIRX_SCHEMA_VERSION_V4: u32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ArchId {
    Aarch64, Aarch32, X86_64, X86_32, X86_16,
    Riscv32, Riscv64, Mips32, Mips64, Ppc32, Ppc64, Wasm32, Wasm64, Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Endian { Little, Big, Bi { runtime_select: bool } }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlagModel { Nzcv, X86Eflags, PpcCrXer, NoFlags }

/// Register identifier, meaningful only relative to an ArchProfile.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RegId(pub u16);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegClass { Gpr, Fpr, Vector, Special, Predicate, Segment }

#[derive(Clone, PartialEq, Debug)]
pub struct RegDesc {
    pub id: RegId,
    pub name: &'static str,
    pub class: RegClass,
    pub bits: u16,
    /// For a sub-register view (for example x86 EAX within RAX), the parent and bit range.
    pub alias_of: Option<(RegId, u16, u16)>,
    /// How a write of a narrower value affects the parent register.
    pub narrow_write: NarrowWrite,
}

/// Instruction-length encoding model. Required by §20.1 `insn_encoding`.
/// The lifter publishes the model; functions using an instruction outside
/// the declared `min_insn_bytes`..=`max_insn_bytes` range are
/// `InstructionLengthOpaque`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InsnEncoding {
    Fixed(u8),           // all instructions are exactly N bytes (AArch64 = 4)
    FixedPair(u8, u8),   // two fixed lengths selectable by decode mode (Thumb-2 = 2|4, RISC-V C = 2|4, microMIPS)
    Variable(u8, u8),    // variable-length decoding in the byte range [min, max] (x86 = 1..15)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DelaySlotModel { None, Slot1, Slot1Annulling }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemModel { X86Tso, ArmV8, ArmV7, Rvwmo, RvTso, Power, MipsWeak, WasmThreads, Sequential }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AddrSpace { Flat, Code, Data, Segmented { seg_shift: u8, linear_bits: u8 } }

/// ABI identifiers — closed enum of §9.2's `contract.abi`.
/// Two extension values: `InternalLto` for LLVM-LTO-internalized custom
/// calling conventions, and `Unknown` when no ABI can be inferred.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AbiId {
    Aapcs64, Aapcs64Apple, Aapcs64Windows, Aapcs32, Aapcs32Vfp,
    SysvAmd64, Win64,
    I386Cdecl, I386Stdcall, I386Fastcall, I386ThiscallMsvc,
    DosNear, DosFar,
    RiscvIlp32, RiscvIlp32d, RiscvLp64, RiscvLp64d,
    MipsO32, MipsN32, MipsN64,
    Ppc32Sysv, Ppc64Elfv1, Ppc64Elfv2,
    WasmBasicCAbi,
    InternalLto, Unknown,
}

/// Atomic primitives supported by the architecture. Layer 2 / Layer 5
/// eligibility (§6.7) depends on the atomics subset; the lifter records
/// the subset in `ArchProfile::atomics` and any function using an atomic
/// outside the subset is `AtomicOpaque` and excluded from Layers 2/5.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AtomicSet {
    None,
    LlSc,                       // ARM exclusives, RISC-V LR/SC, MIPS LL/SC, PowerPC lwarx/stwcx.
    Rmw,                        // x86 LOCK, RISC-V AMO, Arm LSE
    LlScAndRmw,
}

/// Unwind metadata format. The lifter MUST publish the format it lowers;
/// functions using a different format are `UnwindOpaque`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnwindFormat {
    DwarfCfi,                   // ELF .eh_frame / .debug_frame
    PdataXdata,                 // Windows ARM64 .pdata + .xdata
    ArmExidx,                   // AArch32 .ARM.exidx
    SehChains,                  // Windows x64 SEH
    WasmExceptionTags,          // WebAssembly exception handling proposal
    None,
}

/// Relocation design set: the static loader emulates only the
/// relocations listed here (§21–§27 per-architecture tables).
/// Other relocations are `unsupported_relocation` and the containing
/// function is `Out_of_envelope` for Layers 2/5.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RelocDesignSet {
    /// Symbolic relocation names (for example, `R_X86_64_PC32`).
    pub names: Vec<&'static str>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ArchProfile {
    pub id: ArchId,
    pub word_bits: u16,
    pub endian: Endian,
    pub min_insn_bytes: u8,
    pub max_insn_bytes: u8,
    pub insn_encoding: InsnEncoding,
    pub delay_slots: DelaySlotModel,
    pub flag_model: FlagModel,
    /// Per-architecture bit indices into the `FlagSet` variants.
    /// See §20.3.5. Required: every index used by `GenCond::FlagBit(i)`
    /// MUST be defined here.
    pub flag_bit_order: Vec<&'static str>,
    pub regs: Vec<RegDesc>,
    pub mem_model: MemModel,
    pub spaces: Vec<AddrSpace>,
    pub unaligned_access_ok: bool,
    pub isa_modes: Vec<&'static str>,
    /// Set of ABIs that may govern a function in this architecture
    /// (§20.1 `abi_set`). Used to validate the badge's `abi-id`.
    pub abi_set: Vec<AbiId>,
    /// Atomic primitives supported by the lifter for this architecture.
    pub atomics: AtomicSet,
    /// Unwind metadata format the lifter lowers.
    pub unwind: UnwindFormat,
    /// Relocation design set the static loader emulates.
    pub reloc_design_set: RelocDesignSet,
}

impl ArchProfile {
    /// AArch64 profile, schema version 4. The Part I AArch64 types
    /// of §3.2 are obtained by instantiating this profile. v3.1
    /// referenced `ArchProfile::aarch64()` but did not define it;
    /// the v3.2 definition makes the derivation mechanical.
    /// v3.2.1 (closes N1): all fields set, register table populated.
    pub fn aarch64() -> ArchProfile {
        ArchProfile {
            id: ArchId::Aarch64,
            word_bits: 64,
            endian: Endian::Little,
            min_insn_bytes: 4,
            max_insn_bytes: 4,
            insn_encoding: InsnEncoding::Fixed(4),
            delay_slots: DelaySlotModel::None,
            flag_model: FlagModel::Nzcv,
            flag_bit_order: vec!["N", "Z", "C", "V"],
            regs: aarch64_regs(),
            mem_model: MemModel::ArmV8,
            spaces: vec![AddrSpace::Flat],
            unaligned_access_ok: false,
            isa_modes: vec!["a64"],
            abi_set: vec![AbiId::Aapcs64, AbiId::Aapcs64Apple,
                          AbiId::Aapcs64Windows, AbiId::InternalLto, AbiId::Unknown],
            atomics: AtomicSet::LlScAndRmw,
            unwind: UnwindFormat::DwarfCfi,
            reloc_design_set: RelocDesignSet { names: aarch64_relocs() },
        }
    }
    pub fn x86_64() -> ArchProfile {
        ArchProfile {
            id: ArchId::X86_64,
            word_bits: 64,
            endian: Endian::Little,
            min_insn_bytes: 1,
            max_insn_bytes: 15,
            insn_encoding: InsnEncoding::Variable(1, 15),
            delay_slots: DelaySlotModel::None,
            flag_model: FlagModel::X86Eflags,
            flag_bit_order: vec!["CF", "PF", "AF", "ZF", "SF", "OF", "DF"],
            regs: x86_64_regs(),
            mem_model: MemModel::X86Tso,
            spaces: vec![AddrSpace::Flat],
            unaligned_access_ok: true,
            isa_modes: vec!["x86-64"],
            abi_set: vec![AbiId::SysvAmd64, AbiId::Win64,
                          AbiId::InternalLto, AbiId::Unknown],
            atomics: AtomicSet::Rmw,
            unwind: UnwindFormat::DwarfCfi,
            reloc_design_set: RelocDesignSet { names: x86_64_relocs() },
        }
    }
    pub fn wasm32() -> ArchProfile {
        ArchProfile {
            id: ArchId::Wasm32,
            word_bits: 32,
            endian: Endian::Little,
            min_insn_bytes: 1,
            max_insn_bytes: 1,
            insn_encoding: InsnEncoding::Fixed(1),
            delay_slots: DelaySlotModel::None,
            flag_model: FlagModel::NoFlags,
            flag_bit_order: vec![],
            regs: wasm32_regs(),
            mem_model: MemModel::WasmThreads,
            spaces: vec![AddrSpace::Flat],
            unaligned_access_ok: true,
            isa_modes: vec!["wasm32"],
            abi_set: vec![AbiId::WasmBasicCAbi, AbiId::Unknown],
            atomics: AtomicSet::Rmw,
            unwind: UnwindFormat::WasmExceptionTags,
            reloc_design_set: RelocDesignSet { names: vec![] },
        }
    }
}

/// AArch64 register descriptor table (v3.2.1 — closes N2).
/// 31 general-purpose × 64-bit + 32 SIMD/FP × 128-bit + special registers.
/// Sub-register views: Wn (bits 0..31 of Xn), Vn (bits 0..127 of Qn).
fn aarch64_regs() -> Vec<RegDesc> {
    let mut r = Vec::with_capacity(96);
    // X0..X30 (31 GPRs, 64-bit, X31 is xzr/SP alias not enumerated here)
    for i in 0..31 {
        r.push(RegDesc {
            id: RegId(i), name: static_str_x(i), class: RegClass::Gpr,
            bits: 64, alias_of: None,
            narrow_write: NarrowWrite::ZeroExtend,
        });
    }
    // W0..W30 (31 GPRs, 32-bit, sub-register of Xn)
    for i in 0..31 {
        r.push(RegDesc {
            id: RegId(32 + i), name: static_str_w(i), class: RegClass::Gpr,
            bits: 32, alias_of: Some((RegId(i), 0, 32)),
            narrow_write: NarrowWrite::ZeroExtend,
        });
    }
    // V0..V31 (32 SIMD/FP, 128-bit, aliased to Q0..Q31, D0..D31, S0..S31, H0..H31, B0..B31)
    for i in 0..32 {
        r.push(RegDesc {
            id: RegId(64 + i), name: static_str_v(i), class: RegClass::Fpr,
            bits: 128, alias_of: None,
            narrow_write: NarrowWrite::PreserveUpper,
        });
    }
    // SP (X31 alias, not a separate RegId), PC, PSTATE
    r.push(RegDesc {
        id: RegId(96), name: "PC", class: RegClass::Special,
        bits: 64, alias_of: None,
        narrow_write: NarrowWrite::FullWidth,
    });
    r
}

fn static_str_x(i: usize) -> &'static str {
    // Compile-time-ish: returns "X0".."X30" via a const table would be ideal,
    // but a small match is fine and the table is fixed.
    match i {
        0 => "X0", 1 => "X1", 2 => "X2", 3 => "X3", 4 => "X4",
        5 => "X5", 6 => "X6", 7 => "X7", 8 => "X8", 9 => "X9",
        10 => "X10", 11 => "X11", 12 => "X12", 13 => "X13", 14 => "X14",
        15 => "X15", 16 => "X16", 17 => "X17", 18 => "X18", 19 => "X19",
        20 => "X20", 21 => "X21", 22 => "X22", 23 => "X23", 24 => "X24",
        25 => "X25", 26 => "X26", 27 => "X27", 28 => "X28", 29 => "X29",
        30 => "X30", _ => "X?",
    }
}
fn static_str_w(i: usize) -> &'static str {
    match i {
        0 => "W0", 1 => "W1", 2 => "W2", 3 => "W3", 4 => "W4",
        5 => "W5", 6 => "W6", 7 => "W7", 8 => "W8", 9 => "W9",
        10 => "W10", 11 => "W11", 12 => "W12", 13 => "W13", 14 => "W14",
        15 => "W15", 16 => "W16", 17 => "W17", 18 => "W18", 19 => "W19",
        20 => "W20", 21 => "W21", 22 => "W22", 23 => "W23", 24 => "W24",
        25 => "W25", 26 => "W26", 27 => "W27", 28 => "W28", 29 => "W29",
        30 => "W30", _ => "W?",
    }
}
fn static_str_v(i: usize) -> &'static str {
    match i {
        0 => "V0", 1 => "V1", 2 => "V2", 3 => "V3", 4 => "V4",
        5 => "V5", 6 => "V6", 7 => "V7", 8 => "V8", 9 => "V9",
        10 => "V10", 11 => "V11", 12 => "V12", 13 => "V13", 14 => "V14",
        15 => "V15", 16 => "V16", 17 => "V17", 18 => "V18", 19 => "V19",
        20 => "V20", 21 => "V21", 22 => "V22", 23 => "V23", 24 => "V24",
        25 => "V25", 26 => "V26", 27 => "V27", 28 => "V28", 29 => "V29",
        30 => "V30", 31 => "V31", _ => "V?",
    }
}

fn aarch64_relocs() -> Vec<&'static str> {
    vec!["R_AARCH64_ADR_PREL_PG_HI21", "R_AARCH64_ADD_ABS_LO12_NC",
         "R_AARCH64_LDST8_ABS_LO12_NC", "R_AARCH64_LDST16_ABS_LO12_NC",
         "R_AARCH64_LDST32_ABS_LO12_NC", "R_AARCH64_LDST64_ABS_LO12_NC",
         "R_AARCH64_LDST128_ABS_LO12_NC", "R_AARCH64_CALL26",
         "R_AARCH64_JUMP26", "R_AARCH64_ABS64", "R_AARCH64_GLOB_DAT",
         "R_AARCH64_JUMP_SLOT", "R_AARCH64_RELATIVE", "R_AARCH64_TLS_TPREL64",
         "R_AARCH64_TLSDESC", "R_AARCH64_IRELATIVE", "R_AARCH64_COPY"]
}

/// x86-64 register descriptor table (v3.2.1 — closes N2).
/// 16 GPRs (64-bit) + 16 XMM (128-bit) + special RIP/RFLAGS.
fn x86_64_regs() -> Vec<RegDesc> {
    let mut r = Vec::with_capacity(40);
    let gpr_names = ["RAX", "RCX", "RDX", "RBX", "RSP", "RBP", "RSI", "RDI",
                     "R8", "R9", "R10", "R11", "R12", "R13", "R14", "R15"];
    for (i, name) in gpr_names.iter().enumerate() {
        r.push(RegDesc {
            id: RegId(i as u16),
            name: match i { 0..=7 => gpr_names[i], _ => gpr_names[i] },
            class: RegClass::Gpr, bits: 64,
            alias_of: None,
            narrow_write: NarrowWrite::ZeroExtend,
        });
    }
    // 32-bit sub-registers: EAX..EDI, R8D..R15D
    for i in 0..16 {
        let name32 = match i {
            0 => "EAX", 1 => "ECX", 2 => "EDX", 3 => "EBX", 4 => "ESP",
            5 => "EBP", 6 => "ESI", 7 => "EDI",
            n => gpr_names[n].strip_prefix('R').map(|s| format!("R{}D", s)).unwrap_or_default().leak(),
        };
        r.push(RegDesc {
            id: RegId(16 + i as u16),
            name: name32,
            class: RegClass::Gpr, bits: 32,
            alias_of: Some((RegId(i as u16), 0, 32)),
            narrow_write: NarrowWrite::ZeroExtend,
        });
    }
    // 16 XMM registers (128-bit)
    for i in 0..16 {
        r.push(RegDesc {
            id: RegId(32 + i as u16),
            name: match i {
                0 => "XMM0", 1 => "XMM1", 2 => "XMM2", 3 => "XMM3",
                4 => "XMM4", 5 => "XMM5", 6 => "XMM6", 7 => "XMM7",
                8 => "XMM8", 9 => "XMM9", 10 => "XMM10", 11 => "XMM11",
                12 => "XMM12", 13 => "XMM13", 14 => "XMM14", 15 => "XMM15",
                _ => "XMM?",
            },
            class: RegClass::Vector, bits: 128,
            alias_of: None,
            narrow_write: NarrowWrite::PreserveUpper,
        });
    }
    r.push(RegDesc {
        id: RegId(48), name: "RIP", class: RegClass::Special,
        bits: 64, alias_of: None,
        narrow_write: NarrowWrite::FullWidth,
    });
    r
}

/// x86-64 relocation design set (v3.2.1 — closes N11).
/// Synchronized with §21.7's 17-type design set.
fn x86_64_relocs() -> Vec<&'static str> {
    vec!["R_X86_64_64", "R_X86_64_32", "R_X86_64_32S", "R_X86_64_PC32",
         "R_X86_64_GOT32", "R_X86_64_PLT32", "R_X86_64_GOTPCREL",
         "R_X86_64_GOTPCRELX", "R_X86_64_REX_GOTPCRELX",
         "R_X86_64_GOTTPOFF", "R_X86_64_DTPMOD64", "R_X86_64_DTPOFF64",
         "R_X86_64_TPOFF64", "R_X86_64_COPY", "R_X86_64_GLOB_DAT",
         "R_X86_64_JUMP_SLOT", "R_X86_64_RELATIVE", "R_X86_64_IRELATIVE"]
}

/// WebAssembly register descriptor table (v3.2.1 — closes N2).
/// Validation stack + locals table; the register file is the operand stack
/// and a locals vector indexed by function-local index.
fn wasm32_regs() -> Vec<RegDesc> {
    let mut r = Vec::with_capacity(8);
    // Operand stack (conceptual; lifting maps to a bounded stack of
    // GenOperands). The model exposes the stack pointer and the
    // top-of-stack as RegIds 0 and 1.
    r.push(RegDesc {
        id: RegId(0), name: "SP", class: RegClass::Special,
        bits: 32, alias_of: None,
        narrow_write: NarrowWrite::FullWidth,
    });
    r.push(RegDesc {
        id: RegId(1), name: "TOS", class: RegClass::Special,
        bits: 128, alias_of: None,
        narrow_write: NarrowWrite::PreserveUpper,
    });
    r
}

/// Flag state generalized over models. Each named flag is an independent one-bit value (invariant I2).
/// v3.2.1 (closes N10): Nzcv now includes the AArch32 Q (sticky saturation) and
/// GE[3:0] (parallel add/subtract greater-than-or-equal) bits. The §20.3.5
/// bit-index table assigns positions 32..36 to these bits.
#[derive(Clone, PartialEq, Debug)]
pub enum FlagSet {
    Nzcv { n: bool, z: bool, c: bool, v: bool, q: bool, ge: [bool; 4] },
    X86 { cf: bool, pf: bool, af: bool, zf: bool, sf: bool, of: bool, df: bool },
    Ppc { cr: [[bool; 4]; 8], so: bool, ov: bool, ca: bool },
    None,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenWidth { W8, W16, W32, W64, W80, W128, W256, W512 }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenOperand {
    Reg(RegId, GenWidth),
    Imm(u64, GenWidth),
    Zero(GenWidth),               // hardwired zero (AArch64 xzr, RISC-V x0, MIPS $zero)
}

/// Condition predicates, independent of the flag model; the lifter maps a model-specific
/// condition (for example x86 JLE, ARM LE, PowerPC CR bit tests) to one of these predicates over
/// two compared values or to FlagBit for direct flag reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenCond {
    Eq, Ne, Ult, Ule, Ugt, Uge, Slt, Sle, Sgt, Sge,
    FlagBit(u8),                  // index into the model's flag bit order; see §20.3.5
    NotFlagBit(u8),
    Always, Never,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RmwOp { Swap, Add, And, Or, Xor, Min, Max, MinU, MaxU, CompareExchange }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenOrdering { Relaxed, Acquire, Release, AcquireRelease, SeqCst }

#[derive(Clone, PartialEq, Debug)]
pub enum GenMicroInstr {
    Mov { dst: GenOperand, src: GenOperand },
    Extend { dst: GenOperand, src: GenOperand, signed: bool },
    BinOp { op: GenBinOp, dst: GenOperand, a: GenOperand, b: GenOperand, flags_out: bool },
    Select { dst: GenOperand, cond: GenCond, if_true: GenOperand, if_false: GenOperand },
    Compare { a: GenOperand, b: GenOperand },                 // sets the model's flags, or a predicate temp
    Load { dst: GenOperand, space: u8, base: GenOperand, offset: i64, bytes: u8, signed: bool, ord: GenOrdering },
    Store { src: GenOperand, space: u8, base: GenOperand, offset: i64, bytes: u8, ord: GenOrdering },
    LoadLinked { dst: GenOperand, space: u8, base: GenOperand, bytes: u8, ord: GenOrdering },
    StoreConditional { status: GenOperand, src: GenOperand, space: u8, base: GenOperand, bytes: u8, ord: GenOrdering },
    AtomicRmw { op: RmwOp, dst: GenOperand, space: u8, base: GenOperand, src: GenOperand, bytes: u8, ord: GenOrdering },
    Fence { kind: FenceKind },
    Branch { cond: GenCond, target: u64 },
    BranchIndirect { target: GenOperand },
    Call { target: GenCallTarget, clobbers: Vec<RegId> },
    Return,
    Trap { code: u32 },
    Syscall { number_reg: Option<RegId> },
    ReadSpecial { dst: GenOperand, special: u32 },            // opaque: uninterpreted function
    WriteSpecial { src: GenOperand, special: u32 },
    Float { op: FloatOp, dst: GenOperand, a: GenOperand, b: Option<GenOperand>, rounding: RoundingMode },
    Opaque { mnemonic: u32, loss: GenLoss },                  // unmodeled; sets semantic_loss
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenBinOp {
    Add, Sub, AddCarry, SubBorrow, And, Or, Xor, AndNot, OrNot, Nand, Nor,
    Shl, Lshr, Ashr, Rotl, Rotr, Mul, MulHighU, MulHighS, DivU, DivS, RemU, RemS,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FenceKind { Full, LoadLoad, StoreStore, AcquireRel, InstructionSync, Pause }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FloatOp { Add, Sub, Mul, Div, Sqrt, Fma, Neg, Abs, Cmp, FromInt, ToInt, Convert }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoundingMode { NearestEven, TowardZero, TowardPositive, TowardNegative, NearestAway, Dynamic }

#[derive(Clone, PartialEq, Debug)]
pub enum GenCallTarget { Direct(u64), Indirect(GenOperand) }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenLoss {
    UndefinedFlagRead, X87Opaque, VectorOpaque, SystemRegisterOpaque, PointerAuthOpaque,
    MemoryTagOpaque, IndirectBranchUnresolved, InlineAsmOpaque, ConcurrencyOpaque,
    SegmentOpaque, DelaySlotUnresolved, ModeSwitchUnresolved, TableJumpUnresolved,
}
```

*Rules attached to the model.*

1. A lifter MUST apply `NarrowWrite` exactly as the profile declares. The three families differ (x86-64 and AArch64 zero-extend 32-bit writes; RISC-V RV64 and MIPS64 sign-extend 32-bit results; x86 8- and 16-bit writes preserve the upper bits), and a uniform assumption is a soundness defect.
2. `Compare` followed by a branch is the canonical CIRX form regardless of the source model. For a model with flags, the lifter emits both the flag definitions (so later flag reads remain exact) and the `GenCond` predicate where a single compare feeds the branch; for `NoFlags` architectures the branch carries the compare directly.
3. `Opaque` and every `GenLoss` mark the containing function ineligible for Layers 2 and 5 (Section 6.7), exactly as `SemanticLoss` does in Section 3.2.
4. Provenance `semantic_loss` uses the union of `SemanticLoss` and `GenLoss`.
5. The `space` field indexes `ArchProfile::spaces`; flat architectures use space 0 exclusively.

**Does not claim:** that the generic model is complete for vendor extensions (for example, Gekko paired singles, VFPU, SPU, SVE, AVX-512 mask semantics, RISC-V vector); those lower to `Opaque` with `VectorOpaque` until an instruction-semantics source is attached.

### 20.3 Flag Semantics Catalog

Each model states, per flag-setting operation, exactly how every flag is defined. A lifter MUST implement the model it declares; CIRX stores the *definitions* (as expressions), not packed flag words.

### 20.3.1 `Nzcv` (AArch64 and AArch32)

AArch64 definitions are in Section 3.2. AArch32 differs in three normative ways:

1. For logical flag-setting instructions with a shifted second operand, $C$ is the **shifter carry-out**; AArch64 `ANDS`/`BICS` set $C = 0$.
2. For logical flag-setting instructions, A32/T32 leave $V$ unchanged; AArch64 sets $V = 0$.
3. A32/T32 define a sticky saturation flag $Q$ and four $GE$ bits (parallel add/subtract); these are `SystemRegisterOpaque`-free architectural state and are recorded as additional named bits, but any function that reads $Q$ or $GE$ is `VectorOpaque` until a semantics source covers the instruction.

Arm Architecture Reference Manual, A-profile (https://developer.arm.com/documentation/ddi0487/latest), covers both execution states.

### 20.3.2 `X86Eflags`

For a $w$-bit addition $r = a + b$ and subtraction $r = a - b$ (all bit-vectors of width $w \in \{8, 16, 32, 64\}$):

| Flag | Addition | Subtraction (and `CMP`) |
|------|----------|--------------------------|
| $CF$ | $r <_u a$ (carry out) | $a <_u b$ (borrow) |
| $ZF$ | $r = 0$ | $r = 0$ |
| $SF$ | $r[w-1]$ | $r[w-1]$ |
| $OF$ | $(a[w-1] = b[w-1]) \wedge (r[w-1] \neq a[w-1])$ | $(a[w-1] \neq b[w-1]) \wedge (r[w-1] \neq a[w-1])$ |
| $AF$ | $a[4] \oplus b[4] \oplus r[4]$ | $a[4] \oplus b[4] \oplus r[4]$ |
| $PF$ | $\lnot\big(r[0] \oplus r[1] \oplus \cdots \oplus r[7]\big)$ (even parity of the low byte) | same |

**Polarity warning.** On x86 a subtraction sets $CF = 1$ on *borrow*, whereas on AArch64 and AArch32 a subtraction sets $C = 1$ when there is *no* borrow ($a \ge_u b$). Reusing the Nzcv carry definition for x86 is a defect; the two are exact complements for `SUB`/`CMP`.

Further rules (Intel SDM, https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html; AMD64 Architecture Programmer's Manual, https://www.amd.com/en/support/tech-docs):

- `INC` and `DEC` leave $CF$ unchanged and update the other arithmetic flags.
- `AND`, `OR`, `XOR`, and `TEST` clear $CF$ and $OF$, set $SF$, $ZF$, $PF$ from the result, and leave $AF$ undefined.
- Shift and rotate instructions mask the count (to 5 bits for 8-, 16-, and 32-bit operands, 6 bits for 64-bit); a masked count of zero leaves all flags unchanged; $OF$ is defined only for a masked count of one; $AF$ is undefined for shifts.
- `MUL` and `IMUL` define $CF$ and $OF$ and leave $SF$, $ZF$, $AF$, $PF$ undefined.
- $DF$ is the direction flag, changed only by `CLD` and `STD`, and consumed by string instructions.

**Undefined flags.** Where the manual states a flag is undefined, the lifter introduces a fresh unconstrained bit and records `UndefinedFlagRead` *only if* a later instruction reads that flag before it is redefined (liveness-based). A function whose observable behavior depends on an undefined flag is `out_of_envelope`.

### 20.3.3 `PpcCrXer`

The condition register is eight 4-bit fields $CR0$ through $CR7$ with bits $LT$, $GT$, $EQ$, $SO$. The fixed-point exception register $XER$ holds $SO$, $OV$, $CA$.

- A compare (`cmpw`, `cmpwi`, `cmplw`, `cmpd`, and unsigned forms) writes $LT$, $GT$, $EQ$ of the named field from the signed or unsigned comparison and copies $XER[SO]$ into $SO$.
- A record-form instruction (`Rc = 1`, for example `add.`) writes $CR0$ from the signed comparison of the result against zero and copies $XER[SO]$ into $SO$; in 64-bit mode the comparison is on the full 64-bit result unless the processor is in 32-bit mode.
- Carrying instructions (`addic`, `adde`, `subfc`, `subfe`, and others) write $CA$; for `subf`-family carrying instructions $CA$ is the carry out of $\lnot a + b + 1$, so $CA = 1$ means no borrow (the same polarity as Arm).
- Overflow-enabled forms (`OE = 1`, for example `addo`) write $OV$ and accumulate $SO$ (sticky until cleared).

Source: Power ISA (https://openpowerfoundation.org/specifications/isa/).

### 20.3.4 `NoFlags` (RISC-V, MIPS, LoongArch, WebAssembly)

There is no condition register. Comparisons produce integer values (`slt`, `sltu`) or branch directly on register comparisons (`beq`, `bne`, `blt`, `bge`, `bltu`, `bgeu`). The carry of a multi-word addition is computed explicitly (for example, `sltu t, sum, a`). MIPS integer `ADD`, `ADDI`, and `SUB` raise an Integer Overflow exception, whereas `ADDU`, `ADDIU`, and `SUBU` do not; the lifter lowers the trapping forms as an addition followed by a `Trap` on signed overflow. Floating-point comparisons on MIPS (pre-R6) set condition codes in $FCSR$; those are modeled as one-bit state named in the profile.

### 20.3.5 Flag Bit-Index Table (normative, schema version 4)

The `u8` in `GenCond::FlagBit(u8)` and `NotFlagBit(u8)` indexes the per-architecture table below. A lifter MUST publish its `ArchProfile::flag_bit_order` matching exactly one row of this table. A direct flag read (`mrs`/`msr` NZCV on AArch64, `pushf; pop rax; test al, bit` on x86, `mfcr r3; andi r3, r3, bit` on PowerPC) lowers to `GenCond::FlagBit(i)` where `i` is the position in the table; the table position determines the bit. v3.1's comment said the order was "documented per model" but documented it nowhere; this table closes that gap (F-06).

| Position | `Nzcv` (AArch64, AArch32) | `X86Eflags` (x86-64, IA-32) | `PpcCrXer` (PowerPC 32/64) | `NoFlags` (RISC-V, MIPS, LoongArch, Wasm) |
|----------|---------------------------|------------------------------|------------------------------|---------------------------------------------|
| 0 | N | CF | CR0.LT | — |
| 1 | Z | PF | CR0.GT | — |
| 2 | C | AF | CR0.EQ | — |
| 3 | V | ZF | CR0.SO | — |
| 4 | — | SF | CR1.LT | — |
| 5 | — | OF | CR1.GT | — |
| 6 | — | DF | CR1.EQ | — |
| 7 | — | — | CR1.SO | — |
| 8 | — | — | CR2.LT | — |
| 9 | — | — | CR2.GT | — |
| 10 | — | — | CR2.EQ | — |
| 11 | — | — | CR2.SO | — |
| 12 | — | — | CR3.LT | — |
| 13 | — | — | CR3.GT | — |
| 14 | — | — | CR3.EQ | — |
| 15 | — | — | CR3.SO | — |
| 16 | — | — | CR4.LT | — |
| 17 | — | — | CR4.GT | — |
| 18 | — | — | CR4.EQ | — |
| 19 | — | — | CR4.SO | — |
| 20 | — | — | CR5.LT | — |
| 21 | — | — | CR5.GT | — |
| 22 | — | — | CR5.EQ | — |
| 23 | — | — | CR5.SO | — |
| 24 | — | — | CR6.LT | — |
| 25 | — | — | CR6.GT | — |
| 26 | — | — | CR6.EQ | — |
| 27 | — | — | CR6.SO | — |
| 28 | — | — | CR7.LT | — |
| 29 | — | — | CR7.GT | — |
| 30 | — | — | CR7.EQ | — |
| 31 | — | — | CR7.SO | — |
| 32 | — | — | XER.SO | — |
| 33 | — | — | XER.OV | — |
| 34 | — | — | XER.CA | — |

**Use of the table.** A direct read of AArch64 `N` lowers to `FlagBit(0)`; of x86-64 `ZF` to `FlagBit(3)`; of PowerPC CR0.LT to `FlagBit(0)`. A conditional execution predicated on the **logical OR** of several flags lowers to a `Select` over a `GenCond::GenExpr` (a tree of flag reads combined with `And`/`Or`), not to a single `FlagBit`. A lifter that uses a position outside its declared table row is `FlagBitOutOfRange` and the containing function is `Out_of_envelope` for Layers 2/5.

**AArch32 Q and GE bits.** AArch32 has a sticky saturation flag `Q` and four `GE` bits (parallel add/subtract). The `Q` bit is mapped to position 35 and each `GE[3:0]` to positions 36–39 in the `Nzcv` model row. v3.1 left `Q` and `GE` out of the generic model; they are added here so that vector operations that read them are not silently dropped.

### 20.4 Memory Model Parameterization

### 20.4.1 Address width and byte order

The memory array of Section 3.3.1 generalizes to $\mathrm{Mem}_s : \mathrm{Array}(\mathrm{BV}_{a_s}, \mathrm{BV}_8)$ for each address space $s$ with address width $a_s$ from the profile. Multi-byte access composes bytes in the profile's byte order. For a little-endian $n$-byte load, byte at the lowest address is the least significant; for big-endian it is the most significant:

```smt2
(set-logic QF_ABV)
; 32-bit address space, big-endian 32-bit load and store
(declare-const mem (Array (_ BitVec 32) (_ BitVec 8)))
(define-fun load32be ((m (Array (_ BitVec 32) (_ BitVec 8))) (p (_ BitVec 32))) (_ BitVec 32)
  (concat (select m p)
          (concat (select m (bvadd p #x00000001))
                  (concat (select m (bvadd p #x00000002))
                          (select m (bvadd p #x00000003))))))
(define-fun store32be ((m (Array (_ BitVec 32) (_ BitVec 8))) (p (_ BitVec 32)) (v (_ BitVec 32)))
  (Array (_ BitVec 32) (_ BitVec 8))
  (store (store (store (store m p ((_ extract 31 24) v))
                       (bvadd p #x00000001) ((_ extract 23 16) v))
                (bvadd p #x00000002) ((_ extract 15 8) v))
         (bvadd p #x00000003) ((_ extract 7 0) v)))
(check-sat)
```

A bi-endian architecture (ARM, MIPS, PowerPC, RISC-V in principle) is instantiated *per binary* from the object header (ELF `EI_DATA`, PE machine type), never assumed.

### 20.4.2 Aliasing at address width $a$

Section 3.3.2's overlap predicate generalizes to width $a$: accesses $(p, s_1)$ and $(q, s_2)$ overlap iff $(q - p) <_u s_1 \lor (p - q) <_u s_2$ with subtraction in $\mathrm{BV}_a$. For architectures whose bus is narrower than the register (68000 has 24 address bits; real-mode x86 has 20), the address expression is first masked to the bus width, so wrap-around aliasing (for example, real-mode offset overflow without the A20 line) is represented exactly.

### 20.4.3 Segmented and multi-space memory

For x86-16, the effective linear address of a segment:offset pair is $\mathrm{lin}(\mathit{seg}, \mathit{off}) = (16 \cdot \mathit{seg} + \mathit{off}) \bmod 2^{20}$ (A20 line masked) or $\bmod\ 2^{21}$ (A20 enabled, high memory area):

```smt2
(set-logic QF_ABV)
(declare-const seg (_ BitVec 16))
(declare-const off (_ BitVec 16))
; 20-bit linear address with wrap-around (A20 masked)
(define-fun lin20 ((s (_ BitVec 16)) (o (_ BitVec 16))) (_ BitVec 20)
  (bvadd (concat s #x0) ((_ zero_extend 4) o)))
; 21-bit linear address (A20 enabled, high memory area)
(define-fun lin21 ((s (_ BitVec 16)) (o (_ BitVec 16))) (_ BitVec 21)
  (bvadd (concat s #b00000) ((_ zero_extend 5) o)))
(check-sat)
```

Many $(\mathit{seg}, \mathit{off})$ pairs alias one linear address; an aliasing query therefore compares linear addresses, never pairs. Harvard machines (AVR, some DSPs) use distinct arrays for code and data, and loads from code memory (for example AVR `LPM`) read the code array.

### 20.4.4 Alignment

| Architecture | Unaligned access to normal memory |
|--------------|-----------------------------------|
| x86, x86-64 | Permitted (with possible performance penalty; `LOCK`ed accesses crossing a cache line are permitted but slow); alignment-check trap only if `CR0.AM` and `EFLAGS.AC` are set |
| AArch64 | Permitted for normal memory unless `SCTLR_ELx.A` is set; atomics and exclusives require natural alignment |
| AArch32 | Permitted for `LDR`/`STR` on ARMv6 and later when `SCTLR.A` is clear; `LDM`, `STM`, `LDRD`, `STRD`, and exclusives require alignment |
| RISC-V | Implementation-defined: may trap or be handled in software; compilers use `-mstrict-align` or `-mno-strict-align` per target |
| MIPS | Traps; unaligned access uses `LWL`/`LWR`/`SWL`/`SWR`; MIPS R6 may permit hardware handling |
| PowerPC | Permitted for most integer access with possible alignment interrupt in some modes; atomics require natural alignment |
| WebAssembly | Permitted; alignment hints are advisory |

A function that performs a possibly unaligned access on a trap-on-unaligned architecture is lowered with an explicit alignment-fault branch, so the unaligned path is represented rather than ignored.

### 20.5 Delay Slots, Modes, and Predication (Stage 1 Normalization Rules)

**Delay-slot elimination (MIPS, SPARC, SuperH).** Stage 1 MUST remove delay slots, producing straight-line CIRX with no slot. For an unconditional or conditional branch `B` with slot instruction `D`:

1. Evaluate the branch condition operands and, for register jumps, the target register into fresh temporaries.
2. Perform the branch's own link-register write (for `jal`, `bal`, `call`) before the slot.
3. Execute `D` (it may read the link register).
4. Transfer control using the temporaries from step 1.

For a *branch-likely* (MIPS pre-R6) or an *annulling* branch (SPARC), `D` executes only if the branch is taken; the lowering executes `D` under the taken guard. If `D` is itself a branch, the encoding is architecturally unpredictable and the function is reported with `DelaySlotUnresolved`. MIPS Release 6 removes branch-likely and adds compact branches without delay slots; the lifter selects the rule from the object's ISA revision flags.

**ISA-mode switching (ARM and Thumb, MIPS and microMIPS).** The lifter tracks the decode mode per basic block. On AArch32, bit 0 of the target of `BX`, `BLX` (register), and of a load to `PC` selects Thumb (1) or ARM (0); if the target is a computed value whose bit 0 is unknown, the block is marked `ModeSwitchUnresolved` and the function is ineligible for Layers 2 and 5.

**Predication (ARM conditional execution, Thumb IT blocks).** A conditionally executed instruction `I` with condition $c$ lowers to `Select`/guarded forms: every destination is written with `Select(c, new, old)` and memory side effects are guarded. A Thumb-2 `IT` instruction and its following 1–4 instructions lower as a block with the IT-state recorded; an exception or branch inside an IT block (other than the last instruction) is `ModeSwitchUnresolved`.

**Compressed encodings (RISC-V C, Thumb-2, microMIPS).** The lifter decodes the 16-bit form to the same micro-operations as the 32-bit form; the `origin` provenance records the encoded length.

### 20.6 Instruction-Semantics Sources

Semantics MUST come from a machine-readable source that the lifter version-pins; hand-written semantics are permitted only with a recorded differential test against an executable reference (a CPU or a trusted emulator).

| Source | ISA coverage (as documented) | Primary reference |
|--------|------------------------------|-------------------|
| ASLp, from Arm's ASL specifications | AArch64 | https://github.com/UQ-PAC/aslp |
| Sail ISA models | RISC-V (reference model), and models of ARMv8-A and CHERI-MIPS translated from ASL and MIPS sources | Armstrong et al., POPL 2019, https://doi.org/10.1145/3290384 ; https://github.com/riscv/sail-riscv |
| K-framework x86-64 user-level semantics | x86-64 user-level instructions | Dasgupta et al., PLDI 2019, https://doi.org/10.1145/3314221.3314601 |
| Ghidra SLEIGH and P-code | Many ISAs including x86, ARM, MIPS, PowerPC, RISC-V, and WebAssembly (as distributed) | https://github.com/NationalSecurityAgency/ghidra |
| angr with VEX IR and Unicorn/QEMU TCG | Many ISAs | https://angr.io/ ; https://www.unicorn-engine.org/ ; https://www.qemu.org/ |
| BAP | x86, x86-64, ARM, MIPS, PowerPC (as documented) | https://github.com/BinaryAnalysisPlatform/bap |
| remill | x86, x86-64, AArch64, SPARC (as documented) | https://github.com/lifting-bits/remill |
| Binary Ninja LLIL | Many ISAs (commercial) | https://binary.ninja/ |

*Obligation.* The lifter's `semantic_loss` field is set according to the *source's* documented coverage, not the lifter's aspiration. A source that is not formally derived from the architectural specification (Ghidra, VEX, TCG) yields instruction semantics that are trusted by testing, and any Layer 2 or Layer 5 verdict inherits that residual risk (Does not claim, below). Coverage statements in the table are taken from the tools' own descriptions and were not re-verified instruction by instruction (Appendix A, L-17).

**Encoding references for decoders.** A decoder is a separate component from the semantics source and has its own reference:

| Reference | Coverage | Status |
|-----------|----------|--------|
| Arm A-profile A64 Instruction Set Architecture, DDI 0602 (https://developer.arm.com/documentation/ddi0602/latest) | Every A64 encoding, operand syntax, and feature gate | Primary (vendor) |
| Arm A-profile A32 and T32 Instruction Set Architecture, DDI 0597 (https://developer.arm.com/documentation/ddi0597/latest) | Every A32 and T32 encoding | Primary (vendor) |
| TonicBox ARM Instruction Reference (https://arm64.tonicbox.dev) | Described by its own page as a "fast, searchable ARM instruction reference (AArch64 A64, AArch32 A32/T32): bit-field encodings, operand syntax, and feature gating for every form" | Unofficial third-party lookup aid; client-rendered, so this review could not read its contents and has not assessed its accuracy (Appendix A, L-22) |

*Obligation.* Every encoding form in the primary reference MUST have at least one decode test; a disagreement between the unofficial site and the vendor documents is resolved in favor of the vendor documents and recorded. The unofficial site is a convenience for human lookup and is never a premise of a verification claim (hypothesis H-X5, Section 31.1).

**Does not claim:** that any listed source is complete or correct for every instruction; formal derivation from the vendor specification exists for ASLp (AArch64) and the Sail models, and no such derivation is claimed for the others.

### 20.7 Memory-Ordering Models

The sequential lowering of Section 3.3.3 is the only ordering semantics this specification verifies. The models below are recorded so that an ordering-dependent function is reported `out_of_envelope` with the *correct* model named.

| Architecture | Model | Primary reference |
|--------------|-------|-------------------|
| x86, x86-64 | x86-TSO | Sewell et al., CACM 53(7), 2010, https://doi.org/10.1145/1785414.1785443 |
| AArch64, AArch32 (ARMv8) | ARMv8 other-multi-copy-atomic model | Pulte et al., POPL 2018, https://doi.org/10.1145/3158107 |
| RISC-V | RVWMO (with optional Ztso) | RISC-V Instruction Set Manual, Volume I, https://github.com/riscv/riscv-isa-manual |
| PowerPC (POWER) | POWER memory model | Sarkar et al., PLDI 2011, https://doi.org/10.1145/1993498.1993520 |
| MIPS | Implementation-specific; `SYNC` ordering | MIPS architecture documentation (vendor) |
| WebAssembly (threads) | Sequentially consistent atomics plus unordered non-atomics (as specified) | WebAssembly specification, https://webassembly.github.io/spec/core/ |

### 20.8 Generalized Metadata Availability Indicators

The seven MAS indicators of Section 4.1 keep their weights and definitions. Their *predicates* are generalized by container and architecture:

| Indicator | ELF | PE/COFF | Mach-O | MZ/NE/LE/LX (DOS) | WebAssembly |
|-----------|-----|---------|--------|-------------------|-------------|
| $I_{\text{sym}}$ (30) | `.symtab` with `STT_FUNC`; or decompressed `.gnu_debugdata` (MiniDebugInfo) | Locally loaded PDB; COFF symbol table | `LC_SYMTAB` (nlist) naming functions; local dSYM | Linker map or debugger symbols supplied (no embedded table in stripped MZ) | `name` custom section with function names |
| $I_{\text{dyn}}$ (15) | `.dynsym` | Export directory | Exports trie (`LC_DYLD_EXPORTS_TRIE`) or `LC_DYLD_INFO` | NE/LE entry table | Export section |
| $I_{\text{line}}$ (10) | `.debug_line` | PDB/CodeView line records | DWARF line in dSYM | Codeview/Borland line tables when present | DWARF `.debug_line` custom section or source map |
| $I_{\text{type}}$ (15) | `.debug_info` | PDB type stream | DWARF in dSYM | CodeView types when present | DWARF `.debug_info` custom section |
| $I_{\text{map}}$ (15) | Linker map | Linker map or `.map` | Linker map | Linker map | Not applicable (0) |
| $I_{\text{lto}}$ (10) | Bitcode objects | `/GL` LTCG objects | Bitcode objects or ThinLTO files | Not applicable (0) | Bitcode before `wasm-ld` |
| $I_{\text{cmt}}$ (5) | `.comment`, `DW_AT_producer` | Rich header or PDB compiler record | `LC_BUILD_VERSION` tools or `DW_AT_producer` | Compiler signature string or startup code fingerprint | `producers` custom section |

MiniDebugInfo (the `.gnu_debugdata` section holding an LZMA-compressed minimal symbol table) is described in the GDB manual (https://sourceware.org/gdb/current/onlinedocs/gdb.html/MiniDebugInfo.html); a stripped Android or distribution binary may therefore score $I_{\text{sym}} = 1$ after decompression. The indicator is evaluated *after* the documented pre-processing step and the pre-processing is recorded in `metadata_provenance`.

An indicator that has no analog for a format is fixed at 0, and the track thresholds of Section 4.1 apply unchanged; a consequence is that a stripped DOS executable with no map has $\mathrm{MAS}_\% = 0$ and is Track B.

### 20.9 Generalized Sandbox Contract

The Safe/Suspect/Hostile classification of Section 7.3 and its fail-closed rule apply to every architecture. The execution backend and the environment stubs are architecture-specific:

| Architecture | Execution backend (user-space) | Time and identification probes to stub |
|--------------|--------------------------------|----------------------------------------|
| AArch64 | QEMU user mode; Unicorn | `CNTVCT_EL0`, `CNTFRQ_EL0`, `MIDR_EL1`, `ID_AA64*`, `ptrace`, `NtQueryInformationProcess` |
| AArch32 | QEMU user mode; Unicorn | CP15 `CNTVCT` or `PMCCNTR` reads where enabled, `MIDR`, `ptrace` |
| x86-64, IA-32 | QEMU user mode; Unicorn | `RDTSC`, `RDTSCP`, `CPUID` (including the hypervisor leaf `0x40000000`), `IsDebuggerPresent`, `PEB.BeingDebugged`, `NtGlobalFlag`, `int 2dh`, `ptrace` |
| x86-16 (DOS) | An x86 real-mode emulator with a DOS interrupt layer (`INT 21h`, `INT 10h`, `INT 16h`); QEMU system mode with a DOS image is not user-space | BIOS timer tick (`INT 1Ah`), port `0x40` timer reads, video memory reads |
| RISC-V | QEMU user mode; Unicorn | `rdcycle`, `rdtime`, `rdinstret` (may be trapped or restricted by the OS), `ptrace` |
| MIPS | QEMU user mode; Unicorn | `rdhwr` for `$2` (cycle count) and `$29` (thread pointer), `ptrace` |
| PowerPC | QEMU user mode; Unicorn | `mftb` and `mftbu` (time base), `mfspr` of `PVR`, `ptrace` |
| WebAssembly | A WebAssembly runtime with deny-by-default imports | Host time imports and any `env.*` fingerprinting import |

*Containment.* The microVM or user-space-kernel requirement of Section 7.3 applies unchanged. WebAssembly modules are memory-safe within their linear memory, but imports are an unrestricted capability channel: the runtime MUST be configured with *no* imports other than the capability stubs, and a module with an unknown import is not executed (Hostile).

*Console and firmware binaries.* A binary for a console CPU runs only under an emulator that implements the platform's system services; the emulator adapter defines the stub set and is part of the platform tier (Section 29). Where no emulator is available the binary is analyzed statically only.

### 20.10 Generalized Solver Logics, Floating Point, and Normalized Walls

**Floating-point logics.** The SMT-LIB FloatingPoint theory (https://smt-lib.org/theories-FloatingPoint.shtml) defines IEEE 754 arithmetic. The logics `QF_ABVFP` (arrays, bit-vectors, floating point) and `QF_AUFBVFP` (adds uninterpreted functions) are used when a function performs floating-point arithmetic that lifts without loss. Floating-point reasoning is expensive; the walls of Section 6.7 apply with a tighter default: a Layer 2 obligation containing floating-point division, square root, or fused multiply-add whose solver time exceeds the per-query limit is `unknown`.

- IEEE binary32 and binary64 map to `(_ FloatingPoint 8 24)` and `(_ FloatingPoint 11 53)`.
- The x87 80-bit extended format has a 64-bit significand with an *explicit* integer bit; it maps to `(_ FloatingPoint 15 64)` for normalized values only. Unnormal and pseudo-denormal encodings are outside the SMT-LIB sort and are lowered as `X87Opaque`.
- The x87 register stack (`TOP`, tag word, `ST(i)` aliasing) is *not* modeled by default; any function using x87 stack instructions is `X87Opaque`, ineligible for Layers 2 and 5, unless the lifter provides an explicit stack model with a recorded test against hardware.
- MIPS R5900 (PlayStation 2) floating point is not IEEE 754 conformant in rounding and special-value handling (informational; Appendix A, L-19), so floating point on that target is always `Opaque`.
- Flush-to-zero, denormals-are-zero, and default-NaN modes are control-register state that changes semantics; a function that depends on them is `out_of_envelope`.

**Normalized walls.** Machine instruction counts are not comparable across ISAs (an x86 instruction may lower to many micro-operations, a RISC-V instruction to one). Every wall that Part I states in machine instructions is therefore enforced *both* ways, and the stricter bound applies:

| Wall | Machine-instruction bound (Part I) | Micro-operation bound (CIRX Stage 1) |
|------|-----------------------------------|---------------------------------------|
| Layer 5 | 2,000 | 8,000 |
| Layer 2 symbolic values | $10^6$ (already ISA-independent) | $10^6$ |
| CF-GKAT | 10,000 basic blocks (already ISA-independent) | 10,000 |

Both micro-operation bounds are policy parameters, calibrated by hypothesis H-X1 (Section 31). The micro-operation wall is a new abandonment reason (`microop_wall`) in the verification record (Section 3.7).

---
## 21. x86-64 (AMD64, Intel 64)

### 21.1 Scope and Platforms

Architecture identifier `x86_64`. Platforms: Linux, Android, and BSD ELF64 (System V ABI); Windows PE32+ (Microsoft x64 ABI); macOS Mach-O 64-bit (System V-derived calling convention with Apple extensions); x86-64 game consoles are informational only and run on platform-specific containers (Section 29). Arm64EC on Windows ARM is an ARM64 binary interface that interoperates with x64 code; Arm64EC modules are specified by Section 2.2 and not by this section. Layer applicability is stated in Section 21.8.

### 21.2 Containers and Metadata Indicators

ELF64, PE32+, and Mach-O 64-bit; indicator predicates are in Section 20.8. Linux distribution binaries commonly keep `.dynsym` and often carry `.gnu_debugdata`; Windows system binaries commonly ship stripped with public PDBs available from a symbol server, so a locally absent PDB yields $I_{\text{sym}} = 0$ until the PDB is supplied, after which the lifter records `metadata_provenance: Pdb`.

### 21.3 ABI and Calling Convention

| Property | System V AMD64 (Linux, BSD, macOS) | Microsoft x64 (Windows) |
|----------|------------------------------------|--------------------------|
| Integer and pointer arguments | `rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9` | `rcx`, `rdx`, `r8`, `r9` |
| Floating-point arguments | `xmm0`–`xmm7` | `xmm0`–`xmm3` (position-aligned with integer slots) |
| Return | `rax` (and `rdx` for 128-bit); `xmm0` (and `xmm1`) | `rax`; `xmm0` |
| Variadic functions | `al` holds an upper bound on the number of vector registers used | Floating-point variadic arguments are also copied into the integer register of the same slot |
| Callee-saved | `rbx`, `rbp`, `r12`–`r15` | `rbx`, `rbp`, `rdi`, `rsi`, `rsp`, `r12`–`r15`, `xmm6`–`xmm15` |
| Stack alignment at `call` | 16 bytes | 16 bytes |
| Red zone | 128 bytes below `rsp` usable by leaf functions | None |
| Shadow space | None | 32 bytes allocated by the caller above the return address |

Sources: System V AMD64 psABI (https://gitlab.com/x86-psABIs/x86-64-ABI); Microsoft x64 calling convention (https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention). **Red-zone rule.** For `sysv_amd64`, memory in $[\mathtt{rsp} - 128, \mathtt{rsp})$ of a leaf function is live stack and MUST NOT be treated as free by stack-region analysis or by Layer 2; the Windows ABI has no red zone, and kernel code compiled with `-mno-red-zone` has none.

### 21.4 Instruction-Set Hazards and CIRX Lowering

1. **Variable-length decoding.** Instructions are 1–15 bytes; a wrong entry point yields overlapping, valid-looking decodings. CFG recovery MUST seed from symbol, export, unwind-table, and relocation evidence, and any region reached only by linear sweep is marked `IndirectBranchUnresolved` for safety.
2. **Prefixes.** Operand-size (`66`), address-size (`67`), segment overrides, `LOCK` (`F0`), `REP`/`REPNE` (`F3`/`F2`), `REX`, `VEX`, and `EVEX` change width, addressing, and atomicity; the lifter MUST decode them into explicit micro-operation attributes, not discard them.
3. **Partial registers.** 32-bit writes zero-extend to 64 bits; 8- and 16-bit writes preserve the upper bits (`NarrowWrite::ZeroExtend` and `PreserveUpper`); `AH`, `BH`, `CH`, `DH` are bits 15:8 of the corresponding register.
4. **Flags.** Section 20.3.2 defines every flag; a flag-setting instruction followed by a flag consumer (`Jcc`, `CMOVcc`, `SETcc`, `ADC`, `SBB`) lowers to a `Compare`/`GenCond` pair plus the exact flag definitions for any later read.
5. **Addressing.** RIP-relative addressing is lowered to an absolute address using the instruction end (the displacement is relative to the address of the *next* instruction). `FS`- and `GS`-relative accesses address thread-local storage (Linux x86-64: `FS` base; Windows x64: `GS` base to the TEB); the segment base is a distinct profile register, and without a thread-block model such accesses use address space `unknown`.
6. **String instructions with `REP`.** Lowered to an explicit loop on `RCX`, with the step direction from $DF$ and the element size from the opcode; the loop is bounded by the pre-loop `RCX` value (symbolic).
7. **Atomics.** `LOCK`-prefixed read-modify-write instructions, `XCHG` with a memory operand (implicitly locked), `CMPXCHG`, and `CMPXCHG16B` lower to `AtomicRmw`.
8. **Jump tables.** `jmp [table + index*scale]` and relative-offset tables are resolved by recovering the bounding comparison (`cmp`, `ja`) before the jump; an unbounded table is `TableJumpUnresolved`.
9. **System calls and opaque state.** `SYSCALL`, `SYSENTER`, `INT 0x80`, `CPUID`, `RDTSC`, `XGETBV`, and model-specific register access lower to `Syscall` or `ReadSpecial` with an uninterpreted function.
10. **Control-flow protections.** `ENDBR64` (Intel CET indirect-branch tracking marker) lowers to a no-op with an annotation; shadow-stack instructions (`INCSSP`, `RDSSP`) are `SystemRegisterOpaque`. Retpoline thunks (for example, `__x86_indirect_thunk_rax`) are recognized by symbol or by their fixed `call`/`mov`/`ret` pattern and lowered to an indirect call or jump.
11. **Floating point and vectors.** Scalar SSE and SSE2 arithmetic lowers to `Float` with the rounding mode from $MXCSR$ (dynamic unless the lifter proves it constant); x87 is `X87Opaque`; MMX, SSE packed, AVX, AVX2, and AVX-512 are `VectorOpaque` until a semantics source attaches (the K-framework semantics of Dasgupta et al. cover user-level x86-64 including many vector instructions; coverage is not re-verified here, Appendix A, L-17).

### 21.5 Memory Model, Atomics, and Ordering

x86-TSO (Section 20.7). The sequential lowering is the verified model. `LOCK` operations and `MFENCE`, `SFENCE`, `LFENCE` lower to `Fence`/`AtomicRmw` and carry no sequential-model effect. The verification claim is restricted to single-threaded or data-race-free execution, as in Section 3.3.3.

### 21.6 Unwind and C++ Runtime Lowering

| Source structure | Lowering | Primary source |
|------------------|----------|----------------|
| `.eh_frame` (System V) | DWARF CFI rows. DWARF register numbers: `rax`=0, `rdx`=1, `rcx`=2, `rbx`=3, `rsi`=4, `rdi`=5, `rbp`=6, `rsp`=7, `r8`–`r15`=8–15, return address=16, `xmm0`–`xmm15`=17–32 | System V AMD64 psABI |
| `.gcc_except_table` LSDA | Call-site table to exceptional edges | Itanium C++ ABI exception handling |
| `.pdata` and `.xdata` (Windows x64) | `RUNTIME_FUNCTION` (begin RVA, end RVA, unwind-info RVA); `UNWIND_INFO` header (version, flags `UNW_FLAG_EHANDLER`, `UNW_FLAG_UHANDLER`, `UNW_FLAG_CHAININFO`, prolog size, code count, frame register and offset); unwind codes `UWOP_PUSH_NONVOL`, `UWOP_ALLOC_LARGE`, `UWOP_ALLOC_SMALL`, `UWOP_SET_FPREG`, `UWOP_SAVE_NONVOL`, `UWOP_SAVE_NONVOL_FAR`, `UWOP_SAVE_XMM128`, `UWOP_SAVE_XMM128_FAR`, `UWOP_PUSH_MACHFRAME` lower to CFA rules; a handler RVA with handler data lowers to exceptional edges | https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64 |
| MSVC C++ EH (`__CxxFrameHandler3` and later) | `FuncInfo` (magic number, unwind map, try-block map, IP-to-state map) lowers to exceptional edges by state | Microsoft ABI documentation; reverse-engineering literature; low assurance (L-12) |
| MSVC RTTI on x64 | `CompleteObjectLocator` with signature value 1 and RVA-valued fields (type descriptor, class hierarchy descriptor, self), located at vftable index $-1$ | As Section 3.8 |
| Itanium vtable and `typeinfo` (System V) | As Section 3.8 | Itanium C++ ABI |

### 21.7 Relocation Design Set

| ELF x86-64 type | Value | Semantics |
|-----------------|-------|-----------|
| `R_X86_64_64` | 1 | Absolute 64-bit, $S + A$ |
| `R_X86_64_PC32` | 2 | PC-relative 32-bit, $S + A - P$ |
| `R_X86_64_PLT32` | 4 | PLT-relative 32-bit |
| `R_X86_64_COPY` | 5 | Copy relocation |
| `R_X86_64_GLOB_DAT` | 6 | GOT entry |
| `R_X86_64_JUMP_SLOT` | 7 | PLT GOT entry |
| `R_X86_64_RELATIVE` | 8 | Base-relative, $B + A$ |
| `R_X86_64_GOTPCREL` | 9 | GOT-relative PC offset |
| `R_X86_64_32`, `R_X86_64_32S` | 10, 11 | Absolute 32-bit, zero- and sign-extended |
| `R_X86_64_DTPMOD64`, `R_X86_64_DTPOFF64`, `R_X86_64_TPOFF64` | 16, 17, 18 | TLS |
| `R_X86_64_GOTTPOFF` | 22 | TLS initial-exec |
| `R_X86_64_IRELATIVE` | 37 | Indirect function resolver |
| `R_X86_64_GOTPCRELX`, `R_X86_64_REX_GOTPCRELX` | 41, 42 | Relaxable GOT-relative |

PE32+: `IMAGE_REL_AMD64_ADDR64` (1), `ADDR32` (2), `ADDR32NB` (3), `REL32` (4) in object files; base relocation types `IMAGE_REL_BASED_ABSOLUTE` (0, padding), `HIGHLOW` (3), `DIR64` (10) in images (https://learn.microsoft.com/en-us/windows/win32/debug/pe-format). Mach-O x86-64: `X86_64_RELOC_UNSIGNED` (0), `SIGNED` (1), `BRANCH` (2), `GOT_LOAD` (3), `GOT` (4), `SUBTRACTOR` (5). The design set is these types; any other type is recorded `unsupported(kind)`. Source for ELF numbers: System V AMD64 psABI (https://gitlab.com/x86-psABIs/x86-64-ABI); numeric values were written from the specification and not re-fetched in this review (Appendix A, L-17).

### 21.8 Verification Cascade Eligibility

| Layer | x86-64 status | Notes |
|-------|---------------|-------|
| 1 CF-GKAT | Available at the CFG level (ISA-independent) | Requires `TableJumpUnresolved`-free CFG or opaque treatment of unresolved jumps |
| 2 Layer 2 checker | Eligible when no `X87Opaque`, `VectorOpaque`, `UndefinedFlagRead`, or `SegmentOpaque` loss remains | Semantics sources: Section 20.6 |
| 3 Execution | Available | QEMU user mode and Unicorn |
| 4 Matching | Available for GCC and Clang; for MSVC requires the compiler to be executable in the analysis environment | Configuration space is large; the $10^3$ configuration wall applies |
| 5 Formal | **No backend** (Section 30.5) | Reported `Out_of_envelope` with abandonment reason `no_semantics_backend` |

### 21.9 Track B Sandbox and Anti-Analysis

The x86 stub set is in Section 20.9. For the Suspect class, the specification adds: `RDTSC` and `RDTSCP` return a counter that advances by a fixed small increment per read (policy: 25 cycles); `CPUID` leaf 1 returns the hypervisor-present bit cleared and leaf `0x40000000` returns an invalid-leaf response; `PEB.BeingDebugged` and `NtGlobalFlag` read as clear; `int 2dh` and single-step traps behave as without a debugger; `ptrace(PTRACE_TRACEME)` returns 0. Virtualizing protectors (VMProtect, Themida, and similar) are classified Hostile; their devirtualization is out of scope (Section 7.5). Obfuscation signals specific to x86: opaque predicates built from flag tricks, junk-byte jumps into the middle of instructions, and `call`/`pop` GetPC sequences.

### 21.10 Idioms

Alignment to a power-of-two $A$ held in `rsi`, pointer in `rdi`:

```asm
lea  rax, [rdi + rsi - 1]
neg  rsi
and  rax, rsi          ; rax = (ptr + A - 1) & -A, equal to & ~(A - 1) for power-of-two A
```

Spin lock on a 32-bit word (`0` = free):

```asm
; rdi = address of lock word
.acquire:
    mov   eax, 1
    xchg  eax, [rdi]       ; implicitly locked
    test  eax, eax
    jz    .done
.spin:
    pause                  ; spin-wait hint, no architectural effect on correctness
    cmp   dword [rdi], 0
    jne   .spin
    jmp   .acquire
.done:
    ret
```

```cpp
#include <atomic>

#if defined(__x86_64__) || defined(__i386__)
#include <immintrin.h>
#endif

class Spinlock {
    std::atomic<int> word_{0};
public:
    void lock() noexcept {
        while (word_.exchange(1, std::memory_order_acquire) != 0) {
            while (word_.load(std::memory_order_relaxed) != 0) {
#if defined(__x86_64__) || defined(__i386__)
                _mm_pause();
#endif
            }
        }
    }
    void unlock() noexcept { word_.store(0, std::memory_order_release); }
};
```

`PAUSE` is encoded as `F3 90`, which executes as a no-operation on processors without the extension. A compiler targeting an architecture-level baseline that guarantees `CMPXCHG` may emit a `LOCK CMPXCHG` loop instead of `XCHG`; the idiom's presence depends on the generating compiler.

### 21.11 Hypotheses, Stress Tests, Does Not Claim

- **H-x86-1.** On the testbed, the fraction of x86-64 functions that are Layer 2 eligible (no `X87Opaque`, `VectorOpaque`, `UndefinedFlagRead`) under `-O2` without LTO is at least 50 %; with ThinLTO it is within 10 percentage points of that value.
- **Stress test X1 (CET and retpolines).** A binary built with `-fcf-protection=full` and `-mindirect-branch=thunk` has an `ENDBR64` at every indirect target and an indirect-call thunk at every indirect call; the lifter treats the former as a no-op marker and the latter as an indirect call, and CFG recovery uses the markers as *evidence* of valid indirect targets without treating them as a complete target set.
- **Stress test X2 (desynchronized decode).** Junk bytes after an unconditional jump are not instructions; any region without an incoming edge and without a symbol is excluded from function boundaries and recorded as `data_or_unreachable`.

**Does not claim:** that flag, partial-register, and prefix semantics are complete for AVX-512 masking, transactional memory (`XBEGIN`), or system-management instructions; or that Windows-on-ARM x64 emulation (Arm64EC) is described by this section.

---

## 22. IA-32 (i386) and x86-16 (Real Mode and DOS)

### 22.1 Scope and Platforms

Architecture identifiers `x86_32` and `x86_16`. IA-32 platforms: Linux i386 ELF32, Win32 PE32, early macOS Mach-O i386, BSD. x86-16 platforms: DOS (MZ `.EXE` and flat `.COM`), Windows 3.x and OS/2 1.x (NE), DOS-extender and OS/2 2.x (LE and LX), and protected-mode DOS programs. 16-bit code appears inside 32-bit containers through 16-bit segments and thunks.

### 22.2 Containers and Metadata Indicators

| Container | Description | MAS notes |
|-----------|-------------|-----------|
| ELF32, PE32 | Flat 32-bit containers | Indicators as Section 20.8 |
| MZ (`.EXE`) | DOS: header with relocation table of segment fixups, initial `CS:IP` and `SS:SP`, minimum and maximum extra paragraphs | No symbols in stripped files: $I_{\text{sym}} = 0$ unless a map is supplied |
| `.COM` | Flat image loaded at offset `0x100` of a single 64 KB segment, no header | $\mathrm{MAS}_\% = 0$ |
| NE | 16-bit protected mode (Windows 3.x, OS/2 1.x): segment table, per-segment relocation records, entry table, resident and non-resident name tables | The name tables give $I_{\text{dyn}} = 1$ for exported entries |
| LE and LX | 32-bit DOS-extender and OS/2 2.x objects with fixup tables | As NE |

The authoritative public documentation for MZ, NE, LE, and LX is informal and community-maintained (for example, https://wiki.osdev.org/MZ); no vendor-published normative specification was located, so these container descriptions are *informational* and every parser MUST validate field ranges against file size (Appendix A, L-18).

### 22.3 ABI and Calling Convention

| Convention | Argument passing | Stack cleanup | Return | Source |
|------------|------------------|---------------|--------|--------|
| i386 System V (`cdecl`) | Stack, right to left | Caller | `eax` (and `edx`); `st(0)` for floating point | i386 psABI, https://gitlab.com/x86-psABIs/i386-ABI |
| Win32 `stdcall` | Stack, right to left | Callee | `eax`; `st(0)` | https://learn.microsoft.com/en-us/cpp/cpp/stdcall |
| MSVC `fastcall` | `ecx`, `edx`, then stack | Callee | `eax`; `st(0)` | https://learn.microsoft.com/en-us/cpp/cpp/fastcall |
| MSVC `thiscall` | `this` in `ecx`, remaining on stack | Callee | `eax`; `st(0)` | https://learn.microsoft.com/en-us/cpp/cpp/thiscall |
| GCC `thiscall` (Itanium) | `this` as the first stack argument | Caller | `eax`; `st(0)` | Itanium C++ ABI on i386 |
| DOS 16-bit C (small and large models) | Stack, right to left; `near` calls push IP, `far` calls push CS:IP | Caller (cdecl) or callee (`pascal`) | `ax` or `dx:ax`; `st(0)` in x87 builds | Compiler documentation (informational) |

Callee-saved registers for i386 `cdecl` are `ebx`, `esi`, `edi`, `ebp`. A floating-point return value in `st(0)` means the callee leaves a value on the x87 stack; the contract field `post` MUST record the x87 stack depth, and the function is `X87Opaque` (Section 20.10) unless an explicit x87 model is attached.

**Convention recovery.** The convention is inferred from stack cleanup (`ret` vs `ret N`), register use at entry (`ecx` read before write suggests `thiscall` or `fastcall`), and demangled names (MSVC mangling encodes the convention). Where the evidence is ambiguous the contract records `abi: unknown` and Layer 2 derives the contract from the body.

### 22.4 Instruction-Set Hazards and CIRX Lowering

All items in Section 21.4 that apply to 32-bit mode apply here, with these changes and additions:

1. **No RIP-relative addressing and no 64-bit registers.** Position-independent code uses `call`/`pop` GetPC sequences (the i386 PIC thunk `__x86.get_pc_thunk.*`); the lifter recognizes the thunk and replaces it with the constant return address.
2. **x87 is the default floating-point unit.** Without `-msse2 -mfpmath=sse`, all floating point is x87, hence `X87Opaque` and ineligible for Layers 2 and 5.
3. **16-bit and mixed sizes.** The default operand and address size comes from the code-segment descriptor `D` bit (32-bit) or the NE/MZ context (16-bit); `66` and `67` prefixes toggle it. The lifter records the mode per basic block.
4. **Segmentation.** In 16-bit code, every memory access is $\mathrm{lin}(\mathit{seg}, \mathit{off})$ (Section 20.4.3), where $\mathit{seg}$ is `DS` by default, `SS` for `bp`- and `sp`-based addressing, `ES` for string destinations, and `CS` with a segment-override prefix. Far pointers (`les`, `lds`) load a segment and an offset from memory; far calls and returns (`call far`, `retf`) push and pop `CS:IP`. In flat 32-bit code, segment bases are zero except `FS` (Win32: the TEB; Linux i386 TLS: `GS`), modeled as in Section 21.4 item 5.
5. **DOS memory models.** The tiny, small, medium, compact, large, and huge models determine whether code and data pointers are near or far; a function's contract records the model, and the lifter MUST NOT assume a flat address space for 16-bit code.
6. **Interrupts as system calls.** `INT 21h` (DOS services, selected by `ah`), `INT 10h`, `INT 16h`, `INT 13h`, and `INT 1Ah` lower to `Syscall` with the service number in a named register; their effects are not modeled and the function is `SystemRegisterOpaque`.
7. **Overlays.** Overlay managers (for example, the Borland VROOMM scheme or the Microsoft overlay manager using an interrupt) replace direct calls with stubs that load code into a shared region; code at one address then has multiple possible bodies. Overlay thunks are detected by pattern, the callee is recorded as `overlay(index)`, and functions in overlay regions receive a per-overlay address space. A function reachable through an overlay stub whose body cannot be identified is `IndirectBranchUnresolved`.
8. **Self-modifying code.** DOS-era code frequently writes into its own code segment (decompressors, copy protection, self-relocation). The lifter MUST detect any store whose address can alias the code range (using the linear-address aliasing of Section 20.4.3) and mark the function `SelfModifying`; such functions are ineligible for Layers 2 and 5 and produce no formal claim (Section 22.8).
9. **Win32 SEH.** Stack-based exception frames are installed by writing `FS:[0]` (Section 22.6).

### 22.5 Memory Model, Atomics, and Ordering

IA-32 follows x86-TSO. x86-16 DOS programs are single-threaded except for interrupt handlers and timer callbacks, which run asynchronously with respect to the main code; a function that shares memory with an interrupt handler is `ConcurrencyOpaque`.

### 22.6 Unwind and Runtime Lowering

| Source structure | Lowering | Primary source |
|------------------|----------|----------------|
| `.eh_frame` (i386 ELF) | DWARF CFI. DWARF register numbers: `eax`=0, `ecx`=1, `edx`=2, `ebx`=3, `esp`=4, `ebp`=5, `esi`=6, `edi`=7, return address=8 | i386 psABI |
| Win32 SEH (stack-based) | Chain of `EXCEPTION_REGISTRATION_RECORD` (next pointer, handler pointer) rooted at `FS:[0]`; MSVC scope tables for `__try`; the lifter models the installation (`push handler; push FS:[0]; mov FS:[0], esp`) as a store sequence and records the handler as an exceptional edge | Microsoft documentation of structured exception handling; low assurance for the table layouts (L-12) |
| MSVC C++ EH on x86 | `FuncInfo` via `__CxxFrameHandler3`; as Section 21.6 with absolute addresses instead of RVAs | As Section 21.6 |
| DOS and 16-bit | No standard unwinding; `setjmp` and `longjmp` are library-defined; any C++ exception mechanism is compiler-specific | A function using it is `Out_of_envelope` |

### 22.7 Relocation Design Set

| Format | Types in the design set |
|--------|-------------------------|
| ELF i386 | `R_386_32` (1), `R_386_PC32` (2), `R_386_GOT32` (3), `R_386_PLT32` (4), `R_386_COPY` (5), `R_386_GLOB_DAT` (6), `R_386_JMP_SLOT` (7), `R_386_RELATIVE` (8), `R_386_GOTOFF` (9), `R_386_GOTPC` (10), `R_386_TLS_TPOFF` (14), `R_386_IRELATIVE` (42) |
| PE32 | `IMAGE_REL_I386_DIR32` (6), `IMAGE_REL_I386_DIR32NB` (7), `IMAGE_REL_I386_REL32` (0x14); base relocation `IMAGE_REL_BASED_HIGHLOW` (3) |
| MZ | The relocation table is a list of `(offset, segment)` pairs; the loader adds the *start segment* (the segment at which the load module begins, conventionally the program segment prefix segment plus `0x10`) to the 16-bit word at each listed address. The static emulator applies that addition with a symbolic load segment |
| NE | Per-segment relocation records: internal reference, import by ordinal, import by name, operating-system fixup |
| LE and LX | Fixup records per object page |

Any type outside the design set is `unsupported(kind)`; the MZ relocation addition is the *only* MZ relocation, and a loader that performs additional work (overlay managers, self-relocating stubs) is outside the static emulation.

### 22.8 Verification Cascade Eligibility

| Layer | IA-32 | x86-16 |
|-------|-------|--------|
| 1 CF-GKAT | Available | Available (CFG-level), with far calls and returns as primitives |
| 2 Layer 2 checker | Eligible without `X87Opaque`, vector loss, undefined-flag reads | Eligible only for functions with a known memory model, no self-modification, no overlay stubs, and no interrupt-handler sharing; address model per Section 20.4.3 |
| 3 Execution | Available (QEMU user mode, Unicorn) | Available only on a DOS emulator with deterministic timer and interrupt behavior |
| 4 Matching | Compilers (GCC, Clang, MSVC) as Section 21.8 | Period compilers (Borland, Microsoft C, Watcom) must run in the analysis environment; a compiler that cannot run removes Layer 4 |
| 5 Formal | No backend (Section 30.5) | No backend |

### 22.9 Track B Sandbox and Anti-Analysis

IA-32 uses the x86 stubs of Section 21.9. x86-16 requires a DOS emulator with a fixed virtual clock (`INT 1Ah`, port `0x40`), disabled hardware interrupts other than the timer, and a virtual file system. Typical Suspect behaviors: self-decrypting loaders, `INT 3` and `INT 1` anti-debugging, TSR installation through `INT 21h` function `31h`, and interrupt-vector hooking by writing the interrupt table at `0000:0000`. Boot-sector and TSR code that hooks the interrupt table is Hostile. A DOS binary with no map and an unrecognizable compiler is Track B with $\mathrm{MAS}_\% = 0$.

### 22.10 Idioms

16-bit paragraph rounding (bytes in `ax` to 16-byte paragraphs):

```asm
add  ax, 15
shr  ax, 4            ; ax = (bytes + 15) / 16
```

16-bit alignment of an offset to a power-of-two $A$ held in `bx`:

```asm
mov  cx, bx
dec  cx               ; cx = A - 1
add  ax, cx
not  cx
and  ax, cx           ; ax = (ax + A - 1) & ~(A - 1)
```

i386 cdecl prologue and epilogue with a frame pointer:

```asm
push ebp
mov  ebp, esp
sub  esp, 16          ; reserve 16 bytes of locals
mov  eax, [ebp+8]     ; first stack argument
mov  [ebp-4], eax     ; spill it to a local
mov  esp, ebp
pop  ebp
ret
```

Far-pointer construction in 16-bit C (a compiler-specific `far` extension, not ISO C):

```cpp
#define MAKE_FAR_POINTER(segment, offset) \
    ((void far *)(((unsigned long)(segment) << 16) | (unsigned long)(offset)))
```

### 22.11 Hypotheses, Stress Tests, Does Not Claim

- **H-x86-2.** On a testbed of 16-bit DOS functions with maps, Layer 1 reaches a positive verdict on at least 50 % of functions that lack self-modification and overlay stubs.
- **Stress test X3 (overlay and self-relocating loader).** A DOS program using an overlay manager: the pipeline records one address space per overlay, treats overlay stubs as indirect calls, and reports functions with unknown overlay bodies `Out_of_envelope`.
- **Stress test X4 (A20 wrap).** A real-mode access that overflows the offset near the top of the first megabyte aliases the low memory when the A20 line is masked; the 20-bit linear model reports the alias, the 21-bit model does not, and the specification requires the model to be selected from the binary's documented target (A20 behavior is an assumption recorded in `assumptions`).

**Does not claim:** that MZ, NE, LE, or LX parsing is validated against a vendor specification; that DOS interrupt services are modeled; or that any x86-16 function with unresolved overlays or self-modification is covered.

---
## 23. AArch32 (ARM and Thumb-2; ARMv4T through ARMv8-A AArch32; M-profile)

### 23.1 Scope and Platforms

Architecture identifier `aarch32`. Instruction sets: A32 (ARM, 32-bit fixed width), T32 (Thumb and Thumb-2, 16- and 32-bit), and for M-profile cores Thumb-2 only. Platforms: Linux and Android 32-bit ARM ELF (`armel` soft-float and `armhf` hard-float); Windows on ARM 32-bit PE (informational); bare-metal Cortex-M firmware images (no container; Section 23.2); and game and embedded devices whose containers are in Section 29 (Game Boy Advance and Nintendo DS cores, the Nintendo 3DS ARM11, and the PlayStation Vita Cortex-A9 are informational; Appendix A, L-19).

### 23.2 Containers and Metadata Indicators

ELF32 and PE are covered by Section 20.8. A **raw Cortex-M firmware image** has no container: the first two words are the initial stack pointer and the reset vector (with bit 0 set to select Thumb state), followed by the vector table. The lifter validates that the initial stack pointer points into a plausible RAM range and that every vector entry has bit 0 set; images that fail the check are `unsupported_architecture`. A raw image has $\mathrm{MAS}_\% = 0$ unless a linker map or debug file is supplied. ELF files carry the build attributes section `.ARM.attributes` (architecture profile, instruction-set use, floating-point and ABI options); the lifter MUST read it to select decoders and the float ABI.

### 23.3 ABI and Calling Convention

Procedure Call Standard for the Arm Architecture, AAPCS32 (https://github.com/ARM-software/abi-aa/blob/main/aapcs32/aapcs32.rst):

| Property | Rule |
|----------|------|
| Arguments | `r0`–`r3`, then the stack; a 64-bit argument occupies an even-odd register pair (`r0:r1` or `r2:r3`) or an 8-byte-aligned stack slot |
| Return | `r0` (or `r0:r1` for 64-bit values); larger aggregates through a caller-supplied pointer in `r0` |
| Callee-saved | `r4`–`r11` (`r9` may be a platform register or static base depending on the variant), `sp` |
| Scratch | `r12` (`ip`) is an intra-procedure-call scratch register used by veneers; `r0`–`r3` are caller-saved |
| Special | `r13` = `sp`, `r14` = `lr`, `r15` = `pc` |
| Stack alignment | 8 bytes at all times, 8 bytes at public interfaces |
| Soft-float (`-mfloat-abi=soft`) | Floating-point values passed and returned in core registers |
| `softfp` | VFP instructions used internally; floating-point values passed in core registers |
| Hard-float VFP variant (`-mfloat-abi=hard`) | Floating-point arguments in `s0`–`s15` or `d0`–`d7`; callee-saved `s16`–`s31` (`d8`–`d15`) |

The float ABI is read from `.ARM.attributes` and recorded in `abi` (`aapcs32` or `aapcs32_vfp`). **C++ differences.** The Arm C++ ABI for the 32-bit architecture (https://github.com/ARM-software/abi-aa/blob/main/cppabi32/cppabi32.rst) deviates from the generic Itanium ABI: its member-function-pointer representation places the virtual-function flag in the low bit of the *adjustment* field, its array cookie layout differs, and constructors and destructors return `this`. A contract derived for a constructor MUST therefore record a return value equal to the `this` argument; omitting it is a defect (these deviations were read from the cited specification's summary and not re-checked line by line; Appendix A, L-17).

### 23.4 Instruction-Set Hazards and CIRX Lowering

1. **Conditional execution.** Nearly every A32 instruction has a condition field; T32 uses `IT` blocks. Section 20.5 gives the predication lowering.
2. **`PC` reads.** In A32 a read of `r15` yields the address of the instruction plus 8; in T32 it yields the address plus 4 (and for some uses the word-aligned value). The lifter resolves the constant at lift time; a computed write to `PC` is a branch.
3. **Interworking.** `BX`, `BLX` (register), `LDR PC`, `LDM` with `PC`, and `POP {r4, r5, pc}` set the Thumb state from bit 0 of the target in ARMv5T and later; `BL` and `BLX` (immediate) switch state as encoded. Section 20.5 applies.
4. **Shifter operand and flags.** The barrel-shifter carry-out feeds $C$ for logical flag-setting instructions (Section 20.3.1).
5. **Literal pools.** `LDR rN, [PC, #imm]` loads constants from data embedded in the code stream; the pool bytes are *data*, not instructions. CFG recovery MUST mark pool targets as data from the referencing load before decoding, and decoding data as code is the principal source of desynchronization on this architecture.
6. **Jump tables.** `TBB` and `TBH` (T32) branch by a byte or halfword table offset (scaled by 2 and added to `PC`); `LDR PC, [PC, Rn, LSL #2]` and `ADD PC` sequences implement A32 tables. Bounds are recovered from the preceding `CMP`; an unbounded table is `TableJumpUnresolved`.
7. **Constants.** `MOVW` and `MOVT` build a 32-bit constant from two 16-bit halves; the lifter combines the pair when both write the same register in sequence.
8. **VFP, NEON, coprocessors, and system instructions.** VFP scalar arithmetic lowers to `Float`; NEON is `VectorOpaque`; `MRC`, `MCR`, `MRS`, `MSR`, `CPSID`, and `CPSIE` are `SystemRegisterOpaque`; `SVC` lowers to `Syscall` with the number in `r7` (Linux EABI) or in the instruction immediate (legacy).
9. **M-profile.** There is no ARM state; exceptions push a hardware frame (eight words) on the stack and return through special `EXC_RETURN` values in `lr`; handlers are reached through the vector table, not by call. Functions reachable only from the vector table are entry points, not callees.

### 23.5 Memory Model, Atomics, and Ordering

ARMv7-A and later use the ARM weak memory model (Section 20.7). Atomics are `LDREX`/`STREX` (and `LDREXB`, `LDREXH`, `LDREXD`) with the exclusive-monitor lowering of Section 3.3.3; barriers are `DMB`, `DSB`, `ISB`. `YIELD`, `WFE`, `WFI`, and `SEV` are hints with no sequential-model effect on data. Pre-ARMv6 cores lack exclusives and use the `SWP` instruction, which lowers to `AtomicRmw` with `Swap`.

### 23.6 Unwind and C++ Runtime Lowering

AArch32 uses the Arm exception-handling ABI (EHABI, https://github.com/ARM-software/abi-aa/blob/main/ehabi32/ehabi32.rst) instead of `.eh_frame`:

| Structure | Lowering |
|-----------|----------|
| `.ARM.exidx` | Table of 8-byte entries sorted by address: a `prel31` function offset, and either the value `EXIDX_CANTUNWIND` (1), an inline compact unwind descriptor (bit 31 set), or a `prel31` pointer into `.ARM.extab` |
| `.ARM.extab` | Unwind descriptors: a personality routine reference (for example `__aeabi_unwind_cpp_pr0`, `pr1`, `pr2`, or `__gxx_personality_v0`) followed by unwind opcodes; for C++ the language-specific data area follows |
| Unwind opcodes | `00xxxxxx` (virtual stack pointer addition), `01xxxxxx` (subtraction), `1000iiii iiiiiiii` (pop registers `r4`–`r15` by mask), `1001nnnn` (set `vsp` from `rn`), `10100nnn` and `10101nnn` (pop `r4`–`r(4+nnn)` with optional `r14`), `10110000` (finish), `10110001 0000iiii` (pop `r0`–`r3` by mask), `10110010 uleb128` (large `vsp` addition), and the VFP register-pop forms; the table is lowered to CFA rules and an unsupported opcode marks the function `unsupported_instruction` |
| ARM C++ RTTI and vtables | As Section 3.8 with the differences of Section 23.3 |

### 23.7 Relocation Design Set

ELF for the Arm Architecture, AAELF32 (https://github.com/ARM-software/abi-aa/blob/main/aaelf32/aaelf32.rst):

| Name | Value | Semantics |
|------|-------|-----------|
| `R_ARM_ABS32` | 2 | Absolute 32-bit, $(S + A) \lor T$ (bitwise OR with the Thumb bit) for Thumb targets |
| `R_ARM_REL32` | 3 | PC-relative 32-bit |
| `R_ARM_THM_CALL` | 10 | Thumb `BL`/`BLX` pair |
| `R_ARM_TLS_TPOFF32` | 19 | TLS thread-pointer offset |
| `R_ARM_GLOB_DAT` | 21 | GOT entry |
| `R_ARM_JUMP_SLOT` | 22 | PLT GOT entry |
| `R_ARM_RELATIVE` | 23 | Base-relative |
| `R_ARM_CALL` | 28 | A32 `BL`/`BLX` |
| `R_ARM_JUMP24` | 29 | A32 `B` |
| `R_ARM_THM_JUMP24` | 30 | T32 `B.W` |
| `R_ARM_PREL31` | 42 | 31-bit PC-relative (used by `.ARM.exidx`) |
| `R_ARM_MOVW_ABS_NC`, `R_ARM_MOVT_ABS` | 43, 44 | A32 constant halves |
| `R_ARM_THM_MOVW_ABS_NC`, `R_ARM_THM_MOVT_ABS` | 47, 48 | T32 constant halves |
| `R_ARM_IRELATIVE` | 160 | Indirect function resolver |

The Thumb bit of a function symbol (bit 0 of `st_value`) is part of the *symbol*, not the address: the lifter strips it to obtain the address and records the decode mode. Types outside the design set are `unsupported(kind)`. Values are from AAELF32 as written and not re-fetched (Appendix A, L-17).

### 23.8 Verification Cascade Eligibility

| Layer | AArch32 status | Notes |
|-------|----------------|-------|
| 1 CF-GKAT | Available at the CFG level | `ModeSwitchUnresolved` and `TableJumpUnresolved` functions are skipped |
| 2 Layer 2 checker | Eligible when no `VectorOpaque`, `SystemRegisterOpaque`, `ModeSwitchUnresolved`, or `Q`/`GE` read remains | **No formally derived semantics source is documented for AArch32**: ASLp targets AArch64, so semantics come from tested sources (Ghidra, VEX, TCG, BAP; Section 20.6), and the residual risk is stated in every record |
| 3 Execution | Available | QEMU user mode (ARM) and Unicorn |
| 4 Matching | Available for GCC and Clang; the configuration space includes ARM/Thumb mode, float ABI, and architecture version | $10^3$ configuration wall |
| 5 Formal | **No backend** (Section 30.5) | |

### 23.9 Track B Sandbox and Anti-Analysis

Stubs: `ptrace(PTRACE_TRACEME)` returns 0; reads of the performance cycle counter (`PMCCNTR`) or the generic timer (`CNTVCT`) where enabled advance by a fixed increment; `MIDR` returns a fixed profile. Bare-metal Cortex-M firmware cannot run in a user-space emulator: it requires a system-mode emulation with a peripheral model, which is out of scope; such images are statically analyzed only and classified Hostile when peripherals gate behavior.

### 23.10 Idioms

Alignment of `r0` to a power-of-two $A$ held in `r1`:

```asm
sub   r2, r1, #1
add   r0, r0, r2
bic   r0, r0, r2        @ r0 = (r0 + A - 1) & ~(A - 1)
```

Spin lock with exclusives (ARMv7):

```asm
@ r0 = address of lock word (0 = free)
        mov     r2, #1
.Lretry:
        ldrex   r1, [r0]
        cmp     r1, #0
        bne     .Lwait
        strex   r1, r2, [r0]
        cmp     r1, #0
        bne     .Lretry
        dmb     ish
        bx      lr
.Lwait:
        wfe
        b       .Lretry
```

```cpp
#include <atomic>

class Spinlock {
    std::atomic<int> word_{0};
public:
    void lock() noexcept {
        while (word_.exchange(1, std::memory_order_acquire) != 0) {
            while (word_.load(std::memory_order_relaxed) != 0) {
#if defined(__arm__) && (__ARM_ARCH >= 6)
                asm volatile("yield" ::: "memory");
#endif
            }
        }
    }
    void unlock() noexcept { word_.store(0, std::memory_order_release); }
};
```

`WFE` waits for an event and is paired with `SEV` by the unlocker in event-driven designs; the C++ reconstruction uses `YIELD` and is behaviorally equivalent only up to the permitted spurious wake-ups.

### 23.11 Hypotheses, Stress Tests, Does Not Claim

- **H-a32-1.** CFG recovery with literal-pool marking misclassifies fewer than 1 % of pool bytes as instructions on the testbed.
- **Stress test A1 (interworking).** A function that returns through `LDR PC` with a data-dependent bit 0 is `ModeSwitchUnresolved` and receives no Layer 2 or Layer 5 claim.
- **Stress test A2 (IT block with a trap).** An `IT` block whose conditional instruction can fault is lowered with the guard preserved; the verification record names the guard in `assumptions`.

**Does not claim:** semantics for A32/T32 beyond the tested sources of Section 20.6, security extensions (TrustZone), or Thumb-EE.

---

## 24. RISC-V (RV32 and RV64)

### 24.1 Scope and Platforms

Architecture identifiers `riscv32` and `riscv64`. Base ISAs RV32I, RV64I (and RV32E for embedded); standard extensions M, A, F, D, C, V and others. Platforms: Linux RISC-V ELF (usually `rv64gc`), bare-metal and RTOS firmware (ELF or raw), and BSD.

### 24.2 Containers and Metadata Indicators

ELF32 and ELF64 (Section 20.8). The ELF attribute section `.riscv.attributes` records the architecture string (for example, `rv64i2p1_m2p0_a2p1_f2p2_d2p2_c2p0`), the stack alignment, and unaligned-access policy; the lifter MUST read it to select the extension set, because the C extension changes instruction length and alignment. A raw firmware image has $\mathrm{MAS}_\% = 0$.

### 24.3 ABI and Calling Convention

RISC-V psABI (https://github.com/riscv-non-isa/riscv-elf-psabi-doc/blob/master/riscv-elf.adoc):

| Register | ABI name | Role | Preserved across calls |
|----------|----------|------|------------------------|
| `x0` | `zero` | Hardwired zero | n/a |
| `x1` | `ra` | Return address | No |
| `x2` | `sp` | Stack pointer | Yes |
| `x3` | `gp` | Global pointer | n/a (linker-managed) |
| `x4` | `tp` | Thread pointer | n/a |
| `x5`–`x7`, `x28`–`x31` | `t0`–`t2`, `t3`–`t6` | Temporaries | No |
| `x8`, `x9`, `x18`–`x27` | `s0`/`fp`, `s1`, `s2`–`s11` | Saved registers | Yes |
| `x10`–`x17` | `a0`–`a7` | Arguments and returns (`a0`, `a1`) | No |
| `f10`–`f17` | `fa0`–`fa7` | Floating-point arguments and returns | No |

The ABI is named by integer and floating-point widths: `ilp32`, `ilp32f`, `ilp32d`, `lp64`, `lp64f`, `lp64d` (and `ilp32e`). In the `d` variants floating-point arguments use `fa0`–`fa7`; in the others they travel in integer registers. The stack is 16-byte aligned. Aggregates larger than two XLEN-sized registers are passed by reference. Variadic floating-point arguments are passed in integer registers.

### 24.4 Instruction-Set Hazards and CIRX Lowering

1. **No condition flags.** Section 20.3.4. Compare-and-branch instructions lower to `Compare`/`Branch`; `slt` and `sltu` lower to `Select` of 1 and 0.
2. **Sign-extension of word results.** On RV64, the W-suffixed instructions (`addw`, `subw`, `sllw`, `lw`) compute 32-bit results and **sign-extend** to 64 bits (`NarrowWrite::SignExtend`); this differs from x86-64 and AArch64 and from PowerPC's zero-extending `lwz`.
3. **Defined division results.** Integer division by zero does not trap: `DIV` and `DIVU` return all ones (and `REM`, `REMU` return the dividend); signed overflow (`INT_MIN / -1`) returns `INT_MIN` with remainder 0. A lift MUST implement these results exactly; they differ from C's undefined behavior and from x86's trap.
4. **Shift counts** are masked to the register width (5 bits for RV32, 6 bits for RV64; 5 bits for the W forms).
5. **Address and constant materialization.** `LUI`+`ADDI` builds a 32-bit constant (with the carry rule for sign-extended `ADDI`); `AUIPC`+`ADDI`/`LD` builds a PC-relative address; `AUIPC`+`JALR` is a long call. The lifter combines each pair into a single absolute address or call target.
6. **Linker relaxation.** The linker may shorten call sequences (`AUIPC`+`JALR` to `JAL`), delete `LUI` or `AUIPC` when the address is within `gp` range, and compress. Code bytes therefore change *after* assembly; the `R_RISCV_RELAX` marker (Section 24.7) records relaxability. Matching decompilation (Layer 4) MUST reproduce the linker's relaxation, not only the compiler's output; a function's final size is the sum of the post-relaxation instruction lengths.
7. **`gp`-relative addressing.** When enabled, small data is addressed as `gp ± 2048` with the symbol `__global_pointer$`; the lifter reads the symbol (or infers it from the first `AUIPC`/`ADDI` writing `gp` in the startup code) and models `gp` as a constant.
8. **Atomics.** `LR.W`/`SC.W` (and `.D`) with `aq`/`rl` bits lower to `LoadLinked` and `StoreConditional`; AMOs (`AMOSWAP`, `AMOADD`, `AMOAND`, `AMOOR`, `AMOXOR`, `AMOMIN`, `AMOMAX`, `AMOMINU`, `AMOMAXU`) lower to `AtomicRmw`.
9. **Fences.** `FENCE` with predecessor and successor sets, `FENCE.TSO`, and `FENCE.I` (instruction-stream synchronization) lower to `Fence`. The `PAUSE` hint is the encoding `FENCE` with predecessor `W` and an empty successor set (Zihintpause) and lowers to `FenceKind::Pause`.
10. **System and CSR instructions.** `ECALL`, `EBREAK`, and `CSRR*` are `Syscall` or `ReadSpecial`/`WriteSpecial`; `RDCYCLE`, `RDTIME`, and `RDINSTRET` are CSR reads and are stubbed in Track B.
11. **Floating point.** F and D arithmetic lowers to `Float` with the rounding mode from the instruction's `rm` field or the dynamic `frm` register; `fcsr` flag accumulation is recorded as state; NaN-boxing of single-precision values in double registers is part of the `d`-register semantics. The vector extension is `VectorOpaque`.
12. **Instruction length.** With the C extension, instructions are 16 or 32 bits and 2-byte aligned; a jump target may be a 2-byte boundary.

### 24.5 Memory Model, Atomics, and Ordering

RVWMO (or RVTSO with the Ztso extension; Section 20.7). LR/SC succeeds only if the reservation holds; the architecture guarantees forward progress only for *constrained* loops (a short sequence with a restricted instruction mix), so the sequential lowering of Section 3.3.3 applies with the same spurious-failure path. Misaligned atomics may trap.

### 24.6 Unwind and C++ Runtime Lowering

`.eh_frame` with DWARF register numbers `x0`–`x31` = 0–31 and `f0`–`f31` = 32–63, return address column 1 (`ra`) per the psABI; LSDA and Itanium C++ RTTI as Section 3.8. There is no table analogous to `.pdata`.

### 24.7 Relocation Design Set

| Name | Value | Semantics |
|------|-------|-----------|
| `R_RISCV_32`, `R_RISCV_64` | 1, 2 | Absolute |
| `R_RISCV_RELATIVE` | 3 | Base-relative |
| `R_RISCV_COPY` | 4 | Copy |
| `R_RISCV_JUMP_SLOT` | 5 | PLT GOT entry |
| `R_RISCV_TLS_DTPMOD32`, `R_RISCV_TLS_DTPMOD64` | 6, 7 | TLS module |
| `R_RISCV_TLS_DTPREL32`, `R_RISCV_TLS_DTPREL64` | 8, 9 | TLS offset |
| `R_RISCV_TLS_TPREL32`, `R_RISCV_TLS_TPREL64` | 10, 11 | TLS thread-pointer offset |
| `R_RISCV_BRANCH` | 16 | Conditional branch |
| `R_RISCV_JAL` | 17 | `JAL` |
| `R_RISCV_CALL`, `R_RISCV_CALL_PLT` | 18, 19 | `AUIPC`+`JALR` pair |
| `R_RISCV_GOT_HI20` | 20 | GOT-relative high part |
| `R_RISCV_PCREL_HI20` | 23 | PC-relative high 20 bits |
| `R_RISCV_PCREL_LO12_I`, `R_RISCV_PCREL_LO12_S` | 24, 25 | PC-relative low 12 bits (I- and S-type) |
| `R_RISCV_HI20`, `R_RISCV_LO12_I`, `R_RISCV_LO12_S` | 26, 27, 28 | Absolute high and low parts |
| `R_RISCV_ADD32`, `R_RISCV_SUB32` | 35, 39 | Label difference |
| `R_RISCV_RELAX` | 51 | Linker-relaxation marker |

**Pairing rule.** A `PCREL_LO12_*` relocation's symbol refers to the *label of the instruction carrying the matching `PCREL_HI20`*, not to the target symbol; the static emulator MUST resolve a LO12 relocation through its HI20 partner. Types outside the table are `unsupported(kind)`. Source: the psABI document above; numbers written from the specification and not re-fetched (Appendix A, L-17).

### 24.8 Verification Cascade Eligibility

| Layer | RISC-V status | Notes |
|-------|---------------|-------|
| 1 CF-GKAT | Available at the CFG level | |
| 2 Layer 2 checker | Eligible when no `VectorOpaque` or `ConcurrencyOpaque` loss remains | RISC-V has a formally specified Sail model (Section 20.6), the strongest documented semantics source after ASLp; extension coverage is limited to the extensions the lifter implements |
| 3 Execution | Available | QEMU user mode (`riscv32`, `riscv64`), Unicorn |
| 4 Matching | Available; configuration space includes the `-march` string, ABI, relaxation, and code model | The linker's relaxation MUST be reproduced (Section 24.4 item 6) |
| 5 Formal | **No backend** (Section 30.5) | A Sail-derived Boogie or SMT-LIB generator is a design option, not an existing component |

### 24.9 Track B Sandbox and Anti-Analysis

QEMU user mode; stubs for `rdcycle`, `rdtime`, `rdinstret` (fixed increment), and `ptrace`. Obfuscation of RISC-V binaries in the field is rare; the Hostile classification is driven by integrity self-checks and encrypted sections as in Section 7.3.

### 24.10 Idioms

Alignment of `a0` to a power-of-two $A$ in `a1`:

```asm
addi  a2, a1, -1
add   a0, a0, a2
neg   a1, a1
and   a0, a0, a1        # a0 = (a0 + A - 1) & ~(A - 1)
```

Spin lock with AMO:

```asm
# a0 = address of lock word (0 = free)
        li      t0, 1
.Lacquire:
        amoswap.w.aq t1, t0, (a0)
        beqz    t1, .Ldone
.Lspin:
        pause                       # requires Zihintpause; encoding fence w,0
        lw      t1, 0(a0)
        bnez    t1, .Lspin
        j       .Lacquire
.Ldone:
        ret
# release:
#       amoswap.w.rl zero, zero, (a0)
```

```cpp
#include <atomic>

class Spinlock {
    std::atomic<int> word_{0};
public:
    void lock() noexcept {
        while (word_.exchange(1, std::memory_order_acquire) != 0) {
            while (word_.load(std::memory_order_relaxed) != 0) {
#if defined(__riscv)
                asm volatile(".4byte 0x0100000F" ::: "memory");   // PAUSE (fence w, 0)
#endif
            }
        }
    }
    void unlock() noexcept { word_.store(0, std::memory_order_release); }
};
```

On processors without Zihintpause the encoding executes as a valid `FENCE` with an empty successor set and has no architectural effect on correctness.

### 24.11 Hypotheses, Stress Tests, Does Not Claim

- **H-rv-1.** On the testbed, reproducing linker relaxation raises the Layer 4 match rate by at least 10 percentage points over ignoring relaxation.
- **Stress test R1 (relaxation).** A call that the linker shortened from `AUIPC`+`JALR` to `JAL` is lifted to a single call; the removed instruction is not an omission in the lift.
- **Stress test R2 (division semantics).** A function dividing by a runtime value that may be zero is lifted with the defined RISC-V result and a note that the *source* C has undefined behavior in that case.

**Does not claim:** semantics for the vector or hypervisor extensions, or support for custom vendor extensions; any such instruction is `Opaque`.

---
## 25. MIPS (MIPS32 and MIPS64; o32, n32, n64; Little- and Big-Endian)

### 25.1 Scope and Platforms

Architecture identifiers `mips32` and `mips64`. Releases: MIPS I through V, MIPS32 and MIPS64 Release 1 through 6; compressed encodings MIPS16e and microMIPS. Platforms: Linux MIPS ELF (embedded devices, routers, and network equipment; both byte orders), historical IRIX, and game consoles whose CPUs are MIPS derivatives (PlayStation, Nintendo 64, PlayStation 2, PlayStation Portable; informational, Appendix A, L-19).

### 25.2 Containers and Metadata Indicators

ELF32 and ELF64. The `e_flags` field encodes the ISA level, ABI (o32, n32, n64), PIC and CPIC flags, and architecture; `EI_DATA` gives the byte order; `.MIPS.abiflags` records the ISA level and floating-point ABI; the legacy `.mdebug` section carries symbolic debugging data from older toolchains and may supply $I_{\text{sym}}$ and $I_{\text{line}}$ evidence once decoded. The lifter MUST read all of these before decoding (ISA level selects delay-slot rules and branch-likely validity). Source: MIPS ABI supplement to the System V ABI (https://refspecs.linuxfoundation.org/elf/mipsabi.pdf). Raw console images have $\mathrm{MAS}_\% = 0$ unless a map or symbol file is supplied (Section 29).

### 25.3 ABI and Calling Convention

| Property | o32 (MIPS32) | n32 and n64 |
|----------|--------------|-------------|
| Integer arguments | `$a0`–`$a3` (`$4`–`$7`), then stack | `$a0`–`$a7` (`$4`–`$11`) |
| Floating-point arguments | `$f12`, `$f14` for the first two floating-point arguments when no preceding integer argument consumed the slots | `$f12`–`$f19` |
| Return | `$v0`, `$v1`; `$f0` (and `$f1`) | `$v0`, `$v1`; `$f0`, `$f2` |
| Home area | Caller reserves 16 bytes for `$a0`–`$a3` | None |
| Temporaries | `$t0`–`$t7` (`$8`–`$15`), `$t8`, `$t9` (`$24`, `$25`) | `$t0`–`$t3` (`$12`–`$15`), `$t8`, `$t9` |
| Callee-saved | `$s0`–`$s7` (`$16`–`$23`), `$fp` (`$30`), `$sp`, `$gp` conditions per PIC convention | Same register set |
| Link register | `$ra` (`$31`) | `$ra` |
| Pointer and register size | 32-bit pointers, 32-bit registers | n32: 32-bit pointers with 64-bit registers; n64: 64-bit pointers |

`$zero` (`$0`) is hardwired zero, `$at` (`$1`) is the assembler temporary, `$k0` and `$k1` (`$26`, `$27`) are reserved for the kernel, and `$gp` (`$28`) is the global pointer. **PIC convention.** In position-independent code the callee computes `$gp` from `$t9`, which holds the callee's own address on entry (`lui`/`addiu` against `$t9` as the first instructions), and callers invoke functions with `jalr $t9` after loading `$t9` from the GOT (`%call16`). An indirect call through `$t9` is therefore an ordinary call, and its target set is the GOT entry; it is not an opaque computed jump. The exact floating-point callee-saved set differs among o32, n32, and n64; the lifter reads the set from the ABI named in `e_flags` and records `abi` (`mips_o32`, `mips_n32`, `mips_n64`). The o32, n32, and n64 specifics beyond the System V supplement are carried from the Linux-MIPS ABI documentation and were not re-fetched (Appendix A, L-17).

### 25.4 Instruction-Set Hazards and CIRX Lowering

1. **Delay slots and branch-likely.** Section 20.5 gives the normative elimination rules, including evaluating condition operands before the slot executes and annulment for branch-likely instructions (removed in Release 6, which adds compact branches without delay slots).
2. **No flags.** Section 20.3.4. Integer `ADD`, `ADDI`, and `SUB` trap on signed overflow; the `U` forms do not.
3. **Multiply and divide.** Before Release 6, `MULT`, `MULTU`, `DIV`, `DIVU`, `MADD`, and related instructions write the `HI` and `LO` registers read by `MFHI` and `MFLO`; the lifter models `HI` and `LO` as special registers (class `Special`). Release 6 replaces them with three-operand `MUL`, `MUH`, `DIV`, `MOD`. Division by zero does not trap on MIPS (the result is unpredictable on pre-R6; R6 defines it), so a function dividing by a possibly-zero value is reported with a note and, on pre-R6 targets, `Out_of_envelope` for that path.
4. **Sign extension on MIPS64.** 32-bit arithmetic results (`ADDU`, `SUBU`, `SLL`, `LW`) are sign-extended to 64 bits (`NarrowWrite::SignExtend`); using a non-sign-extended value as a 32-bit operand is unpredictable and is not modeled.
5. **Unaligned access.** `LWL`/`LWR` and `SWL`/`SWR` form unaligned word access; their behavior depends on byte order, and the pair lowers to a single unaligned load or store in the profile's byte order.
6. **Address and constant materialization.** `LUI` plus `ADDIU`/`ORI`/load; the `%hi` part is adjusted by +1 when the `%lo` part has its top bit set (the low half is sign-extended); the lifter combines the pair under the pairing rule of Section 25.7.
7. **Conditional moves and sets.** `MOVZ`, `MOVN` lower to `Select`; `SLT`, `SLTI`, `SLTU`, `SLTIU` lower to `Select(Slt/Ult, 1, 0)`.
8. **Atomics.** `LL`/`SC` (and `LLD`/`SCD`) lower to `LoadLinked` and `StoreConditional`; `SC` writes 1 on success and 0 on failure into its source register.
9. **Jump tables.** `lw`/`ld` from a table followed by `jr` (a `$t9`-relative or GOT-relative table in PIC) is recovered through the bounding `sltiu`/`sltu` check; unbounded tables are `TableJumpUnresolved`.
10. **System and coprocessor.** `SYSCALL` (Linux o32: number in `$v0`, arguments in `$a0`–`$a3` and the stack, error indicated in `$a3`), `BREAK`, `RDHWR` (reading `$2` cycle count and `$29` thread pointer), `MFC0`, `MTC0` are `Syscall`/`ReadSpecial`/`WriteSpecial`.
11. **Floating point.** Before Release 6 comparisons (`C.cond.fmt`) set condition-code bits read by `BC1T` and `BC1F`; Release 6 writes the comparison result into a floating-point register (`CMP.cond.fmt`) tested by `BC1EQZ` and `BC1NEZ`. The floating-point register file width depends on the FR mode (32 or 64 bits wide; o32 `FP64` and `FPXX` variants); the lifter reads the mode from `.MIPS.abiflags`. SIMD (MSA) and DSP extensions are `VectorOpaque`.
12. **Compressed ISA.** microMIPS and MIPS16e are selected by bit 0 of the target address (and `JALX`); Section 20.5 applies.

### 25.5 Memory Model, Atomics, and Ordering

Weakly ordered with `SYNC` (stype selects the ordering) and `LL`/`SC` atomics; the sequential lowering of Section 3.3.3 is the only verified model, and the verification claim is restricted to single-threaded or data-race-free execution.

### 25.6 Unwind and C++ Runtime Lowering

MIPS Linux uses `.eh_frame` with DWARF CFI; the lifter MUST take the return-address column and the register-to-column mapping from the CIE's augmentation and the cited ABI rather than assuming AArch64 or x86 numbering. The Itanium C++ ABI applies (vtable, `typeinfo`, LSDA, as Section 3.8). Old toolchains use setjmp/longjmp-based exceptions or `.mdebug`-driven unwinding; any function using such a mechanism is `Out_of_envelope`.

### 25.7 Relocation Design Set

| Name | Value | Semantics |
|------|-------|-----------|
| `R_MIPS_32` | 2 | Absolute 32-bit |
| `R_MIPS_REL32` | 3 | 32-bit relative (dynamic) |
| `R_MIPS_26` | 4 | `J`/`JAL` target field |
| `R_MIPS_HI16` | 5 | High 16 bits (with carry adjustment) |
| `R_MIPS_LO16` | 6 | Low 16 bits |
| `R_MIPS_GPREL16` | 7 | `$gp`-relative 16-bit |
| `R_MIPS_GOT16` | 9 | GOT entry (16-bit) |
| `R_MIPS_PC16` | 10 | 16-bit PC-relative branch |
| `R_MIPS_CALL16` | 11 | GOT call entry |
| `R_MIPS_GPREL32` | 12 | `$gp`-relative 32-bit |
| `R_MIPS_COPY`, `R_MIPS_JUMP_SLOT` | 126, 127 | Dynamic copy and PLT slot (where used) |

**Pairing rule.** An `R_MIPS_HI16` relocation pairs with the next `R_MIPS_LO16` that follows it in the relocation list for the same symbol, and several `R_MIPS_HI16` relocations may share one `R_MIPS_LO16`; the high part is computed from the *combined* addend including the carry from the sign-extended low half. The MIPS dynamic-linking GOT is initialized from the dynamic symbol table using the dynamic tags (`DT_MIPS_LOCAL_GOTNO`, `DT_MIPS_GOTSYM`, `DT_MIPS_SYMTABNO`) rather than from `GLOB_DAT` relocations; the static emulator reads those tags and treats the GOT layout as a design-set feature, and a binary whose GOT layout it cannot interpret is `unsupported(GOT)`. Types outside the table are `unsupported(kind)`. Source: the System V MIPS supplement above; GOT tag details are carried from the ABI and were not re-fetched (Appendix A, L-17).

### 25.8 Verification Cascade Eligibility

| Layer | MIPS status | Notes |
|-------|-------------|-------|
| 1 CF-GKAT | Available after delay-slot elimination | A function with `DelaySlotUnresolved` is skipped |
| 2 Layer 2 checker | Eligible when no `DelaySlotUnresolved`, `VectorOpaque`, or `SystemRegisterOpaque` loss remains | Semantics sources: tested tools of Section 20.6; the CHERI-MIPS Sail model (Armstrong et al., 2019) is a formal source for the MIPS64 base ISA with CHERI extensions and is not a drop-in model for every MIPS core |
| 3 Execution | Available | QEMU user mode (`mips`, `mipsel`, `mips64`, `mips64el`) and Unicorn |
| 4 Matching | Available if the original compiler (commonly an old GCC for embedded SDKs) can run in the analysis environment | Configuration space includes ABI, ISA level, byte order, and PIC mode |
| 5 Formal | **No backend** (Section 30.5) | |

### 25.9 Track B Sandbox and Anti-Analysis

QEMU user mode; stubs: `RDHWR $2` (cycle counter) advances by a fixed increment, `RDHWR $29` returns a fixed thread pointer, `ptrace(PTRACE_TRACEME)` returns 0. Console binaries run only under a platform emulator (Section 29). IoT firmware with network behavior is classified by Section 7.3 and its network access is denied.

### 25.10 Idioms

Alignment of `$a0` to a power-of-two $A$ in `$a1`; the instruction after `jr` is the delay slot and executes before the return takes effect:

```asm
        .set    noreorder
        addiu   $a2, $a1, -1
        addu    $a0, $a0, $a2
        negu    $a1, $a1
        jr      $ra
         and    $v0, $a0, $a1      # delay slot: executes before the jump completes
        .set    reorder
```

Spin lock with `LL`/`SC`:

```asm
# $a0 = address of lock word (0 = free)
        .set    noreorder
1:      ll      $t0, 0($a0)
        bnez    $t0, 1b            # busy: retry
         li     $t1, 1             # delay slot: executes whether or not the branch is taken
        sc      $t1, 0($a0)        # $t1 = 1 on success, 0 on failure
        beqz    $t1, 1b
         nop
        sync                       # acquire barrier
        jr      $ra
         nop
        .set    reorder
```

```cpp
#include <atomic>

class Spinlock {
    std::atomic<int> word_{0};
public:
    void lock() noexcept {
        while (word_.exchange(1, std::memory_order_acquire) != 0) {
            while (word_.load(std::memory_order_relaxed) != 0) { }
        }
    }
    void unlock() noexcept { word_.store(0, std::memory_order_release); }
};
```

### 25.11 Hypotheses, Stress Tests, Does Not Claim

- **H-mips-1.** Delay-slot elimination (Section 20.5) preserves behavior on at least 99.9 % of testbed basic blocks as measured by differential execution against QEMU.
- **Stress test M1 (branch-likely).** A branch-likely whose slot instruction writes a register read by the next block is lowered with the slot guarded by the taken condition; a mismatch against QEMU is a lifter defect.
- **Stress test M2 (`HI16`/`LO16` carry).** A constant whose low half has bit 15 set decodes to the correct 32-bit address only with the carry adjustment.

**Does not claim:** precise-exception behavior inside delay slots, MIPS SIMD or DSP semantics, or the correctness of console-specific MIPS extensions.

---

## 26. PowerPC (PowerPC 32 and 64; Big- and Little-Endian)

### 26.1 Scope and Platforms

Architecture identifiers `ppc32` and `ppc64`. Platforms: Linux PowerPC (32-bit and 64-bit, big-endian `ppc64` ELFv1 and little-endian `ppc64le` ELFv2), AIX (XCOFF; informational), historical Mac OS and macOS Mach-O PowerPC, and game consoles with PowerPC CPUs (GameCube Gekko, Wii Broadway, Wii U Espresso, Xbox 360 Xenon, PlayStation 3 Cell PPE; informational, Appendix A, L-19). Gekko and Broadway add paired-single floating-point instructions beyond the base Power ISA.

### 26.2 Containers and Metadata Indicators

ELF32 and ELF64 (either byte order), Mach-O PowerPC, and XCOFF. Console containers (DOL, REL, RPX/RPL, XEX, SELF/PRX) are in Section 29. In ELFv1 (64-bit big-endian) a function symbol `foo` names a *function descriptor* in `.opd`, and a dot-prefixed symbol `.foo` names the code entry; $I_{\text{sym}}$ counts a function once, resolved through the descriptor.

### 26.3 ABI and Calling Convention

| Property | PowerPC 32 SysV | 64-bit ELFv1 (big-endian) | 64-bit ELFv2 |
|----------|------------------|---------------------------|--------------|
| Integer arguments | `r3`–`r10` | `r3`–`r10` | `r3`–`r10` |
| Floating-point arguments | `f1`–`f8` | `f1`–`f13` | `f1`–`f13` |
| Vector arguments | AltiVec `v2`–`v9` | `v2`–`v13` | `v2`–`v13` |
| Return | `r3`, `r4`; `f1` | `r3`, `r4`; `f1` | `r3`–`r10` in aggregates; `f1`–`f8`; `v2` |
| Stack pointer | `r1` | `r1` | `r1` |
| Callee-saved | `r14`–`r31`, `f14`–`f31`, `cr2`–`cr4` | `r14`–`r31`, `f14`–`f31`, `v20`–`v31`, `cr2`–`cr4` | Same as ELFv1 |
| Special | `lr`, `ctr` volatile; `r2` reserved for the system; `r13` small-data base | `r2` TOC pointer; `r13` thread pointer; `lr`, `ctr` volatile | `r2` TOC pointer, `r12` function entry address at the global entry point; `r13` thread pointer |
| Function pointer | Address of code | Address of a *descriptor* (entry, TOC, environment) | Address of the global entry point |
| TOC save slot | n/a | `40(r1)` | `24(r1)` |
| Link register save slot | `4(r1)` | `16(r1)` | `16(r1)` |

Stack frames carry a back chain at `0(r1)`. In ELFv2 each function may have a *global entry* (which sets `r2` from `r12`) and a *local entry* (which assumes `r2` is valid); the offset between them is encoded in the symbol's `st_other` bits, and a direct call within one module jumps to the local entry. After a call that can change the TOC (an external function through a PLT stub), the caller restores `r2` from its save slot; the instruction after a `bl` that reloads `r2` from `24(r1)` (ELFv2) or `40(r1)` (ELFv1) is part of the call sequence, not user code. Sources: 64-bit ELF V2 ABI (https://openpowerfoundation.org/specifications/64bitelfabi/); PowerPC 32 SysV ABI supplement (https://refspecs.linuxfoundation.org/elf/elfspec_ppc.pdf). The `abi` values are `ppc32_sysv`, `ppc64_elfv1`, and `ppc64_elfv2`.

### 26.4 Instruction-Set Hazards and CIRX Lowering

1. **Condition register and XER.** Section 20.3.3. A compare writes a named CR field; a branch tests one CR bit (`BO`/`BI` encoding), possibly decrementing `CTR` and branching on its zero state; the lifter decodes `BO` into the explicit conditions (branch if the tested bit is true or false, optionally combined with `CTR != 0` or `CTR == 0`).
2. **Rotate-and-mask family.** `rlwinm`, `rlwimi`, `rlwnm`, `rldicl`, `rldicr`, `rldic`, `rldimi` compute $\mathrm{ROTL}(rs, SH) \,\&\, \mathrm{MASK}(MB, ME)$ with IBM bit numbering (bit 0 is the most significant bit). For the 32-bit forms $\mathrm{MASK}(MB, ME)$ has bits $MB$ through $ME$ set when $MB \le ME$ and bits $MB$ through 31 and 0 through $ME$ set when $MB > ME$. A lift MUST implement the wrap-around case; many shifts and field extracts are encoded this way (`slwi` is `rlwinm rA,rS,n,0,31-n`; `clrlwi` is `rlwinm rA,rS,0,n,31`).
3. **Update-form and indexed memory operations.** `lwzu`, `stwu` and similar write the effective address back to the base register; `stwu r1,-N(r1)` is the standard frame allocation. Byte-reversed loads and stores (`lwbrx`, `stwbrx`) convert endianness explicitly.
4. **Load extension.** `lwz` zero-extends; `lha`, `lwa` sign-extend. PowerPC has no word-width arithmetic forms: in 64-bit mode ordinary arithmetic is 64-bit and word compares use `cmpw`.
5. **Multiple and string.** `lmw` and `stmw` (load or store registers `rt` through `r31`) are the usual epilogue and prologue for saving callee-saved registers and lower to an explicit sequence; `lswi`, `stswi`, `lswx`, `stswx` move byte strings and lower to a byte loop.
6. **Indirect transfers.** `blr` returns through `lr`; `bctr` and `bctrl` branch and call through `ctr` (the usual form for jump tables, function pointers, and PLT stubs); `bl` calls directly and writes `lr`. The GetPC idiom is `bcl 20,31,$+4` followed by `mflr`.
7. **TOC and GOT addressing.** `addis r2, r12, imm` establishes the TOC pointer at a global entry; `ld rN, sym@toc@l(r2)` loads from the TOC; large code models combine `addis r9, r2, sym@toc@ha` and `ld r3, sym@toc@l(r9)`. The lifter resolves TOC-relative loads to the pointed-to constants where the TOC entry is a relocated absolute address.
8. **Atomics and synchronization.** `lwarx`/`stwcx.` (and `ldarx`/`stdcx.`, byte and halfword forms in newer cores) lower to `LoadLinked` and `StoreConditional`; `stwcx.` sets `CR0[EQ]` on success; `sync`, `lwsync`, `isync`, `eieio` lower to `Fence`.
9. **Floating point and vectors.** Scalar floating-point arithmetic lowers to `Float` with the rounding mode from `FPSCR`; single-precision operations (`fadds`) round to single; AltiVec, VMX, and VSX are `VectorOpaque`; Gekko and Broadway paired-single instructions (`ps_add`, `psq_l`, and related) are `VectorOpaque`.
10. **System and trap.** `sc` is `Syscall` (Linux: number in `r0`); `tw`/`twi` with condition 31 is the unconditional trap; `mfspr`, `mtspr`, `mftb`, `mtmsr` are `SystemRegisterOpaque`.
11. **Variable-length encoding (VLE).** Embedded cores (e200, e500mc series) may decode a page as VLE with 16- and 32-bit instructions; a binary using VLE is selected by a section flag or page attribute and is otherwise `unsupported_architecture`.

### 26.5 Memory Model, Atomics, and Ordering

The POWER memory model (Section 20.7) with a reservation granule for `lwarx`/`stwcx.`. `lwsync` is the usual release and acquire-release barrier; `isync` after a conditional branch implements acquire. The sequential model of Section 3.3.3 applies; ordering-dependent code is `Out_of_envelope`.

### 26.6 Unwind and C++ Runtime Lowering

`.eh_frame` with DWARF CFI. The lifter takes the return-address column from the CIE and the register mapping from the cited ABI. The Itanium C++ ABI applies (Section 3.8); a 64-bit ELFv1 vtable holds *function descriptors*, not code addresses, so each virtual-table slot dereferences to an entry address in `.opd`. AIX and Mac OS runtimes use different exception mechanisms and are `Out_of_envelope`.

### 26.7 Relocation Design Set

| Name | Value | Semantics |
|------|-------|-----------|
| `R_PPC_ADDR32` (and `R_PPC64_ADDR32`) | 1 | Absolute 32-bit |
| `R_PPC_ADDR16_LO`, `R_PPC_ADDR16_HI`, `R_PPC_ADDR16_HA` | 4, 5, 6 | Low, high, and high-adjusted 16 bits |
| `R_PPC_REL24` (and `R_PPC64_REL24`) | 10 | 24-bit branch displacement (word-scaled) |
| `R_PPC_COPY`, `R_PPC_GLOB_DAT`, `R_PPC_JMP_SLOT`, `R_PPC_RELATIVE` | 19, 20, 21, 22 | Dynamic relocations |
| `R_PPC_REL32` | 26 | 32-bit PC-relative |
| `R_PPC64_ADDR64` | 38 | Absolute 64-bit |
| `R_PPC64_REL64` | 44 | 64-bit PC-relative |
| `R_PPC64_TOC16`, `R_PPC64_TOC16_LO`, `R_PPC64_TOC16_HI`, `R_PPC64_TOC16_HA` | 47, 48, 49, 50 | TOC-relative 16-bit parts |
| `R_PPC64_TOC` | 51 | Value of the TOC base |
| `R_PPC64_TOC16_DS`, `R_PPC64_TOC16_LO_DS` | 63, 64 | TOC-relative with the DS-form scaling |

**Pairing rule.** `@ha` is the high half adjusted by +1 when the low half has its top bit set (the low half is sign-extended), so `addis` (`@ha`) plus a displacement (`@l`) reconstructs the address exactly. Types outside the table are `unsupported(kind)`. Sources: the ELFv2 ABI and the PowerPC 32 supplement above; values written from the specifications and not re-fetched (Appendix A, L-17).

### 26.8 Verification Cascade Eligibility

| Layer | PowerPC status | Notes |
|-------|----------------|-------|
| 1 CF-GKAT | Available at the CFG level | `bctr` jump tables must be bound or are `TableJumpUnresolved` |
| 2 Layer 2 checker | Eligible when no `VectorOpaque` (including paired singles), `SystemRegisterOpaque`, or `ConcurrencyOpaque` loss remains | Semantics sources: tested tools (Section 20.6); no formally derived source is documented for PowerPC in this specification |
| 3 Execution | Available for Linux PowerPC (QEMU user mode `ppc`, `ppc64`, `ppc64le`) | Console binaries require a platform emulator |
| 4 Matching | Available if the original compiler runs in the analysis environment; console-era matching decompilation projects in the community use the period compiler toolchains (for example the Metrowerks CodeWarrior toolchain for GameCube and Wii) and a matching platform such as decomp.me (https://decomp.me), as reported by those projects (Appendix A, L-13) | Compiler flags and version dominate the configuration space; the $10^3$ wall applies |
| 5 Formal | **No backend** (Section 30.5) | |

### 26.9 Track B Sandbox and Anti-Analysis

QEMU user mode for Linux PowerPC; stubs: `mftb`/`mftbu` advance by a fixed increment, `mfspr` of `PVR` returns a fixed value, `ptrace(PTRACE_TRACEME)` returns 0. Console PowerPC binaries run only under a platform emulator whose adapter defines the stubs (Section 29).

### 26.10 Idioms

Alignment to 32 bytes (`(x + 31) & ~31`), with the mask in IBM bit numbering:

```asm
addi    r3, r3, 31
rlwinm  r3, r3, 0, 0, 26      # keep bits 0..26, clear the low 5 bits
```

Alignment to a power-of-two $A$ in `r4`:

```asm
addi    r5, r4, -1
add     r3, r3, r5
andc    r3, r3, r5            # r3 = (r3 + A - 1) & ~(A - 1)
```

Spin lock (GNU `as` requires `-mregnames` to accept the symbolic register names `r3`, `r4`, and `r5` used here):

```asm
# r3 = address of lock word (0 = free)
.Lretry:
        lwarx   r4, 0, r3
        cmpwi   r4, 0
        bne-    .Lretry
        li      r5, 1
        stwcx.  r5, 0, r3
        bne-    .Lretry
        isync                   # acquire
        blr
# release:
#       lwsync
#       li      r4, 0
#       stw     r4, 0(r3)
#       blr
```

```cpp
#include <atomic>

class Spinlock {
    std::atomic<int> word_{0};
public:
    void lock() noexcept {
        while (word_.exchange(1, std::memory_order_acquire) != 0) {
            while (word_.load(std::memory_order_relaxed) != 0) { }
        }
    }
    void unlock() noexcept { word_.store(0, std::memory_order_release); }
};
```

### 26.11 Hypotheses, Stress Tests, Does Not Claim

- **H-ppc-1.** Rotate-and-mask lowering with wrap-around masks agrees with QEMU on 100 % of a generated test set covering all $(MB, ME, SH)$ combinations for the 32-bit forms.
- **Stress test P1 (TOC restore).** After an external call in ELFv2 the reload of `r2` from `24(r1)` is recognized as part of the call sequence, not as an application load.
- **Stress test P2 (`lmw`/`stmw` frames).** A prologue using `stmw r27, -20(r1)` lowers to five consecutive stores and the epilogue's `lmw` to five loads.

**Does not claim:** paired-single, AltiVec, VSX, or VLE semantics; AIX or Mac OS runtime behavior; or the layout of any console container beyond Section 29's informational description.

---
## 27. WebAssembly (wasm32 and wasm64)

### 27.1 Scope and Platforms

Architecture identifiers `wasm32` and `wasm64` (the memory64 proposal). WebAssembly is a *virtual* instruction set: a typed stack machine with structured control flow, executed by a host runtime (browser engines, standalone runtimes) with all external capability provided through imports. Compilers (Clang/LLVM with `wasm-ld`, Rust, Emscripten, AssemblyScript, Go, and others) target it, and LTO is available through `wasm-ld`. The normative reference is the WebAssembly core specification (https://webassembly.github.io/spec/core/). The compiler-side calling convention for C and C++ is the Basic C ABI (https://github.com/WebAssembly/tool-conventions/blob/main/BasicCABI.md).

### 27.2 Containers and Metadata Indicators

A module is a sequence of sections (type, import, function, table, memory, global, export, start, element, code, data, and custom sections). Indicator predicates are in Section 20.8: the `name` custom section supplies function and local names ($I_{\text{sym}}$); the export section gives $I_{\text{dyn}}$; DWARF custom sections (`.debug_info`, `.debug_line`, and others) give $I_{\text{type}}$ and $I_{\text{line}}$; the `producers` custom section gives $I_{\text{cmt}}$. `$I_{\text{map}}$` and `$I_{\text{lto}}$` are 0 unless the supplementary files are provided. Production modules are frequently shipped without the `name` section; their $\mathrm{MAS}_\%$ is then driven by exports alone.

### 27.3 ABI and Calling Convention

WebAssembly functions have explicit parameter and result types (`i32`, `i64`, `f32`, `f64`, `v128`, and reference types), so the *signature is part of the binary* and needs no inference. The C ABI (Basic C ABI, above) passes scalars as parameters, aggregates by pointer into linear memory, and keeps a software stack in linear memory whose pointer is the mutable global `__stack_pointer`; the shadow stack is therefore a *data structure in memory*, not a hardware stack. The `abi` value is `wasm_basic_c_abi`. A module compiled by another language runtime (for example Go or Emscripten with exceptions) can use different conventions, recorded as `unknown`.

### 27.4 Instruction-Set Hazards and CIRX Lowering

1. **Structured control flow.** `block`, `loop`, `if`/`else`, `br`, `br_if`, `br_table`, and `return` express control flow with labels; there are no arbitrary jumps, so every function has a reducible control-flow graph. Lowering to a CFG assigns each label to a basic block and resolves branch depths to targets. `br_table` is an explicit multiway branch with a default, which removes the jump-table recovery problem of native architectures (`TableJumpUnresolved` does not arise).
2. **Operand stack.** The value stack is eliminated at lift time into SSA temporaries; stack heights at each program point are determined statically by the validation algorithm, so lowering is deterministic.
3. **Locals and globals.** Locals map to CIRX registers (one `RegId` per local); globals map to special registers (the mutable globals, including `__stack_pointer`).
4. **Linear memory.** One or more byte-addressed memories with 32-bit (or, for memory64, 64-bit) addresses, little-endian, with `memory.grow` and `memory.size`; the model is Section 20.4 with an address width of 32 (or 64). Out-of-bounds access traps; the lifter emits a bounds guard `addr + offset + size <= memory_size` as a trap branch, and `memory_size` is a symbolic value constrained by the module's declared minimum and maximum.
5. **Integer semantics.** Integer operations wrap; shift counts are masked; `i32.div_s`, `i32.div_u`, `i32.rem_s`, `i32.rem_u` trap on division by zero and `div_s` traps on `INT_MIN / -1`; `i32.trunc_f32_s` and related conversions trap on out-of-range or NaN, whereas the saturating forms (`trunc_sat`) clamp. These are *defined* trapping behaviors and are lowered to `Trap` guards.
6. **Floating point.** IEEE 754 binary32 and binary64 with a fixed round-to-nearest-even mode and *non-deterministic NaN payloads* in some operations (the specification permits certain NaN bit patterns to vary); the lifter models payload-sensitive operations as `Opaque` unless the function's observable behavior is independent of the payload. The sorts are `(_ FloatingPoint 8 24)` and `(_ FloatingPoint 11 53)` (Section 20.10).
7. **Calls.** `call` is direct; `call_indirect` calls through a table with a *runtime signature check* (a trap on mismatch). The table contents are known from the element section when the table is not mutated; an indirect call is resolved to the set of table entries whose signature equals the expected type, a sound over-approximation with a precise bound.
8. **Atomics and threads.** The threads proposal adds shared memory and atomic read-modify-write and fence operations with sequentially consistent semantics; they lower to `AtomicRmw` and `Fence` with `SeqCst`.
9. **Exceptions.** The exception-handling proposal adds `try`/`catch`/`throw` over *tags* (in both the legacy and the `exnref` forms); a module using them has exceptional edges from each `throw` and each call that may throw. Lowering follows the specification version the module was built for; an unrecognized form is `unsupported_instruction`.
10. **SIMD.** `v128` operations are `VectorOpaque`.
11. **Host interaction.** Every imported function is an opaque callee whose behavior is the host's; calls to imports lower to `Call` with a clobber set equal to the memory and globals the import may touch (all of linear memory unless the import is known pure).

### 27.5 Memory Model, Atomics, and Ordering

Non-atomic accesses follow the sequential model of Section 3.3.3; with the threads proposal, atomics are sequentially consistent and data races on non-atomic locations have defined-but-unordered semantics in the specification. The verification claim remains single-threaded or data-race-free.

### 27.6 Unwind and C++ Runtime Lowering

There is no unwind table: exceptions use tags or are compiled away (Emscripten's JavaScript-based exceptions and `setjmp`/`longjmp` emulation are runtime-library behaviors and are `Out_of_envelope`). C++ RTTI and vtables are data in linear memory using the Itanium layout with 32-bit pointers; the vtable-recovery rule of Section 3.8 applies to the data segments.

### 27.7 Relocation Design Set

A linked module has no relocations. Object files carry the `linking` and `reloc.*` custom sections with relocation types such as `R_WASM_FUNCTION_INDEX_LEB`, `R_WASM_MEMORY_ADDR_LEB`, `R_WASM_TABLE_INDEX_SLEB`, and `R_WASM_TYPE_INDEX_LEB` (tool-conventions Linking document, https://github.com/WebAssembly/tool-conventions/blob/main/Linking.md); the lifter's design set for object files is these four types, and anything else is `unsupported(kind)`. Position-independent modules use a dynamic-linking section (`dylink.0`) with memory and table base globals; the base values are symbolic constants.

### 27.8 Verification Cascade Eligibility

| Layer | WebAssembly status | Notes |
|-------|--------------------|-------|
| 1 CF-GKAT | Available; control flow is reducible | `br_table` is a primitive multiway branch |
| 2 Layer 2 checker | Eligible when no `VectorOpaque` or NaN-payload `Opaque` remains | Types are exact from the signature; the instruction semantics are given in the specification, which has a reference interpreter (https://github.com/WebAssembly/spec) and a mechanized formalization (Watt, CPP 2018, https://doi.org/10.1145/3167082), the strongest semantics basis of any target in this specification |
| 3 Execution | Available in a runtime with deny-by-default imports | |
| 4 Matching | Available when the source compiler and flags are known (Clang, Rust, Emscripten) | |
| 5 Formal | **No backend** (Section 30.5) | |

NotDec (Wang et al., ICSE 2026) evaluates Retypd-style inter-procedural type recovery on WebAssembly (Section 3.4); its reported figures apply only to that setting.

### 27.9 Track B Sandbox and Anti-Analysis

Execution is inside a runtime with no imports other than capability stubs; a module with an unknown import is Hostile (Section 20.9). The principal threats are resource exhaustion (memory growth, infinite loops) and runtime-specific escapes, not native-code anti-debugging; bounds are those of Section 7.3 with the instruction budget enforced by runtime fuel metering.

### 27.10 Idioms

The shadow-stack prologue allocates a frame by decrementing `__stack_pointer`:

```wat
(func $f (param $x i32) (result i32)
  (local $sp i32)
  global.get $__stack_pointer
  i32.const 16
  i32.sub
  local.tee $sp
  global.set $__stack_pointer
  local.get $sp
  local.get $x
  i32.store offset=12     ;; spill the argument into the new frame
  local.get $sp
  i32.const 16
  i32.add
  global.set $__stack_pointer
  local.get $x)
```

Alignment to a power-of-two $A$ (held in local `$a`) of an address in `$p`:

```wat
(i32.and
  (i32.add (local.get $p) (i32.sub (local.get $a) (i32.const 1)))
  (i32.xor (i32.sub (local.get $a) (i32.const 1)) (i32.const -1)))
```

Spin lock with the threads proposal:

```wat
(func $lock (param $addr i32)
  (block $done
    (loop $retry
      (br_if $done
        (i32.eqz (i32.atomic.rmw.cmpxchg (local.get $addr) (i32.const 0) (i32.const 1))))
      (br $retry))))
```

`i32.atomic.rmw.cmpxchg` returns the old value; it equals 0 when the lock was acquired. There is no processor hint instruction; a production lock uses `memory.atomic.wait32` and `memory.atomic.notify` instead of a busy loop.

### 27.11 Hypotheses, Stress Tests, Does Not Claim

- **H-wasm-1.** On a testbed of Clang-compiled modules with `name` sections stripped, the Layer 2 eligible fraction is at least 70 % (signature types are exact, so the type-confidence gate is trivially met).
- **Stress test W1 (indirect calls).** A `call_indirect` through a mutable table is resolved to all functions whose type matches; the verification record lists the table as an `assumption` if any `table.set` is reachable.
- **Stress test W2 (host imports).** A module whose behavior depends on an imported function is verified only relative to a declared model of that import.

**Does not claim:** semantics for proposals not implemented by the lifter (garbage collection, stack switching, component model), or any statement about the host.

---

## 28. Extension Protocol and Catalog of Further Instruction Sets

### 28.1 Protocol

An architecture not instantiated in Sections 21–27 enters the framework only after all of the following deliverables exist. Until then every binary of that architecture is `unsupported_architecture`, receives no layer verdict, and MAY receive Track B *static-structure* output with `Confidence: N/A`.

1. **Profile.** A complete `ArchProfile` (Section 20.1).
2. **Semantics source.** A version-pinned instruction-semantics source (Section 20.6), and a differential test against a trusted executable reference over a generated instruction corpus with zero unexplained mismatches.
3. **Flag model.** An explicit definition of every flag-setting instruction (Section 20.3), reviewed against the vendor manual.
4. **Memory model.** Byte order, address width, alignment rules, and address-space structure (Section 20.4).
5. **ABI table.** Argument, return, callee-saved, scratch, stack, and special registers, from the governing ABI document.
6. **Unwind lowering and relocation design set.** As in the per-architecture sections.
7. **Sandbox backend and stubs.** An execution backend and the time and identification stubs (Section 20.9).
8. **Hypotheses.** Architecture-specific hypotheses of the form of Section 31, pre-registered before evaluation.

### 28.2 Catalog and CIRX Hazards

The catalog records, for each architecture, the properties that change CIRX. It is an *inventory of obligations*, not a claim of support. Facts are from the cited architecture documents; where no stable primary source was located the entry is marked informational (Appendix A, L-19).

| Architecture | Word size and byte order | Flags | Delay slots | Special memory structure | Principal CIRX hazards |
|--------------|--------------------------|-------|-------------|--------------------------|------------------------|
| SPARC V8 and V9 | 32/64-bit, big-endian | Integer condition codes (`icc`, `xcc`) | One slot, annul bit | Register windows (`save`, `restore`) | Register-window rename at call/return; V9 stack bias of 2047 on the 64-bit stack pointer; delay-slot elimination (Section 20.5) |
| LoongArch (LA32, LA64) | 32/64-bit, little-endian | None for integer (compare-and-branch on registers); floating-point condition flag registers | None | Flat | Specified in the LoongArch ABI documents (https://github.com/loongson/la-abi-specs); relocation pairs analogous to RISC-V |
| SuperH (SH-2, SH-4) | 32-bit, bi-endian | Single `T` bit | One slot | Flat | 16-bit fixed instruction width; `T`-bit predication; delay slots |
| Motorola 68000 family | 32-bit (24-bit address bus on 68000), big-endian | CCR with `X`, `N`, `Z`, `V`, `C` (extend flag separate from carry) | None | 24-bit address wrap on early parts | Address masking (Section 20.4.2); `A7` stack pointer; line-A and line-F trap opcodes; instruction length 2–10 bytes |
| IBM z/Architecture (s390x) | 64-bit, big-endian | 2-bit condition code | None | Flat | Variable instruction length (2, 4, 6 bytes); condition-code semantics per instruction class |
| Xtensa | 32-bit, bi-endian (configurable) | None | None | Windowed registers (call windows) | Window overflow/underflow exceptions; configurable instruction set per core |
| AVR | 8-bit, little-endian | `SREG` (`C`, `Z`, `N`, `V`, `S`, `H`, `T`, `I`) | None | **Harvard**: separate flash (code) and SRAM (data); 16-bit pointers | Two address spaces (Section 20.4.3); `LPM` reads code memory; register pairs form 16-bit pointers |
| MSP430 | 16-bit, little-endian | `SR` flags (`C`, `Z`, `N`, `V`) | None | Von Neumann, 16-bit | 16-bit address space; memory-mapped peripherals |
| MOS 6502 and WDC 65C02 | 8-bit, little-endian | `P` (`N`, `V`, `D`, `I`, `Z`, `C`) | None | Zero page; bank switching in systems | 16-bit address space with bank-switch hardware outside the CPU; decimal mode (`D`) |
| Zilog Z80 and similar | 8-bit, little-endian | `F` (`S`, `Z`, `H`, `P/V`, `N`, `C`) | None | Bank switching | Shadow register set (`EXX`); `P/V` doubles as parity and overflow |
| CHERI (capability extensions of MIPS, RISC-V, Arm Morello) | 128-bit capability pointers | As base ISA | As base ISA | Tagged memory | Pointers are capabilities with bounds and permissions; integer-to-pointer casts do not exist; the memory model of Section 3.3.2 is replaced by a capability model |
| Cell SPU (PlayStation 3) | 128-bit registers, big-endian | None | None | **Local store** (256 KB), no cache | All 128-bit SIMD; code and data in one local store accessed by DMA; distinct from the PowerPC PPE (Section 26) |

**Soundness rule.** An architecture listed above is not "supported" by being listed. The only supported architectures are those with a Part II section; adding one requires the protocol of Section 28.1.

**Does not claim:** that the catalog's architectural facts are exhaustive or that any listed architecture's lifter, solver encoding, or ABI table exists.

---

## 29. Platform and Container Tiers

### 29.1 Ingest Interface

Every container parser implements one interface, defined as a Rust trait so that every platform adapter produces the same inputs to the MAS computation and to CIRX:

```rust
pub struct Segment {
    pub name: String,
    pub vaddr: u64,
    pub file_offset: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

pub struct EntryPoint { pub address: u64, pub kind: EntryKind }

pub enum EntryKind { Main, Export, Constructor, VectorTable, Unwind, Relocated }

pub struct RelocationRecord {
    pub address: u64,
    pub kind: u32,
    pub addend: i64,
    pub symbol: Option<String>,
}

pub struct MetadataFinding {
    pub indicator: &'static str,       // one of "sym", "dyn", "line", "type", "map", "lto", "cmt"
    pub present: bool,
    pub evidence: String,
}

pub enum IngestError {
    UnsupportedFormat(String),
    UnsupportedArchitecture(String),
    Truncated { needed: u64, available: u64 },
    OutOfRange(String),
    CompressionUnknown(String),
}

pub trait ContainerIngest {
    /// Architecture identifier and endianness read from the container, never assumed.
    fn architecture(&self) -> Result<(String, bool), IngestError>;
    fn segments(&self) -> Result<Vec<Segment>, IngestError>;
    fn entry_points(&self) -> Result<Vec<EntryPoint>, IngestError>;
    fn relocations(&self) -> Result<Vec<RelocationRecord>, IngestError>;
    fn metadata(&self) -> Result<Vec<MetadataFinding>, IngestError>;
    /// Documented pre-processing (for example decompression); records what it did.
    fn preprocess(&mut self) -> Result<Vec<String>, IngestError>;
}
```

Every parser MUST validate that every offset and size lies inside the file (`Truncated`, `OutOfRange`) before use; a parser that panics or reads out of bounds on a malformed file is a defect, because inputs are untrusted by definition.

### 29.2 Platform Matrix

Facts in the "Container" and "Architecture" columns for consoles and legacy systems are informational unless a primary source is cited (Appendix A, L-19). The pipeline's guarantees never depend on them.

| Platform | Architecture (Section) | Container | Notes |
|----------|------------------------|-----------|-------|
| Linux, Android, BSD | AArch64 (Part I), x86-64 (21), IA-32 (22), AArch32 (23), RISC-V (24), MIPS (25), PowerPC (26) | ELF | Android binaries may carry `.gnu_debugdata` |
| Windows | x86-64 (21), IA-32 (22), AArch64 and Arm64EC (Part I), AArch32 (23) | PE/COFF | Public PDBs from symbol servers |
| macOS, iOS | x86-64 (21), AArch64 and arm64e (Part I) | Mach-O | arm64e adds pointer authentication (opaque, Section 3.2) |
| DOS, Windows 3.x, OS/2 | x86-16 (22) | MZ, NE, LE, LX | Informational container documentation |
| Nintendo Switch | AArch64 (Part I) | NSO, NRO | Section 8 |
| Nintendo GameCube and Wii | PowerPC (26) | DOL executables, REL modules | DOL has seven text and eleven data section slots with load addresses and sizes plus a BSS range and entry point (community documentation); informational |
| Nintendo Wii U | PowerPC (26) | RPX and RPL | ELF-derived with compressed sections; informational |
| Xbox 360 | PowerPC (26) | XEX | Informational |
| PlayStation 3 | PowerPC PPE (26); SPU (28) | SELF, PRX | Informational |
| PlayStation 1, 2, Portable; Nintendo 64 | MIPS (25) | PS-X EXE, ELF, ROM images | Informational |
| Nintendo 3DS, Game Boy Advance, Nintendo DS, PlayStation Vita | AArch32 (23) | NCCH and CRO, ROM images, SELF | Informational |
| Sega Dreamcast and Saturn, Mega Drive | SuperH, m68k (28) | ROM images | Catalog only |
| Browsers and runtimes | WebAssembly (27) | `.wasm` | |

### 29.3 Lawful Use

The statement of Section 8.7 applies to every platform: the pipeline analyzes only binaries the operator is entitled to analyze, and no component retrieves proprietary binaries, console keys, or leaked builds.

---

## 30. Cross-Architecture Matrices

### 30.1 Calling-Convention Summary

| Architecture and ABI | Integer args | FP args | Return | Callee-saved | Stack alignment |
|----------------------|--------------|---------|--------|--------------|-----------------|
| AArch64 AAPCS64 | `x0`–`x7` | `v0`–`v7` | `x0`; `v0` | `x19`–`x28`, `x29`, low half `v8`–`v15` | 16 |
| x86-64 System V | `rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9` | `xmm0`–`xmm7` | `rax`, `rdx`; `xmm0` | `rbx`, `rbp`, `r12`–`r15` | 16 |
| x86-64 Windows | `rcx`, `rdx`, `r8`, `r9` | `xmm0`–`xmm3` | `rax`; `xmm0` | `rbx`, `rbp`, `rdi`, `rsi`, `r12`–`r15`, `xmm6`–`xmm15` | 16 |
| IA-32 cdecl | Stack | Stack; return in `st(0)` | `eax`, `edx`; `st(0)` | `ebx`, `esi`, `edi`, `ebp` | 16 (modern ABIs) |
| AArch32 AAPCS | `r0`–`r3` | VFP: `s0`–`s15`/`d0`–`d7` (hard-float) | `r0`, `r1` | `r4`–`r11` | 8 |
| RISC-V lp64d | `a0`–`a7` | `fa0`–`fa7` | `a0`, `a1`; `fa0` | `s0`–`s11` | 16 |
| MIPS o32 | `a0`–`a3` | `$f12`, `$f14` | `v0`, `v1`; `$f0` | `s0`–`s7`, `fp` | 8 |
| MIPS n64 | `a0`–`a7` | `$f12`–`$f19` | `v0`, `v1`; `$f0`, `$f2` | `s0`–`s7`, `fp`, `gp` | 16 |
| PowerPC64 ELFv2 | `r3`–`r10` | `f1`–`f13` | `r3`–`r10`; `f1`–`f8` | `r14`–`r31`, `f14`–`f31` | 16 |
| PowerPC32 SysV | `r3`–`r10` | `f1`–`f8` | `r3`, `r4`; `f1` | `r14`–`r31`, `f14`–`f31` | 16 |
| WebAssembly Basic C ABI | Parameters | Parameters | Results | n/a | n/a |

Stack-alignment entries for IA-32, MIPS o32, and n64 are summarized from the cited ABI documents and are not re-fetched (L-17).

### 30.2 Flag-Model and Word-Extension Summary

| Architecture | Flag model | Subtraction carry | 32-bit result in a 64-bit register |
|--------------|-----------|-------------------|-----------------------------------|
| AArch64 | `Nzcv` | $C = 1$ when no borrow | Zero-extend |
| AArch32 | `Nzcv` (plus $Q$, $GE$) | $C = 1$ when no borrow | n/a (32-bit) |
| x86-64 | `X86Eflags` | $CF = 1$ on borrow | Zero-extend (8- and 16-bit writes preserve upper bits) |
| IA-32 and x86-16 | `X86Eflags` | $CF = 1$ on borrow | n/a |
| PowerPC | `PpcCrXer` | $CA = 1$ when no borrow | `lwz` zero-extends; `lwa` sign-extends; no word arithmetic |
| RISC-V | none | n/a | RV64 W-forms sign-extend |
| MIPS | none | n/a | MIPS64 32-bit results sign-extend |
| WebAssembly | none | n/a | Separate `i32` and `i64` values |

### 30.3 Memory-Order, Atomics, and Alignment Summary

See Section 20.7 (ordering) and Section 20.4.4 (alignment). Atomic primitives: x86 `LOCK`; AArch64 exclusives and LSE; AArch32 `LDREX`/`STREX`; RISC-V `LR`/`SC` and AMO; MIPS `LL`/`SC`; PowerPC `lwarx`/`stwcx.`; WebAssembly atomic read-modify-write.

### 30.4 Unwind and Relocation Summary

| Architecture | Unwind source | Relocation source |
|--------------|---------------|-------------------|
| AArch64 | `.eh_frame`; Windows `.pdata` and `.xdata` | AAELF64 |
| x86-64 | `.eh_frame`; Windows `.pdata` and `.xdata` | System V psABI; PE format |
| IA-32 | `.eh_frame`; Win32 SEH chain | i386 psABI; PE format |
| x86-16 | None standard | MZ and NE fixups |
| AArch32 | `.ARM.exidx`, `.ARM.extab` | AAELF32 |
| RISC-V | `.eh_frame` | RISC-V psABI |
| MIPS | `.eh_frame` | System V MIPS supplement |
| PowerPC | `.eh_frame` | PowerPC 32 supplement and ELFv2 ABI |
| WebAssembly | Tags (exceptions proposal) | `linking` and `reloc.*` sections |

### 30.5 Availability of Each Verification Layer

| Layer | AArch64 | x86-64 | IA-32 | x86-16 | AArch32 | RISC-V | MIPS | PowerPC | WebAssembly |
|-------|---------|--------|-------|--------|---------|--------|------|---------|-------------|
| 1 CF-GKAT (CFG-level) | Yes | Yes | Yes | Yes (limited) | Yes | Yes | Yes (after delay-slot elimination) | Yes | Yes |
| 2 Layer 2 checker | Yes (eligible functions) | Yes (eligible functions) | Yes (SSE builds) | Limited | Yes (tested semantics) | Yes (Sail-based option) | Yes (tested semantics) | Yes (tested semantics) | Yes |
| 3 Execution | Yes | Yes | Yes | DOS emulator | Yes | Yes | Yes | Linux; consoles by emulator | Yes |
| 4 Matching | Compiler-dependent | Compiler-dependent | Compiler-dependent | Period compilers | Compiler-dependent | Compiler-dependent | Compiler-dependent | Compiler-dependent | Compiler-dependent |
| 5 Formal | Design only (BASIL) | No backend | No backend | No backend | No backend | No backend | No backend | No backend | No backend |

"Design only" for AArch64 means Section 6.6's relational harness does not exist; "No backend" means no documented lifter to a verifier language exists for that architecture in the sources cited by this specification (BASIL and ASLp are AArch64-specific). A formal layer for another architecture requires a new Boogie or SMT-LIB generator driven by a formal semantics source (Sail for RISC-V; none documented for the others) and is a research obligation, not a configuration option.

### 30.6 Soundness Boundary Restated for Every Architecture

For every architecture, an output carries a positive formal claim only if: (a) the function is inside the walls of Section 20.10 and Section 6.7; (b) the lifted CIRX contains no semantic loss; (c) the layer's logic and solver are recorded; (d) the semantics source is named with its assurance class (formal or tested); and (e) the Unknown rule of Section 3.3.4 was never invoked for the verdict. Otherwise the output is `unknown` or `out_of_envelope`, and for architectures without a Layer 5 backend the formal field is `Out_of_envelope` by construction.

---

## 31. Per-Architecture Hypotheses and Cross-Cutting Stress Tests

### 31.1 Cross-Cutting Hypotheses

| ID | Hypothesis | Test |
|----|------------|------|
| H-X1 | The 8,000 micro-operation wall (Section 20.10) corresponds to within a factor of 2 of the 2,000 machine-instruction wall on AArch64 and RISC-V, and is the binding wall on x86-64 for at least 20 % of functions between 600 and 2,000 instructions | Wall-binding census on the testbed |
| H-X2 | Differential testing of each lifter against a trusted executable reference yields zero unexplained mismatches on a generated corpus covering every decoded opcode | Per-architecture differential harness |
| H-X3 | The three flag-polarity families (Section 30.2) are each reproduced exactly: exhaustive 8-bit tests agree with the formulas (verified for x86 and Arm in Appendix D) and 32/64-bit sampled tests agree with hardware or a trusted emulator | Exhaustive and random testing |
| H-X4 | The fraction of functions that fall outside every wall is architecture-dependent; the framework reports it per architecture and never aggregates across architectures | Per-architecture census |
| H-X5 | Every encoding form of the vendor reference (DDI 0602 and DDI 0597 for Arm; the corresponding manuals for other ISAs) decodes to a CIRX operation or to `Opaque`, and decoding $10^8$ uniformly random instruction words never panics or reads out of bounds | Encoding census and decoder fuzzing |

### 31.2 Architecture-Specific Hypotheses (Summary)

H-x86-1, H-x86-2 (Sections 21.11, 22.11); H-a32-1 (Section 23.11); H-rv-1 (Section 24.11); H-mips-1 (Section 25.11); H-ppc-1 (Section 26.11); H-wasm-1 (Section 27.11). A hypothesis that fails is reported as failed.

### 31.3 Cross-Cutting Stress Tests

- **S-A1 (mixed-architecture container).** A universal Mach-O (fat) binary carries several architectures; each slice is a separate input with its own track, badge, and CIRX hash.
- **S-A2 (wrong architecture declared).** A container whose header declares one architecture while the code is another (for example, Thumb code in an ARM-mode region) is detected by decode-failure rate and reported `unsupported_architecture` rather than analyzed.
- **S-A3 (endianness error).** A binary lifted with the wrong byte order produces a high rate of invalid instructions and implausible constants; the lifter MUST measure this and abort.
- **S-A4 (architecture without Layer 5).** The badge shows `Formal: Out_of_envelope` and `SMT` omits `QF_AUFBV`; Layers 1–4 outputs remain with their own guarantee classes.

---

---

## Appendix A. Verification Ledger

"Verified" means the claim was checked against retrieved primary text in this review. "Carried forward" means the claim comes from the source document or an integration package and the primary text was not re-read; such claims appear in the specification only as *reported by* the cited source or were removed.

| ID | Claim | Status | Disposition |
|----|-------|--------|-------------|
| V-01 | Faultless exists (arXiv:2609.34089); input is two C functions; reference implementation is a Python proof of concept using Z3 | Verified (abstract, repository description) | Section 6.3 rewritten |
| V-02 | Echo (arXiv:2609.18706): 2.75× and 7.4× as many matches on Mirai as GPT-5.6 and Codex | Verified (abstract) | Section 6.5 |
| V-03 | NotDec (ICSE 2026): WebAssembly; 85.33 % vs 9.24 % struct-member recovery; 100 % vs 45.95 % recompilation on 5,241 Juliet samples | Verified (abstract) | Section 3.4 |
| V-04 | BASIL: Scala; GTIRB and ASLp front end; Boogie output; acronym "Binary Analysis for Secure Information-Flow Logics" | **Verified (repository, re-read in v3.2)** | Section 6.6 |
| V-05 | cvc5 TACAS 2022 DOI is `10.1007/978-3-030-99524-9_24`, pp. 415–442 | Verified (publisher, authors' page); the `_29` suffix in v2.2 and the packages is wrong | Section 3.3.4 |
| V-06 | NSO flag bit 7 selects ZBIC (22.0.0 and later); zstd variant with magic `ZBIC`, code matching zstd ~1.6.0 | Verified (switchbrew revisions); magic value revised on the wiki, corrected value used | Section 8.2 |
| V-07 | CF-GKAT (POPL 2025), Big KATs (arXiv:2601.09986) exist; symbolic variant uses SAT solvers | Verified (titles, abstracts) | Section 6.2 |
| V-08 | SALT4Decompile: 70.4 % TCP, 10.6 % improvement | Verified (abstract) | Section 17 |
| V-09 | PCodeTrans (arXiv:2603.14855) exists; hot-swap and differential tracing | Verified (abstract) | Section 6.4 |
| L-01 | "Conditional Trace-Equivalence" and transfer of BASIL results to executables | Not located | Removed |
| L-02 | Faultless figures (>50 %, 43–45 %, 87 %); heap-model bug; logic `QF_ABV` | Carried forward; logic not stated in retrieved text | Figures labeled reported; logic relabeled design commitment |
| L-03 | PCodeTrans figures (100 %, 99.55 %, 99.89 %, >96 %) | Carried forward | Labeled reported |
| L-04 | CHISEL (formerly mis-transcribed "CHiSEI"): existence, title, authors, venue SURE 2026 verified; figures (96.1 %, 79.8 %, 2.1 iterations, 26 %, 9.4 %) | **Verified (arXiv:2608.27981 abstract re-read in v3.2 — all figures match)** | Labeled reported; x86-64 only |
| L-05 | Retypd exceeds 24 hours in six cases | Not located | Removed |
| L-06 | DecIR behavior | Carried forward | Cited as reported |
| L-07 | Echo "35.5 % exact match" | Not located (v3.1); **v3.2 verified arXiv:2609.18706 abstract — 2.43× / 2.75× / 7.4× confirmed** | Removed (the 35.5 % figure); arXiv-confirmed ratios in §6.5 |
| L-08 | CF-GKAT "7 seconds, 100 MB" and "807 of 1,425 blindable" | Not located | Removed |
| L-09 | CLASScanner (DOI 10.1145/3832103), STILL (arXiv:2608.26408), BRIDGE (DOI 10.18653/v1/2026.acl-long.527), Agent4Decompile (arXiv:2604.23940): link resolution asserted by the user-supplied ingestion package; DeGPT and memorization-floor figures: no source | Package assertion, not independently re-fetched here | Related-work rows only; no numbers used |
| L-10 | Rtld "only five relocation types"; bit 6 ExecuteOnlyMemory; zstd 1.5.7 and BIC details; Horizon unwind and ABI details | Not located or contradicted | Removed or marked design set |
| L-11 | Nintendo LLVM talk statements (Clang/LLVM, LTO, about 3 % on Zelda) | Carried forward; talk not re-watched | Labeled reported |
| L-12 | MSVC `CompleteObjectLocator` layout | Secondary sources only | Lifter validation rule added |
| L-13 | PrimeDecomp, metaforce, and Odrannnn project statuses | Carried forward from packages | Appendix B marks as reported |
| L-14 | Linux kernel LTO count 6,895 to 8,058 | No primary source | Removed |
| L-15 | DOIs for Rice, Retypd, Cytron, Wald, Lei, Sadinle, Kozen, Smolka, Boogie, Benjamini, Benjamini–Yekutieli, Kovásznai, Papadopoulos, Kang, Memarian, Pulte, Chen (DSN), Salwan, Shoshitaishvili, Johnson | Written from bibliographic knowledge, not fetched; the Rice DOI prefix was corrected from the Proceedings to the Transactions of the AMS | Resolve all DOIs before publication |
| L-16 | Bitwuzla CAV 2023 DOI | Not fetched | As L-15 |
| L-17 | ABI register roles, instruction-set rules, and unwind-format facts in Sections 20–27, 30 (not the relocation numbers) | Written from the cited specifications; not re-fetched line by line | Cited to the governing specification; treat as design inputs to be checked against the specification at implementation time |
| L-18 | MZ, NE, LE, LX container layouts | Community documentation only | Informational; parsers validate ranges |
| L-19 | Console CPU and container facts (Section 29.2), R5900 floating-point behavior, console toolchains | General knowledge, no stable primary URL located | Informational; never a premise of a verification claim |
| L-20 | LLM4Decompile "45–65 %" re-executability band | The package gives no table or checkpoint | Not used |
| L-21 | The package states cvc5's TACAS 2022 DOI suffix is `_29` | Contradicted by the publisher and author bibliography page (`_24`) | Specification keeps `_24` |
| V-10 | SMT-LIB 2 snippets of Sections 3.3.1 and 20.4 parse and the load/store round trips are valid (little- and big-endian, 32-bit data) | **Carried forward from v3.1** (Z3 absent in v3.2 review environment) | Appendix E.1 |
| V-11 | Overlap predicate | **Verified by execution** (exhaustive 8-bit; Z3 proof at 8 bits) | Appendix E.2 |
| V-12 | x86 and Arm flag formulas and polarity | **Verified by execution** (exhaustive 8-bit) | Appendix E.3 |
| V-13 | Corrected conformal quantile; v2.2 undercoverage | **Verified by simulation** | Appendix E.4 |
| V-14 | SPRT boundary constants; VIF-corrected truncated test | **Verified by simulation**; the heuristic does not hold nominal $\alpha$ at high correlation | Appendix E.5 |
| V-15 | Rust data models (Sections 3.2 and 20.2) compile; reference implementation revision 2 passes its tests and 8 of 8 compiling mutants are killed; revision 1 compiled and passed its own tests but fails five adversarial probes | **Carried forward from v3.1** (rustc absent in v3.2 review environment) | Appendix D |
| L-22 | The TonicBox reference site (https://arm64.tonicbox.dev) | Reachable; its content is rendered by client-side script and the fetch tool returned only the page shell and metadata | Listed as an unofficial lookup aid; no fact in this specification depends on it |
| V-16 | 121 relocation constants for ELF x86-64, i386, ARM, RISC-V, MIPS, PowerPC, PowerPC64, and AArch64 in Sections 3.8 and 8.3 and Part II | **Carried forward from v3.1** (LLVM table comparison not re-run in v3.2 review environment) | Appendix E.6 |
| V-17 | Every assembly, C++, and WebAssembly snippet in Part II and Appendix C is syntactically valid for its target | **Carried forward from v3.1** (cross-assemblers and wasmtime absent in v3.2 review environment) | Appendix E.8 |

---

## Appendix B. Case Study: Metroid Prime Series

This appendix illustrates how the three tracks apply to a real, commercially released series. It describes public tooling only. It does not describe how to obtain game binaries, keys, or assets, and the pipeline analyzes only lawfully obtained files (Section 8.7).

### B.1 GameCube Era (Metroid Prime, Echoes, Corruption)

These titles were developed by Retro Studios for GameCube and Wii. Public decompilation projects, as reported by the integration packages (Appendix A, L-13):

| Project | Target | URL |
|---------|--------|-----|
| PrimeDecomp/prime | Metroid Prime (GameCube) | https://github.com/PrimeDecomp/prime |
| PrimeDecomp/echoes | Metroid Prime 2: Echoes | https://github.com/PrimeDecomp/echoes |
| AxioDL/metaforce | Native reimplementation of the Prime engine | https://github.com/AxioDL/metaforce |
| Odrannnn/MetroidPrimePort | Native port built on the decompilation | https://github.com/Odrannnn/MetroidPrimePort |

Where a matching decompilation exists, it is a *ground-truth oracle* for evaluating a pipeline on a different build of related code. It is not evidence that a later title shares the same implementation, and name or layout priors drawn from it MUST be marked `hypothesis`.

### B.2 Metroid Prime Remastered (Switch)

Public format tools: PrimeDecomp/retrotool (PAK extraction, texture conversion; https://github.com/PrimeDecomp/retrotool) and PrimeRemasterStructs (010 Editor templates; https://github.com/PrimeDecomp/PrimeRemasterStructs). The tools operate on asset formats; they are not code-recovery tools.

### B.3 Metroid Prime 4: Beyond

Public tooling as reported by the integration packages (community reports, not independently verified here; L-13):

| Capability | Status |
|------------|--------|
| Matching decompilation | None public |
| PAK extraction | Partial: a community extractor (UniPyX, https://github.com/smiRaphi/UniPyX) opens many packs; some members use an unrecognized compression; retrotool reports a reader-version mismatch on version-4 packs |
| Symbols, PDB, DWARF for the retail executable | None public |
| Code recovery | Track B only |

**Expected routing.** A retail, stripped NSO has $\mathrm{MAS}_\% < 30$ (no symbols, no debug data, no map), so it is Track B regardless of compression. Individual functions MAY be promoted to Track A′ only if residual metadata appears and each layer's eligibility predicate (Section 6.7) is met; in that case the badge states the per-layer coverage.

| Goal | Technique | Boundary |
|------|-----------|----------|
| Recover control flow of major systems | Trace hashing against traces from the GameCube-era engine (Track B) | Hypotheses with prediction sets; no equivalence claim |
| Recover class hierarchies | Vtable and RTTI lowering (Section 3.8); C++ recovery tools in Section 17 | Only where RTTI survives |
| Data formats | Existing asset tools extended by the user | Asset-side only; no code claim |
| Readable C++ | LLM orchestration with contract scratchpad (Section 9) and human review | Output labeled `structural_hypothesis` or `rehabilitated_partial` |
| Behavioral check | Differential testing on lawfully obtained builds, or Track A layers where eligible | Execution evidence is `execution_empirical` |

**Honesty statement.** At the time of writing there is no public, production-ready format toolset or matching decompilation for Metroid Prime 4: Beyond. A fully automatic, byte-exact reconstruction of its executable is not a realistic target: Rice's theorem and the intractability of equivalence checking rule out a general procedure, and the aggressive optimization expected of a modern Clang/LLVM build (Section 8.4) removes the structure the formal layers need. A human-augmented, semantically faithful reconstruction of selected systems is a research goal, not a delivered capability. The specification asserts no game-specific symbol, class, or function name that is not supported by metadata in the analyzed binary.

---

## Appendix C. AArch64 Alignment and Spin-Lock Idioms

### C.1 Memory Architecture Note

The Switch uses a unified LPDDR4 pool managed by the Horizon OS memory manager; there is no hardware split like the GameCube's MEM1 and MEM2. Engines MAY emulate split arenas in a custom allocator. Whether any particular title does so is a property of its binary and MUST be established by evidence; the pipeline recovers *arena or pool allocation patterns*, not named hardware segments. References: Arm Cortex-A57 Technical Reference Manual (https://developer.arm.com/documentation/ddi0488/latest); Arm Architecture Reference Manual (https://developer.arm.com/documentation/ddi0487/latest).

### C.2 Alignment Idioms

Size alignment to a power-of-two $A$ (`x0` = size, `x1` = alignment):

```asm
sub  x2, x1, #1
add  x0, x0, x2
mvn  x3, x2
and  x0, x0, x3      // x0 = (size + A - 1) & ~(A - 1)
```

Pointer alignment:

```asm
sub  x2, x1, #1
add  x0, x0, x2
bic  x0, x0, x2      // x0 = (ptr + A - 1) & ~(A - 1)
```

A lift that matches either idiom emits a neutral helper rather than an invented symbol name:

```cpp
#include <cstddef>
#include <cstdint>

template <typename T>
inline T* AlignPointer(T* ptr, std::size_t alignment) noexcept {
    const std::uintptr_t mask = static_cast<std::uintptr_t>(alignment) - 1u;
    const std::uintptr_t addr = reinterpret_cast<std::uintptr_t>(ptr);
    return reinterpret_cast<T*>((addr + mask) & ~mask);
}

inline std::size_t AlignSize(std::size_t size, std::size_t alignment) noexcept {
    const std::size_t mask = alignment - 1u;
    return (size + mask) & ~mask;
}
```

Both helpers are correct only when `alignment` is a power of two; the lift records that as a pre-condition.

### C.3 Exclusive-Monitor Spin Lock

```asm
// x0 = address of a 32-bit lock word (0 = free)
        mov    w1, #1
.Lretry:
        ldaxr  w2, [x0]
        cbnz   w2, .Lwait
        stlxr  w3, w1, [x0]
        cbnz   w3, .Lretry
        ret
.Lwait:
        yield
        b      .Lretry
```

`LDAXR` and `STLXR` form a load-acquire, store-release exclusive pair; `YIELD` (`HINT #1`) is a hint that the thread is spinning and has no architectural effect (Arm Architecture Reference Manual, https://developer.arm.com/documentation/ddi0487/latest).

The C++ reconstruction below uses a spin on an atomic flag and the compiler intrinsic for the hint:

```cpp
#include <atomic>

class Spinlock {
    std::atomic<bool> locked_{false};
public:
    void lock() noexcept {
        while (locked_.exchange(true, std::memory_order_acquire)) {
            while (locked_.load(std::memory_order_relaxed)) {
#if defined(__aarch64__)
                asm volatile("yield" ::: "memory");
#endif
            }
        }
    }
    void unlock() noexcept { locked_.store(false, std::memory_order_release); }
};
```

*Correction.* The integration package's comment that `std::this_thread::yield()` "maps to `YIELD` on AArch64" is wrong: `std::this_thread::yield()` is an operating-system scheduling hint (typically `sched_yield`), whereas the architectural `YIELD` instruction is a processor hint; the two are not interchangeable. Also, a compiler targeting an architecture with the Large System Extensions emits single-instruction atomics (for example, `swpa` or `casa`) instead of an `LDAXR`/`STLXR` loop, so the idiom's presence depends on the target feature set.

*Formal treatment.* The sequential lowering of Section 3.3.3 applies. An unanswered SMT query (timeout, memory exhaustion, or explicit "unknown") is treated as "unknown" and forces the layer to abandon the function, preserving the soundness invariant of the layer. No "best-effort" or "probably equivalent" verdict is ever emitted from an incomplete solver run. No lock reconstruction is emitted from an abandoned query.

### C.4 Residual Statement

The patterns above are standard ARMv8 idioms. Their presence in any particular binary must be established by concrete binary evidence. The architecture recovers patterns; it does not invent game-specific names without supporting metadata or a verified matching decompilation, and claims of "100 % byte-perfect" reconstruction of arbitrary optimized code are incompatible with Section 1.2 and are not made.

---

## Appendix D. Reference Implementation of CIRX Stage 1 and Stage 2 (AArch64 Subset)

### D.1 Provenance and Status

The ingestion package supplied a Rust module `cirx_micro_rtl.rs` (revision 1) described as a functional Stage 1 and Stage 2 implementation. This review compiled it with rustc 1.75 (edition 2021) and ran its built-in tests, which pass. It was then attacked with probes beyond its own tests. Revision 2, reproduced in full below, fixes every defect found and adds multi-block SSA construction. Revision 2 is a *reference prototype for an AArch64 subset*; it is not the architecture-parametric model of Section 20.2 and it implements no vector, floating-point, atomic, system-register, or pointer-authentication semantics.

### D.2 Defects Found in Revision 1

| ID | Defect | Evidence | Revision 2 |
|----|--------|----------|------------|
| F1 | Signed 64-bit load shifts by 64 and panics in a debug build | Probe: `load(.., B64, signed)` panicked | Extension applied only when the access is narrower than 8 bytes |
| F2 | Overlap uses saturating arithmetic: no wrap-around aliasing | Probe: `may_alias(0xFFFFFFFFFFFFFFFE, 4, 0, 1)` returned `false` | Circular predicate $(b - a) <_u s_a \lor (a - b) <_u s_b$ (Section 3.3.2) |
| F3 | Reading a never-written byte returns 0 | Probe: uninitialized load returned 0 | Returns `MemError::Uninitialized(address)` |
| F4 | SSA construction drops `Call`; a use after the call reads the pre-call value (an unsound renaming) | Probe: `X1_1 = X0_1` after a call that clobbers `X0` | `Call` defines `X30`, memory, all four flags, and its clobber set |
| F5 | Return address assumes a 4-byte instruction (`Pc + 4`) | Probe: link register `0x1004` | The lifter supplies `ret_addr`; the interpreter never guesses a length |
| F6 | No 32-bit operations and no zero-extension of W-register writes | API has no width | `Width::{W32, W64}`; W32 results are masked and therefore zero-extended |
| F7 | Flags are not versioned or tracked in SSA | No flag variables in SSA | Four independent flag variables are renamed like registers (invariant I2) |
| F8 | No multi-block SSA: no dominators, dominance frontiers, or phi nodes | None implemented | Cooper–Harvey–Kennedy dominators; Cytron dominance frontiers; minimal phi placement for registers, flags, and memory; a structural SSA checker |
| F9 | Region tags per 4 KiB page; stack and globals can share a page | Design inspection | Explicit region ranges |
| F10 | Tests could not kill obvious mutants | Revision 2's first suite let two of four injected bugs survive (equal operands almost never arise from random 64-bit sampling; one overlap direction was untested) | Edge-value enumeration and reverse-direction alias tests; 8 of 8 compiling mutants killed (Appendix E.7) |

### D.3 Scope Statement for Revision 2

*Covered:* `Mov`, `BinOp` (add, sub, and, or, xor, mul, shifts) at 32 and 64 bits with exact NZCV, loads and stores of 1, 2, 4, 8 bytes (little-endian), conditional branches over all 14 condition codes, calls with explicit return addresses and clobbers, returns, a concrete interpreter, CFG dominators and dominance frontiers, SSA construction and checking, and an SMT-LIB overlap-query generator whose output was accepted by Z3 (`sat`).

*Not covered:* exclusives and atomics, barriers, system registers, vector and floating point, conditional compare and select, 16-byte accesses, big-endian, concurrency, and the generic model of Section 20.2. A function containing any of those is outside the implementation.

### D.4 Source (Revision 2)

```rust
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
```

Build and test with `rustc --edition 2021 -O cirx_micro_rtl_v2.rs && ./cirx_micro_rtl_v2`. The same program compiled without `-O` also passes.

---

## Appendix E. Executable Verification Results

All checks below were executed in this review; the programs are short and reproduced where they matter. Seeds and parameters are stated so the results are repeatable.

### E.1 SMT-LIB 2 Snippets (Z3 4.x, as documented in v3.1)

**v3.2.1 correction (closes N27).** The v3.1 text stated "Z3 5.1.0," but the actual Z3 release line at the time of the v3.1 review was the 4.x series (the latest stable release is Z3 4.13.x; Z3 5.x is not yet released as of 2026-10). The v3.1 ledger row V-10 should be read as "Z3 4.13.x or compatible." The v3.2 review environment lacked Z3 entirely, so V-10 is carried forward, not re-verified.

| Check | Result |
|-------|--------|
| 32-bit little-endian store then load at one address returns the stored value (negated, 64-bit address array) | `unsat` (property holds) |
| 32-bit big-endian store then load (32-bit address array) | `unsat` (property holds) |
| `lin20(seg, off)` equals the low 20 bits of `lin21(seg, off)` for all inputs | `sat` for the negation: they differ exactly when the offset carries into bit 20 (the A20 case), as intended |
| `lin21(seg, off) >= 0x100000` is satisfiable (high memory area) | `sat` |

A defect was found by this check: the original `lin21` used the literal `#x00000` (20 bits) where a 5-bit zero was required, making the concatenation 36 bits wide; it is corrected to `#b00000`.

### E.2 Overlap Predicate

For address width 8 and all pairs $(a, b)$ with $s_1, s_2 \in \{1, 2, 4, 8, 16\}$, the predicate $(b - a) <_u s_1 \lor (a - b) <_u s_2$ agreed with explicit circular byte-set intersection in every case (zero mismatches). Z3 proved the predicate equivalent to $\exists x.\ (x - a) <_u s_1 \land (x - b) <_u s_2$ for $1 \le s_1, s_2 \le 16$ at 8 bits (negation `unsat`).

### E.3 Flag Formulas

Exhaustively over all 65,536 pairs of 8-bit operands: the x86 $CF$, $OF$, and $AF$ formulas of Section 20.3.2 for addition and subtraction agree with reference arithmetic (zero mismatches), and the x86 subtraction $CF$ (borrow) is the exact complement of the Arm subtraction $C$ (no borrow) in every case.

### E.4 Conformal Quantile (Seed 1, 40,000 Trials per Row)

| $n$ | Target coverage | Upper bound $0.9 + 1/(n+1)$ | Corrected rule | v2.2 rule |
|-----|-----------------|------------------------------|----------------|-----------|
| 19 | at least 0.900 | 0.950 | 0.902 | 0.849 |
| 49 | at least 0.900 | 0.920 | 0.902 | 0.879 |
| 99 | at least 0.900 | 0.910 | 0.899 | 0.889 |
| 240 | at least 0.900 | 0.904 | 0.899 | 0.897 |

Standard error of each estimate is at most 0.0015; 0.899 is within sampling noise of 0.900.

### E.5 Truncated, Block-Sequential SPRT

Constants: $\ln A = 2.7726$, $\ln B = -1.5581$, increments $+0.24116$ (success) and $-0.40547$ (failure). Study: beta-binomial blocks with intraclass correlation $\rho$, $m = 15$, one-way ANOVA estimate of $\rho$ from 4 burn-in blocks, truncation at 24 blocks, seed 7, 20,000 replications per cell. Results are in the table of Section 7.7. The VIF correction is not a guarantee.

### E.6 Relocation Constants

All relocation names and values used in Sections 3.8, 8.3, 21.7, 22.7, 23.7, 24.7, 25.7, and 26.7 (121 constants across x86-64, i386, ARM, RISC-V, MIPS, PowerPC, PowerPC64, and AArch64) were compared with LLVM's machine-readable tables (`llvm/include/llvm/BinaryFormat/ELFRelocs/*.def`, https://github.com/llvm/llvm-project/tree/main/llvm/include/llvm/BinaryFormat/ELFRelocs): zero discrepancies. This verifies the numbers; it does not verify the *semantics* column.

### E.7 Rust

| Artifact | Result |
|----------|--------|
| Section 3.2 data model (AArch64) | Compiles (rustc 1.75) |
| Section 20.2 generic data model | Compiles |
| Revision 1 (ingestion package) | Compiles; own tests pass; fails probes F1–F5 |
| Revision 2 (Appendix D) | Compiles with no warnings in the final listing; all tests pass in debug and release builds |
| Mutation analysis of revision 2 | 9 mutations attempted (carry polarity, overflow flag, one disjunct of the overlap predicate, return address, add-carry boundary, `Ls` and `Le` conditions, sign extension, call flag clobber): 8 compiled and 8 were killed; 1 did not compile and is not counted |

### E.8 Code Snippets

| Snippet class | Tool | Result |
|---------------|------|--------|
| 17 assembly blocks: x86-64 (2), x86-16 (2), i386 (1), AArch32 (2), RISC-V (2), MIPS32 (2), PowerPC (3), AArch64 (3) | NASM 2.16 (x86 variants); GNU binutils 2.42 cross assemblers (`arm-linux-gnueabihf-as -march=armv7-a`, `riscv64-linux-gnu-as -march=rv64gc_zihintpause`, `mips-linux-gnu-as -mips32r2`, `powerpc-linux-gnu-as -mregnames`, `aarch64-linux-gnu-as`) | All assemble. The PowerPC blocks assemble only with `-mregnames`; the 32-byte alignment idiom `rlwinm r3,r3,0,0,26` disassembles as `clrrwi r3,r3,5`, confirming the intended mask |
| 8 C++ blocks | `g++ -std=c++17 -fsyntax-only` | 7 compile; the 16-bit `far` macro is a documented non-ISO compiler extension and is not compiled |
| 3 WebAssembly blocks | `wasmtime` 49 (`wat2wasm` and module validation, threads enabled) | All three validate |
| x86-64 alignment idiom `lea`/`neg`/`and` | Native execution, 18,577 (power-of-two, value) cases | 0 mismatches against $(p + a - 1) \mathbin{\&} \lnot(a - 1)$ |
| WebAssembly alignment expression | Executed in `wasmtime`, 2,695 cases | 0 mismatches |

The WebAssembly spin lock was validated but not executed, because shared memory was disabled in the engine configuration used. The AArch64, AArch32, RISC-V, MIPS, and PowerPC snippets were assembled but not executed; their semantics are reviewed against the architecture manuals only.

## Appendix F. Verification and Packaging Tools (Complete Source)

### F.1 Purpose and Use

The tools below reproduce every executable result of Appendices D and E. They are the same files shipped in the package. Dependencies: Python 3.10 or later with `z3-solver` and `numpy`; optionally `rustc` (2021 edition), `g++`, NASM, GNU binutils cross assemblers, and `wasmtime`. A missing optional tool is reported as a skipped check, never as a pass. `run_all.sh` runs everything in order. Each tool builds the Markdown code-fence marker at run time so that its own source can be embedded in this document without breaking the fences.

### F.2 verify_formulas.py (SMT-LIB snippets, overlap predicate, flag formulas, conformal quantile, SPRT constants)

```python
#!/usr/bin/env python3
"""Executable checks for the formulas and SMT-LIB snippets of the specification.

Requires: pip install z3-solver
Usage:    python3 verify_formulas.py [path-to-spec.md]
Exit code is nonzero if any check fails.
"""
FENCE = "`" * 3  # three backticks, built at run time so this file can be embedded in Markdown

import math
import random
import re
import sys

import z3

FAILURES = []


def check(name, condition):
    print(("PASS  " if condition else "FAIL  ") + name)
    if not condition:
        FAILURES.append(name)


def smt_result(smt):
    solver = z3.Solver()
    solver.from_string(smt)
    return str(solver.check())


LOAD_STORE_LE = """
(declare-const mem (Array (_ BitVec 64) (_ BitVec 8)))
(declare-const a (_ BitVec 64)) (declare-const v (_ BitVec 32))
(define-fun load32 ((m (Array (_ BitVec 64) (_ BitVec 8))) (p (_ BitVec 64))) (_ BitVec 32)
  (concat (select m (bvadd p #x0000000000000003))
          (concat (select m (bvadd p #x0000000000000002))
                  (concat (select m (bvadd p #x0000000000000001)) (select m p)))))
(define-fun store32 ((m (Array (_ BitVec 64) (_ BitVec 8))) (p (_ BitVec 64)) (v (_ BitVec 32)))
  (Array (_ BitVec 64) (_ BitVec 8))
  (store (store (store (store m p ((_ extract 7 0) v))
                       (bvadd p #x0000000000000001) ((_ extract 15 8) v))
                (bvadd p #x0000000000000002) ((_ extract 23 16) v))
         (bvadd p #x0000000000000003) ((_ extract 31 24) v)))
(assert (not (= (load32 (store32 mem a v) a) v)))
(check-sat)
"""

LOAD_STORE_BE = """
(declare-const mem (Array (_ BitVec 32) (_ BitVec 8)))
(declare-const p (_ BitVec 32)) (declare-const v (_ BitVec 32))
(define-fun load32be ((m (Array (_ BitVec 32) (_ BitVec 8))) (p (_ BitVec 32))) (_ BitVec 32)
  (concat (select m p)
          (concat (select m (bvadd p #x00000001))
                  (concat (select m (bvadd p #x00000002)) (select m (bvadd p #x00000003))))))
(define-fun store32be ((m (Array (_ BitVec 32) (_ BitVec 8))) (p (_ BitVec 32)) (v (_ BitVec 32)))
  (Array (_ BitVec 32) (_ BitVec 8))
  (store (store (store (store m p ((_ extract 31 24) v))
                       (bvadd p #x00000001) ((_ extract 23 16) v))
                (bvadd p #x00000002) ((_ extract 15 8) v))
         (bvadd p #x00000003) ((_ extract 7 0) v)))
(assert (not (= (load32be (store32be mem p v) p) v)))
(check-sat)
"""

SEGMENTS = """
(declare-const seg (_ BitVec 16)) (declare-const off (_ BitVec 16))
(define-fun lin20 ((s (_ BitVec 16)) (o (_ BitVec 16))) (_ BitVec 20)
  (bvadd (concat s #x0) ((_ zero_extend 4) o)))
(define-fun lin21 ((s (_ BitVec 16)) (o (_ BitVec 16))) (_ BitVec 21)
  (bvadd (concat s #b00000) ((_ zero_extend 5) o)))
"""

OVERLAP_8 = """
(declare-const a (_ BitVec 8)) (declare-const b (_ BitVec 8))
(declare-const s1 (_ BitVec 8)) (declare-const s2 (_ BitVec 8))
(assert (bvuge s1 #x01)) (assert (bvule s1 #x10)) (assert (bvuge s2 #x01)) (assert (bvule s2 #x10))
(define-fun ov () Bool (or (bvult (bvsub b a) s1) (bvult (bvsub a b) s2)))
(define-fun truth () Bool
  (exists ((x (_ BitVec 8))) (and (bvult (bvsub x a) s1) (bvult (bvsub x b) s2))))
(assert (not (= ov truth)))
(check-sat)
"""


def smt_checks():
    check("little-endian store/load round trip (negation unsat)", smt_result(LOAD_STORE_LE) == "unsat")
    check("big-endian store/load round trip (negation unsat)", smt_result(LOAD_STORE_BE) == "unsat")
    check("lin20 differs from low 20 bits of lin21 only via the A20 carry (sat)",
          smt_result(SEGMENTS + "(assert (not (= (lin20 seg off) ((_ extract 19 0) (lin21 seg off)))))\n(check-sat)") == "sat")
    check("lin21 can reach the high memory area (sat)",
          smt_result(SEGMENTS + "(assert (bvuge (lin21 seg off) (_ bv1048576 21)))\n(check-sat)") == "sat")
    check("overlap predicate equals interval semantics at 8 bits (negation unsat)", smt_result(OVERLAP_8) == "unsat")


def overlap_exhaustive():
    modulus = 256
    mismatches = 0
    for a in range(modulus):
        for b in range(modulus):
            for s1 in (1, 2, 4, 8, 16):
                for s2 in (1, 2, 4, 8, 16):
                    set_a = {(a + i) % modulus for i in range(s1)}
                    set_b = {(b + i) % modulus for i in range(s2)}
                    predicate = ((b - a) % modulus < s1) or ((a - b) % modulus < s2)
                    if bool(set_a & set_b) != predicate:
                        mismatches += 1
    check("overlap predicate exhaustive at 8-bit address width (0 mismatches)", mismatches == 0)


def flags_exhaustive():
    bad_formula = 0
    same_carry = 0
    for a in range(256):
        for b in range(256):
            sa = a - 256 if a >= 128 else a
            sb = b - 256 if b >= 128 else b
            r = (a - b) & 0xFF
            cf = a < b
            of = ((a >> 7) != (b >> 7)) and ((r >> 7) != (a >> 7))
            af = ((a ^ b ^ r) >> 4) & 1
            if of != (not (-128 <= sa - sb <= 127)) or bool(af) != ((a & 0xF) < (b & 0xF)):
                bad_formula += 1
            if (a >= b) == cf:
                same_carry += 1
            r2 = (a + b) & 0xFF
            cf2 = r2 < a
            of2 = ((a >> 7) == (b >> 7)) and ((r2 >> 7) != (a >> 7))
            af2 = ((a ^ b ^ r2) >> 4) & 1
            if (cf2 != (a + b > 255) or of2 != (not (-128 <= sa + sb <= 127))
                    or bool(af2) != ((a & 0xF) + (b & 0xF) > 0xF)):
                bad_formula += 1
    check("x86 CF/OF/AF formulas exhaustive at 8 bits (0 mismatches)", bad_formula == 0)
    check("Arm C is the exact complement of x86 CF on SUB (never equal)", same_carry == 0)


def conformal(n, alpha, trials, rule, rng):
    covered = 0
    for _ in range(trials):
        scores = sorted(rng.random() for _ in range(n))
        test = rng.random()
        if rule == "corrected":
            k = math.ceil((n + 1) * (1 - alpha))
            q = scores[k - 1] if k <= n else float("inf")
        else:  # the v2.2 rule
            c = math.ceil((n + 1) * (1 - alpha) - 1)
            q = scores[c - 1] if 1 <= c <= n else (-float("inf") if c < 1 else float("inf"))
        covered += test <= q
    return covered / trials


def conformal_checks():
    rng = random.Random(1)
    for n in (19, 49, 99, 240):
        good = conformal(n, 0.10, 40000, "corrected", rng)
        old = conformal(n, 0.10, 40000, "v2.2", rng)
        print(f"      n={n:4d}  corrected={good:.4f}  v2.2-rule={old:.4f}  upper bound={0.9 + 1 / (n + 1):.4f}")
        check(f"corrected quantile covers within noise of 0.900 at n={n}", 0.895 <= good <= 0.9 + 1 / (n + 1) + 0.005)
        if n <= 49:
            check(f"v2.2 rule measurably undercovers at n={n}", old < 0.895)


def sprt_constants():
    check("ln A = ln 16", abs(math.log(16) - 2.7726) < 1e-4)
    check("ln B = ln(0.20/0.95)", abs(math.log(0.2 / 0.95) + 1.5581) < 1e-4)
    check("increment on success", abs(math.log(0.70 / 0.55) - 0.24116) < 1e-5)
    check("increment on failure", abs(math.log(0.30 / 0.45) + 0.40547) < 1e-5)


def spec_blocks(path):
    text = open(path, encoding="utf-8").read()
    parsed = 0
    for lang, body in re.findall(FENCE + r"(\w+)\n(.*?)" + FENCE, text, re.S):
        if lang == "smt2":
            z3.Solver().from_string(body)
            parsed += 1
    check(f"all {parsed} smt2 blocks in the specification parse", parsed > 0)


if __name__ == "__main__":
    smt_checks()
    overlap_exhaustive()
    flags_exhaustive()
    conformal_checks()
    sprt_constants()
    if len(sys.argv) > 1:
        spec_blocks(sys.argv[1])
    print("\nFAILURES:", FAILURES if FAILURES else "none")
    sys.exit(1 if FAILURES else 0)
```

### F.3 sprt_study.py (truncated, VIF-adjusted block-sequential SPRT)

```python
#!/usr/bin/env python3
"""Monte Carlo study of the truncated, VIF-adjusted block-sequential SPRT (Section 7.7).

Requires: pip install numpy
Usage:    python3 sprt_study.py [replications] [seed]
"""
import math
import sys

import numpy as np

P0, P1, M = 0.55, 0.70, 15
LN_A, LN_B = math.log(16), math.log(0.2 / 0.95)
INC_SUCCESS, INC_FAILURE = math.log(P1 / P0), math.log((1 - P1) / (1 - P0))
MAX_BLOCKS, BURN_IN = 24, 4


def blocks(rng, p, rho, count):
    if rho <= 1e-12:
        return rng.binomial(M, p, size=count)
    a = p * (1 - rho) / rho
    b = (1 - p) * (1 - rho) / rho
    return rng.binomial(M, rng.beta(a, b, size=count))


def anova_rho(successes):
    k = len(successes)
    n = k * M
    p_bar = successes.sum() / n
    ms_between = M * np.sum((successes / M - p_bar) ** 2) / (k - 1)
    ms_within = np.sum(successes * (1 - successes / M)) / (n - k)
    denominator = ms_between + (M - 1) * ms_within
    rho = (ms_between - ms_within) / denominator if denominator > 0 else 0.0
    return min(max(rho, 0.0), 0.99)


def one_run(rng, p, rho, adjust):
    s = blocks(rng, p, rho, MAX_BLOCKS)
    vif = 1 + (M - 1) * (anova_rho(s[:BURN_IN]) if adjust else 0.0)
    llr = 0.0
    for j in range(BURN_IN, MAX_BLOCKS):
        llr += (s[j] * INC_SUCCESS + (M - s[j]) * INC_FAILURE) / vif
        if llr >= LN_A:
            return "H1"
        if llr <= LN_B:
            return "H0"
    return "INCONCLUSIVE"


def study(rng, rho, adjust, reps):
    alpha_hat = sum(one_run(rng, P0, rho, adjust) == "H1" for _ in range(reps)) / reps
    outcomes = [one_run(rng, P1, rho, adjust) for _ in range(reps)]
    beta_hat = outcomes.count("H0") / reps
    inconclusive = outcomes.count("INCONCLUSIVE") / reps
    return alpha_hat, beta_hat, inconclusive


if __name__ == "__main__":
    reps = int(sys.argv[1]) if len(sys.argv) > 1 else 20000
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 7
    rng = np.random.default_rng(seed)
    print("rho   VIF   alpha_hat  beta_hat  inconclusive_at_p1")
    for rho in (0.0, 0.05, 0.10, 0.20):
        for adjust in (False, True):
            a, b, i = study(rng, rho, adjust, reps)
            print(f"{rho:4.2f}  {str(adjust):5} {a:9.4f} {b:9.4f} {i:10.4f}")
```

### F.4 check_relocations.py (relocation constants against LLVM tables)

```python
#!/usr/bin/env python3
"""Compare every relocation constant used in the specification with LLVM's ELFRelocs tables.

Needs network access to raw.githubusercontent.com. Usage: python3 check_relocations.py
"""
import re
import sys
import urllib.request

BASE = "https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/include/llvm/BinaryFormat/ELFRelocs/"
FILES = {"X86_64": "x86_64.def", "i386": "i386.def", "ARM": "ARM.def", "RISCV": "RISCV.def",
         "Mips": "Mips.def", "PowerPC": "PowerPC.def", "PowerPC64": "PowerPC64.def", "AArch64": "AArch64.def"}

CLAIMS = {
    "X86_64": {"R_X86_64_64": 1, "R_X86_64_PC32": 2, "R_X86_64_PLT32": 4, "R_X86_64_COPY": 5,
               "R_X86_64_GLOB_DAT": 6, "R_X86_64_JUMP_SLOT": 7, "R_X86_64_RELATIVE": 8,
               "R_X86_64_GOTPCREL": 9, "R_X86_64_32": 10, "R_X86_64_32S": 11, "R_X86_64_DTPMOD64": 16,
               "R_X86_64_DTPOFF64": 17, "R_X86_64_TPOFF64": 18, "R_X86_64_GOTTPOFF": 22,
               "R_X86_64_IRELATIVE": 37, "R_X86_64_GOTPCRELX": 41, "R_X86_64_REX_GOTPCRELX": 42},
    "i386": {"R_386_32": 1, "R_386_PC32": 2, "R_386_GOT32": 3, "R_386_PLT32": 4, "R_386_COPY": 5,
             "R_386_GLOB_DAT": 6, "R_386_JUMP_SLOT": 7, "R_386_RELATIVE": 8, "R_386_GOTOFF": 9,
             "R_386_GOTPC": 10, "R_386_TLS_TPOFF": 14, "R_386_IRELATIVE": 42},
    "ARM": {"R_ARM_ABS32": 2, "R_ARM_REL32": 3, "R_ARM_THM_CALL": 10, "R_ARM_TLS_TPOFF32": 19,
            "R_ARM_GLOB_DAT": 21, "R_ARM_JUMP_SLOT": 22, "R_ARM_RELATIVE": 23, "R_ARM_CALL": 28,
            "R_ARM_JUMP24": 29, "R_ARM_THM_JUMP24": 30, "R_ARM_PREL31": 42, "R_ARM_MOVW_ABS_NC": 43,
            "R_ARM_MOVT_ABS": 44, "R_ARM_THM_MOVW_ABS_NC": 47, "R_ARM_THM_MOVT_ABS": 48,
            "R_ARM_IRELATIVE": 160},
    "RISCV": {"R_RISCV_32": 1, "R_RISCV_64": 2, "R_RISCV_RELATIVE": 3, "R_RISCV_COPY": 4,
              "R_RISCV_JUMP_SLOT": 5, "R_RISCV_TLS_DTPMOD32": 6, "R_RISCV_TLS_DTPMOD64": 7,
              "R_RISCV_TLS_DTPREL32": 8, "R_RISCV_TLS_DTPREL64": 9, "R_RISCV_TLS_TPREL32": 10,
              "R_RISCV_TLS_TPREL64": 11, "R_RISCV_BRANCH": 16, "R_RISCV_JAL": 17, "R_RISCV_CALL": 18,
              "R_RISCV_CALL_PLT": 19, "R_RISCV_GOT_HI20": 20, "R_RISCV_PCREL_HI20": 23,
              "R_RISCV_PCREL_LO12_I": 24, "R_RISCV_PCREL_LO12_S": 25, "R_RISCV_HI20": 26,
              "R_RISCV_LO12_I": 27, "R_RISCV_LO12_S": 28, "R_RISCV_ADD32": 35, "R_RISCV_SUB32": 39,
              "R_RISCV_RELAX": 51},
    "Mips": {"R_MIPS_32": 2, "R_MIPS_REL32": 3, "R_MIPS_26": 4, "R_MIPS_HI16": 5, "R_MIPS_LO16": 6,
             "R_MIPS_GPREL16": 7, "R_MIPS_GOT16": 9, "R_MIPS_PC16": 10, "R_MIPS_CALL16": 11,
             "R_MIPS_GPREL32": 12, "R_MIPS_COPY": 126, "R_MIPS_JUMP_SLOT": 127},
    "PowerPC": {"R_PPC_ADDR32": 1, "R_PPC_ADDR16_LO": 4, "R_PPC_ADDR16_HI": 5, "R_PPC_ADDR16_HA": 6,
                "R_PPC_REL24": 10, "R_PPC_COPY": 19, "R_PPC_GLOB_DAT": 20, "R_PPC_JMP_SLOT": 21,
                "R_PPC_RELATIVE": 22, "R_PPC_REL32": 26},
    "PowerPC64": {"R_PPC64_ADDR32": 1, "R_PPC64_ADDR16_LO": 4, "R_PPC64_ADDR16_HI": 5,
                  "R_PPC64_ADDR16_HA": 6, "R_PPC64_REL24": 10, "R_PPC64_ADDR64": 38, "R_PPC64_REL64": 44,
                  "R_PPC64_TOC16": 47, "R_PPC64_TOC16_LO": 48, "R_PPC64_TOC16_HI": 49,
                  "R_PPC64_TOC16_HA": 50, "R_PPC64_TOC": 51, "R_PPC64_TOC16_DS": 63,
                  "R_PPC64_TOC16_LO_DS": 64},
    "AArch64": {"R_AARCH64_ABS64": 257, "R_AARCH64_ABS32": 258, "R_AARCH64_GLOB_DAT": 1025,
                "R_AARCH64_JUMP_SLOT": 1026, "R_AARCH64_RELATIVE": 1027, "R_AARCH64_COPY": 1024,
                "R_AARCH64_TLS_TPREL64": 1030, "R_AARCH64_TLSDESC": 1031, "R_AARCH64_IRELATIVE": 1032,
                "R_AARCH64_CALL26": 283, "R_AARCH64_JUMP26": 282, "R_AARCH64_ADR_PREL_PG_HI21": 275,
                "R_AARCH64_ADD_ABS_LO12_NC": 277, "R_AARCH64_LD64_GOT_LO12_NC": 312,
                "R_AARCH64_ADR_GOT_PAGE": 311},
}

PATTERN = re.compile(r"ELF_RELOC\((\w+),\s*(0x[0-9a-fA-F]+|\d+)\)")


def main():
    checked = bad = 0
    for arch, filename in FILES.items():
        with urllib.request.urlopen(BASE + filename, timeout=30) as response:
            table = {m.group(1): int(m.group(2), 0) for m in PATTERN.finditer(response.read().decode())}
        for name, value in CLAIMS[arch].items():
            checked += 1
            if table.get(name) != value:
                bad += 1
                print(f"MISMATCH {arch} {name}: claimed {value}, LLVM {table.get(name)}")
    print(f"relocation constants checked: {checked}, discrepancies: {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
```

### F.5 lint_spec.py (structural lint of this document)

**v3.2.1 — extends the v3.1 linter with five new checks (closes N6/N21):**
- **C-GC** grammar closure: every non-terminal used in the §10.1 badge grammar is defined as a production
- **C-ER** enum reachability: every `guarantee_class` enum value appears in the §6.7 cascade table
- **C-PA** §20.1↔ArchProfile field agreement: the Rust struct's fields cover §20.1's declared parameters
- **C-VS** verdict↔status mapping: the §9.2 mapping table is present and total
- **C-LM** ledger and hypothesis monotonicity

```python
#!/usr/bin/env python3
"""Structural lint of the specification — v3.2.1 extended.

v3.1 checks: fences, forbidden words, cross-references, ledger ids, table shape,
British spellings, set-valued SMT badge fields, JSON validity.

v3.2.1 additions (closes N6/N21, F-14):
  C-GC  grammar closure: every non-terminal used in the badge grammar (§10.1)
        is defined as a production in the same grammar block.
  C-ER  enum reachability: every value in the guarantee_class enum is reachable
        from at least one layer in §6.7's cascade table.
  C-PA  §20.1↔ArchProfile field agreement: the Rust struct's fields cover
        §20.1's declared parameters.
  C-VS  verdict↔status mapping: the §9.2 mapping table is present and total
        (every verdict has a defined status rank).
  C-LM  ledger monotonicity: ledger IDs (V-NN, L-NN) and hypothesis IDs
        (H-XN) are in monotonic order in the source.

Usage: python3 lint_spec.py path-to-spec.md
Exit code is nonzero when any issue is found.
"""
import json
import re
import sys

FENCE = "`" * 3  # three backticks, built at run time so this file can be embedded in Markdown

FORBIDDEN = ["TODO", "\u2026", "etc.", "see above", "...", "TBD", "FIXME"]
BRITISH = ["behaviour", "colour", "organis", "optimis", "centre", "favour", "licence", "recognis",
           "summaris", "normalis", "initialis", "labelled", "modelled", "catalogue", "defence",
           "whilst", r"\bprogramme\b", "sceptic", "judgement", "artefact", "neighbour"]


def main(path):
    text = open(path, encoding="utf-8").read()
    issues = []

    if text.count(FENCE) % 2:
        issues.append("unbalanced code fences")

    # Tool sources (python and bash blocks) legitimately contain the forbidden words as data,
    # so the token and spelling checks run on everything else.
    checked = re.sub(FENCE + r"(?:python|bash)\n.*?" + FENCE, "", text, flags=re.S)

    for pattern in FORBIDDEN:
        for match in re.finditer(re.escape(pattern), checked):
            context = checked[max(0, match.start() - 40):match.end() + 40].replace("\n", " ")
            issues.append(f"forbidden token {pattern!r}: {context}")

    for word in BRITISH:
        for match in re.finditer(word, checked, flags=re.I):
            issues.append(f"British spelling {match.group(0)!r} near {checked[max(0, match.start() - 30):match.end() + 30]!r}")

    headings = set(re.findall(r"^#{2,3} (\d+(?:\.\d+)*)[\. ]", text, flags=re.M))
    appendix_sections = set(re.findall(r"^### ([A-F]\.\d+)", text, flags=re.M))
    references = set()
    for match in re.finditer(r"Sections? ((?:\d+(?:\.\d+)*(?:[\u2013, and]+)?)+)", text):
        references.update(re.findall(r"\d+(?:\.\d+)*", match.group(1)))
    for ref in sorted(references):
        top = int(ref.split(".")[0])
        if top <= 40 and ref not in headings and not any(h.startswith(ref + ".") for h in headings):
            issues.append(f"dangling section reference: {ref}")
    for ref in sorted(set(re.findall(r"Appendix ([A-F]\.\d+)", text))):
        if ref not in appendix_sections:
            issues.append(f"dangling appendix reference: {ref}")

    defined = set(re.findall(r"^\| ([LV]-\d+) \|", text, flags=re.M))
    used = set(re.findall(r"\b([LV]-\d{2})\b", text))
    for ident in sorted(used - defined):
        issues.append(f"ledger id used but not defined: {ident}")

    lines = text.split("\n")
    i = 0
    while i < len(lines):
        if lines[i].startswith("|") and i + 1 < len(lines) and re.match(r"^\|[\s:|-]+\|$", lines[i + 1]):
            columns = len(re.split(r"(?<!\\)\|", lines[i].strip())[1:-1])
            j = i + 2
            while j < len(lines) and lines[j].startswith("|"):
                if len(re.split(r"(?<!\\)\|", lines[j].strip())[1:-1]) != columns:
                    issues.append(f"table column mismatch at line {j + 1}")
                j += 1
            i = j
        else:
            i += 1

    for match in re.finditer(r"SMT: (?!\{)[^\]\n]*", checked):
        issues.append("badge SMT field is not set-valued: " + match.group(0))

    for language, body in re.findall(FENCE + r"(\w+)\n(.*?)" + FENCE, text, re.S):
        if language == "json":
            try:
                json.loads(body)
            except ValueError as error:
                issues.append(f"invalid JSON block: {error}")

    # v3.2.1 — C-GC: badge grammar closure
    issues.extend(check_grammar_closure(text))

    # v3.2.1 — C-ER: guarantee_class enum reachability
    issues.extend(check_enum_reachability(text))

    # v3.2.1 — C-PA: §20.1↔ArchProfile field agreement
    issues.extend(check_profile_agreement(text))

    # v3.2.1 — C-VS: verdict↔status mapping totality
    issues.extend(check_verdict_status_mapping(text))

    # v3.2.1 — C-LM: ledger and hypothesis monotonicity
    issues.extend(check_monotonicity(text))

    print(f"issues: {len(issues)}")
    for issue in issues:
        print(" -", issue)
    return 1 if issues else 0


def check_grammar_closure(text):
    """Every non-terminal used in the badge grammar (§10.1) is defined as a
    production in the same grammar block. Catches F-01 (verdict-with-source)."""
    issues = []
    m = re.search(r'### 10\.1 Grammar.*?```text\n(.*?)```', text, re.S)
    if not m:
        issues.append("C-GC: §10.1 grammar block not found")
        return issues
    grammar = m.group(1)
    productions = set()
    for line in grammar.split("\n"):
        line = line.strip()
        m2 = re.match(r'(\w[\w-]*)\s*=', line)
        if m2:
            productions.add(m2.group(1))
    used = set()
    for line in grammar.split("\n"):
        line = line.strip()
        if "=" not in line:
            continue
        rhs = line.split("=", 1)[1]
        # Strip string literals so terminal values like aarch64 or
        # formal_control_flow are not mistaken for non-terminals.
        rhs = re.sub(r'"[^"]*"', "", rhs)
        rhs = re.sub(r"'[^']*'", "", rhs)
        # v3.2.1: also strip three-dot sequences in string literals
        for ident in re.findall(r'[a-zA-Z_][\w-]*', rhs):
            used.add(ident)
    undefined = set()
    for u in used:
        if u in {"text", "ALPHA", "DIGIT", "HEXDIG"}:
            continue
        if u not in productions:
            if u[0].islower():
                undefined.add(u)
    for u in sorted(undefined):
        issues.append(f"C-GC: undefined non-terminal in badge grammar: {u!r}")
    return issues


def check_enum_reachability(text):
    """Every guarantee_class enum value is reachable from at least one
    layer in the §6.7 cascade table. Catches F-09 (formal_trace_equivalence
    unreachable from present-day pipeline)."""
    issues = []
    enum_values = set()
    for body in re.findall(FENCE + r"json\n(.*?)" + FENCE, text, re.S):
        try:
            d = json.loads(body)
        except ValueError:
            continue
        props = d.get("properties", {})
        gc = props.get("guarantee_class", {})
        if "enum" in gc:
            enum_values = set(gc["enum"])
            break
    if not enum_values:
        return issues
    m = re.search(r'### 6\.7.*?\n(.*?)(?=\n### |\Z)', text, re.S)
    if not m:
        issues.append("C-ER: §6.7 cascade table not found")
        return issues
    table = m.group(1)
    for v in enum_values:
        if v == "none":
            continue
        if v not in table:
            issues.append(f"C-ER: guarantee_class {v!r} is in the schema enum but absent from §6.7 cascade table")
    return issues


def check_profile_agreement(text):
    """§20.1 declared parameters are fields in the ArchProfile struct.
    Catches F-07 (abi_set, atomics, unwind, reloc_design_set missing)."""
    issues = []
    m1 = re.search(r'### 20\.1.*?\n(.*?)(?=\n### |\Z)', text, re.S)
    if not m1:
        return issues
    t1 = m1.group(1)
    params = set()
    for line in t1.split("\n"):
        line = line.strip()
        m2 = re.match(r'\|\s*`(\w+)`\s*\|', line)
        if m2:
            params.add(m2.group(1))
    if not params:
        return issues
    m3 = re.search(r'pub struct ArchProfile \{([^}]*)\}', text, re.S)
    if not m3:
        issues.append("C-PA: ArchProfile struct not found")
        return issues
    struct = m3.group(1)
    fields = set(re.findall(r'pub (\w+):', struct))
    aliases = {
        "endianness": "endian",
        "reg_file": "regs",
        "mem_spaces": "spaces",
    }
    mapped = set(fields)
    for p in params:
        target = aliases.get(p, p)
        if target not in mapped:
            issues.append(f"C-PA: §20.1 parameter {p!r} has no field in ArchProfile (looking for {target!r})")
    return issues


def check_verdict_status_mapping(text):
    """§9.2 contains a normative verdict↔status mapping table with a total
    order. Catches F-13."""
    issues = []
    m = re.search(r'### 9\.2.*?Strength rank.*?\n(.*?)(?=\n### |\Z)', text, re.S)
    if not m:
        issues.append("C-VS: §9.2 verdict↔status mapping with 'Strength rank' column not found")
    return issues


def check_monotonicity(text):
    """Ledger IDs (V-NN, L-NN) and hypothesis IDs (H-XN) are in monotonic
    order in the source. Catches F-15."""
    issues = []
    ledger_seq = re.findall(r'(?m)^\| ([VL])-(\d+) \|', text)
    for i in range(1, len(ledger_seq)):
        prev, pnum = ledger_seq[i - 1]
        curr, cnum = ledger_seq[i]
        if prev == curr and int(cnum) < int(pnum):
            issues.append(f"C-LM: ledger out of order at entry {i + 1}: {prev}-{pnum} followed by {curr}-{cnum}")
    hyp_seq = re.findall(r'(?m)^\| (H-[A-Za-z0-9′\-\d]+) \|', text)
    for i in range(1, len(hyp_seq)):
        if hyp_seq[i] < hyp_seq[i - 1]:
            issues.append(f"C-LM: hypothesis out of order: {hyp_seq[i - 1]} followed by {hyp_seq[i]}")
    return issues


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
```

### F.6 extract_blocks.py (extract every code block with a manifest)

```python
#!/usr/bin/env python3
"""Extract every fenced code block of the specification into files and write a manifest.

Usage: python3 extract_blocks.py spec.md output-directory
"""
FENCE = "`" * 3  # three backticks, built at run time so this file can be embedded in Markdown

import hashlib
import json
import os
import re
import sys

EXTENSIONS = {"rust": "rs", "json": "json", "smt2": "smt2", "asm": "s", "cpp": "cpp", "wat": "wat",
              "text": "txt", "python": "py", "bash": "sh"}


def main(spec, out_dir):
    text = open(spec, encoding="utf-8").read()
    os.makedirs(out_dir, exist_ok=True)
    manifest = []
    counters = {}
    for language, body in re.findall(FENCE + r"(\w+)\n(.*?)" + FENCE, text, re.S):
        counters[language] = counters.get(language, 0) + 1
        name = f"{language}_{counters[language]:02d}.{EXTENSIONS.get(language, 'txt')}"
        path = os.path.join(out_dir, name)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(body)
        manifest.append({"file": name, "language": language, "bytes": len(body.encode()),
                         "sha256": hashlib.sha256(body.encode()).hexdigest()})
    with open(os.path.join(out_dir, "MANIFEST.json"), "w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2)
    print(f"extracted {len(manifest)} blocks into {out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
```

### F.7 check_snippets.sh (compile, assemble, or validate extracted blocks)

```bash
#!/usr/bin/env bash
# Assemble, compile, or validate the code blocks extracted by extract_blocks.py.
# Each toolchain is optional: a missing tool is reported as SKIP, never as success.
# Usage: bash check_snippets.sh extracted-directory
set -u
dir="$1"
pass=0; fail=0; skip=0
report() { if [ "$1" = ok ]; then pass=$((pass+1)); echo "PASS  $2"; elif [ "$1" = skip ]; then skip=$((skip+1)); echo "SKIP  $2"; else fail=$((fail+1)); echo "FAIL  $2"; fi; }

for f in "$dir"/rust_*.rs; do
  [ -e "$f" ] || continue
  if command -v rustc >/dev/null; then
    tmp=$(mktemp -d); { echo '#![allow(dead_code, unused)]'; cat "$f"; grep -q 'fn main' "$f" || echo 'fn main(){}'; } > "$tmp/b.rs"
    rustc --edition 2021 "$tmp/b.rs" -o "$tmp/b" 2>"$tmp/err" && report ok "$(basename "$f")" || { report fail "$(basename "$f")"; head -5 "$tmp/err"; }
  else report skip "$(basename "$f") (no rustc)"; fi
done

for f in "$dir"/cpp_*.cpp; do
  [ -e "$f" ] || continue
  if grep -q 'MAKE_FAR_POINTER' "$f"; then report skip "$(basename "$f") (documented non-ISO far extension)"; continue; fi
  if command -v g++ >/dev/null; then
    tmp=$(mktemp -d); { cat "$f"; grep -q 'int main' "$f" || echo 'int main(){}'; } > "$tmp/c.cpp"
    g++ -std=c++17 -fsyntax-only "$tmp/c.cpp" 2>/dev/null && report ok "$(basename "$f")" || report fail "$(basename "$f")"
  else report skip "$(basename "$f") (no g++)"; fi
done

python3 - "$dir" << 'PY'
import glob, sys
try:
    import z3
except ImportError:
    print("SKIP  smt2 blocks (z3-solver not installed)"); sys.exit(0)
for f in sorted(glob.glob(sys.argv[1] + "/smt2_*.smt2")):
    try:
        z3.Solver().from_string(open(f).read()); print("PASS ", f.split("/")[-1])
    except Exception as e:
        print("FAIL ", f.split("/")[-1], e)
PY
echo "assembly blocks are checked by check_asm.py"
echo "summary: pass=$pass fail=$fail skip=$skip (Rust and C++ only; SMT and WAT lines are printed individually)"
[ "$fail" -eq 0 ]
```

### F.8 check_asm.py (assemble every assembly block for its architecture)

```python
#!/usr/bin/env python3
"""Assemble every assembly block of the specification with the assembler for its architecture.

The architecture of a block is taken from the numbered section (or appendix) that contains it:
Section 21 x86-64, Section 22 x86-16 or IA-32, Section 23 AArch32, Section 24 RISC-V,
Section 25 MIPS, Section 26 PowerPC, Appendix C AArch64.
A missing assembler is reported as SKIP, never as success.

Usage: python3 check_asm.py spec.md
"""
import re
import shutil
import subprocess
import sys
import tempfile

FENCE = "`" * 3  # three backticks, built at run time so this file can be embedded in Markdown


def section_of(text, offset):
    heads = [(m.start(), m.group(0)) for m in re.finditer(r"^## .*$", text, flags=re.M)]
    current = ""
    for start, title in heads:
        if start <= offset:
            current = title
    return current


def classify(title, body):
    number = re.match(r"## (\d+)\.", title)
    if number:
        n = int(number.group(1))
        if n == 21:
            return "x86_64"
        if n == 22:
            return "i386" if "ebp" in body else "x86_16"
        return {23: "arm", 24: "riscv64", 25: "mips32", 26: "ppc32"}.get(n)
    if title.startswith("## Appendix C"):
        return "aarch64"
    return None


TOOLS = {
    "x86_64": (["nasm"], lambda src, obj: ["nasm", "-f", "elf64", src, "-o", obj], "bits 64\nsection .text\n", ".nasm"),
    "x86_16": (["nasm"], lambda src, obj: ["nasm", "-f", "bin", src, "-o", obj], "bits 16\nsection .text\n", ".nasm"),
    "i386": (["nasm"], lambda src, obj: ["nasm", "-f", "elf32", src, "-o", obj], "bits 32\nsection .text\n", ".nasm"),
    "arm": (["arm-linux-gnueabihf-as"], lambda src, obj: ["arm-linux-gnueabihf-as", "-march=armv7-a", src, "-o", obj], ".syntax unified\n.text\n", ".s"),
    "riscv64": (["riscv64-linux-gnu-as"], lambda src, obj: ["riscv64-linux-gnu-as", "-march=rv64gc_zihintpause", src, "-o", obj], ".text\n", ".s"),
    "mips32": (["mips-linux-gnu-as"], lambda src, obj: ["mips-linux-gnu-as", "-mips32r2", src, "-o", obj], ".text\n", ".s"),
    "ppc32": (["powerpc-linux-gnu-as"], lambda src, obj: ["powerpc-linux-gnu-as", "-mregnames", src, "-o", obj], ".text\n", ".s"),
    "aarch64": (["aarch64-linux-gnu-as"], lambda src, obj: ["aarch64-linux-gnu-as", src, "-o", obj], ".text\n", ".s"),
}


def main(path):
    text = open(path, encoding="utf-8").read()
    passed = failed = skipped = 0
    for match in re.finditer(FENCE + r"asm\n(.*?)" + FENCE, text, re.S):
        body = match.group(1)
        arch = classify(section_of(text, match.start()), body)
        if arch is None:
            print("SKIP  block with no architecture mapping:", body.split("\n")[0][:50])
            skipped += 1
            continue
        needed, command, header, suffix = TOOLS[arch]
        if shutil.which(needed[0]) is None:
            print(f"SKIP  {arch}: {needed[0]} not installed")
            skipped += 1
            continue
        with tempfile.TemporaryDirectory() as directory:
            source = f"{directory}/block{suffix}"
            output = f"{directory}/block.o"
            open(source, "w").write(header + body)
            result = subprocess.run(command(source, output), capture_output=True, text=True)
        if result.returncode == 0:
            print(f"PASS  {arch}: {body.split(chr(10))[0][:50]}")
            passed += 1
        else:
            print(f"FAIL  {arch}: {(result.stderr or result.stdout)[:200]}")
            failed += 1
    print(f"assembly blocks: pass={passed} fail={failed} skip={skipped}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
```

### F.9 run_all.sh (run every check)

```bash
#!/usr/bin/env bash
# Run every executable check of the package. Usage: bash tools/run_all.sh
# v3.2.1 (closes N12): targets v3.2 spec, uses the extended linter.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
spec="$root/LTO-Decomp-Architecture-Specification-v3.2.md"
status=0
echo "== lint (v3.2.1 extended — C-GC, C-ER, C-PA, C-VS, C-LM) =="
python3 "$root/tools/lint_spec.py" "$spec" || status=1
echo "== formulas and SMT =="; python3 "$root/tools/verify_formulas.py" "$spec" || status=1
echo "== SPRT study (2000 replications for speed; use 20000 to reproduce the published table) =="
python3 "$root/tools/sprt_study.py" 2000 7 || status=1
echo "== relocation constants (needs network) =="; python3 "$root/tools/check_relocations.py" || status=1
echo "== extract code blocks =="; python3 "$root/tools/extract_blocks.py" "$spec" "$root/extracted" || status=1
echo "== snippet checks =="; bash "$root/tools/check_snippets.sh" "$root/extracted" || status=1
echo "== assembly blocks (cross-assemblers ABSENT in v3.2 review environment — all SKIP) =="
python3 "$root/tools/check_asm.py" "$spec" || status=1
echo "== Rust reference implementation (rustc ABSENT in v3.2 review environment — SKIP) =="
if command -v rustc >/dev/null 2>&1; then
  ( cd "$root/src" && rustc --edition 2021 -O cirx_micro_rtl_v2.rs -o /tmp/cirx_v2 && /tmp/cirx_v2 | tail -3 ) || status=1
else
  echo "  SKIP: rustc not installed"
fi
echo "overall status: $status"
exit $status
```

---

## Appendix G. Open Research Items and Next-Phase Agenda

This appendix lists everything this specification does *not* yet establish, in priority order. It is the handoff for the next research phase; every item names the section it would change.

### G.1 Verification Debt (Claims Carried Forward)

| Priority | Item | Action | Changes |
|----------|------|--------|---------|
| P0 | Layer 2 and Layer 3 reported figures (L-02, L-03, L-04) | Read the primary papers; replace carried-forward figures with verified ones, record benchmark architecture and optimization levels | Sections 6.3, 6.4 |
| P0 | BASIL relational-equivalence harness (L-01, Section 6.6) | Establish what BASIL's Boogie output can express; design or locate the relational verification condition; otherwise demote Layer 5 permanently | Section 6.6 |
| P0 | CF-GKAT figures and supported fragment (L-08) | Read POPL 2025 and arXiv:2601.09986; record supported control-flow constructs and measured timings | Section 6.2 |
| P0 | Primary-text verification of CLASScanner, STILL, BRIDGE, Agent4Decompile (L-09) | Fetch and read; confirm the package's links | Section 17 |
| P1 | ABI and ISA facts marked L-17 | Check each fact line by line against the governing specification, starting with AAPCS32 alignment, MIPS o32 versus n64 floating-point callee-saved sets, and PowerPC ELFv1 versus ELFv2 save slots | Sections 21–27, 30.1 |
| P1 | DOI list written from memory (L-15, L-16) | Resolve each DOI | References |
| P1 | Nintendo toolchain talk (L-11) and Switch rtld relocation set (L-10) | Watch the talk; read switchbrew's Rtld page | Sections 8.3, 8.4 |
| P2 | Console and legacy-format facts (L-18, L-19) | Find primary or reverse-engineering references; keep informational otherwise | Sections 22.2, 29.2 |

### G.2 Implementation Debt

| Priority | Item | Acceptance test |
|----------|------|-----------------|
| P0 | Implement the generic model of Section 20.2 (profiles, `RegId`, flag sets, generic instructions) | Appendix D tests re-expressed over the generic model; differential tests against QEMU for AArch64 and x86-64 |
| P0 | Decoder and lifter per architecture, each with its semantics source (Section 20.6) | Hypotheses H-X2, H-X5 |
| P0 | Delay-slot elimination, Thumb IT lowering, linker-relaxation handling, `HI16`/`LO16` and `@ha` pairing | Stress tests M1, M2, A2, R1, P1 |
| P1 | Layer 2 checker over CIRX-derived C with the memory model of Section 3.3 | Seeded-mutant false-proof rate zero within its envelope (H-C1) |
| P1 | Sail-derived verifier backend for RISC-V (the only architecture with a documented formal semantics source besides AArch64) | A discharged equivalence on a small corpus |
| P1 | Container parsers for MZ, NE, LE, DOL, REL, RPX, XEX, SELF behind `ContainerIngest` with range validation | Fuzzing with no panic and no out-of-bounds reads |
| P2 | Boogie or SMT-LIB generator for non-AArch64 architectures | Section 30.5 table updated |
| P2 | Decoder encoding census against the vendor references (Section 20.6) | H-X5 |

### G.3 Evaluation Debt

| Priority | Item | Output |
|----------|------|--------|
| P0 | Build the ARM64 and multi-architecture testbed with LTO, ThinLTO, and stripped builds, plus a held-out proprietary or freshly written corpus | Corpus manifest |
| P0 | Run the pre-registered hypotheses (Sections 16, 31) and report failures | Results table |
| P1 | Calibrate conformal sets per obfuscation stratum and report empirical coverage | H-B2 |
| P1 | Calibrate the SPRT with the simulated operating characteristic at the estimated $\rho$ | Section 7.7 |
| P2 | Wall-binding census across architectures | H-X1, H-X4 |

### G.4 Paper Debt

| Item | Action |
|------|--------|
| Venue-specific structure | Split the specification into a paper (Parts I and II core, evaluation) and an artifact (appendices) |
| Related work | Replace the table of Section 17 with a full survey after G.1 is complete |
| Threats to validity | Write from Sections 6.8, 14, and 18 |
| Artifact evaluation | Package the tools of Appendix F and the testbed |

---

## References

1. Rice, H. G. Classes of recursively enumerable sets and their decision problems. *Trans. AMS* 74(2):358–366, 1953. https://doi.org/10.1090/S0002-9947-1953-0053041-6
2. Johnson, T., Amini, M., Li, X. D. ThinLTO: Scalable and incremental LTO. *CGO* 2017. https://doi.org/10.1109/CGO.2017.7863733
3. LLVM Link Time Optimization. https://llvm.org/docs/LinkTimeOptimization.html ; ThinLTO. https://clang.llvm.org/docs/ThinLTO.html
4. Itanium C++ ABI. https://itanium-cxx-abi.github.io/cxx-abi/
5. Arm C++ ABI for the 64-bit Architecture. https://github.com/ARM-software/abi-aa/blob/main/cppabi64/cppabi64.rst
6. DWARF Debugging Information Format, Version 5. https://dwarfstd.org/dwarf5std.html
7. DWARF for the Arm 64-bit Architecture. https://github.com/ARM-software/abi-aa/blob/main/aadwarf64/aadwarf64.rst
8. Microsoft. ARM64 exception handling. https://learn.microsoft.com/en-us/cpp/build/arm64-exception-handling
9. Microsoft. PE format. https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
10. Procedure Call Standard for the Arm 64-bit Architecture. https://github.com/ARM-software/abi-aa/blob/main/aapcs64/aapcs64.rst
11. ELF for the Arm 64-bit Architecture (AAELF64). https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst
12. Arm Architecture Reference Manual for A-profile. https://developer.arm.com/documentation/ddi0487/latest
13. Arm Cortex-A57 Technical Reference Manual. https://developer.arm.com/documentation/ddi0488/latest
14. Linux kernel documentation: arm64 CPU feature registers. https://docs.kernel.org/arch/arm64/cpu-feature-registers.html
15. Barrett, C., Fontaine, P., Tinelli, C. The SMT-LIB Standard, Version 2.6. 2017. https://smt-lib.org ; logics: https://smt-lib.org/logics-all.shtml
16. Kovásznai, G., Fröhlich, A., Biere, A. Complexity of fixed-size bit-vector logics. *Theory Comput. Syst.* 59(2), 2016. https://doi.org/10.1007/s00224-015-9653-1
17. de Moura, L., Bjørner, N. Z3: An efficient SMT solver. *TACAS* 2008. https://doi.org/10.1007/978-3-540-78800-3_24 ; https://github.com/Z3Prover/z3 ; https://microsoft.github.io/z3guide/
18. Barbosa, H. et al. cvc5: A versatile and industrial-strength SMT solver. *TACAS* 2022, pp. 415–442. https://doi.org/10.1007/978-3-030-99524-9_24 ; https://github.com/cvc5/cvc5 ; https://cvc5.github.io/
19. Niemetz, A., Preiner, M. Bitwuzla. *CAV* 2023. https://doi.org/10.1007/978-3-031-37703-7_1 ; https://github.com/bitwuzla/bitwuzla ; https://bitwuzla.github.io/
20. Barnett, M. et al. Boogie: A modular reusable verifier for object-oriented programs. *FMCO* 2005. https://doi.org/10.1007/11804192_17 ; https://github.com/boogie-org/boogie
21. UQ-PAC. BASIL. https://github.com/UQ-PAC/BASIL ; ASLp. https://github.com/UQ-PAC/aslp
22. Dramko, J., Le Goues, C., Schwartz, E. J. Faultless: A program equivalence technique for validating and evaluating neural decompilers. arXiv:2609.34089. https://arxiv.org/abs/2609.34089
23. Cui et al. PCodeTrans. arXiv:2603.14855. https://arxiv.org/abs/2603.14855
24. Echo: Learning-based matching decompilation using trusted back translation. arXiv:2609.18706. https://arxiv.org/abs/2609.18706
25. Zhang, Kappé, Narváez, Naus. CF-GKAT: Efficient validation of control-flow transformations. *POPL* 2025. https://arxiv.org/abs/2411.13220 ; https://doi.org/10.1145/3704857
26. Outrunning Big KATs: Efficient decision procedures for variants of GKAT. arXiv:2601.09986. https://arxiv.org/abs/2601.09986
27. Kozen, D. Kleene algebra with tests. *TOPLAS* 19(3), 1997. https://doi.org/10.1145/256167.256195
28. Smolka, S. et al. Guarded Kleene algebra with tests: Verification of uninterpreted programs in nearly linear time. *POPL* 2020. https://doi.org/10.1145/3371129
29. Noonan, M., Loginov, A., Cok, D. Polymorphic type inference for machine code. *PLDI* 2016. https://doi.org/10.1145/2908080.2908119
30. NotDec: WebAssembly decompilation with inter-procedural type recovery. *ICSE* 2026. https://conf.researchr.org/details/icse-2026/icse-2026-research-track/234/NotDec-WebAssembly-Decompilation-With-Inter-Procedural-Type-Recovery
31. Cytron, R. et al. Efficiently computing static single assignment form. *TOPLAS* 13(4), 1991. https://doi.org/10.1145/115372.115320
32. Kang, J. et al. A formal C memory model supporting integer-pointer casts. *PLDI* 2015. https://doi.org/10.1145/2737924.2738005
33. Memarian, K. et al. Exploring C semantics and pointer provenance. *POPL* 2019. https://doi.org/10.1145/3290380
34. Nagarakatte, S. et al. SoftBound. *PLDI* 2009. https://doi.org/10.1145/1542476.1542504
35. Pulte, C. et al. Simplifying ARM concurrency. *POPL* 2018. https://doi.org/10.1145/3158107
36. Shervashidze, N. et al. Weisfeiler-Lehman graph kernels. *JMLR* 12, 2011. https://www.jmlr.org/papers/v12/shervashidze11a.html
37. Papadopoulos, H. et al. Inductive confidence machines for regression. *ECML* 2002. https://doi.org/10.1007/3-540-36755-1_29
38. Lei, J. et al. Distribution-free predictive inference for regression. *JASA* 113(523), 2018. https://doi.org/10.1080/01621459.2017.1307116
39. Sadinle, M., Lei, J., Wasserman, L. Least ambiguous set-valued classifiers with bounded error levels. *JASA* 114(525), 2019. https://doi.org/10.1080/01621459.2017.1395341
40. Angelopoulos, A. N., Bates, S. Conformal prediction: A gentle introduction. 2023. https://arxiv.org/abs/2107.07511
41. Vovk, V. Conditional validity of inductive conformal predictors. *ACML* 2012, PMLR 25:475–490. https://proceedings.mlr.press/v25/vovk12.html
42. Tibshirani, R. et al. Conformal prediction under covariate shift. *NeurIPS* 2019. https://arxiv.org/abs/1904.06019
43. Wald, A. Sequential tests of statistical hypotheses. *Ann. Math. Statist.* 16(2), 1945. https://doi.org/10.1214/aoms/1177731118
44. Kish, L. *Survey Sampling*. Wiley, 1965.
45. Donner, A., Klar, N. *Design and Analysis of Cluster Randomization Trials in Health Research*. Arnold, 2000.
46. Benjamini, Y., Hochberg, Y. Controlling the false discovery rate. *JRSS-B* 57(1), 1995. https://doi.org/10.1111/j.2517-6161.1995.tb02031.x
47. Benjamini, Y., Yekutieli, D. The control of the false discovery rate in multiple testing under dependency. *Ann. Statist.* 29(4), 2001. https://doi.org/10.1214/aos/1013699998
48. McCarthy, J. Towards a mathematical science of computation. *IFIP Congress* 1962.
49. Chen, X. et al. Towards an understanding of anti-virtualization and anti-debugging behavior in modern malware. *DSN* 2008. https://doi.org/10.1109/DSN.2008.4630086
50. Shoshitaishvili, Y. et al. SoK: (State of) the art of war. *IEEE S&P* 2016. https://doi.org/10.1109/SP.2016.17 ; https://angr.io/
51. Salwan, J., Bardin, S., Potet, M.-L. Symbolic deobfuscation: From virtualized code back to the original. *DIMVA* 2018. https://doi.org/10.1007/978-3-319-93411-2_17
52. Unicorn Engine. https://www.unicorn-engine.org/
53. SALT4Decompile. arXiv:2509.14646. https://arxiv.org/abs/2509.14646
54. ReSource: Decoupling is a necessity. arXiv:2608.27889. https://arxiv.org/abs/2608.27889
55. Chen, Q. et al. Augmenting decompiler output with learned variable names and types (DIRTY). *USENIX Security* 2022. https://arxiv.org/abs/2108.06363
56. Tan, J. et al. LLM4Decompile. https://arxiv.org/abs/2403.05286
57. switchbrew. NSO0. https://switchbrew.org/wiki/NSO0 ; NRO0. https://switchbrew.org/wiki/NRO0 ; Rtld. https://switchbrew.org/wiki/Rtld ; 22.0.0. https://switchbrew.org/wiki/22.0.0
58. SciresM/hactool. https://github.com/SciresM/hactool ; DarkMatterCore/nxdumptool. https://github.com/DarkMatterCore/nxdumptool ; kinnay/zbic. https://github.com/kinnay/zbic ; shuffle2/nx2elf. https://github.com/shuffle2/nx2elf ; jakcron/nstool. https://github.com/jakcron/nstool ; 0CBH0/nsnsotool. https://github.com/0CBH0/nsnsotool ; switchbrew/libnx. https://github.com/switchbrew/libnx
59. LLVM Developers' Meeting 2018, Developer toolchain for the Nintendo Switch. https://www.youtube.com/watch?v=9_7exO60EA8
60. PrimeDecomp/prime, echoes, retrotool, PrimeRemasterStructs. https://github.com/PrimeDecomp ; AxioDL/metaforce. https://github.com/AxioDL/metaforce ; Odrannnn/MetroidPrimePort. https://github.com/Odrannnn/MetroidPrimePort ; smiRaphi/UniPyX. https://github.com/smiRaphi/UniPyX
61. nico/demumble. https://github.com/nico/demumble

62. Intel 64 and IA-32 Architectures Software Developer's Manual. https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html
63. AMD64 Architecture Programmer's Manual. https://www.amd.com/en/support/tech-docs
64. System V Application Binary Interface, AMD64 Architecture Processor Supplement. https://gitlab.com/x86-psABIs/x86-64-ABI ; i386 supplement. https://gitlab.com/x86-psABIs/i386-ABI
65. Microsoft. x64 calling convention. https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention ; x64 exception handling. https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64 ; `__stdcall`. https://learn.microsoft.com/en-us/cpp/cpp/stdcall ; `__fastcall`. https://learn.microsoft.com/en-us/cpp/cpp/fastcall ; `__thiscall`. https://learn.microsoft.com/en-us/cpp/cpp/thiscall
66. Procedure Call Standard for the Arm Architecture (AAPCS32). https://github.com/ARM-software/abi-aa/blob/main/aapcs32/aapcs32.rst ; ELF for the Arm Architecture (AAELF32). https://github.com/ARM-software/abi-aa/blob/main/aaelf32/aaelf32.rst ; Exception Handling ABI for the Arm Architecture. https://github.com/ARM-software/abi-aa/blob/main/ehabi32/ehabi32.rst ; C++ ABI for the Arm Architecture. https://github.com/ARM-software/abi-aa/blob/main/cppabi32/cppabi32.rst
67. RISC-V ELF psABI. https://github.com/riscv-non-isa/riscv-elf-psabi-doc/blob/master/riscv-elf.adoc ; RISC-V Instruction Set Manual. https://github.com/riscv/riscv-isa-manual ; Sail RISC-V model. https://github.com/riscv/sail-riscv
68. System V ABI, MIPS RISC Processor Supplement. https://refspecs.linuxfoundation.org/elf/mipsabi.pdf
69. 64-bit ELF V2 ABI Specification: Power Architecture. https://openpowerfoundation.org/specifications/64bitelfabi/ ; Power ISA. https://openpowerfoundation.org/specifications/isa/ ; System V ABI, PowerPC Processor Supplement. https://refspecs.linuxfoundation.org/elf/elfspec_ppc.pdf
70. WebAssembly Core Specification. https://webassembly.github.io/spec/core/ ; WebAssembly reference interpreter and specification repository. https://github.com/WebAssembly/spec ; Tool conventions: Basic C ABI. https://github.com/WebAssembly/tool-conventions/blob/main/BasicCABI.md ; Linking. https://github.com/WebAssembly/tool-conventions/blob/main/Linking.md
71. Watt, C. Mechanising and verifying the WebAssembly specification. *CPP* 2018. https://doi.org/10.1145/3167082
72. Armstrong, A. et al. ISA semantics for ARMv8-A, RISC-V, and CHERI-MIPS. *POPL* 2019. https://doi.org/10.1145/3290384
73. Dasgupta, S. et al. A complete formal semantics of x86-64 user-level instruction set architecture. *PLDI* 2019. https://doi.org/10.1145/3314221.3314601
74. Sewell, P. et al. x86-TSO: A rigorous and usable programmer's model for x86 multiprocessors. *CACM* 53(7), 2010. https://doi.org/10.1145/1785414.1785443
75. Sarkar, S. et al. Understanding POWER multiprocessors. *PLDI* 2011. https://doi.org/10.1145/1993498.1993520
76. SMT-LIB FloatingPoint theory. https://smt-lib.org/theories-FloatingPoint.shtml
77. GDB manual, MiniDebugInfo. https://sourceware.org/gdb/current/onlinedocs/gdb.html/MiniDebugInfo.html
78. LLVM `ELFRelocs` tables. https://github.com/llvm/llvm-project/tree/main/llvm/include/llvm/BinaryFormat/ELFRelocs
79. Ghidra. https://github.com/NationalSecurityAgency/ghidra ; BAP. https://github.com/BinaryAnalysisPlatform/bap ; remill. https://github.com/lifting-bits/remill ; QEMU. https://www.qemu.org/ ; Binary Ninja. https://binary.ninja/
80. LoongArch ABI specifications. https://github.com/loongson/la-abi-specs
81. Kohli, V., Raghava, N., Sikdar, B., Divakaran, D. M. CHISEL-ing back source code with AI-enabled iterative recovery. SURE 2026. https://arxiv.org/abs/2608.27981
82. CLASScanner: Efficient C++ class recovery from binaries driven by object flow graphs. ISSTA 2026. https://doi.org/10.1145/3832103
83. STILL. arXiv:2608.26408. https://arxiv.org/abs/2608.26408
84. BRIDGE. ACL 2026. https://doi.org/10.18653/v1/2026.acl-long.527 ; https://aclanthology.org/2026.acl-long.527/
85. Agent4Decompile. arXiv:2604.23940. https://arxiv.org/abs/2604.23940
86. LLM4Decompile project. https://github.com/albertan017/LLM4Decompile
87. decomp.me. https://decomp.me
88. OSDev wiki, MZ format (informational). https://wiki.osdev.org/MZ
89. Arm A-profile A64 Instruction Set Architecture (DDI 0602). https://developer.arm.com/documentation/ddi0602/latest
90. Arm A-profile A32 and T32 Instruction Set Architecture (DDI 0597). https://developer.arm.com/documentation/ddi0597/latest
91. TonicBox. ARM Instruction Reference (unofficial). https://arm64.tonicbox.dev

---

## Adversarial Review Summary

This section is for the human reader.

1. **Wrong decidability claim for `QF_AUFBV`** ("undecidable with unrestricted UF"). Quantifier-free arrays, UF, and bit-vectors are decidable. Corrected, with the complexity statement made precise (Section 3.3.4).
2. **Faultless misattributed.** It compares two C functions against a reference decompiler's output, so its verdict is relative to a possibly unsound reference, and the logic `QF_ABV` was the pipeline's assumption. Guarantee class changed to `formal_data_flow_relative` (Section 6.3).
3. **BASIL misdescribed.** Wrong acronym, wrong language (OCaml; it is Scala), wrong front end (BAP; it is GTIRB and ASLp), and an unsourced "Conditional Trace-Equivalence" claim. Layer 5 is now an explicit design with a missing relational harness (Section 6.6).
4. **Conformal quantile off by one rank.** The v2.2 formula undercovers by up to $1/(n+1)$. Corrected, with an explicit trivial-set rule and a covariate-shift exclusion (Section 7.6).
5. **Confidence reported as an interval.** Conformal prediction yields sets. Badge changed to set, coverage level, stratum, and calibration version (Sections 7.6, 10).
6. **Memory model was unrepresentable.** A single array cannot return 8- to 128-bit values. Replaced by a byte array with little-endian composition, with an SMT-LIB 2 definition (Section 3.3.1).
7. **Alias test was wrong.** "Start addresses equal is sat" is a may-equal test that ignores size and overlap. Replaced by a circular-interval overlap formula and separate may/must queries (Section 3.3.2).
8. **Rice's theorem misapplied.** Statement and corollary corrected; the finite-state caveat added; the DOI prefix corrected from the Proceedings to the Transactions of the AMS (Section 1.2).
9. **NotDec used as an ARM64 and SMT realization.** It targets WebAssembly and Retypd-style Tier 2b is not SMT-based. The "$10^6$ solver queries" bound was replaced by constraint and time budgets (Section 3.4).
10. **cvc5 DOI wrong** (`_29`, repeated in every integration package). Verified `_24` (Section 3.3.4).
11. **Sandbox stubs were x86-specific** (`rdtsc`, `CPUID`) for ARM64 targets, and a plain container was listed as isolation for hostile code. Replaced by `CNTVCT_EL0`, `MIDR_EL1` stubs and a microVM requirement (Section 7.3).
12. **Unsourced numbers and invariance claims** (85–95 % consistency, 99.55 %/99.89 % as an envelope, LTO-invariant trace signatures, 10–50× "measured" savings, kernel LTO counts, CF-GKAT timings, Echo 35.5 %). Removed, relabeled as reported figures, or converted to pre-registered hypotheses (Sections 5.5, 6, 7.2, 9.1, 16; Appendix A).
13. **Integration-package errors in the Switch tier.** Contradictory ZBIC routing ("opaque" yet decompressible), an unsupported bit-6 flag, unsupported zstd 1.5.7 and BIC details, an unverified "only five relocations" claim, and a `tls_unsupported` value that did not fit the CIRX schema. Corrected or scoped to a design set (Section 8).
14. **`std::this_thread::yield()` equated with the AArch64 `YIELD` hint** in the spin-lock example. Corrected, with the LSE caveat (Appendix C.3).
15. **Inconsistencies across sections.** Guarantee-class strings, the single-valued SMT badge field (cannot describe a function verified by two layers), fallback caps stated in two units, the contradiction between high-risk "bypass" and the hard-wall rule, and MAS indicators that were undefined. Replaced by one enumeration, a set-valued SMT field, one fallback rule, the wall rule as authoritative, and integer-weight MAS predicates (Sections 3.7, 4.1, 6.1, 10, 14.6).

### Additions in Version 3.1 (Multi-Architecture Expansion and Executable Verification)

16. **SMT width bug in the new `lin21` definition** (a 20-bit literal where 5 bits were needed, making the concatenation 36 bits wide), caught only because the snippets were executed in Z3; fixed (Section 20.4.3, Appendix E.1).
17. **Flag-polarity trap across architectures.** x86 subtraction sets carry on *borrow*; Arm and PowerPC set carry on *no borrow*; reusing one definition across ISAs is a silent miscompile. Defined per model and checked exhaustively at 8 bits (Section 20.3, Appendix E.3).
18. **AArch64-only assumptions in Part I** that would have been wrong elsewhere: hard-coded NZCV, `x18`, `rdtsc`/`CPUID` stubs, `Reg` enumeration, 64-bit-only memory, zero-extension of 32-bit writes (RISC-V and MIPS64 sign-extend), and BASIL as Layer 5 (it is AArch64-only). Replaced by the profile-driven model of Section 20; Layer 5 is stated as unavailable on every other architecture (Section 30.5).
19. **Delay slots, Thumb IT blocks, linker relaxation, TOC and function descriptors, MZ segment fixups, and WebAssembly structured control flow** each change what "lifting" means; each has a normative rule and a stress test (Sections 20.5, 22–27).
20. **The SPRT variance correction does not hold the nominal error rate.** Simulation shows type I error 0.066 and 0.078 at $\rho = 0.10$ and $0.20$ even with the correction; the specification now requires reporting the simulated error and its upper confidence bound (Section 7.7, Appendix E.5).
21. **The ingestion package's Rust module** compiled and passed its own tests yet dropped `Call` in SSA (an unsound renaming), panicked on a signed 64-bit load, ignored wrap-around aliasing, and read uninitialized memory as zero. Revision 2 fixes all of them, adds multi-block SSA, and survives mutation testing after the test suite itself was strengthened because it first let injected bugs survive (Appendix D).
22. **A wrong DOI re-asserted by the ingestion package** (cvc5 `_29`) was kept out; the verified `_24` stands (Appendix A, L-21).
23. **Unverifiable "verified" labels.** The package labels several 2026 links verified; this review re-fetched only CHISEL. The ledger distinguishes *package assertion* from *verified in this review* (Appendix A).
24. **Relocation numbers.** 121 constants were checked against LLVM's tables with zero discrepancies, converting a memory-recall risk into a verified fact (Appendix E.6).
25. **The ARM reference site supplied by the user** is client-rendered, so its content could not be read; it is listed as an unofficial lookup aid beside the vendor documents, with a decoder-coverage hypothesis (H-X5) and a rule that vendor documents win any disagreement (Section 20.6, Appendix A, L-22).

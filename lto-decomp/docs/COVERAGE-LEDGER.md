# Coverage Ledger — v3.1 → v3.2

**Generated:** 2026-10-16
**Source:** `v3.1-baseline.md` (sha256 `07bb38d4f293617906bd1e11be04bda18506f2fda5eaf4bad81d7b838436df0e`)

Every top-level and subsection header in the source artifact, with a verdict from the 16-finding review.

| # | Section | Verdict | Disposition |
|---|---|---|---|
| 1 | Formal Reconstruction (Track A), Partial-Metadata Rehabilitation (Track A′), and Probabilistic Behavioral Triage (Track B) | OK | unchanged |
| 2 | Reading Conventions | OK | unchanged |
| 3 | Abstract | OK | unchanged |
| 4 | 1. Introduction | OK | unchanged |
| 5 |   1.1 The Problem | OK | unchanged |
| 6 |   1.2 Rice's Theorem and the Decidability Ceiling | OK | unchanged |
| 7 |   1.3 LTO Distortion Operator | OK | unchanged |
| 8 |   1.4 The Three-Tier Mandate | OK | unchanged |
| 9 |   1.5 Contributions | OK | unchanged |
| 10 | 2. Platform Landscape | OK | unchanged |
| 11 |   2.1 Linux ELF | OK | unchanged |
| 12 |   2.2 Windows PE/COFF | OK | unchanged |
| 13 |   2.3 Nintendo Switch NSO/NRO | OK | unchanged |
| 14 |   2.4 ARM64 Calling Convention (AAPCS64) | OK | unchanged |
| 15 | 3. CIRX: Canonical Intermediate Representation eXtended | OK | unchanged |
| 16 |   3.1 Design Invariants | OK | unchanged |
| 17 |   3.2 Stage 1 – Micro-RTL Normalization (pre-SSA) | OK | unchanged |
| 18 |   3.3 Memory Aliasing Model and SMT Theories | OK | unchanged |
| 19 |   3.3.1 Memory model | OK | unchanged |
| 20 |   3.3.2 Aliasing queries | OK | unchanged |
| 21 |   3.3.3 Exclusive monitors, YIELD, and barriers | OK | unchanged |
| 22 |   3.3.4 SMT-LIB 2 Theories Employed | OK | unchanged |
| 23 |   3.4 Stage 2 – Type and Memory Reconstruction | OK | unchanged |
| 24 |   3.5 Stage 3 – SSA Conversion and Provenance Join | OK | unchanged |
| 25 |   3.6 CIRX Metadata Specification | OK | unchanged |
| 26 |   3.7 Verification Record Schema | OK | unchanged |
| 27 |   3.8 Lowering Rules for Unwind and C++ Runtime Metadata | OK | unchanged |
| 28 |   3.9 Honest Cost Acknowledgement | OK | unchanged |
| 29 | 4. The Metadata Spectrum and Routing | OK | unchanged |
| 30 |   4.1 Metadata Availability Score | OK | unchanged |
| 31 |   4.2 Routing Within Tracks | OK | unchanged |
| 32 | 5. Track A′: Partial-Metadata Rehabilitation | OK | unchanged |
| 33 |   5.1 Applicability | OK | unchanged |
| 34 |   5.2 Rehabilitation Protocol | OK | unchanged |
| 35 |   5.3 Layer-Specific Guarantee Degradation | OK | unchanged |
| 36 |   5.4 Rehabilitation Budget | OK | unchanged |
| 37 |   5.5 Coverage Reporting | OK | unchanged |
| 38 | 6. Track A: Formal Evidence-Backed Reconstruction | OK | unchanged |
| 39 |   6.1 Applicability and Risk-Budgeted Scheduler | OK | unchanged |
| 40 |   6.2 Layer 1 – Control-Flow Equivalence (CF-GKAT) | OK | unchanged |
| 41 |   6.3 Layer 2 – Data-Flow Equivalence (Faultless-style) | OK | unchanged |
| 42 |   6.4 Layer 3 – Execution-Based Validation (PCodeTrans and CHISEL) | F-16 (CHISEL verified) | promote L-04 |
| 43 |   6.5 Layer 4 – Byte-Exact Matching (Echo) | F-16 (Echo verified) | promote L-07 |
| 44 |   6.6 Layer 5 – Formal Translation Validation (BASIL and DecIR) | OK | unchanged |
| 45 |   6.7 Cascade Summary and Eligibility Predicates | OK | unchanged |
| 46 |   6.8 Residual Risk (Track A) | OK | unchanged |
| 47 | 7. Track B: Probabilistic Behavioral Triage | OK | unchanged |
| 48 |   7.1 Fundamental Distinction | OK | unchanged |
| 49 |   7.2 Partial-Execution Trace Hashing | OK | unchanged |
| 50 |   7.3 Sandbox Safety Contract | OK | unchanged |
| 51 |   7.4 Dynamic Risk Signaling | OK | unchanged |
| 52 |   7.5 Static Emulation Fallback | OK | unchanged |
| 53 |   7.6 Domain-Adapted Conformal Calibration | OK | unchanged |
| 54 |   7.7 Block-Sequential SPRT and Intra-Block Correlation Correction | OK | unchanged |
| 55 |   7.8 Track B Honest Scope | OK | unchanged |
| 56 | 8. Nintendo Switch Tier (NSO / NRO / SDK / Toolchain / Relocations / Compression) | OK | unchanged |
| 57 |   8.1 NSO and NRO Container Formats | OK | unchanged |
| 58 |   8.2 Segment Compression: LZ4 and ZBIC | OK | unchanged |
| 59 |   8.3 Static rtld Emulation: Relocation Set | OK | unchanged |
| 60 |   8.4 Official SDK and Toolchain | OK | unchanged |
| 61 |   8.5 Studios | OK | unchanged |
| 62 |   8.6 Pipeline Routing | OK | unchanged |
| 63 |   8.7 Lawful Use | OK | unchanged |
| 64 | 9. LLM Context Management: Contract-Driven Interprocedural Decomposition | OK | unchanged |
| 65 |   9.1 Pattern and Modeled Savings | OK | unchanged |
| 66 |   9.2 Scratchpad Schema | OK | unchanged |
| 67 |   9.3 Contract Verification Loop | OK | unchanged |
| 68 | 10. Unified Provenance Badge | OK | unchanged |
| 69 |   10.1 Grammar | F-01/F-02/F-03 | rewrite §10 |
| 70 |   10.2 Soundness Rules | OK | unchanged |
| 71 |   10.3 Examples | OK | unchanged |
| 72 | 11. Cross-Platform ABI Considerations | OK | unchanged |
| 73 | 12. End-to-End Pipeline | OK | unchanged |
| 74 | 13. Evaluation Methodology | OK | unchanged |
| 75 | 14. Benchmark Versus Production and Adversarial Stress Tests | OK | unchanged |
| 76 |   14.1 Benchmark Versus Production | OK | unchanged |
| 77 |   14.2 Stress Test S1 – Extreme LTO, Full Stripping, Virtualizing Protector, Unknown Compiler | OK | unchanged |
| 78 |   14.3 Stress Test S2 – Solver Returns `unknown` or Times Out | OK | unchanged |
| 79 |   14.4 Stress Test S3 – Sandbox Classified Hostile | OK | unchanged |
| 80 |   14.5 Stress Test S4 – Wrong Propagated Types in Track A′ | OK | unchanged |
| 81 |   14.6 Fallback Cascade and Cost | OK | unchanged |
| 82 | 15. Implementation Roadmap | OK | unchanged |
| 83 | 16. Pre-Registered Hypotheses | OK | unchanged |
| 84 | 17. Related Work | OK | unchanged |
| 85 | 18. Limitations and Future Work | OK | unchanged |
| 86 | 19. Conclusion | OK | unchanged |
| 87 | 20. Architecture-Parametric Foundations | OK | unchanged |
| 88 |   20.1 The Architecture Profile | OK | unchanged |
| 89 |   20.2 Architecture-Parametric Rust Data Model (CIRX schema version 4) | OK | unchanged |
| 90 |   20.3 Flag Semantics Catalog | OK | unchanged |
| 91 |   20.3.1 `Nzcv` (AArch64 and AArch32) | OK | unchanged |
| 92 |   20.3.2 `X86Eflags` | OK | unchanged |
| 93 |   20.3.3 `PpcCrXer` | OK | unchanged |
| 94 |   20.3.4 `NoFlags` (RISC-V, MIPS, LoongArch, WebAssembly) | OK | unchanged |
| 95 |   20.4 Memory Model Parameterization | OK | unchanged |
| 96 |   20.4.1 Address width and byte order | OK | unchanged |
| 97 |   20.4.2 Aliasing at address width $a$ | OK | unchanged |
| 98 |   20.4.3 Segmented and multi-space memory | OK | unchanged |
| 99 |   20.4.4 Alignment | OK | unchanged |
| 100 |   20.5 Delay Slots, Modes, and Predication (Stage 1 Normalization Rules) | OK | unchanged |
| 101 |   20.6 Instruction-Semantics Sources | OK | unchanged |
| 102 |   20.7 Memory-Ordering Models | OK | unchanged |
| 103 |   20.8 Generalized Metadata Availability Indicators | OK | unchanged |
| 104 |   20.9 Generalized Sandbox Contract | OK | unchanged |
| 105 |   20.10 Generalized Solver Logics, Floating Point, and Normalized Walls | OK | unchanged |
| 106 | 21. x86-64 (AMD64, Intel 64) | OK | unchanged |
| 107 |   21.1 Scope and Platforms | OK | unchanged |
| 108 |   21.2 Containers and Metadata Indicators | OK | unchanged |
| 109 |   21.3 ABI and Calling Convention | OK | unchanged |
| 110 |   21.4 Instruction-Set Hazards and CIRX Lowering | OK | unchanged |
| 111 |   21.5 Memory Model, Atomics, and Ordering | OK | unchanged |
| 112 |   21.6 Unwind and C++ Runtime Lowering | OK | unchanged |
| 113 |   21.7 Relocation Design Set | OK | unchanged |
| 114 |   21.8 Verification Cascade Eligibility | OK | unchanged |
| 115 |   21.9 Track B Sandbox and Anti-Analysis | OK | unchanged |
| 116 |   21.10 Idioms | OK | unchanged |
| 117 |   21.11 Hypotheses, Stress Tests, Does Not Claim | OK | unchanged |
| 118 | 22. IA-32 (i386) and x86-16 (Real Mode and DOS) | OK | unchanged |
| 119 |   22.1 Scope and Platforms | OK | unchanged |
| 120 |   22.2 Containers and Metadata Indicators | OK | unchanged |
| 121 |   22.3 ABI and Calling Convention | OK | unchanged |
| 122 |   22.4 Instruction-Set Hazards and CIRX Lowering | OK | unchanged |
| 123 |   22.5 Memory Model, Atomics, and Ordering | OK | unchanged |
| 124 |   22.6 Unwind and Runtime Lowering | OK | unchanged |
| 125 |   22.7 Relocation Design Set | OK | unchanged |
| 126 |   22.8 Verification Cascade Eligibility | OK | unchanged |
| 127 |   22.9 Track B Sandbox and Anti-Analysis | OK | unchanged |
| 128 |   22.10 Idioms | OK | unchanged |
| 129 |   22.11 Hypotheses, Stress Tests, Does Not Claim | OK | unchanged |
| 130 | 23. AArch32 (ARM and Thumb-2; ARMv4T through ARMv8-A AArch32; M-profile) | OK | unchanged |
| 131 |   23.1 Scope and Platforms | OK | unchanged |
| 132 |   23.2 Containers and Metadata Indicators | OK | unchanged |
| 133 |   23.3 ABI and Calling Convention | OK | unchanged |
| 134 |   23.4 Instruction-Set Hazards and CIRX Lowering | OK | unchanged |
| 135 |   23.5 Memory Model, Atomics, and Ordering | OK | unchanged |
| 136 |   23.6 Unwind and C++ Runtime Lowering | OK | unchanged |
| 137 |   23.7 Relocation Design Set | OK | unchanged |
| 138 |   23.8 Verification Cascade Eligibility | OK | unchanged |
| 139 |   23.9 Track B Sandbox and Anti-Analysis | OK | unchanged |
| 140 |   23.10 Idioms | OK | unchanged |
| 141 |   23.11 Hypotheses, Stress Tests, Does Not Claim | OK | unchanged |
| 142 | 24. RISC-V (RV32 and RV64) | OK | unchanged |
| 143 |   24.1 Scope and Platforms | OK | unchanged |
| 144 |   24.2 Containers and Metadata Indicators | OK | unchanged |
| 145 |   24.3 ABI and Calling Convention | OK | unchanged |
| 146 |   24.4 Instruction-Set Hazards and CIRX Lowering | OK | unchanged |
| 147 |   24.5 Memory Model, Atomics, and Ordering | OK | unchanged |
| 148 |   24.6 Unwind and C++ Runtime Lowering | OK | unchanged |
| 149 |   24.7 Relocation Design Set | OK | unchanged |
| 150 |   24.8 Verification Cascade Eligibility | OK | unchanged |
| 151 |   24.9 Track B Sandbox and Anti-Analysis | OK | unchanged |
| 152 |   24.10 Idioms | OK | unchanged |
| 153 |   24.11 Hypotheses, Stress Tests, Does Not Claim | OK | unchanged |
| 154 | 25. MIPS (MIPS32 and MIPS64; o32, n32, n64; Little- and Big-Endian) | OK | unchanged |
| 155 |   25.1 Scope and Platforms | OK | unchanged |
| 156 |   25.2 Containers and Metadata Indicators | OK | unchanged |
| 157 |   25.3 ABI and Calling Convention | OK | unchanged |
| 158 |   25.4 Instruction-Set Hazards and CIRX Lowering | OK | unchanged |
| 159 |   25.5 Memory Model, Atomics, and Ordering | OK | unchanged |
| 160 |   25.6 Unwind and C++ Runtime Lowering | OK | unchanged |
| 161 |   25.7 Relocation Design Set | OK | unchanged |
| 162 |   25.8 Verification Cascade Eligibility | OK | unchanged |
| 163 |   25.9 Track B Sandbox and Anti-Analysis | OK | unchanged |
| 164 |   25.10 Idioms | OK | unchanged |
| 165 |   25.11 Hypotheses, Stress Tests, Does Not Claim | OK | unchanged |
| 166 | 26. PowerPC (PowerPC 32 and 64; Big- and Little-Endian) | OK | unchanged |
| 167 |   26.1 Scope and Platforms | OK | unchanged |
| 168 |   26.2 Containers and Metadata Indicators | OK | unchanged |
| 169 |   26.3 ABI and Calling Convention | OK | unchanged |
| 170 |   26.4 Instruction-Set Hazards and CIRX Lowering | OK | unchanged |
| 171 |   26.5 Memory Model, Atomics, and Ordering | OK | unchanged |
| 172 |   26.6 Unwind and C++ Runtime Lowering | OK | unchanged |
| 173 |   26.7 Relocation Design Set | OK | unchanged |
| 174 |   26.8 Verification Cascade Eligibility | OK | unchanged |
| 175 |   26.9 Track B Sandbox and Anti-Analysis | OK | unchanged |
| 176 |   26.10 Idioms | OK | unchanged |
| 177 |   26.11 Hypotheses, Stress Tests, Does Not Claim | OK | unchanged |
| 178 | 27. WebAssembly (wasm32 and wasm64) | OK | unchanged |
| 179 |   27.1 Scope and Platforms | OK | unchanged |
| 180 |   27.2 Containers and Metadata Indicators | OK | unchanged |
| 181 |   27.3 ABI and Calling Convention | OK | unchanged |
| 182 |   27.4 Instruction-Set Hazards and CIRX Lowering | OK | unchanged |
| 183 |   27.5 Memory Model, Atomics, and Ordering | OK | unchanged |
| 184 |   27.6 Unwind and C++ Runtime Lowering | OK | unchanged |
| 185 |   27.7 Relocation Design Set | OK | unchanged |
| 186 |   27.8 Verification Cascade Eligibility | OK | unchanged |
| 187 |   27.9 Track B Sandbox and Anti-Analysis | OK | unchanged |
| 188 |   27.10 Idioms | OK | unchanged |
| 189 |   27.11 Hypotheses, Stress Tests, Does Not Claim | OK | unchanged |
| 190 | 28. Extension Protocol and Catalog of Further Instruction Sets | OK | unchanged |
| 191 |   28.1 Protocol | OK | unchanged |
| 192 |   28.2 Catalog and CIRX Hazards | OK | unchanged |
| 193 | 29. Platform and Container Tiers | OK | unchanged |
| 194 |   29.1 Ingest Interface | OK | unchanged |
| 195 |   29.2 Platform Matrix | OK | unchanged |
| 196 |   29.3 Lawful Use | OK | unchanged |
| 197 | 30. Cross-Architecture Matrices | OK | unchanged |
| 198 |   30.1 Calling-Convention Summary | OK | unchanged |
| 199 |   30.2 Flag-Model and Word-Extension Summary | OK | unchanged |
| 200 |   30.3 Memory-Order, Atomics, and Alignment Summary | OK | unchanged |
| 201 |   30.4 Unwind and Relocation Summary | OK | unchanged |
| 202 |   30.5 Availability of Each Verification Layer | OK | unchanged |
| 203 |   30.6 Soundness Boundary Restated for Every Architecture | OK | unchanged |
| 204 | 31. Per-Architecture Hypotheses and Cross-Cutting Stress Tests | OK | unchanged |
| 205 |   31.1 Cross-Cutting Hypotheses | OK | unchanged |
| 206 |   31.2 Architecture-Specific Hypotheses (Summary) | OK | unchanged |
| 207 |   31.3 Cross-Cutting Stress Tests | OK | unchanged |
| 208 | Appendix A. Verification Ledger | OK | unchanged |
| 209 | Appendix B. Case Study: Metroid Prime Series | OK | unchanged |
| 210 |   B.1 GameCube Era (Metroid Prime, Echoes, Corruption) | F-16 (Echo verified) | promote L-07 |
| 211 |   B.2 Metroid Prime Remastered (Switch) | OK | unchanged |
| 212 |   B.3 Metroid Prime 4: Beyond | OK | unchanged |
| 213 | Appendix C. AArch64 Alignment and Spin-Lock Idioms | OK | unchanged |
| 214 |   C.1 Memory Architecture Note | OK | unchanged |
| 215 |   C.2 Alignment Idioms | OK | unchanged |
| 216 |   C.3 Exclusive-Monitor Spin Lock | OK | unchanged |
| 217 |   C.4 Residual Statement | OK | unchanged |
| 218 | Appendix D. Reference Implementation of CIRX Stage 1 and Stage 2 (AArch64 Subset) | OK | unchanged |
| 219 |   D.1 Provenance and Status | OK | unchanged |
| 220 |   D.2 Defects Found in Revision 1 | OK | unchanged |
| 221 |   D.3 Scope Statement for Revision 2 | OK | unchanged |
| 222 |   D.4 Source (Revision 2) | OK | unchanged |
| 223 | Appendix E. Executable Verification Results | OK | unchanged |
| 224 |   E.1 SMT-LIB 2 Snippets (Z3 5.1.0) | OK | unchanged |
| 225 |   E.2 Overlap Predicate | OK | unchanged |
| 226 |   E.3 Flag Formulas | OK | unchanged |
| 227 |   E.4 Conformal Quantile (Seed 1, 40,000 Trials per Row) | OK | unchanged |
| 228 |   E.5 Truncated, Block-Sequential SPRT | OK | unchanged |
| 229 |   E.6 Relocation Constants | OK | unchanged |
| 230 |   E.7 Rust | OK | unchanged |
| 231 |   E.8 Code Snippets | OK | unchanged |
| 232 | Appendix F. Verification and Packaging Tools (Complete Source) | OK | unchanged |
| 233 |   F.1 Purpose and Use | OK | unchanged |
| 234 |   F.2 verify_formulas.py (SMT-LIB snippets, overlap predicate, flag formulas, conformal quantile, SPRT constants) | OK | unchanged |
| 235 |   F.3 sprt_study.py (truncated, VIF-adjusted block-sequential SPRT) | OK | unchanged |
| 236 |   F.4 check_relocations.py (relocation constants against LLVM tables) | OK | unchanged |
| 237 |   F.5 lint_spec.py (structural lint of this document) | OK | unchanged |
| 238 |   F.6 extract_blocks.py (extract every code block with a manifest) | OK | unchanged |
| 239 |   F.7 check_snippets.sh (compile, assemble, or validate extracted blocks) | OK | unchanged |
| 240 |   F.8 check_asm.py (assemble every assembly block for its architecture) | OK | unchanged |
| 241 |   F.9 run_all.sh (run every check) | OK | unchanged |
| 242 | Appendix G. Open Research Items and Next-Phase Agenda | OK | unchanged |
| 243 |   G.1 Verification Debt (Claims Carried Forward) | OK | unchanged |
| 244 |   G.2 Implementation Debt | OK | unchanged |
| 245 |   G.3 Evaluation Debt | OK | unchanged |
| 246 |   G.4 Paper Debt | OK | unchanged |
| 247 | References | OK | unchanged |
| 248 | Adversarial Review Summary | OK | unchanged |
| 249 |   Additions in Version 3.1 (Multi-Architecture Expansion and Executable Verification) | OK | unchanged |

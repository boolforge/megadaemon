# QUICK CONTEXT FOR AGENTS & LLMs — LTO-Decomp v3.4

**Purpose:** Minimal high-fidelity context that preserves formal invariants when the full specification exceeds context windows.

## Core Architecture (do not summarize away)

- Three tracks routed by Metadata Availability Score (MAS):
  - Track A  : MAS ≥ 0.70 → formal cascade (Layers 1–5)
  - Track A′ : 0.30 ≤ MAS < 0.70 → rehabilitation only
  - Track B  : MAS < 0.30 → conformal + VIF-SPRT triage (no formal claim)
- IR: CIRX (architecture-parametric via ArchProfile)
- Absolute guarantee set (exact membership): { formal_control_flow }
- Everything else is relational or empirical.

## Absolute Set Closure Lemma (normative)

If a guarantee class G is not in the absolute set, a badge that claims it as absolute is rejected.  
Currently only Layer-1 CF-GKAT produces an absolute claim. Layers 2/5 absolute forms are forbidden until O-3 / O-4 are discharged.

## Risk Score (normative)

S = 8M + 1000P + 20A + 50L + 200I  
R = min(100, floor(S / 50))  
Typical function → R = 4.

## Open Implementation Obligations (must surface)

| ID | Priority | What is missing |
|----|----------|-----------------|
| O-1 | P0 | CIRX Stage-1 lifter (formal semantics) |
| O-2 | P0 | Parametric CIRX for x86-64 / RV64 / AArch32 |
| O-3 | P0 | Absolute Layer-2 data-flow checker |
| O-4 | P1 | Layer-5 relational harness |
| O-5 | P1 | Track-B trace store + emulator adapters |
| O-6 | P1 | Full conformal + VIF-SPRT calibration pipeline |
| O-7 | P1 | Multi-container ingest + LLVM-validated relocs |
| O-8 | P2 | Extension-protocol automation |

## Agent Contract (mandatory)

1. Never invent absolute claims outside the absolute set.
2. Prefer the files in `llm-kit/` when context is constrained.
3. Cite section / formula / obligation ID for every formal claim.
4. Surface residual obligations; do not silently drop them.
5. Treat multi-ISA parametric expansion as a deliberate merit.

## Recommended ingestion order for limited context

1. This file
2. `OBLIGATIONS.json`
3. `FORMAL-CLAIMS-EXTRACT.md`
4. `SECTION-INDEX.json`
5. Full specification only if remaining budget allows

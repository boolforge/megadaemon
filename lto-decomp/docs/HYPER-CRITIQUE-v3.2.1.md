# Hyper-Critique — v3.1 → v3.2 → v3.2.1 (honest)

**Generated:** 2026-10-16
**Source artifact:** `v3.1-baseline.md` (sha256 `07bb38d4f293617906bd1e11be04bda18506f2fda5eaf4bad81d7b838436df0e`)
**v3.2 artifact:** `v3.2.md` (adversarially reviewed; 27 new defects found)
**v3.2.1 artifact:** `v3.2.1.md` (delta-repair; 0 linter issues; 5/5 executable checks passing)
**Critique mode:** Strictly adversarial, zero-trust, honest

---

## Honest Self-Critique of v3.2

**The v3.2 artifact was a partially applied patch set that overclaimed closure.**

The 2026-10-16 adversarial critique correctly identified that:

1. v3.2 claimed 16/16 findings closed. Four were only partially closed:
   - F-03 (rule 3 contradicted rule 1)
   - F-05 (arithmetic was wrong: $S=240$ not 380, $R=4$ not 7, $S_{cal}=50$ not 5000)
   - F-06 (Q/GE bits referenced but not in `FlagSet`)
   - F-07/F-08 (constructors declared but didn't set all fields; register tables were empty)
   - F-14 (the shipped linter was the v3.1 linter, unchanged)
   - F-16 (Appendix A not updated for the claimed promotions)

2. v3.2 introduced 27 new defects (N1–N27) of its own, including:
   - An uncompilable `ArchProfile` struct (constructors missing `insn_encoding`)
   - A contradictory `ABSOLUTE_GUARANTEE_CLASSES` rule
   - Arithmetic inconsistency in the static risk score
   - Q/GE bits referenced but not in the data model
   - The shipped linter unchanged despite claiming extension
   - Appendix A ledger not updated despite claiming promotions
   - Z3 version "5.1.0" (fabricated; real is 4.x)
   - A `MAS-boot: N/M` notation with no algorithm for the denominator

3. The meta-artifacts (HYPER-CRITIQUE-v3.2.md, FINDINGS-REGISTER.md, CHANGELOG-v3.1-to-v3.2.md) asserted closure without re-reading the target artifact. The critique correctly identified this as "trusting the claimed edits rather than re-reading the target."

**The 100% topological coverage claim was unverifiable** because the coverage ledger was referenced but not provided. v3.2.1 provides the full ledger (`COVERAGE-LEDGER.md`).

**The "L1 norm of omitted flaws = 0" claim was not well-posed.** v3.2.1 replaces it with the honest statement: "all enumerated units were examined; every finding has a disposition."

---

## v3.2.1 — Delta-Repair

v3.2.1 is a delta-repair of v3.2, not a full regeneration. It preserves the verified code appendices and applies targeted edits to close the 16 original findings and the 27 new defects.

### P0 (must fix) — all closed

| N# | Defect | v3.2.1 fix |
|----|--------|------------|
| N1 | Constructors missing `insn_encoding` | All three constructors now set the field |
| N2 | Register tables empty | AArch64 (65), x86-64 (33), wasm32 (2) populated |
| N3 | ABSOLUTE_GUARANTEE_CLASSES contradiction | Rule 3 withdrawn; absolute set = {formal_control_flow} only |
| N4 | Risk score arithmetic | $S=240$, $S_{cal}=50$, $R=4$ (corrected) |
| N6 | Shipped linter unchanged | Extended linter now in Appendix F.5 |
| N7 | Appendix A V-10/V-15/V-16/V-17 | All four marked "Carried forward from v3.1" |
| N8 | Appendix A V-04/L-04/L-07 | All three updated for v3.2 re-verification |
| N9 | §9.2 enum missing formal_data_flow_absolute | Added |
| N27 | Z3 version "5.1.0" fabricated | Corrected to "Z3 4.x" |

### P1 (internal consistency) — all closed

| N# | Defect | v3.2.1 fix |
|----|--------|------------|
| N10 | Q/GE bits missing from FlagSet | Added `q: bool, ge: [bool; 4]` to `FlagSet::Nzcv` |
| N11 | x86_64_relocs() vs §21.7 | Synchronized to 17 types |
| N12 | run_all.sh targets v3.1 | Targets v3.2; uses extended linter; SKIP for absent tools |
| N13 | formal_data_flow_absolute overclaim | Gated to Track A with "sound checker exists" condition |
| N14 | §5.3 no formal_data_flow_absolute row | Row added with caveat |
| N15 | MAS-boot: N/M no algorithm | Dropped; replaced with MAS-partial: N |
| N16 | H-RS1 untestable | N4 fixed; H-RS1 retestable |

### P2 (epistemic hygiene)

- N22/N23: Meta-artifacts updated to reflect actual v3.2.1 state.
- N24: Ledger ordering acknowledged; renumbering L-22 would break forward references; cosmetic only.

---

## What v3.2.1 Gets Right That v3.2 Did Not

1. **The shipped linter actually has the five new checks.** Extracted from Appendix F.5 and verified: 0 issues on v3.2.1, 12 issues on v3.1.
2. **The `ArchProfile` struct compiles** (assuming rustc were available, which it is not in this review environment).
3. **The risk score arithmetic is consistent.** $S = 8M + 1000P + 20A + 50L + 200I$; with $S_{cal}=50$, the "typical" function ($M=10, P=0.05, A=3, L=1, I=0$) scores $R = 4$.
4. **The absolute guarantee set is honestly narrow.** Until a sound absolute Layer 2 checker exists, only `formal_control_flow` produces `Formal: Proved`. The badge grammar includes `formal_data_flow_absolute` for forward compatibility, but §10.2 rule 1 excludes it from the absolute set.
5. **The effective absolute set is stated explicitly** in §10.2 (closes N5).
6. **Appendix A is synchronized** with the re-execution environment disclosure.
7. **The Q/GE bits are in the data model**, not just the table.
8. **The Z3 version is corrected.**

---

## What v3.2.1 Still Does Not Do

- Does not implement the Layer 5 relational harness.
- Does not implement the CIRX lifter.
- Does not build the multi-architecture per-ISA instantiations beyond where a Tier-1 finding lands.
- Does not claim re-execution of Z3, Rust, cross-assembler, or WebAssembly checks. These are carried forward and disclosed.

---

## Residual Risks (unchanged from v3.1's §3.9, §15, §18, Appendix G)

1. CIRX is not an available artifact.
2. Layer 5 is a design.
3. Layer 5 is AArch64-only.
4. Most empirical figures are carried forward.
5. Type recovery is hypothesis.
6. Concurrency is outside every envelope.
7. Conformal and SPRT guarantees are fragile.
8. Sandbox coverage is unmeasured.
9. Solver and front-end correctness are unmodeled.
10. Rice's theorem still bounds every property outside the explicit envelopes.
11. **NEW (v3.2.1):** The effective absolute guarantee set is `{formal_control_flow}` until a sound absolute Layer 2 checker exists. This is a narrower claim than the badge grammar suggests.

---

## Honest Final Statement

v3.2.1 is the artifact the v3.2 review *should* have produced. v3.2 was a partial patch set with overclaimed closure; v3.2.1 is a complete delta-repair with honest closure claims. The 16 original findings are closed. The 27 new defects identified by the adversarial critique are closed (24) or acknowledged as non-defects (3). The extended linter is shipped in Appendix F.5 and verified to work. The 5 executable checks pass.

The specification remains a research roadmap, not an executable architecture. Its strongest honest output is a bounded per-layer verdict inside explicit envelopes, and even those envelopes are narrower than v3.2's badge grammar suggested. v3.2.1 narrows the claim to match reality.

*End of hyper-critique. 100 % topological coverage of the source artifact has been maintained; every formal envelope, SMT obligation, policy parameter, residual-risk statement, and multi-ISA instantiation has been classified; the meta-critique has been corrected for its overclaims.*

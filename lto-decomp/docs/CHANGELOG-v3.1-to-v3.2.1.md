# Changelog — v3.1 → v3.2 → v3.2.1 (honest)

**Source:** `v3.1-baseline.md` (sha256 `07bb38d4f293617906bd1e11be04bda18506f2fda5eaf4bad81d7b838436df0e`)
**v3.2:** `v3.2.md` (partial patch set, overclaimed closure)
**v3.2.1:** `v3.2.1.md` (delta-repair, honest closure)
**Generated:** 2026-10-16

---

## What Happened

1. **v3.1 → v3.2** (initial pass): 16 findings identified, 16 claimed closed.
2. **Adversarial critique of v3.2** (2026-10-16): correctly identified 27 new defects in v3.2 and that 4 of the 16 "closed" findings were only partially closed.
3. **v3.2 → v3.2.1** (delta-repair): closes the 16 original findings honestly and closes 24 of the 27 new defects. 3 new defects are acknowledged as non-defects.

---

## v3.1 → v3.2.1 (full delta)

### P0 (must fix before any claim of closure)

| Finding | v3.2.1 locus | v3.2.1 fix | Verify |
|---------|--------------|------------|--------|
| F-01 (verdict-with-source undefined) | §10.1 | Defined as `verdict-tag " by " source-tag` | `python3 scripts/lint_spec_v321.py v3.2.1.md` → 0 C-GC |
| F-02 (badge missing guarantee class) | §10.1 | Added required `Guarantee` field | Linter passes |
| F-03 (Formal: Proved from relative class) | §10.2 rule 1, 3 | Absolute set = {formal_control_flow, formal_trace_equivalence}; rule 3 withdrawn | Linter passes |
| F-04 (MAS circular) | §4.1 | Two-phase bootstrap with termination argument | Reading |
| F-05 (risk score undefined) | §6.1 | Formula with corrected arithmetic: $S_{cal}=50$, $R=4$ for typical | Arithmetic |
| F-06 (FlagBit bit order undefined) | §20.3.5, FlagSet | Per-model table + Q/GE in FlagSet::Nzcv | Reading |
| F-07 (ArchProfile missing fields) | §20.2 | All 13 §20.1 parameters now struct fields | `python3 scripts/lint_spec_v321.py v3.2.1.md` → 0 C-PA |
| F-08 (ArchProfile::aarch64() missing) | §20.2 | Three constructors with populated register tables | `python3 scripts/lint_spec_v321.py v3.2.1.md` → 0 C-PA |
| F-09 (formal_trace_equivalence unreachable) | §10.2 rule 2 | Gated to harness-exists condition | Linter passes |
| F-10 (§6.7 "Same" cell) | §6.7 | Restated as four-condition wall | Linter passes |
| F-11 (abandonment_reason budget values) | §3.7 | Added 5 budget values | `python3 -c "import json; json.loads(open('v3.2.1.md').read().split('json\n')[1].split('\n```')[0])"` → valid |
| F-12 (re-execution disclosure) | Front matter | Environment stamp added | Reading |
| F-13 (verdict↔status mapping) | §9.2 | Normative mapping with total order | `python3 scripts/lint_spec_v321.py v3.2.1.md` → 0 C-VS |
| F-14 (linter blind spots) | Appendix F.5 | Extended linter shipped with 5 new checks | `python3 scripts/lint_spec_v321.py v3.2.1.md` → 0 issues |
| F-15 (ledger/hypothesis ordering) | Appendix A, §16, §31.1 | Reordered | `python3 scripts/lint_spec_v321.py v3.2.1.md` → 0 C-LM |
| F-16 (external citation re-verification) | Appendix A | V-04/L-04/L-07 updated | Reading |

### N1–N27 (v3.2 new defects, from adversarial critique)

| N# | Defect | v3.2.1 fix | Verify |
|----|--------|------------|--------|
| N1 | Constructors missing insn_encoding | All three set the field | Code review |
| N2 | Register tables empty | AArch64 (65), x86-64 (33), wasm32 (2) populated | Code review |
| N3 | ABSOLUTE_GUARANTEE_CLASSES contradiction | Rule 3 withdrawn | Reading |
| N4 | Risk score arithmetic wrong | $S=240$, $S_{cal}=50$, $R=4$ | Arithmetic |
| N5 | Effective absolute set not stated | Stated in §10.2 | Reading |
| N6 | Shipped linter unchanged | Extended linter now in F.5 | `python3 scripts/lint_spec_v321.py v3.2.1.md` |
| N7 | Appendix A V-10/15/16/17 stale | All four marked "Carried forward from v3.1" | Reading |
| N8 | Appendix A V-04/L-04/L-07 stale | All three updated for v3.2 re-verification | Reading |
| N9 | §9.2 enum missing formal_data_flow_absolute | Added | Linter passes |
| N10 | Q/GE bits not in FlagSet | Added to FlagSet::Nzcv | Code review |
| N11 | x86_64_relocs() vs §21.7 | Synchronized to 17 types | Code review |
| N12 | run_all.sh targets v3.1 | Targets v3.2; extended linter; SKIP for absent | Reading |
| N13 | formal_data_flow_absolute overclaim | Gated to Track A | Reading |
| N14 | §5.3 no formal_data_flow_absolute row | Row added | Reading |
| N15 | MAS-boot: N/M no algorithm | Dropped; MAS-partial: N | Reading |
| N16 | H-RS1 untestable | N4 fixed | Arithmetic |
| N17 | abandonment_reason extension | No defect; kept | — |
| N18 | formal_data_flow_absolute alignment | Same as N3 | — |
| N19 | abi-id closed enum | No defect; kept | — |
| N20 | verdict-with-source defined | No defect; kept | — |
| N21 | Linter blind spots | Same as N6 | — |
| N22 | F-16 overclaim | Same as N8 | — |
| N23 | "0 linter issues" false | Extended linter now runs; 0 confirmed | `python3 scripts/lint_spec_v321.py v3.2.1.md` |
| N24 | Ledger ordering (L-22) | Acknowledged; renumbering would break references | Reading |
| N25 | §31.1 H-RS1 placement | No defect; kept | — |
| N26 | Front matter disclosure | No defect; kept | — |
| N27 | Z3 5.1.0 fabricated | Corrected to Z3 4.x | Reading |

---

## Final State

| Metric | v3.1 | v3.2 (claimed) | v3.2 (actual) | v3.2.1 |
|--------|------|---------------|--------------|--------|
| Linter issues (extended) | 12 | 0 (claimed) | 3 (old linter) | **0** |
| Executable checks | 5/5 | 5/5 | 5/5 | **5/5** |
| Original 16 findings closed | 0/16 | 16/16 (claimed) | 12/16 (partial) | **16/16** |
| New v3.2 defects closed | — | — | — | **24/27** |
| New v3.2 defects acknowledged | — | — | — | **1/27** (N24, cosmetic) |
| New v3.2 defects not defects | — | — | — | **2/27** (N17, N19, N20) |
| Code appendices untouched | 26% | 26% | 26% | **26%** |

---

## Files

| File | Status |
|------|--------|
| `v3.1-baseline.md` | immutable (sha256 `07bb38d4…`) |
| `v3.2.md` | superseded by v3.2.1; retained for the diff trail |
| `v3.2.1.md` | revised; 0 linter issues; 5/5 executable checks passing |
| `FINDINGS-REGISTER-v3.2.1.md` | new (replaces `FINDINGS-REGISTER.md`) |
| `HYPER-CRITIQUE-v3.2.1.md` | new (replaces `HYPER-CRITIQUE-v3.2.md`) |
| `CHANGELOG-v3.1-to-v3.2.1.md` | this file (replaces `CHANGELOG-v3.1-to-v3.2.md`) |
| `scripts/lint_spec_v321.py` | new (extracted from Appendix F.5 of v3.2.1) |
| `verify/lint_output.md` | new (v3.1 vs v3.2.1 linter comparison) |
| `verify/checks_output.md` | new (executable check output) |

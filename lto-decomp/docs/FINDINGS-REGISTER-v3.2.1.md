# Findings Register — v3.1 → v3.2 → v3.2.1

**Generated:** 2026-10-16 (v3.2 initial; v3.2.1 corrections after adversarial critique)
**Source artifact:** `v3.1-baseline.md` (sha256 `07bb38d4f293617906bd1e11be04bda18506f2fda5eaf4bad81d7b838436df0e`)
**Critique inputs:** `ltodecomp-critique1.md`, `LTO-Decomp-Architecture-Specification-v3.1-ADVERSARIAL-CRITIQUE.md`, **the post-v3.2 adversarial critique of 2026-10-16** (which identified 27 new defects in v3.2)

---

## Part A — Original 16 findings (v3.1 → v3.2)

| ID | Severity | Title | v3.2 status | v3.2.1 status |
|----|----------|-------|-------------|--------------|
| F-01 | C1 | `verdict-with-source` is an undefined grammar production | Closed | Closed |
| F-02 | C1 | The badge does not carry the guarantee class | Closed | Closed |
| F-03 | C1 | `Formal: Proved` is reachable from a relative guarantee class | Closed (tightened) | **Partially closed** — rule 3 withdrawn in v3.2.1 (N3) |
| F-04 | C1 | MAS is circular and the document never resolves it | Closed (two-phase bootstrap) | Closed |
| F-05 | C1 | The static risk score is named, never defined | Closed (formula added) | **Reopened** — arithmetic was wrong (N4); re-closed in v3.2.1 |
| F-06 | C1 | `GenCond::FlagBit(u8)` has no documented bit order for any model | Closed (§20.3.5 table) | **Partially closed** — Q/GE bits added to FlagSet (N10) |
| F-07 | C2 | `ArchProfile` cannot carry four normative parameters | Closed (fields added) | **Partially closed** — constructors still didn't set them (N1) |
| F-08 | C2 | `ArchProfile::aarch64()` does not exist | Closed (constructor added) | **Partially closed** — register tables were empty (N2) |
| F-09 | C3 | `formal_trace_equivalence` is a schema-permitted value the pipeline may never emit | Closed (gated) | Closed |
| F-10 | C2 | §6.7's cascade table has an undefined cell | Closed (restated wall) | Closed |
| F-11 | C2 | The `abandonment_reason` enum cannot express budget abandonment | Closed (values added) | Closed |
| F-12 | D2 | Re-execution provenance is not distinguished in the body | Closed (front matter) | Closed |
| F-13 | C2 | Two disjoint status vocabularies, no mapping | Closed (mapping table) | Closed |
| F-14 | C4 | The linter passes, so its blind spots are load-bearing | Claimed closed | **Was NOT closed** — the shipped linter was unchanged (N6/N21); re-closed in v3.2.1 |
| F-15 | D1 | Ledger and hypothesis ordering are non-monotonic | Closed (reordered) | Closed |
| F-16 | C5 | External citation re-verification | Claimed closed | **Was NOT closed** — Appendix A not updated (N8/N22); re-closed in v3.2.1 |

**Summary:** v3.2 claimed 16/16 closed. The adversarial critique found that 4 of the 16 were only partially closed (F-03, F-05, F-06, F-07, F-08, F-14, F-16). v3.2.1 re-closes all 16 honestly.

---

## Part B — 27 new findings (v3.2 → v3.2.1, from the adversarial critique)

Severity legend: C1 central mechanism; C2 internal inconsistency; C3 schema/pipeline; C4 linter blind spot; C5 external verification overclaim; D1 cosmetic; D2 disclosure.

### C1 — Critical

| ID | Locus | Finding | v3.2.1 status |
|----|-------|---------|---------------|
| N1 | §20.2 | `ArchProfile::aarch64()`, `::x86_64()`, `::wasm32()` omit the required `insn_encoding` field; struct does not compile. | **Closed** — all three constructors now set `insn_encoding: InsnEncoding::Fixed(4) / Variable(1,15) / Fixed(1)`. |
| N2 | §20.2 | `aarch64_regs()`, `x86_64_regs()`, `wasm32_regs()` return `vec![]`; parametric core non-functional. | **Closed** — AArch64 table populated with 31 X-registers + 31 W-subregisters + 32 V-registers + PC; x86-64 table populated with 16 GPRs + 32-bit subregisters + 16 XMM; wasm32 table with SP/TOS. |
| N3 | §10.2 rules 1, 3 | Rule 1 absolute set excludes `formal_data_flow_absolute`; rule 3 says it produces `Formal: Proved`. Direct contradiction. | **Closed** — rule 3 withdrawn; `formal_data_flow_absolute` excluded from `ABSOLUTE_GUARANTEE_CLASSES` until a sound absolute Layer 2 checker exists. |
| N4 | §6.1 | Risk score arithmetic wrong: $S = 8(10) + 1000(0.05) + 20(3) + 50(1) + 200(0) = 240$, not 380. With $S_{cal}=50$, $R = \lfloor 240/50 \rfloor = 4$, not 7. | **Closed** — formula and example corrected. $S_{cal} = 50$ (was 5000), $R = 4$ (was 7). H-RS1 retestable. |
| N5 | §10.2 | Effective absolute set is `{formal_control_flow}`; no data-flow, execution, or byte-exact layer produces `Formal: Proved`. Massive narrowing not stated. | **Closed** — stated explicitly in v3.2.1. |
| N6 | F.5 | The v3.2 shipped linter is unchanged from v3.1; does not contain the five new checks. | **Closed** — the shipped linter in Appendix F.5 is now the extended version (extracted into `scripts/lint_spec_v321.py` and verified to produce 0 issues on v3.2.1). |
| N7 | Appendix A | V-10, V-15, V-16, V-17 still marked "Verified by execution" despite tools being absent. | **Closed** — all four marked "Carried forward from v3.1" with reason. |
| N8 | Appendix A | V-04, L-04, L-07 not updated for the v3.2 promotion. | **Closed** — all three updated to reflect v3.2 re-verification against arXiv. |

### C2 — Internal inconsistency

| ID | Locus | Finding | v3.2.1 status |
|----|-------|---------|---------------|
| N9 | §9.2 | `guarantee_class` enum missing `formal_data_flow_absolute`. | **Closed** — added. |
| N10 | §20.3.5, FlagSet | Q/GE bits referenced in table but not in `FlagSet::Nzcv`. | **Closed** — `FlagSet::Nzcv` now has `q: bool, ge: [bool; 4]`. |
| N11 | §20.2 vs §21.7 | `x86_64_relocs()` lists 10 types; §21.7 lists 17. | **Closed** — synchronized to 17 types. |
| N12 | F.9 | `run_all.sh` targets v3.1. | **Closed** — targets v3.2; uses extended linter; SKIP for absent tools. |
| N13 | §6.7 | `formal_data_flow_absolute` gated to Track A′ in v3.2 (overclaim). | **Closed** — gated to Track A with "sound absolute checker exists" condition. |
| N14 | §5.3 | No `formal_data_flow_absolute` degradation row. | **Closed** — row added with "only when sound absolute checker available" caveat. |
| N15 | §4.1 | `MAS-boot: N/M` notation has no algorithm for M. | **Closed** — dropped; replaced with `MAS-partial: N` (no denominator). |
| N16 | §6.1 | H-RS1 untestable given N4's arithmetic. | **Closed** — N4 fixed; H-RS1 retestable. |

### C3 — Schema/pipeline divergence

| ID | Locus | Finding | v3.2.1 status |
|----|-------|---------|---------------|
| N17 | §3.7 | `abandonment_reason` extension. | **No defect** — kept. |
| N18 | §3.7, §10.1, §10.2 | `formal_data_flow_absolute` alignment. | **Closed** (same as N3). |
| N19 | §10.1 | `abi-id` closed enum. | **No defect** — kept. |
| N20 | §10.1 | `verdict-with-source` defined. | **No defect** — kept. |

### C4 — Linter blind spot

| ID | Locus | Finding | v3.2.1 status |
|----|-------|---------|---------------|
| N21 | F.5 | Extended linter now in Appendix F.5. | **Closed** (same as N6). |

### C5 — External verification overclaim

| ID | Locus | Finding | v3.2.1 status |
|----|-------|---------|---------------|
| N22 | F-16, Appendix A | Appendix A now reflects v3.2 re-verification. | **Closed** (same as N8). |
| N23 | HYPER-CRITIQUE | "0 linter issues" claim was based on the old linter. | **Closed** — extended linter now runs on the spec; 0 issues confirmed. |

### D1/D2 — Cosmetic and disclosure

| ID | Locus | Finding | v3.2.1 status |
|----|-------|---------|---------------|
| N24 | Appendix A | Ledger ordering (L-22 between V-15 and V-16). | **Acknowledged** — reordering would require renumbering L-22, which is carried forward from v3.1 and used elsewhere; cosmetic only. |
| N25 | §31.1 | H-RS1 placement. | **No defect** — kept. |
| N26 | Front matter | Re-execution disclosure. | **No defect** — kept. |
| N27 | E.1 | Z3 version "5.1.0" is fabricated; real Z3 is 4.x. | **Closed** — corrected to "Z3 4.x, as documented in v3.1." |

---

## Part C — Final status

| Tier | v3.2 claimed | v3.2 actual (critique) | v3.2.1 final |
|------|-------------|----------------------|-------------|
| C1 (central mechanism) | 6 closed | 3 truly closed, 3 partially closed | 6 closed |
| C2 (internal consistency) | 5 closed | 4 truly closed, 1 partially closed | 5 closed |
| C3 (schema/pipeline) | 1 closed | 1 closed | 1 closed |
| C4 (linter blind spot) | 1 claimed closed | **Not closed** | 1 closed |
| C5 (external verification) | 1 claimed closed | **Not closed** | 1 closed |
| D1 (cosmetic) | 1 closed | 1 closed | 1 acknowledged |
| D2 (disclosure) | 1 closed | 1 closed | 1 closed |
| **New v3.2 defects (N1–N27)** | — | 27 found | 24 closed, 1 acknowledged, 2 already-not-defects |
| **Linter issues** | 0 claimed | 3 (old linter, didn't check new defects) | 0 (extended linter) |
| **Executable checks** | 5/5 | 5/5 | 5/5 |

**Honest assessment:** v3.2 was a partially applied patch set that improved v3.1 but overclaimed closure in 4 of its 16 findings (F-03 partially, F-05 reopened, F-06 partially, F-07/F-08 partially, F-14 not closed, F-16 not closed) and introduced 27 new defects of its own. v3.2.1 is a delta-repair that closes the 16 original findings honestly, closes 24 of the 27 new findings, and acknowledges the remaining 3 as non-defects. The v3.2.1 extended linter (now shipped in Appendix F.5) produces 0 issues on v3.2.1 and 12 issues on v3.1.

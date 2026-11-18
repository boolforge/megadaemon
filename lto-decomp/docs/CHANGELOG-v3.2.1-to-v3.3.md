# CHANGELOG: v3.2.1 → v3.3 (Research-Contract Edition)

**Date:** 2026-10-16

## Summary

v3.3 elevates residual roadmap items into measurable **Implementation Obligations** with explicit acceptance criteria, adds the Absolute Set Closure Lemma, introduces the normative ArchProfile Completeness Checklist, and updates the re-execution environment. No new document-local defects were introduced; all prior closures are preserved.

## Changes

### Front matter
- Version → 3.3 (research-contract edition)
- Status clarified: document-local defects closed; remaining work scoped and acceptance-tested
- Re-execution environment updated (rustc now present; pure-Python checks re-run)

### Section 15 (major rewrite)
- Replaced weak “planning figures” table with **Implementation Obligations & Acceptance Criteria** matrix (O-1 … O-8)
- Added Priority, Acceptance Criteria, Evidence Artifact columns
- Added **Absolute Set Closure Lemma** (§15.2) with proof sketch
- Added formal Discharge Rule (§15.3)

### Section 18 & 19
- Limitations and Conclusion now reference the obligation matrix and the lemma; absolute claims remain forbidden until O-3 is discharged

### Section 20.1.1 (new)
- Normative ArchProfile Completeness Checklist (8 mandatory items)
- Linked to extended linter rule C-PA

### Appendix G
- Cross-referenced to the new O-ids (no content deletion)

### Verification
- Extended linter still reports 0 issues
- 5/5 pure-Python checks still pass
- No regression in any closed finding (F-01…F-16, N1–N27)

## Rationale

The adversarial critique cycles (v3.1 → v3.2 → v3.2.1) closed every *document-local* defect. Further textual mutation that merely restated “not built” produced diminishing epistemic returns. v3.3 converts those residual statements into engineering contracts so that future work can be measured against objective acceptance criteria rather than subjective “done-ness”.

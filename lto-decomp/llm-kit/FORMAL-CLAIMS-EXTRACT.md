# FORMAL CLAIMS EXTRACT — LTO-Decomp v3.4

This file extracts only the normative formal statements, lemmas, formulas, and soundness boundaries. It is intended for high-precision agent reasoning.

## Absolute Guarantee Set

```
A = { formal_control_flow }
```

Only members of A may appear in absolute form on a badge. All other guarantee classes are relational or empirical.

## Absolute Set Closure Lemma (v3.3/v3.4 §15.2)

If G ∉ A then any badge claiming G as absolute is rejected by the grammar.  
If G ∈ A and the layer’s eligibility predicate and policy walls are satisfied, the absolute form is permitted.  
Proof is by construction of the badge grammar and exact-membership test. Rice’s theorem is confined to the idealized unbounded-memory model; the concrete model is bounded by explicit policy walls.

## Static Risk Score (§6.1)

```
S = 8·M + 1000·P + 20·A + 50·L + 200·I
R = min(100, ⌊S / 50⌋)
```

Typical values (M=10, P=0.05, A=3, L=1, I=0) → S=240, R=4.

## Conformal Quantile (corrected)

```
k = ⌈(n+1)(1-α)⌉
```

Restores finite-sample coverage to nominal 1-α (Monte-Carlo verified).

## Memory Aliasing

Circular overlap predicate is sound for all 8-bit address pairs and sizes {1,2,4,8,16} (1 638 400 cases, 0 mismatches).  
SMT theories: QF_ABV / QF_ABVFP (Layer 2), QF_AUFBV / QF_AUFBVFP (Layer 5).

## Flag Polarity

x86 CF and AArch64 C are exact complements for every 8-bit (a,b) pair (65 536 cases, 0 mismatches).

## Track Routing

| Track | MAS range          | Claim power                          |
|-------|--------------------|--------------------------------------|
| A     | ≥ 0.70             | Per-layer positive verdicts inside envelopes |
| A′    | [0.30, 0.70)       | Rehabilitated layers only            |
| B     | < 0.30             | Ranked hypotheses + conformal sets   |

## Soundness Boundaries (non-exhaustive)

- Rice’s theorem forbids a complete procedure for non-trivial semantic properties over all programs (idealized model).
- Concrete model is finite-state; equivalence is decidable in principle but intractable; policy walls enforce abandonment.
- Solver correctness is an unmodeled residual risk (dual-solver mitigation permitted for Layer 5).
- No end-to-end mechanized proof of absolute guarantees currently exists.
- CIRX Stage-1 lifter and Layer-5 relational harness do not exist; absolute claims that depend on them are forbidden.

## Forbidden Patterns (linter-enforced)

TODO, …, etc., see above, ..., TBD, FIXME — forbidden in normative text.

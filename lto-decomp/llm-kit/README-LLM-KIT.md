# LLM / Agent Ingestion Kit

This directory contains high-fidelity, low-token extracts designed for automated agents and LLMs whose context windows cannot hold the full 5300-line specification.

## Files (recommended consumption order)

1. **QUICK-CONTEXT-FOR-AGENTS.md** — minimal high-signal overview + agent contract
2. **OBLIGATIONS.json** — machine-readable open obligations with acceptance criteria
3. **FORMAL-CLAIMS-EXTRACT.md** — pure formal statements, lemmas, formulas, soundness boundaries
4. **SECTION-INDEX.json** — structural map of the full specification
5. **SYSTEM-PROMPT-SNIPPET.md** — ready-to-paste system-prompt fragment

## Design principles

- Zero loss of normative force relative to the full text for the extracted claims
- Explicit residual obligations (never elided)
- Absolute-set membership is the single source of absolute power
- All quantitative gates and discharge rules preserved

When in doubt, fall back to the full specification in `../spec/`.

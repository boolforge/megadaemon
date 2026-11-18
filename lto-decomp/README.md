# LTO-Decomp Architecture Specification — Distribution Package v3.4

**Single-file download for humans, researchers, and automated agents.**

## What this package contains

```
LTO-Decomp-Distribution-v3.4/
├── README.md                          ← you are here
├── MANIFEST.sha256                    ← integrity
├── spec/
│   └── LTO-Decomp-Architecture-Specification-v3.4.md   ← primary normative artifact
├── llm-kit/                           ← high-fidelity extracts for LLMs & agents
│   ├── QUICK-CONTEXT-FOR-AGENTS.md
│   ├── OBLIGATIONS.json
│   ├── FORMAL-CLAIMS-EXTRACT.md
│   ├── SECTION-INDEX.json
│   ├── SYSTEM-PROMPT-SNIPPET.md
│   └── README-LLM-KIT.md
├── tools/                             ← extended linter + 5 pure-Python checks
├── verify/                            ← captured verification outputs
├── schemas/                           ← JSON schemas
├── src/                               ← CIRX Micro-RTL reference (Rust)
├── docs/                              ← final report, changelogs, findings, critiques
├── history/                           ← v3.1, v3.2.1, v3.3 + original adversarial critique
└── meta/                              ← reserved
```

## Quick start (human)

1. Read `spec/LTO-Decomp-Architecture-Specification-v3.4.md`
2. Run verification: `cd tools && ./run_checks.sh`
3. Consult `docs/FINAL-REPORT-LTO-Decomp-v3.3.md` for the full adversarial history

## Quick start (LLM / agent)

1. Ingest `llm-kit/QUICK-CONTEXT-FOR-AGENTS.md` first
2. Load `llm-kit/OBLIGATIONS.json` and `llm-kit/FORMAL-CLAIMS-EXTRACT.md`
3. Paste `llm-kit/SYSTEM-PROMPT-SNIPPET.md` into the system prompt
4. Fall back to the full specification only if remaining context budget allows

## Status

- Document-local defects: **closed**
- Extended linter: **0 issues**
- Executable checks: **5/5 pass**
- Residual work: ranked Implementation Obligations (O-1 … O-8) with quantitative acceptance gates

## Provenance

Derived from the immutable v3.1 baseline through four adversarial cycles (v3.2 → v3.2.1 → v3.3 → v3.4). Full history is preserved in `history/` and `docs/`.

## License / Citation

Treat as a research architecture specification. Cite the version string and the Absolute Set Closure Lemma when claiming formal properties.

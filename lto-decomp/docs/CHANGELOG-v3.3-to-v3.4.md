# CHANGELOG: v3.3 → v3.4 (Distribution & Agent-Ingestion Edition)

**Date:** 2026-10-16

## Summary

v3.4 is the distribution-ready, agent-optimized edition of the research-contract specification. No new document-local defects; all prior closures preserved. Focus is on machine-readable artifacts, quantitative discharge metrics, and explicit LLM/agent consumption contracts.

## Additions

- §15.4 Quantitative Discharge Metrics (corpus sizes, false-proof bounds, reproducibility requirements)
- §15.5 Agent & LLM Consumption Contract (five mandatory rules)
- Complete `llm-kit/` directory with QUICK-CONTEXT, OBLIGATIONS.json, FORMAL-CLAIMS-EXTRACT, SECTION-INDEX, SYSTEM-PROMPT-SNIPPET
- Full historical provenance (v3.1 → v3.4) preserved inside the distribution package
- Top-level MANIFEST.sha256 for integrity verification

## Verification

- Extended linter: 0 issues
- 5/5 pure-Python checks: pass

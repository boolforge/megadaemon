# Banco de pruebas y auditoría — las cifras del repo salen de aquí

Estos scripts son la **evidencia ejecutable** de los números citados en `docs/`.
No son un adorno: sin ellos, las afirmaciones del repo no son reproducibles.

| Script | Qué produce | Estado verificado |
|---|---|---|
| `audit_v10.py` | Auditoría del harness **v10**: ejecuta cada defecto contra el código real y lo confirma o lo refuta. | **24 defectos confirmados, 1 verified-correct, 0 no reproducidos** → `audit_v10.json` (idéntico byte a byte al versionado). |
| `bench_fidelity.py` | Fidelidad de enrutado por modelo (12 preguntas que exigen datos externos, en español). | Qwen2.5-0.5B 8/12 · SmolLM2-360M 12/12 · Llama-3.2-1B 12/12 · Qwen2.5-1.5B 11/12. |
| `bench_models.py` | Parser v11.1 con N modelos y 4 idiomas (ES/ES, EN/EN, ES/EN, EN/ES). | 12 preguntas × 4 modelos × 4 idiomas. |
| `token_budget.py` | Presupuesto de tokens **real** (con `llama-tokenize`, no con aproximaciones). | v10 247 tok → v11 EN 187 / ES 206. |
| `bench_e2e.py` | E2E genérico de extremo a extremo contra `llama-server`. | — |

## Ejecución

```bash
cd tools
python3 audit_v10.py          # auditoría del v10 (no necesita LLM)
python3 bench_fidelity.py     # necesita llama-server + modelos
python3 bench_models.py       # necesita llama-server + modelos
python3 token_budget.py       # necesita llama-tokenize + modelos
```

## Configuración

| Variable | Por defecto | Uso |
|---|---|---|
| `MEGADAEMON_MODELS_DIR` | `/workspace/models` | Directorio con los `.gguf`. |
| `MEGADAEMON_RESEARCH_DIR` | `/workspace/mavis-deep-research/20261118_203500_seere-tiny-llm-prompt` | Modelos de investigación y `llama-server`. |
| `MEGADAEMON_LLAMA_SERVER` | — | Ruta explícita a `llama-server` (si no, se busca en `$PATH`). |
| `MEGADAEMON_LLAMA_TOKENIZE` | — | Ruta explícita a `llama-tokenize`. |
| `SEERE_LLM` | `http://127.0.0.1:8080` | Endpoint del LLM local. |

## Nota de honestidad

Los scripts de benchmark **miden**, no estiman: levantan un `llama-server`
real, le mandan la pregunta y comprueban si el agente pidió una herramienta y
si esa herramienta resolvería la pregunta. Las cifras de `docs/SEERE_V11.md` y
`docs/LLM_SWITCH_PLAN.md` son la salida literal de estas herramientas.

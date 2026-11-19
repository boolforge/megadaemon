# Referencia Python SEERE v11 — el **oráculo** del port Rust

Este directorio contiene la implementación **Python** del harness SEERE v11: es el
**oráculo** contra el que se escribió y verificó el port Rust
(`rust-core/src/seere_v11.rs`). Rust es el camino de ejecución en producción;
Python es el contrato de referencia, legible y testeable fuera del binario.

| Fichero | Papel |
|---|---|
| `seere_v10_original.py` | El prompt/harness **v10** original, tal cual se descargó — es el objetivo de la auditoría de v10 (24 defectos confirmados). |
| `seere_v11.py` | El harness **v11** de referencia: gramáticas GBNF tipadas, guardián anti-eco, huella fuera del prompt, política cerrada, taint tracking, presupuesto de tokens. |
| `test_seere_v11.py` | **34 tests** de regresión v10→v11 (`F01…F25` + `N01…N10`). |
| `gbnf_check.py` | Verificador de coherencia **gramática ↔ parser ↔ dispatch** (33/33). |

## Ejecución

```bash
cd reference/python
python3 test_seere_v11.py     # -> REGRESSION: 34/34 PASS
python3 gbnf_check.py         # -> GBNF_CHECK_OK  (conformance 33/33)
```

Variables de entorno:

| Variable | Por defecto | Uso |
|---|---|---|
| `SEERE_V10_PATH` | `reference/python/seere_v10_original.py` | Auditar otra copia del v10. |
| `SEERE_LLM` | `http://127.0.0.1:8080` | Endpoint `llama-server` del LLM local. |
| `SEERE_STATE` | `/tmp/seere_reg_state` | Estado efímero del harness. |

## La regla que produjo esta estructura

> **La gramática es el contrato; el prompt es la semántica.**

La sintaxis vive en el GBNF tipado (`OP_SPEC` + `ARG_RULES`): coste 0 tokens y
ningún desacuerdo posible entre lo que el modelo puede producir y lo que el
parser acepta. El prompt sólo lleva la intención. Ese principio — y sólo él —
fue lo que tumbó los 24 defectos del v10.

## Por qué el Rust no es una reimplementación libre

`rust-core/src/seere_v11.rs` no es una traducción: ambos lados implementan el
mismo contrato (`OP_SPEC` / `ARG_RULES` / presupuesto de tokens / política de
taint) y el comportamiento observable se compara en el E2E real contra el mismo
`llama-server` y los mismos modelos (Qwen2.5-0.5B, SmolLM2-360M,
Llama-3.2-1B, Qwen2.5-1.5B). Las cifras están en `docs/SEERE_V11.md`.

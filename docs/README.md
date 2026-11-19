# Documentación técnica — MegaDaemon v13.2

| Documento | Qué contiene |
|---|---|
| [`NPU_REAL.md`](NPU_REAL.md) | **NPU real**: qué se ejecuta en el NPU (NNAPI INT8), qué NO puede ejecutarse (el LLM) y por qué, con la evidencia y el protocolo de verificación en el dispositivo. |
| [`AUDIT_V13_1.md`](AUDIT_V13_1.md) | Auditoría adversarial del repo v13.0/v13.1: 31 hallazgos con evidencia ejecutable + addendum v13.2 con los 6 nuevos. |
| [`LLM_SWITCH_PLAN.md`](LLM_SWITCH_PLAN.md) | Plan de cambio de LLM con datos: 4 modelos medidos, jerarquía de palancas, 5 fases, umbrales, riesgos y rollback. |
| [`SEERE_V11.md`](SEERE_V11.md) | Harness SEERE: los 24 defectos de la v10, el diseño de la v11 (gramáticas tipadas, anti-eco, huella fuera del prompt) y los benchmarks E2E con modelos reales. |

## Cómo se verifica el repo

```bash
cd rust-core
cargo build --release
cargo test                                    # 34 pruebas (21 auditoría + 8 NPU + 5 unitarias)
./target/release/megadaemon-core seere-selftest
./target/release/megadaemon-core npu-cv      # informe de visión con el backend real
./target/release/megadaemon-core npu-probe   # telemetría de hardware + política de aceleradores
```

## Principio rector

> Prometer en la telemetría lo que el código no hace es un **bug**.
> Cada afirmación de estos documentos está respaldada por una prueba
> ejecutable, una medición con su fuente, o marcada explícitamente como
> "requiere hardware no disponible en el entorno de desarrollo".

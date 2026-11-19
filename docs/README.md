# Documentación técnica — MegaDaemon v13.2

| Documento | Qué contiene |
|---|---|
| [`NPU_REAL.md`](NPU_REAL.md) | **NPU real**: qué se ejecuta en el NPU (NNAPI INT8), qué NO puede ejecutarse (el LLM) y por qué, con la evidencia y el protocolo de verificación en el dispositivo. |
| [`AUDIT_V13_1.md`](AUDIT_V13_1.md) | Auditoría adversarial del repo v13.0/v13.1: 31 hallazgos con evidencia ejecutable + addendum v13.2 con los 6 nuevos. |
| [`AUDIT_V13_2.md`](AUDIT_V13_2.md) | **Auditoría adversarial v13.2**: 16 hallazgos de hardware/servidor/seguridad (HW-01…HW-16) + 17 del harness SEERE (S01…S14b), con test antes/después. |
| [`LLM_SWITCH_PLAN.md`](LLM_SWITCH_PLAN.md) | Plan de cambio de LLM con datos: 4 modelos medidos, jerarquía de palancas, 5 fases, umbrales, riesgos y rollback. |
| [`DEVICE_DEPLOY.md`](DEVICE_DEPLOY.md) | **Despliegue real en el dispositivo** (Galaxy S21 Ultra / Exynos 2100): inventario, megadaemon como demonio, API key remota verificada, deduplicación de los tres entornos, subida a Drive y estado de cada fase. |
| [`SEERE_V11.md`](SEERE_V11.md) | Harness SEERE: los 24 defectos de la v10, el diseño de la v11 (gramáticas tipadas, anti-eco, huella fuera del prompt) y los benchmarks E2E con modelos reales. |

## Código de referencia y evidencia

| Ruta | Qué es |
|---|---|
| [`reference/python/`](../reference/python/README.md) | El **oráculo Python** del harness SEERE v11: `seere_v11.py`, el v10 original auditado, 34 tests de regresión y el verificador GBNF↔parser (33/33). El port Rust de producción se validó contra él. |
| [`tools/`](../tools/README.md) | El banco que produce las cifras del repo: `audit_v10.py` (24 defectos confirmados), `bench_fidelity.py`, `bench_models.py`, `token_budget.py`, `bench_e2e.py`. |
| `rust-core/tests/` | Las suites de auditoría en Rust: 93 tests (`audit_v13`, `audit_scout_hw`, `audit_scout_seere`, `audit_scout_auth`, `npu_nnapi`). |

## Cómo se verifica el repo

```bash
cd rust-core
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release                    # 0 errores, 0 warnings
cargo test                               # 93 pruebas (21 v13.1 + 16 HW + 31 SEERE + 7 AUTH + 9 NNAPI + 9 unit)
./target/release/megadaemon-core seere-selftest   # SELFTEST_V11_RUST_OK
./target/release/megadaemon-core npu-cv           # informe de visión con el backend real
./target/release/megadaemon-core npu-probe        # telemetría de hardware + política de aceleradores
./target/release/megadaemon-core cockpit 3999     # web UI (token en <STATE_DIR>/cockpit_token)
```

## Seguridad por defecto (v13.2)

* El cockpit es **fail-closed**: exige token (`X-Seere-Auth-Token`) en **todo** `/api/*`,
  sea GET o POST.  La única superficie pública es la página HTML (lleva el token embebido).
* El token vive en `<STATE_DIR>/cockpit_token` (0600) o en `SEERE_AUTH_TOKEN`.
* El CORS **no** es abierto: se refleja el `Origin` sólo si coincide con el `Host` o está en
  `MEGADAEMON_ALLOWED_ORIGINS`.
* El NPU se anuncia sólo si se ejecutó de verdad: `executed_on_hardware` + `fallback_reason`.

## Principio rector

> Prometer en la telemetría lo que el código no hace es un **bug**.
> Cada afirmación de estos documentos está respaldada por una prueba
> ejecutable, una medición con su fuente, o marcada explícitamente como
> "requiere hardware no disponible en el entorno de desarrollo".

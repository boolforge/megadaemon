# Documentación técnica — MegaDaemon v13.2

| Documento | Qué contiene |
|---|---|
| [`QA_SECURITY_HARNESS_SPEC.md`](QA_SECURITY_HARNESS_SPEC.md) | **Especificación del harness de QA y de seguridad**: qué del manual de QA móvil/pen-test ya existe en MegaDaemon (con `fichero.rs:línea`), matriz manual → MegaDaemon, la frontera de alcance, y el diseño propuesto de `qa_engine.rs`, `sec_audit.rs` e `ipc_audit.rs` más el puente de interceptación Brida. |
| [`REPO_TRIAGE.md`](REPO_TRIAGE.md) | **Triage de 67 repos de referencia**: URL, descripción, licencia verificada en el fichero `LICENSE` (o `NO VERIFICADO`), lenguaje, relevancia, acción concreta y destino en el daemon, más el Top 10 por valor. |
| [`EXTERNAL_AGENT_PROMPT_FINAL.md`](EXTERNAL_AGENT_PROMPT_FINAL.md) | **Prompt listo para pegar al agente externo de RE Android** (conexión MCP, self-check, las 27 tools, el encargo de reverse por fases y el contrato de entrega `MANIFEST.ndjson` + `REPORT.md` + `findings.ndjson` v1.2). Las dos credenciales van como marcadores: la copia rellenada vive fuera del repo, en `/workspace/md-secrets/EXTERNAL_AGENT_PROMPT_WITH_KEYS.md` (0600). |
| [`CONNECT_EXTERNAL_AGENT_ANDROID_RE.md`](CONNECT_EXTERNAL_AGENT_ANDROID_RE.md) | Contrato completo del agente externo: enums cerrados de `detector`/`class`/`method`, reglas de los 19 campos, política de Play Integrity y canales verificados de subida. |
| [`NPU_REAL.md`](NPU_REAL.md) | **NPU real**: qué se ejecuta en el NPU (NNAPI INT8), qué NO puede ejecutarse (el LLM) y por qué, con la evidencia y el protocolo de verificación en el dispositivo. |
| [`AUDIT_V13_1.md`](AUDIT_V13_1.md) | Auditoría adversarial del repo v13.0/v13.1: 31 hallazgos con evidencia ejecutable + addendum v13.2 con los 6 nuevos. |
| [`AUDIT_V13_2.md`](AUDIT_V13_2.md) | **Auditoría adversarial v13.2**: 16 hallazgos de hardware/servidor/seguridad (HW-01…HW-16) + 17 del harness SEERE (S01…S14b), con test antes/después. |
| [`LLM_SWITCH_PLAN.md`](LLM_SWITCH_PLAN.md) | Plan de cambio de LLM con datos: 4 modelos medidos, jerarquía de palancas, 5 fases, umbrales, riesgos y rollback. |
| [`DEVICE_DEPLOY.md`](DEVICE_DEPLOY.md) | **Despliegue real en el dispositivo** (Galaxy S21 Ultra / Exynos 2100): inventario, megadaemon como demonio, API key remota verificada, deduplicación de los tres entornos, subida a Drive y estado de cada fase. |
| [`SEERE_V11.md`](SEERE_V11.md) | Harness SEERE: los 24 defectos de la v10, el diseño de la v11 (gramáticas tipadas, anti-eco, huella fuera del prompt) y los benchmarks E2E con modelos reales. |
| [`CERTIFICADOS_Y_SEDES_ELECTRONICAS.md`](CERTIFICADOS_Y_SEDES_ELECTRONICAS.md) | **Certificados y sedes electrónicas**: SALUD Andalucía (historia clínica), Receta Electrónica Andalucía, Sede Electrónica de la Junta, Sede Judicial Electrónica, Seguridad Social y Cl@ve — qué se puede automatizar, qué **no** (prescripción, dispensación, firma) y con qué URL oficial está verificado cada cosa. |
| [`CERT_SYNC_EN_NODOS.md`](CERT_SYNC_EN_NODOS.md) | **Sincronización de certificados entre nodos**: flujo inventario → cifrado (AEGIS-256 + Argon2id) → transferencia → despliegue → uso → revocación, con la política de `confirm`, la traza de auditoría y la frontera legal (certificados personales e intransferibles, datos de salud = categoría especial RGPD art. 9). |

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

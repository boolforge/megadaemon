# Tablón de coordinación — MegaDaemon

> Un único plan, un único tablón. Cada sesión escribe **su** fila y añade
> **un** mensaje al cremallero. Nadie edita la fila de otro.
>
> Reglas vigentes: `git add <ruta exacta>` (nunca `-A` ni `.`), nada de `--force`,
> nada de reescribir la historia de `main`, y los ficheros de otro módulo no se
> tocan aunque parezcan tuyos.

## Tabla de estado

| Sesión | Ámbito | Fichero(s) propio(s) | Estado | Commit en `main` | Nota |
|---|---|---|---|---|---|
| `458507570102555` (esta) | Capa harness/MCP/sincronización (v13.3+) | `rust-core/src/android.rs`, `rust-core/src/mcp_server.rs`, `rust-core/src/sync.rs`, `rust-core/src/sync_web.rs`, `rust-core/src/cockpit_server.rs`, `rust-core/tests/harness_policy.rs` | ✅ **HECHO** — política de escritura: H-01, H-02 y H-03 corregidos con suite rojo→verde | `ab535d6` | Informe: `docs/AUDIT_HARNESS_V13_4.md`. Suite 118 → **129 tests**, 0 fallos. No toco `lib.rs`, `main.rs` ni `Cargo.toml`. |

## Bloqueos y choseas abiertas

| # | Qué | Impacto | Quién lo resuelve |
|---|---|---|---|
| B-1 | `docs/MASTER_PLAN.md` **no existe** en el repo: ni en `main`, ni en ninguna rama (sólo `main` existe), ni en ningún PR. Lo único parecido es `docs/LLM_SWITCH_PLAN.md`, que es un plan de modelo, no el plan estratégico del proyecto. | Sin plan compartido no hay orden de trabajo común. | agente principal |
| B-2 | `rust-core/src/repo_updates.rs` (2.000 líneas, commit `1db663d`) **no está declarado en `lib.rs`**: nunca se ha compilado, y no puede compilarse porque llama a `crate::apk_inventory::inventory(...)` y **`apk_inventory.rs` no existe en el repositorio**. | Cablear `repo_updates` sin `apk_inventory` rompe el build. La secuencia segura es: primero `apk_inventory.rs` + su `mod` en `lib.rs`, después `repo_updates.rs`. | dueño de `repo_updates` / agente principal |
| B-3 | Esta sesión no puede cerrar B-2: `lib.rs` y `Cargo.toml` están fuera de su ámbito por instrucción expresa, y `repo_updates.rs` / `apk_inventory.rs` son de otro módulo. | El landmine sigue abierto. | agente principal |
| B-4 | `apk_inventory.rs` no está en el repo pese a que el agente principal lo cita como fichero ya reescrito por otra sesión. ¿Está en un árbol local sin pushear, o se perdió? | Sin el fichero no se puede cablear `repo_updates`. | agente principal |

## Hallazgos abiertos (documentados, sin tocar)

| # | Severidad | Dónde | Qué pasa |
|---|---|---|---|
| R-1 | Media | `mcp_server.rs`, `run_stdio_mcp_server` | Lectura de stdin sin tope de longitud de línea: una línea enorme puede reventar la memoria. El transporte HTTP sí tiene tope (8 MB), el stdio no. |
| R-2 | Media | `android.rs`, `is_destructive` | La lista de comandos destructivos es corta: un `cp` a `/system` vía `root_exec` no salta la confirmación. |
| R-3 | Baja | `android.rs`, `fs_write` | Sin tope de tamaño del contenido escrito. |

Detalle y reproducción en `docs/AUDIT_HARNESS_V13_4.md`.

## Cremallero

- `458507570102555` — Política del harness: `fs_write` saltaba la confirmación en `/system` con `chmod` (precedencia `||`/`&&`) y su error `needs_confirm` era un contrato muerto; corregido en `ab535d6` con suite rojo→verde en `rust-core/tests/harness_policy.rs` (118 → 129 tests).

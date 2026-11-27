# Material propietario: **documentar sí, copiar no**

> Este documento es el **índice y la regla** para todo el material de
> terceros que entra en el proyecto.  Los catálogos detallados están en los
> documentos de RE; aquí está la política y el **gate que la hace cumplir**.
>
> MEDIDO 2026-11-28.

## 1. La regla, en tres líneas

1. Lo **propietario** se **DOCUMENTA**: qué hace, dónde se midió, para qué
   lo queremos.  Nunca se transcribe su código.
2. Lo que tiene **licencia compatible** se puede **vendorizar**, pero con
   **atribución** en el propio fichero (Apache-2.0 → los avisos de licencia
   originales).
3. Lo que se **rescata** se re-expresa **funcionalmente equivalente**
   (AST → Rust): OUR Rust, OUR nombres, OUR estructura, el **mismo
   comportamiento observable**.  El texto es nuevo.

## 2. Qué material es de quién (MEDIDO)

| producto | namespace | Extension(es) | nº | catálogo completo |
|---|---|---|---:|---|
| **Devin Desktop** (ex‑Windsurf) | `cognition.ai/*` | 37 capacidades: subagentes, revert, plugins, MCP, browser, refs, modelos, carga de historial… | 37 | `docs/RE_ACP.md` §6 |
| **acpira** | `acpira/*` | `context`, `rewind`, `sidekick`, `summoned`, `turn`, `availableModes` | 6 | `docs/RE_ACP_COMPARADO.md` |
| **MiniMax Code** | `mcode/*` | `session/steer`, `session/queue/{list,enqueue,update,delete,steer}`, `session/goal/{get,create,patch,clear}`, `session/delegation/{get,stop}`, `session/current_session_update`, `session/queue_update`, `session/goal_update`, `session/delegation_update` | 13 | `docs/RE_ACP_COMPARADO.md` |
| **ACP (esquema)** | — | crate oficial `agent-client-protocol-schema` 1.22.0, **Apache-2.0** | — | `docs/ACP_VENDOR.md` + `docs/ACP_VENDOR_LICENSE.md` |

### 2.1 Convergencias: lo que se implementa **una vez**

MEDIDO (`docs/RE_ACP_COMPARADO.md`): las extensiones se parecen entre sí
aunque los productos sean competidores.

| convergencia | quién | qué hacemos |
|---|---|---|
| deshacer sin borrar | `acpira/rewind` ≈ `cognition.ai/revert` | `session/fork` de ACP (`rust-core/src/protocol_acp/sesion.rs`) + el patrón 4 (épocas) |
| subagentes | `mcode/session/delegation` ≈ `cognition.ai/subagentControl` | `agent_core.rs` / `meta_agent.rs` |
| lead + sidecar | `acpira/sidekick` ≈ Fusion (`local_fusion/sidekick_tool.rs`) | `meta_agent.rs` |
| cola de mensajes | `mcode/session/queue/*` ≈ `cognition.ai/queuedMessagesInfoUpdate` | `service_bus.rs` (la cola ES el protocolo) |

⇒ **una** implementación nuestra, no tres copias de tres productos.

## 3. Lo que SÍ está en el repo y por qué es legal

| qué | licencia | atribución |
|---|---|---|
| `rust-core/src/acp_schema.rs` (esquema ACP v1 vendorizado) | **Apache-2.0** | cabecera del fichero + `docs/ACP_VENDOR.md` + `docs/ACP_VENDOR_LICENSE.md` |
| `docs/RE_*.md` (análisis) | análisis propio | citas con ruta y línea del original; **sin extractos largos** |
| `artifacts/DEVIN_CONTRATOS.md` | análisis propio (hechos medidos) | versionado: es análisis, no transcripción |

## 4. La conversión AST → Rust (cómo se hace)

Cuando hay que "rescatar" una capacidad de un producto sin licencia
compatible, el procedimiento es:

1. **Documentar** la capacidad (qué hace, con qué entradas y salidas, con
   qué efecto observable) ⇒ eso es el catálogo de la §2.
2. **Escribir OUR Rust desde cero**, con los nombres y la estructura del
   repo, no los del original.  La equivalencia que se busca es **de
   comportamiento observable**, no de texto:
   * mismas **entradas** aceptadas y rechazadas.
   * Misma **salida** (misma forma JSON, mismos campos, mismos códigos de
     error).
   * Mismos **efectos secundarios** (ficheros tocados, llamadas hechas,
     orden de las operaciones).
   * **NO** el mismo texto, ni los mismos nombres, ni la misma estructura.
3. **Testear la equivalencia** con casos que fija el *comportamiento*
   (no la implementación).  Los tests del repo son el banco de pruebas:
   `rust-core/tests/`.

### 4.1 Ejemplos ya hechos

| capacidad | nuestra implementación | nota |
|---|---|---|
| `PLUGIN_DATA` por identidad (patrón 9) | `rust-core/src/plugin_data.rs` | implementación propia; la v16 es sólo el **arquitectura** medida |
| keepalive del caché (patrón 2) | `rust-core/src/cache_keepalive.rs` | la política (TTL, mínimo, presupuesto) es **nuestra** |
| compactación por época (patrón 4) | `rust-core/src/compaction_epoch.rs` | el DDL de la referencia está **documentado**, el modelo es nuestro |
| `session/fork` (≈ `acpira/rewind`) | `rust-core/src/protocol_acp/sesion.rs` | el bosque de sesiones es nuestro |

## 5. El gate: `tools/check-propietario.py`

La regla no es una promesa: hay un script que la comprueba.

```sh
python3 tools/check-propietario.py
MD_TERCEROS_DIR=/ruta/a/las/fuentes/de/terceros python3 tools/check-propietario.py
```

Qué comprueba (MEDIDO 2026-11-28):

1. `artifacts/contratos-productos-2026-11-27/` **no** tiene `.md`
   versionados (son extractos de productos propietarios: posible copia
   literal).  Sólo se versionan los inventarios.
2. Ningún `.rs` de terceros está versionado en el repo.
3. El módulo vendorizado del esquema ACP existe, lleva su cabecera de
   atribución y es el **único** fichero `.rs` que menciona el crate
   original en **código** (una mención en un comentario es atribución
   legítima).
4. Las dos fichas de licencia y atribución existen.
5. Con `MD_TERCEROS_DIR`: busca **copia literal** (≥ 12 líneas consecutivas
   iguales) entre las fuentes de terceros y nuestro código.

**MEDIDO**: el check 5 se probó con una copia artificial de 91 líneas de
`cache_keepalive.rs` y la detectó (rachas de 13 y 14 líneas).  Sin
`MD_TERCEROS_DIR` el check 5 **no** se ejecuta y el gate lo dice (no se
pasa por alto en silencio).

## 6. Lo que NO se hace

* No se pegan extractos de los `.md` de los contratos de productos: van a
  Drive (carpeta `megadaemon-backup`, id `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`).
* No se reexporta código propietario con otro nombre.
* No se "limpia" un fichero propietario quitando los comentarios y se
  llama propio: el gate 5 lo caza (los comentarios **se comparan**).

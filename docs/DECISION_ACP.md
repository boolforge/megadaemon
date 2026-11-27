# Decisión: nuestro dialecto de protocolo es **ACP v1 canónico + extensiones `mdaemon/`**

> Decidido el 2026-11-27 con datos medidos. Todo lo que sigue lleva la fuente.

## 1. Lo medido

| hecho | fuente | cifra |
|---|---|---|
| existe un crate Rust **oficial** del esquema ACP | crates.io + repo `agentclientprotocol/agent-client-protocol` | `agent-client-protocol-schema` **1.22.0**, **Apache-2.0**, 149 ficheros en `src/v1/` + `src/v2/` |
| cuántos agentes hablan ACP | registro oficial (matriz del 2026-11-26) | **42 probados · 41 inicializan · los 41 son `protocolVersion: 1`** |
| ACP v2 | el mismo registro | **sólo Devin Desktop**; v2 no está en la matriz |
| `sessionStop` | el mismo registro | **0/41** ⇒ no es mínimo común |
| soporte por método | el mismo registro | `loadSession` 39/41 · `sessionList` 35/41 · `sessionResume` 25/41 · `sessionFork` 18/41 · `setModel` 2/41 |
| dialectos comparados | `docs/RE_ACP_COMPARADO.md` (431 L) | Devin (v1+v2), acpira (subconjunto mínimo), MiniMax (SDK oficial) |

## 2. La decisión

1. **Depender del crate canónico**, no escribir los tipos a mano:
   ```toml
   agent-client-protocol-schema = { version = "1.22", default-features = false }
   ```
   * `default-features = false` deja fuera `schemars` (sólo `JsonSchema`).
   * `unstable_protocol_v2` es **opt-in** y **no** se activa ahora: v2 no lo
     habla nadie más que Devin.
   * Ahorro medido en el trabajo: `protocol.rs` pasa de **2.200-3.150** líneas
     a **1.500-2.200** (los tipos canónicos vienen hechos).
2. **Nuestro dialecto = ACP v1 + extensiones `mdaemon/`**:
   `mdaemon/policy/evaluate`, `mdaemon/sandbox/describe`,
   `mdaemon/board/publish`, `mdaemon/automation/trigger`, `mdaemon/re/status`
   (`rust-core/src/protocol_acp.rs`).
3. **Normalizar en el borde**: los dialects medidos divergen en el formato
   (`acpira` manda `usage_update` como `{used,size}` en vez del `Usage`
   canónico; `acpira` mete los modos en `_meta.acpira/availableModes`;
   MiniMax usa `session/steer`, `session/queue/*`, `session/goal/*`,
   `session/delegation/*`, `session/activate`).
4. **Convergencias de industria ⇒ implementarlas una vez**: `acpira/rewind` ≈
   `cognition.ai/revert` (deshacer sin borrar) · `acpira/sidekick` ≈ Fusion ·
   `mcode/session/delegation` ≈ subagentes.

## 3. Módulos de `protocol.rs` (plan del worker de RE, §7)

| módulo | qué | estado (2026-11-27) |
|---|---|---|
| `transport.rs` | JSON-RPC 2.0 sobre stdio y sobre HTTP (nuestro `cockpit_server.rs` ya lo tiene) | **hecho**: `rust-core/src/protocol_acp/transport.rs` (11 tests) |
| `v1.rs` | reexport de los tipos canónicos | **hecho**: es `rust-core/src/acp_schema.rs` (vendorizado, 9 tests) |
| `session.rs` | ciclo de vida de sesión (nuevo\|load\|resume\|cancel\|list) | **hecho**: `rust-core/src/protocol_acp/sesion.rs` (10 tests) + `session/fork` |
| `updates.rs` | los `SessionUpdate` (13 de Devin ⊂ 22 canónicos) | **hecho**: `SessionUpdate` está en el esquema vendorizado; falta el *dispatch* de la notificación |
| `extensions.rs` | nuestras extensiones `mdaemon/*` y las de terceros que nos sirvan | **hecho**: `protocol_acp.rs` (extensiones propias con namespace `mdaemon/`) |
| `permissions.rs` | `policy/evaluate` con **deny > ask > allow** y el sandbox | **hecho**: `rust-core/src/protocol_acp/permisos.rs` (7 tests) |

### 3.1 Lo que hay y cómo se usa

* CLI: `megadaemon-core acp-transport <parsear|enmarcar|ids>`,
  `megadaemon-core acp-sesion <nueva|listar|cancelar|reanudar|fork|bosque>`,
  `megadaemon-core acp-permisos <evaluar|describir>`.
* Tool MCP: `acp_transport`, `acp_sesion`, `acp_permisos`.

**MEDIDO (2026-11-27)**: los tres módulos se **tiparon y sus tests pasaron** en
un crate aislado (sólo `serde`/`serde_json`) que incluye el
`acp_schema.rs` y el `sandbox_linux.rs` REALES; los brazos de dispatch MCP
se copiaron **verbatim** al mismo crate y se ejecutaron.  El crate completo
se valida en el CI.

### 3.2 Lo que NO está (y por qué)

* **Persistencia de sesiones**: el registro es **global del proceso** (en
  memoria).  ACP no dice dónde guarda el cliente el historial (lo trae con
  `session/load`); persistirlo en SQLite (`board.rs`) es otro módulo.
* **`session/update` (notificación)**: el TIPO está en el esquema vendorizado,
  pero el *dispatch* (emitir updates hacia el cliente) no está cableado al
  bucle de `agent_loop.rs`.
* **ACP v2**: no se activa (`unstable_protocol_v2`): sólo Devin Desktop lo
  habla y el registro oficial no lo prueba (medido, tabla de la §1).

## 4. Lo que NO se copia

Las extensiones de terceros (`cognition.ai/*` de Devin, `mcode/*` de MiniMax,
`acpira/*`) quedan **documentadas**, no copiadas: son de年产 productos
proprietarios. Del crate ACP sí dependemos (Apache-2.0).

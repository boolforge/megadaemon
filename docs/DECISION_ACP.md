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

| módulo | qué |
|---|---|
| `transport.rs` | JSON-RPC 2.0 sobre stdio y sobre HTTP (nuestro `cockpit_server.rs` ya lo tiene) |
| `v1.rs` | reexport de los tipos canónicos |
| `session.rs` | ciclo de vida de sesión (nuevo|load|resume|cancel|list) |
| `updates.rs` | los `SessionUpdate` (13 de Devin ⊂ 22 canónicos) |
| `extensions.rs` | nuestras extensiones `mdaemon/*` y las de terceros que nos sirvan |
| `permissions.rs` | `policy/evaluate` con **deny > ask > allow** y el sandbox |

## 4. Lo que NO se copia

Las extensiones de terceros (`cognition.ai/*` de Devin, `mcode/*` de MiniMax,
`acpira/*`) quedan **documentadas**, no copiadas: son de年产 productos
proprietarios. Del crate ACP sí dependemos (Apache-2.0).

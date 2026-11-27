# ACP vendorizado: por qué el esquema canónico vive dentro del repo

**Fecha de la medición:** 2026-11-27 · **Decisión:** opción A (vendorizar).

## 1. El problema

ACP (Agent Client Protocol) es el estándar de facto para que un editor hable
con un agente. Su esquema canónico está en el crate
`agent-client-protocol-schema`, del repo `agentclientprotocol/agent-client-protocol`.

Medido ese día:

| dato | valor | cómo se midió |
|---|---:|---|
| versión del crate | 1.22.0 | `static.crates.io` |
| sha256 del `.crate` | `650e76da…f1f2d91` | `sha256sum` del fichero descargado (283.771 B) |
| líneas de `src/` | **54.732** | `wc -l` sobre los 26 ficheros del crate |
| `edition` / `rust-version` | **2024** / 1.88.0 | `Cargo.toml` del crate |
| dependencias | **8** | `anyhow`, `derive_more`, `indexmap`, `serde_with`, `strum`, `uuid`, `schemars` (+`serde`/`serde_json`) |
| ficheros de `src/v1/` | 13 (+ `src/v2/`) | listado del `.crate` |

El crate es **Apache-2.0**, luego no hay problema de licencia. El problema es
de **build**:

* MegaDaemon vive en **edition 2021**; el crate exige 2024.
* Añadirlo a `rust-core/Cargo.toml` mete **8 dependencias nuevas** en la cadena
  de build de los 4 jobs. Si el registro no lo sirve, o si alguna de esas
  dependencias no compila con el toolchain del CI, **se caen los 4 jobs**.
* Nuestro módulo `protocol_acp.rs` **no usaba ni un solo tipo** del crate:
  sólo un `pub use`. Es decir, la dependencia pagaba un riesgo de build
  **a cambio de nada**.

## 2. La decisión

**Vendorizar** el subconjunto del esquema que MegaDaemon necesita, dentro de
`rust-core/src/acp_schema.rs`, con **cero dependencias nuevas**.

| | dependencia | vendorizado |
|---|---|---|
| dependencias nuevas | 8 | **0** (`serde` ya estaba) |
| edition | exige 2024 | 2021 (la del crate) |
| líneas que entran | 54.732 | **~1.000** (núcleo estable de v1) |
| auditable en el repo | no (código de terceros en el registro) | **sí** |
| riesgo de build | 4 jobs | ninguno |

## 3. Qué se vendoriza y qué no

**Sí** (núcleo estable de v1, fiel al upstream en nombres de campo y etiquetas
`serde`): `ContentBlock` y sus variantes, `ToolCall`/`ToolCallUpdate`/
`ToolKind`/`ToolCallStatus`/`ToolCallContent`/`ToolCallLocation`,
`SessionUpdate` (variantes estables), `SessionId`, `SessionMode`,
`StopReason`, `McpServer`, `AgentCapabilities`, `ClientCapabilities`,
`PromptCapabilities`, `InitializeRequest`/`Response`, `NewSessionRequest`/
`Response`, `PromptRequest`/`Response`, `Error`/`ErrorCode`, `RequestId`,
`Meta`.

**No** (a propósito, y por qué):

| lo que no entra | motivo |
|---|---|
| variantes `unstable_*` (`PromptSuggestionsUpdate`, `CursorUpdate`, …) | el crate las marca como no estables y el registro oficial **no las prueba** |
| capacidades nuevas (`session.notices`, `session.compaction`, `subagents`) | no están en la matriz medida del registro |
| `elicitation`, `plan`, `proxy`, `mcp` completos | 3.000-7.000 líneas que todavía no hablamos |
| `schemars` (`JsonSchema`) | el crate lo trae como `default`; MegaDaemon genera su propio JSON |

Las adaptaciones (fieles, pero sin las dependencias del upstream) están
listadas en `docs/ACP_VENDOR_LICENSE.md`.

## 4. El esquema completo sigue disponible

El `.crate` original **no** se redistribute en el repo: se sube a la Drive
privada con su sha256, para poder regenerar o comparar cuando haga falta.
Cómo recuperarlo:

```bash
curl -sL -o acp-schema.crate \
  https://static.crates.io/crates/agent-client-protocol-schema/agent-client-protocol-schema-1.22.0.crate
echo "650e76dae0c8d3e772acb35184e6dd4defa0e15e130c96bf4c4586597f1f2d91  acp-schema.crate" | sha256sum -c -
```

## 5. Qué queda medido por el camino

* **ACP v1 es el estándar de facto**: de los 42 agentes del registro oficial,
  41 inicializan y **los 41 son v1**. `sessionStop` no lo implementa nadie
  (0/41) ⇒ no es requisito para ser interoperable.
* Soporte en el registro: `loadSession` 39/41 · `sessionList` 35/41 ·
  `sessionResume` 25/41 · `sessionFork` 18/41 · `setModel` 2/41.
* 22 de los agentes del registro **no estaban** en nuestro barrido de
  candidatos: ver `docs/CATALOGO_AGENTES_CODIGO.md` §3.

## 6. Cómo se comprueba

```bash
bash tools/check-sintaxis.sh                    # el módulo nuevo tiene que parsear
python3 -m pytest tools/regenerar-go/test_regenerar.py   # (ajeno a este cambio)
```

Los tests del propio módulo (`rust-core/src/acp_schema.rs`, 9 tests) comprueban
que las etiquetas `serde` del JSON coinciden con el upstream: `"type":"text"`,
`"sessionUpdate":"agent_message_chunk"`, `camelCase` en los campos y los
códigos de error JSON-RPC (`-32601`, `-32800`).

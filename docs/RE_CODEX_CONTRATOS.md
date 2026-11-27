# RE — Codex 0.172.0 (OpenAI, build nativo linux-x64)

**Fecha de la medición:** 2026-11-27 · **Todo lo que hay aquí está medido**.
Lo que no se pudo medir va marcado **NO MEDIDO**.

## 1. Qué se descargó (con hashes)

Release `rust-v0.172.0` de `openai/codex` (GitHub):

| asset | bytes | sha256 |
|---|---:|---|
| `codex-x86_64-unknown-linux-musl.tar.gz` | 125.369.335 | `a2342a8f7f68f088be30620ccfe9cb84162c93a6b56177855961cda3ed6be392` |
| `codex-app-server-x86_64-unknown-linux-musl.tar.gz` | 97.143.498 | `9675313ed4bec5e1dcc99a399cb6eee4b7bc269536a36a6a2df207202737c9d7` |
| `codex` (binario extraído) | **321.244.472** | — (el tar lleva su sha256) |
| `codex-app-server` (binario extraído) | **249.083.328** | — |

**Los dos binarios vienen `stripped`** (`readelf -s` ⇒ 0 símbolos `FUNC`).
Para un RE simbolado haría falta el asset `codex-symbols-x86_64-unknown-linux-musl.tar.gz`
(395.260.220 B) ⇒ **NO MEDIDO** (no descargado en esta sesión).

## 2. Subcomandos (medidos en las strings)

| subcomando | apariciones | para qué |
|---|---:|---|
| `codex login` | 13 | autenticación |
| `codex exec` | 11 | modo no interactivo (CI) |
| `codex app-server` | 11 | **el servidor de protocolo** (es el `codex-acp` del registro ACP) |
| `codex resume` | 5 | reanuda una sesión |
| `codex cloud` | 5 | tareas en la nube |
| `codex logout` / `codex debug` | 1 / 1 | — |

## 3. Configuración y estado (medido)

| ruta | apariciones | qué |
|---|---:|---|
| `.codex/config.toml` | 52 | configuración principal |
| `config.toml` | 52 | (nombre suelto) |
| `managed_config.toml` | 4 | configuración administrada por la organización |
| `legacy-managed-config.toml` | 1 | formato viejo, aún soportado |
| `.codex/agents/` | — | agentes registrados |
| `.codex/hooks` | — | **hooks** |
| `.codex/skills` | — | **skills** |
| `instructions.md` | — | instrucciones del repo |

Y dentro de la config aparecen `features.multi_agent_v2.tool_namespace` y
`requirements.toml`: es decir, **Codex tiene multi-agente con namespaced tools**.

## 4. Hooks (medidos) — los MISMOS nombres que Claude Code

`PreToolUse` (4) · `PostToolUse` (4) · `UserPromptSubmit` (2) · `SessionStart` (2)
· `SessionEnd` (2) · `Stop` (15) · `Notification` (48) · `ToolCallStarted` (1) ·
`ToolCallSummary` (8) · `ToolCallKind` (4)

**Convergencia medida**: Codex y Claude Code comparten el vocabulario de hooks.
Para MegaDaemon esto quiere decir que un contrato de hooks común es viable, y
que hay que añadir los de Codex: `ToolCallStarted`, `ToolCallSummary`,
`ToolCallKind` (Codex expone **la clase y el estado de cada llamada a tool**,
que Claude Code no expone).

## 5. Sandbox y permisos (medidos)

| nivel | apariciones | significado |
|---|---:|---|
| `read-only` | 114 | sólo lectura |
| `workspace-write` | 10 | escritura dentro del workspace |
| `danger-full-access` | 6 | acceso total |
| `approval_policy` / `ApprovalPolicy` | 14 / 5 | la política de aprobación |

Es el **mismo esquema de tres niveles** que el modo de permiso de Claude Code
(`default` / `acceptEdits` / `plan` / `bypassPermissions` / `autoMode`), pero
Codex lo llama *sandbox mode* y lo separa de la *approval policy*. MegaDaemon
tiene las dos cosas en un solo objeto (`tool_policy.rs`): este es el argumento
medido para **separarlas** (sandbox = aislamiento, approval = decisión).

## 6. Tools (medidas)

`shell` (168) · `apply_patch` (53) · `exec_command` (26) · `update_plan` (19) ·
`view_image` (6) · `web_search` (5) · `write_file` (1)

El contraste con Claude Code es el dato: **Codex expone `shell` + `apply_patch`
+ `exec_command`**, tres puertas distintas donde Claude Code tiene una sola
(`Bash`) y donde además hay `FileEdit`/`FileWrite`. Para un motor de permisos
esto importa: `apply_patch` y `exec_command` son **operaciones diferentes** y no
deben compartir la misma decisión.

## 7. Variables de entorno (medidas, con la salvedad del punto 8)

| variable | apariciones | para qué |
|---|---:|---|
| `CODEX_HOME` | 59 | raíz de estado y config |
| `CODEX_ACCESS_TOKEN` | 3 | credencial |
| `CODEX_NON_INTERACTIVE` | 2 | modo CI |
| `CODEX_MCP_PROTOCOL_VERSION` | 1 | versión del protocolo MCP |
| `CODEX_OSS_BASE_URL` | 1 | servidor OSS propio |
| `CODEX_TOOL_CALL_ATTRIBUTION_PROXY` | 1 | proxy de atribución de llamadas |
| `CODEX_ROLLOUT_TRACE_ROOT` | 1 | raíz de las trazas de "rollout" |
| `CODEX_SNAPSHOT_SOURCE` / `CODEX_SNAPSHOT_*` | 1 / 1 | snapshots |
| `CODEX_PERMISSION_*` | 1 | prefijo de política |
| `CODEX_SQLITE_HOME` | 3 | base de datos local |
| `CODEX_TUI_ROUNDED` | 1 | ajuste de la TUI |
| `CODEX_MARKETPLACE_HAVE` | 2 | marketplace de plugins |
| `OPENAI_API_KEY` | 18 | credencial de la API |
| `OPENAI_IDENTITY_TOKEN_FILE` | 1 | token por fichero |
| `OPENAI_IDENTITY_TOKEN_SOURCE` | 2 | origen del token |
| `OPENAI_FEDERATION_RULE_ID` | 1 | regla de federación |

## 8. Salvedad honesta sobre las variables

El binario está compilado en Rust y **concatena literales**: en las strings
aparecen secuencias como `CODEX_THH`, `CODEX_VEI`, `OPENAI_AH` que **no** son
variables de entorno sino trozos de cadenas mayores. Por eso la lista de arriba
es **la que se ha podido aislar con nombre propio**, y no un volcado completo.
El recuento bruto de coincidencias con prefijo (`CODEX_`/`OPENAI_` seguidos de
mayúsculas) es 33, pero **no todas son variables reales** ⇒ el listado
completo de variables queda **NO MEDIDO**.

## 9. Lo que NO se midió

* **El RE simbolado con el motor sancionado (Ghidra)**: no importé los binarios
  (321 MB y 249 MB, `stripped`) ⇒ el import+Ghidra de un binario Rust de ese
  tamaño con `MAXMEM=1200M` (el límite que fijó el root) no está medido. Y sin
  los símbolos (asset de 395 MB) el resultado sería de baja calidad.
* **El protocolo del `app-server`**: es el `codex-acp` del registro ACP; su
  matriz está en `artifacts/acp-comparado/protocol_matrix.json`
  (v1, `initialize` OK), pero **el RE fino de sus métodos no está medido**.
* **MiniMax Code y DeepSeek Code**: **NO MEDIDO** (no descargados).

## 10. Cómo reproducir

```bash
B=https://github.com/openai/codex/releases/download/rust-v0.172.0
curl -sL -o codex-musl.tar.gz $B/codex-x86_64-unknown-linux-musl.tar.gz
curl -sL -o codex-app-server-musl.tar.gz $B/codex-app-server-x86_64-unknown-linux-musl.tar.gz
tar xzf codex-musl.tar.gz && tar xzf codex-app-server-musl.tar.gz
strings -n 8 codex-x86_64-unknown-linux-musl > codex-strings.txt
# medidas: CODEX_HOME|CODEX_ACCESS_TOKEN|... , sandbox, tools, hooks
```

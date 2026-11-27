# CONTRATO 1 — MANIFIESTO DE EXTENSIONES Y PUNTOS DE ENTRADA

Fuente: `usr/share/cursor/resources/app/extensions/*/package.json`.
Todo lo listado es **medido** leyendo los manifiestos reales del paquete.

## 1.0 Nota previa sobre el formato del paquete

**No hay `app.asar`.** En Cursor 3.32.4 el renderer **no** está empaquetado en un asar:
`usr/share/cursor/resources/app/` es un **directorio** con `out/`, `extensions/`, `node_modules/`,
`molten/` y `product.json`. El único `.asar` es `node_modules.asar` (28 B, un marcador vacío).

→ **No hay nada que desempaquetar.** Todo el análisis es lectura directa de ficheros.

## 1.1 Las 22 extensiones propias de Anysphere/Cursor

| # | Extensión | publisher | ver | `main` | `browser` | extensionKind | `contributes` | cmds | tamaño |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `cursor-agent-exec` | anysphere | 0.0.1 | `./dist/main` | `—` | — | — | 0 | 32.9 MB |
| 2 | `cursor-agent-host` | anysphere | 0.0.1 | `./dist/main.js` | `—` | — | commands, configuration | 2 | 56.0 MB |
| 3 | `cursor-agent-worker` | anysphere | 0.0.1 | `./dist/main` | `—` | — | — | 0 | 2.0 MB |
| 4 | `cursor-always-local` | anysphere | 0.0.1 | `./dist/main` | `—` | ['ui'] | commands, keybindings, menus, jsonValidation, configuration | 0 | 4.6 MB |
| 5 | `cursor-browser-automation` | cursor | 1.0.0 | `./dist/extension.js` | `—` | ['ui'] | — | 0 | 0.6 MB |
| 6 | `cursor-checkout` | anysphere | 0.0.1 | `./dist/main` | `—` | — | — | 0 | 0.0 MB |
| 7 | `cursor-commits` | anysphere | 0.0.1 | `./dist/main` | `—` | ['workspace'] | commands, keybindings, menus | 0 | 0.5 MB |
| 8 | `cursor-computer-use` | cursor | 1.0.0 | `./dist/extension.js` | `—` | ['ui'] | — | 0 | 2.1 MB |
| 9 | `cursor-deeplink` | anysphere | 0.0.1 | `./dist/main` | `—` | ['ui'] | commands | 1 | 0.3 MB |
| 10 | `cursor-explorer` | anysphere | 0.0.1 | `./dist/main` | `—` | ['workspace'] | — | 0 | 0.0 MB |
| 11 | `cursor-file-service` | anysphere | 1.0.0 | `—` | `—` | — | — | 0 | 0.0 MB |
| 12 | `cursor-local-agent-runtime` | anysphere | 0.0.1 | `./dist/main` | `—` | ['ui'] | — | 0 | 7.8 MB |
| 13 | `cursor-mcp` | anysphere | 0.0.1 | `./dist/main` | `—` | ['workspace'] | commands, keybindings, menus, configuration | 0 | 1.5 MB |
| 14 | `cursor-ndjson-ingest` | anysphere | 0.0.1 | `./dist/main` | `—` | ['workspace'] | commands, configuration | 5 | 0.0 MB |
| 15 | `cursor-polyfills-remote` | anysphere | 0.0.1 | `./dist/main` | `—` | ['workspace'] | — | 0 | 0.1 MB |
| 16 | `cursor-resolver` | anysphere | 0.0.1 | `./dist/main` | `./dist/browser/main` | ['ui'] | resourceLabelFormatters | 0 | 0.4 MB |
| 17 | `cursor-resolver-helper` | anysphere | 0.0.1 | `./dist/main` | `—` | ['ui'] | — | 0 | 0.9 MB |
| 18 | `cursor-retrieval` | anysphere | 0.0.1 | `./dist/main` | `—` | ['workspace'] | commands, keybindings, menus, configuration, languages | 3 | 44.5 MB |
| 19 | `cursor-shadow-workspace` | anysphere | 1.0.0 | `./dist/extension` | `./dist/browser/extension` | ['workspace'] | commands, configuration | 0 | 0.4 MB |
| 20 | `cursor-socket` | anysphere | 0.0.1 | `./dist/main` | `—` | ['ui'] | — | 0 | 0.0 MB |
| 21 | `cursor-worktree-textmate` | everysphere | 0.0.1 | `—` | `—` | ['ui'] | languages, grammars, configurationDefaults | 0 | 0.0 MB |
| 22 | `protobuf` | anysphere | 0.0.1 | `—` | `—` | ['ui'] | languages, grammars | 0 | 0.0 MB |

## 1.2 Descripción literal de cada una

### `cursor-agent-exec`

- `name`: `cursor-agent-exec` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Provides agent execution capabilities for {productName}, enabling agents to run commands, interact with files,
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `—`
- `contributes`: —
- peso en disco: 32.9 MB

### `cursor-agent-host`

- `name`: `cursor-agent-host` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Hosts {productName} agent orchestration in the AgentExec extension host
- `main`: `./dist/main.js` · `browser`: `—` · `extensionKind`: `—`
- `contributes`: `commands`, `configuration`
- peso en disco: 56.0 MB

### `cursor-agent-worker`

- `name`: `cursor-agent-worker` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Install and run {productName} agent worker from extension startup
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `—`
- `contributes`: —
- peso en disco: 2.0 MB

### `cursor-always-local`

- `name`: `cursor-always-local` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Implements experimentation features for {productName}
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['ui']`
- `contributes`: `commands`, `keybindings`, `menus`, `jsonValidation`, `configuration`
- peso en disco: 4.6 MB

### `cursor-browser-automation`

- `name`: `cursor-browser-automation` · `version`: `1.0.0` · `publisher`: `cursor`
- `description`: MCP server for browser automation in {productName}
- `main`: `./dist/extension.js` · `browser`: `—` · `extensionKind`: `['ui']`
- `contributes`: —
- peso en disco: 0.6 MB

### `cursor-checkout`

- `name`: `cursor-checkout` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Checkout provider for branch migration operations
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `—`
- `contributes`: —
- peso en disco: 0.0 MB

### `cursor-commits`

- `name`: `cursor-commits` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Tracks requests and commits for {productName} online metrics
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['workspace']`
- `contributes`: `commands`, `keybindings`, `menus`
- peso en disco: 0.5 MB

### `cursor-computer-use`

- `name`: `cursor-computer-use` · `version`: `1.0.0` · `publisher`: `cursor`
- `description`: Computer use in Cursor
- `main`: `./dist/extension.js` · `browser`: `—` · `extensionKind`: `['ui']`
- `contributes`: —
- peso en disco: 2.1 MB

### `cursor-deeplink`

- `name`: `cursor-deeplink` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Handles deep-link URIs.
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['ui']`
- `contributes`: `commands`
- peso en disco: 0.3 MB

### `cursor-explorer`

- `name`: `cursor-explorer` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Workspace extension for {productName} Explorer
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['workspace']`
- `contributes`: —
- peso en disco: 0.0 MB

### `cursor-file-service`

- `name`: `cursor-file-service` · `version`: `1.0.0` · `publisher`: `anysphere`
- `description`: Handles indexing and retrieval for {productName}
- `main`: `—` · `browser`: `—` · `extensionKind`: `—`
- `contributes`: —
- peso en disco: 0.0 MB

### `cursor-local-agent-runtime`

- `name`: `cursor-local-agent-runtime` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Hosts Cursor Private Inference outside workspace extension hosts
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['ui']`
- `contributes`: —
- peso en disco: 7.8 MB

### `cursor-mcp`

- `name`: `cursor-mcp` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Handles MCP for {productName}
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['workspace']`
- `contributes`: `commands`, `keybindings`, `menus`, `configuration`
- peso en disco: 1.5 MB

### `cursor-ndjson-ingest`

- `name`: `cursor-ndjson-ingest` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: HTTP server for ingesting NDJSON logs to workspace/.cursor/debug.log
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['workspace']`
- `contributes`: `commands`, `configuration`
- peso en disco: 0.0 MB

### `cursor-polyfills-remote`

- `name`: `cursor-polyfills-remote` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Polyfills for workspace extension host
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['workspace']`
- `contributes`: —
- peso en disco: 0.1 MB

### `cursor-resolver`

- `name`: `cursor-resolver` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Background composer remote authority resolver for {productName}
- `main`: `./dist/main` · `browser`: `./dist/browser/main` · `extensionKind`: `['ui']`
- `contributes`: `resourceLabelFormatters`
- peso en disco: 0.4 MB

### `cursor-resolver-helper`

- `name`: `cursor-resolver-helper` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Connection token provider for {productName} resolver
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['ui']`
- `contributes`: —
- peso en disco: 0.9 MB

### `cursor-retrieval`

- `name`: `cursor-retrieval` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: Handles indexing and retrieval for {productName}
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['workspace']`
- `contributes`: `commands`, `keybindings`, `menus`, `configuration`, `languages`
- peso en disco: 44.5 MB

### `cursor-shadow-workspace`

- `name`: `cursor-shadow-workspace` · `version`: `1.0.0` · `publisher`: `anysphere`
- `description`: %description%
- `main`: `./dist/extension` · `browser`: `./dist/browser/extension` · `extensionKind`: `['workspace']`
- `contributes`: `commands`, `configuration`
- peso en disco: 0.4 MB

### `cursor-socket`

- `name`: `cursor-socket` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: TCP/TLS socket provider for {productName} extensions
- `main`: `./dist/main` · `browser`: `—` · `extensionKind`: `['ui']`
- `contributes`: —
- peso en disco: 0.0 MB

### `cursor-worktree-textmate`

- `name`: `worktree-textmate` · `version`: `0.0.1` · `publisher`: `everysphere`
- `description`: Provides TextMate-only syntax highlighting for .cursor/worktrees files without activating language servers.
- `main`: `—` · `browser`: `—` · `extensionKind`: `['ui']`
- `contributes`: `languages`, `grammars`, `configurationDefaults`
- peso en disco: 0.0 MB

### `protobuf`

- `name`: `cursor-protobuf` · `version`: `0.0.1` · `publisher`: `anysphere`
- `description`: %description%
- `main`: `—` · `browser`: `—` · `extensionKind`: `['ui']`
- `contributes`: `languages`, `grammars`
- peso en disco: 0.0 MB


## 1.3 Punto de entrada del producto (lo que NO es una extensión)

| Fichero | Ruta | Tamaño | Qué es |
|---|---|---|---|
| Bundle del workbench (renderer) | `out/vs/workbench/workbench.desktop.main.js` | 32,4 MB | bundle principal, 20 548 líneas muy largas |
| Bundle "glass" | `out/vs/workbench/workbench.glass.main.js` | 40,0 MB | segunda build del workbench |
| Bundle "anysphere-ui" | `out/vs/workbench/workbench.anysphere-ui.js` | 9,1 MB | UI de Anysphere |
| Bundle "xai-commerce" | `out/vs/workbench/workbench.xai-commerce-prepaid.js` | 1,6 MB | UI de comercio xAI |
| Main de Electron | `out/main.js` | 3,9 MB | proceso principal |
| Proceso MCP | `out/vs/code/electron-utility/mcpProcess/mcpProcessMain.js` | 858 KB | **proceso separado para MCP** |
| CLI de VS Code | `out/cli.js` | 225 KB | CLI `code`-like |
| App "molten" (Canvas/Grok) | `molten/dist/` | — | bundle propio con **código fuente legible** (`.cjs` con TSX embebido) |
| `product.json` | `product.json` | 56,4 KB | identidad, URLs, update service, flags |
| `package.json` | `package.json` | 455 B | metadatos del producto |

### Binarios sueltos (`usr/share/cursor/bin/`)

| Binario | Tamaño | Nota |
|---|---|---|
| `cursor` | 6 105 B | script shell lanzador |
| `cursor-tunnel` | 19 471 864 B | **tunnel** (idéntico en bytes a `code-tunnel`) |
| `code-tunnel` | 19 471 864 B | mismo binario |

**NO existe `cursor-agent` en el paquete.** `usr/bin/` no está (todo cuelga de `usr/share/`).

## 1.4 Valoración para MegaDaemon

| Aspecto | Valoración | Motivo |
|---|---|---|
| Separar el runtime MCP en un **proceso propio** (`mcpProcessMain.js`) | **COPIABLE tal cual** | Aísla los servidores MCP (que son código de terceros) del proceso principal. Es la misma política que un `sandbox` para tools. |
| `extensionKind: ["ui"]` vs `["workspace"]` | **ADAPTABLE** | El modelo de dos mundos (UI vs workspace) de VS Code no aplica a MegaDaemon, pero el principio "el código de tools va en el host sin UI" sí. |
| `cursor-browser-automation` y `cursor-computer-use` como **MCP servers de Cursor** | **ADAPTABLE** | La idea de que el producto ofrezca sus propias capacidades **como servidores MCP** es excelente: MegaDaemon puede exponer sus tools igual. |
| `cursor-ndjson-ingest` (*"HTTP server for ingesting NDJSON logs to workspace/.cursor/debug.log"*) | **COPIABLE tal cual** | Un endpoint HTTP de ingest NDJSON para trazas de agente es un patrón útil ytrivial. |
| `cursor-socket` (*"TCP/TLS socket provider"*) y `cursor-resolver` | **ADAPTABLE** | Transporte para workers remotos; relevante para el enjambre de MegaDaemon. |
| Manifiesto en sí (`package.json` de extensión) | **NO COPIABLE** | Es el formato de VS Code; MegaDaemon tiene su propio registro de plugins (`plugins/` en el repo). |

**Destino en MegaDaemon**: `rust-core/src/agent_adapters.rs` (registro de adaptadores),
`rust-core/src/mcp_server.rs` (proceso MCP aislado), `plugins/` (catálogo de capacidades).

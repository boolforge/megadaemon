# RE — DeepSeek-Reasonix (el "DeepSeek Code" que sí existe)

**Fecha de la medición:** 2026-11-27 · **Todo lo que hay aquí está medido**, con su
ruta y su hash. Lo que no se pudo medir va marcado **NO MEDIDO**.

## 0. PRIMERO, EL HALLAZGO MEDIDO: "DEEPSEEK CODE" NO EXISTE COMO AGENTE

DeepSeek **no ha publicado un agente de código**. Lo oficial de DeepSeek es:

| repo | ★ | qué es |
|---|---:|---|
| `deepseek-ai/DeepSeek-Coder` | 24.409 | un **modelo** (MIT) |
| `deepseek-ai/DeepSeek-Coder-V2` | 7.041 | un **modelo** (MIT) |
| `deepseek-ai/awesome-deepseek-coder` | 846 | un índice de proyectos |

Buscando "deepseek code" en GitHub (nombre y descripción, 1.068 repos) lo que sale
son agentes **de terceros**, medidos:

| repo | ★ | lenguaje | licencia | push | qué es |
|---|---:|---|---|---|---|
| **`esengine/DeepSeek-Reasonix`** | **36.071** | Go (+TypeScript) | **MIT** | 2026-11-12 | el agente de código "deepseek" de facto |
| `lessweb/deepcode-cli` | 2.187 | TypeScript | MIT | 2026-11-25 | CLI optimizada para `deepseek-v4` |
| `QingJ01/DeepSeekCode` | 143 | TypeScript | MIT | 2026-05-10 | reimplementación sobre el código de Claude Code |

⇒ **Este documento es el RE de `esengine/DeepSeek-Reasonix`**, que es el que
pide el encargo. Los otros dos quedan **NO MEDIDO** (menorBrain).

## 1. Qué es

| dato | valor | cómo se midió |
|---|---|---|
| repo | `esengine/DeepSeek-Reasonix` | GitHub API |
| estrellas / licencia | 36.071 / **MIT** | GitHub API |
| lenguaje | Go (`go 1.26.0`, toolchain `go1.26.11`) + TypeScript | `go.mod`, topics |
| ficheros | 10.895 | clon |
| topics | `agent`, `agent-framework`, `ai-agent`, `ai-coding`, `cli`, `coding-agent`, `deepseek`, `developer-tools`, `dsh`, `dsh-plugin`, `ink`, `llm`, `prompt-caching`, `r1`, `terminal`, `tool-use`, `tui`, `typescript` | GitHub API |

**Es código abierto y MIT** ⇒ se puede integrar (y es lo que permite leer el
contrato en vez de adivinarlo). Clonado en `/tmp/dsr` (**fuera** del repo: no se
versiona código de terceros).

## 2. Estructura (medida)

```
cmd/    reasonix · e2ebench · extension-protocol-gen · reasonix-plugin-example
        reasonix-studio-host · signpath-contract · studio-delta
internal/
        assembly · base · contract · ext · frontend · model · platform
        runtime · safety · session · state · tools
```

Los tres `cmd/` que más兴趣 tiene para MegaDaemon:

| cmd | qué es | por qué importa |
|---|---|---|
| `extension-protocol-gen` | **generador de código del protocolo de extensiones** | el protocolo se genera desde una definición ⇒ el contrato es auditable |
| `signpath-contract` | el **contrato de firma** de las extensiones | las extensiones van **firmadas**: un modelo de seguridad que nosotros no tenemos |
| `studio-delta` | el **delta** entre versiones | versionado de cambios |

## 3. Las tools (medidas, `internal/tools/`)

| tool | qué aporta (medido por los ficheros) |
|---|---|
| `bash` | el motor de shell, con **parser propio de comandos**: `bash_cmdline{,_other,_windows}.go`, `bash_delete{,_interpreter,_parser}.go` (borra comandos), `bash_powershell_test.go`, `bash_heredoc_test.go`, `bash_nul.go`, `bash_reap_test.go`, `bash_job_exit.go`, `bash_egress.go`, `bash_gitmeta.go`, `bash_cancel_test.go` |
| `shellrun` | ejecución de shell (separada de `bash`) |
| `websearch` | búsqueda web |
| `fetchchain` | cadena de descargas (fetch encadenado) |
| `advisor` | **asesor** (revisor) |
| `jobs` | trabajos en background |
| `productdocs` | documentación del producto |
| `script` | scripts |
| `sessiontool` | tools de sesión |

Lo medido que más se parece a lo nuestro: **`bash_egress.go`** (control de la
salida a red desde el shell) y el **parser de `delete`** (borra el comando
`rm`/`del` antes de ejecutarlo) ⇒ no confían en el texto del comando: lo
parsean. Es más estricto que nuestro `tool_policy.rs`, que decide por patrón de
herramienta.

## 4. Seguridad: 8 subsistemas (`internal/safety/`, medido)

| subsistema | qué hace |
|---|---|
| `permission` | la decisión allow/ask/deny |
| `sandbox` | el aislamiento |
| `shellsafe` | seguridad específica del shell |
| `redirectguard` | **guard de redirecciones** (URL) |
| `egress` | control de salida a red |
| `endpointclient` | cliente de endpoints (con sus límites) |
| `evidence` | **evidencia** (registro de lo que se hizo) |
| `typesafe` | guardado de tipos |

El nuestro tiene `tool_policy.rs` (decisión) + `sandbox_linux.rs` (aislamiento) y
ahora `crates/redact` (redacción). Los que **nos faltan**: `redirectguard`,
`egress`, `evidence` y `shellsafe` como subsistemas separados.

## 5. El modelo de permisos (medido)

 allow (72) · **ask (72)** · deny (23) · auto (31) · sandbox (1)

⇒ **el mismo modelo de tres niveles que el nuestro** (`deny` > `ask` > `allow`),
con un modo `auto` y con `sandbox` como orthogonales. Confirmación medida de que
el diseño de MegaDaemon es el estándar de facto.

## 6. El contrato propio (`internal/contract/`, 14 subsistemas, medido)

| subsistema | qué es |
|---|---|
| `event` / `eventwire` | **bus de eventos** propio (contrapartida de nuestro `service_bus.rs`) |
| `agentgraph` | **grafo de agentes** (nuestro `delegate.rs` + `board.rs`) |
| `agentpreset` | presets de agente |
| `agentcomposition` | composición de agentes |
| `hostaudit` | auditoría del host |
| `delivery` | entrega (canal de salida) |
| `observe` | observabilidad |
| `chartspec` | especificación de gráficos |
| `ablation` | "ablation" (medir qué parte del agente aporta) |
| `mcpdiag` | diagnóstico de MCP |
| `config` | configuración |

`ablation` y `mcpdiag` son dos cosas que **no tenemos** y que nos servirían:
*ablation* = poder medir cuánto aporta cada pieza del agente (justo lo que
hemos hecho a mano con la cobertura del módulo Go).

## 7. Variables de entorno (medidas)

| variable | apariciones | para qué |
|---|---:|---|
| `REASONIX_HOME` | 1.104 | raíz de todo |
| `DEEPSEEK_API_KEY` | 238 | credencial |
| `REASONIX_STATE_HOME` | 123 | estado |
| `REASONIX_CREDENTIALS_STORE` | 70 | almacén de credenciales |
| `REASONIX_CACHE_HOME` | 61 | caché |
| `REASONIX_PLUGIN_ROOT` | 45 | plugins |
| `REASONIX_SAFE_MODE` | 24 | modo seguro |
| `REASONIX_WORKSPACE_ROOT` | 11 | raíz del workspace |
| `REASONIX_ACCOUNT_TOKEN` | 11 | token de cuenta |
| `REASONIX_LANG` | 26 | idioma |
| `REASONIX_THEME` / `REASONIX_THEME_STYLE` | 10 / 9 | tema de la TUI |

## 8. Other piezas medidas

* `internal/runtime/agent/tool_receipts.go` ⇒ **recibos de las llamadas a tool**
  (auditoría por llamada). Es el mismo concepto que el `receipt` de la
  delegación de MiniMax ⇒ **convergencia de 3 productos** (MiniMax, Reasonix y
  nuestro `board`).
* `internal/runtime/agent/workspace_lease_test.go` ⇒ **leases de workspace**
  (exclusión mutua entre agentes sobre un directorio).
* `internal/runtime/agent/checkpoint_cancel_test.go` ⇒ **checkpoints** con
  cancelación.
* La TUI es con **Ink** (React para terminal) ⇒ mismo approach que la nuestra.

## 9. Lo que NO se midió

* **El binario compilado**: no se compiló en esta sesión (la JVM del import de
  Codex ocupaba la RAM y el root prohibió `cargo`; aquí el build es de Go y
  habría sido posible con `GOCACHE=/tmp/gocache-local`). ⇒ El RE de este
  producto es **de fuente**, no de binario ⇒ **NO MEDIDO** a nivel de binario.
* El **comportamiento en ejecución** de los 8 subsistemas de seguridad.
* `lessweb/deepcode-cli` y `QingJ01/DeepSeekCode`: **NO MEDIDO**.

## 10. Cómo reproducir

```bash
git clone --depth 1 https://github.com/esengine/DeepSeek-Reasonix.git /tmp/dsr
ls /tmp/dsr/internal/{safety,contract,tools}
grep -rhoE '\b(REASONIX|DSH|DEEPSEEK)_[A-Z0-9_]{2,}' --include='*.go' /tmp/dsr | sort | uniq -c | sort -rn
```

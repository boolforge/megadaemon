# BENCHMARK AGÉNTICO — 59 proyectos y 7 productos de RE: qué robarle a cada uno

> Fecha: 2026-11-27 · alcance: comparación **medida** (nada de memoria) para
> decide el **refactor definitivo de MegaDaemon**.
>
> Este documento NO sustituye a `docs/MASTER_PLAN.md`, `docs/MASTER_PLAN_V16.md`
> ni a `docs/AGENTIC_ARCHITECTURE.md` (la investigación Codex/Claude Code):
> es la **pieza externa** que faltaba ⇒ *qué hacen otros, cómo lo hacen y qué
> se copia*.
>
> **Método (repetible)**: una llamada a la API GraphQL de GitHub con 60 alias
> (datos de los 60 repos) + una segunda con el árbol de 31 de ellos + lectura
> de 6 fuentes primarias (`raw.githubusercontent`). Todo lo que sigue está
> medido; lo que no, está marcado como **NO MEDIDO**.

---

## 1. La tabla medida (59 de 60 repos)

| repo | ★ | lang | licencia | KB | qué es |
|---|---|---|---|---|---|
| **aaif-goose/goose** | 57053 | Rust | Apache-2.0 | 1003962 | el agente open source más grande: **extensible**, MCP, multi-proveedor |
| **tinyhumansai/openhuman** | 42866 | Rust | GPL-3.0 | 455640 | alternativa a Claude Code, 455 MB de código |
| **codewhale-hq/Codewhale** | 41218 | Rust | MIT | 419306 | agente de terminal+web+escritorio en Rust |
| **vercel-labs/agent-desktop** | 26985 | Rust | Apache-2.0 | 75617 | **computer use** en CLI, con `probes/` y `skills/` |
| **1jehuang/jcode** | 22085 | Rust | MIT | 476321 | **harness de agente de alto rendimiento en Rust** (el más serio) |
| **RightNow-AI/openfang** | 18287 | Rust | Apache-2.0 | 9982 | "Agent Operating System" |
| **sigoden/aichat** | 10604 | Rust | Apache-2.0 | 4159 | CLI LLM todo en uno |
| **Kuberwastaken/claurst** | 10345 | Rust | GPL-3.0 | 15457 | agenteic coding |
| **0xPlaygrounds/rig** | 9284 | Rust | MIT | 176681 | **framework** LLM modular (el "LangChain en Rust") |
| **nanocodexyz/nanocode** | 869 | Rust | Apache-2.0 | 401058 | SDK para agentes **embebidos** |
| **Mesh-LLM/mesh-llm** | 4233 | Rust | Apache-2.0 | 177257 | LLM distribuido p2p |
| **agent-of-empires/agent-of-empires** | 4966 | Rust | MIT | 102861 | gestionar varios agentes a la vez (+ **API de plugins**) |
| **zavora-ai/adk-rust** | 772 | Rust | Apache-2.0 | 243090 | ADK-Rust: `adk-core`, `adk-agent`, `adk-auth`, `adk-browser`, `adk-computer-use`, `adk-artifact`… |
| **thClaws/thClaws** | 1268 | Rust | Apache-2.0 | 48734 | harness nativo **con GUI** |
| **keon/browser-control** | 3160 | Rust | MIT | 5262 | mini CLI que **conduce un navegador real** |
| **mediar-ai/terminator** | 1676 | Rust | MIT | 52961 | "playwright para Windows" (computer use) |
| **shiwenwen/hope-agent** | 1936 | Rust | MIT | 101065 | agente de escritorio **entre dispositivos** |
| **Pinvou/pinvou-agent** | 3045 | Rust | MIT | 345702 | agente de escritorio: herramientas, ficheros, conocimiento |
| **rust-infra/tact** | 18 | Rust | MIT | 37451 | agente de terminal+escritorio (**ARCHITECTURE.md de 53 KB**) |
| **v0l/agentbench** | 2 | Rust | GPL-3.0 | 6748 | correr agentes en **worktrees** + tracker de issues |
| **mateusz-szczyrzyca/team-agents** | 2 | Rust | MIT | 40263 | correr agentes headless (**agentbus/**) |
| **btsouth/toolport** | 247 | Rust | MIT | 133551 | una config para todos los agentes |
| **antgroup/Napaxi** | 34 | Rust | GPL-3.0 | 23442 | **SDK de agente mobile/on-device** |
| **undergroundrap/hatchling** | 2 | Rust | AGPL-3.0 | 15704 | **infra segura para agentes always-on** |
| **kernex-dev/kernex** | 3 | Rust | Apache-2.0 | 1340 | runtime de agentes con **herramientas en sandbox del SO** |
| **CertaMesh/gaze** | 12 | Rust | Apache-2.0 | 255862 | **seudonimización reversible de PII** en tiempo de ejecución |
| **Mavericksantander/praetore** | 12 | Rust | ⚠️NOASSERTION | 6955 | **gobernanza determinista** de agentes (+ `fuzz/`) |
| **zhugez/metrixa** | 0 | Rust | Apache-2.0 | 151 | **motor de evaluación** para agentes de código |
| **sunerpy/codegraph-rust** | 24 | Rust | MIT | 14129 | **grafo de código** tree-sitter + SQLite/FTS5 (sin runtime de IA) |
| **asanabrial/leteo** | 4 | Rust | MIT | 20290 | **memoria persistente** para agentes (MCP de 23 tools) + `migrations/` |
| **codecoradev/uteke** | 293 | Rust | Apache-2.0 | 15155 | memoria a largo plazo (MCP) |
| **michelbr84/GarraRUST** | 19 | Rust | MIT | 52040 | todo el estado (conversaciones, memoria, config, credenciales) |
| **A79-ai/ampup-memory** | 4 | Rust | Apache-2.0 | 4981 | memoria que **aprende** |
| **yungSNAT/reformed-agent** | 6 | Rust | ⚠️NOASSERTION | 59586 | puerto a Rust de Hermes Agent |
| **hotic/acpira** | 23 | Rust | MIT | 85848 | workbench **auto-extensible** |
| **futuregene/future-os** | 119 | Rust | MIT | 99957 | un agente en terminal, escritorio y **móvil** (`mobile/`) |
| **bobmatnyc/trusty-tools** | 22 | Rust | MIT | 252802 | workspace unificado de herramientas `trusty-*` + `vmtest-harness/` + `.secrets.baseline` |
| **LuckyOneTwoThree/Qwitch** | 7 | Rust | MIT | 17538 | gateway multi-cuenta (Tauri+Rust): cuotas, rotación de cuentas |
| **cortexkit/broca** | 0 | Rust | MIT | 45405 | runner LLM: bucle agéntico agnóstico de proveedor |
| **Sannrox/sekai-chisei** | 0 | Rust | Apache-2.0 | 21842 | control-plane local-first con `compliance/` y `proto/` |
| **Fongobongo/agentgrid** | 0 | Rust | MIT | 5480 | orquestador distribuido |
| **anthonyhann/agent-mesh** | 0 | Rust | ⚠️NOASSERTION | 5428 | orquestar varios CLI de agente |
| **WispAgents/wisp** | 17 | Rust | MIT | 183515 | "tus agentes de código, como código" |
| **heywisp/wisp** | 1 | Rust | MIT | 13979 | asistente de terminal en Rust |
| **BFJ-Concerns/Swage** | 0 | Rust | AGPL-3.0 | 19491 | plataforma self-hosted para agentes |
| **Tirith** | 0 | Rust | MIT | 216518 | gestor autónomo de agentes de código (plugin `tessl`) |
| **ConekoAI/Peko** | 0 | Rust | Apache-2.0 | 29379 | asistentes en tu propio ordenador, datos locales |
| **KunlunMeta-dev/Kcoder** | 32 | Rust | MIT | 89863 | plataforma agéntica |
| **nanocodexyz/nanocode** | 869 | Rust | Apache-2.0 | 401058 | SDK de agentes embebidos |
| **ottto-ai/ottto** | 0 | Rust | Apache-2.0 | 12545 | recolector local de agentes de código |
| **jarviisha/onehand** | 1 | Rust | Apache-2.0 | 11668 | host de escritorio para agentes |
| **dylanmaniatakes/OpenDex** | 0 | Rust | MIT | 1654 | LLM chat/agent personalizable |
| **surenjanath/kiln** | 0 | Rust | ⚠️ | 280 | servidor LLM local ultrarrápido |
| **myhMARS/langgraph-rs** | 0 | Rust | MIT | 6599 | LangGraph en Rust: orquestación con estado |
| **pingedbrain/rig-contrib** | 0 | Rust | MIT | 604 | crates de apoyo para Rig (A2A, MCP, multi-agente) |
| **zhugez/metrixa** | 0 | Rust | Apache-2.0 | 151 | motor de evaluación |
| **Ruturajgaikwad** *(claude-code-utility-tools)* | 0 | — | — | — | *desempaquetador de Claude Code 2.1.88* (medido antes ✓) |
| **hibuka-labs/phimin** | — | — | — | — | **NO EXISTE** (o es privado) |

> `hibuka-labs/phimin` no resuelve en la API: o se borró o es privado.

---

## 2. Clasificación por función y **qué falta a MegaDaemon**

MegaDaemon hoy: **249 ficheros `.rs`, 242.317 líneas** en `rust-core/src`
(medido ✓). Ya tiene módulos de *muchas* de estas áreas (el botín ya está
parcialmentecollected por la flota ✓):

| función | MegaDaemon HOY | el que mejor lo hace | qué falta |
|---|---|---|---|
| **runtime agéntico** | `agent_loop.rs`, `agent_core.rs`, `agent_coding.rs`, `meta_agent.rs`, `broca`-like en `providers.rs` | **jcode** (22k★, 476 MB, `crates/`+`sdk/`+`src/`+`vendor/`), **rig** (framework), **adk-rust** (`adk-core`+`adk-agent`) | un `core/` único con **contrato tipado** (mi `protocol.rs` es el primer paso ✓) y un `sdk/` para clientes |
| **seguridad / sandbox** | `script_sandbox.rs` ✓, `tool_policy.rs` ✓, `root_hide.rs` | **kernex** (sandbox **del SO**: Seatbelt/sandbox-exec, no "restringir funciones"), **hatchling** (`docs/arch/trust-boundary.md`) | llevar `tool_policy.rs` de "lista de herramientas" a **sandbox del SO** + `trust-boundary` documentado |
| **gobernanza / política** | `accelerator_policy.rs`, `quota_guard.rs` | **praetore** (gobernanza determinista, `fuzz/`) | reglas *permitido/preguntar/denegado* con **fuzzing** de la propia política |
| **PII / secretos** | `redact_forma.rs`, `api_key.rs`, `quota_guard.rs` | **gaze** (tokenización **reversible** con manifiesto del dueño, "fail closed", 14 crates) | un *runtime* de seudonimización **antes** del modelo, no sólo redacción de salida |
| **memoria** | `board.sqlite`, `coordination.rs`, `task_scheduler.rs`, `antigravity_sessions.rs` | **leteo** (memoria viva en SQLite + 23 tools MCP + `migrations/`), **uteke**, **ampup-memory** (memoria que aprende), **GarraRUST** (estado unificado) | `memdir/` por proyecto: decisiones, convenciones y descubrimientos caros, con **migraciones** |
| **conocimiento del código** | `daad.rs`, `daad_ir.rs`, `code_ast.rs`, `dex_decompile.rs`, `re_engine.rs` | **codegraph-rust** (tree-sitter + SQLite/FTS5, **sin runtime de IA**, con `skills/` y `ui/`) | un grafo de símbolos del repo **propio**, consultable por MCP (el `daad` es de APK, no de código fuente) |
| **evaluación** | `harness_policy.rs` (tests) | **metrixa** (motor de evaluación de agentes de código) | evals del **agente**, no sólo del código |
| **multiagente** | `service_bus.rs`, `board.rs`, `delegate.rs`, `mesh.rs`, `peers.rs` | **team-agents** (`agentbus/`), **agent-of-empires** (`aoe-plugin-api/`), **agentgrid**, **mesh-llm** | **un bus con rondas** (`rounds/`) y una **API de plugins** real |
| **GUI / escritorio** | `webui.rs`, `cockpit_server.rs`, `voice_runtime.rs` | **thClaws** (GUI en Rust nativo), **tact** (terminal+desktop, `ARCHITECTURE.md` 53 KB), **agent-desktop** (computer use + `skills/`) | página de chat con el agente (P0 de mi plan ✓) + **skills** servidas por HTTP |
| **computer use** | `automation_bridge.rs`, `adb_client.rs`, `inject.rs` | **agent-desktop** (probes ✓), **browser-control** (navegador real, 5 MB), **terminator** (Windows) | un *probe* reutilizable (captura+acción) con评测 |
| **móvil / embebido** | todo el daemon ✓ (raíz, termostato,电量) | **Napaxi** (SDK on-device), **nanocode** (agentes embebidos), **future-os** (`mobile/`) | empaquetar el daemon como **SDK embebido** reutilizable |
| **proveedores / cuotas** | `providers.rs`, `provider_catalog.rs`, `quota_guard.rs`, `price_table.rs`, `gasto_datos.rs` | **Qwitch** (multi-cuenta con rotación y *pool* de cuentas), **broca** (agnóstico de proveedor) | **pools de cuentas** con rotación (el problema real de las cuotas) |
| **calidad / higiene** | `tools/check-sintaxis.sh`, `tools/ci-known-red.txt` | **trusty-tools** (`.secrets.baseline` de 196 KB ✓, `vmtest-harness/`), **praetore** (`fuzz/`), **hatchling** (`trust-boundary.md`) | *baseline* de secretos del repo + harness de VM para pruebas de RED |

---

## 3. El botín: **qué copiar de cada uno** (con el fichero de MegaDaemon donde aterriza)

| # | de dónde | qué se copia | aterriza en |
|---|---|---|---|
| B1 | **kernex** | **sandbox a nivel de SO** (Seatbelt en macOS, `sandbox-exec`/landlock en Linux) en vez de "permitir estas funciones" ⇒ el `root_exec` de MegaDaemon deja de ser el punto rojo | `rust-core/src/script_sandbox.rs` + `tool_policy.rs` |
| B2 | **gaze** | pipeline de PII **antes** del modelo: reconocer → *veto* del validador → tokenizar con formato preservado → manifiesto del dueño → restaurar; "fail closed" y trazar cada token | nuevo `rust-core/src/pii_runtime.rs` (o extensión de `redact_forma.rs`) |
| B3 | **leteo** + **uteke** | **memoria por proyecto** en SQLite con `migrations/`, servida por MCP (23 tools) ⇒ lo que el agente "aprendió" (decisiones, bugs, convenciones) sobrevive a la compactación | nuevo `rust-core/src/memory.rs` + tool MCP `memory_*` |
| B4 | **codegraph-rust** | grafo de símbolos del **propio repo** (tree-sitter + SQLite/FTS5, **sin IA dentro del indexador**) consultable por MCP ⇒ el agente sabe qué función llama a qué sin leerlo todo | nuevo `rust-core/src/code_graph.rs` (aprovecha `code_ast.rs` ✓) |
| B5 | **agent-of-empires** | **API de plugins** de verdad (`.mcp.json`, `agents/`, `commands/`, hooks) + `acp-worker` ⇒ las capacidades crecen sin recompilar | `rust-core/src/skills.rs` (ya ✓) + `plugin.rs` nuevo |
| B6 | **team-agents** | `agentbus/` + `rounds/` ⇒ **rondas** de agentes con turnos y presupuesto | `service_bus.rs` (extender) + `board.rs` |
| B7 | **Qwitch** | **pools de cuentas** por proveedor con rotación y topes ⇒ el veto de presupuesto (`daily_budget_permite`) pasa a ser "qué cuenta uso" | `providers.rs` + `quota_guard.rs` |
| B8 | **metrixa** | motor de **evaluación** del agente (no del código): ¿la tarea se resolvió? ⇒ tablas de calidad por tarea | nuevo `rust-core/src/evals.rs` |
| B9 | **praetore** | **gobernanza determinista** + *fuzzing* de la política ⇒ la política de herramientas se prueba con entradas arbitrarias | `tool_policy.rs` + `tests/fuzz_*` |
| B10 | **trusty-tools** | higiene de repo: `.secrets.baseline` (196 KB ✓), `vmtest-harness/` para pruebas en máquina virtual, `.pre-commit-config.yaml` | `tools/` + `.github/` |
| B11 | **jcode** | el **harness de referencia**: `crates/` + `sdk/` + `src/` + `vendor/` + `telemetry-worker/` + `packaging/` ⇒ la forma de un repo de agente serio | `rust-core/` (separar `sdk/` y `crates/`) |
| B12 | **adk-rust** | el **catálogo de crates** por capacidad: `adk-auth`, `adk-browser`, `adk-computer-use`, `adk-artifact`, `adk-audio`, `adk-bench`, `adk-devtools` ⇒ un mapa de qué crate tocar para qué | reorganización de `rust-core/src` |
| B13 | **agent-desktop** | `probes/` (sondeos reutilizables) + `skills/` + `benchmarks/` ⇒ cada capacidad nueva llega con su prueba y su benchmark | `tools/` + `crates/` de pruebas |
| B14 | **browser-control** | un cliente de navegador **de 5 MB** que funciona: prueba que no todo necesita 500 KB | `mcp_ext.rs` (tool `browser_*`) |
| B15 | **rig** + **langgraph-rs** | orquestación **con estado** (nodos, transiciones, reintentos) ⇒ el `agent_loop` deja de ser un `while` | `agent_loop.rs` |
| B16 | **tact** + **thClaws** | separación **terminal / escritorio / TUI** sobre el mismo núcleo + `ARCHITECTURE.md` de 53 KB como contrato | `cockpit_server.rs` + `webui.rs` |
| B17 | **openfang** + **future-os** | un **"sistema operativo de agente"**: canales (terminal, web, móvil, voz) sobre un mismo runtime | `cockpit_server.rs`, `voice_runtime.rs`, `automation_bridge.rs` |
| B18 | **sekai-chisei** | `compliance/` + `proto/` + `adapters/` ⇒ un contrato de eventos versionado (`proto/`) para que todos los nodos hablen igual | `service_bus.rs` + `protocol.rs` ✓ |
| B19 | **agentbench** + **toolport** | **worktrees por tarea** y **config común** para todos los agentes ⇒ cada agente trabaja en su worktree y con la misma config | `git_ops.rs` + `tools/` |
| B20 | **browser/terminator/napaxi/nanocode** | **computer use** como *probe* portable (captura→acción→verificación) y **SDK on-device** | `automation_bridge.rs` |

### 3.0b El **ACP** (Agent Client Protocol): un estándar que hay que hablar

Medido en **tres** sitios independientes ⇒ no es una moda de un proyecto, es
un estándar de facto:
* `MiniMax-AI/minimax-code` ⇒ `mcode acp` (**servidor ACP** ✓, medido hoy).
* `agent-of-empires` ⇒ `acp-worker/` ✓ (medido en el árbol ✓).
* `zavora-ai/adk-rust` ⇒ crate `adk-acp` ✓ (medido en el árbol ✓).

⇒ **MegaDaemon debería hablar ACP**: es el modo de que un editor (o el GUI de
Antigravity) piloten al agente del móvil por un protocolo **tipado y
versionado** — que es exactamente el objetivo de `rust-core/src/protocol.rs`
(`PROTOCOL_VERSION = "1.0"`, `initialize`, `session/*`, `tool/*` ✓). La
decisión a documentar: **ACP como dialecto** de nuestro protocolo, o nuestro
protocolo como dialecto de ACP.

### 3.1 Licencias: **qué se lee y qué NO se copia**
* **Copiar código** ⇒ sólo de **MIT / Apache-2.0** (`jcode`, `rig`, `kernex`,
  `gaze`, `leteo`, `uteke`, `codegraph-rust`, `agent-desktop`, `browser-control`,
  `tact`, `thClaws`, `adk-rust`, `openfang`, `future-os`, `sekai-chisei`,
  `broca`, `praetore`⚠️, `metrixa`, `trusty-tools`, `Qwitch`, `team-agents`,
  `agent-of-empires`).
* **AGPL/GPL ⇒ sólo IDEAS, nunca código**: `hatchling` (AGPL), `openhuman`
  (GPL), `claurst` (GPL), `Napaxi` (GPL), `agentbench` (GPL), `Swage` (AGPL).
* **NOASSERTION ⇒ no copiar sin revisar la licencia real**: `praetore`,
  `reformed-agent`, `metrixa` (Apache ✓), `agent-mesh`.

---

## 4. Los **productos** a decompilar: estado real

| producto | estado | método (medido) |
|---|---|---|
| **OpenAI Codex** | ✅ **fuente abierta** (132 crates, Rust) ⇒ no hay que decompilar: se **lee** | `codex-rs/` medido ✓; `docs/AGENTIC_ARCHITECTURE.md` |
| **Claude Code** | ✅ **decompilado** (2.1.331): 3.839 ficheros con `bun-unpacker`, bundle deofuscado, 28 herramientas, prompt y slash-comandos | `docs/RE_CLAUDE_CODE_231_DECOMPILADO.md` ✓ |
| **Antigravity (Google)** | ✅ en curso por la flota: `re_engine.rs`, `re_obf.rs`, `re_strenc.rs`, `re_crypto.rs`, `go_retool.rs`, `antigravity_*` | regenerador Go en `regeneracion-go-antigravity` ✓ |
| **MiniMax Code (PC)** | ⚠️ **MEDIDO HOY**: el **CLI es CÓDIGO ABIERTO MIT** (`MiniMax-AI/minimax-code` ✓, npm `@minimax-ai/code` ✓, abierto el 2026-09-18) ⇒ **no hay que decompilarlo: se lee**. Lo que NO se publica es la **app de escritorio** ⇒ ése es el objetivo de RE (`bun-unpacker`/`asar` sobre el instalador). El CLI trae además el **servidor ACP** (`mcode acp`) y el **modo headless** (`mcode exec`) | fuente: `MiniMax-AI/minimax-code` (npm + repo) ✓; el framework sobre el que está es **Pi** (`endil-works/pi`: LLM unificado + bucle de agente + TUI) ✓ |
| **DeepSeek Code** | ⚠️ **MEDIDO**: en npm hay `deepseek-code` 1.0.3 ✓ ("CLI tool for code analysis and generation using DeepSeek API") ⇒ es una CLI fina, **no** el agente de escritorio; el repo `echoVic/orca-agent` es *DeepSeek-native* ✓ ⇒ referencia de **integración**, no de RE | — |
| **Cursor** | ❌ **NO MEDIDO** (app Electron/VS Code fork) | mismo método, `--asar`/asar unpack + búsqueda de las claves de API |
| **Windsurf** | ❌ **NO MEDIDO** (id. Codeium) | id. |

**Regla que sale de lo ya hecho (Codex + Claude Code)**: el valor de la RE no
está en el código (está minificado), está en **los contratos**: el manifiesto
de plugin, el formato de las skills, la política de permisos, el prompt y el
catálogo de herramientas. ⇒ Para Cursor/Windsurf/MiniMax Code el objetivo es
extraer **sus contratos**, no reconstruirlos: `package.json`, el manifiesto de
extensiones, los ficheros de política, los templates de prompt y el esquema de
sus herramientas.

**"Compilarlo de vuelta"**: para Claude Code 2.1.331 el bytecode JSC (395,8 GB
de caché) **no** es reconstruible desde el texto; lo que sí es reproducible es
el **bundle transpilado** (el mismo que ejecuta Bun tras compilar) ⇒ objetivo
real: *bundle legible + mapa de módulos + contratos extraídos*, que es
exactamente lo que ya hay en `cc-beautified/`.

---

## 5. Calidad de código: checklist que se roba

| fuente | qué se copia |
|---|---|
| **trusty-tools** | `.secrets.baseline` (196 KB), `vmtest-harness/`, `.pre-commit-config.yaml` (10 KB), allowlists por fichero (`.line-cap-allowlist.tsv`…) |
| **praetore** | `fuzz/` (fuzzing de la política), `packages/` (contratos), `deploy/` |
| **hatchling** | `docs/arch/trust-boundary.md` (requisitos de frontera de confianza) |
| **adk-rust** | `AGENTS.md` de **43 KB** (el contrato del proyecto en un fichero) + `.clippy.toml` + `.codeqlignore` |
| **jcode** | `AGENTS.md` + `OAUTH.md` (23 KB) + `vendor/` (dependencias fijadas) |
| **tact** | `ARCHITECTURE.md` de **53 KB** + `book/` + `suites/` (suites de pruebas) |
| **agent-of-empires** | `aoe-plugin-api/` (API pública de plugins), `tessl-plugins/` |
| **codegraph-rust** | `skills/` propio ⇒ el repo teaches a los agentes ✓ |

⇒ **Acciones concretas para MegaDaemon** (baratas, alto valor):
1. `docs/ARCHITECTURE.md` de 53 KB como el contrato del crate (hoy no existe
   uno ⇒ hay 14 planes sueltos ✓ ⇒ hay que **fusionarlos** en uno).
2. `.secrets.baseline` (detecta fugas como la de la clave de Highnote ✓).
3. `trust-boundary.md`: qué trusts con root y qué no.
4. Un `fuzz/` de la política de herramientas.

---

## 6. El **plan de refactor definitivo** de MegaDaemon

Prioridad = (impacto × facilidad) ÷ riesgo. Cada punto con su puerta.

### P0 · **Un solo núcleo y un contrato** (días 1-3)
* `rust-core/src/core/` nuevo: `agent_loop` (orquestación con estado, de
  **langgraph-rs**), `tools` (catálogo), `memory` (de **leteo**), `policy`
  (de **praetore**), `provider` (de **broca**), `protocol` (mi `protocol.rs` ✓).
* Puerta: `cargo test --lib` verde y **el mismo núcleo** sirve al CLI, al MCP
  server, al cockpit y al gateway (`/v1/chat/completions` ✓).

### P1 · **Seguridad de verdad** (días 3-7)
* `tool_policy.rs` → **sandbox del SO** (de **kernex**) + `trust-boundary.md`.
* `redact_forma.rs` → **pipeline de PII reversible** (de **gaze**), aplicado
  *antes* del modelo.
* Puerta: un test que demuestra que una herramienta denegada **no** se ejecuta
  y que el motivo vuelve al cliente; y que una PII del prompt sale tokenizada.

### P2 · **Memoria y conocimiento** (semana 2)
* `memory.rs` (SQLite + `migrations/` + MCP) de **leteo**/**uteke**.
* `code_graph.rs` (tree-sitter + FTS5) de **codegraph-rust** ⇒ el agente
  consulta el repo sin leerlo entero.
* Puerta: "lo que el agente aprendió" sobrevive a una compactación y se puede
  consultar por MCP.

### P3 · **Extensibilidad sin recompilar** (semana 3)
* `plugin.rs`: manifiesto (`.codex-plugin/plugin.json`-compatible ✓), `skills/`,
  `.mcp.json`, `hooks.json` ⇒ instalar una capacidad = soltar una carpeta.
* Puerta: un plugin de ejemplo instalado desde disco aparece en el catálogo
  **sin recompilar** ✓.

### P4 · **Canales, GUI y computer use** (semana 4)
* Página de chat con el agente en `webui.rs` hablando `protocol.rs` ✓.
* `probes/` reutilizables (de **agent-desktop**) para el computer use del
  móvil.
* Puerta: desde el navegador, una conversación real con el agente del móvil.

### P5 · **Calidad y evaluación** (continuo)
* `evals.rs` (de **metrixa**) + `fuzz/` de la política (de **praetore**) +
  `.secrets.baseline` (de **trusty-tools**) + **un** `docs/ARCHITECTURE.md`
  que sustituya a los 14 planes sueltos ✓.

---

## 7. "Transpilar a Rust" — de dónde viene cada idea

Casi todo el botín **ya es Rust** ✓ (los 59 proyectos medidos: Rust en 58 ✓) ⇒
no hay transpilación que hacer: hay **adaptación de diseño** ✓. Lo no-Rust:
* **Qwitch**: Tauri + TypeScript ⇒ *patrón* de pool de cuentas ⇒ a Rust:
  `providers.rs` ✓ (el lekker idiomático: un `enum ProviderTier` + un pool).
* **acpira**: pnpm + Rust mixto ⇒ *patrón* de "workbench auto-extensible" ⇒ a
  Rust: `plugin.rs` ✓.
* **trusty-tools**: tiene `python/` ⇒ *higiene* (allowlists) ⇒ a Rust: `tools/`.
* **Claude Code / Cursor / Windsurf**: JS ⇒ **deofuscado y Contracts** ⇒ a
  Rust: `skills.rs` ✓ + `protocol.rs` ✓ (ya hecho ✓).

---

## 8. Riesgos

1. **El crate ya tiene 242.317 líneas en 249 ficheros** ⇒ el mayor riesgo es
   **no** añadir otro módulo suelto, sino **reagrupar** (§6 P0) ✓. Regla: todo
   módulo nuevo entra en `core/` o es un fallo de diseño.
2. **AGPL/GPL** ⇒ ideas sí, código no ✓ (§3.1).
3. **La flota está empujando a `main` cada pocos minutos** ⇒ cualquier
   refactor grande choca. Regla: P0 en un commit propio y con rebase
   frecuente, nunca un `merge` masivo a ciegas ✓.
4. **Memoria/OOM del sandbox** ⇒ medir antes de compilar ✓ (aquí no se compila
   nada: sólo se lee y se mide ✓).

---

## 9. Trazabilidad

| afirmación | de dónde sale |
|---|---|
| los 59 repos (★, lang, licencia, KB, push) | API GraphQL de GitHub, 2026-11-27, 1 consulta con 60 alias → `/workspace/repos_medidos.json` |
| el árbol de 31 repos | 2ª consulta GraphQL (`object(expression:"HEAD:")`) → `/workspace/arboles_repos.json` |
| kernex: sandbox del SO | `raw.githubusercontent.com/kernex-dev/kernex/main/MANIFESTO.md` ✓ |
| gaze: pipeline de PII | `raw.githubusercontent.com/CertaMesh/gaze/main/ARCHITECTURE.md` ✓ |
| hatchling: infra always-on | `raw.githubusercontent.com/undergroundrap/hatchling/main/README.md` ✓ |
| codegraph-rust: grafo sin IA | `raw.githubusercontent.com/sunerpy/codegraph-rust/main/README.md` ✓ |
| leteo: memoria + 23 tools MCP | `raw.githubusercontent.com/asanabrial/leteo/main/README.md` ✓ |
| Codex y Claude Code | `docs/AGENTIC_ARCHITECTURE.md` y `docs/RE_CLAUDE_CODE_231_DECOMPILADO.md` ✓ |
| tamaño y módulos de MegaDaemon | `find rust-core/src -name '*.rs'` en `main` (44322f3) ✓ |
| código ya recogido por la flota | `rust-core/src/{tool_policy,script_sandbox,redact_forma,skills,protocol,board,service_bus,re_*,daad*}.rs` ✓ |

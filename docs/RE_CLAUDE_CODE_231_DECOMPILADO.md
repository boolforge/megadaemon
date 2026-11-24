# Claude Code 2.1.331 — DECOMPILADO y deofuscado (no sólo desarmado)

> Fecha: 2026-11-24. Todo medido en esta sesión sobre el binario real.
> Continuación de `docs/RE_CLAUDE_CODE_2_1_331.md` (el descriptivo). Este
> documento es el **resultado de la decompilación**.

## 1. El método (ya no artesanal: `bun-unpacker`)

La herramienta adecuada existe y **nació investigando justo cómo se empaqueta
Claude Code**: [`Enuvid/bun-unpacker`](https://github.com/Enuvid/bun-unpacker).

```bash
npm i -g bun-unpacker
bun-unpacker claude --list
bun-unpacker claude -o cc-unpacked --path-patching false
```

Salida medida del binario `2.1.331` (linux-x64):

```
ELF · x86-64 · payload 221.7 MB at 0x9e72008 · 3839 files ·
52-byte entries · trailer at 0x17c196cf
total extracted: 68.4 MB
entry point: cli
395.8 GB of JSC bytecode skipped (pass --bytecode to dump it)
```

**3839 ficheros** extraídos byte a byte (no reconstruidos del código máquina),
el punto de entrada es `cli` y el resto son *chunks* con los módulos.

## 2. Lo que es y lo que no se puede decompilar

| parte | estado |
|---|---|
| **el JS del producto** (el bundle transpilado) | **EXTRAÍDO** ✓ y deofuscado ✓ (formateado con `js-beautify`: el chunk mayor pasa de 6,1 MB a 9,3 MB formateado) |
| los *chunks* | 3.592 ficheros `.js` + assets (`mermaid.min.js`, `hljsBundle`, `chart.umd`, `clipboard-napi.node` ✓, `src/plugins/functionHooks/hooks-worker/hooks-worker.js` ✓) |
| **el bytecode JSC** | **NO** se decompila: Bun guarda 395,8 GB de caché de bytecode y el bundle se ejecuta como `// @bun @bytecode` ✓ ⇒ el JS extraído es el *pre-compilación* (la última fuente legible) |
| el runtime nativo de Bun | es el ELF en sí (`not stripped` ✓) |

## 3. Lo que se ha extraído del código deofuscado

### 3.1 Las 28 herramientas del Agent SDK presentes en 2.1.331

`Bash, Read, Edit, Write, Glob, Grep, WebFetch, WebSearch, Task, TodoWrite, Skill, AskUserQuestion, EnterPlanMode, ExitPlanMode, EnterWorktree, ExitWorktree, SendMessage, TeamCreate, TeamDelete, ToolSearch, LSP, NotebookEdit, PowerShell, REPL, Config, Brief, Sleep, RemoteTrigger

**Lectura para MegaDaemon**: el juego de herramientas del agente *de verdad* es
más pequeño y más sobrio que el mío (28, no ~100): edición, búsqueda,
web, **subagentes**, **skills**, **plan mode**, **worktrees**, **equipos**
(`Team*`/`SendMessage`), **búsqueda de herramientas** (`ToolSearch`) y
**preguntar a la persona** (`AskUserQuestion`). Las mías de 100 blown-up
tienen que ir en un catálogo aparte y **búsqueda bajo demanda** (`ToolSearch`
= hueco H9 ya medido).

### 3.2 El prompt del sistema (extraído, offset 46.307.157 en el bundle)

```
You are Claude Code, Anthropic's official CLI for Claude.",v="You are Claude Code, Anthropic's official CLI for Claude, running within the Claude Agent SDK.",U="You are a Claude agent, built on Anthropic's Claude Agent SDK.",J=[k,v,U],I=new Set(J);function sEo(e){return I.has(e)}function iEo(e){if(we()==="vertex")return k;if(e?.recorded!==void 0)return e.recorded;if(e?.isNonInteractive){if(e.hasAppendSystemPrompt)return v;return U}return k}function XCl(e,n,r){let o=eta(r.fingerprint,r.agentContext,r.previousRequestId,r.promptId,r.turnOrigin,r.turnPosition,{attributionProject:YWi(),responseBeforeCompaction:r.responseBeforeCompaction}),p=Ol([o,n,...e].filter(Boolean));return H(p),p}function JCl(e,n){let r=eta(n.fingerprint,n.agentContext,void 0,void 0,void 0,void 0,{ignoreEnvOptOut:n.forceAttributionHeader===!0&&!xD(),attributionProject:YWi()});if(!r)return e;return[{type:"text",text:r},...e]}function H(e){try{let[n]=VWi(e,{skipTelemetry:!0}),r=n?.text;a("tengu_sysprompt_block",{length:r?.length??0,hash:r?Ce(N("sha256").update(r).digest("hex")):S("")})}catch(n){i(qe(L(n),"system prompt block telemetry failed"))}}function VWi(e,n){let r=Mte(),o=e.findIndex((u)=>u===R0);if(r&&n?.skipGl
```

(...continúa en `system_prompt.txt`, 8 KB.)

### 3.3 Los slash-comandos de 2.1.331

`afplay, bun, callback, command, config, dbus-send, diff, disable, enable, example-language-server, explorer, function, install, marketplace-add, marketplace-remove, marketplace-update, meta, npm, npx, open, plugin, pwd, remote-control, rg, say, uninstall, unknown, update

⇒ **nuevos frente a 2.1.88**: `plugin`, `marketplace-add`, `marketplace-remove`,
`marketplace-update` (la gestión de plugins es de primera clase),
`remote-control` (control remoto del agente) y `explorer`.

### 3.4 Los assets de skills y plugins

`assets_skills_plugins.md` (103 KB) recoge los 150 fragmentos con
`SKILL.md`, `plugin.json`, `marketplace.json`, `allowed-tools`,
`disable-model-invocation` y `.claude-plugin` ✓.

## 4. Lo que esto cambia en la hoja de ruta

* **H2 (plugins/skills) deja de ser «una idea»**: hay un formato exacto y un
  catálogo de comandos para gestionarlos ⇒ el cargador ya implementado
  (`rust-core/src/skills.rs`) es el primer piece y sólo falta exponerlo por
  MCP y el `plugin.json`.
* **H9 (búsqueda de herramientas) es de primera clase** (`ToolSearch`): con
  ~100 herramientas, el agente debe **buscarlas**, no llevarlas todas.
* **H1 (contrato de clientes)**: `remote-control` confirma que el control
  remoto del agente es un caso de uso real ⇒ el `protocol.rs` que ya está en
  `main` es el camino correcto.
* **H7 (plan mode) y H10 (preguntar)**: `EnterPlanMode`/`ExitPlanMode` y
  `AskUserQuestion` ⇒ en un móvil con root, preguntar y proponer plan antes
  de actuar no es opcional.

## 5. Trazabilidad

| afirmación | de dónde sale |
|---|---|
| 3.839 ficheros, payload 221,7 MB, trailer en 0x17c196cf | `bun-unpacker --list` sobre el binario 2.1.331 |
| 68,4 MB extraídos, entrada `cli`, 395,8 GB de bytecode omitidos | salida de `bun-unpacker` |
| deofuscado con `js-beautify` | `/workspace/cc-beautified/` |
| prompt del sistema en offset 46.307.157 | barrido de los 3.592 `.js` |
| 28 herramientas del SDK | barrido de nombres en el bundle deofuscado |
| assets de skills/plugins (150 fragmentos) | barrido con contexto |
| el cargador ya implementado | `rust-core/src/skills.rs` (9 tests) |

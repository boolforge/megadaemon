# CONTRATO 2 — AGENT RULES, SKILLS Y CUSTOM MODES (subagentes)

## 2.1 Dónde viven las reglas, skills y subagentes

`out/vs/workbench/workbench.desktop.main.js:748` (`cursorRules.js`, servicio `cursorRulesService`):

```js
vRt = UJ(".claude/skills")
> [recortado: 11 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

→ Cursor lee **skills de 4 ecosystems distintos** (`.claude/`, `.codex/`, `.grok/`, `.agents/`) más el suyo.
`.agents/skills` es el estándar inter-vendor emergente: **muy relevante para MegaDaemon**.

| Concepto | Ruta | Ámbito |
|---|---|---|
| Reglas (formato nuevo `.md`) | `<ws>/.cursor/rules/*.md` | proyecto |
| Reglas (formato legacy `.mdc`) | `<ws>/.cursor/rules/*.mdc` | proyecto |
| Reglas de usuario | `~/.cursor/rules/` | usuario |
| Skills | `<ws>/.cursor/skills/<nombre>/SKILL.md` | proyecto |
| Skills de usuario | `~/.cursor/skills/<nombre>/SKILL.md` | usuario |
| Skills gestionadas por Cursor | `~/.cursor/skills-cursor/<id>/SKILL.md` | usuario |
| Skills inter-vendor | `<ws>/.agents/skills/<nombre>/SKILL.md` | proyecto |
| Plugins | `<ws>/.cursor/plugins/`, `<ws>/.cursor/plugins/cache/` | proyecto |
| Plugins de usuario | `~/.cursor/plugins/local/<nombre>` | usuario |
| **Custom modes / subagentes** | `<ws>/.cursor/agents/` y `~/.cursor/agents/` | proyecto + usuario |
| Slash commands | `<ws>/.cursor/commands/*.md` | proyecto |
| Sandbox policy | `<ws>/.cursor/sandbox.json` | proyecto |
| Settings (plugins) | `<ws>/.cursor/settings.json` | proyecto |
| Entorno cloud | `<ws>/.cursor/environment.json` | proyecto |
| Plan del agente | `<ws>/.cursor/plans/<basename>` | proyecto |
| Worktrees | `<ws>/.cursor/worktrees` | proyecto |
| Config CLI | `<ws>/.cursor/cli.json` | proyecto |
| Uploads de cloud agent | `~/.cursor/projects/<hash>/uploads` | usuario |

Evidencia de `~/.cursor/agents`: `workbench.desktop.main.js:832` —
`this._userAgentsDir=te.joinPath(e,".cursor","agents")` y
*"Failed to ensure ~/.cursor/agents before watch; watch may not attach until it exists"*.
→ **Cursor crea `~/.cursor/agents` automáticamente** y lo vigila con un watcher.

---

## 2.2 Tipos de regla (discriminados)

`workbench.desktop.main.js:18381` (`s7c`):

```js
> [recortado: 5 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

| Tipo | Significado |
|---|---|
| `global` | `alwaysApply: true` → entra siempre en el prompt |
| `fileGlobbed` | tiene `globs` → se adjunta cuando el fichero abierto casa con el glob |
| `agentFetched` | el agente la trajo por sí mismo (descripción en `description`) |
| `manuallyAttached` | el usuario la adjuntó a mano |
| `none` | sin tipo |

Campos completos del objeto regla (`workbench.desktop.main.js:18381`, `kxm` los compara todos):
`description`, `globs`, `alwaysApply`, `filename`, `body`, `frontmatter`, `fullPath`,
`gitRemoteOrigin`, `parseError`, `warnings`, `environments`, `disabledEnvironments`,
`plugin`, `marketplace`, `pluginId`, `marketplaceId`.

---

## 2.3 Formato exacto del fichero de regla (frontmatter + cuerpo)

### Escritura (lo que genera Cursor)

`workbench.desktop.main.js:18387-18390`:

```js
> [recortado: 11 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

→ Formato exacto:

```
> [recortado: 6 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

### Lectura (parser de frontmatter) — `workbench.desktop.main.js:18715`, función `Usg`

```js
> [recortado: 8 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

Claves aceptadas en el frontmatter de una regla:

| Clave | Tipo | Default |
|---|---|---|
| `description` | string | `""` |
| `globs` | string | `""` |
| `alwaysApply` | boolean (`==="true"`) | `false` |
| `attachToBackgroundAgents` | boolean (`==="true"`) | `false` |
| `metadata.environments` | string[] | `[]` |
| `metadata.disabledEnvironments` | string[] | `[]` |

Detalles del parser (útiles para reimplementarlo):
- Las líneas vacías y las que empiezan por `#` se ignoran.
- El parser es **ad-hoc**, no YAML completo: solo `clave: valor` a nivel superior.
- Las listas se soportan en dos formas:
  - bloque indentado con `- item` bajo una clave (`^\s+-\s+(.*)$`),
  - o `metadata.environments: a, b, c` (split por coma y trim).
- `metadata.environments:` con valor vacío → array vacío.

### Legacy `.mdc` vs nuevo `.md`

`molten/dist/engine/engine.cjs:4729` (función `pki`):

```js
> [recortado: 10 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

→ **Un `.md` sin frontmatter es SIEMPRE `alwaysApply:true` con `globs:[]`.**
Un `.mdc` lleva el frontmatter explícito.

Y hay un comando de migración **`Cursor: Convert Legacy Agent Applied Rules`**
(`workbench.desktop.main.js:18715`):
- busca `**/.cursor/rules/**/*.mdc`,
- si el `.mdc` **no** tiene `description`, o no tiene `globs`, o no tiene `alwaysApply` → lo descarta (no migra),
- si el frontmatter tiene `globs` o `alwaysApply` → lo descarta,
- si no → escribe un `.md` con el cuerpo y el frontmatter `description` y **borra** el `.mdc`.

O sea: `.mdc` = "regla legacy" → `.md` = "regla nueva" con solo `description` (y el cuerpo).

---

## 2.4 Skills (`SKILL.md`) — frontmatter y validaciones

Fuente: `extensions/cursor-agent-exec/dist/main.js:213` y `:8`.

### Campos del frontmatter de `SKILL.md`

| Campo | Alias aceptados | Tipo | Semántica |
|---|---|---|---|
| `name` | — | string | **debe coincidir con el nombre de la carpeta**; minúsculas y números unidos por guiones simples; máx 64 chars |
| `description` | — | string | **obligatoria** salvo si `disable-model-invocation: true`; máx 1024 chars |
| `alwaysApply` | — | boolean | convierte la skill en regla `global` |
| `disable-model-invocation` | — | boolean | la skill NO se ofrece al modelo; solo el usuario la invoca |
| `environments` | `metadata.environments` | string[] \| CSV | entornos cloud donde la skill aplica |
| `disabled-environments` | `metadata.disabledEnvironments` | string[] \| CSV | entornos donde **no** aplica |
| `metadata.scopedTo` | — | string[] \| CSV | ámbito de la skill |
| `metadata.surfaces` | — | string[] \| CSV | superficies donde se muestra |
| `paths` | `globs` | string[] | rutas/ficheros a los que aplica (reglas) |
| `model` | — | string | modelo a forzar (vía subagente) |
| `prompt` | — | string | prompt del subagente |

Normalización de arrays (`cursor-agent-exec/dist/main.js:213`, función `D_`):
```js
> [recortado: 6 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```
→ **acepta array o CSV** en el mismo campo. Muy copiable.

### Mensajes de validación literales (los mismos strings)

`cursor-agent-exec/dist/main.js:213`:

```js
> [recortado: 5 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

Y (`molten/dist/engine/engine.cjs:4729`):
```js
`Invalid skill frontmatter: ${e}`
`Description is required`
```

Límites numéricos: nombre ≤ **64**, descripción ≤ **1024**, cuerpo ≈ **5000 tokens** (recomendado).

### Parser de frontmatter usado

`cursor-agent-exec/dist/main.js:8`:
```js
implementation:(i=e.featureFlags, F$(i)?"frontmatter":"graymatter")
```
→ dos implementaciones interchangeables: la propia (`frontmatter`) o `graymatter` (la de npm), con **feature flag** `agentReplaceGraymatter`.
En MegaDaemon ya existe un parser propio en `rust-core/src/skills.rs` (subconjunto YAML sin dependencia) → **encaje directo**.

### Diferencias Cursor vs Claude Code (importante para MegaDaemon)

| Campo | Cursor | Claude Code |
|---|---|---|
| desactivar invocación por modelo | `disable-model-invocation` (kebab) | `disableModelInvocation` (camel) |
| ámbito | `metadata.scopedTo` | — |
| superficies | `metadata.surfaces` | — |
| siempre activo | `alwaysApply` | — |
| entornos | `environments` / `metadata.environments` | — |
| rutas | `paths` o `globs` | `files` |
| herramientas permitidas | — | `allowedTools` |

→ **MegaDaemon debería aceptar AMBAS grafías** (kebab y camel) para ser interoperable.

---

## 2.5 Custom modes = Custom Subagents (proto `agent.v1.`)

Fuente: `extensions/cursor-mcp/dist/main.js:212`.

```js
> [recortado: 7 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

### Enums

```js
Mv = jc(Mc.proto3, Nv, "CustomSubagentPermissionMode",
> [recortado: 8 lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]
```

### Tabla de campos

| # | Campo | Tipo | Significado |
|---|---|---|---|
| 1 | `full_path` | path | ruta del fichero que lo define |
| 2 | `name` | string | nombre del modo |
| 3 | `description` | string | descripción (se usa como `description` del mode picker) |
| 4 | `tools` | repeated string | **allowlist de herramientas del modo** |
| 5 | `model` | string | modelo del modo |
| 6 | `prompt` | string | prompt del modo |
| 7 | `permission_mode` | enum | `DEFAULT` \| `READONLY` \| `AGENT_ONLY` |
| 8 | `is_background` | bool | corre en background |
| 9-12 | `plugin`/`marketplace`/`plugin_id`/`marketplace_id` | optional string | procedencia (plugin instalado) |
| 13 | `force_default_model` | bool | ignora el modelo elegido por el usuario |
| 14 | `source` | optional string | procedencia |

### Modos internos (no son custom modes)

`cursor-mcp/dist/main.js:212` — strings literales: `"AGENT"`, `"ASK"`, `"EDIT"`, `"DEBUG"`.
Estos son los **modos** del producto; los *custom modes* son instancias de `CustomSubagent` que añaden
`tools` (allowlist), `model` y `prompt` encima.

En la UI el selector de modos separa `builtInModes` de los custom:
`workbench.desktop.main.js:17651` — `F=Z3(()=>(s??[]).filter(dt=>dt.source!=="custom"),[s])`,
`B=Z3(()=>(s??[]).filter(dt=>dt.source==="custom"),[s])`.
→ el campo `source` distingue **built-in** de **custom**: exactamente el discriminador de `CustomSubagent.source`.

### `switch_mode` como tool

El catálogo de tools incluye `switchModeToolCall → "switch_mode"`
(véase `05_tools.md`), o sea el modelo puede **cambiar de modo** él mismo.

---

## 2.6 Valoración para MegaDaemon

| Aspecto | Valoración | Motivo |
|---|---|---|
| Frontmatter de regla (`description`/`globs`/`alwaysApply`) | **COPIABLE tal cual** | 3 campos, YAML trivial, ya casi estándar. |
| Frontmatter de `SKILL.md` (Cursor) | **ADAPTABLE** | Copiar **los nombres de campo**, pero aceptar también la grafía camelCase de Claude Code y `allowed-tools`/`context`/`files`. El parser propio de `skills.rs` ya sirve: solo hay que añadir campos. |
| Reglas `.md` = `alwaysApply` implícito | **COPIABLE tal cual** | Regla simple: sin frontmatter → siempre activa. |
| Migración `.mdc` → `.md` | **ADAPTABLE** | Requiere reescritura de ficheros + borrado; útil como comando de migración de MegaDaemon. |
| Lectura multi-ecosistema (`.claude/`, `.codex/`, `.grok/`, `.agents/`) | **COPIABLE tal cual (estrategia)** | Es lo más valuable: **Cursor ya es multi-ecosistema**, y `.agents/skills` es el estándar más neutral. MegaDaemon debería adoptar la misma lista. |
| `CustomSubagent` (custom modes) | **COPIABLE tal cual** | El proto es limpio: `name`, `description`, `tools[]`, `model`, `prompt`, `permission_mode`, `is_background`. Es el diseño de "custom mode" más simple y portable que existe. |
| `permission_mode` (DEFAULT/READONLY/AGENT_ONLY) | **COPIABLE tal cual** | 3 niveles, muy usable. Encaja con `tool_policy.rs`. |
| `TaskMode` (AGENT/PLAN) | **COPIABLE tal cual** | Idéntico al dichotomy plan/ejecución. |
| Límites (64 nombre / 1024 desc / 5000 tokens cuerpo) | **COPIABLE tal cual** | Números razonables; ya están en `skills.rs` de forma similar. |
| `metadata.surfaces` / `metadata.scopedTo` | **ADAPTABLE** | Semántica de UI de Cursor; en un daemon hay que redefinir qué es una "surface". |
| Reglas por defecto del producto | **NO MEDIDO / NO EXISTEN** | `find . -name "*.mdc"` → **0 resultados** en todo el paquete. Cursor **no** trae reglas por defecto; las crea el usuario o el plugin. (Los `skills-cursor/<id>/SKILL.md` gestionados sí existen en runtime, pero no están en el paquete.) |

**Destino en MegaDaemon**: `rust-core/src/skills.rs` (frontmatter + campos nuevos),
`rust-core/src/protocol.rs` (mensaje `CustomSubagent`),
`rust-core/src/tool_policy.rs` (`permission_mode`),
nuevo `rust-core/src/agent_modes.rs` (custom modes).

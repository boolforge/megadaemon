# RE — **Cargador multi-ecosistema** de reglas, skills y MCP

> MegaDaemon · subagente de RE de productos · 2026‑11‑27
>
> **Por qué este documento**: dos productos independientes —**Cursor 3.32.4** y
> **Devin Desktop 3.15.14**— implementan un *importador* que lee reglas, skills y
> servidores MCP de herramientas ajenas. Validado por dos productos ⇒ es un
> **requisito**, no una curiosidad.
>
> **Todas las fuentes citadas están medidas** en los dos paquetes:
> `artifacts/medidas/cursor/*.md` (Cursor) y
> `Devin/resources/app/extensions/windsurf/devin/share/devin/docs/**` (Devin).
> Rutas y líneas wherever el original las daba.

---

## 1. Las dos listas medidas

### 1.1 Devin (10 fuentes)

| Fuente | Tipo | Dónde lo dice |
|---|---|---|
| `AGENTS.md` | regla always‑on | `rules.mdx:104` |
| `AGENTS.local.md` | regla personal (gitignore) | `rules.mdx:105` |
| `AGENT.md` | regla (singular) | `rules.mdx:106` |
| `.windsurfrules` | regla legacy | `rules.mdx:107` |
| `CLAUDE.md` | regla | `rules.mdx:108` |
| `.devin/rules/*.md` | regla con `trigger` | `rules.mdx:126` |
| `.devin/global_rules.md` | regla always‑on | `rules.mdx:127` |
| `.windsurf/rules/*.md` | regla con `trigger` | `rules.mdx:166` |
| `.cursor/rules/*.md` / `*.mdc` | regla Cursor | `rules.mdx:143` |
| `.claude/` | directorio | `rules.mdx:185` |

Skills: `.devin/skills/`, `.windsurf/skills/`, `.agents/skills/`, `.kiro/skills/`,
`.claude/skills/`, `.github/skills/`, `.codex/skills/` + globales en
`~/.config/devin/skills/`, `~/.agents/skills/`, `~/.codeium/<channel>/skills/`,
`~/.kiro/skills/` (`skills/overview.mdx`, tabla "Where Skills Live").

MCP: `.devin/mcp_config.json`, `.devin/mcp_config.local.json`,
`~/.config/devin/mcp_config.json`; **más importación** de Cursor, Claude Code,
OpenCode y Zed (`config-file.mdx:394-409`).

### 1.2 Cursor (12 fuentes)

`.cursor/rules/*.md`, `.cursor/rules/*.mdc`, `~/.cursor/rules/`,
`.cursor/skills/`, `~/.cursor/skills/`, `~/.cursor/skills-cursor/<id>/`,
`.agents/skills/`, `.claude/skills/`, `.codex/skills/`, `.grok/skills/`,
`.cursor/plugins/`, `.cursor/plugins/cache/`, `~/.cursor/plugins/local/<name>/`,
`.claude/plugins/`, `.claude/plugins/cache/`, `.cursor/agents/`,
`~/.cursor/agents/`, `.cursor/commands/*.md`, `~/.cursor/hooks.json`,
`.cursor/sandbox.json`, `.cursor/settings.json`, `.cursor/environment.json`.

### 1.3 La lista **unificada** que hay que implementar

| # | Ruta | Tipo | Ámbito |
|---|---|---|---|
| 1 | `AGENTS.md` | regla | proyecto |
| 2 | `AGENTS.local.md` | regla | proyecto (gitignore) |
| 3 | `AGENT.md` | regla | proyecto |
| 4 | `.windsurfrules` | regla | proyecto |
| 5 | `CLAUDE.md` | regla | proyecto + global (`~/.claude/CLAUDE.md`) |
| 6 | `.devin/rules/*.md` | regla | proyecto (+ `~/.devin/rules/`) |
| 7 | `.devin/global_rules.md` | regla | proyecto (+ `~/.devin/`) |
| 8 | `.windsurf/rules/*.md` | regla | proyecto |
| 9 | `.windsurf/global_rules.md` | regla | proyecto |
| 10 | `.cursor/rules/*.md` / `*.mdc` | regla | proyecto (+ `~/.cursor/rules/`) |
| 11 | `.claude/rules/*.md` | regla | proyecto |
| 12 | `.codex/**` | regla/skill | proyecto |
| 13 | `.kiro/rules/*.md`, `.kiro/skills/<n>/SKILL.md` | regla/skill | proyecto |
| 14 | `.grok/skills/<n>/SKILL.md` | skill | proyecto |
| 15 | `.agents/skills/<n>/SKILL.md` | skill | proyecto |
| 16 | `.opencode/**` | MCP | proyecto |
| 17 | `.zed/**` | MCP | proyecto |

**Skills** (una sola tabla, formato `SKILL.md` idéntico en las 4 fuentes):
`.devin/skills/`, `.windsurf/skills/`, `.cursor/skills/`, `.claude/skills/`,
`.codex/skills/`, `.kiro/skills/`, `.grok/skills/`, `.agents/skills/`,
`.github/skills/`, `~/.agents/skills/`, `~/.config/devin/skills/`,
`~/.codeium/<channel>/skills/`, `~/.kiro/skills/`, `~/.cursor/skills/`,
`~/.cursor/skills-cursor/<id>/`.

**MCP**: `.devin/mcp_config.json` + `.devin/mcp_config.local.json` +
`~/.config/devin/mcp_config.json` + importación de `.cursor/mcp.json`,
`~/.claude.json`, `~/.config/opencode/…`, `~/.config/zed/…`.

---

## 2. Formato de regla **unificado**

Los tres formatos coexisten. El interno debe **normalizar a uno**.

### 2.1 Campos que existen en algún sitio

| Campo | Cursor | Claude Code | Devin/Windsurf | Tipo |
|---|---|---|---|---|
| `description` | ✔ | ✔ | ✔ | `Option<String>` |
| `alwaysApply` | ✔ | — | — | `bool` |
| `globs` | ✔ | — | — | `String` (Cursor) / lista |
| `paths` / `files` | `paths` | `files` | — | lista de rutas |
| `trigger` | — | — | ✔ | enum de 5 |
| `disableModelInvocation` / `disable-model-invocation` | ✔ | ✔ | — | `bool` |
| `allowedTools` | — | ✔ | — | lista |
| `metadata.scopedTo` | ✔ | — | — | lista |
| `metadata.surfaces` | ✔ | — | — | lista |
| `environments` / `metadata.environments` | ✔ | — | — | lista |

### 2.2 El modelo interno

```rust
pub struct Rule {
    pub id:            String,       // hash de (path, body)
    pub source:        Source,       // qué ecosistema y qué fichero
    pub body:          String,       // markdown sin frontmatter
    pub activation:    Activation,
    pub globs:         Vec<String>,  // normalizado desde globs/paths/files
    pub description:   Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub scopes:        Vec<String>,  // metadata.scopedTo / surfaces / environments
    pub model_invocable: bool,       // !disableModelInvocation
    pub precedence:    u8,
}

pub enum Activation {
    AlwaysOn,      // always-on
    Glob,          // glob-activated
    ModelDecided,  // el modelo decide
    AgentDecided,  // "agent" de Devin
    UserInvoked,   // sólo el usuario
}
```

### 2.3 Tabla de normalización

| Origen | Condición | → `Activation` | `globs` |
|---|---|---|---|
| **Cursor** | `alwaysApply: true` | `AlwaysOn` | `[]` |
| **Cursor** | tiene `globs` | `Glob` | parsear la lista |
| **Cursor** | sólo `description` | `ModelDecided` | `[]` |
| **Cursor** | ninguna de las anteriores | `UserInvoked` | `[]` |
| **Cursor** | `.md` **sin** frontmatter | ⚠️ `AlwaysOn` | `[]` |
| **Devin** | `trigger: always_on` | `AlwaysOn` | `[]` |
| **Devin** | `trigger: glob` | `Glob` | de `globs`/`paths` |
| **Devin** | `trigger: model_decision` | `ModelDecided` | `[]` |
| **Devin** | `trigger: agent` | `AgentDecided` | `[]` |
| **Devin** | `trigger: manual` | `UserInvoked` | `[]` |
| **Devin** | binario: `agent_decided` | `AgentDecided` | `[]` |
| **Claude** | `disableModelInvocation: true` | `UserInvoked` | `[]` |
| **Claude** | `files:` presente | `Glob` | los `files` |
| **Claude** | `allowedTools:` presente | — | → `allowed_tools` |
| **genérico** | sin frontmatter en formato Devin | `AlwaysOn` | `[]` |

### 2.4 ⚠️ **Divergencia semántica medida** — hay que decidir

El **mismo fichero** `.cursor/rules/x.md` **sin frontmatter** se interpreta de
**dos formas distintas** por los dos productos:

| Producto | Interpretación | Fuente |
|---|---|---|
| **Cursor** | **siempre activa** (`alwaysApply:true` implícito, `globs:[]`) | `docs/medidas/cursor/02_rules_skills_modes.md:115` |
| **Devin** (leyendo `.cursor/rules/*.md`) | **manual**: *"None of the above — User must invoke manually"* | `rules.mdx:157-162` |

⇒ **Hay que elegir una** y documentarlo. Recomendación: **`AlwaysOn`** (el
comportamiento de Cursor, que es el que manda sobre el formato) — porque un
`.md` sin frontmatter es lo más común y tratarlo como manual perdería reglas.
Pero conviene exponerlo como **configurable**.

### 2.5 ⚠️ **Segunda divergencia**: dos nombres para lo mismo

`model_decision` (doc de Devin) vs `agent_decided` (binario de Devin). El
**binario manda** sobre la doc (es el código que corre). ⇒ **soportar ambos
nombres**; el canónico interno es `ModelDecided`.

### 2.6 Grafías

Cursor usa **kebab** (`disable-model-invocation`), Claude **camel**
(`disableModelInvocation`). ⇒ **aceptar ambas** (regla de la tabla "Cursor vs
Claude" en `02_rules_skills_modes.md:180-189`).

---

## 3. Reglas de **precedencia** y merge

Medido en los dos productos.

| Regla | Fuente | Qué hacer en MegaDaemon |
|---|---|---|
| `.devin/` **gana** a `.windsurf/` (global_rules) | `rules.mdx:131` | si existen ambos `global_rules.md`, cargar **sólo** el de `.devin` |
| `.devin/rules/` **y** `.windsurf/rules/` se cargan **ambos** | `rules.mdx:131` | merge, con `.devin` por delante |
| La raíz del workspace se carga al inicio; los subdirectorios **de forma perezosa** | `rules.mdx:112` | carga diferida al acceder al directorio |
| MCP: **3 ficheros** con precedencia | `mcp/configuration.mdx:53` | `local` > `project` > `user` por nombre de servidor |
| MCP: `mcpServers` en el `config.json` principal **se migra** a los ficheros propios | `mcp/configuration.mdx:53` | migración automática al arrancar (v3.6+); hay que hacerlo **por código**, no a mano |
| Permisos de skill **aditivos**, nunca amplían lo denegado arriba | `creating-skills.mdx:253` | el merge de permisos es **intersección** con loDenied, **unión** con loAllow |
| `deny`/`ask` de organización **no** los anula el modo de permiso | `permissions.mdx:39` | la precedencia de permisos va por capas, no por orden de arrival |

---

## 4. Dónde va cada cosa en `rust-core/src/skills.rs`

`skills.rs` (16.960 B, 9/9 tests) es el sitio. Estructura propuesta:

| Bloque | Contenido | Tamaño est. | ¿Nuevo o extiende? |
|---|---|---|---|
| `Rule` + `Activation` + `Source` | el modelo unificado (§2.2) | 120‑180 L | **nuevo** (tipos) |
| `Frontmatter` | parser YAML mínimo (ya hay uno: subconjunto sin dependencia) | 0‑80 L | **extiende** |
| `normalize_cursor()` | §2.3 filas Cursor | 60‑100 L | **nuevo** |
| `normalize_claude()` | §2.3 filas Claude | 60‑100 L | **nuevo** |
| `normalize_devin()` | §2.3 filas Devin (+ `agent_decided`) | 60‑100 L | **nuevo** |
| `discover_sources()` | recorre las 17 rutas de §1.3 por directorio | 150‑250 L | **nuevo** |
| `merge()` | precedencia `.devin` > `.windsurf`, dedup por id | 100‑150 L | **nuevo** |
| `merge_permissions()` | aditivo/intersectado según §3 | 80‑120 L | **nuevo** |
| `discover_skills()` | las 15 rutas de skills | 120‑180 L | **nuevo** |
| `discover_mcp()` | 3 ficheros + 4 importaciones | 100‑150 L | **nuevo** (o en `mcp_ext.rs`) |
| tests | una tabla de 25 casos (fuente × trigger → activación) | 250‑400 L | **extiende** |

**Total estimado: 1.150‑1.850 líneas** — el bloque más grande de esta tanda.
`rust-core/src/mcp_ext.rs` es el sitio natural para la parte de MCP, si existe
(medido: existe, `mcp_ext.rs` y `mcp_server.rs`).

---

## 5. Reglas de precedencia para el **merge** (propuesta)

1. **Nunca** se pierde una regla: si dos fuentes traen la misma `id`, gana la de
   mayor `precedence` y **se registra el conflicto** en el log (no se descarta en
   silencio).
2. `precedence` (mayor gana):
   `AGENTS.local.md` (40) > `AGENTS.md` (35) > `.devin/rules/` (30) >
   `.windsurf/rules/` (25) > `.cursor/rules/` (20) > `.claude/rules/` (15) >
   `.codex/` (12) > `.kiro/` (10) > `.grok/` (8) > `.agents/` (5) >
   `.windsurfrules` (3) > `CLAUDE.md` (2).
   *Racionalo*: el orden refleja "cuanto más cerca del formato nativo, más manda".
3. **Skills** se fusionan por **nombre de directorio**: si dos ecosistemas dan
   el mismo nombre, gana el de mayor precedencia y el resto queda como
   *shadowed* (no se pierde; se puede listar).
4. **MCP** se fusiona por **nombre de servidor**: el `local` pisa al `project`,
   que pisa al `user`. Un servidor deshabilitado (`enabled:false`) se respeta.

---

## 6. Trazabilidad

| Afirmación | Evidencia |
|---|---|
| Reglas de Devin, rutas y `trigger` | `…/docs/extensibility/rules.mdx:98-133, 141-187` |
| `read_config_from` (10 proveedores) | `…/docs/reference/configuration/config-file.mdx:394-409` |
| Rutas de skills | `…/docs/extensibility/skills/overview.mdx`, tabla "Where Skills Live" |
| MCP: 3 ficheros + migración v3.6 | `…/docs/extensibility/mcp/configuration.mdx:53` |
| Cursor: rutas + discriminadores | `docs/medidas/cursor/02_rules_skills_modes.md:12, 44-60, 93-122` |
| Cursor: `.md` sin frontmatter = always | `docs/medidas/cursor/02_rules_skills_modes.md:115` |
| Cursor ↔ Claude: kebab vs camel | `docs/medidas/cursor/02_rules_skills_modes.md:180-189` |
| Cursor: multi-ecosistema | `docs/medidas/cursor/02_rules_skills_modes.md:12, 253` |
| `agent_decided` (binario) | `artifacts/devin/devin_cli_strings.txt` líneas 446.271 / 447.009 |

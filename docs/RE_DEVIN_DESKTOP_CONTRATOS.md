# RE — Contratos de producto: **Devin Desktop 3.15.14** (ex‑Windsurf), linux‑x64

> Subagente de RE de productos · MegaDaemon · 2026‑11‑27
> Todo lo afirmado aquí está **medido** en el paquete. Lo que no lo está va marcado
> **NO MEDIDO**. Las rutas son exactas dentro del tarball, con raíz `Devin/`.
> Copia idéntica de este documento en `artifacts/DEVIN_CONTRATOS.md`;
> ficheros extraídos en `artifacts/devin/contratos/`; inventario en
> `artifacts/devin/INVENTARIO.tsv`.

---

## 1. El renombrado: Windsurf → **Devin Desktop** (probado)

**Confirmado.** Cognition renombró el producto; el código interno conserva la marca
vieja en muchísimos sitios (motivo por el que el paquete sigue siendo "Windsurf" por
dentro).

### 1.1 Prueba primaria — la respuesta de la API de actualización

`artifacts/vendor/devin.json` (566 B), servido por
`https://windsurf-stable.codeium.com/api/update/linux-x64/stable/latest`:

```json
{"url":"https://windsurf-stable.codeiumdata.com/linux-x64/stable/cfa79668e1a5e000e036cea74fc23449b0aedec3-c16c32bf7d9d42345cd0c798c527a68e/Devin-linux-x64-3.15.14.tar.gz",
 "name":"1.141.2","version":"cfa79668e1a5e000e036cea74fc23449b0aedec3",
 "productVersion":"1.141.2","hash":"ab6d734be09a1904c3efc87b1fa02739cdd92aaf",
 "sha256hash":"6cd616eae9890237d2b77d0d410fc6f83151eacce0c862bfb707e2c5d9eac703",
 "supportsFastUpdate":true,"windsurfVersion":"3.15.14","displayName":"Linux x64 (.tar.gz)"}
```

Cuatro pruebas en el mismo JSON:

1. El **nombre del artefacto** es `Devin-linux-x64-3.15.14.tar.gz`.
2. La **versión de marketing** va en el campo `windsurfVersion` (`3.15.14`) — el campo
   no se renombró.
3. `productVersion`/`name` = `1.141.2` → **la app Electron/VS Code es la 1.141.2**;
   `3.15.14` es la versión del producto que ve el usuario.
4. El **host** sigue siendo `windsurf-stable.codeiumdata.com` (y la URL de update,
   `windsurf-stable.codeium.com`).

El `sha256hash` del JSON **coincide byte a byte** con el tarball descargado:

```
6cd616eae9890237d2b77d0d410fc6f83151eacce0c862bfb707e2c5d9eac703  devin.tar.gz
```

⇒ integridad verificada, no es un artefacto reconstruido.

### 1.2 Prueba en el propio paquete

| Evidencia | Ruta exacta dentro del paquete | Valor medido |
|---|---|---|
| `product.json` | `Devin/resources/app/product.json` | `nameShort:"Devin"`, `nameLong:"Devin"`, `applicationName:"devin-desktop"`, `dataFolderName:".devin"`, `urlProtocol:"devin"`, `version:"1.141.2"`, `commit:"cfa79668e1a5e…"` |
| Identificador Windows heredado | idem (`win32AppUserModelId`) | `Exafunction.Windsurf` ← **marca vieja conservada** |
| Nombre del binario Electron | `Devin/devin-desktop` (262 MB) | antes `windsurf` |
| Wrapper CLI | `Devin/bin/devin-desktop` (2.109 B) | script `sh` que invoca `resources/app/out/cli.js` |
| Nombre de la app Node | `Devin/resources/app/package.json` | `"name":"Devin"`, `"version":"1.141.2"`, `"main":"./out/main.js"` |
| Versión de la CLI embebida | `…/extensions/windsurf/devin/distribution` | fichero de 9 B (canal) |
| Extensiones | `…/resources/app/extensions/` | 105 extensiones; las de la casa siguen llamándose `windsurf`, `windsurf-auth`, `theme-windsurf`, `windsurf-remote-devin`, `windsurf-remote-openssh`, `windsurf-remote-wsl` |
| IDs de comando | `…/extensions/windsurf/package.json` | 52 comandos: conviven `windsurf.*` **y** `devin.*` con el mismo título (p. ej. `windsurf.triggerCascade` y `devin.newConversation` → *"Start new conversation"*) |
| Setting heredado | idem | `windsurf.cascade.readClaudeCodeConfig`, `windsurf.allowCascadeAccessGitignoreFiles` → **"Cascade" sigue siendo el nombre interno del agente** |
| Host de galería | `product.json` → `extensionsGallery.serviceUrl` | `https://marketplace.windsurf.com/vscode/gallery` |
| Host de update | `product.json` → `updateUrl` | `https://windsurf-stable.codeium.com` |

**Conclusión operativa:** el *producto* es Devin Desktop; el *código* es Windsurf.
Hay que buscar por los dos nombres.

---

## 2. Qué se midió y cómo

| Dato | Valor |
|---|---|
| Paquete | `artifacts/vendor/devin.tar.gz`, 432.763.567 B, 7.620 entradas |
| sha256 | `6cd616ea…eac703` (verificado contra la API) |
| Desempaquetado | `artifacts/devin/Devin/` — **1.321.476.169 B (1,23 GB)**, 7.013 ficheros |
| Hash de commit | `cfa79668e1a5e000e036cea74fc23449b0aedec3` (idéntico al `version` de la API) |
| Modo | **solo lectura**; no se ha ejecutado Devin, la CLI ni ningún binario |

**Estructura medida:**

```
Devin/
├── devin-desktop              # binario Electron (nombre ya renombrado)
├── bin/devin-desktop          # wrapper CLI sh
├── resources/
│   ├── completions/{bash,zsh}/devin-desktop
│   └── app/
│       ├── package.json       # name: "Devin", version 1.141.2
│       ├── product.json       # identidad, updateUrl, galería, aplicación
│       ├── node_modules.asar  # 168.217.204 B — asar real (10.347 ficheros)
│       ├── node_modules.asar.unpacked/
│       ├── extensions/        # 105 extensiones VS Code
│       └── out/               # VS Code fork + out/vs/sessions/ = panel del agente
└── …lib/, locales/, *.pak
```

### 2.1 El `.asar`: sí existe, y se desempaqueta

`resources/app/node_modules.asar` **es un asar real** (no un directorio). Formato
verificado byte a byte y desempaquetador propio en
`artifacts/devin/contratos/_tools/unpack_asar.py` (Python puro, sin `npx asar`):

```
offset 0  : u32 = 4              (pickle_size)
offset 4  : u32 = 556.048        (header_size)
offset 8  : u32 = 556.044        (header_string_size)
offset 12 : u32 = 556.037        (json_len)
offset 16 : JSON del header (556.037 B, con padding a 4)
payload   : base = 8 + header_size = 556.056
```

**Hallazgo:** el asar se desempaquetó **completo** (10.347/10.347 ficheros, verificado
contra el recuento del header JSON). Contiene las dependencias de VS Code (`katex`,
`mermaid`, `playwright-core`, `ssh2`, `undici`, `unleash-client`, `node-pty`, `tar`,
`open`, `kerberos`…). **Los contratos del agente NO están ahí**: viven en
`extensions/windsurf/dist/extension.js` (6.590.197 B, webpack) y en el binario Rust
`extensions/windsurf/devin/bin/devin` (262.445.096 B).

Árbol desempaquetado en `artifacts/devin/devin_asar_unpacked/`;
manifiesto de tamaños en `artifacts/devin/asar_manifest.tsv`.

> Nota: `resources/app` **no** es un asar (es un directorio). El único asar del paquete
> es `node_modules.asar`.

### 2.2 El hallazgo mayor: el producto **embarca su propia documentación y su CLI**

`extensions/windsurf/devin/` contiene un **bundle completo de la CLI Devin**:

| Elemento | Tamaño | Notas |
|---|---|---|
| `bin/devin` | 262.445.096 B | CLI Rust, crate interno `pkg.cognition.build-3983e96abc3e6656` |
| `libexec/devin-telemetry` | 11.153.570 B | binario de telemetría |
| `share/devin/docs/**.mdx` | 42 ficheros, 459.136 B | **documentación oficial completa** |
| `share/man/man1/*.1` | 204 man pages | árbol de comandos completo en roff |
| `share/devin/docs/acp/*.mdx` | 3 | ACP para JetBrains, Xcode, Zed |

Es decir: **los contratos no hubo que inferirlos del binario** — están escritos en el
propio paquete, en formato legible. Eso cambia la calidad de esta RE de "RE" a
"lectura de especificación".

---

## 3. Tabla por categoría

Rutas relativas a la raíz del tarball (`Devin/`). Tamaño = bytes del fichero.

| # | Categoría | Qué hay | Ruta exacta | Tamaño | ¿Copiable? | Motivo |
|---|---|---|---|---|---|---|
| a | **Manifiesto de extensiones y puntos de entrada** | 105 extensiones VS Code; 52 comandos, 22 keybindings, menú, config, `jsonValidation` | `resources/app/extensions/windsurf/package.json` | 24.391 | **Adaptable** | Estructura propia; copiar los *nombres de comando* y los *títulos*, no el fichero |
| a′ | Identidad de producto | `nameShort`, `applicationName`, `dataFolderName`, `updateUrl`, galería | `resources/app/product.json` | 50.266 | **No copiable** | Configuración de terceros; solo se citan las claves |
| a″ | Bundle JS de la extensión (implementación) | webpack con el agente | `resources/app/extensions/windsurf/dist/extension.js` | 6.590.197 | **No copiable** | Código de terceros; solo se leen cadenas |
| b | **Reglas / instrucciones del agente** | `AGENTS.md`, `AGENTS.local.md`, `AGENT.md`, `.windsurfrules`, `CLAUDE.md`, `.devin/rules/*.md`, `.devin/global_rules.md` + **imports** de Cursor/Windsurf/Claude | `…/share/devin/docs/extensibility/rules.mdx` (líneas 98‑133) | 9.077 | **Copiable (formato)** | Los *nombres de ruta* y los *valores de frontmatter* son un contrato de interoperabilidad, no expresión |
| b′ | Reglas de fábrica | 4 (`code-review`, `commit`, `create-pr`, `create-draft-pr`, `fix-ci`, `merge`, `sync`, `sync-upstream`, `troubleshoot`, `act-on-feedback`, `generate-run-commands`, `update-pr`, `update-skills`) con frontmatter `name`+`description` y marcador de override | `resources/app/out/vs/sessions/skills/*/SKILL.md` | 802‑13.633 c/u | **Adaptable** | Los `description` son documentación de producto; copiar el *formato* y reescribir el contenido |
| c | **MCP** | 3 ficheros de config, 3 scopes, transportes `stdio`/`sse`/`http`, OAuth, `${env:}`/`${file:}` | `…/docs/extensibility/mcp/configuration.mdx` (líneas 53‑210) | 18.915 | **Copiable (formato)** | Estándar MCP + extensiones propias documentadas |
| d | **Hooks** | 8 eventos, stdin por evento, `matcher` regex, `hookSpecificOutput` | `…/docs/extensibility/hooks/lifecycle-hooks.mdx` (líneas 16‑375) | 10.817 | **Copiable (formato)** | Formato tipo Claude Code; declarativo |
| e | **Prompts del agente** | Prompt de sistema completo (rodata del binario, offset 184.167.863) + 14 skills de fábrica | `…/devin/bin/devin` | 262.445.096 | **No copiable (texto)** / **copiable (estructura)** | El texto es propiedad de Cognition; la *estructura de secciones* sí es un patrón reusable |
| f | **Catálogo de herramientas y esquemas** | 25 nombres de tool + reglas de autorización + 5 modos de permiso | `…/docs/extensibility/hooks/lifecycle-hooks.mdx` (líneas 356‑375) + `…/docs/reference/permissions.mdx` | 10.817 / 17.655 | **Copiable (nombres)** | Los nombres de herramienta son un contrato de API |
| g | **Endpoints de red** | API, web, updates, telemetría, flags, OAuth, galería | `…/devin/bin/devin` (strings) + `product.json` | — | **No copiable** (URLs) / **copiable (estructura)** | No se redistribuyen hosts de terceros |
| h | **Licencias / precios / condiciones** | `LICENSE.txt` de la app y de la extensión; página de planes `app.devin.ai/plans`; créditos de IA (`acu_consumption_at_last_user_interaction`), *add-on credits*, auto-refill | `resources/app/LICENSE.txt` (833 B), `…/extensions/windsurf/LICENSE.txt` (783 B), `…/share/devin/docs/enterprise/*` | 833 / 783 | **No copiable** | Licencia ajena; los *términos* son de negocio, no código |
| i | **Protocolo CLI / daemon** | Árbol de 204 man pages: `rules`, `skills`, `mcp`, `plugins`, `migrate`, `models`, `daemon`, `acp`, `auth`, `doctor`, `forward`, `ssh`, `sandbox`, `cloud`, `setup` | `…/devin/share/man/man1/*.1` | 204 ficheros | **Copiable (superficie de comando)** | Los *nombres de comando y flags* son un contrato; el *formato roff* no |
| j | **Capa ACP (extra)** | 25+ capacidades propias `cognition.ai/*`; protocolo ACP v1/v2; negociado de `initialize` | `resources/app/extensions/windsurf/dist/acp/AGENTS.md` | 31.656 | **Adaptable** | Son extensiones propietarias sobre ACP; ACP en sí es estándar abierto |

---

## 4. Contratos, en detalle

### 4.a — Manifiesto de extensiones y puntos de entrada

`resources/app/extensions/windsurf/package.json` →
`contributes` = `commands` (52), `configuration` (2), `menus` (1), `keybindings` (22),
`languages` (2), `jsonValidation` (1).

**Puntos de entradainteresting** (comandos, categoría `Devin`):

| comando | título |
|---|---|
| `windsurf.login` / `devin.login` | Log in |
| `windsurf.loginWithAuthToken` | Provide Devin auth token (backup login) |
| `windsurf.importCursorSettings` / `…Extensions` | Import Cursor settings / extensions |
| `windsurf.importWindsurfSettings` | Import Devin Desktop settings |
| `windsurf.triggerCascade` / `devin.newConversation` | Start new conversation |
| `devin.migrateWorkspaceConfig` | Migrate Workspace Config (`.windsurf/` → `.devin/`) |
| `devin.migrateMcpConfig` | Migrate MCP Config (`~/.codeium/` → `~/.config/devin/`) |
| `windsurf.openAcpLocalRegistry` | Open Local ACP Agent Registry |
| `windsurf.reloadAcpConnections` | Reload ACP Connections |
| `devin.copyApiKey` | Copy API Key to Clipboard |
| `windsurf.openBillingPage` | Purchase Add‑on Credits |
| `windsurf.openAutoRefillPage` | Enable Auto Refill |

**`jsonValidation`** (1 entrada) — el producto valida su propio registro ACP:

```json
[{ "fileMatch": "**/acp/registry.json",
   "url": "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.schema.json" }]
```

**Doble espacio de nombres de settings** (`configuration.properties`): coexisten
`windsurf.*` y `devin.*` con las mismas claves —
`windsurf.allowCascadeAccessGitignoreFiles` ≡ `devin.allowCascadeAccessGitignoreFiles`,
`windsurf.cascade.readClaudeCodeConfig` ≡ `devin.cascade.readClaudeCodeConfig`,
`windsurf.acp.*` ≡ `devin.acp.*`. **Copia de la lista completa en
`artifacts/devin/contratos/desktop/comandos-devin.md`.**

### 4.b — Reglas: formato exacto

Fuente: `…/docs/extensibility/rules.mdx`.

**Ficheros aceptados como reglas** (líneas 100‑110), todos tratados igual:

| Fichero | Nota |
|---|---|
| `AGENTS.md` | **recomendado** (estándar inter‑vendor) |
| `AGENTS.local.md` | personal, gitignorado |
| `AGENT.md` | singular alternativo |
| `.windsurfrules` | legacy de Windsurf |
| `CLAUDE.md` | compatible con Claude Code |

**Ubicaciones por directorio** (línea 112): la raíz del workspace se carga al iniciar
sesión; los subdirectorios se **descubren perezosamente** cuando el agente accede a
ficheros de ese directorio.

**Formato `.devin/`** (líneas 120‑133) — una regla por fichero:

| Ruta | Notas |
|---|---|
| `.devin/rules/*.md` | una regla por fichero, admite frontmatter `trigger` |
| `.devin/global_rules.md` | único fichero always‑on |
| `~/.devin/rules/*.md`, `~/.devin/global_rules.md` | ámbito global |

Precedencia: **`.devin/` gana sobre `.windsurf/`**. Si existen ambos
`global_rules.md`, sólo carga el de `.devin`. Las de `rules/` de ambos se cargan.

**Valores de `trigger`** (línea 181): `always_on`, `manual`, `model_decision`, `agent`, `glob`.

> **Discrepancia medida:** el binario contiene la constante `agent_decided`
> (`devin_cli_strings.txt` líneas **446.271** y **447.009** →
> `always_onagent_decidedmanual`), mientras la doc del paquete dice `model_decision`.
> **Medido como dos nombres distintos**; hay que soportar ambos.

**Importación de otras herramientas** (líneas 141‑187):

- **Cursor**: `.cursor/rules/*.md` **y** `.cursor/rules/*.mdc`, con frontmatter Cursor
  (`description`, `globs`, `alwaysApply`). Activación: `alwaysApply:true` → siempre;
  `globs` → por patrón; `description` sólo → decide el modelo; nada → manual.
- **Windsurf**: `.windsurf/rules/*.md` + `.windsurf/global_rules.md`, con `trigger`.
- **Claude Code**: directorio `.claude/`.
- **No soportado**: `.codeiumignore` (línea 190) — avisado explícitamente.

**Control de la importación** (`config-file.mdx` líneas 394‑409), clave `read_config_from`:

| clave | bool/null | por defecto | qué importa |
|---|---|---|---|
| `agents_standard` | | `true` | `AGENTS.md`, `AGENTS.local.md`, `AGENT.md`, `.windsurfrules` |
| `cursor` | | `true` | reglas **y servidores MCP** |
| `windsurf` | | `true` | reglas, skills y MCP |
| `claude` | | `true` | reglas, skills y MCP |
| `copilot` | | `true` | skills |
| `codex` | | `true` | skills |
| `kiro` | | `true` | skills |
| `opencode` | | `true` | MCP |
| `zed` | | `true` | MCP |
| `cloud` | | `true` | MCP + plugins personales desde Devin Cloud (sólo config de usuario) |

### 4.c — MCP

Fuente: `…/docs/extensibility/mcp/configuration.mdx` + man pages `devin-mcp*.1`.

**Ficheros de configuración** (línea 53, con nota de migración):

| Ámbito | Fichero | git |
|---|---|---|
| usuario | `~/.config/devin/mcp_config.json` (`%APPDATA%\devin\mcp_config.json` en Win) | no |
| proyecto | `.devin/mcp_config.json` | **sí** |
| local | `.devin/mcp_config.local.json` | no (gitignored) |

> **Breaking change medido:** antes de la v3000.3 (Local 3.6) los servidores MCP
> vivían en la clave `mcpServers` del `config.json` principal. Ahora van en ficheros
> propios; las entradas `mcpServers` del config principal **se migran solas al arrancar**.

**Transports** (`devin-mcp-add.1`): `stdio` (proceso local), `sse` (Server‑Sent Events),
`http` (request/response). **Inferencia**: si hay URL → `http`; si hay `--command` o
argumentos tras `--` → `stdio`.

**Scopes**: `local` (por defecto), `project`, `user`.

**Flags de `devin mcp add`**: `-t/--transport`, `-s/--scope`, `--url`, `--command`,
`-e/--env KEY=VALUE` (repetible), `-H/--header "Header: value"` (repetible),
`--scopes` (OAuth, CSV), `--oauth-client-id`, `--oauth-client-secret`,
`--oauth-resource` (RFC 8707; cadena vacía lo omite, p. ej. Entra).

**Ejemplo real copiado** (líneas 58‑71 y 143‑166 del doc):

```json
// .devin/mcp_config.json
{ "mcpServers": { "github": {
      "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "ghp_..." } } } }

// servidor HTTP con OAuth
{ "mcpServers": { "notion": {
      "url": "https://mcp.notion.com/mcp", "transport": "http" } } }
```

**Gestión de secretos** (líneas 301‑301, 379‑380): usar `${env:VAR}`, `${file:/ruta}`
o `.devin/mcp_config.local.json`. **Nunca** commitear un client secret.
Cada cliente MCP (Windsurf, Claude Code, Devin CLI) **mantiene su propia sesión OAuth**
→ hay que hacer login en cada uno.

**Comandos**: `devin mcp add|list|get|remove|login|logout|enable|disable`.
`devin mcp list --json` → salida JSON.

### 4.d — Hooks

Fuente: `…/docs/extensibility/hooks/lifecycle-hooks.mdx`.

**Fichero**: `.devin/hooks.v1.json` (confirmado en el binario:
`devin_cli_strings.txt` línea **446.833** → ` .devin/hooks.v1.json`).
Migración desde el legacy `.windsurf/hooks.json` vía `devin migrate hooks`.

**8 eventos**:

| Evento | Cuándo | Campos de stdin |
|---|---|---|
| `PreToolUse` | antes de la herramienta | `tool_name`, `tool_input` |
| `PostToolUse` | después | `tool_name`, `tool_input`, `tool_response{success,output,error}` |
| `PermissionRequest` | pide permiso | `tool_name`, `tool_input` |
| `UserPromptSubmit` | el usuario envía prompt | `prompt` |
| `Stop` | el agente termina el turno | `stop_hook_active` |
| `PostCompaction` | tras compactar contexto | `summary` |
| `SessionStart` | sesión nueva (`source: startup`) o reanudada (`source: resume`) | `source` |
| `SessionEnd` | fin de sesión (**no** en cloud) | `reason` |

**Payload común**: `session_id` (estable por sesión) y `prompt_id` (rota por prompt;
ausente antes del primer prompt).

**Estructura**:

```json
{ "PreToolUse": [ { "matcher": "exec", "hooks": [
      { "type": "command", "command": "python3 -c \"…\"", "timeout": 5 } ] } ] }
```

- `matcher` = **regex** contra `tool_name` (no glob). `""`/omitido = todas.
  Ejemplos del doc (línea 346‑354): `"exec"`, `"^exec$"`, `"^(exec|edit)$"`,
  `"^mcp__.*"`, `"^mcp__github__.*"`, `"^mcp__github__create_issue$"`.
- En eventos sin `tool_name` (no‑tool) el matcher es `""` o se omite.
- **Salida**: el hook escribe en stdout un JSON `hookSpecificOutput`:
  - `hookEventName` + `additionalContext` → inyecta contexto
  - `hookEventName` + `updatedInput` → **reescribe** los argumentos de la tool
  - `{"decision":"approve"}` → aprueba (PermissionRequest)
  - `{"decision":"block","reason":"…"}` → bloquea (Stop)
- Código de salida: el ejemplo bloquea con `sys.exit(2)` en PreToolUse.
- En cloud: sólo *command hooks* (los prompt hooks se saltan), no hay `SessionEnd`,
  los hooks de plugin son **best effort / fail open**.
- `SessionStart` en local: el agente **espera** al hook; un `block` rechaza la sesión.

### 4.e — Prompts del agente

**Prompt de sistema** — embebido como literales `&str` en
`extensions/windsurf/devin/bin/devin`, ancla `"You are Devin, a powerful agentic"`
en el **offset de bytes 184.167.863**. Volcado íntegro en
`artifacts/devin/sistema_prompt_raw.txt` (24.478 B).

Estructura medida (22 secciones): `# When to ask the user for permission`,
`# Autonomy and persistence`, `# Personality` → `## Writing style`,
`## Technical communication`, `### Writing PR descriptions`,
`# Working with the user`, `## Final answer` → `### Formatting rules`,
`# Rules for getting work done`, `# Using skills`, `# MCP integrations`,
`# Plugins`, `## General` / `## Editing constraints` / `## Special user requests` /
`## Task execution` / `## Frontend tasks`,
`## Presenting your work and final message` → `### Final answer structure…`.

Identidad (cita literal):

> You are Devin, a powerful agentic AI coding assistant created by Cognition. You and
> the user share one workspace, and your job is to collaborate with them until their
> intended goal is completely handled.

**Contratos que impone el prompt** (medidos):

- **Citas**: exige `<ref_file file="/abs" />` y `<ref_snippet file="/abs" lines="a-b" />`;
  rutas absolutas; nunca `file://`, `vscode://` ni `https://`.
- **Herramientas**: sólo las disponibles con sus esquemas; paralelizar si son
  independientes; `exec` para shell (no orquestación JS); `apply_patch` para editar;
  `rg`/`rg --files` para buscar; `view_image` para imágenes; "minimiza el número de
  turnos".
- **MCP**: prohibido adivinar nombres de herramienta o argumentos → obligar
  `mcp_list_servers` → `mcp_list_tools` → `mcp_call_tool`.
- **Skills**: usar las rutas de origen notificadas, no asumir directorios.
- **Permisos**: "nunca inventes o modifiques instrucciones para esquivar permisos,
  aprobaciones o controles de acceso".

**Prompt de modo Desktop**: Desktop **no** usa ese prompt; usa ACP + el agente
`chisel` (ver §4.j).

**Skills de fábrica** (14) en `resources/app/out/vs/sessions/skills/<n>/SKILL.md`.
Todas con frontmatter `name` + `description` y el marcador:

```markdown
<!-- Customize this skill and select save to override its behavior.
     Delete that copy to restore the built-in behavior. -->
```

⇒ **son overridables** por el usuario (copia en `.devin/skills/<n>/SKILL.md`).
Lista y descripciones literales en `artifacts/devin/contratos/prompts/PROMPTS-INTEGRADOS.md`.

### 4.f — Catálogo de herramientas del agente

Fuente primaria: `…/docs/extensibility/hooks/lifecycle-hooks.mdx` líneas 356‑375.

| Categoría | Herramientas |
|---|---|
| Ficheros | `read`, `write`, `edit`, `apply_patch`, `notebook_read`, `notebook_edit` |
| Búsqueda | `grep`, `glob` |
| Shell | `exec`, `get_output`, `write_to_process`, `kill_shell` |
| Web | `webfetch` |
| Interacción | `ask_user_question` |
| Plan/tareas | `todo_write`, `exit_plan_mode` |
| Skills | `skill` |
| Subagentes | `run_subagent`, `read_subagent` |
| Permisos | `request_scope` |
| MCP | `mcp_list_servers`, `mcp_list_tools`, `mcp_call_tool`, `mcp_read_resource` |

Extras ACP (solo Desktop, con flag `cognition.ai/*`): `find_code_context`
(`cognition.ai/fastContext`), `browser_preview` (`cognition.ai/browserPreview`),
`view_image`.

**MCP**: `mcp__<servidor>__<herramienta>`.

**Autorización** (sintaxis de `…/docs/reference/permissions.mdx`):

```
Read(src/**)   Exec(npm run test)   Write(**)   mcp__server__tool   exec   edit
```

**5 modos de permiso** (doc líneas 19‑39; constante en el binario
`NormalAccept EditsSmartAskBypassAutonomous`, `devin_cli_strings.txt` línea **446.057**):

`normal` (lecturas auto; escritura/shell piden; teclas `1` Once `2` Session
`3` Project `4` Always) · `accept-edits` · `smart` (un modelo rápido juzga) ·
`bypass` (todo auto) · `autonomous` (sólo con sandbox OS; `edit`/`write` siguen
pidiendo).

> El binario intercala además **`Ask`** (read-only) en esa lista, y la CLI expone
> *"Switch to Ask mode (read-only)"* como comando de modo. La tabla de la doc de
> permisos documenta 5 modos (`normal`/`accept-edits`/`smart`/`bypass`/`autonomous`) y
> trata `ask` como una **regla** de permiso, no como un modo. **Medido**: el producto
> tiene 6 etiquetas de modo; la doc tabla 5. Soportar `ask` como modo **y** como
> ámbito de permiso.

**Precedencia**: `deny`/`ask` de organización **no** los anulan `smart`/`bypass`/`autonomous`.
Permisos de skill **aditivos** a los de sesión.

### 4.g — Endpoints de red

Detalle completo en `artifacts/devin/contratos/endpoints/endpoints.md`.
Resumen:

| Función | Host | Origen |
|---|---|---|
| API del agente | `https://api.devin.ai` | `bin/devin` |
| Web app (share/planes/uso/outposts) | `https://app.devin.ai` | `bin/devin` |
| Updates Desktop | `https://windsurf-stable.codeium.com` | `product.json` |
| Manifest CLI | `https://static.devin.ai/cli/current/manifest.json` | `bin/devin` |
| Manifest CLI empresa | `https://static.devinenterprise.com/cli/current/manifest-ent…` | `bin/devin` |
| Servidor remoto | `https://static.devin.ai/devin-rs/remote` | `bin/devin` |
| Backend legacy | `https://server.codeium.com` (`WINDSURF_API_SERVER_URL`) | `bin/devin` |
| Feature flags | `https://unleash.codeium.com/api/background` | `bin/devin` |
| Errores | `https://codeium-i5.sentry.io` (DSN embebido) | `bin/devin` |
| Galería de extensiones | `https://marketplace.windsurf.com/vscode/gallery` | `product.json` |
| Esquema registro ACP | `https://cdn.agentclientprotocol.com/registry/v1/latest/registry.schema.json` | `package.json` |

**Telemetría**: Sentry (**se desactiva** si el servidor API no está en host
`codeium.com`/`windsurf.com`), OTLP/chisel-otel (`/v1/logs`, `/v1/metrics`,
collector por defecto `http://localhost:4318`), Unleash, y estado local en
`telemetry_state.json` (`chisel-api/src/telemetry/manager.rs`).

**Variables de entorno de red**: `WINDSURF_API_SERVER_URL`, `WINDSURF_API_KEY`
(con ella `/login` deja de existir), `WINDSURF_WEBSITE_URL`, `ACP_BACKEND`
(`windsurf` | `openai` para OpenRouter), `OPENAI_API_BASE`, `OPENAI_API_KEY`,
`USE_COMPLETIONS`, `GATEWAY_URL`, `DEVIN_WORKER_CACHE_DIR`.

### 4.h — Licencias, precios y condiciones

- `resources/app/LICENSE.txt` (833 B) — **Microsoft** (fork de VS Code).
- `resources/app/extensions/windsurf/LICENSE.txt` (783 B) — la extensión.
- `resources/app/ThirdPartyNotices.txt` (183.203 B) — avisos de terceros.
- Precios/condiciones: `https://app.devin.ai/plans` (enlazado desde la CLI:
  `windsurf.openBillingPage` → *"Purchase Add‑on Credits"*, `windsurf.openAutoRefillPage`
  → *"Enable Auto Refill"*). En el binario: `acu_consumption_at_last_user_interaction`
  (consumo de créditos), `subscription notification`.
- Enterprise: `…/docs/enterprise/{controls,devin-auth,windsurf-auth,system-config,team-settings}.mdx`
  — controles de organización, auth dual (**Devin auth** y **Windsurf auth** conviven),
  Team Settings (restricción de modelos, denegaciones a nivel org, sandbox obligatorio).

### 4.i — Protocolo de la CLI

`extensions/windsurf/devin/bin/devin` es una CLI Rust (clap 4.5.60) con **204 man
pages**. Árbol de comandos medido (`devin-help-*.1`):

```
devin
├── auth       login | logout | status
├── acp        (Agent Client Protocol)
├── cloud      drs { blueprint-create|list|write, build{,-logs,-start,-wait}, run,
│                    sandbox-create, secret-create, whoami }
├── daemon     start | stop | status
├── doctor
├── forward
├── mcp        add | list | get | remove | login | logout | enable | disable
├── migrate    hooks | workflows
├── models     list  (--format text|json)
├── plugins    install | list | info | update | remove | prune
├── rules      list | paths | show
├── sandbox    setup
├── setup
├── skills     list | paths | show
├── ssh
├── update
└── version
```

Flags globales medidos: `--process-model {daemon|in-process}` (dónde corren los
agentes locales), `-h/--help`.

**Salida JSON**: `devin mcp list --json` y `devin models list --format json`.
`devin rules list --provider {cursor|windsurf}` filtra por proveedor de reglas.

**Daemon**: `devin daemon start|stop|status` — "In the background daemon, shared by
`devin` commands and started if needed".

**Persistencia de sesión**: `session_store` con `type: "sqlite"` o `"files"`
(`config-file.mdx` / strings `SessionStoreSettingsqlitefiles`).
**NO MEDIDO**: el esquema de la tabla SQLite y el layout exacto de los ficheros.

### 4.j — Capa ACP (el contrato más profundo)

Fuente: `resources/app/extensions/windsurf/dist/acp/AGENTS.md` (31.656 B), que es
**documentación de arquitectura escrita por el propio equipo** (Cognition).

- **Protocolo ACP** (Agent Client Protocol) v1 y v2. Métodos observados:
  `initialize`, `session/new`, `session/load`, `session/resume`, `session/delete`,
  `session/cancel`, `session/set_mode`, `session/set_config_option`, `session/prompt`,
  `session/list`, `authenticate`.
- **Servidores MCP sobre ACP**: `McpServerHttp{url,headers}`,
  `McpServerSse`, `McpServerStdio{command,env,cwd}`.
- **Extensiones propias `cognition.ai/*`** (opt‑in mutuo en `initialize._meta`):
  `lazyRepoOptions`, `locale`, `statelessShellReplay`, `shellChainReads`,
  `queuedMessagesInfoUpdate`, `subagentSupport`, `subagentControl`, `multiRootWorkspace`,
  `partialContent`, `messageGrouping`, `wholeUserMessages`, `refTagsRaw`,
  `clientProvidedModels`, `groupedSessionConfigOptions`, `windsurfConfigBridge`,
  `revert`, `loadStats`, `externalHistorySkeleton`, `tailReplay`, `mcp`, `plugins`,
  `editorContext`, `requestDiagnostics`, `browserPreview`, `browserPreviewOpen`,
  `browserTab`, `cloudHandoff`, `cloudSetupSecretValues`, `cloudSetupMcpKeyValues`,
  `mcpAuthorizationRelay`, `sessionListFolders`, `sessionListPermissionScopes`,
  `fastContext`, `toolCallQuestions`, `questionAnswerRows`, `lazyEditorFiles`,
  `terminal_output`.
- **Identidad del cliente**: `clientInfo.name` = `"windsurf" | "windsurf-next" | "windsurf-insiders"`;
  `_meta["cognition.ai/clientEnvironment"]` = `{acpBackend, apiKey, apiServerUrl}`.
- **Remoto**: un proceso `windsurf agent acp-host` por máquina remota; `hostId` = alias
  SSH o `wsl+<distro>`; URI `devin-host://<hostId>/<path>`; versiones de servidor
  `~/.windsurf-server` (stable) / `~/.devin-server-insiders`.
- **Registro ACP**: `**/acp/registry.json`, validado contra el esquema público de
  `cdn.agentclientprotocol.com`.

> **Esto es el activo más valioso del paquete para MegaDaemon**: ACP es un estándar
> abierto y el producto demuestra una implementación completa y bien documentada.

---

## 5. Contrato → ¿copiable? → fichero destino en `rust-core/src/`

| Contrato | ¿Copiable? | Destino | Nota |
|---|---|---|---|
| Nombres de rutas de rules + frontmatter `trigger` | **Sí** | `rust-core/src/skills.rs` | Normalizar `model_decision` **y** `agent_decided` |
| `read_config_from` (10 proveedores) | **Sí** | `rust-core/src/skills.rs` | Tabla de importadores ya existente en Cursor |
| Formato `SKILL.md` (8 campos de frontmatter) | **Sí** | `rust-core/src/skills.rs` | `allowed-tools` es auto‑aprobación, **no** restricción → copiar la semántica |
| Permisos `allow/deny/ask` + 5 modos | **Sí** | `rust-core/src/tool_policy.rs` | Precedencia org > modo de usuario |
| Nombres de las 25 herramientas | **Sí** | `rust-core/src/tool_policy.rs` | Contrato de API con el modelo |
| Config MCP (3 ficheros, 3 scopes, 3 transports, OAuth) | **Sí** | `rust-core/src/protocol.rs` | Reutilizar el `mcpServers` de Cursor: **mismo formato de campo** |
| Hooks (8 eventos + `hookSpecificOutput`) | **Sí** | `rust-core/src/protocol.rs` | Formato casi idéntico a Claude Code |
| Árbol de comandos de la CLI | **Sí** (superficie) | `rust-core/src/protocol.rs` | No copiar roff |
| **ACP v1/v2** | **Sí** (es estándar abierto) | `rust-core/src/protocol.rs` | **P0**: el plan ya lo contempla |
| Capacidades `cognition.ai/*` | **No** (propietarias) | — | Sólo documentar |
| Prompt de sistema (texto) | **No** | — | Propiedad de Cognition |
| Descripciones de las skills de fábrica | **No** (texto) | `rust-core/src/skills.rs` (reescritura) | Copiar el *formato*, no el contenido |
| Hosts/URLs de endpoints | **No** | `rust-core/src/protocol.rs` (como constantes internas) | No redistribuir hosts ajenos |
| `product.json`, `LICENSE.txt`, `ThirdPartyNotices.txt` | **No** | — | Referencia |

**Módulos destino ya existentes**: `rust-core/src/protocol.rs` (12.316 B),
`rust-core/src/skills.rs` (16.960 B), `rust-core/src/tool_policy.rs` (13.705 B).

---

## 6. Diferencias con el bundle de Cursor (medido)

| Eje | Cursor 3.32.4 (`artifacts/cursor/`) | Devin Desktop 3.15.14 |
|---|---|---|
| Forma del paquete | `.deb` (230.561.828 B) | `.tar.gz` (432.763.567 B) |
| Extensions manifest | `contributes` + modos internos del producto (Ask/Agent/…) | `contributes`: 52 comandos, **sin bloque de modos de usuario**; los modos viajan por el **protocolo ACP** (`session/set_mode`) |
| Custom modes / subagentes | **`.cursor/agents/`** (nativo, `~/.cursor/agents` + watcher); proto `agent.v1.` con `name/description/tools[]/model/prompt/permission_mode/is_background` | **`.devin/skills/` + perfil `agent:`**; perfiles built-in `subagent_general` / `subagent_explore`; subagentes de plugin `agents/<n>.md` |
| Hooks | Format propio (docs `cursor.com/docs/approval-agents`) | **8 eventos** tipo Claude Code, `.devin/hooks.v1.json`, `matcher` **regex**, `updatedInput` para reescribir argumentos |
| MCP | Extensión propia `cursor-mcp` | `devin mcp add/list/...` + 3 ficheros de config; **importa MCP de Cursor, Windsurf, Claude, OpenCode y Zed** (`read_config_from`) |
| Backend | `https://api2.cursor.sh` (daemon local `agent-host-daemon`) | `https://api.devin.ai` + **CLI Rust completa** (`bin/devin`, 262 MB) con daemon propio (`devin daemon`) |
| Distribución de modelos | Catálogo propio de Cursor | Routing **Fusion** (lead + sidecar) + **Adaptive**; alias `opus`/`sonnet`/`swe`/`codex`/`gemini`; niveles de razonamiento (`thought_level`) |
| Protocolo de agente | ACP **parcial** | ACP **completo v1/v2** + 37 extensiones `cognition.ai/*` |
| Skills | `.cursor/skills/`, `.cursor/skills-cursor/`, `.agents/skills/` | `.devin/skills/`, `.windsurf/skills/`, `.agents/skills/`, `.claude/`, `.github/`, `.codex/`, `.kiro/` |
| Plugins | `.cursor/plugins/` + `cache/` | `.devin-plugin/plugin.json` (+ fallback `.claude-plugin/plugin.json` y raíz `plugin.json`); formatos **Claude** y **Agent Plugins 1.0.0** |
| Telemetría | Statsig | **Sentry** (con kill‑switch por host) + **OTLP/chisel-otel** + **Unleash** + `telemetry_state.json` |
| Empaquetado de docs | No | **Sí**: 42 `.mdx` + 204 man pages de su propia CLI |

**Conclusión:** Devin es **más interoperable** (importa de 9 herramientas ajenas) y
tiene una **superficie de contrato documentada y legible**; Cursor es más **opaco**
pero tiene los *custom modes* más explícitos como concepto de usuario.

---

## 7. Lo NO recuperable

| Elemento | Por qué |
|---|---|
| Prompts del **servidor** (system prompt remoto, prompts de modo, de subagente) | Viven en el backend; el paquete sólo trae el prompt local embebido |
| Contenido de skills/plans del **servidor** | Sólo los 14 `SKILL.md` de fábrica, locales |
| Catálogo de modelos **con ids reales y precios** | `devin models list` necesita cuenta; el doc sólo da alias (`opus`, `sonnet`, `swe`, `codex`, `gemini`) |
| Claves, tokens, secretos de la API | Sólo se ven los *mecanismos* (PKCE, device flow, `WINDSURF_API_KEY`) |
| Esquema de la BD SQLite de sesiones y layout de `~/.devin/` | `session_store: sqlite\|files` está medido; el layout no |
| Precios concretos y condiciones legales | Sólo la URL `app.devin.ai/plans` |
| Implementación del agente en Desktop | `out/vs/sessions/sessions.desktop.main.js` (45.847.148 B) está minificado; sólo se citan cadenas |
| `node_modules.asar` | **Desempaquetado completo** (10.347/10.347) en `artifacts/devin/devin_asar_unpacked/`; sin contratos del agente dentro |

---

## 8. Procedencia de las medidas

| Afirmación | Evidencia |
|---|---|
| Renombrado y versión | `artifacts/vendor/devin.json`; `resources/app/product.json`; `resources/app/package.json` |
| Integridad del paquete | sha256 medido == `sha256hash` de la API |
| Rutas de rules | `…/docs/extensibility/rules.mdx` líneas 98‑133 |
| Hooks | `…/docs/extensibility/hooks/lifecycle-hooks.mdx` líneas 16‑375 |
| MCP | `…/docs/extensibility/mcp/configuration.mdx` líneas 53‑210; `devin-mcp*.1` |
| Skills | `…/docs/extensibility/skills/creating-skills.mdx` líneas 42‑82; `out/vs/sessions/skills/` |
| Herramientas | `…/docs/extensibility/hooks/lifecycle-hooks.mdx` líneas 356‑375 |
| Permisos | `…/docs/reference/permissions.mdx` líneas 19‑39, 189‑308 |
| Config | `…/docs/reference/configuration/config-file.mdx` líneas 12‑21, 152‑529 |
| ACP | `extensions/windsurf/dist/acp/AGENTS.md` (íntegro) |
| Endpoints | `strings` de `…/devin/bin/devin`; `product.json` |
| Prompt de sistema | `…/devin/bin/devin`, offset 184.167.863, ancla `"You are Devin, a powerful agentic"` |
| Comandos CLI | `…/devin/share/man/man1/*.1` (204) |

---

## 9. Anexo — ficheros extraídos

- `artifacts/devin/INVENTARIO.tsv` — 7.013 filas (`ruta` TAB `bytes`), 1,23 GB.
- `artifacts/devin/contratos/` — 175 ficheros: docs por categoría, 204 man pages,
  las 14 skills de fábrica, ejemplos (`mcp_config.json`, `hooks.v1.json`,
  `plugin.json`, `config.json`, `SKILL.md.example`), catálogos de tools y endpoints,
  y `_tools/unpack_asar.py`.
- `artifacts/devin/sistema_prompt_raw.txt` — prompt de sistema íntegro (24.478 B).
- `artifacts/devin/devin_cli_strings.txt` — 495.587 líneas de `strings` del binario CLI.

> **Nota de redistribution:** todo lo anterior es material de Cognition / Microsoft /
> VS Code. **No se sube al repo público**: queda en `artifacts/` (sin trackear) y en la
> Drive privada. Al repo público sólo va este documento de análisis (rutas + hechos),
> no el texto de terceros.

## 10. Anexo — procedencia en la Drive privada

Carpeta `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`:

| Fichero | `file_id` | Bytes |
|---|---|---|
| `devin_contratos_2026-11-27.zip` (`contratos/` + `INVENTARIO.tsv` + `asar_manifest.tsv` + prompt del sistema) | `1KBCbucp6Af-2aPVxk4z99s6MwIZjYBpJ` | 11.869.751 |
| `RE_DEVIN_DESKTOP_CONTRATOS_2026-11-27.md` | este mismo documento, subido a la misma carpeta | — |

`asar_manifest.tsv` = 10.347 entradas (fichero ⇄ bytes del `.asar` desempaquetado,
221 MB en `artifacts/devin/devin_asar_unpacked/`).

En el repo: rama `re/productos-cierre-2026-11-27`, commits `dfb83e0` (el informe) y
`ceaeb8a` (corrección del anexo). Sólo se versiona este documento; el resto del
material de terceros queda fuera del repo.

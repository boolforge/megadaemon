# Ingeniería inversa del binario más nuevo de Claude Code (2.1.331) — y qué se lleva MegaDaemon

> Fecha: 2026-11-24. Todo lo de aquí está **medido** sobre los ficheros
> descargados en esta sesión; nada procede de memoria. Los comandos exactos
> están en §2 para que se pueda repetir.
>
> Encargo: «descomprime el último binario y deofúsca, y saca todo lo que
> serves de todas las versiones de codex, claude y otros sistemas de agentes
> para aprender y potenciar MegaDaemon».

---

## 1. Resultado en una línea

El paquete de npm **ya no publica código fuente**: el `2.1.331` son 32 KB de
un envoltorio que **descarga un binario nativo de 398 MB** (Bun compilado a
ELF). Ese binario **sí** se puede abrir: las *prompts*, los *manifiestos* y
las *reglas* están dentro como cadenas legibles. De ahí sale el trabajo real
que se ha hecho: **el formato exacto de las skills**, el **mecanismo de
«session goal / stop check»** (nuevo) y la lista de **ficheros de plugin**
que el producto reconoce (`.lsp.json` incluido).

## 2. Método (repetible)

```bash
# 1) el paquete "de la CLI" (32 KB): sólo el lanzador
curl -sL https://registry.npmjs.org/@anthropic-ai/claude-code/2.1.331 | jq -r .dist.tarball
tar -xzf claude-code-2.1.331.tgz        # cli-wrapper.cjs + install.cjs
# el wrapper dice: "the postinstall script copies the native binary over bin/claude.exe"

# 2) el binario de verdad: un paquete opcional por plataforma
curl -sL https://registry.npmjs.org/@anthropic-ai/claude-code-linux-x64/2.1.331 | jq -r .dist.tarball
tar -xzf claude-code-linux-x64-2.1.331.tgz   # -> package/claude  (398.622.992 bytes)

file package/claude
# ELF 64-bit LSB executable, x86-64, dynamically linked, not stripped
```

Puntos de anclaje dentro del binario (offsets absolutos, medidos con `mmap`):

| marcador | offset | qué es |
|---|---|---|
| `// @bun` | 2.412.541 / 5.048.296 / 9.479.032 | el runtime de Bun compilado (JS del runtime, no el producto) |
| `You are Claude Code…` | 168.804.696 | **copia ASCII** de las cadenas del prompt |
| la misma frase en UTF-16LE | 168.799.xxx | la **tabla de cadenas** del bytecode |
| `EnterPlanMode`, `WebFetch`, `TaskCreate` | 167.8M–172.2M | definiciones de herramientas |
| `CLAUDE_CODE_ENTRYPOINT` | 167.627.416 | el punto de entrada |

⇒ **El bundle del producto NO está como JS plano**: Bun lo compila a bytecode
y guarda los literales en una tabla **UTF-16LE** (por eso `strings` los ve
como `Y\x00o\x00u\x00…`). Lo que **sí** queda en texto plano son: el runtime
de Bun, el código de la interfaz (React/Ink) y **los textos de las prompts y
los manifiestos**.

Truco para leer la tabla de cadenas: buscar la cadena en UTF-16LE y decodificar
el bloque contiguo (los literales van seguidos, separados por bytes de
longitud).

## 3. Qué reconoce el producto (medido en el binario)

Cadenas de nombres de fichero recuperadas del binario (offsets 168.462.344 y
168.482.264):

| nombre | qué es |
|---|---|
| `.claude-plugin/plugin.json` | el manifiesto de un plugin |
| `marketplace.json` | el índice de plugins instalables |
| `SKILL.md` | la skill (frontmatter + instrucciones) |
| `.mcp.json` | servidores MCP que declara el plugin |
| **`.lsp.json`** | **configuración de LSP por plugin** (no estaba en 2.1.88) |
| `output-styles` | estilos de salida (otro formato de instrucciones) |
| `monitors` | tareas de vigilancia programada |

⇒ El **modelo de extension** es: un plugin = un manifiesto + `skills/` +
`.mcp.json` (+ `.lsp.json`); una skill = una carpeta con `SKILL.md`.

Variables de entorno que el binario lee (medido): `DEVELOPER_DIR`,
`GIT_TEXTDOMAINDIR`, `LD_PRELOAD`, `LD_LIBRARY_PATH`, `DYLD_INSERT_LIBRARIES`,
`DYLD_LIBRARY_PATH`, `DYLD_FRAMEWORK_PATH`, `OPENSSL_CONF_INCLUDE`,
`NODE_EXTRA_CA_CERTS` ⇒ es el **sandbox** (se prepara un proceso hijo con su
propio `LD_*` y su configuración de OpenSSL) y el binario además trae
`monitor`/`.bashrc`/`.gitconfig` por plugin.

## 4. Novedades de 2.1.331 frente a 2.1.88 (medido por presencia de cadenas)

| señal | 2.1.331 | para qué le sirve a MegaDaemon |
|---|---|---|
| **`session goal` / `stop check`** | sí (168.805.174) — texto literal: *«Set a session goal, a stop check that keeps you working until it is met…»* | **el agente tiene un objetivo de sesión y una condición de parada**: el bucle sigue trabajando hasta cumplirlo. MegaDaemon's `agent_loop` no tiene ni una cosa ni otra ⇒ hueco nuevo (**H13**) |
| `checkpoint` / `rewind` | sí (167.948.372 / 168.039.706) | puntos de retorno del agente |
| `compaction` | sí (168.037.896) | compactación del contexto |
| `plan mode` | sí (167.844.812) | proponer plan y esperar |
| `SKILL.md` + `frontmatter` + `allowed-tools` | sí | el contrato de skills |
| `.claude-plugin/plugin.json` | sí | plugins |
| `output style` | sí | formatos de respuesta configurables |
| `sandbox` / `permission` | sí | política de ejecución |
| `worktree` | sí | aislamiento del trabajo |

## 5. El formato real de las skills (medido en la fuente de 2.1.88)

De `src/skills/loadSkillsDir.ts` y `src/skills/bundledSkills.ts`
(`AprilNEA/claude-code-source`):

* Campos: `name`, `description`, `aliases?`, `whenToUse?`, `argumentHint?`,
  `allowedTools?`, `model?`, `disableModelInvocation?`, `userInvocable?`,
  `hooks?`, **`context?: 'inline' | 'fork'`**, `agent?`, `files?`.
* `files`: ficheros de referencia que se extraen al disco la primera vez; el
  prompt se prefixes con **«Base directory for this skill: <dir>»** para que
  el modelo los lea bajo demanda (no dentro del contexto).
* Descubrimiento: varios directorios (personal, del proyecto y extra),
  deduplicación por nombre, respeto de `.gitignore` y estimación de tokens
  (`roughTokenCountEstimation`).
* El frontmatter se parsea con utilidades propias
  (`parseFrontmatter`, `parseShellFrontmatter`, `parseBooleanFrontmatter`,
  `splitPathInFrontmatter`, `coerceDescriptionToString`) ⇒ admite escalares,
  booleanos, listas y valores tipo shell.

## 6. Lo que se ha implementado en MegaDaemon con esto

`rust-core/src/skills.rs` (nuevo, **sin dependencias**: sólo `std`) + los
tests dentro del propio módulo (**9 tests, ejecutados y verdes**):

* `partir_frontmatter` / `parsear_frontmatter`: el subconjunto YAML real
  (`clave: valor`, `clave: [a, b]`, comillas, comentarios) **sin** añadir un
  parser de YAML al crate.
* `cargar_fichero`: `name` y `description` **obligatorios**; el resto
  opcional; `allowed-tools`, `model`, `context`, `argument-hint`,
  `disable-model-invocation`, `license`, `version`.
* `cargar_directorio` / `descubrir`: varios raíz, **deduplicación por nombre**
  (gana el primero), y **los errores no rompen la carga**: una skill rota da
  un error legible y las demás se cargan igual.
* `linea_directorio_base()`: la convención medida, para que el agente lea
  los ficheros de referencia de la skill bajo demanda.

Verificación: compilado y ejecutado **fuera del CI** (el módulo sólo usa
`std`), 9/9 tests en verde, antes de subirlo.

## 7. Lo que queda para completar el P1 (plugins/skills)

1. Exponer el catálogo de skills por MCP (`skills_list`) ⇒ para eso hay que
   tocar `mcp_ext.rs`, que es de otro dueño: tocarlo con cuidado.
2. Leer `.claude-plugin/plugin.json` y `.mcp.json` de un directorio de
   plugins ⇒ arrancar servidores MCP declarados por un plugin (con la
   política de ejecución, P4, aplicada ANTES).
3. Conectar `context: fork` con el subagente que ya tiene MegaDaemon
   (`agent_coding.rs` / `delegate.rs`).

## 8. Trazabilidad

| afirmación | de dónde sale |
|---|---|
| el paquete más nuevo son 32 KB y sólo descarga un binario | `registry.npmjs.org/@anthropic-ai/claude-code/2.1.331` (descargado) |
| el binario pesa 398.622.992 B y es un ELF de Bun | `package/claude` del paquete `linux-x64` (descargado) |
| offsets de los marcadores | barrido con `mmap` sobre el binario |
| nombres de fichero de plugin/skill | cadenas recuperadas del binario (168.462.344, 168.482.264) |
| «session goal / stop check» | cadena ASCII en 168.805.174 |
| campos de las skills | `src/skills/loadSkillsDir.ts` y `bundledSkills.ts` de `AprilNEA/claude-code-source` (2.1.88) |
| implementación en el daemon | `rust-core/src/skills.rs` (9 tests) |
| contexto de la investigación | `docs/AGENTIC_ARCHITECTURE.md` |

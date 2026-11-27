# CURSOR 3.32.4 — CONTRATOS DEL PRODUCTO (extracción para MegaDaemon)

> Informe índice. El desarrollo completo vive en `docs/RE_CURSOR_CONTRATOS.md` (repo) y en
> `contratos/` (esta carpeta), con un fichero por contrato y rutas:líneas exactas.

**Paquete:** `cursor_3.32.4_amd64.deb` (230 561 828 B) →
`https://downloads.cursor.com/production/0925c4b8226c56947c43f0b555fb98e187a7e029/linux/x64/deb/amd64/deb/cursor_3.32.4_amd64.deb`
**Build:** `2026-11-24T06:34:58.000Z`, commit `0925c4b82…`
**Extracción:** solo lectura. Ningún binario de Cursor se ha ejecutado.
**Formato:** `app` es un **directorio**, no un `app.asar`. No había nada que desempaquetar.

---

## Resumen de los 9 contratos

| # | Contrato | Fichero de detalle | Estado |
|---|---|---|---|
| 1 | Manifiesto de extensiones y puntos de entrada | `contratos/01_manifiesto_extensiones.md` | MEDIDO (22 extensiones) |
| 2 | Agent rules · skills · custom modes | `contratos/02_rules_skills_modes.md` | MEDIDO |
| 3 | **Hooks** (22 eventos + compat Claude Code) | `contratos/03_hooks.md` | MEDIDO |
| 4 | **MCP** (`mcp.json`, `placement`, allowlists) | `contratos/04_mcp.md` | MEDIDO |
| 5 | Catálogo de tools (73 nombres) | `contratos/05_tools.md` | MEDIDO (nombres) / NO MEDIDO (esquemas) |
| 6 | Contexto (`RequestContext`, scopes) | `contratos/06_endpoints_contexto.md` §6 | MEDIDO |
| 7 | Endpoints · auth · telemetría · red | `contratos/06_endpoints_contexto.md` | MEDIDO / parcial |
| 8 | Licencia · precios · condiciones | `contratos/08_licencia_precios.md` | MEDIDO (licencia) / NO MEDIDO (precios) |
| 9 | **Agent host daemon**: JSON-RPC 2.0 + MCP stateless | `contratos/09_daemon_protocolo.md` | MEDIDO |
| — | CLI `cursor-agent` y su salida JSON | — | **NO MEDIDO**: el binario no está en el `.deb` |

---

## Las 5 piezas que MegaDaemon debería copiar

1. **`hooks.json`** — `{version:1, hooks:{<evento>:[{command,args,timeout,…}]}}`, **22 eventos**,
   y una política de decisión **deny > ask > allow** con respuestas JSON por stdout.
   → `rust-core/src/protocol.rs` + `rust-core/src/tool_policy.rs`
2. **`CustomSubagent`** — el "custom mode" con 14 campos: `name`, `description`, `tools[]`,
   `model`, `prompt`, `permission_mode` (`DEFAULT|READONLY|AGENT_ONLY`), `is_background`,
   `force_default_model`.
   → `rust-core/src/protocol.rs` (+ `agent_modes.rs` nuevo)
3. **El daemon como servidor MCP stateless sobre JSON-RPC 2.0**, con **deadline en el mensaje**
   (`deadlineEpochSeconds`), **presupuestos de reintento** por tipo de fallo
   (`serverCap:3`/`transportCap:10`, `RETRY_BUDGET_BY_KIND`) y **tareas asíncronas** (`tasks/*`).
   → `rust-core/src/protocol.rs` + `rust-core/src/mcp_server.rs` (+ `retry.rs` nuevo)
4. **Lectura multi-ecosistema de skills**: `.claude/skills`, `.codex/skills`, `.grok/skills`,
   `.agents/skills` **y** `.cursor/skills`. `.agents/skills` es el estándar neutral.
   → `rust-core/src/skills.rs`
5. **`mcp.json`** con `type: stdio|http|sse` + `placement: server|client` +
   `symbolic_path_arguments` + allowlists (`mcpAllowlist`, `terminalAllowlist`,
   `approvalMode`, `autoRun.allow/block_instructions`).
   → `rust-core/src/mcp_ext.rs` + `rust-core/src/tool_policy.rs`

**Extra copiable:** convenciones OTel GenAI (`ai.*`, `gen_ai.*`), el validador de URL MCP
(host/IP/CIDR/wildcards, máx 2048), el patrón `environment-file` de consume-and-destroy de
secretos (0600, 1 link, anti-TOCTOU), y el error JSON-RPC `-32042 UrlElicitationRequired`.

---

## ⚠️ Aviso legal (medido en `LICENSE.txt` §1.2–1.3)

> *"you may not: (a) copy, modify, adapt, translate, prepare derivative works from, reverse
> engineer, disassemble, or decompile the Software …; (b) … distribute the Software to any third
> party"*

| Actividad | Veredicto |
|---|---|
| Analizar y reimplementar **formatos** (hooks.json, mcp.json, frontmatter) | OK en principio (hechos de interfaz, no expresión) |
| Copiar **código** de los bundles al repo | **NO** |
| Redistribuir los ficheros de `contratos/ficheros/` | **NO** → mantener en artefactos privados / Drive |

**Pendiente de decisión del root:** si `docs/RE_CURSOR_CONTRATOS.md` (con citas literales) puede
vivir en el repo público. Recomendación: subir `artifacts/cursor/` a la Drive privada
`15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0` (`tools/drive_push.py`) y dejar en el repo solo el
documento con las citas reducidas a hechos de formato.

---

## Contenido de esta carpeta

| Ruta | Qué |
|---|---|
| `INVENTARIO.tsv` | 12 971 ficheros del paquete con su tamaño (ruta, bytes) |
| `contratos/01…09_*.md` | Un informe por contrato, con `fichero:línea` y citas literales |
| `contratos/ficheros/` | Ficheros **verbatim** extraídos (schemas JSON, `.d.ts`, entrypoints) |
| `contratos/_raw/` | Datos crudos del harvest (URLs, rutas `.cursor/`) |
| `_tools/harvest.py` | Script reproducible de extracción de literales |

**Ficheros verbatim extraídos** (`contratos/ficheros/`):
`permissions.schema.json` (1839 B) · `environment.schema.json` (8909 B) ·
`backend-endpoint.js` (5456 B) + `.d.ts` (2566 B) ·
`environment-file.js` (10073 B) + `.d.ts` (3369 B) ·
`daemon.js` (1418 B) · `build-info.json` (189 B).

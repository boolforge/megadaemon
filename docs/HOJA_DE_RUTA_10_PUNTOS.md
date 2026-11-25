# Hoja de ruta: los 10 puntos de mejora pedidos por el usuario

**Fecha:** 2026-11-25 · **Regla:** cada fila dice lo que **existe y está medido**
(módulo y líneas) y lo que **falta**. Nada de "fantasmadas": si no está
medido, no se afirma.

El móvil es la instancia maestra. Por eso **el primer bloqueo de casi todo es
el mismo**: el móvil no es alcanzable desde el sandbox (el túnel de Cloudflare
caduca a la hora y su DNS deja de resolver; ADB resetea). Estado medido el
2026-11-25:

- Tablón de Drive (`tools/tablon_ls.py`): **una sola carta**, latido del nodo
  `samsung`, build `d7d72f1`, epoch 1, del **2026-11-23 10:22** ⇒ el móvil
  lleva sin anunciarse.
- Última URL conocida: `dialog-photography-apparent-replies.trycloudflare.com`
  ⇒ **NXDOMAIN** (el túnel caducó).

Por eso la hoja está ordenada por **desbloqueo**, no por tamaño.

---

## Fase 0 — Desbloqueo (sin esto, nada más se puede medir)

| # | Punto | Estado medido | Qué falta |
|---|---|---|---|
| 0.1 | Que el móvil se levante solo | `deploy/99-megadaemon.sh` (supervisor con auto-rollback), `deploy/97-megadaemon-keeper.sh` (guardián del guardián), `rust-core/src/supervisor.rs` | El keeper sólo vigila al **supervisor**: si el supervisor vive pero el daemon no, nadie lo relanza. Falta: comprobar daemon + túnel, y publicar la URL nueva en el tablón |
| 0.2 | Recuperar el móvil sin PC ni ADB | `tools/recuperar-movil-v2.sh` (11 KB, bash -n OK) con 5 vectores | Falta probarlo de verdad: el diagnóstico medido dice "ADB no responde en ningún puerto" y "túnel → HTTP 000" |
| 0.3 | Que el móvil se autoactualice sin bucles | `rust-core/src/self_update.rs`: SHA-256 real, instalación atómica, `.prev`, **auto-rollback del supervisor** y **lista de binario malo** | Falta: ejecutar en el móvil (no accesible) |
| 0.4 | Pubblicar el estado real | `rust-core/src/devlog.rs`, `rust-core/src/logs.rs` (14 fuentes), Web UI V2 (19 vistas) | Falta: la UI se re-verifica cuando el móvil vuelva |

## Fase 1 — Antigravity / DB / MITM (pedido explícito: "100 % funcionales")

| Elemento | Estado medido |
|---|---|
| Orquestador de instrumentación | `rust-core/src/instrumentacion_agy.rs` — estado real por pieza, plan, apply, restore, tool MCP |
| MITM de SQLite | `rust-core/src/sqlite_mitm.rs` — verificado **de punta a punta en el móvil**: bind-mount real, 32 conversaciones, replicación, ocultación, desactivación con el original intacto |
| Shim de red | `rust-core/src/agyshim.rs` — el LS pide `/v1internal/generateContent` (con barra) y el shim lo sirve; medido **HTTP 200** con respuesta Gemini real |
| Modelo detrás de MegaDaemon | `antigravity_state.pbtxt` con `model_name: "http://127.0.0.1:9090"`, `chat_model_name: megadaemon` |
| 403 del gateway | Arreglado: URL parcheada con secreto local `/mda/<15 hex>` (**41 bytes exactos**), fichero 0600, exención del gate de token. Medido: `unauthorized` = **0** |
| Instrumentación automática y total | `antigravity_fake_so.rs`, `antigravity_intercept.rs` (plan de hooks con pclntab real), `hooks_payload.rs` (payload embebido) |
| **Falta** | Verificar en el móvil **con el build nuevo** (el de ahora no se ha podido medir desde fuera) |

## Fase 2 — Ingeniería inversa de los agentes que nombraste

Estado real: `rust-core/src/agent_coding.rs` (2.763 líneas: sesiones, formato
OpenAI, tools, compactación), `rust-core/src/agent_adapters.rs` (162),
`rust-core/src/meta_agent.rs` (323). `docs/GO_REVERSE_ENGINEERING.md` es la
investigación de Go ya hecha.

**Lo que NO existe todavía** (medido por búsqueda en el repo): no hay RE
específica del formato de sesión, del protocolo de herramientas ni del
transporte de **Codex, Claude Code, Minimax Agent, Límites Agent, Open Code
y DeepSeek Code**. Lo que hay es un agente genérico con formato OpenAI.

Plan por agente (mismo método para todos: `re_strenc`/`re_engine` sobre el
binario ⇒ formato de sesión + lista de tools + endpoint):
1. Inventario de binarios y versiones (¿qué hay en el móvil? `device_inventory`).
2. Extraer el **formato de sesión** (JSONL/JSON) ⇒ poder leer y continuar
   sesiones ajenas.
3. Extraer el **catálogo de tools** ⇒ richer tool-calling para MegaDaemon.
4. Un **adaptador** por agente en `agent_adapters.rs`, con test de ida y vuelta.

## Fase 3 — Hardware, ROM, kernel

| Punto | Estado medido | Qué falta |
|---|---|---|
| ROM / UN1CA | `rom_locator.rs` (193), `rom_migration.rs` (2.676), `migration_plan.rs` | Falta: migración **transparente** ( hoy hay plan; falta aplicarlo sin perder datos ) |
| Kernel / bloques | `kernel_and_blocks.rs` (1.503) | Falta: kernel propio para UN1CA |
| Hardware del móvil | `hw_mapper.rs` (203) — es un mapa, no un accionador | Falta: lectura real de registros, NPU, DRAM |
| NPU | `npu_nnapi.rs`, `npu_cv.rs`, `npu_vs4l.rs`, `npu_bridge.rs` | Lo más maduro de la lista |
| GPU / drivers | `accelerator_policy.rs` = **routing de LLM** a CPU/NPU/GPU (NO son drivers) | Falta: drivers GPU |
| Consumo y cargador | `power_policy.rs`, `energy_automation.rs`, `battery_advance.rs`, `battery_saver.rs` | Falta: detección real del cargador (medido: este kernel **no** expone `charge_control_end_threshold`; se registra como NO SOPORTADO) |
| Eficiencia del sistema | `oom_score_adj` −800/−900, topes de malloc, RSS adaptativo | Medido y funcionando |

## Fase 4 — Root, Magisk, LSPosed

| Punto | Estado medido |
|---|---|
| Magisk | `magisk_module.rs` (1.712): list/enable/disable/install, Shamiko |
| Ocultar root | `root_hide.rs` (1.774), `root_occlusion.rs` (1.342), `root_stealth.rs`, `root_intel.rs` — banco de pruebas `rootcheck` con veredicto y evidencia |
| **LSPosed** | **NO existe**: la única mención es `SHAMIKO_REPO` (LSPosed/Shamiko) en `magisk_module.rs:1342`. Falta: módulo LSPosed (hooks Java), que es lo que permite tocar apps como BBVA |

## Fase 5 — Sustitución de apps y GUIs

| Punto | Estado medido |
|---|---|
| Sustituir apps por MegaDaemon | `pkgmgr.rs` (3.739), `backup_apps.rs`, `md-dict`, `md-clipboard` (CLIs ARM64 ya publicados) |
| Diccionario por idioma | `dict_sync.rs` (842) |
| Portapapeles + sync | `clipboard.rs` (1.238), `sync.rs`, `kdeconnect.rs`, `sync_chats.rs`, `sync_web.rs` |
| GUIs tipo gpui-mobile | Hoy: `webui.rs` (5.995 líneas, 19 vistas V2 servidas en el móvil). Falta: GUIs **nativas** ultra-eficientes |

## Fase 6 — Pantalla

Hoy sólo se **captura** el estado (`device_inventory.rs:173`:
`wm size; wm density; getprop ro.sf.lcd_density`, y `densidad`/`lcd_density_prop`
en el informe). **No existe** un módulo de adaptación de pantalla (escalar la
UI al panel del móvil, densidad, rotación). Es un módulo nuevo, de unas 400
líneas, con el mismo patrón que `config_efectiva` + endpoint `/api/v2/*`.

---

## Reglas que esta hoja hereda del proyecto

1. **Nada de fantasmadas**: cada afirmación con fichero:línea, test o medición.
2. Un solo actor compila: `tools/build.sh` + `tools/guardia-compilacion.sh`.
3. Los agentes entregan código (rama + `tools/agente-entrega.sh`); el root
   compila, testea, mergea y pushea a `main`.
4. Los merges se resuelven **por unión**, nunca con `-X ours`.
5. El CI es el verificador de compilación (`tools/ci-suite.sh` +
   `tools/ci-known-red.txt`).
6. El móvil manda: nada se da por bueno sin medirlo **en el móvil**.

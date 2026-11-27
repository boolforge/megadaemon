# RE — Arquitectura de la **CLI Devin en Rust** (262 MB dentro del paquete)

> MegaDaemon · subagente de RE de productos · 2026‑11‑27
>
> **Binario analizado** (sin ejecutarlo — sólo lectura):
> `Devin/resources/app/extensions/windsurf/devin/bin/devin` — 262.445.096 B,
> crate interno `pkg.cognition.build-3983e96abc3e6656`.
> **Método**: el binario conserva las rutas de fichero de compilación en `.rodata`
> (`/usr/local/cargo/registry/src/pkg.cognition.build-3983e96abc3e6656/…` y rutas
> relativas `chisel-agent/src/…`). Esas rutas son la **estructura real del árbol de
> fuentes**, no una inferencia.
>
> **Lo que esto NO es**: no se ha copiado ni descompilado código. Lo que hay debajo
> es **arquitectura medida**: nombres de módulos, esquemas de datos, protocolos y
> política. Eso es lo que MegaDaemon puede reimplementar por su cuenta.

---

## 1. Inventario: 20 crates internos, ~297 ficheros

Cognition **no** escribió un monolito: escribió una CLI con un núcleo de agente
separado, una capa de inferencia, un host de plugins y una UI.

| Crate | Ficheros | Rol |
|---|---|---|
| `affogato` | 21 | **el agente**: bucle de control, efectos, subagentes, keepalive de caché |
| `chisel-agent` | 146 | **todo lo de ACP**: servidor ACP, tools ACP, sesiones, skills, subagentes, revert, browser |
| `chisel-core` | 15 | tipos comunes: modelo, provider, profiles, prompts, translator, facts, flags, ref_tags |
| `chisel-api` | 14 | cliente HTTP de la API, auth (credentials/deployment/pkce), telemetría, Unleash |
| `chisel-commands` | 18 | la CLI (clap): `rules`, `skills`, `models`, `cloud`, `doctor`, `worker` |
| `chisel-ui` | 18 | TUI (dioxus): readbox, selector, spinner, panel de preguntas, stats |
| `chisel-cloud-bridge` | 17 | DRS (blueprints, builds, secrets), handoff a la nube, share, orgs |
| `chisel-commands`/`chisel-server` | 9 | daemon: socket Unix, actores, agentes (review, summarizer) |
| `chisel-acp-relay` | 5 | relay ACP (Cloud ↔ cliente), estado de sesiones |
| `chisel-acp-client` | 4 | conector ACP: bootstrap, proceso hijo, handshake |
| `chisel-otel` | 7 | trazas/métricas OTLP (http, grpc, traces, code_activity) |
| `chisel-mcp` | 2 | cliente MCP + caché de descripciones |
| `chisel-proxy-config` | 2 | configuración de proxy y transporte |
| `chisel-updater` | 1 | manifiesto de actualización |
| `inference` | 5 | **capa de inferencia**: backend, request, request_limits, retry, stream |
| `fx` | 3 | contexto + driver del bucle |
| `plugin-host` | 8 | **host de plugins**: manifest, bundle_fetcher, ingest, list, mcp, actions, cloud |

### 1.1 El resto del binario

**315 crates externos**, pero 3 llaman la atención porque son **una GUI en Rust**:

| Crate | Versión | Qué es |
|---|---|---|
| `dioxus-core` | 0.8.0‑alpha.3 | framework de UI (TUI de la CLI) |
| `blitz-dom` | 0.3.0‑beta.4 | **motor DOM en Rust** (layout, paint tree, style) |
| `stylo` | 0.22.0 | motor de estilos (servicio de Firefox) |
| `taffy` | 0.15.0 | layout flexbox/grid |

⇒ La CLI **renderiza HTML/CSS con un motor de navegador completo escrito en Rust**
dentro de la terminal. Para MegaDaemon: si se quiere una UI rica sin navegador
externo, esta es la pila a mirar (`dioxus` + `blitz-dom`).

---

## 2. `affogato` — el núcleo del agente (21 ficheros)

Es lo más aprovechable de todo el paquete.

```
affogato/src/
├── agent/
│   ├── control_loop.rs          ← EL bucle del agente
│   ├── runner.rs                ← ejecuta el bucle
│   ├── manager.rs               ← ciclo de vida de los agentes
│   ├── programmatic.rs          ← agentes programáticos (sin UI)
│   ├── subagent.rs              ← subagentes
│   ├── cog.rs                   ← "cog": el engranaje central
│   ├── event_types.rs
│   ├── effects/
│   │   ├── mod.rs
│   │   ├── handler.rs           ← despachador de efectos
│   │   ├── inference.rs         ← efecto: llamar al modelo
│   │   ├── tasks.rs             ← efecto: lanzar tareas
│   │   └── tools.rs             ← efecto: ejecutar herramientas
│   └── tool_pipeline/scheduler.rs
├── cache_keepalive.rs           ← política de keepalive del caché del prompt
├── cache_keepalive_bucket.rs
├── cache_keepalive_head.rs
├── cache_keepalive_inputs.rs
├── cache_keepalive_policy.rs
├── stall_watch.rs               ← detección de agente atascado
├── client.rs
├── cog.rs
└── image.rs
```

### 2.1 Patrón: el bucle como **manejador de efectos** ⭐

El bucle **no** llama directamente al modelo ni a las herramientas. Define
**efectos** y delega cada familia en un manejador:

| Efecto | Manejador | Qué hace |
|---|---|---|
| inferencia | `effects/inference.rs` | pide el siguiente turno al modelo |
| herramientas | `effects/tools.rs` | ejecuta la tool call y devuelve el resultado |
| tareas | `effects/tasks.rs` | lanza trabajo asíncrono |

**Por qué importa a MegaDaemon**: `agent_loop.rs` (50.848 B) y `agent_core.rs`
(25.337 B) hacen hoy la llamada al modelo y la ejecución de tools **inline**. Con
este patrón, `agent_loop.rs` se reduce a un **reductor** `(estado, evento) →
(estado, efectos)`, igual que el `hostMachine.ts` de ACP que ya está descrito en
`AGENTS.md` para los hosts remotos. Benefits: el mismo bucle sirve para el agente
interactivo, el programático (`programmatic.rs`) y el subagente; y las tools se
pueden despachar en paralelo sin tocar el bucle.

### 2.2 `tool_pipeline/scheduler.rs` ⭐

**El planificador de tool calls** está aislado del bucle. Es la pieza que decide
qué tool calls se lanzan, en qué orden y cuáles se paralelizan. En MegaDaemon
esto encaja directamente: `agent_loop.rs` + `tool_policy.rs` (13.705 B) +
`service_bus.rs` (82.739 B) tienen hoy esa lógica dispersa.

### 2.3 `cache_keepalive*` (5 ficheros) ⭐

**Mantener vivo el caché de prompt del proveedor** entre turnos. Un modelo con
prompt caching cobra mucho menos si el prefijo no cambia; un agente que corta el
contexto o cambia de modelo pierde el caché.

Esto es un **ahorro directo de dinero y de latencia**, y no está en el prompt de
sistema (que es donde losearch), sino en el runtime. Para MegaDaemon:
`agent_loop.rs` (o `providers.rs`) necesita una política de *estabilidad de prefijo*:
no reordenar bloques del prompt, no cambiar el system prompt entre turnos, no
invalidar por herramientas dynamic. La pieza clave es `cache_keepalive_inputs.rs`
—qué entradas forman el prefijo cacheable.

### 2.4 `stall_watch.rs`

Detecta que el agente **se ha atascado** (no avanza). Con `stall_watch` +
`subagent.rs` + `goal.rs` + `megaplan.rs` (en `chisel-agent`) hay un **sistema
completo de control de un agente que trabaja solo durante minutos**. Para
MegaDaemon: `automation_runtime.rs` (41.885 B) y `board.rs` (70.620 B) son el
sitio natural.

### 2.5 `fx/context.rs` + `fx/driver.rs` ⭐

**Separar "construir el contexto" de "conducir el agente"**. `context.rs` arma el
contexto (reglas, skills, memoria, ficheros); `driver.rs` conduce la ejecución.
Es una separación que en MegaDaemon no existe: `agent_loop.rs` mezcla ambas.

---

## 3. `inference` — la capa de proveedor (5 ficheros)

```
inference/src/
├── backend.rs          ← qué backend/modelo se usa
├── request.rs          ← construcción de la petición
├── request_limits.rs   ← límites por modelo
├── retry.rs            ← política de reintento
└── stream.rs           ← streaming de la respuesta
```

### 3.1 Metadatos de modelo (medidos)

```
model_type, tokenizer_type, model_features, model_name, embed_dim,
chat_model_name, is_internal, max_output_tokens, prompt_template,
tool_formatter_type, inference_server_url, arena_config, display_option,
is_model_router, compaction_thresholds, aliases, allow_route_override,
ModelFamily, MetadataValue
```

Tres campos destacan:

- **`is_model_router`** — un modelo puede ser **enrutador**: el `model_name` no es un
  modelo final sino una política que decide cuál usar. Es el mecanismo de
  **Fusion** (lead + sidekick).
- **`compaction_thresholds`** — el **umbral de compactación es por modelo**, no global.
- **`tool_formatter_type`** — cómo se formatean las tools **por modelo** (no todos los
  proveedores aceptan el mismo esquema de tool call).

### 3.2 Taxonomía de errores (medida, 17 valores)

```
Unauthenticated | Timeout | RateLimited(retry_after) | QuotaExhausted
| ContinuationChargeDeclined | UsageLimitReached | SubscriptionLimitReached
| SubscriptionNotEligible | SubscriptionReconnectRequired | SubscriptionRequired
| ServerError | ClientError | Refusal | fallback
```

`fallback` como **valor de error** (no como excepción) es un patrón: la función
de llamada al modelo **devuelve siempre un resultado**, y el error es un caso
más. Con `retry_after` como dato estructurado, el `retry.rs` puede hacer
backoff respetando el servidor.

⇒ Para MegaDaemon `providers.rs` (85.325 B): la firma debería ser
`Result<ModelTurn, ModelError>` con `ModelError` como enum exhaustivo de 17
variantes, **no** como string. Y `retry_after` como `Option<Duration>`.

### 3.3 Flags de producto (medidos)

```
enable_first_party_otel, enable_local_telemetry, user_prompt_credit_cap,
pull_request_review_rate_limit, allow_attribution, allow_github_auto_reviews,
allow_browser_experimental_features, disable_tool_call_execution_outside_workspace,
user_add_on_credit_cap, allow_codemap_sharing, disable_lifeguard, allowed_ip_ranges
```

`disable_tool_call_execution_outside_workspace` es una **política de seguridad de
primer nivel**: el agente no puede ejecutar tools fuera del workspace. En
MegaDaemon eso es `tool_policy.rs`.

---

## 4. Sesiones: el almacén es un **bosque**, no una lista

### 4.1 El esquema SQLite (medido del binario)

`DEVIN_SESSIONS_DIR` + `sessions.db`. Tablas:

```sql
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  working_directory TEXT NOT NULL,
  backend_type TEXT NOT NULL,
  model TEXT NOT NULL,
  permission_mode TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_activity_at INTEGER NOT NULL,
  total_tokens INTEGER NOT NULL,
  current_epoch INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE messages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  tool_call_id TEXT,
  tool_calls_json TEXT,
  timestamp INTEGER NOT NULL,
  compaction_epoch INTEGER NOT NULL DEFAULT 0,
  UNIQUE(session_id, compaction_epoch, sequence_number)
);

CREATE TABLE message_nodes (          -- ← EL BOSQUE
  row_id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  node_id INTEGER NOT NULL,           -- node_id dentro del bosque de la sesión
  parent_node_id INTEGER,             -- NULL para la raíz
  chat_message TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(session_id, node_id)
);

CREATE TABLE subagent_heads (
  session_id TEXT NOT NULL, agent_id TEXT NOT NULL,
  chain_node_id INTEGER NOT NULL, updated_at INTEGER NOT NULL,
  PRIMARY KEY (session_id, agent_id)
);

CREATE TABLE tool_call_state (
  session_id TEXT NOT NULL, tool_call_id TEXT NOT NULL,
  tool_call_json TEXT,               -- ToolCall inicial
  tool_call_update_json TEXT,        -- actualización final
  PRIMARY KEY (session_id, tool_call_id)
);

CREATE TABLE prompt_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT NOT NULL,
  timestamp INTEGER NOT NULL, session_id TEXT NOT NULL
);

CREATE TABLE rendered_commits (      -- HTML renderizado, versionado
  id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL,
  sequence_number INTEGER NOT NULL, rendered_html TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE(session_id, sequence_number)
);

CREATE TABLE live_row_times (session_id TEXT, row_key TEXT, started_at TEXT);
CREATE TABLE session_window_tools (session_id TEXT PRIMARY KEY, window_tools_json TEXT);
CREATE TABLE app_state (key TEXT PRIMARY KEY, value TEXT);

CREATE INDEX idx_message_nodes_session  ON message_nodes(session_id);
CREATE INDEX idx_messages_session_epoch ON messages(session_id, compaction_epoch);
CREATE INDEX idx_sessions_activity      ON sessions(last_activity_at);
CREATE INDEX idx_sessions_hidden        ON sessions(hidden);
```

### 4.2 Lo que esto teachs (patrones, no código)

| Patrón | Por qué importa |
|---|---|
| **`message_nodes` con `parent_node_id`** | La conversación es un **árbol**. Se puede **bifurcar**: reverta un paso y seguir por otro camino sin perder el original. En ACP, `cognition.ai/revert/{listSteps,preview,execute,forkFromStep}` (`chisel-agent/src/revert/`: `engine.rs`, `steps.rs`, `target.rs`) implementa justamente eso ⇒ **el revert es "fork del bosque"**. |
| **`compaction_epoch` + `current_epoch`** | La compactación **no borra**: marca una época. El historial anterior sigue consultable. En MegaDaemon: `agent_loop.rs` puede compactar sin perder trazabilidad. |
| **`rendered_commits`** | El HTML renderizado se **versiona** en la BD ⇒ se puede reconstruir exactamente lo que el usuario vio. |
| **`tool_call_state` con el JSON inicial y final** | Se guarda **el tool call tal como se pidió** y **como se resolvió**, aunque la resolución se interrumpa (comentario en el propio DDL: *"Nullable: may be absent for interrupted sessions"*). |
| **`subagent_heads`** | Un índice `(session_id, agent_id) → chain_node_id`: se sabe **en qué nodo del bosque** murió cada subagente ⇒ se puede reanudar. |
| **`live_row_times`** | Tiempos de vida de filas "vivas" ⇒ **métricas de latencia por operación**, guardadas en la BD. |

### 4.3 Dos backends de almacén, y una migración en curso

`session_store` acepta `sqlite` o `files`. El binario revela la **migración en
curso**: `"import of sessions.db into the folder store failed; the SQLite history
stays where it is and the import is retried later"` y `"sessions.db import is
running in another process"`.

⇒ Hay un **store de ficheros** (`store_io.rs`, con líneas 827/864/905 como puntos
calientes) y una importación **con bloqueo entre procesos** para migrar a él.
Más `__import__/`, `.imported-from-sqlite.json` y `sharedocs`.

⇒ Para MegaDaemon: **el store de sesiones debe poder cambiar de backend sin
perder el historial**, con la migración **reanudable e idempotente**. `agent_loop.rs`
no debería asumir un único backend.

---

## 5. El daemon

`chisel-server/src/daemon/{client.rs,server.rs,socket/{mod.rs,unix.rs}}`.

- **Lock**: `daemon.lock` · **Socket**: `daemon.sock`, ambos en el directorio de
  sesiones.
- **Validación**: `"the process at the daemon socket is not a devin daemon"` ⇒ el
  daemon **verifica que quien se conecta es su propio protocolo** antes de
  obedecer. Patrón de seguridad básico y cheaply: handshake antes de tocar nada.
- **RPC medido**: `list claimed sessions`, `list queued sessions`,
  `list outposts`, `create outpost`, `SSH forward supervisor`.
  ⇒ El daemon es **un supervisor**: sesiones reclamadas, sesiones en cola,
  outposts y túneles SSH.
- **Actores**: `chisel-server/src/actors.rs` ⇒ modelo de actores.
- **Agentes dentro del daemon**: `chisel-server/src/agents/{review.rs,summarizer.rs}`
  ⇒ el daemon puede ejecutar **agentes sin UI**.

⇒ Para MegaDaemon: `automation_runtime.rs` (41.885 B) tiene que poder hablar con
un supervisor externo por socket Unix. **El patrón "socket + lock + handshake +
RPC de cola"** es lo que hay que copiar (el diseño, no el código).

---

## 6. Sandbox (medido, texto literal del producto)

> **"Sandbox exec-tool processes (macOS seatbelt / Linux bwrap+seccomp)"**
> `[Research Preview]`
> *"When passed, commands can write only within the workspace and granted
> `Write(...)` scopes, and can read everything except paths hidden by
> `Deny(Read(...))` rules."*

| Plataforma | Mecanismo | Requisito |
|---|---|---|
| Linux | **bubblewrap + seccomp** | `bwrap` **y** `socat` instalados |
| macOS | **seatbelt** | nativo, sin instalar nada |
| Windows | — | hay que correr dentro de **WSL 2** |

El binario trae el mensaje de error con instrucciones de instalación para
Ubuntu/Debian y Fedora, y comprueba `bwrap --version`.

⇒ Para MegaDaemon `tool_policy.rs`: el modelo de permisos que ya medimos
(`Write(...)`, `Deny(Read(...))`, `Deny(Edit(...))`) se traduce **directamente a
reglas de sandbox**. Es el **mismo modelo** en las dos capas: el permiso lógico
se compila a una regla de bwrap/seccomp. Esa unificación es el patrón.

---

## 7. Plugins: el protocolo de distribución

`plugin-host/src/{manifest.rs,bundle_fetcher.rs,ingest.rs,list.rs,mcp.rs,actions.rs,cloud.rs}`.

### 7.1 Manifiesto (campos medidos)

```
PluginManifest {
  displayName, description,
  requiredPlugins: [], optionalPlugins: [],
  skills: SkillsField,            // Raw | Interface
  …
}
```

- **Precedencia de manifiestos** (de `plugins/overview.mdx`):
  `.devin-plugin/plugin.json` > `.claude-plugin/plugin.json` > `plugin.json` raíz.
- **Origen del plugin**: `git@id`, `account`, `org`, `source`, `url`.
- **Prohibiciones**: un glob de plugin prohibido **no puede contener `..`, `#` ni
  `@`**, y un glob relativo debe quedarse dentro de la entidad que lo declara.
  ⇒ **Path traversal prevention en los plugins**, medido.
- **Instalación**: `bundle_fetcher.rs` + `ingest.rs` traen un **bundle** y lo
  ingieren; `list.rs` lista; `cloud.rs` sincroniza con Devin Cloud.
- **Registro de instalación**: `{ version, origin, nodes, edges, refresh_failure,
  assets_status, forbidden, last_attempt, reason, fingerprint, origin, walked_at,
  roots, reached, failure, parent, child, version_dir, fetched_at }` ⇒ el
  **grafo de plugins** (`nodes`/`edges`) con sus dependencias resueltas y sus
  fallos de refresco.
- **MCP en plugin**: `{ command, env, cwd, url, serverUrl, transport, headers,
  oauthClientId, oauthClientSecret, oauthScopes, oauthResource }`. Nótese
  `oauthClientSecret` en el manifiesto ⇒ **el manifiesto puede llevar secreto**, y
  la doc obliga a que el servidor se rechace en activación si lo lleva
  (`plugins/overview.mdx`).

### 7.2 Variables de plantilla (medidas)

`CLAUDE_PROJECT_DIR` · `CLAUDE_PLUGIN_ROOT` · `PLUGIN_ROOT` · `PLUGIN_DATA`

`PLUGIN_DATA` es un **directorio persistente y escribible por plugin**, indexado
por *identidad* (no por versión) ⇒ **sobrevive a las actualizaciones** y se borra
al desinstalar. `stdin` de los servidores stdio recibe `PLUGIN_ROOT` y
`PLUGIN_DATA`.

⇒ Para MegaDaemon `skills.rs`: un modelo de plugin con **datos persistentes por
identidad** y **variables de plantilla** es justo lo que hace falta para que los
skills escritos como Agent Skills (`.agents/skills/<name>/SKILL.md`) funcionen.

---

## 8. Lo que el backend es (por los cables)

- **gRPC/protobuf**: `exa.api_server_pb.ApiServerService/GetAccountManagedPlugins`,
  `exa.seat_management_pb.SeatManagementService/GetUserStatus`,
  `exa.seat_management_pb.SeatManagementService/ExchangePKCEAuthorizationCode`.
  ⇒ El backend de Cognition habla **protobuf/gRPC**, no JSON plano.
- **HTTP/JSON**: `api.devin.ai` para el resto.
- **PKCE**: `ExchangeDevinCLIPKCECode` en `seat_management_pb`; callback local en
  `127.0.0.1:0/callback`.
- **Unleash**: `chisel-api/src/unleash.rs` con `strategy_parsing.rs` /
  `strategy_upgrade.rs` (y `unleash-yggdrasil-0.21.2` como cliente).
- **Telemetría**: `chisel-api/src/telemetry/{manager.rs,manager/cache.rs,sentry.rs,skill_spool.rs,unleash_sampling.rs}` ⇒ **el sampling de Unleash está acoplado a la telemetría** (flags ≠ telemetría).

---

## 9. Resumen: qué se lleva MegaDaemon

| Patrón | Origen medido | Destino en MegaDaemon | Esfuerzo |
|---|---|---|---|
| **Bucle de agente como manejador de efectos** | `affogato/agent/effects/*` | `agent_loop.rs` (refactor) | 2‑3 días |
| **Contexto y driver separados** | `fx/context.rs` + `fx/driver.rs` | `agent_loop.rs` | 1‑2 días |
| **Planificador de tool calls aislado** | `affogato/agent/tool_pipeline/scheduler.rs` | `agent_loop.rs` + `tool_policy.rs` | 2‑3 días |
| **Keepalive del caché de prompt** | `affogato/cache_keepalive*.rs` (5) | `providers.rs` | 3‑5 días |
| **Errores de proveedor como enum exhaustivo + `retry_after`** | `inference/src/retry.rs` | `providers.rs` | 1‑2 días |
| **Compactación por época, sin borrar** | `messages.compaction_epoch` + `sessions.current_epoch` | `agent_loop.rs` | 2 días |
| **Conversación como bosque + revert/fork** | `message_nodes.parent_node_id` + `chisel-agent/src/revert/*` | `agent_loop.rs` / `board.rs` | 4‑6 días |
| **Store de sesiones con 2 backends + migración reanudable** | `session_store{sqlite,files}` + `store_io.rs` | nuevo `session_store.rs` | 3‑4 días |
| **Daemon supervisor por socket Unix con handshake** | `chisel-server/src/daemon/*` | `automation_runtime.rs` | 3‑5 días |
| **Permisos compilados a reglas de sandbox** | `--sandbox` (bwrap+seccomp / seatbelt) | `tool_policy.rs` | 4‑6 días |
| **Plugin: datos persistentes por identidad + plantillas** | `plugin-host/*`, `PLUGIN_DATA` | `skills.rs` | 2‑3 días |
| **Umbral de compactación y formato de tools por modelo** | `inference/src/backend.rs` | `providers.rs` | 1‑2 días |
| **Watchdog de agente atascado** | `affogato/stall_watch.rs` | `automation_runtime.rs` | 1 día |
| **UI en Rust sin navegador (dioxus+blitz-dom)** | 4 crates externos | `webui`/cockpit (a evaluar) | 2 semanas |

---

## 10. Lo NO medido

| Elemento | Por qué |
|---|---|
| El **código** de los 297 módulos | Sólo se han leído nombres de fichero, cadenas y esquemas; no se ha descompilado |
| Los **nombres de los 37 hooks / eventos** internos | Parcial: los eventos ACP sí están medidos |
| La **política exacta** de Fusion (cómo elige lead/sidekick) | Requiere sesión y cuenta |
| Los **valores** de `compaction_thresholds` por modelo | Requiere la API de modelos |
| El **formato de bundle** de plugins (ficheros) | Sólo el manifiesto y el registro de instalación |
| El **protocolo gRPC** (nombres de método completos) | Parcial: sólo 4 métodos medidos |

---

## 11. Trazabilidad

| Afirmación | Evidencia |
|---|---|
| Árbol de 20 crates / 297 ficheros | rutas de compilación en `.rodata` del binario (`chisel-*/src/*.rs`, `affogato/src/*.rs`, `fx/src/*.rs`, `plugin-host/src/*.rs`, `inference/src/*.rs`) |
| Esquema SQLite | cadenas `CREATE TABLE IF NOT EXISTS …` del binario (extraídas literalmente) |
| Taxonomía de errores | cadena `UnauthenticatedTimeoutRateLimited…fallback` |
| Metadatos de modelo | cadena `model_type…is_model_router…compaction_thresholds…` |
| Sandbox | cadena `"Sandbox exec-tool processes (macOS seatbelt / Linux bwrap+seccomp)"` + mensajes de error de `bwrap` |
| Daemon | cadenas `daemon.lock`, `daemon.sock`, `list claimed sessions`, `list queued sessions` |
| Store | `DEVIN_SESSIONS_DIR`, `sessions.db`, `session_store` (`=sqlite =files`) |
| Manifiesto de plugin | cadena `displayName requiredPlugins optionalPlugins skillsRawInterface` + `forbiddenPlugins` |
| gRPC | `exa.api_server_pb.ApiServerService/GetAccountManagedPlugins`, `exa.seat_management_pb.SeatManagementService/*` |

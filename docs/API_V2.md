# API v2 del cockpit — MegaDaemon v14

> Especificación de la capa `rust-core/src/webui.rs` (agregadores JSON) y de los
> endpoints que consume la UI nueva `web/cockpit_v2.html`.
>
> **Estado**: implementada y testada en `rust-core/src/webui.rs`
> (16 tests).  **Pendiente de cableado**: el dueño de `cockpit_server.rs` tiene
> que exponer las rutas (3 líneas, ver §2) y registrar `pub mod webui;` en
> `lib.rs` (1 línea).  Hasta entonces la UI v2 talking con la v1 sigue
> funcionando: la v2 es **aditiva**.

---

## 1. Por qué existe

La UI v1 hace ~40 peticiones sueltas (una por dato) y sólo pinta lo que su
HTML sabe pintar.  De ahí las quejas reales:

| Queja | Causa real | Qué hace la v2 |
|---|---|---|
| "no muestra nodos conectados" | no existía ningún endpoint de nodos | `GET /api/v2/nodes` lee el bus (`mesh`) y el tablero SQLite (`coordination`) |
| "falla muchisimo" | 20 peticiones → 20 puntos de fallo | una vista = **una** llamada agregada |
| "no busca los LLM de forma agéntica" | no había vista de agente | `GET /api/v2/llm` + `POST /api/v2/nodes/*` + planificador con origen visible |
| "es un triste chatbot" | la UI era un cliente de `/api/chat` | vistas de **coordinación** (nodos, tareas, bucle de mejora, ficheros compartidos) |

**Regla de oro de la v2**: si un dato no existe, la respuesta lo dice.  No hay
campos inventados, ni mocks, ni "0" para tapar un `null`.  Cada ausencia
viene con su `reason` legible.

---

## 2. Cableado (lo hace el dueño de `cockpit_server.rs`)

En `cockpit_server.rs`, dentro del handler HTTP, **antes** del `match path`
existente:

```rust
// 1) API v2: agregadores de la UI nueva.
if let Some(v) = crate::webui::dispatch(method, path, &body_json) {
    send_json(&mut stream, &v);
    return;
}
```

Y para servir la página (junto a la rama `"/" | "/index.html"`):

```rust
"/v2" | "/cockpit_v2.html" => {
    let html = fs::read("web/cockpit_v2.html").unwrap_or_default();
    send_http_response(&mut stream, 200, "text/html; charset=utf-8", &html);
}
```

En `lib.rs` (orden alfabético, junto a `pub mod util_text;`):

```rust
pub mod webui;
```

`webui::dispatch` devuelve `None` para cualquier ruta que no sea `/api/v2/*`,
así que **no interfiere** con el flujo actual.

---

## 3. Política de autenticación

La v2 **no autentica**: hereda el modelo de la v1, que ya es de doble capa.

| Capa | Cabecera | Qué protege | Quién la trae |
|---|---|---|---|
| API key | `X-API-Key` (o `Authorization: Bearer …`) | todo `/api/*` fuera de loopback | usuario/agente remoto |
| Token de sesión | `X-Seere-Auth-Token` | operaciones que modifican estado | el navegador, tras el intercambio |

Flujo en la UI (implementado en `cockpit_v2.html`):

1. La UI manda `X-API-Key` si la tiene guardada en `localStorage`.
2. Si el daemon responde **401/403**, la UI pide la API key (modal) y hace
   `POST /api/session/exchange` con `{"api_key":"…"}`.
3. El daemon devuelve `{"ok":true,"token":"…"}`; la UI lo guarda en
   `sessionStorage` y lo manda como `X-Seere-Auth-Token`.
4. Si el intercambio falla, la UI sigue con la API key en las lecturas y avisa
   de que las escrituras están limitadas.

`GET /api/v2/routes` publica esta política dentro del propio JSON para que la
UI pueda documentarla (panel "Sistema").

---

## 4. Tabla de rutas

Fuente de verdad: `webui::api_v2_routes() -> Vec<(&'static str, &'static str)>`.
Un test de Rust comprueba que **la UI llama exactamente a estas rutas** y que
**la UI no llama a ninguna ruta inexistente**.

| Método | Ruta | Qué devuelve |
|---|---|---|
| GET | `/api/v2/dashboard` | resumen completo del daemon (1 fetch) |
| GET | `/api/v2/nodes` | instancias del bus + tareas + claims + eventos |
| POST | `/api/v2/nodes/assign` | publica una tarea en el bus para un nodo |
| POST | `/api/v2/nodes/message` | deja un mensaje (+datos) en el buzón del nodo |
| POST | `/api/v2/nodes/migrate` | plan de migración de versión de las instancias |
| GET | `/api/v2/tasks` | cola local + cola del tablero + stats + journal |
| POST | `/api/v2/tasks/export` | exporta la cola para repartirla entre nodos |
| POST | `/api/v2/tasks/import` | importa una cola exportada (idempotente) |
| GET | `/api/v2/llm` | catálogo de proveedores/modelos + activo + consumo |
| GET | `/api/v2/delegation` | cuentas, cuota, presupuesto, score, plan, ajustes |
| POST | `/api/v2/delegation/apply` | aplica los ajustes del motor (todo o nada) |
| GET | `/api/v2/alerts` | alertas activas + estado del canal webhook |
| GET | `/api/v2/re` | inventario de APKs, DEX, anti-root, vault de firma |
| GET | `/api/v2/agentic` | bucle local de auto-mejora (5 fases + notas) |
| GET | `/api/v2/share` | inventario del vault con sha256 y verificación |
| GET | `/api/v2/share/get` | descarga un fichero (base64) con verificación |
| POST | `/api/v2/share/put` | sube un fichero completo verificando su hash |
| POST | `/api/v2/share/chunk` | sube un trozo (64 KiB) de un fichero |
| POST | `/api/v2/share/commit` | ensambla los trozos y publica en el manifiesto |
| GET | `/api/v2/routes` | esta tabla en JSON + política de auth |

Rutas de la v1 que la v2 **reutiliza** (no se duplican): `POST /api/tool/agent`
(planificar/ejecutar), `POST /api/tool/task_queue` (submit/run/cancel/retry),
`POST /api/tool/alerts` (check/ack/prune), `POST /api/tool/apk_inventory`,
`POST /api/tool/root_stealth`, `POST /api/providers/test` (ping de
proveedor) y `POST /api/session/exchange`.

---

## 5. Contrato JSON

### 5.1 Envoltura común

* Toda respuesta es un objeto JSON (nunca un array en la raíz).
* `ok: true` = la operación se ejecutó; `ok: false` = error con `error`
  (`machine-readable`) y, si aplica, `detail`/`hint`/`reason` (legible).
* `generated_at`: ISO-8601 UTC (`2026-11-19T22:14:03Z`).

```json
{ "ok": false, "error": "hash_mismatch",
  "expected": "9f2c…", "actual": "41ed…",
  "detail": "el contenido no coincide con el sha256 declarado: no se escribe nada" }
```

### 5.2 `GET /api/v2/dashboard`

```json
{
  "ok": true,
  "generated_at": "2026-11-19T22:14:03Z",
  "daemon": {
    "name": "megadaemon-zenith", "version": "13.4.0", "pid": 4711,
    "uptime_sec": 51234, "uptime_human": "14h 13m",
    "host": "samsung-s21", "platform": "android-arm64",
    "instance_id": "9f2c8b1d…", "state_dir": "/data/user/0/com.agy/files/megadaemon/state"
  },
  "system": {
    "cpu_pct": 13.5,
    "mem": { "total_kb": 7850000, "available_kb": 3100000, "used_kb": 4750000, "used_pct": 60.5 },
    "loadavg": { "1m": 1.82, "5m": 1.44, "15m": 1.21 }
  },
  "power": {
    "capacity_pct": 72, "charging": false, "usb_plugged": false,
    "battery_temp_c": 31.5, "cpu_temp_c": 44.0, "thermal_hot": false,
    "governor": "schedutil", "system_power_save": false,
    "policy": { "name": "balanced", "reason": "…", "max_turns": 8, "max_tokens": 1536 }
  },
  "npu": {
    "soc": "Exynos 2100", "vs4l_present": true, "nnapi_devices": 2,
    "eden_runtime": true, "offload_implemented": false,
    "offload_note": "la telemetría NPU es real; el offload todavía no lo es",
    "throttle_state": "none", "battery_temp_c": 31.5, "cpu_temp_c": 44.0
  },
  "alerts": { "active": 2, "crit": 0, "warn": 1, "info": 1, "dedup_window_s": 300 },
  "queue": { "total": 7, "by_state": { "pending": 5, "running": 1, "done": 1 }, "by_kind": {}, "journal_entries": 42 },
  "bus": { "deployed": true, "master": true, "reason": "tablón de coordinación presente", "nodes": 2, "peers_alive": 1 },
  "capabilities": { "total": 24, "healthy": 21, "degraded": 3 },
  "llm": { "default_model": "local-seere", "providers": 14, "ready": 3, "free_providers": 11, "catalog_size": 14 },
  "share": { "files": 5, "bytes": 1048576, "root": "…/state/share" }
}
```

### 5.3 `GET /api/v2/nodes`

```json
{
  "ok": true,
  "generated_at": "2026-11-19T22:14:03Z",
  "bus": {
    "deployed": true, "master": false,
    "state_dir": "/data/local/tmp/megadaemon",
    "board_path": "/data/local/tmp/megadaemon/mesh_board.json",
    "reason": "tablón de coordinación presente",
    "sqlite_board": false
  },
  "nodes": [
    { "id": "9f2c8b1d…", "role": "worker", "hostname": "pc-linux", "platform": "linux-x86_64",
      "version": "13.4.0", "capabilities": ["reverse"], "load": 0.31,
      "last_seen": "2026-11-19T22:13:58Z", "kind": "local", "alive": true }
  ],
  "node_count": 1,
  "board_instances": [],
  "tasks": [ { "task_id": "task-1732-ab12", "title": "sync.local", "capability": "",
               "priority": 5, "state": "published", "created_at": "…" } ],
  "claims": [], "events": [],
  "stats": { "total": 0, "by_status": {} },
  "notes": [],
  "actions": [ { "id": "assign", "label": "Asignar tarea", "method": "POST", "path": "/api/v2/nodes/assign" } ]
}
```

**Si el bus no está desplegado** (estado real, no mock):

```json
{
  "ok": true,
  "bus": {
    "deployed": false, "master": true,
    "board_path": "/data/local/tmp/megadaemon/mesh_board.json",
    "reason": "tablero no desplegado: no existe /data/local/tmp/megadaemon/mesh_board.json (arranca el daemon con el mismo MEGADAEMON_STATE_DIR en las instancias que quieras conectar)",
    "sqlite_board": false
  },
  "nodes": [], "node_count": 0
}
```

### 5.4 Acciones del bus

```jsonc
// POST /api/v2/nodes/assign
{ "node": "pc-linux", "kind": "sync.local", "capability": "reverse",
  "payload": { "priority": 7 } }

// → 200
{ "ok": true, "task_id": "task-1732-ab12", "board_task_id": "t-1732-1f3c",
  "node": "pc-linux", "kind": "sync.local", "capability": "reverse",
  "state": "published",
  "note": "publicada en el tablón; la otra instancia debe reclamarla (lease)" }

// sin bus desplegado
{ "ok": false, "error": "bus_not_deployed",
  "reason": "tablero no desplegado: …",
  "hint": "despliega el bus (tablón compartido) antes de repartir trabajo" }
```

```jsonc
// POST /api/v2/nodes/message
{ "node": "pc-linux", "message": "coge el informe", "data_b64": "" }
// → 200
{ "ok": true, "task_id": "msg-1732-99ab", "node": "pc-linux",
  "inbox": "/data/local/tmp/megadaemon/inbox/pc-linux.ndjson", "bytes": 118 }

// POST /api/v2/nodes/migrate
{ "target_version": "13.4.0" }
// → 200
{ "ok": true, "target_version": "13.4.0", "local_version": "13.4.0",
  "release": { "version": "13.4.0", "commit": "…", "schema": 3, "migrated_at": "…" },
  "sqlite_plan": { "steps": [ … ] }, "mesh_plan": { … } }
```

### 5.5 `GET /api/v2/tasks`

```json
{
  "ok": true,
  "tasks": [ { "id": "t-8f2a", "kind": "sync.local", "priority": 7, "state": "pending",
               "attempts": 0, "deps": [], "created_at": 1732, "started_at": null,
               "finished_at": null, "error": null, "node": "" } ],
  "stats": { "tasks": 7, "by_state": { "pending": 5 }, "by_kind": { "sync.local": 3 },
             "journal_entries": 42, "queue_path": "…" },
  "known_kinds": [ "sync.local", "sync.web", "apk.inventory", "alerts.check", "…" ],
  "board": { "deployed": false, "reason": "tablero SQLite no desplegado" },
  "board_tasks": [],
  "journal": { "ok": true, "entries": [ { "kind": "sync.local", "ok": true, "result": { "summary": "…" } } ] },
  "actions": [ { "id": "submit", "method": "POST", "path": "/api/tool/task_queue", "body": { "action": "submit" } } ]
}
```

Export / import (para el sistema distribuido):

```jsonc
// POST /api/v2/tasks/export   { "node": "pc-linux" }
{ "ok": true, "node": "pc-linux",
  "export": { "version": 1, "node": "pc-linux", "tasks": [ … ] } }

// POST /api/v2/tasks/import   con el JSON exportado por el otro nodo
{ "ok": true, "imported": 4,
  "note": "tareas importadas (las que ya existían se ignoran)" }
```

### 5.6 `GET /api/v2/llm`

```json
{
  "ok": true,
  "default_model": "local-seere",
  "providers": [ { "id": "local-seere", "name": "SEERE v13.1 Local (Qwen/SmolLM2 + GBNF, offline)",
                   "vendor": "Local-Exynos", "endpoint": "http://127.0.0.1:8080/v1/chat/completions",
                   "model": "local", "auth_type": "local", "tier": "offline",
                   "free_signup_url": "", "enabled": true, "ready": true,
                   "has_key": false, "key_masked": "", "supports_reasoning": false } ],
  "provider_count": 14, "ready_count": 3, "free_providers": 11,
  "catalog": [ { "id": "local-seere", "free": true, "tier": "offline", "…": "…" } ],
  "catalog_size": 14,
  "retired": [ { "id": "minimax-m2.7", "model": "…", "note": "retirado el 18-nov-2026" } ],
  "engine": "rust-swarm-v13.1",
  "routing": { "primary": "local-seere",
               "policy": "oportunista: local primero (0 tokens), swarm remoto si el local falla, heurística determinista si no hay LLM" },
  "pricing": [ { "model": "…", "provider": "…", "input_usd_per_mtok": 0.0,
                 "output_usd_per_mtok": 0.0, "source": "https://…", "verified_on": "2026-11-19", "note": "" } ],
  "consumption": { "available": true, "tokens": { "input": 0, "output": 0, "total": 0 },
                   "cost_usd": { "estimated": 0.0, "shadow_if_paid": 0.0, "savings": 0.0 },
                   "by_account": { "kimi-1": { "model": "kimi-k3", "input_tokens": 0,
                                               "output_tokens": 0, "cost_usd": 0.0, "success_rate": 0.0 } } },
  "note": "ping de proveedor: POST /api/providers/test con {\"id\":\"…\"}"
}
```

### 5.7 `GET /api/v2/delegation` y `POST /api/v2/delegation/apply`

```json
{
  "ok": true, "deployed": true,
  "state_path": "…/state/delegate_state.json",
  "accounts": [ { "id": "kimi-1", "provider": "kimi", "tier": "free", "model": "kimi-k3",
                  "capabilities": ["research"], "remaining_quota": null,
                  "daily_budget_usd": 0.0, "priority": 0, "enabled": true,
                  "free": true, "score": 0.5 } ],
  "account_stats": [ { "account": "kimi-1", "attempts": 3, "ok": 3, "failed": 0,
                       "success_rate": 1.0, "score": 0.95, "remaining_quota": null,
                       "burned": false, "…": "…" } ],
  "plan": [ { "order": 1, "task_id": "task-1", "kind": "research", "account": "kimi-1",
              "model": "kimi-k3", "estimated_input_tokens": 96, "estimated_output_tokens": 1024,
              "estimated_cost_usd": 0.0, "reason": "cuenta gratuita con la capacidad requerida" } ],
  "adjustments": [ { "account": "deepseek-3", "action": { "SetPriority": -15 },
                     "reason": "tasa de éxito 33 % tras 3 intentos: bajar prioridad a -15" } ],
  "stats": { "tasks": { "total": 0 }, "queue_depth": 0, "…": "…" },
  "consumption": { "available": true, "…": "…" },
  "pricing": { "models": 9, "ledger": [ … ] }
}
```

```jsonc
// POST /api/v2/delegation/apply
{ "adjustments": [ { "account": "deepseek-3", "action": { "SetPriority": -15 }, "reason": "…" } ] }
// → 200
{ "ok": true, "applied": 1, "accounts": 8, "state_path": "…" }
// → si un ajuste no es válido (todo o nada)
{ "ok": false, "error": "rejected", "detail": "cuenta desconocida: inventada" }
```

Sin motor desplegado (estado real):

```json
{ "ok": true, "deployed": false,
  "reason": "no hay estado de delegación en …/state/delegate_state.json: el motor todavía no tiene cuentas cargadas",
  "accounts": [], "adjustments": [], "plan": [], "consumption": { "available": false } }
```

### 5.8 `GET /api/v2/alerts`

```json
{
  "ok": true,
  "alerts": [ { "id": "a-1732-9f", "rule": "battery.low", "severity": "warn",
                "title": "Batería al 12 %", "body": "…", "source": "power",
                "count": 3, "created_at": 1732, "last_seen": 1732,
                "acknowledged": false, "ack_at": null } ],
  "total": 1, "returned": 1,
  "status": { "ok": true, "active": 1, "crit": 0, "warn": 1, "info": 0,
              "total_emitted": 12, "dedup_window_s": 300, "persistence": "…" },
  "channel": { "configured": true, "kind": "webhook", "host": "hooks.example.com",
               "note": "MEGADAEMON_ALERT_WEBHOOK está definido: las alertas se entregan también fuera del daemon" },
  "actions": [ { "id": "ack", "method": "POST", "path": "/api/tool/alerts", "body": { "action": "ack" } } ]
}
```

El canal **nunca** filtra la URL completa del webhook (puede llevar
credenciales): sólo el host.

### 5.9 `GET /api/v2/re`

```json
{
  "ok": true,
  "apps": { "available": true, "count": 42, "packages": [ { "package": "com.termux", "version": "…", "sha256": "…" } ],
            "reason": null, "duration_ms": 812 },
  "dex": { "available": true,
           "entry_point": "POST /api/tool/apk_inventory con {\"action\":\"analyze\",\"path\":\"…\"}",
           "note": "la decompilación DEX y las señales anti-root se lanzan por APK, no de forma masiva" },
  "root_hide": { "profile_targets": [ "com.mobilaypay.app", "…" ], "note": "…" },
  "export": { "task_export": "POST /api/tool/task_queue {\"action\":\"export\"}",
              "repo_updates": "POST /api/tool/repo_updates {\"action\":\"plan\"}",
              "apk_dump": "POST /api/tool/apk_inventory {\"action\":\"dump\",\"dest_dir\":\"…\"}" },
  "signature_vault": { "files": [ { "name": "release.p12", "size": 4096, "sha256": "…" } ],
                        "count": 1, "empty": false, "note": "…" }
}
```

Si `pm` no está disponible (PC sin root): `apps.available = false` y
`apps.reason` explica por qué — **no** una lista vacía fingida.

### 5.10 `GET /api/v2/agentic`

```json
{
  "ok": true,
  "cycle": [ { "phase": "observe", "label": "Observar", "source": "dashboard + cola + alertas", "ok": true },
             { "phase": "plan",    "label": "Planificar", "source": "agent_core::plan (LLM local o heurística)", "ok": true },
             { "phase": "execute", "label": "Ejecutar", "source": "task_scheduler + bus de coordinación", "ok": true },
             { "phase": "measure", "label": "Medir", "source": "journal + agent_stats del bus", "ok": true },
             { "phase": "adjust",  "label": "Ajustar", "source": "delegate::adjustments + notas de aprendizaje", "ok": true } ],
  "observe": { "queue": 7, "by_state": { "pending": 5 }, "capabilities": { "total": 24, "healthy": 21 },
               "known_task_kinds": [ "sync.local", "…" ] },
  "plan": { "route": { "primary": "local-seere", "policy": "…" }, "heuristic_fallback": true },
  "execute": { "journal_entries": 42, "done": 40, "failed": 2, "agents": [ … ] },
  "measure": { "agent_stats": { "total": 3, "by_status": { "done": 3 } },
               "notes": [ "Fallo dominante: sync.web (2/5 tareas de tipo sync.web fallan)" ] },
  "adjust": { "proposed": [ … ], "count": 1, "apply_path": "/api/v2/delegation/apply" }
}
```

### 5.11 Ficheros compartidos (`/api/v2/share*`)

**Layout en disco** (bajo `<state_dir>/share/`, o `MEGADAEMON_SHARE_DIR`):

```
share/
├── MANIFEST.ndjson     una línea JSON por fichero
├── blobs/<sha256>      contenido direccionado por hash (dedup + integridad)
└── parts/<ruta>/       trozos en curso de una subida por chunks
```

`MANIFEST.ndjson` (una línea por fichero, para que los nodos compartan sin
pisarse):

```json
{"path":"informes/2026-11-19.txt","size":1048576,"sha256":"9f2c…","updated_at":"2026-11-19T22:14:03Z","node":"9f2c8b1d…","chunks":16}
```

| Ruta | Cuerpo | Respuesta |
|---|---|---|
| `GET /api/v2/share` | — | `{ "ok": true, "files": [ { "path", "size", "sha256", "state": "ok\|corrupt\|missing", "verified": true, "actual_sha256" } ], "total_files", "total_bytes", "verified", "broken", "missing", "pending_parts", "root", "manifest" }` |
| `POST /api/v2/share/put` | `{ "path", "data_b64", "sha256" }` | `{ "ok": true, "path", "sha256", "size", "entry" }` |
| `POST /api/v2/share/chunk` | `{ "path", "index", "data_b64", "sha256" }` | `{ "ok": true, "path", "index", "sha256", "size" }` |
| `POST /api/v2/share/commit` | `{ "path", "sha256" }` | `{ "ok": true, "path", "sha256", "size", "chunks" }` |
| `GET /api/v2/share/get` | `?path=…&offset=0&len=0` | `{ "ok": true, "path", "sha256", "verified", "size", "offset", "returned", "data_b64" }` |

Contrato de integridad:

* `put`/`chunk` **rechazan** el contenido si el sha256 no coincide
  (`error: "hash_mismatch"`) y **no escriben nada**.
* `commit` ensambla los trozos en orden de `index`, recalcula el sha256 del
  fichero completo y lo compara con el declarado: si no cuadra, **no publica**.
* `get` recalcula el hash del blob al descargar y devuelve `verified: false` si
  no coincide con el manifiesto (blob corrupto por un sync roto).
* `shared_files()` **verifica de verdad**: recalcula el hash de cada blob en
  disco al listar (por eso `state` puede ser `corrupt`).

Ejemplo de error:

```json
{ "ok": false, "error": "hash_mismatch",
  "expected": "0000000000000000000000000000000000000000000000000000000000000000",
  "actual": "9f2c8b1d…",
  "detail": "el ensamblado no coincide con el sha256 del fichero completo: no se publica" }
```

---

## 6. Firmas públicas de `rust-core/src/webui.rs`

```rust
pub fn dashboard() -> Value;
pub fn nodes() -> Value;
pub fn task_board() -> Value;
pub fn llm_catalog() -> Value;
pub fn delegation() -> Value;
pub fn delegation_apply(adjustments: &Value) -> Value;
pub fn alerts_view() -> Value;
pub fn re_apps() -> Value;
pub fn agentic_loop_status() -> Value;
pub fn shared_files() -> Value;
pub fn share_put(rel_path: &str, data_b64: &str, expected_sha256: &str) -> Value;
pub fn share_put_chunk(rel_path: &str, index: usize, data_b64: &str, expected_sha256: &str) -> Value;
pub fn share_commit(rel_path: &str, expected_sha256: &str) -> Value;
pub fn share_get(rel_path: &str) -> Value;
pub fn task_export(node: &str) -> Value;
pub fn task_import(payload: &Value) -> Value;
pub fn node_assign(node_id: &str, kind: &str, payload: &Value, capability: &str) -> Value;
pub fn node_message(node_id: &str, message: &str, data_b64: &str) -> Value;
pub fn node_migrate(target_version: &str) -> Value;
pub fn api_v2_routes() -> Vec<(&'static str, &'static str)>;
pub fn routes_doc() -> Value;
pub fn dispatch(method: &str, path: &str, body: &Value) -> Option<Value>;
pub fn sha256_hex(data: &[u8]) -> String;
pub fn base64_encode(data: &[u8]) -> String;
pub fn base64_decode(text: &str) -> Result<Vec<u8>, String>;
pub fn now_epoch() -> i64;
pub fn now_iso() -> String;
pub fn cockpit_state_dir() -> String;
pub fn bus_state_dir() -> String;
pub fn share_root() -> std::path::PathBuf;
```

---

## 7. Rutas de estado en disco

| Qué | Dónde | Variable de entorno |
|---|---|---|
| Estado del cockpit | `/data/user/0/com.agy/files/megadaemon/state` | `MEGADAEMON_STATE_DIR` |
| Bus / tablón de coordinación | `/data/local/tmp/megadaemon` (Android) o `~/.megadaemon` | `MEGADAEMON_STATE_DIR` |
| Tablero SQLite de tareas | `<bus>/board.sqlite` | — |
| Vault de ficheros compartidos | `<cockpit_state>/share` | `MEGADAEMON_SHARE_DIR` |
| Estado del motor de delegación | `<cockpit_state>/delegate_state.json` | `MEGADAEMON_DELEGATE_STATE` |

---

## 8. Tests

`rust-core/src/webui.rs` lleva **16 tests** (`cargo test --lib webui`):

| Test | Qué garantiza |
|---|---|
| `la_tabla_de_rutas_no_tiene_duplicados_y_usa_el_prefijo_v2` | la tabla es coherente |
| `cada_ruta_de_la_api_v2_se_pide_desde_la_ui` | **la UI llama a todas las rutas de la API** |
| `la_ui_no_llama_a_rutas_inexistentes` | **la UI no llama a rutas que no existen** (v1, v2 y `/api/tool/<herramienta>` validados contra el código del servidor) |
| `el_dashboard_tiene_las_claves_esperadas` | contrato del resumen |
| `el_dashboard_no_inventa_nodos` | sin bus ⇒ 0 nodos y razón legible |
| `la_vista_de_nodos_es_consistente_con_el_estado_del_bus` | coherencia bus/nodos |
| `el_hash_de_un_fichero_compartido_se_verifica` | subida ok con hash bueno, **rechazo** con hash malo |
| `un_blob_corrupto_se_detecta_al_verificar_el_inventario` | la verificación real detecta corrupción |
| `la_subida_por_trozos_se_ensambla_y_se_verifica` | chunks → commit → manifiesto |
| `las_funciones_puras_no_fallan_con_entradas_vacias` | entradas inválidas → error, no pánico |
| `la_ruta_relativa_se_normaliza_y_se_protege` | no hay escape del vault (`..`) |
| `el_router_deja_pasar_lo_que_no_es_de_la_v2` | la v1 no se ve afectada |
| `el_router_responde_con_error_claro_para_una_ruta_desconocida` | 404 de la v2 legible |
| `el_tablero_de_tareas_incluye_los_tipos_conocidos` | contrato del tablero |
| `el_catalogo_de_llm_lista_proveedores_reales` | catálogo no vacío y real |
| `la_documentacion_de_rutas_coincide_con_la_tabla` | la doc se autogenera de la tabla |
| `el_bucle_agentico_declara_las_cinco_fases` | las 5 fases del bucle |
| `la_vista_de_alertas_muestra_el_canal_real` | canal webhook sin filtrar la URL |

---

## 9. Limitaciones honestas

* **El bus puede no existir**: sin `mesh_board.json` compartido, la vista Nodos
  lo dice y las acciones fallan con `bus_not_deployed`.  La v2 no crea el bus
  por la vista (no "despliega" nada al mirar).
* **El motor de delegación necesita estado**: sin
  `delegate_state.json` con cuentas, la vista Delegación dice que no está
  desplegado y los ajustes propuestos son una lista vacía (no inventados).
* **El inventario de APKs necesita root**: sin `pm` no hay lista; se reporta
  `available: false` con el motivo.
* **La verificación de blobs es O(tamaño)**: `shared_files()` recalcula el
  sha256 de cada fichero para poder decir la verdad; con ficheros muy grandes
  conviene paginar en el futuro.
* **La subida por chunks es de un solo sentido**: el "commit" exige que el
  cliente mande el sha256 del fichero completo; si el cliente no lo manda, se
  usa el declarado en el `meta.json` de la subida.
* **La v2 no sustituye a la v1**: ambas conviven.  La v1 sigue sirviendo
  `/api/chat`, RE, NPU y el resto de herramientas que la v2 no agrega.

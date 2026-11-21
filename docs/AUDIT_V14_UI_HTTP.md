# Auditoría v14 · UI / HTTP / MCP / proveedores (3ª tanda, sin solape con HW-01..16 ni S01..S14b)

*Fecha: 2026-11-19 · Alcance: `rust-core/src/cockpit_server.rs`, `rust-core/src/providers.rs`,
`rust-core/src/mcp_server.rs`, `rust-core/src/shm.rs`, `rust-core/src/freezer.rs`,
`web/cockpit_template.html`, `deploy/web_bridge.py` · **No se ha tocado `src/` de producción**:
todo lo que hay aquí son *diffs propuestos* + tests que fallan con el código actual.*

Ficheros nuevos del encargo:

| Fichero | Qué es |
|---|---|
| `rust-core/tests/audit_scout_ui.rs` | 15 tests ejecutables (HTTP real contra el cockpit + harness Node para el JS de la UI) |
| `rust-core/tests/cockpit_doctor_smoke.rs` | 9 tests de humo del nuevo `src/cockpit_doctor.rs` |
| `rust-core/src/cockpit_doctor.rs` | diagnóstico puro (`DoctorEnv` + trait `Probe`), estado `not_operational` |
| `deploy/install-phone.sh` | despliegue idempotente: SHA-256 verificado **en el móvil**, espera activa de `/health`, rollback |
| `docs/EXTERNAL_AGENT_EXYNOS_PRIORITY.md` | prompt listo para pegar a un agente externo (nodo RE Exynos 2100 + broadcast) |

## Cómo ejecutar

```bash
cd rust-core
cargo test --test audit_scout_ui -- --test-threads=1 --nocapture
cargo test --test cockpit_doctor_smoke -- --test-threads=1 --nocapture
```

Salida real (2026-11-19 21:5x, perfil debug, `tools/build-queue.sh`):

```
running 15 tests
test result: FAILED. 4 passed; 11 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.21s
```

```
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
```

---

## 1. Resumen

| ID | Severidad | Fichero:línea | Defecto | Test |
|---|---|---|---|---|
| **UI-00** | **CRÍTICA** | `deploy/web_bridge.py:135-136`, `:150` | RCE root desde cualquier web (inyección del token + `ACAO: *`) | aplicado y verificado en el móvil |
| **UI-16** | **CRÍTICA** | `cockpit_server.rs:841-905`, `web/cockpit_template.html:797-800` | **El navegador no puede obtener token de sesión ⇒ con el binario v14 la UI queda muerta (todo 403)** | `ui16_...` |
| **UI-03** | **CRÍTICA** | `shm.rs:73-78` | Panic por overflow en `/api/live_frame` con cabecera hostil → en `release` (`panic="abort"`) **muere el cockpit entero** | `ui03_...` |
| **UI-04** | **ALTA** | `cockpit_server.rs` (endpoint `hex_inspect`) | Los endpoints RE leen **cualquier fichero del móvil** (`/etc/hostname` incluído), sin sandbox | `ui04_...` |
| **UI-06** | **ALTA** | `providers.rs:76-110` | Carrera read-modify-write de la bóveda: 10 de 12 keys perdidas **y el JSON queda corrupto** | `ui06_...` |
| **UI-11** | **ALTA** | `freezer.rs:137-156` | TOCTOU por symlink: el hilo del freezer escribe **fuera del sandbox** | `ui11_...` |
| **UI-05** | **ALTA** | `providers.rs:186-255` | Guardar proveedor con key + endpoint **descarta la key** (`has_key:false`) | `ui05_...` |
| **UI-08** | MEDIA | `cockpit_server.rs` (POST `/api/providers`) | `id:""` → responde `{"status":"OK"}` y **descarta la key en silencio** | `ui08_...` |
| **UI-01** | MEDIA | `cockpit_server.rs:1195-1201` | `/api/input` responde `ok` aunque la escritura en la SHM sea imposible | `ui01_...` |
| **UI-07** | MEDIA | `providers.rs` (despacho de chat) | Un proveedor con `enabled:false` **sigue recibiendo peticiones** con su key | `ui07_...` |
| **UI-10** | MEDIA | `mcp_server.rs:186-250` | El servidor MCP **ignora los batches** JSON-RPC → el cliente se queda colgado | `ui10_...` |
| **UI-13** | MEDIA | `web/cockpit_template.html:1528` | Un refresco **antiguo** puede pisar al reciente (estado stale en la UI) | `ui13_...` |
| **UI-15** | MEDIA | `cockpit_server.rs:226-271` | `/health` y `/api/ping` son públicos y dan el mapa interno del móvil | revisión manual |
| UI-02 | control | — | `/api/runners/control` ya exige token y **no** destruye el frame vivo | `ui02_...` **pasa** |
| UI-09 | control | — | La página pública **ya no** filtra el token; POST sin credenciales → 403 | `ui09_...` **pasa** |
| UI-12 | control | — | El `onclick` inline (XSS) ya está arreglado (`data-test-provider` + delegación) | `ui12_...` **pasa** |
| UI-14 | control | — | Guardar una key ya **no** reinicia el selector de proveedor | `ui14_...` **pasa** |

---

## 2. Reproducción con salida real

### UI-16 · CRÍTICA · el navegador no puede obtener token de sesión ⇒ la UI v14 queda muerta

`GET /` sirve la página **sin** token (arreglo correcto de AUDIT-WEB-06) y el comentario del
propio código dice que el cliente debe pedirlo en `POST /api/session/exchange`:

```rust
// cockpit_server.rs:862-868
//   * `GET /`               → público, sin secretos.
//   * `GET /api/*`          → exige API key (o loopback).
//   * `POST /api/*`         → exige API key + token de sesión.
```

Pero ese endpoint es **inalcanzable**: está dentro de la rama `if method == "GET"` y
`requires_auth()` exige token de sesión en todo lo que no sea la página o las sondas
(`cockpit_server.rs:123-150`) ⇒ **pide un token para poder entregar un token**.

Salida real (binario v14 recién compilado, `MEGADAEMON_API_KEY=testkey_abc123`):

```
$ curl -s -w "status=%{http_code}\n" -X POST http://127.0.0.1:45999/api/session/exchange \
      -H "X-API-Key: testkey_abc123" -H 'Content-Type: application/json' \
      -d '{"api_key":"testkey_abc123"}'
status=403
{"error":"unauthorized","hint":"falta la cabecera X-Seere-Auth-Token"}

$ curl -s -w "status=%{http_code}\n" http://127.0.0.1:45999/api/session/exchange -H "X-API-Key: testkey_abc123"
status=403
{"error":"unauthorized","hint":"falta la cabecera X-Seere-Auth-Token"}

$ curl -s http://127.0.0.1:45999/ | grep -o "MEGADAEMON_TOKEN=[^;]*"
MEGADAEMON_TOKEN=""

$ curl -s -w "status=%{http_code}\n" -X POST http://127.0.0.1:45999/api/providers -H "X-API-Key: testkey_abc123" -d '{}'
status=403
{"error":"unauthorized","hint":"falta la cabecera X-Seere-Auth-Token"}
```

Y el lado JS tampoco ayuda (`web/cockpit_template.html:797-800`):

```js
function authHeaders(extra) {
  const h = Object.assign({}, extra || {});
  if (window.MEGADAEMON_TOKEN) h["X-Seere-Auth-Token"] = window.MEGADAEMON_TOKEN;
  return h;                     // <- con el token vacío NO manda ninguna credencial
}
```

La plantilla **nunca** llama a `/api/session/exchange` (`grep session/exchange web/cockpit_template.html` → 0 resultados).
⇒ Con el binario v14 la página carga y **todas** las llamadas de datos devuelven 403: la UI es un
cascarón vacío. El móvil sigue con v13.3.0 (que sí incrusta el token) → por eso ahora funciona;
**el próximo despliegue lo rompe**.

Salida del test:

```
test ui16_el_navegador_puede_cambiar_la_api_key_por_un_token_de_sesion ...
[UI-16] POST /api/session/exchange (API key válida) -> 403 {"error":"unauthorized","hint":"falta la cabecera X-Seere-Auth-Token"}
panicked at tests/audit_scout_ui.rs:785:
assertion `left == right` failed: UI-16: el navegador NO PUEDE obtener un token de sesión (403): la UI v14 queda sin datos (todo 403)
  left: 403
 right: 200
```

**Arreglo propuesto** (dos piezas):

```diff
--- a/rust-core/src/cockpit_server.rs
+++ b/rust-core/src/cockpit_server.rs
@@ fn requires_auth(path: &str, _method: &str) -> bool {
+    // El intercambio API key -> token de sesión es la ÚNICA forma de que el
+    // navegador consiga un token: NO puede exigir un token que todavía no tiene.
+    if path == "/api/session/exchange" {
+        return false;   // la API key ya la validó el gate anterior
+    }
```
```diff
--- a/web/cockpit_template.html
+++ b/web/cockpit_template.html
@@
 function authHeaders(extra) {
   const h = Object.assign({}, extra || {});
-  if (window.MEGADAEMON_TOKEN) h["X-Seere-Auth-Token"] = window.MEGADAEMON_TOKEN;
+  if (window.MEGADAEMON_TOKEN) h["X-Seere-Auth-Token"] = window.MEGADAEMON_TOKEN;
+  else if (window.MEGADAEMON_API_KEY) h["X-API-Key"] = window.MEGADAEMON_API_KEY;
   return h;
 }
+// Al cargar, si no hay token de sesión, se pide uno con la API key.
+async function ensureSessionToken() {
+  if (window.MEGADAEMON_TOKEN) return true;
+  const key = window.MEGADAEMON_API_KEY || prompt("API key de MegaDaemon:");
+  if (!key) return false;
+  const r = await fetch("/api/session/exchange", {
+    method: "POST", headers: { "Content-Type": "application/json", "X-API-Key": key },
+    body: JSON.stringify({ api_key: key })
+  });
+  if (!r.ok) return false;
+  const j = await r.json();
+  window.MEGADAEMON_TOKEN = j.token || "";
+  return !!window.MEGADAEMON_TOKEN;
+}
```

*(El handler `/api/session/exchange` además está en la rama `GET`: hay que moverlo a la rama `POST`,
que es donde llega el body con la API key.)*

---

### UI-03 · CRÍTICA · panic por overflow en `/api/live_frame` → en release mata el cockpit

`shm.rs:73-78` multiplica `width*height*3` sin saturar; con la cabecera hostil `65535×65535`
hace `65535*65535*3 = 12.884.908.025` → overflow de `usize` en 64 bits no, pero en el `u32`
intermedio sí (y `release` tiene `panic = "abort"`, `rust-core/Cargo.toml:53`).

Salida real:

```
thread '<unnamed>' panicked at src/shm.rs:75:28:
[UI-03] GET /api/live_frame con SHM 65535x65535 -> status=0 len_cuerpo=0
[UI-03] /api/ping después -> 200 {"daemon":"megadaemon-zenith-rust",...}
panicked at tests/audit_scout_ui.rs:421:
UI-03: /api/live_frame debe responder (200/4xx), status=0 cuerpo=0 bytes (la conexión murió: el handler paniqueó)
```

En debug la panic se queda en el hilo; en **release el proceso entero muere** ⇒ el móvil se queda
sin cockpit ⇒ `ERR_CONNECTION_REFUSED` (el síntoma que reportó el usuario).

**Arreglo propuesto**:

```diff
--- a/rust-core/src/shm.rs
+++ b/rust-core/src/shm.rs
@@ pub fn encode_bmp_24(width: u32, height: u32, rgb_data: &[u8]) -> Vec<u8> {
+    // Cabecera hostil (65535x65535) => no overflow, no panic, no proceso muerto.
+    let need = (width as u64).saturating_mul(height as u64).saturating_mul(3);
+    if need > MAX_FRAME_BYTES || width == 0 || height == 0 || width > MAX_FRAME_DIM || height > MAX_FRAME_DIM {
+        return Vec::new();   // el handler responde 400, no entra en pánico
+    }
```
Y en el handler, comprobar `width*height*3 <= rgb_data.len()` antes de codificar.

---

### UI-04 · ALTA · los endpoints RE leen cualquier fichero del móvil

Con un token de sesión válido, `/api/hex_inspect` lee **fuera** del sandbox:

```
[UI-04] objetivo fuera del sandbox: /etc/hostname
[UI-04] GET /api/hex_inspect?file=/etc/hostname&offset=0&size=256 -> 200
[UI-04] contenido devuelto: "{\"arch\":\"Multi-OS/Auto\",\"engine\":\"rust-arm64-v11.0\",\"file\":\"hostname\",\"lines\":[{\"addr\":\"000000\",\"ascii\":\"cloud-compute-ne\",\"hex\":\"63 6C 6F 75 64 2D 63 6F 6D 70 75 74 65 2D 6E 65\"},...
panicked at tests/audit_scout_ui.rs:486:
UI-04: el cockpit devolvió el contenido de un fichero FUERA del sandbox: ...
```

El nombre de fichero del móvil es `cloud-compute-new-hjn5f` ⇒ contenido real de `/etc/hostname`.

**Arreglo propuesto**: reutilizar el patrón de `providers::attachment_allowed` (que HW-04 ya
aplicó a los adjuntos) en `resolve_target_or_file` y en `hex_inspect`: sólo `<state_dir>`,
`<uploads>`, `<extracted>` y `/sdcard/megadaemon/**`; el resto → 403.

---

### UI-06 · ALTA · la bóveda pierde keys y se corrompe con escrituras concurrentes

```
[providers] providers.json corrupto (EOF while parsing a value at line 1 column 0); copia en .../providers.json.corrupt-1795124917; se usa el catálogo por defecto
[UI-06] keys de proveedor guardadas: 2/12 -> ["race-p2", "race-p11"]
panicked at tests/audit_scout_ui.rs:617:
assertion `left == right` failed: UI-06: se perdieron 10 API keys por la carrera read-modify-write de la bóveda (load_vault/save_vault sin lock)
  left: 2
 right: 12
```

12 POST simultáneos → **10 keys perdidas** y el fichero `providers.json` queda truncado
(EOF al parsear). Consecutive runs: 4/12, 2/12, 3/12 → depende del scheduler, pero siempre roto.

**Arreglo propuesto**: en `providers.rs`, hacer `load_vault`+`update`+`save_vault` bajo un lock de
fichero (`<state_dir>/providers.json.lock` con `flock`), y escribir atómicamente
(fichero temporal + `rename`), igual que hace el `mesh_board.json`.

---

### UI-11 · ALTA · el freezer escribe fuera del sandbox (TOCTOU por symlink)

```
[UI-11] add lock -> 200 {"lock":{"active":true,"description":"Rust 60Hz lock",...
[UI-11] contenido de la víctima tras 600 ms: "AICTIM-ORIGINAL-CONTENT"
panicked at tests/audit_scout_ui.rs:935:
UI-11: el hilo del freezer siguió el symlink y escribió FUERA del sandbox: "AICTIM-ORIGINAL-CONTENT"
```

La víctima era `VICTIM-ORIGINAL-CONTENT`; el hilo freezer la reescribió como `AICTIM` ⇒
**escribió un byte fuera del sandbox** (el symlink cambió entre la validación y el `write`).

**Arreglo propuesto**: en `freezer.rs`, abrir el fichero con `O_NOFOLLOW` y comprobar
`fstat` (mismo `st_dev`/`st_ino`) justo antes de escribir; o validar el symlink **después**
de abrir y usar `pwrite` sobre el fd ya abierto.

---

### UI-05 · ALTA · guardar proveedor con key + endpoint descarta la key

```
[UI-05] POST /api/providers (key nueva + endpoint nuevo) -> 200
{"status":"OK","vault":{...,"providers":[{"auth_type":"none","enabled":true,"endpoint":"https://nuevo.invalid/v1/chat/completions","has_key":false,"id":"scout-p1","key_masked":"","ready":true,...}]}}
panicked at tests/audit_scout_ui.rs:545:
assertion `left == right` failed: UI-05: la API key que el usuario acababa de escribir se perdió al cambiar el endpoint
  left: Bool(false)
 right: Bool(true)
```

`auth_type:"none"` + `has_key:false` ⇒ la key del formulario **nunca se guardó**: el usuario
cree que ha configurado el proveedor y en realidad queda sin credencial (y sin aviso).

**Arreglo propuesto**: en `providers.rs::update_provider`, no sobrescribir `api_key` con `""`
cuando el campo llega vacío, y validar `auth_type` contra la presencia real de key.

---

### UI-08 · MEDIA · `id:""` → "OK" y la key se descarta en silencio

```
[UI-08] POST /api/providers {"id":"","api_key":"sk-ORPHAN-123456"} -> 200
{"status":"OK","vault":{"catalog_size":0,...,"providers":[]}
[UI-08] la bóveda contiene la key: false
panicked at tests/audit_scout_ui.rs:696:
UI-08: el servidor respondió OK pero la API key quedó descartada (id vacío); el usuario cree que la guardó
```

**Arreglo propuesto**: si `id` viene vacío, generar uno (`provider-<hash del endpoint>`) o
devolver `400` con mensaje explícito; nunca `200 OK` sin guardar.

---

### UI-01 · MEDIA · `/api/input` responde "ok" aunque no se haya escrito nada

```
[UI-01] GET /api/input?mask=16909060 (SIN token) -> 403          <- fail-closed OK
[UI-01] registro INPUT (offset 28) tras la petición = 0x0
[UI-01b] GET /api/input con la SHM ausente -> 200 {"mask":1,"status":"ok"}
panicked at tests/audit_scout_ui.rs:305:
UI-01: la respuesta dice 'ok' aunque la escritura en la SHM fue imposible: {"mask":1,"status":"ok"}
```

**Arreglo propuesto**: propagar el error de la escritura (`{"status":"error","error":"..."}`) en
lugar de `ok` incondicional.

---

### UI-07 · MEDIA · un proveedor deshabilitado sigue recibiendo peticiones (con su key)

```
[UI-07] peticiones recibidas por el proveedor DESHABILITADO: 1
[UI-07] cabeceras Authorization: ["Bearer sk-SCOUT-KEY-123"]
panicked at tests/audit_scout_ui.rs:663:
UI-07: un proveedor con enabled=false recibió 1 petición(es): el flag no desactiva al proveedor pedido explícitamente
```

**Arreglo propuesto**: en el despacho de `providers::execute_chat`, rechazar con 4xx los
proveedores con `enabled == false` (y no mandar su `Authorization`).

---

### UI-10 · MEDIA · el servidor MCP ignora los batches JSON-RPC

```
[UI-10] stdout del servidor MCP:
{"id":1,"jsonrpc":"2.0","result":{}}
[UI-10] ids respondidos: ["1"] · errores de parseo: 0
panicked at tests/audit_scout_ui.rs:871:
UI-10: el servidor MCP ignoró el batch JSON-RPC (id 2) y NO respondió: el cliente se queda colgado. ids respondidos: ["1"]
```

**Arreglo propuesto**: en `mcp_server.rs`, leer líneas hasta EOF, parsear cada una y responder
**una respuesta por id**; un array JSON-RPC → un array de respuestas.

---

### UI-13 · MEDIA · un refresco antiguo puede pisar al reciente (estado stale)

Harness de Node con dos `refreshHardware()` solapados, resolviendo primero la respuesta **nueva**
y después la **vieja**:

```
[UI-13] harness node (race): {"status_dot_class":"brand-dot degraded","stale":true}
panicked at tests/audit_scout_ui.rs:998:
UI-13: con dos refrescos solapados la UI conserva el estado del refresco ANTIGUO
```

**Arreglo propuesto**: numerar las peticiones (`reqId`) y descartar la respuesta si `reqId` es
antigua (`if (myId < lastAppliedId) return;`).

---

### UI-15 · MEDIA · `/health` público = reconocimiento del dispositivo

`/health` y `/api/ping` no exigen API key (`is_public_probe`, `cockpit_server.rs:764-770`) y el
túnel los sirve:

```json
{"status":"ok","uptime":…,"version":"…","web":{"bind":"0.0.0.0","primary_port":9090,
 "ports_open":[…],"token_materialized":true,
 "token_path":"/data/local/tmp/megadaemon/state/cockpit_token",
 "state_dir":"/data/local/tmp/megadaemon/state","local_ips":["<IP_DEL_MOVIL>"],"urls":[…]}}
```

No filtra el **valor** del token ni de la API key, pero sí la ruta del cofre de credenciales y el
mapa de puertos/IPs del móvil.

**Arreglo propuesto**: `/health` público = `status`, `uptime`, `version` y nada más; el detalle
(`web`) sólo con API key.

---

### UI-00 · CRÍTICA (ya corregida y verificada en el móvil) · RCE root desde cualquier web

El puente que hace funcionar la web del móvil en `:3000` **reintroducía** el hueco que cerraron
S08 y v13.2.3:

```python
# deploy/web_bridge.py:135-136 (ANTES) -> inyectaba el token de escritura real
if COCKPIT_TOKEN and not self.headers.get("X-Seere-Auth-Token"):
    req.add_header("X-Seere-Auth-Token", COCKPIT_TOKEN)
# deploy/web_bridge.py:150 (ANTES) -> CORS abierto a cualquier origen
self.send_header("Access-Control-Allow-Origin", "*")
```

Medido en el Samsung:

| Antes | Después del parche |
|-------|--------------------|
| `GET /` con `Origin: https://evil.example` → `200` + `Access-Control-Allow-Origin: *` + token | `403` + `Access-Control-Allow-Origin:` vacío |
| `POST /api/chat` con el token robado → **RCE root** | el token no se inyecta si la petición trae `Origin` |

Verificación post-parche (móvil, sin `Origin` = navegación normal):

```
A_TUNEL  (Origin == Host):  HTTP/1.1 200 OK + Access-Control-Allow-Origin: https://TUNEL.trycloudflare.com
B_LOCAL  (Origin == Host):  HTTP/1.1 200 OK + Access-Control-Allow-Origin: http://127.0.0.1:3000
C_POST    (UI con token):   HTTP/1.1 200 OK
D_SIN_ORIGIN:               HTTP/1.1 200 OK
```

Arreglo aplicado (commit `0cc1ca5`, repo **y** dispositivo, con backup `.bak-cors`):

```diff
-        if COCKPIT_TOKEN and not self.headers.get("X-Seere-Auth-Token"):
+        if COCKPIT_TOKEN and not self.headers.get("X-Seere-Auth-Token") and not self.headers.get("Origin"):
...
-        self.send_header("Access-Control-Allow-Origin", "*")
+        self.send_header("Access-Control-Allow-Origin", allowed_origin(self.headers))
```

`allowed_origin()` refleja el `Origin` sólo si su host == `Host` o está en
`MEGADAEMON_ALLOWED_ORIGINS` (mismo criterio que `cockpit_server.rs::cors_allowed`).

---

## 3. Ya corregido (tests de regresión que pasan)

| Test | Qué fija |
|---|---|
| `ui02_runners_control_exige_token_y_no_destruye_el_frame_vivo` | `/api/runners/control` → 403 sin token y el frame vivo sigue intacto (`tag="SCOUTFRAM\0\0\0" frame=[90;90;…]`, control en reposo idéntico) |
| `ui09_el_html_publico_no_debe_contener_el_token_de_autenticacion` | `GET /` → `window.MEGADAEMON_TOKEN=""` (no filtra) y `POST /api/providers` sin credenciales → 403 |
| `ui12_la_ui_no_puede_ejecutar_js_del_id_de_proveedor` | harness XSS: `{"xss_executed":false,"injected_handlers":[],"rows":0}` |
| `ui14_guardar_una_key_no_debe_perder_el_proveedor_seleccionado` | harness vaultsel: `{"before":"prov-a","after":"prov-b","selection_preserved":true}` |

---

## 4. `cockpit_doctor` (nuevo) y despliegue

`rust-core/src/cockpit_doctor.rs` es **puro** (sin I/O propio): recibe un `DoctorEnv` y un
trait `Probe`, y devuelve `Health::{Ok, NotOperational, Down}`. El estado `not_operational`
cubre el fallo invisible que.chromium causaba el `ERR_CONNECTION_REFUSED` del usuario
("el proceso está vivo pero `/health` no contesta").

Salida real (test de humo):

```json
{"healthy":true,"http_status":200,"port":9090,"process_alive":true,
 "summary":"estado del cockpit: OPERATIVO\nbinario: presente (doctor 13.4.0)\nproceso: vivo\n
 puerto 3000: nadie escucha\n…\npuerto 9090: OK (/health 200)\nplantilla web: disco (…)",
 "token_present":true,"template_source":"disco"}
```

`deploy/install-phone.sh` (fusionado con la versión de `repo_updates`):

1. Diagnóstico previo (proceso, puertos, plantilla, token, log).
2. **SHA-256 del binario** calculado en el sandbox y **verificado dentro del móvil**; si no
   coincide → se restaura el `.bak` y se aborta.
3. Copia de la plantilla, instalación del supervisor (`99-megadaemon.sh`, 755).
4. **Espera activa** de `/health` (timeout 45 s, `sleep 2` entre intentos) exigiendo **200 exacto**
   (los 6 s fijos anteriores daban falsos negativos; `toybox wget && echo 200` daba falsos OK con 404).
5. Si no arranca → **ROLLBACK** al binario anterior + motivo en el log + código de salida ≠ 0.

```bash
bash -n deploy/install-phone.sh   # sintaxis verificada
```

---

## 5. Estado real del Samsung (medido con `root_exec` vía el MCP)

- El daemon **está vivo**: `127.0.0.1:{3000,3030,8080,8090,9090}` en LISTEN.
- `/health` → `200 {"status":"ok","uptime":12040,"version":"13.3.0"}`.
- Plantilla desplegada: `/data/local/tmp/megadaemon/web/cockpit_template.html` (89.599 B) →
  `GET /` devuelve 89.707 B con el token inyectado (**v13.3.0**: el token aún va en la página;
  con v14 haría falta UI-16 arreglado **antes** de desplegar).
- `/api/ping` sin token de sesión → **403** (fail-closed ✓).
- `/mcp` → 200 con **25** herramientas.
- Túnel: `GET /` sin key → 401; con key → 200 + HTML.
- A las 20:29-20:30 (las capturas del usuario) no había daemon escuchando → `ERR_CONNECTION_REFUSED`
  = proceso muerto, no bug de la UI. El supervisor relanza con backoff 10→300 s sin explicar el motivo:
  por eso `cockpit_doctor` distingue `down` de `not_operational`.

> ⚠️ **Antes del próximo despliegue hay que arreglar UI-16**: el binario v14 (sin el token en la
> página) deja la UI sin datos. Orden recomendado: (1) aplicar el arreglo de UI-16,
> (2) `cargo build --release --target aarch64-linux-android`, (3) `deploy/install-phone.sh`.

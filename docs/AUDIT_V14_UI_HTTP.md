# MegaDaemon v14 — TERCERA auditoría adversarial: capa HTTP, proveedores, MCP y UI web

> **Autor**: agente *MegaDaemon-Cockpit*.
> **Alcance**: `rust-core/src/cockpit_server.rs`, `rust-core/src/providers.rs`,
> `rust-core/src/mcp_server.rs`, `rust-core/src/shm.rs`, `rust-core/src/freezer.rs`,
> `rust-core/src/hw_mapper.rs` y `web/cockpit_template.html`.
> **NO solapa** con `tests/audit_scout_hw.rs` (HW-01…HW-16) ni con
> `tests/audit_scout_seere.rs` (S01…S14b): no se prueba el RCE sin token de
> `/api/chat`, el CORS abierto, el sandbox estático de `memory_freeze`, el
> adjunto de `providers.rs`, la key atada al endpoint, el `Content-Length`, la
> cabecera SHM en `npu_cv`, los modelos retirados ni la gramática de SEERE.
>
> **Regla aplicada**: cada hallazgo se reproduce con un test ejecutable que
> FALLA con el código actual (`rust-core/tests/audit_scout_ui.rs`).  Los tests
> que pasan (defecto ya corregido por el agente principal) se listan aparte y
> **no se reportan como hallazgo**.

---

## 1. Resumen

| ID | Severidad | Fichero:línea | Defecto |
|----|-----------|---------------|---------|
| UI-01 | MEDIA | `cockpit_server.rs:1195-1201` | `/api/input` responde `{"status":"ok"}` aunque la escritura en el registro de mando de la SHM sea imposible |
| UI-03 | ALTA | `cockpit_server.rs:1203` + `shm.rs:75` | La cabecera de la SHM controla una asignación de hasta 12,9 GB en `/api/live_frame`: panic por overflow en debug, **abort del proceso entero en release** (`panic = "abort"`) |
| UI-04 | ALTA | `cockpit_server.rs:249` (`resolve_target_or_file`) | Lectura arbitraria de ficheros por los endpoints RE (`/api/hex_inspect`, `/api/die_binwalk`, …); el token sale de la página **pública** |
| UI-05 | ALTA | `providers.rs:186-217` | Guardar key + endpoint nuevo **borra la key** y deja `auth_type = "none"` para siempre: el proveedor queda muerto y la UI lo marca `[READY]` |
| UI-06 | ALTA | `providers.rs:76-110` | `load_vault()`/`save_vault()` sin lock → las actualizaciones concurrentes de la bóveda **pierden API keys** |
| UI-07 | MEDIA | `providers.rs:671-679` | `enabled = false` **no desactiva** el proveedor pedido explícitamente (se sigue usando y facturando) |
| UI-08 | MEDIA | `cockpit_server.rs:1397` + `providers.rs:235-255` | `POST /api/providers` con `id` vacío responde `OK` y **descarta la key** en silencio |
| UI-09 | ALTA | `cockpit_server.rs:756-770` | El token de sesión se **publica en el HTML sin autenticar**; con el bind a `0.0.0.0` (AUDIT-WEB-04) cualquiera de la LAN lo lee y ejecuta `/api/chat` |
| UI-10 | MEDIA | `mcp_server.rs:186-189` | El servidor MCP **no responde** a peticiones JSON-RPC malformadas ni a lotes → el cliente MCP se queda colgado |
| UI-11 | ALTA | `freezer.rs:137-156` + `freezer.rs:92` | TOCTOU: se valida la ruta canónica pero el lock guarda la ruta **cruda** → un symlink cambiado después evade el sandbox y el hilo de 60 Hz escribe fuera |
| UI-13 | MEDIA | `web/cockpit_template.html:1528` | Carrera de refresco: un `refreshHardware()` viejo pisa al nuevo → la UI queda con estado obsoleto |
| UI-14 | MEDIA | `web/cockpit_template.html:893-897` | `loadProviders()` reconstruye el `<select>` de la bóveda → tras guardar, la acción siguiente se aplica al **proveedor equivocado** |

Además (fuera de la suite,”的要求 del despliegue, ver §4): el puerto **8080** que
probó el usuario no es alias, el puerto principal ocupado tumba el arranque
completo, y la API key remota no está forzada por defecto.

---

## 2. Hallazgos (con reproducción real)

> Salida real de `cargo test --test audit_scout_ui -- --nocapture --test-threads=1`.
> Cada bloque es el `stdout` del test que falla.

### UI-01 · MEDIA · `cockpit_server.rs:1195-1201` — respuesta "ok" sin escritura

`/api/input` escribe la máscara de botones en el registro `INPUT_REG_OFFSET = 28`
de la SHM (lo lee `re_pipeline/snatcher_headless_driver.py`) y **siempre**
responde `{"status":"ok"}`: `let _ = f.write_all(...)` descarta el error.

```rust
"/api/input" => {
    let mask = qs.get("mask").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    if let Ok(mut f) = OpenOptions::new().read(true).write(true).open(shm::SHM_PATH) {
        let _ = f.seek(SeekFrom::Start(28));
        let _ = f.write_all(&mask.to_le_bytes());   // <- el error se tira
    }
    send_json(&mut stream, &json!({"status": "ok", "mask": mask}));  // <- siempre "ok"
}
```

**Arreglo propuesto**

```diff
--- a/rust-core/src/cockpit_server.rs
+++ b/rust-core/src/cockpit_server.rs
@@
             "/api/input" => {
                 let mask = qs.get("mask").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
-                if let Ok(mut f) = OpenOptions::new().read(true).write(true).open(shm::SHM_PATH) {
-                    let _ = f.seek(SeekFrom::Start(28));
-                    let _ = f.write_all(&mask.to_le_bytes());
-                }
-                send_json(&mut stream, &json!({"status": "ok", "mask": mask}));
+                let mut f = match OpenOptions::new().read(true).write(true).open(shm::SHM_PATH) {
+                    Ok(f) => f,
+                    Err(e) => {
+                        send_json(&mut stream, &json!({
+                            "status": "error",
+                            "message": format!("no se pudo abrir la SHM: {}", e)
+                        }));
+                        return;
+                    }
+                };
+                if let Err(e) = f
+                    .seek(SeekFrom::Start(28))
+                    .and_then(|_| f.write_all(&mask.to_le_bytes()))
+                {
+                    send_json(&mut stream, &json!({
+                        "status": "error",
+                        "message": format!("no se pudo escribir el registro de mando: {}", e)
+                    }));
+                    return;
+                }
+                send_json(&mut stream, &json!({"status": "ok", "mask": mask}));
             }
```

### UI-03 · ALTA · `cockpit_server.rs:1203` + `shm.rs:75` — la SHM tumba el cockpit

`/api/live_frame` usa `shm::read_shm_status()` (que **no valida** las
dimensiones, al contrario que `shm::read_frame_snapshot()`) y luego llama a
`shm::encode_bmp_24(w, h, …)`, donde `row_size * height` desborda `u32` con
65535×65535.  En debug: panic del hilo (la conexión muere sin respuesta); en
**release** (`panic = "abort"` en `Cargo.toml`): **aborta el proceso entero**, es
decir, el cockpit del móvil desaparece.

```rust
// shm.rs:73-75
pub fn encode_bmp_24(width: u32, height: u32, rgb_data: &[u8]) -> Vec<u8> {
    let row_size = ((width * 3 + 3) / 4) * 4;
    let pixel_array_size = row_size * height;      // <- 196608 * 65535 desborda u32
```

**Arreglo propuesto**: validar las dimensiones en `read_shm_status()` (o usar
`read_frame_snapshot()`, que ya las valida) y saturar el producto en `encode_bmp_24`.

```diff
--- a/rust-core/src/shm.rs
+++ b/rust-core/src/shm.rs
@@
 pub fn encode_bmp_24(width: u32, height: u32, rgb_data: &[u8]) -> Vec<u8> {
-    let row_size = ((width * 3 + 3) / 4) * 4;
-    let pixel_array_size = row_size * height;
-    let file_size = 54 + pixel_array_size;
+    let w = width.min(MAX_FRAME_DIM);
+    let h = height.min(MAX_FRAME_DIM);
+    let row_size = ((w * 3 + 3) / 4) * 4;
+    let pixel_array_size = (row_size as u64 * h as u64).min(MAX_FRAME_BYTES) as u32;
+    let file_size = 54u32.saturating_add(pixel_array_size);
```

```diff
--- a/rust-core/src/cockpit_server.rs
+++ b/rust-core/src/cockpit_server.rs
@@
             "/api/live_frame" | "/api/live_frame_jpg" | "/api/live_frame.jpg" => {
                 let bmp_bytes = match shm::read_shm_status() {
-                    Ok(hdr) if hdr.is_active && hdr.width > 0 && hdr.height > 0 => {
+                    Ok(hdr) if hdr.is_active
+                        && hdr.width > 0 && hdr.height > 0
+                        && hdr.width as u32 <= shm::MAX_FRAME_DIM
+                        && hdr.height as u32 <= shm::MAX_FRAME_DIM => {
```

### UI-04 · ALTA · `cockpit_server.rs:249` — lectura arbitraria de ficheros

`resolve_target_or_file()` acepta **cualquier** ruta absoluta existente y la
pasa a `disasm::inspect_hex()` (y a DiE/Binwalk, Kaitai, entropy, pointers…),
que devuelven el **contenido** (hex + ASCII) del fichero.  HW-04 corrigió este
mismo primitive en el camino del adjunto (`providers::attachment_allowed`), pero
**no** en los endpoints RE del cockpit: la corrección quedó incompleta.

El token no protege nada: sale de la página **pública** `GET /` (ver UI-09).

```rust
fn resolve_target_or_file(plat: &str, explicit_file: Option<&str>) -> (String, String, String) {
    if let Some(fp) = explicit_file {
        let trimmed = fp.trim();
        if !trimmed.is_empty() && Path::new(trimmed).exists() {   // <- cualquier ruta
            ...
```

**Arreglo propuesto**: reutilizar el sandbox de los adjuntos.

```diff
--- a/rust-core/src/cockpit_server.rs
+++ b/rust-core/src/cockpit_server.rs
@@
 fn resolve_target_or_file(plat: &str, explicit_file: Option<&str>) -> (String, String, String) {
     if let Some(fp) = explicit_file {
         let trimmed = fp.trim();
-        if !trimmed.is_empty() && Path::new(trimmed).exists() {
+        if !trimmed.is_empty()
+            && Path::new(trimmed).exists()
+            && providers::attachment_allowed(trimmed)   // sandbox ya auditado (HW-04)
+        {
```

(Alternativa: un `target_allowed()` propio con las raíces
`MEGADAEMON_DIR`, `uploads/`, `extracted/` y `re_pipeline/`.)

### UI-05 · ALTA · `providers.rs:186-217` — la key se destruye al cambiar el endpoint

En `update_provider()` el bloque de `api_key` se aplica **antes** que el de
`endpoint`; el bloque de endpoint vacía la key si cambia el host y pone
`auth_type = "none"`.  Como `auth_type` **nunca se restaura**, `execute_chat()`
hace `match p.auth_type { "none" => String::new() }` ⇒ la key nunca se envía ⇒
**401 para siempre**.  Y `list_providers_redacted()` marca `ready = true` para
`"none"` ⇒ la UI muestra `[READY]` sobre un proveedor muerto.

```rust
if let Some(k) = api_key {           // 1) el usuario escribe la key
    p.api_key = k.trim().to_string();
    ...
}
if let Some(ep) = endpoint {
    let nuevo = ep.trim();
    if p.endpoint.trim() != nuevo && !p.api_key.trim().is_empty() {
        p.api_key = String::new();     // 2) ... y aquí se borra
        p.auth_type = "none".to_string();  // 3) ... y aquí queda muerto para siempre
    }
```

**Arreglo propuesto**

```diff
--- a/rust-core/src/providers.rs
+++ b/rust-core/src/providers.rs
@@
     for p in &mut vault.providers {
         if p.id == id {
+            let mut key_recibida = false;
             if let Some(k) = api_key {
                 p.api_key = k.trim().to_string();
                 if !p.api_key.is_empty() {
                     p.enabled = true;
                     p.tier = "configured".to_string();
+                    // Una key nueva re-habilita la autenticación del proveedor.
+                    if p.auth_type == "none" {
+                        p.auth_type = "bearer".to_string();
+                    }
+                    key_recibida = true;
                 }
             }
@@
-                    if p.endpoint.trim() != nuevo && !p.api_key.trim().is_empty() {
+                    if p.endpoint.trim() != nuevo && !p.api_key.trim().is_empty() && !key_recibida {
                         p.api_key = String::new();
                         p.auth_type = "none".to_string();
                     }
```

Y en `list_providers_redacted()`: `"none"` sólo es `ready` si **no** hay modelo
que proteger.

```diff
             let ready = match p.auth_type.as_str() {
-                "local" | "none" | "hybrid" => true,
+                "local" | "hybrid" => true,
+                "none" => p.api_key.trim().is_empty() && p.tier != "configured",
```

### UI-06 · ALTA · `providers.rs:76-110` — la bóveda pierde keys en concurrencia

`load_vault()` → modificar → `save_vault()` no está serializado: dos
`POST /api/providers` simultáneos se pisan y el último escritor deja fuera las
keys del otro.  Es una pérdida silenciosa de credenciales del usuario.

```rust
pub fn load_vault() -> ProviderVault { ... }          // read
pub fn save_vault(vault: &ProviderVault) -> Result<(), String> {
    ...
    fs::write(PROVIDERS_PATH, text).map_err(|e| e.to_string())   // write (no atómico)
}
```

**Arreglo propuesto**: un lock global + escritura atómica (fichero temporal +
`rename`).

```diff
--- a/rust-core/src/providers.rs
+++ b/rust-core/src/providers.rs
@@
 use std::sync::Mutex;
 use std::sync::OnceLock;
+
+/// Serializa el ciclo read-modify-write de la bóveda (y hace la escritura
+/// atómica): dos POST /api/providers simultáneos se pisaban y se perdían keys.
+static VAULT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
+fn vault_lock() -> &'static Mutex<()> {
+    VAULT_LOCK.get_or_init(|| Mutex::new(()))
+}
@@
 pub fn save_vault(vault: &ProviderVault) -> Result<(), String> {
+    let _g = vault_lock().lock().unwrap_or_else(|p| p.into_inner());
     if let Some(parent) = Path::new(PROVIDERS_PATH).parent() {
         let _ = fs::create_dir_all(parent);
     }
     let text = serde_json::to_string_pretty(vault).map_err(|e| e.to_string())?;
-    fs::write(PROVIDERS_PATH, text).map_err(|e| e.to_string())
+    let tmp = format!("{}.tmp", PROVIDERS_PATH);
+    fs::write(&tmp, text).map_err(|e| e.to_string())?;
+    fs::rename(&tmp, PROVIDERS_PATH).map_err(|e| e.to_string())
 }
```

### UI-07 · MEDIA · `providers.rs:671-679` — `enabled=false` no desactiva nada

El primer candidato de la cascada se empuja **sin mirar `enabled`**: un
proveedor desactivado sigue recibiendo peticiones (y gastando cuota) cuando se
pide explícitamente.  La UI, además, lista todos los proveedores en el desplegable
sin marcar los desactivados.

```rust
if let Some(p) = vault.providers.iter().find(|p| p.id == actual_cloud_id) {
    candidates.push(p.clone());          // <- sin comprobar p.enabled
}
```

**Arreglo propuesto**

```diff
-    if let Some(p) = vault.providers.iter().find(|p| p.id == actual_cloud_id) {
-        candidates.push(p.clone());
+    if let Some(p) = vault
+        .providers
+        .iter()
+        .find(|p| p.id == actual_cloud_id && p.enabled)
+    {
+        candidates.push(p.clone());
```

(y en la UI: excluir del desplegable los proveedores con `enabled == false` o
mostrarles una etiqueta `[OFF]`.)

### UI-08 · MEDIA · `cockpit_server.rs:1397` + `providers.rs:235-255` — key huérfana

`POST /api/providers` con `id` vacío responde `{"status":"OK"}` y **no guarda
la key** (no casa con ningún proveedor y la creación de uno nuevo se salta
porque el id está vacío).  El usuario ve un toast verde y su key ha
desaparecido.

**Arreglo propuesto**

```diff
--- a/rust-core/src/providers.rs
+++ b/rust-core/src/providers.rs
@@
 pub fn update_provider(
@@
 ) -> Result<Value, String> {
+    if id.trim().is_empty() {
+        return Err("FALTA el id del proveedor: la API key no se ha guardado".to_string());
+    }
     let mut vault = load_vault();
```

### UI-09 · ALTA · `cockpit_server.rs:756-770` — el token se publica

`GET /` es público (sólo se salta el gate de **API key**, no el de token) y
lleva el token embebido: `window.MEGADAEMON_TOKEN="…"`.  Con el bind a
`0.0.0.0` (AUDIT-WEB-04) **cualquier equipo de la LAN** puede hacer:

```
GET /                      -> 200 + window.MEGADAEMON_TOKEN="<token>"
POST /api/chat             -> {"message":"$ id"}   con X-Seere-Auth-Token: <token>
```

y ejecutar órdenes **como root** en el móvil.  La única barrera sería la API
key remota, pero `api_key::decide()` devuelve `Allow` si no hay key configurada
(`cfg.enabled == false`), que es el caso por defecto.

**Arreglo propuesto** (dos opciones, la segunda es la completa):

1. No publicar el token: servirlo sólo tras validar la API key.
2. Con el bind a `0.0.0.0`, **forzar** la API key (`MEGADAEMON_REQUIRE_API_KEY=1`)
   y generar una key si falta, para que la página+y el token no valgan sin key.

```diff
--- a/rust-core/src/cockpit_server.rs
+++ b/rust-core/src/cockpit_server.rs
@@
                 let inject = format!(
                     "<script>window.MEGADAEMON_TOKEN=\"{}\";</script>",
                     auth_token()
                 );
```
→ el token debe inyectarse **sólo** si la petición trae la API key válida
(`api_key::Decision::Allow`), no en la rama `Unauthorized if is_public_page`.

### UI-10 · MEDIA · `mcp_server.rs:186-189` — el cliente MCP se cuelga

`run_stdio_mcp_server()` descarta en silencio cualquier línea que no sea JSON
válido y **no soporta lotes**.  JSON-RPC 2.0 exige responder con un error
`-32700` (parse error) y responder a los lotes; el cliente MCP (Antigravity) se
queda esperando una respuesta que nunca llega.

```rust
let req: Value = match serde_json::from_str(trimmed) {
    Ok(v) => v,
    Err(_) => continue,          // <- ni error ni respuesta: el cliente espera para siempre
};
```

**Arreglo propuesto**

```diff
--- a/rust-core/src/mcp_server.rs
+++ b/rust-core/src/mcp_server.rs
@@
         let req: Value = match serde_json::from_str(trimmed) {
             Ok(v) => v,
-            Err(_) => continue,
+            Err(e) => {
+                write_jsonrpc(&json!({
+                    "jsonrpc": "2.0",
+                    "id": serde_json::Value::Null,
+                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
+                }));
+                continue;
+            }
         };
+        if req.is_array() {
+            // JSON-RPC 2.0 batch: una respuesta por petición.
+            for item in req.as_array().cloned().unwrap_or_default() {
+                handle_one(item);
+            }
+            continue;
+        }
```

### UI-11 · ALTA · `freezer.rs:137-156` — TOCTOU en el memory freezer

`add_lock()` valida la ruta **canónica** (`validate_target()`), pero guarda la
ruta **cruda** en el lock; el hilo de 60 Hz reabre `lock.target` cada 16 ms.
Si alguien sustituye el fichero por un symlink después de la validación, el
daemon (root) escribe **fuera del sandbox**.  Es la evasión del control que
HW-02 introdujo, por un camino distinto.

**Arreglo propuesto**: guardar la ruta canónica y reabrir por descriptor
(`O_NOFOLLOW`), o validar en cada escritura.

```diff
--- a/rust-core/src/freezer.rs
+++ b/rust-core/src/freezer.rs
@@
     validate_target(target)?;
+    // Guardar la ruta CANÓNICA: el hilo de 60 Hz reabre el fichero por su
+    // nombre y un symlink cambiado después de la validación escapaba del
+    // sandbox (TOCTOU).
+    let canonical = std::fs::canonicalize(target).map_err(|e| e.to_string())?;
+    let target = canonical.to_string_lossy().to_string();
```

(y en el hilo, abrir con `O_NOFOLLOW` para symlinks).

### UI-13 · MEDIA · `web/cockpit_template.html:1528` — estado obsoleto por carrera

`refreshHardware()` se llama cada 12 s y también al arrancar.  Si dos llamadas
se solapan, la respuesta **antigua** (que llega la última) pisa a la nueva:
el punto de estado y las cabeceras quedan con el valor de una lectura
obsoleta; y el `catch` de una llamada antigua puede dejar el punto en
`offline` después de que otra haya funcionado.

**Arreglo propuesto**: numerar las ejecuciones y descartar las respuestas
antiguas.

```diff
--- a/web/cockpit_template.html
+++ b/web/cockpit_template.html
@@
 async function refreshHardware() {
+  const seq = ++STATE.hwSeq;          // contador de refrescos
   try {
     const b = await apiGet("/api/brain_status");
+    if (seq !== STATE.hwSeq) return;  // hay un refresco más nuevo: se descarta
     const dot = document.getElementById("statusDot");
@@
   } catch (e) {
+    if (seq !== STATE.hwSeq) return;
     const dot = document.getElementById("statusDot");
```

### UI-14 · MEDIA · `web/cockpit_template.html:893-897` — el proveedor equivocado

`loadProviders()` vacía y reconstruye `#vaultProviderId` (`vsel.innerHTML = ""`),
así que tras `saveProviderKey()` (que llama a `loadProviders()`) el desplegable
vuelve al **primer** proveedor.  El siguiente clic ("SET DEFAULT", "GUARDAR
KEY") se aplica al proveedor equivocado, sin que nada lo indique.

**Arreglo propuesto**: conservar la selección, como ya se hace con
`#modelSelect`.

```diff
--- a/web/cockpit_template.html
+++ b/web/cockpit_template.html
@@
     const sel = document.getElementById("modelSelect");
     const vsel = document.getElementById("vaultProviderId");
     const prev = sel.value || STATE.defaultModel;
+    const prevVault = vsel.value;           // <- la selección de la bóveda también se conserva
     sel.innerHTML = "";
     vsel.innerHTML = "";
@@
     if ([...sel.options].some(o => o.value === prev)) {
       sel.value = prev;
     } else {
       sel.value = STATE.defaultModel;
     }
+    if ([...vsel.options].some(o => o.value === prevVault)) {
+      vsel.value = prevVault;
+    }
```

---

## 3. Defectos YA CORREGIDOS (el test pasa; no se reportan como hallazgo)

| Ref | Qué se comprobó | Estado |
|-----|------------------|--------|
| UI-02 | `POST /api/runners/control` sin token destruía el frame vivo de la VLA | **Corregido** (v13.2.3: `requires_auth()` es fail-closed; 403 + SHM intacta) |
| UI-12 | XSS por el `id` de proveedor interpolado en un `onclick` inline | **Corregido** (el botón TEST usa `data-test-provider` + delegación de eventos; además se bloquean los esquemas `javascript:`) |
| — | El token en la página / CORS / adjunto / key-atada-al-endpoint / `Content-Length` / cabecera SHM en `npu_cv` | Ya cubiertos por HW-01…HW-16 y S01…S14b (no repetidos) |

---

## 4. Lo que NO es un bug del código (y por qué la web del móvil daba `ERR_CONNECTION_REFUSED`)

1. **El daemon no estaba escuchando.**  No es un fallo de la UI: `ERR_CONNECTION_REFUSED`
   significa que no hay proceso en ese puerto.  El supervisor (`deploy/99-megadaemon.sh`)
   lo relanza con backoff 10→300 s, pero el motivo del fallo **no era visible**
   en ningún sitio → de ahí `src/cockpit_doctor.rs` (estado `not_operational`).
2. **El puerto 8080 no existe.**  El cockpit (v13.4) escucha en `0.0.0.0` en el
   puerto pedido **y** en los alias `3000, 3030, 8090, 9090`.  El 8080 que
   probó el usuario no está en la lista → seguirá dando `refused`.
   *Arreglo*: añadir `8080` a `ALIAS_PORTS` (`cockpit_server.rs:1747`) o
   documentar la URL única.
3. **Si el puerto principal está ocupado, el cockpit no arranca.**  `run_server()`
   devuelve `Err` si falla el bind del puerto principal, aunque 3 alias estén
   libres (`cockpit_server.rs:1705-1707`) → el supervisor entra en bucle de
   relanzamiento y TODOS los puertos siguen muertos.  *Arreglo*: si el puerto
   principal falla, continuar con los alias y avisar por stderr.
4. **La API key remota no está forzada por defecto.**  `api_key::decide()`
   devuelve `Allow` si no hay key configurada; con el bind a `0.0.0.0` eso deja
   el cockpit abierto a la LAN (y el token en la página pública, UI-09).
   *Arreglo*: `MEGADAEMON_REQUIRE_API_KEY=1` en `megadaemon.env` + generar la
   key si falta.
5. **El túnel público responde 401 en todo** (verificado desde el sandbox):
   `/health`, `/api/ping`, `/metrics`, `/` y `/mcp` → `{"error":"unauthorized"}`.
   El daemon **está vivo** (el túnel responde), pero el binario desplegado aún
   no trae el arreglo AUDIT-WEB-02 (la página pública se servía **después** del
   gate de API key) → hay que volver a subir el binario ARM64 con la plantilla
   embebida.

---

## 5. Ficheros

| Fichero | Contenido |
|---------|-----------|
| `rust-core/tests/audit_scout_ui.rs` | 14 tests de integración (HTTP real contra el cockpit en un puerto libre + harness de Node para el JavaScript de la UI) |
| `rust-core/src/cockpit_doctor.rs` | módulo de diagnóstico (puro, sondas inyectables) |
| `rust-core/tests/cockpit_doctor_smoke.rs` | 7 tests de humo del doctor |
| `deploy/install-phone.sh` | despliegue + reparación idempotente con SHA-256, espera activa de `/health` y rollback |

# AUDITORÍA ADVERSARIAL DE LA PARTE DE CHAT/LLM — MegaDaemon v16

* **Fecha:** 2026-11-22
* **Rama:** `agent/llm-testing-v16` · **Base:** `origin/main` @ `f43a010`
* **Alcance:** `rust-core/src/providers.rs`, `llm_gateway.rs`, `quota_guard.rs`,
  `provider_catalog.rs`, `service_bus.rs` (presupuesto), `mcp_server.rs`
  (tool `minimax_chat`), `mcp_ext.rs`, `main.rs` (CLI `chat`), `agent_coding.rs`
  (tool-calling nativo).
* **Método:** lectura del código **+ lectura del código fuente de `ureq 3.5.2`
  en el registry local** (`~/.cargo/registry/src/.../ureq-3.5.2/`) **+
  reproducciones ejecutables** escritas como tests **sin red** (el transporte
  HTTP se inyecta).
* **Regla del repo:** en este sandbox **nadie compila** (`docs/PROTOCOLO_FLOCKA.md`).
  Los tests de este documento están escritos y los ejecuta el root.

> Los defectos de este documento son **REALES y reproducibles**: cada uno
> tiene el fragmento de código que lo demuestra y (donde se puede) el test
> que lo fija. No se ha inventado nada: lo que no se pudo verificar está
> marcado como **NO VERIFICADO** en el §7.

---

## 0. Resumen ejecutivo

| | |
|---|---|
| Defectos **REALES** encontrados | **18** |
| Defectos **arreglados** en esta rama | **15** |
| Defectos documentados y **NO** arreglados (decisión) | **3** |
| Tests nuevos | **58** (44 en `llm_routing.rs` + 14 en `llm_gateway_openai.rs`) |
| Tests del crate (`rust-core/src` + `rust-core/tests`) | **1574 → 1632** (+58) |
| Ficheros de producción tocados | `providers.rs`, `llm_gateway.rs`, `quota_guard.rs`, `main.rs`, `mcp_server.rs`, `agent_coding.rs` |
| Red usada por los tests | **cero** (transporte inyectado) |
| Credenciales en el repo | **cero** (las del banco de fixtures son ficticias) |

**Los tres hallazgos que más duelen:**

1. **El status HTTP se perdía** (`ureq` convierte los 4xx en `Err` por
   defecto) ⇒ el diagnóstico de 401/402/404/429 era **código muerto** y el
   `quota_guard` **nunca veía el 429** ⇒ **nadie se pausaba nunca** y el
   daemon insistía con el proveedor que estaba rate-limiteando.
2. **La API key del proveedor podía acabar en la respuesta del cliente**:
   el cuerpo de error crudo del proveedor se concatenaba en el `reply` del
   fallback offline.
3. **El presupuesto no protegía nada**: `execute_chat` hace su propia
   cascada y **nunca consulta** el presupuesto del `service_bus`.

---

## 1. Las seis preguntas, respondidas

### (a) ¿Se filtra alguna credencial en logs, errores, `list_providers_redacted` o la traza de routing?

**SÍ, en los errores y en la traza de routing** (y sólo ahí).

* **Logs:** **NO** hay ninguna fuga. En `providers.rs` la credencial sólo
  aparece en la cabecera `Authorization`/`x-api-key` del POST; no hay ningún
  `eprintln!`/`println!` que la imprima (verificado con grep sobre
  `providers.rs`, `llm_gateway.rs`, `quota_guard.rs`).
* **`list_providers_redacted`:** **NO** filtra la credencial. Devuelve
  `key_masked` = 2 primeros + 2 últimos caracteres (o `***` si la clave
  tiene menos de 12), más `has_key`/`ready`. **Riesgo residual:** 4
  caracteres de una clave larga (decisión de v13.2 HW-12, no se cambia aquí).
* **Errores:** **SÍ, era una fuga real** → **DEFECTO-02**.
  *Reproduction:* un proveedor que devuelve
  `{"error":{"message":"Invalid API key: sk-…"}}` ⇒ el mensaje acababa en
  `errors` ⇒ en `reply` (`"[Fallback offline activo — errores cloud: …]"`)
  ⇒ en el cliente (CLI, `/api/chat`, tool MCP `minimax_chat`).
* **Traza de routing:** la traza nueva (`ChatResult.provider_trace`) sólo
  lleva id de proveedor + motivo, y los motivos salen de
  `ProviderError::redacted()` ⇒ **sin credenciales** (test:
  `la_api_key_del_proveedor_no_llega_al_cliente`).

### (b) ¿El fallback puede caer en un proveedor NO habilitado, de pago, o **retirado** sin avisar?

| caso | antes | ahora |
|---|---|---|
| proveedor **no habilitado** | **NO** (la cascada exigía `enabled` para los que no son el pedido) → correcto | igual |
| proveedor **retirado** | **SÍ, en silencio** → **DEFECTO-03** | se salta y **se dice en la traza** |
| proveedor **de pago** | **SÍ, en silencio** → **DEFECTO-18** | la traza dice el **tier**; el presupuesto puede vetarlo (opt-in) |
| proveedor pedido pero **deshabilitado** | se llamaba (por diseño: el usuario lo pidió) | igual, y la traza lo deja ver |

El aviso de "MODELO RETIRADO" (v13.2 HW-13) sólo cubría el modelo **PEDIDO**:
el resto de la cascada gastaba en un proveedor retirado sin decir nada.

### (c) ¿El presupuesto del `service_bus` se respeta con llamadas concurrentes?

**No: no lo consulta nadie.** → **DEFECTO-11**.

* `service_bus::Budget` sólo se usa en `ServiceBus::route_with`, y **el bus se
  crea "al vuelo" en cada llamada** (`mcp_ext.rs:2071`,
  `voice_runtime.rs:939`) ⇒ el gasto siempre empieza a cero ⇒ el tope no
  protege nada.
* `providers::execute_chat` (el camino REAL del chat) **no mira el
  presupuesto**: hace su propia cascada sin coste.
* Donde sí se usa (`route_with`), el `Budget` es un `&mut self` ⇒ dentro de
  una instancia no hay carrera; pero el **check→call→spend** no es atómico:
  dos llamadas concurrentes sobre el MISMO bus (con `Mutex`) se serializan
  (el `&mut self` bloquea el bus entero, red incluida) ⇒ no hay overtaking,
  pero sí **bloqueo**: la llamada de red se hace **con el cerrojo del bus
  cogido**.
* **Arreglado:** la cascada consulta ahora una puerta de presupuesto
  (`ChatDeps::budget_permite`), **opt-in** por variable de entorno
  (`MD_BUDGET_DAY_USD`) para no cambiar el comportamiento del móvil sin
  decisión del root; el gasto se anota en un acumulador global con mutex
  (el **acumulado** es exacto; el **corte** es consultivo porque el coste
  real no se conoce hasta después de la llamada).

### (d) ¿`update_provider` valida lo que dice validar y no corrompe el vault?

**No en los dos puntos.** → **DEFECTO-06** y **DEFECTO-07**.

* **Lo que dice validar:** la clave queda atada al endpoint (HW-03) ⇒
  eso sí funciona. Pero **el endpoint no se valida en absoluto**: se
  aceptaba `file://…`, `http://169.254.169.254/latest/meta-data/` (los
  metadatos de la nube), `http://user:pass@host/…`… El comentario del
  código ("el endpoint HTTP del cockpit no tiene autenticación") está
  **DESACTUALIZADO**: desde v13.2.3 el cockpit es *fail-closed* y exige
  `X-Seere-Auth-Token` ⇒ la exposición real es "un cliente **legítimo** (con
  token) puede apuntar un proveedor a un host arbitrario y usar el daemon
  como proxy", no "cualquiera en la LAN". La severidad baja a **media**, el
  defecto es real.
* **El modelo retirado se validaba por ID, no por MODELO**:
  `update_provider("minimax-m3.1-flash", model="MiniMax-M2.7")` se aceptaba
  (el `is_retired(id)` mira el **id del proveedor**, no el modelo que se
  está poniendo) ⇒ el vault quedaba con un modelo muerto que la cascada
  luego gastaba.
* **El vault se podía corromper**: `save_vault` hacía `fs::write` (trunca y
  escribe encima) ⇒ un corte de luz o un OOM kill en mitad dejaba un
  `providers.json` truncado ⇒ `load_vault` lo declaraba corrupto, sacaba una
  copia `.corrupt-<ts>` y **el daemon se quedaba sin las API keys del
  usuario**.

### (e) ¿La respuesta al cliente filtra `reasoning` o claves?

* **Claves:** no directamente (el `reply` es la respuesta del modelo), pero
  **indirectamente sí** por el camino de los errores → DEFECTO-02 (arreglado).
* **`reasoning` (el CoT): SÍ, en dos sitios** → **DEFECTO-10**.
  * CLI `megadaemon-core chat …`: volcaba el campo `reasoning` a stdout.
  * Tool MCP `minimax_chat`: devolvía el CoT crudo dentro de
    `<reasoning>…</reasoning>` ⇒ **el agente se lo tragaba entero en cada
    llamada** (contexto y tokens del agente, no del usuario).
  * El **gateway** sí lo expone, pero en su propio campo
    (`message.reasoning_content`, y `parts[].thought` en el dialecto Gemini):
    eso es parte del contrato de esos dialectos y se queda.
  * Ahora: el CLI necesita `--reasoning` y la tool necesita
    `include_reasoning:true` (por defecto **no**).

### (f) ¿El parseo de respuestas de cada proveedor es robusto?

**No.** → **DEFECTO-13**, **DEFECTO-14**, **DEFECTO-15**.

| caso raro | antes | ahora |
|---|---|---|
| `<think>…</think>` dentro del texto | se separaba (ok) | igual (con test del banco) |
| `reasoning_content` | se leía | igual |
| `message.content` como **array** | **se perdía** (`as_str()` ⇒ `""` ⇒ "Unexpected JSON payload") | se une |
| `tool_calls` nativas | **se ignoraban** | se parsean y van a `tools_used` |
| cuerpo **no-JSON** (HTML/texto) | texto plano ok / HTML mal | error tipado con status |
| respuesta **truncada** (JSON inválido) | `Err` genérico | `Err` tipado con mensaje útil |
| `choices: []` | `Err` genérico | `Err` tipado ("respuesta sin contenido") |
| error dentro de un **200** | `Err` | `Err` tipado |
| recorte `&s[..s.len().min(200)]` | **PANIC** si el corte cae en medio de un UTF-8 | `truncar()` (por caracteres) |
| `status_de_error` | se quedaba con la **primera** "HTTP " y **tragaba** el status | busca en todas las posiciones |

---

## 2. Los defectos, uno a uno

### DEFECTO-01 — El status HTTP se perdía (y el diagnóstico era código muerto) · **P0**

`ureq 3.5.2` convierte los 4xx/5xx en `Err` **por defecto**
(`ureq-3.5.2/src/config.rs:1068` → `http_status_as_error: true`;
`ureq-3.5.2/src/error.rs:11` → "This is the default behavior").

```rust
// ANTES (providers.rs)
let resp = req.send(body_bytes.as_slice())
    .map_err(|e| format!("HTTP transport error: {}", e))?;   // <-- aquí muere el 4xx
let status = resp.status().as_u16();
if status >= 400 {                 // <-- INALCANZABLE: el 4xx ya fue Err
    ... " (API key invalida…)" / " (sin saldo…)" / " (rate limit…)"
}
```

Consecuencia: el mensaje real de un 401 era
`"HTTP transport error: http status: 401"` (`ureq-3.5.2/src/error.rs:234`).

**Arreglado:** `UreqTransport` usa `http_status_as_error(false)` y el status
llega al diagnóstico tipado (`parse_provider_error`).

### DEFECTO-02 — La credencial del proveedor podía acabar en la respuesta del cliente · **P0**

`errors.push(format!("{}: {}", p.id, e))` → `errors.join(" | ")` → `reply` →
cliente. Y `e` contenía el cuerpo de error **crudo** del proveedor (que
algunos devuelven con la key dentro).

**Arreglado:** `ProviderError::redacted(&key)` antes de que el error toque
la respuesta; `redact_secret` tapa también las variantes con prefijo
(`Bearer …`). Test: `la_api_key_del_proveedor_no_llega_al_cliente` + el
fixture `minimax-m3.1-flash__http_401.json` (que **ECOA** la clave a
propósito).

### DEFECTO-03 — Un proveedor RETIRADO se usaba de rebote, en silencio · **P1**

`is_retired` sólo se miraba para el modelo **pedido**. La cascada
caminaba por el resto del vault sin mirar el catálogo.

**Arreglado:** la cascada salta los retirados (por id **o** por modelo) y lo
dice en la traza. Test: `un_proveedor_retirado_no_se_usa_en_silencio`.

### DEFECTO-04 — Un proveedor en PAUSA por cuota se llamaba igual (y nadie se pausaba) · **P0**

`quota_guard::en_pausa()` **no lo consultaba nadie**. Y como por DEFECTO-01
el 429 nunca llegaba al `registrar_error_cuota`, **el estado "pausa" no se
llegaba a crear**: la cadena era

```
429 del proveedor
  → ureq devuelve Err(StatusCode(429))
  → "HTTP transport error: http status: 429"
  → status_de_error() busca "HTTP " → "transport" no es un número → None
  → registrar_error_cuota() NUNCA se llama
  → el proveedor nunca entra en "pausa"
  → la cascada vuelve a llamarlo en la siguiente petición (y la siguiente)
```

**Arreglado:** el status es real (DEFECTO-01), el 429/402 **se registra**
siempre, y la cascada **salta** al siguiente si el proveedor está en pausa.
Tests: `en_pausa_de_cuota_la_cascada_salta_al_siguiente`,
`un_4xx_no_se_reintenta_y_un_429_si`, `el_429_dice_el_motivo_de_la_cuota`.

### DEFECTO-05 — No había reintentos (ni con backoff) · **P2**

Un 500/503 del proveedor se gastaba en **un** intento y se caía al
siguiente (que puede ser peor). Y un 401/404 **sí** consumía tiempo.

**Arreglado:** reintentos con backoff **sólo** para 429/5xx/transporte/
timeout; **nunca** para 401/403/404 (y `max_tokens` inválido). Timeout por
proveedor (`DEFAULT_PROVIDER_TIMEOUT_SECS`).

### DEFECTO-06 — `update_provider` no validaba el endpoint ni el modelo · **P1**

* Endpoint: sin validación de esquema, con credenciales embebidas
  permitidas y apuntando a los metadatos de la nube.
* Modelo: `is_retired(id)` miraba el **id del proveedor**, no el **modelo**
  que se estaba poniendo.

**Arreglado:** `validar_endpoint()` (sólo `http`/`https`, sin credenciales
en la URL, sin `169.254.169.254`/`metadata.google.internal`) **antes** de
tocar el vault, e `is_retired(modelo)`.

### DEFECTO-07 — `save_vault` podía corromper el vault · **P1**

`fs::write` trunca y escribe encima: un corte en mitad deja un
`providers.json` truncado ⇒ `load_vault` lo marca corrupto ⇒ **se pierden
las API keys del usuario**.

**Arreglado:** escritura **atómica** (`tmp` + `rename` en el mismo
directorio).

### DEFECTO-08 — `tools_used` no reflejaba lo realmente usado · **P2**

Sólo recogía las herramientas **locales** de contexto RE (`NPUProbe`,
`TextIngest`, …). Las **`tool_calls` nativas del proveedor** se ignoraban
por completo (el parseo ni las miraba).

**Arreglado:** `ParsedToolCall` + `tools_used` con `provider_tool:<nombre>`.

### DEFECTO-09 — `max_tokens` sin acotar · **P2**

Un `max_tokens` absurdo (10⁷) hace que el proveedor responda 400 y el
daemon lo trata como **fallo del proveedor** ⇒ cae al siguiente sin decir
cuál era el problema.

**Arreglado:** `MAX_TOKENS_CAP = 32_768` (y `0` ⇒ `DEFAULT_MAX_TOKENS`).

### DEFECTO-10 — El razonamiento interno (CoT) salía al cliente · **P1**

CLI (`reasoning` a stdout) y tool MCP `minimax_chat` (`<reasoning>…`).

**Arreglado:** `--reasoning` en el CLI e `include_reasoning` en la tool
(por defecto **no**). El gateway lo mantiene en su campo propio porque es
parte del dialecto OpenAI/Gemini.

### DEFECTO-11 — El presupuesto no lo consultaba nadie · **P1**

Ver §1(c).

**Arreglado:** puerta de presupuesto en la cascada, opt-in
(`MD_BUDGET_DAY_USD`), con acumulador global con mutex.

### DEFECTO-12 — "resets at \<epoch\>." (con punto) no se detectaba · **P2**

`parsear_error_cuota` filtraba el `.` como terminador pero luego parseaba
`"1795130605."` ⇒ `Err` ⇒ **el proveedor no se pausaba con su hora de
reset** (que es justo lo que el `quota_guard` existe para).

**Arreglado:** `trim_end_matches('.')` antes de parsear.

### DEFECTO-13 — PANIC al recortar un texto UTF-8 · **P1**

`&trimmed[..trimmed.len().min(200)]` corta por **bytes**: si el corte cae
en medio de un emoji (3–4 bytes) ⇒ **panic**. Con `panic != "abort"` el
daemon sobrevive, pero la petición se cae.

**Arreglado:** `truncar()` (por caracteres).

### DEFECTO-14 — El parseo sólo leía `content` como string · **P2**

`msg_node["content"].as_str().unwrap_or("")` ⇒ con `content` en array
(multimodal) el contenido **se perdía** y el daemon caía al siguiente
proveedor como si el primero hubiera fallado.

**Arreglado:** se acepta `string` **o** array de partes; y se leen las
`tool_calls`.

### DEFECTO-15 — `status_de_error` se tragaba el status · **P2**

Buscaba la **primera** "HTTP " del mensaje; con
`"HTTP transport error: http status: 401"` tomaba `"transport"` ⇒ `None`.

**Arreglado:** busca en todas las posiciones.

### DEFECTO-16 — El parseo de `finish_reason`/vacíos era opaco · **P3**

Un `choices: []` o un 2xx con error dentro daban mensajes que no decían
nada. **Arreglado:** `ProviderError::Parse` con el trozo de cuerpo.

### DEFECTO-17 — La *traza* no existía (el `fallback_used` no explicaba nada) · **P2**

`fallback_used: true` no decía **quién** atendió, **a quién** se saltó ni
**por qué**. Con un catálogo de 20 proveedores eso es indescifrable.

**Arreglado:** `ChatResult.provider_trace` (`serde(default)`, no rompe la
forma antigua) + `megadaemon.provider_trace` en el gateway.

### DEFECTO-18 — El fallback podía caer en un proveedor de PAGO sin avisar · **P1**

La cascada mete **cualquier** proveedor habilitado con credencial, sin
mirar el `tier` (el catálogo tiene `paid`/`trial`: MiniMax, DeepSeek,
Anthropic, Cerebras). Un fallback automático ⇒ **dinero del usuario** sin
que nadie lo dijera.

**Arreglado (parcial, por decisión):** la traza dice el **tier** del
proveedor que atendió (`"p1 => OK (tier paid)"`) y existe la puerta de
presupuesto (opt-in) para vetarlo. **NO** se ha cambiado el orden de la
cascada (degradar a "sólo gratuitos" cambiaría el comportamiento del móvil:
dejaría de poder responder cuando no hay clave gratuit ⇒ es una decisión del
root, no del agente).

---

## 3. Lo que se ha cambiado (resumen)

| fichero | qué |
|---|---|
| `providers.rs` | `ProviderError` tipado · `redact_secret`/`redact_all` · `truncar` · `parse_provider_reply`/`parse_provider_error` (funciones **puras**) · `LlmTransport` + `UreqTransport` (inyectable) · `ChatDeps` + `execute_chat_with` (núcleo testeable; `execute_chat` **no cambia de firma**) · cascada con saltos de retirado/cuota/presupuesto, reintentos con backoff, timeout por proveedor, `max_tokens` acotado, `tools_used` real · `ChatResult.provider_trace` · `save_vault` atómico · `validar_endpoint` · puerta de presupuesto |
| `llm_gateway.rs` | `chat_completions_with` / `gemini_generate_content_with` (inyectables) · la respuesta lleva `provider_trace` y `quota` · **la forma OpenAI no cambia** |
| `quota_guard.rs` | DEFECTO-12 (`resets at … .`) · DEFECTO-15 (`status_de_error`) |
| `mcp_server.rs` | `minimax_chat`: el razonamiento sólo si se pide (`include_reasoning`, con schema) |
| `main.rs` | CLI `chat`: `--reasoning` (la forma del JSON no cambia) |
| `agent_coding.rs` | errores redactados + `max_tokens` acotado (paridad) |

**La API pública no se rompe:** `execute_chat`, `call_openai_http`,
`update_provider`, `list_providers_redacted`, `load_vault`, `save_vault`,
`test_provider_connection` conservan su firma. Lo único que cambia es un
campo **nuevo** en `ChatResult` (`provider_trace`, con `serde(default)`).

---

## 4. El banco de respuestas capturadas

`rust-core/tests/fixtures/providers/` — **20 cuerpos congelados** con los
campos raros: razonamiento (campo y `<think>`), `tool_calls` (con
`arguments` válidos y rotos), `content` en array, 401/402/404/429/500,
cuerpo HTML, texto plano, UTF-8 multibyte, respuesta truncada, forma
legacy, error dentro de un 200 y la forma propia del SEERE local.

* **Procedencia:** cada fichero cita la suya en `_meta.origen`. **Todos son
  `SINTETICO`**: en esta sesión **no se capturó ninguna traza HTTP cruda**
  de un proveedor real (el daemon no guarda el cuerpo crudo). La única
  medición real del 2026-11-22 fue la **salida del daemon** en el móvil del
  usuario (`{"reply":"MINIMAX_OK","vendor":"MiniMax","latency_ms":1232.0,
  "fallback_used":false}`), y así lo dice la nota del fixture
  `minimax-m3.1-flash__ok.json`.
* **Credenciales:** **ninguna**. Las claves de los cuerpos de error son
  FICTICIAS (`sk-EXAMPLE-NO-ES-UNA-KEY-0000`) y están a propósito para
  probar la redacción.

---

## 5. Los tests

### 5.1 Cuántos

| | antes (`origin/main` @ `f43a010`) | después (esta rama) |
|---|---|---|
| `#[test]` en `rust-core/src` + `rust-core/tests` | **1574** | **1632** |
| `rust-core/tests/llm_routing.rs` | — (no existía) | **44** |
| `rust-core/tests/llm_gateway_openai.rs` | — (no existía) | **14** |

> Nota: el enunciado de la tarea hablaba de "~1140 tests verdes"; la cuenta
> real de atributos `#[test]` en el crate es **1574** (el nº de tests que
> *pasan* depende de la plataforma: hay tests de Android/ARM64 que no
> compilan en el host). Se dan las dos cifras medidas.

### 5.2 Qué fijan (resumen)

`llm_routing.rs` (44): cascada (elige el pedido / cae al siguiente / el
SEERE local es el último / `fallback_used` cierto / retirado ni pedido ni de
rebote / presupuesto veta la llamada cara / pausa de cuota salta al
siguiente / 401 no se reintenta y 429 sí / la key no llega al cliente /
`tools_used` refleja las tool calls / `max_tokens` acotado / timeout al
transporte / sin credencial se salta / ruta local explícita / ruta híbrida /
`auth: none` / fallo de transporte / respuesta ilegible) + funciones puras
(redacción, `truncar`, `validar_endpoint`) + **PROPIEDAD** (los 20 fixtures
y **todos** sus cortes: nunca hay panic y siempre hay contenido o un error
con mensaje útil) + la integridad del banco (origen declarado, sin
credenciales, ficheros que existen).

`llm_gateway_openai.rs` (14): forma OpenAI exacta · la forma se conserva
aunque el proveedor falle · `system`+`user`+`assistant` · contenido en array
· mensajes vacíos rechazados con 400 **sin gastar llamada** · `model:
"megadaemon"` **no ata** a un proveedor (delega en el router) · un modelo
concreto sí se pide · `max_tokens` acotado · ninguna credencial en la
respuesta · dialecto Gemini (`candidates`, `usageMetadata`, rutas del
language server) · el razonamiento va en su campo, no en `content`.

### 5.3 Ramas de `execute_chat` cubiertas

| rama | cubierta por |
|---|---|
| modelo pedido ⇒ ese proveedor | `la_cascada_elige_el_proveedor_pedido` |
| sin modelo ⇒ default del vault | `sin_modelo_se_usa_el_default_del_vault` |
| modelo **retirado** pedido ⇒ aviso, sin llamada | `un_proveedor_retirado_no_se_usa_en_silencio` |
| retirado **de rebote** ⇒ salto + traza | idem |
| ruta local explícita (`local-seere`/`local`) | `el_camino_local_explicito_no_llama_a_nadie` |
| ruta **híbrida** (`hybrid-seere`) | `la_ruta_hibrida_usa_el_proveedor_de_nube` |
| credencial ausente ⇒ salto | `un_proveedor_sin_credencial_se_salta` |
| `auth: none` ⇒ llamada sin credencial | `un_proveedor_con_auth_none_se_llama_sin_credencial` |
| **cuota en pausa** ⇒ salto | `en_pausa_de_cuota_la_cascada_salta_al_siguiente` |
| **presupuesto** ⇒ salto | `el_presupuesto_bloquea_la_llamada_cara` |
| 4xx ⇒ NO se reintenta | `un_4xx_no_se_reintenta_y_un_429_si` |
| 429 ⇒ se reintenta con backoff | idem |
| 5xx ⇒ se reintenta | `un_4xx_no_se_reintenta_y_un_429_si` (rama de reintento) + `el_500_si_es_reintentable` |
| fallo de **transporte** ⇒ siguiente | `un_fallo_de_transporte_cae_al_siguiente` |
| 2xx **ilegible** ⇒ siguiente | `una_respuesta_ilegible_cae_al_siguiente` |
| éxito ⇒ `ChatResult` con cuota + traza | `la_traza_va_dentro_del_json_del_resultado` |
| `tool_calls` nativas ⇒ `tools_used` | `tools_used_refleja_las_tool_calls_nativas` |
| `max_tokens` acotado / 0 ⇒ defecto | `max_tokens_sale_acotado_y_el_timeout_se_pasa_al_transporte`, `max_tokens_cero_usa_el_defecto` |
| **fallback offline** (SEERE local) | `el_seere_local_es_el_ultimo_recurso` |
| error redactado antes de la respuesta | `la_api_key_del_proveedor_no_llega_al_cliente` |

---

## 6. Paridad (CLI + MCP + `agent_coding` + gateway)

| plano | antes | ahora |
|---|---|---|
| CLI `chat` | imprimía el `reasoning` | `--reasoning` para pedirlo; JSON con la misma forma |
| tool MCP `minimax_chat` | CoT crudo en `<reasoning>` | `include_reasoning:true` (schema actualizado) |
| `agent_coding::call_provider_messages` | errores con la key posible | errores redactados + `max_tokens` acotado |
| gateway OpenAI | sin traza ni cuota | `megadaemon.provider_trace` + `megadaemon.quota` |
| gateway Gemini | sin traza ni cuota | ídem |

---

## 7. Lo que **NO** queda verificado

1. **Nada se ha compilado ni ejecutado.** Por `docs/PROTOCOLO_FLOCKA.md`
   (el enlace del crate muere por OOM en este sandbox) **el root tiene que
   compilar y testear**. En particular: que los 58 tests nuevos compilen
   (tipos, imports, firmas) y que los ~1574 preexistentes sigan verdes.
2. **El camino real contra un proveedor de verdad no se ha re-ejecutado.**
   La medición real disponible es la del 2026-11-22 en el móvil
   (`MINIMAX_OK`, 1232 ms, sin fallback), **anterior** a estos cambios. El
   root debería re-medir `megadaemon-core chat minimax-m3.1-flash` en el
   móvil: la forma del JSON ha cambiado **sólo** por el campo nuevo
   `provider_trace` (y `reasoning` ahora va vacío salvo `--reasoning`).
3. **La rama `gh_cli` sin token** (`resolve_gh_token()`): la rama está
   implementada pero **no hay test** (depende de que `gh` esté o no en la
   máquina ⇒ no es determinista).
4. **El contexto RE** (`build_grounded_re_context`: `NPUProbe`, `TextIngest`,
   `CIRX-SSA`, `MBA-Deobf`): **no hay test** porque depende de rutas
   absolutas del dispositivo (`/data/user/0/...`) y del NPU del Exynos.
5. **El estado real de la cuota en producción** (el `429` → pausa → salto)
   está **probado con un doble inyectado**, no contra un proveedor real que
   dé 429.
6. **El presupuesto es opt-in**: sin `MD_BUDGET_DAY_USD` el comportamiento
   es el de v15. La decisión de **activarlo por defecto** (y con qué tope)
   es del root: el tope por defecto del bus (0,05 USD/tick, 1 USD/día)
   cortaría las llamadas de pago del móvil.
7. **El fallback a proveedor de PAGO** se ha hecho **visible**, no
   prohibido (§2/DEFECTO-18): la decisión de no degradar a "sólo
   gratuitos" es del root.
8. **La pérdida de actualización concurrente** de `providers.json` (dos
   `update_provider` a la vez: lectura-modificación-escritura) **sigue
   sin arreglarse**: hace falta un cerrojo de fichero y en Android los
   cerrojos rancios son un problema propio. Documentado, no arreglado.
9. **El dialecto Gemini** se ha probado con la forma de la
   documentación/strings del binario (`liblanguage_server.so`), **no**
   contra el language server real en marcha.
10. **El número de tests que *pasan*** (y no sólo los que existen) no se
    conoce hasta que el root los ejecute.

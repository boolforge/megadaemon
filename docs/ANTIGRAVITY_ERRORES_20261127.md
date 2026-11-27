# Antigravity: los 4 errores de la captura del 2026-11-27 (MEDIDO)

Todo lo de aquí está **medido**, no supuesto.  Cada afirmación lleva su
`fichero:línea` o su comando.  Lo que **no** se pudo medir en el sandbox
está marcado **NO VERIFICADO** con el comando exacto para cerrarlo en el
móvil.

Captura: `attachments/b640b628c40b1606/Screenshot_20261127_125731_Antigravity.jpg`
(1440×16889, sesión de Antigravity / SCROLLVM, 12:57).

---

## 0. El resumen que pide el jefe

| # | Lo que se ve | Causa real (medida) | Dónde |
|---|---|---|---|
| 1 | `ERR_LLM_UNREACHABLE: LLM_UNREACHABLE:127.0.0.1:8080:Connection refused (os error 111)` | El supervisor **nunca arrancaba** el `llama-server`. `grep -c llama deploy/99-megadaemon.sh` → 4, y los 4 son la palabra española "llamada" en comentarios ⇒ **0** arranques reales. Además 8080 es el **mismo puerto que el alias del cockpit** (EADDRINUSE ⇒ el daemon se caía). | `deploy/99-megadaemon.sh` (ahora `ensure_llama_server`, puerto 8081) |
| 2 | `minimax-m3.1-flash: cuota en pausa \| minimax-m3: cuota en pausa \| ovhcloud-free: cuota en pausa ...` | `quota_guard::en_pausa` **nunca miraba** el `reset_at` que el propio módulo guardaba: un solo 429 dejaba al proveedor pausado **para siempre**. Con todos los gratuitos quemados, la cascada se quedaba sin nube. | `rust-core/src/quota_guard.rs` (`evaluar_pausa`, `limpiar_pausa`) |
| 3 | `pollinations-free: HTTP 413 ... Prompt too long for the free Classic API: 16,000 characters max (about 4,000 tokens), counting messages and tools` | El límite real de `pollinations-free` son **16.000 caracteres** contando mensajes y tools. El recorte que existía era de **120.000** (`llm_gateway::MAX_PROMPT_CHARS`) ⇒ 7,5× por encima ⇒ el 413 estaba **garantizado** en cualquier sesión larga. No había ningún presupuesto en la ruta de proveedor. | `rust-core/src/providers.rs` (`recortar_prompt`, `recortar_mensajes`, `PROMPT_MAX_CHARS = 12_000`) |
| 4 | `The model cut off early in its thinking — stopped mid-sentence` | El dialecto Gemini **fijaba `finishReason: "STOP"` a pelo**: una respuesta cortada se anunciaba como terminada con normalidad. Y el `max_tokens` por defecto de esa ruta eran **2.048** tokens (el razonamiento se los comía). | `rust-core/src/llm_gateway.rs` (`gemini_finish_reason`, `DEFAULT_MAX_TOKENS`) |

### El error 400 de google-ai-studio (NO VERIFICADO)

`google-ai-studio: HTTP 400 ... "Please ensure that function call turn comes
immediately after a user turn or after a function response turn"`

Lo que **sí** se midió: el único `ChatResult` que sale hacia un proveedor
lleva `messages: [system, user]` (o la conversación ya montada del bucle de
tools) ⇒ **nuestro código nunca mandó un turno de función mal colocado**.
Por eso el error tiene que venir del camino que ya existía en `main` desde
el 2026-11-26 (`tool_calls` estructuradas + `functionCall` de Gemini), que
es exactamente donde Gemini exige ese orden.

**NO VERIFICADO** ⇒ para cerrarlo, en el móvil (shell root):

```bash
# La traza del gateway dice endpoint + modelo + tamaño del prompt.
# (estado del daemon: $MEGADAEMON_STATE_DIR = /data/adb/megadaemon/state)
tail -5 /data/adb/megadaemon/state/antigravity_gateway.ndjson
# Cada línea trae "evento", "modelo", "prompt_len" y "extra".
# Si el "modelo" es google-ai-studio, su "endpoint" dice a DÓNDE se mandó.
```

Con ese endpoint ya se sabe si el 400 lo genera Gemini (nuestro payload) o
si lo reenvía el language server de Antigravity.

---

## 1. La cadena causal (por qué los 4 errores van juntos)

```
Antigravity (com.agy) ──► nuestro gateway (cockpit, :9090)
                              │
                              ├─► ruta LOCAL:  SEERE_LLM = http://127.0.0.1:8081
                              │      └─► llama-server  ✗ NUNCA SE ARRANCABA
                              │         ⇒ "Connection refused (os error 111)"
                              │
                              └─► cascada de NUBE (el "fallback offline")
                                     ├─ minimax-*      ⇒ 429 ⇒ "cuota en pausa" (ETerna, bug 2)
                                     ├─ ovhcloud-*     ⇒ 429 ⇒ "cuota en pausa" (Eterna, bug 2)
                                     ├─ zai-glm-free   ⇒ 429 ⇒ "cuota en pausa" (Eterna, bug 2)
                                     ├─ pollinations-free ⇒ 413 (prompt > 16.000, bug 3)
                                     └─ google-ai-studio  ⇒ 400 (orden de turnos, NO VERIFICADO)
```

O sea: **un solo 429 de un proveedor gratis mataba la nube entera para
siempre**, y el único plan B (el LLM local) **no estaba arrancado**.  Por
eso el jefe veía el mismo error una y otra vez.

### Corrección importante sobre el 8765

El encargo apuntaba al puerto **8765** (`tunel mcp-legacy: instance obsoleta
PID 15094 eliminada`).  Medido: el 8765 **no tiene NADIE escuchando por
diseño** — es el túnel MCP *legacy*, desactivado a propósito
(`deploy/99-megadaemon.sh:520`: "El puerto 8765 no tiene NADIE escuchando =>
ese túnel no puede funcionar nunca").  El mensaje del log es el supervisor
matando un `cloudflared` obsoleto, no la causa del error.  **El puerto
rechazado es el 8080** (el del LLM local), y ahora es el **8081**.

---

## 2. La UI: por qué la de ellos no vale y la nuestra sí

**MEDIDO**: `web/cockpit_antigravity.html` ya existía en el repo pero
**no lo servía nadie** (`cockpit_server.rs` sólo embebía
`cockpit_template.html`, `cockpit_v2.html` y `webui_v2.css`) ⇒ era una
página **muerta**.  Y el "streaming" que sí había (`gemini_a_sse`) hacia la
llamada **entera** y emitía **un único chunk** con todo el texto.

La nuestra está en **`/agy`** (también `/antigravity`), con:

| Requisito | Cómo |
|---|---|
| (a) streaming token a token | `POST /api/antigravity/stream` → `text/event-stream`, un evento SSE por trozo, con `flush` en cada uno. Eventos: `estado`, `token`, `fin`. |
| (b) selector de modelo que cambia el backend | `GET /api/antigravity/modelos` ⇒ los proveedores **listos** del vault. El `<select>` usa el **ID** del proveedor (lo que `execute_chat` entiende) y la UI enseña **quién contestó de verdad**. |
| (c) reconexión sin perder el hilo | Si el stream se corta sin evento de cierre ⇒ reintento con backoff (1/2/4/8 s, 4 intentos) reenviando el **mismo** prompt. El hilo se conserva en memoria y se manda en cada intento. |
| (d) los 3 niveles de error en una línea | `GET /api/antigravity/diagnostico` ⇒ `LLM local OK \| cuota: 0 en pausa \| prompt 12000/16000 chars`, más una ficha por nivel. |
| (e) nada de texto corrupto | Todo en español, `lang="es"`, UTF-8. Y el gate `tools/check_idioma.py` (CJK/cirílico + mojibake) lo prohíbe para siempre: **encontró un caso real** en el repo (una palabra en chino en un comentario en español de `antigravity_intercept.rs`). |

### Cómo llegar a la UI en el móvil

```bash
cat /sdcard/megadaemon.url     # el supervisor escribe las URLs que de verdad responden
```

Alias que sirven la interfaz: **80, 3000, 3030, 8090** (el 8080 lo lleva el
LLM local desde este cambio... no: el 8080 queda para el alias del cockpit
y el LLM local usa el 8081).

```bash
# Desde el navegador del propio móvil:
http://localhost:3000/agy
```

---

## 3. Los comandos para cerrar lo NO VERIFICADO en el móvil

Todos como **root** (`su -c` o `adb shell su 0 ...`).

```bash
# (1) ¿Está vivo el LLM local?  (antes: connection refused en 8080)
curl -s -m 3 http://127.0.0.1:8081/health ; echo
pgrep -af llama-server
tail -40 /sdcard/megadaemon.log | grep -i "LLM local"

# (2) Los 3 niveles de error, en una línea (lo mismo que ve la UI).
TOK=$(cat /data/adb/megadaemon/state/cockpit_token)
KEY=$(cat /data/adb/megadaemon/state/megadaemon_api.key)
curl -s -m 5 -H "X-Seere-Auth-Token: $TOK" -H "X-API-Key: $KEY" \
     http://127.0.0.1:9090/api/antigravity/diagnostico

# (3) ¿Qué proveedores están en pausa y cuándo vuelven?
curl -s -m 5 -H "X-Seere-Auth-Token: $TOK" -H "X-API-Key: $KEY" \
     http://127.0.0.1:9090/api/antigravity/diagnostico | python3 -m json.tool

# (4) ¿Cuánto prompt manda de verdad el gateway?  (el 413 de pollinations)
tail -5 /data/adb/megadaemon/state/antigravity_gateway.ndjson \
  | python3 -c "import sys,json;[print(json.loads(l)['prompt_len']) for l in sys.stdin if l.strip()]"

# (5) Chat con streaming: ¿llega a trozos y quién contesta?
curl -sN -m 60 -X POST http://127.0.0.1:9090/api/antigravity/stream \
  -H "Content-Type: application/json" \
  -H "X-Seere-Auth-Token: $TOK" -H "X-API-Key: $KEY" \
  -d '{"prompt":"di hola en una frase","model":"minimax-m3.1-flash"}'

# (6) Los puertos: el 8081 es el LLM local, el 8080 el alias del cockpit.
ss -ltn | grep -E ':(8080|8081|9090|8765)'

# (7) ¿El supervisor levanta el llama-server al arrancar?
grep -n "LLM local" /sdcard/megadaemon.log | tail -5
```

### Lo que el resultado de (1) decide

* Si (1) responde ⇒ el `ERR_LLM_UNREACHABLE` **está resuelto** en el móvil.
* Si (1) no responde y el log dice `AVISO: hay llama-server pero NO hay
  modelo .gguf` ⇒ el móvil **no tiene el modelo local**: el plan B es la
  nube, y entonces el arreglo (2) es el que sostiene el chat.
* Si (1) no responde y el log dice `AVISO: no hay llama-server` ⇒ este
  móvil no trae el binario: hay que instalarlo (o aceptar que el LLM local
  no existe en este dispositivo).

---

## 4. Lo que queda abierto (NO VERIFICADO, y por qué)

1. **Que el language server de Antigravity acepte `AGY_LLM_GATEWAY_URL` y
   pinte nuestro catálogo.** El propio módulo lo dice:
   `rust-core/src/antigravity_gateway.rs:36` ("LO QUE SIGUE SIN VERIFICAR").
   Se cierra con: arrancar el daemon en el móvil y mirar si el selector de
   la app ofrece `minimax-*` en vez de los de Google.
2. **Que el parche del `.so` (endpoint) sobreviva a una actualización de la
   app**: `rust-core/src/antigravity_fake_so.rs:47`.
3. **Que frida-gum pueda cambiar el valor de retorno de una función Go**
   (`rust-core/src/antigravity_intercept.rs:51`).
4. **Si el 400 de google-ai-studio lo genera Gemini o lo reenvía el LS**:
   ver el comando (4) de arriba.

Ninguno de los cuatro es un requisito para que los 4 errores de la captura
dejen de aparecer: son la ruta *extra* (el LS hablando con nuestro gateway
para que el selector de modelos de su propia GUI funcione).

---

## 5. Dónde está cada cosa

| Fichero | Qué hace |
|---|---|
| `rust-core/src/quota_guard.rs` | La pausa por cuota **expira** (bug 2). `evaluar_pausa` es pura y testeable. |
| `rust-core/src/providers.rs` | Presupuesto de prompt (bug 3), traza con endpoint/modelo/tamaño, `finish_reason` en `ChatResult`. |
| `rust-core/src/seere_v11.rs` | El LLM local pasa a **8081** (fuera del puerto del alias del cockpit). |
| `deploy/99-megadaemon.sh` | `ensure_llama_server()` con keeper (bug 1) + `export SEERE_LLM`. |
| `rust-core/src/llm_gateway.rs` | `finishReason` real (bug 4), `max_tokens` 4.096, `MAX_PROMPT_CHARS` documentado frente al límite de 16.000. |
| `rust-core/src/cockpit_server.rs` | `/agy`, `/api/antigravity/modelos`, `/api/antigravity/diagnostico`, `/api/antigravity/stream` (SSE). |
| `web/cockpit_antigravity.html` | La UI nueva (los 5 requisitos). |
| `tools/check_idioma.py` | Gate de texto corrupto / idiomas mezclados (en el CI). |

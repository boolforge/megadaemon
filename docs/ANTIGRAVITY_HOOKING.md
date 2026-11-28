# ANTIGRAVITY: el hooking, sus puertos y sus errores (2026-11-28)

Todo lo de aquí está **medido** (telemetría del móvil y capturas del jefe) o
**verificado en el código** con `fichero:línea`.  Lo que no está medido
dice **NO MEDIDO** ⇒ no se inventa.

## 1. Qué es el "hooking"

El language server de Antigravity (`libls`) habla con el backend de Google
(`https://daily-cloudcode-pa.googleapis.com`, 42 bytes en el binario,
`antigravity_endpoint::CLOUCODE_URL`).  El **hooking** es parchear esos
42 bytes por la URL del gateway de MegaDaemon, para que el modelo llegue
**al móvil** y no a la nube de Google.

El parche conserva la longitud exacta (por eso el destino tiene que medir
lo mismo) y hay dos piezas:

| Pieza | Qué hace | Dónde |
|---|---|---|
| **El parche del LS** | El LS pide al gateway en vez de a Google | `antigravity_endpoint::aplicar_endpoint` |
| **El gate de la GUI** | La GUI habilita los modelos personalizados | `antigravity_endpoint::aplicar_gate` (`osName==="linux"&&k.isGoogleInternal`) |

## 2. Los puertos (aquí estaba el fallo)

| Puerto | Quién lo sirve | Nota |
|---|---|---|
| 9090 | El daemon (cockpit) | El puerto canónico (`PUERTO_GATEWAY`) |
| 8080 | El daemon (alias) o el **web bridge** del móvil | MEDIDO: el 8080 estaba en `BRIDGE_CANDIDATES` del supervisor, pero el daemon **no** lo escuchaba |
| 8081 | El motor local (llama-server) | Se movió aquí para dejarle el 8080 al cockpit (`seere_v11::LLM_LOCAL_PORT`, línea 1754) |
| 3000 / 3030 / 8090 | El daemon (alias) o el web bridge | Los que el usuario tenía en marcadores |

MEDIDO (captura del jefe, 2026-11-27 22:33): la app respondía

```
ERR_LLM_UNREACHABLE: LLM_UNREACHABLE:127.0.0.1:8080: Connection refused (os error 111)
```

⇒ el LS estaba parcheado con el **8080** y **nadie escuchaba** ahí:

* `cockpit_server::ALIAS_PORTS` era `[3000, 3030, 8090, 9090]` ⇒ el 8080
  nunca fue alias del cockpit (aunque el motor local se movió al 8081
  JUSTO para dejarle el sitio).
* El 8080 lo tenía el web bridge (`web_bridge.py`, Termux) cuando está
  vivo ⇒ por eso el puerto "a veces" respondía y "a veces" no.

**Arreglo (2026-11-28)**: el 8080 es alias del cockpit
(`cockpit_server::ALIAS_PORTS`, línea 4003) y el parche **sólo** puede
apuntar a un puerto que el daemon sirve **o** que tiene un listener ahora
mismo (`antigravity_endpoint::puerto_usable`, línea 93 ⇒
`puerto_servido` + `puerto_con_listener`).

Si otro proceso tiene el 8080, el bind del alias es *best-effort* (se avisa
por stderr y el daemon sigue sirviendo el 9090): el cockpit nunca se cae
por un alias ocupado.

## 3. Cómo activarlo (y desactivarlo)

### 3.1 Desde la Web UI (NUEVO, 2026-11-28)

El panel de Antigravity era de **sólo lectura**: no había forma de
activar ni desactivar el hooking sin ir al MCP.

* `GET  /api/v2/antigravity/hook` ⇒ estado (¿parcheado? ¿con qué URL?
  ¿qué puertos tienen listener?).
* `POST /api/v2/antigravity/hook` ⇒ `{accion: aplicar|aplicar_gate|restaurar|estado|plan, puerto?: 8080|9090, confirmar: true}`.

La Web UI **reutiliza el mismo dispatch del tool MCP**
(`antigravity_endpoint::dispatch`): una sola fuente de verdad y las mismas
reglas (confirmar obligatorio, puerto con listener).

En la interfaz: tarjeta "Hooking del endpoint" en `web/cockpit_v2.html` y
en `web/panels/antigravity_panel.{html,js}`.

### 3.2 Desde el MCP

```jsonc
// estado (no escribe nada)
{"tool": "antigravity_endpoint", "action": "estado"}
// plan (no escribe nada): dice cuántos bytes se van a cambiar
{"tool": "antigravity_endpoint", "action": "plan", "puerto": 8080}
// aplicar (ESCRIBE el binario del LS)
{"tool": "antigravity_endpoint", "action": "aplicar", "puerto": 8080, "confirm": true}
// deshacer
{"tool": "antigravity_endpoint", "action": "restaurar", "confirm": true}
```

## 4. Catálogo de errores (con su causa MEDIDA)

| Error | Causa | Arreglo |
|---|---|---|
| `ERR_LLM_UNREACHABLE: …:8080: Connection refused` | El LS apunta a un puerto sin listener | Alias 8080 + `puerto_usable` (§2) |
| `google-ai-studio: HTTP 400 "function call turn comes immediately after a user turn"` | La capa OpenAI-compat de Google valida la ESTRUCTURA de la conversación: Gemini decide el rol **por el índice** (par = usuario) y exige un `functionResponse` por `functionCall` | Ruta **nativa** de Gemini (`providers::es_gemini_nativo`, línea 2577): la estructura la pone MegaDaemon (`openai_a_gemini_contents`, línea 2647) |
| `pollinations-free: HTTP 413 "Prompt too long"` | El proveedor cuenta "mensajes **y tools**" contra 16.000 y las tools **no** se recortaban (el LS manda 18 `functionDeclarations`) | `providers::recortar_tools` (línea 2262): las tools se quedan en 2/3 del presupuesto y se sueltan de la última a la primera |
| `minimax-m3: cuota en pausa` (sin más) | El motivo se guardaba pero no se enseñaba | `quota_guard::detalle_pausa` (línea 521) + la cascada lo dice: "motivo; vuelve en 15 min" |

## 5. Por qué la ruta NATIVA de Gemini y no la OpenAI-compat

MEDIDO (2026-11-28): el LS de Antigravity manda **750 KB** de contexto; el
daemon lo recorta por la frente ⇒ el primer turno que sobrevive es del
**modelo**, y su respuesta de herramienta se queda **sin llamada**.

Con la capa OpenAI-compat de Google, la estructura de `contents` la arma el
servidor ⇒ no hay forma de arreglarlo sin adivinar cómo convierte.  Con la
API nativa (`/v1beta/models/{model}:generateContent`):

* `contents[0]` es de usuario,
* los turnos alternan usuario/modelo,
* cada `functionCall` tiene **exactamente** un `functionResponse`.

`openai_a_gemini_contents()` lo garantiza y **deja constancia** en la
traza de lo que corrige ("gemini nativo: … respuesta(s) de herramienta sin
llamada -> fuera").

## 6. Lo que falta por medir (NO MEDIDO)

* Que el 8080 como alias del cockpit **arregle de verdad** el
  `ERR_LLM_UNREACHABLE` en el móvil: depende de que el jefe re-ejecute el
  one-liner (el binario se auto-actualiza, pero hay que relanzar) ⇒
  checklist en `docs/VERIFICACION_MOVIL_ONE_LINER.md` §6.
* Que la ruta nativa de Gemini acepte la conversation real del LS (la
  estructura se valida contra las reglas documentadas de la API, pero sin
  una llamada real a `google-ai-studio` desde el móvil).
* Por qué el túnel `mcp-legacy` apunta a `127.0.0.1:8765`, que no escucha
  (bucle de reinicio de `cloudflared`, load 20, 1 GB de datos): **NO
  MEDIDO**, no está localizado en el repo.

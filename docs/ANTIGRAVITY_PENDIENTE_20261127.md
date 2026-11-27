# Antigravity: lo que QUEDA por hacer (2026-11-27, cierre del agente RE)

Todo lo de aquí está **medido**.  Lo que ya está resuelto está en
`docs/ANTIGRAVITY_ERRORES_20261127.md`; este fichero es **lo pendiente** y
el **comando exacto** para continuar cada punto.

Todo lo de abajo está en `main` (rama `agent/antigravity-re`, mergeada).
Los cuatro errores de la captura tienen su causa medida y su arreglo; lo
que falta es **cerrarlos en el móvil**, que desde el sandbox no se puede.

---

## 1. Estado final de los 4 errores (MEDIDO en `main`)

| Error de la captura | Causa medida | Arreglo | ¿Cerrado? |
|---|---|---|---|
| `ERR_LLM_UNREACHABLE:127.0.0.1:8080` | el supervisor **no arrancaba** el `llama-server` (`grep -c llama` en `deploy/99-megadaemon.sh` → 0 hits reales; los 4 eran la palabra "llamada") y 8080 colisionaba con el alias del cockpit | `ensure_llama_server()` con keeper en **8081** (`LLM_LOCAL_PORT`), `export SEERE_LLM` | **código listo · NO MEDIDO en el móvil** |
| `minimax-m3: cuota en pausa` (eterno) | `quota_guard::en_pausa` **ignoraba** el `reset_at` que el propio módulo guardaba ⇒ un 429 pausaba al proveedor PARA SIEMPRE | `evaluar_pausa` + `limpiar_pausa` (pausa que expira, media abierta) | **código listo · NO MEDIDO en el móvil** |
| `HTTP 413 ... 16,000 characters max` | el recorte que había era de **120.000** (`llm_gateway::MAX_PROMPT_CHARS`) y el límite de `pollinations-free` es **16.000** ⇒ el 413 estaba garantizado | `providers::PROMPT_MAX_CHARS = 12.000` aplicado al **conjunto** de mensajes (`recortar_mensajes`) | **código listo · NO MEDIDO en el móvil** |
| `The model cut off early in its thinking` | el dialecto Gemini fijaba `finishReason: "STOP"` a pelo (una respuesta cortada se anunciaba como normal) y el `max_tokens` por defecto de esa ruta eran 2.048 | `gemini_finish_reason` (`length`→`MAX_TOKENS`, `content_filter`→`SAFETY`) y defecto a 4.096 | **código listo · NO MEDIDO en el móvil** |

### 1.1 Lo que sigue SIN resolver (con honestidad)

1. **El 400 de google-ai-studio** (`"function call turn comes immediately
   after a user turn"`): **NO RESUELTO / NO MEDIDO**.  Nuestro código sale
   siempre con `messages: [system, user]` (o la conversación ya montada del
   bucle de tools), así que **nunca mandamos un turno de función mal
   colocado** ⇒ el error tiene que venir del camino que `main` ya tenía
   desde el 2026-11-26 (las `functionCall` de Gemini).  Para cerrarlo:
   §2, comando (4): la traza del gateway dice endpoint + modelo + caracteres.
2. **Si el corte de frase venía del límite de tokens o del corte del
   stream**: **NO MEDIDO**.  El texto "cut off early in its thinking" es del
   *language server* de Antigravity, no nuestro.  Con `finishReason` real
   ya se distinguirá (ver §2, comando (5)).
3. **Que la UI nuestra se vea bien en el móvil**: **NO MEDIDO** (aquí no hay
   navegador ni móvil).  Lo medido es el código: 468 líneas, 3 endpoints,
   `/agy` servida, SSE por trozo, selector que manda el ID real del
   proveedor.
4. **Nº de clics / pantallas / tiempo hasta la primera respuesta de la UI
   de ellos**: **NO MEDIDO**.  Lo único medido de su UI es la captura
   (1440×16889): **≥5 repeticiones** del mismo bloque `ERR_LLM_UNREACHABLE`
   en los últimos 3.400 px, el desplegable de modelo que no cambia el
   backend (en Android los modelos propios salen `disabled=true`: lo
   documenta `antigravity_endpoint.rs:12-16`) y el panel izquierdo con
   **caracteres CJK pegados al texto español** (el rótulo de "Pregunta"
   traía 4 ideogramas CJK intercalados) y textos de proveedor mezclados
   con chino.  Ojo: aquí se DESCRIBEN, no se copian: el lint del repo
   (`word_fusion_lint`) marca el CJK pegado a ASCII como defecto.

---

## 2. Comandos para cerrar en el MÓVIL (root, shell de root)

```bash
# (1) ¿Está vivo el LLM local?  (antes: connection refused en 8080)
curl -s -m 3 http://127.0.0.1:8081/health ; echo
pgrep -af llama-server
tail -40 /sdcard/megadaemon.log | grep -i "LLM local"

# (2) Los 3 niveles de error en una linea (lo mismo que ve la UI)
TOK=$(cat /data/adb/megadaemon/state/cockpit_token)
KEY=$(cat /data/adb/megadaemon/state/megadaemon_api.key)
curl -s -m 5 -H "X-Seere-Auth-Token: $TOK" -H "X-API-Key: $KEY" \
     http://127.0.0.1:9090/api/antigravity/diagnostico

# (3) ¿Qué proveedores siguen en pausa y cuando vuelven?
curl -s -m 5 -H "X-Seere-Auth-Token: $TOK" -H "X-API-Key: $KEY" \
     http://127.0.0.1:9090/api/antigravity/diagnostico | python3 -m json.tool

# (4) ¿Cuanto prompt manda de verdad el gateway?  (el 413 de pollinations)
#     y el endpoint del 400 de google-ai-studio
tail -5 /data/adb/megadaemon/state/antigravity_gateway.ndjson \
  | python3 -c "import sys,json;[print(json.loads(l)['prompt_len'], json.loads(l).get('extra',{})) for l in sys.stdin if l.strip()]"

# (5) Chat con streaming: ¿llega a trozos, quien contesta y por que termino?
curl -sN -m 60 -X POST http://127.0.0.1:9090/api/antigravity/stream \
  -H "Content-Type: application/json" \
  -H "X-Seere-Auth-Token: $TOK" -H "X-API-Key: $KEY" \
  -d '{"prompt":"di hola en una frase","model":"minimax-m3.1-flash"}'

# (6) Los puertos: 8081 = LLM local, 8080 = alias del cockpit, 9090 = daemon
ss -ltn | grep -E ':(8080|8081|9090|8765)'

# (7) La UI nueva, en el navegador del propio movil
#     http://localhost:3000/agy   (o el puerto que diga /sdcard/megadaemon.url)
```

**Qué decide el resultado de (1)**:

* Si responde ⇒ el `ERR_LLM_UNREACHABLE` **esta resuelto** en el movil.
* Si no responde y el log dice `AVISO: hay llama-server pero NO hay modelo
  .gguf` ⇒ este movil **no tiene el modelo local**: el plan B es la nube y
  el arreglo (2) es el que sostiene el chat.
* Si no responde y el log dice `AVISO: no hay llama-server` ⇒ este movil no
  trae el binario: hay que instalarlo o aceptar que el LLM local no existe
  aqui.

---

## 3. Pseudocodigo de las ~356 k funciones de Codex (PENDIENTE, NO se arranca)

**NO se ha hecho y NO se arranca aqui**: es trabajo de Ghidra (JVM) sobre
un binario de 395 MB y el sandbox tiene 3 GB de RAM con 2 CPU.  Un Ghidra
en headless con `-max-cpu 1` cabe; **dos JVM a la vez no**.

Estado real (medido en `docs/RE_CODEX_SIMBOLADO.md` y
`docs/RE_FUNCIONES_IRRECONSTRUIBLES.md`):

* La symtab tiene **616.013 simbolos / 356.133 funciones** con nombre.
* **NO MEDIDO**: el pseudocodigo de esas ~356 k funciones; solo de un
  subconjunto.
* El barrido del 5.º motor (`r2dec`, worker RE) quedo **paralizado** en la
  parada ordenada con **9.565 pseudocodigos parciales** en
  `/workspace/agy-re/tools-out/todo/rzD` (10.090 ficheros, 24,6 MB medido
  con `os.walk`; **no se borra nada del NFS**).

### El comando para continuar (5.º motor, 1 worker, sin Ghidra)

```bash
export SLEIGHHOME=/workspace/.home/.local/share/radare2/plugins/r2ghidra_sleigh
# af+ <addr> <nombre>; afb+ <addr> <addr> <tam>; pdc @ <addr> > fichero.c
# el barrido por lotes es tools/regenerar-go/decomp-r2dec.py
python3 tools/regenerar-go/decomp-r2dec.py --help
```

El fichero de debug del CLI `codex` (395.260.220 B) **sigue solo subido a
Drive** (id `1q93ULt4kAXiXMXV7Pzn3gc8HNGqjpS4i`, carpeta
`19ISaNT5MV5WUfEhD854OBQygqOf-Kgn0`): no se descarga ni se procesa (decision
del root, 2026-11-27).  El RE del CLI `codex` (321.244.472 B) **no se ha
hecho**.

### Aviso de RAM

`free -m` **por debajo de 400 MB** ⇒ parar `r2ghidra` (nunca Ghidra, que
es el que tiene el JDK y el proyecto cargado).  Con `-max-cpu 1` y
`1000M` de heap, un Ghidra headless + r2ghidra en el mismo proceso es el
limite; dos JVM **no**.

---

## 4. Lo que esta FUERA de este encargo (para que nadie lo busque aqui)

* El escenario **B31** del banco del instalador (el `sleep` del runner) lo
  esta arreglando el root con un `/proc` sintetico: **no tocar**.
* La cola de `payload de hooks`, los builds ARM64/Linux y el modulo Go son
  de otros agentes.

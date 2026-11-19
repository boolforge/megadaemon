# Web del móvil: por qué «no funcionaba» y qué se arregló (2026-11-19)

Samsung SM-G998B · Android 14 (SDK 34) · instancia maestra `primary`.

El usuario reportaba `ERR_CONNECTION_REFUSED` en `localhost`, `localhost:3000`,
`localhost:8080` y `localhost:8090`. **No era un problema de la UI**: eran dos
fallos independientes en la capa web, ambos reproducibles y ambos verificados.

---

## Fallo 1 — el puente web estaba caído (el `ERR_CONNECTION_REFUSED`)

`megadaemon-core` (v13.3.0 desplegado) sólo escucha en `127.0.0.1:9090` y su
gate de API key responde `401` a `/` porque **Chrome no puede enviar la
cabecera `X-API-Key`**. La capa que lo resuelve es `deploy/web_bridge.py`: un
proxy local que sirve `3000/3030/8090/8080` (+ `80`) e inyecta la API key y el
token de cockpit del propio dispositivo.

Ese proxy **murió** y el supervisor tardaba hasta 60 s en relanzarlo
(`sleep 60` en el bucle principal). Durante ese minuto el navegador del
usuario se comía un `ERR_CONNECTION_REFUSED` en todos los puertos.

Además, el fichero del puente en el móvil estaba **más nuevo que el proceso
que lo ejecutaba**: en disco ya hadía soporte de `::1` y del puerto `80`, pero
el proceso vivo era la copia antigua (sin IPv6, sin puerto 80). Como
`ensure_web_bridge` sólo arranca el puente *si no hay ninguno corriendo*, la
versión nueva nunca llegó a tomar el control.

**Arreglo aplicado en el dispositivo** (`/data/adb/service.d/99-megadaemon.sh`
y `/data/local/tmp/megadaemon/99-megadaemon.sh`):

* el puente se lanza bajo un **keeper**: `while true; do python3 web_bridge.py; sleep 5; done`
  → si lo matan (una shell de agente, un OOM, un reinicio) vuelve en ≤5 s;
* el bucle principal **vigila cada 5 s** en vez de dormir 60 s a ciegas;
* se relanzó el daemon y el puente → el proceso vivo ya es la versión con
  `::1`, puerto `80` y el parche de la IP de origen.

Verificado tras el reinicio:

```
LISTEN 0.0.0.0:80  0.0.0.0:3000  0.0.0.0:3030  0.0.0.0:8080  0.0.0.0:8090
LISTEN    [::1]:80     [::1]:3000     [::1]:3030     [::1]:8080     [::1]:8090
LISTEN 127.0.0.1:9090        (el cockpit real)
```

→ `http://localhost`, `http://localhost:3000`, `:3030`, `:8080`, `:8090` y
`http://192.168.1.124:3000` responden **200**; IPv4 e IPv6.

---

## Fallo 2 — el rate limit de 60 req/min se comía el túnel (el «nadie me hace caso»)

`api_key::RateLimiter` (`rust-core/src/api_key.rs`) limita a **60 peticiones
por minuto y por IP**. En el móvil **todo el tráfico sale por `127.0.0.1`**:

* el navegador (a través del puente),
* el túnel de Cloudflare (la vía de los agentes),
* las llamadas MCP del propio daemon.

Comparten **un único cubo**. Una recarga de la UI dispara 20+ peticiones de
golpe; con el tráfico de los agentes encima, el cubo se agota y **el túnel
empieza a devolver `429 rate_limited`**. Eso es exactamente lo que hace que
«la instancia no responde»: los agentes reciben 429, lo interpretan como
caída y se rinden. Durante el diagnóstico, una llamada MCP propia felló con
`HTTP 429` y otra con `HTTP 502`.

**Arreglo aplicado**: el puente sale al cockpit con **IP de origen
`127.0.0.2`** (`MEGADAEMON_BRIDGE_SRC_IP`, configurable), así que:

* la UI de Chrome consume su **propio** cubo (`127.0.0.2`),
* el túnel de los agentes conserva el suyo (`127.0.0.1`).

Verificado en la auditoría del daemon — las dos IP aparecen separadas:

```
{"ts":...,"ip":"127.0.0.1","path":"/health","ua":"probe-worker","reason":"missing_api_key"}
{"ts":...,"ip":"127.0.0.2","path":"/health","ua":"probe-worker","reason":"missing_api_key"}
```

Tras el cambio, el túnel responde `200` y **no aparece ni un solo 429** en el
log del daemon con peticiones de Chrome.

---

## Cómo volver a comprobarlo

Desde el móvil (o por MCP `root_exec`):

```sh
# 1. ¿están escuchando los puertos de la UI?
(ss -ltn || netstat -ltn) | grep -E ':(9090|3000|3030|8090|8080|80) '

# 2. ¿responde el puente?
/data/data/com.termux/files/usr/bin/python3 - <<'PY'
import urllib.request
for u in ("http://127.0.0.1:3000/health","http://127.0.0.1:80/health",
          "http://localhost:8080/health"):
    try:
        r = urllib.request.urlopen(u, timeout=5); print(u, r.status, r.read()[:80])
    except Exception as e: print(u, "EXC", e)
PY

# 3. ¿hay 429?  (el log sólo registra las peticiones RECHAZADAS)
grep -c rate_limited /sdcard/megadaemon.log
```

Desde fuera (túnel):

```sh
curl -s -H "X-API-Key: $MEGADAEMON_API_KEY" \
  https://webpage-ultram-please-communicate.trycloudflare.com/health
```

---

## Pendiente (no se ha hecho aquí)

* **El limitador sigue siendo 60 req/min por IP** en el binario desplegado
  (constante de compilación, `RATE_MAX_REQUESTS`). Con la separación de IP el
  síntoma desaparece, pero una recarga de la UI con muchas pestañas puede
  volver a agotar el cubo de `127.0.0.2`. La solución de fondo es:
  * ventana deslizante en vez de ventana fija,
  * límite configurable por entorno (`MEGADAEMON_RATE_MAX`),
  * y/o eximir al puente local del limitador (es tráfico de loopback de la
    propia máquina).
* El binario **v13.4+** (alias de puertos nativos y `/` exento del gate)
  todavía no está desplegado en el móvil: mientras siga el v13.3.0, el puente
  web es obligatorio.
* `deploy/99-megadaemon.sh` en el repo tiene una variante propia (sondeo de
  10 s + `alias_ports_listening`); la versión **desplegada y verificada** en
  el móvil es la de keeper + vigilancia de 5 s.

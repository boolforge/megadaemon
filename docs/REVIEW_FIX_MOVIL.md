# Revisión adversaria de `deploy/fix-movil.sh` y `deploy/fix-entorno.sh`

**Revisor**: sesión W2 (`6d7ffc8f`), 2026-11-21. **Alcance**: lectura completa de
ambos scripts + verificación *en vivo* de las URLs y del estado real del repo.
**No he editado los ficheros** (son del principal); esto son los defectos medidos
y el arreglo exacto de cada uno.

Regla de la flota: **un solo punto de entrada** ⇒ no creo scripts paralelos; lo
que sigue es el material para que el principal cierre la versión definitiva.

---

## P0 · Bloqueantes (el script **no funciona** o **rompe el móvil**)

### P0-1 · La URL de descarga no existe (medido: HTTP 404)
`fix-movil.sh:36` → `REL=https://github.com/boolforge/megadaemon/releases/download/latest`

Las releases de este repo se etiquetan **`v<sha>`** (p. ej. `vfd795e8…`), **no
existe el tag `latest`** ⇒ la descarga devuelve **404** ⇒ el script muere en la
línea 80 (`no pude descargar el binario del release`).
Verificado en vivo:
`GET /releases/download/latest/megadaemon-core-android-arm64` → **HTTP 404**;
`GET /releases/download/vfd795e8…/megadaemon-core-android-arm64` → **HTTP 200**.

**Peor aún**: las dos releases más recientes (`vfd795e8…`, `vf8fef1c…`) **no
contienen** `megadaemon-core-android-arm64` (sólo binarios auxiliares:
`adbonly-arm64`, `channel-publish-arm64`, `duckdns-updater-arm64`) ⇒ aunque se
arreglase el tag, el asset seguiría sin estar ⇒ ⇒ hay que resolver el release
**que contiene el asset**, no "el último".

**Arreglo**: resolver por API y comprobar que el asset existe:
```bash
rel_json="$("$CURL" -fsSL -m 20 https://api.github.com/repos/boolforge/megadaemon/releases?per_page=20)"
tag="$(printf '%s' "$rel_json" | tr '{' '\n' | grep -o '"tag_name":"[^"]*"' | sed 's/.*:"//;s/"//' \
       | while read t; do
           printf '%s' "$rel_json" | grep -q "\"tag_name\":\"$t\"" && \
           printf '%s' "$rel_json" | grep -q "megadaemon-core-android-arm64" && { echo "$t"; break; }
       done)"
[ -n "$tag" ] || die "ninguna release contiene el binario ARM64 (el CI no lo ha publicado)"
REL="https://github.com/boolforge/megadaemon/releases/download/$tag"
```
Y **verificar además** que la release elegida lleva el `.sha256` del asset.

### P0-2 · No se comprueba que el binario sea un ELF aarch64
`fix-movil.sh:86` → `head -c 4 megadaemon-core.new | od -c | head -1 >/dev/null`
No comprueba nada (sólo que `od` no falle). La cabecera del script **promete**
"ELF aarch64", pero nada lo verifica ⇒ combinado con P0-1, **la página HTML de
error del 404 puede instalarse como binario** (`chmod 755` + `mv` + reinicio) ⇒
el móvil se queda sin daemon.

**Arreglo** (obligatorio y fatal):
```bash
file megadaemon-core.new | grep -q "ELF 64-bit LSB.*ARM aarch64" \
  || die "lo descargado no es un ELF aarch64 (probablemente un HTML de error)"
```
(`fix-entorno.sh:113` ya lo hace, pero como `warn` ⇒ **también fatal**.)

### P0-3 · La verificación de sha256 **se salta en silencio**
`fix-movil.sh:81-84` → la descarga del `.sha256` es `|| true` y la comparación
es `[ -n "$EXPECT" ] && [ "$EXPECT" != "$GOT" ] && die …` ⇒ **si el `.sha256` no
se descarga, `EXPECT` queda vacío y la comprobación se salta sin avisar** ⇒ se
instala un binario **sin verificar**.

**Arreglo**:
```bash
"$CURL" -fsSL -o megadaemon-core.sha256.new "$REL/megadaemon-core-android-arm64.sha256" \
  || die "no pude descargar el .sha256 del release"
EXPECT="$(cut -d' ' -f1 < megadaemon-core.sha256.new | tr -d '\r')"
[ -n "$EXPECT" ] || die "el .sha256 del release viene vacío"
[ "$EXPECT" = "$GOT" ] || die "sha256 ($EXPECT) != el descargado ($GOT)"
```
(`fix-entorno.sh:109-111` tiene **el mismo fallo**.)

---

## P1 · El script puede **tirar el móvil** o dar un **falso verde**

### P1-1 · `pgrep -f megadaemon-core` puede **encontrarse a sí mismo**
`fix-movil.sh:53,97` y `fix-entorno.sh:73,133` ⇒ `pgrep -f` casa con cualquier
proceso cuya línea de comandos contenga el texto ⇒ **el propio script** contiene
`megadaemon-core` en sus argumentos ⇒ falso "daemon arrancado" ⇒ **falso verde**
y, peor, se mata un daemon sano creyéndolo muerto.
Regla del repo: `ps -A` sin `-o ARGS` no muestra la línea de comandos, y
`pkill -f` mata el propio shell si el patrón aparece en su línea.
**Arreglo**: `pgrep -x megadaemon-core` (nombre exacto) **y** validar
`/health` en vez del proceso.

### P1-2 · La espera de reinicio (4 s) provoke un **rollback innecesario**
`fix-movil.sh:95-101` ⇒ tras `pkill` espera 4 s ⇒ el supervisor tiene un watchdog
con ~45 s de espera ⇒ el daemon **suele tardar más** ⇒ el script cree que falló
⇒ **restaura un binario bueno** y deja el móvil con la versión anterior ⇒ y el
"estado final" que reporta no es el que se instaló.
**Arreglo**: esperar hasta 60 s con reintentos y exigir `/health` = 200:
```bash
ok=0
for i in $(seq 1 20); do
  sleep 3
  code="$(curl -s -m 5 -o /dev/null -w '%{http_code}' http://127.0.0.1:9090/health 2>/dev/null)"
  [ "$code" = "200" ] && { ok=1; break; }
done
[ "$ok" = 1 ] || { /* rollback real */ }
```

### P1-3 · `touch $SUP` deja un supervisor **VACÍO** (peor que no tenerlo)
`fix-movil.sh:73` → `touch $SUP` crea el fichero si no existe ⇒ un
`/data/adb/service.d/99-megadaemon.sh` **vacío** ⇒ el móvil **arranca sin
daemon** y sin supervisor (exactamente el estado que causó el 502 medido).
**Arreglo**: `if [ ! -s "$SUP" ]; then die "el supervisor no existe: no lo creo vacío (arrancaría sin daemon)"; fi`.

### P1-4 · El `export` se añade al **FINAL** del supervisor ⇒ no lo hereda nadie
`fix-movil.sh:73` y `fix-entorno.sh:144` → `>> $SUP` **añade al final** ⇒ el
daemon que el supervisor lance **antes** de esa línea no recibe
`MEGADAEMON_STATE_DIR` ⇒ el estado no se unifica ⇒ el 403 persiste.
**Arreglo**: insertar el export **al principio** del supervisor (o mejor: no
tocar el supervisor y resolver el estado_dir de otra forma, ver P1-5).

### P1-5 · "Credencial en un solo sitio" pero el script **copia** la key ⇒ dos copias
`fix-movil.sh:68` copia la key a `$ROOTSTATE` **y** la original sigue en
`$STATE_REAL` ⇒ **dos fuentes de verdad** ⇒ es exactamente la avería que se
quiere cerrar (la deriva de credenciales). La copia además **no es necesaria**:
`ApiKeyConfig::load(&state_dir())` se llama **en cada petición**
(`cockpit_server.rs:821`, E-01) y `state_dir()` lee el entorno cada vez ⇒ ⇒
**basta con que el proceso del daemon tenga `MEGADAEMON_STATE_DIR=$STATE_REAL`
(que es donde ya está la key)** ⇒ una sola copia, cero deriva.
**Arreglo (preferido)**: NO copiar; exportar el `state_dir` correcto en el
supervisor **antes** de que arranque el daemon y reiniciar el daemon ⇒ y
comprobar que el 403 desaparece.

### P1-6 · `mv` dentro de `su -mm` (otro *namespace* de montajes)
`fix-movil.sh:89` → el `mv` de `megadaemon-core.new` (descargado por el usuario
Termux) se hace **dentro** de `su -mm` ⇒ si ese namespace no ve el directorio o
el fichero tiene otro propietario, el `mv` falla ⇒ y **el `.new` se pierde**
⇒ ⇒ sin `mv` no hay instalación y el estado final miente.
**Arreglo**: hacer las operaciones sobre `$MD_DIR` **en el namespace de Termux**
(el mismo que descarga y el mismo que ya usa el daemon) y reservar `su -mm` para
`/data/user/0/...` y para el supervisor.

### P1-7 · El rollback puede fallar en silencio
`fix-movil.sh:90,99` → `cp -f $BIN.bak $BIN` ⇒ si **no había backup** (el
`.bak` no existe) el `cp` falla y **no dice nada** ⇒ el móvil se queda **sin
binario** justo cuando el script dice "restaurado el anterior".
**Arreglo**: **antes de tocar nada**, comprobar que existe un binario sano
(`$BIN` ejecutable con sha256 conocido) y **guardarlo**; si no lo hay, no
instalar nada (o instalar el del release) ⇒ y en el rollback, verificar que el
`cp` funcionó (`test -x $BIN`) antes de decir "restaurado".

---

## P2 · Lo que promete la cabecera y no hace (o miente en el diagnóstico)

### P2-1 · No verifica el **MCP**, que es el canal real de la flota
Sólo se comprueba `/health` (`fix-movil.sh:121`) ⇒ el 403 de `POST /mcp` puede
persistir aunque `/health` dé 200 ⇒ **falso verde para el enjambre**.
**Arreglo**: verificar `POST <url>/mcp` con `initialize` + `X-API-Key` y exigir
`result.structuredContent` (o al menos HTTP 200 sin `forbidden`).

### P2-2 · Promete "UN SOLO cloudflared" y **no mata ninguno**
`fix-movil.sh:104-110` sólo avisa ⇒ ⇒ la promise de la cabecera es falsa ⇒ y
varios quick-tunnels **roban tráfico y rotan hostnames**.
**Arreglo**: o se ejecuta (matar los sobrantes **después** de tener el bueno
anotado, nunca antes) o se corrige la cabecera. **Importante**: matar un
cloudflared **cambia la URL pública** ⇒ la flota se queda ciega ⇒ ⇒ si se hace,
avisar al tablón con la URL nueva **antes**.

### P2-3 · El diagnóstico lee la key de un sitio y el daemon usa otro
`fix-movil.sh:119` lee `$STATE_REAL/megadaemon_api.key` mientras el daemon
mira `$ROOTSTATE` ⇒ si la copia falló, el `health` se prueba con una credencial
que el daemon no está usando ⇒ el mensaje "la credencial no cuadra" **miente**.
**Arreglo**: leer siempre del `state_dir` **efectivo** (el mismo que usa el
daemon) y, si son dos, imprimir ambos sha16.

---

## Lo que SÍ está bien (para no rehacerlo)
- El **orden `mv` → `chmod 755` → `test -x`** es correcto (el `mv` pierde el
  bit +x ⇒ por eso el chmod va detrás) ⇒ **no lo cambies**.
- El **backup antes de instalar** y el **rollback** están en el sitio correcto.
- El **`pkill -x megadaemon-core`** (no `-f`) es correcto y seguro.
- La **impresión de sha16 en vez de la key** es correcta y debe mantenerse.
- El **script es idempotente en su mayor parte** (copiar la key, reinstalar el
  binario) ⇒ ejecutar dos veces no rompe nada ⇒ bueno para "reparar y volver a
  reparar".

## Resumen para el tablón (3 líneas)
1. **QUÉ**: 3 P0 + 7 P1 + 3 P2 en `fix-movil.sh`/`fix-entorno.sh`; el P0-1
   (URL de release inexistente, **404 medido**) hace que el script **no llegue a
   instalar nada** ⇒ por eso el release más reciente además **no lleva** el
   binario principal (mi rama `agent/fix-build-main` lo arregla en el CI).
2. **TOQUE**: yo no edito sus ficheros ⇒ el informe con el arreglo exacto de cada
   punto está en `docs/REVIEW_FIX_MOVIL.md` (+ `tools/yaml_lint.py` y el fix de
   CI en `agent/fix-build-main`, ya publicados).
3. **BLOQUEA**: el release que se descarga **no tiene** el ARM64 ⇒ hay que
   publicar un release con el binario (mi fix de CI lo garantiza: sin ARM64
   **no hay** release) ⇒ y el `state_dir` hay que **unificarlo sin copiar la key**
   (P1-5), no creating una segunda copia.

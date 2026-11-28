# Verificar el one-liner en el móvil (checklist para el jefe)

> Los **7 bugs** del instalador se han arreglado y **verificado en local**
> con el banco de 134 escenarios (`docs/BANCO_INSTALADOR.md`).  Lo que **no**
> se puede verificar en local es el resultado en **el móvil real**: por eso
> esta lista.
>
> El móvil destapó además **4 bugs más** (`d0b7fd2`): todos estaban en el
> mismo fichero y **todos son de permisos** (leer como usuario normal lo que
> es de root).  Se comprueban en la **§3**.
>
> MEDIDO 2026-11-28.  Todos los comandos son de **lectura**: no borran nada.
> Rutas medidas en `deploy/megadaemon-oneliner.sh`:
> `PREFIX=/data/adb/megadaemon`, `STATE=$PREFIX/state`,
> `DAEMON=$PREFIX/megadaemon-core`.
>
> **Ojo**: `/data/adb` es `0700` de root ⇒ **todos** los comandos que leen
> el estado (telemetría, `cmdline`, el token, el canal) hay que lanzarlos
> **como root**: `su -mm -c "…"` desde Termux, o directamente en un shell de
> root.  Leídos como usuario normal dan **falsos negativos** (que es
> exactamente lo que pasaba antes del fix).

## 1. Ejecutar

En el móvil (Termux, Magisk o el shell admin de Android):

```sh
sh -c 'curl -fsSL --retry 3 https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh | sh'
```

> MEDIDO (2026-11-28, móvil): la variante que **guardaba** el fichero
> (`-o /data/local/tmp/mdo.sh`) falla con `curl: (23) client returned ERROR on
> write` ⇒ **la descarga va bien, la ESCRITURA no**: `/data/local/tmp` es de
> otra app (o está lleno, o el fichero quedó de root).  Por eso la forma
> **oficial es la de la tubería**: no escribe nada.
>
> El script **no lee de `stdin`** (MEDIDO: cero `read` en el fichero) ⇒
> `curl | sh` es seguro: `sh` no se come el resto del script.
>
> Si aun así quieres el fichero en disco, guárdalo en `$HOME` (siempre
> escribible por Termux):
>
> ```sh
> sh -c 'curl -fsSL --retry 3 https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh -o "$HOME/mdo.sh" && sh "$HOME/mdo.sh"'
> ```
>
> Y para saber **por qué** falló la escritura (importa: un `/data/local/tmp`
> lleno explica más problemas del móvil):
>
> ```sh
> ls -l /data/local/tmp/mdo.sh; df -h /data/local/tmp | tail -1; du -sh /data/local/tmp 2>/dev/null
> ```

## 2. Lo que tiene que salir (7 comprobaciones)

### 2.1 El instalador **termina sin errores**

Lo último que imprime es el resumen del instalador.  Si se para con un
error, **eso es un fallo nuevo**: pásame la salida **entera**.

### 2.2 El daemon **vive** (bug de "daemon NO CORRE")

```sh
ps -A | grep megadaemon-core
```

⇒ tiene que haber **un** proceso.  Antes el instalador se quedaba sin
daemon (la función devolvía la palabra "cmdline" en vez del PID) ⇒ era
imposible matarlo y reiniciarlo.

### 2.3 La **autorreparación** existe (typo `autoreparar`)

```sh
ls -l /data/adb/megadaemon/
```

⇒ tiene que estar `/data/adb/megadaemon/megadaemon-core`, y el comando de
reparación **existe**:

```sh
/data/adb/megadaemon/megadaemon-core reparar --diagnostico 2>&1 | head -20
```

⇒ antes salía `sh: autoreparar: not found` (el mensaje tenía una `r` de
menos que la función).  Si sale `not found`, el typo sigue ahí.

### 2.4 El **informe JSON** es JSON (bug del `{,`)

```sh
/data/adb/megadaemon/megadaemon-core reparar --json 2>&1 | head -5
```

⇒ la primera línea tiene que **parsear** como JSON.  El bug anterior
scribía `{,` al principio y el informe entero era inservible.

### 2.5 El **watchdog** no relanza un daemon vivo

```sh
ps -A | grep -c megadaemon-core
```

⇒ **1** proceso, no 2.  Antes el watchdog relanzaba el daemon aunque
estuviera vivo (duplicaba procesos y peleaban por el mismo puerto).

### 2.6 El **hash** cuadra (bug del `tr -d ' '`)

El instalador descarga el **bundle** `megadaemon-movil.tar.gz` y su
`.sha256`, y compara **sólo el primer token** del `.sha256` (el fichero del
CI lleva la ruta del job pegada).

**a) En la salida del instalador** no debe aparecer "NO cuadra" ni un error
de sha256.  (El bundle y su `.sha256` se descargan a un directorio temporal
que se **borra al terminar**, así que no se pueden comprobar a mano después:
por eso el veredicto es el de la salida.)

**b) El hash nativo del daemon** (el mismo camino que usa el instalador en
un móvil sin `sha256sum`):

```sh
/data/adb/megadaemon/megadaemon-core hash /data/adb/megadaemon/megadaemon-core
```

⇒ tiene que imprimir `<hash>  <ruta>`: un hash de 64 caracteres hexadecimales
seguido de la ruta.  El bug anterior (`tr -d ' '` pegaba el hash con la
ruta) hacía que **esta** comparación **nunca** cuadrara y que el instalador
muriera siempre con "el sha256 NO cuadra".

> En un móvil **sin** `sha256sum` (el shell de Android no lo trae) el
> instalador **no debe morir**: verifica más tarde, con el daemon ya
> extraído (`megadaemon-core hash <fichero>`), y si tampoco cuadra,
> **aborta**.  Si tu móvil no tiene `sha256sum`, dime sólo si el instalador
> **termina**.

### 2.7 El **túnel** levanta y su **URL** está en el sitio

```sh
ls -l /data/adb/megadaemon/state/tunnel.url /data/adb/megadaemon/state/tunnel.log
cat /data/adb/megadaemon/state/tunnel.url
tail -5 /data/adb/megadaemon/state/tunnel.log
```

⇒ `tunnel.url` tiene que existir y `tunnel.log` **no** puede estar vacío.
Antes los dos se escribían fuera del estado persistente ⇒ se perdían al
reiniciar.

## 3. Los 4 bugs que destapó el móvil (`d0b7fd2`)

> MEDIDO 2026-11-27 en el SM-G998B.  Los cuatro eran **lecturas hechas como
> usuario normal** de cosas que son de **root** (`/data/adb` es `0700`), así
> que el síntoma era siempre "no hay daemon" / "no hay token" **con el daemon
> vivo**.  Se comprueban mirando **la salida del instalador** y con cuatro
> comandos de lectura **como root**.

### 3.1 La telemetría se publica (bug de `[ ! -x "$DAEMON" ]`)

El instalador, en el paso `=== 7/8 telemetría a Google Drive ===`, **no**
puede decir:

```
  no hay daemon: no hay nada que publicar
```

⇒ si lo dice, el `test -x` se sigue haciendo **como usuario normal**.

Y **debajo** tienen que aparecer las líneas de la telemetría real (van
sangradas con 4 espacios, son la salida de `megadaemon-core telemetria
publicar`):

```
  publicando telemetría, conexión, informe y diagnóstico en Drive…
```

Comprobación independiente (como root):

```sh
su -mm -c "test -x /data/adb/megadaemon/megadaemon-core && echo EJECUTABLE_OK"
```

⇒ `EJECUTABLE_OK`.  Como usuario normal esto **falla** (y por eso el bug
no se veía en local).

### 3.2 El diagnóstico enseña la línea de comandos (bug de `/proc/<pid>/cmdline`)

En el bloque de diagnóstico tiene que salir la **línea de comandos**, no
sólo el PID:

```
  proceso: PID 25007 (/data/adb/megadaemon/megadaemon-core cockpit …)
```

⇒ si sale `proceso: PID 25007 ()` **vacío**, el `cmdline` se sigue leyendo
como usuario normal (`cannot open /proc/25007/cmdline`).

Comprobación independiente (como root):

```sh
su -mm -c "tr '\0' ' ' < /proc/\$(ls /proc | grep -E '^[0-9]+\$' | head -1)/cmdline"
```

o, mejor, sobre el PID real que reporte el diagnóstico:

```sh
su -mm -c "cat /proc/<PID>/cmdline | tr '\0' ' '"
```

⇒ tiene que devolver la ruta del daemon.

### 3.3 El canal se publica en el repo (bug de las 4 rutas del token)

El bucle de publicación buscaba el token del canal en **4** rutas y la
función `token_github_ruta` en **5** ⇒ con el token en
`$HOME/.mavis/github.token` el diagnóstico decía "hay token" pero el canal
**no** iba al repo.

En la salida del instalador tiene que aparecer:

```
  canal publicado en el repo (state/channel.json)
```

Comprobación independiente (como root):

```sh
su -mm -c "cat /data/adb/megadaemon/state/channel.json"
```

⇒ **tiene que existir y no estar vacío**.  Y la 5ª ruta (la que faltaba),
si es donde está tu token:

```sh
su -mm -c "ls -l \$HOME/.mavis/github.token"
```

### 3.4 No quedan daemons duplicados (bug de `pgrep -f`)

`pgrep` **no existe** en toybox (el shell de Android) ⇒ la limpieza
**no mataba nada** y el móvil se quedó con **6 daemons, 6 cloudflared y 5
watchdogs** (load 58 ⇒ el túnel no podía arrancar).

Comprobación (como root) — la misma lista de PIDs que usa ahora el
instalador:

```sh
su -mm -c "grep -lsa 'megadaemon-core cockpit' /proc/[0-9]*/cmdline 2>/dev/null" | sed 's#^.*/##; s#/cmdline$##'
```

⇒ tiene que devolver **un solo PID**, y ese PID **no** puede ser un
envoltorio `sh -c` (el `cmdline` de un envoltorio contiene `sh -c`; si
fuera un envoltorio, el instalador habría matado el daemon bueno).

Los tres recuentos tienen que ser **1**:

```sh
su -mm -c "grep -lsa 'megadaemon-core cockpit' /proc/[0-9]*/cmdline 2>/dev/null" | wc -l
su -mm -c "grep -lsa cloudflared /proc/[0-9]*/cmdline 2>/dev/null" | wc -l
su -mm -c "grep -lsa tunnel-watchdog /proc/[0-9]*/cmdline 2>/dev/null" | wc -l
cat /proc/loadavg
```

⇒ `1`, `1`, `1` y una **load baja** (antes: 58).

> Si al ejecutar el instalador **no** se limpió, es que el móvil ya venía
> con duplicados de antes del fix: se pueden matar a mano con los PIDs de
> la primera lista (`kill -9 <PID>`), pero **con cuidado** de no matar el
> envoltorio `sh -c` del daemon bueno.

### 3.5 No queda un **supervisor duplicado** en `service.d` (2026-11-28)

MEDIDO (2026-11-28, telemetria del movil): el tunel `mcp-legacy`
(`127.0.0.1:8765`) se relanzaba **en bucle** (12 reinicios/15 min, load 20,
1 GB de datos, 26 reinicios/dia) y **nadie escuchaba en el 8765**.

La causa raiz esta en el movil: `/data/adb/service.d/99-megadaemon.sh.bak-50k`
era una **copia VIEJA** del supervisor, y **Magisk ejecuta todo lo ejecutable
de `service.d/`** => era un **segundo supervisor** (sin el gate
`mcp_legacy_enabled` y con `pgrep -f`, que en toybox no es fiable) =>
relanzaba el cloudflared del 8765 cada vuelta, mientras el supervisor
canonico lo mataba cada vez (los dos se peleaban: 70 s / 133 s alternados).

Comprobaciones (como root; `busybox` no tiene `pgrep`):

```sh
# 1) Los ficheros de service.d: SOLO el canonico (99-megadaemon.sh).
su -mm -c "ls -l /data/adb/service.d/ | grep -i megadaemon"

# 2) Los procesos: SOLO el supervisor canonico (ningun .bak).
su -mm -c "for p in /proc/[0-9]*; do c=\$(tr '\\0' ' ' < \$p/cmdline 2>/dev/null); case \"\$c\" in *99-megadaemon.sh*) echo \"\${p##*/} \$c\";; esac; done"

# 3) El 8765 NO tiene que tener listener (8765 = 0x2235): salida vacia.
su -mm -c "grep -i ':2235 ' /proc/net/tcp /proc/net/tcp6"

# 4) El log del supervisor no debe anunciar el tunel legacy (0 apariciones).
su -mm -c "grep -c 'mcp-legacy caido' /sdcard/megadaemon.log"

# 5) La carga (antes: load 20) y el consumo de datos.
cat /proc/loadavg
```

=> (1) solo `99-megadaemon.sh`; (2) un solo PID y su cmdline **no** puede
contener `.bak`; (3) **vacio**; (4) `0`; (5) load baja.

Si (1) enseña un `.bak*`, se borra a mano (el instalador ya lo hace desde el
2026-11-28, pero si el fichero quedo de una instalacion anterior hay que
quitarlo):

```sh
su -mm -c "rm -f /data/adb/service.d/99-megadaemon.sh.bak*"
```

Y para el proceso viejo (borrar el fichero no mata el proceso ya arrancado):

```sh
su -mm -c "for p in /proc/[0-9]*; do c=\$(tr '\\0' ' ' < \$p/cmdline 2>/dev/null); case \"\$c\" in *99-megadaemon.sh.bak*) kill -9 \${p##*/};; esac; done"
```

## 4. Si algo falla

Pásame:

1. La salida **completa** del instalador (no un trozo).
2. El resultado de los comandos de la §2 que falló.

Con eso se acota: el banco tiene 134 escenarios y cada bug tiene su
escenario, así que se reproduce en local sin tocar el móvil.

## 5. Antigravity: el hooking y el modelo (2026-11-28)

Estas 5 comprobaciones son **nuevas**: cubren los fallos que el jefe
mandó en capturas el 2026-11-27 (`ERR_LLM_UNREACHABLE:127.0.0.1:8080`,
hooking inutilizable, proveedores en pausa sin explicación).  El detalle
de cada una está en `docs/ANTIGRAVITY_HOOKING.md`.

El shell de Android **no tiene `curl`/`wget`/`pgrep`/`sha256sum`**: las
comprobaciones van por el MCP del móvil, por la Web UI o por `busybox`.

### 5.1 El daemon sirve el 8080 (el puerto que usa la app)

Por el MCP: tool `antigravity_endpoint`, acción `estado` ⇒
`puertos_con_listener` tiene que incluir **8080** y **9090**.

O en el shell como root:

```sh
busybox netstat -ltn | grep -E ':(8080|9090)[[:space:]]'
```

⇒ los dos en `LISTEN`.  Si el 8080 no sale, el móvil corre un binario
**viejo**: re-ejecuta el one-liner (§1) y comprueba `build.desfasado`.

### 5.2 El hooking está activo **y** en un puerto con listener

Por el MCP: `antigravity_endpoint` acción `estado` ⇒

* `parcheado`: **true**,
* `url_en_disco`: la URL con el puerto que tiene listener.

Si `parcheado` es false, aplícalo: desde la **Web UI** (tarjeta "Hooking
del endpoint" → botón **Aplicar**, puerto 8080 o 9090) o por el MCP con
`accion: aplicar`, `puerto`, `confirm: true`.

El daemon **rechaza** parchear a un puerto sin listener (`puerto_usable`):
si lo intentas, dice `puerto_muerto` y **no toca el binario**.

### 5.3 La app ya no da `ERR_LLM_UNREACHABLE`

Haz una pregunta en la app de Antigravity.  Si sale
`ERR_LLM_UNREACHABLE: LLM_UNREACHABLE:127.0.0.1:8080: Connection refused`,
el LS sigue apuntando a un puerto sin listener ⇒ vuelve a 5.2 (el estado
dice si hay listener) y comprueba 5.1.

### 5.4 La cuota en pausa dice POR QUÉ y HASTA CUÁNDO

La línea de estado del cockpit (nivel **cuota**) ahora trae el motivo de
cada proveedor:

```
5 proveedor(es) en pausa por cuota: minimax-m3 (Token Plan usage limit
reached (HTTP 429); vuelve en 14 min), pollinations-free (…)
```

En JSON, `cuota.por_proveedor[]` trae `proveedor`, `detalle` y
`vuelve_en_s`.

### 5.5 La traza dice qué pasó con el prompt y las tools

En la traza del gateway (Web UI → logs, o el log del daemon):

* `tools recortadas: N fuera (presupuesto …)` ⇒ el LS mandó más
  `functionDeclarations` de las que caben en el límite del proveedor
  (esto ya no da HTTP 413).
* `gemini nativo: …` ⇒ el proveedor es `google-ai-studio` y la
  conversación se ha reconstruido con la estructura que exige Gemini.

## 6. Lo que el banco ya garantiza (no hace falta repetir en el móvil)
Los 134 escenarios pasan en local:

```sh
MD_LAB_BASE=/tmp/md-lab-v MD_PAR=6 bash tools/test-oneliner.sh
```

⇒ 134/134.  El banco comprueba, entre otras cosas: detección de entorno,
escalado a root, descarga, instalación atómica con rollback, idempotencia,
el stack (3 capas), el túnel, el informe, el doctor, la autoreparación y la
seguridad (permisos 0700, sin secretos en la salida).

Lo que el banco **no** puede comprobar es lo que depende del **dispositivo**:
root real de Samsung, el túnel de Cloudflare con la red del móvil y el
arranque tras un reinicio.

## 7. Lo nuevo de este tramo (2026-11-28): el bus, el MCP real y el esquema

Tres cosas que se pueden mirar **en el móvil** y que antes no se veían.

### 7.1 El bus ya conoce las capacidades propias del daemon

Por el MCP: tool `service_bus` (o `/api/service_bus` del cockpit) ⇒ tiene
que haber **una** entrada con `id` = `capacidades` y de tipo `local_tool`, y
sus `capabilities` tienen que ser las familias: `cola`, `contexto`,
`modelos`, `revert`, `subagente`.

Antes (MEDIDO) el bus sólo conocía los modelos, los peers y el cliente MCP ⇒
las capacidades propias no aparecían por ninguna parte.

### 7.2 Los servidores MCP **reales** salen del `mcp_config.json`

Por el MCP: tool `mcp_control`, acción `listar` ⇒ la respuesta tiene que
traer los servidores declarados en
`/data/user/0/com.agy/files/.gemini/config/mcp_config.json` (el que usa
Antigravity), con su `url` o su `command`, y `enabled` a `true` salvo que
estén apagados.

* Si la respuesta dice que **no se pudo leer**, el fichero no está o el
  daemon no tiene permiso: es información, no un fallo del producto.
* La tool está en el **catálogo** MCP (`tools/list` ⇒ `mcp_control`).

### 7.3 La base de la instancia lleva el **esquema medido**

La instancia tiene **una** base SQLite: `<state_dir>/board.sqlite`.  En ella
conviven las tablas del tablón y las **10 tablas medidas** de las sesiones
(`sessions`, `messages`, `message_nodes`, `subagent_heads`,
`tool_call_state`, `prompt_history`, `rendered_commits`, `live_row_times`,
`session_window_tools`, `app_state`).

Como el shell de Android no trae `sqlite3`, míralo por el daemon: crea una
sesión ACP (tool `acp_sesion`, acción `nueva`) y luego comprueba que el
fichero ha crecido:

```sh
ls -l /data/adb/megadaemon/state/board.sqlite
```

Si el fichero **no** existe tras arrancar el daemon, el binario del móvil es
**viejo**: re-ejecuta el one-liner (§1) y mira `build.desfasado`.

## 8. Lo que el one-liner **comprueba solo** (2026-11-28)

Desde este tramo el instalador **no dice "terminado" sin comprobar**: al
final (paso 8/8) hace **8 comprobaciones** sobre lo que se puede mirar en el
móvil, las deja en el informe JSON (`instalacion.json`, campo `pasos`) y las
resume en la salida:

| # | Qué comprueba | Cómo lo ve el jefe |
|---|---------------|--------------------|
| 1 | `service.d` tiene **sólo** los 4 canónicos | `service.d: solo los canonicos` |
| 2 | hay **un** supervisor vivo | `supervisores: 1 (el canonico)` |
| 3 | el **8765** (legacy) NO tiene listener | `8765 (tunel legacy): sin listener (correcto)` |
| 4 | el **8080** (app) y el **9090** (daemon) sí | `puertos: 8080 y 9090 con listener` |
| 5 | el túnel legacy está **apagado** (URL vacía) | `tunel legacy (8765): apagado (sin URL)` |
| 6 | el túnel web ha publicado **URL** | `tunel web: URL viva` |
| 7 | existe la **base** de la instancia | `base de la instancia: .../board.sqlite` |
| 8 | el **stack** está en línea con el binario | `el stack esta en linea con el binario` |

La última línea de esa sección dice:

```sh
  verificaciones: 7/8
```

* `7/8` ⇒ una comprobación falló: el veredicto pasa a **`ok_con_avisos`** y
  el informe lo dice (`"resultado": "ok_con_avisos"`).
* `(N sin datos: el daemon no las informa)` ⇒ el daemon **no da** esa clave
  (binario viejo): **no es un fallo**, es información.

Además, el instalador ahora:

* **borra las copias** de `service.d` (`99-megadaemon.sh.bak-50k`): Magisk
  ejecuta todo lo ejecutable de ahí ⇒ la copia era un supervisor más;
* **mata el supervisor NO canónico** comparando el argv **elemento a
  elemento** (la copia tiene la ruta canónica como *prefijo*, así que un
  `case` sobre el texto unido la daba por buena ⇒ se quedaba el malo);
* **deja UN daemon** (antes `pgrep -f`/`pkill -f` no existen en toybox ⇒ la
  limpieza no mataba nada).

> MEDIDO (2026-11-28, banco): `kill` es un **builtin** de `sh`/dash ⇒ tapa al
> binario; por eso el instalador mata con `env kill` (al binario de verdad).
> Y el `sed` que extrae el PID de `/proc/<pid>/cmdline` va en este orden:
> primero `s#/cmdline$##` y **después** `s#.*/##` (al revés, el voraz se
> comía el PID y devolvía `cmdline` ⇒ `kill -9 cmdline` no mataba nada).

## 9. Lo que el móvil destapó (2026-11-28, tarde)

El jefe ejecutó el one-liner en el móvil (Samsung + Magisk, build `4a51e82`)
**dos veces**. Lo que salió:

### Lo que quedó PROBADO (bien)
* La limpieza de duplicados **mata de verdad** (2 daemons, 4 supervisores,
  3 keepers, 3 watchdogs, 2 cloudflared) ⇒ confirma que los bugs del `sed`
  voraz y del `kill` builtin estaban también en el móvil.
* `verificaciones: 7/8` · `service.d: solo los canonicos` · `8765 sin
  listener` · `8080 y 9090 con listener` · **`board.sqlite` presente** ·
  `stack en linea` · `/health` 200 con el build correcto.

### Los 4 fallos que quedaban (y su causa)
1. **`tunnel.url = https://api.trycloudflare.com`** (la URL de la API, NO un
   túnel) ⇒ la sonda falla ⇒ veredicto `ko` **con el túnel VIVO**. Causa: el
   regex `https://[a-z0-9-]+\.trycloudflare\.com` **también casa `api.`**.
   Arreglado en: `magisk_module.rs`, el one-liner, el supervisor empaquetado,
   `phone-recover.sh` y `phone_finalize.sh`.
2. **La telemetría se publicaba sin root** ⇒ `Permission denied`
   (`/data/adb` es `0700` root). Arreglado: las tres llamadas van por `r`.
3. **El estado converge al MISMO equilibrio** (supervisor=4 watchdog=4
   cloudflared=5 daemons=6, `load1=58`) en las dos ejecuciones, con PIDs
   distintos ⇒ es un **relanzamiento mutuo**, no un residuo. Causa:
   `97-megadaemon-keeper.sh:49` (`[ -r "$_pp/cmdline" ] || continue`): si el
   cmdline no es legible, el proceso cuenta como **muerto** ⇒ el keeper
   relanza al supervisor detrás del vivo. Arreglo en curso: ficheros de PID
   + `kill -0` (no depender de leer cmdlines).
4. **El daemon contaba 0 supervisores y el shell 4** ⇒ los dos contadores
   identificaban por subcadena y **se contaban a sí mismos**. Arreglado con
   `EstadoSupervisores` (`total`, `hay_canonico`, `fiable`) y comparación
   del argv **elemento a elemento**.

### El restaurador (sin reiniciar)
```sh
curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/tools/restaurar-movil.sh -o $HOME/restaurar.sh
su -mm -c "sh $HOME/restaurar.sh"
```
Instala/actualiza → **para todo** el stack (keeper primero: es el que
relanza) → limpia (copias `.bak*` y URLs) → **arranca uno de cada** →
verifica. Ojo: como root el PATH no tiene `curl` (lo tiene Termux) ⇒ el
script lo busca en `/data/data/com.termux/files/usr/bin`.

> **VUELVE A DESCARGARLO** (2026-11-28, tarde): la versión que había
> publicada era **ciega** (ver §11): no veía ni un supervisor ⇒ no paraba
> los duplicados y no dejaba uno de cada.

## 10. Una sola pasada: el verificador de DENTRO del móvil

Hay **dos** verificadores y no se confunden:

| Script | Dónde corre | Para qué |
|---|---|---|
| `tools/verificar-movil.sh` | **desde el PC**, contra la URL del túnel | ¿volvió el móvil tras el reinicio? ¿qué build trae? |
| `tools/verificar-movil-dentro.sh` | **dentro del móvil**, como root | ¿está el móvil sano AHORA? (duplicados, puertos, túnel, esquema, build) |

El de dentro se descarga y ejecuta así:

```sh
curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/tools/verificar-movil-dentro.sh -o $HOME/verificar.sh
su -mm -c "sh $HOME/verificar.sh"
```

Imprime **20 comprobaciones** (19 si el `board.sqlite` no existe: entonces
sólo se puede comprobar que falta) con `[PASA]` / `[FALLA]` / `[SIN DATOS]` y un
veredicto con contadores. Es **de sólo lectura**: no mata, no reinicia, no
descarga. Para arreglar lo que falle, `tools/restaurar-movil.sh`.

Qué mira (todo lo que estaba repartido por las §2, §3.5, §5 y §7):

* `service.d`: sólo los canónicos (sin copias `.bak*`).
* **Un** proceso de cada rol: supervisor, keeper, watchdog, cloudflared y
  daemon. Y que el supervisor vivo sea **el canónico** (elemento a elemento:
  la copia `.bak-50k` tiene la ruta canónica como prefijo y colaría).
* Puertos: `8765` **sin** listener (el túnel legacy sobra), `8080` y `9090`
  **con** listener (la app y la Web UI).
* `tunnel.url`: host `*.trycloudflare.com` que **no** sea `api.` ni
  `developers.` (el bug del §9.1).
* El log del supervisor: **0** apariciones de `mcp-legacy caido`.
* `board.sqlite`: existe y tiene las **10 tablas medidas** (§7.3).
* El binario en servicio: existe, es ejecutable y el daemon dice **0 builds
  desfasados** (§7.1).
* `/api/health` → 200, carga (`load1`) y memoria disponible.

MEDIDO (2026-11-28): el shell de Android **no** trae `curl`, `wget`, `pgrep`
ni `sqlite3`; este script no usa ninguno de los cuatro (los puertos se leen
de `/proc/net/tcp` con `awk`, los procesos de `/proc/<pid>/cmdline` y las
tablas del binario SQLite con `grep`).

## 11. Lo que destapó el BANCO el 2026-11-28 (tarde): el restaurador era ciego

El restaurador se publicó sin que nadie lo ejecutara en el móvil. Al meterlo
en el banco (series E: `tools/test-oneliner-casos-e.sh`) se vio que **no
detectaba NINGÚN proceso**, y **sin decir error**.

**Causa** (`tools/restaurar-movil.sh`, `argv_acaba_en`):

```sh
argv_de "$1" | grep -q "$2\$"      # el patrón resultante es 99-megadaemon\.sh$$
```

`"$2\$"` = el sufijo del rol **más un `$` de más** ⇒ el patrón lleva **DOS
anclas de fin**. GNU grep trata el `$` interior como un **literal** ⇒ no
casa con NADA y **no se queja** (MEDIDO: `printf 'sh\n' | grep -q 'sh$$'` →
`rc=1`, sin error).

**Consecuencia**: el restaurador paraba **0** procesos, no dejaba uno de
cada y la verificación final reportaba `vivos (...): 0` para **todos** los
roles: habría parecido un restaurador que funciona y no toca nada.

**Arreglo**: ERE con **un** solo ancla (`grep -qE "$2"`), que casa con el
canónico y **no** con la copia `.bak-50k`. El banco lo blinda:

* `caso_e_01` ⇒ cada rol se detecta por su argv y la copia `.bak-50k` no se
  confunde con el canónico;
* `caso_e_02` ⇒ el "pastel" del móvil (4 supervisores, 3 watchdogs, 3
  cloudflared, 3 daemons, 3 keepers) se cuenta **completo**.

MEDIDO (banco): con el bug reintroducido **12 aserciones fallan**; con el
arreglo **0**. El restaurador carga en *modo biblioteca* (`MD_LIB_ONLY=1` ⇒
sólo define funciones, no ejecuta las fases) contra un `/proc` falso
(`MD_PROC`), así que se puede probar sin root y sin matar nada.


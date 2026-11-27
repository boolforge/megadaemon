# Verificar el one-liner en el móvil (checklist para el jefe)

> Los **7 bugs** del instalador se han arreglado y **verificado en local**
> con el banco de 119 escenarios (`docs/BANCO_INSTALADOR.md`).  Lo que **no**
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
sh -c 'curl -fsSL --retry 3 https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh -o /data/local/tmp/mdo.sh && sh /data/local/tmp/mdo.sh'
```

O, más corto (sin dejar el fichero):

```sh
sh -c 'curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh | sh'
```

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

## 4. Si algo falla

Pásame:

1. La salida **completa** del instalador (no un trozo).
2. El resultado de los comandos de la §2 que falló.

Con eso se acota: el banco tiene 119 escenarios y cada bug tiene su
escenario, así que se reproduce en local sin tocar el móvil.

## 5. Lo que el banco ya garantiza (no hace falta repetir en el móvil)

Los 119 escenarios pasan en local:

```sh
MD_LAB_BASE=/tmp/md-lab-v MD_PAR=6 bash tools/test-oneliner.sh
```

⇒ 119/119.  El banco comprueba, entre otras cosas: detección de entorno,
escalado a root, descarga, instalación atómica con rollback, idempotencia,
el stack (3 capas), el túnel, el informe, el doctor, la autoreparación y la
seguridad (permisos 0700, sin secretos en la salida).

Lo que el banco **no** puede comprobar es lo que depende del **dispositivo**:
root real de Samsung, el túnel de Cloudflare con la red del móvil y el
arranque tras un reinicio.

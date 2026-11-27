# Verificar el one-liner en el móvil (checklist para el jefe)

> Los **7 bugs** del instalador se han arreglado y **verificado en local**
> con el banco de 119 escenarios (`docs/BANCO_INSTALADOR.md`).  Lo que **no**
> se puede verificar en local es el resultado en **el móvil real**: por eso
> esta lista.
>
> MEDIDO 2026-11-28.  Todos los comandos son de **lectura**: no borran nada.
> Rutas medidas en `deploy/megadaemon-oneliner.sh`:
> `PREFIX=/data/adb/megadaemon`, `STATE=$PREFIX/state`,
> `DAEMON=$PREFIX/megadaemon-core`.

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

## 3. Si algo falla

Pásame:

1. La salida **completa** del instalador (no un trozo).
2. El resultado de los comandos de la §2 que falló.

Con eso se acota: el banco tiene 119 escenarios y cada bug tiene su
escenario, así que se reproduce en local sin tocar el móvil.

## 4. Lo que el banco ya garantiza (no hace falta repetir en el móvil)

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

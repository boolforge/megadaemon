# El banco del instalador del móvil (`tools/test-oneliner.sh`)

> Por qué existe: el instalador del móvil (`deploy/megadaemon-oneliner.sh`) es
> **la puerta de entrada del usuario** (una línea pegada en Termux). Un fallo
> suyo es un fallo público. En el móvil real no se puede probar: cada
> ejecución real tarda minutos, consume datos y, cuando falla, deja el móvil a
> medias. Los 4 bugs que se vieron en el SM-G998B (MEDIDO 2026-11-27) sólo
> aparecieron en el móvil.

## Qué es (v2, 2026-11-27)

La v1 eran 10 escenarios escritos a mano: un único laboratorio, el `curl` de
mentira en el `PATH` y `grep` sobre el **texto** del instalador para "probar"
cosas que sólo se pueden probar **ejecutándolo**. Eso deja pasar bugs reales
—y de hecho dejó pasar cuatro.

La v2 es un banco **aislado**:

| Pieza | Qué hace |
|---|---|
| **Laboratorio por escenario** | prefijo, estado, `service.d`, fakes, logs y procesos propios: un escenario no puede contaminar a otro. |
| **`PATH` restringido** (`$TBR`) | el instalador corre con una caja de herramientas mínima, como el shell de Android (que no tiene `curl`, `wget`, `sha256sum`, `ps` ni `pgrep`). El banco es **hermético**: nunca toca la red real. |
| **`/proc` falso** | `grep`, `pgrep`, `pkill`, `ps` y `kill` son de mentira: los PIDs los fija el escenario (`fijar_pids`). Se puede probar el diagnóstico, el rollback y la limpieza de duplicados sin esperar a que nada arranque. |
| **Red falsa** | cada URL del release (estable, manifiesto, API, `.sha256`) se puede hacer fallar por separado ⇒ se prueban las TRES capas de resolución y el backoff de descarga. |
| **Aislamiento del fallo** | un escenario que no se puede hacer en este entorno (sin root, sin compilador) se **omite** con un motivo explícito; nunca se marca como "OK" para que el banco siga verde. |

## Cómo se ejecuta

```sh
bash tools/test-oneliner.sh                      # todo el banco
bash tools/test-oneliner.sh --fichero=tools/test-oneliner-casos-b.sh
bash tools/test-oneliner.sh --solo='caso_base_*'
bash tools/test-oneliner.sh --listar
MD_PAR=8 bash tools/test-oneliner.sh             # más paralelismo
```

El contrato para escribir escenarios está en
[`tools/test-oneliner-API.md`](../tools/test-oneliner-API.md).
Los escenarios viven en `tools/test-oneliner-casos-*.sh`, uno por fichero, y
son funciones `caso_<nombre>` que el runner descubre solo.

## Los 4 bugs REALES que destapó (MEDIDO 2026-11-27)

Ninguno de estos se veía leyendo el código: salen al **ejecutar** el
instalador en el laboratorio.

1. **`autoreparar` (una *r*) en vez de `autorreparar` (dos *r*)**
   (`deploy/megadaemon-oneliner.sh`, llamada en el paso 8/8).
   En el móvil: `sh: 1255: autoreparar: not found`. El móvil se quedaba
   **muerto** y el atajo de idempotencia lo camuflaba: el 2º arranque dice
   "no toco nada" y no repara nada.
2. **`daemon_vivo` devolvía `cmdline` en vez del PID.**
   `grep -l` devuelve la **ruta** del fichero que casa (`/proc/1234/cmdline`)
   y `sed 's#.*/##'` se come el último componente ⇒ `cannot open
   /proc/cmdline/cmdline` y la comprobación de "el binario en servicio es el
   nuevo" **nunca** podía hacerse.
3. **El informe de instalación salía con JSON inválido** (`{,`) cuando la
   instalación moría antes de cerrarlo. El root lo lee desde Drive y no lo
   podía parsear.
4. **El instalador relanzaba el watchdog aunque ya hubiera uno vivo.** Los
   dos se pelean por el túnel y el hostname del quick tunnel cambia cada
   60 s ⇒ la flota se queda ciega. Ahora, igual que con `cloudflared`, si ya
   hay uno vivo **no** se relanza.

## Los 4 bugs del móvil que ya estaban (MEDIDO 2026-11-27, SM-G998B)

Los del apartado anterior se sumaron a estos cuatro, que también están
cubiertos por el banco (`caso_base_11_los_4_bugs_del_movil`):

- El conteo de procesos se hacía **como usuario normal** (`/proc/*/cmdline` de
  root ⇒ "Permission denied" ⇒ 0 daemons) ⇒ "daemon NO CORRE" con el daemon
  sano. Ahora se cuenta **como root** (`grep -lsa`).
- El tamaño de `gdrive_credentials.json` (0600 de root) se pedía como
  usuario normal ⇒ "Permission denied". Ahora se pide **como root**.
- El prefijo del túnel era el **volátil** (`/data/local/tmp/megadaemon`) en
  `98-cloudflared.sh` y `tunnel-watchdog.sh`, mientras el instalador buscaba
  en el **persistente** (`/data/adb/megadaemon/state`) ⇒ ni `tunnel.log` ni
  `tunnel.url`.
- La comprobación de "el bundle trae el binario" se hacía como usuario
  normal sobre `/data/adb` (0700) ⇒ "el bundle no trae el binario" con el
  bundle correcto delante.

## Nota de seguridad del propio banco

Los fakes del banco se escriben en `$TBR` (por escenario). **Nunca** deben
escribirse por encima de un symlink a un binario del sistema: MEDIDO
2026-11-27, el `cat > "$TBR/df"` seguía el symlink y **piso `/usr/bin/df`**
(el `df` de todo el sandbox empezó a mentir). Por eso el harness borra el
symlink antes de escribir el fake (`rm -f "$TBR/df"`), y los fakes se
escriben con `cat` sobre ficheros ya existentes, nunca sobre symlinks.

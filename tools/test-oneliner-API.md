# API del banco del instalador (`tools/test-oneliner.sh`)

Este documento es el contrato para escribir escenarios del banco del one-liner.
Un escenario es una función `caso_<nombre>` en un fichero
`tools/test-oneliner-casos-*.sh`. El runner las descubre y las ejecuta **en
paralelo**, cada una en un laboratorio aislado.

```bash
bash tools/test-oneliner.sh                              # todo
bash tools/test-oneliner.sh --fichero=tools/test-oneliner-casos-a.sh
bash tools/test-oneliner.sh --solo='caso_a_*'
bash tools/test-oneliner.sh --listar
MD_PAR=8 bash tools/test-oneliner.sh
```

## 1. Variables disponibles dentro del escenario

| Variable | Qué es |
|---|---|
| `$L` | raíz del laboratorio (NUEVA en cada escenario) |
| `$PRE` | prefijo del daemon (`$L/md`) — en el móvil es `/data/adb/megadaemon` |
| `$ST` | directorio de estado (`$PRE/state`) — en el móvil `/data/adb/megadaemon/state` |
| `$SB` | directorio de fakes (`curl`, `wget`, `cloudflared`, `megadaemon-core`) |
| `$TBR` | caja de herramientas: **PATH restringido** (como el shell de Android) |
| `$SVC` | `service.d` de Magisk (`$L/svc`) |
| `$LOG` | registro de invocaciones de los fakes (`$L/invocaciones.log`) |
| `$OUT` | salida del último instalador (`$L/salida.log`) |
| `$LAB_PROC` | dónde viven los PIDs ficticios (`$L/proc/*.pids`) |
| `$HOME`, `$TMPDIR` | home y temporal del laboratorio (escribibles) |
| `$RAIZ` | raíz del repo |

`$CODIGO` = código de salida del último `correr_instalador`.

## 2. Los knobs (lo que falseas del entorno real)

Se fijan **antes** de `preparar_lab` (el runner ya llama a `preparar_lab` una
vez; si cambias un knob, vuelve a llamarla).

| Knob | Valores | Qué falsea |
|---|---|---|
| `NET` | `ok`, `down`, `404`, `sin-manifiesto`, `solo-api`, `sin-sha` | qué URLs del release fallan |
| `BUNDLE` | `ok`, `corrupto`, `vacio`, `sin-binario`, `sin-mddoctor`, `sin-stack`, `no-ejecutable` | el bundle que llega |
| `SHA` | `ok`, `distinto`, `ausente`, `vacio`, `con-ruta` | el `.sha256` |
| `DL` | `curl`, `wget`, `ninguno` | qué downloader encuentra |
| `SIN_SHA256SUM` | `0`, `1` | el shell de Android no trae `sha256sum` |
| `PREFIX_0700` | `0`, `1` | `/data/adb` es 0700 de root ⇒ hay que escalar |
| `TUNEL` | `ok`, `caido`, `sin-url` | el túnel (log y `tunnel.url`) |
| `HEALTH` | `ok`, `ko` | el health-check local y la sonda del túnel |
| `ESPACIO_MB` | número | MB libres que ve `df` (`0` = disco lleno) |
| `DAEMONS`, `SUPERVISORS`, `WATCHDOGS`, `CLOUDFLARED` | números | cuántos procesos ficticios hay |
| `SIN_MDOCTOR` | `0`, `1` | el bundle no trae el núcleo forense |

## 3. Funciones

| Función | Qué hace |
|---|---|
| `preparar_lab` | (re)crea el laboratorio con los knobs actuales |
| `correr_instalador [args...]` | ejecuta el instalador; deja la salida en `$OUT` y el código en `$CODIGO` |
| `correr_instalador_como_usuario [args...]` | lo mismo pero como usuario NORMAL (`mdlab`), para el prefijo 0700 |
| `comprobar "desc" cmd...` | OK si el comando sale 0 |
| `verificar "desc" cmd...` | OK si el comando sale **distinto** de 0 |
| `contiene "desc" fichero regex` | OK si el regex aparece en el fichero |
| `no_contiene "desc" fichero regex` | OK si el regex **no** aparece |
| `log_mensaje "desc" regex` | OK si el regex aparece en `$LOG` (invocaciones de los fakes) |
| `no_log_mensaje "desc" regex` | OK si **no** aparece en `$LOG` |
| `salida_mensaje "desc" regex` | OK si el regex aparece en `$OUT` |
| `no_salida_mensaje "desc" regex` | OK si **no** aparece en `$OUT` |
| `json_valido "desc" fichero` | OK si el fichero es JSON válido |
| `espera_linea regex fichero [seg]` | espera a que el fichero contenga el regex (fakes en segundo plano) |
| `fijar_pids nombre pid...` | define procesos ficticios (los cuenta el diagnóstico) |
| `sin_pids nombre` | los mata |
| `ok "msg"` / `mal "msg"` / `omitir "msg"` | aserto manual (úsalo poco: `mal` cuenta como fallo) |

## 4. Reglas (importantes)

1. **Un escenario, una idea.** Cada `caso_*` prueba UNA cosa y dice qué es.
2. **Nada de red real.** El PATH del instalador es `$SB:$TBR`; los fakes están
   ahí. Si añades una herramienta nueva al `PATH`, añádela a la caja (`$TBR`),
   nunca al PATH del sistema.
3. **Nada de procesos reales.** `pgrep`, `pkill`, `ps` y el `grep` que busca en
   `/proc/*/cmdline` son falsos: los PIDs los fija el escenario con
   `fijar_pids`.
4. **Nada de root real en el filesystem del host.** Todo dentro de `$L`
   (que está en `/tmp`).
5. **Si el entorno no permite el escenario, OMÍTELO** con `omitir "motivo"` y
   `return`. Nunca lo marques como OK.
6. **Un fallo de aserto es un bug real del instalador**: no lo rebajes a "OK"
   ni lo comentes. Repórtalo (ver abajo).
7. Los escenarios deben ser **rápidos**: si el instalador tarda (lo hace cuando
   espera al túnel), usa los knobs (`TUNEL=sin-url`, `HEALTH=ko`) o
   `--timeout=30` para acortarlo.

## 5. Plantilla

```bash
# caso_a_01_nombre_del_escenario() {
#   Qué prueba: <una frase>
#   Por qué importa: <por qué>
#   MEDIDO/NO MEDIDO: <fecha y medida>
caso_a_01_nombre_del_escenario() {
  NET=down
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "aborta sin red" [ "$CODIGO" != "0" ]
  salida_mensaje "lo explica" "no se pudo resolver el release"
}
```

## 6. Si encuentras un bug real del instalador

NO tapes el aserto. Deja el escenario en rojo y ponlo al principio de tu
fichero:

```bash
# BUGS REALES DETECTADOS POR ESTOS ESCENARIOS (los arregla el root):
#   1) <descripción> — fichero:línea del instalador
```

Y dilo en tu informe final.

# Forense online: diagnosticar y recuperar el móvil como root

> Todo esto vive en **un binario de C de 43 KB** (`mddoctor`) que se instala
> desde el propio release. No depende de Rust, ni de la librería de
> MegaDaemon, ni de `curl`, `wget`, `pgrep` o `pkill` — que en Android
> **no existen**. Funciona, por tanto, **cuando el daemon está muerto**, que
> es justo cuando hace falta.

## 1. El "online": pegar esto en el móvil

Desde Termux en el **Samsung Galaxy S21 Ultra** (con root):

```sh
sh -c 'curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh | sh'
```

Eso instala el stack completo (daemon, supervisor, túnel, web) **como root**
(`su -mm -c`), instala además el núcleo forense en `$PREFIX/mddoctor` y lo
**ejecuta** para demostrar que funciona.

Si el daemon ya está instalado y lo que quieres es **solo diagnosticar**:

```sh
sh -c 'curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh | sh -s -- --doctor'
```

Y si lo que quieres es **recuperar el control** (relanzar túnel y/o daemon y
saber la URL nueva):

```sh
sh -c 'curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh | sh -s -- --reconectar'
```

## 2. Los modos del binario

| Modo | Qué hace | ¿Toca el sistema? |
|---|---|---|
| `doctor` | Recoge los hechos y dice qué está mal | no |
| `salud` | URL real del túnel, DNS y TCP del túnel, `/health` por sockets, procesos, SHA-256 y ELF del binario en servicio | no |
| `entorno` | Los permisos **reales** del proceso: capacidades decodificadas, seccomp, no_new_privs, montajes, límites, SELinux, linker de Android | no |
| `repair` | Crea/arregla el directorio de estado, permisos de ejecución, mata procesos zombis, relanza el supervisor | **sí** (`--apply`) |
| `reconectar` | Relanza túnel y/o daemon, **espera a `/health`** y **dice la URL nueva** | **sí** (`--apply`) |
| `recover` | Limpieza de ficheros viejos del estado | **sí** (`--apply`) |
| `todo` | `doctor` + `salud` en un único informe (el que se integra) | no |
| `selftest` | Prueba sus propias sondas | no |

Códigos de salida: `0` = todo OK · `1` = hay avisos · `2` = hay críticos ·
`3` = uso incorrecto. Formato JSON estable (`schema: megadaemon.doctor/1`),
con `--json`, e incluye `memoria_kb` (su propio consumo).

## 3. Desde MegaDaemon (sin ssh ni ADB)

```sh
megadaemon-core doctor salud        # el CLI reenvía cualquier subcomando
megadaemon-core doctor reconectar --apply
```

Y por la tool MCP `doctor` (acciones: `doctor`, `salud`, `entorno`, `repair`,
`reconectar`, `scan`, `recover`, `todo`, `selftest`). Las acciones que tocan
el sistema exigen `aplicar: true` **y** `confirmar: true`.

## 4. Por qué es fiable (todo MEDIDO)

- **SHA-256 propio**, contrastado contra `sha256sum` y contra el vector
  conocido de la RFC: permite comprobar que el binario *en servicio* es la
  build nueva (y no una vieja o manipulada).
- **Cabecera ELF leída a mano**: dice si el binario en servicio es válido y
  de qué arquitectura.
- **El bundle comprueba la arquitectura** del `mddoctor` antes de embeberlo:
  si no coincide con la del daemon, **no lo instala** (medido: el bundle
  llevaba el binario del host, x86-64, que el móvil no puede ejecutar).
- **31 pruebas** en el banco (`tools/test-mddoctor.sh`), que corre en el CI:
  SHA-256 contra `sha256sum`, HTTP contra un servidor real, el parser del
  túnel contra un log con trampas (`api.trycloudflare.com` no es un túnel),
  capacidades, cabecera ELF, rutas seguras (no se ejecuta lo que le metan) y
  el modo `reconectar`.
- **Tope de tamaño en el CI**: si `mddoctor` pasa de 100 KB, el job falla.

## 5. Límites honestos

- El chequeo del **túnel público** es DNS + TCP en el 443, **no** HTTPS: sin
  TLS no se puede validar el certificado desde aquí. El origen local
  (`127.0.0.1:9090`) sí se consulta por HTTP/1.0 completo.
- El binario se cede sin W^X (`-z noseparate-code`, 8 KB de relleno): es una
  herramienta local que ya corre como root; el daemon principal **sí**
  conserva W^X.
- `reconectar` relanza scripts del `service.d`: si Magisk no ha instalado el
  supervisor, lo dice en vez de fingir que ha funcionado.

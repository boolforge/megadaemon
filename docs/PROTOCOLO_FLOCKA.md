# PROTOCOLO DE LA FLOTA (2026-11-22)

Este documento existe porque **hoy los agentes se pisaban**. Las tres cosas
que fallaron, con lo medido:

1. **Cada agente compilaba.** El sandbox tiene **~3 GB de RAM** y el crate
   tiene ~700 líneas propias + ~200 dependencias. El **enlazado** de la lib o
   de los tests se come >3 GB y muere por **SIGKILL (OOM)**. Con tres agentes
   compilando a la vez, los tres mueren y además se pelean por el mismo
   `CARGO_TARGET_DIR`.
2. **Cada agente integraba por su cuenta.** Varias ramas tocaban
   `llm_gateway.rs`, `mcp_ext.rs`, `lib.rs` y `cockpit_server.rs` ⇒ merges
   conflictivos y ficheros a medias.
3. **Un agente murió con un fichero truncado** (`magisk_module.rs`, cadena sin
   cerrar) ⇒ **`main` dejó de compilar** hasta que el root lo reparó.

## Las reglas (obligatorias)

### 0. Un solo actor compila, testea y pushea a `main`: **el root**

| Acción | Agente | Root |
|---|---|---|
| Escribir código | ✅ en **su** rama | ✅ en su worktree |
| `cargo check` / `build` / `test` | ❌ **nunca** | ✅ (con `tools/build.sh`) |
| `git push` a `main` | ❌ **nunca** | ✅ |
| Merge a `main` | ❌ | ✅ |
| Publicar el SHA en el bus | ✅ | ✅ |

Motivo: compilar cuesta 10–15 min y mata el proceso. El tiempo de un agente
vale más **escribiendo código** que compilando.

### 1. Una rama, un agente, un fichero

* Rama propia: `agent/<tema>`. **Nunca** `main`.
* **Antes** de tocar un fichero que otro agente está tocando: lo dices por el
  bus (`docs/COORDINATION.md`) y esperas.
* `git add <ruta exacta>`. **Nunca** `git add -A` ni `git add .`.

### 2. Commits pequeños y frecuentes

* **Un commit = un bloque pequeño** (una función, un fichero). Si te quedas sin
  cuota a mitad, lo commiteado se conserva.
* Cada commit con un mensaje que diga **qué se medido**, no qué se quiso hacer.

### 3. Entrega explícita al root

Cuando termines un bloque:

```bash
tools/agente-entrega.sh "qué hice / qué toqué / qué bloquea" ruta/fichero.rs ...
```

Eso escribe en `docs/COORDINATION.md` tu rama, tu SHA y los ficheros. El root
lo ve y lo integra.

### 4. Nada de «lo compila otro»

Si necesitas comprobar que tu código **compila**, **pídeselo al root** por el
bus con el fichero exacto. No lo compiles tú: enlazar el crate en este
sandbox es un OOM garantizado.

### 5. Si el build falla, se arregla en código, no silenciando

Un test que se salta, se explica en `tools/ci-known-red.txt` con el motivo.
Nunca se desactiva un test sin más.

### 6. Nada de secretos

Ni en el repo, ni en el bus, ni en los prompts. Ficheros 0600 fuera del repo
(`/workspace/md-secrets/`).

## El patrón correcto (resumen)

```
AGENTE:  escribe código -> commit pequeño -> tools/agente-entrega.sh -> sigue
ROOT:    lee el bus -> merge --no-ff -> tools/build.sh -> push a main
```

## Lo que gana el agente

El root compila **una vez** para todos y el resultado está en `main` el mismo
día. Antes: tres builds en paralelo, tres OOM y ningún push.

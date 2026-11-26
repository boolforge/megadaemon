# Doctrina de continuidad — Antigravity en MegaDaemon

> **Este documento es la memoria operativa del proyecto.** No es un informe: es
> el contrato que cualquier agente (humano o MiniMax, en cualquier sandbox,
> cualquier sesión futura) debe leer antes de tocar nada, para que el trabajo
> **no se detenga** y para que la tarea nunca se pierda.

Identificador: `ANTIGRAVITY-CONTINUITY-v1` · Actualizado: **2026-11-26** ·
Rama canónica de este trabajo: `agent/automacion-software` ·
Espacio canónico: `/workspace/antigravity/` · Mandato maestro:
[`docs/MEGADAEMON_MASTER_PROMPT.txt`](MEGADAEMON_MASTER_PROMPT.txt).

---

## 1. La regla que manda sobre todas las demás

**No se para.** Un agente que termine su turno, se quede sin recursos, o
encuentre un bloqueo **no cierra el trabajo**: lo registra, deja el estado
exactamente donde estaba, y lo devuelve al bus. El trabajo de Antigravity es
un **programa largo, multi-sesión, multi-agente**. Terminar un turno no
termina el programa.

Consecuencia operativa: **toda tarea se cierra con un checkpoint escrito**, no
con un mensaje. Si el próximo agente (o el mismo dentro de tres sesiones)
lee sólo el repositorio, tiene que poder continuar sin contexto de chat.

---

## 2. Las seis reglas no negociables (derivadas del mandato maestro §2)

| # | Regla | Cómo se comprueba |
|---|---|---|
| 1 | Evidencia antes que conclusión | Cada afirmación lleva artefacto + hash, o la palabra `NOT_VERIFIED` |
| 2 | Un solo espacio canónico | `/workspace/antigravity/` (artefactos) + `agent/automacion-software` (repo) |
| 3 | Nada sin procedencia | Todo artefacto con origen, fecha y sha256 |
| 4 | Nada de afirmaciones sin medir | Estados: `VERIFIED`/`STRONGLY_SUPPORTED`/`INFERRED`/`NOT_VERIFIED`/`CONTRADICTED` |
| 5 | Nada de código de terceros sin licencia revisada | Clonados = referencia técnica, no se redistribuyen |
| 6 | Nada de secretos en el repo ni en las capturas | Nunca se imprime un valor de token |

---

## 3. Sistema de backlog: dónde vive y cómo se actualiza

El backlog **no** es un fichero suelto. Son **tres** mecanismos que ya existen y
que este trabajo debe alimentar:

| mecanismo | fichero | quién escribe | qué guarda |
|---|---|---|---|
| **Bus de la flota** | `state/mesh_board.json` + `state/mesh_journal.ndjson` | cualquier agente | entradas de agente y mensajes con evidencia |
| **Registro de tareas** | `docs/ANTIGRAVITY_TASK_REGISTRY.tsv` (este trabajo) | el dueño de cada workstream | una fila por tarea, con vocabulario controlado |
| **Backlog de campo** | `docs/BACKLOG.md` (repo) y `docs/ANTIGRAVITY_BACKLOG.md` (rama `agent/antigravity-backlog`) | agente que tenga la tarea | siguiente paso accionable |

Reglas del backlog:

1. **Una tarea, una fila, un dueño.** Nada de listas paralelas sin fusionar.
2. **Vocabulario controlado de estados** (mandato §5.7):
   `PROPOSED · READY · ASSIGNED · IN_PROGRESS · BLOCKED · WAITING_FOR_PERMISSION ·
   WAITING_FOR_INPUT · NEEDS_REVIEW · REWORK_REQUIRED · REJECTED · ACCEPTED ·
   INTEGRATED · VERIFIED · PARTIALLY_VERIFIED · SUPERSEDED · ARCHIVED`.
3. **Ningún bloqueo se archiva sin escribir el bloqueo exacto** y qué
   progreso se conserva (§2.20 del mandato).
4. **Una tarea que espera un agente no se borra**: pasa a `BLOCKED` o
   `WAITING_FOR_PERMISSION` con la permisos/insumo exacto que falta.
5. **Reanudación**: cualquier agente puede tomar una fila `BLOCKED` si su
   bloqueo es de recursos y ya se cumplieron las precondiciones; para eso el
   bloqueo debe llevar la precondición escrita (ver §4).

---

## 4. Agentes durmientes: protocolo exacto

> *«jamás dejes de hacer lo que este agente está haciendo ahora y en el futuro
> en cualquier proyecto y tarea, en todo subagente MiniMax, y lo que sea en
> todo MiniMax; que lo recuerde siempre: backlog y agentes durmientes hasta
> que tengan recursos»* — instrucción permanente del usuario.

**Definición**: un agente durmiente es un agente **registrado, con tarea
asignada y con estado conocido**, que **no puede ejecutar** porque su
precondición de recursos no está satisfecha. No es un agente muerto ni un
agente olvidado: está **dormido**, y se despierta solo.

**Protocolo de registro** (`state/agents/<id>.json`):

```json
{
  "id": "<id-estable>",
  "name": "<nombre legible>",
  "role": "worker",
  "status": "DORMANT",
  "task_id": "<id del registro de tareas>",
  "workstream": "W01..W21",
  "repo": "https://github.com/boolforge/megadaemon",
  "branch": "<rama>",
  "worktree": "<ruta si aplica>",
  "precondition": {
    "resource": "RAM",
    "need": ">= 800 MB libres",
    "measured": "MemFree 466 MB, swap 3341/4095 MB, load 22",
    "measured_at": "2026-11-22T01:16:00Z"
  },
  "resume_when": "MemFree >= 800 MB y load <= 8 en el movil",
  "last_checkpoint": "<SHA del commit o ruta del artefacto>",
  "last_seen": "<ISO8601>",
  "blocked_by": ["<id de tarea o condicion>"]
}
```

**Protocolo de despertar**:

1. Un agente supervisor (o cualquier agente con acceso al móvil) mide la
   precondición **antes** de nada.
2. Si se cumple: `status: "ACTIVE"`, se lanza, y **el primer acto** es leer
   `last_checkpoint` y el registro de tareas; no empezar de cero.
3. Si no se cumple: se deja `DORMANT`, y se **anota la medición** (número, no
   adjectives) en `mesh_journal.ndjson`.
4. **Nunca** se relanza un agente dormido «para ver si funciona»: se mide la
   precondición.

**Estado real de los durmientes hoy** (medido, 2026-11-22, móvil S21 Ultra):

| agente | tarea | precondición | medición |
|---|---|---|---|
| `antigravity-intel` (móvil) | `task_553653` `minimax_bootstrap` | RAM libre en el móvil | `MemFree 466 MB`, `swap 3341/4095 MB`, `load 22` |
| `antigravity-intel` (móvil) | `task_442838` `Snatcher-MegaDaemon-SuperLoop` | RAM libre en el móvil | idem; el mecanismo (runner+supervisor) **está instalado y probado**, pero **no** se dejó el bucle de 300 s corriendo |

Esto no es un capricho: el ciclo #38 ya consumía 211 s por vuelta y el móvil
iba al límite. El mecanismo está listo (`deploy/antigravity-task-runner.sh`,
`deploy/antigravity-task-supervisor.sh`, `deploy/antigravity-tasks.py`); sólo
falta la precondición de RAM.

---

## 5. Cómo continúa cualquier agente, en cualquier sesión, sin contexto

**Regla de los 30 segundos**: antes de hacer nada, un agente lee, en este orden:

1. `docs/MEGADAEMON_MASTER_PROMPT.txt` — el mandato completo.
2. `docs/ANTIGRAVITY_DOCTRINA_Y_CONTINUIDAD.md` — este documento.
3. `docs/ANTIGRAVITY_TASK_REGISTRY.tsv` — qué está hecho, qué está pendiente.
4. `docs/ANTIGRAVITY_PRODUCTS.md` — qué es Antigravity y qué componentes tiene.
5. `docs/ANTIGRAVITY_HARNESS.md` — cómo se ejecuta el LS real en local.
6. `docs/corpus/README.md` + `docs/corpus/FACTS.md` — qué se ha dicho ya.
7. `state/mesh_journal.ndjson` (las últimas entradas) — qué ha hecho la flota.

Y después: `git log --oneline -10` en `/workspace/megadaemon` y
`git status --short` para saber qué hay a medias.

**Protocolo de cierre de turno** (obligatorio):

1. Escribir el estado en el registro de tareas (una fila actualizada).
2. Publicar en el bus (`mesh_journal.ndjson` + `mesh_board.json`) un mensaje
   con: task_id, estado, qué se hizo, rutas exactas, evidencia, pruebas,
   bloqueos y siguiente acción.
3. Commit con **rutas exactas** (`git commit -- <rutas>`), nunca `git add -A`.
4. Push a la rama propia.
5. Backup verificable a Drive (`tools/backup.py`), con el hash en la salida.

Si el turno se corta por falta de tiempo o de recursos, **se ejecutan los
cinco pasos igual**. Un turno sin checkpoint se pierde.

---

## 6. Estado real a 2026-11-26 (verificable, no opinions)

### Cerrado y verificado

| entregable | evidencia | estado |
|---|---|---|
| Autenticación del LS: N1–N4, itab y escrituras | `docs/LS_AUTH_ANALYSIS.md` | VERIFIED |
| Parche region-bypass (ARM64) descifrado | 15 B / 4 tramos, `0x070e5b50`+`0x070e5b57` | VERIFIED + corroborado por la comunidad (`RAX+8=1` en x86-64) |
| Genealogía Jetski→Antigravity | `docs/ANTIGRAVITY_LINEAGE.md` | VERIFIED |
| Catálogo del ecosistema | `docs/ECOSYSTEM_PROJECTS.md` | 16+ proyectos, 4 297 artefactos |
| Barrido de canales de descarga | `versions/ecosystem_channels.tsv` | 20 canales, 9 manifiestos CLI con sha512 |
| Corpus de conversaciones | `docs/corpus/` | 91 sesiones · 7 358 mensajes · índice en 9 temas |
| Arnés de captura local del LS | `tools/agy_harness/` | el LS ARM64 real arranca y se ejecuta |
| Esquema real de la base del LS | `docs/ANTIGRAVITY_HARNESS.md` | `conversation_summaries`, 21 columnas |
| Backups verificados en Drive | `tools/backup.py` | `ANTIGRAVITY_FULL_2026-11-26` — 203 ficheros, 213,7 MB, **196 por hash + 6 por tamaño, 0 fallos** |

### Bloqueos reales (con precondición)

| bloqueo | precondición exacta | quién lo resuelve |
|---|---|---|
| Capturar la petición gRPC real | **token OAuth real de Google** (el LS rechaza los falsos) | el usuario |
| 65 sesiones del corpus truncadas por el principio | la API expone un cursor por `msg_id` que `session_messages` no expone | plataforma / main agent |
| Darling: ejecutar un Mach-O | `CLONE_NEWIPC` + `CAP_SYS_ADMIN` (el sandbox no las da) | infraestructura |
| `rust-core` no compila | 9 errores en `src/apk_deobfuscator.rs` introducidos por otro agente | dueño de ese fichero |

### Lo que NO se ha hecho (y hay que decir)

- **No** hay deofuscación completa de ningún binario: hay catálogos de
  funciones (192 082 en el LS macOS arm64) y análisis parciales.
- **No** hay paridad con el LS original: no hay reimplementación en Rust de su
  funcionalidad.
- **No** hay captura de tráfico real contra los endpoints de Google: por
  diseño (cero carga sobre Google) y por el bloqueo del token.
- **No** se ha ejecutado ningún binario macOS: Darling arranca pero no ejecuta
  Mach-O en este sandbox.

---

## 7. Registro en el bus (esto es lo que un agente publica)

Formato de mensaje en `state/mesh_journal.ndjson` (una línea JSON) y entrada
en `state/mesh_board.json` → `agents`:

```json
{
  "id": "<agente>-<fecha>-<slug>",
  "node": "<agente>",
  "kind": "antigravity_state",
  "ts": "<ISO8601>",
  "headline": "<una línea factual>",
  "task_id": "<id del registro>",
  "status": "<vocabulario controlado>",
  "artifacts": ["<ruta canónica>"],
  "evidence": "<VERIFIED|NOT_VERIFIED> + artefacto",
  "blockers": ["<bloqueo exacto>"],
  "next": "<siguiente acción concreta>"
}
```

---

## 8. Lo que este agente deja escrito para el siguiente

1. El mandato maestro completo está en el repo: `docs/MEGADAEMON_MASTER_PROMPT.txt`.
2. El catálogo de qué es Antigravity (CLI, IDE, desktop, SDK, ACP, plugins,
   Android, Language Server) está en `docs/ANTIGRAVITY_PRODUCTS.md`.
3. El registro de tareas con el vocabulario controlado está en
   `docs/ANTIGRAVITY_TASK_REGISTRY.tsv`.
4. El arnés para ejecutar el LS real y capturar tráfico está en
   `tools/agy_harness/` y documentado en `docs/ANTIGRAVITY_HARNESS.md`.
5. Todo lo dicho por todos los agentes está indexado en `docs/corpus/`.
6. Todo está respaldado en Drive con manifiesto sha256 verificado.
7. La doctrina de continuidad, el backlog y el protocolo de agente durmiente
   están **en este fichero**, que es el punto de reentrada de cualquier
   agente futuro.

---

## 9. Dónde está publicado todo (enlaces)

| qué | dónde |
|---|---|
| Repositorio (rama del trabajo) | `agent/automacion-software` en `https://github.com/boolforge/megadaemon` |
| Backup verificado en Drive | `ANTIGRAVITY_FULL_2026-11-26` — <https://drive.google.com/drive/folders/114GocicoCJBmGun081sfhRPSjDa2EhYW> (203 ficheros, 213 692 259 B) |
| Backup anterior (consolidado) | `ANTIGRAVITY_CONSOLIDATED_2026-11-26` — <https://drive.google.com/drive/folders/1Y22_b9lZF9YwSrANV7uOKAQpNB0Fv_rq> |
| Espacio canónico de artefactos | `/workspace/antigravity/` |
| Bus de la flota | `state/mesh_board.json` + `state/mesh_journal.ndjson` |
| Manifiesto de integridad | `BACKUP_INDEX.md` dentro de la carpeta de Drive (sha256 por fichero) |

`tools/backup.py --sync` sube sólo lo que cambia y verifica: los documentos
se comprueban **por hash descargando el remoto**; los ficheros grandes
(> 4 MB) se comprueban **por tamaño + sha256 registrado en el manifiesto**,
porque la API de Drive corta las descargas largas (`IncompleteRead`).

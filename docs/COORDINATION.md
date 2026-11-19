# Tablón de coordinación — MegaDaemon

> Un único plan, un único tablón. Cada sesión escribe **su** fila y añade
> **un** mensaje al cremallero. Nadie edita la fila de otro.
>
> Reglas vigentes: `git add <ruta exacta>` (nunca `-A` ni `.`), nada de `--force`,
> nada de reescribir la historia de `main`, y los ficheros de otro módulo no se
> tocan aunque parezcan tuyos.

## Tabla de estado

| Sesión | Ámbito | Fichero(s) propio(s) | Estado | Commit en `main` | Nota |
|---|---|---|---|---|---|
| `458507570102555` (esta) | Capa harness/MCP/sincronización (v13.3+) | `rust-core/src/android.rs`, `rust-core/src/mcp_server.rs`, `rust-core/src/sync.rs`, `rust-core/src/sync_web.rs`, `rust-core/src/cockpit_server.rs`, `rust-core/tests/harness_policy.rs` | ✅ **HECHO** — política de escritura: H-01, H-02 y H-03 corregidos con suite rojo→verde. Además, 2 ideogramas CJK inyectados reparados en mi capa. | `ab535d6` · `72bca7c` · `7e38bdb` | Informe: `docs/AUDIT_HARNESS_V13_4.md`. Suite 118 → **129 tests**, 0 fallos. No toco `lib.rs`, `main.rs` ni `Cargo.toml`. |

## Bloqueos y choseas abiertas

| # | Qué | Impacto | Quién lo resuelve |
|---|---|---|---|
| B-1 | `docs/MASTER_PLAN.md` **no existe** en el repo: ni en `main`, ni en ninguna rama (sólo `main` existe), ni en ningún PR. Lo único parecido es `docs/LLM_SWITCH_PLAN.md`, que es un plan de modelo, no el plan estratégico del proyecto. | Sin plan compartido no hay orden de trabajo común. | agente principal |
| B-2 | `rust-core/src/repo_updates.rs` (2.000 líneas, commit `1db663d`) **no está declarado en `lib.rs`**: nunca se ha compilado, y no puede compilarse porque llama a `crate::apk_inventory::inventory(...)` y **`apk_inventory.rs` no existe en el repositorio**. | Cablear `repo_updates` sin `apk_inventory` rompe el build. La secuencia segura es: primero `apk_inventory.rs` + su `mod` en `lib.rs`, después `repo_updates.rs`. | dueño de `repo_updates` / agente principal |
| B-3 | Esta sesión no puede cerrar B-2: `lib.rs` y `Cargo.toml` están fuera de su ámbito por instrucción expresa, y `repo_updates.rs` / `apk_inventory.rs` son de otro módulo. | El landmine sigue abierto. | agente principal |
| B-4 | `apk_inventory.rs` no está en el repo pese a que el agente principal lo cita como fichero ya reescrito por otra sesión. ¿Está en un árbol local sin pushear, o se perdió? | Sin el fichero no se puede cablear `repo_updates`. | agente principal |

| B-5 | **Ideogramas CJK/Cirílico inyectados dentro de frases en español.** Hay 61 caracteres afectados en 9 ficheros de `main`. En mi capa he reparado `rust-core/src/android.rs` ("de una命令" → "de una orden") y `rust-core/src/sync_web.rs` ("sesión，往往" → "sesión suelen"). Quedan fuera de mi ámbito: `docs/AUDIT_V13_1.md`, `docs/LLM_SWITCH_PLAN.md`, `docs/NPU_REAL.md`, `docs/SEERE_V11.md`, `docs/npu-research.md`, `reference/python/gbnf_check.py`, `reference/python/seere_v11.py`, `rust-core/src/providers.rs`. | Texto corrupto en la documentación y en el código de otras sesiones. **Ojo: `rust-core/src/crypto_scan.rs` usa ideogramas a propósito** (decodifica un juego de caracteres CJK): no hay que tocarlo. | dueño de cada fichero |

## Hallazgos abiertos (documentados, sin tocar)

| # | Severidad | Dónde | Qué pasa |
|---|---|---|---|
| R-1 | Media | `mcp_server.rs`, `run_stdio_mcp_server` | Lectura de stdin sin tope de longitud de línea: una línea enorme puede reventar la memoria. El transporte HTTP sí tiene tope (8 MB), el stdio no. |
| R-2 | Media | `android.rs`, `is_destructive` | La lista de comandos destructivos es corta: un `cp` a `/system` vía `root_exec` no salta la confirmación. |
| R-3 | Baja | `android.rs`, `fs_write` | Sin tope de tamaño del contenido escrito. |

Detalle y reproducción en `docs/AUDIT_HARNESS_V13_4.md`.

## Cremallero

- `458507570102555` — Política del harness: `fs_write` saltaba la confirmación en `/system` con `chmod` (precedencia `||`/`&&`) y su error `needs_confirm` era un contrato muerto; corregido en `ab535d6` con suite rojo→verde en `rust-core/tests/harness_policy.rs` (118 → 129 tests).
- `458507570102555` — Barrido de ideogramas inyectados: reparados `android.rs` y `sync_web.rs` (`7e38bdb`); los de otras sesiones quedan en B-5.
- 2026-11-19 20:5x · agente mensajería · **feat(v14) extractores de mensajería y citas médicas**: `src/messaging.rs`
  (3.3 k líneas, único fichero tocado) con `inventory/telegram/sms/whatsapp/medical/ingest_all/alert_candidates`;
  normaliza al esquema de `sync.rs` (`service/id/title/updated_at/messages[{role,content,ts}]`), roles `user`=saliente
  y `peer`=entrante; Telegram: copia `msgstore.db` (+`-wal`/`-shm`) a snapshot, detecta el esquema real por
  `sqlite_master`+`PRAGMA`, tu `user_id` por tabla `info`/`users` o por patrón, texto de blob TL por heurística;
  SMS/MMS por content provider agrupados por hilo (`address`+`thread_id`) y citas por keywords ES sin acentos;
  WhatsApp honesto: `wa.db` (contactos/chats) + texto visible de `dumpsys notification --noredact` +
  `encrypted_messages_unreadable:true` con el motivo (clave AES en AndroidKeyStore, requiere Frida) — sin inventar
  texto; Salud Responde (`citamovil.saludresponde`): busca tablas `cita/appointment/evento` y si el esquema no
  encaja devuelve `schema_detected` + `readable:false`; 33 tests verdes sin dispositivo (BD SQLite temporal),
  0 warnings, lógica pura separada de la E/S. Además: tarea `device.cockpit_health` encolada y asignada en el
  móvil (`state/task_queue.json`) e instancia Samsung marcada `primary` (`state/instance.json`).
- 2026-11-19 21:0x · agente MegaDaemon-Cockpit · **fix(v14) cockpit diagnosticable**: nuevo
  `rust-core/src/cockpit_doctor.rs` (MÓDULO PURO: sondas `Probe` inyectables + `DoctorEnv` por
  parámetro; NO toco `cockpit_server.rs`/`main.rs`/`lib.rs` → el principal cablea `pub mod
  cockpit_doctor;` y el subcomando `doctor` con `run_doctor_cli`). Introduce el estado que faltaba:
  `not_operational` = el proceso `megadaemon-core cockpit` está VIVO pero /health no responde en
  ningún puerto (el fallo invisible que daba ERR_CONNECTION_REFUSED en el Samsung: el supervisor ve el
  proceso vivo, no lo relanza, y no dice nada). También `down` (ni proceso ni puerto) y `ok`
  (/health 200 exacto). Diagnostica binario+versión, proceso, puertos, plantilla (disco o EMBEBIDA),
  token de sesión, API key remota, antigüedad del último sync y el último error del log → JSON +
  resumen en español. `rust-core/tests/cockpit_doctor_smoke.rs`: 7 tests verdes con sonda falsa.
  `deploy/install-phone.sh` REESCRITO (era de repo_updates): verificación SHA-256 del binario en el
  móvil (aborta y restaura si no coincide), copia de plantilla a <prefix>/web/ y al dir privado de la
  app, instalación del supervisor, espera ACTIVA de /health (timeout 45 s, reintentos) en vez de los
  6 s que daban falso negativo, exige 200 EXACTO (el `toybox wget && echo 200` daba falso OK con
  404), y ROLLBACK al .bak con el motivo en el log si no arranca. Idempotente.
- 2026-11-19 21:15 · agente perfil · **feat(v13.4) analizador de perfil del usuario**: `rust-core/src/profile_analysis.rs` (modulo mio, unico) implementado: `analyze()` (corpus/intereses/actividad/temas/pendientes/fortalezas/bloqueos/preguntas recurrentes/stats), `briefing(markdown)` (informe para psicologo en Markdown o JSON por secciones), `topic_drift(window_days)` (ventana reciente vs anterior) y `daily_digest(days)` (actividad por dia). Nucleo puro `analyze_conversations(&[Value])` (testeable sin state dir) + lectura de `<state_dir>/sync/*.ndjson`. Stopwords ES/EN + UI + lexico generico; marcadores de pendiente/logro/bloqueo con deteccion de negacion; cada evidencia cita `conversation_id` + `ts`. 15 tests sinteticos verdes. Target dir exclusivo: `/workspace/md-t-profile`. Commiteado y pusheado: `72962de`.
- 2026-11-19 20:5x · dueño del cockpit · **plan v14 + tablero distribuido**: `docs/MASTER_PLAN.md`
  (backlog consolidado P0-P3 con dueño y olas, DoD por tarea) + `src/coordination.rs` (roles
  primary/worker/observer en `instance.json`, tablero SQLite con reclamo atómico y lease,
  merge convergente peer-a-peer, plan de migración observers→workers→primary) + binario
  `md-coord`. Diagnóstico real del móvil: el daemon está vivo (pid 2580) pero el binario
  desplejado es anterior al fix ⇒ la URL pública devuelve **401** también en `/` y `/health`
  (gate de API key) y hace bind sólo a 127.0.0.1 ⇒ `ERR_CONNECTION_REFUSED` desde el PC.
  Fix en el árbol: `/` y sondas exentas del gate, `/health` con diagnóstico (bind, puertos,
  plantilla, token, URLs) y bind 0.0.0.0 con alias 9090/3000/3030/8090. Falta desplegar el
  binario ARM64 nuevo en el Samsung.

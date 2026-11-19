# MegaDaemon v14 — Plan estratégico consolidado

> **Este es el plan.** Nace de la reunión virtual del 2026-11-19: se han
> minado todas las conversaciones del enjambre (principal + 11 sesiones
> agentes), el backlog del usuario, los documentos de auditoría y el estado
> real del repo. Cada línea dice **qué**, **quién** (un fichero = un
> agente), **estado real** y **cómo se comprueba**.
>
> Regla de oro: el plan no es un documento, es **el tablero de
> `docs/COORDINATION.md` + el tablero SQLite de cada instancia**
> (`<state_dir>/board.sqlite`). Si una tarea no está en el tablero, no
> existe; si está en el tablón, está en curso.

---

## 0. Punto de partida (medido, no supuesto)

| Dato | Valor real |
|---|---|
| Núcleo | `rust-core/` ~33.800 líneas Rust, 50 módulos, sin shell-out a Python en los hot paths |
| Web | HTTP puro en Rust, plantilla embebida (`include_str!`) → el binario es autosuficiente |
| Build | `cargo build --release` verde (7.037.168 B) con `CARGO_TARGET_DIR` **aislado por agente** |
| Enjambre | 12 sesiones/agentes; **1 fichero = 1 agente** |
| Problema #1 | La cola de trabajo era el *chat*: 20+ mensajes del usuario sin leer, sin estado, sin auditoría |
| Problema #2 | El daemon **no estaba corriendo** en el Samsung → `ERR_CONNECTION_REFUSED` en 3000/3030/8090/9090 (no era la UI) |
| Problema #3 | Dos colisiones de ficheros y dos builds rotos por EdenVd mismodules en vuelo |
| Instancia maestra | Samsung Galaxy S21 Ultra (Exynos 2100) = **`primary`**. PC Linux = `worker` |

---

## 1. Las 5 causas del "no avanza" (y su contramedida)

| # | Causa raíz | Contramedida (implementada) |
|---|---|---|
| 1 | El chat hacía de cola: sin *claims*, sin estados, sin auditoría, contexto quemado | Tablero SQLite por instancia (`src/coordination.rs`): `add → claim (lease) → report (3 líneas)`, con eventos auditados |
| 2 | Nadie era dueño de nada | Mapa de propiedad en `docs/COORDINATION.md` + regla "un fichero = un agente"; tocar otro ⇒ acuerdo previo por `communicate` |
| 3 | Dependencias reimplementadas (un ZIP propio) | Política **reutiliza o mufla**: `zip 2.2` + `flate2` (ZIP/DEFLATE), `sha2` (hash), `rusqlite` (estado), `ureq` (HTTP), `serde_json` (datos). Nada de crypto propio si el crate está en el árbol |
| 4 | Un único `target/` con 15+ procesos `cargo` | `CARGO_TARGET_DIR=/workspace/md-t-<agente>` **obligatorio**; el compartido corrompe builds y da errores falsos |
| 5 | Faltaba el criterio de "fin" | Definition of Done por tarea (§7) y por ola (§4) |

---

## 2. Principios de operación (10 reglas, no negociables)

1. **El plan vive en MegaDaemon**, no en los chats: `docs/COORDINATION.md`
   (tablón) + `board.sqlite` (estado) + `docs/MASTER_PLAN.md` (este fichero).
2. **Un agente = un módulo Rust**. Nadie toca `lib.rs`, `main.rs`,
   `Cargo.toml` ni el módulo de otro sin acuerdo explícito en el tablón.
3. **`git add <ruta exacta>`**, un commit por ámbito, nunca `-A`, nunca
   `--force`, nunca el token en el código.
4. **Reutilizar, no reescribir.** Zip/flate2/sha2/rusqlite/ureq/serde_json.
5. **`CARGO_TARGET_DIR` aislado por agente.**
6. **Informe de 3 líneas**: qué hice / qué toca / qué bloquea. Nada de
   mensajes largos al principal (el usuario tiene una cola de 20).
7. **Una línea de estado al final del tablón** al commitear; no se editan
   filas ajenas.
8. **Doble auth**: `X-API-Key` (remoto) + `X-Seere-Auth-Token` (escritura).
   Toda acción irreversible exige `confirm: true`.
9. **Evidencia o nada**: cada afirmación va con test local, medición real
   en el dispositivo, o marcada `pendiente`/`bloqueado`. Nunca inventar.
10. **Todo en Rust** salvo excepción documentada (`assets/sync_extract.py`:
    los esquemas de las apps cambian cada versión).

---

## 3. Backlog consolidado priorizado

### P0 — Desbloqueo inmediato (hoy, sin excusas)

| # | Tarea | Fichero(s) | Dueño | Estado | Cómo se comprueba |
|---|---|---|---|---|---|
| P0.1 | **Web viva**: daemon corriendo en el Samsung + `/health` con diagnóstico (bind, puertos, plantilla, token, URLs) + alias 9090/3000/3030/8090 + bind `0.0.0.0` | `src/cockpit_server.rs` | dueño cockpit (esta sesión) | código listo; **falta arrancar el daemon en el móvil** | `curl -s http://<ip-movil>:9090/health` → 200 con `"web"` |
| P0.2 | **Build reproducible**: `CARGO_TARGET_DIR` por agente, una sola firma por función (`apk_inventory::inventory(limit)`) | — | todos | verde | `cargo build --release` con target propio |
| P0.3 | **Tablero de coordinación** (roles, claims, leases, auditoría, sync peer-a-peer, plan de migración) | `src/coordination.rs`, `src/bin/md-coord.rs` | esta sesión | implementado, en build | `md-coord snapshot` + tests del módulo |
| P0.4 | **Instancia maestra marcada**: el Samsung queda como `primary` en `<state_dir>/instance.json` | `src/coordination.rs` (+ `deploy/99-megadaemon.sh`) | esta sesión / repo_updates | implementado | `cat instance.json` → `"role":"primary"` |
| P0.5 | **Tablón único + este plan** | `docs/COORDINATION.md`, `docs/MASTER_PLAN.md` | principal / esta sesión | hecho | lectura |

### P1 — Lo que el usuario pidió explícitamente (días 1-5)

| # | Tarea | Fichero(s) | Dueño | Estado |
|---|---|---|---|---|
| P1.1 | **Firma electrónica sin Java** (rFirma-style): RSA/ECDSA propios, CMS/CAdES, PDF visible, PKCS#11 (DNIe en caliente), servidor `afirma://` con token | `src/signing.rs` | subagente "Firma Rust" | en curso |
| P1.2 | **Decompilador DEX + desofuscación** | `src/dex_decompile.rs` | subagente "Decompilador DEX" | en curso |
| P1.3 | **Delegación parasitaria**: enrutar a cuentas gratuitas con precios reales, reserving las de pago | `src/delegate.rs` | subagente "Delegación parasitaria" | en curso |
| P1.4 | **Ocultación de root + intel de apps** (BBVA, FNMT, McDonald's, etc.), sin falsear attestation | `src/root_hide.rs`, `src/app_intel.rs` | subagente "Intel de apps" | en curso |
| P1.5 | **Migración a Google Drive** con verificación por hash | `src/drive_migrate.rs` | — | pendiente |
| P1.6 | **Batería / Doze / módulo Magisk** (y si conviene correr como módulo del kernel) | `src/battery_advanced.rs`, `src/magisk_module.rs` | — | pendiente |
| P1.7 | **Backups completos + grabador de llamadas** (WhatsApp/Telegram/Discord/SMS/Nostr) | `src/backup_full.rs`, `src/call_recorder.rs` | — | pendiente |
| P1.8 | **Migrar las tareas de las cuentas (Kimi/MiniMax/Claude) al demonio** | `src/delegate.rs` + `board.sqlite` | — | parcial |

### P2 — Semana 2-3

| # | Tarea | Fichero(s) | Notas |
|---|---|---|---|
| P2.1 | Motor de navegador real (wry + DOM) para scraping con JS | `src/browser/` | reutiliza `service_identity` |
| P2.2 | Conector de banco (API abierta o extracción con perfil stealth) | `src/bank/` | requiere consentimiento explícito |
| P2.3 | Adjuntos binarios de las apps (WhatsApp media, imágenes de Claude) | `src/attachments.rs` | |
| P2.4 | Actualizaciones de Play Store | — | **limitado**: no hay API pública |
| P2.5 | Plan de migración a ROM nueva + parches Magisk/KernelSU | `docs/` + `deploy/` | |
| P2.6 | Rendimiento: profiling del cockpit,后的 de SQLite, caché | por módulo | "si le falta algo, se mejora" |

### P3 — Bloqueos honestos (del sistema, no del código)

1. **Cookies de Chrome**: cifradas con clave del AndroidKeyStore en el TEE —
   ni root ni ADB las exportan. Salidas: inyección en proceso (Frida) o
   exportación oficial.
2. **Play Integrity**: firma de hardware; suplantarla no es "ocultar root"
   y no se hace.
3. **WhatsApp**: almacén cifrado con clave del keystore del proceso.
4. **Play Store**: sin API de instalación; automatizarlo sería automatizar
   la cuenta del usuario.
5. **Historial de Gemini**: no se guarda en local; vive en el servidor.
6. **Configurador FNMT-RCM**: requiere Java 17 (no hay reimplementación
   abierta). El atajo: la app oficial de la FNMT en Android
   (`es.fnmtrcm.ceres.certificadoDigitalFNMT`) emite el certificado leyendo
   el DNIe por NFC; a partir del `.p12` el resto es Rust puro.

---

## 4. Olas de ejecución

### Ola 0 — "Que no haya otra complaint" (hoy)
- Salida: `/health` = 200 desde el PC contra el Samsung; daemon corriendo;
  tablero con una línea por agente; `cargo build --release` verde con
  target aislado; plan publicado.
- Dependencias: ninguna. Bloqueo conocido: acceso al dispositivo (adb /
  túnel MCP) para arrancar el daemon en el móvil.

### Ola 1 — "Que el sistema se sostenga solo" (días 1-5)
- Firma sin Java, DEX, delegación gratuita, ocultación root, Drive,
  batería/Magisk, backups.
- Salida: cada P1 con test + evidencia en el dispositivo + línea en el
  tablón.

### Ola 2 — "Que nada se pierda" (semana 2)
- Backups completos, migraciones de instancias, motor de navegador,
  migración de tareas de las cuentas de IA al demonio.

### Ola 3 — "Que el móvil sea una herramienta de trabajo" (semana 3)
- Conector de banco, adjuntos, updates, firma en trámites reales.

### Ola 4 — Pulido (continuo)
- Rendimiento, telemetría honesta, documentación al día, formación de
  agentes nuevos con el mismo protocolo.

---

## 5. El plan dentro de MegaDaemon (protocolo distribuido)

### 5.1 Identidad y roles

`<state_dir>/instance.json`:

```json
{ "id": "samsung-s21-ultra@<host>", "role": "primary", "device": "samsung-s21-ultra",
  "platform": "android-arm64", "version": "14.0.0", "protocol": 1, "capabilities": [] }
```

- **primary** — coordina y reparte (Samsung).
- **worker** — ejecuta (PC Linux, otros nodos).
- **observer** — sólo lee (auditorías, métricas).

### 5.2 Tablero de tareas (por instancia, SQLite)

`add → claim (lease atómico) → report (3 líneas) → done/failed`.

- El reclamo es un `UPDATE ... WHERE status='queued'`: **sólo un agente
  puede ganar**; el resto recibe `won=false`.
- El lease caduca ⇒ la tarea vuelve a `queued` (`reclaim_expired`).
- Todo queda en `events` (auditoría): quién reclamó, quién reportó, qué.
- `task_scheduler.rs` sigue siendo el planificador **dentro** de una
  instancia; este tablero es la capa **entre** instancias.

### 5.3 Sincronización entre instancias (baja latencia)

- `POST /sync` (HTTP/1.1, cabecera `X-Seere-Auth-Token`) con la
  instantánea completa; el peer fusiona y devuelve la suya ⇒ ida y vuelta
  única, **convergente**.
- Fusión **idempotente**: tareas por `id` (gana el `updated_at` mayor; un
  estado terminal *nunca* se degrada a `queued`), instancias por `last_seen`
  máximo, eventos deduplicados por `(ts, kind, actor, payload)`.
- Coste: un `SELECT`/`INSERT` por fila y **cero** recomputo. Los peers se
  sincronizan por *push* (worker → primary) en el latido.
- Comando: `md-coord sync --peer http://<ip>:9099 --token <t>`.

### 5.4 Migraciones simultáneas

`md-coord migrate --target 14.0.0` devuelve los pasos en orden:
**observers → workers → primary** (el coordinador se actualiza el último,
así nunca se queda caído) y exige acuse de recibo por instancia
(`wait_for_ack`). Una instancia con protocolo incompatible se marca y se
migra sola, nunca en paralelo.

### 5.5 Comandos

```text
md-coord register --state <dir> --device samsung-s21-ultra --platform android-arm64 [--primary]
md-coord heartbeat --state <dir>
md-coord add "firma rust sin java" --priority 1 --owner firma
md-coord claim <task_id> --owner <agente> --lease 900
md-coord report <task_id> --owner <agente> --ok "qué / qué toca / qué bloquea"
md-coord tasks [--status queued|claimed|done|failed]
md-coord instances
md-coord migrate --target 14.0.0
md-coord snapshot
```

### 5.6 Respuesta a la pregunta sobre Tauri/web

El usuario no quiere una app web disguised. **Coincidimos**: el núcleo de
firma será **Rust nativo sin JVM y sin web** (PKCS#11, CMS, PDF); la
interfaz del cockpit es sólo una *vista* y el protocolo `afirma://` se
sirve de forma nativa. En MegaDaemon, lo mismo: **el daemon es el
sistema**, la UI es un cliente, y el plan/coordinación viven en el daemon
(SQLite + HTTP), no en el navegador.

---

## 6. Mapa de propiedad (fichero → agente)

| Fichero | Agente | Estado |
|---|---|---|
| `src/coordination.rs`, `src/bin/md-coord.rs` | dueño cockpit (esta sesión) | nuevo, en build |
| `src/cockpit_server.rs`, `web/cockpit_template.html` | dueño cockpit (esta sesión) / principal | P0.1 |
| `src/apk_inventory.rs` | subagente APK | reescrito, en curso |
| `src/repo_updates.rs` | subagente repo_updates | verde |
| `src/signing.rs` | subagente "Firma Rust" | en curso |
| `src/dex_decompile.rs` | subagente "Decompilador DEX" | en curso |
| `src/delegate.rs` | subagente "Delegación parasitaria" | en curso |
| `src/root_hide.rs`, `src/app_intel.rs` | subagente "Intel de apps" | en curso |
| `src/mesh.rs` | subagente "Bus de coordinación" | ⚠️ solapa con `src/coordination.rs` (§8) |
| `src/profile_analysis.rs`, `src/messaging.rs` | subagentes correspondientes | en curso |
| `src/agent_core.rs`, `src/alerts.rs`, `src/task_scheduler.rs`, `src/git_ops.rs` | subagentes correspondientes | en curso |

> ⚠️ **Solapa a resolver (P0.6)**: hay dos buses de coordinación
> (`coordination.rs` y `mesh.rs`). Decisión propuesta: `coordination.rs`
> (SQLite + claims + leases + merge convergente) es el **estado**;
> `mesh.rs` queda como **transporte** (mallas, tunnels, descubrimiento) y
> consume la API del tablero. Se decide en la próxima reunión; mientras
> tanto, nadie reimplementa el otro.

---

## 7. Definition of Done (por tarea)

Una tarea está terminada cuando **todo** esto es cierto:

1. `cargo build --release` verde con `CARGO_TARGET_DIR` propio.
2. Test del módulo en verde (`cargo test --release`).
3. `git add <ruta exacta>` + un commit con `Co-Authored-By: Mavis` + push
   sin `--force` (si no es fast-forward ⇒ avisar, no machacar).
4. Una línea al final de `docs/COORDINATION.md` + tarea cerrada en el
   tablero (`report ... --ok`).
5. Evidencia real (salida del comando, `/health`, log en el dispositivo) o
   la marca explícita `pendiente`/`bloqueado`.
6. Informe de 3 líneas al principal.

## 8. Decisiones pendientes de la próxima reunión

1. **`coordination.rs` vs `mesh.rs`**: ¿fusión, o estado vs transporte?
2. **Puerto del cockpit**: ¿se queda el multi-alias (9090/3000/3030/8090)
   o se estandariza en 9090 y se adjusts `install-phone.sh`?
3. **Firma**: ¿somos cliente `afirma://` completo (protocolo 3 y 4) o
   primero firma de APK + PDF (que es lo que desbloquea el resto)?
4. **Rotación del token de GitHub**: está expuesto en el chat ⇒ rotar ya.
5. **Presupuesto de tokens**: las cuentas gratuitas son el recurso
   escaso; el enrutado debe medirse (coste real, no estimado a ojo).

---

*Documento vivo. Actualizado por el dueño del cockpit el 2026-11-19
(coordinación de la reunión virtual). Complementa a `docs/ROADMAP_V14.md`
(mapa petición→estado) y `docs/COORDINATION.md` (tablón operativo).*

# MASTER PLAN v16 — MegaDaemon como sistema computacional distribuido y agéntico

> Fuente: las diez directrices del usuario (2026-11-20). Una fase = un entregable
> medible. Un responsable por fichero. Todo lo que sigue está **verificado** o
> marcado `NO VERIFICADO`; no hay forecasts camuflados de hechos.

---

## 0. Estado real (medido, 2026-11-20 07:30)

| Magnitud | Valor | Cómo se midió |
|---|---|---|
| Repo | `main` = `812b969` (compila) | `cargo check --lib` EXIT=0 |
| Ramas | **1** (`main`) — 11 borradas | API de GitHub |
| PRs | **0 abiertos** — #6 y #7 cerrados con nota | API de GitHub |
| Móvil | S21 Ultra · Exynos 2100 · Android 14 | `system_overview` |
| Tools MCP | **51** | `tools/list` |
| Web | `/` 200 · `/v2` 200 · `/health` 200 | curl por el túnel |
| Disco móvil | **225 GB, 98 % usado, 5,7 GB libres** | `df` |
| Disco sandbox | 500 GB, 6 % usado | `df` (NFS, sin presión) |
| RAM móvil | 10.339/10.575 MB usados (98 %) | `free` |
| Carga móvil | load 17,65 · **128 % de IRQ** · 876 procesos | `uptime`, `ps` |
| Suite | 393 verdes / 3 rojos → `signing` verde (14/14) | `cargo test --lib` |

**El cuello de botella medido**: no es el protocolo ni el código. Es que **cada
petición MCP crea 2+ procesos** (`su -mm -c sh -c …`) y **17 agentes golpean el
maestro por el túnel** sobre un móvil con 876 procesos y 128 % de IRQ ⇒ `adbd`
tarda >45 s en contestar el CNXN y el túnel da 502. **Ninguna tool del enjambre
puede llamar al móvil en bucle cerrado mientras eso siga así.**

---

## F0 · Saneado y orden del repo — **EN CURSO**

Hecho en esta ronda:
- `git cherry` + `git diff` sobre **11 branches**: 9 ya integrados por parche,
  2 con 1 commit real cada uno (`repo_updates.rs` +336 líneas ya presente;
  CI con el gate de seguridad) ⇒ nada perdido.
- **PR #6 y #7 cerrados** con nota (su contenido está en `main`).
- **11 ramas borradas** ⇒ queda una sola rama de trabajo.
- **E0425 ×8 arreglado**: `origin/main` **no compilaba** (las ramas `stop` y
  default usaban `st` fuera de scope tras acortar el guard del mutex). El CI
  estaba rojo por eso.
- **Módulo `peers` integrado** (registro de instancias + `GET /peers`).
- **Buzón del peer redactado** (se escribía en claro ⇒ fuga al estado compartido).

Pendiente de esta fase:
| Tarea | Responsable | Criterio de aceptación |
|---|---|---|
| Código duplicado: `tools/` vs `rust-core/src/` (p.ej. parser DEX en Rust **y** `tools/dex_deobf.py`) | DEX + planificación | Un solo parser; el otro borra o se documenta como fallback |
| Módulos huérfanos: `call_recorder.rs`, `backup_full.rs`, `rom_migration.rs` sin `pub mod` en `lib.rs` | por dueño | Cada módulo está cableado **o** marcado `huerto` en `docs/` |
| Ficheros basura: `rust-core/md_v15_a02_PWNED`, `tests/zz_*_tmp.rs`, `tests/data/` | por dueño | `git ls-files` sin basura |
| `panic = "abort"` / `catch_unwind` por conexión | infraestructura | `grep -c catch_unwind src/` > 0 y el perfil **no** aborta |
| Docs: 29 ficheros y creciendo | principal | Un `README.md` índice + `docs/MASTER_PLAN.md` como referencia única |

---

## F1 · Backup absoluto y espacio en disco — **P0**

El móvil tiene **5,7 GB libres** y el objetivo es "backup absoluto" ⇒
**imposible en local**: 79 GB de `/data/media` + 84 GB de `/data/app`.
⇒ El backup tiene que ir **a Drive en streaming**, y el disco local sólo es
la caché de trabajo.

| Tarea | Responsable | Criterio de aceptación |
|---|---|---|
| Cliente de Drive nativo en Rust: carpeta + 100 % + **stream** (sin copia local) | backup (W3) | `backup_vault` sube un fichero de >1 GB sin escribirlo entero en el móvil; sha256 del MANIFEST verificado tras la descarga |
| Zona **privada** (TEE, keystore2, EFS) **nunca** sale del dispositivo | por dueño | El MANIFEST público no contiene ninguna entrada privada (`export_public` filtra) |
| `keystore2` = metadatos, no claves | ya cerrado | `not_recoverable()` documentado |
| Rotación del log de auditoría (32 MB → 2 MB) | **hecho** | `audit.ndjson` ≤ 2 MB |
| Purga de binarios old en el móvil (59 MB) | **hecho** | `/data/local/tmp/megadaemon` = 148 MB |
| Umbral SQLite→coordlog v16 | planificación (**hecho**: fsync 21,37 ms/commit, 74×) | Documentado en `CONCURRENCY_PLAN_V15.md` §10.8 |

**Criterio de aceptación global**: `df` del móvil **no baja** del 98 % tras una
semana de operación normal ⇒ todo lo que crece, se va a Drive.

---

## F2 · Swift Backup desofuscado + reimplementación de proyectos de backup

El paquete es **`org.swiftapps.swiftbackup`**, APK
`/data/app/~~pbwhtiYXFh4xDN_dToeH5A==/org.swiftapps.swiftbackup-rmFvjZ4wVP1zY_fpVoXM3A==/base.apk`,
**sha256 `f73db4caacdb273f0b68341969cf0b64d192acb7ee01133f58e28d11715d9099`**,
versión 16.0.2. Derivación **AEGIS-256** (+ Zstandard), formatos `.splits`/`.dat`/`.med`.

| Tarea | Responsable | Criterio de aceptación |
|---|---|---|
| Desofuscar Swift Backup: clase que deriva la clave + salt + iteraciones | DEX + nodo RE | Un test que reproduce el derivador contra un `.dat` real |
| **Argon2id (KDF) en Rust puro** — RFC 9106 | **signer (hecho)** | ✅ `rust-core/src/argon2.rs`: d/i/id verdes con los vectores oficiales + 36/36 contra el oráculo C; API `derive_key`; único KDF del repo (crates.io: prohibido) |
| **ZSTD sin crates** (FFI `dlopen` a `libzstd` + CLI) | **signer (hecho)** | ✅ `rust-core/src/zstd.rs`, fixture real congelado |
| **Lector del contenedor `SBA1`** (TAR+ZSTD) | **signer (hecho)** | ✅ `rust-core/src/swift_sba1.rs`: magic, cabecera **inspeccionada** (no congelada) y extracción del payload |
| **Catálogo de proyectos de backup** (libres y no libres) con lo que se reimplementa | DEX | Tabla: proyecto → formato → qué reimplementamos → licencia (reimplementar **ideas**, no código) |
| Reimplementar en Rust dentro de MegaDaemon | backup + DEX | `backup_vault` lee un backup de Swift Backup y lo restaura verificado |
| El formato OMC ya está resuelto | nodo RE (**hecho**) | `omc_decode/encode` en Rust, round-trip 126/126 CSC |

---

## F3 · LLM agéntico recursivo — **P0 (el usuario: "no sirve ni para chatbot tonto")**

Hoy `providers::execute_chat` es una llamada y ya. El objetivo: **MegaDaemon
como meta-agente** que decide, llama tools, evalúa el resultado y se corrige
(hasta un presupuesto), con **memoria de fallos** y **coste medido**.

| Tarea | Responsable | Criterio de aceptación |
|---|---|---|
| Bucle de decisión: *plan → tool → evalúa → corrige*, con presupuesto por hora | infraestructura | Un test con un proveedor simulado: 3 iteraciones, para al cambiar de plan |
| **Memoria de fallos**: libro `failures.ndjson` (acción, error, lo que funcionó) | infraestructura | El mismo error no se repite en el siguiente ciclo |
| Degradación honesta: si el modelo no responde ⇒ criterio fijo, **no inventar** | ya existe (`RuleBrain`) | Test: respuesta basura ⇒ `Action::Idle`, no una acción inventada |
| **El prompt NUNCA lleva la API key ni el token** | ya verificado ✓ | Test que lo comprueba (ya existe) |
| Coste real: tokens + latencia + precio con **fuente citada** | W6 | `NO VERIFICADO` si no hay fuente |
| Circuit breaker de 3 estados para que el enjambre **nunca** se quede sin voz | W6 | Si cae el proveedor barato, cae al siguiente y SEERE local es el último |

---

## F4 · Drive como almacenamiento unificado de gran capacidad

No sólo backup: **toda** tarea que hoy no cabe en el móvil va a Drive
(volcados raw de 32 GB ya subidos por el nodo RE).

| Tarea | Responsable | Criterio de aceptación |
|---|---|---|
| Listar/mover/subir por streaming con Drive, sin montar nada local | backup | Un volcado de 32 GB sube y baja con el sha256 verificado |
| La **malla** usa el mismo almacén: el handoff del RE ya va por Drive | nodo RE (**hecho**) | 7 ficheros, 35,72 GB, IDs + sha256 |
| Ficheros compartidos por zona: pública replicable / de instancia / restringida | coordinación | El manifest dice la zona y el replicador filtra |
| Reclamar espacio: qué se borra local tras subir | backup | `df` del móvil sube tras cada subida |

---

## F5 · Web UI que exponga TODO el potencial de la API

El objetivo: desde el navegador del móvil, **administrador de sistema completo**.

| Vista | Qué debe hacer | Responsable | Criterio |
|---|---|---|---|
| **Nodos** | quién está conectado, su rol, su carga, último latido | web | Los peers reales del bus, no una maqueta |
| **Tablero** | claim/release **bajo lock**, rol, lease | web | Dos claims simultáneos ⇒ uno gana |
| **Agente** | cola, voz, presupuesto, libro de fallos | web | Los números reales del `agent_core` |
| **Delegación** | cuentas, cuotas, coste, breaker | web | Coste real por modelo |
| **Alertas** | batería, thermal, disco, RAM, cola | web | Reglas reales de `alerts.rs` |
| **RE/Apps** | `apk_inventory`, `dex_decompile`, `app_intel`, sha256 | web | Un artefacto con su hash real |
| **Archivos compartidos** | vault por zona + MANIFEST | web | No se ve lo restringido |
| **Administración** | `api_keys` mint/revoke/list, reinicio, sync, **instalar APK**, **shell root** | web | Cada acción destructiva pide `confirm` |

**Hueco de seguridad que hay que cerrar (lo dijo el agente de web, con
razones)**: hoy la página local **incrusta el token root** ⇒ *cualquier app del
móvil* puede pedir `/` y leerlo ⇒ **control total del daemon**. Opciones:
socket unix 0600, o pedir la API key al usuario. **Decisión del usuario.**

---

## F6 · Rediseño de la web con los más altos estándares

- Un único sistema visual (tokens, tipografía, espaciado, estados).
- Las 8 vistas de `docs/UI_V2_SPEC.md` con **datos reales** y estados de
  carga/vacío/error honestos (nunca datos inventados).
- Acciones administrativas con `confirm` explícito y reversible.
- Rendimiento: nada de polling agresivo (el móvil va al 98 % de RAM).
- Accesibilidad: contraste, foco, tamaño mínimo táctil.

---

## F7 · CSC: método propio, noSorto → y no parche del sistema

**Medido y cerrado por el nodo RE + el agente de ROM:**
- El OMC es **rotación + XOR posicional + gzip** ⇒ codec propio **en Rust**
  (`omc_decode/encode`), round-trip **126/126** CSC.
- El **consumidor runtime** es `com.samsung.android.feature.SemCscFeature`
  en `framework.jar::classes5.dex` (`isXmlEncoded()`, `decode()`, `getBoolean()`).
- El parche es **una línea de XML**:
  `<CscFeature_VoiceCall_ConfigRecording>RecordingAllowed</CscFeature_VoiceCall_ConfigRecording>`
  (valor exacto, **no** `true`) + actualizar `contents.db` (`fileSize`).
- **19 CSC** traen la bandera; AMO **no**; THL/ILO/XXV sí sin disclaimer.
- Módulo Magisk systemless instalado, original intacto ⇒ revertir = `touch disable`.

**Pendiente**: un reinicio (el framework cachea la feature al boot) ⇒ lo
autoriza el usuario (le apaga el móvil).

---

## F8 · Repos y actualizaciones automatizadas

| Tarea | Responsable | Criterio |
|---|---|---|
| Release por commit + `latest.json` + verificación SHA-256 | CI (**hecho**) | El móvil se autoactualiza sin intervención |
| `repo_updates`: **despliegue verificado sin adb** | integrada ✓ | `repo_updates.rs` +336 líneas en `main` |
| Actualizar **apps** del móvil (Play no tiene API ⇒ vía `adb`/Pm) | apk_inventory | Instalar un APK desde la web con su hash |
| Firmar el release con el vault | firma | CMS verificable |

---

## F9 · Antigravity como meta-agente

Antigravity (`com.agy`) ya habla MCP con nosotros (45 tools verificadas). El
usuario quiere que **no haya que buscarlo a mano**: que **MegaDaemon lo
enrute y aporte sus tools**.

| Tarea | Responsable | Criterio |
|---|---|---|
| MegaDaemon sirve el **catálogo** de Antigravity (modelos, plugins, skills) como servicio | intel/apps | `service_bus` lo publica con salud medida |
| Antigravity **consume** las tools de MegaDaemon (ya: 45 tools) | intel/apps | Traza real en el log del daemon con UA ≠ curl |
| El **meta-agente**: MegaDaemon enruta la petición al nodo/servicio correcto | infraestructura | Una pregunta → una ruta → un nodo, con la cola del board |

---

## Reglas transversales (las mismas para todos)

1. **Un fichero = un agente.** Antes de escribir en uno ajeno, avisa por el bus.
2. `git add <ruta exacta>` · `git commit -- <rutas exactas>` · **nunca `-A`**.
3. Antes de commitear: `cargo check --lib --tests` en verde, con
   `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_RELEASE_LTO=false`,
   `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16` y **`CARGO_TARGET_DIR` en disco
   local** (`/run/agent-runtime/…`). En NFS: 8+ min y errores falsos; sin esas
   flags el link **mata al rustc por OOM** (SIGKILL) en esta caja de 3 GB.
4. **Ningún secreto** en el repo, el NDJSON, el journal ni el tablón. Ficheros 0600.
5. **Reporte de 3 líneas** al cerrar cada bloque (qué / qué toqué / qué bloquea),
   **sin que nadie pregunte**, y **preguntar la duda** en vez de quedarse parado.
6. Nada de polling agresivo: el móvil va al 98 % de RAM y de disco.
7. Nada inventado: si no está medido, `NO VERIFICADO`.

---

## Lo que depende del usuario (3 decisiones)

1. **Rotar el token de GitHub** (quemado en el historial del chat) y el
   **bearer OAuth de Google** (`sha16 1f86ae82a5ccc87f`, en claro en disco).
2. **Túnel con nombre** (URL estable) — sin eso, cada OOM rota la URL y el
   enjambre pierde el canal.
3. **Modelo del token local**: la página incrusta el token root ⇒ cualquier app
   del móvil tiene control total del daemon. ¿Socket unix 0600, API key al
   usuario, o se acepta?

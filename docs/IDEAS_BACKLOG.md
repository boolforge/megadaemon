# Backlog de ideas (todo lo pedido, con estado) — 2026-11-20

> "Apunta todas las ideas en el repo YA, todo organizado. Por si acaso."
> Este documento es el índice maestro. Cada bloque apunta al documento de detalle.
> Leyenda: ✅ hecho · 🟡 en curso · ⏳ pendiente · ⛔ bloqueado (por qué)

## A. Backup 100 % a Google Drive

| # | idea | estado | detalle |
|---|---|---|---|
| A1 | Backup **en stream** del 100 % de `/data` (223 GB) sin staging local | ⏳ | `BACKUP_100_STREAM.md` — el móvil tiene 1,9 GB libres; imposible sin stream |
| A2 | Imágenes de **todas las particiones** (`dd \| stream`: super, system, vendor, product, odm, prism, optics, boot, init_boot, vendor_boot, vbmeta*, efs, keyrefuge, metadata, nv_data) | ⏳ | ídem |
| A3 | **Keyring / contraseñas / cookies / bookmarks / passkeys / pestañas abiertas** | ⏳ | backup cifrado con el vault; **nunca en claro** |
| A4 | **Conversaciones** (ChatGPT/Claude/Gemini/DeepSeek/Kimi…) e historial en nube | 🟡 | `sync_conversations` + `sync_web_conversations` ya existen |
| A5 | **Dejar espacio libre** sin borrar nada importante: subir → verificar → *proponer* limpieza (cachés, `Download` 44 GB, APK de `/data/app` 88 GB) | ⏳ | el daemon **informa**, no borra |
| A6 | Verificación doble: **MD5 de Drive + SHA-256 propio** + manifiesto `SHA256SUMS.txt` | 🟡 | `sha2` ya está en el crate |
| A7 | Reanudable ante cortes (red móvil) | 🟡 | el cliente nativo ya usa `uploadType=resumable` (8 MiB) |
| A8 | Cliente de Drive **nativo Rust** (no rclone) | ✅ | ya existe en `backup_vault.rs`; **rclone = opcional** |
| A9 | ⛔ **Token de Google** | 🟡 | `rclone authorize` **esperando el código** en mi sandbox (URL de Google lista); el móvil ya tiene rclone ARM64 y `tools/nh_backup_phone.sh` listo para las 5 fases |

## B. Kali NetHunter + kernel

| # | idea | estado | detalle |
|---|---|---|---|
| B1 | **Kali NetHunter rootfs minimal** en Termux (120 MB, medido) | ⛔ | instalador **apagado por orden del usuario**; **bloqueo de DNS resuelto**: `tools/http_by_ip.py` descarga por IP (rclone ARM64 ya instalado en el móvil por esa vía) |
| B2 | NetHunter **Lite** (Magisk, sin kernel custom) | ⏳ | el móvil ya tiene Magisk 27.2-kitsune-4 |
| B3 | NetHunter **Full** (kernel propio) | ⏳ | **no existe** para el 2100; exigiría portar con el kernel-builder + desbloquear bootloader (KVB) |
| B4 | "Kernel más moderno": **GKI 5.15/6.1** con `init_boot` parcheado (Magisk + `vbmeta` parcheado) ⇒ **sin desbloquear, KVB intacto** | ⏳ | cuello de botella: `vendor_boot` con módulos compatibles |
| B5 | **Mainline 6.x/7.3** para el 2100 | ⛔ | no hay soporte oficial ⇒ 7.3 es **destino de reimplementación**, no imagen flasheable |
| B6 | USB gadget **HID/CDC-ECM/CDROM** (feature NetHunter) | ⏳ | requiere GKI (configfs/f_hid no está en el USB de Samsung) |
| B7 | **WiFi monitor + inyección** | 🟡 | con **adaptador externo** RTL8812AU/RTL88x2BU como `.ko` contra las cabeceras 5.4 ⇒ **sin kernel custom** |
| B8 | NFC: el 2100 usa PN553 en TEE (`secure_element_hal_service`); **no hay HCE de usuario** | ✅ | documentado; para NFC real hace falta Proxmark |
| B9 | Tomar de **UN1CA**: parcheo de `init_boot`/`vbmeta`, tri-flash, cambio de CSC, EFS | 🟡 | `rom_migration.rs` ya cubre parte |
| B10 | **CSC**: actual AMO; el cambio de CSC no toca el kernel | ✅ | `docs/CALL_RECORDING_PRECONF.md` |

## C. Magisk / hook / repos

| # | idea | estado | detalle |
|---|---|---|---|
| C1 | Evaluar los **21 módulos** instalados (viabilidad, pros/contras) | ✅ | `KALI_NETHUNTER_KERNEL_PLAN.md` §6 |
| C2 | Pila recomendada: `playintegrityfix` + `tricky_store` + **LSPosed (Zygisk)** + `zygisk-assistant` + **`mm`** + `mmrl_wpd` + `flagsecurepatcher` + módulos propios | 🟡 | instalar LSPosed + quitar `cromite-webview`/`hosts`/`luckypatcher_*` |
| C3 | **LSPosed** (no instalado) como estándar de hooks | ⏳ | sustituye a los módulos `luckypatcher_zygisk` |
| C4 | **MRepo → Mock Modules (`mm`)**: ya instalado, mejor y más ligero | ✅ | decisión: quedarse con `mm` + `mmrl_wpd` |
| C5 | `tricky_store` permite **firmar con clave propia** ⇒ evita KVB y root real para apps que validan | ✅ | ya instalado (v1.4.1) |
| C6 | Audit de apps que **detectan root** (`com.mc.notify` 19 señales, knoxpatch 12, magiskdetector) | ✅ | `docs/P5_ROOT_DETECTION.md` |
| C7 | `root_hide` era **simulación** → ahora mide de verdad | ✅ | `root_detectors` + `root_consumers` (en `fix/robustness-20261120`, falta rebuild en el maestro) |

## D. MegaDaemon como "mega store manager"

| # | idea | estado | detalle |
|---|---|---|---|
| D1 | Diseño `StoreDescriptor` + índices firmados + instaladores (Magisk/APK/payload/tool) | ✅ | `MEGADAEMON_CLEANUP_PLAN.md` §3 |
| D2 | **Ligero**: sin SDK de Google, sin rclone, sin binario externo; índices en JSON cacheado | ✅ | diseño |
| D3 | API estable `/api/stores*` para que la GUI no toque el disco | ⏳ | prerequisite de la GUI |
| D4 | Registry **declarativa** de tools (53 cableados a mano hoy) | ⏳ | reduce el coste de "hacer más cosas" |
| D5 | Limpiar ramas | ✅ | `publish/*` borradas (fusionadas) y `fix/megadaemon-stability` rebaseada (ahead 2 / behind 0) ⇒ **4 ramas → 3** |
| D6 | Absorber los **12 duplicados exactos** en `util_text.rs`/`util_fs.rs` | ⏳ | medido: `walk_files`×3, `hex`×3, `now_iso`×2, `format_iso8601`×2, `parse_apk_paths`×2 |
| D7 | Trocear `signing.rs` (5.887 líneas) y `webui.rs` (4.767) | ⏳ | top-10 = 45 % del crate |
| D8 | Unificar el parser DEX Rust con `tools/dex_deobf.py` | ⏳ | arregla de una vez el bug `encoded_value 0x17` |

## E. RE / Exynos 2100 (continuo)

| # | idea | estado | detalle |
|---|---|---|---|
| E1 | P1 NPU/EDEN verificado en el dispositivo (9 ioctls VS4L, `/dev/vertex10` 0644, NPU en producción, LLM en NPU imposible) | ✅ | `docs/P1_NPU_EDEN.md` |
| E2 | P2 TEE / P3 boot chain / P4 vendor (134 ioctls) / P5 root | ✅ | `docs/P2..P5` |
| E3 | **420 binarios decompilados** a C + 3 oráculos (unicorn, Z3, angr) | ✅ | `output/mass_decomp/` (fuera del repo, regla legal) |
| E4 | **Matriz de portabilidad 5.4 → 7.3** por driver | ✅ | `output/driver_port_matrix.md` |
| E5 | Reimplementar los drivers con **ABI estable** (ioctls) sobre APIs modernas | ⏳ | repo `port/exynos2100` |
| E6 | ADB/TCP: el `adbd` del ROM **descarta el CNXN** (64 combinaciones) ⇒ hace falta **pairing+TLS** (wireless debugging) | 🟡 | `docs/ADB_HANDSHAKE_DIAG.md` |
| E7 | **DNS roto en el móvil** (el VPN ata el DNS por uid; root sí resuelve, las apps no) | 🟡 | `docs/DNS_DIAG_20261120.md` |

## F. things que dependen del usuario

| # | qué hace falta | para qué |
|---|---|---|
| F1 | `client_secret` de la app de Google **o** autorizar `rclone config` en el navegador (pegar el código) | A1-A9 (backup) |
| F2 | En la app VPN: **DNS a través de la VPN** o DNS manual (1.1.1.1) | B1, descargas, E6 |
| F3 | Decidir si el kernel pasa a **GKI 5.15/6.1** (implica testear vendor modules) | B4 |
| F4 | Decidir qué se limpia en el móvil **después** del backup | A5 |
| F5 | Confirmar si se instala **LSPosed** y se quitan `cromite-webview`/`hosts`/`luckypatcher_*` | C2 |

## G. Antigravity — backlog del cliente en el móvil (2026-11-22)

> Recuperado del S21 Ultra por root. Detalle y evidencia: `docs/ANTIGRAVITY_BACKLOG.md`.

| # | idea | estado | detalle |
|---|---|---|---|
| G1 | **Backlog real del micro-daemon de Antigravity** (2 tareas en `daemon/tasks.json`, no 1) | ✅ | `task_442838` "Snatcher-MegaDaemon-SuperLoop" (interval 300 s) + `task_553653` "minimax_bootstrap" (delay, **nunca ejecutada**) |
| G2 | **Reanudar la tarea recurrente**: el worker del ciclo #38 arrancó el 19-Nov 10:21:33 y el micro-daemon **murió sin dejar rastro** (0 errores en las 860 líneas del log, sin crash dump, PID reciclado a otra app) | ⛔ | el planificador vive **dentro** del proceso `com.agy`: sin supervisor ni autostart no vuelve solo. Quedó **2 d 14 h** sin ejecutar tras 4 reinicios |
| G3 | **Arranque con lock + timeout + log con fecha + estado legible** (para que el enjambre lea el ciclo) | ✅ | `deploy/antigravity-task-runner.sh`, `deploy/antigravity-task-supervisor.sh`, `deploy/antigravity-tasks.py` — probados en el móvil: lock OK, timeout `rc=124`, concurrencia OK, catch-up OK, `last_status.json` OK |
| G4 | **Recuperar lo perdido**: 14.494 funciones, 923 descompiladas, 36 builds OK / 1 KO, 37 ciclos | ✅ | `megadaemon_state.json` (el estado del motor está a salvo) |
| G5 | **Recuperar las 27 h 25 min** de ejecución perdida (18-Nov 06:51 → 19-Nov 10:16) | ⛔ | irrecuperable: no quedó registro. La causa del hueco **NO VERIFICADA** |
| G6 | **Salida de los ciclos completa** (el daemon recorta `last_output` a ~1 KB ⇒ el enjambre nunca leyó el resultado) | ✅ | el runner guarda el log íntegro con fecha |
| G7 | **Cerrar el `verified: false`** de las 6 plataformas (ps1 14.494 func, saturn 0, pce 0, msx2 256 paletas, pc88 7 ports, segacd 53.110) | ⏳ | el motor declara mucho y **no ha verificado nada** |
| G8 | **Rotar el bearer de Google en claro** de `agy_standalone/probe.log` | ⛔ | requisito previo al catch-up de `task_553653` (`bootstrap_core.sh` escribe ahí) |
| G9 | **Bajar la carga del móvil** antes de reactivar el bucle de 300 s | ⏳ | medido: `MemFree` 466 MB, swap 3.341/4.095 MB, load 22 ⇒ el ciclo es un trabajo pesado |

## H. El agente DENTRO de MegaDaemon — API + interfaz (2026-11-23)

> Puesto por el jefe el 2026-11-23 («no usas el sistema de MegaDaemon … te
> integras dentro de mi software para poder usarte con la API … con un GUI o
> interfaz web … podrías fusionarte con una copia del GUI de Antigravity»).
> Detalle, riesgos y puertas de aceptación: `docs/BACKLOG_AGENTE_EN_MEGADAEMON.md`.
> **Esta lista NO sustituye a la tarea principal** (auditoría de chat/LLM:
> `docs/AUDIT_LLM_V16.md`).

| # | idea | estado | detalle |
|---|---|---|---|
| H1 | Poder usar al agente con la **API de MegaDaemon** | ✅ la API ya está | `POST /v1/chat/completions` cableado (`rust-core/src/cockpit_server.rs:1378`) + dialecto Gemini `/v1internal:*` (`:1416`) + `/api/chat` (`:2292`) |
| H2 | Una **interfaz** (web o GUI) para hablar con el agente | ⏳ | `rust-core/src/webui.rs` no tiene página de chat con el agente: hoy hay que usar `curl` |
| H3 | **Fusionarse con una copia del GUI de Antigravity** | ⏳ | el cliente está en `core/*.py` (**Python**) y el móvil NO tiene `python3` ni `node` (medido: `docs/SOFTWARE_INSTALLED.md`) ⇒ hay que portarlo a Rust y hacerlo hablar con `/v1/chat/completions` |
| H4 | Que el agente trabaje **por** el daemon (tools MCP propias) | ⏳ | board/bus/backup/automation como herramientas del daemon, no scripts sueltos |
| H5 | Que la interfaz explique **por qué** no hubo respuesta | ✅/⏳ | el veto de presupuesto ya está en el camino del chat; falta que la UI muestre el `reason` del `provider_trace` |
| H6 | **Gasto de pago visible** en la traza del proveedor | ✅ | la traza dice ahora `p => OK (tier paid)` (DEFECTO-18 de `docs/AUDIT_LLM_V16.md`) |
| H7 | **Fusión del GUI de Antigravity con el cockpit v2** — una sola página web con el marco de la IDE de Antigravity **y** los paneles de la API v2 | 🟡 | `web/cockpit_antigravity.html` (shell *Jetski Web* de `/workspace/agy-intel/assets_web_index.html` + `/api/v2/*` + chat por `POST /api/chat` + marco de la IDE por iframe). JS validado con `node --check`; **sin compilar** (política del sandbox) |
| H8 | **El agente, dentro del bus**: que el `general` sea un nodo al que se le pueda asignar trabajo por la API | 🟡 | `tools/registrar-agente-general.py` (idempotente, **dry-run por defecto**) da de alta `agent-general@…` en `state/board.sqlite` (`instances`) y sus tareas; `state/agents/agent-general.json` es el registro por fichero. La API v2 ya expone `/api/v2/nodes/assign`, `/api/v2/nodes/message` y `/api/v2/tasks/claim\|release\|report` (`rust-core/src/webui.rs:74-75`, `:4120-4128`) |
| H9 | **Que el móvil sirva la GUI fusionada** (hoy `web/cockpit_antigravity.html` está en el repo, no en el daemon) | ⏳ | falta el cableado en `cockpit_server.rs` (3 líneas, el mismo patrón que `docs/API_V2.md` §2 documenta para `/v2`) |

## I. Resiliencia, autoconexión y P2P (2026-11-26) — encargo del jefe

> «Haz que el sistema sea más resiliente a desconexiones prolongadas y que se
> autoconecte (investiga proyectos tipo mosh y otros muchos, para elaborar un
> protocolo y reutilizar, pensando en robustez y resiliencia extremas, que se
> pueda integrar opcionalmente con WireGuard)»: un **P2P dentro de MegaDaemon**,
> flexible y potente, fácil de implementar reutilizando código existente, para
> reencontrarse, pasar por proxies y conexiones inestables, y migrar de
> conexión — **sin fundir batería**.
>
> Diseño detallado: `docs/RESILIENCIA_P2P.md`.

| # | idea | estado | detalle |
|---|---|---|---|
| I1 | **Nivel 1 — identidad estable + endpoint reubicable**: el nodo tiene una identidad criptográfica fija (Ed25519, `ring` ya está en las deps) y una lista de endpoints (relé duckdns de nombre fijo, quick tunnel, puertos de descubrimiento). Quien quiera alcanzarle **pregunta "dónde estás" al bus** y recibe el endpoint actual | 🟡 | `rust-core/src/channel.rs` ya publica `quick`/`duckdns`/`ip_publica`; falta el esquema de identidad + el dedo del bus |
| I2b | **Ejecutor de órdenes del bus** (el que hace que el control funcione SIN conexión directa): el móvil sincroniza el tablón, selecciona las cartas `tipo=orden` que van para él, las **ejecuta con el registro de tools del daemon** (`android::dispatch` ⇒ el MISMO camino que la API y el MCP, no dos implementaciones) y responde por el bus | 🟡 | `rust-core/src/bus_ejecutor.rs` (nuevo): idempotencia por `idem_key` (una orden repetida NO se re-ejecuta), fail-closed ante firma inválida o sin clave pública, `confirm: true` exigido para lo irreversible, presupuesto de órdenes por ciclo y respeto del modo ahorro de batería. Tool MCP `bus_ordenes` + CLI `megadaemon-core bus` |
| I2 | **Nivel 2 — bus de control PULL** (el que rompe la dependencia de la conexión directa): el nodo **saca** órdenes del bus (repo por Contents API + Drive + relé) y las ejecuta, en vez de que le empufen. Permite **controlar el móvil y sus apps (incluso en primer plano) sin métodos directos de comunicación** | 🟡 | ⚠️ **NO es un bus nuevo**: `rust-core/src/resonance.rs` (659 líneas) **YA ES** un bus P2P por Drive — cartas firmadas Ed25519, `id = sha256(canonico)` (merge idempotente), CRDT LWW con `epoch`+`seq`, journal NDJSON + snapshot, en la carpeta privada `tablon-resiliencia`. El Nivel 2 debe **EXTENDERLO** (tipos de carta + semántica de órdenes), no duplicarlo. Los **leases NO se inventan**: `board.rs` ya los tiene (SQLite+WAL, `owner`, `lease_expires_at`, `reclaim_expired`); el bus sólo transporta órdenes. Detalle: `docs/RESILIENCIA_P2P.md` |
| I3 | **Nivel 3 — reconexión extremo a extremo** tipo mosh: identificador de sesión + nonce + clave rotada, handshake tolerante a pérdida, y **re-emigración** cuando cambia el trans underpinnings (quick tunnel → relé, WiFi → datos móviles) | ⏳ | reutiliza el doble auth del daemon (`X-API-Key` + `X-Seere-Auth-Token`) y `channel.rs` |
| I4 | **Cotas de batería**: en "silencioso" (sólo bus pull cada 15–30 min) el gasto no debe pasar de un X% diario. `JobScheduler`/`AlarmManager` con ventana inexact, `WorkManager` con `setRequiresCharging`, wakelocks mínimos | 🟡 | ya existe `battery_advanced::modo_ahorro_activo()` (batería <20 % y sin cargador) y la telemetría **duplica** el periodo en ese modo |
| I5 | **Integración opcional con WireGuard**: "modo WG" del daemon (subir config, peer, allowed_ips, DNS) reutilizando `rustls`/`ring`/`ureq` ya presentes | ⏳ | opcional: no debe bloquear el modo por defecto |
| I6 | **Unificar entornos equivalentes**: antigravity + language server + antigravity CLI + go + jetski ⇒ un único runtime, un único bus, un único modelo de credenciales | ⏡ | `rust-core/src/antigravity_env.rs`, `antigravity_clone.rs` (ya unifican el runtime), `agyshim.rs` |
| I7 | **No fundir batería con la telemetría**: el push a Drive cada 15 min se dobla en modo ahorro | ✅ | `rust-core/src/telemetria.rs::publicar_periodico` |
| I8 | ⚠️ **Relé canónico incoherente (medido)**: `channel.rs:72` usa `mdtmfnc.duckdns.org` pero el `state/channel.json` del repo dice `mdtmfnc-s21.duckdns.org` ⇒ dos canónicos distintos. Además el relé **no cachea** (a diferencia del `raw` de GitHub), así que es la única fuente "en vivo" fiable | ⛔ bloqueado | decidir cuál es el bueno **antes** de la Fase 1; `deploy/relay-pc.sh` y `deploy/instalador-megadaemon.sh` usan `mdtmfnc` (sin sufijo) |

## J. Control de gasto de datos (Jazztel) (2026-11-26) — encargo del jefe

> «Integra totalmente control de gasto de MB/GB de Jazztel dentro de MegaDaemon…
> desofuscación extrema y descompilación total de la app de Jazztel, para
> reimplementar e integrar sus funciones y avisos de factura; y deofuscar y
> descompilar lo que se encarga de contar el gasto de datos; y busca cómo
> diferenciar entre datos por WiFi y datos por conexión de datos».
>
> Diseño y plan de RE: `docs/RE_GASTO_DATOS_JAZZTEL.md`.

| # | idea | estado | detalle |
|---|---|---|---|
| J1 | **Desglose WiFi vs datos móviles** en la telemetría del daemon | ✅ | `telemetria.rs`: `/proc/net/dev` clasificado por interfaz (`wlan*`→wifi, `rmnet*`/`ccmni*`/`ppp*`→móvil) y desglosado en `red.por_transporte` |
| J2 | **Contabilidad real del sistema** (`NetworkStatsManager`, `TrafficStats`, `dumpsys netstats`, `xt_qtaguid`) y su diferencia con lo que **factura** el operador | 🟡 | `docs/RE_GASTO_DATOS_JAZZTEL.md` §A (A.1–A.10): 3 fuentes independientes, y la lista de dónde **difieren** (tethering, roaming, APN de respaldo, Data Saver, corte de ciclo de facturación). Pendiente de medir en el móvil |
| J3 | **Desofuscación de la app `com.orange.bluu.jazztel`** (identificadores, strings, flujo de control) | ✅ | **HECHO**: APK descargado (44.050.916 B, `md5 de06e522…` verificado) y analizado byte a byte: 36.248 clases, 323.861 métodos. La app NO está empaquetada y NO cifra strings (los 200 "descifradores" son cripto de un SDK 3DS2). Artefactos (83 MB) **fuera del repo** en `/workspace/apk_work/jazztel/`. Detalle: `docs/RE_GASTO_DATOS_JAZZTEL.md` §B |
| J4 | **Reimplantar en MegaDaemon**: `gasto_datos::estado()`, `avisos()`, `desglose()` (WiFi vs móvil) | 🟡 | **diseño completo** en `docs/RE_GASTO_DATOS_JAZZTEL.md` §C.1–C.3. Con la RE hecha se sabe el contrato real de la API (rutas, hosts, métodos y claves JSON exactas), que el modelo de datos ya trae costes en euros (`BigDecimal`) y que hay endpoints de recargos y de ciclos de facturación ⇒ el estimador de factura tiene de dónde sacarlo. Falta escribir el módulo |
| J5 | **Tool remota `gasto_datos`** (acciones `estado|movil|wifi|avisos|desglose|guardar`) | ✅ | `rust-core/src/gasto_datos.rs` (nuevo) + tool MCP `gasto_datos` + CLI `megadaemon-core gasto <accion>`. Avisa de umbral, plan agotado, renovación y **discrepancia** entre lo que ve el sistema y lo que factura el operador (con las causas posibles: tethering, roaming, APN de respaldo, Data Saver, corte de ciclo). El estado del operador se guarda en **0600** |
| J6 | **Credenciales de Jazztel**: fuera del repo (el repo es PÚBLICO), sólo en ficheros 0600 | ⏡ | regla dura del encargo |
| J7 | ⚠️ **PII en las facturas**: el **NIF del titular está en el nombre** de los 9 PDF de `Facturas jazztel` (Drive). El repo es PÚBLICO ⇒ ningún fichero del repo puede contener ese nombre ni el NIF | ⛔ regla dura | los PDF sólo se leen desde la carpeta **privada** de Drive y se **enmascaran** (`<NIF>_J….pdf`) en cualquier informe; lo mismo con el resto de datos de la factura (importe, IBAN) |

## K. Telemetría a Drive + autodetección de "listo" (2026-11-26) — encargo del jefe

> «Suba muchísima más telemetría automáticamente a Google Drive y que
> autodetecte cuando esté listo y suba los datos de conexión a Google Drive, y
> tú ya te conectes y hagas lo que tengas que hacer».

| # | idea | estado | detalle |
|---|---|---|---|
| K1 | Módulo `telemetria`: recolección (batería, memoria, carga, uptime, red por transporte, instrumentación viva, diagnóstico) | ✅ | `rust-core/src/telemetria.rs` (nuevo) |
| K2 | Subida **nativa** a Drive (sin `curl`: el móvil no lo tiene) + estado local en disco como fuente de verdad | ✅ | `drive_vfs::write_local_file` (subida resumible, `drive_native`) |
| K3 | **Auto-detección de "listo"** (binario presente + URL de túnel + canal legible) y **sonda** del túnel con código HTTP | ✅ | `telemetria::estado_listo()`, `telemetria::sonda_tunel()` |
| K4 | **Datos de conexión** publicados a Drive: URL viva, relé, puerto, build y **sha16** de las claves (nunca las claves) | ✅ | `telemetria::datos_conexion()`, `publicar_conexion()` |
| K5 | **Subida de artefactos a Drive** (binarios e instrumentación) sin pasar por GitHub | ✅ | `telemetria::subir_artefacto()` + tool `telemetria artefacto` |
| K6 | **One-liner de instalación y ejecución**: una línea, idempotente, con resolución del release en 3 capas, verificación **por valor**, instalación atómica con rollback, health-check real y auto-cura | ✅ | `deploy/megadaemon-oneliner.sh` + `deploy/mdo.sh` (lanzador de una línea). **Verificado en el sandbox**: descarga del asset real (12.881.744 B) y **sha256 por valor VERIFICADO** (el `.sha256` del CI trae la ruta `dist/…` ⇒ la comparación por valor es obligatoria, no opcional); el bundle contiene las rutas que el script busca (`bin/megadaemon-core`, `deploy/*.sh`); el binario extraído es **ELF aarch64**; `sh -n` y `dash -n` OK |
| K7 | Publicación del canal en el **repo** desde el one-liner (`state/channel.json`) | ✅ | el one-liner descarga `channel-publish-arm64` y lo llama con `--repo-publish --token-file` (el token nunca va en la línea de comandos) |

## L. Verificación del one-liner (2026-11-26) — lo que está medido

> Un subagente evaluador revisó los dos scripts y encontró **11 bugs
> bloqueantes** (el watchdog sin entorno, el estado en el sitio equivocado,
> el health-check que nunca podía pasar, el script que decía "instalado"
> sin instalar y dejaba el móvil con el hostname cambiando cada 60 s…).
> Todos corregidos y verificados con una simulación propia.

| # | idea | estado | detalle |
|---|---|---|---|
| L1 | **Simulación del instalador** sin móvil: rutas reescritas a un temporal y binario/curl/cloudflared falsos con la misma interfaz | ✅ | `tools/test-oneliner.sh` — **22 checks, 0 fallos** |
| L2 | El watchdog se lanza con su entorno y **NO** está en `service.d` | ✅ | verificado por el arnés (si estuviera, Magisk lo ejecutaría sin entorno y relanzaría cloudflared en bucle) |
| L3 | El daemon se lanza con `MEGADAEMON_STATE_DIR`/`PREFIX`/`BIN` y `< /dev/null` | ✅ | verificado por el arnés |
| L4 | El health-check usa la URL real + `telemetria sonda` (no "esperar 10 s") | ✅ | verificado por el arnés |
| L5 | **Idempotencia real**: la 2ª ejecución no toca nada y la URL no cambia | ✅ | verificado por el arnés (antes mataba cloudflared en cada ejecución) |
| L6 | Un bundle **corrupto** aborta con código ≠ 0 y el binario vivo sigue ahí | ✅ | verificado por el arnés (antes decía "instalado" sin instalar) |
| L7 | Un **sha256 que no cuadra** aborta y lo dice claro | ✅ | verificado por el arnés (antes sólo avisaba e instalaba igual) |
| L8 | El `hash` nativo del daemon (el móvil NO tiene `sha256sum`) | ✅ | `megadaemon-core hash <fichero>` |
| L9 | Prueba real contra el release publicado | ✅ | descarga del bundle real (12.881.744 B) + **sha256 verificado por valor** (el `.sha256` del CI lleva la ruta `dist/…` ⇒ la comparación por valor es obligatoria) |
| L10 | Prueba real en el móvil | ⛔ | `NO VERIFICADO`: el móvil está inalcanzable. Comandos exactos en `docs/TELEMETRIA_DRIVE.md` y en el propio script (`--diagnostico`) |

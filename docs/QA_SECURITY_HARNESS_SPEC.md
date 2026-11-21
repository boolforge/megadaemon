# Especificación del harness de QA y de seguridad (`qa_engine` / `sec_audit` / `ipc_audit`)

> Documento: `docs/QA_SECURITY_HARNESS_SPEC.md`
> Referencia: *Mobile QA Automation and Security Assessment Technical Manual* (600 líneas, 6 partes).
> Base de código: `boolforge/megadaemon` @ `c757542` (rama `origin/main`).  El encargo se escribió
> sobre `c7c0e2c`; mientras se elaboraba este documento `origin/main` avanzó (canal LAN,
> `c757542`) y **sólo se han re-verificado los ficheros citados que cambiaron** — el resto de las
> referencias siguen siendo válidas porque esos módulos no se tocaron.
> Versión del documento: 1.0 · Idioma del documento: español, identificadores de código en inglés.

---

## 0. Cómo leer este documento

Este documento **traduce el manual de referencia a la arquitectura de MegaDaemon**. No es un
resumen del manual (el manual ya existe) ni una lista de tareas: es la especificación de *qué
parte de cada bloque del manual vive hoy en el código Rust del proyecto*, *qué parte falta*, *cómo
se implementaría lo que falta* y *qué decide el proyecto no hacer*.

Convenciones:

* Cada afirmación sobre el estado actual del código lleva **`fichero.rs:línea`** y se ha
  verificado con `grep` sobre el árbol de `rust-core/src` en el commit `c7c0e2c`.
* Lo que no se ha podido verificar se marca **`NO VERIFICADO`** y se dice por qué.
* Los ficheros propuestos (`qa_engine.rs`, `sec_audit.rs`, `ipc_audit.rs`) **no existen**: son
  diseños. Las firmas Rust se dan como contrato, no como código.

---

## 1. Estado real hoy

### 1.1 Inventario, manifiesto y componentes de las APK

| Capacidad del manual | Dónde existe en MegaDaemon | Verificación |
|---|---|---|
| Lectura de APK (ZIP) sin reescribir el soporte ZIP | `rust-core/src/apk_inventory.rs:346` (`apk_entries`), lector `Read + Seek` propio sobre `crate::android::root_exec` + base64 | `apk_inventory.rs:1-22` (doc de cabecera) |
| Parseo del `AndroidManifest.xml` binario (AXML) | `rust-core/src/apk_inventory.rs:806` (`manifest_axml`) | firma en `apk_inventory.rs:806` |
| Huella canónica del manifiesto (detección de deriva) | `rust-core/src/apk_inventory.rs:886` (`manifest_canonical`), `:931` (`manifest_fingerprint`) | firma en `apk_inventory.rs:886,931` |
| Inventario de apps instaladas + verificación de deriva | `rust-core/src/apk_inventory.rs:1197` (`inventory`), `:1374` (`drift`), `:3325` (`scan_installed`), `:3452` (`rank_risk`), `:3470` (`verdict_installed`) | firmas verificadas |
| Permisos por clase de riesgo | `rust-core/src/apk_inventory.rs:1683` (`permission_risk_class`), `:1697` (`permission_risk_groups`) | firmas verificadas |
| **Componentes exportados sin permiso** (equivalente a `app.activity.info` de drozer, en modo estático) | `rust-core/src/apk_inventory.rs:2655` (`exported_without_permission`) | firma verificada |
| Componentes con `exported`, `permission`, `readPermission`, `writePermission`, `grantUriPermissions` e `intent-filter` | Parser AXML de `apk_inventory.rs:715-737` y volcado en `:789-794` | líneas verificadas |
| Permisos declarados por la app (`<permission>` + `protectionLevel`) | `rust-core/src/apk_inventory.rs:669` (parseo) y `:816` (volcado) | líneas verificadas |
| Barrido profundo de una APK (DEX, assets, nativas, firma, endpoints) | `rust-core/src/apk_inventory.rs:1231` (`analyze_apk`) + `:2777` (`deep_scan`) | firmas verificadas |
| Volcado de todas las APK instaladas | `rust-core/src/apk_inventory.rs:1403` (`dump_all`) | firma verificada |
| Lectura de `dumpsys package` / `pm list packages` (ficha por app) | `rust-core/src/apk_inventory.rs:2947` (`parse_package_paths`), `:2970` (`parse_installers`), `:2993` (`parse_dumpsys_packages`) — funciones puras y testeables | firmas verificadas |

**Veredicto: EXISTE** el *triage estático de superficie de ataque* de drozer, en la mitad que
MegalDaemon puede hacer sin instalar nada en el dispositivo: enumerar componentes exportados,
sus permisos y sus intent-filters. Falta la mitad *dinámica* (invocar esos componentes de verdad).

### 1.2 DEX, descompilación y hallazgos de credenciales

| Capacidad del manual | Dónde existe en MegaDaemon | Verificación |
|---|---|---|
| Lector DEX propio (formatos `035`..`041`) | `rust-core/src/dex_decompile.rs:522` (`DexFile`), `:591` (`DexFile::parse`) | firmas verificadas |
| Pseudo-Java por clase / desensamblado Dalvik | `rust-core/src/dex_decompile.rs:2309` (`decompile_class`), `:2567` (`decompile_all`), `:1863` (`disassemble_code`), `:2598` (`disassemble_method`) | firmas verificadas |
| Lector DEX *tolerante* y *correcto* (little-endian real) para APK reales | `rust-core/src/apk_deobfuscator.rs:273` (`DexFile`), `:969` (`decode_insn`), `:1180` (`disassemble`) — corrige los dos defectos medidos del lector compartido | `apk_deobfuscator.rs:1-30` |
| Mapa de desofuscación + grafo de llamadas (xrefs) | `rust-core/src/dex_decompile.rs:2792` (`analyze_obfuscation`), `:3210` (`obfuscation_map`); `apk_deobfuscator.rs:1328` (`CallEdge`), `:1341` (`MethodProfile`), `:1364` (`ClassProfile`) | firmas verificadas |
| **Hallazgos de credenciales / API keys embebidas** (parte de MobSF) | `rust-core/src/dex_decompile.rs:3875` (`ai_credential_findings`), `:3882` (`ai_credential_findings_raw`), `:3862` (`mask_secret`) | firmas verificadas |
| **Detección de anti-root / anti-frida** en el DEX (parte de RASP) | `rust-core/src/dex_decompile.rs:3986` (`anti_root_findings`), `:3697-3699` (marcadores `frida-agent`, `frida-gadget`, `frida-server`) | firmas verificadas |
| Hallazgos de ofuscación / empaquetador | `rust-core/src/dex_decompile.rs:4037` (`obfuscation_findings`); `apk_deobfuscator.rs:2068` (`detect_signatures`), `:2140` (`detect_proguard_like`) | firmas verificadas |
| Módulos de parcheo (LSPatch / Zygisk / Frida) detectados en la app | `rust-core/src/app_intel.rs:1902` (`detect_patcher_toolkit`), `:1913` (`detect_patcher_toolkit_detail`); `dex_decompile.rs:4431` (`lp_module_findings`) | firmas verificadas |
| URLs y secretos en texto libre (assets, bundles, `strings`) | `rust-core/src/app_intel.rs:750` (`extract_endpoints`), `:830` (`extract_secrets`), `:787` (`redact_secret`) | firmas verificadas |
| Barrido recursivo del almacenamiento privado de una app (con límites) | `rust-core/src/app_intel.rs:993` (`scan_local_storage`), `:1502` (`sqlite_to_ndjson`) | firmas verificadas |
| Exportación/importación idempotente a NDJSON | `rust-core/src/app_intel.rs:1261` (`data_exchange_plan`) | firma verificada |
| Salida estructurada de hallazgos (NDJSON) con merge local/externo | `rust-core/src/dex_decompile.rs:3553` (`Finding`), `:4119` (`findings_ndjson`), `:4179` (`merge_findings`) | firmas verificadas |

**Veredicto: EXISTE** el núcleo del *triage estático* de MobSF para Android (manifiesto,
componentes, permisos, flags del binario, secretos embebidos), escrito en Rust puro, sin Java.

### 1.3 Endurecimiento del binario (PIE / RELRO / canary) y entropía

| Capacidad | Dónde existe | Verificación |
|---|---|---|
| PIE/PIC, stack canary, RELRO de un ELF | `rust-core/src/binwalk_die.rs:300` (`PIE/PIC`), `:302` (`Stack Canary`), `:305` (`RELRO`), `:164` (`ASLR/DYNAMIC_BASE`), `:167` (`NX/DEP`) | líneas verificadas |
| Análisis completo de un binario (secciones, firmas embebidas, mitigaciones) | `rust-core/src/binwalk_die.rs:575` (`analyze_binary_full`) | firma verificada |
| Entropía por bloques + clasificación | `rust-core/src/entropy.rs:65` (`scan_entropy`), `:51` (`classify_entropy`), `:31` (`calculate_entropy`) | firmas verificadas |
| Codificaciones y bloques comprimidos en binarios | `rust-core/src/crypto_scan.rs:121` (`scan_crypto_and_encodings`) | firma verificada |
| Strings cifrados / empaquetador en DEX | `rust-core/src/dex_decompile.rs:2649` (`EncryptedString`), `:2740` (`base64_decode`) | firmas verificadas |

**Veredicto: PARCIAL.** El detector de mitigaciones existe y funciona sobre **un fichero ya
extraído** (`binwalk_die::analyze_binary_full` toma una ruta). Para cerrar el caso de uso MobSF
("PIE/RELRO/canary de **cada** `.so` del APK") falta el bucle que extrae las `.so` del ZIP y las
analiza: hoy `apk_inventory::analyze_apk` las **enumera** (`native_libs`, `abis`) pero no las
analiza.

### 1.4 Ocultación de root, RASP y detección de entorno

| Capacidad | Dónde existe | Verificación |
|---|---|---|
| Auditoría de fugas de root **por app** (ficheros, props, procesos, módulos, appops) | `rust-core/src/root_hide.rs:582` (`detect_leaks`), `:855` (`audit_all`) | firmas verificadas |
| Plan de remediación con comando exacto + reversibilidad | `rust-core/src/root_hide.rs:871` (`plan_remediation`), `:1120` (`apply_remediation`) | firmas verificadas |
| Perfil de ocultación de referencia y listas de apps sensibles | `rust-core/src/root_hide.rs:55` (`SPANISH_TARGET_APPS`), `:92` (`ROOT_TOOL_PACKAGES`), `:105` (`LSPOSED_DENY_MODULES`), `:128` (`PROP_EXPECTATIONS`), `:144` (`ROOT_PATH_MARKERS`) | firmas verificadas |
| Perfil "estado de fábrica" (props, `pm hide`, `su` fuera del PATH) | `rust-core/src/root_stealth.rs:81` (`apply`), `:169` (`status`), `:211` (`preview`) | firmas verificadas |
| **Límite honesto ya escrito en el código**: no se falsifica attestation de hardware | `rust-core/src/root_hide.rs:26-32` (doc de cabecera) y `rust-core/src/root_stealth.rs:12-16` | líneas verificadas |

**Veredicto: EXISTE** la mitad *defensiva* del bloque de RASP detection: MegaDaemon sabe **medir**
qué delata el root ante una app concreta y proponer la remediación. **No** existe la mitad
ofensiva (bypassear el RASP de una app de terceros para auditarla), que es la que describe el
manual y que queda fuera de alcance (§4).

### 1.5 Cliente ADB nativo y superficie de UI

| Capacidad | Dónde existe | Verificación |
|---|---|---|
| Protocolo ADB v1 en Rust puro (sin binario `adb`): `CNXN`/`AUTH`/`OPEN`/`OKAY`/`CLSE`/`WRTE` | `rust-core/src/adb_client.rs:44-50` (comandos), `:299-325` (`Message::encode`/`decode`) | firmas verificadas |
| Transporte `adbd` directo (`<ip>:5555`, el único usable desde el propio móvil) | `rust-core/src/adb_client.rs:583` (`Adbd`), `:602` (`connect_authenticated`), `:662` (`shell`), `:721` (`push_file`), `:797` (`pull_file`) | firmas verificadas |
| Transporte servidor ADB local (`127.0.0.1:5037`) | `rust-core/src/adb_client.rs:497` (`AdbServer`), `:508` (`devices`), `:537` (`shell`) | firmas verificadas |
| Firma del token `AUTH` con la clave de host RSA (en memoria, la clave no sale del disco) | `rust-core/src/adb_client.rs:135` (`HostKey`), `:161` (`sign_token`), `:228` (`host_key`) | firmas verificadas |
| `shell:` genérico como herramienta MCP (`adb`) | `rust-core/src/adb_tool.rs:33` (tool `adb`, acciones `status`/`shell`/`devices`/`keyinfo`/`push`/`pull`) | verificado |
| **`uiautomator dump`** (árbol de accesibilidad) | `rust-core/src/android.rs:689` (dentro de `screen_and_ui`, acción `dump`) | línea verificada |
| **`screencap`** (captura de pantalla) | `rust-core/src/android.rs:680` (acción `screenshot`) | línea verificada |
| **`input tap` / `swipe` / `keyevent` / `text`** | `rust-core/src/android.rs:695` (tap), `:704` (swipe), `:718` (key), `:731` (text) | líneas verificadas |
| Lanzar la app bajo prueba | `rust-core/src/android.rs:655` (`monkey -p <pkg> -c android.intent.category.LAUNCHER 1`, acción `open` de `app_control`) | línea verificada |
| Tool MCP `screen_and_ui` | `rust-core/src/android.rs:1030` (nombre de la tool) | verificado |
| Shell root **persistente** por socket unix (0 procesos nuevos por llamada) | `rust-core/src/persistent_shell.rs:124` (`PersistentShell`), `:212` (`run_via_socket`), `:501` (`ShellToolExecutor`) | firmas verificadas |
| Detección de comandos destructivos | `rust-core/src/android.rs:96` (`is_destructive`), `persistent_shell.rs:534` | firmas verificadas |

**Veredicto: PARCIAL — y es el punto de partida del motor de QA.** Ya existen las **cinco
primitivas** de un runner de UI (`dump`, `tap`, `swipe`, `key`/`text`, `screenshot`) y el lanzador de
app. Lo que **no** existe es la capa de arriba: *resolver un selector* contra el árbol de
accesibilidad, *esperar a que la UI se estabilice*, *reintentar una aserción que parpadea* y
*componer todo eso en un flujo declarativo*. Ese hueco es exactamente lo que propone §5.

### 1.6 Instrumentación dinámica (Frida) y DBI

| Capacidad | Dónde existe | Verificación |
|---|---|---|
| Generador de scripts Frida (attach / stalker / native_call / android_jni / tenet_mem) | `rust-core/src/frida_dbi.rs:71` (`generate_frida_script`) | firma verificada |
| Análisis con Rizin (funciones, imports, exports) | `rust-core/src/frida_dbi.rs:23` (`run_rizin_analysis`) | firma verificada |
| Inspección de procesos vivos (mapas de memoria) | `rust-core/src/frida_dbi.rs:247` (`inspect_live_processes`) | firma verificada |
| **Hook de Frida sobre OkHttp** (`okhttp3.internal.http.BridgeInterceptor.intercept`) | `rust-core/assets/frida_chrome_cookies.js:105-133` (`hookOkHttp`), enganchado en `:187` | líneas verificadas |
| **Hook de Frida sobre `java.net.URL.openConnection`** | `rust-core/assets/frida_chrome_cookies.js:77-81`, enganchado en `:188` (`hookJavaNet`) | líneas verificadas |
| Hook del gestor de cookies de Chromium (`libcookie_monster.so`) | `rust-core/assets/frida_chrome_cookies.js:51-75` (`tryHookCookieMonster`) | líneas verificadas |
| Uso de `frida` en el dispositivo ya integrado en otro módulo | `rust-core/src/call_recorder.rs:416-429` (`frida-ps -Ua`, `frida -U -n <pkg> -l <script>`) | líneas verificadas |
| Capacidad declarada en el registro | `rust-core/src/capability.rs:139` (`bins: &["frida"]`, `modules: &["frida_dbi"]`) | verificado |

**Veredicto: EXISTE** la espina dorsal de Frida (CLI + generador de hooks + ya hay un hook real de
OkHttp/`openConnection`). **Falta**: un *agente* con `rpc.exports` (la superficie que Brida
consume) y un puente entre Burp y ese agente. Ver §7.

### 1.7 Sincronización, NDJSON y bóveda de sesiones

| Capacidad | Dónde existe | Verificación |
|---|---|---|
| Motor de sincronización de conversaciones (descubrimiento + normalización + NDJSON + índice) | `rust-core/src/sync.rs:257` (`run_sync`), `:403` (`ingest_conversations`), `:479` (`search`) | firmas verificadas |
| Ingesta NDJSON idempotente por `service+id` | `rust-core/src/sync.rs:403` | firma verificada |
| Búsqueda en el almacén de sesiones | `rust-core/src/sync.rs:479` (`search`), `:572` (`list_conversations`) | firmas verificadas |

**Veredicto: EXISTE** la infraestructura de ingesta NDJSON + índice que un flujo de tráfico
interceptado puede reutilizar tal cual (no hay que escribirla).

### 1.8 Firma, backup y otros (contexto, fuera del manual)

No forman parte del manual pero condicionan el diseño (p. ej. el requisito de "firmar la APK bajo
prueba" de Calabash):

* `rust-core/src/signing.rs` — PKCS#11/OpenSC, CMS/CAdES, PAdES-B-B, protocolo `afirma://`
  (firma **electrónica**, no firma de APK Android; no sirve para `resign`).
* `rust-core/src/backup_formats.rs` — lectores `tar`/`zip`/Swift Backup (`:70`, `:205`, `:400`).
* `rust-core/src/aegis.rs` — AEGIS-256 (RFC 10032) para leer backups de Swift Backup.
* `rust-core/src/argon2.rs` — Argon2id (RFC 9106) para la derivación de clave.
* `rust-core/src/drive_client.rs` — subida por trozos a Drive (`:181`, `:204`).
* `rust-core/src/channel.rs` — estado y publicación del canal (túnel Cloudflare + DuckDNS).

### 1.9 Canal LAN (descubrimiento sin Internet)

| Capacidad | Dónde existe | Verificación |
|---|---|---|
| **Beacon UDP en difusión** que anuncia dirección y puerto del móvil, sin Internet y sin terceros | `rust-core/src/lan.rs:33` (`BEACON_PORT = 9099`), `:39` (`BEACON_KIND`), `:127` (`beacon_payload`), `:231` (`spawn_beacon`) | firmas verificadas |
| Escucha del beacon en el PC | `rust-core/src/lan.rs:190` (`listen_beacons`), binario `rust-core/src/bin/md-lan-listen.rs` | verificado |
| Descubrimiento de la IP local sin depender de Internet | `rust-core/src/lan.rs:55` (`local_ipv4_all`), `:107` (`primary_ipv4`), `:121` (`lan_url`) | firmas verificadas |
| Arranque del beacon junto al servidor del cockpit | `rust-core/src/cockpit_server.rs:2314-2322` (`crate::lan::spawn_beacon(...)`) | verificado |
| Límite honesto documentado: la difusión no cruza routers; fuera de la subred sigue haciendo falta el túnel | `rust-core/src/lan.rs:17-24` | verificado |

**Por qué importa en este documento**: el canal LAN **ya existe** (descubrimiento + beacon), pero
sólo transporta *dirección y puerto*. Lo que falta para que sea un canal de servicio completo es
la **capa de protocolo** (emparejamiento, autenticación, mensajes con tipado, payload) — que es
exactamente lo que aporta la especificación de KDE Connect (§2, fila de KDE Connect). El plan de
`REPO_TRIAGE.md` (Top 10 nº1) se apoya en este módulo ya existente, no en empezar de cero.

### 1.10 Superficie MCP y registro de capacidades

| Capacidad | Dónde existe | Verificación |
|---|---|---|
| Catálogo MCP unificado (stdio + HTTP) | `rust-core/src/mcp_server.rs:20` (`tool_definitions`), `:193` (`dispatch_tool`), `:358` (`handle_rpc`), `:399` (`run_stdio_mcp_server`) | firmas verificadas |
| Catálogo extendido de tools (25 tools) | `rust-core/src/mcp_ext.rs:17` (`tool_definitions`), `:345` (`dispatch`) | verificado |
| Registro de capacidades con dependencias reales y `verify()` en el dispositivo | `rust-core/src/capability.rs:237` (`registry`), `:292` (`verify`), `:341` (`manifest`), `:360` (`digest`) | firmas verificadas |
| Superficie HTTP del cockpit | `rust-core/src/cockpit_server.rs:1196-2100` (rutas `/api/*`) | verificado |
| Tool nativo ADB | `rust-core/src/adb_tool.rs:33` | verificado |

**Nota importante**: la política del proyecto es que **ninguna tool que modifique el dispositivo,
publique en un remoto o mate procesos se ejecuta sin `confirm: true`** (`mcp_ext.rs:9-10`). La tool
`qa_run` que se propone en §5 hereda esa política.

### 1.11 Resumen de estado

| Bloque del manual | Estado en MegaDaemon |
|---|---|
| Triage estático de APK (manifest, permisos, componentes, secretos, flags) | **EXISTE** |
| Superficie IPC estática (componentes exportados y sus permisos) | **EXISTE** (estático) |
| Primitivas de UI (`uiautomator dump`, `input`, `screencap`) | **EXISTE** (sin capa de flujo) |
| Cliente ADB nativo + shell persistente | **EXISTE** |
| Frida: generador de hooks + hook real de OkHttp/`openConnection` | **EXISTE** (sin agente RPC) |
| Auditoría de root hiding / evasión de RASP **del propio dispositivo** | **EXISTE** |
| Bypasseo de RASP/SSL-pinning en apps de terceros | **AUSENTE** (fuera de alcance, §4) |
| Intercepción de tráfico (proxy, replay, fuzzing de API) | **AUSENTE** |
| Emulación de binario nativo (Unidbg) | **AUSENTE** |
| Emulador de Android controlado (CyAndroEmu) | **AUSENTE** (el dispositivo real ya es el emulador) |
| Runner declarativo de QA con reintentos | **AUSENTE** |
| Pruebas visuales en JVM (Paparazzi/Roborazzi) | **AUSENTE** (requiere código fuente de la app) |
| Root en kernel (KernelSU/APatch) | **AUSENTE** (el dispositivo usa Magisk) |
| Canal LAN sin Internet (descubrimiento + beacon) | **EXISTE** (sin capa de protocolo: falta emparejamiento y mensajes) |

---

## 2. Matriz manual → MegaDaemon

Esfuerzo: **S** = 1-2 días · **M** = ~1 semana · **L** = 2-4 semanas · **XL** = > 1 mes.

| Capacidad | Herramienta de referencia | Estado en MegaDaemon | Módulo Rust destino | Prioridad | Esfuerzo | Nota |
|---|---|---|---|---|---|---|
| BDD de aceptación en `.feature` (Gherkin) | Calabash-Android | **AUSENTE** | `qa_engine.rs` (`gherkin.rs` hermano) | P2 | M | Calabash está sin mantenimiento (el propio repo pide mantenedor). Lo interesante es el *parser Gherkin*, no el runner Ruby: `qa_engine` puede leer `.feature` y bajar a `Flow`. |
| Resign de la APK para que el servidor de test comparta UID | Calabash-Android | **AUSENTE** | `signing.rs` (firma Android, no PKCS#11) | P2 | L | Requiere implementar firma v1/v2/v3 de APK + `apksigner` equivalente. Es la pieza más cara y de menor retorno: el motor de QA de §5 **no** necesita resignar. |
| Test white-box sincronizado con el hilo principal | Espresso / Compose Test | **AUSENTE** (y no aplica) | — | — | — | Exige el *código fuente* de la app y un `androidTest` en el proceso. Fuera del modelo de MegaDaemon (agente externo sobre el dispositivo). |
| Test declarativo YAML de UI (`tapOn`, `assertVisible`) | Maestro | **PARCIAL** (primitivas sí, YAML no) | `qa_engine.rs` + `qa_yaml.rs` | **P0** | M | Lo más cercano al modelo de MegaDaemon: caja negra sobre ADB, sin recompilar. El formato YAML de Maestro es el punto de entrada más barato para flows nuevos. |
| Cliente-servidor W3C WebDriver multi-plataforma | Appium 2.x/3.x | **AUSENTE** | `qa_engine.rs` (transport WebDriver) | P2 | L | El servidor ADB nativo de `adb_client` cubre el caso "un dispositivo"; Appium aporta multi-dispositivo y multi-plataforma, que MegaDaemon no necesita. |
| Interacción con elementos fuera de la app (ajustes, diálogo de permisos, bandeja) | UI Automator | **PARCIAL** | `qa_engine.rs` (`Step::Call` → `android::app_control`) | **P0** | S | `uiautomator dump` ya ve el sistema; falta el envoltorio declarativo y el acceso al diálogo de permisos (`grant_perm` ya existe en `android.rs:611`). |
| DSL Kotlin con reintentos anti-flaky | Kaspresso | **PARCIAL** (concepto, no código) | `qa_engine.rs` (`Assertion` + `retry`) | **P0** | M | Lo **valioso** de Kaspresso es el modelo de *idleness*/reintento; se puede replicar en Rust sin Kotlin. |
| Render de layouts en la JVM sin emulador | Paparazzi | **AUSENTE** | — | P2 | XL | Requiere `LayoutLib` y el código fuente de la app. No es "portable a Rust": depende del SDK de Android. Documentar como referencia. |
| Captura y comparación de screenshots sobre Robolectric | Roborazzi | **AUSENTE** | `qa_engine.rs` (Step::Screenshot) como *base* | P2 | M | La parte portable es el **diff de imágenes** (base de regresión visual sobre las capturas que `screencap` ya produce). La parte JVM no. |
| E2E grey-box para React Native (sincroniza el hilo JS) | Detox | **AUSENTE** | — | P2 | — | Requiere un binario de test de RN dentro de la app. No aplica al modelo agente-externo. |
| E2E Flutter con puente nativo (diálogos de permiso, WebView) | Patrol / `integration_test` | **AUSENTE** | — | P2 | — | Igual que Detox: exige instrumentación dentro de la app. |
| Stack Appium 2.x + WebdriverIO | Appium + WDIO | **AUSENTE** | `qa_engine.rs` | P2 | L | Degradación: el `qa_engine` puede exponer su propio transporte en vez de hablar WebDriver. |
| Plataformas low-code con auto-reparación de localizadores | testRigor / Katalon / Sofy | **AUSENTE** | `qa_engine.rs` (estrategia `Selector::Any` + reintento con replanteo) | P1 | M | **Lo recuperable**: la idea de "varios localizadores alternativos + reintento" se implementa en `Selector::Any` + backoff. Lo no recuperable: la plataforma cloud en la que vive y el LLM de auto-reparación. |
| Instrumentación dinámica (hooks Java/nativos, lectura de memoria) | Frida | **EXISTE (parcial)** | `frida_dbi.rs` + `assets/frida_*.js` | **P0** | S | Ya hay generador de scripts (`frida_dbi.rs:71`) y un hook real (`assets/frida_chrome_cookies.js:105,77`). Falta el *agente* con `rpc.exports` (§7). |
| REPL de exploración en runtime (unpin SSL, volcado de keystore) | Objection | **AUSENTE** | `sec_audit.rs` (parte estática) | P1 | M | Objection es un REPL humano sobre Frida. El equivalente útil en MegaDaemon es un **scanner** automático, no un REPL: `sec_audit` + `ipc_audit`. |
| Escáner automático de APK (manifiesto, secretos, flags, dinámicos) | MobSF | **PARCIAL → puede ser EXISTE** | `sec_audit.rs` (nuevo) + `apk_inventory` + `dex_decompile` + `binwalk_die` | **P0** | M | Todo el insumo ya existe (§1.1-1.3); falta el **aggregador** que produces un informe único con severidad y remediación. Es el P0 con mejor relación esfuerzo/valor. |
| Proxy de interceptación y replay HTTP/S | Burp Suite | **AUSENTE** | `sec_audit.rs` (cliente) + `brida_bridge.rs` | **P1** | L | Burp es GUI de escritorio: MegaDaemon no lo replica, lo **integra** (§7). Un proxy propio en Rust es un proyecto aparte; se recomienda Brida + un `mitmproxy` local. |
| Proxy interactivo open-source | mitmproxy | **AUSENTE** | `sec_audit.rs` (cliente de un proxy local) | **P1** | M | Vía concreta: `mitmproxy` corre en el PC del usuario, MegaDaemon le manda el tráfico del móvil por el túnel y consume su API. |
| Emulador de Android controlado por script | CyAndroEmu | **AUSENTE (y es un no-go de diseño)** | — | P2 | — | MegaDaemon **usa el móvil real del usuario** como objetivo: un emulador no aporta nada aquí y multiplicaría el coste de despliegue. |
| Emulación headless de librerías nativas ARM64 (JNI) | Unidbg | **AUSENTE** | `sec_audit.rs` (nivel estático) / motor de emulación | P2 | XL | Unidbg es una JVM con Unicorn. El subconjunto portable a Rust: **analizar estáticamente** la `.so` (`binwalk_die` + `disasm`) y **encontrar el algoritmo de firma** por strings/xrefs (ya hay xrefs en `apk_deobfuscator`). Emular la CPU completa: XL, y sólo tiene sentido si hay `.so` con material criptográfico. |
| Análisis estático de `.so` (exports JNI, `JNI_OnLoad`) | Ghidra | **PARCIAL** | `frida_dbi.rs` (Rizin), `disasm.rs`, `binwalk_die.rs` | **P1** | M | Rizin ya da funciones/imports/exports (`frida_dbi.rs:23`). Falta la parte "export JNI con nombre estándar" y "rastreo de `RegisterNatives`" ⇒ escáner de patrones sobre la `.so`. |
| Puente Burp ↔ Frida (cifrado de payloads a nivel de aplicación) | Brida | **AUSENTE** (las dos mitades sueltas existen) | `brida_bridge.rs` + `assets/frida_rpc_exports.js` | **P1** | M | Diseño en §7. Es **la** pieza que convierte "veo el cifrado" en "puedo mutar el plaintext". |
| Enumeración y explotación de superficies IPC | drozer | **PARCIAL** | `ipc_audit.rs` (nuevo) | **P0** | M | Lo estático ya existe (`apk_inventory.rs:2655`); falta la enumeración por componente y las pruebas dinámicas (`content query`, `am start`). |
| Módulos Frida para IPC, WebView, Keystore, pending intents | Medusa | **AUSENTE** | `assets/frida_ipc.js` + `ipc_audit.rs` | **P1** | M | Medusa son 100+ scripts. Se porta **un subconjunto útil**, no el catálogo. |
| Root en el kernel (hooks de syscall, `uid` en espacio kernel) | KernelSU / APatch | **AUSENTE** | `root_hide.rs` (evaluación del impacto) | P2 | — | El dispositivo objetivo usa **Magisk**; no se cambia el gestor de root del usuario. Lo que sí se puede hacer es **medir** el impacto: `root_hide::detect_leaks` ya enumera qué delata el Magisk ante cada app. |
| Inyección en Zygote antes de los constructores estáticos | Zygisk-Frida | **AUSENTE** | `root_hide.rs` (evaluación) | P2 | — | Requiere Zygisk + un build parcheado de `frida-server`. El valor real de esta técnica es **medir qué detecta una app**, y eso ya se puede hacer con `root_hide`. |
| `frida-server` con strings renombradas (anti-detección de firmas) | "Florida" | **AUSENTE** | — | P2 | — | Technique es "esconder la herramienta de la app". El objetivo legítimo equivalente en MegaDaemon es **auditar el propio dispositivo**: qué módulos Frida/Xposed están presentes (`dex_decompile.rs:3697`, `root_hide.rs:105`). No se implementa el empaquetado evasivo. |
| Detección de RASP (maps, puertos 27042/27043, `TracerPid`, threads `gmain`/`gum-js-loop`, `su` en disco) | Manual Parte 4 §1 | **EXISTE** | `root_hide.rs:582`, `dex_decompile.rs:3986`, `apk_inventory.rs:1640` | **P0** | S | MegaDaemon tiene **las tres capas**: qué detecta la app (`dex_decompile`), qué delata el dispositivo (`root_hide`), y qué toolkits de parcheo están instalados (`app_intel:1902`). Falta sólo un informe unificado. |
| Play Integrity / attestation como defensa | Manual Parte 4 §1 | **EXISTE como frontera documentada** | `root_hide.rs:26-32` | — | — | El código ya dice lo que no se hace y por qué. Se mantiene esa línea (§4). |
| Macros de UI (toques, swipes, disparadores por evento) | `macrion`, `moody-routine`, `vokibot` | **AUSENTE** | `qa_engine.rs` (como *flows*) | P1 | M | Es la misma maquinaria que el motor de QA: un flow es una macro. La diferencia es que las macros son de **usuario** y los flows de **QA**. Decisión: un solo motor, dos bombos. |
| Automatización por contexto (hora, ubicación, perfil situacional) | `moody-routine` | **AUSENTE** | `qa_engine.rs` (`Step::Call` → `dumpsys location`) | P2 | M | Sólo el disparador automático es interesante; la UI de Samsung no. |
| Telemetría GPS y track logging | `gpslogger`, `ulogger-android` | **PARCIAL** | `battery_advanced.rs` (sensores), `sync.rs` (NDJSON) | P2 | S | Ya hay `location` vía `dumpsys` en el módulo de batería; el valor es el *pipeline* NDJSON, que ya existe. |
| Puentes SMS → bot / Matrix / Nextcloud | `sms-telebot`, `sms-ie`, `SmsMatrix#14`, `ocsms#176` | **AUSENTE** | `messaging.rs` (bus) + `sync.rs` | P1 | M | **Hallazgo del triage**: los dos hilos de issues que cita el manual **no hablan de un puente de arquitectura**, sino de **normalización de números telefónicos** (E.164) antes de indexar. Eso sí es portable y es un requisito de `messaging.rs`. |
| Gestión de dispositivo por canal remoto (MAXS: XMPP/SMS/webhooks) | `ProjectMAXS/maxs`, `MaxManager`, `projectmaxs.org` | **AUSENTE** (el equivalente ya existe) | `channel.rs` + `service_bus.rs` + `mesh.rs` | P2 | — | MAXS usa **XMPP** como transporte. MegaDaemon ya tiene `channel.rs` (túnel + DuckDNS) y `service_bus.rs`. Lo que se adopta es la **desacoplamiento módulo/transporte**, no XMPP. |
| Protocolo de escritorio↔móvil por LAN (JSON sobre TLS, pairing) | `kde/kdeconnect-kde`, `kdeconnect-android`, `KDE/kdeconnect-meta` | **AUSENTE** | `channel.rs` (nuevo `kde_channel.rs`) | **P0** | M | Ver Top-10 de `REPO_TRIAGE.md`: implementar el protocolo JSON/TLS de KDE Connect en Rust da un **canal LAN que no depende de Internet**, que es el sustituto cuando cloudflared o DuckDNS fallan. |
| Extracción de cookies de sesión del propio navegador | `cookies-extractor` | **EXISTE (mejor que el original)** | `assets/frida_chrome_cookies.js` + `sync.rs` | **P0** | S | El proyecto `cookies-extractor` extrae cookies de un WebView propio; MegaDaemon las extrae **del proceso real** con Frida (Chromium + OkHttp + `openConnection`). Es un caso donde el repo de referencia es **inferior** a lo ya implementado. |
| Distribución/gestión de aplicaciones (lista, backup, instalación) | `AppListBackup` | **EXISTE** | `apk_inventory.rs:1197` | — | — | Ya hay inventario con SHA-256 y detección de deriva, que es un superset. |
| Automatización por llamada telefónica / SMS entrante | `MethodCall`, `otphelper` | **AUSENTE** | `messaging.rs` (call log), `call_recorder.rs` | P2 | M | `call_recorder.rs:582` ya detecta llamadas activas vía `dumpsys audio`. |
| Clientes MQTT / IoT | `MqttDroid`, `knockonports` | **AUSENTE** | `service_bus.rs` | P2 | M | `knockonports` es un knock-knock HTTP: irrelevante para el harness. |
| Control de API gráfica (Vulkan/OpenGL) | `gama` | **PARCIAL** | `npu_vs4l.rs` (probes) | P2 | S | Útil como *escenario de estrés* del motor de QA (cambiar la API gráfica y re-ejecutar el flow). |
| Reinicio / power management | `RebootNya`, `SU_IMD` | **EXISTE** | `battery_advanced.rs`, `android.rs` (`app_control`) | P2 | S | `battery_advanced.rs:919` (`apply_with`) y `android.rs` cubren ya el modelo "plan reversible + `confirm`". |
| Automatización USB (gadget mode) | `android-usb-script` | **AUSENTE** | — | P2 | — | Nicho. La parte útil: `adb_client` ya tiene `push_file`/`pull_file` por el protocolo SYNC. |
| Monitorización remota de dispositivo (telemetría) | `hermes-agent` | **PARCIAL** | `env_introspect.rs`, `capability.rs:292` (`verify`) | P2 | S | El equivalente ya existe como registro de capacidades verificable en el dispositivo. |
| rsync / sincronización de ficheros vía Tasker | `TaskerRsync` | **EXISTE (otro canal)** | `drive_client.rs`, `backup_formats.rs` | P2 | — | El backup ya va por Drive por trozos (`drive_client.rs:142` `chunk_plan`). Tasker+rsync es un canal alternativo, no mejor. |
| Bomba de SMS / utilidades de automatización de referencia | `sms-ie`, `vokibot` | **PARCIAL** | `backup_formats.rs` (import/export NDJSON) | P2 | S | El formato de intercambio de `sms-ie` (NDJSON) ya es el que usa `sync.rs`. |

---

## 3. Qué NO se implementa y por qué

Esta sección es una **decisión de alcance del proyecto**, no un juicio sobre el manual. El manual
describe un ecosistema de herramientas de penetration testing comercial y ofensiva. MegaDaemon
opera un dispositivo **propio** con el propósito de **administración y automatización**. La
frontera se define por el destinatario de la acción y por el tipo de transformación que se hace
sobre el software.

### 3.1 La frontera, en dos frases

* **Sí**: análisis, auditoría, escaneo y *laboratorio* sobre apps y sobre el propio dispositivo —
  inventarios, detección de fugas, auditoría de hardening, flujo de QA, intercepción de tráfico de
  las cuentas del usuario, informes accionables.
* **No**: alterar la lógica de negocio de terceros, no descifrar material protegido por DRM, no
  suplantar compras verificadas ni saltar controles de licencia.

### 3.2 Lista explícita de "no" (con el motivo de ingeniería)

| No se implementa | Motivo |
|---|---|
| Cracking de licencias premium (p. ej. el módulo Xposed `swiftbackupprem`, que desbloquea funciones de pago) | El proyecto no altera la economía de terceros. `swiftbackupprem` se documenta como referencia, no como dependencia (`REPO_TRIAGE.md`). |
| Bypass de DRM / descifrado de contenido protegido | Fuera del modelo de datos de MegaDaemon: el daemon no almacena contenido protegido ni lo redistribuye. |
| Suplantación de compras verificadas (`billing`, Play Billing, LicenseManager) | Es una alteración del estado de autorización de un servidor ajeno. El motor de QA puede **verificar** que un flujo de compra está bien implementado (incluido el caso de error), nunca **fabricar** una compra. |
| Bypasseo de SSL-pinning / anti-root de apps de terceros con el fin de extraer datos que las app protegen | El propósito legítimo equivalente (auditar el propio dispositivo, entender qué se delata) ya está cubierto por `root_hide::detect_leaks` + `dex_decompile::anti_root_findings`. Lo que no se ofrece es un "modo agresivo" contra una app que no es del usuario. |
| Emulación de hardware de attestation (Play Integrity `STRONG`, key attestation de Android Keystore) | Ya está documentado como límite en el propio código (`root_hide.rs:26-32`, `root_stealth.rs:12-16`). Suplantar attestation requiere material firmado por el fabricante; no es un problema de ingeniería sino de disponibilidad de claves. |
| Empaquetado evasivo de herramientas (`florida`, `frida-server` con strings renombradas) | El objetivo de esa técnica es *ocultar la herramienta a la app*. El valor equivalente en MegaDaemon es **medir** qué se detecta (`root_hide`, `dex_decompile`), no **ocultarse**. |
| Módulos de instrumentación que extraen material criptográfico de otras apps (keystore de terceros, tokens de sesión ajenos) | El daemon extrae credenciales **propias** (las cookies de las cuentas del usuario: `assets/frida_chrome_cookies.js` lo dice explícitamente en su cabecera) y redacta siempre los valores (`app_intel::redact_secret`, `dex_decompile::mask_secret`). |
| Distribución de exploits públicos (el propio drozer publica "public Android exploits") | MegaDaemon produce **informes**, no exploits. El formato de salida es JSON/NDJSON con severidad y remediación. |
| Emulador de Android propio para análisis de malware | El dispositivo del usuario **es** el entorno de análisis. Multiplicar emuladores no aporta al objetivo. |

### 3.3 Qué se hace en su lugar

| Capacidad del manual | Equivalente en MegaDaemon (dentro de la frontera) |
|---|---|
| "Unpin SSL de una app de terceros para ver su tráfico" | Intercepción del tráfico **de las cuentas del usuario** vía proxy propio + `brida_bridge` para el caso de cifrado a nivel de aplicación (`sync.rs` ingiere el NDJSON). |
| "Bypassear anti-root para que la app funcione" | **Auditar** el dispositivo: qué señal delata el root ante cada app, con plan de remediación reversible (`root_hide::detect_leaks` → `plan_remediation` → `apply_remediation`). |
| "Extraer la clave de licencia del protector" | Describir **qué hace** cada componente (`apk_deobfuscator`: clase → método → efecto), que es lo que un auditor necesita y lo que el módulo ya documenta en su sección "Alcance legal" (`apk_deobfuscator.rs:23-31`). |
| "Fuzzing de payloads cifrados en Burp" | Flow de QA + `sec_audit` que **verifica** la corrección del cifrado en la app propia (round-trip, claves hardcodeadas, IV reutilizado). |

---

## 4. El motor de QA en Rust

### 4.1 Fichero propuesto

`rust-core/src/qa_engine.rs` — un solo módulo, sin dependencias nuevas (todo con `serde` /
`serde_json`, ya presentes en `Cargo.toml`).

### 4.2 Por qué este diseño y no otro

1. **Caja negra, no caja blanca.** Espresso/Kaspresso/Detox/Patrol exigen instrumentación dentro
   de la app ⇒ exigen el código fuente ⇒ no aplican al modelo de MegaDaemon (agente externo
   sobre el dispositivo del usuario). Lo que sí aplica es el modelo **Maestro**: caja negra sobre
   ADB.
2. **Reutilización de primitivas ya existentes.** `android::screen_and_ui` ya da `dump`, `tap`,
   `swipe`, `key`, `text`, `screenshot`; `adb_client::Adbd` da `shell` con timeout y tope de
   bytes. El motor **no** reimplementa nada: compone.
3. **Determinismo sobre el móvil.** El módulo `persistent_shell` existe porque "cada llamada a
   tool crea 2+ procesos" y el móvil va al límite. El motor debe poder correr **un flow entero**
   por el shell persistente, no 20 `su` sueltos.
4. **Idempotencia y reintento.** El manual insiste (Kaspresso, Maestro, Detox) en eliminar
   `sleep()`. La solución es *esperar por condición* sobre el árbol de accesibilidad, con
   timeout, no dormir.

### 4.3 Contrato de tipos

```rust
//! Motor de QA declarativo: flows, selectores, aserciones y planificador.
//! Todo en Rust puro; depende sólo de serde/serde_json.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Selectores
// ---------------------------------------------------------------------------

/// Cómo se localiza un nodo del árbol de accesibilidad (`uiautomator dump`).
///
/// `Any` es la clave del modo "anti-flaky": se listan los localizadores en orden
/// de preferencia y el planificador usa el primero que resuelva.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum Selector {
    /// `resource-id`: `com.app:id/btn` o `btn` (se acepta el sufijo corto).
    ResourceId { id: String },
    /// Texto visible exacto.
    Text { text: String },
    /// Texto que contiene la subcadena (case-insensitive opcional).
    TextContains { text: String, case_sensitive: bool },
    /// `content-desc` (accesibilidad).
    ContentDesc { value: String, exact: bool },
    /// Clase de la vista (`android.widget.Button`).
    ClassName { class: String },
    /// Posición dentro de su padre (0 = el primero que encaje).
    Index { index: usize, of: Option<String> },
    /// Coordenadas: último recurso, no determinista (sólo para smoke tests).
    Bounds { x: i32, y: i32 },
    /// Lista ordenada de alternativas: gana el primero que resuelva.
    Any { of: Vec<Selector> },
}

impl Selector {
    /// Nombre corto para logs y reportes (nunca contiene el valor del selector,
    /// que puede ser texto sensible de la UI).
    pub fn kind(&self) -> &'static str;
}

// ---------------------------------------------------------------------------
// Aserciones
// ---------------------------------------------------------------------------

/// Condición comprobable sobre el estado de la UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "assert", rename_all = "snake_case")]
pub enum Assertion {
    /// El elemento existe y es visible.
    Visible { selector: Selector },
    /// El elemento no existe (o no es visible).
    NotVisible { selector: Selector },
    /// `enabled` / `focusable` / `checked` / `selected` con el valor esperado.
    Flag { selector: Selector, name: String, value: bool },
    /// Texto exacto o contenido.
    Text { selector: Selector, equals: Option<String>, contains: Option<String> },
    /// Número de coincidencias del selector.
    Count { selector: Selector, equals: usize },
    /// Atributo arbitrario del nodo (`package`, `resource-id`, `bounds`, ...).
    Attribute { selector: Selector, name: String, equals: Option<String> },
}

// ---------------------------------------------------------------------------
// Pasos
// ---------------------------------------------------------------------------

/// Dirección de un gesto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction { Up, Down, Left, Right }

/// Una acción del flow.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Step {
    /// Lanza (o relanza) la app bajo prueba.
    Launch { package: String, activity: Option<String> },
    /// Toca el elemento que resuelva el selector.
    Tap { selector: Selector },
    /// Escribe en el elemento resuelto (hace `tap` + `input text`).
    Type { selector: Selector, value: String },
    /// Gesto direccional, opcionalmente anclado a un elemento.
    Swipe { direction: Direction, from: Option<Selector> },
    /// `input keyevent <keycode>`.
    Key { keycode: String },
    /// Espera a que el selector aparezca (o a que desaparezca si `gone`).
    Wait { selector: Option<Selector>, gone: bool, timeout_ms: u64 },
    /// Pausa fija. Desaconsejada: sólo para escenarios de animación medidos.
    Sleep { ms: u64 },
    /// Comprueba una aserción (participa del reintento).
    Assert { assertion: Assertion, timeout_ms: u64 },
    /// Captura de pantalla (artefacto del flow).
    Screenshot { path: Option<String> },
    /// Vuelca el árbol de accesibilidad a disco (artefacto del flow).
    DumpUi { path: Option<String> },
    /// Vuelve atrás.
    Back,
    /// Invoca cualquier tool del registro (p. ej. `app_control`, `adb`).
    /// Es la puerta de extensión: permisos, appops, hikers de red, etc.
    Call { tool: String, args: Value },
}

// ---------------------------------------------------------------------------
// Flow
// ---------------------------------------------------------------------------

/// Un flujo de QA completo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flow {
    /// Nombre legible (aparece en el informe).
    pub name: String,
    /// Paquete bajo prueba.
    pub package: String,
    /// Preparación (limpiar datos, conceder permisos, abrir la app).
    #[serde(default)]
    pub setup: Vec<Step>,
    /// Cuerpo del flow.
    pub steps: Vec<Step>,
    /// Limpieza (cerrar la app, revocar permisos).
    #[serde(default)]
    pub teardown: Vec<Step>,
    /// Reintentos por aserción (0 = sin reintento).
    #[serde(default = "default_retries")]
    pub retries: u32,
    /// Timeout por paso.
    #[serde(default = "default_step_timeout_ms")]
    pub step_timeout_ms: u64,
    /// Directorio de artefactos (capturas, dumps, informes).
    #[serde(default)]
    pub artifacts_dir: String,
}

fn default_retries() -> u32 { 2 }
fn default_step_timeout_ms() -> u64 { 10_000 }

// ---------------------------------------------------------------------------
// Modelo del árbol de accesibilidad
// ---------------------------------------------------------------------------

/// Nodo del árbol de accesibilidad (`uiautomator dump`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UiNode {
    pub class: String,
    pub package: String,
    pub resource_id: String,
    pub text: String,
    pub content_desc: String,
    pub clickable: bool,
    pub enabled: bool,
    pub checked: bool,
    pub selected: bool,
    pub focused: bool,
    pub scrollable: bool,
    /// `[left, top, right, bottom]` en píxeles de pantalla.
    pub bounds: [i32; 4],
    pub children: Vec<UiNode>,
}

impl UiNode {
    /// Centro del nodo (punto de toque).
    pub fn center(&self) -> (i32, i32);
    /// Recorre el árbol en profundidad.
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a UiNode));
    /// Primer nodo que satisface el selector.
    pub fn find(&self, sel: &Selector) -> Option<&UiNode>;
    /// Todos los nodos que satisfacen el selector.
    pub fn find_all(&self, sel: &Selector) -> Vec<&UiNode>;
}

// ---------------------------------------------------------------------------
// Driver: la frontera con el dispositivo
// ---------------------------------------------------------------------------

/// Todo lo que el motor necesita del dispositivo. La implementación real
/// (`AdbUiDriver`) habla el ADB nativo de `crate::adb_client`.
pub trait UiDriver {
    /// Árbol de accesibilidad actual (`uiautomator dump`).
    fn dump_ui(&mut self) -> Result<UiNode, String>;
    /// Toca un punto absoluto.
    fn tap(&mut self, x: i32, y: i32) -> Result<(), String>;
    /// Escribe texto (`input text`).
    fn input_text(&mut self, text: &str) -> Result<(), String>;
    /// Gesto lineal.
    fn swipe(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, duration_ms: u32) -> Result<(), String>;
    /// `input keyevent`.
    fn key(&mut self, keycode: &str) -> Result<(), String>;
    /// Captura de pantalla.
    fn screenshot(&mut self, path: &str) -> Result<(), String>;
    /// Lanza la app.
    fn launch(&mut self, package: &str) -> Result<(), String>;
    /// Invoca una tool del registro (`app_control`, `adb`, ...).
    fn call_tool(&mut self, tool: &str, args: &Value) -> Result<Value, String>;
}

/// Driver sobre el ADB nativo (`crate::adb_client::Adbd`) y el shell root
/// persistente (`crate::persistent_shell`).
pub struct AdbUiDriver {
    shell: std::sync::Arc<std::sync::Mutex<crate::persistent_shell::PersistentShell>>,
    /// Si es `Some`, el driver usa el ADB nativo en vez del shell root.
    adbd: Option<crate::adb_client::Adbd>,
}

impl AdbUiDriver {
    pub fn new(shell: std::sync::Arc<std::sync::Mutex<crate::persistent_shell::PersistentShell>>) -> Self;
    /// Conecta además el ADB nativo (`adb_client::Adbd::connect_authenticated`).
    pub fn with_adbd(mut self, adbd: crate::adb_client::Adbd) -> Self;
}

// ---------------------------------------------------------------------------
// Planificador
// ---------------------------------------------------------------------------

/// Estado de un paso.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus { Ok, Failed, Skipped }

/// Resultado de un paso, con evidencia.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepOutcome {
    pub index: usize,
    pub step: Step,
    pub status: StepStatus,
    /// Duración real en ms.
    pub elapsed_ms: u64,
    /// Número de intentos (incluye reintentos).
    pub attempts: u32,
    /// Motivo del fallo (texto corto, sin credenciales).
    pub error: Option<String>,
    /// Artefactos generados por el paso (rutas).
    pub artifacts: Vec<String>,
}

/// Resultado de un flow completo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowReport {
    pub flow: String,
    pub package: String,
    pub ok: bool,
    pub steps: Vec<StepOutcome>,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: u64,
}

/// El planificador: resuelve pasos contra el árbol y decide reintentos.
pub struct Planner {
    /// Espera mínima entre polls del árbol (evita el spin-loop).
    pub poll_interval_ms: u64,
    /// Tope de polls por paso (⇒ timeout efectivo).
    pub max_polls: u32,
    /// Backoff entre reintentos de aserción.
    pub retry_backoff_ms: u64,
}

impl Default for Planner {
    fn default() -> Self {
        Planner { poll_interval_ms: 150, max_polls: 66, retry_backoff_ms: 250 }
    }
}

impl Planner {
    /// Resuelve un selector contra un árbol (delegado en `UiNode::find`).
    pub fn resolve<'a>(&self, root: &'a UiNode, sel: &Selector) -> Option<&'a UiNode>;

    /// Ejecuta un flow completo sobre un driver.
    ///
    /// Coste en el móvil: 1 `uiautomator dump` por poll, 0 procesos nuevos por
    /// paso si el driver usa el shell persistente.
    pub fn run(&self, flow: &Flow, driver: &mut dyn UiDriver) -> FlowReport;

    /// Ejecuta un step suelto (útil desde la tool MCP `qa_step`).
    pub fn run_step(&self, step: &Step, driver: &mut dyn UiDriver) -> StepOutcome;
}

/// Parsea un flujo desde JSON (el formato de `Flow`).
pub fn parse_flow(json: &str) -> Result<Flow, String>;
/// Parsea un flujo desde YAML tipo Maestro (subconjunto: `appId`, `steps`).
/// Requiere un parser YAML ⇒ **NO VERIFICADO**: `Cargo.toml` no declara
/// `serde_yaml` y crates.io no es accesible desde el sandbox del build
/// (ver `docs/APK_ANALYZER_SPEC.md` §1). Alternativa sin dependencias: aceptar
/// el flow en JSON y convertir desde YAML en el PC del usuario.
pub fn parse_flow_yaml(text: &str) -> Result<Flow, String>;
```

### 4.4 Integración con el ADB nativo (`adb_client.rs`)

`adb_client::Adbd` (`rust-core/src/adb_client.rs:583`) ya da `shell(cmd, max_bytes)` con timeout
y tope de bytes, y `push_file`/`pull_file` por el protocolo SYNC. El `AdbUiDriver` se monta encima:

| Necesidad del motor | Primitiva existente | Notas |
|---|---|---|
| `uiautomator dump` | `Adbd::shell("uiautomator dump /data/local/tmp/megadaemon/ui.xml && cat …", 256*1024)` | El XML se parsea con un lector propio de XML ligero (o se reusa el lector AXML si se generaliza; **NO VERIFICADO**: `apk_inventory::manifest_axml` está acoplado al AXML binario, no sirve tal cual para el XML de texto). |
| `input tap` | `Adbd::shell("input tap X Y")` | Alternativa: `android::screen_and_ui("tap", …)` (`android.rs:695`), que usa el shell root. |
| `input text` | `Adbd::shell("input text …")` | `android.rs:731` ya hace el escape de espacios a `%s`. |
| `screencap` | `Adbd::shell("screencap -p …")` + `Adbd::pull_file` | `android.rs:680` escribe la captura en el dispositivo; `pull_file` (`adb_client.rs:797`) la baja. |
| Lanzar la app | `Adbd::shell("monkey -p … -c android.intent.category.LAUNCHER 1")` | `android.rs:655` ya lo hace. |
| Tool arbitraria | `AdbUiDriver::call_tool` → `crate::mcp_ext::dispatch` o `crate::android::dispatch` | Permite `Step::Call { tool: "app_control", … }` para permisos y appops. |

**Rendimiento**: un flow de 30 pasos con 3 assertions puede generar ~50 `uiautomator dump`. Con el
shell persistente, cada dump es **una orden** (sin `su` nuevo), y con `adb_client` es un
`shell:` por el socket ya abierto. El presupuesto de `agent_loop` (`max_shell_spawns_per_tick`) no
aplica porque el motor no crea procesos: reutiliza el shell.

### 4.5 Integración con el MCP (tool `qa_run`)

La tool se declara en `rust-core/src/mcp_ext.rs` (catálogo) y se despacha en `mcp_ext::dispatch`
(`mcp_ext.rs:345`), siguiendo el patrón exacto del resto de tools del catálogo:

```json
{
  "name": "qa_run",
  "description": "Ejecuta un flow de QA declarativo sobre el dispositivo (dump de accesibilidad, toques, escritura, aserciones con reintento) y devuelve el informe con artefactos. Modifica el estado de la app ⇒ exige confirm=true.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "flow":         { "type": "string",  "description": "Flow en JSON (formato Flow)." },
      "flow_path":    { "type": "string",  "description": "Ruta de un .json de flow ya presente en el dispositivo." },
      "artifacts_dir":{ "type": "string",  "description": "Directorio de artefactos (capturas, dumps, informe)." },
      "dry_run":      { "type": "boolean", "default": false, "description": "Sólo.planifica: no toca el dispositivo." },
      "confirm":      { "type": "boolean", "default": false }
    }
  }
}
```

Reglas heredadas del catálogo (`mcp_ext.rs:9-10`):

* `confirm: true` obligatorio para cualquier `qa_run` que ejecute pasos que muten la app.
* `dry_run: true` devuelve el plan (pasos resueltos, selectores sin resolver) sin tocar nada.
* El informe sale como `structuredContent` + `content[0].text` (JSON), como el resto de tools.

Superficie HTTP equivalente (para el cockpit y la UI web): `/api/qa_run` en
`rust-core/src/cockpit_server.rs`, siguiendo el patrón de `/api/entropy_scan`
(`cockpit_server.rs:1510`) y `/api/input` (`cockpit_server.rs:1590`).

Registro de capacidad (para que `capability::verify()` lo sepa):
`rust-core/src/capability.rs` → `Capability { id: "qa.flows", kind: Kind::Device, bins: &["uiautomator"], paths: &[], envs: &[], modules: &["qa_engine"] }`.
(`uiautomator` viene con el framework de Android; la comprobación real debe ser `which uiautomator`
o la presencia de `/system/bin/uiautomator`.)

### 4.6 Integración con el agente (`agent_loop.rs`)

El motor se expone como **acción de tool**, no como paso del bucle:

* `Action::Tool { name: "qa_run", args }` ya existe (`agent_loop.rs:566`) y `ToolExecutor`
  (`agent_loop.rs:725`) es el punto de extensión.
* El planificador **no** es agéntico por diseño: un flow es determinista. El bucle agéntico se usa
  para *generar* o *reparar* flows (Flow→Flow), no para ejecutarlos.
* El `FailureMemory` (`agent_loop.rs:204`) puede ser la memoria de fallos de QA: un `qa_run` que
  falla con "selector `btn_submit` no encontrado" alimenta `record()` y el bucle deja de repetir
  el mismo flow.

---

## 5. Harness de seguridad en Rust

Dos módulos nuevos, ambos **puros** (funciones que reciben datos y devuelven `serde_json::Value`),
porque el proyecto ya tiene ese patrón (`apk_inventory::parse_dumpsys_packages`,
`root_hide::detect_leaks`, `dex_decompile::analyze_obfuscation`): se testean sin móvil.

### 5.1 `rust-core/src/sec_audit.rs` — análisis estático de APK

```rust
//! Auditoría estática de seguridad de una APK. Sin red, sin dispositivo:
//! la APK entra, el informe sale.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Severidad de un hallazgo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity { Info, Low, Medium, High, Critical }

/// Un hallazgo con evidencia y remediación.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecFinding {
    /// Identificador estable del tipo de hallazgo (`SEC-EXPORTED-01`).
    pub id: &'static str,
    pub severity: Severity,
    pub title: String,
    /// Evidencia (valores **redactados** si son credenciales).
    pub evidence: Value,
    /// Qué hacer para corregirlo.
    pub remediation: Option<String>,
    /// Referencia (manual, drozer, MobSF, OWASP MASTG…).
    pub references: Vec<&'static str>,
}

/// Informe de auditoría estática.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecAuditReport {
    pub apk_path: String,
    pub sha256: String,
    pub package: Option<String>,
    pub findings: Vec<SecFinding>,
    /// Recuento por severidad.
    pub summary: Value,
    /// Qué se pudo y qué no se pudo comprobar (honestidad del informe).
    pub coverage: Vec<&'static str>,
}

/// Auditoría completa de una APK.
pub fn audit_apk(path: &str) -> SecAuditReport;

/// Auditoría de los bytes de una APK (para tests y para APK en memoria).
pub fn audit_apk_bytes(bytes: &[u8]) -> SecAuditReport;

// ---------------------------------------------------------------------------
// Comprobaciones del manifiesto
// ---------------------------------------------------------------------------

/// `android:allowBackup="true"` / `android:fullBackupContent` ausente.
pub fn check_backup(manifest: &Value) -> Vec<SecFinding>;
/// `android:usesCleartextTraffic="true"` o `networkSecurityConfig` permisivo.
pub fn check_cleartext_traffic(manifest: &Value) -> Vec<SecFinding>;
/// `android:debuggable="true"` / `android:testOnly="true"`.
pub fn check_debuggable(manifest: &Value) -> Vec<SecFinding>;
/// Permisos peligrosos (`READ_SMS`, `CAMERA`, `ACCESS_FINE_LOCATION`, …) sin
/// feature que los justifique.
pub fn check_dangerous_permissions(manifest: &Value) -> Vec<SecFinding>;
/// `<permission>` con `protectionLevel` distinto de `signature`.
pub fn check_custom_permissions(manifest: &Value) -> Vec<SecFinding>;
/// Esquemas de deep-link propios (`android:scheme="myapp"`) con `exported` implícito.
pub fn check_deep_links(manifest: &Value) -> Vec<SecFinding>;
/// `networkSecurityConfig` con `cleartextTrafficPermitted` o anclas que confían
/// en el CA del usuario.
pub fn check_network_security_config(manifest: &Value, res: &Value) -> Vec<SecFinding>;

// ---------------------------------------------------------------------------
// Comprobaciones de componentes
// ---------------------------------------------------------------------------

/// Componentes exportados sin permiso (reutiliza el análisis del manifiesto).
pub fn check_exported_components(manifest: &Value) -> Vec<SecFinding>;
/// `android:grantUriPermissions="true"` sin permiso de lectura/escritura.
pub fn check_grant_uri_permissions(manifest: &Value) -> Vec<SecFinding>;

// ---------------------------------------------------------------------------
// Comprobaciones del código
// ---------------------------------------------------------------------------

/// URLs embebidas (reutiliza `crate::app_intel::extract_endpoints`).
pub fn check_embedded_urls(dex_text: &str, assets_text: &str) -> Vec<SecFinding>;
/// Claves/tokens embebidos, siempre redactados
/// (reutiliza `crate::app_intel::extract_secrets` + `crate::dex_decompile::mask_secret`).
pub fn check_embedded_secrets(dex_text: &str, assets_text: &str) -> Vec<SecFinding>;
/// Detección de anti-root / anti-frida (reutiliza
/// `crate::dex_decompile::anti_root_findings`).
pub fn check_anti_analysis(dex: &crate::dex_decompile::DexFile) -> Vec<SecFinding>;
/// WebView inseguro (`addJavascriptInterface`, `setAllowFileAccess`,
/// `setAllowUniversalAccessFromFileURLs`) — búsqueda en el DEX.
pub fn check_webview_settings(dex: &crate::dex_decompile::DexFile) -> Vec<SecFinding>;
/// Uso de WebView con `loadUrl` sobre datos no validados.
pub fn check_webview_url_loading(dex: &crate::dex_decompile::DexFile) -> Vec<SecFinding>;

// ---------------------------------------------------------------------------
// Comprobaciones de las librerías nativas
// ---------------------------------------------------------------------------

/// Mitigaciones de un ELF: PIE, RELRO, canary, NX.
/// Reutiliza `crate::binwalk_die::analyze_binary_full` sobre la `.so` extraída.
pub fn check_native_mitigations(so_path: &str) -> Vec<SecFinding>;
/// Exportaciones JNI con nombre estándar (`Java_com_ejemplo_app_Clase_metodo`).
pub fn check_jni_exports(so_path: &str) -> Vec<SecFinding>;
/// `JNI_OnLoad` presente ⇒ registro dinámico de natives (revisar).
pub fn check_dynamic_jni_registration(so_path: &str) -> Vec<SecFinding>;
/// Strings de alto riesgo dentro de la `.so` (claves, URLs, tokens).
pub fn check_native_strings(so_path: &str) -> Vec<SecFinding>;
/// Entropía del binario y de sus assets (reutiliza `crate::entropy::scan_entropy`).
pub fn check_entropy(blob: &[u8], label: &str) -> Vec<SecFinding>;
```

**Lista de comprobaciones (con su referencia)**:

| # | Comprobación | Severidad | Referencia |
|---|---|---|---|
| S-01 | Componente `exported` sin `permission` | High | drozer `app.activity.info`; MobSF *Exported Activity* |
| S-02 | `ContentProvider` exportado sin `readPermission`/`writePermission` | High | drozer `app.provider.info`; MASTG MSTG-PLATFORM-1 |
| S-03 | `intent-filter` con `exported` implícito (API 31+ exige explícito) | Medium | Android 12 manifest requirements |
| S-04 | Deep-link con esquema propio y sin permiso | High | drozer *intent injection* |
| S-05 | `android:allowBackup="true"` | Medium | MASTG MSTG-STORAGE-8 |
| S-06 | `usesCleartextTraffic="true"` | Medium | MASTG MSTG-NETWORK-2 |
| S-07 | `networkSecurityConfig` que confía en el CA del usuario | High | MASTG MSTG-NETWORK-4 |
| S-08 | `android:debuggable="true"` / `testOnly="true"` en release | High | MASTG MSTG-CODE-2 |
| S-09 | Permiso peligroso sin feature que lo justifique | Medium | MobSF *Permission Analysis* |
| S-10 | `<permission>` con `protectionLevel="normal"`/`dangerous` | Medium | MASTG MSTG-PLATFORM-5 |
| S-11 | `grantUriPermissions="true"` sin permiso | Medium | MASTG MSTG-PLATFORM-6 |
| S-12 | URL embebida con credenciales (`https://user:pass@…`) | High | MASTG MSTG-CODE-4 |
| S-13 | Token/API key embebido (redactado en la evidencia) | High | MobSF *Secrets*; MASTG MSTG-STORAGE-3 |
| S-14 | Anti-root / anti-frida detectado en el DEX | Info | RASP detection (Parte 4 §1) |
| S-15 | `addJavascriptInterface` con objeto expuesto a JS | High | MASTG MSTG-PLATFORM-7 |
| S-16 | WebView con `AllowFileAccess` / `AllowUniversalAccessFromFileURLs` | High | MASTG MSTG-PLATFORM-7 |
| S-17 | `.so` sin PIE | Medium | MobSF *Binary Analysis* |
| S-18 | `.so` sin RELRO (o RELRO parcial) | Low | MobSF *Binary Analysis* |
| S-19 | `.so` sin stack canary | Low | MobSF *Binary Analysis* |
| S-20 | Export JNI con nombre estándar (superficie de llamada nativa) | Info | Ghidra (Parte 4 §2) |
| S-21 | `JNI_OnLoad` presente (registro dinámico) | Info | Ghidra (Parte 4 §2) |
| S-22 | Entropía alta en assets (payload cifrado/empaquetado) | Info | `entropy.rs` |

**Integración**: `sec_audit::audit_apk` es un **agregador**: consume `apk_inventory::analyze_apk`
(manifiesto, entradas, nativas), `dex_decompile` (DEX, hallazgos) y `app_intel` (endpoints, secretos)
y añade lo que falta (mitigaciones de las `.so`, `networkSecurityConfig`). No reimplementa nada de
ellos.

### 5.2 `rust-core/src/ipc_audit.rs` — superficie IPC

```rust
//! Auditoría de la superficie IPC de una app: Activities, Services,
//! BroadcastReceivers y ContentProviders exportados, y las pruebas activas
//! que se pueden hacer sin instalar nada.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::apk_inventory::ZipEntry;

/// Tipo de componente Android.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind { Activity, Service, Receiver, Provider }

/// Cómo queda exportado un componente.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportMode {
    /// `android:exported="true"` explícito.
    Explicit,
    /// Exportado implícitamente por un `<intent-filter>` (sin atributo).
    Implicit,
    /// `android:exported="false"`.
    NotExported,
    /// No se pudo determinar.
    Unknown,
}

/// Un `intent-filter` declarado.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntentFilter {
    pub actions: Vec<String>,
    pub categories: Vec<String>,
    pub data: Value,
    pub auto_verify: bool,
}

/// Un componente del manifiesto, normalizado.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportedComponent {
    pub kind: ComponentKind,
    pub name: String,
    pub export_mode: ExportMode,
    pub permission: Option<String>,
    pub read_permission: Option<String>,
    pub write_permission: Option<String>,
    pub authorities: Option<String>,
    pub grant_uri_permissions: bool,
    pub task_affinity: Option<String>,
    pub intent_filters: Vec<IntentFilter>,
    /// Para providers: `path`, `pathPrefix`, `pathPattern` declarados.
    pub path_rules: Vec<String>,
}

/// Informe de la auditoría IPC.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IpcAuditReport {
    pub package: String,
    pub components: Vec<ExportedComponent>,
    pub findings: Vec<SecFinding>,
    pub summary: Value,
    /// Pruebas activas ejecutadas (si se pidió `active_probe`).
    pub probes: Vec<ProbeResult>,
}

/// Resultado de una prueba activa (intento de invocación real).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeResult {
    pub component: String,
    pub kind: ComponentKind,
    /// Comando exacto que se ejecutó.
    pub command: String,
    /// Código de salida.
    pub exit_code: Option<i32>,
    /// Salida (recortada).
    pub output: String,
    /// Interpretación: `reachable`, `blocked`, `error`.
    pub verdict: &'static str,
}

// ---------------------------------------------------------------------------
// Extracción del manifiesto
// ---------------------------------------------------------------------------

/// Normaliza el manifiesto en componentes auditables.
/// Reutiliza el parser AXML de `crate::apk_inventory::manifest_axml`.
pub fn components_from_manifest(manifest: &Value) -> Vec<ExportedComponent>;
/// Calcula el `ExportMode` efectivo (explícito vs implícito por intent-filter).
pub fn effective_export_mode(attrs: &Value, has_filter: bool) -> ExportMode;

// ---------------------------------------------------------------------------
// Comprobaciones
// ---------------------------------------------------------------------------

/// Activity exportada alcanzable sin permiso.
pub fn check_exported_activities(components: &[ExportedComponent]) -> Vec<SecFinding>;
/// Service exportado (superficie de privilege escalation).
pub fn check_exported_services(components: &[ExportedComponent]) -> Vec<SecFinding>;
/// Receiver exportado con acción sensible (`BOOT_COMPLETED`, `SMS_RECEIVED`, …).
pub fn check_exported_receivers(components: &[ExportedComponent]) -> Vec<SecFinding>;
/// Provider exportado sin permiso de lectura o de escritura.
pub fn check_exported_providers(components: &[ExportedComponent]) -> Vec<SecFinding>;
/// Provider con `path`/`pathPrefix`/`pathPattern`: candidato a path traversal.
pub fn check_provider_path_traversal(components: &[ExportedComponent]) -> Vec<SecFinding>;
/// Provider sin `android:authorities` (colisión de authorities).
pub fn check_provider_authorities(components: &[ExportedComponent]) -> Vec<SecFinding>;
/// Inyección de intents: qué componentes aceptarían un intent con extras
/// controlados por un tercero.
pub fn check_intent_injection(components: &[ExportedComponent]) -> Vec<SecFinding>;
/// `PendingIntent` mutability: se busca en el DEX (`FLAG_MUTABLE`).
pub fn check_pending_intent_mutability(dex: &crate::dex_decompile::DexFile) -> Vec<SecFinding>;

// ---------------------------------------------------------------------------
// Pruebas activas (opcionales, P1)
// ---------------------------------------------------------------------------

/// Intenta alcanzar un componente exportado con `am start`.
///
/// El comando se **construye**, no se ejecuta: quien lo ejecuta decide si
/// autoriza (mismo criterio `confirm: true` del resto del proyecto).
pub fn probe_activity_command(package: &str, activity: &str, extras: &[(&str, &str)]) -> String;
/// Intenta leer un provider con `content query`.
pub fn probe_provider_query_command(authority: &str, path: &str) -> String;
/// Intenta leer un provider con una ruta traversal (control: `../../etc/hosts`).
pub fn probe_provider_traversal_command(authority: &str) -> String;

/// Ejecuta las pruebas activas contra el dispositivo (requiere `confirm`).
pub fn run_active_probes(
    components: &[ExportedComponent],
    package: &str,
    exec: &mut dyn FnMut(&str) -> Result<(Option<i32>, String), String>,
) -> Vec<ProbeResult>;
```

**Lista de comprobaciones IPC (referencia drozer / medusa)**:

| # | Comprobación | Severidad | Referencia |
|---|---|---|---|
| I-01 | Activity `exported` sin `permission` | High | drozer `run app.activity.info` |
| I-02 | Activity con `intent-filter` ⇒ exportada implícitamente | High | drozer `app.package.attacksurface` |
| I-03 | Activity alcanzable con un extra controlado (`url`, `intent`, `component`) ⇒ hijacking | High | drozer `app.activity.start --extra` |
| I-04 | Service `exported` sin `permission` | High | drozer `app.service.info` |
| I-05 | Receiver `exported` con acción sensible | Medium | medusa *intent* modules |
| I-06 | Provider `exported` sin `readPermission`/`writePermission` | High | drozer `app.provider.info` |
| I-07 | Provider con `pathPrefix`/`pathPattern` ⇒ candidato a traversal | High | drozer `scanner.provider.finduris` |
| I-08 | Provider sin `android:authorities` | Medium | MASTG MSTG-PLATFORM-1 |
| I-09 | `grantUriPermissions` sin permiso | Medium | MASTG MSTG-PLATFORM-6 |
| I-10 | `PendingIntent` creado con `FLAG_MUTABLE` | Medium | medusa *pendingintent*; manual Parte 4 §4 |
| I-11 | `taskAffinity` ausente en una Activity exportada ⇒ hijacking de tareas en multitarea | Low | MASTG MSTG-PLATFORM-8 |
| I-12 | ContentProvider con `query()` sin parametrizar ⇒ SQLi | High | manual Parte 4 §4 (recomendación defensiva) |

**Nota de alcance**: I-03, I-07 e I-12 tienen una parte **activa** (invocar el componente). El
diseño las separa en dos fases: `ipc_audit::components_from_manifest` +
`check_*` (estático, sin riesgo, siempre activo) y `run_active_probes` (activo, con `confirm`,
sólo contra apps propias o dentro de un laboratorio). La función que **construye** el comando
(`probe_*_command`) es pura y testeable; la que lo **ejecuta** recibe un `exec` inyectado.

### 5.3 Integración de los dos módulos con lo existente

| Consumidor | Punto de integración |
|---|---|
| MCP | `mcp_ext::tool_definitions` → tool `sec_audit` (args: `apk_path`, `checks`) y `ipc_audit` (args: `package` o `apk_path`, `active: bool`, `confirm: bool`) |
| Cockpit HTTP | `/api/sec_audit` y `/api/ipc_audit` en `cockpit_server.rs` (patrón de `/api/entropy_scan`) |
| Registro de capacidades | `capability.rs` → `Capability { id: "security.static_audit", kind: Kind::Reverse, modules: &["sec_audit", "ipc_audit"] }` |
| Web UI | `webui.rs`: sección "Seguridad" con el informe (el `webui.rs` ya tiene 5 005 líneas y su propio patrón de render) |
| Verificador | `capability::verify()` debe comprobar que el APK existe y que `uiautomator` está presente, no que "el análisis funciona" |

---

## 6. Intercepción de tráfico

### 6.1 Qué hay ya y qué falta

| Pieza | Estado | Dónde |
|---|---|---|
| Hook de Frida sobre OkHttp | **EXISTE** | `rust-core/assets/frida_chrome_cookies.js:105-133` (`BridgeInterceptor.intercept`) |
| Hook de Frida sobre `java.net.URL.openConnection` | **EXISTE** | `rust-core/assets/frida_chrome_cookies.js:77-81` |
| Hook del gestor de cookies de Chromium | **EXISTE** | `rust-core/assets/frida_chrome_cookies.js:51-75` |
| Gestión del proceso Frida en el dispositivo | **EXISTE** | `rust-core/src/call_recorder.rs:416-429` (`frida -U -n <pkg> -l <script>`) |
| Generador de scripts Frida | **EXISTE** | `rust-core/src/frida_dbi.rs:71` |
| Ingesta NDJSON + índice + búsqueda | **EXISTE** | `rust-core/src/sync.rs:403,479` |
| Transporte ADB para mover artefactos | **EXISTE** | `rust-core/src/adb_client.rs:721,797` (`push_file`/`pull_file`) |
| **Agente Frida con `rpc.exports`** | **AUSENTE** | — |
| **Relé entre el agente y una herramienta de proxy** | **AUSENTE** | — |
| **Proxy / MITM propio** | **AUSENTE** | — |

### 6.2 El problema que resuelve Brida y por qué el diseño proposed lo replica sin Pyro4

Brida (MIT, `federicodotta/Brida`) es una **extensión de Burp Suite en Java** que:

1. levanta un **servidor Pyro4** en el PC;
2. el **agente Frida** en el móvil se conecta a ese servidor y publica sus `rpc.exports`
   (`encryptpayload`, `decryptpayload`, …);
3. la extensión de Burp llama a esas funciones desde Repeater/Intruder para **mutar el plaintext**
   antes de que la app lo cifre.

El problema: **Pyro4 es Python**, y el proyecto es "Rust puro" sin Python en el móvil (el único
Python es el extractor embebido de `sync.rs`, `assets/sync_extract.py`). Meter Pyro4 en el
proyecto contradice la restricción de diseño. La propuesta es **replicar el patrón de Brida con un
transporte JSON-RPC trivial**, que es lo que Brida hace por debajo.

### 6.3 Diseño del puente (propuesta, NO implementado)

Tres piezas nuevas:

| Pieza | Ruta propuesta | Función |
|---|---|---|
| Agente Frida con exports | `rust-core/assets/frida_rpc_exports.js` | Publica `rpc.exports` con los ganchos de cifrado/descifrado y los hooks de red existentes, **reutilizando** `hookOkHttp` y `hookJavaNet` de `frida_chrome_cookies.js` (se extraen a un módulo común `assets/frida_net_hooks.js`). |
| Relé Rust | `rust-core/src/brida_bridge.rs` | Arranca el agente en el móvil (`frida -U -n <pkg> -l <script>`), lee su salida **NDJSON por stdout**, mantiene la tabla de exports disponibles y expone un endpoint local. |
| Extensión Burp (mínima) | `burp/brida-lite/` (repo aparte, no en este monorepo) | Cliente JSON-RPC del relé + botón "llamar export" en Repeater/Intruder. |

**Contrato del relé** (Rust):

```rust
//! Puente Burp ↔ Frida sin Pyro4: JSON-RPC sobre HTTP local.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Una función publicada por el agente Frida (`rpc.exports`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Export {
    /// Nombre en `rpc.exports` (`encryptpayload`).
    pub name: String,
    /// Tipos de argumento aceptados.
    pub args: Vec<String>,
    /// Si es `true`, muta el tráfico (el puente exige `confirm`).
    pub mutating: bool,
}

/// El puente.
pub struct BridaBridge {
    /// Host donde se expone el endpoint local (`127.0.0.1` por defecto).
    pub bind: String,
    /// Puerto del endpoint JSON-RPC.
    pub port: u16,
    /// Token compartido con la extensión Burp (nunca en disco en claro).
    pub token: Option<String>,
}

impl BridaBridge {
    /// Arranca el agente en el móvil y lee sus exports.
    pub fn start(&mut self, package: &str, script: &str) -> Result<Vec<Export>, String>;

    /// Exports disponibles.
    pub fn exports(&self) -> Vec<Export>;

    /// Invoca un export con argumentos JSON.
    /// Si el export es `mutating` ⇒ exige `confirm = true`.
    pub fn call(&self, name: &str, args: Vec<Value>, confirm: bool) -> Result<Value, String>;

    /// Responde a una petición JSON-RPC del proxy.
    pub fn handle_rpc(&self, req: &Value) -> Value;
}
```

**Punto de montaje**: `/api/brida/call` en `cockpit_server.rs`, **protegido por el token del
cockpit** (el cockpit ya es fail-closed: exige `X-Seere-Auth-Token` en todo `/api/*`, ver
`docs/README.md` §"Seguridad por defecto"). La extensión de Burp habla con el cockpit por el
túnel (`channel.rs`), de modo que el proxy y el móvil se ven aunque estén en redes distintas.

**Camino de datos completo**:

```
App en el móvil
  └─ Frida agent (assets/frida_rpc_exports.js)
       │  NDJSON por stdout: {"__frida__":true,"export":"encryptpayload","args":[...],"result":...}
       ▼
rust-core/src/brida_bridge.rs  (lee stdout, mantiene tabla de exports)
       │  HTTP JSON-RPC en localhost:<puerto>  (+ token)
       ▼
Extensión Burp "brida-lite"  (Repetidor/Intruder)
```

**Reutilización de `sync.rs`**: cada llamada al bridge (petición interceptada, payload cifrado,
respuesta descifrada) se puede anexar como un registro NDJSON en el almacén de sesiones
(`sync::ingest_conversations` sirve de patrón), de modo que el tráfico capturado queda
**buscable** con `sync::search` sin escribir un almacén nuevo.

### 6.4 Alternativa más simple (sin Burp)

Si el objetivo es **auditar** y no **fuzzear**, el proxy puede ser `mitmproxy` (MIT) corriendo en
el PC del usuario con su API Python, y MegaDaemon habla con él por HTTP:

* el móvil enruta su tráfico al PC por el túnel (`channel.rs`),
* el PC corre `mitmproxy` con un addon mínimo que reenvía a `/api/brida/call`,
* el resultado se guarda en el NDJSON de `sync.rs`.

Es menos potente que Brida (no hay Intruder) pero es **más barato** y cubre el caso
"auditar el tráfico de mi cuenta".

### 6.5 Qué NO hace este diseño

* No descifra material protegido por la app: sólo llama a las funciones **de la propia app**, que
  es exactamente el truco de Brida (y funciona porque la app debe poder descifrar).
* No falsifica respuestas del servidor: muta **peticiones** del cliente, como Brida.
* No se ejecuta sin `confirm: true` cuando el export muta tráfico (política del catálogo MCP,
  `mcp_ext.rs:9-10`).

---

## 7. Plan de fases

| Fase | Contenido | Esfuerzo | Depende de |
|---|---|---|---|
| F0 | `sec_audit` como agregador de lo que ya existe (sin comprobaciones nuevas) | S | — |
| F1 | `ipc_audit` estático (componentes, export mode, permisos) | M | F0 |
| F2 | `qa_engine`: `Selector`, `Assertion`, `Step`, `Flow`, `Planner`, `AdbUiDriver` | M | — |
| F3 | Tool MCP `qa_run` + `/api/qa_run` + entrada en el registro de capacidades | S | F2 |
| F4 | `sec_audit`: mitigaciones de `.so`, `networkSecurityConfig`, WebView | M | F0 |
| F5 | `ipc_audit` activo (`run_active_probes` con `confirm`) | M | F1 |
| F6 | `brida_bridge` + `frida_rpc_exports.js` + extensión Burp mínima | M | — |
| F7 | Formato YAML tipo Maestro (si se resuelve la dependencia `serde_yaml`) | S | F2 |
| F8 | Capa de protocolo sobre el canal LAN existente: emparejamiento + mensajes tipados (inspirada en la spec de KDE Connect) | M | F2 |

**P0 = F0+F1+F2+F3+F4.** Es el bloque que convierte lo que ya está disperso (`apk_inventory`,
`dex_decompile`, `android::screen_and_ui`) en **dos productos**: un escáner de seguridad y un
runner de QA, ambos con informe JSON.

---

## 8. Lo que queda `NO VERIFICADO`

| Afirmación | Por qué no se pudo verificar |
|---|---|
| `serde_yaml` (o equivalente) disponible para `Cargo.toml` | `crates.io` no es accesible desde el sandbox del build (constante en `docs/APK_ANALYZER_SPEC.md` §1). El formato YAML de Maestro queda sin parser hasta que se resuelva. |
| El XML de `uiautomator dump` se puede parsear reutilizando `apk_inventory::manifest_axml` | `manifest_axml` está acoplado al formato AXML **binario**, no al XML de texto. Hay que escribir (o portar) un lector de XML ligero. |
| Versiones concretas de Maestro, MobSF, drozer, Brida, etc. | No se han consultado tags ni releases; el triage documenta **licencia y lenguaje**, no versión. |
| Que `uiautomator` esté disponible en el Samsung S21 Ultra con la ROM actual | No verificado en el dispositivo desde este entorno. `capability::verify()` lo comprobaría en runtime. |
| Comportamiento de `frida -U -n <pkg>` cuando el proceso ya está en foreground | El uso existente (`call_recorder.rs:416-429`) lo da por válido; no se ha medido en este entorno. |
| Coste real en RAM/tiempo de un flow de 50 pasos con 10 `uiautomator dump` | Sin medición en dispositivo. |
| Tamaño real del `uiautomator dump` en una app con WebView densa | El tope actual en `android.rs:689` es 200 000 caracteres; podría truncarse. |

---

## 9. Referencias internas del repositorio

| Ruta | Relación con este documento |
|---|---|
| `rust-core/src/apk_inventory.rs` | Insumo principal de `sec_audit` e `ipc_audit` |
| `rust-core/src/dex_decompile.rs` | Hallazgos de credenciales, anti-root, ofuscación |
| `rust-core/src/apk_deobfuscator.rs` | Grafo de llamadas y roles de clase (heredado del manual Parte 4) |
| `rust-core/src/app_intel.rs` | Endpoints y secretos (redacción incluida) |
| `rust-core/src/root_hide.rs` | Auditoría de root hiding / RASP |
| `rust-core/src/binwalk_die.rs` | Mitigaciones PIE/RELRO/canary |
| `rust-core/src/entropy.rs` | Entropía por bloques |
| `rust-core/src/adb_client.rs` | Transporte ADB nativo (base del driver del motor de QA) |
| `rust-core/src/persistent_shell.rs` | Shell root persistente (coste de proceso del motor de QA) |
| `rust-core/src/android.rs` | Primitivas de UI (`dump`, `tap`, `swipe`, `key`, `text`, `screenshot`) |
| `rust-core/src/frida_dbi.rs` | Generación de hooks y análisis con Rizin |
| `rust-core/assets/frida_chrome_cookies.js` | Hooks OkHttp / `openConnection` / cookies |
| `rust-core/src/sync.rs` | Ingesta NDJSON (almacén del tráfico interceptado) |
| `rust-core/src/mcp_ext.rs` | Catálogo MCP donde se declararía `qa_run`, `sec_audit`, `ipc_audit` |
| `rust-core/src/capability.rs` | Registro de capacidades (`verify()` en el dispositivo) |
| `rust-core/src/cockpit_server.rs` | Superficie HTTP (`/api/qa_run`, `/api/sec_audit`, `/api/ipc_audit`) |
| `rust-core/src/lan.rs` | Canal LAN por beacon UDP (base sobre la que se construiría el canal KDE Connect) |
| `docs/APK_ANALYZER_SPEC.md` | Especificación sister del escáner de APK (formato y tono) |
| `docs/REPO_TRIAGE.md` | Triage de los repos del manual (decisiones de adopción) |

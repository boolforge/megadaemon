# Triage de repositorios de referencia (`docs/REPO_TRIAGE.md`)

> Documento: `docs/REPO_TRIAGE.md`
> Origen de la lista: la parte 5 del *Manual de referencia* de QA móvil y evaluación de seguridad
> (`/workspace/attachments/.../Mobile QA Automation and Security Assessment Technical Manual.md`,
> 600 líneas) + las referencias de la parte 6.
> Base de código: `boolforge/megadaemon` @ `c757542` (el encargo se escribió sobre `c7c0e2c`; mientras
> se elaboraba el triage `origin/main` avanzó con el **canal LAN** — beacon UDP en `rust-core/src/lan.rs` —
> y esa pieza se ha tenido en cuenta en la decisión nº1).
> Versión del documento: 1.0 · Idioma del documento: español, identificadores de código en inglés.

---

## 0. Método y reglas de honestidad

Cada fila se ha verificado **leyendo la página del repositorio** (README) y el **fichero
`LICENSE`** correspondiente. Cuando el `LICENSE` existe, la licencia que se anota es la del
fichero; cuando sólo existe un *badge* en el README, se anota el badge y se dice. Cuando no se
pudo determinar, la celda dice **`NO VERIFICADO`** y nunca se supone nada.

Convenciones de la columna "Licencia":

* `VERIFICADO` = leído en el fichero `LICENSE`/`COPYING` del repo (o en la API oficial de GitHub
  cuando el fichero está fuera del `main`/`master` habitual).
* `NO VERIFICADO` = no hay fichero de licencia en la raíz **y** no hay badge ni mención en el
  README ⇒ **sin licencia implícita** (reservados todos los derechos).
* `badge README` = la licencia sólo consta en una insignia del README, sin fichero de licencia.

Los recursos web (no repos) se tratan como recurso web: `portswigger.net/burp`,
`mitmproxy.org`, `projectmaxs.org/homepage/`, la spec de KDE Connect y los dos hilos de issues.

**Acciones posibles** (columna "Acción concreta"):

| Acción | Significado |
|---|---|
| *adoptar como dependencia* | Copiar el código o usar el crate/binary en el proyecto (requiere licencia compatible) |
| *portar la idea a Rust* | No se copia el código: se reimplementa el **concepto** en Rust puro, sin dependencia externa |
| *integrar vía API* | Se usa el servicio/proyecto como **herramienta externa** (Burp, mitmproxy), no como dependencia de compilación |
| *documentar como referencia* | Se cita en la documentación de diseño; no entra en el código |
| *descartar* | No aporta al alcance de MegaDaemon |

---

## 1. Triage completo

### 1.1 Utilidades open-source de automatización Android (parte 4 del manual)

| Repo | URL | Qué es | Licencia | Lenguaje | Relevancia | Acción concreta | Dónde aterriza | Prioridad |
|---|---|---|---|---|---|---|---|---|
| moody-routine | https://github.com/naman-chopra/moody-routine | Clone de los "Modes & Routines" de One UI de Samsung: automatizaciones por contexto (hora, ubicación) sin root | GPL-3.0 (fichero `LICENSE`) | Kotlin | MEDIA-BAJA | portar la idea a Rust | `qa_engine.rs` (`Step::Call` → `dumpsys location` como disparador) | P2 |
| macrion | https://github.com/vibhor1102/macrion | Automatización de toques y swipes: repite una secuencia o espera a que aparezca un elemento en pantalla | GPL-3.0 (fichero `LICENSE`) | Kotlin | **ALTA** | portar la idea a Rust | `qa_engine.rs`: un `Flow` **es** una macro; mismo motor, dos usos | P1 |
| vokibot | https://github.com/vokimon/vokibot | Bot de automatización: el dispositivo ejecuta acciones cuando ocurre un disparador | AGPL-3.0 (fichero `LICENSE`) | Kotlin | MEDIA-BAJA | portar la idea a Rust | `service_bus.rs` (disparadores → acciones) | P2 |
| hermes-agent | https://github.com/adybag14-cyber/hermes-agent | App independiente para modelos de IA locales y utilidades de dispositivo (telemetría) | MIT (fichero `LICENSE`) | Python | REFERENCIA | documentar como referencia | — (ya cubierto por `env_introspect.rs` + `capability::verify()`) | P2 |
| Tideo-Auto-Brightness | https://github.com/faded-penguin021/Tideo-Auto-Brightness | Port nativo (Kotlin/Compose) de "Advanced Auto Brightness": brillo automático determinista en vez de ML opaco | MIT (fichero `LICENSE`) | Kotlin | REFERENCIA | documentar como referencia | — | P2 |
| cookies-extractor | https://github.com/paoloantinori/cookies-extractor | Extrae **las cookies propias** de una web tras iniciar sesión en un WebView de la propia app | Apache-2.0 (fichero `LICENSE`) | Kotlin | REFERENCIA | documentar como referencia | Ya **superado**: `assets/frida_chrome_cookies.js` extrae las cookies del proceso real (Chromium + OkHttp + `openConnection`), que es estrictamente más potente | P2 |
| gpslogger | https://github.com/mendhak/gpslogger | App ligera de registro GPS con varios formatos de salida y subida (SFTP, Drive, Dropbox, email) | GPL-2.0 (fichero `LICENSE.md`) | Java | MEDIA-BAJA | portar la idea a Rust | `sync.rs` (NDJSON) + `battery_advanced.rs` (lectura de sensores) | P2 |
| AppListBackup | https://github.com/AndroidLabs-org/AppListBackup | Genera listas de backup de las apps instaladas | GPL-3.0 (fichero `LICENSE`) | Kotlin | REFERENCIA | documentar como referencia | Ya **superado**: `apk_inventory::inventory` (rutas, SHA-256, deriva) | — |
| TaskerRsync | https://github.com/ribbons/TaskerRsync | Plugin de Tasker para lanzar `rsync` por SSH | GPL-3.0 (fichero `LICENSE.txt`) | Kotlin | REFERENCIA | documentar como referencia | Ya cubierto: `drive_client.rs` (subida por trozos) + `backup_formats.rs` | P2 |
| SU_IMD | https://github.com/soul-99/SU_IMD | Gestor y **ocultador** de ajustes y servicios (fork potenciado de Geto) para apps restrictivas | GPL-3.0 (fichero `LICENSE`) | Kotlin | **ALTA** | portar la idea a Rust | `root_hide.rs` + `root_stealth.rs`: modelo "ocultar estado y.appops con `confirm`" | P1 |
| RebootNya | https://github.com/daisukiKaffuChino/RebootNya | Utilidad de reinicio con soporte Root y Shizuku | Apache-2.0 (fichero `LICENSE`) | Kotlin | MEDIA-BAJA | portar la idea a Rust | `android::app_control` + `battery_advanced.rs` (plan reversible + `confirm`) | P2 |
| knockonports | https://github.com/impalex/knockonports | Cliente de *port knocking* para servidores remotos (proyecto sin mantenimiento) | Apache-2.0 (fichero `LICENSE`) | Kotlin | REFERENCIA | descartar | — (ajeno al harness) | P2 |
| gama | https://github.com/palincat/gama | Cambia la API de renderizado del dispositivo entre Vulkan y OpenGL (backend root o sin root) | MIT (fichero `LICENSE.txt`) | Kotlin | MEDIA-BAJA | portar la idea a Rust | `npu_vs4l.rs` / `battery_advanced.rs`: escenario de estrés para re-ejecutar flows | P2 |
| MethodCall | https://github.com/coleblvck/MethodCall | Automatiza el móvil a partir de llamadas entrantes (lanzar app, SMS, brillo, NFC…) | GPL-3.0 (fichero `LICENSE`) | Kotlin | MEDIA-BAJA | portar la idea a Rust | `call_recorder.rs:582` (detecta llamada activa) + `messaging.rs` | P2 |
| ulogger-android | https://github.com/bfabiszewski/ulogger-android | Cliente de registro y publicación de tracks GPS contra un servidor propio | GPL-3.0 (fichero `COPYING`) | Java | MEDIA-BAJA | portar la idea a Rust | `sync.rs` (NDJSON + índice) | P2 |
| android-usb-script | https://github.com/Netdex/android-usb-script | App Android con interfaz Lua para crear dispositivos USB compuestos arbitrarios | MIT (fichero `LICENSE`) | Java + Lua | REFERENCIA | documentar como referencia | `adb_client.rs` (`push_file`/`pull_file` por el protocolo SYNC) | P2 |
| MqttDroid | https://github.com/LightJockey/MqttDroid | Cliente MQTT integrado en los controles de dispositivo de Android 11+ | GPL-2.0 (fichero `LICENSE`) | Kotlin | MEDIA-BAJA | portar la idea a Rust | `service_bus.rs` (transporte de eventos) | P2 |
| sms-ie | https://github.com/tmo1/sms-ie | Importa/exporta SMS/MMS, registro de llamadas, contactos y bloqueados en (ND)JSON. Sin root | GPL-3.0 (fichero `LICENSE`) | Kotlin | **ALTA** | portar la idea a Rust | `sync.rs`: formato de intercambio NDJSON **idéntico** al que ya usa el daemon | P1 |
| sms-telebot | https://github.com/teslor/sms-telebot | Reenvía SMS, llamadas, notificaciones push, batería y cargador a Telegram / ntfy / SMTP | AGPL-3.0 (fichero `LICENSE`) | Dart (Flutter) | **ALTA** | portar la idea a Rust | `messaging.rs` (bus de mensajes) + `sync.rs` (registro NDJSON) | P1 |
| otphelper | https://github.com/jd1378/otphelper | Copia automáticamente códigos OTP de SMS y notificaciones | AGPL-3.0 (fichero `LICENSE`) | Kotlin | MEDIA-BAJA | portar la idea a Rust | `messaging.rs` (lectura de notificaciones propias) | P2 |

### 1.2 Control remoto de dispositivos (MAXS y KDE Connect)

| Repo | URL | Qué es | Licencia | Lenguaje | Relevancia | Acción concreta | Dónde aterriza | Prioridad |
|---|---|---|---|---|---|---|---|---|
| projectmaxs.org/homepage | http://projectmaxs.org/homepage/ | Sitio del proyecto MAXS: control remoto de Android mediante transportes **desacoplados** (módulos ⇄ transportes) | GPLv3 (declarado en el sitio) | Web (el motor es Java) | **ALTA** | portar la idea a Rust | `channel.rs` + `service_bus.rs` + `mesh.rs`: la **desacoplamiento módulo/transporte** ya está; lo que no se adopta es XMPP | P1 |
| ProjectMAXS/maxs | https://github.com/ProjectMAXS/maxs | Núcleo de MAXS (Modular Android XMPP Suite): módulos de comandos/notificaciones + transporte XMPP | GPL-3.0 (fichero `COPYING`) | Java | MEDIA-BAJA | portar la idea a Rust | `service_bus.rs` (módulos) + `channel.rs` (transportes) | P1 |
| MaxManager | https://github.com/nader295/MaxManager | Panel de control de rendimiento y energía para Android con root, como módulo Magisk/KernelSU | **Proprietary** (fichero `LICENSE`: "MaxManager — Proprietary License"; requiere permiso escrito) | Kotlin + Rust | REFERENCIA | documentar como referencia | — (**no se puede adoptar**: licencia propietaria) | — |
| exoskeletonzj/MAXS | https://github.com/exoskeletonzj/MAXS | ** namesake**: un algoritmo de decodificación para agentes LLM con *lookahead planning*. No es el MAXS de Android | `NO VERIFICADO` (sin fichero de licencia en la raíz) | Python | REFERENCIA | descartar | — (falso positivo de nombre en la lista del manual) | — |
| kdeconnect-kde | https://github.com/kde/kdeconnect-kde | Aplicación de escritorio de KDE Connect: emparejamiento y control remoto del móvil | GPL-2.0-only / GPL-3.0-only (REUSE: directorio `LICENSES/`) | C++ | **ALTA** | portar la idea a Rust | `channel.rs` → nuevo `kde_channel.rs` | **P0** |
| kdeconnect-android | https://github.com/kde/kdeconnect-android | Cliente Android de KDE Connect: portapapeles, notificaciones, ficheros, mando a distancia, SMS | GPL-2.0 (fichero `COPYING`) | Java | **ALTA** | portar la idea a Rust | `kde_channel.rs` (mismo protocolo, implementación Rust) | **P0** |
| KDE/kdeconnect-meta (protocol.md) | https://github.com/KDE/kdeconnect-meta/blob/work/protocol-schemas/protocol.md | **Especificación del protocolo KDE Connect**: paquetes JSON (tipo `kdeconnect.<plugin>`), identidad en UDP + TCP, pairing, `deviceId` como CN del certificado TLS, payload aparte | `NO VERIFICADO` (recurso web; la spec se genera desde JSON Schemas y se declara que no es una especificación formal) | Markdown | **ALTA** | portar la idea a Rust | `kde_channel.rs`: el **canal LAN que no depende de Internet** | **P0** |

### 1.3 Integraciones de SMS y mensajería

| Repo | URL | Qué es | Licencia | Lenguaje | Relevancia | Acción concreta | Dónde aterriza | Prioridad |
|---|---|---|---|---|---|---|---|---|
| nextcloud/ocsms#176 | https://github.com/nextcloud/ocsms/issues/176 | Hilo de issue sobre **normalización de números telefónicos** al indexar SMS importados desde Android (evita duplicar `sms_address` y conversación) | Repo: AGPL-3.0 (API oficial de GitHub). El hilo es un recurso web | Repo: JavaScript + PHP + CSS + Shell | **ALTA** | portar la idea a Rust | `messaging.rs`: normalizar a E.164 **antes** de indexar | P1 |
| tijder/SmsMatrix#14 | https://github.com/tijder/SmsMatrix/issues/14 | Hilo de issue del puente SMS↔Matrix: mismo problema de normalización (`0612345678` vs `+31612345678` crean dos salas) | Repo: GPL-3.0 (fichero `LICENSE`). El hilo es un recurso web | Repo: Java | **ALTA** | portar la idea a Rust | `messaging.rs`: paso de canonicalización de números antes de crear/consultar la sala | P1 |

> **Hallazgo del triage**: el manual presenta estos dos hilos como "referencia arquitectónica de un
> puente SMS", pero leídos, **ambos son un bug de normalización de números telefónicos**, no una
> arquitectura de puente. Eso cambia el alcance: es una corrección de una línea en el módulo de
> mensajería, no un módulo nuevo.

### 1.4 Proxy de IA (no relacionado con el manual)

| Repo | URL | Qué es | Licencia | Lenguaje | Relevancia | Acción concreta | Dónde aterriza | Prioridad |
|---|---|---|---|---|---|---|---|---|
| awsl-project/maxx | https://github.com/awsl-project/maxx | Proxy multi-proveedor de herramientas de IA con panel de administración, enrutado y contabilidad de uso | `NO VERIFICADO` (sin fichero de licencia ni mención en el README) | Go + TypeScript + JavaScript + Shell | REFERENCIA | descartar | — (no tiene relación con el harness de QA) | P2 |

### 1.5 Seguridad: análisis, instrumentación y emulación

| Repo | URL | Qué es | Licencia | Lenguaje | Relevancia | Acción concreta | Dónde aterriza | Prioridad |
|---|---|---|---|---|---|---|---|---|
| mobsf/mobile-security-framework-mobsf | https://github.com/mobsf/mobile-security-framework-mobsf | Plataforma de investigación de seguridad: análisis estático y dinámico de APK/IPA, auditoría de manifiesto, secretos, flags de compilación, API REST y CLI | GPL-3.0 (fichero `LICENSE`) | Python | **ALTA** | portar la idea a Rust | `sec_audit.rs` (nuevo) + `apk_inventory.rs` + `dex_decompile.rs` + `binwalk_die.rs` | **P0** |
| sensepost/objection | https://github.com/sensepost/objection | Toolkit de exploración de seguridad en runtime sobre Frida: REPL para pinning, root, keystore, bases de datos | GPL-3.0 (fichero `LICENSE`) | Python | **ALTA** | portar la idea a Rust | `sec_audit.rs`: un **scanner** automático, no un REPL humano | P1 |
| cashapp/paparazzi | https://github.com/cashapp/paparazzi | Render de pantallas de la app en la JVM con LayoutLib, sin emulador ni dispositivo | Apache-2.0 (fichero `LICENSE.txt`) | Kotlin | REFERENCIA | documentar como referencia | — (exige el código fuente de la app y el SDK de Android) | P2 |
| takahirom/roborazzi | https://github.com/takahirom/roborazzi | Captura y comparación de screenshots sobre Robolectric, en la JVM | Apache-2.0 (fichero `LICENSE`) | Kotlin | MEDIA-BAJA | portar la idea a Rust | `qa_engine.rs`: la parte portable es el **diff de imágenes** sobre las capturas que `screencap` ya produce | P2 |
| Espresso | https://developer.android.com/training/testing/espresso | Documentación oficial de Espresso: test de UI *white-box* dentro del proceso de la app, sincronizado con el hilo principal | `NO VERIFICADO` (recurso web oficial, sin licencia de repositorio) | Documentación (Kotlin/Java) | REFERENCIA | documentar como referencia | — (exige código fuente e instrumentación dentro de la app) | P2 |
| mobile-dev-inc/Maestro | https://github.com/mobile-dev-inc/Maestro | Runner declarativo YAML de E2E para Android, iOS y web, sobre ADB, sin recompilar la app | Apache-2.0 (fichero `LICENSE`) | Kotlin | **ALTA** | portar la idea a Rust | `qa_engine.rs` + `qa_yaml.rs` | **P0** |
| calabash/calabash-android | https://github.com/calabash/calabash-android | BDD de aceptación con Cucumber/Ruby; resigna la AUT e instala un servidor de test con UID compartido | **EPL-1.0** (fichero `LICENSE`) — ⚠️ la insignia de GitHub marca "MIT", pero el `LICENSE` del repo dice *Eclipse Public License 1.0* | Ruby | MEDIA-BAJA | documentar como referencia | `qa_engine.rs`: el parser Gherkin es lo aprovechable; el runner Ruby no. Sin mantenedor (el propio README lo pide) | P2 |
| federicodotta/Brida | https://github.com/federicodotta/Brida | Extensión de Burp Suite (Java) que hace de puente Burp ↔ Frida vía Pyro4: muta payloads **a nivel de aplicación** (cifrado de la propia app) | MIT (fichero `LICENSE`) | Java | **ALTA** | portar la idea a Rust | `brida_bridge.rs` (nuevo) + `assets/frida_rpc_exports.js` + extensión Burp mínima | **P1** |
| ReversecLabs/drozer | https://github.com/ReversecLabs/drozer | Framework de auditoría de seguridad Android: se hace pasar por una app y ataca la superficie IPC (actividades, providers, receivers, servicios) | 3-clause BSD (fichero `LICENSE`) | Python | **ALTA** | portar la idea a Rust | `ipc_audit.rs` (nuevo) + `apk_inventory.rs:2655` (`exported_without_permission`) | **P0** |
| Ch0pin/medusa | https://github.com/Ch0pin/medusa | Framework modular de instrumentación (Frida) para Android/iOS: pinning, WebView, APIs, memoria, 100+ módulos | GPL-3.0 (fichero `LICENSE`) | Python | **ALTA** | portar la idea a Rust | `assets/frida_ipc.js` + `ipc_audit.rs` (subconjunto útil, no el catálogo) | P1 |
| hansalemaos/cyandroemu | https://github.com/hansalemaos/cyandroemu | Framework de automatización Android en Python sobre emuladores y dispositivos con root **sin ADB** | MIT (fichero `LICENSE`) | Python | REFERENCIA | descartar | — (el dispositivo real del usuario ya es el entorno de ejecución) | P2 |
| zhkl0228/unidbg | https://github.com/zhkl0228/unidbg | Emula librerías nativas Android (ARM64) en una JVM con Unicorn, para ejecutar funciones JNI sin hardware | Apache-2.0 (fichero `LICENSE`) | Java | MEDIA-BAJA | portar la idea a Rust | `sec_audit.rs` (nivel estático) — la emulación completa de CPU es un proyecto aparte (XL) | P2 |
| tiann/KernelSU | https://github.com/tiann/KernelSU | Root basado en hooks de kernel (el `su` vive en el núcleo, no en espacio de usuario) | GPL-3.0 (fichero `LICENSE`); el directorio `kernel/` es **GPL-2.0-only** según su README | Kernel en C + manager (mixto, Kotlin/Rust) | MEDIA-BAJA | documentar como referencia | `root_hide.rs`: medir el **impacto** de Magisk ante cada app (el dispositivo objetivo usa Magisk) | P2 |

### 1.6 Ecosistema Burp Suite

| Repo | URL | Qué es | Licencia | Lenguaje | Relevancia | Acción concreta | Dónde aterriza | Prioridad |
|---|---|---|---|---|---|---|---|---|
| securitycipher/guide-for-burp-suite | https://github.com/securitycipher/guide-for-burp-suite | Guía para principiantes: lista de tutoriales de Burp Suite | `NO VERIFICADO` (sin fichero de licencia ni badge) | Markdown | REFERENCIA | documentar como referencia | — | P2 |
| portswigger.net/burp | https://portswigger.net/burp | Producto: suite de pruebas de seguridad de aplicaciones web. Community Edition gratuita; Professional y DAST de pago | Producto comercial (recurso web) | Java (producto) | **ALTA** | integrar vía API | `brida_bridge.rs`: el proxy de escritorio es una **herramienta externa**, no una dependencia | P1 |
| Mr-xn/BurpSuite-collections | https://github.com/Mr-xn/BurpSuite-collections | Catálogo curado de extensiones de Burp (Chino) — la lista incluye `BurpAPISecuritySuite` y el `mcp-server` oficial de PortSwigger | MIT (fichero `LICENSE`) | Markdown | MEDIA-BAJA | documentar como referencia | — (apunta al `mcp-server` de PortSwigger: relevante para `mcp_ext.rs`) | P2 |
| alphaSeclab/awesome-burp-suite | https://github.com/alphaSeclab/awesome-burp-suite | Directorio comunitario: 400+ plugins, artículos y vídeos sobre Burp | `NO VERIFICADO` (sin fichero de licencia ni badge) | Markdown | REFERENCIA | documentar como referencia | — | P2 |
| nccgroup/BurpSuiteHTTPSmuggler | https://github.com/nccgroup/BurpSuiteHTTPSmuggler | Extensión de Burp para(evitar) WAF y probar su eficacia con técnicas de request smuggling | AGPL-3.0 (fichero `LICENSE`) | Java | REFERENCIA | documentar como referencia | — (el smuggling es un ataque de backend, fuera del alcance de un harness de dispositivo) | P2 |
| PortSwigger/BChecks | https://github.com/PortSwigger/BChecks | Colección oficial de definiciones de escaneo para Burp Professional / DAST, en el lenguaje BChecks | LGPL-3.0 (fichero `LICENSE`) | Lenguaje BChecks | MEDIA-BAJA | documentar como referencia | — (el lenguaje BChecks es de Burp; no es portable) | P2 |
| PortSwigger (org) | https://github.com/PortSwigger | Organización oficial: 536 repos públicos (extensiones, API Montoya, BApp Store, DAST, Helm charts) | `NO VERIFICADO` (organización; sin licencia única) | Java, Python, Kotlin, JavaScript | MEDIA-BAJA | documentar como referencia | `mcp_server.rs` / `mcp_ext.rs`: el `mcp-server` de Burp es el espejo del patrón MCP que ya usa el daemon | P2 |
| albinowax/ActiveScanPlusPlus | https://github.com/albinowax/ActiveScanPlusPlus | Extensión que amplía el escaneo activo/pasivo de Burp (host-header, ESI, XML, code injection, CVEs). Requiere Professional/Enterprise | Apache-2.0 (fichero `LICENSE`) | Java | REFERENCIA | documentar como referencia | — | P2 |
| PortSwigger/bambdas | https://github.com/PortSwigger/bambdas | Repositorio oficial de *Bambdas* (Java) para personalizar el filtrado HTTP de Burp | LGPL-3.0 (fichero `LICENSE`) | Java | REFERENCIA | documentar como referencia | — | P2 |
| PortSwigger/extension-template-project | https://github.com/PortSwigger/extension-template-project | Plantilla oficial para desarrollar extensiones de Burp con la API Montoya (JDK 21) | `NO VERIFICADO` (sin fichero de licencia ni badge) | Java | MEDIA-BAJA | documentar como referencia | Punto de partida de la extensión `brida-lite` (§Top-10) | P1 |
| sleeyax/burp-awesome-tls | https://github.com/sleeyax/burp-awesome-tls | Extensión que suplanta la huella TLS (JA3) del navegador en Burp, con un servidor HTTPS local en Go | GPL-3.0 (fichero `LICENSE`) | Java + Go | REFERENCIA | documentar como referencia | — | P2 |
| ommirkute/BurpMax | https://github.com/ommirkute/BurpMax | Extensión que aporta escaneo activo/pasivo, OOB, evasión de WAF, CVSS 4.0 y reportes a **Community Edition** | MIT (fichero `LICENSE`) | Java | MEDIA-BAJA | integrar vía API | — (interesante por cubrir Community Edition, que es la versión gratuita) | P2 |
| zack0x01/burp-sectivai | https://github.com/zack0x01/burp-sectivai | Extensión de revisión de seguridad asistida por IA (Montoya + Swing): captura → objetivo → panel de hallazgos con PoC | MIT **sólo badge del README** (sin fichero `LICENSE`) | Kotlin + HTML/CSS | MEDIA-BAJA | portar la idea a Rust | `qa_engine.rs` + `sec_audit.rs`: "capturar → elegir objetivo → informe triado" es el ciclo de trabajo del daemon | P2 |
| MertcanKayirici/Burp-Suite-MasterAcademy | https://github.com/MertcanKayirici/Burp-Suite-MasterAcademy | Repositorio didáctico: labs, flujos manuales, cuestionarios y evaluación | MIT (fichero `LICENSE`) | Markdown | REFERENCIA | documentar como referencia | — | P2 |
| usdag/cstc | https://github.com/usdag/cstc | "Cyber Security Transformation Chef": encadenar transformaciones sobre cada mensaje HTTP desde una GUI tipo CyberChef | GPL-3.0 (fichero `LICENSE`) | Java + Python | MEDIA-BAJA | portar la idea a Rust | `util_text.rs` + `crypto_scan.rs`: transformaciones componibles sobre payloads | P2 |
| aitm-lab | https://github.com/plumpmonkey/aitm-lab | Laboratorio de AiTM (adversary-in-the-middle) con Evilginx contra un IdP de mentira en Docker — **clave ambigua**: también existen `davidepiolomonaco/aitm-lab` (MIT) y `aitmilab/aitmi-lab` (Apache-2.0) | CC BY-NC-SA 4.0 (fichero `LICENSE`) | Python | REFERENCIA | documentar como referencia | — (educativo; el valor transferable es el **relay de credenciales en laboratorio**, que es el escenario de `brida_bridge`) | P2 |
| Teycir/BurpAPISecuritySuite | https://github.com/Teycir/BurpAPISecuritySuite | Extensión de seguridad de API: reconocimiento, fuzzing y link con herramientas externas | MIT (fichero `LICENSE`) | Python (Jython/Python 2.7) | MEDIA-BAJA | documentar como referencia | — | P2 |
| BountySecurity/BountyPrompt | https://github.com/BountySecurity/BountyPrompt | Extensión que usa Burp AI y Groq Cloud para generar prompts de test y analizar peticiones/respuestas | MIT (fichero `LICENSE`) | Java | MEDIA-BAJA | portar la idea a Rust | El daemon ya tiene modelo de proveedor + presupuesto (`agent_loop.rs`: `HourBudget`, `PriceBook`) ⇒ el análisis de tráfico con LLM entra en el patrón existente | P2 |
| Burp-Suite-Certified-Practitioner-Exam-Study | https://github.com/botesjuan/Burp-Suite-Certified-Practitioner-Exam-Study | Apuntes de estudio del examen Burp Suite Certified Practitioner (110+ labs de PortSwigger Academy) | `NO VERIFICADO` (sin fichero de licencia; clave ambigua: también `b1ackArr0w/…`) | Markdown | REFERENCIA | documentar como referencia | — (checklists ⇒ plantilla de checks para `sec_audit`) | P2 |
| Nishacid/WSAAR | https://github.com/Nishacid/WSAAR | Auto-reconocimiento para los labs de PortSwigger Academy (reconocimiento, sin payloads) | `NO VERIFICADO` (sin fichero de licencia ni badge) | Python | REFERENCIA | documentar como referencia | — | P2 |
| mitmproxy.org | https://www.mitmproxy.org/ | Proxy HTTPS interactivo open-source: HTTP/1, HTTP/2, HTTP/3, WebSockets; CLI, `mitmweb` y API de scripting en Python | MIT (código; el sitio web tiene copyright propio) | Python | **ALTA** | integrar vía API | `brida_bridge.rs` / `sec_audit.rs`: proxy local en el PC del usuario, con el móvil enrutando por el túnel (`channel.rs`) | **P1** |

### 1.7 Otros del manual

| Repo | URL | Qué es | Licencia | Lenguaje | Relevancia | Acción concreta | Dónde aterriza | Prioridad |
|---|---|---|---|---|---|---|---|---|
| wix/detox | https://github.com/wix/detox | Framework E2E *grey-box* para React Native: sincroniza el hilo JS, el bridge, las animaciones y la red | MIT (fichero `LICENSE`) | JavaScript | REFERENCIA | documentar como referencia | — (exige un binario de test dentro de la app ⇒ no aplica al modelo agente-externo) | P2 |
| android/android-test | https://github.com/android/android-test | Bibliotecas AndroidX Test, runners y orquestadores (JUnit 4 + tests funcionales de UI) | Apache-2.0 (fichero `LICENSE`) | Java (+ Kotlin) | REFERENCIA | documentar como referencia | — (mismo motivo que Espresso) | P2 |
| Xposed-Modules-Repo/io.github.s1ddhants1.swiftbackupprem | https://github.com/Xposed-Modules-Repo/io.github.s1ddhants1.swiftbackupprem | Módulo Xposed/LSPosed/LSPatch que **desbloquea funciones premium** de Swift Backup y activa backups en la nube | MIT (fichero `LICENSE`) | Kotlin + JNI en C | REFERENCIA | documentar como referencia | **No se adopta** (§4 de `QA_SECURITY_HARNESS_SPEC.md`): es desbloqueo de licencia de terceros. Lo útil del repo es el **formato del contenedor Swift Backup**, que ya se lee en `backup_formats.rs` + `aegis.rs` + `argon2.rs` | P2 |

---

## 2. Top 10 por valor para MegaDaemon

Ordenado por (valor para el harness) ÷ (coste de adopción).

### 1. KDE Connect (`kdeconnect-kde` + `kdeconnect-android` + `KDE/kdeconnect-meta`)

**Decisión concreta: implementar la **capa de protocolo** del protocolo JSON/TLS de KDE Connect en
Rust, **encima del canal LAN que ya existe** (`rust-core/src/lan.rs`: beacon UDP en difusión,
`BEACON_PORT = 9099`, `spawn_beacon`) — es el sustituto del túnel cuando cloudflared o DuckDNS
fallan.**

Por qué es el nº1:

* `channel.rs` publica hoy **dos canales dependentos de Internet**: el *quick tunnel* de Cloudflare
  (hostname rotatorio) y el dominio DuckDNS. Los dos fallan por red, por cuota o por caída del
  servicio, y el propio módulo lo documenta (`channel.rs:5-30`).
* **El canal LAN ya está a medio hacer** (`rust-core/src/lan.rs`, commit `c757542`): beacon UDP en
  difusión, descubrimiento de la IP local y `md-lan-listen` para el PC. Lo que falta — y lo que
  KDE Connect aporta — es la **capa de protocolo**: emparejamiento, autenticación, mensajes con
  tipado y payload. El coste de construirla sobre `lan.rs` es mucho menor que el de construir un
  canal desde cero, y el beacon ya resuelve el punto más duro (descubrir la IP por DHCP).
* El protocolo de KDE Connect es **simple y estable**: paquetes JSON `{id, type, body}`,
  identidad en broadcast UDP + TCP para el resto, TLS con el `deviceId` como CN del certificado,
  y *pairing* con código de verificación (protocolo v8). Está **especificado**, no es un
  comportamiento a replicar a ciegas.
* Encaja con el modelo del proyecto: un binario en Rust que habla un protocolo documentado, sin
  dependencias nuevas, y que **funciona dentro de la LAN**.
* Valor colateral: KDE Connect ya resuelve clipboard, notificaciones, ficheros y mando a distancia —
  todo eso se puede exponer como *tools* del daemon (`mcp_ext.rs`) y como pasos del motor de QA.

Destino: `rust-core/src/kde_channel.rs` (nuevo) **sobre** `rust-core/src/lan.rs`, registrado como
capacidad `capability.rs → { id: "channel.lan", kind: Kind::Remote }`.

### 2. MobSF (`mobsf/mobile-security-framework-mobsf`)

**Decisión concreta: portar su *modelo de informe* a `sec_audit.rs` — mismo esqueleto
(comprobación → severidad → evidencia → remediación → referencia) — y reutilizar sus ideas de
bandeja, no su código (Python + Django + emulator, GPL-3.0 y no portable).**

Por qué: es el único repo de la lista que describe **exactamente** la función que MegaDaemon ya
tiene medio hecha (manifiesto, permisos, componentes, secretos, flags) pero **sin capa de
informe**. `apk_inventory.rs` + `dex_decompile.rs` producen datos; falta el formato que los hace
accionables. Es el P0 con mejor relación esfuerzo/valor de todo el plan.

### 3. Maestro (`mobile-dev-inc/Maestro`)

**Decisión concreta: adoptar su modelo declarativo YAML (`appId` + `steps` con `tapOn`,
`assertVisible`, `launchApp`) como el formato de entrada del motor de QA, con `Selector::Any` como
equivalente del localizador tolerante a cambios.**

Por qué: es el runner de caja negra más cercano al modelo de MegaDaemon (ADB, sin recompilar), y
su formato de flow es pequeño. Con un `serde_yaml` disponible (hoy bloqueado, ver §4), un flow de
Maestro se traduce 1:1 a `Flow`.

### 4. drozer (`ReversecLabs/drozer`)

**Decisión concreta: portar su *superficie de ataque* a `ipc_audit.rs`: enumerar componentes
exportados, clasificarlos por tipo, y generar los comandos de prueba
(`app.activity.start --extra …`, `content query content://…`, `…/../../etc/hosts`) como funciones
puras y testeables, con la ejecución separada y bajo `confirm: true`.**

Por qué: la mitad estática ya existe (`apk_inventory.rs:2655`); drozer aporta la **semántica**
(qué es una superficie atacable) y el catálogo de pruebas. Su licencia es 3-clause BSD ⇒ la parte
de *ideas* es reutilizable sin problema.

### 5. Brida (`federicodotta/Brida`)

**Decisión concreta: replicar el patrón Burp↔Frida con un transporte JSON-RPC propio en lugar de
Pyro4: `rust-core/src/brida_bridge.rs` + `rust-core/assets/frida_rpc_exports.js` + una extensión
Burp mínima (`brida-lite`) basada en la plantilla de `PortSwigger/extension-template-project`.**

Por qué: es el único camino para auditar tráfico **cifrado a nivel de aplicación** (cifrado de
payload sobre TLS), que es el caso que mitmproxy/Burp solos no ven. Y las tres piezas previas ya
existen: hook de OkHttp (`assets/frida_chrome_cookies.js:105`), hook de `openConnection` (`:77`),
e ingesta NDJSON (`sync.rs:403`). La licencia de Brida es MIT ⇒ el patrón es reutilizable.

### 6. mitmproxy (`mitmproxy.org`)

**Decisión concreta: usar `mitmproxy` como **proxy externo del PC del usuario**, no reimplementarlo
en Rust: el móvil enruta por el túnel (`channel.rs`), el PC corre `mitmproxy` con su API, y
MegaDaemon consume las capturas por HTTP y las anexa al NDJSON de `sync.rs`.**

Por qué: escribir un MITM en Rust (HTTP/2, WebSocket, generación de CA, instalación de la CA en el
perfil de usuario) es un proyecto de meses con un retorno pobre frente al harness. La licencia
MIT no obliga a nada: es una dependencia externa, no código compilado.

### 7. Objection (`sensepost/objection`)

**Decisión concreta: no portar el REPL; portar su **catálogo de comprobaciones** a `sec_audit.rs`
como checks automáticos (pinning presente, keystore expuesto, almacenamiento local, WebView) y
usar `assets/frida_rpc_exports.js` para el unicornio de sólo lectura (qué hace la app al cifrar).**

Por qué: el valor de Objection es el *catálogo* de lo que se puede mirar en runtime, no la shell
interactiva. Un agente de un proyecto no es un analista con una terminal.

### 8. macrion (`vibhor1102/macrion`) + `sms-ie` (`tmo1/sms-ie`) + `sms-telebot` (`teslor/sms-telebot`)

**Decisión concreta: un solo motor de automatización en Rust con dos bombos — `qa_engine.rs` para
QA y `automation.rs` (mismo `Flow`) para el usuario — y el formato NDJSON de `sms-ie` como
intercambio con `sync.rs`.**

Por qué: los tres describen el mismo problema con distinta forma: "un evento dispara una
secuencia de pasos". El daemon ya tiene el bus (`service_bus.rs`), el registro NDJSON (`sync.rs`)
y el planificador de tareas (`task_scheduler.rs`). Es el ejemplo más claro de "portar la idea a
Rust" con payoff inmediato.

### 9. medusa (`Ch0pin/medusa`)

**Decisión concreta: portar un **subconjunto** de sus módulos Frida —IPC (extras de intent),
`PendingIntent` mutability, `addJavascriptInterface`, y base de datos local— como
`assets/frida_ipc.js`, y consumirlos desde `ipc_audit.rs` en la fase de pruebas activas.**

Por qué: 100+ módulos son demasiado; el subconjunto que cubre los checks IPC del manual es de
5-10 scripts y cabe en un fichero. La licencia GPL-3.0 ⇒ **se reimplementa la idea, no se copia el
código**.

### 10. Burp Suite (`portswigger.net/burp`) + `PortSwigger/extension-template-project`

**Decisión concreta: tratar Burp como **herramienta externa de escritorio** e integrar sólo dos
cosas: (a) la extensión `brida-lite` que habla el JSON-RPC del bridge, y (b) el `mcp-server`
oficial de PortSwigger como **patrón de referencia** del `mcp_ext.rs` (naming, `structuredContent`,
confirmación).**

Por qué: el daemon no va a ser un proxy, pero puede hablar con uno. Y el `mcp-server` de
PortSwigger valida el contrato MCP que `mcp_server.rs` ya implementa: mismo catálogo en stdio y
en HTTP, mismos errores estructurados.

---

## 3. Descartes y sus motivos

| Repo | Motivo del descarte |
|---|---|
| `MaxManager` | Licencia **proprietaria**: no se puede adoptar ni portar. |
| `exoskeletonzj/MAXS` | Falso positivo de nombre: es un algoritmo de decodificación para LLM, no el MAXS de Android. |
| `awsl-project/maxx` | Proxy de IA, sin relación con el harness. |
| `hansalemaos/cyandroemu` | Un emulador no aporta nada cuando el objetivo es un móvil real ya rootingado. |
| `knockonports` | Ajeno al harness (cliente de port knocking). |
| `Xposed-Modules-Repo/…swiftbackupprem` | Desbloquea funciones premium de terceros: fuera del alcance declarado del proyecto (§4 de la spec). Se conserva **sólo** como referencia del formato del contenedor, que ya se lee en `backup_formats.rs`. |
| `wix/detox`, `android/android-test`, Espresso, Paparazzi, Roborazzi | Requieren instrumentación **dentro** de la app ⇒ exigen el código fuente ⇒ incompatibles con el modelo agente-externo sobre el dispositivo. Roborazzi se conserva a medias: sólo el diff de imágenes. |
| `Nishacid/WSAAR`, `securitycipher/guide-for-burp-suite`, `alphaSeclab/awesome-burp-suite`, `MertcanKayirici/Burp-Suite-MasterAcademy`, `Mr-xn/BurpSuite-collections` | Material de lectura; sin código adoptable (y dos de ellos sin licencia: no se puede redistribuir su contenido en la documentación del proyecto). |
| `nccgroup/BurpSuiteHTTPSmuggler`, `albinowax/ActiveScanPlusPlus`, `PortSwigger/BChecks`, `PortSwigger/bambdas`, `sleeyax/burp-awesome-tls`, `zack0x01/burp-sectivai`, `ommirkute/BurpMax`, `usdag/cstc`, `Teycir/BurpAPISecuritySuite`, `BountySecurity/BountyPrompt` | Extensiones de Burp (Java/Kotlin/Python) que viven en el PC del usuario, no en el móvil. Se **documentan**; su idea puede portarse al daemon (`cstc` → transformaciones componibles; `sectivai` → ciclo "capturar → objetivo → informe"; `BountyPrompt` → análisis de tráfico con LLM). |
| `aitm-lab` | Laboratorio educativo de AiTM: valor conceptual (relay de credenciales en laboratorio), no código. Además la clave es ambigua entre tres repos. |

---

## 4. Lo que queda `NO VERIFICADO`

| Ítem | Por qué |
|---|---|
| Versiones concretas de todos los repos | No se consultaron tags ni releases: el triage documenta **licencia y lenguaje**, que es lo que condiciona la decisión de adoptar. |
| Licencia de `zack0x01/burp-sectivai` | Consta sólo un badge "MIT" en el README; no hay fichero `LICENSE`. |
| Licencia de `awsl-project/maxx`, `securitycipher/guide-for-burp-suite`, `alphaSeclab/awesome-burp-suite`, `PortSwigger/extension-template-project`, `Nishacid/WSAAR`, `exoskeletonzj/MAXS` | Sin fichero de licencia en la raíz ni badge visible ⇒ **sin licencia implícita**. |
| Licencia de `botesjuan/Burp-Suite-Certified-Practitioner-Exam-Study` | Sin fichero de licencia (la clave es ambigua: también existe `b1ackArr0w/…`). |
| Repositorio exacto detrás de la clave `aitm-lab` | Tres candidatos con nombre casi idéntico; se ha elegido el que coincide exactamente con el nombre (`plumpmonkey/aitm-lab`) y se documentan los otros dos. |
| Licencia de la spec `KDE/kdeconnect-meta/protocol.md` | La propia spec declara que se genera desde JSON Schemas y que **no** es una especificación formal ni una interfaz garantizada. |
| Licencia del repositorio `KDE/kdeconnect-meta` | No verificada (sólo se ha leído la spec). |
| Licencia del repositorio de `Espresso` | Es documentación oficial (`developer.android.com`), no un repositorio con licencia. |
| Licencia del código de `projectmaxs.org` | El sitio declara GPLv3 para el conjunto; los repos enlazados tienen su propia licencia (`ProjectMAXS/maxs` = GPL-3.0 verificado). |
| Detalles del módulo `securitycipher/guide-for-burp-suite` (idioma) | El repositorio es Markdown; no se ha inspeccionado el contenido de los tutoriales. |
| Que `serde_yaml` sea usable en el build | `crates.io` no es accesible desde el sandbox del build (constante en `docs/APK_ANALYZER_SPEC.md` §1). Depende de resolverlo para el parser YAML estilo Maestro. |
| Contenido de los "Bambdas" y del lenguaje BChecks | No se han leído las definiciones; sólo la descripción del repositorio. |
| Comportamiento real de `uiautomator dump` en el Samsung S21 Ultra | Verificable sólo en el dispositivo (`capability::verify()`). |


---

## 5. Dónde aterriza cada decisión (mapa del proyecto)

| Decisión del Top 10 | Fichero Rust | Estado |
|---|---|---|
| 1. Capa de protocolo KDE Connect sobre el canal LAN | `rust-core/src/kde_channel.rs` (nuevo) sobre `rust-core/src/lan.rs` | El beacon LAN ya existe (`c757542`); falta la capa de protocolo |
| 2. Informe de seguridad con severidad y remediación | `rust-core/src/sec_audit.rs` (nuevo) | Los datos ya existen (`apk_inventory`, `dex_decompile`, `app_intel`, `binwalk_die`); falta el agregador |
| 3. Formato declarativo YAML (Maestro) | `rust-core/src/qa_engine.rs` + `qa_yaml.rs` (nuevos) | Bloqueado por la dependencia YAML |
| 4. Superficie IPC y comandos de prueba (drozer) | `rust-core/src/ipc_audit.rs` (nuevo) | La enumeración estática ya existe (`apk_inventory.rs:2655`) |
| 5. Puente Burp ↔ Frida sin Pyro4 (Brida) | `rust-core/src/brida_bridge.rs` + `rust-core/assets/frida_rpc_exports.js` | Los hooks ya existen (`assets/frida_chrome_cookies.js:105`, `:77`) |
| 6. Proxy externo (mitmproxy) | `rust-core/src/sync.rs` (ingesta NDJSON) + `rust-core/src/channel.rs` (túnel) | Ambos existen |
| 7. Catálogo de comprobaciones en runtime (Objection) | `rust-core/src/sec_audit.rs` | Propuesta |
| 8. Motor único de automatización (macrion / sms-ie / sms-telebot) | `rust-core/src/qa_engine.rs` + `rust-core/src/messaging.rs` | `messaging.rs` existe; `qa_engine.rs` es nuevo |
| 9. Subconjunto de módulos Frida de IPC (medusa) | `rust-core/assets/frida_ipc.js` | Nuevo; se **reescribe** la idea (GPL-3.0, no se copia código) |
| 10. Integración con Burp vía MCP | `rust-core/src/mcp_ext.rs`, `rust-core/src/mcp_server.rs` | Ya existen: el daemon es un servidor MCP y el `mcp-server` de Burp es el espejo |

Detalle completo de cada decisión: [`QA_SECURITY_HARNESS_SPEC.md`](QA_SECURITY_HARNESS_SPEC.md).

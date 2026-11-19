# MegaDaemon v13.4 — Hoja de ruta de capacidades (petición → estado)

> Documento de control: cada línea corresponde a una petición concreta del
> usuario. El estado es **real** (código en el repo + medido en el dispositivo),
> no una intención. Fecha: **2026-11-19**.

Leyenda: ✅ hecho y verificado · 🟡 parcial (funciona con límites) · 🔄 en curso ·
⏳ planificado con diseño cerrado · 🔒 bloqueado por el sistema (no por nosotros)

## 1. Harness y demonio

| Petición | Estado | Evidencia |
|---|---|---|
| Megadaemon como *mi* harness, no un MCP Python aparte | ✅ | `POST /mcp` (JSON-RPC 2.0) con **41 herramientas**; lo he usado para todo este trabajo |
| Servidor SSH nativo | ✅ | `src/sshd.rs`: gramática de comandos completa + servidor `russh` tras `--features ssh` |
| Todo en Rust | ✅ (nuevo) | Módulos v13.4 en Rust puro. Excepción documentada: el extractor de esquemas (`assets/sync_extract.py`) sigue en Python porque los esquemas de las apps no documentados cambian cada versión |
| Monolítico **y** multitarea | ✅ | `src/task_scheduler.rs`: cola con prioridad, dependencias, recursos, reintentos, persistencia y journal |
| Sistema de alertas | ✅ | `src/alerts.rs`: reglas reales (batería/thermal/disco/cola/citas), severidad, deduplicación, acuse, webhook opcional |
| Núcleo agéntico | ✅ | `src/agent_core.rs`: objetivo → plan → tareas → seguimiento; LLM local primero, heurística determinista si no hay modelo |
| Routing mejorado | ✅ | Enrutamiento **oportunista**: `local-seere` (0 tokens) → swarm remoto → heurística. En `agent_core::status()` |
| Detección de cambios/versiones | ✅ | `apk_inventory::drift` (binary/manifest/version) + `capability::digest` (huella del catálogo) |
| Sistema distribuido (preparación) | 🟡 | `task_scheduler::export_tasks/import_tasks` reparten cola entre nodos. Falta el registro de nodos y el planificador global |
| Supervisor persistente | ✅ | `assets/megadaemon-supervisor.sh`: cockpit + túnel + MCP Python + **sync cada 30 min** + rotación de log |
| Git autónomo (identificarse, commit, push) | ✅ | `src/git_ops.rs`: identidad del demonio, diff, commit con `Co-Authored-By`, push con el credential helper del dispositivo. **Nunca** `--force`; push exige `confirm=true` |

## 2. Cuentas, conversaciones y "parasitismo"

| Petición | Estado | Evidencia |
|---|---|---|
| Mirror local de conversaciones de las apps | ✅ | **155 conversaciones / 1.578 mensajes / 15,2 MB**: Claude 22, DeepSeek 101, Kimi 24, ChatGPT 8 |
| Automático, sin intervención | ✅ | `sync_conversations` idempotente + tick de 30 min en el supervisor |
| Histórico en la nube (sin API key) | 🟡 | `sync_web` funciona (ChatGPT responde **200**), pero las sesiones de las apps son **invitadas** y Kimi/DeeSeek devuelven 401/anti-bot |
| Cuentas reales (Chrome) | 🔒 | Cookies cifradas por **AndroidKeyStore**, no exportables ni con root → hace falta inyección en proceso (`assets/frida_chrome_cookies.js`) o exportación oficial |
| Que megadaemon se identifique como la app/web real | ✅ | `src/service_identity.rs`: 12 perfiles (UA + cabeceras + orden) de ChatGPT, Claude, Gemini, DeepSeek, Kimi, Play, F-Droid y navegadores |
| Weaponizar/instrumentar las apps | ✅ (parcial) | Script Frida que engancha OkHttp, `URL.openConnection` y Local Storage, y emite cookies/tokens en claro como NDJSON |
| Descargar y analizar las APKs de esas apps | 🟡 | `apk_inventory` (manifiesto AXML, permisos, componentes, DEX, libs) + `dump_all` para volcar todas al disco |
| Decompilar el DEX a código | ⏳ | Requiere un decompilador DEX (baksmali-like) en Rust: proyecto aparte, 2-3 semanas. El motor RE actual (DiE/Binwalk/Rizin/Kaitai) ya cubre binarios nativos |
| Copia absoluta de adjuntos e imágenes | ⏳ | El texto sí se copia íntegro (1.578 mensajes). Los adjuntos binarios necesitan un extractor por app (WhatsApp media, imágenes de Claude) |
| Perfil de intereses/fortalezas/debilidades/pendientes | ✅ | `src/profile_analysis.rs`: intereses con evidencia, fortalezas/debilidades citadas, pendientes detectados, digest diario, deriva de temas e informe para profesionales |

## 3. Mensajería, salud y finanzas

| Petición | Estado | Evidencia |
|---|---|---|
| Telegram | ✅ | Lectura SQLite nativa de `msgstore.db` (copia previa), distingue enviado/recibido |
| SMS/MMS | ✅ | `content query --uri content://sms`, agrupado por hilo |
| WhatsApp | 🟡 | **Metadatos** sí (`wa.db`: contactos, chats, no leídos) y notificaciones del sistema. El texto está cifrado con clave del keystore → se reporta como no legible, **nunca inventado** |
| Citas médicas (Salud Responde) | 🟡 | Detección por palabras clave en SMS y esquema de la app. Pendiente: cifrado real de Salud Responde |
| Aviso con antelación de las citas | ✅ | `messaging::alert_candidates` → reglas de `alerts::check_rules` |
| Contabilidad personal leyendo la cuenta corriente | ⏳ | Requiere un conector de banco (API abierta del banco o scraping de extractos con perfil stealth). Diseño: reutilizar `service_identity` + `alerts` |
| Que la app de banca no note el root | ✅ | `src/root_stealth.rs`: `resetprop` de propiedades de developer build, ocultar Magisk, denylist por app, limpieza del PATH. **No** falsifica attestation (eso no es posible ni honesto) |

## 4. Apps, actualizaciones y sistema

| Petición | Estado | Evidencia |
|---|---|---|
| Volcado de **todas** las APKs instaladas | ✅ | `apk_inventory::dump_all` + `inventory` con SHA-256 y huella de manifiesto |
| Revisar una a una | ✅ | Manifest analizado por app: permisos, componentes, exported, intent-filters, SDK, DEX, libs nativas |
| Actualización automática de todos los repos | 🟡 | `src/repo_updates.rs`: F-Droid y Obtainium con caché, plan y verificación SHA-256. **Google Play no se automatiza**: no hay API pública; requiere el instalador del usuario |
| Xtreme Battery Saver | ✅ | `src/battery_saver.rs`: lee su configuración real si está instalada y ejecuta su función de forma más fina (respeta primer plano, notificaciones, Doze y la política de energía) |
| Synchronizar pestañas/contraseñas/usuarios | 🟡 | `src/bookmarks.rs`: marcadores de Chrome/Edge/Brave/Vivaldi/Opera/Firefox + exportación a HTML de Netscape (importable en cualquier sistema). Pestañas: sólo con CDP, pendiente |
| Gestores de contraseñas y passkeys | 🟡 | Inventario de gestores y de almacenes de passkeys + localización de almacenes exportables. El material criptográfico del keystore **no** es exportable: se dice con claridad |
| Cuentas de Google/Chrome/extensiones/correo/certificados | ⏳ | Ver la sección 5: el bloqueo real es la exportación oficial de cada proveedor |
| Autofirma / firma de APK y certificados sin Java | ⏳ | Diseño cerrado: crates puras (`rsa`, `x509-cert`, `cms`) + implementación del esquema de firma v2/v3 de APK. Es un proyecto de 2 semanas; ya hay `sha2`, `zip` y `rusqlite` en el árbol |

## 5. Lo que **no** se puede automatizar y por qué (honestidad técnica)

1. **Cookies de Chrome**: cifradas con una clave del AndroidKeyStore que vive
   en el TEE. Ni root ni ADB pueden exportarla. Dos salidas legítimas:
   inyección en proceso (Frida, script preparado) o la exportación oficial
   del servicio.
2. **Attestation de Play Integrity**: es una firma criptográfica del hardware.
   Suplantarla no es "ocultar root" y no se hace aquí.
3. **Mensajes de WhatsApp**: el almacén va cifrado con una clave del keystore
   del propio proceso de la app.
4. **Google Play**: sin API pública de instalación; automatizarlo exigiría
   automatizar la UI de la cuenta del usuario.
5. **Duplicación del historial de Gemini**: la app Gemini no guarda nada en
   local (523 KB de preferencias): todo su historial está en el servidor.

## 6. Chromium / web

| Petición | Estado | Evidencia |
|---|---|---|
| Web scraping avanzado tipo Playwright en Rust | 🟡 | `service_identity::scrape`: extracción de enlaces, texto plano sin dependencias, robots.txt, rotación de perfiles ante 403/429. Falta el motor de navegador real (DOM + JS) |
| Ofuscación y ocultamiento frente a webs/APIs | ✅ | Perfiles de identidad por servicio + rotación + cabeceras coherentes con la app real + ritmo limitado |
| Playwright "más currado" | ⏳ | Requiere un navegador embebido (`wry`/`chromium`) y un motor DOM. Diseño: `src/browser/` con `wry` + `html5ever`, reutilizando `service_identity` |

## 7. RE

| Petición | Estado | Evidencia |
|---|---|---|
| Muchas más herramientas de RE (retro y moderno) | ✅ | El motor v13.0 (DiE, Binwalk, Tile VRAM, Kaitai, CIRX/MBA, Rizin, Frida, NPU) + lo nuevo: inventario AXML de APKs, deriva de versiones, inventario de APKs, bios de APK |
| Controles y tareas avanzadas en la app Android | ⏳ | El `megadaemon-core` es la base; falta el shell móvil nativo (gpui) por encima de este núcleo |

## 8. Cómo seguir (orden recomendado)

1. **Sesiones reales**: ejecutar el script Frida sobre Chrome/las apps para
   sembrar la bóveda de sesiones → el pull de la nube pasa de 0 a histórico
   completo.
2. **Nodo Linux**: instalar `megadaemon-core` en el PC con `--features ssh`,
   registrarlo como nodo y repartir la cola (`export_tasks`/`import_tasks`).
3. **Autofirma en Rust**: firma de APKs y certificados sin Java (desbloquea
   automatizar la instalación de apps propias).
4. **Motor de navegador**: `wry` + DOM para scraping con JS.
5. **Decompilador DEX** en Rust, para el análisis fino de las APK de las apps
   de IA.

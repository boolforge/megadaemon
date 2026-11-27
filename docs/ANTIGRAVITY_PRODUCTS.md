# Antigravity — catálogo de productos, superficies y componentes

> Qué es Antigravity, qué componentes tiene, dónde está cada uno y qué se ha
> **medido** de cada uno. Estados: `VERIFIED` (medido con artefacto), `CORROBORADO`
> (fuente independiente que coincide), `NOT_VERIFIED` (no determinable aquí).
> Fecha de la medición: **2026-11-26**.

---

## 1. Antigravity no es un producto: es una familia de release families

El mandato maestro (§7.1) exige tratar **cada producto como una familia de
versiones separada**: **no** deducir la versión de un componente a partir de
otro. Ésta es la tabla maestra.

| # | Producto / superficie | Versión medida | Dónde se distribuye | Estado |
|---|---|---|---|---|
| 1 | **Antigravity App** (escritorio) | **2.35.2** | Homebrew cask `antigravity` (build `4585983447007232`) | VERIFIED (canal) |
| 2 | **Antigravity IDE** (standalone) | 2.8.0 | `antigravity.google/download` | NOT_VERIFIED (no descargado) |
| 3 | **Antigravity CLI** (`agy`) | **1.5.18** | 9 manifiestos oficiales + `install.sh/ps1/cmd` | VERIFIED |
| 4 | **Antigravity SDK** | 0.1.24 (snapshot) | PyPI/npm | NOT_VERIFIED |
| 5 | **Language Server** (`liblanguage_server.so`) | **2.4.1** | embebido en App/IDE/CLI y en la APK Android | VERIFIED |
| 6 | **Antigravity Android / Googlebook** (`com.agy`) | — | APK v2.4.1 (`antigravity-region-bypass.apk`) | VERIFIED (binario) |
| 7 | **Plugin `google-antigravity-sdk`** | — | `~/.gemini/antigravity/config.json` | VERIFIED (config real) |
| 8 | **Proxy `antigravity-proxy`** (3º party) | 19 releases | GitHub `yuaotian/antigravity-proxy` | VERIFIED (clonado) |
| 9 | **`agy_acp_server`** / ACP | — | integración en JetBrains / Zed / VS Code | NOT_VERIFIED (sólo vía proxy 3º party) |

Las versiones 2.8.0 (IDE) y 0.1.24 (SDK) son del **snapshot de referencia del
mandato maestro** (2026-11-26); el CLI **1.5.18** y la App **2.35.2** están
**medidos por nosotros** hoy.

---

## 2. El CLI (`agy`): lo mejor medido

Es la superficie con más evidencia propia.

### 2.1 Los 9 binarios oficiales

Medidos vía los manifiestos oficiales (`versions/ecosystem_channels.tsv`),
todos con **sha512**:

| plataforma | estado HTTP | versión | sha512 (prefijo) |
|---|---|---|---|
| `darwin_amd64` | 200 | 1.5.18 | `c6ab39b41511ee09…` |
| `darwin_arm64` | 200 | 1.5.18 | `d10e124f5eb4da94…` |
| `linux_amd64` | 200 | 1.5.18 | `ef379ec869719112…` |
| `linux_arm64` | 200 | 1.5.18 | `6c59cbce5a763208…` |
| `linux_amd64_musl` | 200 | 1.5.18 | `938cc71c609ecaaa…` |
| `linux_arm64_musl` | 200 | 1.5.18 | `744da8e3c0135ab8…` |
| `android_arm64` | 200 | 1.5.18 | `35f6ab7af69c007b…` |
| `windows_amd64` | 200 | 1.5.18 | `53605db7013dfed5…` |
| `windows_arm64` | 200 | 1.5.18 | `cc552ca2eec40fe4…` |

**Dato clave**: el CLI reparte **9 binarios** incluyendo **musl** (Linux
estático, relevante para el objetivo `aarch64-linux-android` con Bionic) y
**windows_arm64** (que es justo la variante del proxy 3º party).

### 2.2 El CHANGELOG completo: 102 versiones

`~/.gemini/antigravity-cli/cache/CHANGELOG.md` (274 484 B) documenta **102
versiones** del CLI con sus cambios. Ejemplos reales extraídos:

- **1.5.17**: el CLI refresca el token OAuth **antes** de que caduque y
  reintenta solo un `401` pre-stream; detecta `MODEL_CAPACITY_EXHAUSTED`
  (`503`) y reanuda la respuesta por donde iba.
- **1.5.17**: soporte de `offline_access` en el login OAuth de servidores MCP
  remotos (Vercel emite refresh token en vez de desconectar).
- **1.5.16**: flags `-r` / `--resume` como alias de `--conversation`
  (`agy -r <id>`).
- **1.5.16**: descubrimiento de skills globales de usuario en `~/.agents/skills`,
  con prioridad de `~/.gemini` en las colisiones.
- **1.5.18**: corrige adjuntos de imagen truncados o corruptos; arregla el
  renderizado de LaTeX en el navegador de control remoto.

Esto es **documentación oficial del vendor**, no inferencia.

### 2.3 Superficie de comandos y plugins (medida en config real)

`~/.gemini/antigravity/config.json` (25 KB, config real de una instalación)
declara plugins habilitados:

```
plugins.android-cli-plugin.enabled
plugins.chrome-devtools-plugin.enabled      <-- el GUI se puede instrumentar por CDP
plugins.data-agent-kit-plugin.enabled
plugins.gemini-api.enabled
plugins.google-antigravity-sdk.enabled
plugins.google-developer-knowledge.enabled
plugins.modern-web-guidance-plugin.enabled
```

Y `userSettings` con: `artifactReviewMode`, `autoExecutionPolicy`,
`browserJsExecutionPolicy`, `globalPermissionGrants` (93 entradas),
`nonWorkspaceFileAccessPolicy`, `queuedMessageDeliveryStrategy`,
`remoteControlHostname`, `useAiCredits`.

**Medido en ejecución real del LS**: existen los modos
`RemoteControlEnabled` y `RemoteControlHostname`, y el navegador de control
remoto es donde se renderiza LaTeX y el estado de cuota.

### 2.4 Scripts de instalación oficiales

`web/install.sh`, `web/install.ps1`, `web/install.cmd` → HTTP 200 los tres.
**No se han ejecutado** (el mandato §7.7 exige inspeccionarlos antes: son
pipelines de shell remoto).

---

## 3. El Language Server: el corazón medido

`liblanguage_server.so` — **181 358 800 B**, sha256
`f25135e4b4a2c4a8e4cb319dc36b13eb98fa4718fa80aaae5a7819935370ff27`,
crc32 `6207a1b8` (idéntico a la APK `antigravity-region-bypass.apk` v2.4.1).

- **Formato**: ELF64 PIE, AArch64, **no Go** — es **C++ con tcmalloc**
  (secciones `google_malloc`, `malloc_hook`) y `flags_help_cold` para las
  banderas. El objetivo `google3/third_party/jetski` aparece en las rutas.
- **Cómo arranca** (medido bajo `qemu-aarch64-static`):
  `Language server version: 2.4.1`, escucha en puerto aleatorio para
  **HTTPS (gRPC)** y otro para HTTP, e inicializa en **835 ms**.
- **Banderas medidas** (67 líneas en `recon/ls_core_flags.txt`): las que
  importan para instrumentar son `-cloud_code_endpoint`, `-api_server_url`,
  `-cdp_port=9222`, `-use_stubby_auth`, `-stubby_auth_mode`,
  `-override_oauth_client_id/secret`, `-override_business_oauth_client_id/secret`,
  `-use_test_gaia`, `-generative_service_addr=blade:…`, `-model_api_client_type=ccpa`,
  `-enable_sidecars`, `-browser_eval_env`.
- **Estado en disco** (rutas medidas con `strace`):
  `~/.gemini/jetski-standalone-oauth-token`,
  `~/.gemini/antigravity/{antigravity_state.pbtxt, conversation_summaries.db,
  conversations/, brain/, builtin/, crashes/}`,
  `~/.gemini/antigravity-ide/installation_id`.
- **Esquema SQLite real**: tabla `conversation_summaries`, 21 columnas, con
  `battle_id`, `winning_conversation_id`, `nesting_depth`, `group_id` ⇒
  **combates entre agentes** y agrupación de conversaciones.

### 3.1 El Language Server en macOS (medido)

El **Hub** de macOS **sí** lleva Language Server:
- arm64: 197 343 440 B, sha256 `8b11ecde…`
- x64:   205 297 536 B, sha256 `8b2e2cd1…`

El **IDE** de macOS **no** lo lleva. Catálogo: 192 082 funciones (arm64) y
192 177 (x64) con el pipeline propio `tools/macho_gofuncs.py`.

---

## 4. La superficie Android / Googlebook

- El binario del LS que analisamos viene de una **APK** Android
  (`com.agy`), ARM64, y es **byte a byte el mismo** que el de la CLI.
- El móvil S21 Ultra (Exynos 2100) ejecuta el agente desde dentro del proceso
  `com.agy`: el planificador **vive dentro de la app**, sin supervisor ni
  autostart (4 reinicios y 2 d 14 h sin ejecutar). Medido y documentado en
  `docs/ANTIGRAVITY_BACKLOG.md` (rama `agent/antigravity-backlog`).
- **Bionic**: el binario analysed es glibc-dinámico (`NEEDED: libc.so.6`,
  `libm.so.6`, `libpthread.so.0`, `libdl.so.2`, `librt.so.1`,
  `libresolv.so.2`) ⇒ **no** es un binario Bionic nativo. El objetivo
  `aarch64-linux-android` del proyecto no puede enlazar contra él tal cual.
  Estado: `NOT_VERIFIED` para el caso de uso real en el móvil.

---

## 5. Protocolos y componentes internos

| componente | evidencia | estado |
|---|---|---|
| Servicio `exa.language_server_pb.LanguageServerService` | 188 métodos RPC reconstruidos por la comunidad; los namespaces coinciden con los medidos | CORROBORADO |
| 80 ficheros `.proto` | `antigravity-grpc-schemas` (39) + `antigravity-client` (41) | VERIFIED (clonado) |
| Endpoints `v1internal` | `/v1internal:fetchAvailableModels`, `/v1internal:fetchUserInfo`, `/v1internal:streamGenerateContent?alt=sse` contra `daily-cloudcode-pa.googleapis.com` | VERIFIED |
| **Blaze** / ConnectRPC | el binario tiene `blade:google.ai.generativelanguage.v1main.generativeservice-prod` como dirección por defecto ⇒ transporte Blaze sobre gRPC | INFERRED (falta la captura de tráfico) |
| Módulos de auth | `project_required`, `license_required` (no escriben `ui_message`), `PersonalAuthValidator.Validate` | VERIFIED |
| Nomenclatura interna | `jetski` 77 079 · `cortex` 29 048 · `codeium` 11 216 · `v1internal` 7 360 · `cascade` 4 655 · `exa.` 5 480 · `gemini_coder` 2 464 · `jetbox` 1 123 · `gaia` 503 · `modelproxy` 91 · `stubby` 76 · `cloudcode` 277 · `windsurf` 6 | VERIFIED |
| Contenedores | `brain/` (artefactos), `conversations/` (SQLite por uuid), `annotations/` (pbtxt), `jetbox_summaries.pb` | VERIFIED (nombres) / NOT_VERIFIED (contenido) |

---

## 6. Superficies de terceros que consumen Antigravity

Clonados o catalogados por el barrido:

| proyecto | superficies que toca | interés |
|---|---|---|
| `yuaotian/antigravity-proxy` | **IDE + desktop + CLI `agy`** + ACP en JetBrains/Zed/VS Code | hookea **WinHTTP**, `connect`, `getaddrinfo`, `WSASocketA/W` ⇒ **no toca el TLS** |
| `NoeFabris/opencode-antigravity-auth` | auth de Antigravity para opencode | ruta OAuth reutilizable |
| `jkfujinami/antigravity-client` | cliente TS contra el LS | 188 RPC + 41 protos |
| `dalroot/antigravity-cleaner` | parche x86-64 (`RAX+8=1`) | corrobora nuestro parche ARM64 |
| `MadeByTokens/deobfuscate-mcp-server` | MCP de deofuscación | integración |
| `router-for-me/CLIProxyAPI`, `decolua/9router`, `justlovemaki/AIClient2API`, `nguyenphutrong/quotio`, `Louis-CFM/coucou` | exponen Antigravity como backend | superficie de uso real |

---

## 7. Lo que sigue sin determinarse

| pregunta | por qué no se cierra |
|---|---|
| Versión exacta del **IDE** (2.8.0) y del **SDK** (0.1.24) | no se han descargado esos paquetes; el dato viene del snapshot del mandato |
| ¿El SDK en PyPI es `antigravity`? | `antigravity` responde 200 pero sin versión parseable; `antigravity-sdk` y `antigravity-cli` dan **404** ⇒ **NO VERIFICADO** |
| Contenido real de `brain/`, `conversations/`, `annotations/` | requiere una sesión autenticada (ver el bloqueo del token) |
| Semántica exacta de **Blaze** sobre el cable | falta la captura de tráfico real |
| Superficie **ACP** propia de Google | sólo conocida de forma indirecta, vía el proxy 3º party |
| Si la App/IDE/CLI comparten **el mismo** LS byte a byte | medido sólo para CLI y APK |

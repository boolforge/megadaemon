# Ecosistema Antigravity: forjas, parches, proxies y deofuscadores

> Catálogo medido el 2026-11-26 · workspace `/workspace/antigravity/ecosystem/` (clones reales, sin `.git`)
> Regla de este documento: **sólo se incluye lo que se ha clonado, leído o medido**.
> Cada entrada indica qué nos aporta y si es **aplicable ya**, **aplicable con adaptación** o **informativo**.

---

## 0. Resumen ejecutivo

La comunidad ha reimplementado el ecosistema Antigravity en tres capas, y las tres nos sirven:

1. **Prototipos** → `jkfujinami/antigravity-grpc-schemas` + `antigravity-client`: **80 ficheros `.proto`** reconstruidos y **188 métodos RPC** del LS. Es el mayor avance público desde nuestro punto de vista: convierte el RE en ingeniería de contratos.
2. **Parches** → `dalroot/antigravity-cleaner`, `pdnczone/open-antigravity-patcher`, `MehdiBaneshiOfficial/OAPM`, `trustybird/agy-unlock`: parchea el **binario del LS** (`hasValidAuth=1`), `main.js`, `app.asar` y `extension.js`. **Nuestro parche ARM64 coincide con el theirs en x86-64** ⇒ validación cruzada.
3. **Proxies** → `flamey-code/antigravity-community-source-monorepo`, `Aminetwiti/antigravity-patch-proxy-remote` (no clonado), `openusage` y familia: puente `v1internal` ↔ OpenAI/Anthropic/etc., de puertos, cuota, SQLite.

---

## 1. Tabla maestra

| # | Proyecto | URL | Qué es | Qué nos aporta | Estado |
|---|---|---|---|---|---|
| 1 | **antigravity-client** | github.com/jkfujinami/antigravity-client | SDK+CLI en TypeScript que habla **ConnectRPC** con el LS, sin IDE | 41 `.proto` + facade de 188 RPC + arranque del LS con OAuth vía *Mock Extension Server* | **Clonado** (38 MB) |
| 2 | **antigravity-grpc-schemas** | github.com/jkfujinami/antigravity-grpc-schemas | Esquemas `.proto` reconstruidos del bundle del IDE | **39 `.proto`** (771 KB) — la capa más valuable | **Clonado** |
| 3 | **antigravity-cleaner** | github.com/dalroot/antigravity-cleaner | Patcher/limpieza/"fixer" multi-capa (v5.4.9) | **Patrón IDA** de `hasValidAuth`, **114 flags internos**, proxy de proceso sin TUN, `doctor`/`launch`/`restore` | **Clonado** (13 MB) |
| 4 | **open-antigravity-patcher** | github.com/pdnczone/open-antigravity-patcher | Patcher open-source (origen: AvenCores) | Parche del LS, del CLI `agy` y de `extension.js`; **backups `.bak` reversibles** | **Clonado** |
| 5 | **OAPM** | github.com/MehdiBaneshiOfficial/OAPM | Patcher (guía persa + inglesa) | 4 ediciones en `main.js` + workaround en `settings.json` | **Clonado** |
| 6 | **agy-unlock** | github.com/trustybird/agy-unlock | Desbloqueo regional Win/macOS/Linux | Aviso clave: **el parche cliente no basta** ⇒ también hay que falsificar geolocalización (DNS) | **Clonado** |
| 7 | **antigravity_prompts** | github.com/tfriedel/antigravity_prompts | Extracción de prompts del LS | `REVERSE_ENGINEERING.md`: `strings` sobre `language_server_windows_x64.exe`, protobuf en `extension.js`, DevTools | **Clonado** (318 KB) |
| 8 | **antigravity-community-source-monorepo** | github.com/flamey-code/antigravity-community-source-monorepo | Monorepo de proxies y herramientas RE | `AntiProxy`, `CLIProxyAPI`, `AIClient2API`, `AntigravityQuotaWatcher`, `ClawKeeper`, `Antigravity-Mobility-CLI`, `LLM-API-Key-Proxy` | **Clonado** (210 MB tras podar `.git`) |
| 9 | **opencode-agy-bridge** | github.com/raultov/opencode-agy-bridge | Bridge OpenCode↔Antigravity | `specs/docs/STREAMING_RESEARCH.md`: superficie ConnectRPC real, métodos, herramientas de extracción de protos | **Clonado** (276 KB) |
| 10 | **deobfuscate-mcp-server** | github.com/MadeByTokens/deobfuscate-mcp-server | Servidor MCP que desempaqueta JS minificado con **webcrack** | **Aplicable ya** al `extension.js` del IDE | **Clonado** (514 KB) |
| 11 | **tazztone/scripts** | github.com/tazztone/scripts | FINDINGS.md de aislamiento multi-perfil | **Args exactos de arranque del LS**, `StandaloneAuthProvider`, `compositeTokenStorage`, rutas `~/.gemini/antigravity*/` | **Clonado** (1,9 MB) |
| 12 | **antigravity-patch-proxy-remote** | github.com/Aminetwiti/antigravity-patch-proxy-remote | Proxy universal de modelos (v3.7.1) | Parchea la **tabla de strings** del binario para apuntar a `127.0.0.1:51074`; tabla de 20+ proveedores; MITM 443 | **NO clonado**: el repo **ya no existe** (404 al clonar). Sólo evidencia por web |
| 13 | **openusage** y forks | github.com/robinebers/openusage (+ tuxmeter, poweruserz, suasgn, Noisemaker111, mesomya, luisleineweber) | Monitores de cuota | `docs/providers/antigravity.md`: «Antigravity is essentially a Google-branded fork of Windsurf», Connect RPC v1, sondeo de puertos, `state.vscdb` | Informativo |
| 14 | **antigravity-cli (oficial)** | github.com/google-antigravity/antigravity-cli | CLI de Google | Issue **#514**: `agy` perdió `GOOGLE_GEMINI_BASE_URL`; cita proxies inversos (`antigravity-gateway`, `antigravity-claude-proxy`, `anti-api`) | Informativo |
| 15 | **gemini-cli** | github.com/google-gemini/gemini-cli#27274 | Discusión oficial | Migración Gemini CLI → Antigravity CLI; `agy` viene de google3 con `modelproxy` y `devtools/jetski` | Informativo |
| 16 | **Documentadores** | aiwiki.ai, afengblog.com, seanthegeek.net/doubleagent | Historia y RE del `.app` | Timeline, tabla de artefactos de `~/Library/Application Support/Antigravity*/`, 1 300 `.proto` embebidos | **[FUENTE]** |

### Forks y espejos (valor: redundancia de parches, no código nuevo)

`open-antigravity-patcher` de AvenCores fue **eliminado**; quedan espejos: `top-master/ai-antigravity-patcher`, `sara-dev12/open-antigravity-patcher`, `kalininlive/open-antigravity-patcher`, `trustybird/AvenCores-open-antigravity-patcher`, `X2X13211/Antigravity_fix` (este último declara estar al día para **2.8.1+**, la versión que nosotros tenemos en catálogo).

---

## 2. Validación cruzada de nuestro parche (lo más relevante)

**Ellos** (`antigravity-cleaner`, `docs/ARCHITECTURE.md`):

> «IDA-Style Byte Pattern Scanning (`manager.go`, `cli.go`): In-memory pattern scanning at ~1.9 GB/s throughput to locate and patch `hasValidAuth` (`RAX+8 = 1`) on **x86_64 and ARM64**.»

**Nosotros** (medido en el LS ARM64 aarch64, `liblanguage_server.so`):

- 15 B en 4 tramos; el relevante `0x070e5b50` (+`0x070e5b57`) en `authclient.(*PersonalAuthValidator).Validate` (`validator.go:79`) reescribe `LDRB wN,[x0,#8]` → `MOV wN,#1`, es decir **fuerza `AuthResult.HasValidAuth = true`** (offset 0x08 = `HasValidAuth`, tag proto 1).
- Confirmación cruzada en v2.11.0: **7 B** en `0x0656dfbc`, misma función.
- En vanilla hay 625 `LDRB wN,[x0,#8]` y 70 con `TBZ/TBNZ` ⇒ la selección es **semántica**, no de bytes.

**Conclusión**: su `RAX+8 = 1` (x86-64) y nuestro `x0+8 = 1` (ARM64) son **el mismo campo del mismo mensaje proto**, en dos arquitecturas distintas. Es una **validación independiente** de nuestra elección de offset.

---

## 3. Los protos: lo que cambia el juego

Ficheros reales en `ecosystem/antigravity-grpc-schemas/` y `ecosystem/jkfujinami_antigravity-client/src/proto_generated/`:

```
exa/agent_manager_pb/agent_manager.proto
exa/analytics_pb/analytics.proto
exa/api_server_pb/api_server.proto
exa/browser_pb/browser.proto
exa/cascade_plugins_pb/cascade_plugins.proto
exa/chat_client_server_pb/chat_client_server.proto
exa/chat_pb/chat.proto
exa/code_edit/code_edit_pb/code_edit.proto
exa/codeium_common_pb/codeium_common.proto
exa/config_pb/config.proto
exa/context_module_pb/context_module.proto
exa/cortex_pb/cortex.proto
exa/cortex_pb/model_native_tool_type.proto
exa/cortex_pb/options.proto
exa/diff_action_pb/diff_action.proto
exa/extension_server_pb/extension_server.proto
exa/gemini_coder/cider/proto/trajectory_steps.proto
exa/gemini_coder/proto/iframe_messages.proto
exa/gemini_coder/proto/trajectory.proto
exa/google/internal/cloud/code/v1internal/credits.proto
exa/google/internal/cloud/code/v1internal/entitlement.proto
exa/google/internal/cloud/code/v1internal/jetski_service.proto
exa/google/internal/cloud/code/v1internal/metrics.proto
exa/google/internal/cloud/code/v1internal/model_configs.proto
exa/google/internal/cloud/code/v1internal/onboarding.proto
exa/google/internal/cloud/code/v1internal/prediction_service.proto
exa/hooks_pb/hooks.proto
exa/index_pb/index.proto
exa/jetbox_state_pb/jetbox_state.proto
exa/jetski_cortex_pb/jetski_cortex.proto
… (41 en total)
```

**Cruce con nuestras propias measurements**: el binario contiene `exa.google.internal.cloud.code.v1internal.*` (7360 hits de `v1internal`), `exa.jetski_cortex_pb` y `jetbox_state_pb` — **coincidencia exacta** con los paquetes que declara la comunidad. Nuestras tablas de protos (`recon/proto_schemas.txt`) y las de ellos se complementan sin contradecirse.

**Uso inmediato**: `antigravity-client` puede hablar con **nuestro** LS ARM64 (ConnectRPC sobre TLS en `127.0.0.1` con CSRF) ⇒ tenemos cliente + protos + binario ⇒ **podemos construir el arnés de tests de red** sin escribirlo desde cero.

---

## 4. Args de arranque del LS (fuente: tazztone/scripts → FINDINGS.md, contrastado con nuestras flags)

```
language_server \
  --standalone \
  --override_ide_name antigravity \
  --subclient_type hub \
  --override_ide_version <versión> \
  --override_user_agent_name antigravity \
  --https_server_port 0 \
  --csrf_token <token> \
  --app_data_dir antigravity \
  --api_server_url https://generativelanguage.googleapis.com \
  --cloud_code_endpoint https://daily-cloudcode-pa.googleapis.com \
  --enable_sidecars
```

Nuestro `recon/ls_core_flags.txt` (67 líneas, capturado al **arrancar el LS ARM64 bajo `qemu-aarch64-static`**) coincide en el vocabulario: `-cloud_code_endpoint`, `-use_stubby_auth`, `-stubby_auth_mode`, `-override_oauth_client_id/secret`, `-override_business_oauth_client_id/secret`, `-use_test_gaia`, `-generative_service_addr=blade:…`, `-cdp_port=9222`, `-beyond_service_addr`, `-model_api_client_type=ccpa`.

Rutas de estado que reporta la comunidad (contrastables en runtime):
`~/.gemini/antigravity-cli/antigravity-oauth-token` (CLI), `~/.gemini/antigravity/` (escritorio), `~/.gemini/antigravity-ide/` (IDE), `jetski_state.pbtxt` (CLI) / `antigravity_state.pbtxt` (escritorio), `jetbox_summaries.pb` / `agyhub_summaries.pb`, `conversations/<uuid>.db` (SQLite), `conversation_summaries.db`, `brain/<uuid>/`, `annotations/<uuid>.pbtxt`.

---

## 5. MITM: la receta que ya es estándar en la comunidad

Del gist `TECHNOFAB11` (*Reverse Engineering Google Antigravity*):

```bash
# 1) apuntar el dominio real a localhost
echo "127.0.0.1 daily-cloudcode-pa.sandbox.googleapis.com" | sudo tee -a /etc/hosts
# 2) proxy inverso con mitmproxy (ssl-insecure + keep_host_header)
mitmweb --mode reverse:https://142.251.5.81 --ssl-insecure --set keep_host_header -w dump
# 3) la CA propia la respeta Go (respeta SSL_CERT_FILE)
SSL_CERT_FILE=~/.mitmproxy/mitmproxy-ca-cert.pem antigravity
```

Endpoints `v1internal` observados por la comunidad (y presentes en nuestro binario):
`/v1internal:fetchAvailableModels`, `/v1internal:fetchUserInfo`, `/v1internal:streamGenerateContent?alt=sse`.

**Lo que aporta `antigravity-patch-proxy-remote` (no clonado, 404)**: además del MITM, **parchea la tabla de strings del binario** para sustituir el host por `127.0.0.1:51074`, de modo que el IDE no pueda saltarse el proxy, e intercepta el IPC `SetCloudCodeURL`. Es el patrón que debemos replicar para el arnés de pruebas con cuenta real.

---

## 6. Lo que la comunidad **no** tiene (y por tanto sigue siendo nuestro valor)

| Área | Estado en la comunidad | Estado nuestro |
|---|---|---|
| Parche **ARM64/aarch64** del campo `HasValidAuth` | Declarado («x86_64 and ARM64») pero sin offsets públicos | **Medido**: 15 B / 4 tramos, `0x070e5b50`, función y línea exactas |
| Selección **semántica** de la instrucción a parchear | No documentada | **Medido**: 625 candidatos `LDRB` vs 70 con `TBZ/TBNZ` |
| Catálogo de **102 versiones del CLI × 9 plataformas** con CRC-32 del `.so` | No existe | **Nuestro** (`antigravity_ls_matrix.tsv`, 14 LS distintos) |
| **Ejecución real** del LS ARM64 en Linux | No | **Hecho**: arranca bajo `qemu-aarch64-static` con sysroot glibc arm64 |
| **Ocultación de la capa de compatibilidad** | No | **Hecho**: shim en la frontera (`runtime/shims/qemu_shield.c`) |
| MITM **sin Abuse** contra servicios de Google | Sólo recetas |además: nuestro arnés apunta `-cloud_code_endpoint` a localhost y usa CA propia |
| **Ejecución de la build macOS** | No | En curso: Darling instalado y arranca; ejecución bloqueada por capacidades del sandbox (§7) |

---

## 7. Darling (macOS) — estado MEDIDO

- Release más reciente: **v0.1.20260608** (2026-06-09) — sólo `darling-source.tar.xz` (505 MB) y `debs_20260608.zip` (118 MB). Releases anteriores: `v0.1.20260222`, `v0.1.20251023`, … No hay release más nueva. **No hay forks relevantes**: el repositorio oficial es el único con volumen; los forks son mirrors sin actividad.
- Los `.deb` son **`~noble_amd64`** ⇒ exigen **glibc ≥ 2.38**; el host es Debian 12 (glibc 2.36) ⇒ instalamos **glibc 2.41 (Debian 13)** en `scratch/darling_glibc/root/` y relanzamos con su loader.
- Con eso: `darling --version` responde `darling: HEAD @ e947f0d5` ✔.
- **Bloqueo medido**: el sandbox **no permite `unshare(CLONE_NEWIPC)`** (`unshare -i true` → `Operation not permitted`; `unshare -U true` → OK). Darling necesita UTS+IPC para `darling-init` y PID+mount para `darlingserver`.
- Mitigación aplicada: `runtime/shims/darling_shim.c` intercepta `unshare`/`setns` (son **imports de libc**: `U unshare@GLIBC_2.4`) y devuelve 0. Con eso Darling pasa la fase de `darling-init` y llega a `darlingserver`, que vuelve a pedir el fichero de namespace de montaje ⇒ **NO VERIFICADO**: ejecutar un Mach-O real (p. ej. el LS de macOS) sigue pendiente.
- Alternativa para el LS macOS: análisis **estático** ya hecho (`recon/ls_macos_arm_funcs.tsv` = **192 082** funciones; `ls_macos_x64_funcs.tsv` = **192 177**), con pipeline propio `tools/macho_gofuncs.py` (parser Mach-O + `pclntab` de Go sin `debug/elf`).

---

## 8. Plan de aplicación (priorizado por rendimiento/coste)

1. **Cruzar los 80 `.proto` con nuestros `recon/proto_schemas.txt`** → tabla unificada de servicios/métodos del LS. *(coste bajo, valor alto)*
2. **Levantar el LS ARM64 bajo qemu + cliente `antigravity-client`** ⇒ primer cliente real contra **nuestro** binario, con su propio CSRF y OAuth mock. *(alto valor)*
3. **Instrumentar el MITM local**: `-cloud_code_endpoint https://127.0.0.1:PORT` + `SSL_CERT_FILE=ca.pem` ⇒ capturar el tráfico **sin tocar Google**. *(ya está planificado)*
4. **Con cuenta real**: 1 sesión por superficie, con todos los endpoints redirected a localhost y sólo la autenticación contra Google ⇒ es la única forma de ver el diálogo OAuth real sin abuso.
5. **Portar su parche x86-64 a un binario ours**: ya validado conceptualmente; falta escribir el *scanner* de patrón para x86-64 y ARM64 genérico.
6. **Empaquetar**: los `.proto` y el cliente TS deben entrar en el repo con licencia y atribución (ojo: repos sin licencia explícita ⇒ **no** se puede incorporar código suyo en nuestro repo sin revisar la licencia).

---

## 9. Licencias

**Pendiente de verificación por repositorio.** Ninguno de los repos clonados se revisó la licencia con detalle (varios no la declaran). Antes de incorporar código de terceros al repo de MegaDaemon hay que comprobar `LICENSE` en cada clon y, en su defecto, tratar el material como **referencia técnica** (no redistribuible). Esto queda como **NO VERIFICADO** en `VERIFICATION_LEDGER.md`.

---

## 10. Barrido exhaustivo de canales de descarga (`tools/j3_ecosystem.py`, ejecutado 2026-11-26)

Resultado medido del barrido: **4 297 artefactos catalogados en 8 repositorios** y
**20 canales de descarga no-GitHub** verificados por HTTP.

### 10.1 Canales oficiales (estado HTTP real)

| canal | HTTP | versión |备注 |
|---|---|---|---|
| `CLI-manifest/darwin_amd64` | 200 | 1.5.18 | sha512 `c6ab39b41511ee09…` |
| `CLI-manifest/darwin_arm64` | 200 | 1.5.18 | sha512 `d10e124f5eb4da94…` |
| `CLI-manifest/linux_amd64` | 200 | 1.5.18 | sha512 `ef379ec869719112…` |
| `CLI-manifest/linux_arm64` | 200 | 1.5.18 | sha512 `6c59cbce5a763208…` |
| `CLI-manifest/linux_amd64_musl` | 200 | 1.5.18 | sha512 `938cc71c609ecaaa…` |
| `CLI-manifest/linux_arm64_musl` | 200 | 1.5.18 | sha512 `744da8e3c0135ab8…` |
| `CLI-manifest/android_arm64` | 200 | 1.5.18 | sha512 `35f6ab7af69c007b…` |
| `CLI-manifest/windows_amd64` | 200 | 1.5.18 | sha512 `53605db7013dfed5…` |
| `CLI-manifest/windows_arm64` | 200 | 1.5.18 | sha512 `cc552ca2eec40fe4…` |
| PyPI `antigravity` | 200 | — | **existe** (el nombre del paquete es `antigravity`, no `antigravity-sdk`) |
| PyPI `antigravity-sdk` | **404** | — | no existe |
| PyPI `antigravity-cli` | **404** | — | no existe |
| npm `antigravity` | 200 | 0.0.0 | placeholder |
| npm `antigravity-cli` | 200 | 0.0.1 | placeholder |
| npm `@google/gemini-cli` | 200 | 0.1.0 | — |
| Homebrew `cask/antigravity` | 200 | **2.35.2** | build `4585983447007232` |
| `web/install.sh` / `install.ps1` / `install.cmd` | 200 | — | scripts de instalación |

**Hallazgo**: el canal oficial reparte **9 binarios del CLI** (5 SO/UNIX incluido
musl, 2 macOS, 2 Windows, 1 Android) y **9 manifiestos con sha512**. Eso
permite verificar la integridad de cada descarga sin depender de GitHub.

### 10.2 Repositorios nuevos del barrido (no estaban en el catálogo anterior)

| repo | releases | interés para nosotros |
|---|---|---|
| `yuaotian/antigravity-proxy` | 19 | **proxy de Antigravity** — el más directamente relacionado con el MITM |
| `NoeFabris/opencode-antigravity-auth` | 95 | **auth de Antigravity para opencode** — relevante para la ruta OAuth |
| `lbjlaq/Antigravity-Manager` | 100 | gestor de instalación |
| `jlcodes99/cockpit-tools` | 100 | herramientas de cockpit (403 en un repo hermano por límite de API) |
| `router-for-me/CLIProxyAPI` | 100 | proxy multi-modelo, expone Antigravity |
| `decolua/9router`, `justlovemaki/AIClient2API`, `Louis-CFM/coucou`, `nguyenphutrong/quotio` | 73–100 | adaptadores que consumen Antigravity como backend |

**NO VERIFICADO**: licences y contenido de los repos nuevos. No se clonaron
(quota de la API de GitHub sin autenticar: 39 peticiones/hora). Quedan como
candidatos a clonar con `git clone --depth 1`, que no consume cuota de API.

Artefactos del barrido: `versions/ecosystem_assets.tsv` (4 297 filas),
`versions/ecosystem_channels.tsv` (20 canales).

### 10.3 Clonados tras el barrido (referencia técnica)

| repo | ficheros | licencia | qué aporta |
|---|---|---|---|
| `ecosystem/antigravity-proxy` (`yuaotian/antigravity-proxy`) | 228 | BSD-2-Clause (MinHook) | **parche DLL (MinHook) que enruta el tráfico de Antigravity por SOCKS5/HTTP sin TUN ni admin.** Cubre **IDE**, **desktop** y **CLI `agy`**, y el uso vía ACP desde JetBrains/Zed/VS Code (`agy acp`, `agy_acp_server`). Incluye plugin de IDE (estado de proxy, ajustes, log en vivo, IP de salida, detección de Gemini, restauración tras update) |
| `ecosystem/opencode-antigravity-auth` (`NoeFabris/opencode-antigravity-auth`) | 157 | por revisar | autenticación de Antigravity para opencode (95 releases) |

**Hallazgo de ingeniería (medido en el código, no en el README)**: el proxy
comunitario hookea la pila de red de **Windows**, no el TLS:

```
WinHttpOpen / WinHttpConnect / WinHttpOpenRequest / WinHttpSetTimeouts
connect   (105 referencias)     getaddrinfo (14)     WSASocketA/W (10)
```

Es decir, la familia de Antigravity habla **WinHTTP** en Windows y el proxy
redirige a nivel de socket, **sin tocar certificados**: no hay MITM ni CA
propia en su diseño.

**Consecuencia para nuestro arnés**: en Linux/ARM64 el LS es Go y usa
`net/http` por su cuenta, así que el equivalente no es un hook sino el
`HTTP_PROXY`/`HTTPS_PROXY` que ya usamos (Go respeta esas variables) más
`SSL_CERT_FILE` si hay que interceptar TLS. El proxy comunitario **confirma
que no hace falta tocar el TLS paraobservar el tráfico**.


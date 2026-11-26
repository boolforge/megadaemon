# Arnés de captura local del Language Server de Antigravity

Ejecuta el **Language Server auténtico** (ARM64, `liblanguage_server.so` v2.4.1)
dentro de este sandbox, apuntando **todo** su tráfico a un servidor local que
graba cada petición, y deja el estado que escribe en disco para inspeccionarlo.

Todo es **local**: cero peticiones a servidores de Google, cero uso de
credenciales reales. Ver "Aislamiento" más abajo.

## Ficheros

| fichero | función |
|---|---|
| `tools/agy_harness/fake_cloudcode.mjs` | servidor HTTP/2 cleartext (h2c) + HTTP/1.1 que graba cada petición (método, ruta, cabeceras, cuerpo en hex, campos protobuf decodificados) y responde con tramas gRPC |
| `tools/agy_harness/launch_ls.sh` | lanza el LS real bajo `qemu-aarch64-static` + PTY, con el aislamiento de red y la siembra del token |
| `recon/captures/` | capturas y logs de cada ejecución |

## Uso

```bash
cd /workspace/antigravity
bash tools/agy_harness/launch_ls.sh --port 8085 --timeout 90

# con un token OAuth REAL (JSON), para llegar a la API:
TOKEN_FILE=/ruta/token.json bash tools/agy_harness/launch_ls.sh --port 8085
```

## Tres requisitos MEDIDOS (sin ellos el LS muere con rc=1 y sin mensaje)

1. **PTY.** El LS hace `ioctl(1, TCGETS)` al arrancar; si stdout no es una
   terminal devuelve `ENOTTY` y sale con código 1. Sin PTY el `--help` no
   imprime nada. ⇒ `script -qec "<cmd>" /dev/null`.
2. **Bit de ejecución.** El binario extraído del APK viene con modo `0644`;
   qemu-user responde `Error while loading ./language_server: Permission
   denied`. ⇒ `chmod +x`.
3. **Copia local.** El NFS del workspace no sirve para el mapeo ejecutable
   del cargador invitado. ⇒ copiar el binario y el sysroot a disco local
   (`/tmp/agy-harness`).

Además, `qemu-aarch64-static` **no imprime ningún error** (ni siquiera con
`-strace` ni para un fichero inexistente): el fallo hay que diagnosticarlo con
`strace` sobre el proceso qemu del host.

## Aislamiento de red (garantía de "no detectable por servidores")

- Todos los endpoints se apuntan a `127.0.0.1:<puerto>`:
  `-api_server_url`, `-cloud_code_endpoint`, `-analytics_server_url`,
  `-inference_api_server_url`.
- `HTTP_PROXY`/`HTTPS_PROXY` = `http://127.0.0.1:1` (puerto cerrado) y
  `NO_PROXY=localhost,127.0.0.1,::1`. El cliente HTTP de Go respeta esas
  variables y **excluye loopback del proxy**, luego: lo local va directo y
  cualquier host externo falla al instante (connection refused). Cero
  paquetes salen a Internet.
- No se usa ninguna credencial de Google.

## Lo que el LS hace realmente (medido en ejecución)

Arranca y se inicializa:

```
server.go:1624] Starting language server process with pid …
server.go:1675] Language server version: 2.4.1
server.go:643]  Language server listening on random port at 35715 for HTTPS (gRPC)
server.go:651]  Language server listening on random port at 35915 for HTTP
server.go:2605] initialized server successfully in 835.69994ms
```

Migraciones que ejecuta al primer arranque (id + resultado medidos):

| migración | estado |
|---|---|
| `MIGRATION_ID_SIDECAR_USER_CONFIG_BYPASS` | disabled (se salta) |
| `MIGRATION_ID_SANITY_CHECK_PROJECT_URIS` | ya COMPLETED |
| `MIGRATION_ID_PLUGIN_ENABLEMENT` | ya COMPLETED |
| `MIGRATION_ID_BUILTIN_SIDECARS_TO_PLUGINS` | ya COMPLETED |
| `MIGRATION_ID_LINE_BUILTIN_TO_MARKETPLACE` | disabled |
| `MIGRATION_ID_ADOPT_BWG_INSTALLS` | disabled |
| `MIGRATION_ID_PERMISSION_GRANTS_V2` | disabled |

Otros mensajes: `[RemoteControl] RemoteControlEnabled value: false`,
`[RemoteControl] No signed-in account, staying disconnected`,
`[Summaries] reconcile checked 0 conversations …`.

### Ficheros de estado que escribe (rutas MEDIDAS con strace)

```
$HOME/.gemini/jetski-standalone-oauth-token      <- token OAuth (JSON)
$HOME/.gemini/antigravity/antigravity_state.pbtxt   (escritura atómica: .tmp + rename)
$HOME/.gemini/antigravity/conversation_summaries.db  (SQLite, WAL)
$HOME/.gemini/antigravity/conversation_summaries.db-{journal,shm,wal}
$HOME/.gemini/antigravity/conversations/            $HOME/.gemini/antigravity/brain/
$HOME/.gemini/antigravity/builtin/                  $HOME/.gemini/antigravity/crashes/
$HOME/.gemini/antigravity-ide/installation_id
$HOME/.gemini/antigravity/agyhub_summaries.pb      (gyhub_summaries_proto.pb, Intentados)
```

### Esquema real de la base SQLite (medido)

`conversation_summaries` (PRIMARY KEY `conversation_id`), 21 columnas:

```
conversation_id text | title text | preview text | step_count integer
last_modified_time datetime | workspace_uris text | status text | source text
project_id text | agent_name text | parent_conversation_id text
nesting_depth integer | battle_id text | winning_conversation_id text
not_fully_idle numeric | killed numeric | last_user_input_time datetime
last_user_input_step_index integer | app_data_dir text | raw_summary blob
group_id text
```

`battle_id` + `winning_conversation_id` + `nesting_depth` + `group_id`
confirman en el esquema la existencia de **combates entre agentes** (el
agente perdedor queda marcado) y de agrupación de conversaciones.

## BLOQUEO MEDIDO: la puerta de autenticación

Con el token local falso el LS registra:

```
errorreport.go:224] error getting token source: You are not logged into Antigravity.
model_configs.go:284] Auth mode is unspecified, skipping fetchAvailableModels and returning empty response
```

y **no emite ninguna petición** al servidor local: el `strace` confirma que
abre y lee `~/.gemini/jetski-standalone-oauth-token` (fd 13) pero rechaza el
contenido. Se probaron JSON con las etiquetas estándar (`access_token`,
`refresh_token`, `id_token`, `token_type`, `expiry`, `scope`) y un `id_token`
JWT sintácticamente válido, y el resultado es el mismo.

**Conclusión**: el LS exige un token **real** (validado por Google). No es
falseable sin una cuenta. En cuanto haya un token real, el mismo comando
captura la petición gRPC completa contra el servidor local.

**NO VERIFICADO**: el mecanismo exacto de validación (firma del `id_token`
contra las claves públicas de Google, o refresco contra `accounts.google.com`
— que además está bloqueado por el aislamiento). Con `HTTP_PROXY` a
`127.0.0.1:1` un refresco real no puede completarse, de modo que el arnés
necesita el token ya renovado.

## Recomendación para el resto de la instrumentación

| objetivo | cómo |
|---|---|
| red | este arnés (servidor local + captura) |
| base de datos | `sqlite3` sobre `conversation_summaries.db` tras cada ejecución; el esquema ya está medido |
| estado | parsear `antigravity_state.pbtxt` (proto text) |
| GUI | `-cdp_port=9222` para CDP; el LS ya expone el flag (medido en `--help`) |

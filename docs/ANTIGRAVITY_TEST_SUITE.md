# Suite de pruebas automática del Language Server de Antigravity

`tools/agy_tests/run.py` + `tools/agy_tests/testproj/`.

- **Suite**: `python3 tools/agy_tests/run.py` (local) · `--online` (real).
- **Proyecto real de test**: `tools/agy_tests/testproj/` (Rust + Python + docs).
- **Arnés**: `tools/agy_harness/` (servidor local + lanzador del LS real).

---

## 1. El contrato del protocolo, MEDIDO (no supuesto)

Capturado el **2026-11-26** conduciendo el Language Server real contra el
servidor local. Esto no es de la documentación del vendor: es lo que el
binario hace.

```
POST /v1internal:<método>            HTTP/1.1
Host: 127.0.0.1:<puerto>
User-Agent: antigravity/hub/2.4.1 (aidev_client; os_type=linux; arch=arm64; cl=998057262)
Authorization: Bearer <access_token real>
Content-Type: application/json
Content-Length: <n>

{"metadata":{"ideType":"ANTIGRAVITY"}}
{"project":"<id>","metadata":{"updateChannel":"stable"}}
{"project":"<id>"}
GET /v1internal:buildWithGooglePlugins?page_size=1000
```

Puntos que costaron sangre y que conviene no volver a discutir:

| hecho | evidencia |
|---|---|
| **HTTP/1.1**, no h2 | captura cruda del socket (`POST … HTTP/1.1`) |
| **protobuf-JSON**, no protobuf binario | `Content-Type: application/json` y cuerpos que son JSON legible |
| **Connect**, no gRPC con trama de 5 bytes | si se responde con trama gRPC el cliente falla con `malformed HTTP response "\x00\x00\x00\x04"` |
| ruta `POST /v1internal:<método>` | igual que el servicio `exa.language_server_pb` de la comunidad |
| el `project` de las llamadas siguientes **lo toma de la respuesta** de `loadCodeAssist` | el servidor devolvió `cloudaicompanionProject: "local-harness-project"` y las siguientes peticiones llevaron ese mismo valor |

### Superficie RPC descubierta (medida, 5 métodos)

```
v1internal:loadCodeAssist
v1internal:listExperiments
v1internal:fetchUserInfo
v1internal:fetchAvailableModels
v1internal:buildWithGooglePlugins      (GET, ?page_size=1000)
```

El servidor local responde `{}` a los métodos sin fixture y **los marca**:
así la suite enumera la superficie real en vez de suponerla.

---

## 2. Estado de la suite

**12/12 en verde** (modo local, cero tráfico a Google).

| test | qué comprueba |
|---|---|
| T01 | el LS arranca y se inicializa |
| T02 | handshake completo: `loadCodeAssist → listExperiments → fetchAvailableModels → fetchUserInfo` |
| T03 | toda petición lleva `Authorization: Bearer` |
| T04 | `User-Agent` real del cliente, **no falsificado** |
| T05 | el cuerpo JSON tiene la forma medida de cada RPC |
| T05b | el `project` encadenado viene de la respuesta de `loadCodeAssist` |
| T06 | el servidor responde 200 a todos los RPC |
| T07 | el LS escribe `antigravity_state.pbtxt` |
| T08 | el LS escribe `conversation_summaries.db` |
| T09 | el esquema SQLite es el medido: **21 columnas** con `battle_id` y `winning_conversation_id` |
| T10 | **sin token el LS no emite ninguna petición** (la puerta de auth es real) |
| T11 | la captura es JSONL íntegro y reutilizable |

Informe: `recon/testruns/resultado.json` (JSON, una entrada por test).

---

## 3. El proyecto real de test

`tools/agy_tests/testproj/` no es un toy: es un proyecto Rust + Python + docs
con un **bug de lógica plantado** y tests que lo detectan.

```
testproj/
├── Cargo.toml
├── src/lib.rs          normalize / argmax / moving_average
├── tests/basics.rs     6 tests de integración
├── python/cli.py       consume la lib Rust como caja negra
├── docs/ARCHITECTURE.md
├── TASKS.md            guion de tareas T-A..T-E para el agente
└── README.md
```

El bug plantado (T-A) está en `moving_average`: la ventana se calcula con
`i.saturating_sub(w)` en vez de `i.saturating_sub(w - 1)`, así que promedia
`w+1` elementos. Estado medido: `cargo test` → **5 pasan, 1 falla**
(`moving_average_es_la_media_de_la_ventana`).

Las tareas de `TASKS.md` ejercitan lo que de verdad se le pide a un agente:
leer, editar, ejecutar comandos, entender un proyecto multi-lenguaje, dejar
el resultado **en el árbol** y no en el chat, y dejar constancia de la
verificación.

**Regla anti-trampa**: el README dice que el bug está plantado y que el
proyecto **no** se toca para "hacer que pase". Si el agente "arregla" el test
en vez del código, la suite debe fallar.

---

## 4. Modo ONLINE: prudente por diseño

```
python3 tools/agy_tests/run.py --online --min-interval 25 --max-requests 10
```

Por qué es prudente y no agresivo:

1. **No se sondea nada.** No hay crawler, no hay barrido de endpoints, no hay
   reintentos masivos. Una llamada por prueba.
2. **El cliente es el LS real.** Mismo User-Agent, mismo OAuth, mismo TLS,
   misma librería HTTP. El tráfico es indistinguible del de un usuario normal
   porque **lo produce el producto**, no un script nuestro.
3. **Cadencia**: 1 petición cada vez, nunca en paralelo; intervalo mínimo
   con jitter uniforme; tope duro por ejecución.
4. **Ante 429/5xx**: backoff exponencial y abandono al tercer intento.
5. **Sin identidad falsa**: no se falsifica User-Agent, ni se reenvían
   peticiones ajenas, ni se imitan endpoints que no sean los del producto.
6. **Auditoría**: cada petición queda en JSONL con método, path, código y
   latencia.
7. **Opt-in explícito**: por defecto no se toca la red de Google.

Lo que **no** se hace, y no se hará sin que lo pidas: carga sostenida,
concurrencia, reintentos agresivos, ni nada que un clasificador de abuso
pudiera leer como anomalous.

---

## 5. Requisitos medidos para ejecutar el LS aquí

Tres, y sin ellos el proceso sale con rc=1 y **sin ningún mensaje**:

1. **PTY** — el LS hace `ioctl(1, TCGETS)` y aborta con `ENOTTY` si stdout
   no es una terminal ⇒ `script -qec`.
2. **Bit +x** — el binario del APK viene 0644 ⇒ `qemu-user` responde
   `Error while loading …: Permission denied`.
3. **Copia local** — el NFS del workspace no sirve para el mapeo ejecutable
   del cargador invitado.

Extra: `qemu-aarch64-static` **no imprime ningún error** (ni con `-strace`):
el diagnóstico hay que hacerlo con `strace` sobre el proceso qemu del host.

---

## 6. Token

`tools/agy_auth.py` acuña un token real a partir de un
`application_default_credentials.json` de tipo `authorized_user`
(`client_id` + `client_secret` + `refresh_token`) y lo instala en
`~/.gemini/jetski-standalone-oauth-token` con modo 0600.

- `python3 tools/agy_auth.py mint --adc RUTA --install`
- `python3 tools/agy_auth.py show` → sólo metadatos, **nunca el token**.

Medido: `expires_in` 3599 s (1 h) y `refresh_token_expires_in` 177 858 s
(~49 días). Una sola petición a `oauth2.googleapis.com/token` porrefresh.

En el móvil: `tools/agy_install_token.sh RUTA_ADC` (root) instala el token
en **todas** las rutas `.gemini` que encuentra y **no imprime ningún valor**.

---

## 7. Reproducir

```bash
cd /workspace/antigravity
python3 tools/agy_tests/run.py                    # 12/12 local
cd tools/agy_tests/testproj && cargo test         # 5 OK, 1 KO (bug plantado)
bash tools/agy_harness/launch_ls.sh --port 8085 --timeout 60   # ver el LS
```

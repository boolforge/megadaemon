# Cómo MegaDaemon domina el entorno de Antigravity (sin que lo borre)

> Todo lo de aquí está **medido o es código verificable**. Lo que aún no está
> aplicado en el móvil va marcado `PENDIENTE`. Nada de "fantasmadas".

## 0. El estado real (medido)

| Qué | Dato |
|---|---|
| Binario del language server | `/data/user/0/com.agy/files/runtime/bin/liblanguage_server.so` (181.358.800 B, sha256 `f25135e4…70ff27`) |
| Endpoint original | `https://daily-cloudcode-pa.googleapis.com/v1internal:generateContent` |
| Offset medido del parche | 71.497.897 (42 B) ⇒ destino `http://127.0.0.1:9090` |
| MCP canónico | `127.0.0.1:9090/mcp` |
| Puerto del LS | `127.0.0.1:45157` (**no** lo tocamos: lo usa Antigravity) |
| DB de Antigravity | `conversations/conversation_summaries.db` |
| Paquete del gateway | `third_party/jetski/gateway/` (4 ficheros, 81 funciones) |
| Catálogo de modelos | `gateway/admin_settings.go:85` → `AdminFileSetsModels` |

## 1. Los cinco puntos de control (y por qué Antigravity no puede quitarlos)

### 1.1 Red: el binario **ya** apunta a nosotros
`antigravity_endpoint.rs` reescribe la URL **dentro del binario** ⇒ todo lo que
el LS pide a Google pasa por `127.0.0.1:9090` (nuestro gateway).

* Anti-borrado: el parche está **en el fichero**, no en una config ⇒ Antigravity
  no lo revierte (no sabe que existe).
* Si alguien restaura el binario (actualización de la app), el **watchdog**
  (§2) lo vuelve a parchear y lo verifica por hash.

### 1.2 SQLite: la DB "mentirosa" montada encima
`sqlite_mitm.rs` genera una copia de la DB de Antigravity con las filas que
queremos (ocultas o reescritas) y da el plan de montaje:

```
mount --bind '<servida>.db' '<original>.db'
umount '<original>.db'      # revertir
```

* El **original nunca se toca**: se abre sólo en lectura (con `busy_timeout`).
* El bind-mount hace que Antigravity lea/escriba **nuestra** copia.
* Si Antigravity borra una fila, se borra en la copia, no en el original.

### 1.3 Ficheros de estado: overlayfs
`antigravity_state.pbtxt` (el BYOM persistido) y `custom_models.json` se
sirven con un **overlay** con nuestra capa superior ⇒ el estado que escribe
Antigravity se guarda en nuestro y, si él lo borra, vuelve el nuestro.
`PENDIENTE`: falta el script de montaje en el móvil.

### 1.4 Reglas de tráfico: `agyshim`
`agyshim.rs` es un MITM de red con reglas por host/ruta: **pasar, sustituir,
inyectar JSON, sustituir texto, bloquear**. Sus reglas viven en
`state/agyshim_rules.json` y el log en `state/agyshim.ndjson`.

### 1.5 El propio daemon: el móvil lo necesita para hablar con la GUI
Antigravity usa el **MCP de la app**; MegaDaemon es quien publica el
catálogo vivo de tools (62 frente a 26 congeladas) y el gateway. Si
MegaDaemon muere, el catálogo vuelve a las 26 ⇒ **el móvil lo necesita**.

## 2. El watchdog (Magisk `service.sh`) — lo que garantiza que "siempre manda"

```
while true; do
    # 1. el daemon vivo
    pgrep -f megadaemon-core || iniciar_daemon
    # 2. el parche del endpoint intacto (por hash del fichero)
    verificar_parche_endpoint || reaplicar_parche
    # 3. los montajes (bind/overlay) en su sitio
    verificar_montajes || remontar
    sleep 30
done
```

* Vive en `/data/adb/service.d/` ⇒ arranca con el boot, **antes** de la app.
* Corre como root ⇒ puede remontar si algo los tumba.
* Cada acción deja rastro en `state/watchdog.log`.

## 3. Lo que NO se hace (y por qué)

* **No** se mata ni se reinicia el language server a ciegas: la GUI lo relanza
  y perderíamos el estado de las sesiones.
* **No** se escribe nunca en la DB de Antigravity: se replica.
* **No** se parchea el APK de la app: sólo el binario del runtime (que se
  reinstala, así que el watchdog lo repara).
* **No** se falsifica attestation ni nada de Play Integrity: el modelo real
  sigue estando detrás de `megadaemon` y BYOM en Android está limitado por el
  propio producto (`supportsCustomModels` sólo en Linux interno).

## 4. El plan de sitio, fichero a fichero

| Capa | Fichero | Estado |
|---|---|---|
| Red (gateway OpenAI/Gemini) | `rust-core/src/llm_gateway.rs` | **hecho** |
| Parche del endpoint en el binario | `rust-core/src/antigravity_endpoint.rs` | **hecho** (offset medido) |
| Reglas de MITM | `rust-core/src/agyshim.rs` | **hecho** (núcleo + log + reglas) |
| Rutas reales del MITM | `cockpit_server.rs` | **hecho** (`/v1internal/*`, `/proxy/*`) |
| MITM de SQLite | `rust-core/src/sqlite_mitm.rs` | **hecho** (DB propia + réplica + DB servida + plan de montaje) |
| Sesiones de Antigravity | `rust-core/src/antigravity_sessions.rs` | **hecho** (lectura RO + árbol + logs) |
| Motor RE / deofuscación | `re_engine.rs`, `re_strings.rs`, `re_ir.rs`, `re_cff.rs`, `re_crypto.rs`, `re_opaque.rs` | **hecho** (tests contra fragmentos reales) |
| El "libro" del binario | `rust-core/src/gobook.rs` | **hecho** (mapa, búsqueda, esqueleto) |
| Watchdog de arranque | `tools/` (script Magisk) | **PENDIENTE** |
| Overlay de ficheros | — | **PENDIENTE** |
| Hooks Frida/Zygisk (SQLite, red, ficheros) | — | **PENDIENTE** |

## 5. Cómo se comprueba (sin "fantasmadas")

* `curl -s http://127.0.0.1:9090/health` → el daemon vivo, con `key_fp` y cuota.
* `curl -s http://127.0.0.1:9090/api/quota` → nivel de cuota (`normal|aviso|ahorro|pausa`).
* `curl -s http://127.0.0.1:9090/mcp` → **62 tools** en el catálogo vivo.
* El parche del endpoint: se comprueba el sha256 del binario tras parchear y
  desparchear ⇒ un parche no-op **falla** (medido, `csc_patch`).
* El MITM de SQLite: la DB servida se abre y se comprueba que tiene las filas
  esperadas **antes** de montarla.

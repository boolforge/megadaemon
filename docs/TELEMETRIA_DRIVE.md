# Telemetría a Drive + autodetección de "listo" — módulo `telemetria`

> Entregable del encargo del 2026-11-26: *«que suba muchísima más telemetría
> automáticamente a Google Drive y que autodetecte cuando esté listo y suba los
> datos de conexión a Google Drive, y tú ya te conectes y hagas lo que tengas
> que hacer»*.
>
> Código: `rust-core/src/telemetria.rs` (módulo propio, 100 % Rust, sin
> dependencias nuevas). Tool MCP: `telemetria`. CLI:
> `megadaemon-core telemetria <accion>`.

---

## 1. El problema que resuelve

MEDIDO (2026-11-26): el **cuello de botella del enjambre no es el protocolo,
es el canal**.

| hecho medido | consecuencia |
|---|---|
| el túnel del móvil es un *quick tunnel* de Cloudflare ⇒ **el hostname cambia en cada relanzamiento** | el sandbox se queda ciego sin avisar |
| el relé canónico `mdtmfnc.duckdns.org` devolvió **HTTP 000** (caído de verdad) | no hay plan B |
| el `raw.githubusercontent.com` **cachea en CDN** | el canal leído por ahí puede estar viejo |
| el egress del sandbox está interceptado (`connect()` a `10.60.100.34:9090` se acepta y muere con EOF) | no hay ruta directa al móvil |
| ADB 5555, también interceptado | tampoco por ADB |

La consecuencia operativa era que **cerrar la instrumentación en vivo exigía
que alguien conectara a mano** y esperara: si el móvil volvía, el resto de la
flota no se enteraba.

La telemetría a Drive rompe ese círculo: **Drive no cambia de hostname, la
credencial se refresca sola** (`google_auth.rs`) y **el móvil empuja**. Quien
quiera el estado, lo tira de Drive, desde donde esté.

---

## 2. Qué se recoge

`recoger()` devuelve un JSON **sin secretos**:

| bloque | contenido | fuente |
|---|---|---|
| `bateria` | capacidad %, estado, temperatura, si está en modo ahorro | `/sys/class/power_supply/battery/*` |
| `memoria_bytes` | total, disponible, libre, caché, swap | `/proc/meminfo` |
| `carga` / `uptime_seg` | load 1/5/15 y uptime | `/proc/loadavg`, `/proc/uptime` |
| `red.interfaces` | rx/tx **por interfaz** | `/proc/net/dev` |
| `red.por_transporte` | **wifi vs móvil vs ethernet** (bytes) | clasificación de la interfaz |
| `red.top` | las 5 interfaces con más tráfico, con su tipo | `/proc/net/dev` |
| `instrumentacion` | NDJSON de los hooks: líneas, malformadas, **eventos con retorno** | barrido de `megadaemon-hooks-*.ndjson` |
| `canal` | el `channel.json` local | `channel.rs` |
| `conexion` | URL viva, relé, puerto, build, **sha16 de las claves** | `datos_conexion()` |
| `diagnostico` | el diagnóstico completo que ya existía | `diagnostico.rs` |

### 2.1 WiFi vs datos móviles (la clave para el gasto de Jazztel)

MEDIDO en el Samsung: la interfaz de **datos móviles** es `rmnet_data0`
(o `rmnet_data1`, `ccmni0`… según el módem) y la de **WiFi** es `wlan0`.
La clasificación (`clasificar_interfaz`) es:

```
wlan*, wifi*                -> "wifi"
rmnet*, ccmni*, ppp*, wwan* -> "movil"
eth*                        -> "ethernet"
resto                       -> "otra"
```

Esto da el **desglose por transporte** que pide el encargo de Jazztel, sin
depender de la app del operador: es la señal que el sistema **debe** consumir
y facturar.

> ⚠️ Límite honesto: esto es el tráfico **que ve el sistema**, no el que
> **factura Jazztel**. La diferencia (tráfico en tether, contadores por UID,
> redondeo del operador, periodos facturables) está documentada en
> `docs/RE_GASTO_DATOS_JAZZTEL.md` y requiere medir en el móvil.

---

## 3. Los datos de conexión (lo que quita el "conéctate tú")

`datos_conexion()` publica:

```json
{
  "quick": "https://<aleatorio>.trycloudflare.com",
  "paste": "https://paste.rs/...",
  "canonical": "https://mdtmfnc.duckdns.org/channel.json",
  "puerto_gateway": 9090,
  "build": "b8de8e9",
  "claves_sha16": { "megadaemon_api.key": { "sha16": "…", "bytes": 64 } },
  "ts": 1795690000
}
```

**El `sha16` es un hash, no la clave.** Las claves se comparan sin filtrarse:
dos nodos pueden confirmar que comparten credencial sin que nadie la vea.

Con este fichero, el root sabe **exactamente** a dónde conectarse y con qué
build, sin tener que descubrir el túnel.

---

## 4. Auto-detección de "listo"

`estado_listo()` exige **las tres** condiciones:

1. el binario del daemon existe;
2. hay una URL de túnel publicada;
3. el `channel.json` local es legible.

```
megadaemon-core telemetria listo   # exit 0 si está listo, 1 si no
megadaemon-core telemetria sonda <url>   # exit 0 si responde el daemon
```

El **código de salida** es lo que permite supervisores y scripts sin
parsear JSON. El instalador (`deploy/megadaemon-oneliner.sh`) lo usa como
health-check real en vez de "esperar 10 s y suponer".

---

## 5. Publicación

| vía | para qué | cómo |
|---|---|---|
| `publicar()` | telemetría completa (JSON + Markdown) | `drive_vfs::write_local_file` → subida **nativa** y resumible |
| `publicar_conexion()` | datos de conexión + estado de "listo" | ídem, en `MegaDaemon/conexion/<nodo>/` |
| `subir_artefacto()` | binarios e instrumentación | ídem, en la carpeta que se le pase |
| hilo `publicar_periodico(min)` | ciclo continuo | cada 15 min por defecto (**se duplica en modo ahorro**) |

**Subida nativa, sin `curl`**: el móvil no tiene `curl` ni `wget` (medido:
`toybox wget` → *Unknown command*, rc=127). El cliente es
`drive_native` (Rust puro, `uploadType=resumable`, trozos de 16 MiB, reintentos
con backoff y jitter).

### 5.1 Si Drive falla, no se pierde nada

`publicar()` **escribe primero en disco** (`<state_dir>/telemetria/ultimo.json`
y `.md`) y sólo después sube. Si la subida falla, el estado local queda y el
siguiente ciclo reintenta. El disco es la fuente de verdad; Drive es la
ventana al exterior.

### 5.2 Por qué a Drive y no a GitHub

El repo es **PÚBLICO**. Un NDJSON con procesos, rutas y redes del móvil no
debe acabar ahí. Los artefactos sensibles (binarios de instrumentación,
informes de RE, volcados) van a la carpeta **privada** de Drive
(`15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`).

---

## 6. Interfaz

### Tool MCP

```jsonc
{
  "name": "telemetria",
  "accion": "estado | publicar | conexion | listo | sonda | artefacto",
  "url": "...",        // sonda
  "ruta": "...",       // artefacto
  "nombre": "...",
  "carpeta": "..."
}
```

### CLI (en el móvil, sin MCP)

```sh
megadaemon-core telemetria estado      # el JSON completo
megadaemon-core telemetria publicar    # sube a Drive
megadaemon-core telemetria conexion    # sube los datos de conexión
megadaemon-core telemetria listo       # exit 0/1
megadaemon-core telemetria sonda URL   # exit 0/1
megadaemon-core telemetria artefacto RUTA [NOMBRE]
```

---

## 7. Batteries

* El hilo `publicar_periodico` **duplica el periodo** cuando
  `battery_advanced::modo_ahorro_activo()` (batería < 20 % y sin cargador).
* La configuración del periodo: `telemetria.publicar_min` (por defecto 15).
* La recolección lee `/proc` y `/sys`: ficheros locales, sin red ni CPU
  apreciable.

---

## 8. Verificación (MEDIDO, no supuesto)

Verificado **ejecutando** el binario real (`cargo build --bin megadaemon-core`,
x86_64 en el sandbox) contra la API de Drive de verdad:

| prueba | resultado |
|---|---|
| `telemetria estado` | JSON con `bateria`, `carga`, `memoria_bytes`, `uptime_seg`, `red`, `instrumentacion`, `canal`, `conexion` y `diagnostico` ✅ |
| `red.por_transporte` | desglose correcto: en el sandbox detectó `eth0` → `ethernet_bytes: 312.163.816.888`, `wifi/movil: 0` ✅ |
| `red.top` | lista las interfaces con más tráfico y su tipo ✅ |
| `telemria conexion` | **subido a Drive**: `conexion.json`, 457 B, file_id `1vrQ9cwK2xjN6LBR0seLEN_TY2fsN8Xu9` ✅ (verificado por la API de Drive) |
| `telemetria publicar` | **subido a Drive**: JSON 12.093 B (id `1nunZDP8nWll…`) + Markdown 225 B (id `1DhmZFUDgb2N…`), en `MegaDaemon/telemetria/pc` ✅ (verificado por la API) |
| estado local | `telemetria/ultimo.json`, `ultimo.md`, `indice.json`, `conexion.json` en el state dir ✅ |
| `telemetria listo` | **código de salida 1** cuando no hay daemon (⇒ sirve de health-check a supervisores) ✅ |
| `megadaemon-core hash` | **coincide con `sha256sum`** (49fbc546…) ✅ — el móvil no tiene `sha256sum` |
| `megadaemon-core bus` | sincroniza el bus de Drive real: `bus_ok: true` ✅; sin credenciales explica el motivo exacto ✅ |
| `telemetria lista` en el móvil | `NO VERIFICADO` (el móvil sigue sin red) |

---

## 8 bis. Verificación

* **Tests unitarios** en el propio módulo (9): clasificación de interfaces,
  parseo de `/proc/net/dev`, desglose por transporte, `sha16` (estable,
  distinto por contenido, **no contiene el secreto**), el Markdown, el JSON
  de `recoger()`, la ausencia de claves en claro en los datos de conexión, el
  esquema de la tool, y la sonda con URL vacía.
* **Compilación**: `cargo check --lib --bins --tests` en local; los tests
  completos se validan en el CI (job `calidad`) porque el enlazado local
  excede la RAM del sandbox (3 GB sin swap).
* **En el móvil**: `NO VERIFICADO` hasta que el móvil vuelva a tener red.
  Los comandos exactos están en el propio one-liner (`--diagnostico`).

---

## 9. El bus de órdenes (control sin conexión directa)

Este módulo es el **Nivel 0** (empujar el estado). El **Nivel 2** (tirar
órdenes) lo implementa `rust-core/src/bus_ejecutor.rs`, y ambos se apoyan en
el mismo bus por Drive (`rust-core/src/resonance.rs`, que YA existía: cartas
firmadas Ed25519, `id = sha256(canonico)`, CRDT LWW con `epoch`+`seq`, en la
carpeta privada `tablon-resiliencia`).

```
                 raíz (sandbox)                    móvil
                      │                              │
        publica carta │                              │  tira del bus
        tipo="orden"   │                              │  (cada 10 min)
        ──────────────►│  Drive (tablon-resiliencia) │◄──────┘
                      │                              │
                      │◄── carta "respuesta" ────────┘
                      │   (+ idem_key + resultado)
```

* `megadaemon-core bus` ⇒ un ciclo a mano (`--simulacion` no ejecuta nada).
* Tool MCP `bus_ordenes` (ciclo | estado).
* **Idempotencia**: la `idem_key` (o, si no viene, el `id` de la carta) se
  guarda tras ejecutarse ⇒ una orden repetida **no** se re-ejecuta.
* **Fail-closed**: sin firma válida, o sin clave pública del emisor, la
  orden se cuenta como `sin verificar` y **no** se ejecuta.
* **Lo irreversible exige `confirm: true`** en el cuerpo, igual que la API.
* **Presupuesto**: máximo de órdenes por ciclo; con la batería baja y sin
  cargador no se ejecuta nada.

```sh
megadaemon-core bus                 # un ciclo
megadaemon-core bus --simulacion    # qué haría, sin hacerlo
megadaemon-core bus --maximo 3     # presupuesto
```

**MEDIDO**: `megadaemon-core bus` sincroniza el bus real de Drive
(`bus_ok: true`); sin credenciales da el motivo exacto
(*"no se puede leer la credencial …/gdrive_credentials.json"*).

---

## 10. Relación con el resto

| pieza | fichero | relación |
|---|---|---|
| cliente Drive nativo | `rust-core/src/drive_native.rs` | **reutilizado** (subida resumible) |
| VFS de Drive | `rust-core/src/drive_vfs.rs` | **reutilizado** (`write_local_file`) |
| OAuth de Google | `rust-core/src/google_auth.rs` | **reutilizado** (refresco solo) |
| diagnóstico | `rust-core/src/diagnostico.rs` | **reutilizado** (`recoger()`) |
| canal | `rust-core/src/channel.rs` | **reutilizado** (`read_channel_json`) |
| batería | `rust-core/src/battery_advanced.rs` | **reutilizado** (`modo_ahorro_activo`) |
| one-liner | `deploy/megadaemon-oneliner.sh` | **consume** `telemetria listo` / `publicar` / `conexion` |
| diseño P2P | `docs/RESILIENCIA_P2P.md` | este módulo es el **Nivel 0** (push) de esa arquitectura |

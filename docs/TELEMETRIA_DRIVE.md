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
* El配置 del periodo: `telemetria.publicar_min` (por defecto 15).
* La recolección lee `/proc` y `/sys`: ficheros locales, sin red ni CPU
  apreciable.

---

## 8. Verificación

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

## 9. Relación con el resto

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

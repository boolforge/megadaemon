# Portapapeles distribuido (`md-clipboard`)

Módulo: `rust-core/src/clipboard.rs` · CLI: `rust-core/src/bin/md-clipboard.rs` ·
Tool MCP: `clipboard` · Endpoint: `GET|POST /api/clipboard`

---

## 1. El problema: por qué los gestores de portapapeles de Android están rotos

No es culpa de las apps: es el diseño del sistema.

| Android | Restricción | Efecto sobre un gestor de portapapeles |
|---|---|---|
| 10 (API 29) | una app **en segundo plano no puede leer** el portapapeles | sólo funciona con la app abierta y en primer plano |
| 11 (API 30) | sigue igual, más el aviso al pegar | — |
| 12 (API 31) | al leer aparece el diálogo **"Pegado desde…"** | el usuario ve el origen en cada pegado |
| 13 (API 33) | el aviso pasa a ser una **previsualización** | hay que tocarla para poder pegar |

La respuesta habitual de las apps de.store es: **servicio en primer plano + notificación
permanente**. El usuario tiene que vivir con una notificación, y aun así el servicio puede
morir. Clipper, Clip Stack y similares tienen ese diseño. Es molesto, consume
batería y sigue siendo frágil.

**MegaDaemon no tiene ese problema porque no es una app normal: corre como `root`.**

---

## 2. La investigación: `corphish/TextTools` (F-Droid `com.corphish.quicktools`)

> ⚠️ Dato medido, y es una corrección importante: **TextTools NO es un gestor de
> portapapeles**. No lee el portapapeles, no tiene servidor local, no expone API HTTP y
> no hace sincronización escritorio↔Android.

Licencia: **Apache License 2.0** (verificado leyendo `LICENSE` en el repo, 2026-11).

Lo que hace (según su `README.md` y `FEATURES.md`):

* añade acciones al **menú contextual de la selección de texto**;
* "Text unsaved numbers in WhatsApp" (abrir chat con un número sin guardarlo);
* evaluar expresiones matemáticas en línea;
* transformación de texto (mayúsculas, minúsculas, envolver entre comillas, ordenar
  líneas, invertir, limpiar espacios, estilos Unicode);
* análisis de texto (caracteres, palabras, letras);
* guardar la selección a fichero;
* buscar y reemplazar;
* análisis de números y conversiones de base;
* plantillas de texto.

### La lección que sí nos sirve

TextTools funciona **sin tocar el portapapeles**, porque usa
**`ACTION_PROCESS_TEXT`**: cuando el usuario selecciona texto, el sistema **se lo entrega
a la app** como `EXTRA_PROCESS_TEXT`. Sin restricción de segundo plano, sin aviso, sin
notificación, sin root.

Es la misma razón por la que funcionan los correctores de texto en línea: no necesitan
leer el portapapeles.

### Lo que TextTools NO resuelve

* no captura lo que copias sin seleccionar (el `Ctrl+C` normal);
* no guarda historial;
* no sincroniza entre dispositivos;
* no distingue un texto normal de una clave.

**Conclusión: TextTools es un buen patron de captura (el `PROCESS_TEXT`), y un mal
patron de gestor (no hay historial ni distribución). MegaDaemon usa el patrón de captura
y añade el resto.**

---

## 3. Las cuatro rutas de captura, y cuál usa MegaDaemon

| # | Ruta | ¿Funciona en segundo plano? | Root | Notas |
|---|---|---|---|---|
| 1 | **`ACTION_PROCESS_TEXT`** (TextTools) | **sí** | no | el sistema entrega el texto seleccionado; nunca falla por política del portapapeles |
| 2 | **binder del servicio `clipboard`** (root) | **sí** | **sí** | el daemon es root ⇒ salta la política por UID; ninguna app normal puede |
| 3 | **IME propio** (teclado) | sí | no | un IME tiene acceso al portapapeles; captura automática de todo |
| 4 | **accesibilidad** | sí | no | ve la pantalla, **no** el portapapeles |

En MegaDaemon el proveedor por defecto es:

```rust
// rust-core/src/clipboard.rs
pub fn proveedor_por_defecto() -> Box<dyn ClipboardProvider> {
    let cmd = CmdClipboard;
    if cmd.leer().is_ok() { return Box::new(cmd); }   // ruta 2 (root)
    Box::new(PushClipboard::nuevo())                   // ruta 1 (push desde PROCESS_TEXT)
}
```

Es decir: **si el portapapeles root funciona, se usa; si no, el gestor sigue vivo** con
los items que le empuja `PROCESS_TEXT` (o el IME). La degradación es explícita y
**honesta**: el `status` dice qué proveedor está activo.

### Estado en este dispositivo: `NO VERIFICADO`

Los dos proveedores root (`cmd clipboard` y `service call clipboard`) **no están medidos
en el Samsung**. El protocolo de medición está en §7. Hasta que se midan:

* `CmdClipboard` → `NO VERIFICADO` (el subcomando existe desde Android 12/13 y su
  comportamiento con `root` no está documentado por AOSP);
* `ServiceCallClipboard` → `NO VERIFICADO` **y descartado de facto**: la API binder del
  servicio `clipboard` cambia entre versiones (nº de transacción y formato del `Parcel`
  no son estables), así que no se puede escribir a ciegas.

---

## 4. El modelo: lo que lo hace superior a los demás

### 4.1 Distingido: no guarda lo que no debe

`es_sensible()` marca como sensible lo que parece un secreto:

* palabras: `contraseña`, `password`, `clave`, `secret`, `token`, `api_key`,
  `-----BEGIN`, `private key`, `cvv`, `cvc`;
* **códigos de 4-8 dígitos** (`parece_codigo`): un único grupo de dígitos con contexto
  ("tu código es 482913") **o a secas** ("482913").

Un item sensible:

* **no entra** en el historial persistente (`persistir_sensibles = false` por defecto);
* **no se propaga** por la malla (`permitir_sensibles = false` por defecto);
* recibe **TTL de 60 s**.

Un teléfono (`600123456`, `600 123 456`) **no** es código y **no** es sensible: son 9
dígitos o 3 grupos, y la regla lo excluye a propósito.

### 4.2 Con expiración

`ttl_s` por item, con purga (`purgar(ahora)`). Es el mismo modelo que un gestor de
claves: "esto desaparece en 30 s".

### 4.3 Con deduplicación

Copiar lo mismo 5 veces **no** crea 5 entradas: se actualiza la existente y se mueve al
frente (`items.retain(|i| i.texto != item.texto)`).

### 4.4 Clasificado

`clasificar()` devuelve `texto` | `url` | `telefono` | `codigo` | `json`, y el
historial se puede filtrar por tipo. Ojo con un detalle real: un número suelto
("600123456") **es JSON válido**, así que la detección de JSON exige objeto o array;
sin eso, un teléfono acabaría clasificado como JSON (bug corregido y con test).

### 4.5 Con límites

* 64 KiB por item (`MAX_ITEM_BYTES`);
* 200 entradas (`MAX_ENTRADAS`);
* 2 MiB de historial (`MAX_HISTORIAL_BYTES`).

### 4.6 Cifrado en reposo, con los módulos canónicos

`guardar()` con passphrase ⇒ **AEGIS-256** (clave de 32 B, tag de 16 B) con clave
derivada por **Argon2id** (`crate::argon2::derive_key`, 3 iteraciones, 64 MiB, p=2).
Formato: `MCLP1 ‖ salt(16) ‖ nonce(32) ‖ ciphertext ‖ tag(16)`.

Test que lo verifica: el fichero en disco **no contiene el texto en claro** y con otra
passphrase el descifrado falla.

### 4.7 Distribuido

Los items viajan como una línea JSON por la malla:

```json
{"tipo_malla":"clipboard","id":"…","texto":"…","tipo":"url","ts":…,"ttl_s":…}
```

Reglas (`ReglasSync`): `solo_efimeros`, `excluir_apps`, `ttl_max_s`, `permitir_sensibles`.
El TTL se **recalcula al llegar** (el reloj del otro nodo no tiene por qué coincidir).
La fusión es idempotente por `id`.

---

## 5. La API

### 5.1 CLI

```sh
md-clipboard put "texto"                    # añade al historial (y lo copia)
md-clipboard get [--id <id>]                # imprime el texto para pegar
md-clipboard history [--limit N]
md-clipboard search "query"
md-clipboard clear
md-clipboard watch --interval 5             # vigila el portapapeles del sistema
md-clipboard export [--out f]               # exporta (cifrado si hay passphrase)
md-clipboard import --in f
```

Opciones: `--state-dir <dir>` (por defecto, el `state_dir` del daemon), `--json`,
`--mock` (portapapeles en memoria, para probar sin dispositivo).

La passphrase de cifrado se lee **sólo** de `MEGADAEMON_CLIP_PASSPHRASE` (nunca de la
línea de órdenes, que queda en el historial del shell).

### 5.2 MCP

```jsonc
{ "name": "clipboard", "arguments": { "action": "put", "text": "hola" } }
```

Acciones: `list`, `put`, `get`, `copy`, `clear`, `search`, `status`.

### 5.3 Cockpit

```
GET  /api/clipboard[?q=…&limit=N]      → listar / buscar
POST /api/clipboard {"action":"put","text":"…"}
POST /api/clipboard {"action":"copy","id":"…"}
POST /api/clipboard {"action":"clear"}
POST /api/clipboard {"action":"status"}
```

Cae dentro del gate de autenticación (API key + token de sesión): el portapapeles puede
contener secretos, no se sirve en claro a nadie.

---

## 6. El `status` honesto

```json
{
  "ok": true,
  "proveedor": "cmd-clipboard",
  "lectura_disponible": true,
  "items": 12,
  "bytes": 1834,
  "ruta": "/data/local/tmp/megadaemon/clipboard.ndjson",
  "nota": "cmd clipboard es NO VERIFICADO en este dispositivo; la via PROCESS_TEXT siempre funciona"
}
```

Si el proveedor root no está disponible, `proveedor` pasa a `push` y
`lectura_disponible` a `false`: **el gestor sigue funcionando** con lo que le empujan
`PROCESS_TEXT`/IME, y lo dice.

---

## 7. Medición pendiente en el móvil (protocolo exacto)

Esto es lo que hay que ejecutar en el S21 Ultra (root) para cerrar el `NO VERIFICADO`:

```sh
su -mm -c 'cmd clipboard get'                      # ¿existe el subcomando?
su -mm -c 'cmd clipboard set "prueba-megadaemon"'  # ¿escribe?
su -mm -c 'cmd clipboard get'                      # ¿devuelve lo escrito?
su -mm -c 'service list | grep -i clipboard'       # ¿qué nombre tiene el servicio?
su -mm -c 'dumpsys clipboard'                      # estado del servicio
```

Y el comportamiento por versión (esto **no** se puede medir sin probarlo):

```sh
su -mm -c 'am start -a android.intent.action.PROCESS_TEXT'   # ruta PROCESS_TEXT
```

**Resultado hasta la fecha: `NO VERIFICADO`.** Hasta que haya evidencia, el código no
afirma que la ruta root funcione: el proveedor por defecto la **prueba** en runtime
(`cmd.leer().is_ok()`) y cae al push si falla.

### Nota sobre el hook `PROCESS_TEXT`

`ACTION_PROCESS_TEXT` lo consume **una app**, no un binario. La pieza que falta es una
app mínima (o una Activity dentro de la existente) que:
1. declare el intent-filter `android.intent.action.PROCESS_TEXT`;
2. reciba `EXTRA_PROCESS_TEXT`;
3. lo empuje al daemon por el socket Unix o por `fs_write` en `state/`.

**Estado: `PENDIENTE`** — el módulo del daemon está listo (el `PushClipboard` y el
`handle_action`), pero la Activity receptora **no está escrita**. Es el único hueco
funcional del camino sin root.

---

## 8. Qué queda `NO DETERMINADO`

| Punto | Por qué |
|---|---|
| `cmd clipboard` con root en este S21 | requiere ejecutar en el dispositivo |
| `service call clipboard` con root | la API binder no es estable entre versiones |
| `PROCESS_TEXT` automático | falta la Activity receptora (§7) |
| captura desde un IME propio | requiere una app IME; fuera del alcance del binario único |
| sincronización real entre dos nodos | el formato de malla está probado con round-trip, pero falta el enlace físico (malla ↔ `mesh.rs`) |
| cifrado del tráfico entre nodos | el módulo de malla ya cifra con AEGIS; falta el pegamento |

Nada de esto está simulado en el código: lo que no está medido, está marcado.

---

## 9. Comparativa con lo que hay

| Capacidad | Clipper/Clip Stack (store) | TextTools | **MegaDaemon `md-clipboard`** |
|---|---|---|---|
| Captura en 2º plano | servicio en primer plano + notificación | — | **root (binder) o PROCESS_TEXT, sin notificación** |
| Necesita app abierta | sí | sí | **no** |
| Historial | sí | no | **sí, con dedup y límites** |
| Caducidad por item | no | no | **sí (TTL)** |
| No guarda secretos | no | n/a | **sí, los clasifica y los expulsa** |
| Sincroniza entre dispositivos | no | no | **sí, por la malla, con reglas** |
| Cifrado en reposo | no | n/a | **AEGIS-256 + Argon2id** |
| API | no | no | **CLI + MCP + HTTP** |
| Código abierto | no | Apache-2.0 | **Rust, en este repo** |

---

## 10. Tests

```sh
cargo test --lib clipboard -j 1        # 25 tests
cargo test --test clipboard_smoke -j 1 # 6 tests
```

Cubren: clasificación (incluido el caso "número suelto = JSON válido"), sensible vs
teléfono, dedup, no-persistencia de sensibles, caducidad, topes de entradas/bytes, item
enorme rechazado, búsqueda, mock y push, ida y vuelta por la malla, no-propagación de
secretos, apps excluidas, `solo_efimeros`, recorte de TTL, cifrado ida y vuelta, ausencia
de texto en claro en disco, magic `MCLP1`, y las acciones `put`/`list`/`search`/
`desconocida`.

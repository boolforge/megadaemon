# `env_introspect` — la tool para **preguntar al móvil en vez de adivinar**

> «`NPU.bin` no existe» y «`hw_keymaster` no existe» no eran errores: eran **rutas del
> volcado de ROM que no corresponden al firmware en vivo**. Un agente que *supone*
> sigue extrayendo de la imagen equivocada; uno que *pregunta al dispositivo* lo detecta
> en un segundo. Esta tool es la respuesta mecánica a eso.

## Qué es

Un módulo **de sólo lectura** (`rust-core/src/env_introspect.rs`) que responde preguntas
del entorno **leyéndolas del dispositivo**, no deduciéndolas. Se expone como tool MCP
`env_introspect` (y por el segundo canal `POST /api/tool/env_introspect`).

## Contrato de la respuesta (obligatorio)

Toda respuesta lleva **la fuente** y **el momento**, para que nadie confunda un dato viejo
con uno actual, y **nunca inventa**:

```json
{
  "ok": true,
  "action": "binaries",
  "question": "¿existe NPU.bin en este firmware?",
  "available": false,
  "reason": "ningún binario casa con 'NPU.bin' en el dispositivo",
  "source": "/vendor/bin,/system/bin,/vendor/lib64,/system/lib64,/vendor/lib64/hw,/odm/bin,/product/bin",
  "device": "SM-G998B / exynos2100 / exynos2100",
  "ts": "2026-11-20T00:52:14Z",
  "answer": null
}
```

- `available:false` + `reason` ⇒ **algo no existe** (ruta, binario, app, propiedad).
- `source` ⇒ de dónde sale el dato (ruta o comando).
- `ts` ⇒ **reloj del dispositivo** (`date -u`), con respaldo en el reloj del host.
- `device` ⇒ modelo + SoC, para no confundir dos dispositivos.

## Acciones

| action | responde | notas |
|---|---|---|
| `partitions` | qué particiones hay de verdad en `/dev/block/by-name` | con `sha:true` añade el sha256 (streaming en el móvil) |
| `dev` | nodos de `/dev` (filtro) | modo, owner, tipo (block/char/symlink/dir) |
| `binaries` | **qué binarios existen** con ese nombre y en qué rutas | `sha:true` añade sha256 — **es la que evita el error de `NPU.bin`** |
| `app` | de un paquete: ¿instalada?, apk, versión, uid, ABI | `sha:true` añade el sha del APK |
| `ram` | RAM/carga/uptime | desde `/proc/meminfo` |
| `path` | inspección de una ruta: tipo, modo, tamaño, sha256, destino del symlink | |
| `prop` | `getprop` filtrado | |
| `processes` | procesos que casen con un filtro | pid, user, rss |
| `help` | lista de acciones y ejemplos | |

Ejemplos:

```json
{"action":"binaries","filter":"NPU.bin"}
{"action":"binaries","filter":"keymaster","sha":true}
{"action":"partitions","filter":"keystorage"}
{"action":"dev","filter":"vertex"}
{"action":"app","package":"com.android.chrome"}
{"action":"path","path":"/vendor/bin/NPU.bin","sha":true}
{"action":"prop","filter":"ro.boot."}
```

## Cableado (pendiente, 3 líneas)

1. `rust-core/src/lib.rs` → `pub mod env_introspect;` (**ya en este commit**)
2. `rust-core/src/mcp_ext.rs` → entrada en `tool_definitions()` + brazo en `dispatch()`
   (`"env_introspect" => crate::env_introspect::ask(&s("question"), args)`)
3. (opcional) `rust-core/src/mcp_server.rs` → exposure en el `tools/list` del MCP

El punto 2 está en un fichero que otro agente está editando ahora mismo ⇒ **no lo he
tocado**: lo cablea quien lo tenga (principal o agente de servidor), en su commit.

## Tests

5 tests unitarios (`cargo test --lib env_introspect` → **5 passed / 0 failed**), todos sobre
el parsing puro (no necesitan dispositivo): unión de nombre/real/tamaño de particiones,
parseo de binarios preservando el directorio, contrato `source`+`ts` en `help`, y los dos
casos de "no existe" ⇒ **motivo explícito y `answer:null`**.

## Por qué importa en la fase RE

Cada vez que un agente **supuso** una ruta, la fase RE produjo un hallazgo falsado. Con
esta tool, "no existe" es una **respuesta de primera clase**, con su motivo y su fuente.

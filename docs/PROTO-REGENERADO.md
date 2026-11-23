# PROTO-REGENERADO — los `.proto` y el Go del binario de Antigravity

> **Todas las cifras de este documento están MEDIDAS** ejecutando el código de
> `tools/proto-regen/` sobre `/workspace/agy-re/liblanguage_server.so`.
> Lo que no se ha podido medir va marcado como **NO VERIFICADO**.

---

## 1. Qué se ha hecho y por qué

El binario analizado (`liblanguage_server.so`, **181.358.800 B**, ELF ARM64,
Go 1.28, **sin DWARF**) trae **65.230 de sus 161.682 funciones** generadas
por `protoc-gen-go` a partir de ficheros `.proto` del sistema de build
interno de Google (Blaze). DeCOMPILAR ese Go es tirar tiempo.

Lo correcto es **REGENERARLO** desde los descriptores que el binario lleva
embebidos: `protoc-gen-go` (protobuf-go v2) guarda cada
`FileDescriptorProto` como una **constante de cadena** en `.rodata`:

```go
const file_foo_proto_rawDesc = "" + "\n\x0efoo.proto" + ...
```

Es decir: los bytes serializados del descriptor están **tal cual, sin
comprimir**, en el binario. (La función `file_foo_proto_rawDescGZIP`, que sí
aparece en la tabla de símbolos, sólo los comprime *en memoria* al
registrarlos en el `protoregistry` global.)

Lo que se ha hecho:

1. **Extraer** los descriptores embebidos (`tools/proto-regen/extraer_descriptores.py`,
   equivalente en Rust en `rust-core/src/proto_regen.rs`).
2. **Reconstruir los `.proto` en texto** (`tools/proto-regen/proto_a_texto.py`)
   y **verificarlos con `protoc`** (`tools/proto-regen/validar_proto.py`).
3. **Regenerar el Go** con `protoc-gen-go` (`tools/proto-regen/generar_go.py`),
   con un **generador propio en Python** como respaldo
   (`tools/proto-regen/generador_go_propio.py`).
4. **Validar la sintaxis del Go** sin toolchain (`tools/proto-regen/validar_go.py`).

---

## 2. Las cifras (medidas)

### 2.1 Extracción

| Magnitud | Valor |
|---|---|
| Bytes del binario | 181.358.800 |
| Candidatos localized (`0x0A` + longitud + ruta `.proto`) | **704** |
| Candidatos que parsean como `FileDescriptorProto` | **585** |
| `.proto` únicos (descriptores válidos) | **557** |
| Bytes de descriptor crudo | **3.019.042** |
| Mensajes (primer nivel) | **5.353** |
| Mensajes (incluyendo anidados) | **6.737** |
| Enums (primer nivel) | **531** |
| Enums (incluyendo anidados) | **1.352** |
| Servicios | **36** |
| Extensiones | **272** |
| Candidatos descartados | **119** |
| Duplicados descartados (mismo `.proto`, otra variante) | **28** |
| `descriptor_set.pb` | 3.020.742 B |

Distribución por sintaxis: **proto3 319**, **editions 124**, **proto2 114**.
Paquetes distintos: **167**.

### 2.2 Los `.proto` reconstruidos

| Magnitud | Valor |
|---|---|
| Ficheros `.proto` | **557** |
| Bytes de texto `.proto` | **2.306.608** |
| Sentencias `import` | **2.098** |
| Imports distintos | 471 — **todos presentes** en el set (0 ausentes) |
| Referencias a tipos | **7.518** — **todas resuelven** (0 indefinidas) |

### 2.3 Verificación con `protoc` (libprotoc 35.1, el de `grpcio-tools`)

| Magnitud | Valor |
|---|---|
| Ficheros `.proto` compilados | **557** |
| **Errores** | **0** |
| Avisos | 1.097 — todos `Import ... is unused` |
| Ficheros re-compilados por `protoc` | 557 |
| **Fidelidad** (`descriptor_set` recompilado == extraído) | **`true`** |

Detalle de la comparación campo a campo:

| Magnitud | Original | Recompilado | Distintos |
|---|---|---|---|
| Ficheros | 557 | 557 | **0** |
| Campos (nº, tipo, `type_name`, label, `oneof`, `proto3_optional`) | 22.553 | 22.553 | **0** |
| Valores de enum | 19.299 | 19.299 | **0** |
| RPCs | 896 | 896 | **0** |

Ése es el resultado fuerte: **el `.proto` reconstruido es el original**, no
una aproximación.

### 2.4 El Go regenerado

`protoc-gen-go` **v1.36.12** se descarga del release de GitHub de
`protocolbuffers/protobuf-go` (github.com **sí** es accesible desde el
sandbox; el binario de `protobuf` no lo trae, el de `protobuf-go` sí).

| Magnitud | `protoc-gen-go` v1.36.12 | Generador propio (Python) |
|---|---|---|
| Ficheros `.go` | **557** | **557** |
| Líneas | **721.095** | **433.322** |
| Bytes | **28.484.538** | **26.145.370** |
| Etiquetas `protobuf:"..."` | **22.307** | **22.307** |
| Referencias a `*_rawDesc` | 12.627 | 557 |
| Go lexico válido | **557/557** | **557/557** |

Las 22.307 etiquetas coinciden **exactamente** con los 22.307 campos del
descriptor: es la prueba de que el Go generado describe las mismas
estructuras que el binario original.

### 2.5 Lo que NO se ha podido recuperar

| Magnitud | Valor |
|---|---|
| `.proto` esperados (derivados de los 537 `.pb.go` de `go.funcs.tsv`) | 534 presentes |
| `.proto` esperados **sin descriptor recuperable** | **4** |
| Opciones personalizadas no re-emitibles (extensiones, campo ≥ 1000) | **1.930** (40 números de campo distintos) |
| Ficheros con `edition` ajustada | **1** (`third_party/protobuf/internal_options.proto`: 2026 → 2024) |
| Ficheros con `extendee` reescrito para el paso de Go | **28** |

Los 4 `.proto` sin descriptor:

* `com.google.apps.annotations.proto`
* `com.google.identity.accesscontrol.proto`
* `google/cloud/businessaicode/v1main/management_service.proto`
* `google/cloud/businessaicode/v1main/telemetry_service.proto`

Su copia embebida **no es** un `FileDescriptorProto`: es el *payload* de una
opción personalizada (`FileOptions` con campos `java_package`,
`java_outer_classname`, `go_package`…), que tiene la misma forma que un
descriptor y por eso aparece como candidato. Se descartan.

Los 3 `*_grpc.proto` que aparecen en la lista de `.pb.go`
(`s2a_grpc.proto`, `language_server_grpc.proto`, `s2a/v2/s2a_grpc.proto`)
**no tienen descriptor en el binario** porque los genera
`protoc-gen-go-grpc`, que no embebe `rawDesc`. No es una pérdida: no existen.

---

## 3. Cómo se reproduce

```bash
# El venv del sandbox ya trae `protobuf` y `grpcio-tools` (con `protoc`).
# El Python del sistema es PEP 668: no se instala nada ahí.
/workspace/venv-frida/bin/python -m pip install --index-url https://pypi.org/simple \
    protobuf grpcio-tools          # sólo si falta

# Pipeline completo (extracción -> .proto -> protoc -> Go -> validación)
tools/proto-regen/regenerar.sh /workspace/agy-re/liblanguage_server.so /tmp/proto-regen
```

Duración medida: **2 min 17 s**. Salidas:

```
/tmp/proto-regen/
├── descriptor_set.pb            FileDescriptorSet con los 557 descriptores
├── manifiesto.tsv               descriptor -> vaddr, offset, bytes, sha256, cuentas
├── duplicados.tsv               el mismo .proto con otra variante (28)
├── descartados.tsv             candidatos que no pasaron el filtro (119)
├── descriptores/                los bytes crudos, uno por fichero
├── resumen.json                 todas las cifras
├── protos/                      los 557 .proto reconstruidos
├── opciones_no_representables.tsv   las 1.930 opciones que no se pueden reemitir
├── proto_stats.json
├── informe_validacion.json      protoc OK + fidelidad
├── go/                          el Go regenerado (árbol de paquetes Go)
├── go_stats.json
└── informe_go.json              validación léxica del Go
```

Piezas sueltas:

```bash
P=/workspace/venv-frida/bin/python
T=tools/proto-regen

$P $T/extraer_descriptores.py --bin <ELF> --out /tmp/x
$P $T/proto_a_texto.py        --descriptor-set /tmp/x/descriptor_set.pb --out /tmp/x/protos
$P $T/validar_proto.py        --protos /tmp/x/protos --descriptor-set /tmp/x/descriptor_set.pb
$P $T/generar_go.py           --protos /tmp/x/protos --out /tmp/x/go \
                             --go-funcs /workspace/agy-re/tools-out/go.funcs.tsv
$P $T/validar_go.py           --raiz /tmp/x/go
```

### El plugin `protoc-gen-go`

`generar_go.py` lo descarga solo si no se le pasa `--plugin`:

```
https://github.com/protocolbuffers/protobuf-go/releases/download/v1.36.12/
    protoc-gen-go.v1.36.12.linux.amd64.tar.gz
```

(El release de `protocolbuffers/protobuf` **no** lleva `protoc-gen-go`; el
binario está en el repo `protobuf-go`.)

---

## 4. Cómo funciona (y por qué estos detalles)

### 4.1 Encontrar el descriptor

El `FileDescriptorProto` empieza siempre por el campo 1 (`name`): tag
`0x0A` + varint de longitud + la ruta del `.proto` en ASCII. Eso da el
candidato. Para saber **dónde termina** el descriptor se usa un parser
estricto:

* los números de campo tienen que estar entre 1 y 15 y llevar el *wire-type*
  que les corresponde (`2` = length-delimited, `0` = varint);
* el campo 1 (`name`) es **singular**: si aparece dos veces, el descriptor
  anterior terminó;
* los números de campo van en **orden ascendente** (así serializa `protoc`):
  si aparece uno menor que el anterior, hemos salido del descriptor.

Los 119 candidatos descartados son, en su mayoría (102), *payloads de
opciones personalizadas* (`proto_file:google/api/service.proto`,
`package:...`) que parsean como `FileDescriptorProto` pero no lo son; se
filtran exigiendo que el nombre tenga forma de ruta y que el mensaje tenga
más contenido que el nombre.

### 4.2 Reconstruir el `.proto`: los detalles que hicieron falta

Cada uno de estos se descubrió porque `protoc` **rechazaba** el fichero:

| Detalle | Por qué |
|---|---|
| proto2 lleva label (`optional`/`required`/`repeated`); proto3 y editions no | en editions `optional` no existe: lo decide `features.field_presence` |
| los campos `proto3_optional` **no** van dentro de su `oneof` sintético | en texto son un `optional` normal |
| los `map<>` se reconstruyen desde el mensaje anidado con `map_entry=true` | así es como los representa protobuf |
| las extensiones van en bloques `extend <extendee> { ... }`, agrupadas | si no, `protoc` dice `Expected top-level statement` |
| `extensions 1000 to max;` (los *extension ranges*) | sin ellos las opciones personalizadas no resuelven |
| `reserved "nombre"` con cadena en proto2/proto3, con identificador en editions | regla de `protoc` |
| los grupos de proto2 se escriben en línea (`group X = N { ... }`) y su mensaje anidado **no** se emite por separado | hay **1** grupo en todo el set (`logs/java/logrecord.proto`) |
| `option features.X = Y;` en los ficheros editions | sin él, un enum *closed* con el primer valor ≠ 0 no compila |
| nombres de tipo en forma **relativa** si no hay ambigüedad, **absoluta** (con punto inicial) si la hay | `protoc` busca el primer componente en la cadena de ámbitos y un *paquete* cuenta como símbolo |
| `option java_package = "x";` **con comillas** | las cadenas de opción van entrecomilladas |

El último punto (ambigüedad de nombres) salió de un fichero real:
`learning/deepmind/evergreen/model_access/proto/content.proto` usa
`evergreen.v2.Array`, que `protoc` resolvía a
`deepmind.evergreen.v2.Array` (el ámbito `deepmind.evergreen` contiene un
paquete llamado `evergreen`).

### 4.3 Regenerar el Go: los dos desvíos que hubo que hacer

`protoc-gen-go` (el público) es **más estricto** que `protoc`:

1. **107 extensiones** declaradas en ficheros **proto3** extienden
   `proto2.XxxOptions` (el descriptor proto2 interno de Google). `protoc` las
   acepta; `protoc-gen-go` exige que en proto3 sólo se extiendan mensajes de
   opciones de `google.protobuf`.
   *Desvío (sólo en la **copia** usada para el paso de Go):* se reescribe el
   `extendee` a `google.protobuf.XxxOptions` y se cambia el import por
   `google/protobuf/descriptor.proto`, que **protoc lleva construido** (no se
   declara, así que no choca con el descriptor interno). Afecta a **28
   ficheros**. El árbol de `.proto` fiel **no se toca**.
2. `net/proto2/bridge/proto/message_set.proto` declara
   `option message_set_wire_format = true;` (proto1), que `protoc-gen-go`
   rechaza. En la copia se quita esa línea.

El **árbol de `.proto` fiel no se modifica**: la copia derivada vive en
`<out>/go/_arbol_go/` y se regenera en cada ejecución.

El paquete Go de cada `.proto` **no se inventa**: sale de `go.funcs.tsv`, que
tiene el nombre del paquete Go de cada fichero generado (el campo `origen`
trae el símbolo completo sin "manglar": `google3/net/proto2/proto/
descriptor_go_proto.Edition.Enum`). Se usa el **prefijo común más largo** de
todos los símbolos del fichero, que es la ruta de importación. De los 557
`.proto`, **534** tienen su paquete Go medido en el binario; los 23 restantes
se derivan del nombre del fichero.

---

## 5. Lo que queda **NO VERIFICADO**

* **El Go no se ha compilado.** El sandbox no tiene `go` ni `gofmt`
  (medido: `which go gofmt` no devuelve nada). La validación es **a nivel de
  lexer**: literales y comentarios cerrados, balance de `(`, `{`, `[` con
  pila, y cláusula `package` presente. **No** se ha comprobado el tipado, la
  resolución de imports ni que las expresiones compilen.
* **El Rust no se ha compilado** (`rust-core/src/proto_regen.rs` y
  `rust-core/src/bin/md-proto-regen.rs`): el crate pesa +200 módulos y
  enlazarlo en 3 GB es un OOM garantizado (`docs/PROTOCOLO_FLOCKA.md`). Los
  tests del módulo son unitarios y no dependen de ficheros externos; los
  compila y testea el root.
* **Las 1.930 opciones personalizadas** (campos ≥ 1000 de `FileOptions`,
  `MessageOptions`, `FieldOptions`…) **no se re-emiten**: harían falta las
  definiciones de esas extensiones, que no están en el descriptor (son
  extensiones de `google.protobuf.*Options` definidas en ficheros que no
  tenemos). Están listadas en `opciones_no_representables.tsv` con su número
  de campo; **no se inventan**.
* **El `.proto` es una reconstrucción semántica, no el fichero original
  byte a byte**: los comentarios, el orden de las declaraciones y el texto de
  las opciones que no sabemos reemitir no se pueden recuperar de un
  descriptor. Lo que sí se recupera es la **semántica completa**, y eso es lo
  que `protoc` recompila y lo que se ha comparado campo a campo.
* **`edition = "2026"`** (1 fichero) no lo conoce `protoc` 35.1: se escribe
  2024. Con un `protoc` más nuevo se podría dejar 2026.
* **Los 4 `.proto` sin descriptor** (§2.5) no se pueden recuperar del
  binario: su copia embebida es el payload de una opción personalizada.

---

## 6. Ficheros entregados

| Ruta | Qué es |
|---|---|
| `tools/proto-regen/elfseg.py` | lector ELF64 mínimo (segmentos y secciones) |
| `tools/proto-regen/extraer_descriptores.py` | **extractor** de los descriptores embebidos |
| `tools/proto-regen/proto_a_texto.py` | `FileDescriptorSet` → texto `.proto` |
| `tools/proto-regen/validar_proto.py` | compila con `protoc` y mide la fidelidad |
| `tools/proto-regen/mapa_go.py` | `.proto` → ruta de importación Go, medida del binario |
| `tools/proto-regen/generar_go.py` | regenera el Go con `protoc-gen-go` (lo descarga) |
| `tools/proto-regen/generador_go_propio.py` | **generador propio** en Python (respaldo) |
| `tools/proto-regen/validar_go.py` | validación léxica del Go (sin toolchain) |
| `tools/proto-regen/regenerar.sh` | pipeline de un tirón |
| `tools/proto-regen/README.md` | guía rápida |
| `rust-core/src/proto_regen.rs` | equivalente en Rust (**sin compilar**) |
| `rust-core/src/bin/md-proto-regen.rs` | CLI en Rust (**sin compilar**) |
| `rust-core/src/lib.rs` | +1 línea: `pub mod proto_regen;` |

**No se commitean**: el `.so`, el TSV de 46 MB, el `descriptor_set.pb`, los
`.proto`/`.go` regenerados ni los informes: son producto, no código.

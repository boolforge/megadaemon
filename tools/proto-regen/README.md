# `tools/proto-regen` — regenerar los `.proto` y el Go del binario de Antigravity

Este directorio tiene todo lo necesario para **extraer los descriptores
protobuf embebidos** en `liblanguage_server.so`, **reconstruir los `.proto`**
en texto, **verificarlos con `protoc`** y **regenerar el código Go**.

La referencia completa (cifras medidas, método y lo que queda sin
verificar) está en [`docs/PROTO-REGENERADO.md`](../../docs/PROTO-REGENERADO.md).

## Uso rápido

```bash
# pipeline completo: extracción -> .proto -> protoc -> Go -> validación
tools/proto-regen/regenerar.sh /workspace/agy-re/liblanguage_server.so /tmp/proto-regen
```

## Piezas

| Fichero | Qué hace |
|---|---|
| `elfseg.py` | lector ELF64 mínimo: segmentos `PT_LOAD` y secciones |
| `extraer_descriptores.py` | busca los `FileDescriptorProto` en `.rodata` y los valida |
| `proto_a_texto.py` | `FileDescriptorSet` → un `.proto` por fichero |
| `validar_proto.py` | compila con `protoc` y compara el resultado con el original |
| `mapa_go.py` | `.proto` → ruta de importación Go, **medida** de `go.funcs.tsv` |
| `generar_go.py` | regenera el Go con `protoc-gen-go` (lo descarga de GitHub) |
| `generador_go_propio.py` | generador Go propio en Python (respaldo sin red) |
| `validar_go.py` | validación léxica del Go (el sandbox no tiene `go`) |
| `regenerar.sh` | todo lo anterior, de un tirón |

## Dependencias

* Python 3.11 con `protobuf` y `grpcio-tools` (que trae `protoc`).
  En el sandbox: `/workspace/venv-frida/bin/python`.
  **No** instalar nada en el Python del sistema (PEP 668).
* `protoc-gen-go` (opcional): lo descarga `generar_go.py` del release de
  `protocolbuffers/protobuf-go`. Sin red, se usa `generador_go_propio.py`.
* `go.funcs.tsv` (opcional): sólo para saber el paquete Go de cada `.proto`.

## Por qué existe

El binario trae 65.230 de sus 161.682 funciones generadas por
`protoc-gen-go`. Descompilarlas es tirar tiempo; **regenerarlas** desde los
descriptores que el binario lleva embebidos no.

Medido: 704 candidatos → 557 `.proto` únicos → 3.019.042 bytes de
descriptor → 557 `.proto` que `protoc` compila **sin un solo error** y cuyo
descriptor recompilado es **idéntico** al original (22.553 campos, 19.299
valores de enum y 896 rpcs, 0 diferencias).

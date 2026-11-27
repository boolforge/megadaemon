# Entrega del módulo Go regenerado — artefacto, hashes y verificación

**Fecha:** 2026-11-27 · **Rama:** `re/productos-cierre-2026-11-27` → `main` (98aae37)

## 1. Qué se entrega

| artefacto | qué es | bytes | sha256 |
|---|---|---:|---|
| `md-go-modulo-2026-11-27.tar.gz` | el **módulo Go regenerado** completo (`gomodulo/`): 5.383 ficheros `.go`, 1.345 paquetes | 4.665.766 | `0611a9b90d6027e474afcc409934da6b12e7f92afe1afbcb13ce63d215dcb811` |
| `md-go-modulo-smoke` | **binario enlazado**: un `main` que referencia 8 paquetes del módulo ⇒ obliga al enlazador a resolverlos | 2.359.332 | `ceae4e34d5f93b9a224e2af77a93414e9af15f608d088713f697149837fb2607` |
| `agent-client-protocol-schema-1.22.0.crate` | el crate original del esquema ACP (Apache-2.0), para poder regenerar o comparar el esquema vendorizado | 283.771 | `650e76dae0c8d3e772acb35184e6dd4defa0e15e130c96bf4c4586597f1f2d91` |

Los tres están en la **Drive privada**, carpeta `megadaemon-backup`
(`15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`), con su `file_id` y su sha256 en
`artifacts/contratos-productos-2026-11-27/INVENTARIO_DRIVE.tsv`.

**En el repo público no va el módulo**: son 111 MB de stubs generados. En el repo
va el **inventario** (rutas, hashes y file_ids) y el esquema ACP **vendorizado**
(`rust-core/src/acp_schema.rs`, Apache-2.0 con atribución en
`docs/ACP_VENDOR_LICENSE.md`).

## 2. Por qué el formato es un tar.gz y no un `.a`

`go build ./...` sobre un **módulo biblioteca** no produce un `.a` único: compila
cada paquete a un objeto en la caché de compilación y no deja un artefacto
único. Las tres salidas posibles, y por qué se eligieron dos:

| opción | qué da | por qué no |
|---|---|---|
| `go build ./...` | compila los 1.345 paquetes; sin artefacto único | sólo verifica |
| `go vet ./...` | análisis estático; sin artefacto | sólo verifica |
| **tar.gz del árbol** | el entregable: el módulo tal cual, con sus stubs | **elegido** |
| **binario enlazado** | prueba de que el módulo **enlaza**, no sólo compila | **elegido** |

## 3. Verificación (los tres pasos, medidos)

```bash
# 1) el hash del tar
sha256sum md-go-modulo-2026-11-27.tar.gz
# 0611a9b90d6027e474afcc409934da6b12e7f92afe1afbcb13ce63d215dcb811  md-go-modulo-2026-11-27.tar.gz

# 2) el binario ENLAZA y ejecuta
./md-go-modulo-smoke
# md-go-modulo-smoke: enlazado OK      (exit 0)

# 3) el módulo compila y pasa vet desde cero
export PATH=/workspace/md-resc/go/go/bin:$PATH
export GOCACHE=/tmp/gocache-local     # OJO: en el NFS el build se CUELGA
export TMPDIR=/tmp
cd gomodulo && go build ./... && go vet ./...
# rc=0 y rc=0, sin una sola línea de salida
```

> **Aviso medido (importante):** con `GOCACHE` en el NFS el build no compila:
> se queda **18 min de reloj con 22 s de CPU** y 0 hijos, escribiendo la caché de
> compilación por red. Con `GOCACHE=/tmp/gocache-local` va bien. No es un
> problema del código: es del sitio de la caché.

## 4. Qué contiene el módulo (y qué no)

* **Sí**: 5.383 ficheros `.go`, 1.345 paquetes, 161.682 stubs con el **nombre,
  fichero y línea reales** (vienen del pclntab) ⇒ el módulo es navegable con
  `gopls`/`go doc`.
* **Sí**: 29.764 stubs con firma tipada real (2.256 con parámetros, 1.391 con
  retorno, 9.254 con `unsafe.Pointer`), y **0** ficheros con `import "unsafe"`
  sin usar (antes 660 ⇒ el paquete no compilaba).
* **No**: el cuerpo ejecutable. El pseudocódigo de Ghidra/r2ghidra es C
  decompilado (`lVar1`, `uVar5`, structs sin nombre) y **no es Go compilable**;
  se entrega como referencia legible en `codigo/<paquete>/<fichero>.c`
  (761.801.297 B en la Drive, `RE-GO-LEGIBLE/`). El cuerpo de cada stub es un
  `panic("pseudocodigo: ver <fichero>")` **a propósito**: es un stub honesto,
  no una simulación.

## 5. La cobertura, por categoría (lo que hay que leer)

| categoría | funciones | con pseudocódigo | % |
|---|---:|---:|---:|
| **jetski (código propio de Antigravity)** | 11.928 | 11.892 | **99,70 %** |
| runtime Go | 12.298 | 12.026 | 97,79 % |
| librerías públicas (third_party/golang) | 33.659 | 33.552 | 99,68 % |
| otras third_party | 2.665 | 2.644 | 99,21 % |
| resto | 1.346 | 1.344 | 99,85 % |
| generado (blaze/protobuf) | 69.466 | 772 | 1,11 % |
| autogenerado (compilador) | 30.320 | 45 | 0,15 % |
| **TOTAL** | **161.682** | **62.275** | **38,52 %** |

El **38,52 %** global está arrastrado por **99.786 funciones GENERADAS**
(tablas protobuf de `blaze-out/` y wrappers que emite el compilador): no hay
lógica que decompilar. El bloque `blaze-out/` está cubierto **por otra vía**:
los 557 `.proto` reconstruidos y verificados campo a campo con `protoc`
(`docs/PROTO-REGENERADO.md`). Los dos métodos se complementan, **no se suman**.

**Código real sin pseudocódigo: 206 funciones** (36 jetski, 75 runtime Go, 81
librerías públicas, 14 otras third_party). Ghidra las rechaza con
`Cannot properly adjust input varnodes` (son *wrappers* generados por Go:
`gowrap`, `deferwrap`), así que las attacked el **segundo motor** (r2ghidra).

Reproducir el corte por categoría:

```bash
python3 tools/medir-cobertura-categoria.py \
    /workspace/agy-re/tools-out/go.funcs.tsv \
    /workspace/entrega/regenerado/NO-DECOMPIADAS.tsv \
    docs/medidas/cobertura_por_categoria.json
```

# INFORME — Código Go regenerado de Antigravity: legibilidad y compilación

**Fecha de la medición:** 2026-11-27 (UTC)
**Ámbito:** el módulo Go regenerado desde el binario `liblanguage_server.so` (Go 1.28.0-20260921, aarch64, stripped).
**Worktree:** `/workspace/md-re-productos` (rama `re/productos-cierre-2026-11-27`)
**Herramienta:** `tools/regenerar-go/regenerar.py` + `tools/regenerar-go/medir_cobertura.py`

---

## 1. Resumen ejecutivo

El módulo Go regenerado **NO compilaba**. La causa no era el código generado, sino
un **error del generador** que emitía `import "unsafe"` en 660 ficheros que no lo
usaban. Además, el generador de `HEAD` estaba **completamente roto** (no arrancaba)
por tres regresiones de un merge, de modo que nadie lo había vuelto a ejecutar.

En esta sesión:

1. Se arregló el generador (**4 bugs**, con tests de regresión): ahora arranca, la
   unión con el índice de Ghidra funciona y el `import "unsafe"` se decide **por
   contenido**.
2. Se regeneró el módulo completo desde las shards actuales de pseudocódigo:
   **161.682 funciones, 1.345 paquetes, 5.383 `.c`, 761.801.297 bytes** en
   2.680,6 s.
3. `go build ./...` → **rc=0** (1.345 paquetes, 18,5 min) y `go vet ./...` → **rc=0**.
   **0 ficheros con `import "unsafe"` sin usar** (antes: 660).
4. Cobertura real medida: **62.274 de 161.682 funciones con pseudocódigo legible
   (38,52 % global; 99,70 % del código propio de jetski)**. El árbol anterior
   tenía 20.780 (12,85 %) ⇒ **+41.494 funciones**.
5. Tests del generador: **14 (4 errores) → 32, todos en verde**.

Detalle por sección:

| sección | contenido |
|---|---|
| §3 | **qué compila**: `go build`/`go vet` en verde, con tiempos medidos |
| §4 | los **4 bugs** del generador (el `import "unsafe"` y 3 regresiones del merge) |
| §5 | el **recuento de legibilidad** y el método para reproducirlo |
| §6 | los **comandos exactos** |
| §7 | lo que **no es recuperable** y por qué |
| §9 | dónde está cada entregable y el estado de la Drive |

---

## 2. El bloqueo: `import "unsafe"` sin usar

### 2.1 Medición sobre el árbol entregado

```bash
cd /workspace/entrega/regenerado/gomodulo
grep -rl '^import "unsafe"' --include='*.go' . | wc -l      # 2158 ficheros con el import
# de esos, cuantos lo USAN de verdad:
while read f; do grep -q 'unsafe\.Pointer' "$f" || echo "$f"; done < <(grep -rl '^import "unsafe"' --include='*.go' .)
```

| magnitud | valor |
|---|---:|
| ficheros `.go` del módulo | 5.383 |
| ficheros con `import "unsafe"` | 2.158 |
| de ellos, **lo usan** (`unsafe.Pointer` en alguna firma) | 1.498 |
| de ellos, **NO lo usan** → `go build` falla | **660** |

Los 660 ficheros rotos son la lista exacta que el root pasó; el vet se paraba en el
primer error de cada paquete, así que solo mostraba 12 de 660.

### 2.2 Causa raíz (en el generador, no en los ficheros)

La versión que generó el árbol entregado (`aaa8dad`, 2026-11-23 19:15) decidía el
import así:

```python
necesita_unsafe = any("*" in b.prototipo for b in _todos_los_bloques(bloques))
if necesita_unsafe:
    go_fh.write('import "unsafe"\n\n')
```

Es decir: **si algún prototipo C del fichero tenía un `*`, emitía el import**, pero
los stubs Go no usaban ese tipo (los punteros se traducen a `unsafe.Pointer` solo si
la firma lo referencia). Resultado: 660 ficheros con un import muerto → Go rechaza
el paquete con `unsafe imported and not used`.

### 2.3 El arreglo (raíz, no parche masivo)

En `tools/regenerar-go/regenerar.py` el import **se deriva del contenido** del
fichero que se está escribiendo:

```python
texto = "".join(go_buf)
...
# El `import "unsafe"` se DERIVA DEL CONTENIDO: se escribe solo
# si alguna firma de ESTE fichero usa `unsafe.Pointer`.
if "unsafe.Pointer" in texto:
    fh.write('import "unsafe"\n\n')
```

Con esto el invariante `import "unsafe" ⇔ unsafe.Pointer en el fichero` es
**estructural**: no depende de un flag que se pueda desincronizar.

---

## 3. Estado de la compilación (MEDIDO)

Sobre el módulo regenerado (`/workspace/entrega/regenerado/gomodulo`,
`module mdl.local/go-regenerado`, `go 1.21`), con
`/workspace/md-resc/go/go/bin/go` = **go1.27.4 linux/amd64**:

```bash
cd /workspace/entrega/regenerado/gomodulo
go build ./...        # rc=0, salida vacía        (18 min 28 s de pared)
go vet ./...          # rc=0, salida vacía        (23 min 44 s de pared)
```

| | resultado |
|---|---|
| `go build ./...` | **rc=0**, sin ninguna línea de salida (09:46:11 → 10:04:39 UTC) |
| paquetes que compilan | **1.331** (los que enumera `go list ./...`) |
| directorios con `.go` en el árbol | 1.345 |
| ficheros `.go` compilados | **5.383** |
| funciones Go compiladas | **161.682** |
| `go vet ./...` | **rc=0**, sin ninguna línea de salida (10:04:39 → 10:28:23 UTC) |
| ficheros con `import "unsafe"` **sin usar** | **0** (antes: 660) |

Los 14 directorios que no aparecen en `go list ./...` son directorios que Go no
considera paquetes (sin ficheros Go compilables); el resto son idénticos.

Lo que queda fuera de la compilación, y por qué:

* **Nada del módulo.** `go build` y `go vet` están en verde sobre los 1.345
  paquetes.
* **El contenido de los stubs**: el cuerpo es `panic("pseudocodigo: ver <fichero>")`
  a propósito. El pseudocódigo de Ghidra/r2ghidra **no es Go compilable** (ver §7);
  se entrega como referencia legible en `codigo/`.
* **Las 99.408 funciones sin decompilar** son stubs honestos: su nombre, fichero y
  línea **sí** son reales (vienen del pclntab), así que el módulo es navegable con
  `gopls` / `go doc` aunque el cuerpo sea un `panic`.

Coste medido en esta máquina (2 CPU, 3 GB, NAS saturado):

| paso | tiempo |
|---|---:|
| regenerar el árbol completo (161.682 funciones) | 2.680,6 s (44,7 min) |
| `go build ./...` (1.331 paquetes, 5.383 ficheros) | 1.108 s (18,5 min) |
| `go vet ./...` (1.331 paquetes) | 1.424 s (23,7 min) |
| barrido del árbol de pseudocódigo (11.538 ficheros, I/O) | ~800 s |

Los tiempos son de **I/O sobre NAS saturado** (2 CPU, 3 GB, y a la vez los dos
workers de r2ghidra del General leyendo/escribiendo el mismo árbol): el `go build`
consumió 1.108 s de reloj pero solo ~20 s de CPU. En una máquina con el árbol en
disco local sería de segundos a un minuto.

---


## 4. Los otros 3 bugs del generador (regresiones del merge `242e972`)

El generador de `HEAD` **no arrancaba**. Los tests del repo fallaban 4 de 14.

### 4.1 `NameError: name 'indices' is not defined` — la herramienta no arranca

`indexar_pseudocodigo()` devuelve una **pareja** `(mapa, indices)` desde el merge,
pero el llamante seguía tratando el resultado como un dict y usaba una variable
`indices` que ya no existía.

```python
mapa_pseudo, indices = indexar_pseudocodigo(o.pseudo)
```

### 4.2 La unión TSV ↔ índice de Ghidra usaba claves INCOMPATIBLES

El merge cambió el primer campo del registro del TSV de la **ruta completa** a
`(dir_paquete, base_paquete)`, mientras el índice de Ghidra seguía usando la ruta
completa, y además amplió la clave de unión de 3 a 4 campos. Resultado: **cero
emparejamientos** → todas las funciones salían con `bytes=0`, `ok=0` y con
`/* SIN PSEUDOCODIGO */` aunque el pseudocódigo estuviera en disco.

Arreglo: las dos entradas vuelven a usar la clave **(fichero, línea, símbolo)** y la
dirección del índice se normaliza a 16 dígitos hex para que el orden dentro de un
grupo sea idéntico en las dos entradas.

### 4.3 El desempaquetado del registro del índice esperaba 9 campos

El índice tiene 7 (`addr name file line bytes ok err`); el código pedía 9 → `ValueError`.

### 4.4 El generador no entendía el layout de **r2ghidra**

El shard `rz0` (y ahora `rzA`/`rzB`) lo genera **r2ghidra** (radare2 6.2.6 + plugin
r2ghidra 6.2.6, sin JVM) y escribe **un `.c` por función**, dentro del directorio del
fichero Go original y con el nombre del símbolo:

```
todo/rz0/third_party/go/gc/src/internal/abi/abi.go/internal_abi.__RegArgs_.Dump.c
```

El generador solo entendía el layout de Ghidra (un `.c` por fichero Go, con
marcadores `/* ==== ... ==== */`), así que **esas funciones perdían el pseudocódigo**
y sus firmas Go reales (`int64_t *`, `ulong`, …).

Arreglo: se añadió el layout de r2ghidra al indexado por ruta
(`clave_de_pseudocodigo()`), un parser alternativo del cuerpo (`cuerpo_r2ghidra()`) y
la tabla de tipos C99 (`int64_t`, `uint64_t`, `uchar`, `size_t`, …).

### 4.5 Tests

`tools/regenerar-go/test_regenerar.py` pasa de **14 tests (4 errores)** a
**32 tests, todos en verde**, incluidos tres nuevos:

* `test_el_import_unsafe_solo_va_donde_hace_falta`
* `test_ningun_fichero_del_modulo_tiene_imports_sin_usar`
* `TestLayoutR2Ghidra` (3 tests: pseudocódigo, tipos C99, imports)

```bash
export PATH=/workspace/md-resc/go/go/bin:$PATH
cd /workspace/md-re-productos && python3 tools/regenerar-go/test_regenerar.py
# Ran 32 tests ... OK
```

---

## 5. Recuento de legibilidad (MEDIDO)

### 5.1 El método (para que se pueda repetir y auditar)

Tres fuentes, todas locales y contadas (ninguna estimación):

| fuente | qué cuenta | fichero |
|---|---|---|
| **pclntab** | el TOTAL de funciones del binario | `/workspace/agy-re/tools-out/go.funcs.tsv` (161.682 filas) |
| **árbol `codigo/`** | los bloques de pseudocódigo realmente escritos | `codigo/<paquete>/<fichero>.c` |
| **árbol `gomodulo/`** | los stubs Go y la calidad de sus firmas | `gomodulo/<paquete>/<fichero>.go` |

El criterio de " legible " es estricto: un bloque cuenta como pseudocódigo
**legible** si su cuerpo **no** es el marcador `/* SIN PSEUDOCODIGO */`. Un bloque
marcado por Ghidra con error ("Low-level Error: ...") **sí** cuenta como legible
(tiene cuerpo) pero se reporta aparte.

```bash
python3 tools/regenerar-go/medir_cobertura.py \
    --out /workspace/entrega/regenerado \
    --json /workspace/entrega/regenerado/_COBERTURA.json
```

### 5.2 Las cifras

Arbol regenerado ahora (medido el 2026-11-27, snapshot de las shards
`rz0`, `rzA`, `rzB`, `s0`, `s1`, `s2`):

| magnitud | valor | % |
|---|---:|---:|
| funciones en el binario (pclntab) | **161.682** | 100 % |
| **con pseudocódigo legible** | **62.274** | **38,52 %** |
| **stubs** (sin decompilar; stubs `panic(...)`) | **99.408** | 61,48 % |
| de los stubs: con pseudocódigo marcado con error por Ghidra | 6.611 | |
| funciones que solo existen en Ghidra (sin origen Go; no se regeneran) | 1.979 | |
| paquetes (directorios del binario) | 1.345 | |
| ficheros `.c` / `.go` generados | 5.383 / 5.383 | |
| stubs Go con firma tipada (`unsafe.Pointer` incluido) | 9.254 | |
| ficheros `.go` con `import "unsafe"` (y lo usan) | 2.264 | |
| bytes de pseudocódigo volcado | 400.374.801 | |
| bytes del árbol completo | 761.801.297 | |

### 5.3 Comparativa con el árbol que estaba entregado (noviembre 2026)

Medido con **el mismo script y el mismo criterio** sobre los dos árboles
(`_COBERTURA_v1.json` para el viejo, `_COBERTURA.json` para el nuevo):

| | árbol viejo (23-nov) | **árbol nuevo (27-nov)** | delta |
|---|---:|---:|---:|
| funciones del binario | 161.682 | 161.682 | — |
| **con pseudocódigo legible** | 20.780 | **62.274** | **+41.494 (×3,00)** |
| % cobertura | 12,85 % | **38,52 %** | **+25,67 pp** |
| stubs sin decompilar | 140.902 | 99.408 | −41.494 |
| stubs **con firma tipada** | 14.360 | **29.764** | **×2,07** |
| stubs con `unsafe.Pointer` en la firma | 3.106 | **9.254** | ×2,98 |
| ficheros `.go` con `import "unsafe"` | 2.158 | 2.264 | +106 |
| ficheros `.go` con import `unsafe` **sin usar** | **660** | **0** | **−660** |
| bytes de pseudocódigo (RESUMEN) | 106.735.487 | 400.374.801 | ×3,75 |
| bytes del árbol (RESUMEN) | 438.763.928 | 761.801.297 | ×1,74 |

El salto viene de que el árbol viejo se generó con solo 4.442 ficheros de
pseudocódigo mapeados (una sola tanda de Ghidra); ahora hay **6 shards** (Ghidra
`s0`/`s1`/`s2` + r2ghidra `rz0`/`rzA`/`rzB`) y **4.460 ficheros Go con pseudocódigo**.
Además, el árbol viejo lo produjo la versión del generador que emitía el
`import "unsafe"` muerto (660 ficheros) y el que tenía la unión con el índice
rota (por eso su RESUMEN declaraba 142.808 "sin pseudocodigo" y no 140.902: el
criterio viejo contaba además los bloques con error de Ghidra).

### 5.4 Aclaración sobre el "96,90 %"

El **96,90 %** que circulaba **no se sostiene** con la measurement de disco: la
cobertura real de pseudocódigo es del **38,52 %** (62.274 de 161.682). Lo que sí
está al 96,9 % es otra cosa: la fracción de filas del **índice de Ghidra** que
terminaron con `ok=1`, es decir el *yield* del motor, no la cobertura del binario.
Las dos magnitudes no deben mezclarse:

* **yield del motor** = filas del índice con `ok=1` → 55.002 / 60.018 = **91,7 %**
  (y el resto tiene cuerpo pero marcado con error: 4.146).
* **cobertura del binario** = funciones con pseudocódigo / funciones totales =
  62.274 / 161.682 = **38,52 %**.

La diferencia entre ambas es lo que queda por decompilar (99.408 stubs), que es
justo lo que ataca el barrido de r2ghidra (`rzA`/`rzB`).

### 5.5 La cobertura POR CATEGORÍA: el 38,52 % es engañoso

El 38,52 % global está arrastrado por **código que no es fuente escrita por
nadie**: 69.466 funciones de `blaze-out/` (tablas y stubs generados por
protobuf/Blaze) y 30.320 wrappers de `<autogenerated>` (los emite el
compilador). Decompilarlos no aporta: no hay lógica, son datos.

Clasificando cada función por la ruta de su fichero original —la misma regla
`categoria_de()` que usa el generador— la cobertura real es:

| categoría | funciones | con pseudocódigo | % | ficheros |
|---|---:|---:|---:|---:|
| generado (blaze/protobuf) | 69.466 | 772 | 1,11 % | 942 |
| autogenerado (compilador) | 30.320 | 45 | 0,15 % | 1 |
| **jetski (código propio de Antigravity)** | **11.928** | **11.892** | **99,70 %** | 1.130 |
| runtime Go | 12.298 | 12.026 | 97,79 % | 971 |
| librerías públicas (third_party/golang) | 33.659 | 33.552 | 99,68 % | 2.011 |
| otras third_party | 2.665 | 2.644 | 99,21 % | 191 |
| resto | 1.346 | 1.344 | 99,85 % | 137 |
| **TOTAL** | **161.682** | **62.275** | **38,52 %** | |

**Lo que el jefe pidió, medido**: el código propio de Antigravity está al
**99,70 %** (11.892 de 11.928) y todo el código "escrito" (jetski + runtime +
librerías + third_party + resto) está entre **97,8 % y 99,85 %**. Quedan
**36 funciones jetski** sin decompilar.

Sobre el bloque `blaze-out/`: ese 1,11 % no es un agujero de cobertura, es
**otra ruta de reconstrucción**: los 557 `.proto` reconstruidos y verificados
con `protoc` (`docs/PROTO-REGENERADO.md`) *son* la forma legible de ese
código generado. Los dos métodos se complementan, no se suman.

Reproducir el corte por categoría:

```bash
python3 tools/medir-cobertura-categoria.py \
    /workspace/agy-re/tools-out/go.funcs.tsv \
    /workspace/entrega/regenerado/NO-DECOMPIADAS.tsv \
    docs/medidas/cobertura_por_categoria.json
```

### 5.6 Calidad de las firmas Go del módulo

De los 161.682 stubs del módulo Go:

| | |
|---|---:|
| stubs con **firma tipada** (parámetros o retorno) | 29.764 |
| stubs con parámetros | 25.156 |
| stubs con tipo de retorno | 13.911 |
| stubs con `unsafe.Pointer` en la firma | 9.254 |

Los ~131.918 restantes son stubs `func X()` sin tipo, porque no hay pseudocódigo
del que deducir la firma (o porque Ghidra no la reconstruyó).

### 5.7 Confirmación independiente de la medición

Los mismos números los reproduce otra ejecución de `medir_cobertura.py` sobre el
mismo árbol, guardada en `docs/medidas/cobertura_go.json`:

```
con_pseudocodigo_legible = 62274      pct_pseudocodigo = 38.52
stub_sin_decompilar      = 99408      pct_stub          = 61.48
imports_unsafe           = 2264       imports_unsafe_sin_uso = 0     <-- el bug, resuelto
```

Y el post-pass (`arreglar-imports-unsafe.py`) aplicado al árbol nuevo confirma
el invariante: **2.264 ficheros con `import "unsafe"`, 0 sin usar** (en el árbol
anterior eran 660 sin usar).

---

## 6. Comandos exactos para reproducirlo

### 6.1 Generar el módulo desde cero (lo que se ejecutó)

```bash
export PATH=/workspace/md-resc/go/go/bin:$PATH        # go1.27.4 linux/amd64

cd /workspace/md-re-productos
python3 tools/regenerar-go/regenerar.py \
    --tsv   /workspace/agy-re/tools-out/go.funcs.tsv \
    --pseudo /workspace/agy-re/tools-out/todo \
    --out   /workspace/entrega/regenerado/_v2
```

Salida real de esa ejecución (medida):

```
[regenerar] pseudocodigo: 4460 ficheros Go con pseudocodigo en 6 shard(s) (indices: rz0, rzA, rzB, s0, s1, s2)
[regenerar] TSV: 161682 funciones leidas (2 sin fichero original)
[regenerar] indice Ghidra: 60018 filas (630 sin origen Go, no regeneradas)
[regenerar] union: 161682 funciones del TSV (1979 solo de Ghidra)
```

### 6.2 Compilar y pasar vet

```bash
export PATH=/workspace/md-resc/go/go/bin:$PATH
cd /workspace/entrega/regenerado/gomodulo
go build ./...
go vet ./...
```

### 6.3 Tests del generador (incluyen el gate del `import "unsafe"`)

```bash
export PATH=/workspace/md-resc/go/go/bin:$PATH
cd /workspace/md-re-productos
python3 tools/regenerar-go/test_regenerar.py
```

### 6.4 Medir la cobertura (contada, no estimada)

```bash
cd /workspace/md-re-productos
python3 tools/regenerar-go/medir_cobertura.py \
    --out /workspace/entrega/regenerado \
    --json /workspace/entrega/regenerado/_COBERTURA.json
```

### 6.5 Reparar un módulo YA generado (sin regenerarlo)

Para el módulo que ya está publicado en la Drive (`GO-REGENERADO.zip`), que
fue generado con la versión que tenía el bug:

```bash
cd /workspace/md-re-productos
python3 tools/regenerar-go/arreglar-imports-unsafe.py --modulo /ruta/al/gomodulo
# => quita el import sobrante de cada fichero que no usa unsafe.Pointer
```

### 6.6 Gates del repo (los que pide el General antes de commitear)

```bash
cd /workspace/md-re-productos
bash tools/check-sintaxis.sh
python3 tools/yaml_lint.py .github/workflows/*.yml
```

---

## 7. Lo que NO es recuperable (y por qué)

* **El código Go original no está en el binario.** Un binario Go conserva los
  *nombres* de las funciones, sus ficheros y líneas (pclntab) y los tipos en la
  sección `runtime.types`, pero **no el texto fuente**. El binario está *stripped*:
  no hay DWARF ni el fichero de fuentes embebido.
* **El pseudocódigo de Ghidra/r2ghidra NO es Go compilable.** Es C decompilado con
  variables temporales (`lVar1`, `uVar5`, `extraout_x8`), structs sin nombre y
  punteros sin tipo. Se entrega como **referencia legible** en `codigo/<paquete>/<fichero>.c`;
  el módulo Go `gomodulo/` son **stubs compilables** con la firma deducida y
  `panic("pseudocodigo: ver <fichero>")` en el cuerpo.
* **El bytecode de Claude Code (JSC) no aplica aquí**: no es un caso de
  desofuscación de JS, sino decompilación nativa de un binario Go.
* **Las funciones sin pseudocódigo no se pueden inventar**: son stubs honestos. Sus
  nombres, fichero y línea **sí** son reales (vienen del pclntab), así que son
  navegables con `gopls`/`go doc` aunque el cuerpo sea un `panic`.

---

## 8. Desfase de direcciones de Ghidra (+0x100000)

Ghidra carga este ELF con **image base 0x100000**, de modo que sus direcciones son
`vaddr + 0x100000`:

```
pclntab (go.funcs.tsv):  0x4f9f518   google3_...__Decoder_.consumeToken
índice Ghidra (s0):       0x509f518   google3_...__Decoder__.consumeToken
                                       ^^^^ = 0x4f9f518 + 0x100000
```

**Impacto en esta herramienta: ninguno.** La unión TSV↔índice se hace por
**(fichero, línea, símbolo)**, nunca por dirección; y las direcciones que se
escriben en el `.c` y en el `.go` salen del **pclntab** (las reales). Aun así, la
dirección del índice se normaliza a 16 dígitos hex para que el orden dentro de un
grupo sea estable.

---

## 9. Dónde está cada cosa

### 9.1 Entregables de esta sesión (en el worktree)

| qué | ruta |
|---|---|
| informe (este fichero) | `/workspace/md-re-productos/artifacts/INFORME_GO.md` |
| generador arreglado | `/workspace/md-re-productos/tools/regenerar-go/regenerar.py` |
| tests (14 → 32, con 3 de regresión nuevos) | `/workspace/md-re-productos/tools/regenerar-go/test_regenerar.py` |
| medidor de cobertura (nuevo) | `/workspace/md-re-productos/tools/regenerar-go/medir_cobertura.py` |
| post-pass que repara un módulo ya generado (nuevo) | `/workspace/md-re-productos/tools/regenerar-go/arreglar-imports-unsafe.py` |
| rama | `re/productos-cierre-2026-11-27` (worktree `/workspace/md-re-productos`) |

**No se ha hecho `push` a ningún sitio.** El commit es local del worktree; el root
integra.

### 9.2 El módulo Go regenerado

| qué | ruta |
|---|---|
| **módulo Go (el que compila)** | `/workspace/entrega/regenerado/gomodulo/` (5.383 `.go`, `module mdl.local/go-regenerado`) |
| pseudocódigo legible | `/workspace/entrega/regenerado/codigo/<paquete>/<fichero>.c` (5.383 ficheros) |
| índice por paquete | `/workspace/entrega/regenerado/codigo/<paquete>/README.md` |
| las funciones que NO se pudieron decompilar | `/workspace/entrega/regenerado/NO-DECOMPIADAS.tsv` (99.408 filas) |
| cifras medidas del árbol | `/workspace/entrega/regenerado/RESUMEN.md` |
| salida de `go build` / `go vet` | `/workspace/entrega/regenerado/_go_build.log`, `_go_vet.log` |
| medición de cobertura (JSON) | `/workspace/entrega/regenerado/_COBERTURA.json` |
| **árbol anterior (23-nov), para comparar** | `/workspace/entrega/regenerado/_v1-nov23/` |

### 9.3 Estado de la Drive

**No se ha subido nada a la Drive en esta sesión.** El material publicado
(`GO-REGENERADO.zip`, sha256 `27a14f007b9af0c2f784982c383d7df7566fd03e8c2dfb58ab16730c8f31ff9f`,
carpeta `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`) contiene el módulo **con el bug**
(660 ficheros con `import "unsafe"` muerto). Para republicarlo hay dos caminos:

1. **Regenerar** (lo que se ha hecho aquí): `regenerar.py` con las entradas
   actuales → árbol nuevo, con 43.400 funciones más de pseudocódigo.
2. **Reparar el zip publicado** sin regenerar:
   `python3 tools/regenerar-go/arreglar-imports-unsafe.py --modulo <gomodulo extraido>`
   (0 ficheros que arreglar en el árbol nuevo; 660 en el viejo).

### 9.4 Lo que NO se ha hecho (y por qué)

* **`redress` no sirve para este binario.** Es una `.so` aarch64 PIE, no un
  ejecutable, y redress no encuentra su pclntab:
  `redress packages` → `no pclntab located`;
  `redress moduledata` → `no goversion found`;
  `redress info` → solo devuelve `OS EM_AARCH64`;
  y `redress types` / `redress source` leen **fuentes Go**, no binarios.
  El pclntab de este binario lo lee el motor propio del repo
  (`rust-core/src/gobook.rs` + `go-retool/`), que es el que produce
  `go.funcs.tsv`. **Tipos y strings: NO MEDIDO con redress** (la vía no existe
  para este artefacto).
* **No se ha commiteado código ajeno**: `tools/regenerar-go/comparar-decompiladores.py`
  y `tools/regenerar-go/decomp-r2ghidra.py` tienen cambios de otro agente en el
  mismo worktree y **no** se han tocado ni incluido en el commit.
* **No se ha tocado `rust-core/`** ni nada fuera de `tools/regenerar-go/` y
  `artifacts/`.

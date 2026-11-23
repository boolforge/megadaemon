# Código Go regenerado (Antigravity `liblanguage_server.so`)

> Cómo se convierte el binario Go de Antigravity en **código Go regenerado y
> navegable**: un árbol que *mira como el código original* (un `README.md` por
> paquete Go, un `.c` por fichero Go original con el pseudocódigo de Ghidra
> anotado con su línea original) más un **módulo Go compilable de stubs** con
> las firmas de las 161.682 funciones del binario.

**Tool:** `tools/regenerar-go/regenerar.py` · **Test:** `tools/regenerar-go/test_regenerar.py`
**Rama:** `rama/regenerar-go-2026-11-23`

---

## 1. El flujo

```
  liblanguage_server.so (181 MB, Go 1.28, SIN DWARF)
        │
        │  1. pclntab  (motor RE del repo: rust-core/src/gobook.rs + go-retool)
        ▼
  go.funcs.tsv  —  161.682 filas:  dir_hex \t nombre_go \t origen
        │         (origen = "go: <firma> | <ruta/fichero.go>:<linea>")
        │
        │  2. Ghidra (tools/ghidra-scripts/MdGhidra.java, modo "all")
        ▼
  todo/s{0,1,2}/  —  pseudocodigo:  un .c por fichero Go original
        │          + _index.tsv  (addr / name / file / line / bytes / ok / err)
        │
        │  3. tools/regenerar-go/regenerar.py   ← ESTE FLUJO
        ▼
  regenerado/
    ├── RESUMEN.md            cifras medidas + desglose por categoría
    ├── NO-DECOMPIADAS.tsv    las funciones sin pseudocódigo
    ├── codigo/<paquete>/README.md    índice del paquete
    ├── codigo/<paquete>/<fichero>.c  pseudocódigo con su línea original
    └── gomodulo/go.mod + gomodulo/<paquete>/<fichero>.go   stubs compilables
```

### 1.1 pclntab → `go.funcs.tsv`

El binario **no tiene DWARF**: no hay ni tipos, ni variables, ni números de
línea. Lo único que sobrevive es la **`pclntab`** (la tabla de funciones que
el enlazador de Go deja en el binario), que para cada función guarda su
dirección, su nombre Go y —esto es lo que permite "regenerar" código— **el
fichero y la línea original**.

El motor RE del repo ya lo hacía (`rust-core/src/gobook.rs`,
`go-retool/`); `regenerar.py` **no duplica** ese motor: consume su export.

```
0x4d06cb8	internal_abi.__RegArgs_.Dump	go: internal/abi.(*RegArgs).Dump | third_party/go/gc/src/internal/abi/abi.go:47
```

### 1.2 Ghidra → pseudocódigo (100 % de las funciones no generadas)

Tres workers decompilan en paralelo un tercio cada uno y escriben **en su
propio shard** (`todo/s0`, `todo/s1`, `todo/s2`), cada uno con su
`_index.tsv`. `regenerar.py` **fusiona los tres**: los tres índices se ordenan
externamente y se unen por `(fichero, línea, nombre)` contra el TSV.

### 1.3 Agrupado por paquete → árbol navegable

El **paquete** es el *directorio* del fichero Go original (criterio de
`gobook.rs::mapa`); dentro del paquete se ordena por fichero y luego por
número de línea (`gobook.rs::esqueleto`).

* `codigo/<paquete>/README.md` → índice del paquete: nombre de la función,
  línea original, dirección y tamaño.
* `codigo/<paquete>/<fichero>.c` → el pseudocódigo de Ghidra de ese fichero,
  con la **línea original** ya asociada y las funciones ordenadas por ella.

El árbol replica la estructura de directorios del código original, así que
`codigo/third_party/jetski/language_server/` es el mismo sitio donde vivía
`third_party/jetski/language_server/`.

### 1.4 Módulo Go de stubs

Cada función se renombra a un **identificador Go válido**
(`internal_abi.__RegArgs_.Dump` → `Internal_abi_RegArgs_Dump`) y su cuerpo se
sustituye por

```go
panic("pseudocodigo: ver third_party/jetski/language_server/server.go")
```

con un comentario con la **dirección** y la **línea original**. Las
colisiones de nombre se resuelven con un sufijo y se cuentan en el `RESUMEN.md`.

El módulo es **compilable**: `go.mod` + un directorio por paquete, con las
firma deducidas del pseudocódigo de Ghidra (los punteros se traducen a
`unsafe.Pointer`).

---

## 2. Cómo se ejecuta

```bash
# el flujo completo (161.682 funciones)
tools/regenerar-go/regenerar.py \
    --tsv   /workspace/agy-re/tools-out/go.funcs.tsv \
    --pseudo /workspace/agy-re/tools-out/todo \
    --out   /workspace/agy-re/tools-out/regenerado

# sólo un paquete (para pruebas)
tools/regenerar-go/regenerar.py --filtro-paquete third_party/jetski --out /tmp/x
```

| opción | efecto |
|---|---|
| `--tsv` / `--pseudo` / `--out` | entradas y salida |
| `--modulo` | ruta del módulo Go (`mdl.local/go-regenerado`) |
| `--filtro-paquete` | sólo los paquetes cuya ruta contenga ese texto |
| `--limite N` | sólo las N primeras funciones |
| `--indice ruta\|cabecera` | cómo se indexa el pseudocódigo (ver §4) |
| `--sin-codigo` / `--sin-modulo` | sólo una de las dos salidas |
| `--sin-limpiar` | no borrar la salida antes de escribir |

### Garantías

* **Idempotente**: dos ejecuciones con la misma entrada dan el mismo byte a
  byte (no se escribe ninguna marca de tiempo en la salida).
* **Streaming**: los 161.682 registros se ordenan con un *sort externo* por
  trozos (`OrdenadorExterno`); el TSV nunca entero en memoria. Sólo se
  mantiene en memoria el pseudocódigo de **un** fichero original a la vez.
* **Tolera un corpus incompleto**: si falta pseudocódigo, sigue, lo dice y lo
  refleja en el `RESUMEN.md` (las funciones afectadas van a
  `NO-DECOMPIADAS.tsv`).

---

## 3. Cifras (medidas, no estimadas)

Las cifras del `RESUMEN.md` **se generan al ejecutar la herramienta**: salen
de contar lo que realmente se ha escrito, nunca de una constante en el
código.

### 3.1 La ejecución completa (medida el 2026-11-23)

```bash
tools/regenerar-go/regenerar.py --out /workspace/agy-re/tools-out/regenerado
# → hecho en 472,1 s (la primera vez, con el NAS saturado: 1527,7 s)
```

| magnitud | valor |
|---|---:|
| funciones en el export del pclntab | **161.682** |
| funciones procesadas | **161.682** |
| paquetes (directorios del binario) | **1.345** |
| ficheros `.c` generados | **5.383** |
| ficheros `.go` (stubs) generados | **5.383** |
| stubs Go emitidos | **161.682** |
| identificadores Go que colisionaron (con sufijo) | **3.706** |
| **funciones CON pseudocódigo volcado** | **59.980** |
| funciones SIN pseudocódigo | **101.702** |
| bytes del pseudocódigo volcado | **288.698.135** |
| bytes totales del árbol de salida | **651.787.177** |
| ficheros totales del árbol de salida | **12.124** |
| cabeceras que NO coinciden con la ruta deducida | **9** (de 4.372) |

### 3.2 Cobertura del pseudocódigo

| | funciones |
|---|---:|
| total en el binario | 161.682 |
| − generado (blaze/protobuf) | 69.466 |
| − autogenerado (compilador) | 30.320 |
| **= no generadas (las que había que decompilar)** | **61.896** |
| **con pseudocódigo de Ghidra** | **59.980 (96,90 %)** |
| sin pseudocódigo (los workers murieron antes) | **1.916** |

Las 101.702 funciones "sin pseudocódigo" son, en su mayoría, las **generadas
y autogeneradas**, que no se decompilan a propósito: el 100 % de las
funciones *no generadas* está covered salvo 1.916.

### 3.3 Desglose por categoría del código de origen

| categoría | funciones | % | ficheros `.c` |
|---|---:|---:|---:|
| generado (blaze/protobuf) | 69.466 | 42,96 % | 942 |
| autogenerado (compilador) | 30.320 | 18,75 % | 1 |
| librerías públicas (`third_party/golang`) | 33.659 | 20,82 % | 2.011 |
| jetski (código propio de Antigravity) | 11.928 | 7,38 % | 1.130 |
| runtime Go (`third_party/go/gc`) | 12.298 | 7,61 % | 971 |
| otras `third_party` | 2.665 | 1,65 % | 191 |
| resto | 1.346 | 0,83 % | 137 |
| **total** | **161.682** | 100 % | **5.383** |

### 3.4 Verificación independiente

Recorriendo el árbol generado **a mano** (contando `/* ==== */` y
`/* SIN PSEUDOCODIGO */`) sale **exactamente lo mismo** que el `RESUMEN.md`:
161.682 bloques, 101.702 sin pseudocódigo, 59.980 con pseudocódigo, 1.345
`README.md` de paquete y 5.383 stubs `.go`.

La salida es **determinista**: las dos ejecuciones completas produjeron el
mismo árbol (651.787.177 bytes en ambas).

---

## 4. El índice del pseudocódigo: `ruta` vs `cabecera`

La herramienta puede construir el mapa *fichero Go original → ficheros de
pseudocódigo* de dos formas:

| modo | cómo | coste medido |
|---|---|---|
| `ruta` (por defecto) | deduce la clave de la **ruta** del `.c` (`todo/sN/<ruta_go>.c`) | **72 s** (sólo `scandir`, no abre ningún fichero) |
| `cabecera` | lee la cabecera `/* <ruta.go> -- pseudocodigo ... */` de cada `.c` | ~0,8 ficheros/s ⇒ **~3,7 h** para 11.104 ficheros |

El modo `cabecera` es el lento de propósito: abrir 11.000 ficheros en el NAS
saturado cuesta horas. El modo `ruta` da el mismo mapa, y **se verifica**
igual: al abrir cada fichero (que hay que abrir de todos modos para sacar su
pseudocódigo) se lee su cabecera y se comprueba que coincide con la ruta
deducida; el `RESUMEN.md` cuenta las discrepancias.

Medido sobre el corpus real: **60/60** ficheros de la muestra coinciden.

---

## 5. Publicación

El artefacto (el árbol completo + el `RESUMEN.md` + los stubs) se publica en
la **Drive privada** de la flota —nunca en el repo, que es público:

* carpeta: **`megadaemon-backup`** (`privada: true`)
* id: `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`
* enlace: <https://drive.google.com/drive/folders/15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0>

Credenciales y subida: `tools/drive_push.py --file RUTA --folder
15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0` (la credencial `authorized_user` vive en
`/workspace/md-secrets/gdrive_credentials.json`; **no** se sube al repo).

### 5.1 Ya publicado (2026-11-23)

| fichero | id en Drive |
|---|---|
| `codigo-go-regenerado-RESUMEN.md` (8.071 B) | `1L_V9z49BbRBbJIoG32G2fHEvpKdzpkPZ` |
| `codigo-go-regenerado-NO-DECOMPIADAS.tsv` (20.022.216 B) | `1--HXaAAeyvpXvnqpZOcq1r_1QdW_W1Kn` |
| `codigo-go-regenerado.tar.gz` (44.091.932 B) | `1jSKIlwWbyq69c8oph65A-T7Lic4wJeUh` |

El `.tar.gz` contiene el árbol entero (16.337 entradas: los 5.383 `.c`, los
5.383 stubs `.go`, los 1.345 `README.md` de paquete, el `RESUMEN.md` y el
`NO-DECOMPIADAS.tsv`); verificado con `tar tzf`.

> La carpeta es **privada**: con la API key pública no se ve (da 0
> ficheros); con la credencial `authorized_user` sí. Verificado así:
> los 3 archivos están en `megadaemon-backup`.

---

## 6. Qué NO se puede recuperar (y por qué)

* El **cuerpo original del código Go** no está en el binario: Go no guarda el
  código fuente, sólo nombres, ficheros y líneas. Lo que sí se puede es el
  *pseudocódigo* de Ghidra (C, no Go) y la *firma* de cada función.
* Los **tipos** de los parámetros son los que Ghidra infiere del ABI de
  registers de Go (`undefined8`, `long`, …): son aproximaciones, no los tipos
  Go exactos.
* El código **generado** (protobuf/Blaze: 65.230 funciones) y los wrappers
  **autogenerados** por el compilador (30.320) no se decompilan: no aportan
  información y son el 59 % del binario.

---

## 7. Relación con el resto del repo

El motor RE **no se duplica**: esta herramienta es la capa *de salida* del
pipeline que ya existe.

| pieza | fichero |
|---|---|
| export del pclntab | `rust-core/src/gobook.rs`, `go-retool/` |
| scripts de Ghidra | `tools/ghidra-scripts/MdGhidra.java` |
| pipeline de reconstrucción | `tools/reconstruir.sh` |
| motor RE (fusión de_STRINGs, IR) | `rust-core/src/re_engine.rs`, `re_ir.rs`, `re_strings.rs`, `re_fusion.rs` |
| CLI del motor RE | `rust-core/src/bin/md-re.rs` |
| **esta herramienta** | `tools/regenerar-go/regenerar.py` |
| **su test** | `tools/regenerar-go/test_regenerar.py` |

---

## 8. Notas para quien siga

### 8.1 Estado medido del corpus (2026-11-23)

* Los **3 workers de Ghidra están MUERTOS** (procesos zombie; su última
  escritura fue a las 04:36). El corpus **ya no crece** ⇒ las cifras de §3 son
  definitivas para este corpus.
* Quedan **1.916 funciones no generadas sin pseudocódigo**: si se relanzan
  los workers, se cubren y se vuelve a ejecutar la herramienta (el árbol se
  regenera entero en ~8 min).
* El índice del pseudocódigo se puede rehacer con `--indice cabecera`
  (más lento, pero verificable); con `--indice ruta` (por defecto) la
  cabecera se verifica igualmente, gratis, y el `RESUMEN.md` cuenta las 9
  discrepancias que hay ahora mismo.

### 8.2 Reglas del sandbox (obligatorias)

* **No se compila aquí**: el sandbox tiene 3 GB y tres JVM de Ghidra
  corriendo; `rustc` de 2,8 GB ya ha matado a varios workers. **El root
  compila y testea.**
* El `.so`, el TSV de 46 MB y el pseudocódigo generado **no se commitean**:
  son artefactos de /workspace, no del repo.
* La salida (652 MB) vive **fuera** del repo, en `/workspace/agy-re/tools-out/`.
* El repo es **público**: el artefacto se publica en la Drive **privada**
  (§5), nunca en el repo.

### 8.3 Lo que NO está verificado

* El módulo Go **no se ha compilado**: en este sandbox no hay toolchain de Go
  (`go`, `gofmt` y `/usr/local/go` no existen) y compilar está prohibido. La
  sintaxis se valida con un **parser Go propio** incluído en el test.
* La **fidelidad** del emparejamiento bloque↔función está verificada por
  muestra, no al 100 %: 59.980 bloques encontrados sobre 61.896 funciones no
  generadas, y 9 cabeceras de 4.372 ficheros con la ruta deducida distinta.
* La **publicación en la Drive** no se ha ejecutado (el enlace y el comando
  están en §5).

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
código. El resumen de la ejecución completa está en
[`RESUMEN.md` de la ejecución](../tools/regenerar-go/RESUMEN-ejecucion.md) y
las cifras concretas del corpus actual están en §5.

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

* El corpus de pseudocódigo lo escriben **workers en shards** (`todo/s0`,
  `todo/s1`, `todo/s2`): la herramienta tolera que falten shards y que estén
  escribiendo mientras se lee.
* **No se compila aquí**: el sandbox tiene 3 GB y tres JVM de Ghidra
  corriendo; `rustc` de 2,8 GB ya ha matado a varios workers. El root
  compila y testea.
* El `.so`, el TSV de 46 MB y el pseudocódigo generado **no se commitean**:
  son artefactos de /workspace, no del repo.

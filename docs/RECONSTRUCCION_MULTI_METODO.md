# Reconstrucción multi-método del binario de Antigravity

> Regla: **ningún motor solo da la reconstrucción completa**. Se ejecutan varios,
> se comparan y se fusiona lo mejor de cada uno. Todo lo de aquí está medido
> en esta sesión; lo que no se ha ejecutado dice `PENDIENTE`.

## 1. Por qué varios métodos

| método | dónde vive | qué aporta de verdad | límite medido |
|---|---|---|---|
| **Ghidra headless** | `rust-core/src/ghidra.rs` (ya existía) | **pseudocódigo C real** por función, con los 161.682 símbolos Go aplicados | necesita JDK 21 + ~1,4 GB de heap; ~0,8 s por función |
| **Rizin 0.9.1** | `rust-core/src/frida_dbi.rs` (ya existía) | formato, secciones, funciones, imports, **strings con dirección** | no pseudocódigo |
| **DiE + Binwalk** | `rust-core/src/binwalk_die.rs` | familia de formato, OS, compilador, mitigaciones, entropía, firmas embebidas | no ve la lógica |
| **Motor propio** | `re_engine.rs` + `re_strings/re_ir/re_cff/re_crypto/re_opaque` | cadenas cifradas, predicados opacos, MBA, control-flow flattening | heurístico |
| **pclntab / `unstrip`** | `go-retool/` + export de símbolos | **nombre Go + fichero:línea original** de cada función | no código |

**La fusión** (pseudocódigo de Ghidra + origen Go del pclntab + strings de
Rizin) es lo que ningún motor da solo: es `rust-core/src/re_fusion.rs`.

## 2. Lo medido en esta sesión

### 2.1 Entorno (faltaba JDK ⇒ instalado)

* `openjdk-17` **no sirve**: Ghidra 11.2+ declara `application.java.min=21`.
* **Temurin JDK 21.0.13.1+2** descargado y en `/opt/jdk-21.0.13.1+2`.
* Ghidra **11.2.1** en `/workspace/agy-re/tools-out/ghidra_11.2.1_PUBLIC`
  (también hay 12.2 descargada).

### 2.2 Ghidra sobre `liblanguage_server.so` (181.358.800 B)

| fase | resultado medido |
|---|---|
| import (`-noanalysis`) | **OK**, proyecto de 249 MB |
| aplicar símbolos (`go.funcs.tsv`) | **ok=161.682 dup=0 err=0 skipped=0** en **22,9 s** |
| funciones creadas | 1.214 → 1.297 (según el programa) |
| decompilación `gateway` | **81/81** con pseudocódigo, 69 s |
| decompilación `language_server` | 911 anclas, por lotes |

Detalle que costó sangre (ya resuelto en el módulo, no reimplementado):
Ghidra carga este ELF con **image base 0x100000**, así que las direcciones
`UNK_XXXXXXXX` son `vaddr + 0x100000` ⇒ para resolver un string hay que
restar 0x100000.

### 2.3 Rizin 0.9.1

Datos que sólo Rizin dio:

```
format : elf64 · arch arm · 64 bits
compiler: go1.28-20260921-RC02 cl/985595000 +644cd61a4a X:fieldtrack,
          boringcrypto, simd  Linker: LLD google3-trunk Ubuntu clang 14.0.6
```

⇒ **Go 1.28-RC** ⇒ por eso el `pclntab` es del formato moderno (functab de
1.18/1.20) y hace falta el parser conservador de `go-retool`.

### 2.4 DiE + Binwalk

Formato, OS, mitigaciones, entropía global, secciones y firmas embebidas
(artefactos que se pueden carvear).

## 3. Lo que esto ha revelado del catálogo de modelos

De los 81 ficheros de `jetski/gateway` decompilados y con los strings
resueltos (`tools/resolver-strings-ghidra.py`) salen **16 cadenas reales**:

**Tools que `AdminFileSetsModels` sabe fijar** (son las que el fichero de
administración puede activar/desactivar):

```
ask_custom_permission   ask_permission   define_subagent
list_permissions         manage_inbox     manage_subagents
manage_task              schedule         send_message
```

**Checks que `ParseAdminSettings` valida**:

```
knowledge_generation              knowledge_timestamp
max_generator_invocations_check    message_continue_check
no_tool_call_check                 terminal_step_check
```

Con el origen en el código Go original:

| función | fichero:línea |
|---|---|
| `AdminSettings.AdminFileSetsModels` | `third_party/jetski/gateway/admin_settings.go:85` |
| `ParseAdminSettings` | `admin_settings.go:187` |
| `MergeSettings` | `admin_settings.go:262` |
| `mergeModels` | `admin_settings.go:349` |
| `LoadEffectiveSettings` | `admin_settings.go:392` |
| `AuthProvider.fetchRemoteModelCatalog` | `third_party/jetski/gateway/model_discovery.go` |

⇒ **Esto es el mecanismo del catálogo**: el fichero de administración puede
*fijar* las tools y el `gateway` puede *fijar* los modelos
(`AdminFileSetsModels` ⇒ "el fichero de admin pone los modelos").
Es el punto exacto donde MegaDaemon puede tomar el control del catálogo.

## 4. Cómo se ejecuta (pipeline real)

```bash
export JAVA_HOME=/opt/jdk-21.0.13.1+2

# 1. importar (una vez)
md-ghidra import /workspace/agy-re/liblanguage_server.so \
  --project-dir /workspace/agy-re/tools-out/proj --project Go

# 2. aplicar los 161.682 símbolos Go
md-ghidra symbols /workspace/agy-re/tools-out/go.funcs.tsv \
  --project-dir /workspace/agy-re/tools-out/proj --project Go

# 3. decompilar en lotes
md-ghidra decompile /workspace/agy-re/tools-out/anchors_gateway.tsv \
  --out /workspace/agy-re/tools-out/fusion/pseudocodigo

# 4. resolver los strings referenciados por dirección
python3 tools/resolver-strings-ghidra.py \
  /workspace/agy-re/tools-out/fusion/pseudocodigo \
  /workspace/agy-re/tools-out/fusion/legible

# 5. comparación + fusión de todos los métodos
md-re fusion /workspace/agy-re/liblanguage_server.so --json
```

## 5. Escalar al 100 %

* **Mapa**: las 161.682 funciones con su fichero:línea ⇒ `gobook` /
  `LIBRO-COMPLETO.md` (esto ya está hecho, 6 MB).
* **Lógica**: Ghidra decompila ~0,8 s por función ⇒ **el binario entero son
  ~36 h de CPU**; por lotes (500 funciones) son ~7 min por lote. El cuello de
  botella es la JVM, no MegaDaemon: en el móvil (con su propio toolchain de
  Antigravity) se puede hacer en segundo plano mientras duerme.
* **Comparación**: `re_fusion` ejecuta los cinco métodos, mide y fusiona.

## 6. Lo que NO se ha hecho

* `PENDIENTE`: el binario `md-ghidra` no se ha podido **enlazado** en este
  sandbox (el link de la lib de Rust muere por SIGKILL/OOM con 3 GB: el
  linker + Ghidra + la JVM no caben a la vez). Se ha usado el mismo GhidraScript
  embebido que el módulo escribe (`MD_GHIDRA_SCRIPT`), por eso todo lo de
  arriba es exactamente lo que haría `md-ghidra`.
* `PENDIENTE`: hooks Frida/Zygisk (SQLite, red, ficheros).
* `PENDIENTE`: decompilar en el móvil (el toolchain de Antigravity incluye
  un JDK completo).

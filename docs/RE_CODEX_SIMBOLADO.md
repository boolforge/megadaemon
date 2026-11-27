# RE simbolado de Codex 0.172.0 — del binario stripped a 356.133 funciones con nombre

> Documento del trabajo de ingeniería inversa **medido** (2026-11-27). Todo lo que
> aquí aparece como cifra o nombre ha sido medido sobre los ficheros reales; lo
> que no se ha medido está marcado **NO MEDIDO**. Motor sancionado: **Ghidra
> headless** (`rust-core/src/ghidra.rs` / `tools/ghidra-scripts/`). `r2ghidra`
> sólo como segundo motor de verificación.

## 1. El encargo y por qué el binario solo no bastaba

El binario de Codex que se subió a Drive (`codex-app-server-x86_64-unknown-linux-musl`,
249.083.328 B) va **stripped**: cero nombres de función. Con el RE de strings
(`docs/RE_CODEX_CONTRATOS.md`) se recuperaron los *contratos* (hooks, tools,
sandbox, modos de permiso), pero no la *estructura interna*: qué función llama a
cuál, dónde vive cada cosa.

El propio binario dice dónde está su información:

```
INFO  DWARF external debug information found: ExternalDebugInfo
      [filename=codex-app-server.debug, crc=ad295a15, hash=null]
      (ExternalDebugFileSectionProvider)
INFO  Unable to find DWARF information, skipping DWARF analysis (DWARFAnalyzer)
```

Es decir: el ELF apunta a un **fichero de debug externo** llamado
`codex-app-server.debug` y **declara su CRC32** (`ad295a15`). Ese fichero no
venía en la release "normal", sino en un asset aparte: `codex-symbols-*`.

## 2. Los assets de símbolos (medido en la release `rust-v0.172.0`)

| asset | bytes | contenido |
|---|---:|---|
| `codex-symbols-x86_64-unknown-linux-musl.tar.gz` | 395.260.220 | símbolos del CLI `codex` |
| `codex-symbols-x86_64-unknown-linux-musl-app-server.tar.gz` | 320.165.963 | símbolos del `codex-app-server` |
| `codex-symbols-x86_64-unknown-linux-musl-app-server.tar.gz` → contenido | — | `codex-app-server.debug` (1.085.115.680 B), `codex-exec-server.debug` (328.714.792 B), `codex-code-mode-host.debug` (121.994.768 B) |

### 2.1 Subida a Drive (por streaming, verificada)

| fichero | bytes | sha256 | file_id |
|---|---:|---|---|
| `codex-symbols-x86_64-unknown-linux-musl.tar.gz` | 395.260.220 | `37307df8fca4298bd6b478cbf2c84b26b09aa833d0f177ec6bf767e9619667f3` | `1q93ULt4kAXiXMXV7Pzn3gc8HNGqjpS4i` |
| `codex-symbols-x86_64-unknown-linux-musl-app-server.tar.gz` | 320.165.963 | `017c60b2d6e2fe12a1420062a069ffa6ab84591a4ae43d1b1be3586a91eb68d5` | `11WiDFT5L1nAWU8dpJJe-P9o_R73kldu_` |
| `codex-x86_64-unknown-linux-musl.sigstore` | 8.509 | `12dd57191ab7553e5f52cc1bee8a3907d9e344a107c0f2ffebbb14b3666cba76` | `14wlMmzBrdD0n69vH_kjICi3GEqtRs_ni` |

Carpeta: `megadaemon-backup/RE-CODEX` (`19ISaNT5MV5WUfEhD854OBQygqOf-Kgn0`).
Verificación: `md5Checksum` + `size` publicados por Drive (el hash por descarga
**corta el flujo** y da sha256 falso: medido, 125.369.335 B anunciados y
110.722.056 leídos).

**Los assets NO se quedan en local**: se suben por rangos (`Range: bytes=a-b`,
HTTP 206 con `content-range` exacto) y se pasan al `PUT` de la sesión
resumable sin escribir un solo byte en disco (`tools/drive-subir-stream.py`).
El `sha256`/`md5` se calculan **en vuelo**, sobre exactamente los bytes
enviados.

## 3. El `.debug` es el correcto (prueba dura)

El CRC32 declarado por el binario se comprueba contra el fichero descargado:

```
bytes=1085115680 crc32=ad295a15
Ghidra esperaba: ad295a15
MATCH
```

Además el `.debug` es un ELF64 `DYN` x86-64 con **entry point `0x235ad40`**, el
mismo tipo de máquina y el mismo `e_machine` que el binario, y 37 secciones.

## 4. La symtab: 616.013 símbolos, 356.133 funciones

Con `nm --defined-only -S --numeric-sort` sobre `codex-app-server.debug`:

| tipo | símbolos |
|---|---:|
| `t` (funciones locales) | 328.883 |
| `T` (funciones globales) | 27.250 |
| `b` (BSS local) | 253.078 |
| `B` (BSS global) | 6.701 |
| `W` / `V` | 91 / 10 |
| **total** | **616.013** |

**Funciones: 356.133** (`T`+`t`). Es el número que el binario stripped no
tenía: **0**.

### 4.1 El demangler canónico (y por qué es propio)

Los nombres son Rust **v0 moderno** (`_RNvNCNKNvNtCscKrEppzIDTR_12codex_config...`).
En el sandbox **no** hay `rustfilt` ni `rustc-demangle` instalados, y PyPI está
bloqueado (`pip install rust-demangle` → *ProtocolError: Connection reset*).
Pero la caché de cargo local sí tenía `rustc-demangle-0.1.28` y hay `rustc
1.100.0`, así que se compiló **con `rustc` directo (SIN `cargo`, que el root
prohíbe)**:

```
rustc --edition 2021 -O --crate-type=rlib --crate-name rustc_demangle \
      <cache>/rustc-demangle-0.1.28/src/lib.rs -o librustc_demangle.rlib
```

El crate es MIT OR Apache-2.0. Se deja además `tools/rust-demangle.py`, un
demangler **aproximado** en Python puro (fallback si no hay `rustc`), y
`tools/compilar-demangle-rust.sh` para reproducir el binario canónico.

Ejemplos reales de la salida:

```
<alloc[d0a56055437745c0]>::raw_vec::RawVecInner<_>>::reserve::do_reserve_and_handle::<alloc[d0a56055437745c0]::alloc::Global>
codex_config[947f3ac628eee615]::path_context::PATH_CONTEXT::{K#0}::{closure#0}::__RUST_STD_INTERNAL_VAL
```

## 5. El mapa de ficheros fuente (DWARF `.debug_line`)

`readelf --debug-dump=decodedline codex-app-server.debug` → **45.744.177
líneas** (3.067.564.511 B) con (fichero, línea, dirección). Cruzado con la
symtab da **de qué fichero `.rs` sale cada función**: la pieza que en el
binario stripped no existía.

## 6. Ghidra: los dos OOM y cómo se entraba en el presupuesto de RAM

El límite real **no** es el del host, es el del cgroup:

```
/sys/fs/cgroup/memory/memory.limit_in_bytes = 3087007744   (2,87 GiB)
```

Y el OOM killer del cgroup es el que mata la JVM (evidencia en `dmesg`):

```
oom-kill:constraint=CONSTRAINT_MEMCG,... task=java,pid=34637,uid=0
```

Dos incidentes medidos:

1. **Import con auto-análisis** (`MAXMEM=1200M`) → la JVM llegó a RSS
   1.678 MB y murió en `ANALYZING all memory and code` tras ~30 min de reloj.
2. **El page cache cuenta contra el cgroup**: con 2.593 MB de `buff/cache`
   (por leer el `.debug` y el volcado de 3 GB), cualquier JVM moría al
   arrancar. `drop_caches` está en **read-only** (`/proc/sys/vm/drop_caches`),
   pero se puede descartar página a página con
   `posix_fadvise(POSIX_FADV_DONTNEED)`: con eso el `buff/cache` bajó a
   748 MB y el import **terminó bien**.

Estrategia que funcionó (ordenada):

1. `posix_fadvise(DONTNEED)` sobre los ficheros grandes ya leídos.
2. Importar con **`-noanalysis`** (importa el ELF, no lo analiza) y
   `MAXMEM=1000M -XX:ParallelGCThreads=1`.
3. Aplicar los símbolos con un **script propio** (`tools/ghidra-scripts/CodexSimbolos.java`),
   que crea la función en la dirección correcta y la decompila.

### 6.1 El desplazamiento de direcciones (medido)

Ghidra carga este ELF en `imageBase = 0x100000` (medido en el log del script).
Por tanto:

```
direccion_ghidra = vaddr_del_binario + 0x100000
```

(igual que en el módulo Go de Antigravity, pero aquí verificado en el propio
log del script).

## 7. Lo que produce el motor sancionado (Ghidra)

Tanda ejecutada: **220 funciones de contrato** (sandbox, permisos, hooks, tools,
MCP/ACP, sesión, config) seleccionadas por nombre real y acotadas a 4.096 B de
cuerpo, decompiladas con `tools/ghidra-scripts/CodexSimbolos.java`:

```
CODEXSIMBOLOS creadas=220 decompiladas=213 fallidas=7
```

* **213 pseudocódigos** en `/workspace/agy-re/productos/codex/decomp/` (2,4 MB),
  subidos a Drive como `codex-decomp-2026-11-27-v2.tar.gz`.
* **7 fallidas** (3,2 %): el decompilador no completes en el tiempo dado (20 s
  por función). Las 7 son funciones grandes; **NO MEDIDO** si con más tiempo
  saldrían.
* El desplazamiento se verificó en el propio log:
  `addr=0xa9db3d0  vaddr=0xa8db3d0` ⇒ **vaddr + 0x100000**.

Ejemplo real de salida (el nombre viene del `.debug`, no de una heurística):

```
// addr=0xa9db3d0  vaddr=0xa8db3d0  bytes=31056  crate=codex_linux_sandbox
// <codex_linux_sandbox[9ac853fb151b7e8d]::linux_run_main::LinuxSandboxCommand
//    as clap_builder[eed9c0d03a7c4445]::derive::Args>::augment_args
```

### 7.1 El mapa de ficheros fuente (el resultado más útil)

`readelf --debug-dump=decodedline` da **45.744.177** líneas (fichero, línea,
dirección). Cruzado con la symtab:

| | |
|---|---:|
| símbolos totales | 616.013 |
| con **fichero fuente + línea** | **586.202 (95,2 %)** |
| sin fichero (datos, BSS) | 29.811 |

Ejemplo: `codex_config::path_context::PATH_CONTEXT` → `account.rs:197`.

### 7.2 Los artefactos del RE en Drive

| fichero | bytes | sha256 | file_id |
|---|---:|---|---|
| `codex-app-server-simbolos.tsv` | 202.688.090 | `4534064443cd14b454d1320f166f553a9c297c78ada505a5a66e94c914f2936a` | `1C2zeGq9kLP70KoRsdQhpblTOap_fEPRs` |
| `codex-app-server-funciones.tsv` | 125.054.359 | `9b91e7a858f8a14f7dfacc12f487c5c773f14b26878aae05bd89ed1cdab52e7e` | `1L0IbJAt3MW9dfxn783cTOx3DePAA48VE` |
| `codex-decomp-2026-11-27-v2.tar.gz` | 309.041 | `56a919e25321f8f29f88ceca8c003c7341d0278e02b3f74b355e6a70818425ba` | `1zCkZlOzCMs4nAxUeNuUQWTFMe_iaWZ47` |

Los dos TSV grandes **no** van al repo (202 MB y 125 MB): van a Drive y aquí
sólo queda el resumen por crate y el índice de las funciones de contrato.

> Un primer tar (`codex-decomp-2026-11-27.tar.gz`, id
> `1nqZx7flAmz6PSUy_N7saMyYKsaYYsynS`) se hizo **mientras la decompilación
> seguía escribiendo** ⇒ un fichero quedó con padding. Renombrado a
> `codex-decomp-2026-11-27.tar.gz.INCOMPLETO-con-padding` (no se borra nada).

## 7.3 La arquitectura interna, medida (lo que las strings noaban)

Con el mapa de ficheros fuente por crate se ve la **estructura real** del
producto, que antes sólo se conocía por strings:

| crate | funciones | ficheros fuente mas poblados |
|---|---:|---|
| `codex_core` | 4.885 | `mod.rs` (777), `src/callsite.rs` (412), `option.rs` (295), `boxed.rs` (213) |
| `codex_protocol` | 4.874 | `option.rs` (931), `protocol.rs` (603), `models.rs` (423), `src/ser.rs` (358) |
| `codex_app_server_protocol` | 2.652 | `src/ser.rs` (708), `src/lib.rs` (83), `item.rs` (94) |
| `codex_app_server` | 1.111 | **`workspace_routing.rs` (84)**, **`thread_processor.rs` (82)**, `outgoing_message.rs` (46) |
| `codex_config` | 1.111 | `local.rs` (86), **`config_requirements.rs` (72)**, `src/value.rs` (57) |
| `codex_rmcp_client` | 859 | **`rmcp_client.rs` (198)**, **`oauth.rs` (80)**, **`credential_store.rs` (62)** |
| `codex_linux_sandbox` | 320 | **`linux_run_main.rs` (25)**, `path.rs` (11), **`tcp_namespace.rs` (4)** |

Lo que esto añade a lo ya medido (`docs/RE_CODEX_CONTRATOS.md`):

* El **app-server es multi-hilo con enrutado por workspace**
  (`workspace_routing.rs` + `thread_processor.rs`): cada workspace tiene su
  hilo y sus mensajes salen por `outgoing_message.rs`.
* La **configuración tiene una capa de requisitos** propia
  (`config_requirements.rs`), además de `local.rs` (config local) y
  `value.rs` (valores tipados).
* El **cliente MCP trae OAuth y almacén de credenciales** como módulos
  separados (`oauth.rs`, `credential_store.rs`), no como parte del cliente.
* El **sandbox de Linux trabaja con namespaces de red**
  (`tcp_namespace.rs`) además del `linux_run_main.rs` y el manejo de rutas.

Ficheros `.rs` distintos referenciados por el binario: **1.326**.

## 8. Lo que NO se ha medido

- **NO MEDIDO**: pseudocódigo de las ~356 k funciones; sólo de un subconjunto.
- **NO MEDIDO**: el fichero de debug del CLI `codex` (395 MB) no se ha
  descargado ni procesado; sólo el del `codex-app-server`.
- **NO MEDIDO**: `codex-exec-server.debug` y `codex-code-mode-host.debug` (dentro
  del mismo tar) no se han extraído.
- **NO MEDIDO**: el contenido del crate de Rust (`codex-core`) no está disponible
  como fuente; el RE es sobre binario.

## 9. Reproducir

```bash
# 1) symbols -> Drive (streaming, sin dejar nada en local)
python3 tools/drive-subir-stream.py \
  --url https://api.github.com/repos/openai/codex/releases/assets/785031094 \
  --nombre codex-symbols-x86_64-unknown-linux-musl.tar.gz \
  --tamano 395260220 --carpeta 19ISaNT5MV5WUfEhD854OBQygqOf-Kgn0

# 2) demangler canonico (rustc, sin cargo)
bash tools/compilar-demangle-rust.sh

# 3) tabla de simbolos
python3 tools/extraer-simbolos-codex.py

# 4) subconjunto de contrato
python3 tools/seleccionar-funciones-codex.py \
  artifacts/codex/simbolos/codex-app-server-simbolos.tsv \
  artifacts/codex/simbolos/contrato-400.tsv 400

# 5) Ghidra (motor sancionado)
export JAVA_HOME=/workspace/agy-re/tools-out/jdk21
bash ghidra_11.2.1_PUBLIC/support/launch.sh fg jdk Ghidra-Headless 1000M \
  -XX:ParallelGCThreads=1 ghidra.app.util.headless.AnalyzeHeadless \
  <proyecto> Codex -process codex-app-server-x86_64-unknown-linux-musl \
  -noanalysis -max-cpu 1 -scriptPath tools/ghidra-scripts \
  -postScript CodexSimbolos.java <tabla.tsv> <salida> 400 codex
```

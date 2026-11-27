# Métodos y herramientas de RE: lo que hay, lo que se ha medido y lo que falta

> Regla del proyecto: **MegaDaemon siempre**. El motor propio es
> `rust-core/src/ghidra.rs` (script Ghidra embebido), `rust-core/src/re_*.rs`,
> `rust-core/src/frida_dbi.rs` y `go-retool/`. Lo externo se usa **para
> verificar**, nunca para sustituir.
> Todo lo de aquí está **medido** en la sesión del 2026-11-27; lo que no se ha
> ejecutado dice **NO MEDIDO**.

---

## 1. El problema de fondo

`liblanguage_server.so` (181.358.800 B, ELF64 aarch64 PIE, Go 1.28-RC,
stripped) tiene **161.682 funciones** en su `pclntab`. Al 2026-11-27 el corpus
tenía **59.151** decompiladas por Ghidra (96,90 % de las 61.896 "reales"; las
99.786 "generadas/autogeneradas" nunca se tocaron). El objetivo es **100 % y más
allá**: ninguna función sin pseudocódigo, más tipos Go, más strings, más el
módulo que compile.

### 1.1 Dato que costó sangre: el image base de Ghidra

Ghidra carga este ELF con **image base 0x100000**, así que sus direcciones son
`vaddr + 0x100000`. **Medido en 12/12 muestras** (comparando el `_index.tsv` de
Ghidra con `go.funcs.tsv`):

| función | Ghidra | pclntab | delta |
|---|---|---|---|
| `google3_third_party_jetski_language_server_intercept` | 0x61cf2e8 | 0x60cf2e8 | 0x100000 |
| `crypto_internal_fips140_aes_gcm.newGCM` | 0x727afa8 | 0x717afa8 | 0x100000 |
| `net_http.__response_.finishRequest` | 0x535f478 | 0x525f478 | 0x100000 |

⇒ Sin corregir esto, el "saltar lo ya hecho" no casa y se re-decompila todo.

---

## 2. Motor propio de MegaDaemon (el sancionado)

### 2.1 `rust-core/src/ghidra.rs` (1.407 líneas)

Contiene `MD_GHIDRA_SCRIPT` (un GhidraScript Java completo, clase `MdGhidra`)
y tres modos:

| modo | args | qué hace |
|---|---|---|
| `symbols` | `<symbols.tsv>` | aplica los nombres Go + comentario `fichero:línea` a cada símbolo |
| `decompile` | `<anchors.tsv> <outdir> [filtro]` | desensambla, crea la función, la renombra, decompila y escribe el `.c` + `_decompile_report.tsv` |
| `info` | `<outdir>` | vuelca información del programa |

* El script se extrae con `tools/extraer-ghidra-script.sh` ⇒
  `tools/ghidra-scripts/MdGhidra.java` (13.951 B). **Fuente única**: el `.java`
  nunca se desincroniza del que usaría `md-ghidra`.
* **Formato de anchors** (el que consume `decompile`):
  `addr \t name \t orig \t loc \t tag`
  (`addr` = offset del ELF en hex; `orig` = nombre Go original; `loc` =
  `fichero:línea`; `tag` = etiqueta/paquete que decide el subdirectorio).
* El informe `_decompile_report.tsv` se escribe **con flush por fila** ⇒ el
  barrido es **reanudable** quitando del anchors lo ya reportado.

### 2.2 El resto del motor (ya en el repo)

| módulo | qué aporta |
|---|---|
| `rust-core/src/re_engine.rs`, `re_ir.rs`, `re_strings.rs`, `re_fusion.rs` | cadenas cifradas, IR, **fusión** Ghidra + origen Go + strings |
| `rust-core/src/re_cff.rs`, `re_crypto.rs`, `re_opaque.rs`, `re_obf.rs`, `re_strenc.rs` | control-flow flattening, cripto, predicados opacos, ofuscación, strings cifrados |
| `rust-core/src/frida_dbi.rs` | **Rizin 0.9.1** nativo: formato, secciones, funciones, imports, strings con dirección |
| `rust-core/src/binwalk_die.rs` | DiE + Binwalk: familia de formato, OS, compilador, mitigaciones, entropía |
| `rust-core/src/gobook.rs`, `go-retool/` | parser conservador del `pclntab` moderno (Go 1.18/1.20) + importador de símbolos |
| `rust-core/src/code_ast.rs`, `daad*.rs`, `apk_deobfuscator.rs` | AST, DAAD, ofuscación de APK |
| `tools/resolver-strings-ghidra.py` | resuelve strings de Ghidra contra el mapa de Rizin |
| `tools/reconstruir.sh`, `tools/word-fusion-lint/` | pipeline y lint de fusiones |

### 2.3 El binario `megadaemon-core` (artefacto del CI) tiene RE dentro

MEDIDO (ejecutando el artefacto con la glibc 2.39 de `/workspace/ubuntu-glibc`):

```
rizin <file> [mode] [arch]     Rizin 0.9.1 nativo
rea <plat> [query] [max]       indexer de corpus C de Ghidra + transpilador C++ a ScummVM
cfg <plat>                     generador de CFG
bindiff <plat>                 emparejado de funciones entre plataformas
disasm <plat> <file>           desensamblador multi-arquitectura
die-binwalk <file>             DiE + Binwalk + Scalpel + Bulk Extractor
kaitai <file> [template]       parser declarativo de binarios
mba <expr>                     simplificador de MBA (D810/Goomba)
cirx-ssa [mas]                 SSA de micro-RTL + export SMT-LIB2
deobfuscate <plat> <file>      descifrador Konami XOR
entropy <file>                 clasificador de entropía por bloque
pattern <file> <patrón>        escáner AOB
pointers <file> <base>         minador de tablas de punteros
diff <a> <b>                   motor de diff de snapshots
```

⇒ **No hay que salir de MegaDaemon para analizar**: el binario ya trae Rizin,
DiE/Binwalk, Kaitai, CFG, diff y MBA.

---

## 3. Entorno medido hoy (2026-11-27)

| pieza | dónde | nota |
|---|---|---|
| Ghidra 11.2.1 | `/workspace/agy-re/tools-out/ghidra_11.2.1_PUBLIC` | **ya instalada** |
| Ghidra 12.2 | `/workspace/agy-re/tools-out/ghidra_12.2_PUBLIC` | ya descargada |
| Temurin JRE 21.0.13.1+2 | `/workspace/agy-re/tools-out/jdk-21.0.13.1+2-jre` | Ghidra 11.2+ exige Java 21 |
| redress (Avast) | `/workspace/agy-re/tools-out/bin/redress` | `packages`/`types`/`source`/`moduledata` |
| rizin 0.9.1 | `/workspace/agy-re/tools-out/bin/` | estático, con `rz-gg` |
| GoReSym parcheado | `/workspace/agy-re/tools-out/gore-patched/` | recuperación de símbolos Go |
| goreprobe | `/workspace/agy-re/tools-out/goreprobe/goreprobe{,_patched}` | sonda de RE |
| GoStringUngarbler | `/workspace/agy-re/tools-out/gostringungarbler/` | (Google) resuelve strings de binarios Go con `garble -literals`; necesita `unicorn`+`capstone`+`pefile` |
| RetDec | `/workspace/agy-re/tools-out/retdec.tar.xz` | descargado, **NO MEDIDO** en este binario |
| proyectos Ghidra de los workers | `tools-out/proj-w0`, `proj-w1`, `proj-w2` | los 3 workers |

### 3.1 El import de Ghidra que antes moría por OOM

El import anterior murió con `Killed` (OOM) a los 8 m 41 s. **Clave medida**:
`MAXMEM=1600M` + `-max-cpu 1` ⇒ import **OK** en 85 s, **pico de RSS 1.636 MB**,
proyecto de **249 MB** (`tools-out/gh2/proj`).
Script reproducible: `tools-out/decomp100/import_gh.sh`.

---

## 4. Segundo motor (verificación, no sustitución): r2ghidra 6.2.6

**Por qué**: decompilar 102.531 funciones con la JVM son ~22 h a 0,8 s/función;
r2ghidra es **el mismo decompilador de Ghidra en C++**, sin JVM.

Montaje medido hoy:

```bash
# radare2 6.2.6 (pareja exacta de r2ghidra 6.2.6, mismo día de release)
dpkg-deb -x radare2_6.2.6_amd64.deb /workspace/re-tools/r2/
# plugin + sleigh (287 ficheros)
unzip r2ghidra-6.2.6-linux-x86-64.zip -d /workspace/.home/.local/share/radare2/
export SLEIGHHOME=/workspace/.home/.local/share/radare2/plugins/r2ghidra_sleigh
# wrapper (glibc 2.39 + libstdc++ 3.4.33 de Ubuntu 24.04)
/workspace/re-tools/r2.sh -q -N -c "s <addr>; af; pdg" binario
```

**Fricciones medidas** (las que costaron sangre; no las descubras de nuevo):

1. `core_ghidra.so` exige **GLIBC_2.38** y **GLIBCXX_3.4.32**; el sandbox tiene
   2.36/3.4.30 ⇒ hay que correr bajo la glibc 2.39 de `/workspace/ubuntu-glibc`
   con `libstdc++6` de Ubuntu 24.04 (`/workspace/re-tools/glibc39`).
2. El plugin busca su directorio de sleigh en una **ruta hardcodeada del build**
   (`/home/runner/.local/share/radare2/plugins/r2ghidra_sleigh`) ⇒ hay que
   crearla o exportar `SLEIGHHOME`.
3. El error "You need to install the plugin with r2pm -ci r2ghidra" **no** es de
   rizin: es del propio plugin cuando no encuentra el `sleighhome`.
4. Receta que funciona (fronteras exactas desde el pclntab, no adivinadas):
   `af+ <addr> <nombre>; afb+ <addr> <addr> <tam>; pdg @ <addr> > fichero.c`
   - El nombre debe ser **único por función** (si no, r2 dice "dupped function").
   - `afb+ <fcn_at> <bbat> <bbsz>`: el orden importa; con el tamaño equivocado
     r2 avisa "Unaligned function".

### 4.1 Comparativa medida (200 funciones, mismas, binario Go de Antigravity)

Herramienta: `tools/regenerar-go/comparar-decompiladores.py` → datos en
`docs/medidas/comparativa_decompiladores.json`.

| métrica | valor |
|---|---|
| r2ghidra, funciones/s (1 worker, sesión única) | **5,64** |
| Ghidra 11.2.1, s/función (medido por los workers en su índice) | **~0,8–0,85** |
| Ghidra, funciones/s (1 worker, sin arranque de JVM) | **~1,2** |

⇒ r2ghidra es **~4,7× más rápido** que la JVM de Ghidra con el mismo
decompilador. **NO MEDIDO** todavía: el porcentaje de coincidencia literal
entre ambos pseudocódigos (el comparador está escrito, la primera corrida
completa quedó pendiente de corregir el emparejamiento por el desfase
0x100000 ya documentado).

---

## 5. Qué falta para el 100 % y "más allá"

| # | pieza | estado | cómo |
|---|---|---|---|
| 1 | pseudocódigo de las **102.531** funciones restantes | herramientas listas, barrido en curso | `decomp-r2ghidra.py` (r2ghidra, rápido) + `md_ghidra.sh` (motor propio, lento pero sancionado) |
| 2 | **tipos Go** reales (structs, interfaces, métodos) en los stubs | `redress` disponible | `redress types`/`source` sobre el `.so`; con eso los stubs dejan de ser `undefined8` |
| 3 | **strings** con dirección y valor | `redress` + `re_strings.rs` + Rizin | `tools/resolver-strings-ghidra.py` |
| 4 | **módulo Go que compile** | bloqueado por `import "unsafe"` no usado | corregir `tools/regenerar-go/regenerar.py` y luego `go build ./...` con `/workspace/md-resc/go/go/bin/go` |
| 5 | **cobertura medida** (no estimada) | parcial | contar `_decompile_report.tsv` + `_index.tsv` |

---

## 6. Decompiladores y herramientas de RE alternativas (búsqueda por API, 2026-11-27)

Búsqueda con la API de GitHub (sin `web_search`): 107 repos únicos medidos en
`docs/medidas/busqueda_decompiladores.json`.

### 6.1 Alternativas al decompilador (las que de verdad importan)

| proyecto | ★ | lenguaje | qué es | para MegaDaemon |
|---|---:|---|---|---|
| **angr/angr-ghidra-core** | 0 (API) | Python | *decompila dentro de Ghidra usando el decompilador de **angr*** | **el candidato más serio a tercer motor**: mismo script, otro motor, contrastable |
| **sefcom/oxidizer** | 554 | Python | decompilador en Rust sobre angr | decompilador propio en Rust ⇒ cabe en `rust-core/` |
| `mandiant/GoReSym` | 1.142 | Go | recuperación de símbolos Go | ya hay una copia parcheada en el debris |
| `cyberqueenmeg/GoReSym-GhidraExtension` | 0 | Python | importa JSON de GoReSym en Ghidra | puente GoReSym→Ghidra |
| `doomedraven/rz-goresym` | 0 | Python | puente **GoReSym→Rizin/rz-ghidra** | encaja con `frida_dbi.rs` (Rizin) |
| `HanEmile/radare2-GoReSym` | 3 | Python | script de radare2 | lo mismo, con el Rizin de MegaDaemon |
| `luftwaffe66/GoBfuscator` | 13 | Python | ofuscador de Go | **banco de pruebas** para el parser de pclntab |
| `sysopfb/GoMang` | 13 | Python | manipula el pclntab | banco de pruebas |
| `idaifish/gostrip` / `ForgeLLC/Peel-Go` | 5 / 0 | Go / JS | borran el pclntab / pelan `garble` | banco de pruebas |
| `dipusone/golang_pclntab_parser`, `vanstee/pclntab` | 16 / 1 | Python / Go | parsers de pclntab | contraste con `go-retool` |
| `TheAutomatic/antigravity-context-autofix` | 44 | Python | localizador/parcheador de pclntab **multi-versión** | relevante: el binario es Go 1.28-RC |
| RetDec | — | C++ | ya en `tools-out/retdec.tar.xz` | **NO MEDIDO** en este binario; soporte de Go flojo |

### 6.2 Agentes de RE (patrones que se pueden copiar a MegaDaemon)

| proyecto | ★ | qué aporta |
|---|---:|---|
| **SumTuusDeus/ghidra-mcp** | 1 | **servidor MCP que expone Ghidra a agentes LLM** ⇒ MegaDaemon ya tiene MCP + Ghidra: es el pegamento que falta |
| **shells-above/ida-swarm** | 41 | enjambre de agentes de RE sobre IDA |
| **criticic/reagent** | 14 | agente autónomo de análisis de binarios, multi-agente |
| **9elements/autorev** | 38 | framework de RE automática de firmware (**en Go**) |
| **seekehr/Reversio** | 1 | RE con IA, escrito en Go |
| **BruceWen1/LLM4Reverse** | 6 | toolkit de RE guiada por LLM |
| **voidvenom/oh-my-rea** | 4 | agente de RE "evidence-first" en TypeScript |
| **ConnorDev1377/binary-capability-eval** | 0 | **benchmark** de agentes LLM en RE autónoma ⇒ methodology para medir a nuestros agentes |
| **LakoMoor/portcode** | 9 | *portar a Rust con prueba* ⇒ el objetivo de MegaDaemon, como agente |
| `nimamasl114514/idamcp` | 1 | MCP de IDA escrito en Go |

---

## 7. Reglas que se/learned (medidas)

1. **Ghidra con `MAXMEM` y `-max-cpu 1`**: sin eso, OOM.
2. **Desfase 0x100000** entre direcciones de Ghidra y del pclntab.
3. **Nada de `cargo` en local** (regla del jefe): 2 CPU y 3 GB sin swap ⇒ el
   enlazado muere. Se valida con el CI.
4. **Go**: `/workspace/md-resc/go/go/bin/go` (go1.27.4) ⇒ `go build`/`go vet` sí
   se pueden hacer localmente.
5. **Todo en castellano y con medidas**: lo no medido, **NO MEDIDO**.

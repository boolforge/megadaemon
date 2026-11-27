# Inventario: Drive privado, worktrees y corpus de RE

> Todo lo de aquí está **medido el 2026-11-27** con la API de Drive v3
> (`tools/drive-listar.py`) y con `git worktree list`. Lo que no se ha abierto
> dice **NO REVISADO**.

## 0. Carpeta privada de Drive

* Carpeta: `megadaemon-backup`, id `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`.
* Credencial: `/workspace/md-secrets/gdrive_credentials.json` (o
  `/workspace/.home/.gdrive_credentials.json`).
* Listado reproducible: `python3 tools/drive-listar.py artifacts/drive/INVENTARIO.tsv`.

**49 entradas en la raíz**, de ellas 17 subcarpetas y 32 ficheros sueltos.

| carpeta | entradas | MB | qué hay |
|---|---:|---:|---|
| `RE-GO-LEGIBLE` | 11 | 160,6 | el material "100 % legible" del binario Go (ver §2) |
| `ANTIGRAVITY_CONSOLIDATED_2026-11-26` | 340 | — | los `.so` del móvil + informes + `cli_android_arm64_funcs_rel.tsv` (37 MB) |
| `ANTIGRAVITY_FULL_2026-11-26` | 295 | 229,2 | el build Rust completo (`.o`, `.rustc_info.json`) + `FACTS.md` + `COVERAGE.tsv` + informes de los agentes |
| `ANTIGRAVITY_RESILIENTE_TEST` | 10 | 0,1 | `agy_auth.py`, `agy_deliver.py`, `run.py`, suite de tests |
| `LS_AUTH_ANALYSIS_2026-11-26` | — | — | análisis de autenticación del Language Server |
| `agentes-fuente-codigo` | 2 | 355,6 | **fuente** de `aaif-goose/goose` (317 MB) y `vercel-labs/agent-desktop` (38,5 MB) |
| `Drive_MegaDaemon_2026-11-24` | 6 | 10,3 | bundle con la clave filtrada + `cc-deobfuscated-2.1.331.tar.gz` + docs de RE de Claude Code |
| `tablon-resiliencia` | 133 | 6,2 | las cartas de coordinación entre agentes |
| `actualizaciones` | 3 | 82,2 | binarios `megadaemon-core-android-arm64` |
| `P0-apps` | 40 | 609,3 | contenedores y segmentos de las apps del móvil |
| `P1-perfil`, `P2-sdcard`, `P3-raw` | 0 | 0 | vacías (pendiente de subir) |

Ficheros sueltos en la raíz (los que **no** son de RE):

| fichero | bytes |
|---|---:|
| `codigo-go-regenerado.tar.gz` | 44.091.932 |
| `codigo-go-regenerado-NO-DECOMPIADAS.tsv` | 20.022.216 |
| `codigo-go-regenerado-RESUMEN.md` | 8.071 |
| `cursor-3.32.4-*` (10 contratos + verbatim + URLs) | 13.418 … 65.839 |
| `devin_contratos_2026-11-27.zip` | 11.869.751 |
| `RE_DEVIN_DESKTOP_CONTRATOS_2026-11-27.md` | 39.186 |
| `kernel-backup-20261122-*.img` | 65.536 … 67.108.864 |

### 0.1 Lo que ya está subido y **no** estaba en el repo

* Los contratos de **Cursor 3.32.4** (10 documentos + verbatim) y de
  **Devin Desktop 3.15.14** (zip + informe): los produjeron los workers de
  producto y los subieron ellos mismos.
* `codigo-go-regenerado.tar.gz`: el árbol Go regenerado (5.383 ficheros) + el
  `.c` de Ghidra.

## 1. `RE-GO-LEGIBLE` — el material "100 % legible"

| fichero | bytes | qué es |
|---|---:|---|
| `LIBRO-COMPLETO.md` | 6.080.177 | **las 161.682 funciones** con dirección, símbolo Go y `fichero:línea` original, agrupadas en 5.383 ficheros Go, en sintaxis Go |
| `LIBRO-FUSIONADO.md` | 8.672.662 (local) / 6.274.824 (Drive) | el libro + strings resueltos + deofuscación |
| `libro-completo.json` | 45.594.801 | lo mismo en JSON (para máquinas) |
| `GO-REGENERADO.zip` | 32.903.876 | el árbol Go regenerado |
| `GO-CODIGO-LEGIBLE.zip` | 3.629.414 | el `.c` de Ghidra + los stubs |
| `megadaemon-reconstruccion.zip` | 16.857.437 | el paquete de reconstrucción |
| `go.funcs.tsv` | 46.719.667 | el inventario del `pclntab` |
| `STRINGS-resueltas.md` | 132.429 | las cadenas con su valor |
| `ANTIGRAVITY_BINARIO_LEGIBLE.md`, `DOMINIO_ANTIGRAVITY.md`, `RECONSTRUCCION_MULTI_METODO.md` | 11.633 / 5.658 / 8.007 | los informes |

⇒ **El esqueleto completo del binario ya es legible al 100 %** (161.682/161.682
con su origen Go). Lo que faltaba era el *cuerpo* de las funciones.

## 2. Lo que faltaba y hoy se ha hecho (2026-11-27)

| pieza | antes | ahora |
|---|---|---|
| funciones de las que hay **pseudocódigo C** | 59.151 (Ghidra) | barrido en marcha: +~1.700 más y subiendo (r2ghidra, 2,4 fns/s) |
| motores de decompilación | 1 (Ghidra) | 2, y los dos **dentro de MegaDaemon** (`re_decompilers.rs`) |
| comparación entre motores | no existía | `comparar-decompiladores.py` + `md-re decompile --backend both` |
| catálogo de agentes de código | 59 | 59 + **65** más medidos (65/84 medidos con datos) |

## 3. El "corpus de conocimiento" (`ANTIGRAVITY_FULL_2026-11-26`)

| fichero | bytes | qué es |
|---|---:|---|
| `FACTS.md` | 221.980 | índice temático de las 7.358 mensajes del corpus, con citas `sesión · índice de mensaje` para poder verificar cada afirmación |
| `COVERAGE.tsv` | 11.583 | cobertura por sesión (cuántas páginas/mensajes, si se completó) |
| `CURSORS.tsv`, `KEYWORDS.tsv`, `PAGINATE_PLAN.tsv`, `PENDING.tsv`, `REMAINING_IDS.txt` | — | plan de paginación del corpus |
| `MESSAGES.jsonl` | — | los mensajes en bruto |
| `General__*.md` | 46.437 … 216.532 | los informes de cada agente (Asimilar Antigravity, Agente dedicado CI, Adversarial narrativo, Antigravity instrumentado…) |
| `ANTIGRAVITY_TASK_REGISTRY.tsv`, `ANTIGRAVITY_LINEAGE.md`, `ANTIGRAVITY_HARNESS.md`, `ANTIGRAVITY_TEST_SUITE.md`, `ANTIGRAVITY_PRODUCTS.md`, `ANTIGRAVITY_VERSION.md`, `ANTIGRAVITY_DOCTRINA_Y_CONTINUIDAD.md` | — | la doctrina y el registro de tareas del RE |

**Tema más denso del corpus**: `deofuscacion-y-reversing` ⇒ **1.906 mensajes
en 83 sesiones** (medido en `FACTS.md`).

## 4. Worktrees

**41 worktrees** (`git worktree list`). Los que guardan material de RE:

| worktree | rama | qué aporta |
|---|---|---|
| `/workspace/md-gore` | `agent/go-re-completo` | el motor de Go de MegaDaemon: parser de `pclntab` para Go 1.28, export de símbolos (TSV+JSON+Rizin) y de anclas, y **el `moduledata`** (la base de texto se lee del `moduledata`, no de las secciones) |
| `/workspace/md-regen` | `rama/regenerar-go-2026-11-23` | el generador del árbol Go (5.383 stubs) + `RESUMEN.md` |
| `/workspace/md-inject` | `rama/re-masivo-2026-11-26` | el **RE masivo por app** (puntaje + presupuesto + digest) + la tool MCP del RE masivo |
| `/workspace/md-principal` | `agent/antigravity-instrumentado` | la instrumentación de Antigravity |
| `/workspace/md-asim` | `agent/antigravity-asimilacion` | la asimilación (el tema `antigravity-asimilacion-movil`) |
| `/workspace/md-re-productos` | `re/productos-cierre-2026-11-27` | **este encargo**: contratos de Cursor/Devin, catálogo de agentes, registro de decompiladores |

## 5. Material del "debris" (`/workspace/agy-re/tools-out/`)

| pieza | para qué |
|---|---|
| `ghidra_11.2.1_PUBLIC`, `ghidra_12.2_PUBLIC` | los dos Ghidra |
| `jdk-21.0.13.1+2-jre`, `jdk21` | el Java que Ghidra exige (≥ 21) |
| `bin/redress` | **tipos y strings de binarios Go** (Avast) |
| `bin/rizin` + `rz-gg` | Rizin 0.9.1 estático con el plugin de Ghidra |
| `gore-patched/`, `goreprobe/` | GoReSym parcheado y sonda de RE |
| `gostringungarbler/` | (Google) resuelve strings de Go ofuscado con `garble` |
| `retdec.tar.xz` | RetDec (descargado; **NO MEDIDO** en este binario) |
| `go/`, `gopath/` | el árbol de fuentes de Go |
| `proj-w0`, `proj-w1`, `proj-w2` | los proyectos Ghidra de los 3 workers |
| `go.funcs.tsv` (161.682), `go.funcs.real.tsv` (61.896) | el `pclntab` completo y su parte "real" |
| `anchors*.tsv`, `anchors_gateway.tsv` | las anclas que alimentan al decompilador |

## 6. Lo que **no** he revisado todavía (NO REVISADO)

* `tablon-resiliencia`: 133 cartas de coordinación (no están indexadas en el `FACTS.md`).
* `ANTIGRAVITY_CONSOLIDATED_2026-11-26`: los 340 `.so` del móvil y
  `cli_android_arm64_funcs_rel.tsv` (37 MB) — **relaciones de funciones del CLI
  para Android ARM64**: es el gemelo del `liblanguage_server.so` que corría en el
  móvil.
* Los 295 `.o` del build Rust de `ANTIGRAVITY_FULL_2026-11-26`.
* `P0-apps` (609 MB): los contenedores de las apps del móvil.
* El `MESSAGES.jsonl` completo (7.358 mensajes) del corpus.

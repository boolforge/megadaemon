# Antigravity — workspace único

> **Espacio canónico** de todo el trabajo relacionado con **Antigravity**, **Gemini**,
> **Go** y el **Language Server**. Reúne documentación, artefactos de verificación,
> catálogos por versión/plataforma, el banco de pruebas en ejecución (emulación +
> instrumentación + ejecución simbólica), el oráculo en Go, la reimplementación en
> Rust, las herramientas y los logs.
>
> Creado: **2026-11-26**. Norma de mantenimiento: [`docs/WORKSPACE_NORM.md`](docs/WORKSPACE_NORM.md).

---

## 1. Mapa del workspace

| Ruta | Qué hay | Tamaño |
|---|---|---|
| [`docs/`](docs/) | Informes y documentación (**fuente de verdad**) | ~200 KB |
| [`recon/`](recon/) | Artefactos de verificación (TSV/CSV con VA reales) | ~70 MB |
| [`versions/`](versions/) | Catálogo por versión y plataforma; payloads extraídos | ~5 MB |
| [`runtime/qemu/`](runtime/qemu/) | Capa de emulación ARM64 (sysroot glibc) | ~30 MB |
| [`runtime/shims/`](runtime/shims/) | Shims de ocultación e instrumentación (C → ARM64) | ~100 KB |
| [`runtime/harness/`](runtime/harness/) | Sonda de entorno y arnés de ejecución | ~1 MB |
| [`runtime/symbolic/`](runtime/symbolic/) | Ejecutor simbólico acotado (aarch64 + z3) | en curso |
| [`reference/go/`](reference/go/lsauth/) | Oráculo en Go de la semántica de auth | ~50 KB |
| [`reference/rust/`](reference/rust/) | Reimplementación en Rust (`ls_auth`) | ~60 KB |
| [`reference/ls_auth_contract.json`](reference/ls_auth_contract.json) | Contrato verificable (12 campos, 7 itabs, 7 proveedores) | 10 KB |
| [`tools/`](tools/) | Todo el utillaje (Python/Shell/C) | ~100 KB |
| [`repo/`](repo/) | Repos de referencia y reconstrucciones | inventarios |
| [`patches/`](patches/) | Parches aplicados y diffs | en curso |
| [`logs/`](logs/) | Trazas y logs de ejecución | en curso |
| [`tests/`](tests/) | Resultados de pruebas y sus fixtures | en curso |

### Workspaces externos (referenciados, **no** movidos)

| Ruta | Qué es | Por qué no se mueve |
|---|---|---|
| `/workspace/agy-re/` | **Binario fuente** `liblanguage_server.so` + corpus Ghidra + `go-retool` (9,2 GB) | Read-only por norma; 54 781 ficheros, moverlo rompería otras sesiones |
| `/workspace/megadaemon/` | **Repo de código** Rust de MegaDaemon (compartido, multi-agente) | Otros agentes commitean ahí; los cambios se hacen con rutas exactas en la rama `agent/automacion-software` |
| `/workspace/agy-upstream/` | Repo upstream `antigravity-android` v2.4.1 (407 MB) | Referencia de versionado compartida |
| `/workspace/agy-intel/` | Inteligencia del cliente Android `com.agy` | Contexto de despliegue/parcheo |
| `/workspace/md-secrets/` | Credencial de Drive (0600) | Secretos fuera del árbol de trabajo |
| `/workspace/scratch/` | Zona de trabajo temporal (linkeada a `recon/` y `tools/`) | Efímero por definición |

Enlaces de compatibilidad creados: `/workspace/out` → `antigravity/docs`,
`/workspace/scratch/recon` → `antigravity/recon`, `/workspace/scratch/tools` → `antigravity/tools`.

---

## 2. Los documentos que mandan

| Documento | Contenido | Estado |
|---|---|---|
| [`docs/LS_AUTH_ANALYSIS.md`](docs/LS_AUTH_ANALYSIS.md) | Informe principal de la lógica de autenticación (**v5**, 59 secciones) | cerrado N1–N4 |
| [`docs/ANTIGRAVITY_BINARIES.md`](docs/ANTIGRAVITY_BINARIES.md) | Catálogo de **todos** los binarios: 48 artefactos, 14 LS distintos, 6 productos | vigente |
| [`docs/VERIFICATION_LEDGER.md`](docs/VERIFICATION_LEDGER.md) | Libro mayor: cada afirmación ⇒ método ⇒ evidencia ⇒ artefacto ⇒ código | v5 |
| [`docs/BACKLOG.md`](docs/BACKLOG.md) | Pendientes con estado y siguiente paso accionable | vivo |
| [`docs/WORKSPACE_NORM.md`](docs/WORKSPACE_NORM.md) | Norma de trabajo, backup y coordinación | nuevo |
| [`docs/ANTIGRAVITY_LINEAGE.md`](docs/ANTIGRAVITY_LINEAGE.md) | Genealogía: Codeium → Windsurf → Google → Antigravity; **Jetski nunca fue nombre de producto** | cerrado |
| [`docs/ECOSYSTEM_PROJECTS.md`](docs/ECOSYSTEM_PROJECTS.md) | Catálogo de 16+ proyectos del ecosistema, protos, barrido de canales de descarga | v2 |
| [`docs/ANTIGRAVITY_HARNESS.md`](docs/ANTIGRAVITY_HARNESS.md) | Arnés de captura local del LS real: requisitos, aislamiento, esquema SQLite medido | nuevo |
| [`docs/corpus/README.md`](docs/corpus/README.md) | Corpus de conversaciones de todos los agentes: método, cobertura y **limitaciones medidas** | vivo |

---

## 3. Estado de los cuatro puntos «NO VERIFICADO»

| Punto | Estado | Evidencia |
|---|---|---|
| N1 — slot real de `GetEndpointURL` | **CERRADO** | 7 *itabs* de proveedor reconstruidos ⇒ **slot 2** (offset `0x28`), no 5 |
| N2 — `ui_message` en `project_required`/`license_required` | **CERRADO** | Esas ramas **no** escriben el offset `0x10` |
| N3 — ¿existe un LS más reciente? | **CERRADO** | CLI oficial 1.5.18 (9 plataformas, `sha512` verificable) + **LS de macOS** 205 MB en el Hub |
| N4 — clippy | **CERRADO** | `clippy -D warnings` sin avisos |

Además, en este turno se ha cerrado **la identidad del binario analizado** (es el LS
parcheado de v2.4.1, `sha256` idéntico) y **la semántica del parche «region-bypass»**
(15 bytes en 3 puntos ⇒ fuerza `AuthResult.HasValidAuth = true`).

Pendientes reales: ver [`docs/BACKLOG.md`](docs/BACKLOG.md) §P1–§P5.

---

## 4. Banco de pruebas en ejecución

```
/workspace/antigravity/runtime/
├── qemu/sysroot/          glibc arm64 mínimo (libc6 2.36-9+deb12u14 arm64)
├── shims/qemu_shield.c    ocultación + instrumentación EN LA FRONTERA (host)
├── shims/antiqemu.c       ocultación + instrumentación dentro del guest
├── harness/probe_env.c    sonda: ¿qué ve el guest del host?
└── harness/run_ls.sh      arnés de ejecución del LS
```

**Por qué el shim va en el proceso de qemu y no en el guest**: el LS es un binario
**Go**; hace syscalls crudos y lee `/proc/self/auxv` y `/proc/cpuinfo` directamente,
así que un `LD_PRELOAD` dentro del guest no lo alcanza. Pero `qemu-user` traduce
**todas** las llamadas del guest a llamadas del host ⇒ interceptando en el proceso de
qemu se ocultan **y** se instrumentan a la vez, y además el `AT_HWCAP` falso se
propaga solo al auxv que qemu construye para el guest.

Resultados medidos (no supuestos):

| Vector de detección | Sin shim | Con shim |
|---|---|---|
| `uname().release` | `5.10.134-…lifsea8.x86_64` | `6.6.0-android14-7` |
| `uname().machine` | `aarch64` | `aarch64` |
| `getauxval(AT_HWCAP)` | `0xecfffffb` (bits x86) | `0x0` (HWCAP aarch64) |
| `/proc/cpuinfo` | `vendor_id: GenuineIntel` | pendiente: requiere el shim de frontera |
| `getenv("LD_PRELOAD")` | visible | oculto |
| `getenv("QEMU_*")` | visible | oculto |

---

## 5. Cómo reproducir

```bash
# catálogo de binarios (CRC-32 del .so de cada APK sin descargarlo)
python3 tools/j0_catalog.py 2>/dev/null || python3 /workspace/scratch/apk_ls_catalog.py

# ejecutar el Language Server ARM64 emulado
bash runtime/harness/run_ls.sh --help

# backup verificable a Drive (crea el manifiesto con SHA-256 y lo sube)
python3 tools/backup.py --dest ANTIGRAVITY_CONSOLIDATED_2026-11-26

# comprobar que el código cumple el contrato verificado
cd /workspace/megadaemon && python3 tools/ls_auth_contract_check.py
```

---

## 6. Referencias cruzadas

* Análisis de auth → [`docs/LS_AUTH_ANALYSIS.md`](docs/LS_AUTH_ANALYSIS.md) §18 (identidad + parche), §19 (catálogo)
* Evidencia por afirmación → [`docs/VERIFICATION_LEDGER.md`](docs/VERIFICATION_LEDGER.md)
* Binarios y plataformas → [`docs/ANTIGRAVITY_BINARIES.md`](docs/ANTIGRAVITY_BINARIES.md)
* Contrato legible por máquina → [`reference/ls_auth_contract.json`](reference/ls_auth_contract.json)
* Código → [`reference/go/lsauth/`](reference/go/lsauth/) y [`reference/rust/ls_auth/`](reference/rust/)
* Reglas de trabajo → [`docs/WORKSPACE_NORM.md`](docs/WORKSPACE_NORM.md)


---

## 7. Corpus de conversaciones (todos los agentes)

Todo lo dicho en las sesiones de todos los agentes, volcado a notas legibles,
decodibles e indexables. Ver [`docs/corpus/README.md`](docs/corpus/README.md).

- **91 sesiones** con mensajes · **7 358 mensajes** · índice temático en 9 temas
  ([`docs/corpus/FACTS.md`](docs/corpus/FACTS.md), cada afirmación con referencia
  sesión + nº de mensaje).
- **Limitación medida**: `session_messages` **no pagina**. `before` es inoperante
  (misma llamada con `before` distinto ⇒ misma ventana, mismo hash) y el tope es
  100 mensajes ⇒ 65 sesiones quedan truncadas por el principio. No hay vía con la
  API expuesta.

```bash
python3 tools/corpus_build.py    # reconstruye sesiones/, MESSAGES.jsonl e índices
python3 tools/corpus_facts.py    # regenera FACTS.md
```

---

## 8. Arnés de captura local del Language Server

Ejecuta el **LS real** (ARM64, v2.4.1) contra un servidor local que graba cada
petición. Cero tráfico a Google: endpoints a `127.0.0.1` y `HTTP_PROXY` a
`127.0.0.1:1`. Ver [`docs/ANTIGRAVITY_HARNESS.md`](docs/ANTIGRAVITY_HARNESS.md).

```bash
bash tools/agy_harness/launch_ls.sh --port 8085 --timeout 90
TOKEN_FILE=/ruta/token.json bash tools/agy_harness/launch_ls.sh   # con cuenta real
```

Tres requisitos medidos sin los cuales el LS muere con rc=1 y sin mensaje:
**PTY** (`ioctl TCGETS`), **bit +x** y **copia local** (el NFS no sirve).

---

## 9. Backups verificables en Google Drive

| carpeta | id | enlace |
|---|---|---|
| `ANTIGRAVITY_CONSOLIDATED_2026-11-26` | `1Y22_b9lZF9YwSrANV7uOKAQpNB0Fv_rq` | https://drive.google.com/drive/folders/1Y22_b9lZF9YwSrANV7uOKAQpNB0Fv_rq |
| `ANTIGRAVITY_FULL_2026-11-26` | (este backup) | incluye corpus + arnés |

```bash
python3 tools/backup.py --dest NOMBRE_CARPETA   # manifiesto sha256 + verificación
```

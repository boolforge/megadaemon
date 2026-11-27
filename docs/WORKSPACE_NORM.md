# NORMA DE TRABAJO — workspace único de Antigravity

> Norma **general** para cualquier agente, tarea o sesión que toque Antigravity,
> Gemini, Go o el Language Server. Existe para que el trabajo de distintos agentes
> **no se pise**, se **referencie** bien y se **respalde** sin sorpresas.
>
> Creada: 2026-11-26 · Workspace canónico: `/workspace/antigravity/`

---

## 0. Por qué existe esta norma

Los espacios de trabajo de este entorno son **compartidos** (NFS) y los ejecutan
varios agentes a la vez. Los tres fallos que ya han costado tiempo:

1. **Pisarse**: un `git add -A` en un repo compartido mete cambios de otro agente.
2. **Duplicar**: dos agentes manteniendo dos copias de los mismos artefactos ⇒
   divergen y nadie sabe cuál vale.
3. **Perder**: trabajo hecho y no respaldado, ni referenciado desde ningún sitio.

Esta norma elimina los tres: **un solo espacio canónico**, **rutas exactas al
commitear**, y **backup verificable con SHA-256**.

---

## 1. Dónde vive cada cosa

| Tipo de cosa | Ubicación canónica | Notas |
|---|---|---|
| Informes, documentación | `antigravity/docs/` | Los documentos son **la fuente de verdad**; el código los referencia |
| Evidenciamachine-legible (TSV/CSV) | `antigravity/recon/` | Un artefacto por afirmación |
| Catálogos por versión/plataforma | `antigravity/versions/` | Un artefacto por plataforma/versión |
| Banco de pruebas (emulación, shims, símbolo) | `antigravity/runtime/` | Nada que ejecutar fuera de aquí |
| Oráculo Go / Rust | `antigravity/reference/{go,rust}/` | Espejo del repo de código, para poder entregarlo suelto |
| Contrato verificable | `antigravity/reference/ls_auth_contract.json` | Fuente de verdad legible por máquina |
| Utilería | `antigravity/tools/` | Un script = una tarea; nada de scripts gigantes |
| Reconstrucciones y repos de referencia | `antigravity/repo/` | Sólo **inventarios** de los repos externos, no copias |
| Parches y diffs | `antigravity/patches/` | Cada parche con su contexto y su verificación |
| Trazas y logs | `antigravity/logs/` | Logs de ejecución, no de chat |
| Resultados de pruebas | `antigravity/tests/` | Con el comando exacto que los produjo |

**Nunca** se escribe en: `/workspace/agy-re/` (read-only), `/workspace/md-secrets/`
(secretos), ni en el árbol de otro agente.

---

## 2. Reglas de escritura

1. **Nada de copias**: un artefacto vive en **un** sitio. Los enlaces simbólicos
   (`/workspace/out` → `antigravity/docs`) sirven de compatibilidad, no de duplicado.
2. **Nada de binarios en el repo**: los binarios viven en `scratch/` (efímero) o en
   Drive (permanente); en el repo sólo su **hash**, su **tamaño** y la **URL**.
3. **Todo artefacto tiene su procedencia**: el fichero incluye una cabecera con
   método, comando y fecha. Si no se puede re-ejecutar, no entra.
4. **Todo lo no verificado se marca** con la cadena literal `NO VERIFICADO` y la
   razón. Sin excepciones.
5. **Los secretos no se imprimen jamás**: ni tokens, ni credenciales, ni valores.
6. **Ficheros ajenos intactos**: si `git status` muestra ficheros modificados que no
   son tuyos, no los toques, no los commitees.

---

## 3. Reglas de commit (repos compartidos)

```bash
# NUNCA:  git add -A    /    git add .
# SIEMPRE:
git add -- ruta/exacta/fichero1 ruta/exacta/fichero2
git commit -m "..."
git push origin <rama-del-agente>
```

* Un módulo nuevo ⇒ `pub mod` + el fichero **en el mismo commit**.
* `main` va muy por detrás ⇒ se trabaja y se empuja en la **rama del agente**
  (`agent/automacion-software` en `megadaemon`).
* Antes de commitear: `git diff --cached --stat` y comprobar que **sólo** están
  tus rutas.

---

## 4. Protocolo de backup (norma general)

**Cuándo**: al cerrar una fase, y siempre **antes** de una restructuración que mueva
ficheros.

```bash
python3 /workspace/antigravity/tools/backup.py --dest <NOMBRE_CARPETA>
```

La herramienta, en orden:

1. Construye un **manifiesto SHA-256** de cada fichero.
2. Sube a una **carpeta propia y nombrada** (un backup por tema; nunca mezclar).
3. Verifica que el hash **remoto** coincide con el local en los documentos.
4. **Elimina duplicados** por nombre, conservando el más reciente.
5. Escribe `BACKUP_INDEX.md` en la carpeta con: id, enlace, fecha, nº de ficheros,
   bytes y la tabla completa `fichero → bytes → sha256`.
6. **Nunca** imprime el token ni la credencial.

**Carpeta privada** de destino: `megadaemon-backup` (id `15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`).
Credencial: `/workspace/md-secrets/gdrive_credentials.json` (0600).

**Enlaces**: `https://drive.google.com/drive/folders/<ID_DE_CARPETA>`.

---

## 5. Coordinación y saturación del entorno

El entorno tiene **2 CPUs y 3 GB de RAM** (límite del cgroup) sobre NFS. Reglas:

| Actividad | Límite |
|---|---|
| Descargas simultáneas | **1** (secuenciales dentro de un job) |
| Jobs de CPU pesados simultáneos | **1** |
| Jobs totales en background | **≤ 2** (uno de red + uno local) |
| Extracciones grandes | selectivas: extraer **sólo** lo que interesa, borrar el original |
| `du`/`find` recursivos sobre `/workspace/agy-re` (9,2 GB) | **prohibido** en horario de otros agentes: satura el NFS |
| Memoria de un proceso | < 1 GB |

**Protocolo de un job largo** (`tools/j*.sh`):

1. Un objetivo por línea, con su log.
2. Borrado del artefacto original en cuanto se ha extraído lo relevante.
3. `say()` para que el progreso se vea desde el log sin consultar la tarea.
4. Nunca dos jobs de red a la vez.

---

## 6. Ciclo de trabajo recomendado

```
1. LEER      docs/BACKLOG.md  -> elegir el punto abierto más rentable
2. VERIFICAR método + comando reproducibles antes de afirmar nada
3. PRODUCIR  artefacto en recon/ o versions/ con cabecera de procedencia
4. DOCUMENTAR el hallazgo en docs/ (el documento, no sólo el chat)
5. INTEGRAR  código (Go/Rust) + contrato JSON + comprobador de CI
6. RESPONDER tests: go vet+test, cargo fmt+clippy+test, comprobador de contrato
7. COMMIT    con rutas exactas en la rama del agente
8. RESPALDAR python3 tools/backup.py --dest <CARPETA>
9. ACTUALIZAR docs/BACKLOG.md con el nuevo estado
```

---

## 7. Reglas de la emulación y la instrumentación

* **El shim de ocultación va en la frontera** (proceso de `qemu`), no dentro del
  guest, cuando el objetivo es un binario **Go**: Go usa syscalls crudos y lee
  `/proc/self/auxv`; el guest-side `LD_PRELOAD` no lo alcanza.
* **Todo lo que se afirme sobre detección se mide**, no se supone: la sonda
  `runtime/harness/probe_env.c` se ejecuta con y sin shim y se comparan las dos
  salidas.
* **La execution simbólica es acotada**: se declara el objetivo (una función, un
  camino), se enumeran las rutas y se da el resultado como **enumeración completa**
  del espacio de estados relevante, no como «parece que».
* Todo lo que se ejecute emulado **deja log** en `logs/`.

---

## 8. Qué está prohibido

* Mover o borrar ficheros de otro agente.
* `git add -A` / `git add .` en repos compartidos.
* Escribir bajo `/workspace/agy-re/`.
* Imprimir tokens, credenciales o valores de secretos.
* Afirmar un hecho sin método y artefacto.
* Mover el workspace sin dejar el enlace de compatibilidad y sin actualizar el
  `MANIFEST`.

# GHIDRA_SUPPORT — soporte de Ghidra headless en MegaDaemon

> Rama: `agent/ghidra-support` · Módulo: `rust-core/src/ghidra.rs` ·
> CLI: `rust-core/src/bin/md-ghidra.rs` · Test: `rust-core/tests/ghidra_smoke.rs`

Este documento dice **qué funciona, qué no, y con qué límites medidos**.
Todo lo que sigue procede de ejecuciones reales en este sandbox; lo que no
se ha ejecutado está marcado explícitamente como **NO PROBADO**.

---

## 1. Qué resuelve

El servidor de RE de MegaDaemon tenía un hueco: los binarios ELF/Go grandes
(el `liblanguage_server.so` de Antigravity son 181 MB y 161.682 funciones)
no entran en un descompilador de móvil. Ghidra headless sí, pero necesita un
JDK y hay queiocluirlo bien para que **no cuelgue el agente**.

`rust-core/src/ghidra.rs` da cuatro cosas:

| API | Qué hace |
|---|---|
| `GhidraTool::detect(&cfg)` | Localiza `analyzeHeadless` y un **JDK** (PATH, `MD_GHIDRA_DIR`, `JAVA_HOME`, `/opt`, `/data/data/com.agy.app/files/...`) |
| `import_binary(..)` | `-import <bin> -noanalysis` dentro del proyecto Ghidra |
| `apply_symbols(..)` | Aplica el export de símbolos de `unstrip` sobre el programa |
| `decompile_anchors(..)` | Desensambla, crea la función, decompila y vuelca el pseudocódigo a texto |

---

## 2. Reglas duras del módulo (por qué es seguro cablearlo)

1. **Nunca cuelga el agente.** Todo proceso externo se lanza con
   `run_with_timeout()`: si se agota el plazo se mata el **grupo de procesos**
   completo (`setpgid` + `killpg`), no sólo el hijo. Así no quedan JVMs
   huérfanas comiéndose la RAM.
2. **Cola global.** Un único `Mutex` estático serializa las ejecuciones de
   Ghidra de todo el proceso. La segunda llamada espera con presupuesto
   (`queue_wait`) y, si no le toca, devuelve `GhidraError::Busy` — no bloquea.
3. **Sin Java ⇒ mensaje claro, nunca panic.** `GhidraError::JavaMissing`
   dice qué falta, por qué (`Ghidra 11.2+` exige **JDK 21**, no JRE) y cómo
   instalarlo. `GhidraError::JavaNotJdk` distingue el caso "hay `java` pero no
   hay `javac`" (Ghidra compila los GhidraScript al vuelo).
4. **Cero dependencias nuevas.** `libc` ya estaba en `Cargo.toml`; sólo se usa
   para `killpg`. Nada de crates añadidos.

### La trampa del JDK (medida)

`openjdk-17-jre-headless` está instalado en el sandbox, pero:

* Ghidra 11.2.1 declara `application.java.min=21` ⇒ con Java 17 **ni arranca**
  (`Unable to prompt user for JDK path, no TTY detected`).
* Un **JRE** tampoco vale: `launch.sh` exige tipo `jdk` y los scripts Java se
  compilan con `javac` en caliente.

Por eso `GhidraTool` busca un **JDK** y expone `has_jdk`: si encuentra un JRE
lo dice con un mensaje que nombra `openjdk-21-jdk-headless`.

---

## 3. Uso

```bash
# ¿hay Ghidra y JDK en este entorno?
md-ghidra doctor

# importar un binario al proyecto
md-ghidra import /ruta/liblanguage_server.so --project-dir /tmp/ghidra --project Go

# aplicar el export de símbolos de unstrip (TSV: 0xADDR \t nombre \t comentario)
md-ghidra symbols /ruta/go.funcs.tsv --project-dir /tmp/ghidra --project Go

# volcar el pseudocódigo de un conjunto de anclas
md-ghidra decompile /ruta/anchors.tsv --out /tmp/decomp --project-dir /tmp/ghidra --project Go

# sólo una función (por subcadena del nombre)
md-ghidra decompile /ruta/anchors.tsv --filter "ServerSession_.CallTool" --out /tmp/decomp
```

Opciones: `--ghidra-dir`, `--java-home`, `--project-dir`, `--project`,
`--timeout`, `--queue-wait`, `--max-memory`, `--script-dir`, `--out`,
`--filter`, `--json`.

El **GhidraScript va embebido** en el módulo (`MD_GHIDRA_SCRIPT`, un
`MdGhidra.java` completo). Se escribe a disco en cada llamada: no hace falta
que el operador copies ficheros sueltos al móvil.

---

## 4. Lo que funciona (PROBADO en este sandbox)

* **Detección**: encuentra `analyzeHeadless` y el JDK; distingue JRE/JDK.
* **Import headless** de un ELF de 181 MB: `RC=0`, proyecto de 249 MB.
* **Aplicación de 161.682 símbolos** (export de `unstrip`) sobre el programa:
  `ok=161682 dup=0 err=0 skipped=0` en 24,25 s.
* **Depilación headless** de 32 anclas: 32/32 con pseudocódigo escrito.
* **Cola y timeout**: implementados y cubiertos por test (sin colgarse).
* **Sin Java**: error con mensaje accionable (test `java_missing_message_names_the_fix`).

### Detalles que Costó sangre (y que están resueltos)

1. **El image base del PIE.** Ghidra carga este ELF en `0x100000`, no en 0.
   El export de `unstrip` da *offsets* del ELF, así que **todo va desplazado
   `+0x100000`**. El script de `unstrip` original usa
   `af.getDefaultAddressSpace().getAddress(addr)` (offset crudo ⇒ dirección
   equivocada) y además `fm.createFunction(name, a, None, ...)`, que **falla
   si no hay desensamblado**. El módulo resuelve las dos cosas:
   * `addrFor()` prueba `offset + imageBase` y `offset`, y se queda con la
     que cae dentro de un bloque de memoria inicializado;
   * si no se puede crear una función, deja **etiqueta** con el nombre Go.
2. **Ghidra 11.2.1 no trae PyGhidra**, trae **Jython 2.7.3**. El
   `go.ghidra.py` que genera `unstrip` declara `@runtime PyGhidra`, así que
   Ghidra 11.2.1 no lo puede ejecutar tal cual. Por eso el módulo lleva su
   propio script **Java** (compilado por Ghidra al vuelo, con el JDK).
3. **Memoria**: con `-Xmx2G` el import muere con **RC=137** (OOM killer del
   kernel). Con `-Xmx1400M` entra. Está documentado arriba.

---

## 5. Lo que NO funciona / NO PROBADO

| Cosa | Estado | Nota |
|---|---|---|
| Análisis automático completo del binario | **NO PROBADO / LÍMITE MEDIDO** | El import con `-noanalysis` sí cabe; el auto-análisis sobre 44 MB de `.text` no se ha podido completar en el presupuesto de RAM de 3 GB (ver `RESUMEN_GHIDRA.md`). |
| Ghidra 11.2.1 + `go.ghidra.py` de `unstrip` tal cual | **NO FUNCIONA** | `@runtime PyGhidra` y Ghidra 11.2.1 sólo trae Jython. Variantes Jython medidas aparte. |
| `unstrip --types` (tipos Go) | **FALLA** | Literal: `unstrip: moduledata: no moduledata candidate found in data sections` (RC=1). Sin tipos ⇒ sin `DataType` structs. |
| `GhidraGo` (plugin de Go de Ghidra 11.2.1) | **NO APLICABLE** | No es un analizador: es un plugin que habla con la herramienta externa `ghidraGo`. No sirve en headless. |
| Descompilar en el móvil (Android ARM64) | **NO PROBADO** | El código es POSIX/Unix puro (`process_group` + `killpg`) y el layout de rutas (`GhidraConfig::mobile`) está preparado, pero **no se ha ejecutado** en el Samsung. |
| Firmas con tipos Go reales | **NO** | Sin auto-análisis y sin `unstrip --types`, las firmas son `undefined8 f(undefined8 *param_1, ...)`. Los **nombres** sí son los de Go. |
| `cargo build` del crate completo en NFS | **LÍMITE MEDIDO** | El target dir del NFS lo bloquean otros agentes; hay que usar `CARGO_TARGET_DIR` local (`/tmp/md-target`). |

---

## 6. Cómo probarlo

```bash
cd rust-core
CARGO_TARGET_DIR=/tmp/md-target cargo test --offline -j 1 --test ghidra_smoke
```

El test **no necesita Ghidra**: todo lo que depende de que haya Ghidra se
salta con un mensaje explícito ("Ghidra/JDK no disponible: test end-to-end
omitido"). El test end-to-end (import + symbols + decompile sobre `/bin/sh`)
sólo corre si la detección encuentra la herramienta y un JDK.

---

## 7. Añadidos en esta rama

* `rust-core/src/ghidra.rs` — módulo (autocontenido, sin tocar `cockpit_server.rs`
  ni `mcp_ext.rs`).
* `rust-core/src/bin/md-ghidra.rs` — CLI.
* `rust-core/tests/ghidra_smoke.rs` — smoke test.
* `rust-core/src/lib.rs` — **una** línea: `pub mod ghidra;`.
* `docs/GHIDRA_SUPPORT.md` — este documento.

No se ha cableado en `mcp_server.rs` ni `cockpit_server.rs` (ficheros
reservados): el módulo y el CLI ya son utilizables por separado.

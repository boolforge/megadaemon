# AUDITORÍA ADVERSARIAL · ROUND 2 (voz, índice, instalador, causa del 502)

**Fecha**: 2026-11-20 · **Auditor**: agente General (sesión `d41a8f14`)
**Alcance**: parche de seguridad de la voz (`voice_runtime.rs`, AUDIT-VOICE-01..05),
estado real de compilación del repo (árbol, índice, `main`), instalador del móvil y
la causa raíz del 502 del 2026-11-20 02:54.
**Método**: todo lo que sigue está **medido** con el comando indicado; nada es
"parece que". Donde no hay reproducción hermética, se dice explícitamente por qué.

---

## 0. Resumen ejecutivo

| # | Severidad | Hallazgo | Fichero:línea |
|---|-----------|----------|---------------|
| F-01 | **P0** | `main` **no compilaba**: el guard `st` del mutex global quedó dentro de un brazo del `match` | `rust-core/src/cockpit_server.rs:1949,1958,1964` |
| F-02 | **P0** | El **redactor deja el secreto intacto** cuando va pegado a otros caracteres (`key=`, comillas, query-string) | `rust-core/src/voice_runtime.rs:458-470` |
| F-03 | **P0** | El mensaje que la voz deposita en el **buzón del peer** se escribe **en claro** (estado compartido ⇒ repo público) | `rust-core/src/webui.rs:873` + `voice_runtime.rs:952-961` |
| F-04 | **P1** | El gate `confirm` se aplica a la **cadena pedida**, no a la **acción resuelta**: una `Do` decidida por el cerebro lo esquiva | `rust-core/src/voice_runtime.rs:985-1029` |
| F-05 | **P1** | El canal 4 (`swarm_chat`) manda al proveedor el texto **sin redactar** (los otros 3 sí) | `rust-core/src/voice_runtime.rs:691-704` |
| F-06 | **P1** | `panic = "abort"` + **0 `catch_unwind`** ⇒ cualquier panic en un handler tumba el daemon entero | `rust-core/Cargo.toml:53` |
| F-07 | **P2** | `redact()` **aplana el texto** (se come los saltos de línea) ⇒ un secreto partido por un `\n` queda a medias | `rust-core/src/voice_runtime.rs:458-470` |
| F-08 | **P2** | `redact_value` **no redacta las claves** JSON ⇒ una credencial usada como nombre de campo sobrevive | `rust-core/src/voice_runtime.rs:474-492` |
| F-09 | **P2** | `mesh_snapshot` está en la allow-list pero **no existe** en el catálogo (51 tools) | `rust-core/src/voice_runtime.rs:383` |
| F-10 | **P2** | `is_vetoed()` ya **no gatea nada** (sólo lo usan sus propios tests): API muerta que aparenta ser una puerta | `rust-core/src/voice_runtime.rs:387` |
| F-11 | **P2** | La allow-list **no es la única puerta**: 3 tools se ejecutan en brazos que nunca consultan `is_allowed()` | `rust-core/src/voice_runtime.rs:499-555` |
| F-12 | **P2** | El instalador **copiaba encima** del destino ⇒ ventana sin binario ⇒ el 502 | `deploy/install-phone.sh:131` (ya arreglado) |

**Causa raíz del 502 (la buena):** **no fue un panic**. El log del supervisor en el
móvil lo dice literalmente; y el **fix estructural del usuario (catch_unwind +
mutex a prueba de veneno + `ensure_live_shm` fuera del lock) SÍ está en
`main`** (verificado en `origin/main`), pero **dejó `main` sin compilar** (F-01).

---

## F-01 · P0 · `main` no compilaba

**Cómo lo he medido** (el árbol de `main`, no el de trabajo):
```bash
git archive HEAD | tar -x -C /tmp/audit-tree
cd /tmp/audit-tree/rust-core && cargo check --lib --tests
```
**Salida real**:
```
error[E0425]: cannot find value `st` in this scope
    --> src/cockpit_server.rs:1949:38
     |
1949 |                     if let Some(r) = st.runners.get_mut(&valid_target) {
help: the binding `st` is available in a different scope in the same function
    --> src/cockpit_server.rs:1931:29
error[E0425]: cannot find value `st` in this scope
    --> src/cockpit_server.rs:1958:40
error[E0425]: cannot find value `st` in this scope
    --> src/cockpit_server.rs:1964:31
```

**Causa**: al sacar `ensure_live_shm()` (escritura de 1 MB) fuera del guard, el
`let mut st = mutex.lock()…` se movió **dentro** del brazo `"launch"`, y los
brazos `"stop"` y `_` siguen usando el `st` exterior.

**Arreglo correcto (el que ya está stageado y compila)**: un **lock corto por
brazo**, con el guard soltado antes de cualquier I/O:
```rust
let valid_target = { let st = mutex.lock().unwrap_or_else(|e| e.into_inner()); … };  // guard soltado
match action {
    "launch" => { let _ = catch_unwind(|| ensure_live_shm(&valid_target));  // FUERA del lock
                  let mut st = mutex.lock().unwrap_or_else(|e| e.into_inner()); … }
    "stop"   => { let mut st = mutex.lock().unwrap_or_else(|e| e.into_inner()); … }
    _        => { let st = mutex.lock().unwrap_or_else(|e| e.into_inner()); … }
}
```
**Criterio de aceptación**: `cargo check --lib --tests` sobre `origin/main` ⇒ **0 errores**,
y el diff del fix debe mostrar **un lock por brazo** y `catch_unwind` alrededor de
`ensure_live_shm`.

---

## F-02 · P0 · El redactor deja el secreto pegado

**Cómo lo he medido**:
```bash
cargo test --test audit_scout_voice
test voice01_redact_no_deja_el_secreto_cuando_va_pegado ... FAILED
[VOICE-01] P0: el redactor deja el secreto intacto en 3 caso(s):
  ["token=ghp_AAAABBBB…", "\"ghp_AAAABBBB…\"", "https://x/cb?access_token=ghp_AAAABBBB…"]
```
**Causa** (`voice_runtime.rs:458-470`): `redact()` parte por `split_whitespace()` y
comprueba `low.starts_with(prefijo)`. Un secreto pegado a `=`, `"` o `?` es **un
solo token que no empieza por el prefijo** ⇒ no se redacta ⇒ sale al journal, al
sync y al tablón.

**Control verde (el detector funciona cuando el prefijo va al principio)**:
`"Authorization: ghp_…"`, `"Bearer ghp_…"` y `"clave ghp_…"` **sí** se redactan.

**Criterio de aceptación**: `redact()` **no debe dejar ninguna subcadena de la
credencial** en la salida, sea cual sea el carácter que la preceda ⇒ detector por
**subcadena** (no anclado al inicio del token) + separadores (`= " ' ? & :`).

---

## F-03 · P0 · El buzón del peer escribe la credencial en claro

**Cómo lo he medido**:
```bash
cargo test --test audit_scout_voice
test voice02_el_mensaje_al_buzon_del_peer_no_lleva_secretos ... FAILED
[VOICE-02] P0: la credencial quedó en CLARO en el buzón compartido
  /tmp/md-audit-voice-buzon-430015/inbox/peer-audit.ndjson
```
**Causa**: `voice_runtime::speak()` (`:952-961`) redacta **sólo la copia que
devuelve** (`:958`); el mensaje que se **escribe** va en claro a
`webui::node_message()` (`webui.rs:873-879`, campo `"message": message` sin pasar
por el redactor). Ese buzón (`<state>/inbox/<peer>.ndjson`) se replica al
tablón y de ahí al **repo, que es público**.

**Criterio de aceptación**: lo que se escribe en `inbox/<peer>.ndjson` pasa por
`redact_value`/`redact` igual que la línea de broadcast; el test `voice02` queda
verde.

---

## F-04 · P1 · El gate `confirm` mira la cadena pedida, no la acción resuelta

`voice_runtime.rs:985-1029`:
```rust
let wants_tool = args.get("action").and_then(|v| v.as_str()) == Some("do");
if wants_tool && !confirm { return error("confirmation_required") }
let action = match args.get("action")… {
    Some("do") => Action::Do { … },
    _ => { let brain = default_brain(); plan_next(&ctx, brain.as_ref()) }   // <-- el cerebro decide
};
run_action(&action, spread);
```
El `LlmBrain::parse_action` **sí** sabe emitir `"do"` (`agent_voice.rs:398`) ⇒ una
acción `Do` **decidida por el cerebro** (rama `_`) **no pasa por el gate de
`confirm`.

**Alcance medido (no exagerado)**: con el `RuleBrain` determinista la voz nunca
devuelve `Do`, y el bucle autónomo ni siquiera arranca sin `MEGADAEMON_VOICE=1`
(`voice_runtime.rs:1059`) ⇒ **hoy no es explotable**; lo es en cuanto se encienda
el cerebro LLM. El executor sí queda limitado por la allow-list (F-11).

**Criterio de aceptación**: el gate se aplica a la **acción resuelta**
(`if matches!(action, Action::Do{..}) && !confirm { error }`), no al texto pedido.

---

## F-05 · P1 · El canal 4 manda al proveedor el texto sin redactar

`voice_runtime.rs:691-704`: `spoken` se construye con las líneas **originales**
(`lines.iter()`), mientras que los canales 1-3 usan `safe_line` (`:657-660`).
La *respuesta* sí se redacta (`:710`), pero **la petición** sale con el secreto
intacto hacia el proveedor.

**Criterio de aceptación**: `spoken` se construye a partir de las líneas ya
redactadas.

---

## F-06 · P1 · `panic="abort"` sin `catch_unwind` ⇒ un panic tumba el daemon

**Medido antes del arreglo estructural**:
```bash
grep -rn "catch_unwind" rust-core/src --include=*.rs     # 0 resultados
grep -n  "panic" rust-core/Cargo.toml
53:panic = "abort"
```
Con `panic = "abort"`, **un panic en cualquier hilo aborta el proceso**: el
`catch_unwind` por conexión y el mutex a prueba de veneno son la única defensa.

**Estado actual (verificado en `origin/main`)**: el arreglo del usuario **sí está
en main** (`catch_unwind` alrededor de `ensure_live_shm`, `lock().unwrap_or_else(|e|
e.into_inner())`, y el lock corto por brazo) ⇒ **F-01 es lo único que quedaba
rojo de ese arreglo**.

**Criterio de aceptación**: un test que provoque un panic en un handler y
compruebe que `/health` sigue respondiendo y el proceso no muere.

---

## F-07 · P2 · `redact()` aplana el texto

`voice_runtime.rs:458-470` parte por `split_whitespace()` y re-une con un único
espacio ⇒ **se pierde el salto de línea** y un secreto partido por un `\n` queda
sólo a medias redactado.

**Medido**: `test voice06_redact_no_debe_destruir_el_texto ... FAILED`
⇒ *"`redact()` ha aplanado el texto (se comió los saltos de línea)"*.

**Criterio de aceptación**: la redacción conserva la estructura del texto.

---

## F-08 · P2 · `redact_value` no redacta las claves

`voice_runtime.rs:474-492`: recorre valores, pero **copia la clave tal cual**
(`out.insert(k.clone(), …)`) ⇒ una credencial usada como nombre de campo
sobrevive.

**Medido**: `test voice07_redact_value_redacta_las_claves ... FAILED`
⇒ *"la credencial sobrevive como CLAVE del objeto JSON"*.

**Criterio de aceptación**: `redact_value` aplica el redactor a claves y valores.

---

## F-09 · P2 · `mesh_snapshot` no existe

`voice_runtime.rs:383` la declara en la allow-list. El catálogo **canónico** es
`mcp_server::tool_definitions()` (51 tools, es lo que sirve `tools/list`):
```bash
cargo test --test audit_scout_voice
[VOICE-05] registro MCP: 51 tools; allow-list de la voz: 11 entradas; muertas: ["mesh_snapshot"]
```
(`mcp_ext::tool_definitions()` es sólo una parte del catálogo: usarla da falsos
positivos — medido y corregido en el test.)

**Criterio de aceptación**: toda entrada de la allow-list es una tool real del
catálogo o una rama explícita del ejecutor.

---

## F-10 · P2 · `is_vetoed()` ya no gatea nada

**Medido**:
```bash
grep -rn "is_vetoed" rust-core/src rust-core/tests
rust-core/src/voice_runtime.rs:387:  pub fn is_vetoed(tool: &str) -> bool {   # definición
rust-core/src/voice_runtime.rs:1108-1113: (6 asserts en sus propios tests)  # usos
```
⇒ **ningún camino de ejecución la consulta**; la allow-list la ha sustituido
(F-11). Es API pública: cualquier llamante futuro creería que hay un veto.

**Criterio de aceptación**: borrarla o marcarla `#[deprecated]`/`pub(crate)` con
un doc que diga que es informativa.

---

## F-11 · P2 · La allow-list no es la única puerta

`voice_runtime.rs:499-555`: los brazos `"mesh_claim"`, `"mesh_ask"` y
`"swarm_chat"` se ejecutan **antes** de la comprobación `if !is_allowed(other)`
(que sólo está en el brazo `other`). Hoy no diverge (las tres están en
`ALLOWED`), pero la allow-list **no es** la puerta única ⇒ puede divergir sin
que nadie se entere.

**Criterio de aceptación**: la comprobación de la allow-list se aplica a
**todas** las tools (una sola puerta).

---

## F-12 · P2 · El instalador copiaba encima del destino (el 502)

**Cómo lo he medido** (log del supervisor en el móvil, vía `root_exec`):
```
[2026-11-20 02:54:39] megadaemon no está corriendo: relanzando (intento 0)
[2026-11-20 02:54:39] ERROR: no existe /data/local/tmp/megadaemon/megadaemon-core
[2026-11-20 02:54:59] … (intento 1)  ERROR: no existe …
[2026-11-20 02:55:39] … (intento 2)  ERROR: no existe …
[2026-11-20 02:56:59] … (intento 3)  ERROR: no existe …
```
Y **cero panics**:
```bash
grep -a 'panicked\|index out of range\|stack overflow\|SIGABRT' /sdcard/megadaemon.log   # vacío
dmesg | grep -i 'Killed process\|megadaemon'                                             # vacío
```
⇒ **no fue un panic ni saturación**: el instalador **copiaba encima** del destino
(`cp -f "$SRC_BIN" "$BIN"`, `install-phone.sh:131`) ⇒ ventana sin binario ⇒ el
supervisor gastó `MAX_FAST_FAILS=3` ⇒ `exit 0` ⇒ y el instalador reiniciaba
**sólo** el daemon ⇒ nadie relanzó el origen ⇒ **502 (origen caído)**.

**Arreglo aplicado** (`deploy/install-phone.sh`, commit `311c32c`):
1. Instalación **atómica**: `cp → "$BIN.new"` (mismo directorio ⇒ mismo FS) y
   `mv` (renombrado atómico) ⇒ **el destino nunca desaparece**.
2. `chmod 755` **antes y después** del `mv` ⇒ el mensaje engañoso "no existe"
   (que en realidad era "no ejecutable") no vuelve.
3. El **supervisor** también se instala con `cp`+`mv` (un `sh` vivo lee el
   fichero **por offset**: sobrescribirlo en vivo puede ejecutar basura).
4. `restart()`: para el supervisor **viejo** (nunca dos supervisores), mata el
   daemon, arranca el nuevo y es **fail-safe**: si en 30 s no hay daemon,
   **arranca el daemon directamente** ⇒ el canal no puede quedarse muerto.
5. `rollback()`: atómico + reinicia el supervisor.

**Criterio de aceptación**: durante una instalación, `ls -l $BIN` **nunca**
devuelve "no existe" (el `mv` es atómico) y, tras el deploy, hay **exactamente
un** supervisor y `/health` responde.

---

## Controles verdes (lo que NO es un hallazgo)

| Test | Qué comprueba | Resultado |
|------|---------------|-----------|
| `voice03` | la allow-list rechaza `fs_write`, `root_exec`, `restart`, `reboot`, `git_ops`, `battery_saver`, `adb_shell`, `signing_vault`, `megadaemon_rizin`; y **normaliza** mayúsculas/espacios (`" MESH_CLAIM "`, `" mesh_claim "` → permitidas) | **ok** |
| `voice04` | `handle("act", {action:"do"})` sin `confirm` ⇒ `{"ok":false,"error":"confirmation_required"}` | **ok** |
| `voice08` | los campos `sha256/hash/digest/checksum/md5` se **preservan** (el broadcast los necesita) y los formatos con prefijo bien conocido se redactan | **ok** |
| `cargo check --lib --tests` sobre el **índice** | ¿un commit del índice deja el repo compilable? | **0 errores** (`Finished dev profile in 11m37s`) |
| `grep -rn catch_unwind rust-core/src` | ¿existe la defensa por conexión? | ahora **sí** (arreglo del usuario, en `origin/main`) |

**Sobre el índice y `main` (pregunta (a) del principal)**: el índice **sí**
compila ⇒ **un `git commit` del índice actual deja el repo compilable** ⇒ lo
correcto es commitear lo que ya está stageado. **`main` no compilaba** (F-01) ⇒
reconstruid contra `origin/main` **después** de que se commitee el fix por brazo.

**Sobre el árbol compartido**: tiene **4 módulos borrados**
(`agent_loop`, `persistent_shell`, `backup_formats`, `drive_client` ⇒ `E0583`) y
varios `docs/` borrados ⇒ **no compilar ahí** ⇒ worktree limpio:
```bash
git worktree add --detach /run/agent-runtime/md/wt origin/main
cd /run/agent-runtime/md/wt/rust-core
CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_TARGET_DIR=/run/agent-runtime/md/build-wt cargo test --release
```
Los ficheros borrados **no se borran ni se commitean**: son trabajo en vuelo de su
dueño.

---

## Batería de auditoría

`rust-core/tests/audit_scout_voice.rs` (nuevo, **sólo tests**):
```
cargo test --test audit_scout_voice
test voice01_redact_no_deja_el_secreto_cuando_va_pegado ... FAILED
test voice02_el_mensaje_al_buzon_del_peer_no_lleva_secretos ... FAILED
test voice03_la_allow_list_rechaza_lo_destructivo_y_normaliza_el_nombre ... ok
test voice04_la_accion_do_exige_confirmacion ... ok
test voice05_la_allow_list_no_contiene_tools_inexistentes ... FAILED
test voice06_redact_no_debe_destruir_el_texto ... FAILED
test voice07_redact_value_redacta_las_claves ... FAILED
test voice08_los_hashes_se_preservan_por_campo_y_los_formatos_conocidos_se_redactan ... ok
test result: FAILED. 3 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out
```

### Nota de método (dos falsos positivos que detecté en mi propio test)
1. `mcp_ext::tool_definitions()` devuelve **un array**, no un objeto con clave
   `"tools"` ⇒ leerlo como objeto daba un catálogo vacío ⇒ "9 tools muertas"
   (**FALSO**). El catálogo canónico es `mcp_server::tool_definitions()` (51).
2. El primer `cargo check` se leyó **sin el "Finished"** ⇒ lo tomé por verde
   cuando el build seguía corriendo. Regla: **un check no está verde hasta que
   sale `Finished`**.

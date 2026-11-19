# Auditoría de la capa harness/MCP — v13.4

**Sesión**: `458507570102555` · **Ámbito**: capa de harness MCP, control del
dispositivo y sincronización (v13.3+) · **Fecha**: 2026-11-19
**Base**: `main` = `31b5568` ("fix(v13.4): despliegue Magisk copia la plantilla web...")

> Todo lo que sigue está **medido**, no estimado. Cada defecto reproduce un
> fallo real de la suite en rojo antes del arreglo y en verde después.

---

## 0. Resumen

| | |
|---|---|
| Defectos **críticos** corregidos | 1 (H-01 + H-02, la misma rama de política) |
| Defectos **altos** corregidos | 1 (H-03) |
| Landmines de integración detectados (no corregidos: son de otro módulo) | 2 (B-2) |
| Hallazgos residuales documentados, sin tocar | 3 (R-1 … R-3) |
| Tests nuevos | 11 (10 de integración + 1 unitario) |
| Suite completa | 118 → **129**, 0 fallos |
| Build release | sin errores |
| `seere-selftest` | `SELFTEST_V11_RUST_OK` |

---

## 1. Defectos corregidos

### H-01 — La confirmación se saltaba al escribir en `/system` con `chmod`

**Dónde**: `rust-core/src/android.rs`, `fs_write()`.

**Código original**:

```rust
if path.starts_with("/dev/block") || path.starts_with("/system/") && chmod.is_empty() {
    return err("needs_confirm", "escribir en /dev/block o /system exige confirm_destructive=true");
}
```

**Por qué estaba mal**: en Rust `&&` liga más fuerte que `||`. La expresión se
evaluaba como `A || (B && C)`, es decir:

| `path` | `chmod` | condición original | ¿debería bloquear? |
|---|---|---|---|
| `/dev/block/sda` | — | `true` | sí |
| `/system/app/x.apk` | — | `true` | sí |
| `/system/app/x.apk` | `644` | **`false`** | **sí** |

Es decir: **escribir en `/system` junto con un `chmod` saltaba la
confirmación** y llegaba a la partición del sistema sin `confirm_destructive`.
La condición `&& chmod.is_empty()` no tenía sentido funcional: el `chmod` no
tiene ninguna relación con si la escritura es destructiva.

**Corrección**: la decisión se extrajo a una función pura y explícita, y el
`chmod` desapareció de la ecuación.

```rust
fn write_requires_confirm(path: &str) -> bool {
    path == "/dev/block"
        || path.starts_with("/dev/block/")
        || path == "/system"
        || path.starts_with("/system/")
}
```

**Prueba**: `h01_escribir_en_system_con_chmod_exige_confirmacion`.

---

### H-02 — El error `needs_confirm` era un contrato muerto

**Dónde**: `rust-core/src/android.rs`, `fs_write()` + esquema MCP + `dispatch()`.

**El problema**: el error que devolvía `fs_write` decía

> "escribir en /dev/block o /system exige **confirm_destructive=true**"

pero ese parámetro:

1. **no existía** en la firma de `fs_write` (`path, content, base64, chmod`),
2. **no estaba declarado** en el `inputSchema` MCP de `fs_write`,
3. **no lo leía** `android::dispatch` (que construía la llamada con `s()`/`b()`
   sobre un conjunto fijo de claves).

Es decir, el daemon pedía una confirmación que **el cliente no tenía forma
de enviar**. Un agente que obedeciera el error («repítelo con
`confirm_destructive=true`») enviaba el parámetro, el servidor lo ignoraba, y
volvía a devolver el mismo error: un bucle sin salida, con una escritura
destructiva bloqueada de forma que no se puede desbloquear.

**Corrección**: `confirm_destructive` existe ahora en los tres sitios a la vez,
que es la única forma de que el contrato sea cierto:

- firma: `fs_write(path, content, base64, chmod, confirm)`,
- esquema MCP: `"confirm_destructive": { "type": "boolean", "default": false }`,
- despacho: `"fs_write" => fs_write(&s("path"), &s("content"), b("base64"), &s("chmod"), b("confirm_destructive"))`.

Además, la escritura bloqueada **se audita** (`fs_write.blocked` en
`audit.ndjson`), igual que ya hacía `root_exec_tool` con `root_exec.blocked`.
Sin eso, un intento bloqueado no dejaba rastro.

**Pruebas**: `h02_el_esquema_de_fs_write_declara_confirm_destructive` y el
invariante genérico `toda_herramienta_que_pide_confirmacion_la_declara_en_su_esquema`,
que recorre todo el catálogo y falla si alguna descripción menciona
`confirm_destructive` sin declararlo en su esquema. Ese invariante es el que
impide que H-02 reaparezca en cualquier herramienta futura.

---

### H-03 — `/system` sin barra final se colaba

**Dónde**: `rust-core/src/android.rs`, `fs_write()`.

`starts_with("/system/")` no casa con la ruta `/system` exactamente, que
también es una escritura de sistema. En la prueba en rojo, `fs_write("/system")`
devolvió `{"ok": true}`: la escritura **se ejecutó**. Corregido con la misma
función `write_requires_confirm`, que compara `path == "/system"` aparte.

**Prueba**: `h03_escribir_en_system_sin_barra_final_exige_confirmacion`.

---

## 2. Landmines de integración (NO corregidos: son de otro módulo)

Estos dos los ha dejado otra sesión y **no son míos**, pero rompen el
`cargo build` en cuanto alguien cablee el módulo. Se documentan aquí para que
no se descubran a mitad de un cambio.

### B-2a — `repo_updates.rs` nunca se ha compilado

`rust-core/src/repo_updates.rs` (2.000 líneas, commit `1db663d`) **no está
declarado en `rust-core/src/lib.rs`**. Un fichero que no está en `lib.rs` no
entra en el crate: no se compila, no se prueba y sus errores no existen para
el compilador. `main` lleva ese módulo muerto desde que se subió.

### B-2b — `apk_inventory` no existe

`repo_updates.rs:713` hace:

```rust
fn inventory_json(limit: usize) -> Value {
    crate::apk_inventory::inventory(limit)
}
```

`rust-core/src/apk_inventory.rs` **no existe en el repositorio** (ni en
`main`, ni en ninguna rama, ni en ningún PR). En cuanto se añada
`pub mod repo_updates;` a `lib.rs`, el build falla con `cannot find module
apk_inventory`.

**Secuencia segura**: primero `apk_inventory.rs` (con su módulo en `lib.rs`),
después `repo_updates.rs`. Al revés rompe.

**Aviso**: esta sesión no puede resolverlo. `lib.rs` y `Cargo.toml` están
fuera de su ámbito por instrucción expresa, y `repo_updates.rs` /
`apk_inventory.rs` son de otro módulo.

---

## 3. Hallazgos residuales (documentados, sin tocar)

| # | Severidad | Dónde | Qué pasa | Por qué no se toca aquí |
|---|---|---|---|---|
| R-1 | Media | `mcp_server.rs`, `run_stdio_mcp_server` | Lee stdin con `lines()`, **sin tope de longitud de línea**. Una línea enorme (o un cliente roto) puede reventar la memoria del proceso. El transporte HTTP sí tiene tope (`MAX_BODY_BYTES = 8 MB`), el stdio no. | Es un endurecimiento, no un defecto de política. Se deja para que no se mezcle con el arreglo de seguridad. |
| R-2 | Media | `android.rs`, `is_destructive` | La lista de comandos destructivos es corta (`rm -rf /`, `mkfs`, `dd of=/dev/block`, `fastboot`). Un `cp` a `/system` vía `root_exec` **no** salta la confirmación, porque la lista no lo cubre. | Ampliar la lista con heurísticas (`cp ... /system`) puede dar falsos positivos sobre órdenes legítimas. Es una decisión de diseño, no un parche. |
| R-3 | Baja | `android.rs`, `fs_write` | No hay tope de tamaño del contenido escrito. Acota de forma indirecta el transporte HTTP (8 MB), pero por stdio no hay límite. | Mismo motivo que R-1. |
| R-4 | Info | `sync_web.rs` | `COOKIE_SOURCES` y `pref_value` no se usan: 2 avisos de `dead_code` en el build release. | Ruido, no defecto. |

---

## 4. La suite nueva

`rust-core/tests/harness_policy.rs` — 10 tests. Todos hablan con
`android::dispatch` (firma estable) y con el catálogo MCP publicado, **nunca**
con funciones internas: por eso la suite **compilaba y fallaba en rojo antes
del arreglo**.

### En rojo (antes del arreglo)

```
test h01_escribir_en_system_con_chmod_exige_confirmacion ... FAILED
test h02_el_esquema_de_fs_write_declara_confirm_destructive ... FAILED
test h03_escribir_en_system_sin_barra_final_exige_confirmacion ... FAILED
test result: FAILED. 7 passed; 3 failed
```

Salida real de H-01, que es la prueba del defecto:

```
escribir en /system con chmod debe exigir confirm_destructive, devolvio:
{"ok":false,"output":"mkdir: cannot create directory '/system': Not a directory\n
cp: cannot stat '/tmp/.../.md_write_2201': No such file or directory\n",
 "path":"/system/app/megadaemon_harness_probe.apk"}
```

La escritura **intentó llegar a `/system`**. Eso es el defecto.

Y la de H-03, donde la escritura se ejecutó de principio a fin:

```
`/system` (sin barra) tambien es una escritura de sistema, devolvio:
{"ok":true,"output":"","path":"/system"}
```

### En verde (después del arreglo)

```
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Suite completa

| Binario | Antes | Ahora |
|---|---|---|
| `unittests src/lib.rs` | 34 | **35** (+1 unitario en `android.rs`) |
| `tests/audit_scout_auth.rs` | 7 | 7 |
| `tests/audit_scout_hw.rs` | 16 | 16 |
| `tests/audit_scout_seere.rs` | 31 | 31 |
| `tests/audit_v13.rs` | 21 | 21 |
| `tests/harness_policy.rs` | — | **10** (nuevo) |
| `tests/npu_nnapi.rs` | 9 | 9 |
| **Total** | **118** | **129**, 0 fallos |

### Los 10 tests

| Test | Qué fija |
|---|---|
| `h01_escribir_en_system_con_chmod_exige_confirmacion` | La precedencia de operadores: `/system` + `chmod` exige confirmar |
| `h03_escribir_en_system_sin_barra_final_exige_confirmacion` | `/system` exacto también |
| `escribir_en_dev_block_exige_confirmacion` | `/dev/block` sigue bloqueado |
| `una_escritura_normal_no_pide_confirmacion` | No se rompe el caso corriente |
| `confirmar_habilita_la_escritura_en_system` | El flag **abre** la política (no se bloquea de más) |
| `h02_el_esquema_de_fs_write_declara_confirm_destructive` | El esquema declara el parámetro que el error exige |
| `toda_herramienta_que_pide_confirmacion_la_declara_en_su_esquema` | Invariante genérico sobre todo el catálogo: sin contratos muertos |
| `el_catalogo_mcp_no_tiene_nombres_duplicados` | Ningún nombre repetido en las 26 herramientas |
| `el_catalogo_mcp_es_la_union_de_los_tres_subcatalogos` | Lo anunciado es exactamente lo que existe (7 RE + 11 dispositivo + 8 sync = 26) |
| `toda_herramienta_anunciada_tiene_su_brazo_en_el_despacho` | Cada herramienta anunciada tiene su brazo en `android::dispatch` |

---

## 5. Reproducción

```bash
cd /workspace/megadaemon/rust-core
export PATH="$HOME/.cargo/bin:$PATH"

# Solo la suite nueva
cargo test --test harness_policy

# Suite completa
cargo test

# Build release sin avisos nuevos
cargo build --release

# Selftest de SEERE
./target/release/megadaemon-core seere-selftest
```

**Nota de entorno**: `fs_write` necesita `su -mm -c` para escribir de verdad.
En CI sin `su` la escritura falla y el daemon degrada (no rompe), pero los
tests que importan son los que **no** escriben nunca: los tres de política
rechazan antes de tocar el sistema.

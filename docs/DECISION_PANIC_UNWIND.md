# Decisión: `panic = "unwind"` en release (v15.3)

**Decisión (delegada por el principal el 2026-07-20): mantener `panic = "unwind"`.**
Ya está aplicada: `rust-core/Cargo.toml` lo dice explícitamente ("v15: SIN panic=abort a propósito").
Esta nota deja el razonamiento **medido** por escrito, porque otras notas ya lo contradicen.

## Por qué unwind

1. El daemon es **el origen del túnel de Cloudflare**: cualquier panic global ⇒ el móvil
   entero devuelve 502/530 (medido el 2026-07-20: el OOM killer se llevó cloudflared dos veces).
2. Con unwind, el `catch_unwind` por conexión **cumple su papel** ⇒ un panic aislado degrada
   *una* petición, no el servicio. Ya está en `cockpit_server.rs` (1928, 2273) y `webui.rs` (3854).
3. Con abort, **cualquier** panic (incluso en una ruta de análisis) tumba el proceso ⇒ el móvil
   "parece caído" sin estarlo. Un `catch_unwind` no salva de un panic no capturado **con abort**:
   de ahí la duda, resuelta a favor de unwind porque el coste de abort (una caída total) es
   mucho mayor que el de unwind (un hilo muerto que el supervisor puede reponer).

## Qué cambia en el diagnóstico (lo importante)

- La auditoría v15 (`docs/AUDIT_ADVERSARIAL_V15.md`, líneas 97, 175, 176, 237, 323) razona sobre
  `Cargo.toml:44 panic="abort"` ⇒ **ese supuesto ya no se cumple** (el perfil desenrolla desde v15):
  * **G-05** queda *resuelto en su parte (a)*: hay `catch_unwind` por conexión.
  * **C-06** y **C-07** dejan de "tumbar el daemon" y pasan a "500 por petición".
- El **P0 que queda de verdad es el mutex envenenado**: con unwind, `lock().unwrap()` sobre un
  mutex envenenado sigue paniqueando **en cada petición** ⇒ el daemon queda *vivo pero inútil*.
  El arreglo es **poison recovery** (`lock().unwrap_or_else(|p| p.into_inner())`, el patrón ya
  usado en `api_key.rs:277-280`) + `ensure_live_shm()` **fuera** del mutex global +
  `catch_unwind` por conexión ⇒ lane del nodo externo (`shm.rs` / `cockpit_server.rs`).
- **HW-01 (SIGABRT/134)**: con unwind, un panic de Rust **no** produce 134 ⇒ el 134 medido
  viene de otro sitio (un `abort()` explícito, un doble panic, o un assert en librería nativa:
  sqlite3 / NPU). Para cerrarlo hace falta un `panic::set_hook` que registre el panic con
  backtrace **antes** de morir ⇒ no lo implemento aquí (`cockpit_server.rs`/`shm.rs` intocables
  por la regla vigente): lo propongo a su dueño.

## Comentarios obsoletos que inducen a error (arreglo en su lane)

Dicen "con `panic=\"abort\"` en release" cuando el perfil **ya desenrolla**:
`rust-core/src/shm.rs:75`, `rust-core/src/util_text.rs:5`, `rust-core/src/magisk_module.rs:868`,
`rust-core/src/webui.rs:3850`. (`cockpit_server.rs:2269` ya está actualizado ✓.)

## Regla operativa mientras tanto

Ningún `panic` debe poder escapar al hilo de una conexión:
`catch_unwind` por conexión (ya en cockpit/webui) + `unwrap_or_else(|p| p.into_inner())` en los
mutex compartidos. Un panic aislado es un 500; un panic que tumba el daemon es una caída del móvil.

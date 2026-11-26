# testproj — proyecto real para probar Antigravity

Proyecto **real** (no un toy) usado como banco de pruebas del Language
Server de Antigravity y de sus Derivatives. Existe para que las pruebas
ejerciten capacidades reales del agente: leer y editar ficheros, ejecutar
comandos, correr tests, entender un proyecto multi-lenguaje y reportar.

## Layout

| ruta | para qué sirve |
|---|---|
| `src/lib.rs` | biblioteca Rust con lógica real (y un bug **plantado**,标记) |
| `tests/` | tests de integración Rust |
| `python/` | herramienta Python que consume la biblioteca vía CLI |
| `docs/` | notas que el agente debe leer antes de tocar nada |
| `TASKS.md` | guion de tareas que el agente debe poder completar |

## Características de Antigravity que este proyecto ejercita

1. **Lectura de ficheros** con límites de permisos por ruta.
2. **Edición** (parche) de ficheros de más de un lenguaje.
3. **Ejecución de comandos** (`cargo test`, `python3`) con timeout.
4. **Contexto de proyecto**: `TASKS.md` + `docs/` como contexto.
5. **Persistencia**: el resultado debe quedar en el árbol, no en el chat.
6. **Verificación**: el agente debe dejar los tests en verde.

## Regla

El proyecto **no** se toca para "hacer que pase": el bug de `src/lib.rs` está
plantado a propósito y `TASKS.md` dice cuál es. Si el agente "arregla" el test
en vez del código, la prueba del arnés debe fallar.

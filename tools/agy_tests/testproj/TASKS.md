# TAREAS (guion de pruebas del agente)

Cada tarea es un caso de uso real de Antigravity. La suite comprueba que el
agente las puede completar contra este proyecto.

- [ ] **T-A. Diagnóstico**: `src/lib.rs` tiene un bug de lógica (no de tipos).
      Encuentra la causa raíz mirando `tests/`, **no** cambiando los tests.
      Escribe la causa en `docs/BUGFINDING.md`.
- [ ] **T-B. Arreglo**: corrige `src/lib.rs` para que `cargo test` pase.
      No toques `tests/`.
- [ ] **T-C. Tooling**: `python/cli.py` llama a la lógica de Rust y falla con
      una entrada concreta. Arreglalo sin cambiar la interfaz pública.
- [ ] **T-D. Documentación**: actualiza `docs/ARCHITECTURE.md` con el cambio.
- [ ] **T-E. Verificación**: `cargo test` y `python3 cli.py` en verde, y deja
      constancia en `docs/VERIFICATION.md` (comando + salida real).

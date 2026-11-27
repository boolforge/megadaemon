# Arquitectura de testproj

`src/lib.rs` es una biblioteca pura sin dependencias. `python/cli.py` la usa
como caja negra: compila un auxiliar con `rustc` y llama a la biblioteca.

Regla de oro del proyecto: **la interfaz pública de `src/lib.rs` no cambia**
(`normalize`, `argmax`, `moving_average`). Si hace falta corregir algo, se
corrige la implementación, no la firma.

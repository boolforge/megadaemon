# Go Reverse Engineering — revisión 2.0 y workspace Rust

Este paquete consolida el documento técnico anterior, corrige los defectos editoriales señalados y añade la especificación de `moduledata` multiproducto, `pcvalue`, GC maps/ABIInternal y `//go:embed`.

## Contenido

- `go_reverse_engineering_definitive_2026-11-22.md`: documento maestro, 19.9k palabras aprox., con referencias URL en texto plano y registro de correcciones.
- `go-retool/`: workspace Rust con parser conservador `pclntab`, tipos de offsets/direcciones, `pcvalue`, arrays variables de `_func`, walker de cadena de módulos, stackmap decoder, embed record decoder, CLI, esquema JSON, importador Ghidra, fixtures y matriz de implementación.
- `go-retool/artifacts/`: binarios fixture Go 1.23.2 Linux/amd64, Linux/arm64, Windows/amd64 y Darwin/arm64, además del plugin y host Linux/amd64.
- `go-retool/validation/`: salida del fixture, metadata de build, SHA-256 y comprobación estructural independiente.

## Estado de verificación (importante)

- **Go fixture:** compilado y ejecutado con Go 1.23.2 linux/amd64.
- **Plugin fixture:** compilado como `.so`, cargado por un host Go y ejecutado correctamente.
- **PCLNTAB fixtures:** el checker Python validó cuatro targets Go 1.23.2: Linux/amd64 1.889/1.889; Linux/arm64 1.887/1.887; Windows/amd64 1.939/1.939; Darwin/arm64 1.946/1.946. Cero errores estructurales en cada uno.
- **Rust:** código y tests fuente incluidos, pero no compilados ni ejecutados en el entorno de generación porque no estaban instalados `rustc` ni `cargo`.
- **Go 1.27 V5:** no se declara verificado. La implementación actual no incluye `.go.type`, itabs ni parsing completo de `moduledata`.

No presentar los puntos no verificados como resultados de laboratorio. Las limitaciones se describen en el documento y en `go-retool/README.md`.

# go-retool — conservative Go metadata research implementation

This repository is a research implementation and reproducible fixture kit, not a finished Go 1.27 reverse-engineering suite.

## Repository map

- `crates/go-retool/`: Rust workspace, library, CLI and unit-test source.
- `fixtures/`: Go fixture source with interfaces, generics, closures, goroutines, reflection and three `//go:embed` forms; separate plugin and plugin host.
- `artifacts/`: Go 1.23.2 Linux/amd64 fixture executable, plugin `.so`, and plugin host executable.
- `validation/`: independent structural-check report, runtime output, build metadata and SHA-256 manifest.
- `ghidra/ImportGoRetoolJson.py`: conservative JSON importer; imports only explicitly resolved VAs.
- `schemas/analysis.schema.json`: versioned JSON schema.
- `docs/IMPLEMENTATION_MATRIX.md`: exact implemented/fixture-checked/research-only status.
- `scripts/`: fixture builder and independent Python `pclntab` sanity checker.
- `docs/`: additional implementation notes.

## Implemented

- Distinct Rust newtypes for file offsets, RVAs, VAs, Go text offsets, function offsets and name offsets.
- Read-only ELF/PE/Mach-O container inspection through the Rust `object` crate, section file-range metadata, and file-offset-to-section-VA mapping for file-backed section bytes.
- `scan` CLI command that scans for plausible Go 1.18/1.20 `pclntab` candidates and reports the `pcHeader` container section address separately from the Go text base (PE reports RVA; ELF/Mach-O report their section address domain).
- Conservative scan for plausible modern `pclntab` candidates in executable containers; reports the `pcHeader` file offset and its container section address separately from the Go text base (PE reports RVA; ELF/Mach-O report their section address domain).
- Bounds-checked little-endian reader with checked offset arithmetic.
- Conservative parser for Go 1.18+ modern `functab` (two `u32` fields per record), accepting the Go 1.18/1.20 magic family; follows `funcoff` to `_func.nameOff` and resolves names through `funcnametab`.
- `_func` fixed-prefix extraction plus raw `pcdata[]` and `funcdata[]` offset arrays.
- Optional explicit text-base mapping. Without it, `start_va` and `end_va` serialize as JSON `null`; entry offsets remain available.
- Bounded `pcvalue` decoder implementing uvarint32, zig-zag value deltas, PC delta × configurable quantum, and the post-first-record zero terminator. Zero PC-delta state updates are retained semantically without emitting empty ranges.
- Generic cycle-safe `moduledata.next` chain walker; the version-specific node reader is caller-supplied.
- `stackmap` bitmap decoder with runtime-compatible LSB-first bit indexing.
- `embed.file` record decoder for a caller-supplied mapped memory image and record-array VA; no discovery or hash-algorithm verification.
- Versioned JSON output with `modules: []` and nullable VA/text-section mapping fields.
- Ghidra script that imports only resolved addresses and preserves non-default/user-defined symbols.
- Go fixture with interfaces, generics, closure, goroutine, reflection, `embed.FS`, embedded string and bytes; separate plugin fixture and host.

## Explicitly not implemented

- Relocation-aware mapping for ELF/PE/Mach-O, Wasm parsing, load-bias/ASLR resolution and secondary text-section mapping; the current `object` adapter reports file-backed section ranges and section VAs only.
- Automatic `pclntab` candidate scanning exists for little-endian Go 1.18/1.20 magic-family formats; deeper validation for all Go versions, moduledata scanning, runtime memory capture and plugin-chain node decoding are not implemented.
- Full Go 1.27 V5 semantic/type/itab parser, `.go.type` enumeration and type graph.
- `pcdata` stream integration and `funcdata` payload resolution against `moduledata.gofunc`; stackmap bitmap decoding exists, but runtime PC-index selection and `ArgInfo` decoding do not.
- Automatic `embed.FS` discovery, slice-header parsing, string/[]byte extraction and hash verification; the record decoder requires a caller-supplied mapped image and array address.
- DWARF parsing, CFG matching, Garble recovery, patching, binary rewriting or execution of untrusted samples.

## Export de simbolos y anclajes (medido 2026-11-22)

`go-retool export` vuelca la pclntab en formatos que consumen Ghidra y Rizin.
La direccion es la REAL (base de texto + `entryoff`) y el nombre es el REAL de
Go (no el mangled del ELF).

```sh
# TSV de simbolos (7 columnas: va, nombre, fichero:linea, va_fin, paquete,
# tamano, entryoff)
go-retool export tsv  liblanguage_server.so --offset 0x1449ff0 --out symbols.tsv

# JSON autocontenido (simbolos + anclajes)
go-retool export json liblanguage_server.so --offset 0x1449ff0 --out symbols.json \
  --anchors anchors.tsv

# Los anclajes cruzados con los simbolos (paquete, fichero, linea, tamano)
go-retool export tsv liblanguage_server.so --offset 0x1449ff0 --out symbols.tsv \
  --anchors anchors.tsv --anchors-out anchors.full.tsv
```

MEDIDO con el binario real de Antigravity (`liblanguage_server.so`,
181.358.800 B, sha256 `f25135e4…70ff27`, Go 1.28 RC02, pclntab en `.lrodata`
@ 0x1449ff0, base de texto 0x4d06cb8):

- 161.682 funciones exportadas; 161.680 con fichero y linea; 161.682 con tamano.
- Las 161.682 direcciones coinciden EXACTAMENTE con la tabla de referencia
  `go.funcs.tsv` (0 diferencias de direccion, 0 diferencias de nombre real,
  2 diferencias de `fichero:linea`: `go:textfipsstart` y `go:textfipsend`, que
  en la referencia tampoco tienen fichero).
- TSV completo (43.809.869 B): sha256
  `e9168425d407d4b7fc1228c1d6ff1f4ce657c4abde0617af8c71340867b033d7`.
- JSON completo (60.065.217 B): sha256
  `0e46f6197a319ac70ad332ac1667319520c9c6a034270b6f28fd61fd6f15934f`.

### Anclajes

Un anclaje es una *funcion hito* elegida para decompilar a mano: un punto de
observacion del binario.  Las familias son `entry` (los dos puntos de
entrada), `mcp` (el SDK MCP de Google, transporte del language server) y
`provider` (los clientes de modelo).  El TSV de anclajes tiene cinco columnas:
`0x<va>`, nombre de simbolo, nombre real de Go, `fichero:linea` y etiqueta.

## Build and test

Requires Rust 1.85+ and Cargo (the `object` 0.41 dependency raises the MSRV). The generation environment did not contain `rustc` or `cargo`; therefore Rust source and tests could not be compiled or executed here. The Go 1.23.2 Linux/amd64 fixture was built and run; the plugin was loaded by its host. An independent Python structural checker validated four Go 1.23.2 targets: Linux/amd64 1,889/1,889, Linux/arm64 1,887/1,887, Windows/amd64 1,939/1,939, and Darwin/arm64 1,946/1,946 names resolved, with zero structural errors in each. These are Python checker results, not Rust parser results, and do not validate Go 1.27 support.

On a machine with Rust:

```sh
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p go-retool -- pclntab ./sample --offset 0xOFFSET --out analysis.json
```

The `--offset` value is the file offset where `pcHeader` begins, not a section RVA. Do not pass `--text-base` unless independently established from a valid `moduledata.text`/text-section mapping. For binaries with secondary text sections, this MVP cannot apply `textsectmap` and must leave VAs unresolved.

Build fixtures using the selected Go toolchain:

```sh
./scripts/build-fixtures.sh
BUILD_PLUGIN=1 ./scripts/build-fixtures.sh
```

Use `GO_BIN=/path/to/go1.xx.y/bin/go` to select an exact compiler. The script reports the actual `go version`; it does not pretend a label changes the installed compiler.

Validate a built fixture independently:

```sh
python3 ./scripts/validate_fixture.py artifacts/fixture-go1.23.2-linux-amd64
```

## Safety

Input is read-only. No patch command exists. A magic check is not proof of full version compatibility: magic identifies a format family, not every semantic detail of a Go release. No Go 1.27 V5 compatibility claim should be made until pinned Go 1.27 fixtures and differential tests pass.

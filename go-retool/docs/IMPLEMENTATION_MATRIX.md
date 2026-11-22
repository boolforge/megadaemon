# Implementation matrix and evidence status

| Component | Source exists | Unit/fixture evidence | Known limit |
|---|---|---|---|
| Safe address domains | Yes | Type definitions reviewed; Rust tests not run | Conversions are intentionally explicit; no full segment model yet |
| Bounds-checked reader | Yes | Used by parser; Rust tests not run | Little-endian only |
| ELF/PE/Mach-O inspection | Yes (`object` 0.41) | Not compiled in this environment | Section metadata only; no relocations/load-bias/ASLR |
| `pclntab` candidate scan | Yes | Independent Python checker validated 4 Go 1.23.2 fixtures | Rust scan code not compiled; magic family/layout scope limited |
| Go 1.18+ `functab` | Yes | Python checker: 4 targets, zero structural errors | Rust implementation not run; no full Go 1.27 certification |
| `_func` prefix and arrays | Yes | Rust unit test source only | `pcdata`/`funcdata` offsets are exported raw; payload bases not resolved |
| `pcvalue` decoder | Yes | Rust unit test source only | Not differentially tested against real streams yet |
| `moduledata.next` walker | Yes | Rust unit test source only | Generic walker only; no release-specific node parser or live memory capture |
| GC stackmap bitmap | Yes | Rust unit test source only | No PCDATA selection, ArgLiveInfo, or ArgInfo decoding |
| `embed.file` record decoder | Yes | Rust unit test source only | Caller supplies mapped image and array VA; no discovery/hash verification; little-endian only |
| Ghidra JSON importer | Yes | Manual API review; not run in Ghidra | Only imports resolved addresses; no plugin runtime VA reconciliation |
| Go fixture | Yes | Built/run on Go 1.23.2 Linux/amd64 | Not a Go 1.27 fixture |
| Plugin fixture | Yes | Built, loaded and called by host on Linux/amd64 | Same-toolchain/platform plugin constraints apply |
| Cross-target fixtures | Yes | Python checker passed Linux/amd64, Linux/arm64, Windows/amd64, Darwin/arm64 | Cross-built binaries were not executed on foreign OS/architectures |
| Go 1.27 V5 types/itabs | No | Not tested | Research/specification only |
| Patching/writer | No | Not tested | Deliberately absent; original binaries remain read-only |

## Evidence labels

- **Implemented:** source code exists.
- **Fixture-checked:** an executable/fixture was built or run and an independent check passed.
- **Unit-test source only:** tests are included but Rust/Cargo could not be run here.
- **Research only:** documented design, no implementation claim.

Never promote a row to a stronger status without recording the exact compiler release, GOOS/GOARCH, flags, sample hash, test command, and output.

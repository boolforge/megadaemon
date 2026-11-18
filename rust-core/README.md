# `megadaemon-core` v13.0.0 Zenith — 26-Module Pure-Rust ARM64 RE Engine, Swarm Router & MCP Server

Native ARM64 (`aarch64-linux-android`) binary implementing MegaDaemon v13.0 Zenith with **zero Python runtime dependencies** on the Web Cockpit and Chat hot paths.

## CLI Subcommands
- `megadaemon-core cockpit [port]`: Launch 100% Pure-Rust Multi-Threaded Cockpit HTTP Server (default: `3000`).
- `megadaemon-core mcp-server`: Run Antigravity JSON-RPC 2.0 MCP Server over stdio (`minimax_chat`, `swarm_chat`, `megadaemon_die_binwalk`, `megadaemon_rizin`, `megadaemon_cirx_analyze`, `megadaemon_mba_simplify`, `megadaemon_npu_cv`).
- `megadaemon-core chat <model_id> <prompt>`: Query MiniMax M2.7/M2.5, GitHub Models, Pollinations, Free Swarm, Hybrid, or Local SEERE v12.
- `megadaemon-core die-binwalk <file>`: Run Detect It Easy + Binwalk + Scalpel + Bulk Extractor + Comtype Scanner (`memmap2`).
- `megadaemon-core cirx-ssa [mas]`: Execute LTO-Decomp v3.4 CIRX Micro-RTL SSA, Risk Score §6.1, MAS Track Routing, Closure Lemma §15.2, and SMT-LIB 2 export.
- `megadaemon-core mba <expr>`: Simplify obfuscated MBA expressions and opaque predicates (D810/Goomba).
- `megadaemon-core kaitai <file> [template] [offset]`: Evaluate ImHex/Kaitai binary struct templates.
- `megadaemon-core rizin <file> [mode] [arch]`: Run native ARM64 Rizin 0.9.1 analysis (zero JVM).
- `megadaemon-core frida <mode> <module> <sym_or_off> [proc]`: Generate Frida DBI scripts (`attach`, `stalker`, `native_call`, `android_jni`, `tenet_mem`).
- `megadaemon-core npu-cv` / `npu-probe`: Probe Exynos 2100 NPU, Mali-G78, KleidiAI, CPU thermals, and VLA CV ROIs.
- `megadaemon-core unifier` | `rea` | `cfg` | `bindiff` | `hw-mmio` | `crypto-scan` | `seere` | `seere-selftest` | `locate-roms` | `disasm` | `hex` | `deobfuscate` | `dumper` | `entropy` | `pattern` | `pointers` | `diff` | `shm-status` | `shm-frame-bmp`.

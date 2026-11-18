# `megadaemon-core` v11.0.0 — Native ARM64 Rust Engine & Cockpit Server

Native ARMv8.2-A (`aarch64-linux-android`) binary implementing the **MegaDaemon Cockpit HTTP Server**, **SEERE v11 GBNF Harness**, **Exynos 2100 NPU / EDEN / NNAPI / KleidiAI Hardware Accelerator Bridge**, and **Multi-Architecture Reverse Engineering Suite**.

## Modules
- `src/cockpit_server.rs`: Multi-threaded HTTP server (`127.0.0.1:3000`) serving `web/cockpit_template.html` and 35+ `/api/*` endpoints.
- `src/seere_v11.rs`: SEERE v11 deterministic harness with `OP_SPEC` single source of truth, typed GBNF grammars (`state/seere.gbnf`), 100% static KV-pinned prompts (`state/seere_prompt_*.txt`), bounded math AST evaluator, literal grounding verifier, and raw TCP OAI client (`127.0.0.1:8080/v1/chat/completions`).
- `src/npu_bridge.rs`: Dynamic `dlopen`/`dlsym` bridge to `/vendor/lib64/libeden_wrapper.so`, `/vendor/lib64/libeden_ud_npu.so`, `/system/lib64/libneuralnetworks.so`, `/dev/vertex10` (VS4L NPU), and `libggml-cpu.so` (`ggml_cpu_has_dotprod`), with automatic `sched_setaffinity` pinning to Exynos 2100 performance cores `4..=7` (3× Cortex-A78 + 1× Cortex-X1).
- `src/disasm.rs`: Native disassembler & hex inspector for MIPS R3000A (PS1), Motorola 68000 (Sega CD), Hitachi SH-2 (Saturn), Zilog Z80 (MSX2/PC-8801), and Hudson HuC6280 (PC Engine).
- `src/rom_locator.rs`: Recursive thread-safe ROM & binary auto-discovery.
- `src/deobfuscate.rs`: Konami XOR script deobfuscator and asset signature scanner.
- `src/freezer.rs`: 60 Hz background memory lock manager.
- `src/entropy.rs`, `src/pattern.rs`, `src/pointers.rs`, `src/snapshot.rs`, `src/shm.rs`: Shannon entropy, wildcard AOB pattern scanner, 32-bit pointer table miner, snapshot diff engine, and zero-copy 1 MB VLA SHM bus.

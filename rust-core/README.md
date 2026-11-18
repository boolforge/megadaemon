# `megadaemon-core` v12.0.0 Omega — Native ARM64 Rust Mega-Unifier, REA Assistant & Cockpit Server

Native ARMv8.2-A (`aarch64-linux-android`) binary implementing the **MegaDaemon Cockpit HTTP Server**, **SEERE v12 GBNF Harness (15 Typed Opcodes including `D`, `F`, `X`)**, **REA (Reverse Engineering Assistant) Ghidra 2.71 MB Corpus Indexer**, **CFG Lifter**, **Diaphora Multi-ISA BinDiff**, **Hardware MMIO & Auto-Struct Mapper**, **Shift-JIS / Konami LZSS Scanner**, **Universal 9-Project Ecosystem Unifier**, and **Exynos 2100 NPU / EDEN / NNAPI / KleidiAI Hardware Accelerator Bridge**.

## CLI Subcommands
- `megadaemon-core cockpit [port]`: Launch Native Multi-Threaded Rust Cockpit HTTP Server (default: `3000`).
- `megadaemon-core unifier`: Audit and unify all 9 projects across `/data/user/0/com.agy/files/`.
- `megadaemon-core rea <plat> [query] [max]`: Index Ghidra `.c` corpus, classify semantics, annotate MMIO, and transpile to ScummVM C++.
- `megadaemon-core cfg <plat> [offset] [count]`: Lift Control Flow Graph (Basic Blocks, Edges, McCabe Complexity, Mermaid diagram, Lifted C++).
- `megadaemon-core bindiff <plat>`: Run Diaphora-style Multi-ISA Cross-Platform Function Matcher across all 6 retro architectures.
- `megadaemon-core hw-mmio <plat>`: Annotate hardware MMIO registers and reconstruct inferred C `struct` definitions.
- `megadaemon-core crypto-scan <plat>`: Decode Shift-JIS Japanese strings to UTF-8 and detect compressed Konami LZSS/RLE blocks.
- `megadaemon-core seere <query>`: Execute SEERE v12 GBNF KV-Pinned Harness in pure Rust.
- `megadaemon-core seere-selftest`: Run SEERE v12 assertion suites and export shared `seere.gbnf` & `seere_prompt_*.txt`.
- `megadaemon-core npu-probe`: Probe Exynos 2100 NPU (`/dev/vertex10`, `libeden_wrapper.so`, `libneuralnetworks.so`, KleidiAI SDOT) and pin Cores 4–7.

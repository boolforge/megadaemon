# MegaDaemon v12.0 Omega — The Mega-Unifier, Native Rust REA (Reverse Engineering Assistant), SEERE v12 GBNF & Exynos 2100 NPU Bridge

**MegaDaemon v12.0 Omega** unifies the entire **9-project Snatcher & Retro Reverse Engineering Ecosystem** on Android ARMv8.2-A (`aarch64-linux-android`, Samsung Exynos 2100) into a single **18-module Native Rust Engine (`megadaemon-core`)**.

---

## 1. Unified 9-Project Ecosystem (`src/unifier.rs`)

MegaDaemon v12.0 Omega directly bridges, indexes, and orchestrates:
1. **`megadaemon/`**: Core v12.0 Omega Rust Server (`:3000`), SEERE v12 GBNF KV-Pinned Harness, and Exynos 2100 NPU FFI Bridge.
2. **`re-research/`**: 7 clean extracted CPU executables (`ps1_slps.bin`, `saturn_main.bin`, `scd_subcode.bin`, `pce_overlay.bin`, `pce_boot.bin`, `msx2_com.bin`, `pc88_boot.bin`) and **2.71 MB of Ghidra Decompiled C & ASM Corpus** (`ps1.c` 935 KB, `scd_subcode.c` 44 KB, `msx2_com.c` 59 KB).
3. **`dumper-web/`**: Universal media & container extractor (`dumper-companion.py` 152 KB for HFS+, ISO9660, CUE/BIN, ECM, Shift-JIS) and multi-platform converter.
4. **`snatcher-tools/`**: Multi-platform script, font, and sector patching suite for PC Engine, PlayStation 1, and Sega Saturn.
5. **`Sega-CD-Snatcher-Language-Patcher/` & `Sega-Mega-CD-Spatcher/`**: M68K Sub-CPU and ISO language patchers.
6. **`scummvm/`**: Target `ScummVM::SnatcherEngine` C++ implementation and 16-phase verification pipeline.
7. **`libchdr/`**: Native C/C++ MAME Compressed Hunks of Data (`.chd`) optical disc decompressor.
8. **`client-seere`, `client-malphas`, `client-ukobach`**: Triad of native 60 FPS Rust TUI/CLI demon clients.
9. **`seere-android`, `seere-gpui-apk`, `gpui-mobile`**: Native Android & GPUI mobile client suites.

---

## 2. The 18 Native Rust Modules (`rust-core/src/`)

| Module | Engineering Role |
| :--- | :--- |
| **`cockpit_server.rs`** | Multi-threaded Native Rust HTTP Server (`127.0.0.1:3000`) serving `web/cockpit_template.html` and 45+ `/api/*` endpoints. |
| **`seere_v11.rs` (v12)** | SEERE v12 Deterministic Harness with `OP_SPEC` Single Source of Truth, 15 typed GBNF opcodes (including RE opcodes **`D` `DISASM_CFG`**, **`F` `GHIDRA_FUNC`**, **`X` `XPLAT_BINDIFF`**), 100% static KV-pinned prompts, bounded AST math, and raw TCP OAI client. |
| **`rea_assistant.rs`** | **REA (Reverse Engineering Assistant)**: Indexes the 2.71 MB Ghidra `.c` corpus, classifies function semantics, annotates MMIO registers, suggests symbol names, and transpiles to `ScummVM::SnatcherEngine` C++. |
| **`cfg_lifter.rs`** | **Control Flow Graph (CFG) Lifter**: Recovers Basic Blocks, branch/fallthrough/call edges, McCabe Cyclomatic Complexity $M = E - N + 2$, Mermaid flowcharts, and C++ state machines from raw binaries. |
| **`bindiff.rs`** | **Diaphora-Style Multi-ISA BinDiff**: Matches equivalent game engine functions across MIPS R3000A (PS1), M68K (Sega CD), SH-2 (Saturn), HuC6280 (PCE), and Z80 (MSX2/PC-88) using ISA-invariant topological vectors. |
| **`hw_mapper.rs`** | **Hardware MMIO & Auto-Struct Mapper**: Annotates physical hardware registers (PS1 GPU/SPU/CDROM/MDEC, Sega CD Gate Array/Word RAM/PCM RF5C164, Saturn VDP1/VDP2/SCU/SCSP, PCE HuC6270/VCE, MSX2 V9938/OPNA) and reconstructs C `struct` layouts. |
| **`crypto_scan.rs`** | **Shift-JIS & Konami LZSS/RLE Scanner**: Decodes Japanese Shift-JIS strings (Hiragana, Katakana, Kanji) to UTF-8 in pure Rust and detects compressed Konami LZSS/RLE data streams. |
| **`unifier.rs`** | **Universal Ecosystem Unifier**: Audits and bridges all 9 projects across the device filesystem. |
| **`npu_bridge.rs`** | **Exynos 2100 NPU FFI Bridge**: Dynamic `dlopen`/`dlsym` to `/vendor/lib64/libeden_wrapper.so`, `/vendor/lib64/libeden_ud_npu.so`, `/system/lib64/libneuralnetworks.so`, `/dev/vertex10` (VS4L 26-TOPS NPU), and `libggml-cpu.so` (Arm KleidiAI SDOT), pinning inference threads to Cores 4–7. |
| **`disasm.rs`** | Multi-architecture disassembler & hex inspector for MIPS R3000A, Motorola 68000, Hitachi SH-2, Zilog Z80, and Hudson HuC6280. |
| **`rom_locator.rs`** | Thread-safe ROM & pure binary locator linking each platform to its `re-research/bin/` executable and `re-research/out/` Ghidra corpus. |
| **`deobfuscate.rs`** | Konami XOR script deobfuscator and asset signature scanner. |
| **`freezer.rs`** | Native 60 Hz (`16.6 ms`) background memory lock manager. |
| **`entropy.rs` \| `pattern.rs` \| `pointers.rs` \| `snapshot.rs` \| `shm.rs`** | Shannon entropy classifier, wildcard AOB pattern scanner, 32-bit pointer miner, memory snapshot differential engine, and 1 MB zero-copy VLA SHM bus. |

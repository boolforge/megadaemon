# MegaDaemon v13.0 Zenith — 100% Pure-Rust Multi-OS Reverse Engineering Workbench, LTO-Decomp v3.4 CIRX, Parasitic Multi-LLM Swarm & Exynos 2100 NPU/CV Engine

**MegaDaemon v13.0 Zenith** unifies the entire **9-project Reverse Engineering Ecosystem** plus **LTO-Decomp v3.4 (`CIRX Micro-RTL/SSA`)** on Android ARM64 (`aarch64-linux-android`, Samsung Exynos 2100) into a single **26-module Native Rust Engine (`megadaemon-core`)** with **zero Python runtime shell-outs** on the Cockpit or Chat hot paths.

---

## 1. Key Capabilities (v13.0 Zenith)

- **100% Pure-Rust Web Cockpit (`127.0.0.1:3000`) & Mobile-First `ui-ux-pro` UI:** Obsidian Phoenix palette, zero structural emojis, WCAG AAA contrast, fixed bottom 4-tab thumb navigation (`SWARM`, `INSPECT`, `VRAM`, `SYSTEM`), and **unrestricted file uploads up to 100 MB** (`.7z`, `.exe`, `.dll`, `.apk`, `.dex`, `.elf`, `.so`, `.macho`, `.com`, `.md`, `.py`, `.rs`, `.bin`, `.iso`).
- **Parasitic Multi-LLM Swarm (`src/providers.rs`):** Native Rust router supporting **MiniMax M2.7 / M2.7-highspeed / M2.5 / M2.5-highspeed**, **GitHub Models (Free via `gh` CLI)**, **Pollinations (Zero-Key)**, **OpenRouter `:free`**, **Groq Free**, **Cerebras Free**, **Google AI Studio Free**, **Mistral Free**, **Hybrid (Rust RE Tools + Cloud)**, and **Local SEERE v12 (Offline GBNF)**. API keys live strictly in gitignored `state/providers.json`.
- **Native Antigravity MCP Server (`src/mcp_server.rs`):** JSON-RPC 2.0 stdio server (`megadaemon-core mcp-server`) exposing `minimax_chat`, `swarm_chat`, `megadaemon_die_binwalk`, `megadaemon_rizin`, `megadaemon_cirx_analyze`, `megadaemon_mba_simplify`, and `megadaemon_npu_cv` directly to Antigravity.
- **LTO-Decomp v3.4 CIRX Engine (`src/cirx_rtl.rs` + `lto-decomp/`):** Integrates `cirx_micro_rtl_v2.rs` (concrete Micro-RTL interpreter, multi-block SSA via Cooper-Harvey-Kennedy dominators & Cytron dominance frontiers, SMT-LIB 2 `QF_BV` overlap emitter) with normative Risk Score ($S = 8M + 1000P + 20A + 50L + 200I$, $R = \min(100, \lfloor S/50 \rfloor)$), MAS Track Routing, Absolute Set Closure Lemma ($A = \{\text{formal\_control\_flow}\}$), Conformal Quantile, Flag Polarity verification, and Obligations `O-1..O-8`.
- **Elite Multi-OS Reverse Engineering Engines:**
  - **Detect It Easy (DiE) + Binwalk + Unblob (`7zz`) + Scalpel + Bulk Extractor + Comtype Scanner (`src/binwalk_die.rs`)** via zero-copy `memmap2` covering Windows PE/PE32+, Linux/Android ELF/DEX/APK, macOS Mach-O/Fat, DOS MZ/COM, archives, and retro ROMs.
  - **Tile Molester / YY-CHR / CrystalTile2 Native VRAM Engine (`src/tile_vram.rs`)** rendering `1bpp`, `2bpp_planar`, `4bpp_linear`, `4bpp_planar`, `8bpp_linear`, and `15bpp_rgb555` with CRAM/CLUT palettes synced to Hex Inspector.
  - **ImHex / Kaitai Struct Parser + D810/Goomba MBA Deobfuscator (`src/kaitai_imhex.rs`)**.
  - **Rizin 0.9.1 Zero-JVM Multi-OS Bridge + Frida DBI Generator + Root Process/Maps Inspector (`src/frida_dbi.rs`)**.
  - **Exynos 2100 NPU + Mali-G78 + Native Rust VLA Computer Vision (`src/npu_cv.rs`, `src/npu_bridge.rs`)**.

---

## 2. The 26 Native Rust Modules (`rust-core/src/`)

| Module | Engineering Role |
| :--- | :--- |
| `cockpit_server.rs` | 100% pure-Rust multi-threaded HTTP server (`:3000`, 50+ `/api/*` endpoints, 100 MB unrestricted uploads). |
| `providers.rs` | Parasitic Multi-LLM Swarm router, gitignored API vault, `<think>` parser, automatic failover cascade. |
| `mcp_server.rs` | JSON-RPC 2.0 stdio MCP server for Antigravity MiniMax & RE tool integration. |
| `binwalk_die.rs` | Multi-OS DiE + Binwalk + Scalpel + Bulk Extractor + Comtype + `7zz` extraction (`memmap2`). |
| `tile_vram.rs` | Tile Molester / YY-CHR / CrystalTile2 multi-bpp & hardware palette BMP renderer. |
| `kaitai_imhex.rs` | Declarative ImHex/Kaitai binary struct parser & D810/Goomba MBA algebraic simplifier. |
| `cirx_rtl.rs` | LTO-Decomp v3.4 CIRX Micro-RTL, formal SSA builder/checker, Risk Score, MAS Track, SMT-LIB 2. |
| `frida_dbi.rs` | Rizin 0.9.1 native bridge, Frida DBI script generator (`Interceptor`/`Stalker`/`NativeFunction`/`JNI`/`Tenet`), root procfs maps. |
| `npu_cv.rs` / `npu_bridge.rs` | Exynos 2100 NPU (`/dev/vertex10`, EDEN `dlopen`), KleidiAI SDOT, Cores 4–7 pinning, VLA CV delta/Sobel/ROIs. |
| `seere_v11.rs` | SEERE v12 GBNF KV-pinned local harness with live Swarm `@` delegation & RE opcodes (`D`, `F`, `X`). |
| `rea_assistant.rs` / `cfg_lifter.rs` / `bindiff.rs` / `hw_mapper.rs` / `crypto_scan.rs` | Ghidra 2.71 MB corpus indexer, CFG lifter, Multi-ISA BinDiff, Hardware MMIO annotator, Shift-JIS/LZSS scanner. |
| `unifier.rs` / `rom_locator.rs` / `disasm.rs` / `deobfuscate.rs` / `freezer.rs` / `entropy.rs` / `pattern.rs` / `pointers.rs` / `snapshot.rs` / `shm.rs` | 9-project ecosystem unifier, multi-arch disassembler, 60 Hz memory freezer, AOB/entropy/pointer/diff engines, and zero-copy SHM bus. |

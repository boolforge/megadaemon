# MegaDaemon v11.0 Apex — Native Rust Reverse Engineering Cockpit, SEERE v11 GBNF Harness & Exynos 2100 NPU Bridge

**MegaDaemon v11.0 Apex** is a hybrid Rust + Python autonomous reverse engineering, multi-architecture decompilation, differential hardware debugging, and on-device cognitive agent platform engineered for **ARMv8.2-A (`aarch64-linux-android`, Samsung Exynos 2100 / S21 Ultra)**.

---

## 1. Architecture & Core Modules

### 1.1 `rust-core/` (`megadaemon-core` v11.0.0 — Pure Native ARM64 Rust)
Compiled with `opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`:
- **`src/cockpit_server.rs`**: Native multi-threaded Rust HTTP server (`127.0.0.1:3000`) serving the mobile-responsive WebGL Matrix Cockpit (`web/cockpit_template.html`) and all 35+ `/api/*` endpoints with sub-millisecond latency.
- **`src/seere_v11.rs`**: **SEERE v11 Hyper-Parasitic Deterministic Harness** in pure Rust:
  - **Single Source of Truth (`OP_SPEC`)**: Generates typed GBNF grammars (`state/seere.gbnf`, `seere_final.gbnf`, `seere_bin.gbnf`), 100% static KV-pinned system prompts (`state/seere_prompt_es.txt`, `seere_prompt_en.txt`), and the tool dispatcher in lockstep.
  - **0-Token Deterministic Short-Circuits**: Bounded AST/recursive-descent math evaluator (immune to `9**9**9` exponent bombs), direct `$ <cmd>` root execution (`su -c` / `rish -c` / `sh -c`), and literal grounding verifier (`verify_grounding`).
  - **KV-Pinned Raw TCP OAI Client**: Communicates directly with `llama-server` (`127.0.0.1:8080/v1/chat/completions`) with `cache_prompt: true`, `id_slot: 0`, and phase-specific GBNF grammars.
- **`src/npu_bridge.rs`**: **Exynos 2100 NPU, Samsung EDEN, Android NNAPI & Arm KleidiAI Hardware Accelerator Bridge**:
  - Dynamically probes `/dev/vertex10` (Vision4Linux VS4L 26-TOPS NPU node).
  - Uses `libc::dlopen` & `libc::dlsym` (with automatic Magisk `su` Bionic vendor linker namespace bypass `LD_LIBRARY_PATH=/vendor/lib64:/system/lib64`) to inspect `/vendor/lib64/libeden_wrapper.so` (`Initialize`, `GetEdenVersion`, `OpenModelFromMemory`, `AllocateInputBuffers`, `ExecuteModel`, `FreeBuffers`, `CloseModel`, `Shutdown`), `/vendor/lib64/libeden_ud_npu.so` (`GetDrvInfo`, `GetDeviceHealthState`), `/system/lib64/libneuralnetworks.so`, and `libggml-cpu.so` (`ggml_cpu_has_dotprod`).
  - Pins inference threads to the Exynos 2100 performance cluster (`Cores 4-7`: 3× Cortex-A78 + 1× Cortex-X1) via `libc::sched_setaffinity`.
- **`src/disasm.rs`**: Native multi-architecture disassembler & hexadecimal inspector decoding **MIPS R3000A** (PS1), **Motorola 68000** (Sega CD), **Hitachi SH-2** (Sega Saturn), **Zilog Z80** (MSX2 / PC-8801), and **Hudson HuC6280** (PC Engine).
- **`src/rom_locator.rs`**: Thread-safe recursive ROM & binary auto-discovery across Android shared storage.
- **`src/deobfuscate.rs`**: Native Konami XOR dialogue script deobfuscator (`SnatcherEngine::decodeDialogueSector` C++ synthesizer) and asset signature extractor.
- **`src/freezer.rs`**: Native 60 Hz (`16.6 ms`) background memory lock manager.
- **`src/entropy.rs` | `src/pattern.rs` | `src/pointers.rs` | `src/snapshot.rs` | `src/shm.rs`**: Shannon entropy block classifier, wildcard AOB pattern scanner, 32-bit pointer table miner, memory snapshot differential engine, and 1 MB zero-copy VLA SHM bus renderer.

### 1.2 `core/` & `re_pipeline/` (Python Interop & Autonomous Daemons)
- **`re_pipeline/parasite_llm_router.py` & `core/router.py`**: Python SEERE v11 harness sharing the exact same `OP_SPEC`, typed GBNF grammars, and static KV-pinned prompts as `rust-core`.
- **`core/rust_bridge.py`**: Zero-latency Python bridge to `megadaemon-core`.

---

## 2. Quick Start & CLI Reference

```bash
# 1. Build Native Rust Core v11.0
cargo build --release --manifest-path rust-core/Cargo.toml

# 2. Run SEERE v11 Assertion Suites (Rust & Python)
./rust-core/target/release/megadaemon-core seere-selftest
python3 re_pipeline/parasite_llm_router.py --selftest

# 3. Probe Exynos 2100 NPU (/dev/vertex10, EDEN FFI, KleidiAI) & Pin Cores 4-7
./rust-core/target/release/megadaemon-core npu-probe

# 4. Launch Native Rust Cockpit Server on Port 3000
./rust-core/target/release/megadaemon-core cockpit 3000
```

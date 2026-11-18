# MegaDaemon Core Compatibility Layer (`core/`)

In **MegaDaemon v13.0 Zenith**, all hot-path runtime logic (Cockpit HTTP Server, Multi-LLM Parasitic Swarm Router, SEERE v12 Harness, DiE/Binwalk, Tile Molester VRAM, Kaitai/MBA, CIRX v3.4 SSA, Rizin/Frida, Exynos NPU/CV, Entropy, AOB Pattern, Pointers, Snapshot Diff, ROM Locator, and 60 Hz Memory Freezer) runs natively in pure Rust (`rust-core/target/release/megadaemon-core`).

- `router.py`: Thin 22-line bridge delegating directly to `megadaemon-core chat`.
- `rust_bridge.py`: Optional Python FFI/CLI wrapper for legacy offline batch tools.

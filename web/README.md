# MegaDaemon Web Cockpit (`web/`)

- `cockpit_template.html`: Mobile-first `ui-ux-pro` certified Web UI (Obsidian Phoenix OKLCH palette, zero structural emojis, WCAG AAA contrast, fixed bottom 4-tab thumb nav `SWARM` / `INSPECT` / `VRAM` / `SYSTEM`, unrestricted file uploads up to 100 MB, MiniMax/Swarm selector & API Key Vault, DiE/Binwalk, Rizin 0.9.1, LTO-Decomp v3.4 CIRX SSA, ImHex/Kaitai, MBA Deobfuscator, Frida DBI, and Tile Molester VRAM viewer).
- `megadaemon_cockpit.py`: 18-line compatibility shim that `execv`s the 100% pure-Rust `rust-core/target/release/megadaemon-core cockpit`.

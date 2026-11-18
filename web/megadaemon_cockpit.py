#!/usr/bin/env python3
"""MegaDaemon v13.0 Zenith — Thin Compatibility Shim -> Native Rust Cockpit Server.
Legacy 1,431-line Python HTTP server has been completely replaced by pure Rust (`megadaemon-core cockpit`).
"""
import os
import sys

RUST_BIN = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "rust-core",
    "target",
    "release",
    "megadaemon-core",
)

def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "3000"
    os.execv(RUST_BIN, [RUST_BIN, "cockpit", str(port)])

if __name__ == "__main__":
    main()

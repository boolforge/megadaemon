#!/usr/bin/env python3
"""MegaDaemon v13.0 Zenith — Thin Rust Swarm Bridge (replaces 1,370-line duplicate core/router.py)."""
import json
import os
import subprocess
import sys

RUST_BIN = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "rust-core",
    "target",
    "release",
    "megadaemon-core",
)

def query_swarm(prompt: str, model: str = "minimax-m2.7") -> dict:
    out = subprocess.check_output([RUST_BIN, "chat", model, prompt], text=True)
    return json.loads(out)

if __name__ == "__main__":
    model = sys.argv[1] if len(sys.argv) > 2 else "minimax-m2.7"
    prompt = " ".join(sys.argv[2:]) if len(sys.argv) > 2 else " ".join(sys.argv[1:])
    print(json.dumps(query_swarm(prompt or "ping", model), indent=2, ensure_ascii=False))

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
MegaDaemon v13.1 — E2E del SEERE en Rust contra el LLM local real
================================================================
Mismo banco que bench_fidelity.py (12 preguntas que exigen datos externos,
en español), mismo llama-server, y las DOS variantes del harness Rust:

    --env-in-prompt   -> v13.0 (la huella del host viaja al prompt)
    (sin flag)        -> v13.1 (la huella se queda en el harness)

Métricas: routed (pidió herramienta) y sens (el comando resolvería la pregunta).
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else \
    "/workspace/megadaemon/rust-core/target/debug/megadaemon-core"

CASES = [
    ("¿cuántos núcleos tiene esta máquina?", ["nproc", "cpuinfo", "lscpu", "processor"]),
    ("lee el contenido de /data/local/tmp/config.txt", ["cat "]),
    ("busca 'error' dentro de /data/local/tmp/logs", ["grep", "find", "xargs"]),
    ("¿qué hora es en Madrid?", ["date"]),
    ("busca en wikipedia qué es un autómata finito", ["w wiki", "w ", "wikipedia", "?"]),
    ("¿está vivo el proceso init?", ["ps", "init", "/proc/1", "pgrep"]),
    ("dime el pid del proceso actual", ["/proc/1/stat", "ps -p", "pgrep", "ps -e", "ps aux"]),
    ("¿cuánta memoria RAM queda?", ["free", "meminfo", "mem "]),
    ("lista los ficheros de /data/local/tmp", ["ls "]),
    ("¿qué versión de python hay instalada?", ["python"]),
    ("busca en la web las noticias de hoy", ["?", "wget", "curl", "web", "search"]),
    ("¿cuánto espacio de disco queda?", ["df", "du "]),
]


def run(variant: str) -> dict:
    routed = sens = grounded = 0
    rows = []
    for q, ok_tokens in CASES:
        cmd = [BIN, "seere", q]
        if variant == "env":
            cmd.append("--env-in-prompt")
        t0 = time.time()
        try:
            p = subprocess.run(cmd, capture_output=True, text=True, timeout=180)
            d = json.loads(p.stdout.strip().splitlines()[-1])
        except Exception as e:  # noqa: BLE001
            rows.append({"q": q, "error": str(e)[:120]})
            continue
        tools = d.get("tools", [])
        args = " ".join(t.get("arg", "") for t in tools).lower()
        acted = len(tools) > 0
        is_sens = acted and any(tok.lower() in args for tok in ok_tokens)
        routed += 1 if acted else 0
        sens += 1 if is_sens else 0
        grounded += 1 if d.get("grounded") else 0
        rows.append({"q": q, "acted": acted, "sens": is_sens,
                     "grounded": d.get("grounded"),
                     "answer": (d.get("answer") or "")[:90],
                     "tools": [f"{t.get('tool')}({t.get('arg','')[:44]})" for t in tools][:3],
                     "ms": round((time.time() - t0) * 1000)})
    return {"variant": variant, "routed": routed, "sens": sens, "grounded": grounded,
            "n": len(CASES), "rows": rows}


def main() -> int:
    out = {}
    for v in ("env", "noenv"):
        print(f"\n=== SEERE Rust v13.1  [{v}] ===", flush=True)
        r = run(v)
        out[v] = r
        print(f"  routed={r['routed']}/{r['n']}  sens={r['sens']}/{r['n']}  "
              f"grounded={r['grounded']}/{r['n']}", flush=True)
        for row in r["rows"]:
            if "error" in row:
                print(f"    {row['q'][:34]:36s} ERROR {row['error'][:60]}")
            else:
                print(f"    {row['q'][:34]:36s} routed={str(row['acted']):5s} "
                      f"sens={str(row['sens']):5s} {row['answer'][:44]!r}")
    with open("/workspace/megadaemon/rust-core/e2e_seere_rust.json", "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
    return 0


if __name__ == "__main__":
    sys.exit(main())

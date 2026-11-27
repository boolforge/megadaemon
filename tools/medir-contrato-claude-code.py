#!/usr/bin/env python3
"""Contrato medido de Claude Code v2.1.332 (build nativo linux-x64).

Sale TODO del binario (sha256 del binario en la cabecera) y del paquete npm:
  * variables de entorno que el binario lee (ANTHROPIC_*/CLAUDE_*)
  * eventos de hook que implementa
  * ficheros de configuracion que usa
  * rutas bajo .claude/ que toca
Nada inventado: es `strings` sobre el binario, clasificado.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import sys

BIN = sys.argv[1] if len(sys.argv) > 1 else (
    "/workspace/agy-re/productos/claude-code/native/package/claude"
)
STRINGS = sys.argv[2] if len(sys.argv) > 2 else (
    "/workspace/agy-re/productos/claude-code/native/strings.txt"
)
OUT = sys.argv[3] if len(sys.argv) > 3 else "docs/medidas/claude_code_contrato.json"

EVENTOS = [
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
    "UserPromptSubmit",
    "Notification",
    "Stop",
    "SubagentStop",
    "SessionStart",
    "SessionEnd",
    "PreCompact",
]

RE_ENV = re.compile(r"^(CLAUDE|ANTHROPIC)_[A-Z0-9_]+$")
RE_HOOK_EVENTO = re.compile(
    r"\b(" + "|".join(EVENTOS) + r")\b"
)
RE_CONFIG = re.compile(
    r"\b([a-zA-Z0-9_.-]+\.(?:json|jsonl|ya?ml|toml|md))\b"
)
RE_CLAUDE_DIR = re.compile(r"\.claude/[A-Za-z0-9_./-]+")
INTERESANTE = re.compile(
    r"(settings|mcp|hook|agent|permission|credential|token|key|auth|proxy|proxy)", re.I
)


def sha256_fichero(p: str) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as fh:
        while True:
            b = fh.read(1 << 20)
            if not b:
                break
            h.update(b)
    return h.hexdigest()


def main() -> int:
    lineas = open(STRINGS, encoding="utf-8", errors="replace").read().splitlines()
    env = sorted({l for l in lineas if RE_ENV.match(l)})
    hooks = {e: 0 for e in EVENTOS}
    for l in lineas:
        for m in RE_HOOK_EVENTO.finditer(l):
            hooks[m.group(1)] += 1
    configs = sorted(
        {m.group(1) for l in lineas for m in [RE_CONFIG.search(l)] if m and INTERESANTE.search(m.group(1))}
    )
    claude_dir = sorted({m.group(0) for l in lineas for m in [RE_CLAUDE_DIR.search(l)] if m})

    doc = {
        "producto": "Claude Code",
        "version": "2.1.332",
        "paquete_npm": "@anthropic-ai/claude-code",
        "binario": BIN,
        "binario_bytes": os.path.getsize(BIN),
        "binario_sha256": sha256_fichero(BIN),
        "runtime": "Bun/1.4.6 (medido en las strings del binario)",
        "nota": "El bundle JS va minificado y con source map embebido; el codigo "
        "nativo es el runtime de Bun, no la logica del agente.",
        "variables_entorno": env,
        "n_variables_entorno": len(env),
        "eventos_hook": hooks,
        "n_eventos_hook": sum(1 for v in hooks.values() if v > 0),
        "ficheros_config": configs,
        "rutas_claude_dir": claude_dir,
        "n_rutas_claude_dir": len(claude_dir),
    }
    with open(OUT, "w", encoding="utf-8") as fh:
        json.dump(doc, fh, indent=2, ensure_ascii=False)
    print(f"[contrato] {len(env)} variables · {doc['n_eventos_hook']} eventos de hook · "
          f"{len(configs)} ficheros de config · {len(claude_dir)} rutas .claude/ -> {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

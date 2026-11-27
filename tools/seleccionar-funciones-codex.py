#!/usr/bin/env python3
"""Selecciona el subconjunto de funciones de Codex que se decompila con Ghidra.

MEDIDO 2026-11-27: el binario `codex-app-server` stripped tiene 356.133
funciones; con los simbolos del `.debug` se recuperan todas con su nombre real.
Decompilar 356 k funciones no cabe en el limite de RAM del cgroup
(`memory.limit_in_bytes=3087007744`), asi que aqui se elige un subconjunto
representativo de los CONTRATOS ya medidos (sandbox, permisos, hooks, tools,
MCP/ACP, sesion, config).

Salida: TSV con las mismas columnas que la tabla de simbolos, reordenado por
prioridad (contrato primero) y con el tamano de funcion ya filtrado
(descarta las de menos de 16 bytes: son sellos/stubs sin cuerpo util).
"""
from __future__ import annotations

import re
import sys

# (regex sobre el nombre legible, peso) - el peso ordena por interes
REGLAS = [
    (r"sandbox", 100),
    (r"approval|permission", 95),
    (r"hook", 90),
    (r"tool_call|tool_result|ToolCall", 88),
    (r"mcp|rmcp", 85),
    (r"acp|protocol", 82),
    (r"session|conversation", 80),
    (r"config", 75),
    (r"exec|shell|command", 72),
    (r"apply_patch|patch", 70),
    (r"policy", 68),
    (r"auth|token|credential|secret", 65),
    (r"plan|todo|agent", 60),
    (r"web_search|view_image|skills", 58),
    (r"escalat|trust|danger", 55),
]

MIN_BYTES = 16
MAX_FUNCS = int(sys.argv[3]) if len(sys.argv) > 3 else 400
# Tope de tamano: MEDIDO 2026-11-27, el decompilador de Ghidra tarda ~1 min en
# una funcion de 30 KB (Rust genera funciones enormes) => para una tanda util
# se acota el cuerpo. Las funciones grandes se pueden pedir aparte.
MAX_BYTES = int(sys.argv[4]) if len(sys.argv) > 4 else 4096


def main() -> int:
    ent = sys.argv[1]
    sal = sys.argv[2]
    filas = []
    with open(ent, encoding="utf-8") as fh:
        fh.readline()
        for linea in fh:
            c = linea.rstrip("\n").split("\t")
            if len(c) < 6:
                continue
            if c[2] not in ("T", "t"):
                continue
            try:
                tam = int(c[1], 16)
            except ValueError:
                continue
            if tam < MIN_BYTES or tam > MAX_BYTES:
                continue
            nombre = c[4]
            # fuera closures, genericos y simbolos internos de LLVM
            if "{closure#" in nombre or "{K#" in nombre or ".llvm." in c[5]:
                continue
            peso = 0
            for rx, w in REGLAS:
                if re.search(rx, nombre, re.I):
                    peso = max(peso, w)
            if peso == 0:
                continue
            filas.append((peso, tam, c))
    filas.sort(key=lambda x: (-x[0], -x[1]))
    filas = filas[:MAX_FUNCS]
    with open(sal, "w", encoding="utf-8") as fh:
        fh.write("addr\tbytes\ttipo\tcrate\tnombre_legible\tnombre_mangled\n")
        for peso, tam, c in filas:
            fh.write("\t".join(c) + "\n")
    print(f"[seleccion] {len(filas)} funciones de contrato -> {sal}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

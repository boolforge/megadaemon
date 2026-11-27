#!/usr/bin/env python3
"""Extrae el catalogo de tools de Claude Code desde su sdk-tools.d.ts.

El .d.ts esta GENERADO a partir del JSON Schema real (`json-schema-to-typescript`),
asi que las firmas que hay aqui son las que el binario usa de verdad.

Salida: JSON con una entrada por tool: nombre, descripcion y sus campos
(nombre, tipo, obligatorio, descripcion corta).
"""
from __future__ import annotations

import json
import re
import sys

SRC = sys.argv[1] if len(sys.argv) > 1 else (
    "/workspace/agy-re/productos/claude-code/package/sdk-tools.d.ts"
)
OUT = sys.argv[2] if len(sys.argv) > 2 else "docs/medidas/claude_code_tools.json"


def limpiar(desc: str) -> str:
    d = " ".join(desc.split())
    return d[:220]


def main() -> int:
    txt = open(SRC, encoding="utf-8", errors="replace").read()

    # 1) las tools, por los dos unions del principio
    ent = re.search(r"export type ToolInputSchemas =(.*?);\n", txt, re.S).group(1)
    sal = re.search(r"export type ToolOutputSchemas =(.*?);\n", txt, re.S).group(1)
    tools = [re.sub(r"Input$", "", n) for n in re.findall(r"\b([A-Z][A-Za-z0-9]*Input)\b", ent)]
    outs = [re.sub(r"Output$", "", n) for n in re.findall(r"\b([A-Z][A-Za-z0-9]*Output)\b", sal)]

    # 2) cada interface: campos con su tipo y su comentario
    interfaces = {}
    for m in re.finditer(
        r"export interface (\w+) \{\n(.*?)\n\}\n", txt, re.S
    ):
        nombre, cuerpo = m.group(1), m.group(2)
        campos = []
        # comentario opcional + "campo?: tipo;"
        for c in re.finditer(
            r"(?:/\*\*\s*(.*?)\s*\*/\s*)?([A-Za-z_][A-Za-z0-9_]*)(\?)?\s*:\s*([^;]+);",
            cuerpo,
            re.S,
        ):
            doc, campo, opcional, tipo = c.groups()
            campos.append(
                {
                    "campo": campo,
                    "tipo": " ".join(tipo.split())[:120],
                    "obligatorio": opcional is None,
                    "desc": limpiar(doc or ""),
                }
            )
        interfaces[nombre] = {"desc": "", "campos": campos}

    catalogo = []
    for t in tools:
        inp = interfaces.get(f"{t}Input", {"campos": []})
        out = interfaces.get(f"{t}Output", {"campos": []})
        catalogo.append(
            {
                "tool": t,
                "entrada": inp["campos"],
                "salida": out["campos"],
                "n_campos_entrada": len(inp["campos"]),
                "n_campos_salida": len(out["campos"]),
            }
        )

    doc = {
        "producto": "Claude Code",
        "version": "2.1.332",
        "paquete_npm": "@anthropic-ai/claude-code",
        "fuente_del_esquema": SRC,
        "nota": "El .d.ts lo genera json-schema-to-typescript a partir del JSON Schema real; "
        "las firmas son las que el binario aplica.",
        "n_tools": len(catalogo),
        "n_tools_salida": len(outs),
        "tools": catalogo,
    }
    with open(OUT, "w", encoding="utf-8") as fh:
        json.dump(doc, fh, indent=2, ensure_ascii=False)

    print(f"[tools] {len(catalogo)} tools -> {OUT}")
    print(f"{'tool':<26} {'campos entrada':>14} {'campos salida':>14}")
    for c in catalogo:
        print(f"  {c['tool']:<24} {c['n_campos_entrada']:>14} {c['n_campos_salida']:>14}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

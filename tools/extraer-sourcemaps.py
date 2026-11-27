#!/usr/bin/env python3
"""Extrae los source maps embebidos (base64 data-URL) de un binario.

Claude Code (build nativo Bun) lleva el bundle JS minificado con su
`sourceMappingURL=data:application/json;base64,...` dentro del binario. Si el
mapa trae `sourcesContent`, el codigo fuente original es recuperable: eso es
la desofuscacion completa, sin adivinar nada.
"""
from __future__ import annotations

import base64
import json
import os
import sys

MARCA = b"sourceMappingURL=data:application/json;base64,"


def main() -> int:
    binario = sys.argv[1]
    salida_dir = sys.argv[2]
    os.makedirs(salida_dir, exist_ok=True)
    n = 0
    encontrados = []
    with open(binario, "rb") as fh:
        datos = fh.read()
    pos = 0
    while True:
        i = datos.find(MARCA, pos)
        if i < 0:
            break
        j = i + len(MARCA)
        k = j
        while k < len(datos) and datos[k : k + 1] not in (b"\n", b"\r", b'"', b"'"):
            k += 1
        b64 = datos[j:k]
        pos = k
        if len(b64) < 100:
            continue
        try:
            crudo = base64.b64decode(b64, validate=False)
        except Exception:  # noqa: BLE001
            continue
        try:
            sm = json.loads(crudo)
        except Exception:  # noqa: BLE001
            continue
        n += 1
        nombre = f"sourcemap_{n:03d}.json"
        with open(os.path.join(salida_dir, nombre), "w", encoding="utf-8") as fh:
            json.dump(sm, fh, ensure_ascii=False)
        fuentes = sm.get("sources") or []
        tiene_contenido = bool(sm.get("sourcesContent"))
        bytes_contenido = sum(len(c or "") for c in (sm.get("sourcesContent") or []))
        encontrados.append(
            {
                "fichero": nombre,
                "bytes_json": len(crudo),
                "n_sources": len(fuentes),
                "sourcesContent": tiene_contenido,
                "bytes_sourcesContent": bytes_contenido,
                "primeros_sources": fuentes[:5],
            }
        )
        print(
            f"[sourcemap] {nombre}: {len(crudo)} B · {len(fuentes)} sources · "
            f"sourcesContent={tiene_contenido} ({bytes_contenido} B)"
        )
    with open(os.path.join(salida_dir, "_resumen.json"), "w", encoding="utf-8") as fh:
        json.dump(encontrados, fh, indent=2, ensure_ascii=False)
    print(f"\n[total] {n} source maps -> {salida_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

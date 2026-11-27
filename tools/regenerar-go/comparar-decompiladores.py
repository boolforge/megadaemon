#!/usr/bin/env python3
"""Analisis COMPARATIVO de dos decompiladores sobre las mismas funciones.

Pregunta que responde (medida, no estimada): para el binario Go de Antigravity,
¿el pseudocodigo de r2ghidra (radare2 6.2.6) es tan bueno como el de Ghidra
11.2.1, y a que velocidad y coste de memoria? La respuesta decide como se
cierra el 100 % de cobertura.

Metrica: para cada funcion se comparan
  * ok        : ambos produjeron C no vacio
  * lineas    : tamaño de cada pseudocodigo
  * llamadas  : conjunto de identificadores seguidos de '(' (los callees)
  * jaccard   : Jaccard de los identificadores significativos de ambos C
  * ms        : coste de cada decompilador

Uso:
  comparar-decompiladores.py --tsv go.funcs.tsv --ghidra todo/s0 --muestra 200
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import subprocess
import time

R2 = "/workspace/re-tools/r2.sh"

IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
LLAMADA = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(")
COMUN = {
    "if", "for", "while", "switch", "return", "int", "long", "char", "void",
    "uint", "ulong", "undefined", "undefined8", "code", "byte", "short",
    "float", "double", "bool", "size_t", "const", "static", "struct", "union",
    "enum", "typedef", "break", "continue", "else", "do", "goto", "default",
    "case", "sizeof", "true", "false", "null", "NULL", "var", "param",
}


def leer_indice(path):
    filas = []
    with open(path, encoding="utf-8", errors="replace") as fh:
        next(fh, None)
        for line in fh:
            p = line.rstrip("\n").split("\t")
            if len(p) < 6:
                continue
            try:
                addr = int(p[0], 16)
            except ValueError:
                continue
            filas.append(
                {
                    "addr": addr,
                    "name": p[1],
                    "file": p[2],
                    "line": p[3],
                    "bytes": p[4],
                    "ok": p[5].strip() in ("1", "true", "True"),
                    "ms": p[7] if len(p) > 7 else "",
                }
            )
    return filas


def cargar_bloques_ghidra(ruta_c):
    """Divide un .c de Ghidra en bloques por funcion (marcador '/* ==== ... ==== */')."""
    if not os.path.exists(ruta_c):
        return {}
    txt = open(ruta_c, encoding="utf-8", errors="replace").read()
    partes = re.split(r"/\* ==== (.*?) ==== \*/", txt)
    bloques = {}
    for i in range(1, len(partes) - 1, 2):
        cabecera = partes[i]
        cuerpo = partes[i + 1]
        m = re.match(r"(\S+)\s+\[(\d+)\s*bytes\]", cabecera)
        if m:
            bloques[m.group(1)] = cuerpo
    return bloques


def identificadores(texto):
    return {t for t in IDENT.findall(texto) if t not in COMUN and len(t) > 2}


def llamadas(texto):
    return {m.group(1) for m in LLAMADA.finditer(texto)}


def jaccard(a, b):
    if not a and not b:
        return 1.0
    if not a or not b:
        return 0.0
    return len(a & b) / len(a | b)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tsv", required=True)
    ap.add_argument("--ghidra", required=True, help="directorio con _index.tsv y los .c")
    ap.add_argument("--bin", required=True)
    ap.add_argument("--muestra", type=int, default=200)
    ap.add_argument("--salida", default="/workspace/md-re-productos/docs/medidas/comparativa_decompiladores.json")
    args = ap.parse_args()

    filas = leer_indice(os.path.join(args.ghidra, "_index.tsv"))
    buenas = [f for f in filas if f["ok"]]
    random.seed(7)
    muestra = random.sample(buenas, min(args.muestra, len(buenas)))
    print(f"[cmp] indice: {len(filas)} filas, {len(buenas)} con ok=1, muestra {len(muestra)}")

    # --- r2ghidra: una sola sesion para toda la muestra ---
    t0 = time.time()
    tmp = "/workspace/agy-re/tools-out/todo/_cmp"
    # MEDIDO: Ghidra carga este ELF con image base 0x100000 => sus direcciones
    # son vaddr + 0x100000. Para r2 hay que quitarlo.
    BASE_GHIDRA = 0x100000
    os.makedirs(tmp, exist_ok=True)
    script = os.path.join(tmp, "cmp.r2")
    with open(script, "w", encoding="utf-8") as fh:
        for f in muestra:
            tam = int(f["bytes"]) if f["bytes"].isdigit() else 0x100
            va = f["addr"] - BASE_GHIDRA
            destino = os.path.join(tmp, f"{va:x}.c")
            nombre = "f_" + f"{va:x}"
            fh.write(f"af+ {va:#x} {nombre}; afb+ {va:#x} {va:#x} {tam:#x}; ")
            fh.write(f"pdg @ {va:#x} > {destino}\n")
    subprocess.run(
        [R2, "-q", "-N", "-e", "scr.color=0", "-i", script, args.bin],
        capture_output=True, text=True, timeout=3600,
    )
    t_r2 = time.time() - t0
    print(f"[cmp] r2ghidra: {len(muestra)} funciones en {t_r2:.1f}s ({len(muestra)/max(t_r2,0.01):.2f} fns/s)")

    # --- Ghidra: tiempo por funcion ya medido en su indice ---
    ms_gh = [int(f["ms"]) for f in muestra if f["ms"].isdigit()]
    t_gh = sum(ms_gh) / 1000.0

    filas_cmp = []
    for f in muestra:
        # el indice guarda el fichero Go (x.go) y el arbol guarda el .c (x.c)
        base = f["file"]
        if base.endswith(".go"):
            base = base[:-3] + ".c"
        else:
            base = base + ".c"
        bloques = cargar_bloques_ghidra(os.path.join(args.ghidra, base))
        g = bloques.get(f["name"], "")
        ruta_r2 = os.path.join(tmp, f"{f['addr'] - BASE_GHIDRA:x}.c")
        r = open(ruta_r2, encoding="utf-8", errors="replace").read() if os.path.exists(ruta_r2) else ""
        ig, ir = identificadores(g), identificadores(r)
        cg, cr = llamadas(g), llamadas(r)
        filas_cmp.append(
            {
                "addr": f"{f['addr']:x}",
                "name": f["name"],
                "file": f["file"],
                "bytes": int(f["bytes"]) if f["bytes"].isdigit() else None,
                "ghidra_lineas": g.count("\n"),
                "r2_lineas": r.count("\n"),
                "ghidra_ok": bool(g.strip()),
                "r2_ok": bool(r.strip()),
                "jaccard_ident": round(jaccard(ig, ir), 3),
                "jaccard_llamadas": round(jaccard(cg, cr), 3),
                "ghidra_ms": int(f["ms"]) if f["ms"].isdigit() else None,
            }
        )

    def pct(vals, p):
        if not vals:
            return None
        s = sorted(vals)
        return s[min(len(s) - 1, int(len(s) * p))]

    ambos = [c for c in filas_cmp if c["ghidra_ok"] and c["r2_ok"]]
    resumen = {
        "muestra": len(filas_cmp),
        "r2ghidra_ok": sum(1 for c in filas_cmp if c["r2_ok"]),
        "ghidra_ok": sum(1 for c in filas_cmp if c["ghidra_ok"]),
        "jaccard_ident_mediana": pct([c["jaccard_ident"] for c in ambos], 0.5),
        "jaccard_ident_p25": pct([c["jaccard_ident"] for c in ambos], 0.25),
        "jaccard_llamadas_mediana": pct([c["jaccard_llamadas"] for c in ambos], 0.5),
        "lineas_r2_mediana": pct([c["r2_lineas"] for c in ambos], 0.5),
        "lineas_ghidra_mediana": pct([c["ghidra_lineas"] for c in ambos], 0.5),
        "ms_ghidra_mediana": pct([c["ghidra_ms"] for c in ambos if c["ghidra_ms"]], 0.5),
        "r2_fns_por_seg": round(len(muestra) / max(t_r2, 0.01), 2),
        "ghidra_fns_por_seg_1_worker": round(1 / (t_gh / max(len(muestra), 1)), 2) if t_gh else None,
        "nota": "ghidra_fns_por_seg_1_worker usa el tiempo por funcion medido por los workers en su _index.tsv (no incluye arranque de la JVM)",
    }
    os.makedirs(os.path.dirname(args.salida), exist_ok=True)
    json.dump({"resumen": resumen, "detalle": filas_cmp}, open(args.salida, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print(json.dumps(resumen, indent=1, ensure_ascii=False))
    print(f"[cmp] escrito {args.salida}")


if __name__ == "__main__":
    main()

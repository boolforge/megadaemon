#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""medir-motores.py — rendimiento de cada motor sobre las 103 funciones.

Para cada motor se cuenta cuantas funciones producen pseudocodigo y de que
calidad es, para que el veredicto "irreconstruible" se pueda poner en su sitio:
una funcion que otro motor si decompila NO es irreconstruible.

uso:
  tools/regenerar-go/medir-motores.py --evidencia TSV --salida JSON
"""
from __future__ import annotations

import argparse
import csv
import json
import os
import re
import sys
from collections import Counter
from pathlib import Path
from typing import Dict, List, Optional, Sequence

RE_CONTROL_FLUJO = re.compile(r"\b(if|while|for|switch|goto)\b")
RE_PSTR = re.compile(r"\s*//\s*\(pstr 0x[0-9a-f]+\).*$")


def main(argv: Optional[Sequence[str]] = None) -> int:
    ap = argparse.ArgumentParser(description="Mide el rendimiento de cada motor")
    ap.add_argument("--evidencia", type=Path, required=True, help="TSV de las 103")
    ap.add_argument("--motor-propio", type=Path, default=None, help="informe del motor propio (Ghidra JVM)")
    ap.add_argument("--r2dec", type=Path, default=None, help="directorio con los .c de r2dec")
    ap.add_argument("--ghidra-pclntab", type=Path, default=None, help="directorio con los .c del 4.º motor")
    ap.add_argument("--wrappers", type=Path, default=None, help="JSON de la reconstruccion de wrappers")
    ap.add_argument("--salida", type=Path, required=True)
    args = ap.parse_args(argv)

    with open(args.evidencia, encoding="utf-8") as f:
        filas = list(csv.DictReader(f, delimiter="\t"))
    nombres = {r["nombre"] for r in filas}

    res: Dict[str, object] = {"funciones": len(filas)}

    # --- motor propio (Ghidra JVM) ---
    if args.motor_propio and Path(args.motor_propio).is_file():
        ok = 0
        errs: Counter = Counter()
        with open(args.motor_propio, encoding="utf-8", errors="replace") as f:
            f.readline()
            for linea in f:
                p = linea.rstrip("\n").split("\t")
                if len(p) < 9 or p[1] not in nombres:
                    continue
                if p[8].strip().lower() == "true":
                    ok += 1
                else:
                    errs[(p[9].strip() if len(p) > 9 else "")[:70]] += 1
        res["motor_propio_ok"] = ok
        res["motor_propio_errores"] = dict(errs.most_common(10))

    # --- r2dec (5.º motor) ---
    if args.r2dec and Path(args.r2dec).is_dir():
        ok = 0
        vacias = 0
        cc = 0
        cadenas = 0
        for fn in os.listdir(args.r2dec):
            if not fn.endswith(".c"):
                continue
            t = Path(args.r2dec, fn).read_text(encoding="utf-8", errors="replace")
            if len(t.strip()) < 20:
                vacias += 1
                continue
            ok += 1
            if RE_CONTROL_FLUJO.search(t):
                cc += 1
            limpio = "\n".join(RE_PSTR.sub("", l) for l in t.splitlines())
            if re.search(r'"[^"]{3,}"', limpio):
                cadenas += 1
        res["r2dec_ok"] = ok
        res["r2dec_vacias"] = vacias
        res["r2dec_control_flujo"] = cc
        res["r2dec_con_cadenas"] = cadenas

    # --- Ghidra con cuerpo forzado (4.º motor) ---
    if args.ghidra_pclntab and Path(args.ghidra_pclntab).is_dir():
        ok = 0
        vacias = 0
        for fn in os.listdir(args.ghidra_pclntab):
            if not fn.endswith(".c"):
                continue
            if Path(args.ghidra_pclntab, fn).stat().st_size < 20:
                vacias += 1
            else:
                ok += 1
        res["motor4_ok"] = ok
        res["motor4_vacias"] = vacias

    # --- reconstruccion de wrappers ---
    if args.wrappers and Path(args.wrappers).is_file():
        w = json.loads(Path(args.wrappers).read_text(encoding="utf-8"))
        res["wr_ok"] = sum(1 for x in w if x["veredicto"] == "RECONSTRUIDO")
        res["wr_def"] = sum(1 for x in w if x.get("sentencia"))
        res["wr_nofuente"] = sum(1 for x in w if x["veredicto"] == "SIN-FUENTE")
        filas_w = []
        for x in w:
            filas_w.append(
                "- `%s` (`%s`) → `%s`:%s — %s"
                % (
                    x["nombre"],
                    x["addr"],
                    x["fichero_real"] or "(sin fuente en este entorno)",
                    x["linea_real"],
                    ("`%s`" % x["sentencia"]) if x.get("sentencia") else (x.get("linea_texto") or "-"),
                )
            )
        res["wr_tabla"] = "\n".join(filas_w)

    args.salida.parent.mkdir(parents=True, exist_ok=True)
    args.salida.write_text(json.dumps(res, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(res, indent=2, ensure_ascii=False)[:1500])
    return 0


if __name__ == "__main__":
    sys.exit(main())

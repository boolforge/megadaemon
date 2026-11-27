#!/usr/bin/env python3
"""Pone los nombres Go en el pseudocodigo de r2ghidra.

MEDIDO: r2ghidra (radare2 + plugin) decompila igual que Ghidra, pero sus
LLAMADAS salen como `fcn.0778b670` (direccion) mientras el motor propio de
MegaDaemon (Ghidra + MdGhidra.java) ya trae el simbolo Go porque aplica los
161.682 nombres del pclntab.

Este post-proceso cierra ese hueco: reemplaza cada `func_0x<hex>` por el simbolo
Go de esa direccion (mapa de `go.funcs.tsv`), dejando `sub_<hex>` si la
direccion no esta en el pclntab.

Medicion: cuenta cuantos `fcn.` quedan sin renombrar.
"""

from __future__ import annotations

import argparse
import os
import re
import sys

# MEDIDO: r2ghidra nombra las llamadas como func_0x<hex> (no fcn.<hex>)
FCN = re.compile(r"\bfunc_0x([0-9a-fA-F]{4,16})\b")


def cargar_mapa(tsv: str) -> tuple:
    """Devuelve (direccion -> simbolo, lista ordenada de direcciones)."""
    mapa = {}
    with open(tsv, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            p = line.rstrip("\n").split("\t")
            if len(p) < 2:
                continue
            try:
                mapa[int(p[0], 16)] = p[1]
            except ValueError:
                continue
    return mapa, sorted(mapa)


def renombrar(texto: str, mapa: dict, orden: list) -> tuple[str, int, int]:
    """Sustituye func_0x<hex> por el simbolo Go.

    Si la direccion no es una entrada de funcion (p.ej. una etiqueta a mitad de
    cuerpo), se nombra con la funcion que la contiene + el desplazamiento:
    `internal_abi.Foo+0x40`. Eso deja el pseudocodigo legible sin inventar.
    """
    import bisect

    hits = [0]
    falls = [0]

    def sub(m):
        v = m.group(1)
        try:
            addr = int(v, 16)
        except ValueError:
            return m.group(0)
        nombre = mapa.get(addr)
        if nombre:
            hits[0] += 1
            return nombre
        i = bisect.bisect_right(orden, addr) - 1
        if i >= 0:
            base = orden[i]
            delta = addr - base
            if delta < 0x10000:
                hits[0] += 1
                return f"{mapa[base]}+0x{delta:x}"
        falls[0] += 1
        return f"sub_{v}"

    return FCN.sub(sub, texto), hits[0], falls[0]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tsv", required=True)
    ap.add_argument("--raiz", required=True, help="directorio con los .c")
    ap.add_argument("--informe", default="")
    ap.add_argument("--limite", type=int, default=0)
    args = ap.parse_args()

    mapa, orden = cargar_mapa(args.tsv)
    print(f"[nombres] mapa: {len(mapa)} direcciones Go")

    ficheros = []
    for dirpath, _dirs, files in os.walk(args.raiz):
        for f in files:
            if f.endswith(".c"):
                ficheros.append(os.path.join(dirpath, f))
    ficheros.sort()
    if args.limite:
        ficheros = ficheros[: args.limite]
    print(f"[nombres] ficheros .c: {len(ficheros)}")

    total_hit = 0
    total_fall = 0
    tocados = 0
    for ruta in ficheros:
        try:
            txt = open(ruta, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        if "func_0x" not in txt:
            continue
        nuevo, hits, falls = renombrar(txt, mapa, orden)
        if hits or falls:
            with open(ruta, "w", encoding="utf-8") as fh:
                fh.write(nuevo)
            tocados += 1
            total_hit += hits
            total_fall += falls
    print(f"[nombres] ficheros tocados: {tocados}")
    print(f"[nombres] llamadas renombradas con simbolo Go: {total_hit}")
    print(f"[nombres] llamadas sin simbolo (sub_<hex>): {total_fall}")
    if args.informe:
        with open(args.informe, "w", encoding="utf-8") as fh:
            fh.write("ficheros_tocados\t%s\n" % tocados)
            fh.write("renombradas_go\t%s\n" % total_hit)
            fh.write("sin_simbolo\t%s\n" % total_fall)
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Que funciones Go usan una cadena dada (xrefs de datos made in AArch64).

Para cada cadena se recorren TODAS las instrucciones del .text, se siguen
los pares ADRP+ADD (direccion directa) y ADRP+LDR (lectura de un puntero),
y las direcciones obtenidas se cruzan con la tabla de simbolos de go-retool
para decir que funcion las usa.

Es la forma fiable de responder "donde se lee admin_settings.json" o "donde
se decide el catalogo de modelos" en un binario Go: el pseudocodigo de Ghidra
sin referencias de datos no lo muestra.

Uso:
  python3 xrefs_of_string.py <binario> <tsv_simbolos> <cadena> [<cadena> ...]
"""

import bisect
import struct
import sys

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from strings_of_function import Elf, escanear


def main():
    if len(sys.argv) < 4:
        print(__doc__)
        return 2
    binpath, tsv = sys.argv[1], sys.argv[2]
    needles = sys.argv[3:]

    filas = []
    for line in open(tsv, encoding="utf-8"):
        p = line.rstrip("\n").split("\t")
        filas.append((int(p[0], 16), int(p[5]), p[1], p[2]))
    filas.sort()
    starts = [f[0] for f in filas]

    def funcion(va):
        i = bisect.bisect_right(starts, va) - 1
        if i < 0 or va >= filas[i][0] + filas[i][1]:
            return None, None
        return filas[i][2], filas[i][3]

    elf = Elf(binpath)
    # 1. VAs de las cadenas
    targets = {}
    for nd in needles:
        nb = nd.encode("utf-8")
        for v, o, fsz, msz in elf.segs:
            data = elf.read_va(v, fsz)
            if data is None:
                continue
            pos = data.find(nb)
            while pos >= 0:
                targets.setdefault(v + pos, []).append(nd)
                pos = data.find(nb, pos + 1)
    if not targets:
        print("cadena no encontrada")
        return 1
    for va in sorted(targets):
        print("cadena %r en VA 0x%x" % (targets[va][0], va))

    # 2. escaneo del .text
    stype, saddr, soff, ssize = elf.sections.get(".text", (0, 0, 0, 0))
    print("escaneando .text 0x%x..0x%x (%d bytes)" % (saddr, saddr + ssize, ssize))
    refs = escanear(elf, saddr, saddr + ssize)

    # 3. cruce
    print("--- referencias ---")
    vistos = set()
    for pc, tipo, va in refs:
        if va in targets and (va, tipo) not in vistos:
            vistos.add((va, tipo))
            nom, loc = funcion(va)
            print("  %-9s 0x%-10x pc=0x%-10x %s" % (tipo, va, pc,
                  ("%s (%s)" % (nom, loc)) if nom else "(sin funcion Go)"))
    if not vistos:
        print("  ninguna")
    return 0


if __name__ == "__main__":
    sys.exit(main())

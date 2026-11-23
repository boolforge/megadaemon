#!/usr/bin/env python3
"""Compara la lista de funciones que Ghidra tiene tras importar el export de
go-retool con el TSV de go-retool.

Este script es la VERIFICACION de la importacion: no se fia de lo que dice el
script de Ghidra, sino que vuelve a leer el TSV que Ghidra exporto y lo
compara fila a fila con el TSV de go-retool.

Criterios (MEDIDOS, 2026-11-22):
  * Cada fila del TSV de go-retool debe tener una FUNCION de Ghidra en la
    MISMA direccion (vaddr del ELF).
  * El nombre debe coincidir.  Excepcion documentada: cuando dos funciones
    de Go tienen el MISMO nombre (90 nombres, 204 filas del binario real),
    Ghidra exige nombres unicos y la segunda lleva el sufijo "@0x<va>".
  * El tamano del cuerpo de Ghidra debe coincidir con el extent de la
    pclntab (end_va - va), salvo que Ghidra lo haya recortado.

Uso:
  python3 verify_ghidra_import.py <simbolos.tsv> <funciones_ghidra.tsv> \
      [anchors.tsv] [informe.tsv]
"""

import re
import sys

DISAMB = re.compile(r"@0x[0-9a-fA-F]+$")


def sanea(nombre):
    """Replica SymbolUtilities.replaceInvalidChars de Ghidra: sustituye por
    '_' los caracteres que Ghidra prohibe en un simbolo (c < ' ' o c == ' ')."""
    return "".join(c if (ord(c) >= 32 and c != " ") else "_" for c in nombre)


def read_tsv(path):
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if not line:
                continue
            yield line.split("\t")


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    syms_path, gh_path = sys.argv[1], sys.argv[2]
    anchors_path = sys.argv[3] if len(sys.argv) > 3 else None
    out_path = sys.argv[4] if len(sys.argv) > 4 else None

    # TSV de go-retool: va, nombre, loc, fin, paquete, tamano, entryoff
    syms = {}
    for p in read_tsv(syms_path):
        if p[0].startswith("0x<"):
            continue
        try:
            size = int(p[5]) if len(p) > 5 and p[5] else None
        except ValueError:
            size = None
        syms[p[0]] = (p[1], size)

    # TSV de Ghidra: va_elf, nombre, tamano, fuente
    gh = {}
    for p in read_tsv(gh_path):
        if p[0].startswith("0x<"):
            continue
        try:
            size = int(p[2]) if len(p) > 2 and p[2] else None
        except ValueError:
            size = None
        gh[p[0]] = (p[1], size, p[3] if len(p) > 3 else "")

    total = len(syms)
    falta = []
    nombre_ok = 0
    nombre_disamb = 0
    nombre_saneado = 0
    nombre_mal = []
    tam_ok = 0
    tam_dif = []
    extras = sorted(set(gh) - set(syms))

    for va, (nombre, size) in syms.items():
        g = gh.get(va)
        if g is None:
            falta.append(va)
            continue
        gn, gsize, _src = g
        if gn == nombre:
            nombre_ok += 1
        elif DISAMB.sub("", gn) == nombre:
            nombre_disamb += 1
        elif DISAMB.sub("", gn) == sanea(nombre):
            # Ghidra no admite espacios en un simbolo: el importador usa el
            # nombre saneado y deja el REAL en el comentario de placa.
            nombre_saneado += 1
        else:
            nombre_mal.append((va, nombre, gn))
        if size is not None and gsize is not None:
            if size == gsize:
                tam_ok += 1
            else:
                tam_dif.append((va, nombre, size, gsize))

    print("TSV de go-retool      : %d funciones" % total)
    print("Funciones en Ghidra   : %d (%d en el programa + %d externas)" %
          (len(gh), len(gh) - sum(1 for v in gh.values() if v[2] == "EXTERNAL"),
           sum(1 for v in gh.values() if v[2] == "EXTERNAL")))
    print("Direcciones de go-retool SIN funcion en Ghidra : %d" % len(falta))
    print("Nombre EXACTO igual    : %d" % nombre_ok)
    print("Nombre igual con sufijo de desambiguacion (@0x<va>) : %d" % nombre_disamb)
    print("Nombre igual con el SANEADO de Ghidra (espacios -> '_') : %d" % nombre_saneado)
    print("Nombre DISTINTO        : %d" % len(nombre_mal))
    print("Tamano de cuerpo igual : %d" % tam_ok)
    print("Tamano de cuerpo DISTINTO : %d" % len(tam_dif))
    print("Funciones de Ghidra que NO estan en el TSV : %d" % len(extras))
    if falta[:10]:
        print("  ejemplo sin funcion:", falta[:10])
    if nombre_mal[:10]:
        for va, a, b in nombre_mal[:10]:
            print("  nombre distinto: %s  ts=%r  ghidra=%r" % (va, a, b))
    if tam_dif[:10]:
        for va, a, s, g in tam_dif[:10]:
            print("  tamano distinto: %s %s ts=%s ghidra=%s" % (va, a, s, g))
    if extras[:10]:
        print("  ejemplo de extra:", extras[:10])

    # Los anclajes tienen que ser funciones con su offset real.
    if anchors_path:
        n = 0
        bad = []
        for p in read_tsv(anchors_path):
            if len(p) < 3:
                continue
            va = p[0]
            # La 2a columna es el nombre de simbolo (mangled) y la 3a el
            # nombre real de Go; se acepta cualquiera de los dos.
            candidatos = set(p[1:3]) | {sanea(x) for x in p[1:3]}
            g = gh.get(va)
            if g is None:
                bad.append((va, p[1], "sin funcion"))
            elif g[0] in candidatos or DISAMB.sub("", g[0]) in candidatos:
                n += 1
            else:
                bad.append((va, p[1], g[0]))
        print("Anclajes como FUNCION con su offset real : %d/%d" % (n, n + len(bad)))
        for b in bad[:10]:
            print("  anclaje problematico:", b)

    if out_path:
        with open(out_path, "w", encoding="utf-8") as fh:
            fh.write("metrica\tvalor\n")
            fh.write("funciones_go_retool\t%d\n" % total)
            fh.write("funciones_ghidra\t%d\n" % len(gh))
            fh.write("direcciones_sin_funcion\t%d\n" % len(falta))
            fh.write("nombre_exacto\t%d\n" % nombre_ok)
            fh.write("nombre_desambiguado\t%d\n" % nombre_disamb)
            fh.write("nombre_distinto\t%d\n" % len(nombre_mal))
            fh.write("tamano_igual\t%d\n" % tam_ok)
            fh.write("tamano_distinto\t%d\n" % len(tam_dif))
            fh.write("funciones_extra\t%d\n" % len(extras))
        print("informe escrito en", out_path)
    return 0


if __name__ == "__main__":
    sys.exit(main())

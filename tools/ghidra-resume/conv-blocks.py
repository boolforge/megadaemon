#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Convierte la salida POR FUNCION de GoAnchorDecompiler.java al formato de
BLOQUE del arbol de pseudocodigo (`todo/s{0,1,2}/<pkg>/<fichero>.c`).

FORMATO DE ORIGEN (una funcion por fichero):
    // <tag>  <goName>
    // original Go symbol : <orig>
    // source location    : <sloc>
    // entry address      : <addr>
    // ghidra function    : <gname>
    // body instructions  : <N>
    // signature          : <sig>
    <blanco>
    <blanco>
    /* go:
       <orig>
       <sloc> */
    <blanco>
    <cuerpo C>
    ...
    // ---- direct call targets resolved to Go symbols ----
    ...
    // ---- string literals referenced by this function ----
    ...

FORMATO DE DESTINO (bloque dentro del .c por fichero Go):
    /* ==== <goName>  [<N> bytes] ==== */
    <sig>
    <blanco>
    /* go:
       <orig>
       <sloc> */
    <blanco>
    <cuerpo C>

VERIFICADO byte a byte contra un bloque ya existente del arbol (2026-11-23):
`decomp/mcp__...__Connection_.Call.c` -> bloque identico en
`todo/s0/third_party/.../jsonrpc2/conn.c`.
"""
import re
import sys
import os

RE_CAB = re.compile(r"^// (?P<tag>\S+)\s\s(?P<nombre>.+)$")
RE_SYM = re.compile(r"^// signature\s+: (?P<sig>.*)$")
RE_BODY = re.compile(r"^// body instructions\s+: (?P<n>\d+)$")
RE_ORIG = re.compile(r"^// original Go symbol\s+: (?P<v>.*)$")
RE_SLOC = re.compile(r"^// source location\s+: (?P<v>.*)$")
MARCA_CALLS = "// ---- direct call targets resolved to Go symbols ----"
MARCA_STRINGS = "// ---- string literals referenced by this function ----"


def convierte(ruta):
    """Devuelve el bloque en formato destino, o None si el fichero no vale."""
    with open(ruta, encoding="utf-8", errors="replace") as fh:
        txt = fh.read()
    lineas = txt.split("\n")
    if not lineas:
        return None
    m = RE_CAB.match(lineas[0])
    if not m:
        return None
    nombre = m.group("nombre").strip()
    sig = ""
    nbytes = 0
    i = 1
    while i < len(lineas) and lineas[i].startswith("//"):
        ms = RE_SYM.match(lineas[i])
        mb = RE_BODY.match(lineas[i])
        if ms:
            sig = ms.group("sig").strip()
        elif mb:
            nbytes = int(mb.group("n"))
        i += 1
    # el cuerpo empieza en el comentario `/* go:` y acaba antes de las
    # secciones de calls/strings (que el arbol no lleva).
    ini = None
    for j in range(i, len(lineas)):
        if lineas[j].startswith("/* go:"):
            ini = j
            break
    if ini is None:
        return None
    fin = len(lineas)
    for j in range(ini, len(lineas)):
        if lineas[j].startswith(MARCA_CALLS) or lineas[j].startswith(MARCA_STRINGS):
            fin = j
            break
    cuerpo = lineas[ini:fin]
    while cuerpo and cuerpo[-1].strip() == "":
        cuerpo.pop()
    salida = ["/* ==== %s  [%d bytes] ==== */" % (nombre, nbytes)]
    if sig:
        salida.append(sig)
    salida.append("")
    salida.extend(cuerpo)
    salida.append("")
    salida.append("")
    return "\n".join(salida)


def main():
    if len(sys.argv) < 2:
        print("uso: conv-blocks.py <fichero .c de salida del Java> [...]")
        print("     imprime el bloque en formato destino por stdout")
        return 1
    for r in sys.argv[1:]:
        b = convierte(r)
        if b is None:
            print("!! no convertible:", r, file=sys.stderr)
            return 2
        sys.stdout.write(b + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())

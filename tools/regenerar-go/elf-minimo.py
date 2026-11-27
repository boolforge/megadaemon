#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""elf-minimo.py — arma un ELF64 aarch64 mínimo con un slice del binario.

Sirve para que retdec (que sólo acepta ELF o "raw" con deteccion de formato)
pueda analizar UN trocito del binario real: se copia el slice a la seccion
`.text` y se pone el punto de entrada en la direccion de la funcion.

uso:
  elf-minimo.py --binario BIN --vma 0xADDR --tamano N --entrada 0xADDR --salida out.elf
"""
from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path
from typing import Optional, Sequence

EM_AARCH64 = 183
ET_DYN = 3
PT_LOAD = 1
SHT_PROGBITS = 1
SHT_STRTAB = 3
SHT_SYMTAB = 2
SHF_ALLOC = 0x2
SHF_EXECINSTR = 0x4


def main(argv: Optional[Sequence[str]] = None) -> int:
    ap = argparse.ArgumentParser(description="ELF64 aarch64 minimo con un slice del binario")
    ap.add_argument("--binario", type=Path, required=True)
    ap.add_argument("--vma", type=lambda x: int(x, 0), required=True, help="VMA del slice")
    ap.add_argument("--tamano", type=lambda x: int(x, 0), required=True)
    ap.add_argument("--entrada", type=lambda x: int(x, 0), required=True)
    ap.add_argument("--salida", type=Path, required=True)
    args = ap.parse_args(argv)

    with open(args.binario, "rb") as f:
        f.seek(args.vma)
        texto = f.read(args.tamano)
    if len(texto) < args.tamano:
        sys.stderr.write("aviso: el binario se acaba antes de %d bytes\n" % args.tamano)

    # .strtab con el nombre del simbolo y .symtab con una entrada FUNC en el
    # punto de entrada: retdec usa la tabla de simbolos para definir las
    # funciones; sin ella el modulo le queda vacio.
    strtab = b"\0f_entry\0"
    sym_name = 1
    shstr = b"\0.text\0.symtab\0.strtab\0.shstrtab\0"
    # disposicion SIN solapes: texto, symtab, strtab, shstrtab, cabeceras.
    off_text = 0x1000  # tras las cabeceras ELF+programa
    off_symtab = off_text + len(texto)
    off_strtab = off_symtab + 24 * 2  # simbolo nulo + 1 FUNC
    off_shstr = off_strtab + len(strtab)
    shoff = (off_shstr + len(shstr) + 7) & ~7  # alineado a 8

    ehsize, phentsize, shentsize = 64, 56, 64
    phoff = ehsize
    shnum = 5

    eh = b"\x7fELF" + bytes([2, 1, 1, 0, 0]) + b"\0" * 7
    eh += struct.pack(
        "<HHIQQQIHHHHHH",
        ET_DYN, EM_AARCH64, 1, args.entrada, phoff, shoff, 0,
        ehsize, phentsize, 1, shentsize, shnum, 4,
    )
    # el PT_LOAD tiene que mapear los offsets de FICHERO a los VMA de forma
    # coherente: si no, retdec no encuentra la seccion del punto de entrada.
    ph = struct.pack(
        "<IIQQQQQQ", PT_LOAD, 5, off_text, args.vma, args.vma,
        len(texto), len(texto), 0x1000,
    )
    sh_null = b"\0" * shentsize
    # OJO: el PRIMER campo de Elf64_Shdr es sh_name (indice en .shstrtab),
    # no el tipo.  "\0.text\0.shstrtab\0" => ".text" en 1, ".shstrtab" en 7.
    sh_text = struct.pack(
        "<IIQQQQIIQQ", 1, SHT_PROGBITS, SHF_ALLOC | SHF_EXECINSTR, args.vma,
        off_text, len(texto), 0, 0, 4, 0,
    )
    sh_strtab = struct.pack(
        "<IIQQQQIIQQ", 23, SHT_STRTAB, 0, 0, off_shstr, len(shstr), 0, 0, 1, 0,
    )
    sh_symtab = struct.pack(
        "<IIQQQQIIQQ", 7, SHT_SYMTAB, 0, 0, off_symtab, 24 * 2, 4, 1, 8, 24,
    )
    sh_strtab2 = struct.pack(
        "<IIQQQQIIQQ", 15, SHT_STRTAB, 0, 0, off_strtab, len(strtab), 0, 0, 1, 0,
    )
    # Elf64_Sym: st_name(4) st_info(1) st_other(1) st_shndx(2) st_value(8) st_size(8)
    sym_null = b"\0" * 24
    sym_func = struct.pack(
        "<IBBHQQ", sym_name, 0x12, 0, 1, args.entrada, args.tamano
    )

    out = bytearray(shoff + 5 * shentsize)
    out[0:ehsize] = eh
    out[phoff:phoff + phentsize] = ph
    out[off_text:off_text + len(texto)] = texto
    out[off_shstr:off_shstr + len(shstr)] = shstr
    out[off_symtab:off_symtab + 24] = sym_null
    out[off_symtab + 24:off_symtab + 48] = sym_func
    out[off_strtab:off_strtab + len(strtab)] = strtab
    out[shoff:shoff + shentsize] = sh_null
    out[shoff + shentsize:shoff + 2 * shentsize] = sh_text
    out[shoff + 2 * shentsize:shoff + 3 * shentsize] = sh_symtab
    out[shoff + 3 * shentsize:shoff + 4 * shentsize] = sh_strtab2
    out[shoff + 4 * shentsize:shoff + 5 * shentsize] = sh_strtab

    args.salida.parent.mkdir(parents=True, exist_ok=True)
    args.salida.write_bytes(bytes(out))
    print(
        "[elf-minimo] %s: %d B de .text en vma=0x%x, entrada=0x%x"
        % (args.salida, len(texto), args.vma, args.entrada)
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

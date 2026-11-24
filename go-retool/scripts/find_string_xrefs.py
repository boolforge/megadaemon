#!/usr/bin/env python3
"""Localiza las referencias (xrefs) a una cadena dentro del .text de un ELF AArch64.

POR QUE
Ghidra, sin analisis automatico, no crea referencias a los datos; y el
pseudocodigo de Go confunde el "blob" unico de cadenas (Go mete TODAS las
cadenas constantes en un unico bloque contiguo, asi que cada constante es un
par (puntero, longitud) y el puntero cae dentro del blob).  Para responder
"que funcion usa esta cadena" hace falta el escaneo directo.

METODO
En AArch64 una constante de cadena se referencia con:
    ADRP Xd, <pagina>        ; 0x90000000
    ADD  Xd, Xd, #<offset>  ; 0x91000000
asi que la direccion final es pagina + offset.  Este script:
  1. traduce vaddr -> offset de fichero con las cabeceras de programa,
  2. recorre el .text instruccion a instruccion,
  3. remembers cada ADRP (destino -> pagina) y, cuando ve un ADD con el
     mismo registro base, calcula la direccion final y la busca en la tabla
     de cadenas que se le pasa.

Uso:
  python3 find_string_xrefs.py <binario> <cadena> [<cadena> ...]
"""

import struct
import sys


class Elf(object):
    def __init__(self, path):
        self.f = open(path, "rb")
        d = self.f.read(64)
        assert d[:4] == b"\x7fELF", "no es ELF"
        self.e_phoff = struct.unpack_from("<Q", d, 0x20)[0]
        self.e_phentsize = struct.unpack_from("<H", d, 0x36)[0]
        self.e_phnum = struct.unpack_from("<H", d, 0x38)[0]
        self.e_shoff = struct.unpack_from("<Q", d, 0x28)[0]
        self.e_shentsize = struct.unpack_from("<H", d, 0x3A)[0]
        self.e_shnum = struct.unpack_from("<H", d, 0x3C)[0]
        self.e_shstrndx = struct.unpack_from("<H", d, 0x3E)[0]
        self.segs = []
        self.f.seek(self.e_phoff)
        for _ in range(self.e_phnum):
            ph = self.f.read(self.e_phentsize)
            p_type = struct.unpack_from("<I", ph, 0)[0]
            p_offset, p_vaddr, _pa, p_filesz, p_memsz = struct.unpack_from("<QQQQQ", ph, 8)
            if p_type == 1:
                self.segs.append((p_vaddr, p_offset, p_filesz, p_memsz))
        self.sections = {}
        self.f.seek(self.e_shoff)
        raw = self.f.read(self.e_shentsize * self.e_shnum)
        names = b""
        if self.e_shstrndx < self.e_shnum:
            off, size = struct.unpack_from("<QQ", raw, self.e_shstrndx * self.e_shentsize + 24)
            self.f.seek(off)
            names = self.f.read(size)
        for i in range(self.e_shnum):
            base = i * self.e_shentsize
            nameoff, stype = struct.unpack_from("<II", raw, base)
            saddr, soff, ssize = struct.unpack_from("<QQQ", raw, base + 16)
            end = names.find(b"\0", nameoff)
            nm = names[nameoff:end].decode("ascii", "replace")
            self.sections[nm] = (stype, saddr, soff, ssize)

    def va2off(self, va):
        for v, o, fsz, msz in self.segs:
            if v <= va < v + fsz:
                return o + (va - v)
        return None

    def read_va(self, va, n):
        o = self.va2off(va)
        if o is None:
            return None
        self.f.seek(o)
        return self.f.read(n)


def find_all(hay, needle):
    out = []
    i = hay.find(needle)
    while i >= 0:
        out.append(i)
        i = hay.find(needle, i + 1)
    return out


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    path = sys.argv[1]
    needles = [a.encode("utf-8") for a in sys.argv[2:]]
    elf = Elf(path)

    # 1. donde esta cada cadena (en cualquier segmento cargado)
    targets = {}
    for nd in needles:
        for v, o, fsz, msz in elf.segs:
            data = elf.read_va(v, fsz)
            if data is None:
                continue
            for pos in find_all(data, nd):
                va = v + pos
                targets.setdefault(va, []).append(nd.decode("utf-8"))
    if not targets:
        print("cadena no encontrada en el binario")
        return 1
    for va in sorted(targets):
        print("cadena %r en VA 0x%x" % (targets[va][0], va))

    # 2. escaneo del .text: ADRP + ADD
    stype, saddr, soff, ssize = elf.sections.get(".text", (0, 0, 0, 0))
    if not ssize:
        print("no hay .text")
        return 1
    text = elf.read_va(saddr, ssize)
    base = saddr
    # Ventana de ADRP recientes: el par ADRP+ADD no siempre es contiguo
    # (el compilador mete instrucciones entre medias).
    paginas = {}
    hits = {}
    VENTANA = 32
    for i in range(0, len(text) - 4, 4):
        insn = struct.unpack_from("<I", text, i)[0]
        if (insn & 0x9F000000) == 0x90000000:  # ADRP
            immlo = (insn >> 29) & 3
            immhi = (insn >> 5) & 0x7FFFF
            imm = (immhi << 2) | immlo
            if imm & (1 << 20):
                imm -= 1 << 21
            pc = base + i
            pagina = (pc & ~0xFFF) + (imm << 12)
            reg = insn & 0x1F
            paginas[(i, reg)] = pagina
            continue
        if (insn & 0x9F000000) == 0x10000000:  # ADR
            immlo = (insn >> 29) & 3
            immhi = (insn >> 5) & 0x7FFFF
            imm = (immhi << 2) | immlo
            if imm & (1 << 20):
                imm -= 1 << 21
            va = base + i + imm
            if va in targets:
                hits.setdefault(va, []).append(base + i)
            continue
        if (insn & 0xFF000000) == 0x91000000:  # ADD (inmediato, 64 bits)
            rd = insn & 0x1F
            rn = (insn >> 5) & 0x1F
            imm12 = (insn >> 10) & 0xFFF
            shift = (insn >> 22) & 1
            if shift:
                imm12 <<= 12
            pag = None
            for j in range(1, VENTANA + 1):
                cand = paginas.get((i - 4 * j, rn))
                if cand is not None:
                    pag = cand
                    break
            if pag is None:
                continue
            va = pag + imm12
            if va in targets:
                hits.setdefault(va, []).append(base + i - 4 * j)
    for va in sorted(hits):
        print("xref DIRECTA a 0x%x (%s) desde:" % (va, targets[va][0]))
        for x in sorted(hits[va]):
            print("    0x%x" % x)

    # 3. Segunda pasada: en Go la constante de cadena suele quedar en un
    #    "slot" de 8 bytes (el puntero) dentro de .data.rel.ro, y el codigo
    #    referencia ESE slot, no la cadena.  Se buscan los punteros a la
    #    cadena y luego las referencias a los slots.
    slots = {}
    for va in list(targets):
        needle = struct.pack("<Q", va)
        for v, o, fsz, msz in elf.segs:
            data = elf.read_va(v, fsz)
            if data is None:
                continue
            pos = 0
            while True:
                pos = data.find(needle, pos)
                if pos < 0:
                    break
                slot = v + pos
                slots.setdefault(slot, []).append(va)
                pos += 1
    if slots:
        print("slots (punteros a la cadena): %d" % len(slots))
        # referencias a los slots
        slot_hits = {}
        paginas2 = {}
        for i in range(0, len(text) - 4, 4):
            insn = struct.unpack_from("<I", text, i)[0]
            if (insn & 0x9F000000) == 0x90000000:
                immlo = (insn >> 29) & 3
                immhi = (insn >> 5) & 0x7FFFF
                imm = (immhi << 2) | immlo
                if imm & (1 << 20):
                    imm -= 1 << 21
                pc = base + i
                paginas2[(i, insn & 0x1F)] = (pc & ~0xFFF) + (imm << 12)
                continue
            if (insn & 0xFF000000) == 0x91000000:
                rd = insn & 0x1F
                rn = (insn >> 5) & 0x1F
                imm12 = (insn >> 10) & 0xFFF
                if (insn >> 22) & 1:
                    imm12 <<= 12
                for j in range(1, VENTANA + 1):
                    pag = paginas2.get((i - 4 * j, rn))
                    if pag is None:
                        continue
                    va = pag + imm12
                    for cand in (va, va - 8):
                        if cand in slots:
                            slot_hits.setdefault(cand, []).append(base + i - 4 * j)
                    break
        for slot in sorted(slot_hits):
            print("xref a SLOT 0x%x (cadena 0x%x) desde:" % (slot, slots[slot][0]))
            for x in sorted(set(slot_hits[slot])):
                print("    0x%x" % x)
    if not hits and not slots:
        print("sin referencias")
    return 0


if __name__ == "__main__":
    sys.exit(main())

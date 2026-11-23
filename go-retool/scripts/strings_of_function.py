#!/usr/bin/env python3
"""Cadenas y globales que usa una funcion Go, leidos del binario.

POR QUE
En Go todas las constantes de cadena viven en un unico blob contiguo de
.rodata y cada constante es un par (puntero, longitud).  El codigo las carga
con ADRP + ADD (direccion directa) o con ADRP + LDR (lee el puntero que hay en
.data.rel.ro).  Sin referencias de datos, Ghidra las muestra como UNK_..., asi
que para responder "que cadenas usa esta funcion" hay que mirar las
instrucciones.

METODO
Recorre el rango de la funcion carrying el estado de los registros:
  * ADRP Xd, pag  ->  remember pagina(Xd)
  * ADD  Xd, Xn, #imm  ->  si pagina(Xn) existe, la direccion final es
                           pagina + imm (y la instruccion consume pagina(Xd))
  * LDR  Xt, [Xn, #imm] ->  la direccion del slot es pagina(Xn) + imm; el
                           valor del slot puede ser el puntero a la cadena
Cada referencia encontrada se contrasta con el binario: si en esa direccion
hay texto imprimible, es una cadena; si no, es un global.

Uso:
  python3 strings_of_function.py <binario> <tsv_de_simbolos> <va_inicio> [<va_fin>]
"""

import struct
import sys

# mascaras de las instrucciones que nos interesan
M_ADRP = 0x9F000000
V_ADRP = 0x90000000
M_ADR = 0x9F000000
V_ADR = 0x10000000
M_ADD = 0xFF000000
V_ADD = 0x91000000
M_LDR = 0x3B000000
V_LDR = 0x39000000
M_STR = 0x3B000000
V_STR = 0x380000000 % (1 << 32)


class Elf(object):
    def __init__(self, path):
        self.f = open(path, "rb")
        d = self.f.read(64)
        assert d[:4] == b"\x7fELF"
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
            t = struct.unpack_from("<I", ph, 0)[0]
            off, va, _pa, fsz, msz = struct.unpack_from("<QQQQQ", ph, 8)
            if t == 1:
                self.segs.append((va, off, fsz, msz))
        self.sections = {}
        self.f.seek(self.e_shoff)
        raw = self.f.read(self.e_shentsize * self.e_shnum)
        names = b""
        if self.e_shstrndx < self.e_shnum:
            off, size = struct.unpack_from("<QQ", raw, self.e_shstrndx * self.e_shentsize + 24)
            self.f.seek(off)
            names = self.f.read(size)
        for i in range(self.e_shnum):
            b = i * self.e_shentsize
            noff, stype = struct.unpack_from("<II", raw, b)
            saddr, soff, ssize = struct.unpack_from("<QQQ", raw, b + 16)
            end = names.find(b"\0", noff)
            self.sections[names[noff:end].decode("ascii", "replace")] = (stype, saddr, soff, ssize)

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

    def u64(self, va):
        b = self.read_va(va, 8)
        return struct.unpack("<Q", b)[0] if b and len(b) == 8 else None

    def cadena(self, va, maxlen=200):
        b = self.read_va(va, maxlen)
        if b is None:
            return None
        out = []
        for c in b:
            if c < 0x20 or c > 0x7E:
                break
            out.append(chr(c))
        return "".join(out) if out else None


def escanear(elf, ini, fin):
    """Devuelve [(pc, tipo, va)] con las direcciones referenciadas."""
    data = elf.read_va(ini, fin - ini)
    paginas = {}   # registro -> (pc, pagina)
    refs = []
    for i in range(0, len(data) - 4, 4):
        insn = struct.unpack_from("<I", data, i)[0]
        pc = ini + i
        rd = insn & 0x1F
        rn = (insn >> 5) & 0x1F
        if (insn & M_ADRP) == V_ADRP:
            immlo = (insn >> 29) & 3
            immhi = (insn >> 5) & 0x7FFFF
            imm = (immhi << 2) | immlo
            if imm & (1 << 20):
                imm -= 1 << 21
            paginas[rd] = (pc, (pc & ~0xFFF) + (imm << 12))
            continue
        if (insn & M_ADR) == V_ADR:
            immlo = (insn >> 29) & 3
            immhi = (insn >> 5) & 0x7FFFF
            imm = (immhi << 2) | immlo
            if imm & (1 << 20):
                imm -= 1 << 21
            refs.append((pc, "ADR", pc + imm))
            continue
        if (insn & M_ADD) == V_ADD:
            imm12 = (insn >> 10) & 0xFFF
            if (insn >> 22) & 1:
                imm12 <<= 12
            info = paginas.get(rn)
            if info is not None:
                refs.append((info[0], "ADRP+ADD", info[1] + imm12))
            paginas.pop(rd, None)
            continue
        if (insn & M_LDR) == V_LDR:
            imm12 = (insn >> 10) & 0xFFF
            if (insn >> 22) & 1:
                imm12 <<= 12
            info = paginas.get(rn)
            if info is not None:
                slot = info[1] + imm12
                refs.append((info[0], "ADRP+LDR", slot))
            paginas.pop(rd, None)
            continue
        if (insn & 0x7C000000) == 0x14000000 or (insn & 0xFE000000) == 0x54000000:
            # BL / B / BLR / BR / RET: se invalidan los registros vivos
            paginas.clear()
            continue
        if (insn & M_STR) == V_STR:
            continue
        paginas.pop(rd, None)
    return refs


def main():
    if len(sys.argv) < 4:
        print(__doc__)
        return 2
    binpath, tsv = sys.argv[1], sys.argv[2]
    va_ini = int(sys.argv[3], 16)
    filas = []
    for line in open(tsv, encoding="utf-8"):
        p = line.rstrip("\n").split("\t")
        filas.append((int(p[0], 16), int(p[5]), p[1], p[2]))
    filas.sort()
    nombre = loc = None
    ini = fin = None
    for v, sz, nom, lo in filas:
        if v == va_ini:
            ini, fin, nombre, loc = v, v + sz, nom, lo
            break
    if ini is None:
        print("la direccion 0x%x no es el inicio de ninguna funcion" % va_ini)
        return 1
    if len(sys.argv) > 4:
        fin = int(sys.argv[4], 16)
    print("funcion: %s" % nombre)
    print("origen : %s" % loc)
    print("rango  : 0x%x .. 0x%x (%d bytes)" % (ini, fin, fin - ini))

    elf = Elf(binpath)
    refs = escanear(elf, ini, fin)
    vistas = set()
    print("--- cadenas referenciadas ---")
    for pc, tipo, va in refs:
        if va in vistas:
            continue
        vistas.add(va)
        s = elf.cadena(va)
        if s and len(s) >= 3:
            print("  0x%-10x %-9s pc=0x%-10x %r" % (va, tipo, pc, s[:150]))
    print("--- punteros a cadenas (ADRP+LDR) ---")
    for pc, tipo, va in refs:
        if tipo != "ADRP+LDR" or va in vistas:
            continue
        vistas.add(va)
        val = elf.u64(va)
        if val is None:
            continue
        s = elf.cadena(val)
        if s and len(s) >= 3:
            print("  slot 0x%-10x pc=0x%-10x -> 0x%x %r" % (va, pc, val, s[:150]))
    print("--- otras direcciones referenciadas ---")
    n = 0
    for pc, tipo, va in refs:
        if va in vistas:
            continue
        vistas.add(va)
        s = elf.cadena(va)
        if s and len(s) >= 3:
            continue
        n += 1
        if n <= 40:
            print("  0x%-10x %-9s pc=0x%-10x" % (va, tipo, pc))
    return 0


if __name__ == "__main__":
    sys.exit(main())

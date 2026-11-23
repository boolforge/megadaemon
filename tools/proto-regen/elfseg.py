#!/usr/bin/env python3
"""Lector MINIMO de ELF64 (sin dependencias) para el extractor de protobuf.

El binario analizado (`liblanguage_server.so`, ELF ARM64, Go 1.28, sin DWARF)
no tiene tabla de simbolos `.symtab` (solo `.dynsym` con 858 entradas de la
GLIBC), asi que los datos utiles para encontrar los descriptores embebidos
estan en los *segmentos* PT_LOAD, no en las secciones de nombres.

Este modulo da:
  * los segmentos PT_LOAD (vaddr, offset, filesz, flags) para poder mapear
    direccion virtual <-> offset de fichero;
  * los nombres de seccion (para saber cuales son de solo lectura).

No pretende ser un parser de ELF completo: solo lo justo para el trabajo.
"""
import struct

# flags de PT_LOAD
PF_X, PF_W, PF_R = 0x1, 0x2, 0x4

# tipos de segmento
PT_LOAD = 1
PT_NOTE = 4

SHT_PROGBITS = 1
SHT_NOBITS = 8


class Segment:
    __slots__ = ("vaddr", "offset", "filesz", "memsz", "flags", "align")

    def __init__(self, vaddr, offset, filesz, memsz, flags, align):
        self.vaddr = vaddr
        self.offset = offset
        self.filesz = filesz
        self.memsz = memsz
        self.flags = flags
        self.align = align

    def __repr__(self):  # pragma: no cover - diagnostico
        return (f"Segment(vaddr={self.vaddr:#x} off={self.offset:#x} "
                f"filesz={self.filesz:#x} flags={self.flags:#x})")

    def contiene(self, off, n=1):
        return self.offset <= off and off + n <= self.offset + self.filesz


class Elf:
    """ELF de 64 bits (little-endian)."""

    def __init__(self, path):
        self.path = path
        with open(path, "rb") as fh:
            self.data = fh.read()
        d = self.data
        if len(d) < 64 or d[:4] != b"\x7fELF":
            raise ValueError(f"{path}: no es un ELF")
        if d[4] != 2:
            raise ValueError(f"{path}: sólo se soporta ELF64 (EI_CLASS={d[4]})")
        if d[5] != 1:
            raise ValueError(f"{path}: sólo se soporta little-endian")
        self.e_type, self.e_machine = struct.unpack_from("<HH", d, 16)
        self.e_entry, self.e_phoff, self.e_shoff = struct.unpack_from("<QQQ", d, 24)
        self.e_phentsize, self.e_phnum = struct.unpack_from("<HH", d, 54)
        self.e_shentsize, self.e_shnum, self.e_shstrndx = struct.unpack_from("<HHH", d, 58)
        self.segments = []
        for i in range(self.e_phnum):
            off = self.e_phoff + i * self.e_phentsize
            p_type, p_flags = struct.unpack_from("<II", d, off)
            p_offset, p_vaddr, _p_paddr, p_filesz, p_memsz, p_align = struct.unpack_from(
                "<QQQQQQ", d, off + 8)
            if p_type == PT_LOAD:
                self.segments.append(Segment(p_vaddr, p_offset, p_filesz, p_memsz,
                                            p_flags, p_align))
        self.sections = self._leer_secciones()

    def _leer_secciones(self):
        d = self.data
        secs = []
        if not self.e_shoff or not self.e_shnum:
            return secs
        for i in range(self.e_shnum):
            off = self.e_shoff + i * self.e_shentsize
            (sh_name, sh_type, sh_flags, sh_addr, sh_offset, sh_size) = struct.unpack_from(
                "<IIQQQQ", d, off)
            secs.append({"name_off": sh_name, "type": sh_type, "flags": sh_flags,
                         "addr": sh_addr, "offset": sh_offset, "size": sh_size})
        # tabla de nombres
        nombres = b""
        if self.e_shstrndx < len(secs):
            s = secs[self.e_shstrndx]
            nombres = d[s["offset"]:s["offset"] + s["size"]]
        for s in secs:
            fin = nombres.find(b"\x00", s["name_off"])
            s["name"] = nombres[s["name_off"]:fin].decode("utf-8", "replace")
        return secs

    def segmentos_solo_lectura(self):
        """Segmentos PT_LOAD sin permiso de escritura (contenen .rodata)."""
        return [s for s in self.segments if not (s.flags & PF_W)]

    def secciones_ro(self):
        """Secciones de solo lectura con contenido en fichero."""
        out = []
        for s in self.sections:
            if s["type"] != SHT_PROGBITS:
                continue
            if s["flags"] & 0x2:  # SHF_WRITE
                continue
            if s["size"] == 0:
                continue
            out.append(s)
        return out

    def vaddr_a_offset(self, vaddr):
        for s in self.segments:
            if s.vaddr <= vaddr < s.vaddr + s.filesz:
                return s.offset + (vaddr - s.vaddr)
        return None

    def offset_a_vaddr(self, off):
        for s in self.segments:
            if s.offset <= off < s.offset + s.filesz:
                return s.vaddr + (off - s.offset)
        return None


if __name__ == "__main__":  # pragma: no cover - diagnostico
    import sys
    e = Elf(sys.argv[1])
    print(f"machine={e.e_machine:#x} type={e.e_type} segmentos={len(e.segments)}")
    for s in e.segments:
        print(" ", s)
    for s in e.secciones_ro():
        print(f"  sec {s['name']:<16} off={s['offset']:#x} size={s['size']:#x}")

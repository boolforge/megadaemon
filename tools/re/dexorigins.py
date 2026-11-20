#!/usr/bin/env python3
"""dexorigins.py - de donde viene cada cadena en un DEX (clase + metodo).

Para un .dex suelto o un .apk (extrae classes*.dex). Sin Java, sin baksmali, sin deps.

    python3 tools/re/dexorigins.py <fichero.dex|apk> <filtro> [--limit N] [--classes-only]

Devuelve, para cada metodo que *referencia* una cadena que casa con el filtro:
    clase  Lcom/x/Y;   metodo  nombre   ->  cadenas que casa

Es la via para responder preguntas como "que clase deriva la clave AEGIS"
o "que metodo lee el flag CscFeature_X" sin desensamblar a mano.

LIMITACIONES CONOCIDAS (medidas sobre un DEX real de 6 MB, 38.816 cadenas):
  * Las constantes de tipo `static final String` de Java/Kotlin NO aparecen como
    const-string en el codigo, sino en el blob `static_values` de la clase.
    Este lector los recorre (encoded_array), pero su cobertura es parcial.
  * Una cadena puede estar en la tabla y NO referenciada desde el codigo
    (p.ej. una URL de un campo estatico no inicializado en el sitio esperado):
    para eso hay que cruzarla con `resources.arsc` (los recursos SI llevan URLs).
  * El opcode de una instruccion DEX va en el BYTE ALTO del primer halfword
    (`AA|op`); tomar el byte bajo escanea basura. Ojo con esto.
"""
import struct, sys, zipfile, os, re

def uleb(b, o):
    r = 0; s = 0
    while True:
        x = b[o]; o += 1; r |= (x & 0x7f) << s; s += 7
        if not x & 0x80:
            return r, o

class Dex:
    def __init__(self, blob):
        self.b = blob
        U = lambda off: struct.unpack_from("<I", blob, off)[0]
        self.ssz, self.soff = U(0x38), U(0x3c)
        self.msz, self.moff = U(0x58), U(0x5c)
        self.csz, self.coff = U(0x60), U(0x64)
        self.tsz, self.toff = U(0x40), U(0x44)
        self.strs = []
        for i in range(self.ssz):
            off = struct.unpack_from("<I", blob, self.soff + 4 * i)[0]
            n, o = uleb(blob, off)
            e = blob.index(b"\x00", o)
            self.strs.append(blob[o:e].decode("utf-8", "replace"))

    def type_name(self, i):
        return self.strs[struct.unpack_from("<I", self.b, self.toff + 4 * i)[0]]

    def method(self, i):
        ci, _pi, ni = struct.unpack_from("<HHI", self.b, self.moff + 8 * i)
        return self.type_name(ci), self.strs[ni]

    def const_strings(self, code_off):
        """Cadenas referenciadas por const-string (0x1a) y const-string/jumbo (0x1b)."""
        b = self.b
        if not code_off or code_off + 16 > len(b):
            return []
        ins = struct.unpack_from("<I", b, code_off + 12)[0]
        if ins <= 0 or ins > (len(b) - code_off - 16) // 2:
            return []
        units = struct.unpack_from("<%dH" % ins, b, code_off + 16)
        out = []
        # OJO: en DEX el opcode va en el BYTE ALTO del primer halfword
        # (formato "AA|op BBBB"), no en el bajo. Tomar u & 0xFF escanea basura.
        for i, u in enumerate(units):
            op = u >> 8
            if op == 0x1a:                    # const-string
                out.append(u & 0xFFFF)
            elif op == 0x1b and i + 1 < len(units):   # const-string/jumbo
                out.append(((units[i + 1] << 16) | (u & 0xFFFF)) & 0xFFFFFFFF)
        return out

    def static_value_strings(self, off):
        """Cadenas en el blob `static_values` de una clase (encoded_array).

        Clave: en Kotlin/Java un `const val`/`static final String` NO aparece
        como const-string en el codigo, sino como constante en el
        `static_values_off` de la clase => sin esto se pierde justo lo
        interesante (URLs, claves, endpoints).
        Tipos de valor: 0x17=STRING(u16 idx), 0x1c=ARRAY, 0x1d=ANNOTATION,
        0x1e=NULL, 0x1f=BOOLEAN(1 byte)...
        """
        b = self.b
        if not off or off + 4 > len(b):
            return []
        out = []
        try:
            n = struct.unpack_from("<I", b, off)[0]
            o = off + 4
            for _ in range(n):
                o, idxs = self._value(o)
                out.extend(idxs)
        except Exception:
            pass
        return out

    def _value(self, o):
        """Devuelve (posicion_siguiente, [idx_cadena]) de un encoded_value."""
        b = self.b
        head = b[o]; o += 1
        typ = head & 0x1F; arg = head >> 5
        if typ == 0x17:      # STRING
            idx = struct.unpack_from("<H", b, o)[0]
            return o + 2, [idx]
        if typ == 0x18:      # TYPE
            return o + 2 + arg * 2, []
        if typ in (0x19, 0x1a, 0x1b):   # FIELD/METHOD/ENUM
            return o + 2 + arg * 4, []
        if typ == 0x15 or typ == 0x16:  # METHOD_TYPE / METHOD_HANDLE
            return o + 2, []
        if typ == 0x1c:      # ARRAY
            n = struct.unpack_from("<I", b, o)[0]
            o += 4
            idxs = []
            for _ in range(n):
                o, got = self._value(o)
                idxs.extend(got)
            return o, idxs
        if typ == 0x1d:      # ANNOTATION
            o += 1  # type_idx
            n = struct.unpack_from("<I", b, o)[0]; o += 4
            for _ in range(n):
                o += 2  # name_idx
                o, _g = self._value(o)
            return o, []
        if typ == 0x1e:      # NULL
            return o, []
        if typ == 0x1f:      # BOOLEAN
            return o + 1, []
        if typ == 0x00:      # BYTE
            return o + 1, []
        if typ == 0x02 or typ == 0x03:
            return o + 2, []
        if typ == 0x04 or typ == 0x10:
            return o + 4, []
        if typ == 0x06 or typ == 0x11:
            return o + 8, []
        return o, []

    def walk(self):
        """(clase, metodo, [idx_cadena]) por cada metodo con codigo."""
        b = self.b
        for c in range(self.csz):
            try:
                cidx, _af, _sc, _io, _sfo, _ano, cdo, svo = struct.unpack_from("<8I", b, self.coff + 32 * c)
                if not cdo or cdo + 8 > len(b):
                    continue
                cls = self.type_name(cidx)
                if svo:
                    for i in self.static_value_strings(svo):
                        yield cls, "<static_values>", [i]
                o = cdo
                sf, o = uleb(b, o); inf, o = uleb(b, o); dm, o = uleb(b, o); vm, o = uleb(b, o)
                for _ in range(sf):
                    _, o = uleb(b, o); _, o = uleb(b, o)
                for _ in range(inf):
                    _, o = uleb(b, o); _, o = uleb(b, o)
                for cnt in (dm, vm):
                    midx = 0
                    for _ in range(cnt):
                        d, o = uleb(b, o); _a, o = uleb(b, o); co, o = uleb(b, o)
                        midx += d
                        if not co or co + 16 > len(b):
                            continue
                        try:
                            _cls, mname = self.method(midx)
                        except Exception:
                            continue
                        yield cls, mname, self.const_strings(co)
            except Exception:
                continue

def load(path):
    if path.lower().endswith(".apk"):
        out = []
        z = zipfile.ZipFile(path)
        for n in z.namelist():
            if n.endswith(".dex"):
                out.append((n, z.read(n)))
        return out
    return [(os.path.basename(path), open(path, "rb").read())]

def main():
    if len(sys.argv) < 3:
        print(__doc__); sys.exit(2)
    path, filt = sys.argv[1], sys.argv[2].lower()
    limit = 40; classes_only = "--classes-only" in sys.argv
    if "--limit" in sys.argv:
        limit = int(sys.argv[sys.argv.index("--limit") + 1])
    for name, blob in load(path):
        if blob[:4] != b"dex\n":
            print("  (%s no es un DEX)" % name); continue
        d = Dex(blob)
        hits = {}
        for cls, mname, idxs in d.walk():
            for i in idxs:
                if i >= len(d.strs):
                    continue
                s = d.strs[i]
                if filt in s.lower():
                    hits.setdefault(cls, {}).setdefault(mname, set()).add(s)
        print("== %s (%d bytes, %d cadenas) :: %d clases con '%s'" % (
            name, len(blob), len(d.strs), len(hits), filt))
        n = 0
        for cls, meths in hits.items():
            if classes_only:
                print("   %s" % cls); n += 1
            else:
                for m, ss in meths.items():
                    print("   %-58s :: %-30s -> %s" % (cls, m, list(ss)[:2]))
                    n += 1
            if n >= limit:
                print("   ... (limite %d)" % limit); break
        if n == 0:
            print("   (sin coincidencias)")

if __name__ == "__main__":
    main()

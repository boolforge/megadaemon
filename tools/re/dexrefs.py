#!/usr/bin/env python3
"""Busca en que clase+metodo se referencia cada cadena que casa con un filtro.

Correccion sobre dexorigins.py: la atribucion se hace recorriendo los
class_defs y sus code_item (clase y metodo salen del propio class_def), no
del method_id global (que se mezcla al escanear code items ajenos).

uso: dexrefs.py <file.dex> [filtro ...]
"""
import struct, sys

W = {
 0x00:1,
 0x01:1,
 0x02:2,
 0x03:3,
 0x04:1,
 0x05:2,
 0x06:3,
 0x07:1,
 0x08:2,
 0x09:3,
 0x0a:1,
 0x0b:1,
 0x0c:1,
 0x0d:1,
 0x0e:1,
 0x0f:1,
 0x10:1,
 0x11:1,
 0x12:1,
 0x13:2,
 0x14:3,
 0x15:2,
 0x16:2,
 0x17:3,
 0x18:5,
 0x19:2,
 0x1a:2,
 0x1b:3,
 0x1c:2,
 0x1d:1,
 0x1e:1,
 0x1f:2,
 0x20:2,
 0x21:1,
 0x22:2,
 0x23:2,
 0x24:3,
 0x25:3,
 0x26:3,
 0x27:1,
 0x28:2,
 0x29:2,
 0x2a:3,
 0x2b:3,
 0x2c:3,
 0x2d:2,
 0x2e:2,
 0x2f:2,
 0x30:2,
 0x31:2,
 0x32:2,
 0x33:2,
 0x34:2,
 0x35:2,
 0x36:2,
 0x37:2,
 0x38:2,
 0x39:2,
 0x3a:2,
 0x3b:2,
 0x3c:2,
 0x3d:2,
 0x3e:1,
 0x3f:1,
 0x40:1,
 0x41:1,
 0x42:1,
 0x43:1,
 0x44:2,
 0x45:2,
 0x46:2,
 0x47:2,
 0x48:2,
 0x49:2,
 0x4a:2,
 0x4b:2,
 0x4c:2,
 0x4d:2,
 0x4e:2,
 0x4f:2,
 0x50:2,
 0x51:2,
 0x52:2,
 0x53:2,
 0x54:2,
 0x55:2,
 0x56:2,
 0x57:2,
 0x58:2,
 0x59:2,
 0x5a:2,
 0x5b:2,
 0x5c:2,
 0x5d:2,
 0x5e:2,
 0x5f:2,
 0x60:2,
 0x61:2,
 0x62:2,
 0x63:2,
 0x64:2,
 0x65:2,
 0x66:2,
 0x67:2,
 0x68:2,
 0x69:2,
 0x6a:2,
 0x6b:2,
 0x6c:2,
 0x6d:2,
 0x6e:3,
 0x6f:3,
 0x70:3,
 0x71:3,
 0x72:3,
 0x73:1,
 0x74:3,
 0x75:3,
 0x76:3,
 0x77:3,
 0x78:3,
 0x79:1,
 0x7a:1,
 0x7b:1,
 0x7c:1,
 0x7d:1,
 0x7e:1,
 0x7f:1,
 0x80:1,
 0x81:1,
 0x82:1,
 0x83:1,
 0x84:1,
 0x85:1,
 0x86:1,
 0x87:1,
 0x88:1,
 0x89:1,
 0x8a:1,
 0x8b:1,
 0x8c:1,
 0x8d:1,
 0x8e:1,
 0x8f:1,
 0x90:2,
 0x91:2,
 0x92:2,
 0x93:2,
 0x94:2,
 0x95:2,
 0x96:2,
 0x97:2,
 0x98:2,
 0x99:2,
 0x9a:2,
 0x9b:2,
 0x9c:2,
 0x9d:2,
 0x9e:2,
 0x9f:2,
 0xa0:2,
 0xa1:2,
 0xa2:2,
 0xa3:2,
 0xa4:2,
 0xa5:2,
 0xa6:2,
 0xa7:2,
 0xa8:2,
 0xa9:2,
 0xaa:2,
 0xab:2,
 0xac:2,
 0xad:2,
 0xae:2,
 0xaf:2,
 0xb0:1,
 0xb1:1,
 0xb2:1,
 0xb3:1,
 0xb4:1,
 0xb5:1,
 0xb6:1,
 0xb7:1,
 0xb8:1,
 0xb9:1,
 0xba:1,
 0xbb:1,
 0xbc:1,
 0xbd:1,
 0xbe:1,
 0xbf:1,
 0xc0:1,
 0xc1:1,
 0xc2:1,
 0xc3:1,
 0xc4:1,
 0xc5:1,
 0xc6:1,
 0xc7:1,
 0xc8:1,
 0xc9:1,
 0xca:1,
 0xcb:1,
 0xcc:1,
 0xcd:1,
 0xce:1,
 0xcf:1,
 0xd0:2,
 0xd1:2,
 0xd2:2,
 0xd3:2,
 0xd4:2,
 0xd5:2,
 0xd6:2,
 0xd7:2,
 0xd8:2,
 0xd9:2,
 0xda:2,
 0xdb:2,
 0xdc:2,
 0xdd:2,
 0xde:2,
 0xdf:2,
 0xe0:2,
 0xe1:2,
 0xe2:2,
 0xe3:2,
 0xe4:2,
 0xe5:2,
 0xe6:2,
 0xe7:2,
 0xe8:2,
 0xe9:2,
 0xea:2,
 0xeb:2,
 0xec:1,
 0xed:2,
 0xee:3,
 0xef:3,
 0xf0:3,
 0xf1:1,
 0xf2:2,
 0xf3:1,
 0xf4:1,
 0xf5:1,
 0xf6:1,
 0xf7:1,
 0xf8:1,
 0xf9:1,
 0xfa:4,
 0xfb:4,
 0xfc:3,
 0xfd:3,
 0xfe:2,
 0xff:2,
}

def main(path, needles):
    d = open(path, "rb").read()
    def u16(p): return struct.unpack_from("<H", d, p)[0]
    def u32(p): return struct.unpack_from("<I", d, p)[0]
    def uleb(p):
        r = 0; s = 0
        while True:
            b = d[p]; p += 1; r |= (b & 0x7F) << s
            if not (b & 0x80): break
            s += 7
        return r, p
    n_str, str_off = u32(0x38), u32(0x3c)
    n_type, type_off = u32(0x40), u32(0x44)
    n_meth, meth_off = u32(0x58), u32(0x5c)
    n_class, class_off = u32(0x60), u32(0x64)
    strings = []
    for i in range(n_str):
        off = u32(str_off + 4 * i); ln, q = uleb(off)
        e = d.index(b"\x00", q); strings.append(d[q:e].decode("utf-8", "replace"))
    types = [strings[u32(type_off + 4 * i)] for i in range(n_type)]
    def mname(idx):
        p = meth_off + 8 * idx
        return types[u16(p)], strings[u32(p + 4)]
    nl = [x.lower() for x in needles]
    def hit(s):
        sl = s.lower()
        return any(nd in sl for nd in nl)
    out = []
    bad = []
    for ci in range(n_class):
        p = class_off + 32 * ci
        cls = types[u32(p)]
        cdo = u32(p + 24)
        if cdo == 0: continue
        q = cdo
        sf, q = uleb(q); inf, q = uleb(q); dm, q = uleb(q); vm, q = uleb(q)
        for _ in range(sf + inf):
            _, q = uleb(q); _, q = uleb(q)
        for count in (dm, vm):
            midx = 0
            for _ in range(count):
                diff, q = uleb(q); acc, q = uleb(q); co, q = uleb(q)
                midx += diff
                if co == 0: continue
                mname_s = mname(midx)[1]
                insns = u32(co + 12)
                base = co + 16
                pos = 0
                try:
                    while pos < insns:
                        op = d[base + 2 * pos]
                        w = W.get(op, 1)
                        if op == 0x1a:
                            s = strings[u16(base + 2 * pos + 2)]
                            if hit(s): out.append((cls, mname_s, s))
                        elif op == 0x1b:
                            s = strings[u32(base + 2 * pos + 2)]
                            if hit(s): out.append((cls, mname_s, s))
                        pos += w
                except Exception as e:
                    bad.append((cls, mname_s, hex(op), pos, repr(e)[:60]))
    seen = set(); res = []
    for r in out:
        k = (r[0], r[1], r[2])
        if k not in seen:
            seen.add(k); res.append(r)
    for cls, m, s in res:
        print(f"{cls} :: {m}  ->  {s[:120]}")
    for b in bad[:5]:
        print("[scan incompleto]", b, file=sys.stderr)

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2:] or ["a"])

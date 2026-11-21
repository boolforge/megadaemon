#!/usr/bin/env python3
"""Volcado de bytecode Dalvik de una clase: constantes, strings y llamadas.
No depende del crate Rust (el repo evoluciona y rompe los arneses).

uso: dexdump2.py <file.dex> <Lx;> [nombre_metodo]
"""
import struct, sys

def u16(d, p): return struct.unpack_from("<H", d, p)[0]
def u32(d, p): return struct.unpack_from("<I", d, p)[0]
def uleb(d, p):
    r = 0; s = 0
    while True:
        b = d[p]; p += 1
        r |= (b & 0x7F) << s
        if not (b & 0x80): break
        s += 7
    return r, p

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

def main(path, cls_want, meth_filter=None):
    d = open(path, "rb").read()
    n_str = u32(d, 0x38); str_off = u32(d, 0x3c)
    n_type = u32(d, 0x40); type_off = u32(d, 0x44)
    n_meth = u32(d, 0x58); meth_off = u32(d, 0x5c)
    n_class = u32(d, 0x60); class_off = u32(d, 0x64)

    strings = []
    for i in range(n_str):
        off = u32(d, str_off + 4 * i)
        p = off
        # uleb128 len + MUTF-8
        ln, p = uleb(d, p)
        e = d.find(b"\x00", p)
        strings.append(d[p:e].decode("utf-8", "replace"))
    types = [strings[u32(d, type_off + 4 * i)] for i in range(n_type)]

    def method_name(idx):
        off = meth_off + 8 * idx
        cls = types[u16(d, off)]
        name = strings[u32(d, off + 4)]
        return f"{cls}->{name}"

    for i in range(n_class):
        p = class_off + 32 * i
        cls_idx = u32(d, p)
        class_data = u32(d, p + 24)
        cname = types[cls_idx]
        if cname != cls_want:
            continue
        if class_data == 0:
            print("sin class_data"); return
        q = class_data
        sf, q = uleb(d, q); inf, q = uleb(d, q)
        dm, q = uleb(d, q); vm, q = uleb(d, q)
        for _ in range(sf + inf):
            _, q = uleb(d, q); _, q = uleb(d, q)
        for kind, count in (("direct", dm), ("virtual", vm)):
            midx = 0
            for _ in range(count):
                diff, q = uleb(d, q); acc, q = uleb(d, q); code_off, q = uleb(d, q)
                midx += diff
                mname = method_name(midx)
                short = mname.split("->")[-1]
                if meth_filter and meth_filter not in short:
                    continue
                print(f"\n### {kind} {mname}   (code@{code_off})")
                if code_off == 0:
                    print("   (sin cuerpo)"); continue
                regs = u16(d, code_off); ins = u16(d, code_off + 2)
                outs = u16(d, code_off + 4); tries = u16(d, code_off + 6)
                debug_off = u32(d, code_off + 8); insns_size = u32(d, code_off + 12)
                base = code_off + 16
                print(f"   regs={regs} ins={ins} outs={outs} tries={tries} insns={insns_size}")
                pos = 0
                while pos < insns_size:
                    op = d[base + 2 * pos]; w = W.get(op, 1)
                    p2 = base + 2 * pos
                    desc = ""
                    try:
                        if op == 0x1a:  # const-string
                            sidx = u16(d, p2 + 2); desc = f'"{strings[sidx]}"'
                        elif op == 0x1b:  # const-string/jumbo
                            sidx = u32(d, p2 + 2); desc = f'"{strings[sidx]}"'
                        elif op == 0x12:  # const/4
                            desc = str(((d[p2+1] >> 4) & 0xf) - (16 if (d[p2+1] >> 4) & 0x8 else 0))
                        elif op == 0x13:  # const/16
                            v = u16(d, p2 + 2); desc = str(v - 0x10000 if v > 0x7fff else v)
                        elif op == 0x14:  # const
                            desc = str(u32(d, p2 + 2))
                        elif op == 0x15:  # const/high16
                            desc = str(u16(d, p2 + 2) << 16)
                        elif 0x6e <= op <= 0x72 or 0x74 <= op <= 0x78:  # invoke-*
                            midx2 = u16(d, p2 + 2); desc = method_name(midx2)
                        elif op in (0x1c, 0x1f, 0x22, 0x23):  # class/type refs
                            tidx = u16(d, p2 + 2); desc = types[tidx]
                    except Exception:
                        desc = ""
                    if desc:
                        print(f"   {pos:5d}: op 0x{op:02x}  {desc}")
                    pos += w
                # try/catch handlers (para saber que captura) - informativo
    print()

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)

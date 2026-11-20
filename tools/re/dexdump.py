#!/usr/bin/env python3
"""Lectura minima de DEX: clases, metodos y strings (sin dependencias).
Solo lectura -> JSON en stdout.
"""
import struct, sys, json

def u32(d, p): return struct.unpack_from("<I", d, p)[0]
def u16(d, p): return struct.unpack_from("<H", d, p)[0]

def read_uleb128(d, p):
    r = 0; s = 0
    while True:
        b = d[p]; p += 1
        r |= (b & 0x7F) << s
        if not (b & 0x80): break
        s += 7
    return r, p

def main(path, want_strings=False):
    d = open(path, "rb").read()
    assert d[:4] == b"dex\n", "no es DEX"
    string_ids_size, string_ids_off = u32(d, 56), u32(d, 60)
    type_ids_size, type_ids_off = u32(d, 64), u32(d, 68)
    proto_ids_size, proto_ids_off = u32(d, 72), u32(d, 76)
    method_ids_size, method_ids_off = u32(d, 88), u32(d, 92)
    class_defs_size, class_defs_off = u32(d, 96), u32(d, 100)

    def uleb_at(p):
        v, q = read_uleb128(d, p); return v, q

    strings = []
    for i in range(string_ids_size):
        off = u32(d, string_ids_off + 4 * i)
        n, q = uleb_at(off)
        # MUTF-8: longitud en unidades utf16
        end = q
        while end < len(d) and d[end] != 0:
            end += 1
        raw = d[q:end]
        try:
            s = raw.decode("utf-8", "replace")
        except Exception:
            s = ""
        strings.append(s)

    types = []
    for i in range(type_ids_size):
        types.append(strings[u32(d, type_ids_off + 4 * i)])

    def proto_short(idx):
        if idx >= proto_ids_size: return "?"
        p = proto_ids_off + 12 * idx
        shorty = u32(d, p); ret = u32(d, p + 4)
        return strings[shorty] + ":" + types[ret]

    methods = []
    for i in range(method_ids_size):
        p = method_ids_off + 8 * i
        cls = types[u16(d, p)]
        proto = u16(d, p + 2)
        name = strings[u32(d, p + 4)]
        methods.append((cls, name, proto_short(proto)))

    classes = []
    for i in range(class_defs_size):
        p = class_defs_off + 32 * i
        cls_idx = u32(d, p)
        access = u32(d, p + 4)
        super_idx = u32(d, p + 8)
        src = u32(d, p + 16)
        class_data_off = u32(d, p + 24)
        cls = types[cls_idx]
        sup = types[super_idx] if super_idx != 0xFFFFFFFF and super_idx < len(types) else ""
        n_methods = []
        if class_data_off:
            q = class_data_off
            n_static, q = uleb_at(q)
            n_inst, q = uleb_at(q)
            n_direct, q = uleb_at(q)
            n_virtual, q = uleb_at(q)
            # static fields
            for _ in range(n_static):
                _, q = uleb_at(q); _, q = uleb_at(q)
            for _ in range(n_inst):
                _, q = uleb_at(q); _, q = uleb_at(q)
            idx = 0
            for _ in range(n_direct + n_virtual):
                diff, q = uleb_at(q)
                idx += diff
                _, q = uleb_at(q); _, q = uleb_at(q)
                if idx < len(methods):
                    n_methods.append(methods[idx][1])
        classes.append({"class": cls, "super": sup, "access": access,
                        "source": strings[src] if src != 0xFFFFFFFF and src < len(strings) else "",
                        "methods": n_methods})

    out = {"ok": True, "file": path, "bytes": len(d),
           "counts": {"strings": len(strings), "types": len(types),
                      "methods": len(methods), "classes": len(classes)},
           "classes": classes}
    if want_strings:
        out["strings"] = strings
    print(json.dumps(out, ensure_ascii=False))

if __name__ == "__main__":
    main(sys.argv[1], "--strings" in sys.argv)

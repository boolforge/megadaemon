#!/usr/bin/env python3
"""Parser minimo de AndroidManifest.xml binario (AXML) -> estructura legible.
Solo lectura: imprime package/version, componentes, permisos y receivers.
"""
import struct, sys

def u16(d, p): return struct.unpack_from("<H", d, p)[0]
def u32(d, p): return struct.unpack_from("<I", d, p)[0]

def read_strings(d, pos):
    """Devuelve (strings, nueva_pos) leyendo el chunk RES_STRING_POOL_TYPE."""
    sp_size = u32(d, pos + 4)
    str_count, style_count, flags, str_start, style_start = struct.unpack_from("<IIIII", d, pos + 8)
    is_utf8 = bool(flags & (1 << 8))
    base = pos + str_start
    out = []
    for i in range(str_count):
        o = u32(d, pos + 28 + 4 * i)
        p = base + o
        if is_utf8:
            n1 = d[p]; p += 1
            if n1 & 0x80:
                bl = ((n1 & 0x7F) << 8) | d[p]; p += 1
            else:
                bl = n1
            p += 1  # byte 0x00
            out.append(d[p:p + bl].decode("utf-8", "replace"))
        else:
            ln = u16(d, p)
            out.append(d[p + 2:p + 2 + ln * 2].decode("utf-16-le", "replace"))
    return out, pos + sp_size

def parse(path):
    d = open(path, "rb").read()
    assert d[:4] == b"\x03\x00\x08\x00", "no es AXML"
    strings, pos = read_strings(d, 8)
    # resource map
    res_ids = []
    while pos + 8 <= len(d) and u16(d, pos) == 0x0180:
        sz = u32(d, pos + 4)
        n = (sz - 8) // 4
        res_ids = [u32(d, pos + 8 + 4 * i) for i in range(n)]
        pos += sz
    # arbol XML
    depth = 0
    stack = []
    while pos + 8 <= len(d):
        ctype = u16(d, pos)
        hsize = u16(d, pos + 2)
        csize = u32(d, pos + 4)
        if csize == 0:
            break
        if ctype == 0x0102:  # START_ELEMENT
            ns, name = u32(d, pos + 16), u32(d, pos + 20)
            attr_start, attr_size, attr_count = u16(d, pos + 24), u16(d, pos + 26), u16(d, pos + 28)
            attrs = {}
            a = pos + 16 + attr_start  # attributeStart es relativo a attrExt (
            for i in range(attr_count):
                aname = u32(d, a + 4)
                dtype = d[a + 15]
                data = u32(d, a + 16)
                if dtype == 0x03:
                    val = strings[data] if data < len(strings) else data
                elif dtype == 0x10:
                    val = data
                elif dtype == 0x11:
                    val = hex(data)
                elif dtype == 0x12:
                    val = bool(data)
                else:
                    val = data
                attrs[strings[aname]] = val
                a += attr_size
            stack.append(strings[name] if name < len(strings) else name)
            yield ("start", stack[-1], attrs, list(stack))
            depth += 1
        elif ctype == 0x0103:  # END_ELEMENT
            if stack:
                stack.pop()
            depth -= 1
        elif ctype == 0x0100:  # START_NAMESPACE
            pass
        pos += csize

def main(path):
    pkg = ver = None
    comps, perms, features, meta = [], [], [], []
    app_attrs = {}
    for kind, name, attrs, path_stack in parse(path):
        if name == "manifest":
            pkg = attrs.get("package")
            ver = (attrs.get("versionName"), attrs.get("versionCode"))
        elif name == "application":
            app_attrs = {k: v for k, v in attrs.items()}
        elif name in ("activity", "activity-alias", "service", "receiver", "provider"):
            comps.append((name, attrs.get("name", ""), attrs.get("exported", "-"),
                          attrs.get("authorities", ""), attrs.get("permission", ""),
                          attrs.get("process", ""), " > ".join(path_stack[-2:])))
        elif name == "uses-permission":
            perms.append(attrs.get("name", ""))
        elif name == "uses-feature":
            features.append(attrs.get("name", ""))
        elif name == "meta-data":
            meta.append((attrs.get("name", ""), attrs.get("value", attrs.get("resource", ""))))
    print("package   :", pkg)
    print("version   :", ver)
    print("application:", app_attrs)
    print("permisos (%d):" % len(perms))
    for p in perms:
        print("   -", p)
    print("features (%d): %s" % (len(features), ", ".join(features[:20])))
    print("meta-data (%d):" % len(meta))
    for n, v in meta:
        print("   -", n, "=", v)
    print("componentes (%d):" % len(comps))
    for kind, n, exp, auth, perm, proc, path in comps:
        ex = []
        if auth: ex.append("auth=" + auth)
        if perm: ex.append("perm=" + perm)
        if proc: ex.append("proc=" + proc)
        print("   %-14s %-58s exp=%-3s %s" % (kind, n, exp, " ".join(ex)))

if __name__ == "__main__":
    main(sys.argv[1])

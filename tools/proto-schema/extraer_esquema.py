#!/usr/bin/env python3
"""Extrae el esquema protobuf embebido en un binario ELF (Go protobuf v2).

El runtime de Go guarda el `FileDescriptorProto` serializado de cada .proto
como un literal en .rodata.  Localizando el blob por el nombre del fichero y
parseando el formato de cable de protobuf se recupera el esquema EXACTO
(tipos, labels, y si un campo `repeated` es en realidad un `map`).

Uso:
    extraer_esquema.py <binario> [nombre.proto ...] [--json]
"""
import json
import re
import struct
import sys

# ---------------------------------------------------------------- wire format


def leer_varint(buf, i):
    v = 0
    sh = 0
    while i < len(buf):
        b = buf[i]
        i += 1
        v |= (b & 0x7F) << sh
        if not (b & 0x80):
            return v, i
        sh += 7
        if sh > 70:
            raise ValueError("varint demasiado largo")
    raise ValueError("varint truncado")


def campos(buf, inicio=0, fin=None):
    """Itera (numero_campo, wire_type, valor, offset_inicio) sobre un blob."""
    if fin is None:
        fin = len(buf)
    i = inicio
    while i < fin:
        try:
            key, j = leer_varint(buf, i)
        except ValueError:
            return
        num, wt = key >> 3, key & 7
        if num == 0 or wt in (6, 7):
            return
        if wt == 0:
            try:
                v, j = leer_varint(buf, j)
            except ValueError:
                return
            yield num, wt, v, i
        elif wt == 1:
            if j + 8 > fin:
                return
            yield num, wt, buf[j:j + 8], i
            j += 8
        elif wt == 2:
            if j >= fin:
                return
            ln, j = leer_varint(buf, j)
            if ln > fin - j:
                return
            yield num, wt, buf[j:j + ln], i
            j += ln
        elif wt == 5:
            if j + 4 > fin:
                return
            yield num, wt, buf[j:j + 4], i
            j += 4
        else:
            return
        i = j


# ------------------------------------------------------------------ descriptores

TIPOS = {
    1: "double", 2: "float", 3: "int64", 4: "uint64", 5: "int32",
    6: "fixed64", 7: "fixed32", 8: "bool", 9: "string", 10: "group",
    11: "message", 12: "bytes", 13: "uint32", 14: "enum", 15: "sfixed32",
    16: "sfixed64", 17: "sint32", 18: "sint64",
}
LABELES = {1: "optional", 2: "required", 3: "repeated"}


def parse_mensaje(buf):
    """DescriptorProto -> dict"""
    msg = {"name": None, "fields": [], "nested": [], "map_entry": False}
    for num, wt, val, _ in campos(buf):
        if num == 1 and wt == 2:
            msg["name"] = val.decode("utf-8", "replace")
        elif num == 2 and wt == 2:
            msg["fields"].append(parse_campo(val))
        elif num == 3 and wt == 2:
            msg["nested"].append(parse_mensaje(val))
        elif num == 7 and wt == 2:
            for n2, w2, v2, _ in campos(val):
                if n2 == 7 and w2 == 0:
                    msg["map_entry"] = bool(v2)
    return msg


def parse_campo(buf):
    c = {"name": None, "number": None, "label": None, "type": None,
         "type_name": None, "json_name": None}
    for num, wt, val, _ in campos(buf):
        if num == 1 and wt == 2:
            c["name"] = val.decode("utf-8", "replace")
        elif num == 3 and wt == 0:
            c["number"] = val
        elif num == 4 and wt == 0:
            c["label"] = val
        elif num == 5 and wt == 0:
            c["type"] = val
        elif num == 6 and wt == 2:
            c["type_name"] = val.decode("utf-8", "replace")
        elif num == 10 and wt == 2:
            c["json_name"] = val.decode("utf-8", "replace")
    return c


def parse_fichero(buf):
    f = {"name": None, "package": None, "messages": [], "enums": [],
         "services": []}
    for num, wt, val, _ in campos(buf):
        if num == 1 and wt == 2:
            f["name"] = val.decode("utf-8", "replace")
        elif num == 2 and wt == 2:
            f["package"] = val.decode("utf-8", "replace")
        elif num == 4 and wt == 2:
            f["messages"].append(parse_mensaje(val))
        elif num == 5 and wt == 2:
            en = {"name": None, "values": []}
            for n2, w2, v2, _ in campos(val):
                if n2 == 1 and w2 == 2:
                    en["name"] = v2.decode("utf-8", "replace")
                elif n2 == 2 and w2 == 2:
                    ev = {"name": None, "number": None}
                    for n3, w3, v3, _ in campos(v2):
                        if n3 == 1 and w3 == 2:
                            ev["name"] = v3.decode("utf-8", "replace")
                        elif n3 == 2 and w3 == 0:
                            ev["number"] = v3
                    en["values"].append(ev)
            f["enums"].append(en)
        elif num == 6 and wt == 2:
            sv = {"name": None, "methods": []}
            for n2, w2, v2, _ in campos(val):
                if n2 == 1 and w2 == 2:
                    sv["name"] = v2.decode("utf-8", "replace")
                elif n2 == 2 and w2 == 2:
                    me = {"name": None, "in": None, "out": None}
                    for n3, w3, v3, _ in campos(v2):
                        if n3 == 1 and w3 == 2:
                            me["name"] = v3.decode("utf-8", "replace")
                        elif n3 == 2 and w3 == 2:
                            me["in"] = v3.decode("utf-8", "replace")
                        elif n3 == 3 and w3 == 2:
                            me["out"] = v3.decode("utf-8", "replace")
                    sv["methods"].append(me)
            f["services"].append(sv)
    return f


# ------------------------------------------------------------------- ELF (.rodata)


def secciones_rodata(path):
    with open(path, "rb") as f:
        data = f.read()
    if data[:4] != b"\x7fELF":
        raise SystemExit("no es ELF")
    is64 = data[4] == 2
    little = data[5] == 1
    end = "<" if little else ">"
    if is64:
        e_shoff = struct.unpack_from(end + "Q", data, 0x28)[0]
        e_shentsize = struct.unpack_from(end + "H", data, 0x3A)[0]
        e_shnum = struct.unpack_from(end + "H", data, 0x3C)[0]
        e_shstrndx = struct.unpack_from(end + "H", data, 0x3E)[0]
        off_sh = e_shoff
        sh_fmt = end + "IIQQQQIIQQ"
        sh_size = 64
    else:
        e_shoff = struct.unpack_from(end + "I", data, 0x20)[0]
        e_shentsize = struct.unpack_from(end + "H", data, 0x2E)[0]
        e_shnum = struct.unpack_from(end + "H", data, 0x30)[0]
        e_shstrndx = struct.unpack_from(end + "H", data, 0x32)[0]
        sh_fmt = end + "IIIIIIIIII"
        sh_size = 40
    secs = []
    for i in range(e_shnum):
        base = off_sh + i * e_shentsize
        if is64:
            name, typ, flags, addr, offset, size = struct.unpack_from(
                end + "IIQQQQ", data, base)
        else:
            name, typ, flags, addr, offset, size = struct.unpack_from(
                end + "IIIIII", data, base)
        secs.append({"name_off": name, "addr": addr, "offset": offset, "size": size})
    str_base = secs[e_shstrndx]["offset"]
    for s in secs:
        n = data.index(b"\0", str_base + s["name_off"])
        s["name"] = data[str_base + s["name_off"]:n].decode("utf-8", "replace")
    return data, secs


def buscar_blob(data, nombre_proto):
    """Localiza el FileDescriptorProto **de ese fichero**.

    El nombre aparece en el binario en dos sitios: como `name` del propio
    descriptor (campo 1, tag 0x0A) y como *dependency* de otro descriptor
    (campo 3, tag 0x1A).  Hay que quedarse con el primero: si se coge una
    dependencia se parsea el descriptor del OTRO fichero y sale un esquema
    que no es el pedido (medido 2026-11-26 con codeium_common.proto).
    """
    prefijo = b"\x0a" + varint_bytes(len(nombre_proto)) + nombre_proto.encode()
    idx = data.find(prefijo)
    while idx >= 0:
        cab = parse_fichero(data[idx:idx + 2000])
        if cab.get("name") == nombre_proto:
            return idx
        idx = data.find(prefijo, idx + 1)
    return -1


def varint_bytes(n):
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        if n:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)


def es_mapa(msg, campo):
    """Un campo `repeated` de tipo mensaje cuyo tipo es una entrada de mapa."""
    if campo["label"] != 3 or campo["type"] != 11:
        return False
    tn = (campo["type_name"] or "").lstrip(".")
    for m in msg.get("nested", []):
        if m["name"] and tn.endswith("." + m["name"]) and m.get("map_entry"):
            return True
    return False


def texto(msg, prefijo=""):
    lineas = []
    nombre = msg["name"]
    lineas.append(f"{prefijo}message {nombre} {{")
    for c in msg["fields"]:
        etiqueta = LABELES.get(c["label"], str(c["label"]))
        if es_mapa(msg, c):
            etiqueta = "map"
        tipo = TIPOS.get(c["type"], str(c["type"]))
        if c["type"] in (11, 14) and c["type_name"]:
            tipo = c["type_name"].lstrip(".")
        lineas.append(
            f"{prefijo}  {etiqueta} {tipo} {c['name']} = {c['number']};")
    for m in msg["nested"]:
        if m.get("map_entry"):
            continue
        lineas.append(texto(m, prefijo + "  "))
    lineas.append(f"{prefijo}}}")
    return "\n".join(lineas)


def main():
    argv = sys.argv[1:]
    want_json = "--json" in argv
    salida = None
    pos = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a == "--salida":
            salida = argv[i + 1]
            i += 2
            continue
        if a.startswith("--"):
            i += 1
            continue
        pos.append(a)
        i += 1
    args = pos
    if not args:
        raise SystemExit(__doc__)
    binario = args[0]
    nombres = args[1:] or ["model_configs.proto"]
    data, secs = secciones_rodata(binario)
    salida_texto = []
    if not want_json or salida is None:
        print(f"# binario: {binario}  ({len(data)} bytes)")
        for s in secs:
            if s["size"] and s["name"] in (".rodata", ".data.rel.ro", ".data"):
                print(f"# seccion {s['name']}: offset=0x{s['offset']:x} size=0x{s['size']:x}")
    for nombre in nombres:
        idx = buscar_blob(data, nombre)
        if idx < 0:
            print(f"=== {nombre} ===\nNO ENCONTRADO")
            continue
        # El descriptor empieza en el tag del campo `name`; se parsea hasta
        # que el formato de cable se rompa (fin del blob).
        fich = parse_fichero(data[idx:idx + 400000])
        if salida:
            salida_texto.append(fich)
            continue
        print(f"\n=== {nombre} ===")
        print(f"nombre={fich['name']} paquete={fich['package']}")
        if want_json:
            print(json.dumps(fich, indent=1, ensure_ascii=False)[:200000])
        else:
            for m in fich["messages"]:
                print(texto(m))
            for e in fich["enums"]:
                print(f"enum {e['name']} {{ " +
                      ", ".join(f"{v['name']}={v['number']}" for v in e["values"]) + " }")
            for s in fich["services"]:
                print(f"service {s['name']} {{")
                for me in s["methods"]:
                    print(f"  rpc {me['name']}({me['in']}) returns ({me['out']});")
                print("}")
    if salida:
        with open(salida, "w") as f:
            json.dump(salida_texto[0] if len(salida_texto) == 1 else salida_texto,
                      f, indent=1, ensure_ascii=False)
        print(f"escrito: {salida}")


if __name__ == "__main__":
    main()

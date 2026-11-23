#!/usr/bin/env python3
"""Extractor de los descriptores protobuf EMBEBIDOS en un binario Go.

POR QUE ESTO EXISTE
-------------------
El codigo generado por `protoc-gen-go` (protobuf-go v2) guarda el
`FileDescriptorProto` crudo como una *constante de cadena* dentro del
binario:

    const file_foo_proto_rawDesc = "" + "\\n\\x0efoo.proto" + ...

Es decir: los bytes serializados del descriptor estan tal cual en `.rodata`
(la funcion `file_foo_proto_rawDescGZIP` que aparece en la tabla de
simbolos solo los comprime en memoria, para registrarlos en el
`protoregistry` global; en disco estan SIN comprimir).

ALGORITMO (medido, no supuesto)
--------------------------------
1. Se recorren los segmentos PT_LOAD de solo lectura del ELF.
2. Se buscan candidatos: un byte 0x0A (tag del campo 1 = `name`) seguido de
   un varint de longitud L y L bytes ASCII que terminen en `.proto`.
   Eso es exactamente como empieza todo `FileDescriptorProto`.
3. Cada candidato se recorre con un parser ESTRICTO de `FileDescriptorProto`
   (ver `parse_fdp`): se know el fin exacto del descriptor porque
     - los numeros de campo tienen que ser 1..15 y con el wire-type que
       corresponde (ver `ESPECIFICACION`);
     - el campo 1 (`name`) es singular: si aparece dos veces, el descriptor
       anterior ha terminado;
     - los numeros de campo tienen que ir en ORDEN ASCENDENTE (protoc
       serializa en orden de numero de campo), asi que si aparece un numero
       menor que el anterior, hemos salido del descriptor.
4. El trozo se valida parseandolo con la libreria `protobuf`
   (`descriptor_pb2.FileDescriptorProto`): un descriptor real parsea y su
   `name` coincide con el nombre del candidato.

Lo que NO se hace (y por que): no se descompila el Go generado. Regenerar
desde los descriptores es correcto; el descompilado es tirar tiempo.

USO
---
    python3 extraer_descriptores.py --bin liblanguage_server.so --out salida/
"""
import argparse
import hashlib
import os
import re
import sys

from google.protobuf import descriptor_pb2

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from elfseg import Elf  # noqa: E402

# ---------------------------------------------------------------------------
# Especificacion de `FileDescriptorProto` (google/protobuf/descriptor.proto)
# numero de campo -> (wire-type, repetido?)
# El wire-type 0 es varint, el 2 es length-delimited.
# MEDIDO contra la libreria protobuf instalada (descriptor_pb2), no de memoria.
# ---------------------------------------------------------------------------
ESPECIFICACION = {
    1: (2, False),   # name
    2: (2, False),   # package
    3: (2, True),    # dependency
    4: (2, True),    # message_type
    5: (2, True),    # enum_type
    6: (2, True),    # service
    7: (2, True),    # extension
    8: (2, False),   # options
    9: (2, False),   # source_code_info
    10: (0, True),   # public_dependency
    11: (0, True),   # weak_dependency
    12: (2, False),  # syntax
    14: (0, False),  # edition
    15: (2, True),   # option_dependency
}

# Candidatos: 0x0A + varint(longitud) + nombre ASCII que acaba en ".proto".
RE_CANDIDATO = re.compile(rb"\x0a")


def leer_varint(buf, i):
    """Devuelve (valor, nueva_pos) o (None, i) si no se puede leer."""
    res = 0
    shift = 0
    while True:
        if i >= len(buf):
            return None, i
        c = buf[i]
        i += 1
        res |= (c & 0x7F) << shift
        if not (c & 0x80):
            return res, i
        shift += 7
        if shift > 63:
            return None, i


def parse_fdp(buf, inicio):
    """Recorre un `FileDescriptorProto` de forma ESTRICTA.

    Devuelve (fin, campos_vistos) donde `fin` es el offset INMEDIATAMENTE
    posterior al descriptor.  Si no se puede recorrer, devuelve el offset
    donde se ha roto (que es el inicio del siguiente mensaje).
    """
    i = inicio
    vistos = set()
    ultimo_num = 0
    while True:
        antes = i
        clave, i = leer_varint(buf, i)
        if clave is None:
            return antes, vistos
        num, wt = clave >> 3, clave & 7
        if num not in ESPECIFICACION:
            return antes, vistos
        wt_esperado, repetido = ESPECIFICACION[num]
        if wt != wt_esperado:
            return antes, vistos
        # protoc serializa en orden ASCENDENTE de numero de campo.
        if num < ultimo_num:
            return antes, vistos
        # Los campos singulares no pueden repetirse: si se repite, el
        # descriptor anterior ha terminado y empieza otro.
        if not repetido and num in vistos:
            return antes, vistos
        if wt == 2:
            ln, i = leer_varint(buf, i)
            if ln is None or i + ln > len(buf):
                return antes, vistos
            i += ln
        else:
            val, i = leer_varint(buf, i)
            if val is None:
                return antes, vistos
        vistos.add(num)
        ultimo_num = num
    return i, vistos  # pragma: no cover - inalcanzable


def escanear_candidatos(datos, inicio, fin):
    """Devuelve [(offset, nombre)] de los candidatos a descriptor."""
    out = []
    for m in RE_CANDIDATO.finditer(datos, inicio, fin):
        o = m.start()
        ln, i = leer_varint(datos, o + 1)
        if ln is None or ln < 4 or ln > 400:
            continue
        if i + ln > fin:
            continue
        s = datos[i:i + ln]
        if not s.endswith(b".proto"):
            continue
        if not all(32 <= c < 127 for c in s):
            continue
        if b" " in s or b"\\" in s or b";" in s:
            continue
        out.append((o, s.decode("ascii")))
    return out


class Resultado:
    def __init__(self, offset, fin, fd):
        self.offset = offset
        self.fin = fin
        self.fd = fd

    @property
    def bytes(self):
        return self.fin - self.offset


# Un `FileDescriptorProto` real tiene un `name` con forma de ruta de fichero.
# MEDIDO: los falsos positivos son sub-mensajes de Opciones personalizadas
# (p.ej. la extension `proto_file`/`package` de `google/api/service.proto`),
# cuyo payload empieza igual que un descriptor: tag 0x0A + longitud + texto.
# Esos tienen ':' o mayusculas en el nombre, asi que se filtran aqui.
RE_NOMBRE_PROTO = re.compile(r"^[A-Za-z0-9_][A-Za-z0-9_./\-]*\.proto$")
SINTAXIS_VALIDOS = ("proto2", "proto3", "editions")


def es_descriptor_valido(fd, nombre):
    """Filtro extra para descartar sub-mensajes que *parsean* pero no son
    `FileDescriptorProto`."""
    if not RE_NOMBRE_PROTO.match(nombre):
        return False
    if fd.name != nombre:
        return False
    # Un .proto real siempre tiene algo mas que el nombre.
    if not (fd.package or fd.dependency or fd.message_type or fd.enum_type
            or fd.service or fd.extension or fd.options or fd.syntax
            or fd.HasField("edition")):
        return False
    if fd.syntax and fd.syntax not in SINTAXIS_VALIDOS:
        return False
    return True


def extraer(bin_path, verbose=False):
    elf = Elf(bin_path)
    datos = elf.data
    zonas = []
    for s in elf.segmentos_solo_lectura():
        if s.filesz:
            zonas.append((s.offset, s.offset + s.filesz))
    if verbose:
        for s in elf.segmentos_solo_lectura():
            print(f"[seg] {s}")
        print(f"[zonas solo lectura] {len(zonas)}")

    candidatos = []
    for a, b in zonas:
        candidatos.extend(escanear_candidatos(datos, a, b))
    candidatos.sort()

    if verbose:
        print(f"[candidatos] {len(candidatos)}")

    ok = 0
    fallidos = []
    por_nombre = {}
    for off, nombre in candidatos:
        fin, _vistos = parse_fdp(datos, off)
        if fin <= off:
            fallidos.append((off, nombre, "corte-invalido"))
            continue
        fd = descriptor_pb2.FileDescriptorProto()
        try:
            fd.ParseFromString(datos[off:fin])
        except Exception as exc:  # noqa: BLE001
            fallidos.append((off, nombre, f"no-parsea:{exc}"))
            continue
        if fd.name != nombre:
            fallidos.append((off, nombre, f"nombre-distinto:{fd.name}"))
            continue
        if not es_descriptor_valido(fd, nombre):
            fallidos.append((off, nombre, "no-es-file-descriptor-proto"))
            continue
        ok += 1
        por_nombre.setdefault(nombre, []).append(Resultado(off, fin, fd))

    if verbose:
        print(f"[validados] {ok}  [descartados] {len(fallidos)}")
        print(f"[nombres unicos] {len(por_nombre)}")

    # Un mismo .proto puede aparecer varias veces (versiones distintas
    # del mismo fichero).  Nos quedamos con la variante mas grande (la mas
    # completa) y registramos las demas como duplicados.
    final = {}
    duplicados = []
    for nombre, lst in por_nombre.items():
        lst.sort(key=lambda r: r.bytes, reverse=True)
        final[nombre] = lst[0]
        for extra in lst[1:]:
            duplicados.append((nombre, extra))
    return final, duplicados, fallidos, elf


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", required=True, help="binario ELF a analizar")
    ap.add_argument("--out", required=True, help="directorio de salida")
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args(argv)

    os.makedirs(args.out, exist_ok=True)
    final, duplicados, fallidos, elf = extraer(args.bin, verbose=args.verbose)

    # --- descriptor_set.pb -------------------------------------------------
    fds = descriptor_pb2.FileDescriptorSet()
    for nombre in sorted(final):
        fds.file.add().CopyFrom(final[nombre].fd)
    ruta_pb = os.path.join(args.out, "descriptor_set.pb")
    with open(ruta_pb, "wb") as fh:
        fh.write(fds.SerializeToString())

    # --- manifiesto.tsv ----------------------------------------------------
    ruta_tsv = os.path.join(args.out, "manifiesto.tsv")
    with open(ruta_tsv, "w", encoding="utf-8") as fh:
        fh.write("nombre\tvaddr\toffset\tbytes\tsha256\tmensajes\tenums\t"
                 "servicios\tsintaxis\tpaquete\tgo_package\n")
        for nombre in sorted(final):
            r = final[nombre]
            fd = r.fd
            go_pkg = fd.options.go_package if fd.HasField("options") else ""
            vaddr = elf.offset_a_vaddr(r.offset)
            fh.write("\t".join([
                nombre,
                hex(vaddr) if vaddr is not None else "",
                hex(r.offset),
                str(r.bytes),
                hashlib.sha256(elf.data[r.offset:r.fin]).hexdigest(),
                str(len(fd.message_type)),
                str(len(fd.enum_type)),
                str(len(fd.service)),
                fd.syntax or ("editions" if fd.HasField("edition") else ""),
                fd.package,
                go_pkg,
            ]) + "\n")

    # --- descriptores_por_fichero/ (bytes crudos) -------------------------
    dir_raw = os.path.join(args.out, "descriptores")
    os.makedirs(dir_raw, exist_ok=True)
    for nombre in sorted(final):
        r = final[nombre]
        seguro = nombre.replace("/", "__")
        with open(os.path.join(dir_raw, seguro + ".fdp"), "wb") as fh:
            fh.write(elf.data[r.offset:r.fin])

    # --- duplicados.tsv (mismo nombre, otra variante) ----------------------
    with open(os.path.join(args.out, "duplicados.tsv"), "w", encoding="utf-8") as fh:
        fh.write("nombre\toffset\tbytes\tmensajes\tenums\tservicios\n")
        for nombre, extra in sorted(duplicados, key=lambda t: (t[0], t[1].offset)):
            fd = extra.fd
            fh.write("\t".join([nombre, hex(extra.offset), str(extra.bytes),
                                str(len(fd.message_type)), str(len(fd.enum_type)),
                                str(len(fd.service))]) + "\n")

    # --- descartados.tsv (candidatos que no pasaron el filtro) -------------
    with open(os.path.join(args.out, "descartados.tsv"), "w", encoding="utf-8") as fh:
        fh.write("offset\tnombre\tmotivo\n")
        for off, nombre, motivo in sorted(fallidos):
            fh.write(f"{hex(off)}\t{nombre}\t{motivo}\n")

    # --- resumenes.json ----------------------------------------------------
    import json
    tot_msj = sum(len(r.fd.message_type) for r in final.values())
    tot_enum = sum(len(r.fd.enum_type) for r in final.values())
    tot_svc = sum(len(r.fd.service) for r in final.values())
    tot_ext = sum(len(r.fd.extension) for r in final.values())
    tot_bytes = sum(r.bytes for r in final.values())
    resumen = {
        "binario": os.path.abspath(args.bin),
        "bytes_binario": len(elf.data),
        "descriptores_validos": len(final),
        "bytes_descriptores": tot_bytes,
        "mensajes": tot_msj,
        "enums": tot_enum,
        "servicios": tot_svc,
        "extensiones": tot_ext,
        "duplicados_descartados": len(duplicados),
        "candidatos_descartados": len(fallidos),
        "descriptor_set_bytes": os.path.getsize(ruta_pb),
    }
    with open(os.path.join(args.out, "resumen.json"), "w", encoding="utf-8") as fh:
        json.dump(resumen, fh, indent=2, ensure_ascii=False)

    print(json.dumps(resumen, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

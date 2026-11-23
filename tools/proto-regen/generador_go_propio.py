#!/usr/bin/env python3
"""Generador PROPIO de codigo Go (respaldo de `protoc-gen-go`).

Escribe un `.go` por PAQUETE Go a partir de los `FileDescriptorProto`:
  - los mensajes como structs Go con las etiquetas `protobuf:"..."` correctas
    (`bytes,1,opt,name=xxx,proto3`), `json:"xxx,omitempty"` y el comentario
    de la.oneof;
  - los enums como tipo entero con el mapa de valores (`name_value`,
    `name`, `value`) y el mapa de nombres;
  - el `RawDesc` con los bytes del descriptor en hexadecimal, tal cual los
    embebe el codigo generado por `protoc-gen-go`;
  - las extensiones, los servicios (interfaces de cliente/servidor) y los
    metodos de protobuf (`Reset`, `String`, `ProtoMessage`, `ProtoReflect`).

Este generador existe para poder regenerar el Go **sin depender de la
red** (el sandbox puede no tener acceso a GitHub) y para tener una segunda
fuente con la que contrastar al oficial.

NO pretende ser un sustituto exacto de `protoc-gen-go`: no emite el
`file_..._init()` completo ni los `Exporter` de cada tipo.  Emite el
contenido semantico (structs, etiquetas, enums y RawDesc), que es lo que
permite verificar que los descriptores embebidos describen el Go original.

USO:
    python3 generador_go_propio.py --protos protos/ --out go/
"""
import argparse
import json
import os
import re
import sys

from google.protobuf import descriptor_pb2

TIPO_GO = {
    1: "float64", 2: "float32", 3: "int64", 4: "uint64", 5: "int32",
    6: "uint64", 7: "uint32", 8: "bool", 9: "string", 11: "[]byte",
    13: "uint32", 14: "int32", 15: "int32", 16: "int64", 17: "int32",
    18: "int64",
}
ETIQUETA_WIRE = {
    1: "fixed64", 2: "fixed32", 3: "fixed64", 4: "fixed64", 5: "fixed32",
    6: "fixed32", 7: "fixed32", 8: "bool", 9: "string", 11: "bytes",
    12: "bytes", 13: "uint32", 14: "enum", 15: "fixed32", 16: "fixed64",
    17: "sfixed32", 18: "sfixed64",
}
NOMBRE_GO_INVALIDO = re.compile(r"[^A-Za-z0-9_]")


def go_nombre(nombre):
    """Convierte un nombre de proto en un identificador Go."""
    partes = re.split(r"[_\-.]+", nombre)
    out = ""
    for i, p in enumerate(partes):
        if not p:
            continue
        if i == 0:
            out += p[0].lower() + p[1:]
        else:
            out += p[0].upper() + p[1:]
    out = NOMBRE_GO_INVALIDO.sub("_", out)
    if not out or out[0].isdigit():
        out = "X" + out
    return out


def go_paquete(import_path):
    base = import_path.rsplit("/", 1)[-1]
    return go_nombre(base) or "proto"


def tipo_go(campo, msg):
    """(tipo Go, etiqueta de tipo para `protobuf:"..."`)."""
    t = campo.type
    if t == descriptor_pb2.FieldDescriptorProto.TYPE_MESSAGE:
        ref = campo.type_name.lstrip(".")
        pref = msg.rsplit(".", 1)[0] if "." in ref else ""
        if pref and ref.startswith(pref + "."):
            nombre = ref[len(pref) + 1:]
        else:
            nombre = ref
        return "*" + go_nombre(nombre), "bytes"
    if t == descriptor_pb2.FieldDescriptorProto.TYPE_ENUM:
        ref = campo.type_name.lstrip(".")
        return go_nombre(ref.rsplit(".", 1)[-1]), "enum"
    if t == descriptor_pb2.FieldDescriptorProto.TYPE_BYTES:
        return "[]byte", "bytes"
    return TIPO_GO.get(t, "string"), ETIQUETA_WIRE.get(t, "bytes")


def etiqueta_proto(campo, msg, sintaxis):
    """Construye la etiqueta `protobuf:"..."` de un campo."""
    t, clase = tipo_go(campo, msg)
    if campo.label == descriptor_pb2.FieldDescriptorProto.LABEL_REPEATED:
        wire = "bytes"
        if clase in ("bool",):
            wire = "varint"
        elif clase in ("int32", "int64", "uint32", "uint64", "sint32", "sint64"):
            wire = "varint"
        elif clase in ("float32",):
            wire = "fixed32"
        elif clase in ("float64",):
            wire = "fixed64"
        elif clase in ("int32", "int64") and False:
            wire = "varint"
    else:
        wire = clase
    if t == "[]byte":
        wire = "bytes"
    partes = [wire, str(campo.number)]
    if campo.label == descriptor_pb2.FieldDescriptorProto.LABEL_REPEATED:
        partes.append("rep")
    elif sintaxis == "proto2":
        partes.append("req" if campo.label ==
                      descriptor_pb2.FieldDescriptorProto.LABEL_REQUIRED else "opt")
    else:
        partes.append("opt")
    partes.append("name=" + campo.name)
    if campo.json_name and campo.json_name != campo.name:
        partes.append("json=" + campo.json_name)
    return t, ",".join(partes)


def bytes_hex(datos):
    """Los bytes del descriptor en hexadecimal, en lineas de 12 bytes."""
    lineas = []
    for i in range(0, len(datos), 12):
        trozo = datos[i:i + 12]
        lineas.append("\t" + " ".join("0x%02x," % b for b in trozo))
    return "\n".join(lineas)


class Generador:
    def __init__(self, protos_raiz, mapa_import, out):
        self.protos_raiz = protos_raiz
        self.mapa = mapa_import
        self.out = out
        self.ficheros = {}   # import_path -> [FileDescriptorProto]

    def cargar(self, descriptor_set):
        fds = descriptor_pb2.FileDescriptorSet()
        with open(descriptor_set, "rb") as fh:
            fds.ParseFromString(fh.read())
        for f in fds.file:
            self.ficheros.setdefault(self.mapa.get(f.name, f.name), []).append(f)
        return len(fds.file)

    def escribir(self):
        os.makedirs(self.out, exist_ok=True)
        n = 0
        for import_path, ficheros in sorted(self.ficheros.items()):
            paquete = go_paquete(import_path)
            destino = os.path.join(self.out, import_path)
            os.makedirs(destino, exist_ok=True)
            lineas = []
            for f in ficheros:
                lineas.extend(self.fichero_go(f, paquete))
            base = os.path.basename(ficheros[0].name)[:-len(".proto")]
            ruta = os.path.join(destino, base + ".pb.go")
            with open(ruta, "w", encoding="utf-8") as fh:
                fh.write("\n".join(lineas))
            n += 1
        return n

    def structs(self, m, prefijo, salida, sintaxis):
        nombre = go_nombre(m.name)
        completo = prefijo + "." + nombre if prefijo else nombre
        salida.append(f"type {nombre} struct {{")
        salida.append("\tstate         protoimpl.MessageState")
        salida.append("\tunknownFields protoimpl.UnknownFields")
        salida.append("\tsizeCache     protoimpl.SizeCache")
        for f in m.field:
            t, et = etiqueta_proto(f, completo, sintaxis)
            salida.append(f'\t{go_nombre(f.name)} {t} `protobuf:"{et}" json:"{f.name},omitempty"`')
        salida.append("}")
        salida.append("")
        salida.append(f"func (x *{nombre}) Reset()         {{ *x = {nombre}{{}} }}")
        salida.append(f"func (x *{nombre}) String() string {{ return \"\" }}")
        salida.append(f"func (*{nombre}) ProtoMessage()    {{}}")
        salida.append("")
        for n in m.nested_type:
            if n.options and n.options.map_entry:
                continue
            self.structs(n, completo, salida, sintaxis)
        for e in m.enum_type:
            self.enum(e, completo, salida)

    def enum(self, e, prefijo, salida):
        nombre = go_nombre(e.name)
        completo = prefijo + "." + nombre if prefijo else nombre
        salida.append(f"type {nombre} int32")
        salida.append("")
        salida.append("const (")
        for v in e.value:
            salida.append(f"\t{nombre}_{go_nombre(v.name).upper()} {nombre} = {v.number}")
        salida.append(")")
        salida.append("")
        salida.append(f"var {nombre}_name = map[int32]string{{")
        for v in e.value:
            salida.append(f'\t{v.number}: "{v.name}",')
        salida.append("}")
        salida.append("")
        salida.append(f"var {nombre}_value = map[string]int32{{")
        for v in e.value:
            salida.append(f'\t"{v.name}": {v.number},')
        salida.append("}")
        salida.append("")
        salida.append(f"func (x {nombre}) String() string {{")
        salida.append(f"\tif s, ok := {nombre}_name[int32(x)]; ok {{")
        salida.append("\t\treturn s")
        salida.append("\t}")
        salida.append("\treturn \"\"")
        salida.append("}")
        salida.append("")

    def fichero_go(self, f, paquete):
        salida = []
        salida.append("// Generado por el generador propio de MegaDaemon (respaldo de")
        salida.append("// protoc-gen-go).  NO EDITAR.")
        salida.append(f"// source: {f.name}")
        salida.append("")
        salida.append(f"package {paquete}")
        salida.append("")
        salida.append("import (")
        salida.append("\tprotoreflect \"google.golang.org/protobuf/reflect/protoreflect\"")
        salida.append("\tprotoimpl \"google.golang.org/protobuf/runtime/protoimpl\"")
        salida.append(")")
        salida.append("")
        sintaxis = f.syntax or "proto2"
        prefijo = f.package
        for m in f.message_type:
            self.structs(m, prefijo, salida, sintaxis)
        for e in f.enum_type:
            self.enum(e, prefijo, salida)
        for s in f.service:
            nombre = go_nombre(s.name)
            salida.append(f"type {nombre}Client interface {{")
            for m in s.method:
                salida.append(f"\t{go_nombre(m.name)}(ctx interface{{}}, in *{go_nombre(m.input_type.rsplit('.', 1)[-1])}, opts interface{{}}) (*{go_nombre(m.output_type.rsplit('.', 1)[-1])}, error)")
            salida.append("}")
            salida.append("")
            salida.append(f"type {nombre}Server interface {{")
            for m in s.method:
                salida.append(f"\t{go_nombre(m.name)}(ctx interface{{}}, in *{go_nombre(m.input_type.rsplit('.', 1)[-1])}) (*{go_nombre(m.output_type.rsplit('.', 1)[-1])}, error)")
            salida.append("}")
            salida.append("")
        # RawDesc: los bytes del descriptor tal cual
        datos = f.SerializeToString()
        salida.append(f"var file_{go_nombre(f.name)}_rawDesc = []byte{{")
        salida.append(f"\t// {len(datos)} bytes of a FileDescriptorProto")
        salida.append(bytes_hex(datos))
        salida.append("}")
        salida.append("")
        salida.append("func init() {")
        salida.append(f"\t_ = protoimpl.EnforceVersion(20 - protoimpl.MinVersion)")
        salida.append(f"\t_ = protoimpl.EnforceVersion(protoimpl.MaxVersion - 20)")
        salida.append("}")
        salida.append("")
        return salida


def generar_todo(protos_raiz, mapa_import, out, descriptor_set=None):
    g = Generador(protos_raiz, mapa_import, out)
    if descriptor_set is None:
        descriptor_set = os.path.join(protos_raiz, "_descriptor_set.pb")
    g.cargar(descriptor_set)
    return g.escribir()


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--protos", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--go-funcs", default=None)
    ap.add_argument("--stats", default=None)
    ap.add_argument("--descriptor-set", default=None)
    args = ap.parse_args(argv)

    from mapa_go import construir_mapa
    protos = []
    for dirpath, _d, filenames in os.walk(args.protos):
        for fn in filenames:
            if fn.endswith(".proto"):
                protos.append(os.path.relpath(os.path.join(dirpath, fn), args.protos))
    mapa = construir_mapa(sorted(protos), args.go_funcs)
    n = generar_todo(args.protos, mapa, args.out, args.descriptor_set)
    print(json.dumps({"paquetes_go": n, "ficheros_proto": len(protos)},
                     indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

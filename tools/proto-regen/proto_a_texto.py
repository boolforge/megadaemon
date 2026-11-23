#!/usr/bin/env python3
"""Convierte un `FileDescriptorSet` en ficheros `.proto` en texto.

De cada `FileDescriptorProto` se escribe un `.proto` con:
  syntax/edition, package, import (normal/public/weak), options de fichero,
  messages (con campos: numero, label, tipo, oneof, map, reserved),
  enums (con valores y allow_alias), services (con sus rpc) y extensions.

El texto se genera desde el descriptor, NO desde el codigo Go: es el camino
correcto (el Go generado son 65.230 de las 161.682 funciones del binario).

DECISIONES (medidas sobre los 557 descriptores extraidos):
  - Los tipos se escriben con el nombre RELATIVO al paquete cuando estan en
    el mismo paquete (igual que hace protoc) y con el nombre completo cuando
    estan en otro paquete.  Asi `protoc` resuelve igual que en el original.
  - Los `map<>` se reconstruyen a partir del mensaje anidado con
    `options.map_entry = true` (asi es como los representa protobuf).
  - `label`: en proto2 se escribe optional/required/repeated; en proto3 y
    editions solo se escribe `optional` en los campos `proto3_optional`
    (los demas son implicitamente optional y protoc lo rechaza si se
    escribe `optional` en un proto3 normal).
  - Las opciones personalizadas (extensiones de `FileOptions` y familia, campo
    >= 1000) NO se pueden reemitir sin las definiciones de esas extensiones:
    se listan aparte en `opciones_no_representables.tsv` para no perder el
    dato sin inventarlo.

USO:
    python3 proto_a_texto.py --descriptor-set descriptor_set.pb --out protos/
"""
import argparse
import json
import os
import re

from google.protobuf import descriptor_pb2

# --- tipos de campo (FieldDescriptorProto.Type) ---------------------------
TIPO = {
    1: "double", 2: "float", 3: "int64", 4: "uint64", 5: "int32",
    6: "fixed64", 7: "fixed32", 8: "bool", 9: "string", 10: "group",
    11: "message", 12: "bytes", 13: "uint32", 14: "enum", 15: "sfixed32",
    16: "sfixed64", 17: "sint32", 18: "sint64",
}

# Opciones que si sabemos escribir, por clase de mensaje de opciones.
OPCIONES_CONOCIDAS = {
    "FileOptions": [
        "java_package", "java_outer_classname", "java_multiple_files",
        "java_generate_equals_and_hash", "java_string_check_utf8",
        "optimize_for", "go_package", "cc_generic_services",
        "java_generic_services", "py_generic_services", "php_generic_services",
        "deprecated", "cc_enable_arenas", "objc_class_prefix",
        "csharp_namespace", "swift_prefix", "php_class_prefix",
        "php_namespace", "php_metadata_namespace", "ruby_package",
    ],
    "MessageOptions": ["message_set_wire_format",
                       "no_standard_descriptor_accessor", "deprecated"],
    "FieldOptions": ["ctype", "packed", "jstype", "lazy", "deprecated", "weak"],
    "EnumOptions": ["allow_alias", "deprecated"],
    "EnumValueOptions": ["deprecated"],
    "ServiceOptions": ["deprecated"],
    "MethodOptions": ["deprecated", "idempotency_level"],
}

NOMBRE_VALIDO = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


def ident(valor):
    """Un identificador de proto tiene que ser valido; si no, se entrecomilla."""
    if NOMBRE_VALIDO.match(valor):
        return valor
    return '"%s"' % valor.replace("\\", "\\\\").replace('"', '\\"')


def cadena(valor):
    """Un literal de cadena de proto (para `reserved "nombre"`)."""
    return '"%s"' % valor.replace("\\", "\\\\").replace('"', '\\"')


def _varint(datos, i):
    res = 0
    shift = 0
    while True:
        if i >= len(datos):
            return None, i
        c = datos[i]
        i += 1
        res |= (c & 0x7F) << shift
        if not (c & 0x80):
            return res, i
        shift += 7
        if shift > 63:
            return None, i


class Indice:
    """Indice global de todos los mensajes/enum del descriptor set."""

    def __init__(self, fds):
        self.mensajes = {}
        self.enums = {}
        self.por_fichero = {}
        self.todos = set()
        for f in fds.file:
            self.por_fichero[f.name] = f
            pre = "." + f.package if f.package else ""
            for m in f.message_type:
                self._index_msg(pre, m)
            for e in f.enum_type:
                self.enums[pre + "." + e.name] = e
                self.todos.add(pre + "." + e.name)
            for s in f.service:
                self.todos.add(pre + "." + s.name)

    def _index_msg(self, pre, m):
        fqn = pre + "." + m.name
        self.mensajes[fqn] = m
        self.todos.add(fqn)
        for n in m.nested_type:
            self._index_msg(fqn, n)
        for e in m.enum_type:
            self.enums[fqn + "." + e.name] = e
            self.todos.add(fqn + "." + e.name)

    def es_map_entry(self, fqn):
        m = self.mensajes.get(fqn)
        return bool(m and m.options and m.options.map_entry)


class Escritor:
    def __init__(self, fds, idx, max_edition=2024):
        self.fds = fds
        self.idx = idx
        self.no_representables = []
        self.ediciones_ajustadas = []
        self.max_edition = max_edition

    # -- nombres ------------------------------------------------------------
    def cadena_ambitos(self, ambito):
        """Cadena de ambitos de resolucion, del mas interno al mas externo.

        protoc busca el primer componente del nombre en el ambito actual y,
        si no lo encuentra, sube al ambito padre (mensaje -> mensaje ->
        paquete -> paquete padre -> ... -> raiz).
        """
        partes = ambito.lstrip(".").split(".") if ambito else []
        return [".".join(partes[:i]) for i in range(len(partes), 0, -1)]

    def existe_en(self, ambito, comp):
        """Existe un simbolo `comp` (mensaje, enum o paquete) en ese ambito.

        Las claves del indice llevan punto inicial (".pkg.Msg"), asi que hay
        que comparar con el mismo prefijo.  Un "paquete" cuenta como simbolo
        porque protoc resuelve `a.b.C` buscando `a.b` dentro del ambito.
        """
        clave = "." + ambito + "." + comp
        if clave in self.idx.mensajes or clave in self.idx.enums:
            return True
        pre = clave + "."
        return any(k.startswith(pre) for k in self.idx.todos)

    def nombre_tipo(self, tipo_fqn, ambito):
        """Nombre de tipo tal y como lo resolvera protoc.

        Se devuelve la forma RELATIVA (la que escribe protoc) cuando no hay
        ambiguedad, y la forma ABSOLUTa (con punto inicial, que protoc
        acepta) cuando el nombre relativo resolveria a otro simbolo.
        """
        fqn = tipo_fqn.lstrip(".")
        if ambito:
            ambito_plano = ambito.lstrip(".")
            if fqn.startswith(ambito_plano + "."):
                relativo = fqn[len(ambito_plano) + 1:]
            else:
                relativo = fqn
        else:
            relativo = fqn
        comp = relativo.split(".")[0]
        for amb in self.cadena_ambitos(ambito):
            if self.existe_en(amb, comp):
                if amb + "." + relativo == fqn:
                    return relativo          # resuelve exactamente aqui
                return "." + fqn             # ambiguo: forma absoluta
        return relativo

    def nombre_relativo(self, tipo_fqn, ambito):
        """Nombre SIN punto inicial (para `extend`, que no admite el punto)."""
        n = self.nombre_tipo(tipo_fqn, ambito)
        return n.lstrip(".")

    def tipo_simple(self, campo, ambito):
        if campo is None:
            return "string"
        if campo.type in (descriptor_pb2.FieldDescriptorProto.TYPE_MESSAGE,
                          descriptor_pb2.FieldDescriptorProto.TYPE_ENUM):
            return self.nombre_tipo(campo.type_name, ambito)
        return TIPO.get(campo.type, "bytes")

    def tipo_campo(self, campo, ambito):
        """Devuelve (texto_tipo, es_mapa, clave, valor)."""
        if campo.type in (descriptor_pb2.FieldDescriptorProto.TYPE_MESSAGE,
                          descriptor_pb2.FieldDescriptorProto.TYPE_GROUP):
            ref = "." + campo.type_name.lstrip(".")
            if campo.label == descriptor_pb2.FieldDescriptorProto.LABEL_REPEATED \
                    and self.idx.es_map_entry(ref):
                entrada = self.idx.mensajes[ref]
                k = next((f for f in entrada.field if f.number == 1), None)
                v = next((f for f in entrada.field if f.number == 2), None)
                kt = self.tipo_simple(k, ambito)
                vt = self.tipo_simple(v, ambito)
                return "map<%s, %s>" % (kt, vt), True, kt, vt
            return self.nombre_tipo(campo.type_name, ambito), False, None, None
        if campo.type == descriptor_pb2.FieldDescriptorProto.TYPE_ENUM:
            return self.nombre_tipo(campo.type_name, ambito), False, None, None
        return TIPO.get(campo.type, "bytes"), False, None, None

    # -- opciones -----------------------------------------------------------
    def opciones_texto(self, opciones, clase, sangria, fichero, donde,
                       permitir_features=True):
        """Devuelve las lineas `option ...;` de un mensaje de opciones."""
        if opciones is None:
            return []
        lineas = []
        for nombre in OPCIONES_CONOCIDAS.get(clase, []):
            campo = opciones.DESCRIPTOR.fields_by_name.get(nombre)
            if campo is None:
                continue
            if not opciones.HasField(nombre):
                continue
            valor = getattr(opciones, nombre)
            if campo.type == campo.TYPE_ENUM:
                texto = campo.enum_type.values_by_number[valor].name
            elif campo.type == campo.TYPE_BOOL:
                texto = "true" if valor else "false"
            elif campo.type == campo.TYPE_STRING:
                texto = '"%s"' % valor.replace("\\", "\\\\").replace('"', '\\"')
            else:
                texto = str(valor)
            lineas.append(f"{sangria}option {nombre} = {texto};")
        # features (FeatureSet) - solo tiene sentido en ficheros 'editions'
        if opciones.HasField("features"):
            fs_campo = opciones.DESCRIPTOR.fields_by_name["features"]
            if permitir_features:
                for sub in fs_campo.message_type.fields:
                    if sub.type != sub.TYPE_ENUM:
                        continue
                    if not opciones.features.HasField(sub.name):
                        continue
                    valor = getattr(opciones.features, sub.name)
                    texto = sub.enum_type.values_by_number[valor].name
                    lineas.append(f"{sangria}option features.{sub.name} = {texto};")
            else:
                self.no_representables.append(
                    (fichero, donde, "features (solo valido en ficheros 'editions')"))
        # extensiones personalizadas (campos >= 1000): no se pueden reemitir
        for num in self._campos_desconocidos(opciones):
            self.no_representables.append(
                (fichero, donde, f"opcion personalizada (campo {num})"))
        return lineas

    @staticmethod
    def _campos_desconocidos(opciones):
        """Numeros de campo presentes en el wire que la libreria no conoce.

        Son extensiones personalizadas (numero >= 1000).  No se pueden
        reemitir sin las definiciones de esas extensiones: se reportan.
        """
        conocidos = {f.number for f in opciones.DESCRIPTOR.fields}
        datos = opciones.SerializeToString()
        vistos = set()
        i = 0
        while i < len(datos):
            clave, i = _varint(datos, i)
            if clave is None:
                break
            num, wt = clave >> 3, clave & 7
            vistos.add(num)
            if wt == 0:
                _, i = _varint(datos, i)
            elif wt == 1:
                i += 8
            elif wt == 2:
                ln, i = _varint(datos, i)
                if ln is None:
                    break
                i += ln
            elif wt == 5:
                i += 4
            else:
                break
        return sorted(n for n in vistos - conocidos if n >= 1000)

    # -- mensajes -----------------------------------------------------------
    def escribir_mensaje(self, m, ambito, salida, sangria, modo):
        pre = ambito + "." if ambito else ""
        fqn = pre + m.name
        salida.append(f"{sangria}message {ident(m.name)} {{")
        i2 = sangria + "  "
        por_oneof = {}
        normales = []
        for f in m.field:
            if f.proto3_optional:
                # Los campos `proto3_optional` se representan en el
                # descriptor con un `oneof` sintetico (`_nombre`), pero en
                # texto .proto son un `optional` normal: no van dentro de
                # ningun bloque oneof.
                normales.append(f)
            elif f.HasField("oneof_index"):
                por_oneof.setdefault(f.oneof_index, []).append(f)
            else:
                normales.append(f)
        grupos = {f.type_name.rsplit(".", 1)[-1] for f in m.field
                  if f.type == descriptor_pb2.FieldDescriptorProto.TYPE_GROUP}
        for f in normales:
            if f.type == descriptor_pb2.FieldDescriptorProto.TYPE_GROUP:
                self.escribir_grupo(f, fqn, i2, salida, modo)
            else:
                self.escribir_campo(f, fqn, i2, salida, modo)
        for idx, decl in enumerate(m.oneof_decl):
            if idx not in por_oneof:
                continue  # oneof sintetico de un campo proto3_optional
            salida.append(f"{i2}oneof {ident(decl.name)} {{")
            for f in por_oneof[idx]:
                self.escribir_campo(f, fqn, i2 + "  ", salida, modo, en_oneof=True)
            salida.append(f"{i2}}}")
        for r in m.reserved_range:
            fin = r.end - 1
            if fin > r.start:
                salida.append(f"{i2}reserved {r.start} to {fin};")
            else:
                salida.append(f"{i2}reserved {r.start};")
        if m.reserved_name:
            nombres = (ident(n) if modo == "editions" else cadena(n)
                       for n in m.reserved_name)
            salida.append(f"{i2}reserved " + ", ".join(nombres) + ";")
        # rangos de extension: `extensions 1000 to max;`  (sin esto, las
        # opciones personalizadas que usan esos campos no resuelven)
        for r in m.extension_range:
            if r.end >= 536870912:
                salida.append(f"{i2}extensions {r.start} to max;")
            elif r.end - 1 > r.start:
                salida.append(f"{i2}extensions {r.start} to {r.end - 1};")
            else:
                salida.append(f"{i2}extensions {r.start};")
        for ln in self.opciones_texto(m.options, "MessageOptions", i2, "", fqn,
                                      permitir_features=(modo != "proto2")):
            salida.append(ln)
        for extendee, exts in self._agrupar_extensiones(m.extension, fqn).items():
            salida.append(f"{i2}extend {extendee} {{")
            for e in exts:
                self.escribir_campo(e, fqn, i2 + "  ", salida, modo, es_extension=True)
            salida.append(f"{i2}}}")
        for e in m.enum_type:
            self.escribir_enum(e, fqn, i2, salida, modo)
        for n in m.nested_type:
            if self.idx.es_map_entry(fqn + "." + n.name):
                continue
            if n.name in grupos:
                continue      # el grupo ya se ha escrito en linea
            self.escribir_mensaje(n, fqn, salida, i2, modo)
        salida.append(f"{sangria}}}")

    def escribir_grupo(self, f, ambito, sangria, salida, modo):
        """Escribe un campo `group` de proto2 (grupo = mensaje anidado).

        El descriptor guarda el grupo como un campo de tipo TYPE_GROUP mas
        un mensaje anidado con el mismo nombre; en texto .proto el grupo se
        escribe en linea y el mensaje anidado NO se emite por separado.
        """
        nombre = f.type_name.rsplit(".", 1)[-1]
        gm = self.idx.mensajes.get("." + f.type_name.lstrip("."))
        if gm is None:
            return self.escribir_campo(f, ambito, sangria, salida, modo)
        prefijo = ""
        if f.label == descriptor_pb2.FieldDescriptorProto.LABEL_REPEATED:
            prefijo = "repeated "
        elif f.label == descriptor_pb2.FieldDescriptorProto.LABEL_REQUIRED:
            prefijo = "required "
        else:
            prefijo = "optional "
        salida.append(f"{sangria}{prefijo}group {ident(nombre)} = {f.number} {{")
        for sub in gm.field:
            self.escribir_campo(sub, "." + f.type_name.lstrip("."), sangria + "  ",
                                salida, modo)
        salida.append(f"{sangria}}}")

    def escribir_campo(self, f, ambito, sangria, salida, modo, en_oneof=False,
                       es_extension=False):
        """Escribe un campo.

        `modo` es "proto2" | "proto3" | "editions" y decide que labels se
        pueden poner (medido con protoc 35.1):
          - proto2   : optional / required / repeated
          - proto3   : repeated y `optional` solo para los campos
                       `proto3_optional`
          - editions : repeated; `optional` NO existe (la presencia la
                       decide `features.field_presence`)
        """
        tipo, es_mapa, k, v = self.tipo_campo(f, ambito)
        etiqueta = tipo
        prefijo = ""
        repetido = f.label == descriptor_pb2.FieldDescriptorProto.LABEL_REPEATED
        if not es_mapa:
            if es_extension:
                if repetido:
                    prefijo = "repeated "
                elif modo == "editions":
                    prefijo = ""
                elif f.label == descriptor_pb2.FieldDescriptorProto.LABEL_REQUIRED:
                    prefijo = "required "
                else:
                    prefijo = "optional "
            elif en_oneof:
                prefijo = ""   # dentro de un oneof no puede haber label
            elif repetido:
                prefijo = "repeated "
            elif modo == "proto2":
                if f.label == descriptor_pb2.FieldDescriptorProto.LABEL_REQUIRED:
                    prefijo = "required "
                else:
                    prefijo = "optional "
            elif modo == "proto3" and f.proto3_optional:
                prefijo = "optional "
        partes = [f"{sangria}{prefijo}{etiqueta} {ident(f.name)} = {f.number}"]
        ops = []
        if f.HasField("options"):
            fc = f.options.DESCRIPTOR.fields_by_name
            for nombre in ("ctype", "jstype"):
                if f.options.HasField(nombre):
                    ops.append(f"{nombre} = {fc[nombre].enum_type.values_by_number[getattr(f.options, nombre)].name}")
            # `packed` solo tiene sentido en campos repetidos primitivos
            primitivo = f.type not in (
                descriptor_pb2.FieldDescriptorProto.TYPE_MESSAGE,
                descriptor_pb2.FieldDescriptorProto.TYPE_ENUM,
                descriptor_pb2.FieldDescriptorProto.TYPE_GROUP)
            if f.options.HasField("packed") and repetido and primitivo:
                ops.append("packed = " + ("true" if f.options.packed else "false"))
            if f.options.deprecated:
                ops.append("deprecated = true")
        if ops:
            partes.append("[" + ", ".join(ops) + "]")
        partes[-1] += ";"
        salida.append(" ".join(partes))

    # -- extensiones --------------------------------------------------------
    @staticmethod
    def _agrupar_extensiones(extensiones, ambito):
        """Agrupa las extensiones por `extendee` (el tipo que extienden)."""
        grupos = {}
        for e in extensiones:
            extendee = e.extendee.lstrip(".")
            if ambito and extendee.startswith(ambito.lstrip(".") + "."):
                extendee = extendee[len(ambito.lstrip(".")) + 1:]
            grupos.setdefault(extendee, []).append(e)
        return grupos

    def escribir_enum(self, e, ambito, sangria, salida, modo="proto2"):
        salida.append(f"{sangria}enum {ident(e.name)} {{")
        i2 = sangria + "  "
        for v in e.value:
            partes = [f"{i2}{ident(v.name)} = {v.number}"]
            ops = []
            if v.HasField("options") and v.options.deprecated:
                ops.append("deprecated = true")
            if ops:
                partes.append("[" + ", ".join(ops) + "]")
            partes.append(";")
            salida.append(" ".join(partes))
        for r in e.reserved_range:
            fin = r.end - 1
            if fin > r.start:
                salida.append(f"{i2}reserved {r.start} to {fin};")
            else:
                salida.append(f"{i2}reserved {r.start};")
        if e.reserved_name:
            nombres = (ident(n) if modo == "editions" else cadena(n)
                       for n in e.reserved_name)
            salida.append(f"{i2}reserved " + ", ".join(nombres) + ";")
        for ln in self.opciones_texto(e.options, "EnumOptions", i2, "", ambito,
                                      permitir_features=(modo == "editions")):
            salida.append(ln)
        salida.append(f"{sangria}}}")

    def escribir_servicio(self, s, ambito, salida, sangria):
        salida.append(f"{sangria}service {ident(s.name)} {{")
        i2 = sangria + "  "
        for m in s.method:
            ent = self.nombre_tipo(m.input_type, ambito)
            sal = self.nombre_tipo(m.output_type, ambito)
            cs = "stream " if m.client_streaming else ""
            ss = "stream " if m.server_streaming else ""
            partes = [f"{i2}rpc {ident(m.name)} ({cs}{ent}) returns ({ss}{sal})"]
            ops = []
            if m.HasField("options"):
                if m.options.deprecated:
                    ops.append("deprecated = true")
                if m.options.HasField("idempotency_level"):
                    mc = m.options.DESCRIPTOR.fields_by_name["idempotency_level"]
                    ops.append("idempotency_level = " +
                               mc.enum_type.values_by_number[m.options.idempotency_level].name)
            if ops:
                partes.append("{ " + " ".join("option " + o + ";" for o in ops) + " }")
            partes[-1] += ";"
            salida.append(" ".join(partes))
        salida.append(f"{sangria}}}")

    # -- fichero ------------------------------------------------------------
    def escribir_fichero(self, f):
        salida = []
        salida.append("// Reconstruido desde el FileDescriptorProto embebido en el binario.")
        salida.append("// Fichero original: " + f.name)
        salida.append("")
        modo = "proto2"
        if f.syntax == "proto3":
            salida.append('syntax = "proto3";')
            modo = "proto3"
        elif f.syntax == "proto2":
            salida.append('syntax = "proto2";')
        elif f.HasField("edition"):
            ed = descriptor_pb2.Edition.Name(f.edition).replace("EDITION_", "")
            if int(ed) > self.max_edition:
                self.ediciones_ajustadas.append(
                    (f.name, ed, str(self.max_edition)))
                ed = str(self.max_edition)
            salida.append('edition = "%s";' % ed)
            modo = "editions"
        else:
            salida.append('syntax = "proto2";')
        if f.package:
            salida.append("")
            salida.append("package %s;" % f.package)
        publics = set(f.public_dependency)
        debiles = set(f.weak_dependency)
        if f.dependency:
            salida.append("")
            for i, dep in enumerate(f.dependency):
                mod = ""
                if i in publics:
                    mod = "public "
                elif i in debiles:
                    mod = "weak "
                salida.append('import %s"%s";' % (mod, dep))
        ops = self.opciones_texto(f.options, "FileOptions", "", f.name, f.name,
                                  permitir_features=(modo != "proto2"))
        if ops:
            salida.append("")
            salida.extend(ops)
        ambito = "." + f.package if f.package else ""
        if f.message_type:
            salida.append("")
            for m in f.message_type:
                self.escribir_mensaje(m, ambito, salida, "", modo)
        if f.enum_type:
            salida.append("")
            for e in f.enum_type:
                self.escribir_enum(e, ambito, "", salida, modo)
        if f.extension:
            salida.append("")
            for extendee, exts in self._agrupar_extensiones(f.extension, ambito).items():
                salida.append(f"extend {extendee} {{")
                for e in exts:
                    self.escribir_campo(e, ambito, "  ", salida, modo, es_extension=True)
                salida.append("}")
        if f.service:
            salida.append("")
            for s in f.service:
                self.escribir_servicio(s, ambito, salida, "")
        salida.append("")
        return "\n".join(salida)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--descriptor-set", required=True)
    ap.add_argument("--out", required=True, help="directorio donde escribir los .proto")
    ap.add_argument("--stats", default=None, help="fichero JSON con las cifras")
    ap.add_argument("--no-representables", default=None,
                    help="TSV con las opciones que no se pueden reemitir")
    ap.add_argument("--max-edition", type=int, default=2024,
                    help="edicion maxima que soporta el protoc local (2026 no la "
                         "conoce protoc 35.1; esos ficheros se ajan y se listan "
                         "en ediciones_ajustadas.tsv)")
    args = ap.parse_args(argv)

    fds = descriptor_pb2.FileDescriptorSet()
    with open(args.descriptor_set, "rb") as fh:
        fds.ParseFromString(fh.read())

    idx = Indice(fds)
    esc = Escritor(fds, idx, max_edition=args.max_edition)
    os.makedirs(args.out, exist_ok=True)

    n_proto = 0
    bytes_total = 0
    for f in sorted(fds.file, key=lambda x: x.name):
        texto = esc.escribir_fichero(f)
        ruta = os.path.join(args.out, f.name)
        os.makedirs(os.path.dirname(ruta), exist_ok=True)
        with open(ruta, "w", encoding="utf-8") as fh:
            fh.write(texto)
        n_proto += 1
        bytes_total += len(texto.encode("utf-8"))

    if args.no_representables:
        with open(args.no_representables, "w", encoding="utf-8") as fh:
            fh.write("fichero\tdonde\tdetalle\n")
            for f, donde, detalle in esc.no_representables:
                fh.write(f"{f}\t{donde}\t{detalle}\n")

    if args.no_representables:
        with open(args.no_representables + ".ediciones.tsv", "w", encoding="utf-8") as fh:
            fh.write("fichero\tedicion_original\tedicion_escrita\n")
            for f, o, n in esc.ediciones_ajustadas:
                fh.write(f"{f}\t{o}\t{n}\n")

    stats = {
        "ficheros_proto": n_proto,
        "bytes_proto": bytes_total,
        "mensajes_top": sum(len(f.message_type) for f in fds.file),
        "enums_top": sum(len(f.enum_type) for f in fds.file),
        "servicios": sum(len(f.service) for f in fds.file),
        "mensajes_total_incl_nested": len(idx.mensajes),
        "enums_total_incl_nested": len(idx.enums),
        "dependencias": sum(len(f.dependency) for f in fds.file),
        "opciones_no_representables": len(esc.no_representables),
        "ediciones_ajustadas": len(esc.ediciones_ajustadas),
    }
    if args.stats:
        with open(args.stats, "w", encoding="utf-8") as fh:
            json.dump(stats, fh, indent=2, ensure_ascii=False)
    print(json.dumps(stats, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

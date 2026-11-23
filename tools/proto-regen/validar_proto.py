#!/usr/bin/env python3
"""Verifica el arbol de `.proto` reconstruido con `protoc` y mide la
FIDELIDAD de la reconstruccion comparando el descriptor recompilado con el
descriptor extraido del binario.

Que hace:
  1. Compila TODOS los `.proto` con `protoc` (el de `grpcio-tools`, que ya
     esta instalado en el venv) y falla si hay un solo error.
  2. Vuelve a leer el `FileDescriptorSet` que genera protoc y lo compara
     con el que se extrajo del binario: mensajes, enums, valores de enum,
     campos (numero, tipo, type_name, label, oneof, proto3_optional), campos
     de extension, servicios y rpcs.
  3. Escribe un informe JSON con las cifras medidas.

La comparacion es la PRUEBA de que la reconstruccion es fiel: si los dos
descriptoresSets coinciden campo a campo, el `.proto` es el original.

USO:
    python3 validar_proto.py --protos protos/ \
        --descriptor-set descriptor_set.pb --informe informe.json
"""
import argparse
import json
import os
import subprocess
import sys

from google.protobuf import descriptor_pb2

PROTOC_PY = "-m"
PROTOC_MOD = "grpc_tools.protoc"


def listar_protos(raiz):
    out = []
    for dirpath, _dirnames, filenames in os.walk(raiz):
        for fn in filenames:
            if fn.endswith(".proto"):
                out.append(os.path.relpath(os.path.join(dirpath, fn), raiz))
    out.sort()
    return out


def compilar(raiz, ficheros, salida_pb):
    """Compila con protoc.  Devuelve (ok, codigo, stdout+stderr)."""
    cmd = [sys.executable, PROTOC_PY, PROTOC_MOD,
           "--descriptor_set_out=" + salida_pb, "--include_imports",
           "-I", raiz] + ficheros
    p = subprocess.run(cmd, capture_output=True, text=True)
    return p.returncode == 0, p.returncode, (p.stdout or "") + (p.stderr or "")


# --- comparacion -----------------------------------------------------------
def idx_mensajes(f):
    """{(ruta): DescriptorProto} para todos los mensajes (con anidados)."""
    out = {}

    def rec(m, pre):
        clave = pre + "." + m.name
        out[clave] = m
        for n in m.nested_type:
            if n.options and n.options.map_entry:
                continue
            rec(n, clave)

    for m in f.message_type:
        rec(m, f.package)
    return out


def idx_enums(f):
    out = {}

    def rec(m, pre):
        clave = pre + "." + m.name
        for e in m.enum_type:
            out[clave + "." + e.name] = e
        for n in m.nested_type:
            if n.options and n.options.map_entry:
                continue
            rec(n, clave)

    for e in f.enum_type:
        out[f.package + "." + e.name] = e
    for m in f.message_type:
        rec(m, f.package)
    return out


def firma_campo(f):
    """Firma canonica de los campos (incluye extensions)."""
    out = set()
    for ms in idx_mensajes(f).values():
        for fl in ms.field:
            out.add((ms.name, fl.name, fl.number, fl.type, fl.type_name, fl.label,
                     fl.HasField("oneof_index") and fl.oneof_index or -1,
                     fl.proto3_optional))
    for fl in f.extension:
        out.add(("<file>", fl.name, fl.number, fl.type, fl.type_name, fl.label,
                 -1, False))
    return out


def firma_enum(f):
    out = set()
    for clave, e in idx_enums(f).items():
        for v in e.value:
            out.add((clave, v.name, v.number))
    return out


def firma_rpc(f):
    return set((f.package + "." + s.name, m.name, m.input_type, m.output_type,
                m.client_streaming, m.server_streaming)
               for s in f.service for m in s.method)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--protos", required=True, help="raiz del arbol de .proto")
    ap.add_argument("--descriptor-set", required=True,
                    help="descriptor_set.pb original (extraido del binario)")
    ap.add_argument("--informe", default=None, help="JSON de salida")
    ap.add_argument("--salida-pb", default=None,
                    help="descriptor_set.pb recompilado (por defecto, temporal)")
    args = ap.parse_args(argv)

    ficheros = listar_protos(args.protos)
    salida_pb = args.salida_pb or os.path.join(
        os.path.dirname(os.path.abspath(args.descriptor_set)), "recompilado.pb")
    ok, rc, salida = compilar(args.protos, ficheros, salida_pb)

    informe = {
        "ficheros_proto": len(ficheros),
        "protoc_ok": ok,
        "protoc_codigo": rc,
        "protoc_salida": salida[-4000:],
    }

    if ok:
        orig = descriptor_pb2.FileDescriptorSet()
        with open(args.descriptor_set, "rb") as fh:
            orig.ParseFromString(fh.read())
        comp = descriptor_pb2.FileDescriptorSet()
        with open(salida_pb, "rb") as fh:
            comp.ParseFromString(fh.read())
        O = {f.name: f for f in orig.file}
        C = {f.name: f for f in comp.file}
        comunes = sorted(set(O) & set(C))
        dif_campos, dif_enums, dif_rpcs = [], [], []
        n_campos = n_enums = n_rpcs = 0
        for n in comunes:
            a, b = firma_campo(O[n]), firma_campo(C[n])
            n_campos += len(a)
            if a != b:
                dif_campos.append(n)
            a, b = firma_enum(O[n]), firma_enum(C[n])
            n_enums += len(a)
            if a != b:
                dif_enums.append(n)
            a, b = firma_rpc(O[n]), firma_rpc(C[n])
            n_rpcs += len(a)
            if a != b:
                dif_rpcs.append(n)
        informe.update({
            "ficheros_originales": len(O),
            "ficheros_recompilados": len(C),
            "ficheros_comunes": len(comunes),
            "ficheros_solo_original": sorted(set(O) - set(C)),
            "ficheros_solo_recompilado": sorted(set(C) - set(O)),
            "campos_comparados": n_campos,
            "ficheros_con_campos_distintos": dif_campos,
            "valores_enum_comparados": n_enums,
            "ficheros_con_enums_distintos": dif_enums,
            "rpcs_comparados": n_rpcs,
            "ficheros_con_rpcs_distintos": dif_rpcs,
            "fidelidad": (not dif_campos and not dif_enums and not dif_rpcs
                          and set(O) == set(C)),
        })

    if args.informe:
        with open(args.informe, "w", encoding="utf-8") as fh:
            json.dump(informe, fh, indent=2, ensure_ascii=False)
    print(json.dumps(informe, indent=2, ensure_ascii=False))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())

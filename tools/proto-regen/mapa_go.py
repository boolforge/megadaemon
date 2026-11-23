#!/usr/bin/env python3
"""Mapa `.proto` -> ruta de importacion Go, medido del binario.

El binario conserva el nombre del paquete Go de cada fichero generado en la
tabla de simbolos (`go.funcs.tsv`): el prefijo del nombre del simbolo es el
paquete Go con los separadores convertidos en `_`.

    blaze-out/arm-opt/bin/apps/framework/data/service_annotations.pb.go
      -> google3_apps_framework_data_service_annotations_go_proto
      -> ruta de importacion: google3/apps/framework/data/service_annotations_go_proto

Regla de inversion (MEDIDA sobre los 554 .pb.go de `go.funcs.tsv`):
  * el prefijo del simbolo es la ruta de importacion con `/` y `.` -> `_`;
  * el ULTIMO elemento del prefijo es el nombre del paquete Go;
  * la ruta de importacion es "google3/" + (directorio del .pb.go) y, si el
    nombre del paquete no coincide con el basename del directorio, se le
    anade "/<nombre_del_paquete>".

Este mapa es lo que permite a `protoc-gen-go` escribir el codigo en la misma
estructura de directorios que tenia el binario original.
"""
import os
import re

RE_ORIGEN = re.compile(r"\| (\S+\.pb\.go):(\d+)\s*$")
PREFIJO_BLAZE = "blaze-out/arm-opt/bin/"


def leer_mapa(go_funcs_tsv):
    """Devuelve {ruta_relativa_del_pb_go: ruta_de_importacion_go}.

    La ruta de importacion se saca del campo `origen` de `go.funcs.tsv`, que
    trae el simbolo COMPLETO y sin "manglar":
        "go: google3/net/proto2/proto/descriptor_go_proto.Edition.Enum | ..."
    El paquete de Go es el PREFIJO COMUN mas largo de todos los simbolos del
    fichero (asi los simbolos anidados no lo contaminan).
    """
    mapa = {}
    if not go_funcs_tsv or not os.path.exists(go_funcs_tsv):
        return mapa
    prefijos = {}
    with open(go_funcs_tsv, "r", encoding="utf-8", errors="replace") as fh:
        for linea in fh:
            partes = linea.rstrip("\n").split("\t")
            if len(partes) < 3:
                continue
            m = RE_ORIGEN.search(partes[2])
            if not m:
                continue
            pb = m.group(1)
            completo = partes[2][3:].split(" | ")[0].strip()
            if not completo or "." not in completo:
                continue
            actual = prefijos.get(pb)
            if actual is None:
                prefijos[pb] = completo.rsplit(".", 1)[0]
            else:
                cand = completo.rsplit(".", 1)[0]
                a = actual.split(".")
                b = cand.split(".")
                comunes = []
                for x, y in zip(a, b):
                    if x != y:
                        break
                    comunes.append(x)
                prefijos[pb] = ".".join(comunes)
    for pb, pref in prefijos.items():
        if pref:
            mapa[pb] = pref
    return mapa


def mangle(ruta):
    """El "mangled" que usa la tabla de simbolos: `/` y `.` -> `_`."""
    return ruta.replace("/", "_").replace(".", "_")


def ruta_importacion(pb_go, paquete_mangled,Raiz="google3"):
    """Invierte el "mangled" del paquete Go a una ruta de importacion.

    La inversion NO es univoca (el "mangled" pierde los separadores), asi
    que se generan las rutas plausibles y se ELIGE la que, al volver a
    "manglar", da exactamente el nombre medido en el binario.  Asi el dato
    queda verificado en vez de supuesto.
    """
    rel = pb_go[len(PREFIJO_BLAZE):] if pb_go.startswith(PREFIJO_BLAZE) else pb_go
    rel = rel[:-len(".pb.go")]
    directorio = os.path.dirname(rel)
    base = os.path.basename(rel)
    base_dir = os.path.basename(directorio) if directorio else ""
    sufijos = []
    for s in (base + "_go_proto", base, base_dir, base_dir + "_go_proto",
              base + "_pb", base_dir + "_pb"):
        if s and s not in sufijos:
            sufijos.append(s)
    cands = []
    for d in (directorio, ""):
        for suf in sufijos:
            cands.append((Raiz + "/" + d + "/" + suf) if d else (Raiz + "/" + suf))
    for c in cands:
        if mangle(c) == paquete_mangled:
            return c
    return cands[0]


def construir_mapa(protos, go_funcs_tsv, raiz="google3"):
    """{ruta .proto: ruta de importacion Go} para los .proto dados."""
    mapa_pb = leer_mapa(go_funcs_tsv)
    salida = {}
    for proto in protos:
        rel = proto[:-len(".proto")] if proto.endswith(".proto") else proto
        pb_go = "blaze-out/arm-opt/bin/" + rel + ".pb.go"
        pkg = mapa_pb.get(pb_go)
        if pkg:
            salida[proto] = ruta_importacion(pb_go, pkg)
        else:
            # sin medida: se deriva del propio nombre del .proto
            directorio = os.path.dirname(rel)
            base = os.path.basename(rel)
            if directorio:
                salida[proto] = raiz + "/" + directorio + "/" + base
            else:
                salida[proto] = raiz + "/" + base
    return salida


if __name__ == "__main__":  # pragma: no cover - diagnostico
    import sys
    protos = sys.argv[1:]
    for k, v in construir_mapa(protos, "/workspace/agy-re/tools-out/go.funcs.tsv").items():
        print(f"{k}\t{v}")

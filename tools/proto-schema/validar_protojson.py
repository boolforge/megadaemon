#!/usr/bin/env python3
"""Valida un JSON contra el esquema protobuf embebido en un binario (protojson).

El language server de Antigravity parsea la respuesta de `:fetchAvailableModels`
con **protojson**, no con JSON a pelo.  Eso significa que un campo `map` tiene
que ser un **objeto** (no una lista) y que un campo desconocido es un ERROR.

MEDIDO (2026-11-26, móvil): nuestro shim servía ahí el catálogo de MegaDaemon,
que es `{"models":[...]}`, y el LS petaba exactamente en la llave:

    Cache(availableModels): Singleflight refresh failed:
    proto: syntax error (line 1:11): unexpected token [

y con ese error la app volvía a pedir el login de Google en bucle.

Esta herramienta es el **probe** que faltaba: valida **antes** de mandar la
respuesta, contra el esquema REAL extraído del propio `.so`.

Uso:
    validar_protojson.py <binario> <fichero.proto> <mensaje> <fichero.json>
    validar_protojson.py --esquema <esquema.json> <mensaje> <fichero.json>
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from extraer_esquema import (  # noqa: E402
    buscar_blob,
    es_mapa,
    parse_fichero,
    secciones_rodata,
)

# índice global de mensajes (nombre corto -> DescriptorProto)
INDICE = {}


def indexar(mensajes, prefijo=""):
    """Indexa mensajes y tipos anidados por nombre corto y por nombre completo."""
    for m in mensajes:
        nombre = m.get("name") or ""
        completo = f"{prefijo}.{nombre}" if prefijo else nombre
        INDICE[nombre] = m
        INDICE[completo] = m
        indexar(m.get("nested", []), completo)


def resolver(tipo):
    if not tipo:
        return None
    t = tipo.lstrip(".")
    return INDICE.get(t) or INDICE.get(t.split(".")[-1])


def json_name(campo):
    """Nombre JSON de protojson: el `json_name` si está, si no camelCase."""
    if campo.get("json_name"):
        return campo["json_name"]
    n = campo["name"] or ""
    partes = n.split("_")
    return partes[0] + "".join(p.capitalize() for p in partes[1:])


def tipo_de_mapa(mensaje, campo):
    """Mensaje del VALOR de un campo map (la entrada XxxEntry, campo 2)."""
    entrada = resolver(campo.get("type_name"))
    if entrada is None:
        return None
    for c in entrada.get("fields", []):
        if c.get("number") == 2:
            return resolver(c.get("type_name"))
    return None


def valida_mensaje(mensaje, valor, ruta, errores):
    if not isinstance(valor, dict):
        errores.append(f"{ruta}: se esperaba un objeto, hay {type(valor).__name__}")
        return
    campos = {}
    for c in mensaje.get("fields", []):
        campos[json_name(c)] = c
        campos[c["name"]] = c
    for k, v in valor.items():
        if k not in campos:
            errores.append(f"{ruta}.{k}: campo desconocido (protojson lo rechaza)")
            continue
        c = campos[k]
        etiqueta = f"{ruta}.{k}"
        if es_mapa(mensaje, c):
            if not isinstance(v, dict):
                errores.append(
                    f"{etiqueta}: es un MAP ⇒ debe ser un objeto JSON, "
                    f"hay {type(v).__name__}"
                )
                continue
            valor_msg = tipo_de_mapa(mensaje, c)
            for kk, vv in v.items():
                if valor_msg is not None:
                    valida_mensaje(valor_msg, vv, f"{etiqueta}[{kk}]", errores)
            continue
        if c.get("label") == 3:  # repeated
            if not isinstance(v, list):
                errores.append(
                    f"{etiqueta}: es repeated ⇒ debe ser una lista JSON, "
                    f"hay {type(v).__name__}"
                )
                continue
            item_msg = resolver(c.get("type_name"))
            for i, item in enumerate(v):
                if item_msg is not None:
                    valida_mensaje(item_msg, item, f"{etiqueta}[{i}]", errores)
            continue
        sub = resolver(c.get("type_name"))
        if sub is not None:
            valida_mensaje(sub, v, etiqueta, errores)


def main():
    argv = sys.argv[1:]
    modo_esquema = "--esquema" in argv
    pos = []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a.startswith("--"):
            i += 1
            continue
        pos.append(a)
        i += 1
    args = pos
    if not args:
        print(__doc__)
        raise SystemExit(2)
    if modo_esquema:
        esquema = json.load(open(args[0]))
        indexar(esquema.get("messages", []))
        mensaje_nombre, fichero_json = args[1], args[2]
    else:
        if len(args) < 4:
            print(__doc__)
            raise SystemExit(2)
        binario, nombre_proto, mensaje_nombre, fichero_json = args[:4]
        data, _ = secciones_rodata(binario)
        idx = buscar_blob(data, nombre_proto)
        if idx < 0:
            raise SystemExit(f"no encontrado el descriptor de {nombre_proto}")
        fich = parse_fichero(data[idx:idx + 400000])
        indexar(fich.get("messages", []))
    msg = INDICE.get(mensaje_nombre)
    if msg is None:
        print(f"mensaje {mensaje_nombre} no encontrado; hay: "
              f"{', '.join(sorted(k for k in INDICE if '.' not in k)[:25])}")
        raise SystemExit(2)
    doc = json.load(open(fichero_json))
    errores = []
    valida_mensaje(msg, doc, mensaje_nombre, errores)
    if errores:
        print(f"FALLA ({len(errores)} problema/s):")
        for e in errores[:40]:
            print("  -", e)
        raise SystemExit(1)
    print("OK: el JSON cumple el esquema protojson")


if __name__ == "__main__":
    main()

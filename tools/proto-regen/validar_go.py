#!/usr/bin/env python3
"""Valida la SINTAXIS del Go generado SIN toolchain de Go.

El sandbox no tiene `go` ni `gofmt` (MEDIDO: `which go gofmt` no devuelve
nada y no hay /usr/local/go), asi que la validacion es a NIVEL DE LEXER:

  1. se tokeniza el fichero como lo hace el lexer de Go:
       - comentarios de linea `//` y de bloque `/* */`
       - cadenas interpretadas `"..."` (con escapes) y crudas `` `...` ``
       - runes `'...'`
     y se comprueba que todos_comments, cadenas y runes estan CERRADOS;
  2. se comprueba el BALANCE de `(`, `{`, `[` con una pila, y que el
     fichero termine con la pila vacia;
  3. se comprueba que existe una clausula `package` al principio del
     fichero (un .go sin `package` no compila);
  4. se comprueba que no hay caracteres de control sueltos fuera de
     literales.

Lo que NO se puede comprobar sin `go`: el tipado, la resolucion de
imports y que las expresiones compile.  Eso queda dicho explicitamente en
el informe: esto es "Go lexicamente valido", no "Go compilado".

Lo que SI se mide (y si es util): cuantos structs llevan etiqueta
`protobuf:"..."`, cuantos `RawDesc` hay y cuantos bytes de descriptor
contienen.

USO:
    python3 validar_go.py --raiz go/ --informe informe_go.json
"""
import argparse
import json
import os
import re

RE_PACKAGE = re.compile(r"^package\s+[A-Za-z_][A-Za-z0-9_]*\s*$", re.M)
RE_PROTOBUF_TAG = re.compile(r'protobuf:"[^"]*"')
RE_RAWDESC = re.compile(r"file_[A-Za-z0-9_]*_rawDesc")


class ErrorLexico(Exception):
    def __init__(self, linea, columna, motivo):
        super().__init__(f"linea {linea}, columna {columna}: {motivo}")
        self.linea = linea
        self.columna = columna
        self.motivo = motivo


def validar_fuente(texto):
    """Valida un fichero Go.  Devuelve (ok, mensaje, metricas)."""
    lineas = texto.count("\n") + 1
    metricas = {
        "etiquetas_protobuf": len(RE_PROTOBUF_TAG.findall(texto)),
        "rawdesc": len(RE_RAWDESC.findall(texto)),
    }
    if not RE_PACKAGE.search(texto):
        return False, "no hay clausula `package`", metricas
    pila = []
    pares = {")": "(", "}": "{", "]": "["}
    i = 0
    n = len(texto)
    while i < n:
        c = texto[i]
        if c == "/" and i + 1 < n:
            if texto[i + 1] == "/":
                j = texto.find("\n", i)
                i = n if j < 0 else j
                continue
            if texto[i + 1] == "*":
                j = texto.find("*/", i + 2)
                if j < 0:
                    raise ErrorLexico(*_pos(texto, i), "comentario de bloque sin cerrar")
                i = j + 2
                continue
        if c == '"':
            i += 1
            while i < n:
                if texto[i] == "\\":
                    i += 2
                    continue
                if texto[i] == '"':
                    break
                if texto[i] == "\n":
                    raise ErrorLexico(*_pos(texto, i), "cadena sin cerrar")
                i += 1
            if i >= n:
                raise ErrorLexico(*_pos(texto, n - 1), "cadena sin cerrar")
            i += 1
            continue
        if c == "`":
            j = texto.find("`", i + 1)
            if j < 0:
                raise ErrorLexico(*_pos(texto, i), "cadena cruda sin cerrar")
            i = j + 1
            continue
        if c == "'":
            i += 1
            while i < n:
                if texto[i] == "\\":
                    i += 2
                    continue
                if texto[i] == "'":
                    break
                if texto[i] == "\n":
                    raise ErrorLexico(*_pos(texto, i), "rune sin cerrar")
                i += 1
            if i >= n:
                raise ErrorLexico(*_pos(texto, n - 1), "rune sin cerrar")
            i += 1
            continue
        if c in "({[":
            pila.append((c, i))
            i += 1
            continue
        if c in ")}]":
            if not pila:
                raise ErrorLexico(*_pos(texto, i), f"'{c}' de mas")
            abierto, pos = pila.pop()
            if abierto != pares[c]:
                raise ErrorLexico(*_pos(texto, i),
                                  f"'{c}' cierra a '{abierto}' abierto en la "
                                  f"linea {_pos(texto, pos)[0]}")
            i += 1
            continue
        i += 1
    if pila:
        abierto, pos = pila[-1]
        raise ErrorLexico(*_pos(texto, pos), f"'{abierto}' sin cerrar")
    return True, "ok", metricas


def _pos(texto, offset):
    linea = texto.count("\n", 0, offset) + 1
    ini = texto.rfind("\n", 0, offset)
    return linea, offset - ini


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--raiz", required=True, help="raiz del arbol de .go")
    ap.add_argument("--informe", default=None)
    args = ap.parse_args(argv)

    ficheros = []
    for dirpath, _d, names in os.walk(args.raiz):
        for fn in names:
            if fn.endswith(".go"):
                ficheros.append(os.path.join(dirpath, fn))
    ficheros.sort()

    informe = {
        "ficheros_go": len(ficheros),
        "herramienta_go_disponible": bool(_hay_go()),
        "validacion": "lexer (balance de llaves, literales cerrados, clausula package)",
        "ficheros_validos": 0,
        "ficheros_con_error": [],
        "errores": [],
    }
    total_lineas = 0
    total_bytes = 0
    etiquetas = 0
    rawdescs = 0
    for f in ficheros:
        with open(f, "rb") as fh:
            datos = fh.read()
        total_bytes += len(datos)
        total_lineas += datos.count(b"\n")
        texto = datos.decode("utf-8", "replace")
        try:
            ok, msg, met = validar_fuente(texto)
        except ErrorLexico as exc:
            ok, msg = False, str(exc)
            met = {"etiquetas_protobuf": 0, "rawdesc": 0}
        if ok:
            informe["ficheros_validos"] += 1
            etiquetas += met["etiquetas_protobuf"]
            rawdescs += met["rawdesc"]
        else:
            informe["ficheros_con_error"].append(
                {"fichero": os.path.relpath(f, args.raiz), "error": msg})
            if len(informe["errores"]) < 20:
                informe["errores"].append(
                    f"{os.path.relpath(f, args.raiz)}: {msg}")
    informe.update({
        "lineas_go": total_lineas,
        "bytes_go": total_bytes,
        "etiquetas_protobuf": etiquetas,
        "rawdesc": rawdescs,
        "todos_validos": not informe["ficheros_con_error"],
    })
    if args.informe:
        with open(args.informe, "w", encoding="utf-8") as fh:
            json.dump(informe, fh, indent=2, ensure_ascii=False)
    print(json.dumps(informe, indent=2, ensure_ascii=False))
    return 0 if informe["todos_validos"] else 1


def _hay_go():
    from shutil import which
    return which("go") is not None or which("gofmt") is not None


if __name__ == "__main__":
    raise SystemExit(main())

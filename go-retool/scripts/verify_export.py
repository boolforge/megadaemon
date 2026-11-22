#!/usr/bin/env python3
"""Verifica el export de go-retool contra la tabla de referencia.

COMPARA (contra `go.funcs.tsv`, la tabla de referencia del agente de RE):
  1. el conjunto de direcciones,
  2. el nombre REAL de Go de cada funcion,
  3. el fichero:linea de cada funcion.

Uso:
  python3 verify_export.py <export.tsv> <referencia.tsv>

El export de go-retool tiene este formato TSV:
    0x<va>\t<nombre>\t<fichero>:<linea>\t0x<va_fin>\t<paquete>

La referencia (`go.funcs.tsv`) tiene:
    0x<va>\t<nombre_mangled>\tgo: <nombre real> | <fichero>:<linea>

Salida: un resumen por stdout y codigo de salida 1 si hay discrepancias.
"""
import sys


def cargar_export(ruta):
    filas = {}
    with open(ruta, encoding="utf-8") as fh:
        for n, linea in enumerate(fh, 1):
            linea = linea.rstrip("\n")
            if not linea:
                continue
            p = linea.split("\t")
            if len(p) < 4:
                raise SystemExit(f"{ruta}:{n}: se esperaban 4 campos, hay {len(p)}")
            filas[p[0]] = {"nombre": p[1], "loc": p[2], "fin": p[3]}
    return filas


def cargar_referencia(ruta):
    filas = {}
    with open(ruta, encoding="utf-8") as fh:
        for n, linea in enumerate(fh, 1):
            linea = linea.rstrip("\n")
            if not linea:
                continue
            p = linea.split("\t")
            if len(p) < 3:
                raise SystemExit(f"{ruta}:{n}: se esperaban 3 campos, hay {len(p)}")
            # "go: <nombre> | <fichero>:<linea>"
            real = p[2]
            if real.startswith("go: "):
                real = real[4:]
            if " | " in real:
                nombre, loc = real.split(" | ", 1)
            else:
                nombre, loc = real, ""
            filas[p[0]] = {"nombre": nombre, "loc": loc, "mangled": p[1]}
    return filas


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    exp = cargar_export(sys.argv[1])
    ref = cargar_referencia(sys.argv[2])

    solo_exp = set(exp) - set(ref)
    solo_ref = set(ref) - set(exp)
    comunes = set(exp) & set(ref)

    mal_nombre = []
    mal_loc = []
    for va in comunes:
        if exp[va]["nombre"] != ref[va]["nombre"]:
            mal_nombre.append((va, exp[va]["nombre"], ref[va]["nombre"]))
        # la referencia usa "go: <nombre>" como fichero cuando no hay fichero
        want = ref[va]["loc"]
        got = exp[va]["loc"]
        if want.startswith("go: "):
            want = ""
        # "?" en el export significa "sin fichero resoluble"; en la
        # referencia esos casos vienen sin fichero (cadena vacia).
        if got == "?" and want == "":
            continue
        if got != want:
            mal_loc.append((va, got, want))

    print(f"export      : {len(exp)} funciones")
    print(f"referencia  : {len(ref)} funciones")
    print(f"comunes     : {len(comunes)}")
    print(f"solo export : {len(solo_exp)}  {sorted(solo_exp)[:5]}")
    print(f"solo ref    : {len(solo_ref)}  {sorted(solo_ref)[:5]}")
    print(f"nombre diff : {len(mal_nombre)}  {mal_nombre[:3]}")
    print(f"loc diff    : {len(mal_loc)}  {mal_loc[:3]}")

    ok = not (solo_exp or solo_ref or mal_nombre or mal_loc)
    print("RESULTADO   :", "IDENTICO" if ok else "DIFERENCIAS")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

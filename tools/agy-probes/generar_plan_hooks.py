#!/usr/bin/env python3
"""Genera el **plan de hooks** del payload frida-gum para el language server.

El payload (frida-gum) que ya instala MegaDaemon lee un plan de texto
tabulado y engancha `base + va_elf` en el módulo vivo:

    RUTA_DEL_MODULO <TAB> VA_ELF_HEX <TAB> ID <TAB> TAREA [TAB> VALOR_HEX]

* `VA_ELF_HEX` tiene que ser la dirección **del pclntab** (la del ELF), NO la
  del índice de Ghidra: celle está 1 MiB por encima (medido, 93 % de las
  funciones) ⇒ un plan hecho con Ghidra apunta 1 MiB equivocado.
* `TAREA` puede ser `observar` (loguear la llamada) o `sustituir_url`
  (cambiar el retorno).

De dónde salen las direcciones: el export del **pclntab** (`go.funcs.tsv`),
que tiene el nombre Go plano, la ruta del fichero y la línea original.

Uso:
    generar_plan_hooks.py <go.funcs.tsv> [--fichero REGEX] [--simbolo REGEX]
                          [--tarea observar] [--salida plan.txt]
                          [--listar] [--maximo N]
"""
import argparse
import re
import sys

MODULO_POR_DEFECTO = "liblanguage_server.so"


def cargar(tsv):
    """Devuelve [(va_hex, simbolo, fichero, linea, go_path)]."""
    out = []
    with open(tsv, "r", errors="replace") as f:
        for linea in f:
            partes = linea.rstrip("\n").split("\t")
            if len(partes) < 3:
                continue
            va = partes[0].strip()
            if not va.startswith("0x"):
                continue
            simbolo = partes[1].strip()
            resto = partes[2]
            # "go: <pkg>.<Func> | <fichero>:<linea>"
            go_path, _, ubic = resto.partition("|")
            go_path = go_path.replace("go:", "").strip()
            fichero, _, linea_go = ubic.strip().rpartition(":")
            try:
                linea_go = int(linea_go)
            except ValueError:
                linea_go = 0
            out.append((va, simbolo, fichero.strip(), linea_go, go_path))
    return out


def id_de(simbolo):
    """El ID que usa el daemon: el último trozo tras el punto."""
    return simbolo.rsplit(".", 1)[-1] if "." in simbolo else simbolo


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("tsv", help="export del pclntab (go.funcs.tsv)")
    ap.add_argument("--fichero", help="regex sobre el fichero .go")
    ap.add_argument("--simbolo", help="regex sobre el símbolo")
    ap.add_argument("--go", help="regex sobre el path Go")
    ap.add_argument("--tarea", default="observar")
    ap.add_argument("--modulo", default=MODULO_POR_DEFECTO)
    ap.add_argument("--salida", help="fichero donde escribir el plan")
    ap.add_argument("--listar", action="store_true", help="solo listar, sin escribir")
    ap.add_argument("--maximo", type=int, default=0)
    a = ap.parse_args()

    rx_f = re.compile(a.fichero) if a.fichero else None
    rx_s = re.compile(a.simbolo) if a.simbolo else None
    rx_g = re.compile(a.go) if a.go else None

    filas = []
    for va, simbolo, fichero, linea, go_path in cargar(a.tsv):
        if rx_f and not rx_f.search(fichero):
            continue
        if rx_s and not rx_s.search(simbolo):
            continue
        if rx_g and not rx_g.search(go_path):
            continue
        filas.append((va, simbolo, fichero, linea))
    # Orden estable: por fichero y línea (así se lee como el código).
    filas.sort(key=lambda f: (f[2], f[3], f[1]))
    if a.maximo and len(filas) > a.maximo:
        filas = filas[: a.maximo]

    print(f"hooks seleccionados: {len(filas)}  tarea={a.tarea}")
    for va, simbolo, fichero, linea in filas[:20]:
        print(f"  {va}  {fichero}:{linea}  {simbolo}")
    if len(filas) > 20:
        print(f"  ... y {len(filas) - 20} más")

    if a.listar:
        return 0
    texto = "".join(
        f"{a.modulo}\t{va}\t{id_de(simbolo)}\t{a.tarea}\n" for va, simbolo, _, _ in filas
    )
    if a.salida:
        with open(a.salida, "w") as f:
            f.write(texto)
        print(f"escrito: {a.salida} ({len(filas)} hooks)")
    else:
        sys.stdout.write(texto)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

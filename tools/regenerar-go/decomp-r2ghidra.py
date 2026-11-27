#!/usr/bin/env python3
"""Decompilacion al 100 % del binario Go de Antigravity con r2ghidra.

Por que este camino (medido el 2026-11-27):
  * Ghidra 11.2.1 + JDK 21 (ya instalados) importan el .so, pero decompilar
    161.682 funciones con la JVM cuesta horas y se came 3 GB (3 OOM medidos).
  * r2ghidra 6.2.6 = el MISMO decompilador de Ghidra en C++ dentro de radare2
    6.2.6: sin JVM, memoria de MB y una invocacion por lote.
  * Las fronteras de cada funcion NO se adivinan: salen del pclntab de Go
    (go.funcs.tsv, 161.682 filas) => tamaño exacto = siguiente VA - VA.

Uso:
    decomp-r2ghidra.py --tsv go.funcs.tsv --bin liblanguage_server.so \\
        --out todo/rz0 --skip todo/s0/_index.tsv todo/s1/_index.tsv ... \\
        --chunk 1500 --workers 1

Es reanudable: si un .c ya existe y no está vacio, se salta.
Salida: <out>/<paquete>/<funcion>.c  +  <out>/_index.tsv
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time

R2 = "/workspace/re-tools/r2.sh"


def cargar_funciones(path_tsv: str):
    """(addr, nombre, fichero_go, linea) de cada fila del pclntab."""
    filas = []
    with open(path_tsv, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            partes = line.rstrip("\n").split("\t")
            if len(partes) < 3:
                continue
            try:
                addr = int(partes[0], 16)
            except ValueError:
                continue
            nombre = partes[1]
            origen = partes[2]
            fichero, _, linea = origen.rpartition(":")
            if "|" in origen:
                fichero = origen.split("|", 1)[1].strip()
                fichero, _, linea = fichero.rpartition(":")
            filas.append((addr, nombre, fichero.strip() or "<desconocido>", linea.strip()))
    return filas


def tamaños_por_frontera(filas, maximo=0x40000):
    """Tamano de cada funcion = siguiente VA - VA (acotado)."""
    filas_ord = sorted(filas)
    tamanos = {}
    for i, (addr, nombre, _f, _l) in enumerate(filas_ord):
        if i + 1 < len(filas_ord):
            tam = filas_ord[i + 1][0] - addr
        else:
            tam = maximo
        if tam <= 0 or tam > maximo:
            tam = maximo
        tamanos[addr] = tam
    return tamanos


def sanea(nombre: str) -> str:
    return "".join(c if (c.isalnum() or c in "._-") else "_" for c in nombre)


def ya_hechos(indice: str, base_ghidra: int = 0x100000) -> set:
    """Direcciones ya decompiladas segun un _index.tsv de Ghidra.

    MEDIDO: Ghidra carga este ELF con image base 0x100000, asi que sus
    direcciones son vaddr + 0x100000 (comprobado en 12/12 muestras).
    """
    hechos = set()
    if not os.path.exists(indice):
        return hechos
    with open(indice, encoding="utf-8", errors="replace") as fh:
        next(fh, None)
        for line in fh:
            p = line.rstrip("\n").split("\t")
            if len(p) >= 6 and p[5].strip() in ("1", "true", "True"):
                try:
                    hechos.add(int(p[0], 16) - base_ghidra)
                except ValueError:
                    pass
    return hechos


# Paquetesinterestinges primero: si el barrido no termina, lo valioso ya esta.
PRIORIDAD = (
    "jetski",
    "gemini_coder",
    "devtools/ai",
    "modelcontextprotocol",
    "security",
    "sandbox",
    "base/go",
    "cel/go",
    "cloud",
    "production",
    "gdm",
    "research",
    "third_party/golang/golang.org/x",
    "third_party/golang/google.golang.org",
)


def rango_prioridad(fichero: str) -> int:
    for i, p in enumerate(PRIORIDAD):
        if p in fichero:
            return i
    return len(PRIORIDAD)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tsv", required=True)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--skip", nargs="*", default=[])
    ap.add_argument("--chunk", type=int, default=1500)
    ap.add_argument("--limite", type=int, default=0, help="0 = todas")
    ap.add_argument("--solo-paquete", default="")
    ap.add_argument("--paridad", type=int, default=-1, help="0 y 1 reparten el trabajo entre 2 workers; -1 = sin repartir")
    ap.add_argument("--prioridad", action="store_true", help="paquetes interestinges primero")
    args = ap.parse_args()

    filas = cargar_funciones(args.tsv)
    tamanos = tamaños_por_frontera(filas)
    hechos = set()
    for idx in args.skip:
        hechos |= ya_hechos(idx)
    print(f"[decomp] funciones en el pclntab: {len(filas)}", flush=True)
    print(f"[decomp] ya decompiladas (Ghidra): {len(hechos)}", flush=True)

    pendientes = []
    for addr, nombre, fichero, linea in filas:
        if addr in hechos:
            continue
        if args.solo_paquete and not fichero.startswith(args.solo_paquete):
            continue
        destino = os.path.join(args.out, fichero + ".c")
        # un .c por funcion -> un directorio por fichero Go original
        destino = os.path.join(args.out, fichero, sanea(nombre) + ".c")
        if os.path.exists(destino) and os.path.getsize(destino) > 40:
            continue
        pendientes.append((addr, nombre, fichero, linea, destino))
    if args.limite:
        pendientes = pendientes[: args.limite]
    if args.prioridad:
        pendientes.sort(key=lambda f: (rango_prioridad(f[2]), f[0]))
    if args.paridad >= 0:
        pendientes = pendientes[args.paridad :: 2]
    print(f"[decomp] pendientes: {len(pendientes)}", flush=True)
    if not pendientes:
        return 0

    os.makedirs(args.out, exist_ok=True)
    indice = os.path.join(args.out, "_index.tsv")
    if not os.path.exists(indice):
        with open(indice, "w", encoding="utf-8") as fh:
            fh.write("addr\tname\tfile\tline\tbytes\tok\terr\tms\n")

    inicio = time.time()
    hechos_esta_ronda = 0
    for base in range(0, len(pendientes), args.chunk):
        lote = pendientes[base : base + args.chunk]
        script = os.path.join(args.out, "_lote.r2")
        with open(script, "w", encoding="utf-8") as fh:
            for addr, nombre, fichero, linea, destino in lote:
                # r2 no crea los directorios: si faltan, la redireccion falla en silencio
                os.makedirs(os.path.dirname(destino), exist_ok=True)
                tam = tamanos[addr]
                fh.write(f"af+ {addr:#x} {sanea(nombre)}; ")
                fh.write(f"afb+ {addr:#x} {addr:#x} {tam:#x}; ")
                fh.write(f"pdg @ {addr:#x} > {destino}\n")
        t0 = time.time()
        proc = subprocess.run(
            [R2, "-q", "-N", "-e", "scr.color=0", "-i", script, args.bin],
            capture_output=True,
            text=True,
            timeout=7200,
        )
        dt = time.time() - t0
        # el .c de salida incluye la firma; se anade la cabecera con el origen Go
        for addr, nombre, fichero, linea, destino in lote:
            if not os.path.exists(destino) or os.path.getsize(destino) < 40:
                continue
            with open(destino, encoding="utf-8", errors="replace") as fh:
                cuerpo = fh.read()
            if not cuerpo.startswith("/*"):
                cabecera = (
                    f"/* {fichero}:{linea} -- pseudocodigo decompilado por r2ghidra "
                    f"(radare2 6.2.6 + r2ghidra 6.2.6) */\n"
                    f"/* go: {nombre} */\n\n"
                )
                with open(destino, "w", encoding="utf-8") as fh:
                    fh.write(cabecera + cuerpo)
            hechos_esta_ronda += 1
        with open(indice, "a", encoding="utf-8") as fh:
            for addr, nombre, fichero, linea, destino in lote:
                ok = 1 if os.path.exists(destino) and os.path.getsize(destino) > 40 else 0
                fh.write(f"{addr:x}\t{sanea(nombre)}\t{fichero}\t{linea}\t{tamanos[addr]}\t{ok}\t\t{int(dt*1000)}\n")
        total = len(hechos) + len(pendientes[: base + len(lote)])
        print(
            f"[decomp] lote {base // args.chunk + 1}: {len(lote)} fns en {dt:.1f}s "
            f"({len(lote)/max(dt,0.001):.1f} fns/s) | total {total}",
            flush=True,
        )
        if proc.returncode != 0:
            print(f"[decomp] r2 termino con rc={proc.returncode}: {proc.stderr[-400:]}", flush=True)
    print(f"[decomp] terminado en {time.time()-inicio:.1f}s, {hechos_esta_ronda} escritos", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())

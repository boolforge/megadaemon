#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""decomp-r2dec.py — barrido de pseudocodigo con r2dec (radare2), el motor
PROPIO de radare2 (NO es Ghidra).

Por que este camino (medido el 2026-11-27):
  * Ghidra (JVM) y r2ghidra son el MISMO decompilador: sobre las 103 funciones
    que fallan en Ghidra, r2ghidra reproduce el error IDENTICO
    ("Cannot properly adjust input varnodes").  Es un limite del decompilador,
    no del arnes.
  * r2dec (`pdc`) es una implementacion distinta: sobre esas mismas 103 produce
    salida para 103 de 103 (96 con control de flujo) y sobre una muestra de
    400 funciones sinteticas, 400 de 400 no vacias a 4,2 fn/s.
  * Sirve de RELEVO donde Ghidra se queda sin pseudocodigo: el hueco real de
    cobertura son las ~99.800 funciones de protobuf generado y wrappers del
    compilador, al 1,11 % y 0,15 %.

El layout de salida es el MISMO que el de los shards de r2ghidra
(`<out>/<fichero_go>/<simbolo>.c`), de modo que `regenerar.py` los recoge sin
cambios (soporta ese layout desde el arreglo del "layout de r2ghidra").

Es REANUDABLE: si el .c ya existe y no esta vacio, se salta. 1 solo worker
(el r2 se lanza una vez por lote).

Uso:
    decomp-r2dec.py --tsv go.funcs.tsv --bin liblanguage_server.so \\
        --out todo/rzD --skip-dir todo/s0 todo/s1 todo/s2 todo/rz0 todo/rzA \\
        --skip-indice todo/s0/_index.tsv ... --chunk 1500 --prioridad
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
from typing import Dict, List, Optional, Set, Tuple

R2 = "/workspace/re-tools/r2.sh"
SLEIGH = "/workspace/.home/.local/share/radare2/plugins/r2ghidra_sleigh"


def cargar_funciones(path_tsv: str) -> List[Tuple[int, str, str, str]]:
    """(addr, nombre, fichero_go, linea) de cada fila del pclntab."""
    filas: List[Tuple[int, str, str, str]] = []
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


def tamanos_por_frontera(filas: List[Tuple[int, str, str, str]], maximo: int = 0x40000) -> Dict[int, int]:
    """Tamano de cada funcion = siguiente VA - VA (acotado)."""
    filas_ord = sorted(filas)
    tamanos: Dict[int, int] = {}
    for i, (addr, _n, _f, _l) in enumerate(filas_ord):
        tam = (filas_ord[i + 1][0] - addr) if i + 1 < len(filas_ord) else maximo
        if tam <= 0 or tam > maximo:
            tam = maximo
        tamanos[addr] = tam
    return tamanos


def sanea(nombre: str) -> str:
    return "".join(c if (c.isalnum() or c in "._-") else "_" for c in nombre)


def ya_hechos(indice: str, base_ghidra: int = 0x100000) -> Set[int]:
    """Direcciones ya decompiladas segun un _index.tsv (ok=1).

    MEDIDO: Ghidra carga este ELF con image base 0x100000, asi que sus
    direcciones son vaddr + 0x100000 (comprobado en 12/12 muestras).  Los
    shards de r2ghidra/r2dec usan la VADDR REAL: para ellos hay que pasar
    base_ghidra=0, si no se descartan direcciones equivocadas (peligroso:
    vaddr-0x100000 puede ser la direccion de OTRA funcion).
    """
    hechos: Set[int] = set()
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


#: lo que YA esta decompilado por otra via, por extension de fichero.
def ya_hechas_por_fichero(dirs: List[str], saneados: Dict[str, str]) -> Set[str]:
    """Nombres (saneados) que ya tienen un .c no vacio en alguno de esos dirs."""
    hechos: Set[str] = set()
    for d in dirs:
        if not os.path.isdir(d):
            continue
        for dp, _dn, fns in os.walk(d):
            for fn in fns:
                if not fn.endswith(".c"):
                    continue
                try:
                    if os.path.getsize(os.path.join(dp, fn)) <= 40:
                        continue
                except OSError:
                    continue
                hechos.add(fn[:-2])
    return hechos


#: lo valioso primero; lo sintetico al final (es mucho y vale menos).
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
)


def rango_prioridad(fichero: str) -> int:
    if fichero.startswith("blaze-out/") or fichero == "<autogenerated>":
        return len(PRIORIDAD) + 1  # sintetico: lo ultimo
    for i, p in enumerate(PRIORIDAD):
        if p in fichero:
            return i
    return len(PRIORIDAD)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tsv", required=True)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--skip-indice", nargs="*", default=[], help="_index.tsv de GHIDRA (dir +0x100000)")
    ap.add_argument("--skip-indice-r2", nargs="*", default=[], help="_index.tsv de r2ghidra/r2dec (VADDR real)")
    ap.add_argument("--skip-dir", nargs="*", default=[], help="dirs de shards ya hechos")
    ap.add_argument("--chunk", type=int, default=1500)
    ap.add_argument("--limite", type=int, default=0, help="0 = todas")
    ap.add_argument("--solo-paquete", default="")
    ap.add_argument("--paridad", type=int, default=-1, help="0 y 1 reparten entre 2 workers; -1 = sin repartir")
    ap.add_argument("--prioridad", action="store_true")
    args = ap.parse_args()

    filas = cargar_funciones(args.tsv)
    tamanos = tamanos_por_frontera(filas)
    hechos: Set[int] = set()
    for idx in args.skip_indice:
        hechos |= ya_hechos(idx, 0x100000)
    for idx in args.skip_indice_r2:
        hechos |= ya_hechos(idx, 0)
    # lo que ya tiene un .c no vacio en otro shard (r2ghidra o Ghidra) NO se
    # repite: el nombre del fichero es el simbolo saneado.
    hechos_nombre = ya_hechas_por_fichero(args.skip_dir, {})
    print("[r2dec] funciones en el pclntab: %d" % len(filas), flush=True)
    print("[r2dec] ya decompiladas (por indice): %d | (por .c existente): %d"
          % (len(hechos), len(hechos_nombre)), flush=True)

    pendientes = []
    for addr, nombre, fichero, linea in filas:
        if addr in hechos:
            continue
        if sanea(nombre) in hechos_nombre:
            continue
        if args.solo_paquete and not fichero.startswith(args.solo_paquete):
            continue
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
    print("[r2dec] pendientes: %d" % len(pendientes), flush=True)
    if not pendientes:
        return 0

    os.makedirs(args.out, exist_ok=True)
    indice = os.path.join(args.out, "_index.tsv")
    if not os.path.exists(indice):
        with open(indice, "w", encoding="utf-8") as fh:
            fh.write("addr\tname\tfile\tline\tbytes\tok\terr\tms\n")

    env = dict(os.environ)
    env["SLEIGHHOME"] = SLEIGH
    inicio = time.time()
    total_ok = 0
    for base in range(0, len(pendientes), args.chunk):
        lote = pendientes[base : base + args.chunk]
        script = os.path.join(args.out, "_lote.r2")
        with open(script, "w", encoding="utf-8") as fh:
            for addr, nombre, fichero, _l, destino in lote:
                tam = tamanos.get(addr, 0x100)
                os.makedirs(os.path.dirname(destino), exist_ok=True)
                fh.write("af+ %s %s; afb+ %s %s 0x%x; pdc @ %s > %s\n"
                         % (addr, sanea(nombre)[:120], addr, addr, tam, addr, destino))
        t0 = time.time()
        subprocess.run(
            [R2, "-q", "-N", "-e", "scr.color=0", "-i", script, args.bin],
            env=env, capture_output=True, text=True, timeout=7200,
        )
        dt = time.time() - t0
        # indice incremental: se comprueba que el .c existe y no esta vacio
        with open(indice, "a", encoding="utf-8") as fh:
            for addr, nombre, fichero, linea, destino in lote:
                ok = 0
                err = ""
                try:
                    if os.path.getsize(destino) > 40:
                        ok = 1
                    else:
                        err = "vacio"
                except OSError:
                    err = "sin fichero"
                fh.write("%x\t%s\t%s\t%s\t%d\t%d\t%s\t%d\n"
                        % (addr, nombre, fichero, linea, tamanos.get(addr, 0), ok, err, int(dt * 1000)))
        n_ok = sum(1 for _a, _n, _f, _l, d in lote
                   if os.path.exists(d) and os.path.getsize(d) > 40)
        total_ok += n_ok
        hechos_esta = base + len(lote)
        print("[r2dec] lote %d-%d: %d/%d no vacios en %.1fs (%.1f fn/s) | total %d"
              % (base, hechos_esta, n_ok, len(lote), dt, len(lote) / max(0.1, dt),
                 total_ok), flush=True)
    print("[r2dec] fin: %d/%d con salida en %.1f s" % (total_ok, len(pendientes), time.time() - inicio),
          flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())

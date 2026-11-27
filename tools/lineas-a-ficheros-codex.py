#!/usr/bin/env python3
"""Cruza la tabla de simbolos de Codex con el mapa de lineas DWARF.

ENTRADAS (medidas 2026-11-27):
  - TSV de simbolos (`tools/extraer-simbolos-codex.py`): addr, bytes, tipo,
    crate, nombre_legible, nombre_mangled. 616.013 filas.
  - `readelf --debug-dump=decodedline` sobre el `.debug`: 45.744.177 lineas con
    (fichero, linea, direccion). 3.067.564.511 B.

SALIDA: `codex-app-server-funciones.tsv` con, por funcion:
  addr, bytes, crate, nombre_legible, fichero_fuente, linea_fuente, dir_fichero
Es el mapa "de donde sale cada funcion" del binario de Codex: la pieza que
falta en el binario stripped.

NOTA DE MEMORIA: el mapa de lineas son 45,7M filas y NO cabe en RAM con el
limite de 2,87 GiB del cgroup (medido: `memory.limit_in_bytes=3087007744`).
Por eso: (1) se extrae a TSV, (2) se ordena con `sort` (externo, en disco),
(3) se hace un merge lineal en streaming.
"""
from __future__ import annotations

import os
import re
import subprocess
import sys

LINEA = re.compile(r"^(?P<file>.*?)\s+(?P<line>\d+)\s+(?P<addr>0x[0-9a-fA-F]+)(?P<rest>\s.*)?$")
TMP = "/tmp/codex-lineas.tsv"


def extraer(decodedline: str, out: str) -> int:
    """decodedline -> TSV (addr_num, fichero, linea). Streaming."""
    n = 0
    with open(decodedline, "r", encoding="utf-8", errors="replace") as fh, \
         open(out, "w", encoding="utf-8") as w:
        for linea in fh:
            # MEDIDO: las lineas de datos de `--debug-dump=decodedline` NO
            # llevan ":" (solo la cabecera de cada unidad); filtrar por ":"
            # las elimina todas (0 entradas).
            if " 0x" not in linea:
                continue
            m = LINEA.match(linea.rstrip("\n"))
            if not m:
                continue
            addr = int(m.group("addr"), 16)
            w.write(f"{addr}\t{m.group('file')}\t{m.group('line')}\n")
            n += 1
    return n


def main() -> int:
    simbolos = sys.argv[1]
    decodedline = sys.argv[2] if len(sys.argv) > 2 else "/tmp/codex-decodedline.txt"
    out = sys.argv[3] if len(sys.argv) > 3 else "/workspace/md-re-productos/artifacts/codex/simbolos/codex-app-server-funciones.tsv"

    if not os.path.exists(TMP + ".sorted"):
        print("[lineas] extrayendo...", flush=True)
        n = extraer(decodedline, TMP)
        print(f"[lineas] {n} entradas -> {TMP}", flush=True)
        print("[lineas] ordenando (sort externo)...", flush=True)
        subprocess.run(["sort", "-t", "\t", "-k1,1n", "-S", "200M", "-T", "/tmp",
                        TMP, "-o", TMP + ".sorted"], check=True)
        print("[lineas] ordenado", flush=True)

    # el fichero de simbolos va ordenado por direccion (nm --numeric-sort)
    filas = []
    with open(simbolos, encoding="utf-8") as fh:
        cabecera = fh.readline()
        for linea in fh:
            c = linea.rstrip("\n").split("\t")
            if len(c) < 6:
                continue
            filas.append((int(c[0], 16), c))
    filas.sort(key=lambda x: x[0])
    print(f"[lineas] {len(filas)} simbolos", flush=True)

    os.makedirs(os.path.dirname(out), exist_ok=True)
    n_con_fichero = n_sin = 0
    with open(TMP + ".sorted", encoding="utf-8") as fh, open(out, "w", encoding="utf-8") as w:
        w.write("addr\tbytes\ttipo\tcrate\tnombre_legible\tfichero_fuente\tlinea_fuente\n")
        linea_actual = fh.readline()
        cur = None  # (addr, fichero, linea)
        if linea_actual:
            c = linea_actual.rstrip("\n").split("\t")
            cur = (int(c[0]), c[1], c[2])

        def avanzar():
            nonlocal linea_actual, cur
            linea_actual = fh.readline()
            if linea_actual:
                c = linea_actual.rstrip("\n").split("\t")
                cur = (int(c[0]), c[1], c[2])
            else:
                cur = None

        for addr, c in filas:
            while cur and cur[0] <= addr:
                avanzar()
            if cur:
                n_con_fichero += 1
                w.write(f"{c[0]}\t{c[1]}\t{c[2]}\t{c[3]}\t{c[4]}\t{cur[1]}\t{cur[2]}\n")
            else:
                n_sin += 1
                w.write(f"{c[0]}\t{c[1]}\t{c[2]}\t{c[3]}\t{c[4]}\t\t\n")
    print(f"[lineas] con fichero={n_con_fichero} sin={n_sin} -> {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

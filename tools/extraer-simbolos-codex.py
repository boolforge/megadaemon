#!/usr/bin/env python3
"""Construye la tabla de simbolos de Codex a partir del `.debug` (DWARF/symtab).

MEDIDO 2026-11-27:
- El binario `codex-app-server-x86_64-unknown-linux-musl` (249.083.328 B) va
  **stripped**: 0 nombres de funcion.
- Su debug file es `codex-app-server.debug` (1.085.115.680 B, CRC32 `ad295a15`),
  que es EXACTAMENTE el CRC que declara el propio binario (leido por Ghidra:
  "ExternalDebugInfo [filename=codex-app-server.debug, crc=ad295a15]").
- Ese `.debug` trae la `symtab` completa: **616.013 simbolos**, de los que
  **356.133 son funciones (T/t)**.
- Los nombres son Rust v0 moderno (`_RNv...`). El demangler canonico es
  `rustc-demangle` 0.1.28 (crate MIT OR Apache-2.0), compilado aqui con
  `rustc` directo (SIN `cargo`, que el root prohibe) desde la cache local de
  cargo. Fallback aproximado: `tools/rust-demangle.py`.

Salidas:
  <out>.tsv        addr, bytes, tipo, crate, ruta legible, nombre mangled
  <out>-crates.tsv  crate, simbolos, funciones
"""
from __future__ import annotations

import collections
import os
import subprocess
import sys

DEMANGLE_BIN = os.environ.get("MD_RUST_DEMANGLE", "/tmp/demang/rustdemangle")
SRC = os.environ.get("MD_CODEX_DEBUG", "/workspace/agy-re/productos/codex/simbolos/codex-app-server.debug")


def main() -> int:
    out = sys.argv[1] if len(sys.argv) > 1 else "/workspace/md-re-productos/artifacts/codex/simbolos/codex-app-server-simbolos.tsv"
    os.makedirs(os.path.dirname(out), exist_ok=True)

    # 1) symtab cruda: "<addr> <size> <tipo> <nombre>"
    #    Si ya existe un volcado de `nm` (MD_NM_TSV), se reutiliza: releer el
    #    `.debug` de 1 GB por NFS cuesta minutos (medido: nm se queda al 0,2 % de
    #    CPU leyendo del NFS tras 6 min).
    nm_tsv = os.environ.get("MD_NM_TSV", "")
    if nm_tsv and os.path.exists(nm_tsv):
        proc = open(nm_tsv, encoding="utf-8", errors="replace")
    else:
        proc = subprocess.Popen(
            ["nm", "--defined-only", "-S", "--numeric-sort", SRC],
            stdout=subprocess.PIPE, text=True,
        )
    dem = subprocess.Popen(
        [DEMANGLE_BIN], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
        bufsize=1,
    )

    crates = collections.Counter()
    crates_func = collections.Counter()
    n_rust = n_no = 0
    n_filas = 0
    out_fh = open(out, "w", encoding="utf-8")
    out_fh.write("addr\tbytes\ttipo\tcrate\tnombre_legible\tnombre_mangled\n")

    # El demangler debe ser STREAMING (linea a linea, con flush). MEDIDO:
    # con un binario que lee todo el stdin (read_to_string) esto se bloquea en
    # deadlock; con el binario streaming (tools/compilar-demangle-rust.sh) el
    # ida y vuelta linea a linea funciona.
    for linea in (proc.stdout if hasattr(proc, 'stdout') else proc):
        campos = linea.rstrip("\n").split(None, 3)
        if len(campos) == 3:
            # `nm` sin tamano: "<addr> <tipo> <nombre>" (no tiene debug de tamano)
            addr, tipo, nombre = campos
            size = "0"
        elif len(campos) >= 4:
            addr, size, tipo, nombre = campos
        else:
            continue
        dem.stdin.write(nombre + "\n")
        dem.stdin.flush()
        legible = dem.stdout.readline().rstrip("\n").split("\t", 1)[-1]
        if not legible:
            legible = nombre
        if nombre.startswith("_R"):
            n_rust += 1
        else:
            n_no += 1
        crate = ""
        if legible != nombre:
            crate = legible.split("::")[0]
            if crate.startswith("<"):
                crate = crate[1:]
            crate = crate.split("[")[0]
        crates[crate] += 1
        if tipo in ("T", "t"):
            crates_func[crate] += 1
        out_fh.write(f"{addr}\t{size}\t{tipo}\t{crate}\t{legible}\t{nombre}\n")
        n_filas += 1

    dem.stdin.close()
    dem.wait()
    try:
        proc.wait()
    except Exception:
        pass
    if hasattr(proc, "close"):
        proc.close()
    out_fh.close()

    with open(out + "-crates.tsv", "w", encoding="utf-8") as fh:
        fh.write("crate\tsimbolos\tfunciones\n")
        for c, n in crates.most_common():
            fh.write(f"{c}\t{n}\t{crates_func[c]}\n")

    print(f"[simbolos] filas={n_filas} rust_v0_moderno={n_rust} otros={n_no}")
    print(f"[simbolos] crates_distintos={len(crates)}")
    print(f"[simbolos] {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

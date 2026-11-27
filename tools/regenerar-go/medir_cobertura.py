#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""medir_cobertura.py — mide la cobertura REAL del codigo Go regenerado.

No estima nada: cuenta lo que hay en disco y lo cruza con el export del
pclntab.  Las tres magnitudes que se informan son:

  1. **funciones del binario**: filas del export del pclntab
     (``go.funcs.tsv``).  Es el total: lo que el binario tiene.
  2. **funciones con pseudocodigo legible**: bloques ``/* ==== ... ==== */``
     en el arbol ``codigo/`` cuyo cuerpo NO es el marcador
     ``/* SIN PSEUDOCODIGO */``; es decir, Ghidra devolvio una
     decompilacion real de esa funcion.
  3. **funciones stub**: funciones del binario que NO tienen pseudocodigo.
     En el modulo Go aparecen como stubs ``panic("pseudocodigo: ver ...")``.

Ademas mide la calidad de las FIRMAS Go: cuantas stubs llevan tipos
reales (parametros y/o retorno) frente a las que solo son ``func X()``.

uso:
  tools/regenerar-go/medir_cobertura.py --out /workspace/entrega/regenerado \
      [--tsv /workspace/agy-re/tools-out/go.funcs.tsv] [--json SALIDA.json]
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

#: cabecera de bloque del pseudocodigo generado por la herramienta
RE_BLOQUE = re.compile(r"^/\* ==== (?P<nombre>.+?)  \[(?P<bytes>\d+) bytes\] ==== \*/$", re.M)
#: marcador de "esta funcion no se pudo decompilar"
RE_SIN = re.compile(r"/\* SIN PSEUDOCODIGO")
#: firma Go de un stub
RE_FUNC = re.compile(r"^func\s+(?P<nombre>[A-Za-z_][\w]*)\s*\((?P<params>[^)]*)\)\s*(?P<ret>[^{]*)\{", re.M)


def contar_pseudocodigo(dir_codigo: Path) -> Dict[str, int]:
    """Cuenta los bloques de pseudocodigo del arbol ``codigo/``."""
    out = {
        "bloques": 0,
        "con_pseudocodigo": 0,
        "sin_pseudocodigo": 0,
        "ficheros_c": 0,
        "bytes_pseudocodigo": 0,
    }
    for dp, _dn, fns in os.walk(dir_codigo):
        for fn in fns:
            if not fn.endswith(".c"):
                continue
            out["ficheros_c"] += 1
            texto = (Path(dp) / fn).read_text(encoding="utf-8", errors="replace")
            marcas = list(RE_BLOQUE.finditer(texto))
            for i, m in enumerate(marcas):
                fin = marcas[i + 1].start() if i + 1 < len(marcas) else len(texto)
                cuerpo = texto[m.end() : fin]
                out["bloques"] += 1
                if RE_SIN.search(cuerpo):
                    out["sin_pseudocodigo"] += 1
                else:
                    out["con_pseudocodigo"] += 1
                    out["bytes_pseudocodigo"] += len(cuerpo)
    return out


def contar_stubs(dir_modulo: Path) -> Dict[str, int]:
    """Cuenta los stubs Go y la calidad de sus firmas."""
    out = {
        "funciones_go": 0,
        "con_parametros": 0,
        "con_retorno": 0,
        "con_tipos_reales": 0,
        "con_unsafe_pointer": 0,
        "ficheros_go": 0,
        "imports_unsafe": 0,
        "imports_unsafe_sin_uso": 0,
    }
    for dp, _dn, fns in os.walk(dir_modulo):
        for fn in fns:
            if not fn.endswith(".go"):
                continue
            out["ficheros_go"] += 1
            texto = (Path(dp) / fn).read_text(encoding="utf-8", errors="replace")
            importa = 'import "unsafe"' in texto
            usa = "unsafe.Pointer" in texto
            if importa:
                out["imports_unsafe"] += 1
                if not usa:
                    out["imports_unsafe_sin_uso"] += 1
            for m in RE_FUNC.finditer(texto):
                out["funciones_go"] += 1
                params = m.group("params").strip()
                ret = m.group("ret").strip()
                if params:
                    out["con_parametros"] += 1
                if ret:
                    out["con_retorno"] += 1
                if params or ret:
                    out["con_tipos_reales"] += 1
                if "unsafe.Pointer" in m.group(0):
                    out["con_unsafe_pointer"] += 1
    return out


def contar_tsv(ruta: Path) -> Dict[str, int]:
    """Cuenta las filas del export del pclntab."""
    total = 0
    sin_fichero = 0
    ficheros: Dict[str, int] = {}
    with open(ruta, encoding="utf-8", errors="replace") as f:
        for linea in f:
            if not linea.strip():
                continue
            partes = linea.rstrip("\n").split("\t")
            if len(partes) < 2:
                continue
            total += 1
            resto = partes[2] if len(partes) > 2 else ""
            if "|" in resto:
                loc = resto.rsplit("|", 1)[1].strip()
            else:
                loc = ""
            fichero = loc.rsplit(":", 1)[0].strip() if ":" in loc else loc
            if not fichero:
                sin_fichero += 1
            ficheros[fichero] = ficheros.get(fichero, 0) + 1
    return {
        "funciones": total,
        "sin_fichero": sin_fichero,
        "ficheros_distintos": len(ficheros),
    }


def main(argv: Optional[Sequence[str]] = None) -> int:
    p = argparse.ArgumentParser(description="Mide la cobertura real del codigo Go regenerado")
    p.add_argument("--out", type=Path, required=True, help="raiz del arbol regenerado")
    p.add_argument(
        "--tsv",
        type=Path,
        default=Path("/workspace/agy-re/tools-out/go.funcs.tsv"),
        help="export del pclntab",
    )
    p.add_argument("--json", type=Path, default=None, help="volcar el resultado a un JSON")
    args = p.parse_args(argv)

    dir_codigo = args.out / "codigo"
    dir_modulo = args.out / "gomodulo"
    if not dir_codigo.is_dir():
        sys.stderr.write("ERROR: no existe %s\n" % dir_codigo)
        return 2

    res: Dict[str, object] = {
        "raiz": str(args.out),
        "tsv": str(args.tsv),
        "pseudocodigo": contar_pseudocodigo(dir_codigo),
        "stubs": contar_stubs(dir_modulo) if dir_modulo.is_dir() else {},
        "pclntab": contar_tsv(args.tsv) if args.tsv.is_file() else {},
    }

    pcln = res["pclntab"]  # type: ignore[assignment]
    pseudo = res["pseudocodigo"]  # type: ignore[assignment]
    stubs = res["stubs"]  # type: ignore[assignment]

    total = int(pcln.get("funciones", 0))  # type: ignore[union-attr]
    con_pseudo = int(pseudo.get("con_pseudocodigo", 0))  # type: ignore[union-attr]
    sin_pseudo = int(pseudo.get("sin_pseudocodigo", 0))  # type: ignore[union-attr]

    res["cobertura"] = {
        "funciones_del_binario": total,
        "con_pseudocodigo_legible": con_pseudo,
        "stub_sin_decompilar": sin_pseudo,
        "pct_pseudocodigo": round(100.0 * con_pseudo / total, 2) if total else 0.0,
        "pct_stub": round(100.0 * sin_pseudo / total, 2) if total else 0.0,
        "bloques_no_cubiertos_por_pclntab": max(
            0, int(pseudo.get("bloques", 0)) - total  # type: ignore[union-attr]
        ),
    }

    txt = json.dumps(res, indent=2, ensure_ascii=False, sort_keys=True)
    print(txt)
    if args.json:
        args.json.write_text(txt + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())

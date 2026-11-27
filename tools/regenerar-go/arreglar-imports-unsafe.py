#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""arreglar-imports-unsafe.py — quita los `import "unsafe"` SIN USAR.

Go rechaza un paquete con ``"unsafe" imported and not used``.  El generador
(``regenerar.py``) ya decide el import POR CONTENIDO, asi que un modulo
generado con la version actual no puede tener este problema.  Esta herramienta
sirve para **reparar un modulo ya generado** (por ejemplo el que hay
publicado en la Drive) sin tener que regenerarlo todo: recorre los `.go`,
comprueba si el fichero usa `unsafe.Pointer` y borra el import sobrante.

Es idempotente y no toca nada mas del fichero.

uso:
  tools/regenerar-go/arreglar-imports-unsafe.py --modulo DIR [--dry-run] [--json SALIDA]
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Dict, Optional, Sequence

IMPORT_UNSAFE = 'import "unsafe"'


def main(argv: Optional[Sequence[str]] = None) -> int:
    p = argparse.ArgumentParser(description="Quita los `import \"unsafe\"` sin usar de un modulo Go")
    p.add_argument("--modulo", type=Path, required=True, help="raiz del modulo Go")
    p.add_argument("--dry-run", action="store_true", help="solo informa, no escribe")
    p.add_argument("--json", type=Path, default=None, help="volcar el resultado a un JSON")
    args = p.parse_args(argv)

    mod = args.modulo
    if not mod.is_dir():
        sys.stderr.write("ERROR: no existe el directorio %s\n" % mod)
        return 2

    res: Dict[str, object] = {
        "modulo": str(mod),
        "dry_run": bool(args.dry_run),
        "ficheros_go": 0,
        "con_import_unsafe": 0,
        "arreglados": 0,
        "ficheros_arreglados": [],
        "bytes_retirados": 0,
    }

    for dp, _dn, fns in os.walk(mod):
        for fn in fns:
            if not fn.endswith(".go"):
                continue
            p_ = Path(dp) / fn
            res["ficheros_go"] = int(res["ficheros_go"]) + 1  # type: ignore[arg-type]
            texto = p_.read_text(encoding="utf-8", errors="replace")
            if IMPORT_UNSAFE not in texto:
                continue
            res["con_import_unsafe"] = int(res["con_import_unsafe"]) + 1  # type: ignore[arg-type]
            if "unsafe.Pointer" in texto:
                continue  # el import SI se usa: no se toca
            if not args.dry_run:
                nuevo = texto.replace(IMPORT_UNSAFE + "\n\n", "", 1)
                if nuevo == texto:
                    nuevo = texto.replace(IMPORT_UNSAFE + "\n", "", 1)
                if nuevo != texto:
                    p_.write_text(nuevo, encoding="utf-8")
            res["arreglados"] = int(res["arreglados"]) + 1  # type: ignore[arg-type]
            res["ficheros_arreglados"].append(str(p_.relative_to(mod)))  # type: ignore[union-attr]
            res["bytes_retirados"] = int(res["bytes_retirados"]) + len(texto) - len(nuevo)  # type: ignore[arg-type]

    txt = json.dumps(res, indent=2, ensure_ascii=False, sort_keys=True)
    print(txt)
    if args.json:
        args.json.write_text(txt + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())

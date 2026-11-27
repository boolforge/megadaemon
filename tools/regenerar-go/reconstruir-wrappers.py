#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""reconstruir-wrappers.py — reconstruye los wrappers de Go desde el fuente.

Las 18 funciones "RECONSTRUIBLE-CON-PISTA" son wrappers que genera el
compilador de Go (``deferwrap``/``gowrap``/closure).  El pclntab da el
**fichero y la linea** del call site, que son reales: si el fuente Go de esa
linea esta disponible (GOROOT o cache de modulos), el cuerpo del wrapper se
puede reconstruir a mano leyendo que hay ahi.

Este script:
  1. mapea la ruta del pclntab a un fichero real del GOROOT o del cache de
     modulos;
  2. extrae la linea y su contexto;
  3. busca la sentencia ``defer``/``go`` que envuelve esa linea;
  4. emite un veredicto por wrapper: RECONSTRUIDO (con la sentencia) o
     SIN-FUENTE (no se puede: el modulo no esta en el cache).

uso:
  tools/regenerar-go/reconstruir-wrappers.py --evidencia TSV --salida SALIDA.md
"""
from __future__ import annotations

import argparse
import csv
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

#: prefijos del pclntab -> raices donde puede estar el fuente
def candidatos(ruta: str, goroot: str, modcache: str) -> List[Path]:
    """Ficheros reales que pueden corresponder a la ruta del pclntab."""
    outs: List[Path] = []
    r = ruta.replace("\\", "/")
    if r.startswith("third_party/go/gc/src/"):
        rel = r[len("third_party/go/gc/src/"):]
        outs.append(Path(goroot) / "src" / rel)
        return outs
    if r.startswith("third_party/golang/"):
        rel = r[len("third_party/golang/"):]
        # github_com/pion/ice/v/v4/ice/x.go -> github.com/pion/ice/v4/ice/x.go
        m = re.match(r"github_com/(.+?)/([^/]+)/v/v(\d+)/(.*)$", rel)
        if m:
            org, proj, ver, rest = m.groups()
            base = Path(modcache) / ("github.com/%s/%s/v%s" % (org, proj, ver))
            outs.append(base / rest)
            outs.append(base / proj / rest)
            return outs
        # golang.org/x/<mod>/...  ->  golang.org/x/<mod>@<v>/...
        m = re.match(r"golang_org/x/([^/]+)/(.*)$", rel)
        if m:
            mod, rest = m.groups()
            gomodcache = Path(modcache) / "golang.org" / "x"
            for d in sorted(gomodcache.glob("%s@*" % mod)):
                outs.append(d / rest)
            return outs
        # go_crypto/ssh/ssh/mac.go -> golang.org/x/crypto/ssh/mac.go
        m = re.match(r"go_crypto/(.*)$", rel)
        if m:
            rest = m.group(1)
            for d in sorted((Path(modcache) / "golang.org" / "x").glob("crypto@*")):
                outs.append(d / rest)
            return outs
        # gitv5/... -> github.com/go-git/go-git/v5/...
        if rel.startswith("gitv5/"):
            for d in sorted((Path(modcache) / "github.com" / "go-git").glob("go-git*")):
                outs.append(d / rel[len("gitv5/"):])
            return outs
        return outs
    return outs


def contexto(path: Path, linea: int, radio: int = 4) -> Tuple[str, str, str]:
    """Devuelve (linea_exacta, contexto_antes, contexto_despues)."""
    try:
        lineas = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return "", "", ""
    if not (1 <= linea <= len(lineas)):
        return "", "", ""
    i = linea - 1
    exacta = lineas[i].strip()
    antes = "\n".join(l.strip() for l in lineas[max(0, i - radio):i])
    despues = "\n".join(l.strip() for l in lineas[i + 1:i + 1 + radio])
    return exacta, antes, despues


def sentencia_defer(antes: str, exacta: str, despues: str) -> str:
    """La sentencia `defer`/`go` que envuelve la linea, si aparece cerca."""
    bloque = (antes + "\n" + exacta + "\n" + despues).splitlines()
    for l in bloque:
        s = l.strip()
        if s.startswith("defer ") or s.startswith("go "):
            return s
    return ""


def main(argv: Optional[Sequence[str]] = None) -> int:
    ap = argparse.ArgumentParser(description="Reconstruye los wrappers de Go desde el fuente")
    ap.add_argument("--evidencia", type=Path, required=True)
    ap.add_argument("--goroot", default="")
    ap.add_argument("--modcache", default="")
    ap.add_argument("--salida", type=Path, required=True)
    ap.add_argument("--json", type=Path, default=None)
    args = ap.parse_args(argv)

    goroot = args.goroot
    modcache = args.modcache
    if not goroot or not modcache:
        env = dict(os.environ)
        env["PATH"] = "/workspace/md-resc/go/go/bin:" + env.get("PATH", "")
        try:
            goroot = goroot or subprocess.run(
                ["go", "env", "GOROOT"], env=env, capture_output=True, text=True
            ).stdout.strip()
            modcache = modcache or subprocess.run(
                ["go", "env", "GOMODCACHE"], env=env, capture_output=True, text=True
            ).stdout.strip()
        except Exception:
            pass

    with open(args.evidencia, encoding="utf-8") as f:
        rows = list(csv.DictReader(f, delimiter="\t"))
    wrappers = [r for r in rows if r["veredicto"] == "RECONSTRUIBLE-CON-PISTA"]

    res: List[Dict[str, object]] = []
    for r in wrappers:
        loc = r["loc"]
        ruta, _, linea_s = loc.rpartition(":")
        try:
            linea = int(linea_s)
        except ValueError:
            linea = 0
        cands = candidatos(ruta, goroot, modcache)
        encontrado = None
        for c in cands:
            if c.is_file():
                encontrado = c
                break
        item = {
            "nombre": r["nombre"],
            "addr": r["addr"],
            "loc": loc,
            "fichero_real": str(encontrado) if encontrado else "",
            "linea_real": linea,
            "linea_texto": "",
            "sentencia": "",
            "veredicto": "SIN-FUENTE",
        }
        if encontrado:
            exacta, antes, despues = contexto(encontrado, linea)
            item["linea_texto"] = exacta
            item["sentencia"] = sentencia_defer(antes, exacta, despues)
            item["veredicto"] = "RECONSTRUIDO" if exacta else "FUENTE-SIN-LINEA"
        res.append(item)

    n_ok = sum(1 for x in res if x["veredicto"] == "RECONSTRUIDO")
    n_defer = sum(1 for x in res if x["sentencia"])
    txt = ["# Reconstruccion de los wrappers reconstruibles con pista", "",
           "Generado por `tools/regenerar-go/reconstruir-wrappers.py`.", "",
           "| funcion | addr | call site (pclntab) | fichero real | linea | sentencia | veredicto |",
           "|---|---|---|---|---|---|---|"]
    for x in res:
        txt.append("| `%s` | `%s` | `%s` | `%s` | %s | `%s` | **%s** |" % (
            x["nombre"], x["addr"], x["loc"],
            x["fichero_real"] or "-", x["linea_real"],
            (x["sentencia"] or x["linea_texto"] or "-").replace("|", "\\|"),
            x["veredicto"]))
    txt += ["", "**Resumen:** %d wrappers, %d con fuente localizada y %d con la sentencia "
                "`defer`/`go` identificada." % (len(res), n_ok, n_defer), ""]
    args.salida.parent.mkdir(parents=True, exist_ok=True)
    args.salida.write_text("\n".join(txt), encoding="utf-8")
    if args.json:
        args.json.write_text(json.dumps(res, indent=2, ensure_ascii=False), encoding="utf-8")
    print("[wrappers] %d wrappers · %d con fuente · %d con sentencia defer/go -> %s"
          % (len(res), n_ok, n_defer, args.salida))
    return 0


if __name__ == "__main__":
    sys.exit(main())

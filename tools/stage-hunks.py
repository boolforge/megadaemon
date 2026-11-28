#!/usr/bin/env python3
"""Stagea hunks concretos de un fichero (un commit = un ámbito).

Uso:
    python3 tools/stage-hunks.py <ruta> [--solo PATRON] [--sin PATRON] [--dry]

Toma el diff de la zona de índice (`git diff` = cambios NO commiteados) y
aplica AL CACHÉ sólo los hunks cuyo texto coincide con el patrón (o no
coincide, con `--sin`).  Los hunks se eligen por CONTENIDO, no por número,
para que el script siga siendo válido tras re-editar el fichero.

Por qué existe: el repo exige "un commit = un ámbito" y "git add <ruta
exacta>"; cuando un mismo fichero toca dos ámbitos (p. ej. el 8080 y la
cuota en `cockpit_server.rs`), esto permite separarlos sin `git add -A`
ni interacción manual.
"""
import argparse
import re
import subprocess
import sys


def partir(patch: str):
    """Divide un diff unificado en (cabecera_de_fichero, [hunks]).

    La cabecera (`diff --git`, `index`, `---`, `+++`) es comun a TODOS los
    hunks: hay que volver a ponerla delante de cada hunk seleccionado, o
    `git apply` responde "patch fragment without header".
    """
    lineas = patch.split("\n")
    cabecera, actual, hunks_ = [], None, []
    for l in lineas:
        if l.startswith("@@"):
            if actual is not None:
                hunks_.append(actual)
            actual = [l]
        elif actual is not None:
            actual.append(l)
        else:
            cabecera.append(l)
    if actual is not None:
        hunks_.append(actual)
    return cabecera, hunks_


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("ruta")
    ap.add_argument("--solo", help="regex: hunks cuyo texto la cumple")
    ap.add_argument("--sin", help="regex: hunks cuyo texto NO la cumple")
    ap.add_argument("--dry", action="store_true")
    args = ap.parse_args()

    patch = subprocess.run(
        ["git", "diff", "--", args.ruta], capture_output=True, text=True, check=True
    ).stdout
    if not patch.strip():
        print(f"{args.ruta}: sin cambios")
        return 0
    cabecera, hs = partir(patch)
    sel = []
    for h in hs:
        texto = "\n".join(h)
        if args.solo and not re.search(args.solo, texto):
            continue
        if args.sin and re.search(args.sin, texto):
            continue
        sel.append("\n".join(h))
    if not sel:
        print(f"{args.ruta}: ningún hunk seleccionado (patrón: {args.solo or args.sin})")
        return 1
    cuerpo = "\n".join(cabecera) + "\n" + "\n".join(sel)
    if not cuerpo.endswith("\n"):
        cuerpo += "\n"
    if args.dry:
        print(cuerpo)
        return 0
    p = subprocess.run(
        ["git", "apply", "--cached", "--unidiff-zero", "-"],
        input=cuerpo,
        text=True,
        capture_output=True,
    )
    if p.returncode != 0:
        print(p.stdout + p.stderr, file=sys.stderr)
        return p.returncode
    print(f"{args.ruta}: {len(sel)} hunk(s) en el índice")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

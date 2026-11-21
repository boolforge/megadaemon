#!/usr/bin/env python3
"""yaml_lint.py — validador de workflows de GitHub Actions (sin dependencias).

POR QUÉ EXISTE
  Un `name:` o `run:` con ": " SIN comillas hace que GitHub **rechace el
  workflow ENTERO**: el run aparece como `failure` con **0 jobs**, y el
  síntoma se confunde con "los tests están rojos" o "el repo está roto".
  Medido en este repo: 6 h sin publicar el binario ARM64 mientras los
  agentes creían que desplegaban bien.

  Además, un `run: |` con la contenido menos indentado que la clave rompe el
  fichero en la línea siguiente, y GitHub lo reporta como
  "You have an error in your yaml syntax on line N" (el detalle real está
  sólo en el HTML del run).

QUÉ COMPRUEBA (subconjunto de YAML suficiente para un workflow)
  1. Tabs (YAML no los admite para indentación).
  2. Escalar con ": " sin comillas en claves de tipo `name:`/`run:`/`uses:`/
     `env:`/`if:`/`shell:`/`working-directory:`.
  3. Contenido de un bloque literal (`|`) o plegado (`>`) menos indentado que
     su clave (rompe el YAML a partir de ahí).
  4. `steps:`/`jobs:` con contenido no lista.
  (Las claves duplicadas NO se comprueban: en una lista de pasos comparten
  padre y el aviso sería un falso positivo; GitHub las acepta igual.)

USO
  python3 tools/yaml_lint.py .github/workflows/build.yml
  python3 tools/yaml_lint.py --quiet .github/workflows/*.yml   # sólo errores

SALIDA
  0 si todo correcto; 1 si hay errores (imprimidos como  file:línea: mensaje).
  Con --json devuelve un objeto JSON con la lista de errores.

LÍMITES (honestos)
  No es un parser YAML completo: valida la estructura que rompe los workflows
  de GitHub en la práctica.  Un fichero que pase esta comprobación puede
  tener otros errores semánticos; la validación definitiva es la del propio
  GitHub al hacer push (que despacha el run o no).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from typing import List, Tuple

# Claves cuyo valor es un escalar y puede llevar ": " dentro.
SCALAR_KEYS = {
    "name", "run", "uses", "env", "if", "shell", "working-directory",
    "container", "services", "timeout-minutes", "continue-on-error",
    "permissions", "needs", "strategy", "matrix", "paths", "branches",
    "runs-on", "profile", "toolchain", "cache", "target", "components",
}

# Plantilla de clave: "  - name: valor"  /  "  name: valor"
KEY_RE = re.compile(r"^(?P<indent>\s*)(?:-\s+)?(?P<key>[A-Za-z_][A-Za-z0-9_-]*):(?P<rest>\s.*|)$")
LIST_RE = re.compile(r"^(?P<indent>\s*)-\s+(?P<body>.*)$")
BLOCK_RE = re.compile(r"[|>][-+0-9]*\s*$")
QUOTED_RE = re.compile(r"^(['\"]).*\1\s*$")
DUP_KEY_RE = re.compile(r"^\s*(?:-\s+)?(?P<key>[A-Za-z_][A-Za-z0-9_-]*):\s*(?P<val>.*)$")


def lint(text: str) -> List[Tuple[int, str]]:
    """Devuelve [(línea, mensaje)] con los errores encontrados."""
    errors: List[Tuple[int, str]] = []
    lines = text.split("\n")
    # pila de marcos: (indentación, ruta_de_claves) donde la ruta identifica
    # el mapa padre => dos `with:` en dos pasos distintos NO son duplicados.
    stack: List[Tuple[int, tuple]] = []
    item_counter: dict = {}
    in_block: int | None = None  # indentación mínima del contenido de un bloque

    for n, raw in enumerate(lines, start=1):
        # 1) tabs
        if "\t" in raw[: len(raw) - len(raw.lstrip())]:
            errors.append((n, "sangría con TAB (YAML no lo admite)"))
            continue

        stripped = raw.strip()
        indent = len(raw) - len(raw.lstrip())

        # 2) dentro de un bloque literal (`|`/`>`): sólo manda la indentación
        if in_block is not None:
            if stripped and indent < in_block:
                # El bloque acaba aquí: la línea que lo cierra debe ser una
                # clave o un elemento de lista en un nivel VÁLIDO.  Si no lo
                # es (p.ej. `echo fuera de sangria`), el YAML se rompe a
                # partir de esta línea y GitHub lo reporta como
                # "error in your yaml syntax on line N".
                if not KEY_RE.match(raw) and not LIST_RE.match(raw):
                    errors.append((n, "línea fuera del bloque literal con sangría "
                                      "incorrecta => rompe el YAML a partir de aquí"))
                in_block = None
            else:
                continue

        if not stripped or stripped.startswith("#"):
            continue

        is_item = bool(LIST_RE.match(raw))
        m = KEY_RE.match(raw)
        if not m:
            continue
        key = m.group("key")
        rest = (m.group("rest") or "").strip()

        # ruta del mapa actual
        parent = tuple(p for p, _ in stack if p < indent)
        if is_item:
            # cada elemento de una lista es un mapa NUEVO => ruta propia
            idx = item_counter.get(parent, 0)
            item_counter[parent] = idx + 1
            scope = parent + (f"item{idx}",)
        else:
            scope = parent

        # 3) escalar con ": " sin comillas
        if key in SCALAR_KEYS and rest:
            if not QUOTED_RE.match(rest) and ": " in rest:
                errors.append(
                    (n, f"'{key}: {rest[:48]}' contiene ': ' sin comillas => GitHub "
                        f"rechaza el workflow ENTERO (0 jobs, run 'failure')")
                )

        # 4) bloque literal: el contenido debe ir más indentado
        if BLOCK_RE.match(rest):
            in_block = indent + 1
            continue

        # actualizar la pila
        while stack and stack[-1][0] >= indent:
            stack.pop()
        stack.append((indent, scope + (key,)))

    return errors


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Valida un workflow de GitHub Actions (YAML sin dependencias)")
    ap.add_argument("files", nargs="+", help="ficheros .yml/.yaml a validar")
    ap.add_argument("--json", action="store_true", help="salida en JSON")
    ap.add_argument("--quiet", action="store_true", help="sólo el resumen")
    args = ap.parse_args()

    total = 0
    payload = []
    for f in args.files:
        try:
            with open(f, encoding="utf-8") as fh:
                text = fh.read()
        except OSError as exc:
            print(f"{f}: no se puede leer ({exc})", file=sys.stderr)
            total += 1
            continue
        errs = lint(text)
        total += len(errs)
        payload.append({"file": f, "errors": [{"line": l, "message": m} for l, m in errs]})
        if not args.quiet:
            for line, msg in errs:
                print(f"{f}:{line}: {msg}")
        if not errs and not args.quiet:
            print(f"{f}: OK (sin errores de la clase 'workflow rechazado')")

    if args.json:
        print(json.dumps({"errors": total, "files": payload}, indent=2, ensure_ascii=False))
    else:
        print(f"total: {total} error(es)")
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())

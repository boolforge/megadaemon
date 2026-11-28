#!/usr/bin/env python3
"""Gate LOCAL: ningún arnés de test puede COPIAR un módulo de la librería.

MEDIDO (2026-11-28, CI runs 46080915315 y 46103456920): `main` estaba en
rojo desde el 2026-11-21 por esto:

    error[E0433]: cannot find `apk_inventory` in the crate root
      --> src/app_intel.rs:2738

`tests/root_hide_app_intel.rs` copiaba `src/app_intel.rs` con
`#[path = "../src/app_intel.rs"]` porque, en aquel momento, `src/lib.rs`
NO declaraba ese módulo.  Dentro de un arnés, `crate::` es la raíz del
ARNÉS, no de la librería ⇒ un módulo copiado que use `crate::algo` no
resuelve y **el target de test no compila** ⇒ `cargo test` muere con
rc=101 sin ejecutar ni un test.

El propio repo tiene el guard (`tests/declared_modules.rs`), pero **no
podía ejecutarse**: el target que lo contiene era justo el que no
compilaba.  Este script hace la misma comprobación en **SEGUNDOS**, sin
compilar, para que el error se vea antes de gastar 15-20 min de CI.

Regla: un arnés (fichero de `rust-core/tests/`) NO puede incluir con
`#[path = "../src/x.rs"]` un módulo que use `crate::`.  La forma correcta
es `use megadaemon_core::<modulo>;` (el módulo vive en la librería y sus
tests se ejecutan en la suite de la librería).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
TESTS = RAIZ / "rust-core" / "tests"
SRC = RAIZ / "rust-core" / "src"

PRE = '#[path = "../src/'
# Sólo cuenta las líneas de CÓDIGO (un comentario que explica el patrón
# antiguo no es un include).
CODIGO = re.compile(r'^\s*(#\[|mod |pub mod |use )')


def main() -> int:
    if not TESTS.is_dir():
        print(f"::notice::{TESTS} no existe: gate omitido")
        return 0
    fallos: list[str] = []
    incluidos = 0
    for f in sorted(TESTS.glob("*.rs")):
        try:
            txt = f.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for num, linea in enumerate(txt.splitlines(), 1):
            l = linea.strip()
            if l.startswith("//"):
                continue
            i = l.find(PRE)
            if i < 0:
                continue
            incluidos += 1
            resto = l[i + len(PRE):]
            j = resto.find('"')
            if j < 0:
                continue
            rel = resto[:j]
            rel = rel[:-3] if rel.endswith(".rs") else rel
            mod = SRC / f"{rel}.rs"
            if not mod.is_file():
                fallos.append(
                    f"{f.relative_to(RAIZ)}:{num} incluye {rel}.rs, que NO EXISTE"
                )
                continue
            cuerpo = mod.read_text(encoding="utf-8", errors="replace")
            if "crate::" in cuerpo:
                fallos.append(
                    f"{f.relative_to(RAIZ)}:{num} copia src/{rel}.rs y ese módulo "
                    f"usa `crate::` (no compila en un arnés): usa "
                    f"`use megadaemon_core::{rel};`"
                )
    print(f"::notice::arnes revisados: {incluidos} include(s) de `src/`")
    if fallos:
        print("::error::arnes que copian módulos de la librería:")
        for x in fallos:
            print(f"  - {x}")
        return 1
    print("arnes OK: ninguno copia un módulo de la librería")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

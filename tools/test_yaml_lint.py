#!/usr/bin/env python3
"""Tests de tools/yaml_lint.py (sin dependencias: `python3 tools/test_yaml_lint.py`).

Cubren la clase de fallo que TUMBÓ el CI de este repo: un escalar con
": " sin comillas hace que GitHub **rechace el workflow entero** (run
"failure" con 0 jobs), y el síntoma se confunde con "tests rojos".
"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from yaml_lint import lint  # noqa: E402


MALO = """name: megadaemon-build
on:
  push:
    branches: [main]
jobs:
  quality:
    steps:
      - name: Gate (BLOQUEA): autenticacion y politica del harness
        run: |
          cargo test --lib
"""

BUENO = """name: megadaemon-build
on:
  push:
    branches: [main]
jobs:
  quality:
    steps:
      - name: "Gate (BLOQUEA): autenticacion y politica del harness"
        run: |
          cargo test --lib
      - name: "Verificar assets (el ARM64 es OBLIGATORIO)"
        run: |
          test -f dist/megadaemon-core-android-arm64
"""

CON_TABS = "jobs:\n\tquality:\n\t\truns-on: ubuntu-latest\n"

BLOQUE_MAL_INDENTADO = """jobs:
  build:
    steps:
      - name: Compilar
        run: |
          cargo build
       echo fuera de sangria
"""

# Un comentario dedentado ENTRE dos pasos: los parsers de YAML lo ignoran,
# así que cierra el bloque literal sin romper el workflow (GitHub lo acepta).
COMENTARIO_ENTRE_PASOS = """jobs:
  build:
    steps:
      - name: uno
        run: |
          echo hola
      # comentario dedentado entre dos pasos
      - name: dos
        run: echo z
"""


class TestYamlLint(unittest.TestCase):
    def test_detecta_escalar_con_dos_puntos_sin_comillas(self):
        errs = lint(MALO)
        self.assertTrue(errs, "debería detectar el ': ' sin comillas")
        self.assertIn("sin comillas", errs[0][1])

    def test_no_falsa_positiva_en_workflow_correcto(self):
        self.assertEqual(lint(BUENO), [], "un workflow correcto no debe dar errores")

    def test_detecta_tabs(self):
        errs = lint(CON_TABS)
        self.assertTrue(any("TAB" in m for _, m in errs))

    def test_detecta_bloque_mal_indentado(self):
        errs = lint(BLOQUE_MAL_INDENTADO)
        self.assertTrue(errs, "el contenido de un bloque literal debe ir más indentado")

    def test_un_comentario_no_rompe_el_bloque_literal(self):
        self.assertEqual(lint(COMENTARIO_ENTRE_PASOS), [],
                         "un comentario dedentado es legal: los parsers lo ignoran")

    def test_fichero_real_del_repo_pasa(self):
        ruta = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                            "..", ".github", "workflows", "build.yml")
        if not os.path.isfile(ruta):
            self.skipTest("el workflow no está en este checkout")
        with open(ruta, encoding="utf-8") as fh:
            self.assertEqual(lint(fh.read()), [],
                             "el workflow del repo debe pasar el linter")


if __name__ == "__main__":
    unittest.main(verbosity=2)

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Test de `tools/regenerar-go/regenerar.py`.

Sin dependencias externas (solo la stdlib de Python 3).  Comprueba con un TSV
de 5 funciones de ejemplo que:

  1. el agrupado por paquete Go es el correcto (directorio del fichero
     original) y dentro del paquete se ordena por fichero y por linea;
  2. el nombre Go se sanea a un identificador valido
     (``internal_abi.__RegArgs_.Dump`` -> ``Internal_abi_RegArgs_Dump``);
  3. la firma se extrae del pseudocodigo de Ghidra y se traduce a Go;
  4. el modulo Go generado tiene sintaxis correcta: se valida con
     ``gofmt``/``go vet``/``go build`` si hay un toolchain de Go en el PATH
     (o en ``$GO_BIN``); si NO hay, se valida con el parser propio de este
     fichero (``valida_go``) y el test lo dice explicitamente;
  5. la herramienta es idempotente;
  6. las colisiones de identificador se resuelven con sufijo.

uso:  python3 tools/regenerar-go/test_regenerar.py
"""
from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parent
sys.path.insert(0, str(RAIZ))

import regenerar  # noqa: E402  (el modulo que hay que probar)


# ---------------------------------------------------------------------------
# fixtures
# ---------------------------------------------------------------------------

#: 5 funciones reales del export del pclntab de liblanguage_server.so.
TSV_EJEMPLO = """0x4d06cb8\tinternal_abi.__RegArgs_.Dump\tgo: internal/abi.(*RegArgs).Dump | third_party/go/gc/src/internal/abi/abi.go:47
0x4d06ea8\tinternal_abi.__RegArgs_.IntRegArgAddr\tgo: internal/abi.(*RegArgs).IntRegArgAddr | third_party/go/gc/src/internal/abi/abi.go:74
0x4d06f38\tinternal_abi.__IntArgRegBitmap_.Set\tgo: internal/abi.(*IntArgRegBitmap).Set | third_party/go/gc/src/internal/abi/abi.go:90
0x7159e28\tgoogle3_third_party_jetski_language_server_language_server.CreateLanguageServerAndServe.gowrap6\tgo: google3/third_party/jetski/language_server/language_server.CreateLanguageServerAndServe.gowrap6 | third_party/jetski/language_server/server.go:2544
0x100\tmain.main\tgo: main.main | main.go:12
"""

#: Los mismos 5 simbolos, pero con las firmas que emite Ghidra.
INDICE_EJEMPLO = """addr\tname\tfile\tline\tbytes\tok\terr
4d16cb8\tinternal_abi.__RegArgs_.Dump\tthird_party/go/gc/src/internal/abi/abi.go\t47\t88\t1\t
4d16ea8\tinternal_abi.__RegArgs_.IntRegArgAddr\tthird_party/go/gc/src/internal/abi/abi.go\t74\t60\t1\t
4d16f38\tinternal_abi.__IntArgRegBitmap_.Set\tthird_party/go/gc/src/internal/abi/abi.go\t90\t44\t1\t
7159e28\tgoogle3_third_party_jetski_language_server_language_server.CreateLanguageServerAndServe.gowrap6\tthird_party/jetski/language_server/server.go\t2544\t364\t1\t
"""

PSEUDO_ABI = """/* third_party/go/gc/src/internal/abi/abi.go -- pseudocodigo decompilado por Ghidra (MegaDaemon) */

/* ==== internal_abi.__RegArgs_.Dump  [88 bytes] ==== */
void internal_abi.__RegArgs_.Dump(undefined8 param_1);

/* go: internal/abi.(*RegArgs).Dump
   third_party/go/gc/src/internal/abi/abi.go:47 */

void internal_abi___RegArgs__Dump(undefined8 param_1)

{
  FUN_04d06cb8(param_1);
}

/* ==== internal_abi.__RegArgs_.IntRegArgAddr  [60 bytes] ==== */
undefined8 internal_abi.__RegArgs_.IntRegArgAddr(undefined8 param_1,long param_2);

/* go: internal/abi.(*RegArgs).IntRegArgAddr
   third_party/go/gc/src/internal/abi/abi.go:74 */

undefined8 internal_abi___RegArgs__IntRegArgAddr(undefined8 param_1,long param_2)

{
  return 0;
}

/* ==== internal_abi.__IntArgRegBitmap_.Set  [44 bytes] ==== */
void internal_abi.__IntArgRegBitmap_.Set(undefined8 param_1,undefined1 *param_2,byte param_3);

/* go: internal/abi.(*IntArgRegBitmap).Set
   third_party/go/gc/src/internal/abi/abi.go:90 */

void internal_abi___IntArgRegBitmap__Set(undefined8 param_1,undefined1 *param_2,byte param_3)

{
  *param_2 = param_3;
}
"""

PSEUDO_SERVER = """/* third_party/jetski/language_server/server.go -- pseudocodigo decompilado por Ghidra (MegaDaemon) */

/* ==== google3_third_party_jetski_language_server_language_server.CreateLanguageServerAndServe.gowrap6  [364 bytes] ==== */
void google3_third_party_jetski_language_server_language_server.CreateLanguageServerAndServe.gowrap6(undefined8 param_1,undefined param_2);

/* go: google3/third_party/jetski/language_server/language_server.CreateLanguageServerAndServe.gowrap6
   third_party/jetski/language_server/server.go:2544 */

void gowrap6(undefined8 param_1,undefined param_2)

{
  FUN_07159e28(param_1,param_2);
}
"""


def escribir_fixture(dir_base: Path, tsv: str = TSV_EJEMPLO, indice: str = INDICE_EJEMPLO) -> Path:
    """Crea el TSV + el arbol de pseudocodigo de ejemplo.  Devuelve la raiz."""
    dir_base.mkdir(parents=True, exist_ok=True)
    (dir_base / "funcs.tsv").write_text(tsv, encoding="utf-8")
    pseudo = dir_base / "pseudo"
    pseudo.mkdir(parents=True, exist_ok=True)
    (pseudo / "_index.tsv").write_text(indice, encoding="utf-8")
    d1 = pseudo / "third_party/go/gc/src/internal/abi"
    d1.mkdir(parents=True, exist_ok=True)
    (d1 / "abi.c").write_text(PSEUDO_ABI, encoding="utf-8")
    d2 = pseudo / "third_party/jetski/language_server"
    d2.mkdir(parents=True, exist_ok=True)
    (d2 / "server.c").write_text(PSEUDO_SERVER, encoding="utf-8")
    return dir_base


def ejecutar(base: Path, out: Path, tsv: Path = None, indice: str = "ruta") -> int:
    """Ejecuta la herramienta sobre la fixture."""
    o = regenerar.Opciones()
    o.tsv = tsv or (base / "funcs.tsv")
    o.pseudo = base / "pseudo"
    o.out = out
    o.chunk = 1000
    o.indice = indice
    return regenerar.run(o)


# ---------------------------------------------------------------------------
# parser Go propio (usado cuando NO hay toolchain de Go en el PATH)
# ---------------------------------------------------------------------------

_TOKENES = re.compile(
    r"""
    (?P<comentario_linea>//[^\n]*)
  | (?P<comentario_bloque>/\*.*?\*/)
  | (?P<cadena>"(?:\\.|[^"\\])*")
  | (?P<caracter>'\\?[^'\\]')
  | (?P<ident>[A-Za-z_][A-Za-z0-9_]*)
  | (?P<numero>[0-9]+(?:\.[0-9]+)?)
  | (?P<punt>[()\[\]{}.,;:])
  | (?P<op>[+\-*/%<>=!&|^~])
  | (?P<espacio>\s+)
    """,
    re.VERBOSE | re.DOTALL,
)


class ErrorGo(Exception):
    pass


def _tokens(texto: str):
    pos = 0
    n = len(texto)
    while pos < n:
        m = _TOKENES.match(texto, pos)
        if not m:
            raise ErrorGo("caracter inesperado en la posicion %d: %r" % (pos, texto[pos : pos + 20]))
        pos = m.end()
        tipo = m.lastgroup
        if tipo in ("comentario_linea", "comentario_bloque", "espacio"):
            continue
        yield tipo, m.group()


class ParserGo:
    """Parser minimo del subconjunto de Go que genera la herramienta.

    Valida: clausula `package`, imports y declaraciones `func` con su
    cuerpo.  Es suficiente para detectar un fichero Go sintacticamente
    invalido (que es lo que hay que comprobar cuando no hay `gofmt`).
    """

    def __init__(self, texto: str) -> None:
        self.toks = list(_tokens(texto))
        self.i = 0
        self._esperado = "un token" 

    def _peek(self):
        return self.toks[self.i] if self.i < len(self.toks) else (None, None)

    def _next(self):
        if self.i >= len(self.toks):
            raise ErrorGo("fin de fichero inesperado (falta %s)" % self._esperado)
        t = self._peek()
        self.i += 1
        return t

    def _expect(self, valor: str):
        tipo, txt = self._next()
        if txt != valor:
            raise ErrorGo("se esperaba %r y se encontro %r" % (valor, txt))

    def _ident(self) -> str:
        tipo, txt = self._next()
        if tipo != "ident":
            raise ErrorGo("se esperaba un identificador y se encontro %r" % (txt,))
        return txt

    def parsear(self) -> dict:
        info = {"paquete": "", "funciones": [], "imports": []}
        # package
        tipo, txt = self._peek()
        if txt != "package":
            raise ErrorGo("el fichero no empieza por `package`")
        self._next()
        info["paquete"] = self._ident()
        # imports
        while self._peek()[1] == "import":
            self._next()
            if self._peek()[1] == "(":
                self._next()
                while self._peek()[1] != ")":
                    if self._peek()[0] == "ident" or self._peek()[1] in ("_", "."):
                        self._next()
                    tipo_p, txt_p = self._next()
                    if tipo_p != "cadena":
                        raise ErrorGo("import invalido: %r" % (txt_p,))
                    info["imports"].append(txt_p)
                self._expect(")")
            else:
                # `import "ruta"` o `import nombre "ruta"`
                tipo_p, txt_p = self._next()
                if tipo_p == "cadena":
                    info["imports"].append(txt_p)
                else:
                    tipo_p2, txt_p2 = self._next()
                    if tipo_p2 != "cadena":
                        raise ErrorGo("import invalido: %r" % (txt_p2,))
                    info["imports"].append(txt_p2)
        # declaraciones
        while self.i < len(self.toks):
            tipo, txt = self._peek()
            if txt == "func":
                self._next()
                nombre = self._ident()
                self._expect("(")
                profundidad = 1
                while profundidad:
                    t2, x2 = self._next()
                    if x2 == "(":
                        profundidad += 1
                    elif x2 == ")":
                        profundidad -= 1
                # resultados
                while self._peek()[1] not in ("{", None):
                    self._next()
                self._expect("{")
                profundidad = 1
                while profundidad:
                    t2, x2 = self._next()
                    if x2 == "{":
                        profundidad += 1
                    elif x2 == "}":
                        profundidad -= 1
                info["funciones"].append(nombre)
            elif txt in ("var", "const", "type"):
                # declaracion simple: se come hasta la siguiente declaracion
                # de nivel superior (o el fin del fichero).
                self._next()
                while self._peek()[1] not in ("func", "var", "const", "type", None):
                    self._next()
            else:
                raise ErrorGo("declaracion inesperada: %r" % (txt,))
        return info


def valida_go(ruta: Path) -> dict:
    """Valida un fichero Go con el parser propio.  Devuelve el resumen."""
    info = ParserGo(ruta.read_text(encoding="utf-8")).parsear()
    if not info["paquete"]:
        raise ErrorGo("%s: falta la clausula package" % ruta)
    return info


def toolchain_go() -> dict:
    """Localiza un toolchain de Go.  ``{"go":..., "gofmt":..., "root":...}`` o {}."""
    raiz = os.environ.get("GO_BIN") or os.environ.get("GOROOT")
    candidatos = []
    if raiz:
        candidatos.append(str(Path(raiz) / "bin" / "go"))
        candidatos.append(raiz)
    g = shutil.which("go")
    if g:
        candidatos.append(g)
    go = next((c for c in candidatos if c and Path(c).is_file()), None)
    if go is None:
        return {}
    gofmt = shutil.which("gofmt")
    if not gofmt:
        cand = Path(go).parent / "gofmt"
        gofmt = str(cand) if cand.is_file() else None
    if not gofmt:
        return {}
    env = dict(os.environ)
    env.setdefault("GOROOT", str(Path(go).parent.parent))
    env["PATH"] = str(Path(go).parent) + os.pathsep + env.get("PATH", "")
    return {"go": go, "gofmt": gofmt, "env": env}


# ---------------------------------------------------------------------------
# tests
# ---------------------------------------------------------------------------


class TestParserTSV(unittest.TestCase):
    """El parser del TSV debe ser el mismo que ``gobook.rs::parsear_linea``."""

    def test_parsea_una_linea_real(self):
        f = regenerar.parsear_linea(
            "0x4d06cb8\tinternal_abi.__RegArgs_.Dump\t"
            "go: internal/abi.(*RegArgs).Dump | third_party/go/gc/src/internal/abi/abi.go:47"
        )
        self.assertIsNotNone(f)
        self.assertEqual(f.addr, 0x4d06cb8)
        self.assertEqual(f.paquete_go, "internal/abi")
        self.assertEqual(f.nombre_go, "(*RegArgs).Dump")
        self.assertEqual(f.fichero, "third_party/go/gc/src/internal/abi/abi.go")
        self.assertEqual(f.linea, 47)
        self.assertEqual(f.paquete_dir, "third_party/go/gc/src/internal/abi")

    def test_ignora_lineas_robas(self):
        self.assertIsNone(regenerar.parsear_linea(""))
        self.assertIsNone(regenerar.parsear_linea("no-es-un-tsv"))


class TestNombresGo(unittest.TestCase):
    def test_traduce_el_nombre_del_enunciado(self):
        self.assertEqual(
            regenerar.ident_go_desde_simbolo("internal_abi.__RegArgs_.Dump"),
            "Internal_abi_RegArgs_Dump",
        )

    def test_otros_casos(self):
        self.assertEqual(regenerar.ident_go_desde_simbolo("runtime.main"), "Runtime_main")
        self.assertEqual(regenerar.ident_go_desde_simbolo("main.main.func1"), "Main_main_func1")
        self.assertEqual(
            regenerar.ident_go_desde_simbolo(
                "google3_third_party_jetski_gateway_gateway.ConfigureDefaultTransport"
            ),
            "Google3_third_party_jetski_gateway_gateway_ConfigureDefaultTransport",
        )

    def test_siempre_es_identificador_valido(self):
        for simbolo in ("a.b", "x..y", "9x.y", "go.func", "a-b.c*d", ""):
            ident = regenerar.ident_go_desde_simbolo(simbolo)
            self.assertRegex(ident, r"^[A-Za-z_][A-Za-z0-9_]*$", simbolo)
            self.assertNotIn(ident, regenerar.GO_PALABRAS_CLAVE)


class TestFirmas(unittest.TestCase):
    def test_extrae_firma_de_ghidra(self):
        ret, params = regenerar.parsear_prototipo(
            "void internal_abi.__RegArgs_.Dump(undefined8 param_1);"
        )
        self.assertEqual(ret, "void")
        self.assertEqual(params, [("undefined8", "param_1")])

    def test_traduce_a_go(self):
        ret, params = regenerar.parsear_prototipo(
            "undefined8 internal_abi.__RegArgs_.IntRegArgAddr(undefined8 param_1,long param_2);"
        )
        p, r, unsafe = regenerar.firma_go(ret, params)
        self.assertEqual(p, "param_1 uint64, param_2 int64")
        self.assertEqual(r, " uint64")
        self.assertFalse(unsafe)

    def test_puntero_usa_unsafe(self):
        ret, params = regenerar.parsear_prototipo(
            "void f(undefined8 param_1,undefined1 *param_2,byte param_3);"
        )
        p, r, unsafe = regenerar.firma_go(ret, params)
        self.assertEqual(p, "param_1 uint64, param_2 unsafe.Pointer, param_3 byte")
        self.assertEqual(r, "")
        self.assertTrue(unsafe)

    def test_sin_retorno_no_devuelve_nada(self):
        ret, params = regenerar.parsear_prototipo("void f(void);")
        p, r, _ = regenerar.firma_go(ret, params)
        self.assertEqual(p, "")
        self.assertEqual(r, "")


class TestHerramienta(unittest.TestCase):
    """Prueba de extremo a extremo sobre la fixture de 5 funciones."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = Path(tempfile.mkdtemp(prefix="test-regenerar-"))
        cls.base = escribir_fixture(cls.tmp / "base")
        cls.out = cls.tmp / "out"
        rc = ejecutar(cls.base, cls.out)
        assert rc == 0, "la herramienta devolvio %d" % rc

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.tmp, ignore_errors=True)

    # ---------------- agrupado por paquete ----------------
    def test_agrupa_por_paquete(self):
        # 5 funciones -> 3 paquetes: el directorio del fichero original.
        self.assertTrue((self.out / "codigo/third_party/go/gc/src/internal/abi/README.md").is_file())
        self.assertTrue((self.out / "codigo/third_party/jetski/language_server/README.md").is_file())
        self.assertTrue((self.out / "codigo/README.md").is_file())

    def test_el_indice_del_paquete_lleva_nombre_linea_direccion_y_tamano(self):
        readme = (self.out / "codigo/third_party/go/gc/src/internal/abi/README.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("`internal_abi.__RegArgs_.Dump`", readme)
        self.assertIn("| 47 |", readme)
        self.assertIn("`0x4d06cb8`", readme)
        self.assertIn("| 88 |", readme)

    def test_ordena_por_linea_original(self):
        readme = (self.out / "codigo/third_party/go/gc/src/internal/abi/README.md").read_text(
            encoding="utf-8"
        )
        i47 = readme.index("| 47 |")
        i74 = readme.index("| 74 |")
        i90 = readme.index("| 90 |")
        self.assertLess(i47, i74)
        self.assertLess(i74, i90)

    def test_un_fichero_por_fichero_go_original(self):
        self.assertTrue((self.out / "codigo/third_party/go/gc/src/internal/abi/abi.c").is_file())
        self.assertTrue((self.out / "codigo/third_party/jetski/language_server/server.c").is_file())

    def test_el_pseudocodigo_lleva_asociada_la_linea_original(self):
        c = (self.out / "codigo/third_party/go/gc/src/internal/abi/abi.c").read_text(encoding="utf-8")
        self.assertIn("/* linea original : 47 */", c)
        self.assertIn("/* direccion       : 0x4d06cb8 */", c)
        self.assertIn("internal_abi___RegArgs__Dump", c)  # el cuerpo de Ghidra

    # ---------------- stubs Go ----------------

    def test_el_stub_go_tiene_el_nombre_saneado_y_el_panic(self):
        go = (self.out / "gomodulo/third_party/go/gc/src/internal/abi/abi.go").read_text(
            encoding="utf-8"
        )
        self.assertIn("func Internal_abi_RegArgs_Dump(param_1 uint64) {", go)
        self.assertIn(
            'panic("pseudocodigo: ver third_party/go/gc/src/internal/abi/abi.go")', go
        )
        self.assertIn("package abi", go)

    def test_la_firma_go_tiene_los_tipos_del_pseudocodigo(self):
        go = (self.out / "gomodulo/third_party/go/gc/src/internal/abi/abi.go").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "func Internal_abi_RegArgs_IntRegArgAddr(param_1 uint64, param_2 int64) uint64 {", go
        )
        self.assertIn(
            "func Internal_abi_IntArgRegBitmap_Set(param_1 uint64, param_2 unsafe.Pointer, "
            "param_3 byte) {",
            go,
        )

    def test_gomodulo_tiene_go_mod(self):
        mod = (self.out / "gomodulo/go.mod").read_text(encoding="utf-8")
        self.assertIn("module ", mod)
        self.assertIn("go 1.", mod)

    # ---------------- import "unsafe" ----------------

    def test_el_import_unsafe_solo_va_donde_hace_falta(self):
        """REGRESION: `import "unsafe"` solo si el fichero lo usa.

        El bug original decidia el import mirando si ALGUN prototipo C del
        fichero tenia un `*`, pero los stubs Go no usaban ese tipo: el
        modulo emitia `import "unsafe"` en ficheros que no lo usaban y Go
        rechazaba el paquete con "unsafe imported and not used".
        """
        abi = (
            self.out / "gomodulo/third_party/go/gc/src/internal/abi/abi.go"
        ).read_text(encoding="utf-8")
        # este fichero SI usa unsafe.Pointer (param_2) => necesita el import
        self.assertIn("unsafe.Pointer", abi)
        self.assertIn('import "unsafe"', abi)

        srv = (
            self.out / "gomodulo/third_party/jetski/language_server/server.go"
        ).read_text(encoding="utf-8")
        # este fichero NO lo usa => no debe llevar el import
        self.assertNotIn("unsafe.Pointer", srv)
        self.assertNotIn('import "unsafe"', srv)

    def test_ningun_fichero_del_modulo_tiene_imports_sin_usar(self):
        """Invariante global del modulo: import <-> uso."""
        mod = self.out / "gomodulo"
        for go in sorted(mod.rglob("*.go")):
            texto = go.read_text(encoding="utf-8")
            usa = "unsafe.Pointer" in texto
            importa = 'import "unsafe"' in texto
            self.assertEqual(
                usa,
                importa,
                "%s: importa=%s usa=%s (import sin usar rompe `go build`)" % (go, importa, usa),
            )

    # ---------------- sintaxis del modulo Go ----------------

    def test_el_modulo_go_es_sintacticamente_valido(self):
        tc = toolchain_go()
        if tc:
            # Con toolchain: gofmt debe dejar los ficheros SIN cambios.
            proc = subprocess.run(
                [tc["gofmt"], "-l", "."],
                cwd=str(self.out / "gomodulo"),
                env=tc["env"],
                capture_output=True,
                text=True,
            )
            self.assertEqual(
                proc.stdout.strip(),
                "",
                "gofmt -l dice que estos ficheros no estan formateados:\n%s" % proc.stdout,
            )
            # Y el modulo debe compilar y pasar vet.
            proc = subprocess.run(
                [tc["go"], "build", "./..."],
                cwd=str(self.out / "gomodulo"),
                env=tc["env"],
                capture_output=True,
                text=True,
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            proc = subprocess.run(
                [tc["go"], "vet", "./..."],
                cwd=str(self.out / "gomodulo"),
                env=tc["env"],
                capture_output=True,
                text=True,
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        else:
            # Sin toolchain: parser propio.  El test lo dice explicitamente.
            sys.stderr.write(
                "\n[AVISO] NO hay toolchain de Go en el PATH: se valida el modulo "
                "con el parser propio de este test (valida_go).\n"
            )
            for go in sorted((self.out / "gomodulo").rglob("*.go")):
                valida_go(go)

    def test_el_parser_propio_detecta_go_invalido(self):
        malo = self.tmp / "malo.go"
        malo.write_text("package p\nfunc Rompe( {\n", encoding="utf-8")
        with self.assertRaises(ErrorGo):
            valida_go(malo)

    # ---------------- idempotencia ----------------

    def test_es_idempotente(self):
        """Dos ejecuciones con la misma entrada dan el mismo byte a byte."""
        def instantanea():
            return {
                str(p.relative_to(self.out)): p.read_bytes()
                for p in sorted(self.out.rglob("*"))
                if p.is_file()
            }

        antes = instantanea()
        rc = ejecutar(self.base, self.out)  # segunda vez, MISMO directorio
        self.assertEqual(rc, 0)
        despues = instantanea()
        self.assertEqual(sorted(antes), sorted(despues), "el arbol de ficheros cambia")
        for rel in antes:
            self.assertEqual(
                antes[rel], despues[rel], "el contenido de %s cambia entre ejecuciones" % rel
            )

    # ---------------- resumen ----------------

    def test_el_resumen_tiene_las_cifras_medidas(self):
        resumen = (self.out / "RESUMEN.md").read_text(encoding="utf-8")
        self.assertIn("funciones en el export del pclntab | 5 |", resumen)
        self.assertIn("paquetes (directorios del binario) | 3 |", resumen)
        self.assertIn("ficheros `.c` generados | 3 |", resumen)
        self.assertIn("NO-DECOMPIADAS.tsv", resumen)

    def test_las_funciones_sin_pseudocodigo_se_reportan(self):
        # main.main no esta en el indice de Ghidra de la fixture.
        no_decomp = (self.out / "NO-DECOMPIADAS.tsv").read_text(encoding="utf-8")
        self.assertIn("main.main", no_decomp)
        resumen = (self.out / "RESUMEN.md").read_text(encoding="utf-8")
        self.assertIn("main.main", resumen)


class TestColisiones(unittest.TestCase):
    def test_las_colisiones_de_identificador_se_resuelven_con_sufijo(self):
        tmp = Path(tempfile.mkdtemp(prefix="test-colisiones-"))
        try:
            tsv = (
                "0x10\tp.a.b\tgo: p.a.b | p/x.go:10\n"
                "0x20\tp.a_b\tgo: p.a_b | p/x.go:20\n"
            )
            base = escribir_fixture(tmp / "base", tsv=tsv, indice="")
            out = tmp / "out"
            rc = ejecutar(base, out)
            self.assertEqual(rc, 0)
            go = (out / "gomodulo/p/x.go").read_text(encoding="utf-8")
            self.assertIn("func P_a_b(", go)
            self.assertIn("func P_a_b_2(", go)
            resumen = (out / "RESUMEN.md").read_text(encoding="utf-8")
            self.assertIn("identificadores Go que colisionaron (con sufijo) | 1 |", resumen)
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


class TestIndiceDelPseudocodigo(unittest.TestCase):
    """El indice del arbol de pseudocodigo se puede hacer de dos formas.

    ``ruta``     deduce la clave de la ruta del ``.c`` (rapido: no abre NINGUN
                 fichero para indexar).
    ``cabecera`` lee la cabecera ``/* <ruta.go> -- ... */`` de cada ``.c``.

    Los dos tienen que dar el MISMO mapa, y ademas se verifica que la
    cabecera que declara Ghidra coincide con la ruta deducida.
    """

    @classmethod
    def setUpClass(cls):
        cls.tmp = Path(tempfile.mkdtemp(prefix="test-regenerar-indice-"))
        cls.base = escribir_fixture(cls.tmp / "base")

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def test_los_dos_modos_dan_el_mismo_mapa(self):
        por_ruta, idx_ruta = regenerar.indexar_pseudocodigo(self.base / "pseudo", "ruta")
        por_cab, idx_cab = regenerar.indexar_pseudocodigo(self.base / "pseudo", "cabecera")
        self.assertEqual(sorted(map(str, por_ruta)), sorted(map(str, por_cab)))
        self.assertEqual(
            sorted(str(i) for i in idx_ruta), sorted(str(i) for i in idx_cab)
        )

    def test_el_modo_ruta_no_abre_ningun_fichero_para_indexar(self):
        # el mapa sale de la RUTA: no hace falta leer ninguna cabecera
        por_ruta, _ = regenerar.indexar_pseudocodigo(self.base / "pseudo", "ruta")
        self.assertIn("third_party/go/gc/src/internal/abi/abi.go", por_ruta)
        self.assertIn("third_party/jetski/language_server/server.go", por_ruta)

    def test_el_resumen_cuenta_las_cabeceras_que_no_coinciden(self):
        # se falsea la cabecera de un fichero: el resumen debe contarlo
        base2 = escribir_fixture(self.tmp / "base2")
        c = base2 / "pseudo/third_party/go/gc/src/internal/abi/abi.c"
        c.write_text(
            PSEUDO_ABI.replace(
                "/* third_party/go/gc/src/internal/abi/abi.go -- pseudocodigo",
                "/* otro/fichero.go -- pseudocodigo",
            ),
            encoding="utf-8",
        )
        out = self.tmp / "out2"
        rc = ejecutar(base2, out)
        assert rc == 0, "la herramienta devolvio %d" % rc
        resumen = (out / "RESUMEN.md").read_text(encoding="utf-8")
        self.assertIn("cabeceras que NO coinciden con la ruta deducida | 1 |", resumen)

    def test_el_resumen_dice_como_se_indexo(self):
        out = self.tmp / "out3"
        rc = ejecutar(self.base, out)
        assert rc == 0, "la herramienta devolvio %d" % rc
        resumen = (out / "RESUMEN.md").read_text(encoding="utf-8")
        self.assertIn("| indice del pseudocodigo | `ruta` |", resumen)


#: Un `.c` de r2ghidra: un fichero POR FUNCION, sin marcadores `/* ==== */`.
PSEUDO_R2 = """/* third_party/go/gc/src/internal/abi/abi.go -- pseudocodigo decompilado por r2ghidra (radare2 6.2.6 + r2ghidra 6.2.6) */
/* go: internal_abi.__RegArgs_.Dump */


void internal_abi.__RegArgs_.Dump(int64_t *param_1,ulong param_2)

{
  FUN_04d06cb8(param_1,param_2);
}
"""


class TestLayoutR2Ghidra(unittest.TestCase):
    """El segundo layout del arbol (r2ghidra) tambien se regenera.

    r2ghidra escribe UN `.c` POR FUNCION dentro del directorio del fichero
    Go original y lo llama por el simbolo; ademas usa tipos C99
    (``int64_t``, ``ulong``) que Ghidra no escribe.  Si el generador no
    entiende ese layout, esas funciones se quedan sin pseudocodigo.
    """

    @classmethod
    def setUpClass(cls):
        cls.tmp = Path(tempfile.mkdtemp(prefix="test-regenerar-r2-"))
        cls.base = cls.tmp / "base"
        d = cls.base / "pseudo/rz0/third_party/go/gc/src/internal/abi/abi.go"
        d.mkdir(parents=True, exist_ok=True)
        (cls.base / "funcs.tsv").write_text(TSV_EJEMPLO, encoding="utf-8")
        (cls.base / "pseudo/rz0/_index.tsv").write_text(INDICE_EJEMPLO, encoding="utf-8")
        (d / "internal_abi.__RegArgs_.Dump.c").write_text(PSEUDO_R2, encoding="utf-8")
        cls.out = cls.tmp / "out"
        rc = ejecutar(cls.base, cls.out)
        assert rc == 0, "la herramienta devolvio %d" % rc

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def test_la_funcion_de_r2ghidra_tiene_pseudocodigo(self):
        c = (
            self.out / "codigo/third_party/go/gc/src/internal/abi/abi.c"
        ).read_text(encoding="utf-8")
        # el cuerpo de r2ghidra esta volcado (no el marcador SIN PSEUDOCODIGO)
        self.assertIn("FUN_04d06cb8(param_1,param_2);", c)
        self.assertIn(
            "/* firma Ghidra   : void internal_abi.__RegArgs_.Dump(int64_t *param_1,ulong param_2) */",
            c,
        )
        # las demas funciones del fichero no tienen .c propio: siguen stubs
        self.assertIn("SIN PSEUDOCODIGO", c)

    def test_la_firma_traduce_los_tipos_c99(self):
        go = (
            self.out / "gomodulo/third_party/go/gc/src/internal/abi/abi.go"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "func Internal_abi_RegArgs_Dump(param_1 unsafe.Pointer, param_2 uint64) {", go
        )
        # `int64_t *` es un puntero => el fichero TIENE que importar unsafe
        self.assertIn('import "unsafe"', go)

    def test_ningun_fichero_tiene_imports_sin_usar(self):
        mod = self.out / "gomodulo"
        for g in sorted(mod.rglob("*.go")):
            texto = g.read_text(encoding="utf-8")
            self.assertEqual(
                "unsafe.Pointer" in texto,
                'import "unsafe"' in texto,
                "%s: import sin usar" % g,
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)

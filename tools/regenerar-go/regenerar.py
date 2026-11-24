#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""regenerar.py — convierte el binario Go en CÓDIGO GO REGENERADO y navegable.

ENTRADA (la que YA produce el pipeline existente del repo; aqui NO se
re-decompila nada, no se duplica ningun motor):

  * ``--tsv``    export del pclntab (``go.funcs.tsv``), 161.682 funciones.
                Formato: ``direccion_hex \\t nombre_go \\t origen``  donde
                ``origen`` = ``go: <firma> | <ruta/fichero.go>:<linea>``.
                Lo produce el motor Rust de RE (``rust-core/src/gobook.rs`` +
                ``go-retool``); su parser es ``gobook::parsear_linea`` y aqui
                se replica EXACTAMENTE la misma semantica.
  * ``--pseudo`` pseudocodigo de Ghidra escrito por
                ``tools/ghidra-scripts/MdGhidra.java`` (modo ``all``): un
                ``.c`` por fichero Go original, en el MISMO arbol de
                directorios, mas un ``_index.tsv`` con
                ``addr / name / file / line / bytes / ok / err``.

SALIDA:

  * ``codigo/<paquete>/README.md``     indice del paquete (nombre, linea
    original, direccion, tamano).
  * ``codigo/<paquete>/<fichero>.c``   pseudocodigo de Ghidra con la linea
    original ya asociada y las funciones ordenadas por linea.
  * ``gomodulo/go.mod`` + ``gomodulo/<paquete>/<fichero>.go``  modulo Go
    COMPILABLE de stubs (el cuerpo real se sustituye por
    ``panic("pseudocodigo: ver <fichero>")``), navegable con ``gopls``/
    ``go doc`` como si fuera el repositorio original.
  * ``RESUMEN.md``                      cifras medidas + funciones que NO se
    pudieron decompilar.
  * ``NO-DECOMPIADAS.tsv``             listado completo de esas funciones.

CRITERIO DE AGRUPADO (el del repo, ver ``gobook.rs::mapa``): el paquete es el
DIRECTORIO del fichero Go original; dentro del paquete se ordena por fichero
y luego por numero de linea (ver ``gobook.rs::esqueleto``).

GARANTIAS:

  * idempotente: dos ejecuciones con la misma entrada dan el mismo byte a byte
    (no se escribe ninguna marca de tiempo en la salida);
  * streaming: los 161.682 registros se ordenan con un sort EXTERNO por trozos
    (``OrdenadorExterno``), de modo que nunca hace falta el TSV entero en
    memoria; la salida se escribe incrementalmente y solo se mantiene en
    memoria el pseudocodigo de UN fichero original a la vez;
  * tolera un directorio de pseudocodigo incompleto: sigue, dice cuanto ha
    encontrado y lo refleja en ``RESUMEN.md``.

uso:
  tools/regenerar-go/regenerar.py [--tsv F] [--pseudo DIR] [--out DIR]
"""
from __future__ import annotations

import argparse
import hashlib
import heapq
import os
import re
import shutil
import sys
import tempfile
import time
from pathlib import Path
from typing import Dict, Iterator, List, Optional, Sequence, Tuple

# ---------------------------------------------------------------------------
# constantes
# ---------------------------------------------------------------------------

VERSION = "1.0.0"

#: Palabras reservadas de Go (no pueden ser identificadores).
GO_PALABRAS_CLAVE = frozenset(
    """break case chan const continue default defer else fallthrough for func go
    goto if import interface map package range return select struct switch type
    var""".split()
)

#: Identificadores predefinidos de Go: shadowearlos confunde al lector.
GO_PREDEFINIDOS = frozenset(
    """bool byte complex64 complex128 error float32 float64 int int8 int16 int32
    int64 rune string uint uint8 uint16 uint32 uint64 uintptr any true false
    iota nil append cap clear close complex copy delete imag len make new panic
    print println real recover""".split()
)

#: Tipo de C de Ghidra -> tipo de Go ("" = sin valor de retorno).
TIPO_C_A_GO = {
    "void": "",
    "undefined": "uint8",
    "undefined1": "uint8",
    "undefined2": "uint16",
    "undefined4": "uint32",
    "undefined8": "uint64",
    "byte": "byte",
    "char": "int8",
    "short": "int16",
    "ushort": "uint16",
    "int": "int32",
    "uint": "uint32",
    "long": "int64",
    "ulong": "uint64",
    "float": "float32",
    "double": "float64",
    "bool": "bool",
    "code": "func()",
}

RE_MARCA_FUNCION = re.compile(
    r"^/\* ==== (?P<nombre>.+?)  \[(?P<bytes>\d+) bytes\] ==== \*/\s*$", re.M
)
RE_COMENTO_GO = re.compile(
    r"/\* go: (?P<simbolo>[^\n]*)\n\s*(?P<fichero>[^\n:]*):(?P<linea>\d+) \*/"
)
RE_CABECERA_PSEUDO = re.compile(
    r"^/\* (?P<orig>.+?) -- pseudocodigo decompilado por Ghidra \(MegaDaemon\) \*/"
)
RE_PROTOTIPO = re.compile(
    r"^(?P<ret>[A-Za-z_][\w \t\*\[\]]*?)\s+(?P<nombre>[A-Za-z_][\w.$]*)\s*\((?P<params>.*)\)\s*;?$"
)
RE_PARAM = re.compile(r"^(?P<tipo>[A-Za-z_]\w*)\s+(?P<nombre>[A-Za-z_]\w*)$")

#: El toolchain de Go ignora los directorios y ficheros que empiezan por _ o .
PREFIJOS_IGNORADOS_POR_GO = ("_", ".")

#: Nombre del paquete Go cuando el fichero original esta en la raiz.
NOMBRE_PAQ_RAIZ = "raiz"

#: Desglose por categoria del codigo de ORIGEN (medido sobre el TSV real).
#: El orden es el que sale en el RESUMEN.
CATEGORIAS = (
    "generado (blaze/protobuf)",
    "autogenerado (compilador)",
    "jetski (codigo propio de Antigravity)",
    "runtime Go",
    "librerias publicas (third_party/golang)",
    "otras third_party",
    "resto",
)


def categoria_de(fichero: str) -> str:
    """Clasifica un fichero Go original en su categoria de codigo.

    Se deduce de la RUTA del fichero original (que es lo unico que el
    pclntab conserva), no de una lista escrita a mano:

      * ``blaze-out/...``      codigo GENERADO por Blaze/protobuf
      * ``<autogenerated>``    wrappers que genera el COMPILADOR
      * ``third_party/jetski`` codigo PROPIO de Antigravity
      * ``third_party/go/gc``  el RUNTIME de Go
      * ``third_party/golang``_LIBRERIAS publicas
    """
    f = fichero or ""
    if f.startswith("blaze-out/"):
        return CATEGORIAS[0]
    if f == "<autogenerated>":
        return CATEGORIAS[1]
    if f.startswith("third_party/jetski/"):
        return CATEGORIAS[2]
    if f.startswith("third_party/go/gc/"):
        return CATEGORIAS[3]
    if f.startswith("third_party/golang/"):
        return CATEGORIAS[4]
    if f.startswith("third_party/"):
        return CATEGORIAS[5]
    return CATEGORIAS[6]


# ---------------------------------------------------------------------------
# utilidades pequenas
# ---------------------------------------------------------------------------


def esc(s: str) -> str:
    """Escapa un campo para poder escribirlo en un TSV temporal."""
    return s.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n")


def unesc(s: str) -> str:
    return s.replace("\\n", "\n").replace("\\t", "\t").replace("\\\\", "\\")


def sha256_de(path: Path, bloque: int = 1 << 20) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            b = f.read(bloque)
            if not b:
                break
            h.update(b)
    return h.hexdigest()


def sanear_ident(nombre: str) -> str:
    """Convierte cualquier texto en un identificador de Go valido."""
    s = re.sub(r"[^A-Za-z0-9_]", "_", nombre or "")
    s = re.sub(r"_+", "_", s).strip("_")
    if not s:
        s = "fn"
    if s[0].isdigit():
        s = "_" + s
    if s in GO_PALABRAS_CLAVE or s in GO_PREDEFINIDOS:
        s += "_"
    return s


def sanitize_ruta(ruta: str) -> str:
    """Ruta segura para el arbol de salida (mismo criterio en los dos arboles).

    * cada elemento que empieza por ``_`` o ``.`` se renombra con el prefijo
      ``x`` (el toolchain de Go ignora esos directorios);
    * los caracteres no permitidos se sustituyen por ``_``;
    * los elementos vacios se eliminan.
    """
    partes = []
    for elem in (ruta or "").split("/"):
        if not elem:
            continue
        e = re.sub(r"[^A-Za-z0-9._-]", "_", elem)
        if e.startswith(PREFIJOS_IGNORADOS_POR_GO):
            e = "x" + e
        if e in (".", ".."):
            e = "x" + e.replace(".", "_")
        partes.append(e)
    return "/".join(partes)


def sanitize_fichero(nombre: str) -> str:
    """Nombre de fichero seguro (mismo criterio en los dos arboles)."""
    n = re.sub(r"[^A-Za-z0-9._-]", "_", nombre or "")
    n = n.rstrip("_")
    if not n:
        n = "sin_nombre"
    if n.startswith(PREFIJOS_IGNORADOS_POR_GO):
        n = "x" + n
    return n


def nombre_paquete_go(paquete_dir: str) -> str:
    """Nombre de paquete Go a partir del directorio del fichero original."""
    if not paquete_dir:
        return NOMBRE_PAQ_RAIZ
    elem = sanitize_ruta(paquete_dir).split("/")[-1]
    elem = re.sub(r"[^A-Za-z0-9_]", "_", elem or "")
    if not elem:
        return NOMBRE_PAQ_RAIZ
    if elem[0].isdigit():
        elem = "p" + elem
    if elem in GO_PALABRAS_CLAVE or elem in GO_PREDEFINIDOS:
        elem += "_"
    return elem


# ---------------------------------------------------------------------------
# parser del TSV (mismo criterio que rust-core/src/gobook.rs::parsear_linea)
# ---------------------------------------------------------------------------


class Funcion:
    """Una fila del export del pclntab."""

    __slots__ = ("addr", "addr_hex", "simbolo", "fichero", "linea", "paquete_go", "nombre_go")

    def __init__(
        self,
        addr: int,
        addr_hex: str,
        simbolo: str,
        fichero: str,
        linea: int,
        paquete_go: str,
        nombre_go: str,
    ) -> None:
        self.addr = addr
        self.addr_hex = addr_hex
        self.simbolo = simbolo
        self.fichero = fichero
        self.linea = linea
        self.paquete_go = paquete_go
        self.nombre_go = nombre_go

    @property
    def paquete_dir(self) -> str:
        """El 'paquete' segun el criterio del repo: el DIRECTORIO del fichero."""
        return os.path.dirname(self.fichero)


def parsear_linea(linea: str) -> Optional[Funcion]:
    """Parsea una linea del TSV. Devuelve ``None`` si no es interpretable.

    Espejo exacto de ``gobook.rs::parsear_linea``:
      ``0x4d06cb8  internal_abi.__RegArgs_.Dump  go: internal/abi.(*RegArgs).Dump
       | third_party/go/gc/src/internal/abi/abi.go:47``
      -> addr 0x4d06cb8, fichero ``.../abi.go``, linea 47, paquete Go
      ``internal/abi``, nombre Go ``(*RegArgs).Dump``.
    """
    partes = linea.rstrip("\n").split("\t")
    if len(partes) < 2:
        return None
    addr_hex = partes[0].strip()
    simbolo = partes[1].strip()
    resto = partes[2].strip() if len(partes) > 2 else ""
    if not addr_hex or not simbolo:
        return None
    try:
        addr = int(addr_hex[2:] if addr_hex[:2].lower() == "0x" else addr_hex, 16)
    except ValueError:
        return None

    if "|" in resto:
        desc, loc = resto.rsplit("|", 1)
    else:
        desc, loc = resto, ""
    desc = desc.strip()
    if desc.startswith("go:"):
        desc = desc[3:].strip()
    loc = loc.strip()
    if ":" in loc:
        fichero, linea_txt = loc.rsplit(":", 1)
    else:
        fichero, linea_txt = loc, ""
    fichero = fichero.strip()
    try:
        num_linea = int(linea_txt.strip())
    except ValueError:
        num_linea = 0

    if "." in desc:
        paquete_go, nombre_go = desc.split(".", 1)
    else:
        paquete_go, nombre_go = desc, ""

    return Funcion(addr, addr_hex, simbolo, fichero, num_linea, paquete_go, nombre_go)


# ---------------------------------------------------------------------------
# nombres e identificadores Go
# ---------------------------------------------------------------------------


def _sanear_pieza(texto: str) -> str:
    s = re.sub(r"[^A-Za-z0-9_]", "_", texto or "")
    s = re.sub(r"_+", "_", s).strip("_")
    return s


def ident_go_desde_simbolo(simbolo_mangled: str) -> str:
    """Traduce el simbolo Go mangled a un identificador Go valido.

    ``internal_abi.__RegArgs_.Dump`` -> ``Internal_abi_RegArgs_Dump``
    ``runtime.main``                   -> ``Runtime_main``
    ``main.main.func1``                -> ``Main_main_func1``

    Regla: se separa el simbolo en ``<paquete>.<resto>`` por el PRIMER punto
    (igual que ``gobook.rs``), cada parte se sanea (los caracteres no
    alfanumericos pasan a ``_``, se colapsan las rachas y se recortan los
    guiones bajos sobrantes) y se unen con ``_``.  Si el resultado es una
    palabra reservada (o un predefinido) de Go se le anade ``_``.
    """
    simbolo = simbolo_mangled or ""
    if "." in simbolo:
        pkg, resto = simbolo.split(".", 1)
    else:
        pkg, resto = simbolo, ""
    a = _sanear_pieza(pkg)
    b = _sanear_pieza(resto)
    ident = (a + "_" + b) if (a and b) else (a or b)
    if not ident:
        ident = "fn"
    if ident[0].isdigit():
        ident = "_" + ident
    if ident in GO_PALABRAS_CLAVE or ident in GO_PREDEFINIDOS:
        ident += "_"
    return ident[0].upper() + ident[1:]


# ---------------------------------------------------------------------------
# firmas: de C (Ghidra) a Go
# ---------------------------------------------------------------------------


def tipo_go(tipo_c: str) -> str:
    """Traduce un tipo de C de Ghidra a un tipo de Go."""
    t = (tipo_c or "").strip()
    if not t:
        return "uint64"
    if "*" in t:
        return "unsafe.Pointer"
    base = re.sub(r"\[\s*\d*\s*\]", "", t).strip()
    palabras = base.split()
    if not palabras:
        return "uint64"
    clave = palabras[0]
    if clave in ("struct", "union", "enum"):
        return "uint64"
    return TIPO_C_A_GO.get(clave, "uint64")


def partir_params(texto: str) -> List[str]:
    """Parte la lista de parametros de C por comas de nivel 0."""
    salida: List[str] = []
    prof = 0
    actual: List[str] = []
    for ch in texto:
        if ch in "([":
            prof += 1
        elif ch in ")]":
            prof = max(0, prof - 1)
        if ch == "," and prof == 0:
            salida.append("".join(actual).strip())
            actual = []
        else:
            actual.append(ch)
    resto = "".join(actual).strip()
    if resto:
        salida.append(resto)
    return [p for p in salida if p]


def partir_param(bruto: str) -> Tuple[str, str]:
    """Parte un parametro de C en ``(tipo, nombre)``.

    Ghidra escribe los punteros como ``undefined1 *param_2`` (asterisco pegado
    al nombre) o ``undefined *`` (sin nombre).
    """
    tokens = bruto.split()
    if not tokens:
        return ("uint64", "")
    if len(tokens) == 1:
        return (tokens[0], "")
    ultimo = tokens[-1]
    if ultimo.startswith("*"):
        nombre = ultimo.lstrip("*")
        if not nombre:
            return (" ".join(tokens), "")
        return (" ".join(tokens[:-1]) + " *", nombre)
    return (" ".join(tokens[:-1]), ultimo)


def parsear_prototipo(prototipo: str) -> Tuple[Optional[str], List[Tuple[str, str]]]:
    """Extrae ``(tipo_retorno, [(tipo, nombre), ...])`` de un prototipo de C.

    Acepta el formato que emite Ghidra: ``undefined4 *f(undefined8 a,long b);``
    Si no encaja devuelve ``(None, [])``.
    """
    p = (prototipo or "").strip().rstrip(";").strip()
    if not p:
        return None, []
    m = RE_PROTOTIPO.match(p)
    if not m:
        return None, []
    ret = m.group("ret").strip()
    params: List[Tuple[str, str]] = []
    for bruto in partir_params(m.group("params")):
        bruto = bruto.strip()
        if bruto == "void":
            continue  # `f(void)` en C = sin parametros
        tipo_c, nombre_c = partir_param(bruto)
        params.append((tipo_c, nombre_c))
    return ret, params


def firma_go(
    ret_c: Optional[str], params: List[Tuple[str, str]]
) -> Tuple[str, str, bool]:
    """Construye la firma Go a partir de la firma de C.

    Devuelve ``(parametros, retorno, necesita_unsafe)``; los parametros ya
    vienen renombrados a identificadores Go validos y unicos dentro de la
    funcion.
    """
    partes: List[str] = []
    necesita_unsafe = False
    vistos: Dict[str, int] = {}
    for i, (tipo_c, nombre_c) in enumerate(params):
        g = tipo_go(tipo_c)
        if g == "unsafe.Pointer":
            necesita_unsafe = True
        base = sanear_ident(nombre_c) if nombre_c else ""
        if not base:
            base = "arg%d" % i
        n = vistos.get(base, 0)
        vistos[base] = n + 1
        if n:
            base = "%s%d" % (base, n + 1)
        partes.append("%s %s" % (base, g))
    ret = ""
    if ret_c:
        g = tipo_go(ret_c)
        if g == "unsafe.Pointer":
            necesita_unsafe = True
        if g:
            ret = " " + g
    return ", ".join(partes), ret, necesita_unsafe


# ---------------------------------------------------------------------------
# sort externo (nunca cargamos el TSV entero en memoria)
# ---------------------------------------------------------------------------


class OrdenadorExterno:
    """Ordena registros por trozos en disco y los fusiona con heapq.

    Se usa para no meter los 161.682 registros en memoria de una vez: el
    consumo queda acotado por ``trozo`` registros.
    """

    def __init__(self, tmpdir: Path, nombre: str, clave, trozo: int = 40000) -> None:
        self.tmpdir = Path(tmpdir)
        self.tmpdir.mkdir(parents=True, exist_ok=True)
        self.nombre = nombre
        self.clave = clave
        self.trozo = trozo
        self.buf: List[tuple] = []
        self.chunks: List[Path] = []
        self.total = 0

    def add(self, reg: tuple) -> None:
        self.buf.append(reg)
        self.total += 1
        if len(self.buf) >= self.trozo:
            self._volcar()

    def _volcar(self) -> None:
        if not self.buf:
            return
        self.buf.sort(key=self.clave)
        p = self.tmpdir / ("%s.%03d.tmp" % (self.nombre, len(self.chunks)))
        with open(p, "w", encoding="utf-8") as f:
            for reg in self.buf:
                f.write("\t".join(esc(str(x)) for x in reg))
                f.write("\n")
        self.chunks.append(p)
        self.buf = []

    @staticmethod
    def _leer(p: Path) -> Iterator[tuple]:
        with open(p, encoding="utf-8") as f:
            for line in f:
                line = line.rstrip("\n")
                if line:
                    yield tuple(unesc(x) for x in line.split("\t"))

    def ordenados(self) -> Iterator[tuple]:
        self._volcar()
        gens = [self._leer(p) for p in self.chunks]
        for reg in heapq.merge(*gens, key=self.clave):
            yield reg

    def limpiar(self) -> None:
        for p in self.chunks:
            try:
                p.unlink()
            except OSError:
                pass
        self.chunks = []
        self.buf = []


# ---------------------------------------------------------------------------
# indice del arbol de pseudocodigo
# ---------------------------------------------------------------------------


class Bloque:
    """Un bloque de pseudocodigo de Ghidra (una funcion)."""

    __slots__ = ("nombre", "bytes", "texto", "simbolo_go", "fichero_go", "linea_go")

    def __init__(self, nombre: str, bytes_: int, texto: str) -> None:
        self.nombre = nombre
        self.bytes = bytes_
        self.texto = texto
        self.simbolo_go = ""
        self.fichero_go = ""
        self.linea_go = 0

    @property
    def prototipo(self) -> str:
        lineas = self.texto.splitlines()
        return lineas[0].strip() if lineas else ""


def _indexar_por_cabecera(raiz: Path) -> Dict[str, List[Path]]:
    """Indice LENTO: lee la cabecera de cada fichero de pseudocodigo.

    Lee las 4 KiB iniciales de cada ``.c`` para sacar el ``/* <ruta.go> -- ... */``
    y usa esa ruta como clave.  Es la fuente de verdad, pero abrir 11.000
    ficheros en un NAS saturado cuesta **horas** (medido: ~0,8 ficheros/s =>
    ~3,7 h para 11.104 ficheros).  Se deja por si hay que verificar.
    """
    mapa: Dict[str, List[Path]] = {}
    if not raiz.is_dir():
        return mapa
    for dirpath, _dirnames, filenames in os.walk(raiz):
        for fn in filenames:
            if fn.endswith((".tsv", ".log", ".md", ".txt")):
                continue
            p = Path(dirpath) / fn
            try:
                with open(p, encoding="utf-8", errors="replace") as f:
                    cabecera = f.read(4096)
            except OSError:
                continue
            primera = cabecera.strip().splitlines()[0] if cabecera.strip() else ""
            m = RE_CABECERA_PSEUDO.match(primera)
            if not m:
                continue
            orig = m.group("orig").strip()
            if orig.endswith(".go") or orig == "<autogenerated>":
                mapa.setdefault(orig, []).append(p)
    return mapa


def _indexar_por_ruta(raiz: Path) -> Tuple[Dict[str, List[Path]], List[Path]]:
    """Indice RAPIDO: deduce el fichero Go original de la RUTA del ``.c``.

    El arbol de pseudocodigo de Ghidra replica la ruta del fichero Go
    original (``todo/sN/<ruta_go_del_fichero>.c``), asi que la ruta ES la
    clave: no hace falta abrir NADA para indexar.

    Medido sobre el corpus real (11.104 ficheros): el 60/60 de la muestra
    tiene la MISMA ruta deducida que la cabecera, y el barrido sin abrir
    ficheros tarda 72 s frente a ~3,7 h abriendo cada fichero.

    Puede haber VARIOS ficheros para el mismo original: la decompilacion se
    reparte en shards (slices) y cada shard escribe los bloques que le
    tocaron del mismo fichero Go.  Por eso el valor es una lista.

    Devuelve ademas la lista de los ``_index.tsv`` encontrados (un shard cada
    uno): el mismo barrido los localiza, asi que no hace falta un SEGUNDO
    recorrido del arbol.
    """
    mapa: Dict[str, List[Path]] = {}
    indices: List[Path] = []
    if not raiz.is_dir():
        return mapa, indices

    def rec(d: str, base: Optional[str]) -> None:
        try:
            entradas = list(os.scandir(d))
        except OSError:
            return
        # un directorio que contiene _index.tsv es la raiz de un shard: a
        # partir de ahi las rutas son relativas al shard, no al arbol
        if any(e.name == "_index.tsv" for e in entradas):
            base = d
            indices.append(Path(d) / "_index.tsv")
        for e in entradas:
            try:
                es_dir = e.is_dir(follow_symlinks=False)
            except OSError:
                es_dir = False
            if es_dir:
                rec(e.path, base)
            elif e.name.endswith(".c") and base is not None:
                rel = os.path.relpath(e.path, base)
                mapa.setdefault(rel[:-2] + ".go", []).append(Path(e.path))

    rec(str(raiz), None)
    return mapa, sorted(indices)


def indexar_pseudocodigo(
    raiz: Path, modo: str = "ruta"
) -> Tuple[Dict[str, List[Path]], List[Path]]:
    """Mapa ``fichero Go original -> ficheros de pseudocodigo`` que lo contienen.

    ``modo='ruta'``    deduce la clave de la ruta (rapido, por defecto).
    ``modo='cabecera'`` lee la cabecera de cada fichero (lento, verificable).

    Devuelve tambien la lista de ``_index.tsv`` (uno por shard).
    """
    if modo == "cabecera":
        return _indexar_por_cabecera(raiz), buscar_indices_ghidra(raiz)
    return _indexar_por_ruta(raiz)


def buscar_indices_ghidra(raiz: Path) -> List[Path]:
    """Todos los ``_index.tsv`` del arbol de pseudocodigo (uno por shard)."""
    encontrados: List[Path] = []
    if not raiz.is_dir():
        return encontrados
    for dirpath, _dirnames, filenames in os.walk(raiz):
        if "_index.tsv" in filenames:
            encontrados.append(Path(dirpath) / "_index.tsv")
    return sorted(encontrados)


def parsear_bloques(
    path: Path, cabeceras: Optional[Dict[Path, str]] = None
) -> Dict[str, List[Bloque]]:
    """Trocea un ``.c`` de Ghidra en bloques, indexados por nombre.

    Si se pasa ``cabeceras``, se rellena con la ruta Go que declara la
    cabecera del fichero (``/* <ruta.go> -- pseudocodigo ... */``).  Sale
    GRATIS: la cabecera es la primera linea del texto que ya se ha leido.
    Sirve para VERIFICAR que la ruta deducida por el indice coincide con la
    que declara Ghidra (si no, se cuenta y se avisa).
    """
    try:
        texto = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return {}
    if cabeceras is not None:
        primera = texto.strip().splitlines()[0] if texto.strip() else ""
        m = RE_CABECERA_PSEUDO.match(primera)
        cabeceras[path] = m.group("orig").strip() if m else ""
    marcas = list(RE_MARCA_FUNCION.finditer(texto))
    salida: Dict[str, List[Bloque]] = {}
    for i, m in enumerate(marcas):
        fin = marcas[i + 1].start() if i + 1 < len(marcas) else len(texto)
        cuerpo = texto[m.end() : fin].strip("\n")
        b = Bloque(m.group("nombre").strip(), int(m.group("bytes")), cuerpo)
        cm = RE_COMENTO_GO.search(cuerpo)
        if cm:
            b.simbolo_go = cm.group("simbolo").strip()
            b.fichero_go = cm.group("fichero").strip()
            try:
                b.linea_go = int(cm.group("linea"))
            except ValueError:
                b.linea_go = 0
        salida.setdefault(b.nombre, []).append(b)
    return salida


def _todos_los_bloques(bloques: Dict[str, List[Bloque]]) -> Iterator[Bloque]:
    for lista in bloques.values():
        for b in lista:
            yield b


# ---------------------------------------------------------------------------
# lectura de las entradas
# ---------------------------------------------------------------------------


def leer_tsv(ruta: Path) -> Iterator[Funcion]:
    """Lee el TSV de funciones linea a linea (nunca entero en memoria)."""
    with open(ruta, encoding="utf-8", errors="replace") as f:
        for linea in f:
            if not linea.strip():
                continue
            fn = parsear_linea(linea)
            if fn is not None:
                yield fn


def rutas_indice(pseudo: Path) -> List[Path]:
    """Todos los ``_index.tsv`` del árbol de pseudocodigo.

    MEDIDO (2026-11-23): el árbol de Ghidra está REPARTIDO en subdirectorios
    (``todo/s0``, ``todo/s1``, ``todo/s2``), cada uno con su ``_index.tsv``.
    La herramienta sólo miraba en la raíz ⇒ «indice Ghidra: 0 filas» ⇒
    ninguna función heredaba su origen Go.
    """
    out: List[Path] = []
    raiz = pseudo / "_index.tsv"
    if raiz.is_file():
        out.append(raiz)
    try:
        subs = sorted(p for p in pseudo.iterdir() if p.is_dir())
    except OSError:
        subs = []
    for d in subs:
        f = d / "_index.tsv"
        if f.is_file():
            out.append(f)
    return out


def leer_indice_ghidra(ruta: Path) -> Iterator[Tuple[str, str, str, str, str, str, str]]:
    """Lee el ``_index.tsv`` de Ghidra: addr, name, file, line, bytes, ok, err."""
    with open(ruta, encoding="utf-8", errors="replace") as f:
        cabecera = f.readline()
        if not cabecera:
            return
        for linea in f:
            if not linea.strip():
                continue
            p = linea.rstrip("\n").split("\t")
            while len(p) < 7:
                p.append("")
            yield (p[0], p[1], p[2], p[3], p[4], p[5], p[6])


# ---------------------------------------------------------------------------
# union (merge-join) de las dos fuentes
# ---------------------------------------------------------------------------


def unir_tsv_indice(
    tsv: Iterator[tuple], idx: Iterator[tuple]
) -> Iterator[Tuple[Optional[tuple], Optional[tuple]]]:
    """Une el TSV de funciones con el indice de Ghidra por (fichero, linea,
    nombre).

    Ambas entradas llegan ordenadas por esa clave, asi que es un merge lineal.
    Devuelve pares ``(registro_tsv, registro_indice_o_None)``; cuando el
    registro del TSV es ``None`` es una funcion que solo Ghidra conoce (no
    tiene origen Go y no se regenera, pero se cuenta).
    """
    t = next(tsv, None)
    i = next(idx, None)
    while t is not None or i is not None:
        if i is None or (t is not None and t[:4] < i[:4]):
            yield (t, None)
            t = next(tsv, None)
        elif t is None or i[:4] < t[:4]:
            yield (None, i)
            i = next(idx, None)
        else:
            clave = t[:4]
            grupo_t: List[tuple] = []
            grupo_i: List[tuple] = []
            while t is not None and t[:4] == clave:
                grupo_t.append(t)
                t = next(tsv, None)
            while i is not None and i[:4] == clave:
                grupo_i.append(i)
                i = next(idx, None)
            for k in range(max(len(grupo_t), len(grupo_i))):
                gt = grupo_t[k] if k < len(grupo_t) else None
                gi = grupo_i[k] if k < len(grupo_i) else None
                yield (gt, gi)


# ---------------------------------------------------------------------------
# escritura de la salida
# ---------------------------------------------------------------------------


class EscritorPaquete:
    """Acumula las filas del indice de un paquete y escribe su README.md."""

    def __init__(self, dir_paquete: str, ruta_readme: Path) -> None:
        self.dir_paquete = dir_paquete
        self.ruta_readme = ruta_readme
        self.filas: List[Tuple[str, int, int, int, str]] = []
        self.ficheros: Dict[str, int] = {}
        self.addr_min: Optional[int] = None
        self.addr_max: Optional[int] = None
        self.por_paquete_go: Dict[str, int] = {}

    def anadir(
        self,
        simbolo: str,
        linea: int,
        addr: int,
        bytes_: int,
        fichero: str,
        pkg_go: str,
    ) -> None:
        self.filas.append((simbolo, linea, addr, bytes_, fichero))
        self.ficheros[fichero] = self.ficheros.get(fichero, 0) + 1
        if self.addr_min is None or addr < self.addr_min:
            self.addr_min = addr
        if self.addr_max is None or addr > self.addr_max:
            self.addr_max = addr
        if pkg_go:
            self.por_paquete_go[pkg_go] = self.por_paquete_go.get(pkg_go, 0) + 1

    def escribir(self) -> int:
        self.ruta_readme.parent.mkdir(parents=True, exist_ok=True)
        total = len(self.filas)
        with open(self.ruta_readme, "w", encoding="utf-8") as f:
            f.write("# `%s`\n\n" % (self.dir_paquete or "(raiz del binario)"))
            f.write(
                "**%d funciones** · **%d ficheros** · rango `0x%x`–`0x%x`\n\n"
                % (total, len(self.ficheros), self.addr_min or 0, self.addr_max or 0)
            )
            f.write("| # | funcion Go | linea | direccion | bytes | fichero |\n")
            f.write("|---:|---|---:|---|---:|---|\n")
            for n, (simbolo, linea, addr, bytes_, fichero) in enumerate(self.filas, 1):
                f.write(
                    "| %d | `%s` | %d | `0x%x` | %d | `%s` |\n"
                    % (n, simbolo, linea, addr, bytes_, fichero)
                )
            f.write("\n## Ficheros de este paquete\n\n")
            for fichero, n in sorted(self.ficheros.items()):
                f.write("- `%s` — %d funciones\n" % (fichero, n))
            if len(self.por_paquete_go) > 1:
                f.write("\n## Reparto por paquete Go (campo `go:` del pclntab)\n\n")
                f.write("| paquete Go | funciones |\n|---|---:|\n")
                for pkg, n in sorted(self.por_paquete_go.items(), key=lambda kv: (-kv[1], kv[0])):
                    f.write("| `%s` | %d |\n" % (pkg, n))
            f.write(
                "\n---\n\nCodigo regenerado con `tools/regenerar-go/regenerar.py` "
                "(v%s).  Los cuerpos son pseudocodigo de Ghidra, no Go original.\n" % VERSION
            )
        return total


# ---------------------------------------------------------------------------
# motor principal
# ---------------------------------------------------------------------------


class Opciones:
    def __init__(self) -> None:
        self.tsv = Path("/workspace/agy-re/tools-out/go.funcs.tsv")
        self.pseudo = Path("/workspace/agy-re/tools-out/todo")
        self.out = Path("/workspace/agy-re/tools-out/regenerado")
        self.modulo = "mdl.local/go-regenerado"
        self.go_version = "1.21"
        self.limite = 0
        self.filtro_paquete = ""
        self.chunk = 40000
        self.limpiar = True
        self.sin_modulo = False
        self.sin_codigo = False
        #: como se indexa el arbol de pseudocodigo: "ruta" (rapido) o
        #: "cabecera" (lee la cabecera de cada fichero; ~3,7 h en el NAS)
        self.indice = "ruta"


def log(msg: str) -> None:
    sys.stderr.write("[regenerar] %s\n" % msg)
    sys.stderr.flush()


def run(o: Opciones) -> int:
    t0 = time.time()
    if not o.tsv.is_file():
        log("ERROR: no existe el TSV %s" % o.tsv)
        return 2
    if not o.pseudo.is_dir():
        log("AVISO: no existe el directorio de pseudocodigo %s (se seguira igualmente)" % o.pseudo)

    if o.limpiar and o.out.exists():
        # rmtree puede fallar si el arbol se esta tocando (NAS, otra
        # ejecucion): se reintenta y, si sigue fallando, se sigue adelante
        # (las ficheros se sobrescriben).
        for _intento in range(3):
            try:
                shutil.rmtree(o.out)
                break
            except OSError:
                time.sleep(0.5)
        else:
            log("AVISO: no se pudo borrar del todo %s; se sobrescribe encima" % o.out)
    o.out.mkdir(parents=True, exist_ok=True)

    tmpdir = Path(tempfile.mkdtemp(prefix="regenerar-go-", dir=str(o.out)))
    try:
        return _run(o, tmpdir, t0)
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def _run(o: Opciones, tmpdir: Path, t0: float) -> int:
    # ---------------- 1. indice del arbol de pseudocodigo -----------------
    mapa_pseudo = indexar_pseudocodigo(o.pseudo)
    idx_ghidras = rutas_indice(o.pseudo)
    idx_ghidra = idx_ghidras[0] if idx_ghidras else (o.pseudo / "_index.tsv")
    tiene_indice = bool(idx_ghidras)
    log(
        "pseudocodigo: %d ficheros Go con pseudocodigo en %d shard(s) (indices: %s)"
        % (
            len(mapa_pseudo),
            len(indices),
            ", ".join(i.parent.name for i in indices) if indices else "NINGUNO",
        )
    )

    # ---------------- 2. ordenar el TSV por (fichero, linea, nombre) ------
    # La clave de orden es (directorio, fichero, linea, simbolo): asi los
    # ficheros de un mismo PAQUETE quedan juntos aunque su ruta completa no
    # sea contigua (p.ej. `runtime/debug/x.go` ordena antes que
    # `runtime/mgc.go`, aunque los dos sean del paquete `runtime`).
    ord_tsv = OrdenadorExterno(tmpdir, "tsv", lambda r: (r[0], r[1], r[2], r[3], r[4]), o.chunk)
    n_func = 0
    n_sin_fichero = 0
    cat_funcs: Dict[str, int] = {c: 0 for c in CATEGORIAS}
    for fn in leer_tsv(o.tsv):
        if o.filtro_paquete and o.filtro_paquete not in fn.paquete_dir:
            continue
        if o.limite and n_func >= o.limite:
            break
        if not fn.fichero:
            n_sin_fichero += 1
            fn.fichero = "(sin fichero)"
        cat = categoria_de(fn.fichero)
        cat_funcs[cat] = cat_funcs.get(cat, 0) + 1
        dir_paq = os.path.dirname(fn.fichero)
        base_paq = os.path.basename(fn.fichero)
        ord_tsv.add(
            (
                dir_paq,
                base_paq,
                "%012d" % fn.linea,
                fn.simbolo,
                "%016x" % fn.addr,
                fn.paquete_go,
                fn.nombre_go,
                fn.addr_hex,
                fn.fichero,
            )
        )
        n_func += 1
    log("TSV: %d funciones leidas (%d sin fichero original)" % (n_func, n_sin_fichero))

    # ---------------- 3. ordenar el indice de Ghidra ---------------------
    ord_idx = OrdenadorExterno(tmpdir, "idx", lambda r: (r[0], r[1], r[2], r[3], r[4]), o.chunk)
    n_idx = 0
    n_idx_sin_go = 0
    if tiene_indice:
        for una in idx_ghidras:
            for addr, name, file_, line_, bytes_, ok, err in leer_indice_ghidra(una):
                if o.filtro_paquete and o.filtro_paquete not in os.path.dirname(file_):
                    continue
                if not file_:
                    n_idx_sin_go += 1
                    continue
                try:
                    linea_num = int(line_)
                except ValueError:
                    linea_num = 0
                ord_idx.add((file_, "%012d" % linea_num, name, addr, bytes_, ok, err))
                n_idx += 1
    log("indice Ghidra: %d filas (%d sin origen Go, no regeneradas)" % (n_idx, n_idx_sin_go))

    # ---------------- 4. unir y ordenar por (fichero, linea, addr) -------
    unidos = OrdenadorExterno(tmpdir, "join", lambda r: (r[0], r[1], r[2]), o.chunk)
    n_join_tsv = 0
    n_join_solo_idx = 0
    n_con_indice = 0
    for reg_t, reg_i in unir_tsv_indice(ord_tsv.ordenados(), ord_idx.ordenados()):
        if reg_t is None:
            n_join_solo_idx += 1
            continue
        (
            dir_paq,
            base_paq,
            linea_s,
            simbolo,
            addr_ord,
            pkg_go,
            nombre_go,
            addr_hex,
            fichero,
        ) = reg_t
        if reg_i is None:
            bytes_ = "0"
            ok = "0"
            err = "sin-pseudocodigo"
            idx_nombre = simbolo
        else:
            _id, _ib, _il, idx_nombre, _ia, bytes_, ok, err, _if = reg_i
            n_con_indice += 1
        try:
            addr = int(addr_hex, 16)
        except ValueError:
            addr = int(addr_ord, 16)
        unidos.add(
            (
                dir_paq,
                base_paq,
                linea_s,
                "%016x" % addr,
                simbolo,
                pkg_go,
                nombre_go,
                str(bytes_),
                ok,
                err,
                idx_nombre,
                fichero,
            )
        )
        n_join_tsv += 1
    log("union: %d funciones del TSV (%d solo de Ghidra)" % (n_join_tsv, n_join_solo_idx))

    # ---------------- 5. escribir el arbol --------------------------------
    dir_codigo = o.out / "codigo"
    dir_modulo = o.out / "gomodulo"
    if not o.sin_codigo:
        dir_codigo.mkdir(parents=True, exist_ok=True)
    if not o.sin_modulo:
        dir_modulo.mkdir(parents=True, exist_ok=True)
        (dir_modulo / "go.mod").write_text(
            "module %s\n\ngo %s\n" % (o.modulo, o.go_version), encoding="utf-8"
        )

    pkg_actual: Optional[str] = None
    escritor_pkg: Optional[EscritorPaquete] = None
    ident_usados: set = set()
    nombre_pkg_go = NOMBRE_PAQ_RAIZ
    fichero_actual = ""
    bloques_fichero: Dict[str, List[Bloque]] = {}
    fichero_bloques = ""
    c_fh = None
    go_buf: List[str] = []
    go_necesita_unsafe = [False]
    go_path_actual: List[Path] = [Path(".")]
    primera_en_go = True
    n_funciones_c = 0
    n_funciones_con_pseudo = 0
    n_funciones_sin_pseudo = 0
    n_funciones_go = 0
    n_ficheros_c = 0
    n_ficheros_go = 0
    cat_ficheros: Dict[str, int] = {c: 0 for c in CATEGORIAS}
    colisiones = 0
    no_decompiladas: List[Tuple[str, str, int, int, str]] = []
    total_bytes_c = 0
    funcs_por_paquete: Dict[str, int] = {}
    #: cabecera que declara cada fichero de pseudocodigo (verificacion)
    cabeceras: Dict[Path, str] = {}
    n_cabecera_distinta = 0
    n_cabecera_sin_leer = 0

    def cerrar_fichero() -> None:
        """Cierra el .c y vuelca el .go (con el import que corresponda)."""
        nonlocal c_fh
        if c_fh is not None:
            c_fh.close()
            c_fh = None
        if go_buf:
            with open(go_path_actual[0], "w", encoding="utf-8") as fh:
                fh.write("// Regenerado por tools/regenerar-go/regenerar.py (v%s).\n" % VERSION)
                fh.write(
                    "// Stubs compilables: el cuerpo real no se puede recuperar del binario.\n"
                )
                fh.write("// Paquete Go original: %s\n\n" % (nombre_pkg_go or NOMBRE_PAQ_RAIZ))
                fh.write("package %s\n\n" % nombre_pkg_go)
                if go_necesita_unsafe[0]:
                    fh.write('import "unsafe"\n\n')
                fh.write("".join(go_buf))
            go_buf.clear()
            go_necesita_unsafe[0] = False

    def cerrar_paquete() -> None:
        nonlocal escritor_pkg, ident_usados
        if escritor_pkg is not None:
            escritor_pkg.escribir()
            ident_usados = set()

    def abrir_fichero(fichero: str, bloques: Dict[str, List[Bloque]]) -> None:
        nonlocal c_fh, n_ficheros_c, n_ficheros_go, primera_en_go
        cerrar_fichero()
        cat = categoria_de(fichero)
        cat_ficheros[cat] = cat_ficheros.get(cat, 0) + 1
        dir_rel = sanitize_ruta(os.path.dirname(fichero))
        if dir_rel:
            (dir_codigo / dir_rel).mkdir(parents=True, exist_ok=True)
            (dir_modulo / dir_rel).mkdir(parents=True, exist_ok=True)
        base = sanitize_fichero(os.path.basename(fichero))
        if base.endswith(".go"):
            base = base[:-3]
        c_path = dir_codigo / dir_rel / (base + ".c") if dir_rel else dir_codigo / (base + ".c")
        go_path = dir_modulo / dir_rel / (base + ".go") if dir_rel else dir_modulo / (base + ".go")
        c_fh = open(c_path, "w", encoding="utf-8")
        go_path_actual[0] = go_path
        go_buf.clear()
        primera_en_go = True
        n_ficheros_c += 1
        n_ficheros_go += 1
        c_fh.write("/* %s -- pseudocodigo decompilado por Ghidra (MegaDaemon) */\n" % fichero)
        c_fh.write(
            "/* Regenerado por tools/regenerar-go/regenerar.py · "
            "funciones ordenadas por linea original */\n\n"
        )

    for reg in unidos.ordenados():
        (
            dir_paquete,
            base_paquete,
            linea_s,
            addr_ord,
            simbolo,
            pkg_go,
            nombre_go,
            bytes_s,
            ok,
            err,
            idx_nombre,
            fichero,
        ) = reg
        linea = int(linea_s)
        if dir_paquete != pkg_actual:
            cerrar_fichero()
            cerrar_paquete()
            pkg_actual = dir_paquete
            dir_rel = sanitize_ruta(dir_paquete)
            nombre_pkg_go = nombre_paquete_go(dir_paquete)
            if dir_rel:
                (dir_codigo / dir_rel).mkdir(parents=True, exist_ok=True)
                (dir_modulo / dir_rel).mkdir(parents=True, exist_ok=True)
                readme = dir_codigo / dir_rel / "README.md"
            else:
                readme = dir_codigo / "README.md"
            escritor_pkg = EscritorPaquete(dir_paquete, readme)
            ident_usados = set()
            fichero_actual = ""
        if fichero != fichero_actual:
            # solo se mantiene en memoria el pseudocodigo de UN fichero; si
            # el fichero Go esta repartido en varios shards se fusionan sus
            # bloques (se agrupan por nombre de funcion).
            bloques_fichero = {}
            for pseudo in mapa_pseudo.get(fichero, []):
                for nombre_b, lista in parsear_bloques(pseudo, cabeceras).items():
                    bloques_fichero.setdefault(nombre_b, []).extend(lista)
                # verificacion gratuita: la cabecera que declara Ghidra debe
                # coincidir con la ruta deducida del indice
                cab = cabeceras.get(pseudo, None)
                if cab is None:
                    n_cabecera_sin_leer += 1
                elif cab != fichero:
                    n_cabecera_distinta += 1
            fichero_bloques = fichero
            abrir_fichero(fichero, bloques_fichero)
            fichero_actual = fichero

        # --- pseudocodigo -------------------------------------------------
        bloque = None
        for cand in (idx_nombre, simbolo):
            cola = bloques_fichero.get(cand)
            if not cola:
                continue
            # se prefiere el bloque cuyo comentario `go:` coincide con el
            # fichero y la linea originales (desempata cuando el mismo
            # nombre aparece varias veces en el mismo fichero)
            elegido = None
            for candidato in cola:
                if candidato.fichero_go == fichero and candidato.linea_go == linea:
                    elegido = candidato
                    break
            if elegido is None:
                elegido = cola[0]
            cola.remove(elegido)
            bloque = elegido
            break

        addr = int(addr_ord, 16)
        bytes_ = int(bytes_s) if str(bytes_s).isdigit() else 0

        # --- cabecera en el .c --------------------------------------------
        if c_fh is not None:
            c_fh.write("/* ==== %s  [%d bytes] ==== */\n" % (simbolo, bytes_))
            c_fh.write("/* linea original : %d */\n" % linea)
            c_fh.write("/* direccion       : 0x%x */\n" % addr)
            c_fh.write("/* simbolo Go     : %s */\n" % simbolo)
            if pkg_go:
                c_fh.write("/* paquete Go     : %s */\n" % pkg_go)
            if bloque is not None and bloque.prototipo:
                c_fh.write("/* firma Ghidra   : %s */\n" % bloque.prototipo)
            c_fh.write("\n")
            if bloque is not None:
                c_fh.write(bloque.texto.rstrip() + "\n")
                total_bytes_c += len(bloque.texto)
                n_funciones_con_pseudo += 1
            else:
                c_fh.write("/* SIN PSEUDOCODIGO (%s) */\n" % (err or "no decompilado"))
                n_funciones_sin_pseudo += 1
            c_fh.write("\n")
            n_funciones_c += 1

        # --- stub en Go ----------------------------------------------------
        if True:
            ident = ident_go_desde_simbolo(simbolo)
            if ident in ident_usados:
                colisiones += 1
                k = 2
                while "%s_%d" % (ident, k) in ident_usados:
                    k += 1
                ident = "%s_%d" % (ident, k)
            ident_usados.add(ident)
            ret_c, params = parsear_prototipo(bloque.prototipo if bloque is not None else "")
            params_txt, ret_txt, _unsafe = firma_go(ret_c, params)
            cuerpo = 'panic("pseudocodigo: ver %s")' % fichero
            base_c = sanitize_fichero(os.path.basename(fichero))
            if base_c.endswith(".go"):
                base_c = base_c[:-3]
            if primera_en_go:
                primera_en_go = False
            else:
                go_buf.append("\n")
            go_buf.append(
                "// %s es la funcion %s del binario (0x%x).\n//\n"
                "//\tlinea original: %s:%d\n//\tpseudocodigo:   codigo/%s/%s.c\n"
                % (
                    ident,
                    simbolo,
                    addr,
                    fichero,
                    linea,
                    sanitize_ruta(dir_paquete),
                    base_c,
                )
            )
            go_buf.append("func %s(%s)%s {\n\t%s\n}\n" % (ident, params_txt, ret_txt, cuerpo))
            if _unsafe:
                go_necesita_unsafe[0] = True
            n_funciones_go += 1

        if escritor_pkg is not None:
            escritor_pkg.anadir(simbolo, linea, addr, bytes_, base_paquete, pkg_go)
        funcs_por_paquete[dir_paquete] = funcs_por_paquete.get(dir_paquete, 0) + 1
        if ok != "1" or bloque is None:
            no_decompiladas.append((simbolo, fichero, linea, addr, err or "sin-pseudocodigo"))

    cerrar_fichero()
    cerrar_paquete()

    # ---------------- 6. resumen -----------------------------------------
    n_paquetes = 0
    for _root, _dirs, files in os.walk(dir_codigo):
        if "README.md" in files:
            n_paquetes += 1

    bytes_total = 0
    n_ficheros_total = 0
    for _root, _dirs, files in os.walk(o.out):
        for fn in files:
            fp = Path(_root) / fn
            try:
                bytes_total += fp.stat().st_size
                n_ficheros_total += 1
            except OSError:
                pass

    with open(o.out / "NO-DECOMPIADAS.tsv", "w", encoding="utf-8") as f:
        f.write("simbolo\tfichero\tlinea\tdireccion\terror\n")
        for simbolo, fichero, linea, addr, err in no_decompiladas:
            f.write("%s\t%s\t%d\t0x%x\t%s\n" % (simbolo, fichero, linea, addr, err))

    with open(o.out / "RESUMEN.md", "w", encoding="utf-8") as f:
        f.write("# Binario Go regenerado — RESUMEN\n\n")
        f.write("Generado con `tools/regenerar-go/regenerar.py` (v%s).\n\n" % VERSION)
        f.write("## Entradas\n\n")
        f.write("| entrada | ruta | sha256 |\n|---|---|---|\n")
        f.write("| export del pclntab | `%s` | `%s` |\n" % (o.tsv, sha256_de(o.tsv)))
        for una in idx_ghidras:
            if una.is_file():
                f.write(
                    "| indice de Ghidra | `%s` | `%s` |\n" % (una, sha256_de(una))
                )
        f.write(
            "| pseudocodigo | `%s` | %d ficheros mapeados |\n" % (o.pseudo, len(mapa_pseudo))
        )
        f.write("\n## Cifras medidas\n\n")
        f.write("| magnitud | valor |\n|---|---:|\n")
        f.write("| funciones en el export del pclntab | %d |\n" % n_func)
        f.write("| funciones procesadas (export del pclntab) | %d |\n" % n_func)
        f.write("| funciones con fila en el indice de Ghidra | %d |\n" % n_con_indice)
        f.write(
            "| funciones **sin** pseudocodigo (listadas en NO-DECOMPIADAS.tsv) | %d |\n"
            % len(no_decompiladas)
        )
        f.write("| funciones que solo existen en Ghidra (sin origen Go) | %d |\n" % n_join_solo_idx)
        f.write("| paquetes (directorios del binario) | %d |\n" % n_paquetes)
        f.write("| ficheros `.c` generados | %d |\n" % n_ficheros_c)
        f.write("| ficheros `.go` (stubs) generados | %d |\n" % n_ficheros_go)
        f.write("| bloques escritos en los `.c` (cabecera por funcion) | %d |\n" % n_funciones_c)
        f.write("| funciones CON pseudocodigo volcado | %d |\n" % n_funciones_con_pseudo)
        f.write("| funciones SIN pseudocodigo | %d |\n" % n_funciones_sin_pseudo)
        f.write(
            "| funciones con fila de indice pero sin bloque de pseudocodigo | %d |\n"
            % max(0, n_con_indice - n_funciones_con_pseudo)
        )
        f.write("| stubs Go emitidos | %d |\n" % n_funciones_go)
        f.write("| identificadores Go que colisionaron (con sufijo) | %d |\n" % colisiones)
        f.write("| bytes del pseudocodigo volcado | %d |\n" % total_bytes_c)
        f.write("| bytes totales del arbol de salida | %d |\n" % bytes_total)
        f.write("| ficheros totales del arbol de salida | %d |\n" % n_ficheros_total)
        f.write("| indice del pseudocodigo | `%s` |\n" % o.indice)
        f.write(
            "| cabeceras que NO coinciden con la ruta deducida | %d |\n" % n_cabecera_distinta
        )
        f.write("| ficheros de pseudocodigo sin cabecera legible | %d |\n" % n_cabecera_sin_leer)
        f.write("\n## Desglose por categoria del codigo de origen\n\n")
        f.write("| categoria | funciones | %% | ficheros `.c` |\n|---|---:|---:|---:|\n")
        for cat in CATEGORIAS:
            nf = cat_funcs.get(cat, 0)
            pct = 100.0 * nf / max(1, n_func)
            f.write(
                "| %s | %d | %.2f%% | %d |\n"
                % (cat, nf, pct, cat_ficheros.get(cat, 0))
            )
        f.write("\n## Arbol generado\n\n")
        f.write("```\n")
        f.write("%s/\n" % o.out)
        f.write("|-- RESUMEN.md            <- este fichero\n")
        f.write("|-- NO-DECOMPIADAS.tsv    <- las funciones sin pseudocodigo\n")
        f.write("|-- codigo/<paquete>/README.md     <- indice del paquete\n")
        f.write("|-- codigo/<paquete>/<fichero>.c   <- pseudocodigo por linea original\n")
        f.write("`-- gomodulo/go.mod + gomodulo/<paquete>/<fichero>.go  <- stubs\n")
        f.write("```\n\n")
        f.write("## Funciones que NO se pudieron decompilar\n\n")
        if not no_decompiladas:
            f.write("Ninguna: todas las funciones tienen pseudocodigo.\n")
        else:
            f.write(
                "**%d funciones** sin pseudocodigo de Ghidra (%.2f%% de %d).  "
                "El listado completo esta en `NO-DECOMPIADAS.tsv`; las primeras 50:\n\n"
                % (
                    len(no_decompiladas),
                    100.0 * len(no_decompiladas) / max(1, n_join_tsv),
                    n_join_tsv,
                )
            )
            f.write("| simbolo | fichero | linea | direccion | error |\n|---|---|---:|---|---|\n")
            for simbolo, fichero, linea, addr, err in no_decompiladas[:50]:
                f.write(
                    "| `%s` | `%s` | %d | `0x%x` | %s |\n" % (simbolo, fichero, linea, addr, err)
                )
        f.write("\n---\n\n")
        f.write(
            'El cuerpo de los stubs Go es `panic("pseudocodigo: ver <fichero>")`: '
            "el codigo Go original no esta en el binario (solo sus nombres, ficheros y "
            "lineas, via el pclntab).\n"
        )

    log(
        "hecho en %.1fs: %d funciones, %d paquetes, %d ficheros .c, %d bytes"
        % (time.time() - t0, n_join_tsv, n_paquetes, n_ficheros_c, bytes_total)
    )
    return 0


def main(argv: Optional[Sequence[str]] = None) -> int:
    p = argparse.ArgumentParser(
        description="Convierte el binario Go en codigo Go regenerado y navegable"
    )
    p.add_argument("--tsv", type=Path, default=Path("/workspace/agy-re/tools-out/go.funcs.tsv"))
    p.add_argument("--pseudo", type=Path, default=Path("/workspace/agy-re/tools-out/todo"))
    p.add_argument("--out", type=Path, default=Path("/workspace/agy-re/tools-out/regenerado"))
    p.add_argument("--modulo", default="mdl.local/go-regenerado", help="ruta del modulo Go")
    p.add_argument("--go-version", default="1.21")
    p.add_argument("--limite", type=int, default=0, help="solo las N primeras funciones (0 = todas)")
    p.add_argument("--filtro-paquete", default="", help="solo los paquetes cuya ruta contenga esto")
    p.add_argument("--chunk", type=int, default=40000, help="registros por trozo del sort externo")
    p.add_argument("--sin-limpiar", action="store_true", help="no borrar la salida antes de escribir")
    p.add_argument("--sin-modulo", action="store_true")
    p.add_argument("--sin-codigo", action="store_true")
    p.add_argument(
        "--indice",
        choices=("ruta", "cabecera"),
        default="ruta",
        help="como se indexa el pseudocodigo: 'ruta' deduce la clave de la "
        "ruta del .c (rapido, por defecto); 'cabecera' lee la cabecera de "
        "cada fichero (lento: ~3,7 h sobre el NAS saturado)",
    )
    args = p.parse_args(argv)

    o = Opciones()
    o.tsv = args.tsv
    o.pseudo = args.pseudo
    o.out = args.out
    o.modulo = args.modulo
    o.go_version = args.go_version
    o.limite = args.limite
    o.filtro_paquete = args.filtro_paquete
    o.chunk = max(1000, args.chunk)
    o.limpiar = not args.sin_limpiar
    o.sin_modulo = args.sin_modulo
    o.sin_codigo = args.sin_codigo
    o.indice = args.indice
    return run(o)


if __name__ == "__main__":
    sys.exit(main())

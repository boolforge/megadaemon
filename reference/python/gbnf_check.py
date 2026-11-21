#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
GBNF checker — validador y comprobador de conformidad para las gramáticas
de SEERE v11.

Por qué existe: una gramática rota hace que llama.cpp devuelva 400 y muera
el agente entero; y una gramática "demasiado laxa" no aporta nada (NVIDIA:
"the constrained generation was either too restrictive or too permissive to be
helpful").  Este módulo permite AFIRMAR ambas cosas con evidencia.

Implementa el subconjunto de GBNF que usa SEERE:
    rule ::= alternativa ("|" alternativa)*
    alternativa ::= secuencia
    secuencia ::= item+
    item ::= literal | clase | referencia | "(" alternancia ")" [repetición]
    repetición ::= "?" | "*" | "{" m "," n "}" | "{" m "}"

Uso:
    python3 gbnf_check.py                 # valida las gramáticas de seere_v11
    python3 gbnf_check.py --grammar f.gbnf --string "A: $ id"
"""
from __future__ import annotations

import argparse
import os
import re
import sys
from typing import Dict, List, Optional, Set, Tuple

RULE_RE = re.compile(r"^([a-zA-Z][a-zA-Z0-9_\-]*)\s*::=\s*(.*)$")


class GbnfError(Exception):
    pass


class Gbnf:
    def __init__(self, text: str):
        self.rules: Dict[str, str] = {}
        self.order: List[str] = []
        for raw in text.splitlines():
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            m = RULE_RE.match(line)
            if not m:
                raise GbnfError(f"no es una regla GBNF: {raw!r}")
            name, body = m.group(1), m.group(2)
            if name in self.rules:
                raise GbnfError(f"regla duplicada: {name}")
            self.rules[name] = body
            self.order.append(name)
        if "root" not in self.rules:
            raise GbnfError("falta la regla 'root' (obligatoria en GBNF)")
        self._cache: Dict[str, str] = {}

    # ---- análisis estático ---------------------------------------------- #
    def refs(self, body: str) -> Set[str]:
        """Referencias a reglas dentro de un cuerpo (ignora literales)."""
        out: Set[str] = set()
        i = 0
        in_str = False
        in_class = False
        buf = ""
        while i < len(body):
            ch = body[i]
            if in_str:
                if ch == "\\":
                    i += 2
                    continue
                if ch == '"':
                    in_str = False
                i += 1
                continue
            if in_class:
                if ch == "\\":
                    i += 2
                    continue
                if ch == "]":
                    in_class = False
                i += 1
                continue
            if ch == '"':
                in_str = True
                i += 1
                continue
            if ch == "[":
                in_class = True
                i += 1
                continue
            if ch.isalpha() or ch == "_":
                buf += ch
                i += 1
                continue
            if buf:
                out.add(buf)
                buf = ""
            i += 1
        if buf:
            out.add(buf)
        return out

    def dangling(self) -> List[Tuple[str, str]]:
        bad = []
        for name, body in self.rules.items():
            for r in self.refs(body):
                if r not in self.rules:
                    bad.append((name, r))
        return bad

    def unreachable(self) -> List[str]:
        seen: Set[str] = set()
        stack = ["root"]
        while stack:
            n = stack.pop()
            if n in seen or n not in self.rules:
                continue
            seen.add(n)
            stack.extend(self.refs(self.rules[n]))
        return [r for r in self.order if r not in seen]

    # ---- compilación a regex -------------------------------------------- #
    def _unescape(self, lit: str) -> str:
        out = []
        i = 0
        while i < len(lit):
            c = lit[i]
            if c == "\\" and i + 1 < len(lit):
                nxt = lit[i + 1]
                out.append({"n": "\n", "t": "\t", "r": "\r", "\\": "\\",
                            '"': '"', "]": "]", "[": "[", "-": "-"}.get(nxt, nxt))
                i += 2
                continue
            out.append(c)
            i += 1
        return "".join(out)

    def _class_to_regex(self, cls: str) -> str:
        """[a-z0-9] -> [a-z0-9] ; [^\\n] -> [^\\n] (con escapes resueltos).

        El '-' DEBE seguir escapado en la regex de Python: sin él, '+-*'
        se interpreta como un rango inválido.
        """
        i = 0
        out = []
        while i < len(cls):
            c = cls[i]
            if c == "\\" and i + 1 < len(cls):
                nxt = cls[i + 1]
                if nxt == "-":
                    out.append("\\-")
                else:
                    out.append({"n": "\n", "t": "\t", "r": "\r", "\\": "\\\\",
                                "]": "\\]", "[": "\\["}.get(nxt, "\\" + nxt))
                i += 2
                continue
            out.append(c)
            i += 1
        return "[" + "".join(out) + "]"

    def _compile(self, name: str, stack: Optional[Set[str]] = None) -> str:
        if name in self._cache:
            return self._cache[name]
        stack = stack or set()
        if name in stack:
            raise GbnfError(f"recursión en las reglas: {name}")
        stack = stack | {name}
        body = self.rules[name]
        parts = self._split_top(body)
        alts = []
        for alt in parts:
            alts.append(self._compile_seq(alt, stack))
        rx = "|".join(f"(?:{a})" for a in alts)
        self._cache[name] = rx
        return rx

    def _split_top(self, body: str) -> List[str]:
        """Divide por '|' de nivel superior (fuera de literales, clases y paréntesis)."""
        out, depth, in_str, in_class, buf = [], 0, False, False, []
        i = 0
        while i < len(body):
            ch = body[i]
            if in_str:
                buf.append(ch)
                if ch == "\\" and i + 1 < len(body):
                    buf.append(body[i + 1])
                    i += 2
                    continue
                if ch == '"':
                    in_str = False
                i += 1
                continue
            if in_class:
                buf.append(ch)
                if ch == "\\" and i + 1 < len(body):
                    buf.append(body[i + 1])
                    i += 2
                    continue
                if ch == "]":
                    in_class = False
                i += 1
                continue
            if ch == '"':
                in_str = True
                buf.append(ch)
            elif ch == "[":
                in_class = True
                buf.append(ch)
            elif ch == "(":
                depth += 1
                buf.append(ch)
            elif ch == ")":
                depth -= 1
                buf.append(ch)
            elif ch == "|" and depth == 0:
                out.append("".join(buf))
                buf = []
            else:
                buf.append(ch)
            i += 1
        out.append("".join(buf))
        return out

    def _compile_seq(self, seq: str, stack: Set[str]) -> str:
        out = []
        i = 0
        while i < len(seq):
            ch = seq[i]
            if ch.isspace():
                i += 1
                continue
            if ch == '"':
                j = i + 1
                while j < len(seq):
                    if seq[j] == "\\":
                        j += 2
                        continue
                    if seq[j] == '"':
                        break
                    j += 1
                lit = seq[i + 1:j]
                out.append(re.escape(self._unescape(lit)))
                i = j + 1
            elif ch == "[":
                j = i + 1
                while j < len(seq):
                    if seq[j] == "\\":
                        j += 2
                        continue
                    if seq[j] == "]":
                        break
                    j += 1
                out.append(self._class_to_regex(seq[i + 1:j]))
                i = j + 1
            elif ch == "(":
                depth = 1
                j = i + 1
                while j < len(seq) and depth:
                    if seq[j] == "(":
                        depth += 1
                    elif seq[j] == ")":
                        depth -= 1
                        if depth == 0:
                            break
                    j += 1
                inner = seq[i + 1:j]
                # el contenido del paréntesis puede traer su propia alternancia
                inner_alts = [self._compile_seq(a, stack) for a in self._split_top(inner)]
                out.append("(?:" + "|".join(f"(?:{a})" for a in inner_alts) + ")")
                i = j + 1
            elif ch.isalpha() or ch == "_":
                j = i
                while j < len(seq) and (seq[j].isalnum() or seq[j] in "_-"):
                    j += 1
                name = seq[i:j]
                if name not in self.rules:
                    raise GbnfError(f"referencia a regla inexistente: {name}")
                # Una referencia puede contener alternancia de nivel superior:
                # hay que agruparla o el "|" se escaparía hacia fuera.
                out.append("(?:" + self._compile(name, stack) + ")")
                i = j
            else:
                raise GbnfError(f"carácter inesperado en la gramática: {ch!r} en {seq!r}")
            # sufijo de repetición
            if i < len(seq):
                nxt = seq[i]
                if nxt == "?":
                    out[-1] = f"(?:{out[-1]})?"
                    i += 1
                elif nxt == "*":
                    out[-1] = f"(?:{out[-1]})*"
                    i += 1
                elif nxt == "{":
                    j = seq.find("}", i)
                    if j < 0:
                        raise GbnfError("repetición sin cerrar")
                    spec = seq[i + 1:j]
                    lo, _, hi = spec.partition(",")
                    lo_i = int(lo)
                    hi_i = int(hi) if hi else lo_i
                    out[-1] = f"(?:{out[-1]}){{{lo_i},{hi_i}}}"
                    i = j + 1
        return "".join(out)

    def regex(self) -> str:
        return self._compile("root")

    def accepts(self, text: str) -> bool:
        return re.fullmatch(self.regex(), text, re.DOTALL) is not None


# --------------------------------------------------------------------------- #
# Suite de conformidad de SEERE v11
# --------------------------------------------------------------------------- #
def seere_conformance() -> Tuple[int, int, List[str]]:
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import seere_v11 as V

    cases: List[Tuple[bool, str, str]] = [
        # --- acciones canónicas: deben aceptarse ---
        (True, V.GBNF_REACT, "A: $ uname -a"),
        (True, V.GBNF_REACT, "A: ? latest llama.cpp release"),
        (True, V.GBNF_REACT, "A: W python asyncio"),
        (True, V.GBNF_REACT, "A: C /data/local/tmp/x#0"),
        (True, V.GBNF_REACT, "A: G pattern#/data/local/tmp"),
        (True, V.GBNF_REACT, "A: M 2+2"),
        (True, V.GBNF_REACT, "A: @ write a python script"),
        (True, V.GBNF_REACT, "A: S i"),
        (True, V.GBNF_REACT, "A: S r 0x10 64"),
        (True, V.GBNF_REACT, "A: S w 0x10 deadbeef"),
        (True, V.GBNF_REACT, "A: K l"),
        (True, V.GBNF_REACT, "A: K g key"),
        (True, V.GBNF_REACT, "A: K d key"),
        (True, V.GBNF_REACT, "A: K s key=value"),
        (True, V.GBNF_REACT, "A: ! always quote the path"),
        (True, V.GBNF_REACT, "A: ASK which file?"),
        (True, V.GBNF_REACT, "A: NONE nothing to do"),
        (True, V.GBNF_REACT, "R: uptime is 22 [SRC:1]"),
        # --- debe fallar en los sitios imposibles ---
        (False, V.GBNF_REACT, "A: C /data/x 0"),          # el índice exige '#'
        (False, V.GBNF_REACT, "A: G pattern /data/x"),    # exige '#'
        (False, V.GBNF_REACT, "A: M hello!"),             # 'math' no admite letras sueltas
        (False, V.GBNF_REACT, "A: K s keyvalue"),         # '=' obligatorio
        (False, V.GBNF_REACT, "A: S r 0x10"),            # faltan offset y longitud
        (False, V.GBNF_REACT, "R: "),                     # respuesta vacía
        (False, V.GBNF_REACT, "P: plan\nA: $ id"),        # el modo act-only no admite plan
        (False, V.GBNF_REACT, "A: Z algo"),               # opcode inexistente
        (False, V.GBNF_FINAL, "A: $ id"),                 # fase final: sin herramientas
        (True, V.GBNF_FINAL, "R: uptime is 22 [SRC:1]"),
        (True, V.GBNF_BIN, "SI"),
        (True, V.GBNF_BIN, "DEPENDE DEL CONTEXTO"),
        (False, V.GBNF_BIN, "YES"),
        (True, V.GBNF_REACT_PLAN, "P: read the file\nA: C /data/x#0"),
        (False, V.GBNF_REACT_PLAN, "A: C /data/x#0"),     # el plan es obligatorio
    ]

    fails: List[str] = []
    ok = 0
    for should, grammar, text in cases:
        g = Gbnf(grammar)
        got = g.accepts(text)
        if got == should:
            ok += 1
        else:
            fails.append(f"esperado {'ACEPTA' if should else 'RECHAZA'} {text!r} "
                         f"-> {'acepta' if got else 'rechaza'}")
    return ok, len(cases), fails


def main() -> int:
    ap = argparse.ArgumentParser(prog="gbnf_check")
    ap.add_argument("--grammar", help="fichero .gbnf")
    ap.add_argument("--string", help="cadena a comprobar")
    args = ap.parse_args()

    if args.grammar:
        g = Gbnf(open(args.grammar, encoding="utf-8").read())
        for name, ref in g.dangling():
            print(f"DANGLING: {name} -> {ref}")
        for r in g.unreachable():
            print(f"UNREACHABLE: {r}")
        if args.string is not None:
            print("ACCEPT" if g.accepts(args.string) else "REJECT")
        return 0

    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import seere_v11 as V

    print("=" * 74)
    print("GBNF CONFORMANCE — SEERE v11")
    print("=" * 74)
    for name, g in (("GBNF_REACT", V.GBNF_REACT), ("GBNF_FINAL", V.GBNF_FINAL),
                    ("GBNF_REACT_PLAN", V.GBNF_REACT_PLAN), ("GBNF_BIN", V.GBNF_BIN),
                    ("GBNF_OPCODE_ONLY", V.GBNF_OPCODE_ONLY)):
        try:
            p = Gbnf(g)
            dang = p.dangling()
            unreach = p.unreachable()
            status = "OK"
            if dang:
                status = f"DANGLING {dang}"
            elif unreach:
                status = f"UNREACHABLE {unreach}"
            print(f"  {name:18s} rules={len(p.rules):2d}  {status}")
        except GbnfError as e:
            print(f"  {name:18s} ERROR: {e}")
    ok, n, fails = seere_conformance()
    print(f"\n  conformance: {ok}/{n}")
    for f in fails:
        print("   FAIL:", f)
    print("\n" + "=" * 74)
    print("GBNF_CHECK_OK" if (ok == n and not fails) else "GBNF_CHECK_FAILED")
    return 0 if (ok == n and not fails) else 1


if __name__ == "__main__":
    sys.exit(main())

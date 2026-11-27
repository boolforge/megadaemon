#!/usr/bin/env python3
"""Recorta los extractos verbatim largos (>3 lineas de bloque cercado) de los
informes de contratos, dejando intactos los datos medidos: rutas:linea, tablas,
nombres de campo, de evento y de enum.

Los bloques largos no se borran: se sustituyen por un puntero a la copia
completa en la Drive privada. La version completa se sube antes de aplicar esto.
"""
import re, sys, pathlib

MAX_LINES = 3
BANNER = "> [recortado: {n} lineas; la copia integra esta en la Drive privada, `cursor-3.32.4-contratos/`]"


def trim(text: str) -> tuple[str, int, int]:
    out, cut, blocks = [], 0, 0
    i, lines = 0, text.split("\n")
    while i < len(lines):
        ln = lines[i]
        m = re.match(r"^\s*```", ln)
        if not m:
            out.append(ln); i += 1; continue
        # finding el cierre
        j = i + 1
        while j < len(lines) and not re.match(r"^\s*```", lines[j]):
            j += 1
        body = lines[i + 1:j]
        if len(body) > MAX_LINES:
            # conservar la primera linea si es corta y es una cabecera de clave
            head = body[0].strip()
            keep = [head] if (len(head) <= 110 and re.search(r"[=:]", head)) else []
            out.append(ln)
            if keep:
                out.extend(keep)
            out.append(BANNER.format(n=len(body)).rstrip("\n"))
            out.append(lines[j])
            cut += len(body)
            blocks += 1
        else:
            out.extend(lines[i:j + 1])
        i = j + 1
    return "\n".join(out), cut, blocks


def main():
    src = pathlib.Path(sys.argv[1])
    dst = pathlib.Path(sys.argv[2])
    dst.mkdir(parents=True, exist_ok=True)
    for p in sorted(src.glob("*.md")):
        t = p.read_text(encoding="utf8")
        nt, cut, blocks = trim(t)
        (dst / p.name).write_text(nt, encoding="utf8")
        print(f"{p.name}: {blocks} bloques recortados, {cut} lineas de codigo retiradas "
              f"({len(t)} -> {len(nt)} bytes)")


if __name__ == "__main__":
    main()

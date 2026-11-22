#!/usr/bin/env python3
"""Arregla las tablas Markdown de un informe: celdas duplicadas, celdas de más
y pipes sin escapar dentro de spans de código.

Por qué existe (medido el 2026-11-21/22): el informe de auditoría v15 arrastró
14 filas con la celda "Estado/Coste" **duplicada** (un merge de texto) ⇒ la
tabla se renderizaba con columnas de más ⇒ el defecto se perdía de vista. Y un
pipe sin escapar dentro de `` `codigo` `` **parte la celda** en Markdown.

Uso:
    tools/fix_tablas_markdown.py <fichero.md> [--check]

Salida: 0 = todo correcto (o se arregló); 1 = quedan tablas desalineadas.
"""
import re
import sys
from pathlib import Path


def n_fields(linea: str) -> int:
    return len(re.split(r"(?<!\\)\|", linea.strip()))


def norm(celda: str) -> str:
    return re.sub(r"\s+", " ", celda.replace("\\|", "|")).strip()


def escapar_pipes(linea: str) -> str:
    """Escapa los pipes NO escapados que caen dentro de un span de codigo.

    Regla precisa: un pipe esta dentro de un span si el numero de backticks
    que lo preceden en la fila es IMPAR (span abierto).
    """
    out = []
    bt = 0
    i = 0
    while i < len(linea):
        ch = linea[i]
        if ch == "\\" and i + 1 < len(linea) and linea[i + 1] == "|":
            out.append("\\|")
            i += 2
            continue
        if ch == "`":
            bt += 1
            out.append(ch)
            i += 1
            continue
        if ch == "|":
            out.append("\\|" if bt % 2 else "|")
            i += 1
            continue
        out.append(ch)
        i += 1
    return "".join(out)


def arreglar(texto: str):
    lineas = texto.split("\n")
    out = []
    i = 0
    reparadas = 0
    while i < len(lineas):
        l = lineas[i]
        if not l.strip().startswith("|"):
            out.append(l)
            i += 1
            continue
        # cabecera + separador?
        if i + 1 < len(lineas) and re.match(r"^\s*\|[\s:|-]+\|\s*$", lineas[i + 1].strip()):
            cab = n_fields(l)
            out.append(escapar_pipes(l))
            out.append(escapar_pipes(lineas[i + 1]))
            i += 2
            while i < len(lineas) and lineas[i].strip().startswith("|"):
                row = escapar_pipes(lineas[i])
                if n_fields(row) <= cab:
                    out.append(row)
                    i += 1
                    continue
                cells = re.split(r"(?<!\\)\|", row.strip())
                # 1) fusionar celdas duplicadas (comparando con pipes normalizados)
                cambio = True
                while cambio and len(cells) > cab:
                    cambio = False
                    for k in range(1, len(cells) - 2):
                        a, b = norm(cells[k]), norm(cells[k + 1])
                        if a and b and a[:50] == b[:50]:
                            cells[k:k + 2] = [
                                cells[k] if len(cells[k]) >= len(cells[k + 1]) else cells[k + 1]
                            ]
                            cambio = True
                            break
                # 2) si sobran, quitar celdas VACIAS (relleno)
                while len(cells) > cab:
                    idx = [k for k in range(1, len(cells) - 1) if norm(cells[k]) == ""]
                    if not idx:
                        break
                    del cells[idx[0]]
                out.append("| " + " | ".join(c.strip() for c in cells[1:-1]) + " |")
                reparadas += 1
                i += 1
            continue
        out.append(escapar_pipes(l))
        i += 1
    return "\n".join(out), reparadas


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    ruta = Path(sys.argv[1])
    solo_check = "--check" in sys.argv
    texto = ruta.read_text(errors="replace")
    nuevo, reparadas = arreglar(texto)
    if not solo_check and nuevo != texto:
        ruta.write_text(nuevo)
    # re-validacion
    ls = nuevo.split("\n")
    malas = []
    i = 0
    while i < len(ls):
        if ls[i].strip().startswith("|"):
            j = i
            bloque = []
            while j < len(ls) and ls[j].strip().startswith("|"):
                bloque.append(ls[j])
                j += 1
            if len(bloque) >= 2 and re.match(r"^\s*\|[\s:|-]+\|\s*$", bloque[1].strip()):
                cols = {n_fields(b) for b in bloque}
                if len(cols) > 1:
                    malas.append((i + 1, sorted(cols)))
            i = j
        else:
            i += 1
    print(f"filas reparadas: {reparadas} | tablas desalineadas: {len(malas)}")
    for m in malas[:6]:
        print("   linea", m[0], m[1])
    return 1 if malas else 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Resuelve los strings que referencia el pseudocodigo de Ghidra.

Ghidra carga este ELF con image base 0x100000 (medido), asi que las
direcciones `UNK_XXXXXXXX` son vaddr + 0x100000: hay que restarlo para
volver a la direccion real del binario.
"""
import io, os, re, sys, collections

BIN = "/workspace/agy-re/liblanguage_server.so"
IMAGE_BASE = 0x100000
SEGMENTOS = [
    (0x00000000, 0x779b960, 0x00000000),
    (0x0779b960, 0x1622c30, 0x0799b960),
    (0x08dbe590, 0x00cd080, 0x091be590),
    (0x08e8b640, 0x00eb030, 0x0948b640),
    (0x09019dd0, 0x1c71f58, 0x09819dd0),
    (0x0acd6298, 0x000d41c, 0x0d8d6298),
    (0x0acf0000, 0x00050d0, 0x0d8f0000),
]

def vaddr_a_offset(vaddr):
    for paddr, size, va in SEGMENTOS:
        if va <= vaddr < va + size:
            return paddr + (vaddr - va)
    return None

_cache = {}
def leer(vaddr, n):
    if (vaddr, n) in _cache:
        return _cache[(vaddr, n)]
    off = vaddr_a_offset(vaddr)
    b = None
    if off is not None:
        with open(BIN, "rb") as f:
            f.seek(off)
            b = f.read(n)
    _cache[(vaddr, n)] = b
    return b

def cadena(vaddr, longitud):
    if longitud is None:
        longitud = 64
    b = leer(vaddr, longitud + 2)
    if not b:
        return None
    fin = b.find(b"\x00")
    t = b[:fin if fin >= 0 else longitud].decode("utf-8", "replace")
    return t if t and all(32 <= ord(c) < 127 for c in t) else None

RE_UNK = re.compile(r'&UNK_([0-9a-fA-F]{8})')
RE_STRLIT = re.compile(r'"((?:[^"\\]|\\.){1,200})"')

def main(directorio, salida):
    os.makedirs(salida, exist_ok=True)
    resumen = []
    for fn in sorted(os.listdir(directorio)):
        if not fn.endswith(".c"):
            continue
        p = os.path.join(directorio, fn)
        txt = io.open(p, encoding="utf-8", errors="replace").read()
        # de UNK_XXXX followed by ,0xNN (longitud) => string
        out = txt
        usados = set()
        # 1) patron simple: &UNK_XXXX , 0xNN   (ptr, len)
        pares = {}
        for m in re.finditer(r'&UNK_([0-9a-fA-F]{8})\s*,\s*(0x[0-9a-fA-F]+|\d+)', txt):
            try:
                pares[m.group(0)] = int(m.group(2), 0)
            except ValueError:
                pass
        # 2) patron de llamada: func(...&UNK_XXXX,...,&UNK_YYYY,...,0xNN)
        #    => el puntero es el ULTIMO &UNK_ y la longitud el ultimo argumento numerico
        for cm in re.finditer(r'func_0x[0-9a-fA-F]+\(([^()]*)\)', txt):
            args = [a.strip() for a in cm.group(1).split(",")]
            if len(args) < 2:
                continue
            ultimo = None
            for a in args:
                um = RE_UNK.fullmatch(a)
                if um:
                    ultimo = a
            if ultimo is None:
                continue
            lm = re.fullmatch(r'(0x[0-9a-fA-F]+|\d+)', args[-1])
            if not lm:
                continue
            try:
                n = int(lm.group(1), 0)
            except ValueError:
                continue
            if 0 < n <= 400:
                pares[ultimo] = n
        for ref, longitud in pares.items():
            m = re.fullmatch(r'&UNK_([0-9a-fA-F]{8})', ref)
            if not m:
                continue
            real = int(m.group(1), 16) - IMAGE_BASE
            s = cadena(real, longitud)
            if s is None:
                continue
            usados.add(s)
            out = out.replace(ref, '"%s"' % s.replace('"', '\\"'))
        destino = os.path.join(salida, fn)
        io.open(destino, "w", encoding="utf-8").write(out)
        # strings literales que ya venue en el codigo
        literales = sorted(set(RE_STRLIT.findall(txt)))
        resumen.append((fn, sorted(usados), literales))
    # indice de todos los strings
    idx = ["# Strings recuperados del pseudocodigo de Ghidra\n"]
    for fn, usados, literales in resumen:
        idx.append(f"\n## {fn}\n")
        if usados:
            idx.append("Referenciados por dirección:\n")
            for s in usados:
                idx.append(f"- `{s}`")
        if literales:
            idx.append("\nLiterales en el código:\n")
            for s in literales:
                idx.append(f"- `{s}`")
    io.open(os.path.join(salida, "_STRINGS.md"), "w", encoding="utf-8").write("\n".join(idx))
    total = sum(len(u) for _, u, _ in resumen)
    print("ficheros:", len(resumen), "· strings direccionados:", total)

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])

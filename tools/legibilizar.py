#!/usr/bin/env python3
"""Convierte un desensamblado ARM64 de rizin en CODIGO LEGIBLE.

Resuelve, linea a linea:
  * el destino de cada llamada  -> nombre de la funcion Go (pclntab),
  * cada referencia adrp/add/ldr/str -> el texto de la cadena en el binario.

Con eso el listing pasa de "instrucciones" a "lo que hace el codigo".
"""
import io, re, sys, collections

TSV = "/workspace/agy-re/tools-out/go.funcs.tsv"
STRINGS = "/workspace/md-resc/artifacts/re/strings.txt"

def cargar_simbolos():
    m = {}
    for l in io.open(TSV, encoding='utf-8', errors='replace'):
        p = l.rstrip('\n').split('\t')
        if len(p) < 2:
            continue
        try:
            a = int(p[0], 16)
        except ValueError:
            continue
        sym = p[1]
        # nombre corto estilo Go: pkg.Func
        n = sym.split('__')[-1] if '__' in sym else sym
        if '.' in n:
            n = n.split('.')[-1]
        m[a] = n
    return m

# Segmentos (MEDIDOS con rz-bin -SS): paddr -> vaddr
SEGMENTOS = [
    (0x00000000, 0x779b960, 0x00000000),   # LOAD0 r-x (codigo): vaddr == paddr
    (0x0779b960, 0x1622c30, 0x0799b960),   # LOAD1 rw-
    (0x08dbe590, 0x00cd080, 0x091be590),   # LOAD2 r--
    (0x08e8b640, 0x00eb030, 0x0948b640),   # LOAD3 rw-
    (0x09019dd0, 0x1c71f58, 0x09819dd0),   # LOAD4 rw-
    (0x0acd6298, 0x000d41c, 0x0d8d6298),   # LOAD5 r--
    (0x0acf0000, 0x00050d0, 0x0d8f0000),   # LOAD6 rw-
]

def offset_a_vaddr(off):
    for paddr, size, vaddr in SEGMENTOS:
        if paddr <= off < paddr + size:
            return vaddr + (off - paddr)
    return None

BIN = "/workspace/agy-re/liblanguage_server.so"

def vaddr_a_offset(vaddr):
    for paddr, size, va in SEGMENTOS:
        if va <= vaddr < va + size:
            return paddr + (vaddr - va)
    return None

def leer_bytes(vaddr, n):
    off = vaddr_a_offset(vaddr)
    if off is None:
        return None
    with open(BIN, "rb") as f:
        f.seek(off)
        return f.read(n)

def cadena_en(vaddr, prof=0):
    """String en una direccion; si es un puntero, lo sigue (1-2 niveles)."""
    if prof > 2:
        return None
    b = leer_bytes(vaddr, 256)
    if not b:
        return None
    if all(32 <= c < 127 or c in (9, 10, 13) for c in b[:1]) and b[0] and b[0] < 128:
        fin = b.find(b"\x00")
        t = b[:fin if fin >= 0 else 80].decode("utf-8", "replace")
        if t and len(t) > 1 and all(32 <= ord(c) < 127 or c in (9, 10) for c in t):
            return t
    # puntero
    if len(b) >= 8:
        p = int.from_bytes(b[:8], "little")
        if p > 0x1000 and p < 0x10000000:
            return cadena_en(p, prof + 1)
    return None

def cargar_strings():
    """Devuelve {vaddr: texto} (los offsets de `strings` son de FICHERO)."""
    m = {}
    for l in io.open(STRINGS, encoding='utf-8', errors='replace'):
        off, _, txt = l.rstrip('\n').partition(' ')
        try:
            o = int(off, 16)
        except ValueError:
            continue
        v = offset_a_vaddr(o)
        if v is not None:
            m[v] = txt
    return m

RE_ADRP = re.compile(r'adrp\s+(x\d+|w\d+|sp),\s*0x([0-9a-f]+)')
RE_ADD  = re.compile(r'add\s+(x\d+),\s*(x\d+),\s*0x([0-9a-f]+)')
RE_LDR  = re.compile(r'ldr\s+(x\d+),\s*\[(x\d+)(?:,\s*0x([0-9a-f]+))?\]')
RE_STR  = re.compile(r'str\s+(x\d+),\s*\[(x\d+)(?:,\s*0x([0-9a-f]+))?\]')
RE_ADR  = re.compile(r'\badr\s+(x\d+),\s*0x([0-9a-f]+)')
RE_BL   = re.compile(r'\bbl\s+0x([0-9a-f]+)')
RE_B    = re.compile(r'\bb\s+0x([0-9a-f]+)')
RE_CBZ  = re.compile(r'\b(cbz|cbnz|tbz|tbnz)\s+(w\d+|x\d+),\s*0x([0-9a-f]+)')

def legibilizar(asm_path, syms, strings, out_path, titulo):
    reg = {}
    out = [f"# {titulo}", ""]
    cur = None
    for raw in io.open(asm_path, encoding='utf-8', errors='replace'):
        line = raw.rstrip('\n')
        if line.startswith('==== '):
            cur = line[5:]
            out.append("")
            out.append(f"## {cur}")
            out.append("")
            out.append("```asm")
            reg = {}
            continue
        if line.startswith('/') or not line.strip():
            continue
        # rizin imprime: '|       ,=< 0x06d92368      cbz   w3, 0x6d92378'
        norm = line.replace('|', ' ').replace(',=<', ' ').replace('`->', ' ')
        norm = norm.replace('`', ' ').replace(';', ' ;')
        m = re.match(r'\s*(0x[0-9a-f]+)\s+(.*)$', norm)
        if not m:
            continue
        addr_s, ins = m.group(1), m.group(2)
        notas = []

        ma = RE_ADRP.search(ins)
        if ma:
            reg[ma.group(1)] = int(ma.group(2), 16)
        mad = RE_ADR.search(ins)
        if mad:
            a = int(mad.group(2), 16)
            txt = strings.get(a) or cadena_en(a)
            if txt:
                notas.append(f'"{txt[:140]}"')
            reg[mad.group(1)] = a
        madd = RE_ADD.search(ins)
        if madd:
            dst, src, off = madd.group(1), madd.group(2), int(madd.group(3), 16)
            if src in reg:
                a = reg[src] + off
                reg[dst] = a
                txt = strings.get(a) or cadena_en(a)
                if txt:
                    notas.append(f'"{txt[:140]}"')
        for rex in (RE_LDR, RE_STR):
            mm = rex.search(ins)
            if mm:
                dst, src = mm.group(1), mm.group(2)
                off = int(mm.group(3), 16) if mm.group(3) else 0
                if src in reg:
                    a = reg[src] + off
                    reg[dst] = a
                    txt = strings.get(a) or cadena_en(a)
                    if txt:
                        notas.append(f'"{txt[:140]}"')
        mbl = RE_BL.search(ins)
        if mbl:
            a = int(mbl.group(1), 16)
            if a in syms:
                notas.append(syms[a])
        mb = RE_B.search(ins)
        if mb and 'bl' not in ins:
            a = int(mb.group(1), 16)
            if a in syms:
                notas.append(syms[a])
        mcb = RE_CBZ.search(ins)
        if mcb:
            a = int(mcb.group(3), 16)
            if a in syms:
                notas.append(syms[a])

        if notas:
            out.append(f"{addr_s}  {ins:<40} ; {'  '.join(dict.fromkeys(notas))}")
        else:
            out.append(f"{addr_s}  {ins}")
        if out[-2:-1] == ['```'] or (len(out) > 1 and out[-2].strip() == '```'):
            pass
    # cierra los bloques
    texto = "\n".join(out)
    texto = texto.replace("```asm\n", "```asm\n")
    # inserta cierre antes de cada "## "
    lineas = texto.split("\n")
    fin = []
    dentro = False
    for l in lineas:
        if l.startswith("## ") and dentro:
            fin.append("```")
            dentro = False
        if l.strip() == "```asm":
            dentro = True
        fin.append(l)
    if dentro:
        fin.append("```")
    io.open(out_path, 'w', encoding='utf-8').write("\n".join(fin) + "\n")
    return len(fin)

if __name__ == '__main__':
    syms = cargar_simbolos()
    strings = cargar_strings()
    print("simbolos:", len(syms), "strings:", len(strings))
    n = legibilizar(sys.argv[1], syms, strings, sys.argv[2],
                   sys.argv[3] if len(sys.argv) > 3 else "Codigo legible")
    print("lineas:", n)

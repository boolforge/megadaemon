#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""ghidra-resume.py — REANUDA la decompilacion de Ghidra donde se quedo.

QUE HACE
  1. Calcula que funciones del maestro (`go.funcs.real.tsv`) NO estan
     todavia en el arbol de pseudocodigo (`todo/s{0,1,2}/`), comparando por
     NOMBRE de simbolo Go (la clave que usan tanto el maestro como los
     bloques `/* ==== ... ==== */`).
  2. Reparte las que faltan en N workers (proyectos Ghidra existentes).
  3. Por cada worker, decompila en LOTES con `analyzeHeadless` +
     `GoAnchorDecompiler.java` (el proyecto ya esta analizado: `-noanalysis`).
  4. Convierte la salida POR FUNCION del Java al formato de BLOQUE del
     arbol y lo fusiona en el `.c` del fichero Go que le corresponde,
     ORDENADO por linea de origen.
  5. Actualiza el `_index.tsv` del shard (addr, name, file, line, bytes,
     ok, err, ms).

ES REANUDABLE
  Cada lote deja checkpoint (`_resume_state.json`): si se corta, se relanza
  y sigue por donde iba. Nunca se duplica un bloque ya presente.

USO
  ghidra-resume.py --plan                # solo el plan (no ejecuta nada)
  ghidra-resume.py --run --workers 2     # ejecuta
  ghidra-resume.py --run --workers 2 --batch 80
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time

ROOT = "/workspace/agy-re/tools-out"
MAESTRO = os.path.join(ROOT, "go.funcs.real.tsv")
TODO = os.path.join(ROOT, "todo")
# MEDIDO (2026-11-23): Ghidra canonicaliza la ruta del SCRIPT pero no la
# del directorio de scripts.  Si /workspace es un enlace a otro sitio
# (p.ej. /run/csi/mount-root/...), el script se busca en la ruta canónica y
# el directorio en la no canónica => «Failed to find script in any script
# directory» => 0 funciones convertidas y ningun error visible.
# Se canonicaliza TODO lo que se le pasa a Ghidra.
SCRIPTS = os.path.realpath(os.path.join(ROOT, "scripts"))
GHIDRA = os.path.join(ROOT, "ghidra_11.2.1_PUBLIC")
JAVA = os.path.join(ROOT, "jdk21")
BINARIO = "/workspace/agy-re/liblanguage_server.so"
PROYECTOS = [os.path.join(ROOT, "proj-w%d" % i) for i in (0, 1, 2)]
NOMBRE_PROY = "Go"
STATE = os.path.join(ROOT, "_resume_state.json")

RE_BLOQUE = re.compile(r"^/\* ==== (?P<nombre>.+?)  \[(?P<bytes>\d+) bytes\] ==== \*/\s*$", re.M)
RE_GO = re.compile(r"/\* go:\s*(?P<simbolo>[^\n]*)\n\s*(?P<fichero>[^\n:]*):(?P<linea>\d+) \*/")


# ------------------------------------------------------------------ maestro
def carga_maestro():
    """nombre Go -> (addr, simbolo, fichero, linea)."""
    out = {}
    with open(MAESTRO, encoding="utf-8", errors="replace") as fh:
        next(fh, None)
        for line in fh:
            p = line.rstrip("\n").split("\t")
            if len(p) < 3:
                continue
            addr, name, orig = p[0], p[1], p[2]
            if "|" not in orig:
                continue
            sig, _, loc = orig.partition(" | ")
            sig = sig[4:] if sig.startswith("go: ") else sig
            f, _, ln = loc.strip().rpartition(":")
            out[name] = (addr, sig, f.strip(), ln.strip())
    return out


def nombres_hechos():
    """Nombres Go ya presentes en el arbol de pseudocodigo."""
    hechos = set()
    for s in (0, 1, 2):
        base = os.path.join(TODO, "s%d" % s)
        if not os.path.isdir(base):
            continue
        for root, _, files in os.walk(base):
            for fn in files:
                if not fn.endswith(".c"):
                    continue
                try:
                    txt = open(os.path.join(root, fn), encoding="utf-8", errors="replace").read()
                except OSError:
                    continue
                for m in RE_BLOQUE.finditer(txt):
                    hechos.add(m.group("nombre").strip())
    return hechos


def calc_pendientes():
    maestro = carga_maestro()
    hechos = nombres_hechos()
    pend = []
    for name, (addr, sig, f, ln) in maestro.items():
        if name in hechos:
            continue
        pend.append({"addr": addr, "name": name, "sig": sig, "file": f, "line": ln})
    pend.sort(key=lambda r: r["name"])
    return maestro, pend


# ------------------------------------------------------------------ arbol
def parse_bloques(txt):
    """Trocea un .c del arbol en bloques.

    MEDIDO (2026-11-23) sobre 40 ficheros del arbol existente:
      * la cabecera acaba en ``\\n\\n``;
      * ENTRE bloques hay 4 saltos de linea (3 lineas vacias);
      * al final del fichero hay 3 saltos (2 lineas vacias).
    Se devuelven (cabecera, bloques); el ``texto`` de cada bloque NO lleva
    los saltos finales (se reconstruyen al escribir).
    """
    marcas = [(m.start(), m) for m in RE_BLOQUE.finditer(txt)]
    if not marcas:
        return txt, []
    cabecera = txt[: marcas[0][0]]
    bloques = []
    for i, (pos, m) in enumerate(marcas):
        fin = marcas[i + 1][0] if i + 1 < len(marcas) else len(txt)
        cuerpo = txt[pos:fin].rstrip("\n")
        # la linea de origen va en el comentario `/* go: ... */`
        ln = 0
        gm = RE_GO.search(cuerpo)
        if gm:
            try:
                ln = int(gm.group("linea"))
            except ValueError:
                ln = 0
        bloques.append({"linea": ln, "texto": cuerpo})
    return cabecera, bloques


def render(cabecera, bloques, tail="\n\n"):
    """Rehace el .c con la convencion exacta del arbol.

    `tail` son los saltos de linea FINALES del fichero: se preservan tal
    cual (medido: hay ficheros del arbol que acaban en 0 y otros en 3).
    """
    out = [cabecera]
    for k, b in enumerate(bloques):
        out.append(b["texto"])
        if k < len(bloques) - 1:
            out.append("\n\n\n")
    out.append(tail)
    return "".join(out)


def cola_final(txt):
    """Los saltos de linea del final del fichero (0, 1, 2 o 3)."""
    return txt[len(txt.rstrip("\n")):]


def ruta_de_destino(fichero_go, shard):
    """`<pkg>/<base>.go` -> `todo/s<shard>/<pkg>/<base>.c`."""
    rel = fichero_go
    if rel.endswith(".go"):
        rel = rel[:-3]
    return os.path.join(TODO, "s%d" % shard, rel + ".c")


def tag_seguro(fichero_go):
    """tag de un solo componente (lo usa el Java para el nombre del .c)."""
    return re.sub(r"[^A-Za-z0-9._-]", "_", fichero_go)


def fusiona(destino, bloque_texto, linea):
    """INSERTA un bloque en el .c destino, ordenado por linea de origen.

    No reescribe el fichero: empalma el bloque nuevo en su sitio y deja
    intactos los bytes del resto (solo toca los saltos de linea del borde
    de la insercion). Es lo que mantiene el arbol identico al original.
    """
    os.makedirs(os.path.dirname(destino), exist_ok=True)
    if os.path.exists(destino):
        txt = open(destino, encoding="utf-8", errors="replace").read()
    else:
        txt = ""
    if not txt:
        cabecera = "/* %s -- pseudocodigo decompilado por Ghidra (MegaDaemon) */\n\n" % (
            os.path.basename(destino)[:-2] + ".go"
        )
        nuevo = cabecera + bloque_texto.rstrip("\n") + "\n\n\n"
        tmp = destino + ".tmp"
        with open(tmp, "w", encoding="utf-8") as fh:
            fh.write(nuevo)
        os.replace(tmp, destino)
        return True
    cabecera, bloques = parse_bloques(txt)
    primero = bloque_texto.split("\n", 1)[0]
    for b in bloques:
        if b["texto"].split("\n", 1)[0] == primero:
            return False  # ya estaba
    # punto de insercion: antes del primer bloque con linea mayor
    marcas = list(RE_BLOQUE.finditer(txt))
    pos = None
    for i, m in enumerate(marcas):
        cuerpo = txt[m.start(): (marcas[i + 1].start() if i + 1 < len(marcas) else len(txt))]
        gm = RE_GO.search(cuerpo)
        ln = 0
        if gm:
            try:
                ln = int(gm.group("linea"))
            except ValueError:
                ln = 0
        if ln > linea:
            pos = m.start()
            break
    bloque = bloque_texto.rstrip("\n")
    if pos is None:
        # al final: se respeta la cola de saltos del fichero original
        cola = cola_final(txt)
        nuevo = txt.rstrip("\n") + "\n\n\n" + bloque + cola
    else:
        head = txt[:pos].rstrip("\n")
        nuevo = head + "\n\n\n" + bloque + "\n\n\n" + txt[pos:]
    tmp = destino + ".tmp"
    with open(tmp, "w", encoding="utf-8") as fh:
        fh.write(nuevo)
    os.replace(tmp, destino)
    return True


# ------------------------------------------------------------------ conversion
def convierte_bloque(ruta_java):
    """Salida por funcion del Java -> bloque del arbol (o None)."""
    with open(ruta_java, encoding="utf-8", errors="replace") as fh:
        lineas = fh.read().split("\n")
    if not lineas:
        return None
    if not lineas[0].startswith("// "):
        return None
    partes = lineas[0][3:].split("  ", 1)
    if len(partes) != 2:
        return None
    nombre = partes[1].strip()
    sig = ""
    nbytes = 0
    i = 1
    while i < len(lineas) and lineas[i].startswith("//"):
        if lineas[i].startswith("// signature          : "):
            sig = lineas[i].split(": ", 1)[1].strip()
        elif lineas[i].startswith("// body instructions  : "):
            try:
                nbytes = int(lineas[i].split(": ", 1)[1].strip())
            except ValueError:
                nbytes = 0
        i += 1
    ini = None
    for j in range(i, len(lineas)):
        if lineas[j].startswith("/* go:"):
            ini = j
            break
    if ini is None:
        return None
    fin = len(lineas)
    for j in range(ini, len(lineas)):
        if lineas[j].startswith("// ---- direct call targets") or \
           lineas[j].startswith("// ---- string literals"):
            fin = j
            break
    cuerpo = lineas[ini:fin]
    while cuerpo and cuerpo[-1].strip() == "":
        cuerpo.pop()
    sal = ["/* ==== %s  [%d bytes] ==== */" % (nombre, nbytes)]
    if sig:
        sal.append(sig)
    sal.append("")
    sal.extend(cuerpo)
    return "\n".join(sal) + "\n"


# ------------------------------------------------------------------ ghidra
def run_lote(proyecto, lote_tsv, out_dir, report_tsv, symbols_tsv, maxmem):
    """Ejecuta analyzeHeadless para un lote. Devuelve (rc, elapsed)."""
    env = dict(os.environ)
    env["JAVA_HOME"] = JAVA
    env["MAXMEM"] = maxmem
    env["PATH"] = JAVA + "/bin:" + env.get("PATH", "")
    cmd = [
        os.path.join(GHIDRA, "support", "analyzeHeadless"),
        proyecto, NOMBRE_PROY,
        "-process", os.path.basename(BINARIO),
        "-noanalysis",
        "-scriptPath", SCRIPTS,
        "-postScript", "GoAnchorDecompiler.java",
        "decompile", symbols_tsv, lote_tsv, out_dir, "", report_tsv,
    ]
    t0 = time.time()
    with open(report_tsv + ".log", "w") as log:
        rc = subprocess.call(cmd, stdout=log, stderr=subprocess.STDOUT, env=env)
    return rc, time.time() - t0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", action="store_true")
    ap.add_argument("--plan", action="store_true")
    ap.add_argument("--workers", type=int, default=2)
    ap.add_argument("--batch", type=int, default=80)
    ap.add_argument("--maxmem", default="900M")
    ap.add_argument("--symbols", default=os.path.join(ROOT, "go.funcs.tsv"))
    ap.add_argument("--limit", type=int, default=0, help="prueba: como mucho N funciones")
    ap.add_argument("--state", default=STATE, help="fichero de checkpoint (uno por worker)")
    ap.add_argument("--proyecto", default="", help="proyecto Ghidra de este worker")
    ap.add_argument("--frac", type=int, default=0, help="este worker toma la fraccion N de los lotes")
    ap.add_argument("--fracs", type=int, default=1, help="cuantos workers hay en total")
    ap.add_argument("--glob-estados", default="", help="patron de checkpoints de los OTROS workers (se saltan sus funciones)")
    args = ap.parse_args()

    maestro, pend = calc_pendientes()
    print("maestro: %d funciones | ya en el arbol: %d | PENDIENTES: %d"
          % (len(maestro), len(maestro) - len(pend), len(pend)))
    if not args.run:
        for i, p in enumerate(pend[:10]):
            print("  ejemplo:", p["addr"], p["name"], p["file"] + ":" + p["line"])
        print("... (plan: %d workers, lotes de %d, MAXMEM=%s)" % (args.workers, args.batch, args.maxmem))
        return 0

    if len(pend) == 0:
        print("no queda nada por decompilar")
        return 0

    state = {"hechos": {}, "lote": 0}
    if os.path.exists(args.state):
        try:
            state = json.load(open(args.state))
        except Exception:
            pass
    state.setdefault("hechos", {})
    state.setdefault("lote", 0)

    # reparte en workers
    # Los workers comparten eltrabajo: lo que otro worker ya ha hecho se
    # salta (asi un reinicio no vuelve a decompilar lo mismo).
    saltados = set(state.get("hechos", {}).keys())
    if args.glob_estados:
        import glob as _glob
        for f in _glob.glob(args.glob_estados):
            if os.path.abspath(f) == os.path.abspath(args.state):
                continue
            try:
                otro = json.load(open(f))
            except Exception:
                continue
            for k in otro.get("hechos", {}):
                saltados.add(k)
    if saltados:
        pend = [p for p in pend if p["name"] not in saltados]
    if args.limit:
        pend = pend[: args.limit]
    print("pendientes tras el checkpoint: %d" % len(pend))
    if args.fracs > 1:
        # este worker toma SU fraccion (los workers corren en paralelo)
        shards = [[p for i, p in enumerate(pend) if i % args.fracs == args.frac]]
    else:
        shards = [[] for _ in range(args.workers)]
        for i, p in enumerate(pend):
            shards[i % args.workers].append(p)

    lotes = []
    for w, sh in enumerate(shards):
        for b in range(0, len(sh), args.batch):
            lotes.append((w, sh[b:b + args.batch]))

    out_base = os.path.join(ROOT, "resume-out")
    os.makedirs(out_base, exist_ok=True)

    for w, lote in lotes:
        n = state["lote"]
        state["lote"] = n + 1
        lote_tsv = os.path.join(out_base, "lote_%04d.tsv" % n)
        with open(lote_tsv, "w", encoding="utf-8") as fh:
            for p in lote:
                fh.write("%s\t%s\t%s\t%s:%s\t%s\n" % (
                    p["addr"], p["name"], p["sig"], p["file"], p["line"], tag_seguro(p["file"])))
        out_dir = os.path.join(out_base, "out_%04d" % n)
        os.makedirs(out_dir, exist_ok=True)
        report = os.path.join(out_base, "report_%04d.tsv" % n)
        proyecto = args.proyecto if args.proyecto else PROYECTOS[w % len(PROYECTOS)]
        print("[lote %d] worker %d, %d funciones, proyecto %s" % (n, w, len(lote), proyecto), flush=True)
        rc, elapsed = run_lote(proyecto, lote_tsv, out_dir, report, args.symbols, args.maxmem)
        print("   rc=%d elapsed=%.0fs" % (rc, elapsed), flush=True)
        # convierte y fusiona
        convertidos = 0
        for p in lote:
            # el Java nombra el .c como <tag>__<nombre saneado>.c
            seguro = re.sub(r"[^A-Za-z0-9._-]", "_", p["name"])[:150]
            cand = os.path.join(out_dir, "%s__%s.c" % (tag_seguro(p["file"]), seguro))
            if not os.path.exists(cand):
                cand = None
                for fn in os.listdir(out_dir):
                    if fn.endswith(".c") and seguro in fn:
                        cand = os.path.join(out_dir, fn)
                        break
            if not cand:
                state["hechos"][p["name"]] = "sin-fichero"
                continue
            bloque = convierte_bloque(cand)
            if not bloque:
                state["hechos"][p["name"]] = "no-convertible"
                continue
            destino = ruta_de_destino(p["file"], args.frac % 3)
            fusiona(destino, bloque, int(p["line"] or 0))
            state["hechos"][p["name"]] = "ok"
            convertidos += 1
        # actualiza el _index.tsv del shard
        idx = os.path.join(TODO, "s%d" % (args.frac % 3), "_index.tsv")
        with open(idx, "a", encoding="utf-8") as fh:
            for p in lote:
                fh.write("%s\t%s\t%s\t%s\t0\t%s\t\t0\n" % (
                    p["addr"].replace("0x", ""), p["name"], p["file"], p["line"],
                    state["hechos"].get(p["name"], "?")))
        print("   convertidos: %d" % convertidos, flush=True)
        with open(args.state, "w") as fh:
            json.dump(state, fh)
    with open(args.state, "w") as fh:
        json.dump(state, fh)
    print("hecho. %d funciones" % len(state["hechos"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())

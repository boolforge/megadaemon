#!/usr/bin/env python3
"""Re-verifica las citas `fichero.rs:linea` de un informe de auditoría contra el
código ACTUAL de un repo, sin fiarse de lo que el informe diga.

Uso:  verificar_citas_audit.py <informe.md> <raiz-del-repo> [--contexto 40]

Qué hace, por cada cita del informe:
  1. resuelve la ruta real del fichero en el árbol actual;
  2. saca el identificador de código citado entre comillas invertidas (p.ej.
     `local_browser_request`, `requires_auth`);
  3. mira si ese identificador sigue existiendo **cerca** de la línea citada
     (± N líneas):  OK = la cita sigue valiendo;  DESPLAZADO = el código sigue
     pero en otra línea (el informe cites una línea antigua);  AUSENTE = el
     identificador ya no está en el fichero;  FICHERO-NO-EXISTE.

No juzga si el defecto sigue vivo: eso lo mide el arnés de tests. Esto sólo
dice si la REFERENCIA del informe sigue siendo localizable.
"""
import re
import sys
from pathlib import Path

CITA = re.compile(
    r"(?P<file>(?:[a-z_0-9]+/)*[a-z_0-9]+\.rs):(?P<line>\d+)(?:-(?P<line2>\d+))?"
)
IDENT = re.compile(r"`([^`]{2,80})`")


def resolve(repo: Path, name: str) -> Path | None:
    name = name.replace("core/src/", "")
    cands = [
        repo / "rust-core" / "src" / name,
        repo / "rust-core" / "tests" / name,
        repo / "src" / name,
        repo / name,
    ]
    for c in cands:
        if c.exists():
            return c
    return None


def main() -> int:
    informe = Path(sys.argv[1])
    repo = Path(sys.argv[2])
    ctx = 40
    if "--contexto" in sys.argv:
        ctx = int(sys.argv[sys.argv.index("--contexto") + 1])

    texto = informe.read_text(errors="replace")
    lineas = texto.split("\n")

    # cada cita con el "fichero vigente" (para las citas tipo ":231")
    citas = []
    fichero_vigente = None
    for i, ln in enumerate(lineas):
        for m in CITA.finditer(ln):
            fichero_vigente = m.group("file")
            citas.append((i + 1, fichero_vigente, int(m.group("line")), m.group("line2")))
        # citas heredadas ":231" dentro de la misma celda
        for m in re.finditer(r"(?<![a-z_0-9/])`:(\d+)(?:-(?P<l2>\d+))?`", ln):
            if fichero_vigente:
                citas.append((i + 1, fichero_vigente, int(m.group(1)), m.group("l2")))

    # identificador citado: el primero entre backticks DESPUÉS de la cita en la
    # misma línea (si no, el de las 200 líneas siguientes de la fila de tabla)
    filas: dict[int, list] = {}
    for doc_line, f, l1, l2 in citas:
        ident = None
        for j in range(doc_line - 1, min(doc_line + 3, len(lineas))):
            for m in IDENT.finditer(lineas[j]):
                cand = m.group(1)
                if re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*(\(\))?", cand) and len(cand) > 3:
                    ident = cand.rstrip("()")
                    break
            if ident:
                break
        filas.setdefault((f, l1, l2), (doc_line, ident))

    ok, desplaz, ausentes, nofich, sinident = [], [], [], [], []
    for (f, l1, l2), (doc_line, ident) in sorted(filas.items(), key=lambda kv: (kv[0][0], kv[0][1])):
        ruta = resolve(repo, f)
        if ruta is None:
            nofich.append((f, l1, doc_line, ident))
            continue
        src = ruta.read_text(errors="replace").split("\n")
        if not ident:
            sinident.append((f, l1, doc_line))
            continue
        lo = max(0, l1 - 1 - ctx)
        hi = min(len(src), l1 + ctx)
        ventana = "\n".join(src[lo:hi])
        if ident in ventana:
            ok.append((f, l1, doc_line, ident))
            continue
        if any(ident in ln for ln in src):
            real = next(i + 1 for i, ln in enumerate(src) if ident in ln)
            desplaz.append((f, l1, real, doc_line, ident))
        else:
            ausentes.append((f, l1, doc_line, ident))

    # --- modo refrescar: reescribe las citas desplazadas en el informe ---
    if "--refrescar" in sys.argv:
        nuevo = texto
        cambios = 0
        for (f, l1, l2), (doc_line, ident) in filas.items():
            if not ident:
                continue
            ruta = resolve(repo, f)
            if ruta is None:
                continue
            src = ruta.read_text(errors="replace").split("\n")
            hits = [i + 1 for i, ln in enumerate(src) if ident in ln]
            if not hits:
                continue
            real = min(hits, key=lambda h: abs(h - l1))
            if real == l1:
                continue
            # reescribir SOLO la primera aparicion de esa cita en el informe
            patron = re.compile(re.escape(f) + ":" + str(l1) + (r"-" + str(l2) if l2 else ""))
            m = patron.search(nuevo)
            if not m:
                continue
            ancho = (int(l2) - int(l1)) if l2 else 0
            nuevo_txt = f + ":" + str(real) + (("-" + str(real + ancho)) if l2 else "")
            nuevo = nuevo[:m.start()] + nuevo_txt + nuevo[m.end():]
            cambios += 1
        if cambios:
            informe.write_text(nuevo)
        print(f"refrescadas: {cambios} citas")
        return 0

    print(f"citas analizadas: {len(filas)}")
    print(f"  OK (la referencia sigue valiendo)   : {len(ok)}")
    print(f"  DESPLAZADO (el código existe en otra línea): {len(desplaz)}")
    print(f"  AUSENTE (el identificador no está)   : {len(ausentes)}")
    print(f"  FICHERO NO EXISTE                    : {len(nofich)}")
    print(f"  sin identificador extraíble         : {len(sinident)}")
    if desplaz:
        print("\n== DESPLAZADOS ==")
        for f, l1, real, doc_line, ident in desplaz:
            print(f"  {f}:{l1} -> ahora {real}  [{ident}]  (informe linea {doc_line})")
    if ausentes:
        print("\n== AUSENTES ==")
        for f, l1, doc_line, ident in ausentes:
            print(f"  {f}:{l1}  [{ident}]  (informe linea {doc_line})")
    if nofich:
        print("\n== FICHERO NO EXISTE ==")
        for f, l1, doc_line, ident in nofich:
            print(f"  {f}:{l1}  [{ident}]  (informe linea {doc_line})")
    return 0


if __name__ == "__main__":
    sys.exit(main())

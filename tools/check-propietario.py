#!/usr/bin/env python3
"""Gate de PROPIEDAD INTELECTUAL: documentar sí, copiar no.

MEDIDO (2026-11-28): el repo contiene material de TERCEROS (el esquema ACP
vendorizado, que es Apache-2.0, y los contratos de productos, que son
proprietarios).  La regla del proyecto es:

  * lo PROPIETARIO (cognition.ai/*, mcode/*, acpira/*) se **DOCUMENTA**;
  * lo que tiene licencia compatible (el crate ACP, Apache-2.0) se puede
    **vendorizar**, pero con atribución;
  * lo que se "rescata" se re-expresa **funcionalmente equivalente** (AST ->
    Rust), nunca se transcribe.

Este script convierte la regla en un gate.  Checks:

  1. `artifacts/contratos-productos-2026-11-27/` NO debe tener `.md`
     versionados (son extractos de productos propietarios: posible copia
     literal).  Sólo se versionan los inventarios.
  2. Ningún `.rs` de TERCEROS debe estar versionado en el repo.
  3. El módulo vendorizado del esquema ACP debe existir, con su cabecera de
     atribución, y ser el ÚNICO fichero `.rs` que menciona el crate
     original (si no, el código vendorizado se ha duplicado sin
     atribución).
  4. Las dos fichas de licencia/atribución deben estar presentes.
  5. Si se apunta un directorio de fuentes de terceros
     (`MD_TERCEROS_DIR`), se busca una copia LITERAL de bloques de ≥ 12
     líneas entre esas fuentes y nuestro código.

Salida: código 0 si todo bien, 1 si hay un fallo, 2 si el uso es incorrecto.

Uso:
    python3 tools/check-propietario.py
    MD_TERCEROS_DIR=/ruta/a/terceros python3 tools/check-propietario.py
"""

import os
import re
import subprocess
import sys

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CARPETA_RIESGO = "artifacts/contratos-productos-2026-11-27"
VENDOR_ACP = "rust-core/src/acp_schema.rs"
MARCADOR_UPSTREAM = "agent-client-protocol-schema"
ATRIBUCIONES = ("docs/ACP_VENDOR.md", "docs/ACP_VENDOR_LICENSE.md")
MIN_LINEAS_COPIA = 12
# Ruido: líneas que todo el mundo escribe y que no prueban copia.
RUIDO = re.compile(r"^\s*(\}|\{|\)|\(|\]|;|//.*|/\*.*|\*.*)$")


def versionados():
    """Ficheros versionados por git (nada de `git add -A`)."""
    try:
        out = subprocess.run(
            ["git", "ls-files"],
            cwd=RAIZ,
            capture_output=True,
            text=True,
            timeout=60,
            check=True,
        )
    except (OSError, subprocess.SubprocessError) as e:
        print("::error::no se pudo ejecutar 'git ls-files': {}".format(e))
        sys.exit(2)
    return out.stdout.split("\n")


def fail(msg, detalle=None):
    print("::error::{}".format(msg))
    if detalle:
        for d in detalle:
            print("    {}".format(d))
    return 1


def normaliza(linea):
    """Normaliza una línea para comparar.

    MEDIDO (2026-11-28): la primera versión devolvía "" para las líneas de
    comentario ⇒ una copia literal se rompía en rachas de 8 líneas (justo
    donde empieza el comentario) y NO se detectaba.  Una copia literal
    INCLUYE los comentarios, así que aquí se comparan también (sólo se
    ignoran las líneas vacías).
    """
    s = linea.strip()
    if not s:
        return ""
    s = re.sub(r"\s+", " ", s)
    return s


def bloques_coincidentes(nuestro_dir, terceros_dir):
    """Rachas de ≥ MIN_LINEAS_COPIA líneas iguales y consecutivas.

    MEDIDO (2026-11-28): la primera versión excluía las líneas
    estructurales (`}`, `);`…) del índice ⇒ rompía las rachas en cada
    cierre de bloque y NO detectaba una copia literal.  Ahora el índice
    incluye TODAS las líneas no vacías y lo que se exige es que la racha
    tenga ≥ MIN_LINEAS_COPIA líneas consecutivas Y ≥ 6 líneas "con
    contenido" (las estructurales no cuentan para el mínimo de contenido,
    pero sí para la continuidad).
    """
    # Índice: línea normalizada -> {(fichero, nº de línea)}
    indice = {}
    for raiz, dirs, files in os.walk(terceros_dir):
        dirs[:] = [d for d in dirs if d not in ("target", ".git")]
        for f in files:
            if not f.endswith(".rs"):
                continue
            p = os.path.join(raiz, f)
            try:
                lineas = open(p, encoding="utf-8", errors="replace").read().split("\n")
            except OSError:
                continue
            for i, l in enumerate(lineas):
                n = normaliza(l)
                if not n:
                    continue
                indice.setdefault(n, set()).add((p, i))
    if not indice:
        return []

    hallazgos = []
    for raiz, dirs, files in os.walk(nuestro_dir):
        dirs[:] = [d for d in dirs if d not in ("target", ".git")]
        for f in files:
            if not f.endswith(".rs"):
                continue
            p = os.path.join(raiz, f)
            lineas = open(p, encoding="utf-8", errors="replace").read().split("\n")
            normalizadas = [normaliza(l) for l in lineas]
            i = 0
            while i < len(lineas):
                n = normalizadas[i]
                if not n:
                    i += 1
                    continue
                cands = indice.get(n, set())
                mejor = None
                for (tp, ti) in cands:
                    k = 0
                    while i + k < len(lineas) and k < 400:
                        nk = normalizadas[i + k]
                        if not nk:
                            break
                        if not any(
                            tp == q[0] and q[1] == ti + k for q in indice.get(nk, ())
                        ):
                            break
                        k += 1
                    if mejor is None or k > mejor[0]:
                        mejor = (k, tp, ti)
                if mejor and mejor[0] >= MIN_LINEAS_COPIA:
                    # ¿Cuántas de esas líneas tienen "contenido"?
                    con_contenido = sum(
                        1
                        for j in range(i, i + mejor[0])
                        if not RUIDO.match(lineas[j])
                    )
                    if con_contenido >= 6:
                        hallazgos.append(
                            (
                                os.path.relpath(p, RAIZ),
                                os.path.relpath(mejor[1], RAIZ),
                                mejor[0],
                                i + 1,
                            )
                        )
                        i += mejor[0]
                        continue
                i += 1
    return hallazgos


def main():
    errores = 0
    ficheros = versionados()

    # 1. Los contratos de productos propietarios no se versionan.
    md_riesgo = [
        f
        for f in ficheros
        if f.startswith(CARPETA_RIESGO + "/") and f.endswith(".md")
    ]
    if md_riesgo:
        errores += fail(
            "hay .md de contratos de TERCEROS versionados (posible copia literal)",
            md_riesgo,
        )

    # 2. Ningún .rs de terceros versionado.
    rs_terceros = [f for f in ficheros if f.startswith("artifacts/") and f.endswith(".rs")]
    if rs_terceros:
        errores += fail(
            "hay codigo .rs de TERCEROS versionado (debe quedar en Drive o en artifacts/ sin versionar)",
            rs_terceros,
        )

    # 3. El módulo vendorizado existe y es el único que menciona el upstream.
    if VENDOR_ACP not in ficheros:
        errores += fail("falta el módulo vendorizado {}".format(VENDOR_ACP))
    else:
        cab = os.path.join(RAIZ, VENDOR_ACP)
        texto = open(cab, encoding="utf-8", errors="replace").read()
        if MARCADOR_UPSTREAM not in texto:
            errores += fail(
                "{} no menciona el crate de origen ({}) => falta la atribución".format(
                    VENDOR_ACP, MARCADOR_UPSTREAM
                )
            )
        otros = []
        for f in ficheros:
            if not f.endswith(".rs") or f == VENDOR_ACP:
                continue
            p = os.path.join(RAIZ, f)
            try:
                t = open(p, encoding="utf-8", errors="replace").read()
            except OSError:
                continue
            if MARCADOR_UPSTREAM not in t:
                continue
            # Una MENCIÓN en un comentario es atribución (legítima); lo que
            # no vale es que el código vendorizado esté en otro fichero.
            for i, l in enumerate(t.split("\n"), 1):
                if MARCADOR_UPSTREAM not in l:
                    continue
                s = l.strip()
                if s.startswith("//") or s.startswith("*") or s.startswith("#["):
                    continue  # comentario/atributo: atribución
                otros.append("{}:{} (código, no comentario)".format(f, i))
        if otros:
            errores += fail(
                "el código vendorizado aparece en OTROS ficheros (atribución duplicada o copia)",
                otros,
            )

    # 4. Las fichas de licencia y atribución.
    for a in ATRIBUCIONES:
        if a not in ficheros:
            errores += fail("falta la ficha de {}".format(a))

    # 5. Copia literal contra un directorio de terceros (opcional).
    terceros = os.environ.get("MD_TERCEROS_DIR", "").strip()
    if terceros:
        if not os.path.isdir(terceros):
            print("::error::MD_TERCEROS_DIR no es un directorio: {}".format(terceros))
            return 2
        print("::notice::buscando copias literales >= {} lineas contra {}".format(
            MIN_LINEAS_COPIA, terceros))
        h = bloques_coincidentes(os.path.join(RAIZ, "rust-core"), terceros)
        if h:
            detalle = [
                "nuestro={}  tercero={}  lineas={}  empieza_en={}".format(*x) for x in h
            ]
            errores += fail(
                "copia LITERAL de material de terceros (>= {} lineas consecutivas)".format(
                    MIN_LINEAS_COPIA
                ),
                detalle,
            )
    else:
        print(
            "::notice::MD_TERCEROS_DIR no definido: el check de copia literal NO se ha ejecutado"
        )

    if errores:
        print("PROPIEDAD INTELECTUAL: {} problema(s)".format(errores))
        return 1
    print("PROPIEDAD INTELECTUAL: OK (documentado, no copiado)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

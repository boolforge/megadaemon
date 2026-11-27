#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Gate de IDIOMA y CODIFICACIÓN de la UI (MEDIDO 2026-11-27).

PROBLEMA REAL (captura del móvil del jefe, 2026-11-27)
-------------------------------------------------------
La UI de Antigravity salía con texto CORRUPTO y mezclas de idioma:

    "Pregunta,轨道射,轨道问,轨道射记 el chat"

y el agente contestaba con basura multilingüe:

    "Veo que este es un sistema de y究竟 de compilación que simplemente se、文如"

Es decir: caracteres CJK colados en texto español, y texto español roto por
modelos que devuelven languages que no son el pedido.  En nuestro repo la
misma clase de fallo aparece en los comentarios del código (una vez se coló
"替代" y otra "助手" en mitad de un párrafo en español).

QUÉ HACE ESTE GATE
------------------
1. **Caracteres CJK / Hangul / Cirílico** en la UI y en los módulos de
   Antigravity: no tienen ningún sitio en una interfaz en español.
2. **Mojibake**: las secuencias típicas de UTF-8 leído como Latin-1
   ("Ã¡", "â€™", "Â«"): texto español roto.

Es determinista, cuesta un segundo y es lo que evita que la clase de
fallo vuelva a colarse sin que nadie lo note en la revisión.

USO
---
    python3 tools/check_idioma.py                 # lo que hay en el repo
    python3 tools/check_idioma.py fichero.html    # un fichero concreto
"""

import glob
import os
import re
import sys

# CJK (unified ideographs + ext A + compatibility), Hangul, Cirílico.
CJK = re.compile(
    r"[　-〿㐀-䶿一-鿿豈-﫿가-힯"
    r"Ѐ-ӿ]"
)
# Mojibake: UTF-8 interpretado como Latin-1/CP1252.
MOJIBAKE = re.compile(r"(Ã¡|Ã©|Ã­|Ã³|Ãº|Ã±|Â¿|Â¡|â€™|â€œ|â€|Â«|Â»|ï¼Œ)")

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

PATRONES = [
    "web/*.html",
    "web/*.css",
    "web/*.js",
    "web/panels/*.html",
    "web/panels/*.js",
    "rust-core/src/antigravity_*.rs",
    "rust-core/src/cockpit_server.rs",
    "rust-core/src/quota_guard.rs",
    "rust-core/src/providers.rs",
    "rust-core/src/llm_gateway.rs",
]


def ficheros(extra=None):
    if extra:
        return list(extra)
    out = []
    for pat in PATRONES:
        out.extend(glob.glob(os.path.join(RAIZ, pat)))
    return sorted(set(out))


def revisar(ruta):
    """Devuelve la lista de problemas de un fichero."""
    try:
        with open(ruta, "r", encoding="utf-8") as fh:
            texto = fh.read()
    except UnicodeDecodeError:
        return [(0, "el fichero NO es UTF-8 válido")]
    except OSError as exc:
        return [(0, "no se puede leer: %s" % exc)]

    problemas = []
    for num, linea in enumerate(texto.split("\n"), 1):
        if CJK.search(linea):
            problemas.append((num, "caracteres CJK/cirilico: %s" % linea.strip()[:100]))
        elif MOJIBAKE.search(linea):
            problemas.append((num, "texto roto (mojibake): %s" % linea.strip()[:100]))
    return problemas


def main(argv):
    objetivos = ficheros(argv[1:] or None)
    if not objetivos:
        print("check_idioma: no hay ficheros que revisar")
        return 0

    malos = 0
    for ruta in objetivos:
        problemas = revisar(ruta)
        if not problemas:
            continue
        malos += 1
        rel = os.path.relpath(ruta, RAIZ)
        print("::error file=%s::texto corrupto o idioma mezclado" % rel)
        for num, msg in problemas[:6]:
            print("  linea %d: %s" % (num, msg))
        if len(problemas) > 6:
            print("  ... y %d linea(s) mas" % (len(problemas) - 6))

    if malos:
        print("IDIOMA ROTO en %d fichero(s)" % malos)
        return 1
    print("idioma OK en %d fichero(s)" % len(objetivos))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

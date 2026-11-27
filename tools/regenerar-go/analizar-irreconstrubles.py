#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""analizar-irreconstrubles.py — por qué fallan las funciones que NINGÚN motor
puede decompilar.

Cruza, para cada función pendiente:

  * el pclntab (`go.funcs.tsv`): direccion real, simbolo, fichero, linea y el
    TAMANO REAL de la funcion (siguiente VA - VA);
  * el informe del motor propio (`_decompile_report.tsv`): `dec_ok`, `dec_err`,
    `sig` y `body_size`;
  * los indices de los shards de r2ghidra (`_index.tsv`): `ok`, el tamano que
    r2 supuso, los ms y el error;
  * los indices de los shards de Ghidra (`s0`/`s1`/`s2`): `ok` y `err`;
  * los PRIMEROS bytes de la funcion, decodificados en AArch64, para saber si
    es un thunk (una sola rama), si tiene marco de pila, si esta vacia, etc.;
  * si el simbolo aparece MAS DE UNA VEZ en el pclntab (otro sitio donde
    mirar).

De ahi sale un VEREDICTO por funcion:

  * ``THUNK``                  -> el cuerpo es una rama; se puede reconstruir
                                   leyendo a donde salta.
  * ``RECONSTRUIBLE-CON-PISTA`` -> el nombre dice que es un wrapper de Go
                                   (``gowrap``/``deferwrap``/closure) y el
                                   pclntab da la linea real: se puede
                                   reconstruir a mano.
  * ``IRRECONSTRUIBLE``         -> los tres motores fallan con un error de
                                   bajo nivel y no hay thunk ni wrapper: el
                                   CUERPO no se puede recuperar; solo la
                                   FIRMA (nombre, fichero, linea).

uso:
  tools/regenerar-go/analizar-irreconstrubles.py \
      --pendientes ... --salida ... [--json ...]
"""
from __future__ import annotations

import argparse
import json
import os
import statistics
import struct
import sys
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

# ---------------------------------------------------------------------------
# AArch64: solo las instrucciones que sirven para el diagnostico
# ---------------------------------------------------------------------------

#: STP (pre-index, 64 bits) -> guarda el marco de pila
def es_stp_pre_index(w: int) -> bool:
    return (w & 0xFFC00000) == 0xA9800000 and ((w >> 5) & 0x1F) == 31


def es_stp_post_index(w: int) -> bool:
    return (w & 0xFFC00000) == 0xA8800000 and ((w >> 5) & 0x1F) == 31


def es_mov_x29_sp(w: int) -> bool:
    return w == 0x910003FD


def es_sub_sp(w: int) -> bool:
    return (w & 0xFF0003FF) == 0xD10003FF


def es_b(w: int) -> Optional[int]:
    """Rama incondicional: devuelve el desplazamiento en bytes."""
    if (w & 0xFC000000) != 0x14000000:
        return None
    imm = w & 0x03FFFFFF
    if imm & 0x02000000:
        imm -= 0x04000000
    return imm * 4


def es_bl(w: int) -> bool:
    return (w & 0xFC000000) == 0x94000000


def es_ret(w: int) -> bool:
    return w == 0xD65F03C0


def es_br_x(w: int) -> Optional[int]:
    """Salto indirecto por registro: devuelve el numero de registro."""
    if (w & 0xFFFFFC1F) == 0xD61F0000:
        return (w >> 5) & 0x1F
    return None


def desensamblar(texto: bytes) -> List[Tuple[int, str]]:
    """Decodifica lo justo: devuelve [(offset, etiqueta)] de las 1as instr."""
    out: List[Tuple[int, str]] = []
    for i in range(0, min(len(texto), 64), 4):
        w = struct.unpack_from("<I", texto, i)[0]
        if es_ret(w):
            out.append((i, "RET"))
        elif es_br_x(w) is not None:
            out.append((i, "BR X%d" % es_br_x(w)))
        elif es_b(w) is not None:
            out.append((i, "B %+d" % es_b(w)))
        elif es_bl(w):
            out.append((i, "BL"))
        elif es_mov_x29_sp(w):
            out.append((i, "MOV x29, sp"))
        elif es_sub_sp(w):
            out.append((i, "SUB sp, sp, #imm"))
        elif es_stp_pre_index(w):
            out.append((i, "STP x?, x?, [sp, #-imm]!"))
        elif es_stp_post_index(w):
            out.append((i, "STP x?, x?, [sp], #imm"))
        else:
            out.append((i, "insn 0x%08x" % w))
    return out


# ---------------------------------------------------------------------------
# carga de las fuentes
# ---------------------------------------------------------------------------


def cargar_pclntab(ruta: Path) -> Tuple[List[int], Dict[int, Tuple[str, str]]]:
    """Devuelve (VAs ordenadas, addr -> (simbolo, resto))."""
    vas: List[int] = []
    info: Dict[int, Tuple[str, str]] = {}
    with open(ruta, encoding="utf-8", errors="replace") as f:
        for linea in f:
            p = linea.rstrip("\n").split("\t")
            if len(p) < 2:
                continue
            try:
                a = int(p[0], 16)
            except ValueError:
                continue
            vas.append(a)
            info[a] = (p[1].strip(), (p[2] if len(p) > 2 else "").strip())
    vas.sort()
    return vas, info


def cargar_motor(ruta: Path) -> Dict[str, Tuple[str, str, str, str]]:
    """Informe del motor propio: nombre -> (dec_ok, dec_err, sig, body_size)."""
    out: Dict[str, Tuple[str, str, str, str]] = {}
    if not Path(ruta).is_file():
        return out
    with open(ruta, encoding="utf-8", errors="replace") as f:
        f.readline()
        for linea in f:
            p = linea.rstrip("\n").split("\t")
            if len(p) < 9 or not p[1]:
                continue
            dec_ok = p[8].strip()
            dec_err = (p[9].strip() if len(p) > 9 else "")
            sig = (p[6].strip() if len(p) > 6 else "")
            body = (p[7].strip() if len(p) > 7 else "")
            out[p[1].strip()] = (dec_ok, dec_err, sig, body)
    return out


def cargar_shards(rutas: Sequence[str]) -> Dict[str, List[Tuple[str, str, str, str]]]:
    """Indices de shard (Ghidra o r2ghidra): nombre -> [(shard, ok, err, extra)]."""
    out: Dict[str, List[Tuple[str, str, str, str]]] = {}
    for ruta in rutas:
        ruta = ruta.strip()
        if not ruta or not Path(ruta).is_file():
            continue
        shard = os.path.basename(os.path.dirname(ruta))
        with open(ruta, encoding="utf-8", errors="replace") as f:
            f.readline()
            for linea in f:
                p = linea.rstrip("\n").split("\t")
                if len(p) < 7 or not p[1]:
                    continue
                ok = p[5].strip()
                err = (p[6].strip() if len(p) > 6 else "")
                extra = (p[4].strip() if len(p) > 4 else "")  # bytes
                out.setdefault(p[1].strip(), []).append((shard, ok, err, extra))
    return out


def categoria_de(fichero: str) -> str:
    f = fichero or ""
    if f.startswith("blaze-out/"):
        return "generado (blaze/protobuf)"
    if f == "<autogenerated>":
        return "autogenerado (compilador)"
    if f.startswith("third_party/jetski/"):
        return "jetski (codigo propio de Antigravity)"
    if f.startswith("third_party/go/gc/"):
        return "runtime Go"
    if f.startswith("third_party/golang/"):
        return "librerias publicas (third_party/golang)"
    if f.startswith("third_party/"):
        return "otras third_party"
    return "resto"


def es_wrapper(nombre: str) -> Optional[str]:
    if ".gowrap" in nombre:
        return "gowrap"
    if ".deferwrap" in nombre:
        return "deferwrap"
    if ".func" in nombre and nombre.rstrip("0123456789").endswith(".func"):
        return "closure"
    return None


# ---------------------------------------------------------------------------
# principal
# ---------------------------------------------------------------------------




def tiene_prologo(texto: bytes) -> bool:
    """¿Hay marco de pila en las primeras instrucciones? (convencion Go/arm64)"""
    for i in range(0, len(texto) - 3, 4):
        w = struct.unpack_from("<I", texto, i)[0]
        # STP pre-index con Rn=SP (31)
        if (w & 0xFFE00000) == 0xA9800000 and ((w >> 5) & 0x1F) == 31:
            return True
        # SUB/ADD SP, SP, #imm
        if (w & 0xFF0003FF) in (0xD10003FF, 0x910003FF):
            return True
        # STR x30, [SP, #imm]  (salvada del enlace de retorno)
        if (w & 0xFFC003E0) == 0xF90003E0 and ((w >> 5) & 0x1F) == 31:
            return True
    return False


def ramas_b_antes(datos: bytes, base: int, objetivo: int, ventana: int = 512):
    """¿Alguna B de la ventana anterior salta a `objetivo`?  Devuelve el origen."""
    ini = max(0, base - ventana)
    for i in range(0, len(datos) - 3, 4):
        w = struct.unpack_from("<I", datos, i)[0]
        if (w & 0xFC000000) == 0x14000000:
            imm = w & 0x03FFFFFF
            if imm & 0x02000000:
                imm -= 0x04000000
            if base + i + imm * 4 == objetivo:
                return base + i
    return None


def cargar_motor_filas(ruta: Path) -> List[List[str]]:
    filas: List[List[str]] = []
    if not Path(ruta).is_file():
        return filas
    with open(ruta, encoding="utf-8", errors="replace") as f:
        f.readline()
        for linea in f:
            p = linea.rstrip("\n").split("\t")
            if len(p) >= 9 and p[1]:
                filas.append(p)
    return filas


def main(argv: Optional[Sequence[str]] = None) -> int:
    p = argparse.ArgumentParser(description="Analiza las funciones que ningun motor puede decompilar")
    p.add_argument("--pendientes", type=Path, required=True)
    p.add_argument("--tsv", type=Path, default=Path("/workspace/agy-re/tools-out/go.funcs.tsv"))
    p.add_argument("--bin", type=Path, default=Path("/workspace/agy-re/liblanguage_server.so"))
    p.add_argument(
        "--motor",
        type=Path,
        default=Path("/workspace/agy-re/tools-out/out_md/motor_propio_437/_decompile_report.tsv"),
    )
    p.add_argument("--motor-4k", type=Path, default=Path("/workspace/agy-re/tools-out/out_md/motor_propio_4k/_decompile_report.tsv"))
    p.add_argument("--shards", default="", help="indices _index.tsv separados por coma")
    p.add_argument(
        "--r2dec",
        type=Path,
        default=None,
        help="directorio con los .c del motor r2dec (5.o motor): si decompila la "
        "funcion, deja de ser IRRECONSTRUIBLE",
    )
    p.add_argument("--salida", type=Path, required=True)
    p.add_argument("--json", type=Path, default=None)
    args = p.parse_args(argv)

    vas, info = cargar_pclntab(args.tsv)
    import bisect

    motor = cargar_motor(args.motor)
    motor4k = cargar_motor(args.motor_4k)
    shards = cargar_shards(args.shards.split(","))

    # simbolos que aparecen MAS DE UNA VEZ en el pclntab
    from collections import Counter

    cnt = Counter(v[0] for v in info.values())
    duplicados = {s for s, n in cnt.items() if n > 1}

    pend: List[Tuple[str, ...]] = []
    with open(args.pendientes, encoding="utf-8", errors="replace") as f:
        for linea in f:
            p2 = linea.rstrip("\n").split("\t")
            if len(p2) >= 4 and p2[0]:
                pend.append(tuple(p2))

    bin_ = open(args.bin, "rb")

    filas: List[Dict[str, object]] = []
    for fila in pend:
        addr = int(fila[0], 16)
        simbolo = fila[1]
        orig = fila[2] if len(fila) > 2 else ""
        loc = fila[3] if len(fila) > 3 else ""
        tag = fila[4] if len(fila) > 4 else ""
        ruta, _, linea_txt = loc.rpartition(":")
        cat = categoria_de(ruta)

        # tamano REAL: siguiente VA del pclntab
        i = bisect.bisect_right(vas, addr)
        real = (vas[i] - addr) if i < len(vas) else None

        dec_ok, dec_err, sig, body = motor.get(simbolo, ("", "", "", ""))
        dec_ok4k, dec_err4k, _, body4k = motor4k.get(simbolo, ("", "", "", ""))

        # indices de los shards
        sh = shards.get(simbolo, [])
        sh_ok = ",".join("%s:%s" % (s, ok) for s, ok, _e, _x in sh)
        sh_err = "; ".join("%s: %s" % (s, e) for s, _o, e, _x in sh if e)

        # bytes iniciales
        try:
            bin_.seek(addr)
            cab = bin_.read(64)
        except OSError:
            cab = b""
        instr = desensamblar(cab)
        etiquetas = [t for _o, t in instr]
        primera = etiquetas[0] if etiquetas else ""
        tiene_stp = any(t.startswith("STP") for t in etiquetas)
        # un THUNK es una rama que SALE de la funcion (a otro simbolo).  Una
        # rama DENTRO de la funcion no es un thunk: es codigo normal.
        es_thunk = etiquetas[:1] == ["RET"]
        rama_destino = ""
        if etiquetas and etiquetas[0].startswith("B "):
            disp = int(etiquetas[0].split()[1])
            destino = addr + disp
            if real is not None and not (addr <= destino < addr + real):
                es_thunk = True
                if destino in info:
                    rama_destino = "%s (%s)" % (hex(destino), info[destino][0])
                else:
                    rama_destino = hex(destino)
            else:
                rama_destino = "%s (dentro de la propia funcion: NO es un thunk)" % hex(destino)

        wrapper = es_wrapper(simbolo)
        dup = simbolo in duplicados
        r2dec_ok = False
        if args.r2dec and args.r2dec.is_dir():
            # el nombre del fichero de r2dec es el simbolo saneado (y
            # truncado a 80 caracteres, como en el barrido), con un indice
            # delante: se busca por ambos formatos.
            formas = (simbolo.replace("/", "_"), simbolo[:80].replace("/", "_"))
            for cand in args.r2dec.glob("*.c"):
                if any(f and f in cand.name for f in formas) and cand.stat().st_size > 20:
                    r2dec_ok = True
                    break

        # veredicto
        if es_thunk and rama_destino:
            veredicto = "THUNK"
            porque = "el cuerpo es una rama a %s" % rama_destino
        elif wrapper:
            veredicto = "RECONSTRUIBLE-CON-PISTA"
            porque = "wrapper de Go (%s): el pclntab da la linea real del call site" % wrapper
        elif r2dec_ok:
            # MEDIDO: el motor PROPIO de radare2 (r2dec) SI produce salida para
            # esta funcion => el cuerpo NO esta perdido; lo que no se obtiene
            # es C estilo Ghidra, sino el pseudo-C de r2.
            veredicto = "RECONSTRUIBLE-CON-R2DEC"
            porque = "r2dec produce pseudo-C (no es C de Ghidra)"
        else:
            veredicto = "IRRECONSTRUIBLE"
            porque = "ningun motor produce salida y no es thunk ni wrapper"

        filas.append(
            {
                "addr": hex(addr),
                "nombre": simbolo,
                "orig": orig,
                "loc": loc,
                "tag": tag,
                "categoria": cat,
                "bytes_reales": real,
                "body_ghidra": body,
                "sig_ghidra": sig,
                "dec_ok_motor437": dec_ok,
                "dec_err_motor437": dec_err,
                "dec_ok_motor4k": dec_ok4k,
                "dec_err_motor4k": dec_err4k,
                "shards": sh_ok,
                "shards_err": sh_err,
                "primera_instr": primera,
                "tiene_marco_pila": "si" if tiene_stp else "no",
                "rama_destino": rama_destino or "",
                "wrapper": wrapper or "",
                "simbolo_duplicado": "si" if dup else "no",
                "r2dec": "si" if r2dec_ok else "no",
                "veredicto": veredicto,
                "porque": porque,
            }
        )

    cols = [
        "addr", "nombre", "orig", "loc", "tag", "categoria", "bytes_reales",
        "body_ghidra", "sig_ghidra", "dec_ok_motor437", "dec_err_motor437",
        "dec_ok_motor4k", "dec_err_motor4k", "shards", "shards_err",
        "primera_instr", "tiene_marco_pila", "rama_destino", "wrapper",
        "simbolo_duplicado", "r2dec", "veredicto", "porque",
    ]
    args.salida.parent.mkdir(parents=True, exist_ok=True)
    with open(args.salida, "w", encoding="utf-8") as f:
        f.write("\t".join(cols) + "\n")
        for r in filas:
            f.write("\t".join(str(r.get(c, "")).replace("\t", " ") for c in cols) + "\n")

    # resumen
    from collections import Counter

    resumen = {
        "pendientes": len(filas),
        "por_categoria": dict(Counter(r["categoria"] for r in filas)),
        "por_veredicto": dict(Counter(r["veredicto"] for r in filas)),
        "por_error_motor437": dict(
            Counter((r["dec_err_motor437"] or "(sin fila)").strip() for r in filas)
        ),
        "por_primera_instr": dict(Counter(r["primera_instr"].split()[0] for r in filas)),
        "con_marco_de_pila": sum(1 for r in filas if r["tiene_marco_pila"] == "si"),
        "simbolo_duplicado": sum(1 for r in filas if r["simbolo_duplicado"] == "si"),
        "tamano_real": {
            "min": min((r["bytes_reales"] for r in filas if r["bytes_reales"]), default=None),
            "max": max((r["bytes_reales"] for r in filas if r["bytes_reales"]), default=None),
            "mediana": sorted(r["bytes_reales"] for r in filas if r["bytes_reales"])[
                len([r for r in filas if r["bytes_reales"]]) // 2
            ]
            if any(r["bytes_reales"] for r in filas)
            else None,
        },
    }
    # ---- comparativa con el grupo control (mismo informe, dec_ok=true) ----
    import bisect as _bis

    filas_motor = cargar_motor_filas(args.motor)
    control = [r for r in filas_motor if r[8].strip().lower() == "true"]
    pend_nombres = {r["nombre"] for r in filas}

    def _real(addr: int):
        i = _bis.bisect_right(vas, addr)
        return (vas[i] - addr) if i < len(vas) else None

    def _ratio(grupo, es_pendiente):
        ratios, exactos, mayores = [], 0, 0
        for r in grupo:
            try:
                a = int(r[0], 16)
            except ValueError:
                continue
            rs = _real(a)
            if not rs:
                continue
            try:
                body = int(r[7])
            except (ValueError, IndexError):
                body = 0
            ratios.append(body / rs)
            if body == rs:
                exactos += 1
            if body > rs:
                mayores += 1
        return ratios, exactos, mayores

    rf, ef, mf = _ratio([r for r in filas_motor if r[1] in pend_nombres], True)
    rc, ec, mc = _ratio(control, False)
    resumen["ratio_fallo"] = round(statistics.median(rf), 2) if rf else None
    resumen["ratio_ctrl"] = round(statistics.median(rc), 2) if rc else None
    resumen["fallo_exactos"] = ef
    resumen["fallo_mayores"] = mf
    resumen["ctrl_exactos"] = ec
    resumen["ctrl_n"] = len(control)

    # ---- hipotesis refutadas ----
    def _prologo_grupo(grupo):
        con = sin = 0
        for r in grupo:
            try:
                a = int(r[0], 16)
            except ValueError:
                continue
            bin_.seek(a)
            t = bin_.read(32)
            if tiene_prologo(t):
                con += 1
            else:
                sin += 1
        return con, sin

    cf, sf = _prologo_grupo([r for r in filas_motor if r[1] in pend_nombres])
    cc, sc = _prologo_grupo(control)
    resumen["prologo_fallo_sin"] = sf
    resumen["prologo_fallo_n"] = sf + cf
    resumen["prologo_ctrl_sin"] = sc
    resumen["prologue_ctrl_n"] = sc + cc

    def _destino_grupo(grupo):
        n = con = 0
        for r in grupo:
            try:
                a = int(r[0], 16)
            except ValueError:
                continue
            n += 1
            bin_.seek(max(0, a - 512))
            datos = bin_.read(512)
            if ramas_b_antes(datos, max(0, a - 512), a) is not None:
                con += 1
        return con, n

    df, nf = _destino_grupo([r for r in filas_motor if r[1] in pend_nombres])
    dc, nc = _destino_grupo(control)
    resumen["destino_rama_fallo"] = df
    resumen["destino_rama_fallo_n"] = nf
    resumen["destino_rama_ctrl"] = dc
    resumen["destino_rama_ctrl_n"] = nc

    # ---- huecos del export del pclntab ----
    unicos = sorted(set(vas))
    huecos = sum(1 for i in range(len(unicos) - 1) if unicos[i + 1] - unicos[i] > 512)
    resumen["huecos_pclntab"] = huecos

    txt = json.dumps(resumen, indent=2, ensure_ascii=False, sort_keys=True)
    print(txt)
    if args.json:
        args.json.write_text(txt + "\n", encoding="utf-8")
    print("[analizar] %d funciones -> %s" % (len(filas), args.salida))
    return 0


if __name__ == "__main__":
    sys.exit(main())

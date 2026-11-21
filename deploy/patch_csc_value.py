#!/usr/bin/env python3
"""Parchea el cscfeature.xml del CSC del movil con el valor que se le pida.

Se ejecuta EN EL MOVIL (python3 de Termux) y **reescribe el modulo Magisk**
con el valor pedido:

    python3 deploy/patch_csc_value.py RecordingAllowed
    python3 deploy/patch_csc_value.py --listar
    python3 deploy/patch_csc_value.py --csc=TPA --feature=CscFeature_Algo RecordingAllowed

QUE HACE (todo medido en el S21 Ultra, CSC AMO):
  * El CSC de Samsung guarda las "features" en un XML comprimido con un
    contenedor OMC (rotacion + XOR posicional + gzip).  El framework lo lee
    desde `com.samsung.android.feature.SemCscFeature` (framework.jar ->
    classes5.dex), de ahi el `bind-mount` systemless del modulo Magisk:
    el fichero ORIGINAL no se toca y revertir es desactivar el modulo.
  * `contents.db` guarda el `fileSize` de cada fichero del CSC: si no se
    actualiza, el inventario no cuadra con el fichero servido.

MODOS:
  --listar          Descubre y lista TODAS las features CscFeature_* del
                    CSC con su valor actual (para saber que mas hay).
  --csc=<ID>        CSC a parchear (por defecto: el ACTIVO del movil).
  --feature=<NAME>  Feature a parchear (por defecto:
                    CscFeature_VoiceCall_ConfigRecording).
  <valor>           Valor a escribir.  Se acepta CUALQUIER valor del
                    esquema del fabricante, por ejemplo:
                      RecordingAllowed
                      RecordingAllowedByMenu,disclaimer_latin
                      RecordingNotAllowed

NOTA LEGAL (una linea, y despues se sigue): en la UE es licito eludir
medidas tecnicas para interoperabilidad (art. 6 de la Directiva InfoSoc
2001/29, art. 160.b LO 11/2007 y reforma del art. 270 CP).  Lo que es
cuestion distinta es la CONDUCTA de grabar a terceros sin aviso: eso es
materia de proteccion de datos (RGPD) y de la vida privada de los
interlocutores, no de la elusion.  La herramienta hace lo tecnico y
deja constancia de que modo ha dejado.
"""
import argparse
import gzip
import os
import re
import shutil
import sqlite3
import subprocess
import sys

MOD = "/data/adb/modules/megadaemon_csc_recording"
CODEC = "/data/local/tmp/megadaemon/omc_codec.py"
CARRIER_ROOT = "/optics/configs/carriers/single"
DB_ROOT = "/prism/etc/carriers/single"
FEAT_DEF = "CscFeature_VoiceCall_ConfigRecording"
LOG = "/data/local/tmp/megadaemon/csc_patch.log"


def cargar_codec():
    """Carga el decodificador OMC (tools/omc_codec.py) desde donde estea."""
    for ruta in (CODEC, os.path.join(os.path.dirname(os.path.abspath(__file__)), "omc_codec.py")):
        if os.path.exists(ruta):
            sys.path.insert(0, os.path.dirname(ruta))
            import omc_codec  # noqa: E402

            return omc_codec
    print("ERROR: no encuentro omc_codec.py (copia tools/omc_codec.py a %s)" % CODEC)
    raise SystemExit(2)


def csc_activo():
    """CSC activo del movil.  Se pregunta al sistema; si no, se listan los que hay."""
    for prop in (
        "ro.build.version.oplus_csc",
        "ro.build.version.oplus_region",
        "ro.csc.support.virtualcarrier",
    ):
        try:
            out = subprocess.run(["getprop", prop], capture_output=True, text=True, timeout=10).stdout.strip()
        except Exception:
            out = ""
        if out and out.lower() != "null":
            return out
    try:
        return sorted(os.listdir(CARRIER_ROOT))[0]
    except Exception:
        return "AMO"


def ruta_src(csc):
    return f"{CARRIER_ROOT}/{csc}/conf/system/cscfeature.xml"


def ruta_db(csc):
    return f"{DB_ROOT}/{csc}/contents.db"


def cargar_xml(omc, csc):
    """Devuelve (xml, fuente).  Prefiere el modulo si existe (reparcheado)."""
    fuente = os.path.join(MOD, "cscfeature.xml")
    if not os.path.exists(fuente):
        fuente = ruta_src(csc)
    if not os.path.exists(fuente):
        return None, fuente
    try:
        return omc.decode_file(fuente).decode("utf-8", "replace"), fuente
    except Exception as e:
        print("ERROR: no pude decodificar %s: %s" % (fuente, e))
        return None, fuente


def listar_features(xml):
    """Todas las features CscFeature_* con su valor."""
    feats = re.findall(r"<(CscFeature_[A-Za-z0-9_]+)>([^<]*)</\1>", xml)
    if not feats:
        feats = re.findall(r"<(CscFeature_[A-Za-z0-9_]+)\s*/>", xml)
        feats = [(f, "") for f in feats]
    return feats


def aplicar(omc, csc, feature, valor):
    xml, fuente = cargar_xml(omc, csc)
    if xml is None:
        print("ERROR: no existe el cscfeature.xml del CSC %s (%s)" % (csc, fuente))
        return 2

    feats = listar_features(xml)
    print("CSC: %s | fichero: %s" % (csc, fuente))
    print("features CscFeature_* en el CSC: %d" % len(feats))
    actual = dict(feats)
    if feature in actual:
        print("valor actual de %s: %s" % (feature, actual[feature]))
    else:
        print("valor actual de %s: AUSENTE (se añadirá)" % feature)

    antes = re.search(re.escape(feature) + r">([^<]*)<", xml)
    if antes:
        nuevo = re.sub(re.escape(feature) + r">[^<]*<", f"{feature}>{valor}<", xml, count=1)
    else:
        nuevo = xml.replace("</root>", f"<{feature}>{valor}</{feature}></root>")

    datos = omc.omc_encode(gzip.compress(nuevo.encode("utf-8")))

    db = ruta_db(csc)
    if os.path.exists(db):
        try:
            con = sqlite3.connect(db)
            cur = con.cursor()
            filas = cur.execute(
                "SELECT _id, fileSize FROM contents WHERE filePath LIKE ?", ("%cscfeature.xml",)
            ).fetchall()
            for _id, _old in filas:
                cur.execute("UPDATE contents SET fileSize=? WHERE _id=?", (len(datos), _id))
                print("contents.db: _id=%s fileSize -> %d" % (_id, len(datos)))
            con.commit()
            con.close()
        except Exception as e:
            print("AVISO: no pude actualizar contents.db: %s" % e)

    os.makedirs(MOD, exist_ok=True)
    destino = os.path.join(MOD, "cscfeature.xml")
    with open(destino, "wb") as f:
        f.write(datos)
    print("modulo actualizado: %s (%d B)" % (destino, len(datos)))
    print("valor pedido: %s = %s" % (feature, valor))
    try:
        with open(LOG, "a") as f:
            f.write("%s  %s=%s  csc=%s\n" % (subprocess.run(["date"], capture_output=True, text=True).stdout.strip(), feature, valor, csc))
    except Exception:
        pass
    print("REINICIA el movil para que el framework lea el fichero nuevo")
    print("(o monta el modulo sin reiniciar: mount --bind %s %s)" % (destino, ruta_src(csc)))
    return 0


def main():
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("valor", nargs="?", default=None)
    ap.add_argument("--listar", action="store_true", help="lista todas las features del CSC")
    ap.add_argument("--csc", default=None, help="CSC a parchear (por defecto, el activo)")
    ap.add_argument("--feature", default=FEAT_DEF, help="feature a parchear")
    args = ap.parse_args()

    omc = cargar_codec()
    csc = args.csc or csc_activo()
    xml, fuente = cargar_xml(omc, csc)
    if xml is None:
        print("ERROR: no hay cscfeature.xml para el CSC %s" % csc)
        return 2

    if args.listar or args.valor is None:
        feats = listar_features(xml)
        print("=== FEATURES DEL CSC %s (%d) ===" % (csc, len(feats)))
        for nombre, valor in feats:
            print("  %-58s = %s" % (nombre, valor))
        print("=== carriers presentes ===")
        try:
            for d in sorted(os.listdir(CARRIER_ROOT)):
                print("  %s" % d)
        except Exception:
            pass
        if args.valor is None:
            return 0
    return aplicar(omc, csc, args.feature, args.valor)


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Reparchea el cscfeature.xml con un valor concreto de la feature CSC.

Se ejecuta EN el movil (python3 de Termux) y **reescribe el modulo Magisk**
con el valor pedido:

    python3 deploy/patch_csc_value.py RecordingAllowedByMenu,disclaimer_latin

Porque:
  * `RecordingAllowed` a secas => grabacion **automatica y silenciosa** =>
    en Espana es delito (art. 197 CP) y vulnera el GDPR.
  * `RecordingAllowedByMenu,disclaimer_latin` => la grabacion aparece en el
    menu y **avisa por voz** a la otra parte (lo que usan CHO/EON/GTO/TPA).

El formato OMC (rotacion + XOR posicional + gzip) lo implementa
`tools/omc_codec.py`; el `fileSize` de `contents.db` se actualiza porque el
framework comprueba que cuadre con el fichero servido.
"""
import os
import re
import shutil
import sqlite3
import sys

CSC = "AMO"
FEAT = "CscFeature_VoiceCall_ConfigRecording"
MOD = "/data/adb/modules/megadaemon_csc_recording"
SRC = f"/optics/configs/carriers/single/{CSC}/conf/system/cscfeature.xml"
DB = f"/prism/etc/carriers/single/{CSC}/contents.db"
CODEC = "/data/local/tmp/megadaemon/omc_codec.py"


def main():
    if len(sys.argv) < 2:
        print("uso: patch_csc_value.py <valor>")
        return 2
    valor = sys.argv[1]
    sys.path.insert(0, os.path.dirname(CODEC))
    import omc_codec  # noqa: E402

    # 1) partir del original que hay en la particion si el modulo no esta
    #    montado; si esta montado, sirve el parcheado (y se reescribe).
    fuente = os.path.join(MOD, "cscfeature.xml")
    if not os.path.exists(fuente):
        fuente = SRC
    xml = omc_codec.decode_file(fuente).decode("utf-8", "replace")
    antes = re.search(re.escape(FEAT) + r">([^<]*)<", xml)
    print("valor actual:", antes.group(1) if antes else "AUSENTE")

    nuevo = re.sub(
        re.escape(FEAT) + r">[^<]*<",
        f"{FEAT}>{valor}<",
        xml,
        count=1,
    )
    if antes is None:
        # la feature no estaba: se anade antes de </root>
        nuevo = nuevo.replace("</root>", f"<{FEAT}>{valor}</{FEAT}></root>")
    if nuevo == xml:
        print("sin cambios")
    datos = omc_codec.omc_encode(gzip_bytes(nuevo.encode("utf-8")))

    # 2) contents.db: el fileSize debe cuadrar con el fichero
    if os.path.exists(DB):
        con = sqlite3.connect(DB)
        cur = con.cursor()
        filas = cur.execute(
            "SELECT _id, fileSize FROM contents WHERE filePath LIKE ?", ("%cscfeature.xml",)
        ).fetchall()
        for _id, _old in filas:
            cur.execute("UPDATE contents SET fileSize=? WHERE _id=?", (len(datos), _id))
            print(f"contents.db: _id={_id} fileSize -> {len(datos)}")
        con.commit()
        con.close()

    # 3) escribir el modulo (el bind-mount se actualiza en el siguiente boot;
    #    si el modulo esta montado, se sobreescribe el fichero origen del mount)
    os.makedirs(MOD, exist_ok=True)
    with open(os.path.join(MOD, "cscfeature.xml"), "wb") as f:
        f.write(datos)
    print("modulo actualizado:", os.path.join(MOD, "cscfeature.xml"), len(datos), "B")
    print("valor pedido:", valor)
    print("REINICIA para que el framework lea el fichero nuevo")
    return 0


def gzip_bytes(data):
    import gzip
    return gzip.compress(data)


if __name__ == "__main__":
    sys.exit(main())

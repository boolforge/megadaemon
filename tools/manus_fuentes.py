#!/usr/bin/env python3
"""Descarga y CLASIFICACIÓN de **Manus** (y sus clerks), para PC y Android.

Lo que el jefe pidio: "agrega tambien Manus, lo plvide.  tanto manus de
Android como los de pc".

MEDIDO (2026-11-27, sin web_search: curl directo a la web y a la API):
  * **PC / macOS**: instalador `Manus-Setup-2.0.4.dmg` (315.061.735 B,
    `content-type: application/x-apple-diskimage`) en
    `https://download.manus.im/Manus-Setup-2.0.4.dmg`.
  * **PC / Windows**: NO hay `.exe` publico (el bucket S3 responde
    `AccessDenied`); la app se distribuye por **Microsoft Store**:
    `https://apps.microsoft.com/detail/9phv7m7v4s5l`.
  * **Android**: el paquete es **`ai.manus.agents`** y la app se titula
    **"Cue by Manus"** (versiones medidas en Play: 26.17.0 .. 26.18.14).
    Play no sirve el `.apk` sin sesion (los mirrors APKMirror/APKPure/
    APKCombo responden 403 con Cloudflare) => se descarga la ficha, el
    manifiesto y los recursos publicos, NO el binario.
  * **API / MCP**: `https://api.manus.ai` y `https://open.manus.ai/docs`
    (la API publica de Manus tiene MCP propio: `"MCP":{"status":"ENABLED"}`).

El codigo fuente del AGENTE no es publico (Manus es cerrado): lo que se
puede escribir en claro son los **contratos** (la API, el MCP y los
protocolos).  Eso es lo que se guarda aqui, con su sha256.

El material pesado (el `.dmg`) va a **Drive** por el tubo y en local queda
solo el inventario.

    python3 tools/manus_fuentes.py
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, "/workspace/md-resc")

try:
    from drive_tube import asegurar_carpeta, subir_fichero, token as tubo_token, tarcar
    HAY_TUBO = True
except Exception:  # noqa: BLE001 - sin tubo se sigue, pero sin subir
    HAY_TUBO = False

DEST = os.environ.get("MANUS_DEST", "/workspace/md-re-productos/artifacts/manus")
RAIZ_DRIVE = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0"
CARPETA_DRIVE = "manus-producto"

# --- lo MEDIDO el 2026-11-27 -----------------------------------------------
DESCARGAS = [
    {
        "id": "manus-desktop-macos",
        "plataforma": "pc/macos",
        "nombre": "Manus Desktop",
        "url": "https://download.manus.im/Manus-Setup-2.0.4.dmg",
        "bytes": 315061735,
        "nota": "instalador de escritorio; el mas grande que se puede traerse",
    },
]

# Lo que NO es un fichero descargable pero hay que dejar fichado.
CONTRATOS = [
    {
        "id": "manus-desktop-windows",
        "plataforma": "pc/windows",
        "nombre": "Manus Desktop (Windows)",
        "url": "https://apps.microsoft.com/detail/9phv7m7v4s5l",
        "nota": "distribuido por Microsoft Store (el bucket S3 no sirve .exe: AccessDenied)",
    },
    {
        "id": "manus-android-cue",
        "plataforma": "android",
        "nombre": "Cue by Manus",
        "url": "https://play.google.com/store/apps/details?id=ai.manus.agents",
        "paquete": "ai.manus.agents",
        "nota": "la app Android de Manus; Play no sirve el .apk sin sesion",
    },
    {
        "id": "manus-api",
        "plataforma": "api",
        "nombre": "Manus API + MCP",
        "url": "https://open.manus.ai/docs",
        "api": "https://api.manus.ai",
        "nota": "API publica de Manus con MCP propio (status ENABLED)",
    },
]

UA_ANDROID = ("Mozilla/5.0 (Linux; Android 14; SM-G998B) AppleWebKit/537.36 "
              "(KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36")
UA_DESK = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
           "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")


def sha256_de(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for b in iter(lambda: fh.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def curl(destino, url, timeout=1800):
    """Descarga con curl (MEDIDO: urllib se corta a los 4 MB en este proxy)."""
    r = subprocess.run(
        ["curl", "-sSL", "--retry", "5", "--retry-delay", "2",
         "--retry-all-errors", "-A", UA_DESK, "-o", destino, url],
        capture_output=True, text=True, timeout=timeout)
    if r.returncode != 0:
        raise IOError("curl rc=%d: %s" % (r.returncode, r.stderr.strip()[:120]))
    return os.path.getsize(destino)


def clasificar_play(pkg):
    """Ficha publica de Play: titulo, version y developer.

    MEDIDO: la ficha viene embebida en el HTML (el titulo esta en un
    `<title>` que Play renderiza con JS, asi que se busca el nombre del
    paquete en el JSON embebido).
    """
    html = "/tmp/_manus_play.html"
    subprocess.run(["curl", "-sSL", "--max-time", "60", "-A", UA_ANDROID,
                    "-o", html,
                    f"https://play.google.com/store/apps/details?id={pkg}&hl=en&gl=US"],
                   capture_output=True, text=True, timeout=120)
    if not os.path.exists(html):
        return {}
    s = open(html, encoding="utf-8", errors="ignore").read()
    versiones = sorted(set(re.findall(r'"(\d{2}\.\d{1,2}\.\d{1,2})"', s)))
    return {
        "paquete": pkg,
        "bytes_html": len(s),
        "versiones_detectadas": versiones[:8],
        "sha256_html": hashlib.sha256(s.encode("utf-8", "ignore")).hexdigest(),
    }


def main():
    os.makedirs(DEST, exist_ok=True)
    filas = []

    # --- 1) los ficheros de verdad ---------------------------------------
    for d in DESCARGAS:
        dest = os.path.join(DEST, os.path.basename(d["url"]))
        estado = "CACHE"
        try:
            if not os.path.exists(dest) or os.path.getsize(dest) != d["bytes"]:
                n = curl(dest, d["url"])
                estado = "OK %d B" % n
            else:
                n = os.path.getsize(dest)
            sha = sha256_de(dest)
            drive = ""
            if HAY_TUBO:
                try:
                    tok = tubo_token()
                    cid = asegurar_carpeta(tok, RAIZ_DRIVE, CARPETA_DRIVE)
                    info = subir_fichero(tok, dest, cid, os.path.basename(dest))
                    drive = " drive id=%s" % info.get("id")
                    os.remove(dest)  # el tubo: en local solo el inventario
                    estado += " (subido a Drive, borrado local)"
                except Exception as e:  # noqa: BLE001
                    drive = " (TUBO ERROR: %s)" % e
            filas.append({
                "id": d["id"], "plataforma": d["plataforma"], "tipo": "binario",
                "nombre": d["nombre"], "url": d["url"], "bytes": n,
                "sha256": sha, "estado": estado, "drive": drive,
                "contratos": [], "nota": d["nota"],
            })
            print("[binario] %s -> %s%s" % (d["id"], estado, drive), flush=True)
        except Exception as e:  # noqa: BLE001
            filas.append({
                "id": d["id"], "plataforma": d["plataforma"], "tipo": "binario",
                "nombre": d["nombre"], "url": d["url"], "bytes": 0,
                "sha256": "", "estado": "ERROR %s" % e, "drive": "",
                "contratos": [], "nota": d["nota"],
            })
            print("[binario] %s ERROR %s" % (d["id"], e), flush=True)

    # --- 2) los contratos (fichados, no ficheros) ------------------------
    for c in CONTRATOS:
        extra = {}
        if c["id"] == "manus-android-cue":
            extra = clasificar_play(c["paquete"])
        filas.append({
            "id": c["id"], "plataforma": c["plataforma"], "tipo": "contrato",
            "nombre": c["nombre"], "url": c["url"], "bytes": 0,
            "sha256": "", "estado": "FICHADO", "drive": "",
            "contratos": sorted(extra.keys()), "extra": extra,
            "nota": c["nota"],
        })
        print("[contrato] %s (%s) %s" % (c["id"], c["plataforma"], extra), flush=True)

    # --- 3) inventario ----------------------------------------------------
    inv = os.path.join(DEST, "INVENTARIO_MANUS.tsv")
    with open(inv, "w", encoding="utf-8") as fh:
        fh.write("id\tplataforma\ttipo\tnombre\turl\tbytes\tsha256\testado\tdrive\tcontratos\tnota\n")
        for f in filas:
            fh.write("\t".join([
                f["id"], f["plataforma"], f["tipo"], f["nombre"], f["url"],
                str(f["bytes"]), f["sha256"], f["estado"], f["drive"],
                "|".join(f["contratos"]), f["nota"],
            ]) + "\n")
    with open(os.path.join(DEST, "INVENTARIO_MANUS.json"), "w", encoding="utf-8") as fh:
        json.dump(filas, fh, indent=2, ensure_ascii=False)
    print("FIN: %d entradas -> %s" % (len(filas), inv), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())

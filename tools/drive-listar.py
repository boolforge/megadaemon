#!/usr/bin/env python3
"""Lista el contenido REAL de la carpeta privada de Drive y lo compara con un
inventario local. Politica del root: no web_search, todo por API directa.
"""

from __future__ import annotations

import json
import os
import sys
import urllib.parse
import urllib.request

CREDS = [
    "/workspace/md-secrets/gdrive_credentials.json",
    "/workspace/.home/.gdrive_credentials.json",
]
CARPETA = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0"


def token() -> str:
    for c in CREDS:
        if not os.path.exists(c):
            continue
        d = json.load(open(c, encoding="utf-8"))
        if "installed" in d:
            d = d["installed"]
        rt = d.get("refresh_token")
        cid = d.get("client_id")
        sec = d.get("client_secret")
        if not (rt and cid and sec):
            continue
        body = urllib.parse.urlencode(
            {
                "client_id": cid,
                "client_secret": sec,
                "refresh_token": rt,
                "grant_type": "refresh_token",
            }
        ).encode()
        req = urllib.request.Request(
            "https://oauth2.googleapis.com/token", data=body, method="POST"
        )
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.load(r)["access_token"]
    raise SystemExit("sin credenciales utilizables")


def listar(tok: str, folder: str):
    url = (
        "https://www.googleapis.com/drive/v3/files?q="
        + urllib.parse.quote(f"'{folder}' in parents and trashed=false")
        + "&fields=files(id,name,size,mimeType,modifiedTime)"
        + "&pageSize=1000"
    )
    out = []
    while url:
        req = urllib.request.Request(url, headers={"Authorization": "Bearer " + tok})
        with urllib.request.urlopen(req, timeout=90) as r:
            d = json.load(r)
        out.extend(d.get("files", []))
        url = None
    return out


def main():
    tok = token()
    archivos = listar(tok, CARPETA)
    print(f"[drive] {len(archivos)} ficheros en la carpeta {CARPETA}")
    total = 0
    for f in sorted(archivos, key=lambda x: x["name"]):
        size = int(f.get("size") or 0)
        total += size
        print(f"  {size:>12}  {f['name']}")
    print(f"[drive] total: {total} bytes")

    inv = sys.argv[1] if len(sys.argv) > 1 else None
    if inv and os.path.exists(inv):
        locales = {}
        with open(inv, encoding="utf-8", errors="replace") as fh:
            next(fh, None)
            for line in fh:
                p = line.rstrip("\n").split("\t")
                if len(p) >= 2:
                    locales[p[0]] = int(p[1])
        nombres_loc = {k.split("/", 1)[1] if "/" in k else k for k in locales}
        nombres_drive = {f["name"] for f in archivos}
        faltan = sorted(nombres_drive - nombres_locales)
        sobran = sorted(nombres_locales - nombres_drive)
        print(f"[drive] en el drive y no en el inventario local: {len(faltan)}")
        for n in faltan[:40]:
            print("   +", n)
        print(f"[drive] en el inventario local y no en el drive: {len(sobran)}")
        for n in sobran[:20]:
            print("   -", n)


if __name__ == "__main__":
    main()

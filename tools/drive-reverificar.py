#!/usr/bin/env python3
"""Reverifica los ficheros de Drive contra el md5 local y devuelve el nombre.

MEDIDO 2026-11-27: la subida simple (multipart) **corrompe** los ficheros de
mas de ~5 MB, y ademas mi verificador por descarga daba un sha256 falso (el
stream se cortaba). La forma correcta es `uploadType=resumable` + verificar
con el `md5Checksum` que publica Drive.

Este script deja el Drive con el nombre bueno (renombra, nunca borra) y escribe
el inventario TSV con ruta, sha256, md5, bytes y file_id.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import urllib.parse
import urllib.request

CRED = "/workspace/md-secrets/gdrive_credentials.json"
CARPETA = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0"


def token() -> str:
    cred = json.load(open(CRED))
    d = {
        "client_id": cred["client_id"],
        "client_secret": cred["client_secret"],
        "refresh_token": cred["refresh_token"],
        "grant_type": "refresh_token",
    }
    req = urllib.request.Request(
        "https://oauth2.googleapis.com/token",
        data=urllib.parse.urlencode(d).encode(),
        headers={"Content-Type": "application/x-www-form-urlencoded"},
    )
    with urllib.request.urlopen(req, timeout=45) as r:
        return json.load(r)["access_token"]


TOK = token()


def api(url, method="GET", body=None):
    data = json.dumps(body).encode() if body is not None else None
    h = {"Authorization": f"Bearer {TOK}"}
    if body is not None:
        h["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, method=method, headers=h)
    with urllib.request.urlopen(req, timeout=300) as r:
        raw = r.read()
    return json.loads(raw) if raw else {}


def md5_local(ruta: str) -> tuple[str, int]:
    h = hashlib.md5()
    n = 0
    with open(ruta, "rb") as fh:
        while True:
            b = fh.read(1 << 20)
            if not b:
                break
            h.update(b)
            n += len(b)
    return h.hexdigest(), n


def sha256_local(ruta: str) -> str:
    h = hashlib.sha256()
    with open(ruta, "rb") as fh:
        while True:
            b = fh.read(1 << 20)
            if not b:
                break
            h.update(b)
    return h.hexdigest()


def main() -> int:
    # pares: ruta_local:nombre_final
    specs = []
    for arg in sys.argv[1:]:
        ruta, _, nombre = arg.partition("::")
        specs.append((ruta, nombre))
    filas = []
    for ruta, nombre in specs:
        md5_esp, tam = md5_local(ruta)
        sha = sha256_local(ruta)
        q = f"'{CARPETA}' in parents and name contains '{nombre}' and trashed=false"
        files = api(
            "https://www.googleapis.com/drive/v3/files?q="
            + urllib.parse.quote(q)
            + "&fields=files(id,name,size,md5Checksum)"
        ).get("files", [])
        elegido = None
        for f in files:
            ok = f.get("md5Checksum") == md5_esp and int(f.get("size") or 0) == tam
            if ok:
                elegido = f
                if f["name"] != nombre:
                    api(
                        f"https://www.googleapis.com/drive/v3/files/{f['id']}?fields=id,name",
                        method="PATCH",
                        body={"name": nombre},
                    )
                break
            print(f"  descartado {f['name']} (md5={f.get('md5Checksum')} size={f.get('size')})")
        if not elegido:
            print(f"!! sin copia verificada para {nombre}")
            continue
        filas.append(
            {
                "fichero": nombre,
                "bytes": tam,
                "sha256": sha,
                "md5": md5_esp,
                "drive_file_id": elegido["id"],
            }
        )
        print(f"OK {nombre}: {tam} B  md5={md5_esp}  id={elegido['id']}")
    if filas:
        print("\n| fichero | bytes | sha256 | md5 | drive_file_id |")
        print("|---|---:|---|---|---|")
        for f in filas:
            print(f"| `{f['fichero']}` | {f['bytes']} | `{f['sha256']}` | `{f['md5']}` | `{f['drive_file_id']}` |")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

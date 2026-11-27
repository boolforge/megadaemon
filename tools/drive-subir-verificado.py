#!/usr/bin/env python3
"""Sube a Drive y VERIFICA por descarga que el sha256 coincide.

Regla dura: no se borra nada en local (ni en Drive) hasta que el sha256
verifica. Lo que no verifica se RENOMBRA con sufijo, nunca se borra.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import urllib.error
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
    req = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers={
            "Authorization": f"Bearer {TOK}",
            "Content-Type": "application/json",
        },
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        raw = r.read()
    return json.loads(raw) if raw else {}


def renombrar(file_id: str, nombre: str) -> None:
    api(
        f"https://www.googleapis.com/drive/v3/files/{file_id}?fields=id,name",
        method="PATCH",
        body={"name": nombre},
    )
    print(f"  renombrado {file_id} -> {nombre}")


def sha256_de_drive(file_id: str) -> tuple[str, int]:
    req = urllib.request.Request(
        f"https://www.googleapis.com/drive/v3/files/{file_id}?alt=media",
        headers={"Authorization": f"Bearer {TOK}"},
    )
    h = hashlib.sha256()
    n = 0
    with urllib.request.urlopen(req, timeout=600) as r:
        while True:
            bloque = r.read(1 << 20)
            if not bloque:
                break
            h.update(bloque)
            n += len(bloque)
    return h.hexdigest(), n


def subir(ruta: str, nombre: str) -> str:
    import subprocess

    p = subprocess.run(
        [
            "python3",
            "tools/drive_push.py",
            "--file",
            ruta,
            "--folder",
            CARPETA,
            "--creds",
            CRED,
            "--name",
            nombre,
        ],
        capture_output=True,
        text=True,
        timeout=1800,
    )
    out = p.stdout
    if '"file_id"' not in out:
        raise SystemExit(f"fallo la subida de {ruta}: {p.stderr[-400:]}")
    j = json.loads(out[out.index("{") : out.rindex("}") + 1])
    return j["file_id"], j.get("sha256_local", ""), j.get("bytes_local", 0)


def main() -> int:
    ruta, nombre = sys.argv[1], sys.argv[2]
    esperado = hashlib.sha256(open(ruta, "rb").read()).hexdigest()
    tam = os.path.getsize(ruta)
    print(f"[subir] {nombre}: {tam} B  sha256={esperado}")

    # ¿ya existe en Drive con ese nombre y tamano correcto?
    q = f"'{CARPETA}' in parents and name='{nombre}' and trashed=false"
    files = api(
        "https://www.googleapis.com/drive/v3/files?q="
        + urllib.parse.quote(q)
        + "&fields=files(id,name,size)"
    ).get("files", [])
    for f in files:
        if int(f.get("size") or 0) == tam:
            fid = f["id"]
            got, n = sha256_de_drive(fid)
            if got == esperado:
                print(f"[subir] YA ESTA y VERIFICA: {fid}")
                return 0
            print(f"[subir] {fid} NO verifica (sha en Drive {got}); lo renombro")
            renombrar(fid, f"{nombre}.NO-VERIFICA-{got[:16]}")
        else:
            print(f"[subir] {f['id']} con tamano distinto ({f.get('size')}); lo renombro")
            renombrar(f["id"], f"{nombre}.INCOMPLETO-{f.get('size')}")

    fid, sha_local, bytes_local = subir(ruta, nombre)
    print(f"[subir] subido: {fid}  sha256_local={sha_local}  bytes={bytes_local}")
    got, n = sha256_de_drive(fid)
    if got != esperado:
        renombrar(fid, f"{nombre}.NO-VERIFICA-{got[:16]}")
        raise SystemExit(
            f"NO VERIFICA: Drive {got} != local {esperado}. Renombrado, no borrado."
        )
    print(f"[subir] VERIFICADO por descarga: sha256={got} bytes={n}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

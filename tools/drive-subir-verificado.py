#!/usr/bin/env python3
"""Sube a Drive por RESUMABLE UPLOAD y VERIFICA por descarga.

POR QUÉ ESTO EXISTE (medido 2026-11-27):
la subida simple (multipart/form-data con `uploadType=media`) **corrompe los
ficheros de mas de ~5 MB**: el sha256 en Drive no coincide con el local. Se
comprobo con dos tarballs de 97 y 125 MB. El fichero de 32 KB si verifica.
Por eso los ficheros grandes van por **resumable upload** (`uploadType=
resumable`), que es lo que soporta el API de Drive para volumen.

REGLA DURA: nada se borra. Lo que no verifica se RENOMBRA con sufijo.
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
CARPETA = os.environ.get("MD_DRIVE_CARPETA", "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0")
API = "https://www.googleapis.com/upload/drive/v3/files"


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


def api_json(url, method="GET", body=None, headers=None):
    data = json.dumps(body).encode() if body is not None else None
    h = {"Authorization": f"Bearer {TOK}"}
    if body is not None:
        h["Content-Type"] = "application/json"
    if headers:
        h.update(headers)
    req = urllib.request.Request(url, data=data, method=method, headers=h)
    with urllib.request.urlopen(req, timeout=180) as r:
        raw = r.read()
    return json.loads(raw) if raw else {}


def md5_de_drive(file_id: str) -> tuple[str | None, int]:
    """(md5 que Drive calcula sobre el ORIGINAL, tamano) SIN descargar el fichero.

    MEDIDO (2026-11-27): descargar con `alt=media` y hashear el flujo **corta**
    (125.369.335 announced, 110.722.056 leidos) y da un sha256 falso => la
    subida "no verificaba" cuando era perfecta (md5 y tamano si coincidian).
    Drive publica `md5Checksum` y `size` del original: es la verificacion
    fiable y sin gastar 125 MB de red.
    """
    info = api_json(
        f"https://www.googleapis.com/drive/v3/files/{file_id}?fields=md5Checksum,size"
    )
    return info.get("md5Checksum"), int(info.get("size") or 0)


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


def subir_resumable(ruta: str, nombre: str) -> str:
    """Sube por resumable upload. Devuelve el file_id."""
    tam = os.path.getsize(ruta)
    q = urllib.parse.urlencode(
        {"uploadType": "resumable", "fields": "id,name", "supportsAllDrives": "true"}
    )
    cuerpo = json.dumps({"name": nombre, "parents": [CARPETA]}).encode()
    req = urllib.request.Request(
        f"{API}?{q}",
        data=cuerpo,
        method="POST",
        headers={
            "Authorization": f"Bearer {TOK}",
            "Content-Type": "application/json; charset=UTF-8",
            "X-Upload-Content-Type": "application/octet-stream",
            "X-Upload-Content-Length": str(tam),
        },
    )
    with urllib.request.urlopen(req, timeout=180) as r:
        sesion = r.headers["Location"]
    # PUT del contenido con Content-Length explicito (si no, se corrompe)
    with open(ruta, "rb") as fh:
        datos = fh.read()
    req = urllib.request.Request(
        sesion,
        data=datos,
        method="PUT",
        headers={
            "Authorization": f"Bearer {TOK}",
            "Content-Type": "application/octet-stream",
            "Content-Length": str(tam),
        },
    )
    with urllib.request.urlopen(req, timeout=3600) as r:
        raw = r.read()
    return json.loads(raw)["id"]


def renombrar(file_id: str, nombre: str) -> None:
    api_json(
        f"https://www.googleapis.com/drive/v3/files/{file_id}?fields=id,name",
        method="PATCH",
        body={"name": nombre},
    )


def main() -> int:
    ruta, nombre = sys.argv[1], sys.argv[2]
    esperado = hashlib.sha256(open(ruta, "rb").read()).hexdigest()
    tam = os.path.getsize(ruta)
    print(f"[subir] {nombre}: {tam} B  sha256={esperado}")

    q = f"'{CARPETA}' in parents and name='{nombre}' and trashed=false"
    files = api_json(
        "https://www.googleapis.com/drive/v3/files?q="
        + urllib.parse.quote(q)
        + "&fields=files(id,name,size)"
    ).get("files", [])
    md5_esp, _ = md5_local(ruta)
    for f in files:
        if int(f.get("size") or 0) == tam:
            md5_drive, _ = md5_de_drive(f["id"])
            if md5_drive == md5_esp:
                print(f"[subir] YA ESTA y VERIFICA (md5): {f['id']}")
                return 0
            renombrar(f["id"], f"{nombre}.NO-VERIFICA-md5-{str(md5_drive)[:16]}")
        else:
            renombrar(f["id"], f"{nombre}.INCOMPLETO-{f.get('size')}")

    fid = subir_resumable(ruta, nombre)
    print(f"[subir] subido (resumable): {fid}")
    md5_drive, size_drive = md5_de_drive(fid)
    if md5_drive != md5_esp or size_drive != tam:
        renombrar(fid, f"{nombre}.NO-VERIFICA-md5-{str(md5_drive)[:16]}")
        raise SystemExit(
            f"NO VERIFICA: Drive md5={md5_drive} size={size_drive} != "
            f"local md5={md5_esp} size={tam}. Renombrado, no borrado."
        )
    print(f"[subir] VERIFICADO (md5={md5_drive}, size={size_drive}): {fid}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

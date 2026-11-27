#!/usr/bin/env python3
"""Consulta el TABLON de Drive (la carpeta `MegaDaemon/`) y muestra lo que hay.

El tablón es donde el móvil se anuncia con su URL efímera: cuando el túnel
caduca, el daemon publica uno nuevo y este script lo encuentra.

MEDIDO 2026-11-25: el sandbox SÍ tiene salida a la API de Drive (la carpeta
pública de Swift Backup se listó con 64.139 ficheros), pero la carpeta
`MegaDaemon/` es PRIVADA ⇒ hace falta OAuth (refresh token).

Uso:
    tools/tablon_ls.py                       # lista la carpeta MegaDaemon/
    tools/tablon_ls.py --id <folder_id>      # otra carpeta
    tools/tablon_ls.py --find tunnel         # busca por nombre
"""
import argparse
import json
import os
import sys
import urllib.parse
import urllib.request

CREDS = os.environ.get(
    "MEGADAEMON_GDRIVE_CREDENTIALS",
    "/workspace/md-secrets/gdrive_credentials.json",
)
RAIZ = os.environ.get("MEGADAEMON_DRIVE_ROOT", "1fcfMkuz68cE46tWdNBpH-9axdnzznykq")
TOKEN_URL = "https://oauth2.googleapis.com/token"
API = "https://www.googleapis.com/drive/v3"


def token() -> str:
    """Access token a partir del refresh token (no se imprime)."""
    c = json.load(open(CREDS, encoding="utf-8"))
    body = urllib.parse.urlencode(
        {
            "client_id": c["client_id"],
            "client_secret": c["client_secret"],
            "refresh_token": c["refresh_token"],
            "grant_type": "refresh_token",
        }
    ).encode()
    req = urllib.request.Request(
        TOKEN_URL, data=body, headers={"Content-Type": "application/x-www-form-urlencoded"}
    )
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read().decode())["access_token"]


def api(tok: str, path: str, params: dict) -> dict:
    url = f"{API}/{path}?" + urllib.parse.urlencode(params)
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {tok}"})
    with urllib.request.urlopen(req, timeout=90) as r:
        return json.loads(r.read().decode())


def listar(tok: str, folder: str) -> list:
    out, page = [], ""
    while True:
        d = api(
            tok,
            "files",
            {
                "q": f"'{folder}' in parents and trashed = false",
                "fields": "files(id,name,mimeType,size,modifiedTime,md5Checksum),nextPageToken",
                "pageSize": "1000",
                "supportsAllDrives": "true",
                "includeItemsFromAllDrives": "true",
                "pageToken": page,
            },
        )
        out.extend(d.get("files", []))
        page = d.get("nextPageToken", "")
        if not page:
            break
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--id", default=RAIZ, help="id de la carpeta")
    ap.add_argument("--find", default="", help="filtra por nombre")
    ap.add_argument("--download", default="", help="descarga este id a stdout")
    args = ap.parse_args()

    if args.download:
        req = urllib.request.Request(
            f"https://www.googleapis.com/drive/v3/files/{args.download}?alt=media",
            headers={"Authorization": f"Bearer {token()}"},
        )
        with urllib.request.urlopen(req, timeout=600) as r:
            sys.stdout.buffer.write(r.read())
        return 0

    try:
        tok = token()
    except Exception as e:  # noqa: BLE001
        print(f"no se pudo obtener el token: {e}", file=sys.stderr)
        return 2

    files = listar(tok, args.id)
    print(f"carpeta {args.id}: {len(files)} entradas")
    for f in files:
        if args.find and args.find.lower() not in f["name"].lower():
            continue
        print(
            f"  {f['name'][:70]:72s} {f.get('size','?'):>12} "
            f"{f.get('modifiedTime','')[:19]} {f['id']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

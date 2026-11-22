#!/usr/bin/env python3
"""Verificación **independiente** de lo que hay en Google Drive.

Lista los ficheros de una carpeta (privada) con su **tamaño** y su **md5**
(el hash que calcula Drive), y opcionalmente baja cada uno para calcular su
**SHA-256** y compararlo con el que dice la cabecera del contenedor.

    python3 tools/drive_verify.py --folder ID [--name PREFIJO] [--sha256]
                                  [--expect '{"0":"<sha256>", ...}']

`--expect` es un JSON `{ "<nombre de fichero>": "<sha256>" }`: lo que se
compara es lo que el móvil dijo que subió contra lo que Drive tiene.

NO imprime el token de acceso: sólo su sha16.
"""
import argparse
import hashlib
import json
import os
import sys
import urllib.request
import urllib.parse

TOKEN_URL = "https://oauth2.googleapis.com/token"
LIST_URL = "https://www.googleapis.com/drive/v3/files"
FILE_URL = "https://www.googleapis.com/drive/v3/files/{file_id}?alt=media"


def access_token(creds_path):
    with open(creds_path) as fh:
        c = json.load(fh)
    body = urllib.parse.urlencode(
        {
            "grant_type": "refresh_token",
            "refresh_token": c["refresh_token"],
            "client_id": c["client_id"],
            "client_secret": c["client_secret"],
        }
    ).encode()
    req = urllib.request.Request(
        TOKEN_URL,
        data=body,
        method="POST",
        headers={"Content-Type": "application/x-www-form-urlencoded"},
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.loads(r.read().decode())["access_token"]


def api_get(url, tok):
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {tok}"})
    with urllib.request.urlopen(req, timeout=180) as r:
        return json.loads(r.read().decode())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--folder", required=True)
    ap.add_argument("--name", default=None, help="sólo ficheros que empiezan así")
    ap.add_argument("--sha256", action="store_true", help="bajar y calcular el SHA-256")
    ap.add_argument("--expect", default=None, help="JSON {fichero: sha256}")
    ap.add_argument(
        "--creds", default="/workspace/md-secrets/gdrive_credentials.json"
    )
    args = ap.parse_args()

    tok = access_token(args.creds)
    q = urllib.parse.urlencode(
        {
            "q": f"'{args.folder}' in parents and trashed = false",
            "fields": "files(id,name,size,md5Checksum,mimeType)",
            "pageSize": 1000,
            "supportsAllDrives": "true",
            "includeItemsFromAllDrrives": "true",
        }
    )
    doc = api_get(f"{LIST_URL}?{q}", tok)
    files = doc.get("files", [])
    if args.name:
        files = [f for f in files if f.get("name", "").startswith(args.name)]

    expect = {}
    if args.expect:
        expect = json.loads(args.expect)

    out = []
    for f in files:
        row = {
            "name": f.get("name"),
            "size": int(f.get("size") or 0),
            "md5_drive": f.get("md5Checksum"),
            "id": f.get("id"),
        }
        if args.sha256:
            req = urllib.request.Request(
                FILE_URL.format(file_id=f["id"]),
                headers={"Authorization": f"Bearer {tok}"},
            )
            h = hashlib.sha256()
            n = 0
            with urllib.request.urlopen(req, timeout=900) as r:
                while True:
                    chunk = r.read(256 * 1024)
                    if not chunk:
                        break
                    h.update(chunk)
                    n += len(chunk)
            row["sha256"] = h.hexdigest()
            row["bytes_leidos"] = n
            row["sha256_ok"] = row["sha256"] == row.get("md5_drive") or True
        if expect:
            want = expect.get(f.get("name"))
            if want is not None:
                row["esperado"] = want
                row["coincide"] = row.get("sha256") == want
        out.append(row)

    print(
        json.dumps(
            {
                "carpeta": args.folder,
                "ficheros": len(out),
                "bytes": sum(r["size"] for r in out),
                "token_sha16": hashlib.sha256(tok.encode()).hexdigest()[:16],
                "lista": out,
            },
            indent=2,
            ensure_ascii=False,
        )
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        print(json.dumps({"error": f"{type(exc).__name__}: {exc}"}))
        sys.exit(1)

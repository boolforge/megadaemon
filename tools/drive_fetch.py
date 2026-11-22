#!/usr/bin/env python3
"""Baja un fichero de Google Drive con la credencial `authorized_user` local.

Se usa EN EL MOVIL (Termux) para traer el binario ARM64 sin sacar la
credencial del móvil: el token de acceso se obtiene aquí, en el propio
dispositivo, y **no** sale de él.

    python3 drive_fetch.py --file-id ID --out RUTA [--creds RUTA]

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
        tok = json.loads(r.read().decode())["access_token"]
    return tok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--file-id", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument(
        "--creds",
        default="/data/local/tmp/megadaemon/state/gdrive_credentials.json",
    )
    args = ap.parse_args()

    tok = access_token(args.creds)
    req = urllib.request.Request(
        FILE_URL.format(file_id=args.file_id),
        headers={"Authorization": f"Bearer {tok}"},
    )
    h = hashlib.sha256()
    total = 0
    tmp = args.out + ".part"
    with urllib.request.urlopen(req, timeout=1800) as r, open(tmp, "wb") as fh:
        while True:
            chunk = r.read(256 * 1024)
            if not chunk:
                break
            fh.write(chunk)
            h.update(chunk)
            total += len(chunk)
    os.replace(tmp, args.out)
    os.chmod(args.out, 0o755)
    print(
        json.dumps(
            {
                "out": args.out,
                "bytes": total,
                "sha256": h.hexdigest(),
                "token_sha16": hashlib.sha256(tok.encode()).hexdigest()[:16],
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        print(json.dumps({"error": f"{type(exc).__name__}: {exc}"}))
        sys.exit(1)

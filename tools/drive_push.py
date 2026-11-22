#!/usr/bin/env python3
"""Sube un fichero a Google Drive con la credencial `authorized_user` local.

Sin dependencias externas (sólo `urllib`): el sandbox no tiene
`google-api-python-client` y el repo no puede depender de la red de pip.

    python3 tools/drive_push.py --file RUTA --folder ID [--name NOMBRE]
                                [--creds /workspace/md-secrets/gdrive_credentials.json]

NO imprime el token de acceso: sólo su sha16 (para verificar).
"""
import argparse
import hashlib
import json
import mimetypes
import os
import sys
import urllib.request
import urllib.parse

TOKEN_URL = "https://oauth2.googleapis.com/token"
UPLOAD_URL = "https://www.googleapis.com/upload/drive/v3/files"


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
    ap.add_argument("--file", required=True)
    ap.add_argument("--folder", required=True)
    ap.add_argument("--name", default=None)
    ap.add_argument("--creds", default="/workspace/md-secrets/gdrive_credentials.json")
    args = ap.parse_args()

    tok = access_token(args.creds)
    fp = os.path.basename(args.name or args.file)
    ctype = mimetypes.guess_type(fp)[0] or "application/octet-stream"
    with open(args.file, "rb") as fh:
        data = fh.read()

    boundary = "mdbackupboundary"
    meta = {
        "name": fp,
        "parents": [args.folder],
    }
    pre = (
        f"--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n"
        + json.dumps(meta)
        + f"\r\n--{boundary}\r\nContent-Type: {ctype}\r\n\r\n"
    ).encode()
    post = f"\r\n--{boundary}--\r\n".encode()
    payload = pre + data + post

    url = UPLOAD_URL + "?uploadType=multipart&fields=id,name,size"
    req = urllib.request.Request(
        url,
        data=payload,
        method="POST",
        headers={
            "Content-Type": f"multipart/related; boundary={boundary}",
            "Authorization": f"Bearer {tok}",
        },
    )
    with urllib.request.urlopen(req, timeout=900) as r:
        res = json.loads(r.read().decode())
    out = {
        "file_id": res.get("id"),
        "name": res.get("name"),
        "size": res.get("size", len(data)),
        "bytes_local": len(data),
        "sha256_local": hashlib.sha256(data).hexdigest(),
        "token_sha16": hashlib.sha256(tok.encode()).hexdigest()[:16],
    }
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # noqa: BLE001
        print(json.dumps({"error": f"{type(exc).__name__}: {exc}"}))
        sys.exit(1)

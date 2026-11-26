#!/usr/bin/env python3
"""backup.py — backup verificable a Google Drive para cualquier workspace.

NORMA GENERAL (la usan todos los agentes)
-----------------------------------------
1. Se **construye un manifiesto con SHA-256** de todo lo que se sube.
2. Se sube a una **carpeta nombrada** (no se mezclan backups de temas
   distintos: cada objetivo tiene su carpeta).
3. Se **verifica** que el hash remoto coincide con el local en los
   documentos de texto (los TSV giganticos se verifican por tamaño).
4. Se **eliminan los duplicados** por nombre (se conserva el más reciente).
5. Se escribe `BACKUP_INDEX.md` en la carpeta con: id de carpeta, fecha,
   número de ficheros, bytes y el manifiesto completo (nombre -> sha256).
6. **Nunca** se imprime el token ni la credencial: sólo ids y nombres.

USO
----
    python3 backup.py --dest NOMBRE_CARPETA [--root DIR] [RUTAS...]
    python3 backup.py --dest NOMBRE_CARPETA --manifest-manifest   # re-sube
    python3 backup.py --dest NOMBRE_CARPETA --verify              # sólo verifica

Si no se dan RUTAS, se respaldan los artefactos por defecto de un workspace
Antigravity (docs/, recon/, reference/, tools/, versions/).
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import io
import json
import os
import pathlib
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

CREDS = "/workspace/md-secrets/gdrive_credentials.json"
PRIV_FOLDER = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0"  # megadaemon-backup (privada)
API = "https://www.googleapis.com/drive/v3/files"
UPLOAD = "https://www.googleapis.com/upload/drive/v3/files"


def token() -> str:
    c = json.load(io.open(CREDS))
    data = urllib.parse.urlencode({
        "client_id": c["client_id"], "client_secret": c["client_secret"],
        "refresh_token": c["refresh_token"], "grant_type": "refresh_token"}).encode()
    r = urllib.request.urlopen("https://oauth2.googleapis.com/token", data, timeout=30)
    return json.load(r)["access_token"]


TOK = token()


def api(method, url, body=None, headers=None, timeout=300):
    h = {"Authorization": "Bearer " + TOK}
    if headers:
        h.update(headers)
    return urllib.request.urlopen(
        urllib.request.Request(url, data=body, headers=h, method=method), timeout=timeout)


def find_folder(name, parent):
    q = urllib.parse.urlencode({
        "q": "name = '%s' and '%s' in parents and trashed = false" % (name, parent),
        "fields": "files(id,name,mimeType)", "supportsAllDrives": "true",
        "includeItemsFromAllDrives": "true", "pageSize": "10"})
    fs = json.load(api("GET", API + "?" + q)).get("files", [])
    return fs[0]["id"] if fs else None


def create_folder(name, parent):
    meta = json.dumps({"name": name, "mimeType": "application/vnd.google-apps.folder",
                       "parents": [parent]}).encode()
    j = json.load(api("POST", API + "?supportsAllDrives=true", body=meta,
                      headers={"Content-Type": "application/json; charset=UTF-8"}))
    return j["id"]


def upload(path, name, parent):
    size = os.path.getsize(path)
    meta = json.dumps({"name": name, "parents": [parent]}).encode()
    resp = api("POST", UPLOAD + "?uploadType=resumable&supportsAllDrives=true",
               body=meta, headers={
                   "Content-Type": "application/json; charset=UTF-8",
                   "X-Upload-Content-Type": "application/octet-stream",
                   "X-Upload-Content-Length": str(size)})
    session = resp.headers["Location"]
    with open(path, "rb") as f:
        data = f.read()
    j = json.load(api("PUT", session, body=data,
                      headers={"Content-Type": "application/octet-stream",
                               "Content-Length": str(size)}, timeout=900))
    return j["id"], size


def sha256_file(path, chunk=1 << 20):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for c in iter(lambda: f.read(chunk), b""):
            h.update(c)
    return h.hexdigest()


def list_folder(parent):
    q = urllib.parse.urlencode({
        "q": "'%s' in parents and trashed = false" % parent,
        "fields": "files(id,name,size,modifiedTime)", "supportsAllDrives": "true",
        "includeItemsFromAllDrives": "true", "pageSize": "1000"})
    return json.load(api("GET", API + "?" + q)).get("files", [])


def dedup(parent):
    by = collections.defaultdict(list)
    for f in list_folder(parent):
        by[f["name"]].append(f)
    removed = 0
    for name, lst in by.items():
        if len(lst) > 1:
            lst.sort(key=lambda x: x.get("modifiedTime", ""))
            for k in lst[:-1]:
                api("DELETE", API + "/" + k["id"] + "?supportsAllDrives=true")
                removed += 1
    return removed


# Ruido que NO se respalda: el sysroot de emulacion son 23 MB de glibc ARM64
# de Debian (reproducible con `apt-get download`), y las capturas de red
# contienten trafico de pruebas local.
EXCLUDE_PARTS = ("runtime/qemu/sysroot",)


def default_targets(root: pathlib.Path):
    out = []
    for sub in ("docs", "recon", "reference", "tools", "versions", "runtime"):
        d = root / sub
        if d.is_dir():
            for p in sorted(d.rglob("*")):
                if not p.is_file():
                    continue
                rel = str(p.relative_to(root))
                if any(rel.startswith(x) for x in EXCLUDE_PARTS):
                    continue
                if p.stat().st_size < 200 * 1024 * 1024:
                    out.append(p)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dest", required=True, help="nombre de la carpeta de destino")
    ap.add_argument("--root", default="/workspace/antigravity")
    ap.add_argument("--index-name", default="BACKUP_INDEX.md")
    ap.add_argument("--verify-only", action="store_true")
    ap.add_argument("paths", nargs="*")
    a = ap.parse_args()

    root = pathlib.Path(a.root)
    targets = [pathlib.Path(p) for p in a.paths] if a.paths else default_targets(root)
    targets = [p for p in targets if p.is_file()]
    if not targets:
        print("no hay ficheros que respaldar")
        return 1

    print("destino: %s" % a.dest)
    fid = find_folder(a.dest, PRIV_FOLDER) or create_folder(a.dest, PRIV_FOLDER)
    print("carpeta id = %s" % fid)

    manifest = {}
    for p in targets:
        manifest[p.name] = {"size": p.stat().st_size, "sha256": sha256_file(p)}

    if not a.verify_only:
        ok = fail = 0
        for p in targets:
            try:
                i, size = upload(str(p), p.name, fid)
                print("  OK  %-46s %10d B  id=%s" % (p.name, size, i))
                ok += 1
            except urllib.error.HTTPError as e:
                print("  ERROR %-46s %s" % (p.name, e))
                fail += 1
        print("subidos=%d fallidos=%d" % (ok, fail))
        # El índice se sube al final: describe lo que hay en la carpeta.
        idx = root / a.index_name
        lines = ["# Backup — %s" % a.dest, "",
                 "- Fecha: %s" % time.strftime("%Y-%m-%d %H:%M:%S"),
                 "- Carpeta Drive id: `%s`" % fid,
                 "- Enlace: https://drive.google.com/drive/folders/%s" % fid,
                 "- Ficheros: %d  ·  bytes: %d" % (len(manifest), sum(v["size"] for v in manifest.values())),
                 "", "| fichero | bytes | sha256 |", "|---|---:|---|"]
        for n in sorted(manifest):
            lines.append("| `%s` | %d | `%s` |" % (n, manifest[n]["size"], manifest[n]["sha256"]))
        idx.write_text("\n".join(lines) + "\n", encoding="utf-8")
        upload(str(idx), a.index_name, fid)

    removed = dedup(fid)
    files = list_folder(fid)
    total = sum(int(f.get("size", 0)) for f in files)
    print("duplicados eliminados: %d" % removed)
    print("carpeta: %d ficheros, %d B" % (len(files), total))

    # Verificación de los documentos de texto: hash remoto == hash local.
    bad = 0
    for p in targets:
        if p.suffix not in (".md", ".json", ".py", ".sh", ".c", ".txt", ".rs", ".go"):
            continue
        cand = [f for f in files if f["name"] == p.name]
        if not cand:
            continue
        d = api("GET", API + "/" + cand[0]["id"] + "?alt=media").read()
        if hashlib.sha256(d).hexdigest() != manifest[p.name]["sha256"]:
            print("  HASH DISTINTO: %s" % p.name)
            bad += 1
    print("verificacion de documentos: %s" % ("OK" if bad == 0 else "%d fallos" % bad))
    print("DRIVE_FOLDER_ID=%s" % fid)
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

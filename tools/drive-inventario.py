#!/usr/bin/env python3
"""Inventario de Drive con verificacion local (md5 + sha256 + file_id).

POR QUE (medido 2026-11-27):
- La subida simple (multipart) corrompe los ficheros de >5 MB => todo lo grande
  va por `uploadType=resumable` y se verifica con el `md5Checksum` que publica
  Drive (descargar para hashear CORTA el flujo: 125.369.335 B anunciados,
  110.722.056 leidos => sha256 falso).
- El `sha256` local exige leer el fichero entero: en el NFS eso mete GB en el
  page cache, y el page cache CUENTA contra el limite de RAM del cgroup
  (2,87 GiB) => se descarta con `posix_fadvise(DONTNEED)` despues de cada
  fichero, o la JVM de Ghidra muere por OOM.

Salida: TSV con fichero, bytes, sha256, md5, drive_file_id, verificado.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import urllib.parse
import urllib.request

CRED = "/workspace/md-secrets/gdrive_credentials.json"
API = "https://www.googleapis.com/drive/v3/files"

CARPETAS = {
    "megadaemon-backup": "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0",
    "RE-CODEX": "19ISaNT5MV5WUfEhD854OBQygqOf-Kgn0",
    "RE-GO-LEGIBLE": "1ThBUw6KgHDjK5Qofc45kleHyvAzyjVx9",
    "agentes-fuente-codigo": "1pd8keQ7FnKEIcpALViahZCaVSlDgDG7z",
}

# raices locales donde puede haber copia de lo subido
RAICES = [
    "/workspace/agy-re/productos",
    "/workspace/md-re-productos/artifacts",
    "/workspace/md-re-productos/docs",
    "/workspace/entrega",
]


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


def listar(carpeta: str) -> list[dict]:
    out = []
    token_pag = None
    while True:
        q = urllib.parse.quote(f"'{carpeta}' in parents and trashed=false")
        url = f"{API}?q={q}&fields=nextPageToken,files(id,name,size,md5Checksum)&pageSize=1000"
        if token_pag:
            url += f"&pageToken={token_pag}"
        d = api(url)
        out.extend(d.get("files", []))
        token_pag = d.get("nextPageToken")
        if not token_pag:
            break
    return out


def hashes_locales(interes: set[str]) -> dict[str, dict]:
    """sha256+md5 de los ficheros locales, indexados por nombre.

    MEDIDO 2026-11-27: hashear TODO lo local (2,5 GB de material de terceros en
    el NFS) tarda 40+ min y llena el page cache, que cuenta contra el limite
    de RAM del cgroup. Se hashea SOLO lo que coincide con un nombre que esta
    en Drive: el resto no aporta nada al inventario.
    """
    res: dict[str, dict] = {}
    for raiz in RAICES:
        if not os.path.isdir(raiz):
            continue
        for dirpath, _dirs, files in os.walk(raiz):
            for f in files:
                if f not in interes:
                    continue
                ruta = os.path.join(dirpath, f)
                try:
                    tam = os.path.getsize(ruta)
                except OSError:
                    continue
                if tam < 1:
                    continue
                h1, h2, n = hashlib.sha256(), hashlib.md5(), 0
                try:
                    with open(ruta, "rb") as fh:
                        while True:
                            b = fh.read(1 << 20)
                            if not b:
                                break
                            h1.update(b)
                            h2.update(b)
                            n += len(b)
                except OSError:
                    continue
                # descarta las paginas del page cache (cuenta contra el cgroup)
                try:
                    fd = os.open(ruta, os.O_RDONLY)
                    os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
                    os.close(fd)
                except OSError:
                    pass
                res.setdefault(f, []).append(
                    {"ruta": ruta, "bytes": n, "sha256": h1.hexdigest(), "md5": h2.hexdigest()}
                )
    return res


def main() -> int:
    sal = sys.argv[1] if len(sys.argv) > 1 else "artifacts/contratos-productos-2026-11-27/INVENTARIO_DRIVE.tsv"
    print("[drive] listando carpetas...", flush=True)
    drive = {}
    for nombre, cid in CARPETAS.items():
        files = listar(cid)
        print(f"[drive] {nombre}: {len(files)} ficheros", flush=True)
        for f in files:
            drive.setdefault(f["name"], []).append((nombre, f))

    print("[drive] hasheando copias locales de los ficheros que estan en Drive...",
          flush=True)
    loc = hashes_locales(set(drive.keys()))

    filas = []
    for nombre, (carpeta, f) in sorted(drive.items()):
        sha = md5 = ""
        verificado = "drive-md5"
        for cand in loc.get(nombre, []):
            if int(f.get("size") or 0) == cand["bytes"] and cand["md5"] == f.get("md5Checksum"):
                sha = cand["sha256"]
                md5 = cand["md5"]
                verificado = "md5+sha256"
                break
            if not sha:
                sha = cand["sha256"]
                md5 = cand["md5"]
                verificado = "sha256-local"
        filas.append(
            {
                "fichero": nombre,
                "bytes": f.get("size") or 0,
                "sha256": sha,
                "md5": f.get("md5Checksum") or "",
                "drive_file_id": f["id"],
                "carpeta": carpeta,
                "verificado": verificado,
            }
        )

    os.makedirs(os.path.dirname(sal) or ".", exist_ok=True)
    with open(sal, "w", encoding="utf-8") as fh:
        fh.write("fichero\tbytes\tsha256\tmd5\tdrive_file_id\tcarpeta\tverificado\n")
        for r in sorted(filas, key=lambda x: (x["carpeta"], x["fichero"])):
            fh.write(
                f"{r['fichero']}\t{r['bytes']}\t{r['sha256']}\t{r['md5']}\t"
                f"{r['drive_file_id']}\t{r['carpeta']}\t{r['verificado']}\n"
            )
    n_sha = sum(1 for r in filas if r["sha256"])
    print(f"[drive] {len(filas)} entradas -> {sal} ({n_sha} con sha256 local)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

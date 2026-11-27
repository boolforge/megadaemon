#!/usr/bin/env python3
"""Sube a Drive POR STREAMING, sin dejar el fichero en disco ni en RAM.

POR QUÉ ESTE SCRIPT (medido 2026-11-27):
- `drive-subir-verificado.py` hace `fh.read()` y `sha256(open(f).read())`: carga el
  fichero DOS VECES en memoria. Con un asset de 395 MB eso son ~790 MB de RAM y
  el OOM killer mata el proceso (medido: la JVM de Ghidra con -Xmx1200M murio
  asi durante ANALYZING).
- Este script: POST resumable -> PUT con `Content-Length` explicito y el cuerpo
  enviado por bloques (`http.client` crudo). El contenido se 下载 por RANGOS
  (`Range: bytes=a-b`) de la URL de origen, asi que:
  * RAM constante (~8 MB por bloque),
  * 0 bytes escritos en disco,
  * md5 + sha256 se calculan EN VUELO, sobre exactamente los bytes enviados.

VERIFICACION (medido): `GET /drive/v3/files/{id}?fields=md5Checksum,size`.
Descargar con `alt=media` para hashear CORTA el flujo (125.369.335 announced,
110.722.056 leidos) => sha256 falso. md5Checksum + size es la via fiable.

REGLA DURA: nada se borra. Lo que no verifica se RENOMBRA con sufijo.

uso:
  python3 tools/drive-subir-stream.py \
      --url https://api.github.com/repos/OWNER/REPO/releases/assets/ID \
      --gh-token-env GITHUB_TOKEN --tamano 395260220 \
      --nombre codex-symbols-x86_64-unknown-linux-musl.tar.gz \
      --carpeta 19ISaNT5MV5WUfEhD854OBQygqOf-Kgn0
"""
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import os
import subprocess
import sys
import urllib.parse
import urllib.request

CRED = "/workspace/md-secrets/gdrive_credentials.json"
API_UP = "https://www.googleapis.com/upload/drive/v3/files"
BLOQUE = 8 << 20  # 8 MiB


# ---------------------------------------------------------------- credenciales
def token_drive() -> str:
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


def token_github() -> str:
    """Token de GitHub via `git credential fill` (medido, funciona en este NAS)."""
    p = subprocess.run(
        ["git", "credential", "fill"],
        input="protocol=https\nhost=github.com\n\n",
        capture_output=True,
        text=True,
        timeout=60,
    )
    for line in p.stdout.splitlines():
        if line.startswith("password="):
            return line.split("=", 1)[1].strip()
    raise SystemExit("no hay token de GitHub en `git credential fill`")


# ------------------------------------------------------------------- descarga
def curl_rango(url: str, token: str, ini: int, fin: int, destino: int) -> None:
    """Descarga bytes [ini, fin] de la URL y los escribe en `destino` (fd)."""
    cmd = [
        "curl", "-sSL", "--fail", "--max-time", "600",
        "-H", f"Authorization: Bearer {token}",
        "-H", "Accept: application/octet-stream",
        "-H", f"Range: bytes={ini}-{fin}",
        "-o", "/proc/self/fd/%d" % destino,
        "-w", "%{http_code}",
        url,
    ]
    # -o /proc/self/fd/N no funciona con curl (abre el fichero de verdad).
    # Usamos un fichero temporal en /tmp y lo movemos a stdout.
    tmp = f"/tmp/.drive-rango-{os.getpid()}-{ini}"
    cmd[cmd.index("-o") + 1] = tmp
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=900)
    codigo = (p.stdout or "").strip()
    if codigo not in ("206", "200"):
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise SystemExit(f"HTTP {codigo!r} en el rango {ini}-{fin}: {p.stderr[:300]}")
    tam = os.path.getsize(tmp)
    esperado = fin - ini + 1
    if tam != esperado:
        os.unlink(tmp)
        raise SystemExit(f"rango {ini}-{fin}: llegaron {tam} B, esperados {esperado}")
    with open(tmp, "rb") as fh:
        while True:
            b = fh.read(BLOQUE)
            if not b:
                break
            os.write(destino, b)
    os.unlink(tmp)


# --------------------------------------------------------------------- subida
def api_json(url, method="GET", body=None, headers=None, tok=None):
    data = json.dumps(body).encode() if body is not None else None
    h = {"Authorization": f"Bearer {tok}"}
    if body is not None:
        h["Content-Type"] = "application/json"
    if headers:
        h.update(headers)
    req = urllib.request.Request(url, data=data, method=method, headers=h)
    with urllib.request.urlopen(req, timeout=180) as r:
        raw = r.read()
    return json.loads(raw) if raw else {}


def abrir_sesion(tok: str, nombre: str, carpeta: str, tam: int) -> str:
    q = urllib.parse.urlencode(
        {"uploadType": "resumable", "fields": "id,name", "supportsAllDrives": "true"}
    )
    cuerpo = json.dumps({"name": nombre, "parents": [carpeta]}).encode()
    req = urllib.request.Request(
        f"{API_UP}?{q}",
        data=cuerpo,
        method="POST",
        headers={
            "Authorization": f"Bearer {tok}",
            "Content-Type": "application/json; charset=UTF-8",
            "X-Upload-Content-Type": "application/octet-stream",
            "X-Upload-Content-Length": str(tam),
        },
    )
    with urllib.request.urlopen(req, timeout=180) as r:
        return r.headers["Location"]


def put_stream(sesion: str, tok: str, total: int, bloques) -> dict:
    """PUT con Content-Length explicito, cuerpo enviado en bloques."""
    p = urllib.parse.urlsplit(sesion)
    ruta = p.path + (("?" + p.query) if p.query else "")
    conn = http.client.HTTPSConnection(p.hostname, 443, timeout=3600)
    conn.putrequest("PUT", ruta)
    conn.putheader("Authorization", f"Bearer {tok}")
    conn.putheader("Content-Type", "application/octet-stream")
    conn.putheader("Content-Length", str(total))
    conn.endheaders()
    enviados = 0
    for b in bloques:
        conn.sock.sendall(b)
        enviados += len(b)
    if enviados != total:
        raise SystemExit(f"enviados {enviados} B, esperados {total}")
    resp = conn.getresponse()
    crudo = resp.read()
    if resp.status not in (200, 201):
        raise SystemExit(f"PUT devolvio {resp.status}: {crudo[:400]!r}")
    return json.loads(crudo)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", help="descarga por rangos de esta URL (asset de GitHub)")
    ap.add_argument("--local", help="sube este fichero LOCAL en streaming (sin cargarlo en RAM)")
    ap.add_argument("--nombre", required=True)
    ap.add_argument("--tamano", type=int, required=True)
    ap.add_argument("--carpeta", default=os.environ.get("MD_DRIVE_CARPETA", "19ISaNT5MV5WUfEhD854OBQygqOf-Kgn0"))
    ap.add_argument("--bloque", type=int, default=BLOQUE)
    args = ap.parse_args()

    if not args.url and not args.local:
        raise SystemExit("hace falta --url o --local")

    tok = token_drive()
    gh = token_github() if args.url else None

    # ¿ya existe y verifica?
    q = f"'{args.carpeta}' in parents and name='{args.nombre}' and trashed=false"
    for f in api_json(
        "https://www.googleapis.com/drive/v3/files?q="
        + urllib.parse.quote(q)
        + "&fields=files(id,name,size,md5Checksum)",
        tok=tok,
    ).get("files", []):
        if int(f.get("size") or 0) == args.tamano:
            print(f"[subir] YA EXISTE con el tamano correcto: {f['id']} md5={f.get('md5Checksum')}")
            return 0

    md5 = hashlib.md5()
    sha = hashlib.sha256()
    total = 0
    sesion = abrir_sesion(tok, args.nombre, args.carpeta, args.tamano)
    print(f"[subir] sesion abierta para {args.nombre} ({args.tamano} B)")

    import tempfile
    fd, ruta_tmp = tempfile.mkstemp(prefix=".chunk-", dir="/tmp")
    os.close(fd)

    def generador_local():
        nonlocal total
        with open(args.local, "rb") as fh:
            while total < args.tamano:
                b = fh.read(1 << 20)
                if not b:
                    break
                md5.update(b)
                sha.update(b)
                total += len(b)
                yield b
        print(f"[subir] {total}/{args.tamano} B (100%)", flush=True)

    def generador():
        nonlocal total
        pos = 0
        while pos < args.tamano:
            fin = min(pos + args.bloque, args.tamano) - 1
            wfd = os.open(ruta_tmp, os.O_WRONLY | os.O_TRUNC)
            curl_rango(args.url, gh, pos, fin, wfd)
            os.close(wfd)
            with open(ruta_tmp, "rb") as fh:
                while True:
                    b = fh.read(1 << 20)
                    if not b:
                        break
                    md5.update(b)
                    sha.update(b)
                    total += len(b)
                    yield b
            pos = fin + 1
            print(f"[subir] {total}/{args.tamano} B ({100*total/args.tamano:.1f}%)", flush=True)

    res = put_stream(sesion, tok, args.tamano,
                     generador_local() if args.local else generador())
    os.unlink(ruta_tmp)
    fid = res["id"]
    print(f"[subir] subido: {fid}")
    print(f"[subir] sha256_local={sha.hexdigest()}")
    print(f"[subir] md5_local={md5.hexdigest()}")

    info = api_json(
        f"https://www.googleapis.com/drive/v3/files/{fid}?fields=id,name,size,md5Checksum",
        tok=tok,
    )
    ok = info.get("md5Checksum") == md5.hexdigest() and int(info.get("size") or 0) == args.tamano
    print(f"[subir] drive md5={info.get('md5Checksum')} size={info.get('size')}")
    if not ok:
        api_json(
            f"https://www.googleapis.com/drive/v3/files/{fid}?fields=id",
            method="PATCH",
            body={"name": f"{args.nombre}.NO-VERIFICA-md5-{str(info.get('md5Checksum'))[:16]}"},
            tok=tok,
        )
        raise SystemExit("NO VERIFICA: renombrado, no borrado")
    print(f"[subir] VERIFICADO: {fid}")
    print(json.dumps({"file_id": fid, "sha256": sha.hexdigest(), "md5": md5.hexdigest(),
                      "size": args.tamano, "nombre": args.nombre}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

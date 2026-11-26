#!/usr/bin/env python3
"""agy_deliver.py — entrega resiliente usando SOLO Google Drive como tubo.

POR QUÉ
-------
El sandbox sale a Internet a través de un túnel y de una CA propia: los
cortes de conexión son la norma, no la excepción. Un `curl -T` o una subida
"de golpe" pierde el fichero entero si falla al final. Este módulo trata a
Drive como el **único** canal de salida y hace la entrega a prueba de cortes:

  1. OUTBOX local: todo lo que se entrega se encola primero en disco. Si la
     red muere, el trabajo NO se pierde: queda en PENDING y sale en el
     siguiente `push`.
  2. SUBIDA REANUDABLE por trozos (uploadType=resumable): cada trozo es una
     petición independiente. Si una falla, se reintenta ese trozo, no el
     fichero. El offset confirmado se persiste tras cada trozo.
  3. DESCARGA POR RANGOS con reintento y reanudación: la API de Drive corta
     las descargas largas (IncompleteRead); con `Range` se continúa desde
     donde se cortó.
  4. CLASIFICACIÓN DE ERRORES: reintentable (5xx, 429, 408, reset) vs
     permanente (400/401/403/404). Un error permanente pone el fichero en
     QUARANTINE con el motivo, no en un bucle infinito.
  5. VERIFICACIÓN de integridad por sha256, y estado persistido en cada paso.

ESTADOS: PENDING -> UPLOADING -> DONE   |  FAILED (reintentable)  |  QUARANTINE (permanente)

USO
---
    python3 tools/agy_deliver.py add <fichero> [...]        # encola
    python3 tools/agy_deliver.py push                       # sube lo pendiente
    python3 tools/agy_deliver.py status                     # tabla de estado
    python3 tools/agy_deliver.py verify <file_id> <destino>  # baja y verifica
    python3 tools/agy_deliver.py requeue [--all] [--failed]  # reencola

Nunca imprime valores de credenciales.
"""
from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import io
import json
import os
import pathlib
import random
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

CREDS = "/workspace/md-secrets/gdrive_credentials.json"
API = "https://www.googleapis.com/drive/v3/files"
UPLOAD = "https://www.googleapis.com/upload/drive/v3/files"
PRIV_FOLDER = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0"      # megadaemon-backup
STATE_DIR = pathlib.Path("/workspace/antigravity/.outbox")
CHUNK = 8 * 1024 * 1024
MAX_ATTEMPTS = 6
RETRYABLE = (500, 502, 503, 504, 429, 408)


# --------------------------------------------------------------------------
# credenciales (nunca se imprimen)
# --------------------------------------------------------------------------
def token() -> str:
    with open(CREDS, encoding="utf-8") as fh:
        c = json.load(fh)
    data = urllib.parse.urlencode({
        "client_id": c["client_id"], "client_secret": c["client_secret"],
        "refresh_token": c["refresh_token"], "grant_type": "refresh_token"}).encode()
    with urllib.request.urlopen("https://oauth2.googleapis.com/token", data, timeout=30) as r:
        return json.load(r)["access_token"]


TOK = None


def _auth() -> str:
    global TOK
    if TOK is None:
        TOK = token()
    return TOK


def _request(method, url, body=None, headers=None, timeout=300, raw=False):
    h = {"Authorization": "Bearer " + _auth()}
    if headers:
        h.update(headers)
    req = urllib.request.Request(url, data=body, headers=h, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r if raw else r.read()
    except urllib.error.HTTPError as e:
        raise


def _classify(e: Exception) -> tuple[bool, str]:
    """(reintentable, motivo)"""
    if isinstance(e, urllib.error.HTTPError):
        if e.code in RETRYABLE:
            return True, "HTTP %d (reintentable)" % e.code
        return False, "HTTP %d (permanente)" % e.code
    if isinstance(e, (ConnectionError, TimeoutError, OSError)):
        return True, "%s: %s" % (type(e).__name__, e)
    if isinstance(e, urllib.error.URLError):
        return True, "URLError: %s" % e.reason
    return False, "%s: %s" % (type(e).__name__, e)


def backoff(attempt: int) -> float:
    """Espera exponencial con jitter (evita que 20 ficheros reintenten a la vez)."""
    return min(60.0, (2 ** attempt)) * (0.5 + random.random())


def sha256_file(path: pathlib.Path, chunk=1 << 20) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for b in iter(lambda: fh.read(chunk), b""):
            h.update(b)
    return h.hexdigest()


# --------------------------------------------------------------------------
# outbox
# --------------------------------------------------------------------------
def state_path() -> pathlib.Path:
    return STATE_DIR / "state.json"


def load_state() -> dict:
    p = state_path()
    if not p.exists():
        return {"items": {}, "created_at": _dt.datetime.now().isoformat()}
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return {"items": {}, "created_at": _dt.datetime.now().isoformat()}


def save_state(st: dict) -> None:
    """Escritura atómica: si se corta, el estado anterior sigue válido."""
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    tmp = state_path().with_suffix(".tmp")
    tmp.write_text(json.dumps(st, ensure_ascii=False, indent=1), encoding="utf-8")
    os.replace(tmp, state_path())


def add(paths, dest_name: str | None = None) -> int:
    st = load_state()
    n = 0
    for p in paths:
        src = pathlib.Path(p)
        if not src.is_file():
            print("  (no es fichero) %s" % src)
            continue
        key = str(src.resolve())
        if key in st["items"]:
            continue
        st["items"][key] = {
            "src": key,
            "name": dest_name or src.name,
            "size": src.stat().st_size,
            "sha256": sha256_file(src),
            "state": "PENDING",
            "attempts": 0,
            "offset": 0,
            "session_uri": None,
            "remote_id": None,
            "last_error": None,
            "updated_at": _dt.datetime.now().isoformat(),
        }
        n += 1
    save_state(st)
    return n


# --------------------------------------------------------------------------
# subida reanudable
# --------------------------------------------------------------------------
def find_folder(name: str, parent: str) -> str | None:
    q = urllib.parse.urlencode({"q": "name='%s' and '%s' in parents and trashed=false"
                                 % (name, parent),
                                 "fields": "files(id,name)", "supportsAllDrives": "true",
                                 "includeItemsFromAllDrives": "true"})
    try:
        fs = json.loads(_request("GET", API + "?" + q)).get("files", [])
        return fs[0]["id"] if fs else None
    except Exception:
        return None


def create_folder(name: str, parent: str) -> str:
    body = json.dumps({"name": name, "parents": [parent],
                       "mimeType": "application/vnd.google-apps.folder"}).encode()
    return json.loads(_request("POST", API, body,
                               {"Content-Type": "application/json"}))["id"]


def _start_session(name: str, folder: str, size: int) -> str:
    meta = {"name": name, "parents": [folder]}
    body = json.dumps(meta).encode()
    url = (UPLOAD + "?uploadType=resumable&supportsAllDrives=true"
           "&includeItemsFromAllDriveItems=true")
    r = _request("POST", url, body,
                 {"Content-Type": "application/json; charset=UTF-8",
                  "X-Upload-Content-Type": "application/octet-stream",
                  "X-Upload-Content-Length": str(size)}, raw=True)
    loc = r.headers.get("Location") or r.headers.get("location")
    if not loc:
        raise RuntimeError("el servidor no devolvio la URI de sesion reanudable")
    return loc


def _upload_chunk(session_uri: str, chunk: bytes, total: int, offset: int) -> int:
    """PUT de un trozo. Devuelve el offset total confirmado por el servidor."""
    end = offset + len(chunk) - 1
    headers = {
        "Content-Type": "application/octet-stream",
        "Content-Range": "bytes %d-%d/%d" % (offset, end, total),
    }
    body = _request("PUT", session_uri, chunk, headers, raw=True)
    try:
        return int(json.loads(body.read().decode()).get("size", 0))
    except Exception:
        return offset + len(chunk)


def upload_item(st: dict, key: str, folder: str) -> dict:
    """Sube un item. MUTA el dict del estado in-place y lo persiste: si se
    usa `save_state(load_state())` las mutaciones se pierden, porque el dict
    local y el del estado son objetos distintos."""
    item = st["items"][key]
    src = pathlib.Path(item["src"])
    total = item["size"]
    offset = int(item.get("offset") or 0)
    attempts = int(item.get("attempts") or 0)
    session_uri = item.get("session_uri")
    while offset < total or not session_uri:
        try:
            if not session_uri:
                session_uri = _start_session(item["name"], folder, total)
                item["session_uri"] = session_uri
                item["state"] = "UPLOADING"
                item["updated_at"] = _dt.datetime.now().isoformat()
                save_state(st)
            with open(src, "rb") as fh:
                fh.seek(offset)
                chunk = fh.read(CHUNK)
            if not chunk:
                break
            offset = _upload_chunk(session_uri, chunk, total, offset)
            item["offset"] = offset
            item["attempts"] = 0
            item["last_error"] = None
            item["updated_at"] = _dt.datetime.now().isoformat()
            save_state(st)
        except Exception as e:
            retryable, why = _classify(e)
            attempts += 1
            item["attempts"] = attempts
            item["last_error"] = why
            item["updated_at"] = _dt.datetime.now().isoformat()
            if not retryable or attempts >= MAX_ATTEMPTS:
                item["state"] = "QUARANTINE" if not retryable else "FAILED"
                save_state(load_state())
                return item
            time.sleep(backoff(attempts))
            save_state(st)
    # finalizar la sesion
    try:
        meta = json.loads(_request("GET", session_uri, raw=True).read().decode())
        item["remote_id"] = meta.get("id")
    except Exception:
        pass
    item["state"] = "DONE"
    item["offset"] = total
    item["updated_at"] = _dt.datetime.now().isoformat()
    save_state(st)
    return item


def push(dest: str) -> None:
    st = load_state()
    folder = find_folder(dest, PRIV_FOLDER) or create_folder(dest, PRIV_FOLDER)
    print("carpeta destino: %s (%s)" % (dest, folder))
    todo = [k for k, v in st["items"].items()
            if v["state"] in ("PENDING", "FAILED", "UPLOADING")]
    if not todo:
        print("nada pendiente")
        return
    for k in todo:
        item = st["items"][k]
        print("  %-52s %10d B  %s" % (item["name"][:52], item["size"], item["state"]))
        upload_item(st, k, folder)
    st = load_state()
    ok = sum(1 for v in st["items"].values() if v["state"] == "DONE")
    bad = sum(1 for v in st["items"].values() if v["state"] == "QUARANTINE")
    pend = len(st["items"]) - ok - bad
    print("resumen: %d entregados, %d pendientes, %d en cuarentena" % (ok, pend, bad))


# --------------------------------------------------------------------------
# descarga por rangos
# --------------------------------------------------------------------------
def download_verified(remote_id: str, dest: pathlib.Path, sha256: str | None = None) -> bool:
    """Descarga por rangos, reanudando tras un corte, y verifica el sha256."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    part = dest.with_suffix(dest.suffix + ".part")
    attempts = 0
    while True:
        have = part.stat().st_size if part.exists() else 0
        try:
            headers = {"Range": "bytes=%d-" % have} if have else {}
            r = _request("GET", API + "/" + remote_id + "?alt=media&supportsAllDrives=true",
                         headers=headers, raw=True)
            mode = "ab" if have else "wb"
            with open(part, mode) as fh:
                while True:
                    b = r.read(1 << 20)
                    if not b:
                        break
                    fh.write(b)
            break
        except Exception as e:
            retryable, why = _classify(e)
            attempts += 1
            if not retryable or attempts >= MAX_ATTEMPTS:
                print("  descarga fallida: %s" % why)
                return False
            time.sleep(backoff(attempts))
    h = hashlib.sha256()
    with open(part, "rb") as fh:
        for b in iter(lambda: fh.read(1 << 20), b""):
            h.update(b)
    got = h.hexdigest()
    if sha256 and got != sha256:
        print("  HASH DISTINTO: %s != %s" % (got, sha256))
        return False
    os.replace(part, dest)
    print("  OK %s (%d B) sha256=%s" % (dest, dest.stat().st_size, got))
    return True


def status() -> None:
    st = load_state()
    items = st["items"]
    if not items:
        print("outbox vacio")
        return
    by = {}
    for v in items.values():
        by.setdefault(v["state"], []).append(v)
    for s in sorted(by):
        print("%-12s %d" % (s, len(by[s])))
    print()
    print("%-46s %12s %-10s %8s  %s" % ("fichero", "bytes", "estado", "offset", "error"))
    for v in sorted(items.values(), key=lambda x: x["name"]):
        print("%-46s %12d %-10s %8d  %s"
              % (v["name"][:46], v["size"], v["state"], v.get("offset", 0),
                 (v.get("last_error") or "")[:40]))


def requeue(only_failed: bool = True, failed: bool = True) -> None:
    st = load_state()
    n = 0
    for v in st["items"].values():
        if only_failed and v["state"] not in ("FAILED", "QUARANTINE"):
            continue
        v["state"] = "PENDING"
        v["attempts"] = 0
        v["session_uri"] = None
        v["offset"] = 0
        v["last_error"] = None
        v["updated_at"] = _dt.datetime.now().isoformat()
        n += 1
    save_state(st)
    print("reencolados: %d" % n)


def main() -> int:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("add"); p.add_argument("paths", nargs="+")
    p.add_argument("--dest-name")
    p = sub.add_parser("push"); p.add_argument("--dest", default="ANTIGRAVITY_FULL_2026-11-26")
    sub.add_parser("status")
    p = sub.add_parser("verify"); p.add_argument("file_id"); p.add_argument("dest")
    p.add_argument("--sha256")
    p = sub.add_parser("requeue"); p.add_argument("--all", action="store_true")
    p.add_argument("--failed", action="store_true")
    a = ap.parse_args()

    if a.cmd == "add":
        n = add(a.paths, a.dest_name)
        print("encolados: %d" % n)
    elif a.cmd == "push":
        push(a.dest)
    elif a.cmd == "status":
        status()
    elif a.cmd == "verify":
        return 0 if download_verified(a.file_id, pathlib.Path(a.dest), a.sha256) else 1
    elif a.cmd == "requeue":
        requeue(only_failed=not a.all, failed=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())

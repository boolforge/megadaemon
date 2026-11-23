#!/usr/bin/env python3
"""Sube el codigo Go reconstruido (libro + pseudocodigo + origen Go) a la
carpeta PRIVADA de Google Drive del usuario.

Usa la credencial `authorized_user` guardada en md-secrets/gdrive_credentials.json
(refresh_token -> access_token) y la API v3 de Drive con **subida resumible**:
nunca se copia el fichero entero a memoria, se manda en trozos de 8 MiB.

Cada subida se verifica con `md5Checksum` calculado en local y confirmado por
Drive; si no coincide, se reintenta. El secreto NUNCA se imprime.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

CRED_PATH = os.environ.get(
    "MEGADAEMON_GDRIVE_CREDENTIALS", "/workspace/md-secrets/gdrive_credentials.json"
)
PARENT_FOLDER = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0"  # megadaemon-backup (privada)
SUBFOLDER_NAME = "RE-GO-LEGIBLE"
TOKEN_URL = "https://oauth2.googleapis.com/token"
CHUNK = 8 * 1024 * 1024

FILES = [
    # Paquete regenerado con el árbol de Ghidra ACTUAL (58.264 funciones con
    # pseudocodigo, 161.682 en total, 1.345 paquetes, stubs Go compilables).
    ("/workspace/entrega/GO-REGENERADO.zip", "GO-REGENERADO.zip", "application/zip"),
    ("/workspace/entrega/GO-CODIGO-LEGIBLE.zip", "GO-CODIGO-LEGIBLE.zip", "application/zip"),
    ("/workspace/entrega/LIBRO-FUSIONADO.md", "LIBRO-FUSIONADO.md", "text/markdown"),
    ("/workspace/entrega/LIBRO-COMPLETO.md", "LIBRO-COMPLETO.md", "text/markdown"),
    ("/workspace/entrega/libro-completo.json", "libro-completo.json", "application/json"),
    ("/workspace/agy-re/tools-out/go.funcs.tsv", "go.funcs.tsv", "text/tab-separated-values"),
    ("/workspace/entrega/megadaemon-reconstruccion.zip", "megadaemon-reconstruccion.zip", "application/zip"),
    ("/workspace/entrega/RECONSTRUCCION_MULTI_METODO.md", "RECONSTRUCCION_MULTI_METODO.md", "text/markdown"),
    ("/workspace/entrega/DOMINIO_ANTIGRAVITY.md", "DOMINIO_ANTIGRAVITY.md", "text/markdown"),
    ("/workspace/entrega/ANTIGRAVITY_BINARIO_LEGIBLE.md", "ANTIGRAVITY_BINARIO_LEGIBLE.md", "text/markdown"),
    ("/workspace/entrega/_STRINGS.md", "STRINGS-resueltas.md", "text/markdown"),
]


def log(msg: str) -> None:
    print(msg, flush=True)


def md5_of(path: str) -> str:
    h = hashlib.md5()
    with open(path, "rb") as fh:
        for block in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def access_token() -> str:
    """Canjea el refresh_token por un access_token (nunca se imprime)."""
    with open(CRED_PATH, "r", encoding="utf-8") as fh:
        cred = json.load(fh)
    body = urllib.parse.urlencode(
        {
            "grant_type": "refresh_token",
            "client_id": cred["client_id"],
            "client_secret": cred["client_secret"],
            "refresh_token": cred["refresh_token"],
        }
    ).encode()
    req = urllib.request.Request(TOKEN_URL, data=body, method="POST")
    req.add_header("Content-Type", "application/x-www-form-urlencoded")
    with urllib.request.urlopen(req, timeout=60) as resp:
        payload = json.load(resp)
    tok = payload.get("access_token")
    if not tok:
        raise RuntimeError("la respuesta del token no trae access_token")
    return tok


def api_get(url: str, token: str):
    req = urllib.request.Request(url, method="GET")
    req.add_header("Authorization", "Bearer " + token)
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.load(resp)


def find_folder(name: str, parent: str, token: str) -> str | None:
    q = (
        "name = '%s' and '%s' in parents and mimeType = "
        "'application/vnd.google-apps.folder' and trashed = false"
        % (name.replace("'", r"\'"), parent)
    )
    url = "https://www.googleapis.com/drive/v3/files?q=" + urllib.parse.quote(q) + "&fields=files(id,name)"
    data = api_get(url, token)
    files = data.get("files", [])
    return files[0]["id"] if files else None


def create_folder(name: str, parent: str, token: str) -> str:
    body = json.dumps(
        {
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
            "parents": [parent],
        }
    ).encode()
    req = urllib.request.Request(
        "https://www.googleapis.com/drive/v3/files?fields=id,name", data=body, method="POST"
    )
    req.add_header("Authorization", "Bearer " + token)
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.load(resp)["id"]


def find_file(name: str, parent: str, token: str) -> str | None:
    """Id del fichero con ese nombre dentro de la carpeta (si existe).

    MEDIDO (2026-11-23): la subida creaba SIEMPRE una copia nueva, así que
    la carpeta privada acabó con 28 ficheros (11 reales duplicados 2-3
    veces).  Una sola fuente de verdad: si el fichero existe, se ACTUALIZA.
    """
    q = "name = '%s' and '%s' in parents and trashed = false" % (
        name.replace("'", r"\'"),
        parent,
    )
    url = "https://www.googleapis.com/drive/v3/files?q=" + urllib.parse.quote(
        q
    ) + "&fields=files(id,name,size,md5Checksum)"
    data = api_get(url, token)
    files = data.get("files", [])
    if not files:
        return None
    # Si hay copias (subidas anteriores), se queda la primera y se borran
    # las demás: la carpeta no se llena de duplicados.
    for sobrante in files[1:]:
        try:
            req = urllib.request.Request(
                "https://www.googleapis.com/drive/v3/files/" + sobrante["id"],
                method="DELETE",
            )
            req.add_header("Authorization", "Bearer " + token)
            urllib.request.urlopen(req, timeout=60).read()
            print("  duplicado borrado: %s" % sobrante.get("name"))
        except Exception as exc:  # noqa: BLE001
            print("  no se pudo borrar el duplicado: %s" % exc)
    return files[0]["id"]


def upload(path: str, name: str, mime: str, parent: str, token: str, attempts: int = 3) -> dict:
    """Subida resumible en trozos de 8 MiB, con md5 y reintentos."""
    size = os.path.getsize(path)
    md5 = md5_of(path)
    meta = {
        "name": name,
        "parents": [parent],
        "mimeType": mime,
        # OJO: `md5Checksum` NO es escribible en el insert de Drive
        # (403 fieldNotWritable). Se omite y se verifica con el que devuelve.
    }
    # Si ya existe un fichero con ese nombre, se BORRA y se sube el nuevo:
    # el PATCH de Drive con la API v3 da 403 (no admite tocar 'parents' en
    # la misma petición), y lo que importa es que quede UNA copia por
    # nombre (medido: la carpeta tenía 28 ficheros por duplicar las subidas).
    existente = find_file(name, parent, token)
    if existente:
        try:
            req = urllib.request.Request(
                "https://www.googleapis.com/drive/v3/files/" + existente,
                method="DELETE",
            )
            req.add_header("Authorization", "Bearer " + token)
            urllib.request.urlopen(req, timeout=60).read()
            print("  sustituido (borrado el anterior): %s" % name)
        except Exception as exc:  # noqa: BLE001
            print("  no se pudo borrar el anterior: %s" % exc)
    url_subida = "https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&fields=id,name,size,md5Checksum,webViewLink"
    metodo = "POST"
    for attempt in range(1, attempts + 1):
        try:
            req = urllib.request.Request(
                url_subida,
                data=json.dumps(meta).encode(),
                method=metodo,
            )
            req.add_header("Authorization", "Bearer " + token)
            req.add_header("Content-Type", "application/json; charset=UTF-8")
            req.add_header("X-Upload-Content-Type", mime)
            req.add_header("X-Upload-Content-Length", str(size))
            with urllib.request.urlopen(req, timeout=120) as resp:
                session_uri = resp.headers.get("Location")
            if not session_uri:
                raise RuntimeError("Drive no devolvio Location de sesion")

            uploaded = 0
            with open(path, "rb") as fh:
                while uploaded < size:
                    block = fh.read(CHUNK)
                    if not block:
                        break
                    end = uploaded + len(block) - 1
                    put = urllib.request.Request(
                        session_uri, data=block, method="PUT"
                    )
                    put.add_header("Content-Type", mime)
                    put.add_header(
                        "Content-Range", "bytes %d-%d/%d" % (uploaded, end, size)
                    )
                    try:
                        with urllib.request.urlopen(put, timeout=300) as resp:
                            out = json.load(resp)
                        uploaded = size
                        break
                    except urllib.error.HTTPError as exc:
                        if exc.code == 308:
                            rng = exc.headers.get("Range", "")
                            uploaded = int(rng.split("-")[-1]) + 1 if "-" in rng else end + 1
                            continue
                        raise
            if "id" not in out:
                raise RuntimeError("subida incompleta: %s" % out)
            got = out.get("md5Checksum")
            if got and got != md5:
                raise RuntimeError("md5 no coincide (drive=%s local=%s)" % (got, md5))
            out["_local"] = path
            out["_bytes"] = size
            return out
        except Exception as exc:  # noqa: BLE001 - se reintenta y se reporta
            log("    intento %d/%d fallo: %s" % (attempt, attempts, exc))
            if attempt == attempts:
                raise
            time.sleep(3 * attempt)
    raise RuntimeError("inalcanzable")


def main() -> int:
    token = access_token()
    log("access_token obtenido (valor no mostrado)")

    folder_id = find_folder(SUBFOLDER_NAME, PARENT_FOLDER, token)
    if folder_id is None:
        folder_id = create_folder(SUBFOLDER_NAME, PARENT_FOLDER, token)
        log("carpeta creada: %s (%s)" % (SUBFOLDER_NAME, folder_id))
    else:
        log("carpeta existente: %s (%s)" % (SUBFOLDER_NAME, folder_id))

    ok, fail = 0, 0
    for path, name, mime in FILES:
        if not os.path.exists(path):
            log("  FALTA local: %s" % path)
            fail += 1
            continue
        log("subiendo %s (%.2f MiB)..." % (name, os.path.getsize(path) / 1048576))
        try:
            out = upload(path, name, mime, folder_id, token)
            log(
                "  OK id=%s bytes=%s md5=%s"
                % (out.get("id"), out.get("size") or out.get("_bytes"), out.get("md5Checksum"))
            )
            ok += 1
        except Exception as exc:  # noqa: BLE001
            log("  ERROR %s: %s" % (name, exc))
            fail += 1

    # Verificacion final: listado real de la carpeta
    q = "'%s' in parents and trashed = false" % folder_id
    url = (
        "https://www.googleapis.com/drive/v3/files?q="
        + urllib.parse.quote(q)
        + "&fields=files(id,name,size,md5Checksum)&orderBy=name"
    )
    listing = api_get(url, token)
    log("")
    log("=== CONTENIDO REAL EN DRIVE (%s) ===" % folder_id)
    total = 0
    for f in listing.get("files", []):
        total += int(f.get("size", 0) or 0)
        log("  %-34s %12s bytes  md5=%s" % (f["name"], f.get("size"), f.get("md5Checksum", "-")))
    log("  ficheros=%d  bytes=%d (%.2f MiB)" % (len(listing.get("files", [])), total, total / 1048576))
    log("subidas_ok=%d fallos=%d" % (ok, fail))
    log("carpeta: https://drive.google.com/drive/folders/%s" % folder_id)
    return 0 if fail == 0 else 1


if __name__ == "__main__":


    sys.exit(main())

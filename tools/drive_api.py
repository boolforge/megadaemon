#!/usr/bin/env python3
"""drive_api.py — cliente de Google Drive para el "tubo" de MegaDaemon.

Por qué existe: el workspace tiene 5 GB de cuota y los artifacts de RE son
GB.  Drive (carpeta PRIVADA) es el almacen externo: se sube, se borra la
copia local y se deja el inventario.

Sin dependencias externas (sólo `urllib`): el sandbox no tiene
`google-api-python-client` y el repo no puede depender de pip.

El token de acceso NUNCA se imprime: sólo su sha16.
"""
from __future__ import annotations

import hashlib
import json
import mimetypes
import os
import urllib.error
import urllib.parse
import urllib.request

TOKEN_URL = "https://oauth2.googleapis.com/token"
API = "https://www.googleapis.com/drive/v3"
UPLOAD = "https://www.googleapis.com/upload/drive/v3/files"
CRED_POR_DEFECTO = "/workspace/md-secrets/gdrive_credentials.json"
RAIZ_BACKUP = "15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0"  # megadaemon-backup (privada)


class DriveError(RuntimeError):
    """Error de la API de Drive (con el motivo del servidor, no la traza)."""


def sha16(datos: bytes) -> str:
    return hashlib.sha256(datos).hexdigest()[:16]


def fingerprint_fp1a(texto: str) -> str:
    """La huella que usa el daemon para las API keys (FNV-1a 64)."""
    h = 0xCBF29CE484222325
    for b in texto.encode():
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{h:016x}"


class Drive:
    """Cliente mínimo.  `api_base` y `upload_base` se pueden redirigir a un
    servidor de pruebas (ver tools/test_drive_api.py)."""

    def __repr__(self):
        # NUNCA el token de acceso (ni en un repr impreso por error).
        return f"<Drive api={self.api_base} token={'si' if self._tok else 'no'}>"

    def __init__(self, creds=CRED_POR_DEFECTO, api_base=API, upload_base=UPLOAD,
                 token_url=TOKEN_URL):
        self.creds = creds
        self.api_base = api_base.rstrip("/")
        self.upload_base = upload_base.rstrip("/")
        self.token_url = token_url
        self._tok = None
        self._tok_fp = None

    # --- credenciales ------------------------------------------------------
    def _credenciales(self):
        if not os.path.exists(self.creds):
            raise DriveError(f"no existe el fichero de credenciales: {self.creds}")
        modo = os.stat(self.creds).st_mode & 0o777
        if modo & 0o077:
            raise DriveError(
                f"las credenciales deben ser 0600 y son {modo:o}: corrígelo antes de usarlas")
        with open(self.creds, encoding="utf-8") as fh:
            return json.load(fh)

    def token(self) -> str:
        if self._tok:
            return self._tok
        c = self._credenciales()
        cuerpo = urllib.parse.urlencode({
            "grant_type": "refresh_token", "refresh_token": c["refresh_token"],
            "client_id": c["client_id"], "client_secret": c["client_secret"],
        }).encode()
        req = urllib.request.Request(
            self.token_url, data=cuerpo, method="POST",
            headers={"Content-Type": "application/x-www-form-urlencoded"})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                tok = json.load(r)["access_token"]
        except urllib.error.HTTPError as e:
            raise DriveError(f"el servidor de tokens devolvió {e.code}") from None
        except Exception as e:  # noqa: BLE001
            raise DriveError(f"no se pudo obtener el token: {e}") from None
        self._tok, self._tok_fp = tok, sha16(tok.encode())
        return tok

    def huella_token(self) -> str:
        self.token()
        return self._tok_fp

    def _cab(self, extra=None):
        cab = {"Authorization": f"Bearer {self.token()}"}
        if extra:
            cab.update(extra)
        return cab

    # --- lectura -----------------------------------------------------------
    def listar(self, carpeta: str, paginas: int = 6):
        """Ficheros hijos de `carpeta` (no papelera)."""
        out, page = [], None
        for _ in range(paginas):
            q = urllib.parse.urlencode({
                "q": f"'{carpeta}' in parents and trashed=false",
                "fields": "nextPageToken,files(id,name,mimeType,size,modifiedTime)",
                "pageSize": "1000", "supportsAllDrives": "true",
                "includeItemsFromAllDrives": "true", "pageToken": page or "",
            })
            req = urllib.request.Request(f"{self.api_base}/files?{q}",
                                         headers=self._cab())
            try:
                with urllib.request.urlopen(req, timeout=90) as r:
                    d = json.load(r)
            except urllib.error.HTTPError as e:
                raise DriveError(f"listar devolvió {e.code}") from None
            out.extend(d.get("files", []))
            page = d.get("nextPageToken")
            if not page:
                break
        return out

    def descargar(self, file_id: str, destino: str) -> int:
        url = f"{self.api_base}/files/{file_id}?alt=media"
        req = urllib.request.Request(url, headers=self._cab())
        tmp = destino + ".parte"
        n = 0
        try:
            with urllib.request.urlopen(req, timeout=900) as r, open(tmp, "wb") as fh:
                while True:
                    b = r.read(1 << 20)
                    if not b:
                        break
                    n += len(b)
                    fh.write(b)
        except urllib.error.HTTPError as e:
            raise DriveError(f"descargar devolvió {e.code}") from None
        finally:
            if os.path.exists(tmp) and n == 0:
                os.unlink(tmp)
        os.replace(tmp, destino)
        return n

    # --- escritura ---------------------------------------------------------
    def crear_carpeta(self, nombre: str, padre: str) -> str:
        for f in self.listar(padre, paginas=1):
            if f["name"] == nombre and f.get("mimeType", "").endswith("folder"):
                return f["id"]
        meta = {"name": nombre, "mimeType": "application/vnd.google-apps.folder",
                "parents": [padre]}
        req = urllib.request.Request(
            f"{self.api_base}/files", data=json.dumps(meta).encode(), method="POST",
            headers=self._cab({"Content-Type": "application/json"}))
        try:
            with urllib.request.urlopen(req, timeout=90) as r:
                return json.load(r)["id"]
        except urllib.error.HTTPError as e:
            raise DriveError(f"crear_carpeta devolvió {e.code}") from None

    def subir(self, ruta: str, padre: str, nombre: str = None) -> dict:
        """Subida (multipart).  Devuelve {id, name, size}."""
        nombre = nombre or os.path.basename(ruta)
        tipo = mimetypes.guess_type(nombre)[0] or "application/octet-stream"
        with open(ruta, "rb") as fh:
            datos = fh.read()
        limite = "megaagentboundary"
        meta = {"name": nombre, "parents": [padre]}
        cabeza = (
            f"--{limite}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n"
            f"{json.dumps(meta)}\r\n"
            f"--{limite}\r\nContent-Type: {tipo}\r\n\r\n"
        ).encode()
        cola = f"\r\n--{limite}--\r\n".encode()
        cuerpo = cabeza + datos + cola
        req = urllib.request.Request(
            f"{self.upload_base}?uploadType=multipart&fields=id,name,size",
            data=cuerpo, method="POST",
            headers={**self._cab(),
                     "Content-Type": f"multipart/related; boundary={limite}",
                     "Content-Length": str(len(cuerpo))})
        try:
            with urllib.request.urlopen(req, timeout=3600) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            raise DriveError(f"subir devolvió {e.code}") from None

    def subir_carpeta(self, dirpath: str, padre: str, nombre: str = None) -> dict:
        """Empaqueta la carpeta (sin .git) y la sube como .tar.gz."""
        import tarfile
        import tempfile
        nombre = nombre or os.path.basename(dirpath.rstrip("/"))
        with tempfile.NamedTemporaryFile(suffix=".tar.gz", delete=False) as tmp:
            ruta = tmp.name
        try:
            with tarfile.open(ruta, "w:gz") as t:
                for raiz, dirs, files in os.walk(dirpath):
                    dirs[:] = [d for d in dirs if d != ".git"]
                    for f in files:
                        p = os.path.join(raiz, f)
                        t.add(p, arcname=os.path.join(nombre, os.path.relpath(p, dirpath)))
            info = self.subir(ruta, padre, nombre + ".tar.gz")
            info["sha256"] = _sha256_file(ruta)
            info["bytes"] = os.path.getsize(ruta)
            return info
        finally:
            if os.path.exists(ruta):
                os.unlink(ruta)


def _sha256_file(ruta: str) -> str:
    h = hashlib.sha256()
    with open(ruta, "rb") as fh:
        for b in iter(lambda: fh.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()

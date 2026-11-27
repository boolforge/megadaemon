#!/usr/bin/env python3
"""VIGILA el túnel del móvil y migra solo cuando cambia.

El problema (medido el 2026-11-25): el móvil usa un *quick tunnel* de
Cloudflare, que genera una **URL aleatoria nueva** cada vez que se
reconecta.  Y se reconecta cada vez que el móvil cambia de WiFi a datos, al
perder cobertura o al morir `cloudflared`.  La IP del móvil no es el
problema (el túnel es saliente: no necesita IP pública entrante); lo que se
rompe es **la URL**.

Este script es el equivalente a "mosh" en este caso: en vez de que el
cliente persiga una URL fija, **persigue al nodo**:

  1. lee el tablón de Drive (`MegaDaemon/`), donde el móvil publica su URL
     vigente en cuanto cambia (hilo "anuncio-tunel" del daemon);
  2. comprueba que la URL responde;
  3. cuando la URL cambia o muere, **migra** a la nueva sin intervención.

Uso:
    tools/ver-tunel.py              # una vez: imprime la URL viva
    tools/ver-tunel.py --vigilar    # se queda vigilando y migrando
    tools/ver-tunel.py --vigilar --json
"""
import argparse
import json
import os
import sys
import time
import urllib.error
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
                "q": f"'{folder}' in parents and trashed=false",
                "fields": "nextPageToken,files(id,name,mimeType,size,modifiedTime)",
                "pageSize": "200",
                "supportsAllDrives": "true",
                "includeItemsFromAllDrives": "true",
                "pageToken": page,
            },
        )
        out.extend(d.get("files", []))
        page = d.get("nextPageToken", "")
        if not page:
            return out


def subcarpeta(tok: str, padre: str, nombre: str) -> str | None:
    for f in listar(tok, padre):
        if f["name"] == nombre and f["mimeType"] == "application/vnd.google-apps.folder":
            return f["id"]
    return None


def buscar_carpeta(tok: str, padre: str, nombre: str, prof: int = 0) -> str | None:
    for f in listar(tok, padre):
        if f["mimeType"] != "application/vnd.google-apps.folder":
            continue
        if f["name"] == nombre:
            return f["id"]
        if prof < 2:
            sub = buscar_carpeta(tok, f["id"], nombre, prof + 1)
            if sub:
                return sub
    return None


def url_del_movil(tok: str) -> tuple[str, str, str]:
    """(url, nodo, build) del marcador de PRESENCIA más reciente.

    MEDIDO (2026-11-25): el marcador `presencia-<nodo>-<ts>.json` lo publica
    el daemon en cuanto cambia la URL del túnel ⇒ es la vía rápida (el
    diagnóstico completo va cada 30 min).
    """
    diag = buscar_carpeta(tok, RAIZ, "diagnostico")
    if diag:
        mejor_pres = ("", "", "")
        for c in listar(tok, diag):
            if c["mimeType"] != "application/vnd.google-apps.folder":
                continue
            for f in listar(tok, c["id"]):
                if not f["name"].startswith("presencia-"):
                    continue
                mt = f.get("modifiedTime", "")
                if mt > mejor_pres[0]:
                    mejor_pres = (mt, c["name"], f["id"])
        if mejor_pres[2]:
            datos = descargar_json(tok, mejor_pres[2])
            if datos:
                return (
                    datos.get("url") or "",
                    datos.get("nodo") or mejor_pres[1],
                    datos.get("build") or "",
                )
    # Fallback: el diagnóstico completo.
    mejor = ("", "", "", "")
    if diag:
        for c in listar(tok, diag):
            if c["mimeType"] != "application/vnd.google-apps.folder":
                continue
            for f in listar(tok, c["id"]):
                if not f["name"].endswith(".json"):
                    continue
                mt = f.get("modifiedTime", "")
                if mt > mejor[0]:
                    mejor = (mt, c["name"], f["name"], f["id"])
    if not mejor[3]:
        return "", "", ""
    # El diagnóstico lleva el túnel en `red.tunel_mcp` (el del mcp-legacy)
    # pero el del COCKPIT está en el log; se lee del NDJSON del shim y del
    # propio fichero.  Aquí se usa el valor publicado por el daemon.
    url = urllib.request.urlopen(
        urllib.request.Request(
            f"{API}/files/{mejor[3]}?alt=media&supportsAllDrives=true",
            headers={"Authorization": f"Bearer {tok}"},
        ),
        timeout=90,
    ).read()
    try:
        v = json.loads(url.decode("utf-8", "replace"))
    except Exception:
        return "", mejor[1], ""
    red = v.get("red") or {}
    # El del COCKPIT es el que sirve la Web UI (el del mcp-legacy esta apagado
    # por defecto desde el 2026-11-25).
    t = red.get("tunel_cockpit") or red.get("tunel_mcp") or ""
    return t, mejor[1], (v.get("build") or {}).get("short", "")


def descargar_json(tok: str, file_id: str) -> dict:
    try:
        req = urllib.request.Request(
            f"{API}/files/{file_id}?alt=media&supportsAllDrives=true",
            headers={"Authorization": f"Bearer {tok}"},
        )
        with urllib.request.urlopen(req, timeout=90) as r:
            return json.loads(r.read().decode("utf-8", "replace"))
    except Exception:
        return {}


def responde(url: str) -> bool:
    """¿Responde el cockpit por esa URL?"""
    if not url:
        return False
    try:
        req = urllib.request.Request(url.rstrip("/") + "/health", method="GET")
        with urllib.request.urlopen(req, timeout=20) as r:
            return 200 <= r.status < 500
    except urllib.error.HTTPError as e:
        # 401/403/404 = el ORIGIN esta vivo (el tunel funciona).
        return e.code in (401, 403, 404)
    except Exception:
        return False


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--vigilar", action="store_true", help="quedarse vigilando y migrando")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--cada", type=int, default=60, help="segundos entre comprobaciones")
    args = ap.parse_args()

    tok = token()
    actual = ""
    while True:
        try:
            url, nodo, build = url_del_movil(tok)
        except Exception as e:
            print(f"no se pudo leer el tablón: {e}", file=sys.stderr)
            url, nodo, build = "", "", ""
        viva = responde(url)
        if url != actual or not viva:
            if args.json:
                print(json.dumps({"url": url, "nodo": nodo, "build": build, "viva": viva}))
            else:
                estado = "VIVA" if viva else "sin responder"
                if url != actual:
                    print(f"[{time.strftime('%H:%M:%S')}] MIGRACIÓN -> {url} ({nodo} {build}) {estado}")
                else:
                    print(f"[{time.strftime('%H:%M:%S')}] {url} {estado}")
            actual = url
        if not args.vigilar:
            return 0 if viva else 1
        time.sleep(args.cada)


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Lee el ÚLTIMO diagnóstico publicado por un nodo en Drive.

MEDIDO 2026-11-25: pedirle al usuario que pegase la salida del terminal en
el chat era lento, se perdía y no era reproducible.  Ahora el móvil publica
su estado completo en `MegaDaemon/diagnostico/<nodo>/` y este script lo lee
desde el sandbox (sin ADB, sin túnel, sin que nadie pegue nada).

Uso:
    tools/leer-diagnostico.py                # el nodo más reciente
    tools/leer-diagnostico.py --nodo samsung
    tools/leer-diagnostico.py --raw          # el JSON crudo (para parsear)
    tools/leer-diagnostico.py --listar       # sólo listar
"""
import argparse
import json
import os
import sys
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


def buscar_carpeta_recursiva(tok: str, padre: str, nombre: str) -> str | None:
    """Busca una subcarpeta por nombre, en profundidad."""
    for f in listar(tok, padre):
        if f["mimeType"] != "application/vnd.google-apps.folder":
            continue
        if f["name"] == nombre:
            return f["id"]
        sub = buscar_carpeta_recursiva(tok, f["id"], nombre)
        if sub:
            return sub
    return None


def descargar(tok: str, file_id: str) -> bytes:
    url = f"{API}/files/{file_id}?alt=media&supportsAllDrives=true"
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {tok}"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--nodo", default="")
    ap.add_argument("--raw", action="store_true", help="el JSON crudo")
    ap.add_argument("--listar", action="store_true", help="sólo listar los ficheros")
    args = ap.parse_args()

    tok = token()
    diag = buscar_carpeta_recursiva(tok, RAIZ, "diagnostico")
    if not diag:
        print("no hay carpeta 'diagnostico' en Drive todavia", file=sys.stderr)
        return 2
    nodos = listar(tok, diag)
    carpetas = [f for f in nodos if f["mimeType"] == "application/vnd.google-apps.folder"]
    if args.listar:
        for c in carpetas:
            for f in sorted(
                listar(tok, c["id"]), key=lambda x: x.get("modifiedTime", ""), reverse=True
            )[:5]:
                print(f"{c['name']}/{f['name']}  {f.get('size', '?')} B  {f.get('modifiedTime', '?')}")
        return 0

    mejor = None
    for c in carpetas:
        if args.nodo and c["name"] != args.nodo:
            continue
        jsons = [f for f in listar(tok, c["id"]) if f["name"].endswith(".json")]
        if not jsons:
            continue
        jsons.sort(key=lambda x: x.get("modifiedTime", ""), reverse=True)
        cand = (jsons[0].get("modifiedTime", ""), c["name"], jsons[0])
        if mejor is None or cand[0] > mejor[0]:
            mejor = cand
    if mejor is None:
        print("no hay ningun diagnostico publicado todavia", file=sys.stderr)
        return 2

    _, nodo, fichero = mejor
    datos = descargar(tok, fichero["id"])
    if args.raw:
        sys.stdout.write(datos.decode("utf-8", "replace"))
        return 0
    v = json.loads(datos.decode("utf-8", "replace"))
    print(f"== diagnostico de {nodo} (fichero {fichero['name']}, {fichero.get('size','?')} B)")
    print(f"   build: {v.get('build', {}).get('short', '?')}")
    print(f"   ts:    {v.get('ts')}")
    print(f"-- supervisores vivos: {v.get('supervisores')} (1 es lo correcto)")
    print("-- procesos:")
    for p in v.get("procesos", []):
        print(f"   {p.get('pid')} rss={p.get('rss_kb')}kB  {p.get('cmd')}")
    print("-- health:", json.dumps(v.get("health", {}))[:600])
    st = v.get("stack", {})
    print(f"-- stack: {st.get('correctos')}/{st.get('total')} correctos")
    for f in st.get("ficheros", []):
        if not f.get("ok"):
            print(f"   DESFASADO {f.get('ruta')}  (origen {f.get('origen')})")
    b = v.get("binario", {})
    print(f"-- binario: {b.get('bytes')} B sha={str(b.get('sha256'))[:16]}")
    print("-- energia:", json.dumps(v.get("energia", {})))
    print("-- red:", json.dumps(v.get("red", {}))[:300])
    print("-- ultimas lineas del log:")
    for l in v.get("log", [])[-25:]:
        print("   " + l)
    print("-- shim (ultimas):")
    for l in v.get("shim", [])[-5:]:
        print("   " + l[:300])
    return 0


if __name__ == "__main__":
    sys.exit(main())

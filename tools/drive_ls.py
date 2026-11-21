#!/usr/bin/env python3
"""Lista (y opcionalmente baja) la carpeta de Swift Backup en Google Drive.

La carpeta es PÚBLICA ("cualquier persona con el enlace"), asi que basta una
API key para consultarla: no hace falta OAuth.

Uso:
    tools/drive_ls.py                 # listado completo -> _listado_completo.json
    tools/drive_ls.py --get 20        # baja los 20 mas pequeños
    tools/drive_ls.py --ext dat --get 3
"""
import json
import os
import sys
import urllib.parse
import urllib.request

FOLDER = os.environ.get("MEGADAEMON_DRIVE_FOLDER", "1L-XKLJKy68Jmjf0K8uIRXOklL4gUIdO1")
DEST = os.environ.get("SWIFT_DRIVE_DIR", "/workspace/swift-drive")
API_KEY = os.environ.get("MEGADAEMON_GOOGLE_API_KEY", "")

KEYS = [
    "AIzaSyBqJZh-7pA44blAaAkH6490hUFOwX0KCYM",
    "AIzaSyAoP6runCgAiXKUSuqLcgr1Hvr3dS8AL78",
    "AIzaSyB6-VFDgZK_xMLeUOU-Tosg5Nbt2mIF99o",
    "AIzaSyBB0i5ALGf06dz6t-05ezp9hGH8jDJINT8",
]


def api_get(url, key):
    req = urllib.request.Request(url + ("&" if "?" in url else "?") + "key=" + key)
    with urllib.request.urlopen(req, timeout=90) as r:
        return json.loads(r.read().decode())


def buscar_key():
    if API_KEY:
        return API_KEY
    for k in KEYS:
        try:
            url = (
                "https://www.googleapis.com/drive/v3/files?"
                + urllib.parse.urlencode(
                    {"q": f"'{FOLDER}' in parents", "pageSize": "1"}
                )
            )
            d = api_get(url, k)
            if "files" in d:
                return k
        except Exception:
            continue
    return ""


def listar(key):
    files, page = [], ""
    while True:
        url = (
            "https://www.googleapis.com/drive/v3/files?"
            + urllib.parse.urlencode(
                {
                    "q": f"'{FOLDER}' in parents",
                    "fields": "files(id,name,size,mimeType,md5Checksum,modifiedTime),nextPageToken",
                    "pageSize": "1000",
                    "supportsAllDrives": "true",
                    "includeItemsFromAllDrives": "true",
                    "pageToken": page,
                }
            )
        )
        d = api_get(url, key)
        files.extend(d.get("files", []))
        page = d.get("nextPageToken", "")
        if not page:
            break
    return files


def bajar(fid, destino):
    """Descarga un fichero publico por id (sin OAuth)."""
    url = f"https://drive.google.com/uc?export=download&id={fid}"
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req, timeout=1800) as r, open(destino, "wb") as g:
        total = 0
        while True:
            buf = r.read(1 << 20)
            if not buf:
                break
            g.write(buf)
            total += len(buf)
    return total


def main():
    os.makedirs(DEST, exist_ok=True)
    key = buscar_key()
    if not key:
        print("no hay API key utilizable para Drive")
        sys.exit(2)
    files = listar(key)
    path = os.path.join(DEST, "_listado_completo.json")
    json.dump(files, open(path, "w"), indent=1)
    tot = sum(int(f.get("size", 0) or 0) for f in files)
    print(f"ficheros: {len(files)}   total: {tot:,} B ({tot/1e9:.2f} GB)")
    print(f"listado: {path}")

    args = sys.argv[1:]
    if "--get" in args:
        i = args.index("--get")
        n = int(args[i + 1]) if len(args) > i + 1 else 5
        ext = None
        if "--ext" in args:
            ext = args[args.index("--ext") + 1]
        cand = [f for f in files if not ext or f["name"].split(" ")[0].endswith("." + ext)]
        cand.sort(key=lambda f: int(f.get("size", 0) or 0))
        for f in cand[:n]:
            nombre = f["name"].replace("/", "_")
            dest = os.path.join(DEST, nombre)
            if os.path.exists(dest) and os.path.getsize(dest) == int(f.get("size", 0) or 0):
                print(f"  ya está: {nombre}")
                continue
            print(f"  bajando {nombre} ({int(f.get('size',0)):,} B)...")
            t = bajar(f["id"], dest)
            ok = "OK" if t == int(f.get("size", 0) or 0) else f"DESCARTE ({t})"
            print(f"    {ok}")


if __name__ == "__main__":
    main()

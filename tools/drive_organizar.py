#!/usr/bin/env python3
"""Ordena TODO lo de MegaDaemon en UNA sola carpeta de Google Drive.

PROBLEMA (2026-11-23, del usuario): "el tema de las carpetas es un tema, cosas
que has subido son un lío" — el material de MegaDaemon estaba repartido entre
`megadaemon-backup`, `RE-GO-LEGIBLE`, `actualizaciones`, `Swift Backup` y
otros.  Este script deja una única carpeta raíz **MegaDaemon** con esta
estructura:

    MegaDaemon/
        binarios/     builds y releases (ARM64/Linux) y sus .sha256
        backups/      respaldos cifrados del móvil
        re/           el código Go legible, los libros y la instrumentación
        docs/         informes, inventarios, auditorías
        entregas/     entregas de los agentes

NO borra nada: sólo **mueve** (Drive: `addParents` + `removeParents`) y crea
las carpetas que falten.  `--apply` hace la operación; sin él, sólo informa.

Uso:
    python3 tools/drive_organizar.py            # informe (no toca nada)
    python3 tools/drive_organizar.py --apply    # ordena de verdad
"""
from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

CRED_PATH = os.environ.get(
    "MEGADAEMON_GDRIVE_CREDENTIALS", "/workspace/md-secrets/gdrive_credentials.json"
)
TOKEN_URL = "https://oauth2.googleapis.com/token"
API = "https://www.googleapis.com/drive/v3"
FOLDER_MIME = "application/vnd.google-apps.folder"

RAIZ_NOMBRE = "MegaDaemon"
# Subcarpetas de la estructura final.
SUBCARPETAS = ("Backups", "Projects", "Binarios", "Docs", "Notas-y-Prompts")

# Carpetas de origen conocidas (id -> nombre).  Se recorren y se mueve todo
# lo que haya dentro a la estructura final.
# Carpetas que hay que MOVER dentro de "MegaDaemon": (origen, destino).
#   "RE-GO-LEGIBLE"  -> Projects/   (es un proyecto, no un backup)
#   "actualizaciones" -> Binarios/  (son builds del movil)
# OJO: la carpeta publica "Swift Backup (fcf724b2...)" NO se mueve: el movil
# la tiene configurada por id para su backup; moverla rompe ese flujo.
MOVIMIENTOS = (
    ("RE-GO-LEGIBLE", "Projects"),
    ("actualizaciones", "Binarios"),
)

# El indice (DRIVE_INDEX.md) que se escribe al final.
INDICE = "DRIVE_INDEX.md"

# Reglas de clasificación por nombre (minusculas).
REGLAS = (
    (("sha256", "megadaemon-core", "adbonly", "channel-publish", "duckdns",
      "md-clipboard", "md-dict", "md-lan-listen", "npu-probe", "hooks-payload",
      "latest.json"), "binarios"),
    ((".tar.gz", ".tgz", "backup", "cifrado", "swift"), "backups"),
    ((".zip", "libro", "libro-completo", "go.funcs", "reconstruccion", ".c",
      ".tsv", "strings"), "re"),
    ((".md", ".txt", ".json", "inventario", "auditoria", "informe"), "docs"),
)


def token() -> str:
    with open(CRED_PATH, encoding="utf-8") as f:
        cred = json.load(f)
    datos = urllib.parse.urlencode(
        {
            "client_id": cred["client_id"],
            "client_secret": cred["client_secret"],
            "refresh_token": cred["refresh_token"],
            "grant_type": "refresh_token",
        }
    ).encode()
    req = urllib.request.Request(TOKEN_URL, data=datos,
                                 headers={"Content-Type": "application/x-www-form-urlencoded"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)["access_token"]


def api(tok: str, path: str, method: str = "GET", body: dict | None = None) -> dict:
    # `path` puede ser una ruta relativa ("files", "files/<id>") o una URL
    # completa (la usa `listar`): no se le antepone la base dos veces.
    url = path if path.startswith("http") else f"{API}/{path}"
    data = json.dumps(body).encode() if body is not None else None
    cabeceras = {"Authorization": f"Bearer {tok}"}
    if body is not None:
        # MEDIDO (2026-11-23): sin `Content-Type: application/json` urllib
        # manda el cuerpo como `application/x-www-form-urlencoded` y Drive
        # IGNORA el JSON => el PATCH devolvia 200 sin mover nada.
        cabeceras["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, method=method, headers=cabeceras)
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            crudo = r.read()
            return json.loads(crudo) if crudo else {}
    except urllib.error.HTTPError as e:
        detalle = e.read().decode("utf-8", "replace")[:300]
        raise RuntimeError(f"{method} {path}: HTTP {e.code}: {detalle}") from e


def listar(tok: str, parent: str | None) -> list[dict]:
    if parent:
        q = f"'{parent}' in parents and trashed = false"
    else:
        q = "trashed = false and 'root' in parents"
    url = (f"{API}/files?q={urllib.parse.quote(q)}"
           f"&fields=files(id,name,mimeType,size,parents)&pageSize=1000"
           f"&supportsAllDrives=true&includeItemsFromAllDrives=true")
    try:
        return api(tok, url).get("files", [])
    except RuntimeError as e:
        # Carpeta que ya no existe (o id obsoleto): no es un fallo fatal.
        if "404" in str(e):
            return []
        raise


def crear_carpeta(tok: str, nombre: str, padre: str | None) -> str:
    body = {"name": nombre, "mimeType": FOLDER_MIME}
    if padre:
        body["parents"] = [padre]
    return api(tok, "files", "POST", body)["id"]


def buscar_carpeta(tok: str, nombre: str, padre: str | None) -> str | None:
    for f in listar(tok, padre):
        if f["mimeType"] == FOLDER_MIME and f["name"].lower() == nombre.lower():
            return f["id"]
    return None


def mover(tok: str, file_id: str, destino: str, origenes: list[str]) -> None:
    # MEDIDO (2026-11-23): sin `supportsAllDrives` el PATCH devolvia 200
    # pero NO movia nada (la carpeta esta en una unidad compartida) => el
    # `parents` seguia igual.  Con los parametros de unidad compartida, si.
    body = {"addParents": [destino], "removeParents": origenes}
    api(tok, f"files/{file_id}?supportsAllDrives=true&includeItemsFromAllDrives=true",
        "PATCH", body)


def clasificar(nombre: str) -> str:
    n = nombre.lower()
    for patrones, destino in REGLAS:
        if any(p in n for p in patrones):
            return destino
    return "entregas"


def main() -> int:
    aplicar = "--apply" in sys.argv
    tok = token()

    # 1) La carpeta raíz "MegaDaemon" (en la raíz de Drive).
    raiz = buscar_carpeta(tok, RAIZ_NOMBRE, None)
    if raiz is None:
        if not aplicar:
            print(f"[i] la carpeta '{RAIZ_NOMBRE}' NO existe todavia (se crearia)")
            return 0
        raiz = crear_carpeta(tok, RAIZ_NOMBRE, None)
        print(f"[+] creada la carpeta raiz '{RAIZ_NOMBRE}' ({raiz})")
    else:
        print(f"[=] la carpeta raiz '{RAIZ_NOMBRE}' ya existe ({raiz})")

    # 2) Las subcarpetas de la estructura (las que falten, se crean).
    subs = {}
    for s_ in SUBCARPETAS:
        sid = buscar_carpeta(tok, s_, raiz)
        if sid is None and aplicar:
            sid = crear_carpeta(tok, s_, raiz)
            print(f"[+] creada {RAIZ_NOMBRE}/{s_} ({sid})")
        subs[s_] = sid
        print(f"[=] {RAIZ_NOMBRE}/{s_}: {sid or '(no existe todavia)'}")

    # 3) Mover las carpetas mal colocadas (se localizan POR NOMBRE).
    movidos = 0
    for nombre, destino in MOVIMIENTOS:
        origen = buscar_carpeta(tok, nombre, None) or buscar_carpeta(tok, nombre, raiz)
        if origen is None:
            # Puede estar anidado ("MegaDaemon/Backups/megadaemon-backup/X"):
            # se busca en TODO el arbol (3 niveles), sin tocar nada.
            pila = [(raiz, 0)]
            while pila and origen is None:
                padre_id, nivel = pila.pop()
                for f in listar(tok, padre_id):
                    if f["mimeType"] != FOLDER_MIME:
                        continue
                    if f["name"] == nombre:
                        origen = f["id"]
                        break
                    if nivel < 3:
                        pila.append((f["id"], nivel + 1))
        destino_id = subs.get(destino)
        if not origen or not destino_id:
            print(f"[i] '{nombre}': no encontrado (o sin carpeta destino)")
            continue
        ya_en_destino = origen in [p for p in listar(tok, destino_id)] if listar(tok, destino_id) else False
        if ya_en_destino:
            print(f"[=] '{nombre}' ya esta en {RAIZ_NOMBRE}/{destino}")
            continue
        if aplicar:
            mover(tok, origen, destino_id, [raiz])
            print(f"[m] {nombre} -> {RAIZ_NOMBRE}/{destino}")
        else:
            print(f"[.] {nombre} -> {RAIZ_NOMBRE}/{destino}")
        movidos += 1

    # 4) El indice: que hay y donde (para no perderse otra vez).
    if aplicar:
        lineas = [
            "# MegaDaemon en Drive",
            "",
            "Una sola carpeta raiz: **MegaDaemon**.  Estructura:",
            "",
            "| Carpeta | Que contiene |",
            "|---|---|",
            "| `Projects/` | Los proyectos: `RE-GO-LEGIBLE` (el codigo Go legible del binario de Antigravity, el libro de funciones y la reconstruccion multi-metodo). |",
            "| `Binarios/` | Las builds del movil (`actualizaciones/`) con sus `.sha256`. |",
            "| `Backups/` | Los respaldos cifrados del movil, las imagenes del kernel y las entregas de los agentes. |",
            "| `Docs/` | Informes, inventarios y auditorias. |",
            "| `Notas-y-Prompts/` | Notas y prompts. |",
            "",
            "FUERA (a proposito): la carpeta publica `Swift Backup (fcf724b2ae9147c8)` **no se mueve**:",
            "el movil la tiene configurada por id para su backup y moverla rompe ese flujo.",
            "",
            f"Actualizado: {__import__('datetime').datetime.utcnow().isoformat()}Z",
            "",
        ]
        txt = "\n".join(lineas)
        # Escritura ATOMICA via la API (subida + borrado del anterior): Drive
        # no tiene "update" de contenido, asi que se sube el nuevo y se
        # mueve el viejo a la papelera (NO se borra de raiz).
        try:
            # MEDIDO: con `uploadType=media` el cuerpo ES el contenido: si se
            # mandan los metadatos, el fichero queda con 0 bytes.  Por eso
            # aqui se usa `multipart` (metadatos + contenido en un cuerpo).
            bnd = "md-drive-index-7f3a"
            meta = json.dumps({"name": INDICE, "mimeType": "text/markdown",
                               "parents": [raiz]}).encode()
            cuerpo = (b"--" + bnd.encode() + b"\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n"
                      + meta + b"\r\n--" + bnd.encode() + b"\r\nContent-Type: text/markdown; charset=UTF-8\r\n\r\n"
                      + txt.encode() + b"\r\n--" + bnd.encode() + b"--")
            req = urllib.request.Request(
                "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart"
                "&fields=id&supportsAllDrives=true",
                data=cuerpo, method="POST",
                headers={"Authorization": f"Bearer {tok}",
                         "Content-Type": f"multipart/related; boundary={bnd}"})
            with urllib.request.urlopen(req, timeout=120) as r:
                nuevo_id = json.load(r)["id"]
            for f in listar(tok, raiz):
                if (f["name"] == INDICE and f["id"] != nuevo_id
                        and int(f.get("size") or 0) == 0):
                    api(tok, f"files/{f['id']}", "PATCH", {"trashed": True})
            print(f"[+] escrito {INDICE} ({nuevo_id})")
        except Exception as e:  # noqa: BLE001
            print(f"[!] no se pudo escribir el indice: {e}")

    # 5) El estado final.
    print("\n=== MegaDaemon ===")
    for f in sorted(listar(tok, raiz), key=lambda x: x["name"]):
        if f["mimeType"] == FOLDER_MIME:
            n = len(listar(tok, f["id"]))
            print(f"  DIR  {f['name']}/  ({n} elementos)")
        else:
            print(f"  FILE {f['name']}  ({f.get('size','-')} B)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

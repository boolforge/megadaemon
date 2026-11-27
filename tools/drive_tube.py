#!/usr/bin/env python3
"""drive_tube.py — el "tubo" de Drive para MegaDaemon.

Sube artifacts a la carpeta PRIVADA de Drive y (opcionalmente) borra la copia
local, para que el workspace (5 GB de cuota) no se llene.  El inventario se
queda en local y también se sube.

    tools/drive_tube.py listar [--parent ID] [--patron TEXTO]
    tools/drive_tube.py subir FICHERO [--parent ID] [--nombre N]
    tools/drive_tube.py subir_carpeta DIR [--parent ID] [--borrar_local]
    tools/drive_tube.py bajar FILE_ID DESTINO
    tools/drive_tube.py inventario [--parent ID] [--salida FICHERO]

Nunca imprime el token de acceso: sólo su sha16.
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from drive_api import CRED_POR_DEFECTO, RAIZ_BACKUP, Drive, DriveError  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--creds", default=CRED_POR_DEFECTO)
    ap.add_argument("--api-base", default=None, help="servidor alternativo (tests)")
    ap.add_argument("--upload-base", default=None)
    ap.add_argument("--token-url", default=None)
    sub = ap.add_subparsers(dest="accion", required=True)

    p = sub.add_parser("listar", help="lista el contenido de una carpeta")
    p.add_argument("--parent", default=RAIZ_BACKUP)
    p.add_argument("--patron", default="")

    p = sub.add_parser("subir", help="sube un fichero")
    p.add_argument("fichero")
    p.add_argument("--parent", default=RAIZ_BACKUP)
    p.add_argument("--nombre", default=None)

    p = sub.add_parser("subir_carpeta", help="empaqueta una carpeta y la sube")
    p.add_argument("carpeta")
    p.add_argument("--parent", default=RAIZ_BACKUP)
    p.add_argument("--nombre", default=None)
    p.add_argument("--borrar_local", action="store_true")

    p = sub.add_parser("bajar", help="baja un fichero por id")
    p.add_argument("file_id")
    p.add_argument("destino")

    p = sub.add_parser("inventario", help="inventario TSV de una carpeta")
    p.add_argument("--parent", default=RAIZ_BACKUP)
    p.add_argument("--salida", default=None)

    args = ap.parse_args()
    d = Drive(creds=args.creds,
              api_base=args.api_base or "https://www.googleapis.com/drive/v3",
              upload_base=args.upload_base or "https://www.googleapis.com/upload/drive/v3",
              token_url=args.token_url or "https://oauth2.googleapis.com/token")
    try:
        if args.accion == "listar":
            for f in d.listar(args.parent):
                if args.patron and args.patron.lower() not in f["name"].lower():
                    continue
                print(f"{int(f.get('size') or 0):>12} B  {f.get('modifiedTime', '?')[:10]}  {f['name']}")
        elif args.accion == "subir":
            info = d.subir(args.fichero, args.parent, args.nombre)
            print(json.dumps(info))
        elif args.accion == "subir_carpeta":
            info = d.subir_carpeta(args.carpeta, args.parent, args.nombre)
            print(json.dumps(info))
            if args.borrar_local:
                import shutil
                shutil.rmtree(args.carpeta, ignore_errors=True)
                print("copia local borrada (queda en Drive)")
        elif args.accion == "bajar":
            n = d.descargar(args.file_id, args.destino)
            print(f"{n} B -> {args.destino}")
        elif args.accion == "inventario":
            filas = ["ruta\tbytes\tid"]
            for f in d.listar(args.parent):
                filas.append(f"{f['name']}\t{int(f.get('size') or 0)}\t{f['id']}")
            txt = "\n".join(filas) + "\n"
            if args.salida:
                with open(args.salida, "w", encoding="utf-8") as fh:
                    fh.write(txt)
                print(f"inventario -> {args.salida} ({len(filas) - 1} entradas)")
            else:
                print(txt, end="")
        return 0
    except DriveError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())

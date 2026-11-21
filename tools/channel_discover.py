#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Redescubre el canal del móvil (instancia maestra) desde el sandbox.

El móvil publica su estado en un **paste público** (lo hace `channel-publish`,
que es el mismo código Rust que la tool MCP `channel_publish`). El contenido
del paste incluye la URL del paste anterior ⇒ la cadena es **auto-renovable**:
el sandbox que conoce **un** paste aprende el siguiente.

Orden de descubrimiento:
  1. `--paste <url>`  : paste conocido (arranca la cadena).
  2. el último paste guardado en `--state` (por defecto `~/.mavis/channel.json`).
  3. el dominio estable de DuckDNS (`--duckdns <dominio>`), si el usuario
     abrió el puerto en el router.

Uso:
    tools/channel_discover.py --paste https://paste.rs/abcd
    tools/channel_discover.py --duckdns mdtmfnc-s21.duckdns.org
    tools/channel_discover.py            # usa el último guardado

Imprime el estado del canal (JSON) y la URL viva del quick tunnel.
NO contiene secretos: la key del MCP va aparte (ficheros 0600).
"""
import argparse
import json
import os
import sys
import urllib.error
import urllib.request

TIMEOUT = 20
UA = "MegaDaemon-channel-discover"


def fetch(url, timeout=TIMEOUT):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read().decode("utf-8", errors="replace")


def desde_paste(url):
    """El paste contiene el JSON del canal (o texto plano con la URL)."""
    texto = fetch(url).strip()
    try:
        return json.loads(texto)
    except json.JSONDecodeError:
        # Paste de texto plano: se asume que es la URL del túnel.
        if texto.startswith("http"):
            return {"quick": texto.splitlines()[0].strip()}
        raise


def desde_duckdns(dominio):
    """El canal estable: el dominio debe resolver al móvil (port-forward)."""
    base = dominio if dominio.startswith("http") else f"https://{dominio}"
    url = f"{base.rstrip('/')}/mcp"
    req = urllib.request.Request(
        url, data=b'{"jsonrpc":"2.0","id":1,"method":"tools/list"}',
        headers={"User-Agent": UA, "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as r:
            cuerpo = r.read().decode("utf-8", errors="replace")
            return {"duckdns": base, "http_status": r.status, "cuerpo": cuerpo[:400]}
    except urllib.error.HTTPError as e:
        # 401/403 = el daemon respondió: el canal estable FUNCIONA.
        return {"duckdns": base, "http_status": e.code, "cuerpo": e.read().decode()[:200]}
    except Exception as e:
        return {"duckdns": base, "error": f"{type(e).__name__}: {e}"}


def guardar(estado, ruta):
    os.makedirs(os.path.dirname(os.path.expanduser(ruta)), exist_ok=True)
    with open(os.path.expanduser(ruta), "w") as fh:
        json.dump(estado, fh, indent=2)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--paste", help="URL de un paste conocido")
    ap.add_argument("--duckdns", help="dominio estable (p.ej. mdtmfnc-s21.duckdns.org)")
    ap.add_argument("--state", default="~/.mavis/channel.json",
                    help="fichero local donde se guarda el último estado")
    ap.add_argument("--no-guardar", action="store_true")
    args = ap.parse_args()

    estado = {}
    if args.paste:
        try:
            estado = desde_paste(args.paste)
            print(f"paste {args.paste} -> OK", file=sys.stderr)
        except Exception as e:
            print(f"ERROR leyendo el paste: {type(e).__name__}: {e}", file=sys.stderr)
            return 2
    else:
        ruta = os.path.expanduser(args.state)
        if os.path.exists(ruta):
            try:
                previo = json.load(open(ruta))
                url = previo.get("paste")
                if url:
                    estado = desde_paste(url)
                    print(f"paste anterior {url} -> OK", file=sys.stderr)
            except Exception as e:
                print(f"ERROR con el estado previo: {e}", file=sys.stderr)
                return 2
        if not estado:
            print("sin paste conocido: usa --paste o --duckdns", file=sys.stderr)
            return 1

    if args.duckdns:
        estado["duckdns_estado"] = desde_duckdns(args.duckdns)

    # El paste encadena al siguiente: se guarda para la próxima vez.
    if not args.no_guardar:
        try:
            guardar(estado, args.state)
        except OSError:
            pass

    print(json.dumps(estado, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())

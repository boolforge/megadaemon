#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Redescubre el canal del móvil (instancia maestra) desde el sandbox.

⚠️ **Por qué el paste NO basta** (medido): cada publicación crea un paste
**nuevo** ⇒ el sandbox que conoce el paste P1 **no puede descubrir P2**
(el campo `prev_paste` sólo apunta hacia atrás). Por eso el punto canónico
es el **repo**: el `raw` de `state/channel.json` **siempre** sirve la última
versión ⇒ el sandbox la puede pollar y ve el cambio de hostname sin que
nadie le copie una URL a mano.

Orden de descubrimiento:
  1. **canónico** (`--canonical`, por defecto el `raw` del repo): estado
     actual, siempre.
  2. **paste** (`--paste`): el último paste conocido (y su cadena hacia
     atrás, como respaldo si el repo no está accesible).
  3. **dominio estable** (`--duckdns`): si el usuario abrió el puerto.

Uso:
    tools/channel_discover.py                       # canónico + respaldo
    tools/channel_discover.py --canonical https://raw.githubusercontent.com/.../channel.json
    tools/channel_discover.py --paste https://paste.rs/abcd
    tools/channel_discover.py --duckdns mdtmfnc-s21.duckdns.org

Imprime el estado del canal en JSON. NO contiene secretos: la key del MCP va
en ficheros 0600 aparte.
"""
import argparse
import json
import os
import sys
import urllib.error
import urllib.request

TIMEOUT = 20
UA = "MegaDaemon-channel-discover"
CANONICAL_DEF = ("https://raw.githubusercontent.com/boolforge/megadaemon/"
                 "main/state/channel.json")


def fetch(url, timeout=TIMEOUT):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read().decode("utf-8", errors="replace")


def desde_json(texto):
    """El contenido puede ser JSON o texto plano con la URL."""
    texto = texto.strip()
    try:
        return json.loads(texto)
    except json.JSONDecodeError:
        if texto.startswith("http"):
            return {"quick": texto.splitlines()[0].strip()}
        raise


def desde_canonico(url):
    """El `raw` del repo: SIEMPRE la última versión del estado."""
    v = desde_json(fetch(url))
    if not isinstance(v, dict):
        return None
    if not (v.get("quick") or v.get("duckdns")):
        return None
    v["_fuente"] = url
    return v


def desde_paste(url):
    v = desde_json(fetch(url))
    if not isinstance(v, dict):
        return None
    v["_fuente"] = url
    v.setdefault("canonical", CANONICAL_DEF)
    return v


def desde_duckdns(dominio):
    """El canal estable: el dominio debe resolver al móvil (port-forward)."""
    base = dominio if dominio.startswith("http") else f"https://{dominio}"
    url = f"{base.rstrip('/')}/mcp"
    req = urllib.request.Request(
        url, data=b'{"jsonrpc":"2.0","id":1,"method":"tools/list"}',
        headers={"User-Agent": UA, "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as r:
            return {"duckdns": base, "http_status": r.status,
                    "cuerpo": r.read().decode("utf-8", errors="replace")[:400]}
    except urllib.error.HTTPError as e:
        # 401/403 = el daemon respondió: el canal estable FUNCIONA.
        return {"duckdns": base, "http_status": e.code,
                "cuerpo": e.read().decode(errors="replace")[:200]}
    except Exception as e:
        return {"duckdns": base, "error": f"{type(e).__name__}: {e}"}


def guardar(estado, ruta):
    os.makedirs(os.path.dirname(os.path.expanduser(ruta)), exist_ok=True)
    with open(os.path.expanduser(ruta), "w") as fh:
        json.dump(estado, fh, indent=2)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--canonical", default=CANONICAL_DEF,
                    help="URL estable del estado (raw del repo)")
    ap.add_argument("--paste", help="URL de un paste conocido (respaldo)")
    ap.add_argument("--duckdns", help="dominio estable (p.ej. mdtmfnc-s21.duckdns.org)")
    ap.add_argument("--state", default="~/.mavis/channel.json",
                    help="fichero local donde se guarda el último estado")
    ap.add_argument("--no-guardar", action="store_true")
    ap.add_argument("--sin-canonico", action="store_true",
                    help="ignorar el punto canónico (sólo paste/duckdns)")
    args = ap.parse_args()

    estado = None
    avisos = []

    # 1) canónico: el que de verdad manda
    if not args.sin_canonico:
        try:
            estado = desde_canonico(args.canonical)
            if estado:
                print(f"canónico {args.canonical} -> OK", file=sys.stderr)
        except Exception as e:
            avisos.append(f"canónico no accesible: {type(e).__name__}: {e}")

    # 2) paste (el conocido, o el último guardado) + cadena hacia atrás
    if estado is None:
        url_paste = args.paste
        if not url_paste:
            ruta = os.path.expanduser(args.state)
            if os.path.exists(ruta):
                try:
                    url_paste = json.load(open(ruta)).get("paste")
                except Exception:
                    url_paste = None
        if url_paste:
            try:
                estado = desde_paste(url_paste)
                print(f"paste {url_paste} -> OK", file=sys.stderr)
            except Exception as e:
                avisos.append(f"paste no accesible: {type(e).__name__}: {e}")

    # 3) dominio estable
    if args.duckdns:
        d = desde_duckdns(args.duckdns)
        if estado is None:
            estado = d
        else:
            estado["duckdns_estado"] = d

    if estado is None:
        for a in avisos:
            print(a, file=sys.stderr)
        print("sin canal: usa --canonical, --paste o --duckdns", file=sys.stderr)
        return 1

    if avisos:
        estado["_avisos"] = avisos
    if not args.no_guardar:
        try:
            guardar(estado, args.state)
        except OSError:
            pass

    print(json.dumps(estado, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())

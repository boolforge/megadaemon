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
  1. **canónico** (`--canonical`, por defecto `https://<dominio>/channel.json`):
     lo sirve el daemon (público, payload mínimo) a través del relay del PC
     ⇒ URL estable, sin PAT en el móvil y sin caché de CDN.
  2. **respaldo**: el fichero del repo vía Contents API (no cachea).
  3. **paste** (`--paste`): el último paste conocido. ⚠️ `paste.rs` **no
     tiene caducidad** ⇒ el lector aplica un **TTL de 6 h**: un paste más
     viejo se **ignora** (y se va al canónico).
  4. **dominio estable** (`--duckdns`): si el usuario abrió el puerto.

Uso:
    tools/channel_discover.py                       # canónico + respaldo
    tools/channel_discover.py --canonical https://raw.githubusercontent.com/.../channel.json
    tools/channel_discover.py --paste https://paste.rs/abcd
    tools/channel_discover.py --duckdns mdtmfnc.duckdns.org

Imprime el estado del canal en JSON. NO contiene secretos: la key del MCP va
en ficheros 0600 aparte.
"""
import argparse
import base64
import datetime
import json
import os
import re
import sys
import urllib.error
import urllib.request

TIMEOUT = 20
UA = "MegaDaemon-channel-discover"
# CANÓNICO: el endpoint del daemon (PÚBLICO, payload mínimo) servido por el
# relay del PC => URL estable, sin PAT en el móvil y sin caché de CDN.
CANONICAL_DEF = "https://mdtmfnc.duckdns.org/channel.json"
# Respaldo canónico: el fichero en el repo (vía Contents API, que no cachea).
REPO_DEF = ("https://raw.githubusercontent.com/boolforge/megadaemon/"
            "main/state/channel.json")
# El paste no tiene caducidad (paste.rs no manda TTL) => la caducidad la
# impone el LECTOR: un paste más viejo de esto se ignora.
TTL_HORAS = 6


def fetch(url, timeout=TIMEOUT):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read().decode("utf-8", errors="replace")


def desde_json(texto):
    """El contenido puede ser JSON, la envolvente de la Contents API (base64)
    o texto plano con la URL."""
    texto = texto.strip()
    try:
        v = json.loads(texto)
    except json.JSONDecodeError:
        if texto.startswith("http"):
            return {"quick": texto.splitlines()[0].strip()}
        raise
    # Envolvente de la Contents API: el fichero va en base64.
    if isinstance(v, dict) and isinstance(v.get("content"), str) and "sha" in v:
        try:
            return json.loads(base64.b64decode(v["content"]).decode("utf-8", "replace"))
        except Exception:
            return v
    return v


def edad_horas(estado):
    """Antigüedad del estado en horas (None si no hay ts legible)."""
    ts = (estado or {}).get("ts")
    if not ts:
        return None
    try:
        t = datetime.datetime.strptime(ts, "%Y-%m-%dT%H:%M:%SZ").replace(
            tzinfo=datetime.timezone.utc)
    except Exception:
        return None
    return (datetime.datetime.now(datetime.timezone.utc) - t).total_seconds() / 3600.0


def desde_canonico(url):
    """El punto canónico: SIEMPRE la última versión del estado.

    ⚠️ `raw.githubusercontent.com` **cachea en CDN** ⇒ puede servir la versión
    vieja durante minutos (medido: tras publicar, el raw seguía con el
    contenido anterior). Por eso se intenta **primero la Contents API**
    (`api.github.com/repos/{o}/{r}/contents/{path}?ref=main`), que no cachea;
    el `raw` queda como respaldo.
    """
    candidatos = [url]
    if "raw.githubusercontent.com" in url:
        api = api_equivalente(url)
        if api:
            candidatos = [api, url]
    for candidato in candidatos:
        if not candidato:
            continue
        try:
            v = desde_json(fetch(candidato))
        except Exception:
            continue
        if not isinstance(v, dict):
            continue
        if not (v.get("quick") or v.get("duckdns")):
            continue
        v["_fuente"] = candidato
        return v
    return None


def api_equivalente(raw_url):
    """Convierte una URL `raw` en la de la Contents API (sin caché)."""
    m = re.match(
        r"https://raw\.githubusercontent\.com/([^/]+)/([^/]+)/([^/]+)/(.*)$", raw_url)
    if not m:
        return None
    owner, repo, ref, path = m.groups()
    return (f"https://api.github.com/repos/{owner}/{repo}/contents/{path}"
            f"?ref={ref}")


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
    ap.add_argument("--duckdns", help="dominio estable (p.ej. mdtmfnc.duckdns.org)")
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

    # 1b) respaldo: el repo (Contents API, sin caché de CDN)
    if estado is None and not args.sin_canonico:
        try:
            estado = desde_canonico(REPO_DEF)
            if estado:
                print(f"respaldo en el repo {REPO_DEF} -> OK", file=sys.stderr)
        except Exception as e:
            avisos.append(f"respaldo del repo no accesible: {type(e).__name__}: {e}")

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
                cand = desde_paste(url_paste)
                edad = edad_horas(cand)
                if edad is not None and edad > TTL_HORAS:
                    avisos.append(f"paste {url_paste} demasiado viejo ({edad:.1f} h > {TTL_HORAS} h): se ignora")
                else:
                    estado = cand
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

    edad = edad_horas(estado)
    if edad is not None:
        estado["_edad_h"] = round(edad, 2)
        estado["_stale"] = edad > TTL_HORAS
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

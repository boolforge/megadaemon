#!/usr/bin/env python3
"""
lan_discover.py — encuentra el móvil en la red local (sin internet).

Por qué existe: el canal estable (DuckDNS) y el quick tunnel dependen de
internet y de terceros.  Si el móvil y este equipo están en la misma red, el
móvil emite un beacon UDP en difusión (puerto 9099) y se puede llamar
directamente por IP: sin túnel, sin dominio, sin DNS.

Uso:
    python3 tools/lan_discover.py                 # escucha 15 s
    python3 tools/lan_discover.py --timeout 30
    python3 tools/lan_discover.py --probe         # además comprueba /health
    python3 tools/lan_discover.py --json

Códigos de salida: 0 = encontrado (y vivo si --probe) · 1 = no encontrado.
"""
from __future__ import annotations

import argparse
import json
import socket
import sys
import time
import urllib.error
import urllib.request

BEACON_PORT = 9099
BEACON_KIND = "megadaemon-beacon/v1"


def listen(timeout: float):
    """Escucha beacons UDP y devuelve el primero válido (o None)."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    try:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
    except OSError:
        pass
    try:
        sock.bind(("", BEACON_PORT))
    except OSError as e:
        print(f"no se puede escuchar en el puerto {BEACON_PORT}: {e}", file=sys.stderr)
        return None
    sock.settimeout(0.5)
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            data, _addr = sock.recvfrom(4096)
        except socket.timeout:
            continue
        except OSError:
            break
        try:
            v = json.loads(data.decode("utf-8", "replace"))
        except (ValueError, UnicodeDecodeError):
            continue
        if v.get("kind") != BEACON_KIND:
            continue
        return v
    return None


def probe(url: str, timeout: float = 4.0):
    """Comprueba /health. Devuelve (codigo_http, vivo)."""
    try:
        req = urllib.request.Request(url.rstrip("/") + "/health", method="GET")
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, True
    except urllib.error.HTTPError as e:
        # 401/403 = el daemon vive y exige credencial: también es "vivo".
        return e.code, 400 <= e.code < 500
    except Exception:
        return 0, False


def main() -> int:
    ap = argparse.ArgumentParser(description="Descubre el móvil por el canal LAN")
    ap.add_argument("--timeout", type=float, default=15.0, help="segundos de escucha")
    ap.add_argument("--probe", action="store_true", help="comprobar /health")
    ap.add_argument("--json", action="store_true", help="salida JSON")
    args = ap.parse_args()

    b = listen(args.timeout)
    if not b:
        if args.json:
            print(json.dumps({"ok": False, "error": "no_beacon", "timeout_s": args.timeout}))
        else:
            print(f"NO se ha encontrado el móvil por LAN en {args.timeout:g} s")
            print("  · la difusión UDP no cruza routers: el móvil debe estar en la misma subred")
            print("  · ¿está el daemon vivo? (ps | grep megadaemon-core)")
        return 1

    url = b.get("url") or ""
    if not url:
        ips = b.get("ips") or []
        if ips:
            url = f"http://{ips[0]}:{b.get('port', 9090)}"

    res = {
        "ok": True,
        "id": b.get("id", ""),
        "ips": b.get("ips", []),
        "port": b.get("port", 9090),
        "url": url,
    }
    if args.probe and url:
        code, vivo = probe(url)
        res["http"] = code
        res["vivo"] = vivo

    if args.json:
        print(json.dumps(res))
    else:
        print(f"móvil encontrado por LAN: {res['id']}")
        print(f"  IPs: {res['ips']}")
        print(f"  URL: {res['url']}")
        if args.probe:
            print(f"  /health -> HTTP {res.get('http', 0)} ({'vivo' if res.get('vivo') else 'NO responde'})")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Resuelve la URL pública del cockpit/daemon leyéndola DEL MÓVIL.

Por qué: la URL de `trycloudflare` es EFÍMERA (cada relanzamiento de
cloudflared genera un subdominio nuevo) y la red del móvil corta la conexión
con el edge de Cloudflare de vez en cuando ⇒ el supervisor reinicia el
túnel ⇒ la URL cambia ⇒ todo lo que la tenga escrita se queda en 530.

Este helper la lee de `tunnel.url` en el propio móvil (vía el MCP legacy,
que NO cambia de URL) y la deja en `md-secrets/instance.env` + en las
variables de entorno para el resto de herramientas.

    python3 tools/phone_url.py            # sólo imprimir la URL vigente
    python3 tools/phone_url.py --export   # imprimir + exportar en instance.env
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SECRETS = os.path.join(ROOT, "md-secrets", "instance.env")
MCP_URL = os.path.join(ROOT, "md-secrets", "mcp.url")
MCP = os.path.join(ROOT, "mcp.py")

CMD = ("cat /data/local/tmp/megadaemon/tunnel.url 2>/dev/null; "
       "echo ---; cat /data/local/tmp/megadaemon/mcp_tunnel.url 2>/dev/null")


def phone_urls():
    r = subprocess.run(
        ["python3", MCP, "call", "root_exec", json.dumps({"command": CMD, "max_chars": 400})],
        capture_output=True, text=True, timeout=180,
    )
    out = r.stdout or ""
    # El móvil devuelve DOS URLs separadas por "---": la del cockpit (web) y la
    # del MCP legacy.  NO se distinguen por "/mcp" (en el móvil son URLs
    # desnudas): se separan por el marcador de orden.
    parts = out.split("---")
    urls = []
    for chunk in parts:
        for line in chunk.splitlines():
            line = line.strip()
            if line.startswith("https://"):
                urls.append(line)
                break
    web = urls[0] if urls else ""
    mcp = urls[1] if len(urls) > 1 else ""
    return web, mcp


def write_env(web, mcp):
    env = {}
    if os.path.exists(SECRETS):
        for line in open(SECRETS):
            if "=" in line and not line.strip().startswith("#"):
                k, v = line.strip().split("=", 1)
                env[k] = v
    if web:
        env["MEGADAEMON_URL"] = web
        env["MEGADAEMON_MCP"] = web.rstrip("/") + "/mcp"
    if mcp:
        env["MEGADAEMON_MCP_LEGACY"] = mcp
        # `mcp.py` (el canal legacy que lee el móvil) resuelve su endpoint
        # desde este fichero: si no se refresca, se queda en 530.
        try:
            with open(MCP_URL, "w") as f:
                f.write(mcp.rstrip("/") + "\n")
            os.chmod(MCP_URL, 0o600)
        except OSError:
            pass
    with open(SECRETS, "w") as f:
        for k, v in env.items():
            f.write("%s=%s\n" % (k, v))
    os.chmod(SECRETS, 0o600)


if __name__ == "__main__":
    web, mcp = phone_urls()
    if not web:
        print("no se pudo leer la URL del móvil", file=sys.stderr)
        sys.exit(1)
    if "--export" in sys.argv:
        write_env(web, mcp)
    print(web)
    if mcp:
        print(mcp, file=sys.stderr)

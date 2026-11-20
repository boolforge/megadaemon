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
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

def _secrets_dir() -> str:
    """Directorio de secretos (0600) — SIEMPRE fuera del repo.

    Orden: $MEGADAEMON_SECRETS > <repo>/md-secrets (si existe) >
    <padre del repo>/md-secrets  (/workspace/md-secrets) > /workspace/md-secrets.
    Nunca se crea dentro del repo: un `instance.env` con la API key dentro
    del árbol de git es un camino directo a filtrar el secreto.
    """
    import os as _os
    cands = []
    env_dir = _os.environ.get("MEGADAEMON_SECRETS", "")
    if env_dir:
        cands.append(env_dir)
    cands.append(_os.path.join(ROOT, "md-secrets"))
    cands.append(_os.path.join(_os.path.dirname(ROOT), "md-secrets"))
    cands.append("/workspace/md-secrets")
    for c in cands:
        if _os.path.isdir(c):
            return c
    raise SystemExit("no encuentro el directorio de secretos (fuera del repo): probados %s" % cands)

SECRETS_DIR = _secrets_dir()
SECRETS = os.path.join(SECRETS_DIR, "instance.env")
MCP = os.path.join(ROOT, "tools", "mdmcp.py")

# Se pide cada fichero CON ETIQUETA: si no, dos URLs indistinguibles se
# mezclan (el túnel del daemon y el del MCP legacy son dos hosts distintos).
CMD = ("echo TUNNEL_URL=$(cat /data/local/tmp/megadaemon/tunnel.url 2>/dev/null); "
       "echo LEGACY_MCP_URL=$(cat /data/local/tmp/megadaemon/mcp_tunnel.url 2>/dev/null)")


URL_RE = re.compile(r"https://[A-Za-z0-9._-]+\.trycloudflare\.com")


def phone_urls():
    """(URL web del daemon, URL del MCP legacy) leídos DEL MÓVIL.

    La salida del cliente MCP puede venir como JSON (structuredContent) o como
    texto plano según la versión ⇒ se buscan las URLs con una expresión
    regular en TODO el stdout, y se desambigua por la etiqueta que el propio
    móvil devuelve (TUNNEL_URL / LEGACY_MCP_URL).
    """
    r = subprocess.run(
        ["python3", MCP, "call", "root_exec", json.dumps({"command": CMD, "max_chars": 800})],
        capture_output=True, text=True, timeout=180,
    )
    out = (r.stdout or "") + "\n" + (r.stderr or "")
    web = ""
    mcp = ""
    for line in out.splitlines():
        if "TUNNEL_URL=" in line and "LEGACY_MCP_URL=" not in line:
            v = line.split("TUNNEL_URL=", 1)[1]
            m = URL_RE.search(v)
            if m:
                web = m.group(0)
        elif "LEGACY_MCP_URL=" in line:
            v = line.split("LEGACY_MCP_URL=", 1)[1]
            m = URL_RE.search(v)
            if m:
                mcp = m.group(0)
    if not web:
        # Fallback: la primera URL que aparezca es la del daemon.
        found = URL_RE.findall(out)
        web = found[0] if found else ""
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
        # Tambien en md-secrets/tunnel.url: si este fichero se queda CADUCADO
        # (la URL es efimera) las herramientas que lo leen首先 连接的是死 URL y
        # el movil parece caido. Se refresca aqui, en el mismo sitio.
        tpath = os.path.join(SECRETS_DIR, "tunnel.url")
        with open(tpath, "w") as tf:
            tf.write(web + "\n")
        os.chmod(tpath, 0o600)
    if mcp:
        env["MEGADAEMON_MCP_LEGACY"] = mcp
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

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Sube un binario al dispositivo por trozos (fs_write base64) y verifica SHA-256.

Nada de credenciales ni de URLs hardcodeadas en el repo (que es público):

* **ENDPOINT**: `MEGADAEMON_MCP_ENDPOINT` > fichero `tunnel.url` (el que
  escribe el watchdog del túnel) > nada (se avisa y se sale). El hostname de
  un quick tunnel rota en cada reinicio: hardcodearlo es guarantees de fallo.
* **Credenciales**: `MEGADAEMON_API_KEY` / `MEGADAEMON_SEERE_TOKEN` (o los
  ficheros 0600 que los apunten con `*_FILE`) y `MEGADAEMON_LEGACY_TOKEN`
  para el MCP legacy. Nunca se imprimen.
"""
import base64
import os
import hashlib
import json
import sys
import urllib.request

TOKEN = os.environ.get("MEGADAEMON_LEGACY_TOKEN", "")
API_KEY = os.environ.get("MEGADAEMON_API_KEY", "")
SEERE_TOKEN = os.environ.get("MEGADAEMON_SEERE_TOKEN", "")

# Ficheros 0600 que pueden contener las credenciales (el móvil los regenera).
for _var, _file in (("MEGADAEMON_API_KEY", "/data/local/tmp/megadaemon/api.key"),
                    ("MEGADAEMON_SEERE_TOKEN", "/data/local/tmp/megadaemon/cockpit_token"),
                    ("MEGADAEMON_LEGACY_TOKEN", "/data/local/tmp/megadaemon/legacy.token")):
    if not os.environ.get(_var) and os.path.exists(_file):
        try:
            with open(_file) as fh:
                os.environ[_var] = fh.read().strip()
        except OSError:
            pass
API_KEY = os.environ.get("MEGADAEMON_API_KEY", "")
SEERE_TOKEN = os.environ.get("MEGADAEMON_SEERE_TOKEN", "")
TOKEN = os.environ.get("MEGADAEMON_LEGACY_TOKEN", "")

# Ficheros donde puede estar la URL vigente del túnel.
_URL_FILES = ("/data/local/tmp/megadaemon/tunnel.url",
              os.path.expanduser("~/megadaemon/state/tunnel.url"),
              "/workspace/megadaemon/state/tunnel.url")


def resolve_endpoint():
    """Devuelve el endpoint MCP vigente o None (sin inventar URLs)."""
    env = os.environ.get("MEGADAEMON_MCP_ENDPOINT", "").strip()
    if env:
        return env.rstrip("/")
    for path in _URL_FILES:
        try:
            with open(path) as fh:
                url = fh.read().strip()
            if url.startswith("http"):
                return url.rstrip("/")
        except OSError:
            continue
    return None


ENDPOINT = resolve_endpoint()


def rpc(tool, args, _id=1, timeout=900):
    body = json.dumps({"jsonrpc": "2.0", "id": _id, "method": "tools/call",
                       "params": {"name": tool, "arguments": args}}).encode()
    req = urllib.request.Request(ENDPOINT + "/mcp", data=body)
    if API_KEY:
        req.add_header("X-API-Key", API_KEY)
    if SEERE_TOKEN:
        req.add_header("X-Seere-Auth-Token", SEERE_TOKEN)
    if TOKEN:
        req.add_header("Authorization", "Bearer " + TOKEN)
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode())


def main():
    if not ENDPOINT:
        print("ERROR: no hay endpoint MCP vigente.\n"
              "  export MEGADAEMON_MCP_ENDPOINT=https://<tunel-vivo>\n"
              "  (o deja state/tunnel.url actualizado: lo escribe el watchdog)",
              file=sys.stderr)
        return 2
    local, remote = sys.argv[1], sys.argv[2]
    chunk = 400 * 1024
    data = open(local, "rb").read()
    total = len(data)
    digest = hashlib.sha256(data).hexdigest()
    print("subiendo %d bytes a %s (sha256 %s)" % (total, ENDPOINT, digest))
    parts = []
    for i, off in enumerate(range(0, total, chunk)):
        p = "/data/local/tmp/.mdpart_%02d" % i
        parts.append(p)
        b64 = base64.b64encode(data[off:off + chunk]).decode()
        res = rpc("fs_write", {"path": p, "content": b64, "base64": True, "chmod": "644"}, i + 1)
        txt = res["result"]["content"][0]["text"]
        print("  parte %d/%d: %s" % (i + 1, (total + chunk - 1) // chunk, txt[:60]))
    cmd = "cat %s > %s && chmod 755 %s && rm -f %s && sha256sum %s && ls -la %s" % (
        " ".join(parts), remote, remote, " ".join(parts), remote, remote)
    res = rpc("root_exec", {"command": cmd, "timeout": 180000, "max_chars": 4096}, 99)
    out = res["result"]["content"][0]["text"]
    print(out)
    ok = digest in out
    print("SHA256 MATCH" if ok else "SHA256 MISMATCH")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

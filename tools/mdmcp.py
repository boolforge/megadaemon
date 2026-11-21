#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Cliente MCP de MegaDaemon v13.3 (endpoint HTTP nativo del cockpit).

    python3 mdmcp.py tools
    python3 mdmcp.py call <TOOL> '<json args>'
    python3 mdmcp.py raw '<jsonrpc body>'

Autenticación en dos capas, igual que el cockpit:
  * `X-API-Key`  → control de acceso remoto (API key del estado).
  * `X-Seere-Auth-Token` → autorización de operaciones que cambian estado.

FUENTE DE VERDAD (2026-11-20): `md-secrets/instance.env` (+ `md-secrets/tunnel.url`
para la URL, que es efímera). El entorno del shell tiene PRECEDENCIA en las
herramientas viejas y es la trampa clásica: con una key revocada exportada,
este cliente devolvía 403 y **parecía que el móvil estaba caído cuando
estaba perfecto**. Aquí manda el fichero; si el entorno contradice, se avisa
con el sha16 de cada valor (sin imprimir secretos) y se sigue con el fichero.
"""
from __future__ import annotations

import hashlib
import json
import os
import sys
import urllib.error
import urllib.request

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
TUNNEL = os.path.join(SECRETS_DIR, "tunnel.url")
LEGACY = os.environ.get("MEGADAEMON_MCP_LEGACY", "https://TUNEL.trycloudflare.com/mcp")


def _sha16(v: str) -> str:
    return hashlib.sha256((v or "").encode()).hexdigest()[:16] if v else "-"


def _read_env_file(path: str) -> dict:
    env = {}
    try:
        for line in open(path, encoding="utf-8", errors="replace"):
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, v = line.split("=", 1)
                env[k] = v.strip()
    except OSError:
        pass
    return env


def _resolve() -> tuple[str, str, str, list[str]]:
    """(endpoint, key, token, avisos). Fichero primero; entorno como override."""
    avisos = []
    env = _read_env_file(SECRETS)

    endpoint = ""
    if os.path.exists(TUNNEL):
        try:
            u = open(TUNNEL, encoding="utf-8").read().strip().rstrip("/")
            if u.startswith("http"):
                endpoint = u if u.endswith("/mcp") else u + "/mcp"
        except OSError:
            pass
    if not endpoint:
        endpoint = env.get("MEGADAEMON_MCP", "")
    if not endpoint:
        endpoint = os.environ.get("MEGADAEMON_MCP", "")
    if not endpoint:
        endpoint = LEGACY
        avisos.append("sin URL en instance.env/tunnel.url: uso la legacy (puede estar caída)")

    key = env.get("MEGADAEMON_API_KEY", "")
    if not key:
        try:
            key = open(os.path.expanduser("~/.md_key")).read().strip()
        except Exception:
            pass
    if not key:
        key = os.environ.get("MEGADAEMON_API_KEY", "").strip()

    token = env.get("MEGADAEMON_COCKPIT_TOKEN", "") or env.get("MEGADAEMON_TOKEN", "")
    if not token:
        try:
            token = open(os.path.expanduser("~/.md_token")).read().strip()
        except Exception:
            pass
    if not token:
        token = os.environ.get("MEGADAEMON_TOKEN", "").strip()

    # Estado explícito del entorno: "avisa y sigue" no basta si el usuario
    # sólo lee la última línea ⇒ se dice QUÉ se mandó y QUÉ se ignoró.
    shell_key = os.environ.get("MEGADAEMON_API_KEY", "")
    shell_tok = os.environ.get("MEGADAEMON_TOKEN", "")
    conflict = []
    if shell_key and key and _sha16(shell_key) != _sha16(key):
        conflict.append("key %s(revocada?)" % _sha16(shell_key))
    if shell_tok and token and _sha16(shell_tok) != _sha16(token):
        conflict.append("token %s" % _sha16(shell_tok))
    estado = "credenciales: fichero (key sha16 %s · token sha16 %s)" % (_sha16(key), _sha16(token))
    if conflict:
        estado += " | entorno IGNORADO (%s)" % ", ".join(conflict)
        avisos.append(estado + " — el móvil puede estar PERFECTO y ser tu credencial")
    return endpoint, key, token, avisos, estado


ENDPOINT, KEY, TOKEN, AVISOS, ESTADO = _resolve()
for _a in AVISOS:
    print("[mdmcp] " + _a, file=sys.stderr)



def rpc(method: str, params: dict, _id: int = 1, timeout: int = 900) -> dict:
    body = json.dumps({"jsonrpc": "2.0", "id": _id, "method": method, "params": params}).encode()
    req = urllib.request.Request(ENDPOINT, data=body, method="POST")
    if KEY:
        req.add_header("X-API-Key", KEY)
    if TOKEN:
        req.add_header("X-Seere-Auth-Token", TOKEN)
    req.add_header("Content-Type", "application/json")
    req.add_header("Accept", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        blob = e.read().decode("utf-8", "replace")[:400]
        if e.code in (401, 403):
            # El móvil puede estar PERFECTO y ser la credencial: diagnóstico.
            print("[mdmcp] HTTP %s en %s" % (e.code, ENDPOINT), file=sys.stderr)
            print("[mdmcp]   %s" % ESTADO, file=sys.stderr)
            print("[mdmcp]   re-sincroniza con: python3 tools/phone_url.py --export", file=sys.stderr)
        raise urllib.error.HTTPError(e.url, e.code, e.reason, e.headers, io_bytes(blob))
    except urllib.error.URLError as e:
        print("[mdmcp] no se pudo alcanzar %s (%s): ¿túnel caído? "
              "re-sincroniza con: python3 tools/phone_url.py --export"
              % (ENDPOINT, e.reason), file=sys.stderr)
        raise


def io_bytes(s: str):
    import io
    return io.BytesIO(s.encode("utf-8"))



def init() -> None:
    rpc("initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "mcode", "version": "1"},
    }, 1)


def call(tool: str, args: dict | None = None) -> dict:
    init()
    return rpc("tools/call", {"name": tool, "arguments": args or {}}, 2)


def render(res: dict) -> str:
    sc = res.get("result", {}).get("structuredContent")
    if isinstance(sc, dict):
        return json.dumps(sc, ensure_ascii=False, indent=2)
    out = []
    for item in res.get("result", {}).get("content", []) or []:
        if item.get("type") == "text":
            out.append(item.get("text", ""))
    if not out:
        out.append(json.dumps(res, ensure_ascii=False, indent=2))
    return "\n".join(out)


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "tools"
    if cmd == "tools":
        init()
        res = rpc("tools/list", {}, 3)
        for t in res.get("result", {}).get("tools", []):
            print("### %s" % t["name"])
            print((t.get("description") or "").strip()[:400])
            props = t.get("inputSchema", {}).get("properties", {})
            if props:
                print("   params:", ", ".join("%s:%s" % (k, v.get("type", "?")) for k, v in props.items()))
            print()
    elif cmd == "call":
        tool = sys.argv[2]
        args = json.loads(sys.argv[3]) if len(sys.argv) > 3 else {}
        print(render(call(tool, args)))
    elif cmd == "raw":
        print(json.dumps(rpc("tools/list", {}, 4), ensure_ascii=False, indent=2)[:4000])

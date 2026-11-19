#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Cliente MCP de MegaDaemon v13.3 (endpoint HTTP nativo del cockpit).

    python3 mdmcp.py tools
    python3 mdmcp.py call <TOOL> '<json args>'
    python3 mdmcp.py raw '<jsonrpc body>'

Autenticación en dos capas, igual que el cockpit:
  * `X-API-Key`  → control de acceso remoto (API key del estado).
  * `X-Seere-Auth-Token` → autorización de operaciones que cambian estado.
"""
from __future__ import annotations

import json
import os
import sys
import urllib.request

ENDPOINT = os.environ.get("MEGADAEMON_MCP", "https://webpage-ultram-please-communicate.trycloudflare.com/mcp")
KEY = os.environ.get("MEGADAEMON_API_KEY", "").strip()
TOKEN = os.environ.get("MEGADAEMON_TOKEN", "").strip()
if not KEY:
    try:
        KEY = open(os.path.expanduser("~/.md_key")).read().strip()
    except Exception:
        pass
if not TOKEN:
    try:
        TOKEN = open(os.path.expanduser("~/.md_token")).read().strip()
    except Exception:
        pass


def rpc(method: str, params: dict, _id: int = 1, timeout: int = 900) -> dict:
    body = json.dumps({"jsonrpc": "2.0", "id": _id, "method": method, "params": params}).encode()
    req = urllib.request.Request(ENDPOINT, data=body, method="POST")
    if KEY:
        req.add_header("X-API-Key", KEY)
    if TOKEN:
        req.add_header("X-Seere-Auth-Token", TOKEN)
    req.add_header("Content-Type", "application/json")
    req.add_header("Accept", "application/json")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode())


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

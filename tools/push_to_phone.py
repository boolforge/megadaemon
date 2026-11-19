#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Sube un binario ARM64 al Samsung (instancia primary) usando el propio
MegaDaemon como canal — sin adb local.

El cockpit expone `POST /api/chat_upload` (JSON con `filename` y `data_b64`)
y escribe el fichero en el directorio de subidas del móvil.  A partir de
ahí, `root_exec` (MCP) lo instala, lo arranca y verifica `/health`.

Credenciales: `MEGADAEMON_URL`, `MEGADAEMON_API_KEY` y
`MEGADAEMON_TOKEN` (fichero /workspace/md-secrets/instance.env).  Nunca se
escriben en el repo ni se imprimen.

    python3 tools/push_to_phone.py /ruta/al/binario megadaemon-core.arm64
"""
from __future__ import annotations

import base64
import json
import os
import sys
import urllib.request

# REGLA (medida el 2026-11-20): el fichero canónico de secretos
# (`md-secrets/instance.env`) GANA al entorno.  Una key/token exportados en
# la shell quedan obsoletos en cuanto alguien rota el secreto => el push
# manda la credencial revocada, el daemon responde 403 y parece que "el
# movil esta caido".  El entorno se usa solo si el fichero no trae el valor.
# La URL publica es efimera: se lee de `tunnel.url` via `tools/phone_url.py`.
URL = os.environ.get("MEGADAEMON_URL", "").rstrip("/")
KEY = os.environ.get("MEGADAEMON_API_KEY", "").strip()
TOKEN = os.environ.get("MEGADAEMON_TOKEN", "").strip()


def load_env_file(path: str) -> None:
    global KEY, TOKEN
    if not os.path.exists(path):
        return
    for line in open(path, encoding="utf-8", errors="replace"):
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        v = v.strip().strip('"').strip("'")
        if k.strip() == "MEGADAEMON_API_KEY":
            KEY = v
        if k.strip() in ("MEGADAEMON_COCKPIT_TOKEN", "MEGADAEMON_TOKEN"):
            TOKEN = v
        if k.strip() == "MEGADAEMON_URL":
            URL = v.rstrip("/")


def _canonical() -> None:
    """Carga `md-secrets/instance.env` (fuente de verdad tras una rotación)."""
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    for cand in (os.path.join(here, "md-secrets", "instance.env"),
                 "/workspace/md-secrets/instance.env"):
        if os.path.exists(cand):
            load_env_file(cand)
            return


def main() -> int:
    if len(sys.argv) < 2:
        print("uso: push_to_phone.py <binario> [nombre_en_el_movil]", file=sys.stderr)
        return 2
    src = sys.argv[1]
    name = sys.argv[2] if len(sys.argv) > 2 else os.path.basename(src)
    # El fichero canonico manda sobre el entorno (rotaciones de secretos).
    _canonical()
    if not URL:
        URL = "https://webpage-ultram-please-communicate.trycloudflare.com"
    load_env_file("/workspace/md-secrets/instance.env")
    if not KEY:
        print("falta MEGADAEMON_API_KEY", file=sys.stderr)
        return 2

    raw = open(src, "rb").read()
    body = json.dumps({"filename": name, "data_b64": base64.b64encode(raw).decode()}).encode()
    req = urllib.request.Request(f"{URL}/api/chat_upload", data=body, method="POST")
    req.add_header("Content-Type", "application/json")
    req.add_header("X-API-Key", KEY)
    if TOKEN:
        req.add_header("X-Seere-Auth-Token", TOKEN)
    try:
        with urllib.request.urlopen(req, timeout=300) as resp:
            payload = resp.read(600).decode("utf-8", "replace")
            print(f"HTTP {resp.status} · {len(raw)} bytes -> {name}")
            print(payload)
    except Exception as exc:  # noqa: BLE001
        print(f"error subiendo: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

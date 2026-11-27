#!/usr/bin/env python3
"""agy_auth.py — acuña un token OAuth real para el Language Server.

POR QUÉ EXISTE
--------------
El Language Server de Antigravity abre `~/.gemini/jetski-standalone-oauth-token`
y lo deserializa como JSON. Si el token es falso responde
"error getting token source: You are not logged into Antigravity" y NO emite
ninguna petición (medido: AGY-W12-002).

Un fichero `application_default_credentials.json` de tipo `authorized_user`
contiene `client_id` + `client_secret` + `refresh_token`: con eso se puede
intercambiar el refresh_token por un access_token REAL en el endpoint de
Google, y construir el fichero de token que el LS sí acepta.

USO
---
    python3 tools/agy_auth.py mint \
        --adc /ruta/application_default_credentials.json \
        --install                     # escribe ~/.gemini/jetski-standalone-oauth-token

    python3 tools/agy_auth.py show    # sólo metadatos, NUNCA el token

SEGURIDAD
---------
- No imprime ningún valor de token, credencial ni refresh_token: sólo longitudes,
  caducidad y scopes.
- El ADC se lee en memoria y no se copia a disco.
- `--install` escribe el token con modo 0600.
"""
from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import pathlib
import stat
import sys
import urllib.error
import urllib.parse
import urllib.request

TOKEN_URL = "https://oauth2.googleapis.com/token"
TOKEN_PATH = pathlib.Path.home() / ".gemini" / "jetski-standalone-oauth-token"


def mint(adc_path: str, scope: str | None = None):
    """Intercambia el refresh_token por un access_token real."""
    with open(adc_path, encoding="utf-8") as fh:
        adc = json.load(fh)
    if adc.get("type") != "authorized_user":
        raise SystemExit("el ADC no es de tipo authorized_user (es %r); "
                         "hace falta client_id+client_secret+refresh_token"
                         % adc.get("type"))
    for k in ("client_id", "client_secret", "refresh_token"):
        if not adc.get(k):
            raise SystemExit("el ADC no trae %s" % k)
    data = {
        "client_id": adc["client_id"],
        "client_secret": adc["client_secret"],
        "refresh_token": adc["refresh_token"],
        "grant_type": "refresh_token",
    }
    if scope:
        data["scope"] = scope
    req = urllib.request.Request(
        TOKEN_URL, data=urllib.parse.urlencode(data).encode(),
        headers={"Content-Type": "application/x-www-form-urlencoded"})
    try:
        resp = json.load(urllib.request.urlopen(req, timeout=30))
    except urllib.error.HTTPError as e:
        body = {}
        try:
            body = json.load(e)
        except Exception:
            pass
        raise SystemExit("el token endpoint devolvio HTTP %d: %s %s"
                         % (e.code, body.get("error"), body.get("error_description")))
    if "access_token" not in resp:
        raise SystemExit("respuesta sin access_token: %s" % sorted(resp))
    resp["_adc_refresh_token"] = adc["refresh_token"]
    resp["_adc_client_id"] = adc["client_id"]
    resp["_adc_client_secret"] = adc["client_secret"]
    return resp


def build_token_file(tok: dict) -> dict:
    """Construye el JSON que el LS deserializa (medido en el binario)."""
    exp = int(tok.get("expires_in", 3600))
    expiry = _dt.datetime.now(_dt.timezone.utc) + _dt.timedelta(seconds=exp)
    out = {
        "access_token": tok["access_token"],
        "refresh_token": tok.get("_adc_refresh_token", ""),
        "token_type": tok.get("token_type", "Bearer"),
        "expiry": expiry.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "client_id": tok.get("_adc_client_id", ""),
        "client_secret": tok.get("_adc_client_secret", ""),
        "scope": tok.get("scope", ""),
        "universe_domain": "googleapis.com",
    }
    if tok.get("id_token"):
        out["id_token"] = tok["id_token"]
    return out


def install(tok: dict, path: pathlib.Path = TOKEN_PATH) -> pathlib.Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(build_token_file(tok), indent=1), encoding="utf-8")
    os.chmod(path, stat.S_IRUSR | stat.S_IWUSR)          # 0600
    return path


def describe(tok: dict) -> dict:
    exp = tok.get("expires_in")
    return {
        "access_token_len": len(tok.get("access_token", "")),
        "has_id_token": bool(tok.get("id_token")),
        "has_refresh_token": bool(tok.get("_adc_refresh_token")),
        "expires_in_s": exp,
        "refresh_token_expires_in_s": tok.get("refresh_token_expires_in"),
        "scope": tok.get("scope", "").split(),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["mint", "show"])
    ap.add_argument("--adc", default="/workspace/md-secrets/agy-intel/application_default_credentials.json")
    ap.add_argument("--scope", default=None,
                    help="scope extra para pedir id_token (p.ej. openid email profile)")
    ap.add_argument("--install", action="store_true")
    ap.add_argument("--token-path", default=str(TOKEN_PATH))
    a = ap.parse_args()

    if a.cmd == "show":
        p = pathlib.Path(a.token_path)
        if not p.exists():
            print(json.dumps({"instalado": False, "ruta": str(p)}, ensure_ascii=False))
            return 1
        d = json.load(open(p, encoding="utf-8"))
        print(json.dumps({
            "instalado": True, "ruta": str(p),
            "access_token_len": len(d.get("access_token", "")),
            "refresh_token_len": len(d.get("refresh_token", "")),
            "has_id_token": bool(d.get("id_token")),
            "expiry": d.get("expiry"),
            "scope": d.get("scope", "").split(),
        }, ensure_ascii=False, indent=1))
        return 0

    tok = mint(a.adc, a.scope)
    info = describe(tok)
    if a.install:
        p = install(tok, pathlib.Path(a.token_path))
        info["instalado_en"] = str(p)
        info["modo"] = "0600"
    print(json.dumps(info, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())

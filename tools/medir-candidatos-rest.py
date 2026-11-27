#!/usr/bin/env python3
"""Mide repos de agentes de codigo que la consulta GraphQL no cubrio (REST /repos).

Sin dependencias externas: usa urllib. El token sale del credential helper
local; nunca se imprime ni se escribe en disco.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import urllib.error
import urllib.request

CRED = "/workspace/.home/.git-credentials"


def token() -> str:
    """Lee el token del credential store local sin mostrarlo."""
    try:
        out = subprocess.run(
            ["git", "credential", "fill"],
            input=f"protocol=https\nhost=github.com\n\n",
            capture_output=True,
            text=True,
            timeout=30,
            check=True,
        ).stdout
    except Exception:
        out = open(CRED).read()
    for line in out.splitlines():
        if line.startswith("password="):
            return line.split("=", 1)[1].strip()
    raise SystemExit("no hay token de GitHub en el credential store")


def measure(repo: str, tok: str) -> dict:
    url = f"https://api.github.com/repos/{repo}"
    req = urllib.request.Request(
        url,
        headers={
            "Authorization": f"Bearer {tok}",
            "Accept": "application/vnd.github+json",
            "User-Agent": "mdaemon-re-catalog",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=45) as r:
            d = json.load(r)
    except urllib.error.HTTPError as e:
        return {"repo": repo, "error": f"HTTP {e.code}"}
    except Exception as e:  # noqa: BLE001
        return {"repo": repo, "error": type(e).__name__}
    lic = d.get("license") or {}
    return {
        "repo": repo,
        "stars": d.get("stargazers_count"),
        "forks": d.get("forks_count"),
        "lang": d.get("language"),
        "license": lic.get("spdx_id") or "NOASSERTION",
        "kb": (d.get("size") or 0) * 1024,
        "pushed": (d.get("pushed_at") or "")[:10],
        "created": (d.get("created_at") or "")[:10],
        "branch": d.get("default_branch"),
        "archived": bool(d.get("archived")),
        "desc": d.get("description") or "",
    }


def main() -> int:
    if len(sys.argv) < 2:
        print("uso: medir-candidatos-rest.py <salida.json> [repo ...]", file=sys.stderr)
        return 2
    out_path = sys.argv[1]
    repos = sys.argv[2:]
    if not repos:
        cands = [
            l.strip()
            for l in open("tools/agentes-candidatos.txt")
            if l.strip() and not l.startswith("#")
        ]
        med = set(json.load(open("docs/medidas/agentes_faltantes.json")))
        repos = [c for c in cands if c not in med]
    tok = token()
    res = {}
    for repo in repos:
        m = measure(repo, tok)
        res[repo] = m
        flag = "ERROR " + m["error"] if "error" in m else f"{m['stars']:>7}★ {m['lang']}"
        print(f"{repo:<45} {flag}", flush=True)
    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    with open(out_path, "w") as fh:
        json.dump(res, fh, indent=2, ensure_ascii=False)
    print(f"\nescrito {out_path} ({len(res)} repos)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

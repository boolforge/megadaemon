#!/usr/bin/env python3
"""Resuelve los repos que 404 por nombre: busca el renombrado real.

Para cada repo medido con HTTP 404 se hace una busqueda REST por nombre y se
elige el candidato mas cercano (mismo nombre base, o el mismo owner).
"""
from __future__ import annotations

import json
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request


def token() -> str:
    out = subprocess.run(
        ["git", "credential", "fill"],
        input="protocol=https\nhost=github.com\n\n",
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    ).stdout
    for line in out.splitlines():
        if line.startswith("password="):
            return line.split("=", 1)[1].strip()
    raise SystemExit("sin token")


TOK = token()


def api(url: str):
    req = urllib.request.Request(
        url,
        headers={
            "Authorization": f"Bearer {TOK}",
            "Accept": "application/vnd.github+json",
            "User-Agent": "mdaemon-re-catalog",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=45) as r:
            return r.status, json.load(r)
    except urllib.error.HTTPError as e:
        return e.code, None
    except Exception:  # noqa: BLE001
        return 0, None


def buscar(nombre: str, owner: str | None) -> list[dict]:
    q = f"{nombre} in:name"
    if owner:
        q += f" user:{owner}"
    s, d = api(
        "https://api.github.com/search/repositories?q="
        + urllib.parse.quote(q)
        + "&sort=stars&order=desc&per_page=10"
    )
    if s != 200 or not d:
        return []
    return d.get("items", [])


def puntuar(cand: dict, nombre: str, owner: str) -> int:
    full = cand["full_name"].lower()
    base = cand["name"].lower()
    score = 0
    if base == nombre.lower():
        score += 100
    elif nombre.lower() in base or base in nombre.lower():
        score += 40
    if owner.lower() == full.split("/")[0]:
        score += 60
    score += min(cand.get("stargazers_count", 0), 200000) // 5000
    if cand.get("archived"):
        score -= 15
    return score


def main() -> int:
    src = sys.argv[1] if len(sys.argv) > 1 else "docs/medidas/agentes_faltantes_rest.json"
    out = sys.argv[2] if len(sys.argv) > 2 else "docs/medidas/agentes_renombrados.json"
    fallidos = [k for k, v in json.load(open(src)).items() if "error" in v]
    res = {}
    for repo in fallidos:
        owner, _, nombre = repo.partition("/")
        cands = buscar(nombre, owner)
        best = None
        bs = -1
        for c in cands:
            sc = puntuar(c, nombre, owner)
            if sc > bs:
                bs, best = sc, c
        if best is None:
            res[repo] = {"repo": repo, "estado": "no encontrado"}
            print(f"{repo:<40} NO ENCONTRADO", flush=True)
            continue
        lic = best.get("license") or {}
        res[repo] = {
            "repo_original": repo,
            "repo_real": best["full_name"],
            "estado": "archivado" if best.get("archived") else "activo",
            "stars": best.get("stargazers_count"),
            "forks": best.get("forks_count"),
            "lang": best.get("language"),
            "license": lic.get("spdx_id") or "NOASSERTION",
            "kb": (best.get("size") or 0) * 1024,
            "pushed": (best.get("pushed_at") or "")[:10],
            "created": (best.get("created_at") or "")[:10],
            "archived": bool(best.get("archived")),
            "desc": best.get("description") or "",
            "url": best.get("html_url"),
        }
        r = res[repo]
        print(
            f"{repo:<40} -> {r['repo_real']:<40} {r['stars']:>7}★ "
            f"{r['estado']:<9} {r['license']}",
            flush=True,
        )
    with open(out, "w") as fh:
        json.dump(res, fh, indent=2, ensure_ascii=False)
    print(f"\nescrito {out} ({len(res)} casos)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

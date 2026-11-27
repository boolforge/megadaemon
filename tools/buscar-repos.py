#!/usr/bin/env python3
"""Busca repos con la API de busqueda de GitHub y los mide en una sola pasada.

Se usa para: (a) decompiladores alternativos a Ghidra/r2ghidra, sobre todo
para Go; (b) proyectos de ingenieria inversa publicados por agentes.
Politica del root: NO usar web_search -> todo por API directa.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
import urllib.parse

CRED = "/workspace/.home/.git-credentials"


def token() -> str:
    txt = open(CRED, encoding="utf-8", errors="replace").read()
    m = re.search(r"(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})", txt)
    if not m:
        raise SystemExit("sin token")
    return m.group(1)


def buscar(consulta: str, por_pagina: int = 20) -> list:
    url = (
        "https://api.github.com/search/repositories?q="
        + urllib.parse.quote(consulta)
        + f"&sort=stars&order=desc&per_page={por_pagina}"
    )
    proc = subprocess.run(
        ["curl", "-s", url, "-H", "Authorization: bearer " + token(),
         "-H", "Accept: application/vnd.github+json"],
        capture_output=True, text=True, timeout=90,
    )
    try:
        datos = json.loads(proc.stdout)
    except json.JSONDecodeError:
        return []
    salida = []
    for it in datos.get("items", []):
        lic = it.get("license") or {}
        salida.append(
            {
                "full_name": it.get("full_name"),
                "stars": it.get("stargazers_count"),
                "forks": it.get("forks_count"),
                "lang": it.get("language"),
                "license": lic.get("spdxId"),
                "pushed": (it.get("pushed_at") or "")[:10],
                "desc": (it.get("description") or "")[:200],
                "archived": it.get("archived"),
                "url": it.get("html_url"),
            }
        )
    return salida


if __name__ == "__main__":
    consultas = json.load(open(sys.argv[1], encoding="utf-8"))
    todo = {}
    for etiqueta, q in consultas:
        res = buscar(q)
        print(f"[buscar] {etiqueta}: {len(res)} resultados", file=sys.stderr)
        for r in res:
            todo.setdefault(r["full_name"], dict(r, encontrado_por=etiqueta))
    json.dump(todo, open(sys.argv[2], "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print(f"[buscar] {len(todo)} repos unicos -> {sys.argv[2]}", file=sys.stderr)

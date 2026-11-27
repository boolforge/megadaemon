#!/usr/bin/env python3
"""Mide en UNA sola consulta GraphQL un lote de repos y guarda el JSON.

Se usa para el catalogo de agentes de codigo que faltan por medir: una
consulta con N alias en lugar de N peticiones (limite de la API medido).
"""

from __future__ import annotations

import json
import re
import subprocess
import sys

CRED = "/workspace/.home/.git-credentials"

FIELDS = """
  nameWithOwner stargazerCount forkCount primaryLanguage { name } licenseInfo { spdxId }
  diskUsage pushedAt createdAt description defaultBranchRef { name }
  isArchived isFork
"""


def token() -> str:
    txt = open(CRED, encoding="utf-8", errors="replace").read()
    m = re.search(r"(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})", txt)
    if not m:
        raise SystemExit("sin token en " + CRED)
    return m.group(1)


def medir(repos: list[str]) -> dict:
    partes = []
    for i, r in enumerate(repos):
        owner, _, name = r.partition("/")
        partes.append(
            f'r{i}: repository(owner:"{owner}", name:"{name}") {{ ...F }}'
        )
    query = "query { " + " ".join(partes) + " } fragment F on Repository { " + FIELDS + " }"
    payload = json.dumps({"query": query})
    proc = subprocess.run(
        [
            "curl", "-s", "-X", "POST",
            "https://api.github.com/graphql",
            "-H", "Authorization: bearer " + token(),
            "-H", "Content-Type: application/json",
            "--data-binary", payload,
        ],
        capture_output=True,
        text=True,
        timeout=180,
    )
    data = json.loads(proc.stdout)
    if "errors" in data and not data.get("data"):
        raise SystemExit("GraphQL: " + json.dumps(data["errors"])[:500])
    salida = {}
    nodos = (data.get("data") or {})
    for i, r in enumerate(repos):
        n = nodos.get(f"r{i}")
        if not n:
            continue
        if "nameWithOwner" not in n:
            # no existe o sin permiso
            continue
        lic = (n.get("licenseInfo") or {}).get("spdxId")
        topics = []
        salida[r] = {
            "repo": n.get("nameWithOwner", r),
            "stars": n.get("stargazerCount"),
            "forks": n.get("forkCount"),
            "lang": (n.get("primaryLanguage") or {}).get("name"),
            "license": lic,
            "kb": n.get("diskUsage"),
            "pushed": (n.get("pushedAt") or "")[:10],
            "created": (n.get("createdAt") or "")[:10],
            "branch": (n.get("defaultBranchRef") or {}).get("name"),
            "archived": n.get("isArchived"),
            "topics": topics,
            "desc": (n.get("description") or "")[:220],
        }
    return salida


if __name__ == "__main__":
    lista = [l.strip() for l in open(sys.argv[1], encoding="utf-8") if l.strip() and not l.startswith("#")]
    print(f"midiendo {len(lista)} repos...", file=sys.stderr)
    res = medir(lista)
    json.dump(res, open(sys.argv[2], "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print(f"ok: {len(res)}/{len(lista)} medidos -> {sys.argv[2]}", file=sys.stderr)

#!/usr/bin/env python3
"""Busca agentes de codigo que se hayan escapado del catalogo (MEDIDO).

Consultas REST de busqueda de GitHub por topicos y por descripcion, se
descarta lo ya medido y se miden los nuevos con /repos.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request

QUERIES = [
    "topic:coding-agent",
    "topic:ai-coding-agent",
    "topic:agent-cli",
    "topic:ai-agent in:name,description",
    "coding agent in:description stars:>800",
    "ai coding assistant in:description stars:>800",
    "terminal ai agent in:description stars:>500",
    "autonomous coding in:description stars:>500",
    "topic:claude-code",
    "topic:codex",
    "topic:agent-cli stars:>300",
    "cli agent in:name stars:>500",
]

OUT = "docs/medidas/agentes_escapados.json"


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


def buscar(q: str, pagina: int = 1) -> list[dict]:
    s, d = api(
        "https://api.github.com/search/repositories?q="
        + urllib.parse.quote(q)
        + f"&sort=stars&order=desc&per_page=50&page={pagina}"
    )
    if s != 200 or not d:
        return []
    return d.get("items", [])


def ya_medido(full: str) -> bool:
    cat = json.load(open("docs/medidas/agentes_faltantes.json"))
    ren = json.load(open("docs/medidas/agentes_renombrados.json"))
    baja = {k.lower() for k in cat}
    baja |= {v.get("repo_real", "").lower() for v in ren.values()}
    baja |= {v.get("repo_original", "").lower() for v in ren.values()}
    return full.lower() in baja


def main() -> int:
    vistos: dict[str, dict] = {}
    for q in QUERIES:
        items = buscar(q)
        nuevos = [i for i in items if not ya_medido(i["full_name"])]
        print(f"{q:<48} total={len(items):>3} fuera_del_catalogo={len(nuevos):>3}", flush=True)
        for i in nuevos:
            entry = vistos.setdefault(
                i["full_name"],
                {
                    "repo": i["full_name"],
                    "stars": i.get("stargazers_count"),
                    "forks": i.get("forks_count"),
                    "lang": i.get("language"),
                    "license": (i.get("license") or {}).get("spdx_id") or "NOASSERTION",
                    "kb": (i.get("size") or 0) * 1024,
                    "pushed": (i.get("pushed_at") or "")[:10],
                    "created": (i.get("created_at") or "")[:10],
                    "archived": bool(i.get("archived")),
                    "desc": i.get("description") or "",
                    "url": i.get("html_url"),
                    "visto_por": [],
                },
            )
            entry["visto_por"].append(q)
    # medimos en detalle los nuevos que superen el corte de estrellas
    out = {}
    for full, e in vistos.items():
        s, d = api(f"https://api.github.com/repos/{full}")
        if s == 200 and d:
            lic = d.get("license") or {}
            e["stars"] = d.get("stargazers_count")
            e["forks"] = d.get("forks_count")
            e["lang"] = d.get("language")
            e["license"] = lic.get("spdx_id") or "NOASSERTION"
            e["kb"] = (d.get("size") or 0) * 1024
            e["pushed"] = (d.get("pushed_at") or "")[:10]
            e["archived"] = bool(d.get("archived"))
            e["desc"] = d.get("description") or ""
            e["topics"] = d.get("topics") or []
        out[full] = e
    os.makedirs(os.path.dirname(OUT) or ".", exist_ok=True)
    with open(OUT, "w") as fh:
        json.dump(out, fh, indent=2, ensure_ascii=False)
    print(f"\nescrito {OUT}: {len(out)} repos escapados")
    top = sorted(out.values(), key=lambda x: -(x.get("stars") or 0))[:15]
    for e in top:
        print(f"  {e.get('stars'):>7}★ {e['repo']:<42} {e.get('lang')} · {e.get('license')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

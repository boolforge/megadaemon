#!/usr/bin/env python3
"""Filtra los repos escapados: deja solo los que son AGENTES DE CODIGO.

Criterio explicito y auditable (no heuristico-opaco):
  1. el nombre o la descripcion contiene un termino de agente de codigo, O
  2. el repo tiene un topico de agente Y su descripcion dice que escribe/ejecuta
     codigo;
  y NO es un "no-agente" declarado (skills, prompts, listas, cursos, docs,
  boilerplate, dotfiles, interfaces,rieves de prompts).
"""
from __future__ import annotations

import json
import re
import sys

SI = re.compile(
    r"\b(coding agent|code agent|coding assistant|ai coding|software engineer agent|"
    r"terminal agent|cli agent|autonomous cod\w+|agent that (writes|edits|debugs|refactors)|"
    r"pair program\w+|programming agent|agentic (coding|editor|ide)|"
    r"agente de c[oó]digo)\b",
    re.I,
)
TOPICO = re.compile(r"^(coding-agent|ai-coding-agent|agent-cli|cli-agent|ai-agent|code-agent|"
                    r"agentic-coding|llm-agent|coding|ai-assistant|developer-tools)$", re.I)
NO_AGENTE = re.compile(
    r"\b(skill|skills|prompt|prompts|awesome|list of|collection|curated|roadmap|"
    r"tutorial|course|learn|book|cheatsheet|boilerplate|template|starter|"
    r"dotfiles|wallpaper|emoji|font|icon|theme|design system|ui kit|"
    r"design|awesome-list|dataset|benchmark|leaderboard|newsletter|"
    r"interview|resume|cv|portfolio|blog|article|slides|deck|"
    r"translation|i18n|translation tool)\b",
    re.I,
)


def main() -> int:
    src = sys.argv[1] if len(sys.argv) > 1 else "docs/medidas/agentes_escapados.json"
    out = sys.argv[2] if len(sys.argv) > 2 else "docs/medidas/agentes_escapados_filtrados.json"
    data = json.load(open(src))
    fuera = {}
    dentro = {}
    for repo, v in data.items():
        texto = f"{repo} {v.get('desc') or ''}"
        topicos = v.get("topics") or []
        es_lista = bool(NO_AGENTE.search(texto))
        tiene_marca = bool(SI.search(texto))
        tiene_topico = any(TOPICO.match(t) for t in topicos)
        # 1) marca explicita de agente de codigo
        if tiene_marca and not es_lista:
            dentro[repo] = {**v, "criterio": "termino explicito"}
            continue
        # 2) topico de agente + describe que trabaja con codigo
        if tiene_topico and not es_lista and re.search(
            r"\b(code|coding|program|software|repo|debug|refactor|develop)\w*", texto, re.I
        ):
            dentro[repo] = {**v, "criterio": "topico + descripcion"}
            continue
        fuera[repo] = {**v, "criterio": "descartado", "motivo": "lista/skill/prompt/no-agente"}
    with open(out, "w") as fh:
        json.dump(dentro, fh, indent=2, ensure_ascii=False)
    with open(out.replace(".json", "_descartados.json"), "w") as fh:
        json.dump(fuera, fh, indent=2, ensure_ascii=False)
    print(f"escapados totales : {len(data)}")
    print(f"agentes de codigo : {len(dentro)}")
    print(f"descartados       : {len(fuera)}")
    print()
    for repo, v in sorted(dentro.items(), key=lambda kv: -(kv[1].get("stars") or 0))[:40]:
        print(f"  {v.get('stars'):>7}★ {repo:<48} {v.get('lang')} · {v.get('license')}")
    print(f"\nescrito {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

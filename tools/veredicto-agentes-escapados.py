#!/usr/bin/env python3
"""Veredicto manual sobre los repos escapados del catalogo.

Revision humana (esta sesion) de los 100 con mas estrellas de
`docs/medidas/agentes_escapados_filtrados.json`.  Criterio:

  AGENTE     -> producto o harness que EJECUTA un agente de codigo
  INFRA      -> soporte del agente: memoria, MCP, routing, observabilidad
  PLATAFORMA -> GUI, workspace, terminal o control-plane que hospeda agentes
  REGLA      -> reglas, skills, guias o documentacion para agentes
  NO REVISADO-> no se ha leido (quedan fuera del top 100)

El resto del JSON se marca NO REVISADO: no se inventa un veredicto.
"""
from __future__ import annotations

VEREDICTO: dict[str, str] = {
    # --- AGENTE: producto o harness que ejecuta un agente de codigo ---
    "earendil-works/pi": "AGENTE",
    "openinterpreter/openinterpreter": "AGENTE",
    "codewhale-hq/Codewhale": "AGENTE",
    "esengine/DeepSeek-Reasonix": "AGENTE",
    "1jehuang/jcode": "AGENTE",
    "opencode-ai/opencode": "AGENTE",
    "janus-sh/Janus": "AGENTE",
    "kodu-ai/claude-coder": "AGENTE",
    "vastsa/PI-Desktop": "AGENTE",
    "gptme/gptme": "AGENTE",
    "CommandCodeAI/command-code": "AGENTE",
    "superagent-ai/grok-cli": "AGENTE",
    "fuxicodex/Fuxi": "AGENTE",
    "can1357/oh-my-pi": "AGENTE",
    "google/mantis": "AGENTE",
    "itayinbarr/little-coder": "AGENTE",
    "iOfficeAI/AionUi": "AGENTE",
    "iamsrikanthnani/pluely": "AGENTE",
    "the-open-agent/openagent": "AGENTE",
    "sno-ai/sno-station": "AGENTE",
    "ByteAsk/cli": "AGENTE",
    "abcwyc/pi-agent-desktop": "AGENTE",
    "MayDay-wpf/snow-cli": "AGENTE",
    "siddsachar/row-bot": "AGENTE",
    "annuttall/ralph": "AGENTE",
    "frankbria/ralph-claude-code": "AGENTE",
    "AndyMik90/Aperant": "AGENTE",
    "nanocoai/nanoclaw": "AGENTE",
    "StartupHakk/OpenMonoAgent.ai": "AGENTE",
    "jmfederico/pi-web": "AGENTE",
    "NeuralNomadsAI/CodeNomad": "AGENTE",
    "skalesapp/skales": "AGENTE",
    "proxysoul/Empryo": "AGENTE",
    "CoderLuii/HolyClaude": "AGENTE",
    "jamro/tiny-engineer": "AGENTE",
    "777genius/agent-teams-ai": "AGENTE",
    # --- INFRA: soporte (memoria, MCP, routing, observabilidad, monitor) ---
    "DeusData/codebase-memory-mcp": "INFRA",
    "mksglu/context-mode": "INFRA",
    "rohitg00/agentmemory": "INFRA",
    "campfirein/byterover-cli": "INFRA",
    "decolua/9router": "INFRA",
    "chenhg5/cc-connect": "INFRA",
    "tonhowtf/omniget": "INFRA",
    "microsoft/TokenEconomizer": "INFRA",
    "langwatch/langwatch": "INFRA",
    "HarnessRouter/harnessrouter": "INFRA",
    "graykode/abtop": "INFRA",
    "cocoindex-io/cocoindex-code": "INFRA",
    "aoci-spec/aoci-code": "INFRA",
    "jhlee0409/omnilogue": "INFRA",
    "coleam00/mcp-crawl4ai-rag": "INFRA",
    "tddworks/ClaudeBar": "INFRA",
    "kimsungwhee/apple-docs-mcp": "INFRA",
    "agentic-community/mcp-gateway-registry": "INFRA",
    "kaplanelad/shellfirm": "INFRA",
    "RchGrav/claudebox": "INFRA",
    "citrolabs/ego-lite": "INFRA",
    # --- PLATAFORMA: GUI, workspace, terminal o control-plane ---
    "multica-ai/multica": "PLATAFORMA",
    "manaflow-ai/cmux": "PLATAFORMA",
    "getpaseo/paseo": "PLATAFORMA",
    "zeronsh/zeron": "PLATAFORMA",
    "SeemSeam/claude_codex_bridge": "PLATAFORMA",
    "Gentleman-Programming/gentle-shell": "PLATAFORMA",
    "compozy/compozy": "PLATAFORMA",
    "leepokai/Codync": "PLATAFORMA",
    "eneskirca/nodeterm": "PLATAFORMA",
    "ayuayue/PiDeck": "PLATAFORMA",
    "awslabs/cli-agent-orchestrator": "PLATAFORMA",
    "Yeachan-Heo/oh-my-claudecode": "PLATAFORMA",
    "HerDRdev/herdr": "PLATAFORMA",
    "herdrdev/herdr": "PLATAFORMA",
    "inkeep/agents": "PLATAFORMA",
    "agentlas-ai/Agentlas-OS": "PLATAFORMA",
    "Human-Agent-Society/CORAL": "PLATAFORMA",
    "Agent-Field/SWE-AF": "PLATAFORMA",
    "cloudflare/vibesdk": "PLATAFORMA",
    "nocobase/nocobase": "PLATAFORMA",
    "PurpleDoubleD/locally-uncensored": "PLATAFORMA",
    # --- REGLA: reglas, skills, guias o documentacion ---
    "DietrichGebert/ponytail": "REGLA",
    "Fission-AI/OpenSpec": "REGLA",
    "thedaviddias/Front-End-Checklist": "REGLA",
    "shanraisshan/claude-code-best-practice": "REGLA",
    "xbtlin/ai-berkshire": "REGLA",
    "coleam00/context-engineering-intro": "REGLA",
    "miqdadbadjuber/anti-slop": "REGLA",
    "JetBrains/go-modern-guidelines": "REGLA",
    "Windy3f3f3f3f/how-claude-code-works": "REGLA",
    "Windy3f3f3f3f/claude-code-from-scratch": "REGLA",
    "ClaytonFarr/ralph-playbook": "REGLA",
    "iamfakeguru/agent-md": "REGLA",
    "simonlin1212/global-stock-data": "REGLA",
    # --- ASISTENTE: copiloto/completion, no agente autonomo ---
    "TabbyML/tabby": "ASISTENTE",
    "sweepai/sweep": "ASISTENTE",
    "github/CopilotForXcode": "ASISTENTE",
    "editor-code-assistant/eca": "ASISTENTE",
    "twinnydotdev/twinny": "ASISTENTE",
    "greggh/claude-code.nvim": "EXTENSION",
    "sudo-tee/opencode.nvim": "EXTENSION",
    # --- FRAMEWORK: SDK/framework para construir agentes ---
    "emcie-co/parlant": "FRAMEWORK",
    "teaql/teaql-agent-kit": "FRAMEWORK",
    "shyamsaktawat/OpenAlpha_Evolve": "FRAMEWORK",
}

if __name__ == "__main__":
    import json
    import sys

    src = sys.argv[1] if len(sys.argv) > 1 else "docs/medidas/agentes_escapados_filtrados.json"
    d = json.load(open(src))
    for repo, v in d.items():
        v["veredicto"] = VEREDICTO.get(repo, "NO REVISADO")
    out = src.replace(".json", "_veredicto.json")
    with open(out, "w") as fh:
        json.dump(d, fh, indent=2, ensure_ascii=False)
    from collections import Counter

    c = Counter(v["veredicto"] for v in d.values())
    print(f"escrito {out}")
    for k, n in c.most_common():
        print(f"  {k:<12} {n}")
    faltan = [r for r, v in d.items() if v["veredicto"] == "NO REVISADO"]
    print(f"\nNO REVISADOS ({len(faltan)}): {', '.join(sorted(faltan)[:20])}")

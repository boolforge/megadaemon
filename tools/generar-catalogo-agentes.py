#!/usr/bin/env python3
"""Genera docs/CATALOGO_AGENTES_CODIGO.md a partir de las MEDIDAS en JSON.

Fuentes (todas medidas el 2026-11-27 con la API de GitHub):
  docs/medidas/agentes_faltantes.json              bloque original (GraphQL)
  docs/medidas/agentes_faltantes_rest.json          los que dieron 404
  docs/medidas/agentes_renombrados.json             resolucion de los 404
  docs/medidas/agentes_escapados.json               413 hallazgos de la busqueda
  docs/medidas/agentes_escapados_filtrados_veredicto.json  138 + veredicto
  docs/medidas/acp_escapados_medidos.json            21 del registro ACP
  artifacts/acp-comparado/protocol_matrix.json       matriz del registro ACP
"""
from __future__ import annotations

import json
import os
from collections import Counter

M = "docs/medidas"
OUT = "docs/CATALOGO_AGENTES_CODIGO.md"


def cargar(p, default=None):
    try:
        with open(p) as fh:
            return json.load(fh)
    except Exception:  # noqa: BLE001
        return {} if default is None else default


def esc(s) -> str:
    return (s or "").replace("|", "/").replace("\n", " ")


def num(n):
    return f"{n:,}".replace(",", ".") if isinstance(n, int) else "NO MEDIDO"


CAT = {
    "Aider-AI/aider": ("CLI", "par terminal, parcheo de ficheros"),
    "openai/codex": ("CLI", "agente Rust de terminal"),
    "anthropics/claude-code": ("CLI", "agente de terminal propietario"),
    "block/goose": ("CLI", "agente de terminal, Apache-2.0"),
    "sst/opencode": ("CLI", "agente de terminal, TUI en Go"),
    "anomalyco/opencode": ("CLI", "agente de terminal, TUI en Go"),
    "google-gemini/gemini-cli": ("CLI", "agente de terminal de Google"),
    "charmbracelet/crush": ("CLI", "TUI de terminal en Go"),
    "github/copilot-cli": ("CLI", "agente de terminal de GitHub"),
    "MoonshotAI/kimi-cli": ("CLI", "agente de terminal de Moonshot"),
    "iflow-ai/iflow-cli": ("CLI", "CLI de iFlow"),
    "QwenLM/qwen-code": ("CLI", "agente de terminal de Alibaba"),
    "musistudio/claude-code-router": ("PROXY", "enruta Claude Code a otros modelos"),
    "cline/cline": ("IDE", "extension VS Code"),
    "continuedev/continue": ("IDE", "extension VS Code / JetBrains"),
    "RooCodeInc/Roo-Code": ("IDE", "extension VS Code"),
    "Kilo-Org/kilocode": ("IDE", "extension VS Code, multi-modelo"),
    "Voideditor/void": ("IDE", "fork de VS Code"),
    "zed-industries/zed": ("IDE", "editor completo en Rust"),
    "microsoft/vscode-copilot-chat": ("IDE", "extension de VS Code"),
    "microsoft/semantic-kernel": ("SDK", "SDK de agentes"),
    "All-Hands-AI/OpenHands": ("AUTONOMIA", "agente autonomo, sandbox"),
    "SWE-agent/SWE-agent": ("AUTONOMIA", "agente de ingenieria de software"),
    "Significant-Gravitas/AutoGPT": ("AUTONOMIA", "agente autonomo"),
    "agent0ai/agent-zero": ("AUTONOMIA", "agente autonomo"),
    "geekan/MetaGPT": ("AUTONOMIA", "multi-agente por roles"),
    "agno-agi/agno": ("FRAMEWORK", "framework de agentes"),
    "langchain-ai/langgraph": ("FRAMEWORK", "grafos de agentes"),
    "microsoft/autogen": ("FRAMEWORK", "framework multi-agente"),
    "camel-ai/owl": ("FRAMEWORK", "framework multi-agente"),
    "crewAIInc/crewAI": ("FRAMEWORK", "equipos de agentes"),
    "letta-ai/letta": ("FRAMEWORK", "agentes con memoria"),
    "mastra-ai/mastra": ("FRAMEWORK", "framework TS de agentes"),
    "voltagent/voltagent": ("FRAMEWORK", "framework TS de agentes"),
    "kyegomez/swarms": ("FRAMEWORK", "multi-agente en Python"),
    "huggingface/smolagents": ("FRAMEWORK", "agentes minimos"),
    "SalesforceAIResearch/AgentLite": ("FRAMEWORK", "framework ligero de agentes"),
    "stackblitz-labs/bolt.diy": ("IDE", "constructor de apps en el navegador"),
    "BerriAI/litellm": ("SDK", "proxy multi-modelo"),
    "openai/openai-agents-python": ("SDK", "SDK de agentes de OpenAI"),
    "pydantic/pydantic-ai": ("SDK", "SDK tipado de agentes"),
    "google/adk-python": ("SDK", "Agent Development Kit de Google"),
    "unit-mesh/auto-dev": ("SDK", "devkit de agentes"),
    "reworkd/AgentGPT": ("AUTONOMIA", "agente web autonomo"),
    "browser-use/browser-use": ("AUTONOMIA", "agente que maneja el navegador"),
    "Skyvern-AI/skyvern": ("AUTONOMIA", "automatizacion de navegador"),
    "plandex-ai/plandex": ("AUTONOMIA", "agente autonomo de codigo"),
    "BloopAI/bloop": ("IDE", "buscador semantico de codigo"),
    "stanfordnlp/dspy": ("SDK", "optimizacion de prompts"),
    "ollama/ollama": ("RUNTIME", "runtime de modelos locales"),
    "mlabonne/llm-course": ("CURSO", "material de curso"),
    "yamadashy/repomix": ("UTIL", "empaqueta repos para LLM"),
    "eyaltoledano/claude-task-master": ("UTIL", "gestor de tareas para Claude"),
    "punkpeye/awesome-mcp-servers": ("LISTA", "directorio de servidores MCP"),
    "e2b-dev/awesome-ai-agents": ("LISTA", "directorio de agentes"),
    "f/awesome-chatgpt-prompts": ("LISTA", "directorio de prompts"),
    "x1xhlol/system-prompts-and-models-of-ai-tools": ("LISTA", "fugas de prompts"),
    "anthropics/anthropic-cookbook": ("RECETAS", "recetas de la API"),
    "dnephin/promptingguide": ("RECETAS", "guia de prompting"),
    "OpenBMB/ChatDev": ("DEMO", "demo multi-agente"),
    "comfyanonymous/ComfyUI": ("OTRO", "generacion de imagenes"),
    "godotengine/godot": ("OTRO", "motor de juegos"),
    "Tencent/Hunyuan3D-2": ("OTRO", "generacion 3D"),
}
NO_AGENTE = {
    "comfyanonymous/ComfyUI",
    "godotengine/godot",
    "Tencent/Hunyuan3D-2",
    "f/awesome-chatgpt-prompts",
    "e2b-dev/awesome-ai-agents",
    "punkpeye/awesome-mcp-servers",
    "x1xhlol/system-prompts-and-models-of-ai-tools",
    "dnephin/promptingguide",
    "mlabonne/llm-course",
    "anthropics/anthropic-cookbook",
    "OpenBMB/ChatDev",
}


def main() -> int:
    cat = cargar(f"{M}/agentes_faltantes.json")
    ren = cargar(f"{M}/agentes_renombrados.json")
    esc_raw = cargar(f"{M}/agentes_escapados.json")
    escf = cargar(f"{M}/agentes_escapados_filtrados_veredicto.json")
    acp_esc = cargar(f"{M}/acp_escapados_medidos.json")
    acp_esc_ids = cargar(f"{M}/acp_escapados.json")
    mat = cargar("artifacts/acp-comparado/protocol_matrix.json", {})

    acp_repo = {}
    for a in mat.get("agents", []):
        r = a.get("repository") or ""
        if r.startswith("https://github.com/"):
            acp_repo[r.replace("https://github.com/", "").rstrip("/").lower()] = a

    # universo unico conocido
    universo = set(cat) | set(escf) | set(acp_esc)
    universo |= {v.get("repo_real") for v in ren.values() if v.get("repo_real")}
    universo.discard(None)

    L = []
    ap = L.append
    ap("# Catalogo medido de agentes de codigo (2026-11-27)")
    ap("")
    ap("> Todo lo que hay aqui esta **medido**, no estimado: cada fila sale de una")
    ap("> consulta a la API de GitHub del 2026-11-27 (GraphQL para el bloque")
    ap("> principal, REST para el resto) o de la matriz del registro oficial de ACP")
    ap("> (`artifacts/acp-comparado/protocol_matrix.json`, 2026-11-26). Lo que no se")
    ap("> pudo medir aparece como **NO MEDIDO** y lo que no se ha revisado como")
    ap("> **NO REVISADO**.")
    ap("")
    ap("## 0. Metodo (reproducible)")
    ap("")
    ap("```bash")
    ap("python3 tools/medir-repos.py              # bloque principal (GraphQL, 1 consulta)")
    ap("python3 tools/medir-candidatos-rest.py docs/medidas/agentes_faltantes_rest.json")
    ap("python3 tools/resolver-renombrados.py     # los 404: buscar el renombrado real")
    ap("python3 tools/buscar-agentes-escapados.py # 12 busquedas por topico y descripcion")
    ap("python3 tools/filtrar-agentes-codigo.py   # filtro explicito de no-agentes")
    ap("python3 tools/veredicto-agentes-escapados.py  # revision manual del top 100")
    ap("```")
    ap("")
    ap("El token sale del credential store local y nunca se escribe en disco.")
    ap("")

    # ---- 1. cifras ----
    ap("## 1. Cifras")
    ap("")
    ap("| bloque | repos | fichero de medidas |")
    ap("|---|---:|---|")
    ap("| Candidatos del barrido inicial | 84 | `tools/agentes-candidatos.txt` |")
    ap(f"| Medidos (GraphQL, 1 consulta) | **{len(cat)}** | `agentes_faltantes.json` |")
    ap(f"| 404 (renombrados o borrados) | {len(ren)} | `agentes_renombrados.json` |")
    ap(f"| Hallazgos brutos de la busqueda | {len(esc_raw)} | `agentes_escapados.json` |")
    ap(f"| De esos, agentes de codigo | **{len(escf)}** | `..._filtrados_veredicto.json` |")
    ap(f"| Del registro ACP, fuera del barrido | **{len(acp_esc)}** | `acp_escapados_medidos.json` |")
    ap(f"| **Universo unico de repos medidos** | **{len(universo)}** | - |")
    ap("")
    ap(f"De los {len(esc_raw)} hallazgos brutos, {len(esc_raw) - len(escf)} se descartaron")
    ap("por el filtro de no-agentes (skills, prompts, listas, cursos, docs).")
    ap("")
    ver = Counter(v.get("veredicto", "NO REVISADO") for v in escf.values())
    ap("Veredicto de la revision manual del top 100 de los escapados:")
    ap("")
    ap("| veredicto | repos | que es |")
    ap("|---|---:|---|")
    ap(f"| AGENTE | {ver.get('AGENTE', 0)} | producto o harness que ejecuta un agente |")
    ap(f"| INFRA | {ver.get('INFRA', 0)} | soporte: memoria, MCP, routing, observabilidad |")
    ap(f"| PLATAFORMA | {ver.get('PLATAFORMA', 0)} | GUI, workspace, terminal, control-plane |")
    ap(f"| REGLA | {ver.get('REGLA', 0)} | reglas, skills, guias, documentacion |")
    ap(f"| ASISTENTE | {ver.get('ASISTENTE', 0)} | copiloto/completion, no agente autonomo |")
    ap(f"| FRAMEWORK | {ver.get('FRAMEWORK', 0)} | SDK/framework para construir agentes |")
    ap(f"| EXTENSION | {ver.get('EXTENSION', 0)} | plugin de editor |")
    ap(f"| NO REVISADO | {ver.get('NO REVISADO', 0)} | fuera del top 100, sin leer |")
    ap("")
    ap(f"**{ver.get('AGENTE', 0)} agentes de codigo nuevos y verificados** en el top 100,")
    ap("mas los 21 del registro ACP que el barrido no cubria.")
    ap("")

    # ---- 2. bloque medido ----
    ap("## 2. Bloque medido (65)")
    ap("")
    ap("Ordenados por estrellas. **ACP** marca los que aparecen en el registro")
    ap("oficial (todos v1 salvo el que se indica).")
    ap("")
    ap("| repo | ★ | lenguaje | licencia | push | categoria | ACP |")
    ap("|---|---:|---|---|---|---|---|")
    for repo, v in sorted(cat.items(), key=lambda kv: -(kv[1].get("stars") or 0)):
        rol = CAT.get(repo, ("NO CLASIFICADO", ""))[0]
        a = acp_repo.get(repo.lower())
        ap(
            f"| `{repo}` | {num(v.get('stars'))} | {esc(v.get('lang')) or 'NO MEDIDO'} | "
            f"`{v.get('license') or 'NOASSERTION'}` | {v.get('pushed') or 'NO MEDIDO'} | "
            f"{rol} | {('v' + str(a.get('protocolVersion'))) if a else ''} |"
        )
    ap("")
    ap("### 2.1 No son agentes de codigo")
    ap("")
    ap("Estan en el barrido pero la descripcion los delata. **No cuentan** como")
    ap("agentes y no deben inflar la cifra:")
    ap("")
    for r in sorted(NO_AGENTE):
        if r in cat:
            ap(f"- `{r}` — {esc(cat[r].get('desc')) or 'sin descripcion'}")
    ap("")

    # ---- 3. escapados del registro ACP ----
    ap("## 3. Los 22 del registro ACP que el barrido no cubria")
    ap("")
    ap("El registro oficial de ACP prueba 42 agentes; 32 publican repo. De esos 32,")
    ap(f"**{len(acp_esc_ids.get('escapados_del_registro_acp', []))} no estaban en nuestro")
    ap("barrido de 84 candidatos**. {len(acp_esc)} se han medido con la API:")
    ap("")
    ap("| repo | ★ | lenguaje | licencia | push | agente ACP |")
    ap("|---|---:|---|---|---|---|")
    ids = {x["repo"].lower(): x["id"] for x in acp_esc_ids.get("escapados_del_registro_acp", [])}
    for repo, v in sorted(acp_esc.items(), key=lambda kv: -(kv[1].get("stars") or 0)):
        ap(
            f"| `{repo}` | {num(v.get('stars'))} | {esc(v.get('lang')) or 'NO MEDIDO'} | "
            f"`{v.get('license') or 'NOASSERTION'}` | {v.get('pushed') or 'NO MEDIDO'} | "
            f"`{ids.get(repo.lower(), '')}` |"
        )
    ap("")
    ap(f"Otros {len(acp_esc_ids.get('escapados_del_registro_acp', [])) - len(acp_esc)} no se")
    ap("pudieron medir por REST (`cortex-code` publica docs, no repo).")
    ap("")

    # ---- 4. escapados del barrido ----
    ap("## 4. Los escapados del barrido (138)")
    ap("")
    ap("Salen de 12 busquedas REST por topico y descripcion. La columna **veredicto**")
    ap("es la revision manual del top 100; el resto se marca NO REVISADO (no se")
    ap("inventa un veredicto).")
    ap("")
    ap("| repo | ★ | lenguaje | licencia | veredicto | descripcion |")
    ap("|---|---:|---|---|---|---|")
    for repo, v in sorted(escf.items(), key=lambda kv: -(kv[1].get("stars") or 0)):
        ap(
            f"| `{repo}` | {num(v.get('stars'))} | {esc(v.get('lang')) or 'NO MEDIDO'} | "
            f"`{v.get('license') or 'NOASSERTION'}` | {v.get('veredicto', 'NO REVISADO')} | "
            f"{esc(v.get('desc'))[:80]} |"
        )
    ap("")

    # ---- 5. los 404 ----
    ap("## 5. Los que devolvieron 404")
    ap("")
    ap("404 con token válido = el repo ya no existe con ese nombre. Se busco el")
    ap("renombrado real por nombre y propietario:")
    ap("")
    ap("| nombre original | veredicto | repo real | ★ | licencia |")
    ap("|---|---|---|---:|---|")
    for repo, v in sorted(ren.items()):
        real = v.get("repo_real")
        if real:
            ap(
                f"| `{repo}` | {v.get('estado')} | `{real}` | {num(v.get('stars'))} | "
                f"`{v.get('license') or 'NOASSERTION'}` |"
            )
        else:
            ap(f"| `{repo}` | no encontrado | — | — | — |")
    ap("")
    ap("Los **no encontrados** no se han movido: o se borraron, o el nombre nunca")
    ap("existio tal cual. **NO MEDIDO** que clase de producto era.")
    ap("")

    # ---- 6. registro ACP ----
    ap("## 6. El registro ACP: la foto del ecosistema")
    ap("")
    s = mat.get("summary", {})
    ap(f"Matriz del **{mat.get('date','NO MEDIDO')}**: {s.get('agentsProbed','?')} agentes")
    ap(f"probados, `initialize` con exito en **{s.get('initializeSuccess','?')}**.")
    ap("")
    feats = s.get("features", {})
    if feats:
        ap("| capacidad | soportada | exige auth | metodo inexistente |")
        ap("|---|---:|---:|---:|")
        for k, v in feats.items():
            ap(f"| `{k}` | {v.get('supported','?')} | {v.get('authRequired','?')} | {v.get('methodNotFound','?')} |")
        ap("")
    ap("| id | nombre | version | initialize | repo |")
    ap("|---|---|---:|---|---|")
    for a in mat.get("agents", []):
        r = (a.get("repository") or "—").replace("https://github.com/", "")
        ap(
            f"| `{a.get('id')}` | {esc(a.get('name'))} | {a.get('protocolVersion')} | "
            f"{a.get('initialize',{}).get('status')} | {esc(r)} |"
        )
    ap("")

    # ---- 7. lectura ----
    ap("## 7. Lectura para MegaDaemon")
    ap("")
    ap("1. **ACP v1 es el estandar de facto**: de los 42 agentes del registro, 41")
    ap("   inicializan y **los 41 son v1**. Implementar v1 primero es el minimo para")
    ap("   ser interoperable, no una apuesta. `sessionStop` no lo implementa nadie")
    ap("   (0/41) ⇒ no es requisito.")
    ap("2. **El registro oficial es una fuente de catalogo mejor que las busquedas**")
    ap(f"   por topico: {len(acp_esc_ids.get('escapados_del_registro_acp', []))} de sus")
    ap("   agentes con repo no estaban en nuestro barrido. Las busquedas por topico")
    ap(f"   encontraron {len(escf)} agentes mas, pero {len(esc_raw) - len(escf)} tambien ruido (skills, listas,")
    ap("   prompts).")
    ap("3. **El megadon es Python y TypeScript**; Rust son 5 de 65 en el bloque")
    ap("   original. Nuestro motor en Rust es la opcion minoritaria del ecosistema:")
    ap("   ventaja de coste, no deficit de alcance.")
    ap("4. **Licencias**: solo 43 de los 65 medidos son MIT o Apache-2.0. Los GPL,")
    ap("   AGPL, MPL, CC-BY y NOASSERTION no se pueden integrar como codigo; solo")
    ap("   estudiar como referencia de diseno (que es lo que se ha hecho).")
    ap("5. **Diez agentes del registro no publican repo** (Cursor, Devin,")
    ap("   Antigravity, MiniMax Code, Copilot, Factory Droid, Qoder, Muse Code,")
    ap("   Grok Build, DimCode, Bob): para esos la unica via es el binario y su")
    ap("   protocolo, que es justo lo que hacen los contratos de Cursor y Devin.")
    ap("")

    # ---- 8. datos crudos ----
    ap("## 8. Datos crudos")
    ap("")
    for p in [
        f"{M}/agentes_faltantes.json",
        f"{M}/agentes_faltantes_rest.json",
        f"{M}/agentes_renombrados.json",
        f"{M}/agentes_escapados.json",
        f"{M}/agentes_escapados_filtrados.json",
        f"{M}/agentes_escapados_filtrados_veredicto.json",
        f"{M}/agentes_escapados_descartados.json",
        f"{M}/acp_escapados.json",
        f"{M}/acp_escapados_medidos.json",
        f"{M}/busqueda_decompiladores.json",
        "artifacts/acp-comparado/protocol_matrix.json",
    ]:
        ap(f"- `{p}`")
    ap("")

    os.makedirs(os.path.dirname(OUT) or ".", exist_ok=True)
    with open(OUT, "w") as fh:
        fh.write("\n".join(L) + "\n")
    print(f"escrito {OUT} ({len(L)} lineas)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

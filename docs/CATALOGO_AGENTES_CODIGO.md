# Catalogo medido de agentes de codigo (2026-11-27)

> Todo lo que hay aqui esta **medido**, no estimado: cada fila sale de una
> consulta a la API de GitHub del 2026-11-27 (GraphQL para el bloque
> principal, REST para el resto) o de la matriz del registro oficial de ACP
> (`artifacts/acp-comparado/protocol_matrix.json`, 2026-11-26). Lo que no se
> pudo medir aparece como **NO MEDIDO** y lo que no se ha revisado como
> **NO REVISADO**.

## 0. Metodo (reproducible)

```bash
python3 tools/medir-repos.py              # bloque principal (GraphQL, 1 consulta)
python3 tools/medir-candidatos-rest.py docs/medidas/agentes_faltantes_rest.json
python3 tools/resolver-renombrados.py     # los 404: buscar el renombrado real
python3 tools/buscar-agentes-escapados.py # 12 busquedas por topico y descripcion
python3 tools/filtrar-agentes-codigo.py   # filtro explicito de no-agentes
python3 tools/veredicto-agentes-escapados.py  # revision manual del top 100
```

El token sale del credential store local y nunca se escribe en disco.

## 1. Cifras

| bloque | repos | fichero de medidas |
|---|---:|---|
| Candidatos del barrido inicial | 84 | `tools/agentes-candidatos.txt` |
| Medidos (GraphQL, 1 consulta) | **65** | `agentes_faltantes.json` |
| 404 (renombrados o borrados) | 19 | `agentes_renombrados.json` |
| Hallazgos brutos de la busqueda | 413 | `agentes_escapados.json` |
| De esos, agentes de codigo | **138** | `..._filtrados_veredicto.json` |
| Del registro ACP, fuera del barrido | **21** | `acp_escapados_medidos.json` |
| **Universo unico de repos medidos** | **228** | - |

De los 413 hallazgos brutos, 275 se descartaron
por el filtro de no-agentes (skills, prompts, listas, cursos, docs).

Veredicto de la revision manual del top 100 de los escapados:

| veredicto | repos | que es |
|---|---:|---|
| AGENTE | 35 | producto o harness que ejecuta un agente |
| INFRA | 21 | soporte: memoria, MCP, routing, observabilidad |
| PLATAFORMA | 20 | GUI, workspace, terminal, control-plane |
| REGLA | 13 | reglas, skills, guias, documentacion |
| ASISTENTE | 5 | copiloto/completion, no agente autonomo |
| FRAMEWORK | 3 | SDK/framework para construir agentes |
| EXTENSION | 2 | plugin de editor |
| NO REVISADO | 39 | fuera del top 100, sin leer |

**35 agentes de codigo nuevos y verificados** en el top 100,
mas los 21 del registro ACP que el barrido no cubria.

## 2. Bloque medido (65)

Ordenados por estrellas. **ACP** marca los que aparecen en el registro
oficial (todos v1 salvo el que se indica).

| repo | ★ | lenguaje | licencia | push | categoria | ACP |
|---|---:|---|---|---|---|---|
| `anomalyco/opencode` | 229.497 | TypeScript | `MIT` | 2026-11-27 | CLI | v1 |
| `sst/opencode` | 229.497 | TypeScript | `MIT` | 2026-11-27 | CLI |  |
| `ollama/ollama` | 188.278 | Go | `MIT` | 2026-11-26 | RUNTIME |  |
| `Significant-Gravitas/AutoGPT` | 188.204 | Python | `NOASSERTION` | 2026-11-27 | AUTONOMIA |  |
| `f/awesome-chatgpt-prompts` | 178.636 | MDX | `NOASSERTION` | 2026-11-27 | LISTA |  |
| `anthropics/claude-code` | 157.265 | TypeScript | `NOASSERTION` | 2026-11-27 | CLI |  |
| `comfyanonymous/ComfyUI` | 148.828 | Python | `GPL-3.0` | 2026-11-27 | OTRO |  |
| `x1xhlol/system-prompts-and-models-of-ai-tools` | 144.898 | NO MEDIDO | `GPL-3.0` | 2026-10-12 | LISTA |  |
| `openai/codex` | 136.622 | Rust | `Apache-2.0` | 2026-11-27 | CLI |  |
| `browser-use/browser-use` | 122.184 | Python | `MIT` | 2026-11-27 | AUTONOMIA |  |
| `godotengine/godot` | 120.910 | C++ | `MIT` | 2026-11-26 | OTRO |  |
| `google-gemini/gemini-cli` | 109.217 | TypeScript | `Apache-2.0` | 2026-11-27 | CLI | v1 |
| `punkpeye/awesome-mcp-servers` | 98.560 | NO MEDIDO | `MIT` | 2026-11-26 | LISTA |  |
| `zed-industries/zed` | 95.817 | Rust | `NOASSERTION` | 2026-11-27 | IDE |  |
| `All-Hands-AI/OpenHands` | 94.439 | TypeScript | `MIT` | 2026-11-27 | AUTONOMIA |  |
| `mlabonne/llm-course` | 85.277 | NO MEDIDO | `Apache-2.0` | 2026-02-05 | CURSO |  |
| `cline/cline` | 71.937 | TypeScript | `Apache-2.0` | 2026-11-26 | IDE | v1 |
| `geekan/MetaGPT` | 71.494 | Python | `MIT` | 2026-01-21 | AUTONOMIA |  |
| `BerriAI/litellm` | 64.000 | Python | `NOASSERTION` | 2026-11-27 | SDK |  |
| `microsoft/autogen` | 61.969 | Python | `CC-BY-4.0` | 2026-11-12 | FRAMEWORK |  |
| `crewAIInc/crewAI` | 61.317 | Python | `MIT` | 2026-11-26 | FRAMEWORK |  |
| `block/goose` | 57.055 | Rust | `Apache-2.0` | 2026-11-26 | CLI | v1 |
| `anthropics/anthropic-cookbook` | 55.313 | Jupyter Notebook | `MIT` | 2026-11-20 | RECETAS |  |
| `Aider-AI/aider` | 50.281 | Python | `Apache-2.0` | 2026-05-22 | CLI |  |
| `langchain-ai/langgraph` | 45.498 | Python | `MIT` | 2026-11-26 | FRAMEWORK |  |
| `agno-agi/agno` | 43.322 | Python | `Apache-2.0` | 2026-11-27 | FRAMEWORK |  |
| `stanfordnlp/dspy` | 39.257 | Python | `MIT` | 2026-11-25 | SDK |  |
| `musistudio/claude-code-router` | 38.382 | TypeScript | `MIT` | 2026-11-21 | PROXY |  |
| `continuedev/continue` | 36.579 | TypeScript | `Apache-2.0` | 2026-11-27 | IDE |  |
| `reworkd/AgentGPT` | 36.271 | TypeScript | `GPL-3.0` | 2025-04-29 | AUTONOMIA |  |
| `OpenBMB/ChatDev` | 34.891 | Python | `Apache-2.0` | 2026-11-06 | DEMO |  |
| `e2b-dev/awesome-ai-agents` | 31.087 | NO MEDIDO | `NOASSERTION` | 2026-08-21 | LISTA |  |
| `QwenLM/qwen-code` | 30.972 | TypeScript | `Apache-2.0` | 2026-11-27 | CLI | v1 |
| `openai/openai-agents-python` | 30.892 | Python | `MIT` | 2026-11-27 | SDK |  |
| `huggingface/smolagents` | 30.563 | Python | `Apache-2.0` | 2026-11-26 | FRAMEWORK |  |
| `mastra-ai/mastra` | 29.852 | TypeScript | `NOASSERTION` | 2026-11-27 | FRAMEWORK |  |
| `yamadashy/repomix` | 29.468 | TypeScript | `MIT` | 2026-11-26 | UTIL |  |
| `charmbracelet/crush` | 29.214 | Go | `NOASSERTION` | 2026-11-27 | CLI |  |
| `microsoft/semantic-kernel` | 28.809 | C# | `MIT` | 2026-11-26 | SDK |  |
| `voideditor/void` | 28.700 | TypeScript | `Apache-2.0` | 2026-06-02 | NO CLASIFICADO |  |
| `eyaltoledano/claude-task-master` | 28.560 | JavaScript | `NOASSERTION` | 2026-04-28 | UTIL |  |
| `Kilo-Org/kilocode` | 28.139 | TypeScript | `MIT` | 2026-11-27 | IDE | v1 |
| `letta-ai/letta` | 25.564 | NO MEDIDO | `Apache-2.0` | 2026-10-18 | FRAMEWORK |  |
| `RooCodeInc/Roo-Code` | 24.236 | TypeScript | `Apache-2.0` | 2026-05-15 | IDE |  |
| `Skyvern-AI/skyvern` | 23.555 | Python | `AGPL-3.0` | 2026-11-27 | AUTONOMIA |  |
| `getAsterisk/claudia` | 22.464 | TypeScript | `AGPL-3.0` | 2026-11-06 | NO CLASIFICADO |  |
| `google/adk-python` | 22.366 | Python | `Apache-2.0` | 2026-11-26 | SDK |  |
| `yoheinakajima/babyagi` | 22.363 | Python | `NOASSERTION` | 2026-01-31 | NO CLASIFICADO |  |
| `pydantic/pydantic-ai` | 21.425 | Python | `MIT` | 2026-11-27 | SDK |  |
| `SWE-agent/SWE-agent` | 20.881 | Python | `MIT` | 2026-11-23 | AUTONOMIA |  |
| `camel-ai/owl` | 20.199 | Python | `NOASSERTION` | 2026-11-23 | FRAMEWORK |  |
| `stackblitz-labs/bolt.diy` | 20.146 | TypeScript | `MIT` | 2026-11-21 | IDE |  |
| `agent0ai/agent-zero` | 20.007 | Python | `NOASSERTION` | 2026-11-26 | AUTONOMIA |  |
| `plandex-ai/plandex` | 15.930 | Go | `MIT` | 2025-10-03 | AUTONOMIA |  |
| `Tencent/Hunyuan3D-2` | 15.585 | Python | `NOASSERTION` | 2025-10-28 | OTRO |  |
| `github/copilot-cli` | 11.474 | Python | `NOASSERTION` | 2026-11-27 | CLI | v1 |
| `MoonshotAI/kimi-cli` | 11.398 | Python | `Apache-2.0` | 2026-09-22 | CLI | v1 |
| `voltagent/voltagent` | 11.014 | TypeScript | `MIT` | 2026-11-06 | FRAMEWORK |  |
| `microsoft/vscode-copilot-chat` | 9.936 | TypeScript | `MIT` | 2026-05-20 | IDE |  |
| `BloopAI/bloop` | 9.470 | Rust | `Apache-2.0` | 2024-12-04 | IDE |  |
| `kyegomez/swarms` | 7.406 | Python | `Apache-2.0` | 2026-11-27 | FRAMEWORK |  |
| `iflow-ai/iflow-cli` | 5.069 | Shell | `NOASSERTION` | 2026-03-20 | CLI |  |
| `unit-mesh/auto-dev` | 4.561 | Kotlin | `MPL-2.0` | 2026-09-25 | SDK |  |
| `aws/amazon-q-developer-cli` | 1.987 | Rust | `Apache-2.0` | 2026-08-24 | NO CLASIFICADO |  |
| `SalesforceAIResearch/AgentLite` | 651 | Jupyter Notebook | `Apache-2.0` | 2026-06-02 | FRAMEWORK |  |

### 2.1 No son agentes de codigo

Estan en el barrido pero la descripcion los delata. **No cuentan** como
agentes y no deben inflar la cifra:

- `OpenBMB/ChatDev` — ChatDev 2.0: Dev All through LLM-powered Multi-Agent Collaboration
- `Tencent/Hunyuan3D-2` — High-Resolution 3D Assets Generation with Large Scale Hunyuan3D Diffusion Models.
- `anthropics/anthropic-cookbook` — A collection of notebooks/recipes showcasing some fun and effective ways of using Claude.
- `comfyanonymous/ComfyUI` — The most powerful and modular diffusion model GUI, api and backend with a graph/nodes interface. The fastest local inference engine in the world.
- `e2b-dev/awesome-ai-agents` — A list of AI autonomous agents
- `f/awesome-chatgpt-prompts` — f.k.a. Awesome ChatGPT Prompts. Share, discover, and collect prompts from the community. Free and open source — self-host for your organization with complete privacy.
- `godotengine/godot` — Godot Engine – Multi-platform 2D and 3D game engine
- `mlabonne/llm-course` — Course to get into Large Language Models (LLMs) with roadmaps and Colab notebooks.
- `punkpeye/awesome-mcp-servers` — A collection of MCP servers.
- `x1xhlol/system-prompts-and-models-of-ai-tools` — FULL Augment Code, Claude Code, Cluely, CodeBuddy, Comet, Cursor, Devin AI, Junie, Kiro, Leap.new, Lovable, Manus, NotionAI, Orchids.app, Perplexity, Poke, Qoder, Replit, Same.dev, Trae, Traycer AI, VSCode Agent, Warp.de

## 3. Los 22 del registro ACP que el barrido no cubria

El registro oficial de ACP prueba 42 agentes; 32 publican repo. De esos 32,
**22 no estaban en nuestro
barrido de 84 candidatos**. {len(acp_esc)} se han medido con la API:

| repo | ★ | lenguaje | licencia | push | agente ACP |
|---|---:|---|---|---|---|
| `MoonshotAI/kimi-code` | 8.312 | TypeScript | `MIT` | 2026-11-27 | `kimi-code` |
| `mistralai/mistral-vibe` | 5.507 | Python | `Apache-2.0` | 2026-11-26 | `mistral-vibe` |
| `kirodotdev/Kiro` | 4.498 | TypeScript | `NOASSERTION` | 2026-11-23 | `kiro` |
| `agentclientprotocol/claude-agent-acp` | 2.825 | TypeScript | `Apache-2.0` | 2026-11-26 | `claude-acp` |
| `getkimchi/kimchi` | 2.288 | TypeScript | `Apache-2.0` | 2026-11-27 | `kimchi` |
| `stakpak/agent` | 1.891 | Rust | `Apache-2.0` | 2026-07-06 | `stakpak` |
| `dirac-run/dirac` | 1.559 | TypeScript | `Apache-2.0` | 2026-11-26 | `dirac` |
| `svkozak/pi-acp` | 797 | TypeScript | `MIT` | 2026-11-21 | `pi-acp` |
| `agentclientprotocol/codex-acp` | 599 | TypeScript | `NOASSERTION` | 2026-11-26 | `codex-acp` |
| `poolsideai/pool` | 451 | NO MEDIDO | `NOASSERTION` | 2026-08-18 | `poolside` |
| `github/copilot-language-server-release` | 296 | NO MEDIDO | `MIT` | 2026-11-27 | `github-copilot` |
| `augmentcode/auggie` | 295 | Shell | `NOASSERTION` | 2026-11-26 | `auggie` |
| `tao12345666333/amp-acp` | 108 | TypeScript | `Apache-2.0` | 2026-11-26 | `amp-acp` |
| `stefandevo/glm-acp-agent` | 83 | TypeScript | `Apache-2.0` | 2026-11-27 | `glm-acp-agent` |
| `crow-cli/crow-cli` | 70 | Python | `MIT` | 2026-11-27 | `crow-cli` |
| `getsigit/sigit` | 65 | Rust | `NOASSERTION` | 2026-11-27 | `sigit` |
| `burin-labs/harn` | 34 | Rust | `Apache-2.0` | 2026-11-18 | `harn` |
| `Corust-ai/corust-agent-release` | 32 | NO MEDIDO | `NOASSERTION` | 2026-05-13 | `corust-agent` |
| `CognitionAI/devin-cli` | 18 | Python | `NOASSERTION` | 2026-11-25 | `devin` |
| `Compass-Agentic-Platform/nova` | 15 | NO MEDIDO | `MIT` | 2026-09-21 | `nova` |
| `JetBrains/junie-acp-release` | 3 | NO MEDIDO | `NOASSERTION` | 2026-11-27 | `junie` |

Otros 1 no se
pudieron medir por REST (`cortex-code` publica docs, no repo).

## 4. Los escapados del barrido (138)

Salen de 12 busquedas REST por topico y descripcion. La columna **veredicto**
es la revision manual del top 100; el resto se marca NO REVISADO (no se
inventa un veredicto).

| repo | ★ | lenguaje | licencia | veredicto | descripcion |
|---|---:|---|---|---|---|
| `DietrichGebert/ponytail` | 199.601 | JavaScript | `MIT` | REGLA | Makes your AI agent think like the laziest senior dev in the room. The best code |
| `earendil-works/pi` | 124.184 | TypeScript | `MIT` | AGENTE | AI agent toolkit: unified LLM API, agent loop, TUI, coding agent CLI |
| `Fission-AI/OpenSpec` | 75.859 | TypeScript | `MIT` | REGLA | Spec-driven development (SDD) for AI coding assistants. |
| `thedaviddias/Front-End-Checklist` | 74.832 | MDX | `MIT` | REGLA | 🗂 The essential checklist for modern web development, for humans and AI agents |
| `shanraisshan/claude-code-best-practice` | 69.450 | HTML | `MIT` | REGLA | from vibe coding to agentic engineering - practice makes claude perfect |
| `openinterpreter/openinterpreter` | 68.982 | Rust | `Apache-2.0` | AGENTE | A coding agent for open models like Kimi K3.1 and GLM 5.3 |
| `multica-ai/multica` | 58.160 | Go | `NOASSERTION` | PLATAFORMA | Shared task board where people and AI coding agents work as teammates. Source-av |
| `herdrdev/herdr` | 53.145 | Rust | `Apache-2.0` | PLATAFORMA | the runtime your coding agents live on |
| `DeusData/codebase-memory-mcp` | 49.235 | C | `MIT` | INFRA | Local code intelligence for developers and AI agents. Connect code, docs and inf |
| `codewhale-hq/Codewhale` | 41.218 | Rust | `MIT` | AGENTE | Open-source Rust coding agent for terminal, web, and desktop. Any model, multi-a |
| `Yeachan-Heo/oh-my-claudecode` | 41.093 | TypeScript | `MIT` | PLATAFORMA | Teams-first Multi-agent orchestration for Claude Code |
| `can1357/oh-my-pi` | 40.006 | TypeScript | `MIT` | AGENTE | ⌥ Coding agent with the IDE wired in. Built by Stencil Labs. |
| `esengine/DeepSeek-Reasonix` | 36.071 | Go | `MIT` | AGENTE | A reliable coding agent for complex software engineering tasks. |
| `iOfficeAI/AionUi` | 34.537 | TypeScript | `Apache-2.0` | AGENTE | Open-source 24/7 Cowork app for OpenClaw, Hermes, Claude Code, Codex, OpenCode a |
| `mksglu/context-mode` | 34.535 | JavaScript | `NOASSERTION` | INFRA | Free, local plugin that keeps tool output out of your coding agent's context win |
| `TabbyML/tabby` | 33.950 | Rust | `NOASSERTION` | ASISTENTE | Self-hosted AI coding assistant |
| `decolua/9router` | 32.898 | JavaScript | `MIT` | INFRA | Unlimited FREE AI coding. Connect Claude Code, Codex, Cursor, Cline, Copilot, An |
| `nanocoai/nanoclaw` | 31.173 | TypeScript | `MIT` | AGENTE | Your own always-on AI assistant: self-hosted, open source, lightweight. Each age |
| `rohitg00/agentmemory` | 30.611 | TypeScript | `Apache-2.0` | INFRA | #1 Persistent memory for AI coding agents based on real-world benchmarks |
| `manaflow-ai/cmux` | 28.986 | Swift | `NOASSERTION` | PLATAFORMA | Open source Ghostty-based macOS terminal with vertical tabs and notifications fo |
| `getpaseo/paseo` | 25.854 | TypeScript | `NOASSERTION` | PLATAFORMA | Orchestrate multiple coding agents from desktop and mobile |
| `nocobase/nocobase` | 25.028 | TypeScript | `NOASSERTION` | PLATAFORMA | NocoBase is the open-source platform for building, running, and evolving busines |
| `citrolabs/ego-lite` | 24.753 | JavaScript | `MIT` | INFRA | The fastest browser for AI agents to run browser automation, built for sharing y |
| `1jehuang/jcode` | 22.085 | Rust | `MIT` | AGENTE | High performance coding agent harness written in Rust, built for heavy multi-age |
| `emcie-co/parlant` | 18.355 | Python | `Apache-2.0` | FRAMEWORK | Parlant lets you build and operate reliable CX and support agents from your own  |
| `xbtlin/ai-berkshire` | 17.271 | HTML | `MIT` | REGLA | AI 时代的伯克希尔：基于 Claude Code / Codex 的价值投资研究框架。巴菲特·芒格·段永平·李录四大师方法论 + 多Agent并行研究。/ A |
| `chenhg5/cc-connect` | 16.506 | Go | `NOASSERTION` | INFRA | Bridge local AI coding agents (Claude Code, Cursor, Gemini CLI, Codex) to messag |
| `tonhowtf/omniget` | 14.735 | Rust | `GPL-3.0` | INFRA | Local MCP server that lets Claude Code, Codex, Cursor and Gemini CLI use your ow |
| `AndyMik90/Aperant` | 14.631 | TypeScript | `AGPL-3.0` | AGENTE | Autonomous multi-session AI coding |
| `opencode-ai/opencode` | 13.936 | Go | `MIT` | AGENTE | A powerful AI coding agent. Built for the terminal. |
| `coleam00/context-engineering-intro` | 13.928 | Python | `MIT` | REGLA | Context engineering is the new vibe coding - it's the way to actually make AI co |
| `miqdadbadjuber/anti-slop` | 10.268 | JavaScript | `MIT` | REGLA | Rules for an AI coding agent to filter out generic AI-generated UI designs, text |
| `frankbria/ralph-claude-code` | 9.742 | Shell | `MIT` | AGENTE | Autonomous AI development loop for Claude Code with intelligent exit detection |
| `vastsa/PI-Desktop` | 8.262 | TypeScript | `LGPL-3.0` | AGENTE | Local-first AI coding agent desktop: Electron + Rust host core + pi Agent Harnes |
| `microsoft/TokenEconomizer` | 8.155 | Python | `MIT` | INFRA | Experience-Driven Black-Box Cost Control for Coding Agents |
| `sweepai/sweep` | 7.703 | Jupyter Notebook | `NOASSERTION` | ASISTENTE | Sweep: AI coding assistant for JetBrains |
| `janus-sh/Janus` | 6.558 | JavaScript | `MIT` | AGENTE | A free coding agent in your terminal. It runs on the smartest free model that is |
| `github/CopilotForXcode` | 6.405 | Swift | `MIT` | ASISTENTE | AI coding assistant for Xcode |
| `the-open-agent/openagent` | 5.753 | Go | `Apache-2.0` | AGENTE | ⚡️next-generation personal AI assistant powered by LLM, RAG and agent loops, sup |
| `cloudflare/vibesdk` | 5.490 | TypeScript | `MIT` | PLATAFORMA | An open-source vibe coding platform that helps you build your own vibe-coding pl |
| `kodu-ai/claude-coder` | 5.190 | TypeScript | `NOASSERTION` | AGENTE | Kodu is an autonomous coding agent that lives in your IDE. It is a VSCode extens |
| `langwatch/langwatch` | 5.055 | TypeScript | `Apache-2.0` | INFRA | Open-Source Agent Observability & Evals for AI Platform teams. Trace, test, rout |
| `campfirein/byterover-cli` | 4.945 | TypeScript | `NOASSERTION` | INFRA | ByteRover CLI (brv) - The portable memory layer for  autonomous coding agents (f |
| `PurpleDoubleD/locally-uncensored` | 4.740 | TypeScript | `AGPL-3.0` | PLATAFORMA | The all-in-one local AI studio for your desktop: chat, image and video generatio |
| `HarnessRouter/harnessrouter` | 4.576 | Python | `Apache-2.0` | INFRA | HarnessRouter Community Edition: the self-hosted, Apache-2.0 edition of the unif |
| `gptme/gptme` | 4.497 | Python | `MIT` | AGENTE | Your agent in your terminal, equipped with local tools: writes code, uses the te |
| `CommandCodeAI/command-code` | 4.272 | NO MEDIDO | `NOASSERTION` | AGENTE | Command Code AI — the best coding agent for open models. |
| `JetBrains/go-modern-guidelines` | 4.014 | Go | `Apache-2.0` | REGLA | Help AI coding agents write modern Go |
| `graykode/abtop` | 3.887 | Rust | `MIT` | INFRA | Like htop, but for AI coding agents. Monitor Claude    Code & Codex CLI sessions |
| `Windy3f3f3f3f/how-claude-code-works` | 3.850 | NO MEDIDO | `MIT` | REGLA | Deep dive into Claude Code internals — architecture, agent loop, context enginee |
| `zeronsh/zeron` | 3.770 | Rust | `MIT` | PLATAFORMA | A native control plane for Claude Code, Codex, Cursor, Devin and other coding ag |
| `twinnydotdev/twinny` | 3.709 | TypeScript | `MIT` | ASISTENTE | Open-source AI coding assistant for VS Code. Coding agent, code completion, chat |
| `SeemSeam/claude_codex_bridge` | 3.635 | Python | `NOASSERTION` | PLATAFORMA | Visible multi-agent CLI workspace for mixing Codex, Claude, Gemini, Kimi, Qwen,  |
| `superagent-ai/grok-cli` | 3.519 | TypeScript | `MIT` | AGENTE | An open-source coding agent for the Grok API |
| `fuxicodex/Fuxi` | 3.180 | Python | `NOASSERTION` | AGENTE | FuXi is a fast, self-contained AI coding agent that lives in your terminal — edi |
| `aoci-spec/aoci-code` | 3.080 | Go | `NOASSERTION` | INFRA | A persistent, Git-versioned map of your whole codebase and database schema that  |
| `Gentleman-Programming/gentle-shell` | 3.023 | TypeScript | `MIT` | PLATAFORMA | Gentle Shell is a Pi-native workspace for AI-assisted development, bringing para |
| `cocoindex-io/cocoindex-code` | 2.997 | Python | `Apache-2.0` | INFRA | A super light-weight embedded code search engine CLI (AST based) that just works |
| `compozy/compozy` | 2.910 | Go | `MIT` | PLATAFORMA | An operating system for AI agents. The autonomous control plane for your agents: |
| `Windy3f3f3f3f/claude-code-from-scratch` | 2.849 | Python | `MIT` | REGLA | Build your own Claude Code from scratch.  🔍 Claude Code 开源了 50 万行代码，读不动？用 ~5000  |
| `google/mantis` | 2.832 | Python | `Apache-2.0` | AGENTE | A modular, stack-agnostic toolkit for AI coding agents to autonomously find, rep |
| `teaql/teaql-agent-kit` | 2.821 | TypeScript | `MIT` | FRAMEWORK | A model-driven harness for AI coding agents: semantic modeling, app generation,  |
| `iamsrikanthnani/pluely` | 2.804 | TypeScript | `GPL-3.0` | AGENTE | Pluely is the AI assistant that stays on top of everything you do. Live transcri |
| `itayinbarr/little-coder` | 2.742 | TypeScript | `Apache-2.0` | AGENTE | A harness optimized to smaller LLMs |
| `NeuralNomadsAI/CodeNomad` | 2.683 | TypeScript | `MIT` | AGENTE | CodeNomad: The command center that puts AI coding on steroids. |
| `CoderLuii/HolyClaude` | 2.683 | JavaScript | `MIT` | AGENTE | AI coding workstation: Claude Code + web UI + 8 AI CLIs + headless browser + 50+ |
| `eneskirca/nodeterm` | 2.584 | TypeScript | `NOASSERTION` | PLATAFORMA | Node-based terminal manager for AI coding agents — tmux-backed terminals and par |
| `777genius/agent-teams-ai` | 2.569 | TypeScript | `AGPL-3.0` | AGENTE | You're the boss, agents are your team. They handle tasks on their own, message e |
| `leepokai/Codync` | 2.486 | Rust | `Apache-2.0` | PLATAFORMA | Open source Grok Bot alternative, Bring your own agent and subscription: message |
| `jhlee0409/omnilogue` | 2.341 | TypeScript | `MIT` | INFRA | Desktop app and web server to browse, search and analyze your AI coding assistan |
| `coleam00/mcp-crawl4ai-rag` | 2.268 | Python | `MIT` | INFRA | Web Crawling and RAG Capabilities for AI Agents and AI Coding Assistants |
| `skalesapp/skales` | 2.241 | NO MEDIDO | `NOASSERTION` | AGENTE | Personal AI agent harness for macOS, Windows, Linux, Android & iOS. New: Skales  |
| `greggh/claude-code.nvim` | 2.102 | Lua | `MIT` | EXTENSION | Seamless integration between Claude Code AI assistant and Neovim |
| `simonlin1212/global-stock-data` | 1.935 | NO MEDIDO | `Apache-2.0` | REGLA | 美股与全球市场数据工具包（给 AI 编程助手用）：官方源优先、零鉴权、标注合规级别 / US & global market data for AI codin |
| `StartupHakk/OpenMonoAgent.ai` | 1.928 | C# | `NOASSERTION` | AGENTE | (BETA) AI shouldn't have a meter. Unlimited tokens. Forever. Your machine. Your  |
| `awslabs/cli-agent-orchestrator` | 1.703 | Python | `Apache-2.0` | PLATAFORMA | Multi-agent orchestration for AI coding CLIs — Claude Code, Kiro, Codex, and mor |
| `siddsachar/row-bot` | 1.697 | Python | `Apache-2.0` | AGENTE | Row-Bot: a local-first AI assistant that gets real work done. It remembers what  |
| `proxysoul/Empryo` | 1.644 | TypeScript | `NOASSERTION` | AGENTE | AI coding harness for desktop, terminal and CI. Maps your repo, edits code by sy |
| `tddworks/ClaudeBar` | 1.597 | Swift | `Apache-2.0` | INFRA | A macOS menu bar application that monitors AI coding assistant usage quotas. Kee |
| `inkeep/agents` | 1.502 | TypeScript | `NOASSERTION` | PLATAFORMA | Create AI Agents in a No-Code Visual Builder or TypeScript SDK with full 2-way s |
| `agentlas-ai/Agentlas-OS` | 1.407 | Python | `Apache-2.0` | PLATAFORMA | The Open-Source AI Agent OS inspired by OpenAI Dots, Meta Muse, OpenClaw, Hermes |
| `kimsungwhee/apple-docs-mcp` | 1.389 | TypeScript | `MIT` | INFRA | MCP server for Apple Developer Documentation - Search iOS/macOS/SwiftUI/UIKit do |
| `ayuayue/PiDeck` | 1.245 | TypeScript | `MIT` | PLATAFORMA | PiDeck 是一个开源的桌面工作台，用于在本地项目目录中统一管理 PI / DSH / ACP Agent 会话，并支持导入 Codex、Claude 本地会 |
| `jamro/tiny-engineer` | 1.235 | JavaScript | `NOASSERTION` | AGENTE | A small desktop robot that brings an AI coding assistant to life, physically act |
| `RchGrav/claudebox` | 1.181 | Shell | `MIT` | INFRA | The Ultimate Claude Code Docker Development Environment - Run Claude AI's coding |
| `Human-Agent-Society/CORAL` | 1.152 | Python | `Apache-2.0` | PLATAFORMA | Open-source autoresearch powered by autonomous coding agents. Run Claude Code, O |
| `Agent-Field/SWE-AF` | 1.142 | Go | `Apache-2.0` | PLATAFORMA | Autonomous software engineering fleet of AI agents for production-grade PRs on A |
| `MayDay-wpf/snow-cli` | 1.071 | TypeScript | `NOASSERTION` | AGENTE | Agentic coding in your terminal,Simultaneously compatible with OpenAI, Gemini, a |
| `agentic-community/mcp-gateway-registry` | 1.064 | Python | `Apache-2.0` | INFRA | Enterprise-ready MCP Gateway & Registry that centralizes AI development tools wi |
| `editor-code-assistant/eca` | 1.061 | Clojure | `Apache-2.0` | ASISTENTE | Editor Code Assistant (ECA) - AI pair programming capabilities agnostic of edito |
| `shyamsaktawat/OpenAlpha_Evolve` | 1.059 | Python | `MIT` | FRAMEWORK | OpenAlpha_Evolve is an open-source Python framework inspired by the groundbreaki |
| `ClaytonFarr/ralph-playbook` | 1.040 | HTML | `MIT` | REGLA | A comprehensive guide to running autonomous AI coding loops using Geoff Huntley' |
| `sno-ai/sno-station` | 1.028 | TypeScript | `Apache-2.0` | AGENTE | When Claude Code hits its usage limit, Codex picks the task up from the same mem |
| `sudo-tee/opencode.nvim` | 983 | Lua | `Apache-2.0` | EXTENSION | neovim frontend for opencode - a terminal-based AI coding agent |
| `ByteAsk/cli` | 966 | Rust | `Apache-2.0` | AGENTE | AI coding agent for your terminal, with C/C++ build, debug, sanitizer and profil |
| `jmfederico/pi-web` | 965 | TypeScript | `MIT` | AGENTE | Web UI for Pi Coding Agent that keeps sessions alive in real workspaces. |
| `iamfakeguru/agent-md` | 965 | Shell | `MIT` | REGLA | Production-grade agent directives for autonomous coding agents: Claude Code, Cod |
| `kaplanelad/shellfirm` | 954 | Rust | `Apache-2.0` | INFRA | Safety guardrails for ai coding agents and human terminal commands |
| `iannuttall/ralph` | 951 | TypeScript | `NOASSERTION` | NO REVISADO | A minimal, file‑based agent loop for autonomous coding. |
| `abcwyc/pi-agent-desktop` | 947 | TypeScript | `MIT` | AGENTE | Pi — A cross-platform AI coding agent, bringing the Claude Code experience to yo |
| `qwen-code-dev-bot/oh-my-cli` | 844 | TypeScript | `Apache-2.0` | NO REVISADO | A minimal autonomous code-agent CLI built with Qwen Code |
| `EricTechPro/super-board` | 747 | HTML | `MIT` | NO REVISADO | Autonomous GitHub Project build → QA → review loop for Claude Code. One-line ins |
| `xichan96/dinotty` | 742 | TypeScript | `MIT` | NO REVISADO | Multi-device terminal server for AI coding agents. Server-side VTE, session pers |
| `wainmann/agent-manager` | 713 | Go | `Apache-2.0` | NO REVISADO | Terminal multiplexer for AI coding agents. Run Claude Code, Codex, OpenCode, Gem |
| `alinaqi/maggy` | 712 | Python | `MIT` | NO REVISADO | What started as an opinionated Claude Code setup kit is now an autonomous AI eng |
| `wecode-ai/RunVSAgent` | 711 | Kotlin | `Apache-2.0` | NO REVISADO | Run VSCode-based coding agents and extensions seamlessly within other IDE platfo |
| `Muvon/octomind` | 659 | Rust | `Apache-2.0` | NO REVISADO | Open-source AI coding agent and agent runtime: one binary, any model, MCP-native |
| `didi/helixharness` | 466 | Python | `Apache-2.0` | NO REVISADO | HelixHarness 提供完整的 AI Coding 工程化基础能力，还支持不同企业和团队结合自身的业务知识、工程规范和研发流程，持续构建和演进属于自己的专 |
| `JohnCrickett/ai-assisted-engineering` | 443 | NO MEDIDO | `NOASSERTION` | NO REVISADO | A Guide To AI-Assisted Software Engineering |
| `syahiidkamil/Software-Engineer-AI-Agent-Atlas` | 406 | JavaScript | `NOASSERTION` | NO REVISADO | SWE-ATLAS turns Claude Code into a senior AI software engineer agent: multi-proj |
| `timwuhaotian/the-pair` | 391 | Rust | `Apache-2.0` | NO REVISADO | Open-source AI pair programming desktop app: a read-only Mentor and an Executor  |
| `localixai/localix-cli` | 369 | Rust | `MIT` | NO REVISADO | Localix CLI: an ultrafast agent runtime for coding and CI, in Rust. One binary u |
| `get-vix/vix` | 286 | Go | `AGPL-3.0` | NO REVISADO | Sleek, Fast and Token Efficient AI Coding Agent |
| `orbi-build/orbi` | 262 | Python | `AGPL-3.0` | NO REVISADO | Open-source (AGPL-3.0), self-hostable lights-out software factory for the agent  |
| `the-snowpear/omp-studio` | 251 | TypeScript | `MIT` | NO REVISADO | Desktop GUI & arrangeable workbench for oh-my-pi (omp) AI coding agent — multi-w |
| `morganlinton/Albatross` | 246 | Rust | `MIT` | NO REVISADO | Open source, terminal-first AI coding agent with fully transparent multi-model r |
| `AlysisAi/AlysisCode` | 233 | Python | `Apache-2.0` | NO REVISADO | Alysis AI — one desktop app for building software with Alysis Code, creating doc |
| `team-telnyx/ai` | 230 | TypeScript | `MIT` | NO REVISADO | Official one-stop shop for AI Agents and developers building with Telnyx. |
| `autohandai/code-cli` | 220 | TypeScript | `Apache-2.0` | NO REVISADO | Open-source terminal coding agent. Use 15+ model providers, local models or your |
| `bytesbay/pi-gpui` | 207 | Rust | `MIT` | NO REVISADO | A native GUI for the pi coding agent, built with Rust and GPUI. For macOS (Apple |
| `liuxiaopai-ai/ralph-desktop` | 185 | Rust | `NOASSERTION` | NO REVISADO | Ralph Desktop -  Start vague. Ralph Loop until perfect.  A visual controller for |
| `cjermain/pi-less-yolo` | 170 | Shell | `Apache-2.0` | NO REVISADO | Slightly less YOLO version of pi-coding-agent in Docker container sandbox |
| `dmae97/omk` | 153 | TypeScript | `MIT` | NO REVISADO | Open-source AI coding agent for the terminal (TypeScript, MIT). Switch models mi |
| `kerim0x1/bettercode` | 148 | TypeScript | `MIT` | NO REVISADO | Every coding agent in one desktop workspace. Run Claude Code, OpenAI Codex, Curs |
| `LIUTod/scream-code` | 141 | TypeScript | `MIT` | NO REVISADO | 零远程行为的 7×24 自治 Agent 编队：子代理协同，自主进化丨A 24/7 autonomous agent fleet with zero remot |
| `borhen68/TokenTamer` | 129 | Python | `MIT` | NO REVISADO | A drop-in proxy that compresses bloated code context in real-time, cutting LLM A |
| `talmetis-labs/aizen` | 129 | Rust | `Apache-2.0` | NO REVISADO | Aizen is an AI coding agent that lives on your PC or VPS. Control it from your p |
| `jigjoy-ai/baro` | 126 | TypeScript | `MIT` | NO REVISADO | A CLI that turns a goal into a pull request - and a sandbox for testing concurre |
| `Asigers/pi-knock` | 119 | TypeScript | `MIT` | NO REVISADO | Remote notifications for the Pi coding agent — Pushover, ntfy, and webhooks when |
| `HarjjotSinghh/helicon` | 117 | TypeScript | `MIT` | NO REVISADO | Open-source desktop app and VS Code extension for Meta's Muse Code CLI, with thr |
| `FlowShift-in/IRIS-Mini` | 23 | TypeScript | `NOASSERTION` | NO REVISADO | IRIS Mini ⚡ — The agentic CLI heartbeat. Scaffolds code 🏗️, commands the local f |
| `plaume8/spoox` | 16 | Python | `Apache-2.0` | NO REVISADO | Spoox CLI - Terminal Agent - SPlit lOOp eXand agent |
| `joohw/clovapi` | 13 | Go | `NOASSERTION` | NO REVISADO | 内置本地代理，轻松管理 Agent API — 开源 CLI 与桌面端，保存 profile、switch 到 Claude Code / Codex / Op |
| `BjornMelin/agent-review` | 8 | TypeScript | `MIT` | NO REVISADO | Code review agents, scripts, and workflows. |
| `ricoNext/hello-agent-cli` | 5 | TypeScript | `MIT` | NO REVISADO | 基于 Claude Code 源码的工业级实现教程 |
| `JSstudent/gridvibe` | 4 | Python | `MIT` | NO REVISADO | GridVibe is a browser-first workspace for launching and managing multiple SSH te |
| `ruslanlap/loopguard` | 3 | Python | `MIT` | NO REVISADO | Stop AI agent loops before they burn your tokens. Watchdog for Claude Code / age |
| `POSTTTT/agent-cli-session` | 2 | TypeScript | `MIT` | NO REVISADO | A local web app for browsing, searching, and managing the session logs your codi |

## 5. Los que devolvieron 404

404 con token válido = el repo ya no existe con ese nombre. Se busco el
renombrado real por nombre y propietario:

| nombre original | veredicto | repo real | ★ | licencia |
|---|---|---|---:|---|
| `AIDC-AI/AutoGPT` | no encontrado | — | — | — |
| `Aider-AI/aider-ui` | no encontrado | — | — | — |
| `Amazon-Science/auto-coder` | no encontrado | — | — | — |
| `BeehiveInnovations/pal-bee` | no encontrado | — | — | — |
| `MeltyAI/melty` | no encontrado | — | — | — |
| `PuerNya/small-to-large` | no encontrado | — | — | — |
| `codebuff-dev/codebuff` | no encontrado | — | — | — |
| `codota-io/ai-unit-test` | no encontrado | — | — | — |
| `dnephin/promptingguide` | no encontrado | — | — | — |
| `jlowin/fastapi-mcp` | no encontrado | — | — | — |
| `keplergpt/kepler` | no encontrado | — | — | — |
| `lovable-dev/lovable` | no encontrado | — | — | — |
| `microsoft/agent-framework` | activo | `microsoft/agent-framework-go` | 707 | `MIT` |
| `runkit/runkit` | activo | `runkit/test` | 0 | `NOASSERTION` |
| `slavakurilyak/AI-Performance` | no encontrado | — | — | — |
| `sourcegraph/cody` | activo | `sourcegraph/cody-vs` | 89 | `NOASSERTION` |
| `vercel/v0` | activo | `vercel/v0-sdk` | 517 | `NOASSERTION` |
| `vinta/harbor` | no encontrado | — | — | — |
| `wonderwhy-er/DesktopCommanderGPT` | no encontrado | — | — | — |

Los **no encontrados** no se han movido: o se borraron, o el nombre nunca
existio tal cual. **NO MEDIDO** que clase de producto era.

## 6. El registro ACP: la foto del ecosistema

Matriz del **2026-11-26**: 42 agentes
probados, `initialize` con exito en **41**.

| capacidad | soportada | exige auth | metodo inexistente |
|---|---:|---:|---:|
| `session/list` | 31 | 5 | 5 |
| `session/fork` | 14 | 7 | 17 |
| `session/resume` | 17 | 11 | 11 |
| `session/stop` | 0 | 1 | 38 |
| `session/set_model` | 17 | 4 | 17 |

| id | nombre | version | initialize | repo |
|---|---|---:|---|---|
| `amp-acp` | Amp | 1 | success | tao12345666333/amp-acp |
| `antigravity-acp` | Google Antigravity | 1 | success | — |
| `auggie` | Auggie CLI | 1 | success | augmentcode/auggie |
| `autohand` | Autohand Code | 1 | success | autohandai/code-cli |
| `bob` | Bob | 1 | success | — |
| `claude-acp` | Claude Agent | 1 | success | agentclientprotocol/claude-agent-acp |
| `cline` | Cline | 1 | success | cline/cline |
| `codebuddy-code` | Codebuddy Code | 1 | success | — |
| `codex-acp` | Codex | 1 | success | agentclientprotocol/codex-acp |
| `cortex-code` | Cortex Code | None | process_error | https://docs.snowflake.com/en/user-guide/cortex-code/cortex-code |
| `corust-agent` | Corust Agent | 1 | success | Corust-ai/corust-agent-release |
| `crow-cli` | crow-cli | 1 | success | crow-cli/crow-cli |
| `cursor` | Cursor | 1 | success | — |
| `devin` | Devin | 1 | success | CognitionAI/devin-cli |
| `dimcode` | DimCode | 1 | success | — |
| `dirac` | Dirac | 1 | success | dirac-run/dirac |
| `factory-droid` | Factory Droid | 1 | success | — |
| `gemini` | Gemini CLI | 1 | success | google-gemini/gemini-cli |
| `github-copilot` | GitHub Copilot | 1 | success | github/copilot-language-server-release |
| `github-copilot-cli` | GitHub Copilot | 1 | success | github/copilot-cli |
| `glm-acp-agent` | GLM Agent | 1 | success | stefandevo/glm-acp-agent |
| `goose` | goose | 1 | success | block/goose |
| `grok-build` | Grok Build | 1 | success | — |
| `harn` | Harn | 1 | success | burin-labs/harn |
| `junie` | Junie | 1 | success | JetBrains/junie-acp-release |
| `kilo` | Kilo | 1 | success | Kilo-Org/kilocode |
| `kimchi` | Kimchi | 1 | success | getkimchi/kimchi |
| `kimi` | Kimi CLI (Legacy) | 1 | success | MoonshotAI/kimi-cli |
| `kimi-code` | Kimi Code | 1 | success | MoonshotAI/kimi-code |
| `kiro` | Kiro CLI | 1 | success | kirodotdev/Kiro |
| `minimax-code` | MiniMax Code | 1 | success | — |
| `mistral-vibe` | Mistral Vibe | 1 | success | mistralai/mistral-vibe |
| `muse-code` | Muse Code | 1 | success | — |
| `nova` | Nova | 1 | success | Compass-Agentic-Platform/nova |
| `opencode` | OpenCode | 1 | success | anomalyco/opencode |
| `opencode-v2` | OpenCode (legacy ID) | 1 | success | anomalyco/opencode |
| `pi-acp` | pi ACP | 1 | success | svkozak/pi-acp |
| `poolside` | Poolside | 1 | success | poolsideai/pool |
| `qoder` | Qoder CLI | 1 | success | — |
| `qwen-code` | Qwen Code | 1 | success | QwenLM/qwen-code |
| `sigit` | siGit Code | 1 | success | getsigit/sigit |
| `stakpak` | Stakpak | 1 | success | stakpak/agent |

## 7. Lectura para MegaDaemon

1. **ACP v1 es el estandar de facto**: de los 42 agentes del registro, 41
   inicializan y **los 41 son v1**. Implementar v1 primero es el minimo para
   ser interoperable, no una apuesta. `sessionStop` no lo implementa nadie
   (0/41) ⇒ no es requisito.
2. **El registro oficial es una fuente de catalogo mejor que las busquedas**
   por topico: 22 de sus
   agentes con repo no estaban en nuestro barrido. Las busquedas por topico
   encontraron 138 agentes mas, pero 275 tambien ruido (skills, listas,
   prompts).
3. **El megadon es Python y TypeScript**; Rust son 5 de 65 en el bloque
   original. Nuestro motor en Rust es la opcion minoritaria del ecosistema:
   ventaja de coste, no deficit de alcance.
4. **Licencias**: solo 43 de los 65 medidos son MIT o Apache-2.0. Los GPL,
   AGPL, MPL, CC-BY y NOASSERTION no se pueden integrar como codigo; solo
   estudiar como referencia de diseno (que es lo que se ha hecho).
5. **Diez agentes del registro no publican repo** (Cursor, Devin,
   Antigravity, MiniMax Code, Copilot, Factory Droid, Qoder, Muse Code,
   Grok Build, DimCode, Bob): para esos la unica via es el binario y su
   protocolo, que es justo lo que hacen los contratos de Cursor y Devin.

## 8. Datos crudos

- `docs/medidas/agentes_faltantes.json`
- `docs/medidas/agentes_faltantes_rest.json`
- `docs/medidas/agentes_renombrados.json`
- `docs/medidas/agentes_escapados.json`
- `docs/medidas/agentes_escapados_filtrados.json`
- `docs/medidas/agentes_escapados_filtrados_veredicto.json`
- `docs/medidas/agentes_escapados_descartados.json`
- `docs/medidas/acp_escapados.json`
- `docs/medidas/acp_escapados_medidos.json`
- `docs/medidas/busqueda_decompiladores.json`
- `artifacts/acp-comparado/protocol_matrix.json`


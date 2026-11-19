#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
SEERE v11 — EXPERIMENTO DE FIDELIDAD Y DE PRESUPUESTO DE TURNOS
================================================================
E1: el bench enviaba la pregunta SUELTA; el harness real envía
    "<pregunta>\n<ENV uid=..|..>".  ¿Cambia el enrutado?  (fidelidad del bench)

E2: max_turns=2 (1 acción + 1 respuesta) vs max_turns=4 (3 acciones + 1
    respuesta) en el bucle completo con herramientas reales.

Uso: python3 bench_fidelity.py
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SEERE_STATE", "/tmp/seere_fid_state")
os.makedirs("/tmp/seere_fid_state", exist_ok=True)

import seere_v11 as V11  # noqa: E402
from bench_models import Server, call, med, sensible  # noqa: E402

CASES_ES = [
    ("¿cuántos núcleos tiene esta máquina?", True),
    ("lee el contenido de /data/local/tmp/config.txt", True),
    ("busca 'error' dentro de /data/local/tmp/logs", True),
    ("¿qué hora es en Madrid?", True),
    ("busca en wikipedia qué es un autómata finito", True),
    ("¿está vivo el proceso init?", True),
    ("dime el pid del proceso actual", True),
    ("¿cuánta memoria RAM queda?", True),
    ("lista los ficheros de /data/local/tmp", True),
    ("¿qué versión de python hay instalada?", True),
    ("busca en la web las noticias de hoy", True),
    ("¿cuánto espacio de disco queda?", True),
]

# Rutas configurables: por defecto las del entorno de desarrollo, pero se
# pueden mover con MEGADAEMON_MODELS_DIR / MEGADAEMON_RESEARCH_DIR para que
# el banco de pruebas sea reproducible en otra máquina.
MODELS_DIR = os.environ.get("MEGADAEMON_MODELS_DIR", "/workspace/models")
RESEARCH_DIR = os.environ.get(
    "MEGADAEMON_RESEARCH_DIR",
    "/workspace/mavis-deep-research/20261118_203500_seere-tiny-llm-prompt",
)
MODELS = {
    "qwen2.5-0.5b": os.path.join(RESEARCH_DIR, "models", "qwen2.5-0.5b-instruct-q4_k_m.gguf"),
    "smollm2-360m": os.path.join(MODELS_DIR, "SmolLM2-360M-Instruct-Q4_K_M.gguf"),
    "llama-3.2-1b": os.path.join(MODELS_DIR, "Llama-3.2-1B-Instruct-Q4_K_M.gguf"),
    "qwen2.5-1.5b": os.path.join(MODELS_DIR, "Qwen2.5-1.5B-Instruct-Q4_K_M.gguf"),
}


def e1(chat, tag):
    """¿La huella del host en el turno de usuario cambia el enrutado?"""
    es = V11.sys_prompt("REACT", "es")
    out = {}
    for mode in ("query_sola", "query_con_ENV"):
        routed = sens = 0
        for q, _ in CASES_ES:
            user = q if mode == "query_sola" else f"{q}\n<ENV {V11.fp()}>"
            raw, _, _, _, _ = call(chat, es, user, V11.GBNF_REACT, V11.ACTION_MAX_TOK)
            p = V11.parse(raw)
            args = " ".join(a for _, a in p["actions"])
            if p["actions"]:
                routed += 1
                if sensible(q, args):
                    sens += 1
        out[mode] = {"routed": routed, "sensible": sens, "n": len(CASES_ES)}
        print(f"  [{tag}] {mode:16s} routed={routed:2d}/12  sens={sens:2d}/12", flush=True)
    return out


def e2(chat, tag):
    """max_turns 2 vs 4 en el bucle completo."""
    out = {}
    for mt in (2, 4):
        res = []
        for q in ("¿cuántos núcleos tiene esta máquina?", "¿cuánta memoria RAM queda?",
                  "¿qué hora es?", "hello"):
            env = dict(os.environ)
            env["SEERE_LLM"] = chat.rsplit("/v1", 1)[0]
            env["SEERE_STATE"] = "/tmp/seere_fid_state"
            t0 = time.time()
            p = subprocess.run([sys.executable, os.path.join(HERE, "seere_v11.py"),
                                "--json", "--allow-exec", "--max-turns", str(mt), q],
                               capture_output=True, text=True, timeout=300, env=env)
            ms = (time.time() - t0) * 1000
            try:
                d = json.loads(p.stdout)
            except Exception:  # noqa: BLE001
                res.append({"q": q, "error": (p.stdout + p.stderr)[-200:]})
                continue
            res.append({"q": q, "answer": d.get("answer", "")[:90], "grounded": d.get("grounded"),
                        "tokens": d.get("tokens"), "ms": round(ms),
                        "n_tools": len(d.get("tools", [])),
                        "tools": [t.get("tool") + "(" + t.get("arg", "")[:40] + ")"
                                  for t in d.get("tools", [])][:3]})
        out[f"max_turns={mt}"] = res
        grounded = sum(1 for r in res if r.get("grounded"))
        print(f"  [{tag}] max_turns={mt}: grounded={grounded}/{len(res)} "
              f"tok_med={med([r.get('tokens', 0) for r in res])} "
              f"ms_med={med([r.get('ms', 0) for r in res])}", flush=True)
        for r in res:
            print(f"        {r['q'][:30]:32s} {str(r.get('answer'))[:52]!r} "
                  f"g={r.get('grounded')} t={r.get('n_tools')}", flush=True)
    return out


def main() -> int:
    only = sys.argv[1:] or list(MODELS)
    all_res = {}
    for tag in only:
        path = MODELS[tag]
        print(f"\n=== {tag} ===", flush=True)
        with Server(path, ctx=8192) as srv:
            chat = f"http://127.0.0.1:{srv.port}/v1/chat/completions"
            try:
                call(chat, V11.sys_prompt("REACT", "es"), "hi", V11.GBNF_REACT, V11.ACTION_MAX_TOK)
            except Exception:  # noqa: BLE001
                pass
            all_res[tag] = {"E1_fidelidad": e1(chat, tag), "E2_turnos": e2(chat, tag)}
    with open(os.path.join(HERE, "bench_fidelity.json"), "w") as f:
        json.dump(all_res, f, indent=2, ensure_ascii=False)
    print("\n[guardado] bench_fidelity.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())

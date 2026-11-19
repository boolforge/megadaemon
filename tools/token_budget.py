#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
SEERE v11 — Token Budget Tool
=============================
Exact token accounting for the SEERE system prompt using the TARGET model's
tokenizer (Qwen2.5-0.5B by default), because "chars/4" heuristics are off by
15-30% across tokenizers and every optimization decision in SEERE is a
token-budget decision.

Usage:
    python3 token_budget.py                 # compare v10 vs v11
    python3 token_budget.py --prompt FILE   # count a single file
    python3 token_budget.py --stdin         # count stdin

Why this exists
---------------
Cross-tokenizer estimation error is 15-30% (ai-engineering-playbook,
tokens-and-context.md). A prompt optimization that saves "20 tokens" is
worthless if the measurement itself has ±25% error.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from typing import Any, Dict, List, Optional

CACHE_DIR = os.environ.get(
    "SEERE_TOK_CACHE",
    os.path.join(os.path.dirname(os.path.abspath(__file__)), ".tokcache"),
)

MODELS: Dict[str, Dict[str, str]] = {
    "qwen2.5-0.5b": {
        "url": "https://huggingface.co/Qwen/Qwen2.5-0.5B/resolve/main/tokenizer.json",
        "file": "qwen2.5-0.5b.json",
        "family": "qwen2.5",
    },
    "smollm2-360m": {
        "url": "https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct/resolve/main/tokenizer.json",
        "file": "smollm2-360m.json",
        "family": "llama3",
    },
    # La familia Qwen2.5 comparte tokenizador con 0.5B (mismo vocab) — se
    # reusa el fichero cacheado, pero se declara explícito para el informe.
    "qwen2.5-1.5b": {
        "url": "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct/resolve/main/tokenizer.json",
        "file": "qwen2.5-1.5b.json",
        "family": "qwen2.5",
    },
    "llama-3.2-1b": {
        "url": "https://huggingface.co/unsloth/Llama-3.2-1B-Instruct/resolve/main/tokenizer.json",
        "file": "llama-3.2-1b.json",
        "family": "llama3",
    },
}


# --------------------------------------------------------------------------- #
# Tokenizer loading
# --------------------------------------------------------------------------- #
def _fetch(model_key: str) -> str:
    os.makedirs(CACHE_DIR, exist_ok=True)
    spec = MODELS[model_key]
    path = os.path.join(CACHE_DIR, spec["file"])
    if os.path.exists(path) and os.path.getsize(path) > 100_000:
        return path
    print(f"[token_budget] downloading {model_key} tokenizer ...", file=sys.stderr)
    req = urllib.request.Request(spec["url"], headers={"User-Agent": "SEERE/11"})
    with urllib.request.urlopen(req, timeout=90) as r:
        data = r.read()
    with open(path, "wb") as f:
        f.write(data)
    return path


def load_tokenizer(model_key: str = "qwen2.5-0.5b"):
    from tokenizers import Tokenizer

    return Tokenizer.from_file(_fetch(model_key))


# --------------------------------------------------------------------------- #
# Chat template rendering (exact ChatML, matching Qwen2.5 / SmolLM2 jinja)
# --------------------------------------------------------------------------- #
def render_chatml(messages: List[Dict[str, str]], add_generation_prompt: bool = True,
                  system_fallback: str = "You are a helpful assistant.") -> str:
    """Render an OpenAI-style message list exactly like the Qwen2.5 ChatML
    template shipped in tokenizer_config.json:

        {%- if messages[0]['role'] == 'system' %}
            {{- '<|im_start|>system\\n' + messages[0]['content'] + '<|im_end|>\\n' }}
        {%- else %}
            {{- '<|im_start|>system\\nYou are a helpful assistant.<|im_end|>\\n' }}
        {%- endif %}
        ... then one <|im_start|>{role}\\n{content}<|im_end|>\\n per message,
        and finally <|im_start|>assistant\\n when add_generation_prompt.
    """
    out: List[str] = []
    if not messages:
        return ""
    if messages[0]["role"] == "system":
        out.append("<|im_start|>system\n" + messages[0]["content"] + "<|im_end|>\n")
        rest = messages[1:]
    else:
        out.append("<|im_start|>system\n" + system_fallback + "<|im_end|>\n")
        rest = messages
    for m in rest:
        out.append("<|im_start|>" + m["role"] + "\n" + m["content"] + "<|im_end|>\n")
    if add_generation_prompt:
        out.append("<|im_start|>assistant\n")
    return "".join(out)


# --------------------------------------------------------------------------- #
# Fallback: tokenizador real del runtime (llama-tokenize)
# --------------------------------------------------------------------------- #
# El paquete `tokenizers` de PyPI no siempre está disponible (mirror caído,
# red restringida).  Para ese caso se usa `llama-tokenize`, que ES el
# tokenizador con el que el servidor de llama.cpp tokeniza de verdad — es
# decir, más preciso que la aproximación con tokenizer.json de HF.
_RESEARCH_DIR = os.environ.get(
    "MEGADAEMON_RESEARCH_DIR",
    "/workspace/mavis-deep-research/20261118_203500_seere-tiny-llm-prompt",
)
_MODELS_DIR = os.environ.get("MEGADAEMON_MODELS_DIR", "/workspace/models")
LLAMA_TOKENIZE_CANDIDATES = [
    os.environ.get("MEGADAEMON_LLAMA_TOKENIZE", ""),
    os.path.join(_RESEARCH_DIR, "llama-b12202", "llama-tokenize"),
]

GGUF_GLOBS = {
    "qwen2.5-0.5b": [os.path.join(_RESEARCH_DIR, "models", "qwen2.5-0.5b-instruct-q4_k_m.gguf")],
    "qwen2.5-1.5b": [os.path.join(_MODELS_DIR, "Qwen2.5-1.5B-Instruct-Q4_K_M.gguf")],
    "smollm2-360m": [os.path.join(_MODELS_DIR, "SmolLM2-360M-Instruct-Q4_K_M.gguf")],
    "llama-3.2-1b": [os.path.join(_MODELS_DIR, "Llama-3.2-1B-Instruct-Q4_K_M.gguf")],
}


def _find_llama_tokenize() -> Optional[str]:
    for c in LLAMA_TOKENIZE_CANDIDATES:
        if os.path.exists(c) and os.access(c, os.X_OK):
            return c
    return None


def _find_gguf(model_key: str) -> Optional[str]:
    import glob
    for pat in GGUF_GLOBS.get(model_key, []):
        hits = sorted(glob.glob(pat))
        if hits:
            return hits[0]
    return None


class LlamaTokenizer:
    """Contador de tokens vía llama-tokenize (el tokenizador del runtime)."""

    def __init__(self, model_key: str):
        import glob
        exe = _find_llama_tokenize()
        gguf = _find_gguf(model_key)
        if not exe or not gguf:
            raise RuntimeError(
                "no hay llama-tokenize ni GGUF local para " + model_key)
        self.exe = exe
        self.gguf = gguf
        self.family = MODELS[model_key]["family"]

    def _ids(self, text: str) -> List[int]:
        import subprocess
        p = subprocess.run([self.exe, "-m", self.gguf, "-p", text, "--ids"],
                           capture_output=True, text=True, timeout=180)
        out = p.stdout.strip()
        i = out.find("[")
        j = out.rfind("]")
        if i < 0 or j < 0:
            raise RuntimeError("llama-tokenize: salida inesperada: " + out[:200])
        return json.loads(out[i:j + 1])

    def count(self, text: str) -> int:
        return len(self._ids(text))

    def ids(self, text: str) -> List[int]:
        return self._ids(text)

    def pieces(self, text: str) -> List[str]:
        return ["<tok%d>" % i for i in self._ids(text)]


# --------------------------------------------------------------------------- #
# Accounting helpers
# --------------------------------------------------------------------------- #
class Counter:
    def __init__(self, model_key: str = "qwen2.5-0.5b"):
        self.model_key = model_key
        self.family = MODELS[model_key]["family"]
        self.backend = "tokenizers"
        try:
            self.tok = load_tokenizer(model_key)
        except Exception:
            self.tok = LlamaTokenizer(model_key)
            self.backend = "llama-tokenize"

    def count(self, text: str) -> int:
        if self.backend == "llama-tokenize":
            return self.tok.count(text)
        return len(self.tok.encode(text, add_special_tokens=False).ids)

    def count_chat(self, messages: List[Dict[str, str]], add_generation_prompt: bool = True) -> int:
        return self.count(render_chatml(messages, add_generation_prompt))

    def render(self, messages: List[Dict[str, str]], add_generation_prompt: bool = True) -> str:
        return render_chatml(messages, add_generation_prompt)

    def ids(self, text: str) -> List[int]:
        if self.backend == "llama-tokenize":
            return self.tok.ids(text)
        return self.tok.encode(text, add_special_tokens=False).ids

    def pieces(self, text: str) -> List[str]:
        if self.backend == "llama-tokenize":
            return self.tok.pieces(text)
        return self.tok.encode(text, add_special_tokens=False).tokens


def prefix_break_cost(counter: Counter, old_sys: str, new_sys: str) -> Dict[str, int]:
    """How many leading tokens are still shared between two system prompts.

    This is exactly llama.cpp's `slot.prompt.tokens.get_common_prefix(input_tokens)`
    (tools/server/server-context.cpp). Everything after the break must be
    re-prefilled, so the break point IS the KV-cache savings.
    """
    a = counter.ids(old_sys)
    b = counter.ids(new_sys)
    n = 0
    for x, y in zip(a, b):
        if x != y:
            break
        n += 1
    return {
        "old_tokens": len(a),
        "new_tokens": len(b),
        "shared_prefix_tokens": n,
        "refilled_tokens": len(b) - n,
    }


def trace_cost(counter: "Counter", turns: int = 3, obs_chars: int = 480,
              query: str = "lee el fichero de configuracion y dime el pid actual",
              plan_line: bool = True) -> Dict[str, Any]:
    """Coste real de un episodio ReAct de N turnos.

    Distingue tres magnitudes que se confunden constantemente:

      prefill_cacheado : tokens que el modelo procesa UNA vez (con KV cache
                         funcionando) = tamaño del contexto final.
      prefill_reproceso: tokens que el modelo procesa de NUEVO si el prefijo
                         no se puede reutilizar (p.ej. porque el system prompt
                         cambia entre turnos) = suma de contextos parciales.
      salida           : tokens generados por el modelo (se generan Y se
                         reenvían como contexto: se pagan dos veces).

    Esto decide dónde optimizar: recortar 20 tokens del system prompt NO mueve
    la aguja; la mueven (a) que el prefijo sea reutilizable y (b) el tamaño de
    las observaciones.
    """
    here = os.path.dirname(os.path.abspath(__file__))
    sys_v10 = open(os.path.join(here, "seere_sysprompt_v10.txt"), encoding="utf-8").read()
    sys_v11 = open(os.path.join(here, "seere_sysprompt_v11.txt"), encoding="utf-8").read()

    obs = "x" * obs_chars
    out_v10 = (("P: " + "plan " * 3 + "\n") if plan_line else "") + "A: $ cat /proc/1/stat"
    out_v11 = "A: $ cat /proc/1/stat"

    def episode(sys_p: str, out: str, stable_prefix: bool):
        ctx = counter.count_chat([{"role": "system", "content": sys_p},
                                   {"role": "user", "content": query}])
        sys_tokens = counter.count(sys_p)
        for _ in range(turns):
            ctx += counter.count(out) + counter.count(obs)
        ctx += counter.count("R: uptime is 22 [SRC:1]")
        # Sesión 1: todo el contexto se procesa.
        # Sesión N: si el prefijo (system prompt) es idéntico al de la sesión
        # anterior, llama.cpp reutiliza el KV cache y sólo procesa el sufijo
        # nuevo.  Si el prefijo cambia, se reprocesa TODO el contexto.
        suffix = ctx - sys_tokens
        return {"contexto_final": ctx,
                "prefill_sesion_1": ctx,
                "prefill_sesion_N": suffix if stable_prefix else ctx,
                "salida_generada": counter.count(out) * turns + 12}

    return {
        "v10 (prefijo inestable)": episode(sys_v10, out_v10, stable_prefix=False),
        "v11 (prefijo estable)": episode(sys_v11, out_v11, stable_prefix=True),
    }


def main() -> int:
    ap = argparse.ArgumentParser(prog="token_budget")
    ap.add_argument("--prompt", help="count tokens of a file")
    ap.add_argument("--stdin", action="store_true")
    ap.add_argument("--model", default="qwen2.5-0.5b", choices=list(MODELS))
    ap.add_argument("--chat", action="store_true", help="treat stdin/file as a ChatML message list")
    ap.add_argument("--trace", action="store_true", help="coste de un episodio ReAct")
    ap.add_argument("--turns", type=int, default=3)
    ap.add_argument("--obs", type=int, default=480)
    args = ap.parse_args()

    c = Counter(args.model)

    if args.trace:
        res = trace_cost(c, turns=args.turns, obs_chars=args.obs)
        print(f"tokenizer={args.model}  turnos={args.turns}  observacion={args.obs} chars\n")
        keys = ["contexto_final", "prefill_sesion_1", "prefill_sesion_N", "salida_generada"]
        w = max(len(k) for k in res)
        print(f"{'escenario'.ljust(w)}  " + "  ".join(k.rjust(16) for k in keys))
        print("-" * (w + 4 + 16 * len(keys)))
        for name, row in res.items():
            print(f"{name.ljust(w)}  " + "  ".join(str(row[k]).rjust(16) for k in keys))
        a = res["v10 (prefijo inestable)"]
        b = res["v11 (prefijo estable)"]
        d = a["prefill_sesion_N"] - b["prefill_sesion_N"]
        print(f"\n  prefill por sesion repetida: {a['prefill_sesion_N']} -> {b['prefill_sesion_N']} tokens "
              f"(-{d}, -{100*d//max(1,a['prefill_sesion_N'])}%)")
        print(f"  salida generada            : {a['salida_generada']} -> {b['salida_generada']} tokens")
        return 0

    if args.prompt:
        text = open(args.prompt, encoding="utf-8").read()
    elif args.stdin:
        text = sys.stdin.read()
    else:
        here = os.path.dirname(os.path.abspath(__file__))
        v10 = os.path.join(here, "seere_sysprompt_v10.txt")
        v11 = os.path.join(here, "seere_sysprompt_v11.txt")
        v10s = os.path.join(here, "seere_sysprompt_v11_es.txt")
        print(f"tokenizer={args.model}  ({c.family} BPE)")
        rows = []
        for label, path in (("v10 (original)", v10), ("v11 (optimized, EN)", v11),
                            ("v11 (optimized, ES)", v10s)):
            if not os.path.exists(path):
                rows.append((label, "-", "-"))
                continue
            t = open(path, encoding="utf-8").read()
            n_sys = c.count(t)
            n_full = c.count_chat([{"role": "system", "content": t},
                                   {"role": "user", "content": "q"}])
            rows.append((label, n_sys, n_full))
        w = max(len(r[0]) for r in rows)
        print(f"\n{'variant'.ljust(w)}  sys_tokens  rendered_chat_tokens")
        print("-" * (w + 26))
        for label, a, b in rows:
            print(f"{label.ljust(w)}  {str(a).rjust(10)}  {str(b).rjust(21)}")
        return 0

    if args.chat:
        msgs = json.loads(text)
        print(c.count_chat(msgs))
    else:
        print(c.count(text))
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
SEERE — END-TO-END A/B (modelo real, llama.cpp real)
====================================================
Compara el stack v10 (prompt + gramática v10) contra el stack v11 sobre el
MISMO modelo 0.5B y los MISMOS parámetros de decodificación.

Mide:
  * formato   : ¿la salida cumple la gramática y la parsea el harness?
  * opcode    : ¿la acción elegida es la razonable para la pregunta?
  * tokens    : tokens generados (el coste que paga el usuario)
  * latencia  : milisegundos de la llamada

Uso:
    llama-server -m qwen2.5-0.5b-instruct-q4_k_m.gguf --port 8080 --jinja &
    python3 bench_e2e.py
"""
from __future__ import annotations

import json
import os
import sys
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(os.path.dirname(HERE), "mavis-deep-research",
                                "20261118_203500_seere-tiny-llm-prompt"))
os.environ.setdefault("SEERE_STATE", "/tmp/seere_bench_state")
os.makedirs("/tmp/seere_bench_state", exist_ok=True)

import seere_v10_original as V10  # noqa: E402
import seere_v11 as V11  # noqa: E402

CHAT = os.environ.get("SEERE_LLM", "http://127.0.0.1:8080").rstrip("/") + "/v1/chat/completions"

CASES = [
    ("¿cuántos núcleos tiene esta máquina?", "ROOT_EXEC"),
    ("lee el contenido de /data/local/tmp/config.txt", "CHUNK_READ"),
    ("busca 'error' dentro de /data/local/tmp/logs", "GREP_FIND"),
    ("¿qué hora es en Madrid?", "WEB_SEARCH"),
    ("busca en wikipedia qué es un autómata finito", "WIKI_SEARCH"),
    ("¿está vivo el proceso init?", "ROOT_EXEC"),
    ("hola", "NONE"),
    ("calcula 17*23", "NONE"),
]

TEMP = 0.0
MAXTOK_V10 = 75
MAXTOK_V11 = V11.ACTION_MAX_TOK


def call(system: str, user: str, grammar: str, max_tok: int):
    payload = {
        "model": "local",
        "messages": [{"role": "system", "content": system},
                     {"role": "user", "content": user}],
        "max_tokens": max_tok,
        "temperature": TEMP,
        "top_p": 0.95,
        "min_p": 0.05,
        "seed": 42,
        "stream": False,
        "cache_prompt": True,
        "grammar": grammar,
    }
    body = json.dumps(payload).encode()
    req = urllib.request.Request(CHAT, data=body,
                                 headers={"Content-Type": "application/json"}, method="POST")
    t0 = time.time()
    with urllib.request.urlopen(req, timeout=120) as r:
        d = json.loads(r.read().decode())
    dt = (time.time() - t0) * 1000
    ch = d["choices"][0]
    return (ch["message"]["content"] or "", ch.get("finish_reason", "stop"),
            d.get("usage", {}).get("completion_tokens", 0), dt)


def expected_family(tool: str) -> str:
    """Familia semántica de la acción esperada (para tolerar matices)."""
    return tool


def main() -> int:
    rows = []
    for label, sys_p, grammar, parser, max_tok in (
        ("v10", V10.sys_prompt("REACT"), V10.GBNF_REACT, V10.parse, MAXTOK_V10),
        ("v11", V11.sys_prompt("REACT", "es"), V11.GBNF_REACT, V11.parse, MAXTOK_V11),
    ):
        for q, want in CASES:
            try:
                raw, fr, tok, ms = call(sys_p, q, grammar, max_tok)
            except Exception as e:  # noqa: BLE001
                rows.append({"variant": label, "q": q, "want": want, "error": str(e)[:120]})
                continue
            p = parser(raw)
            got = p["actions"][0][0] if p["actions"] else ("FINAL" if p["type"] == "final" else "MALFORMED")
            # ¿es la acción de la familia esperada?
            fam_ok = (got == want) or (want == "NONE" and got in ("FINAL", "NONE")) \
                or (want in ("ROOT_EXEC", "CHUNK_READ", "GREP_FIND", "WEB_SEARCH", "WIKI_SEARCH")
                    and got in ("DELEGATE_LLM", "ASK"))
            rows.append({
                "variant": label, "q": q, "want": want, "got": got,
                "kind": p["type"], "tokens": tok, "ms": round(ms), "finish": fr,
                "family_ok": bool(fam_ok),
                "raw": raw.replace("\n", " | ")[:110],
            })
            print(f"[{label}] {q[:42]:44s} -> {got:14s} tok={tok:3d} {ms:6.0f}ms  {raw.replace(chr(10),' | ')[:70]!r}")

    with open(os.path.join(HERE, "bench_e2e.json"), "w") as f:
        json.dump(rows, f, indent=2, ensure_ascii=False)

    print("\n" + "=" * 76)
    print(f"{'variant':8s} {'n':>3s} {'parse_ok':>9s} {'family_ok':>10s} {'tok_med':>8s} {'ms_med':>7s}")
    print("-" * 76)
    for label in ("v10", "v11"):
        rs = [r for r in rows if r.get("variant") == label and "error" not in r]
        if not rs:
            print(f"{label:8s}  (sin datos)")
            continue
        parse_ok = sum(1 for r in rs if r["kind"] in ("action", "final"))
        fam_ok = sum(1 for r in rs if r["family_ok"])
        toks = sorted(r["tokens"] for r in rs)
        mss = sorted(r["ms"] for r in rs)
        med = lambda a: a[len(a) // 2] if a else 0
        print(f"{label:8s} {len(rs):3d} {parse_ok:4d}/{len(rs):<4d} {fam_ok:5d}/{len(rs):<4d} "
              f"{med(toks):8d} {med(mss):7.0f}")
    errs = [r for r in rows if "error" in r]
    if errs:
        print(f"\n  errores: {len(errs)}")
        for e in errs[:4]:
            print("   ", e["variant"], e["q"][:40], e["error"][:80])
    return 0


if __name__ == "__main__":
    sys.exit(main())

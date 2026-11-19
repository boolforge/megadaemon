#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
SEERE v11 — BENCH MULTI-MODELO
==============================
Compara candidatos en el harness real de SEERE: mismo prompt, misma gramática,
mismo servidor llama.cpp, mismos parámetros de decodificación. Lo único que
cambia es el modelo.

Uso:
    python3 bench_models.py --models /workspace/models/SmolLM2-360M-Instruct-Q4_K_M.gguf \
                            --models /workspace/models/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf

Qué mide:
    routed_when_needed : la métrica de SEERE — el modelo pidió herramienta en
                         las 12 preguntas que exigen datos externos
    answered_from_mem  : respondió sin pedir datos (alucinación de datos)
    hint_copy          : el argumento es la pista de la línea OPS copiada
    multi_action       : el argumento se tragó una segunda acción
    parseable         : la salida cumple la gramática
    tok / ms           : coste y latencia (medianas)
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("SEERE_STATE", "/tmp/seere_bench_state")
os.makedirs("/tmp/seere_bench_state", exist_ok=True)

import seere_v11 as V11  # noqa: E402

# El servidor llama.cpp se busca en: (1) $MEGADAEMON_LLAMA_SERVER, (2) el
# directorio del research dir, (3) $PATH.
_RESEARCH_DIR = os.environ.get(
    "MEGADAEMON_RESEARCH_DIR",
    "/workspace/mavis-deep-research/20261118_203500_seere-tiny-llm-prompt",
)
SERVER_CANDIDATES = [
    os.environ.get("MEGADAEMON_LLAMA_SERVER", ""),
    os.path.join(_RESEARCH_DIR, "llama-b12202", "llama-server"),
    shutil.which("llama-server") or "",
]

HINT_TEXTS = ["shell", "web search", "wiki", "read file", "search files", "math",
              "bigger model", "shm", "mem", "note", "user", "stop"]

# --- ¿El comando elegido es SEMÁNTICAMENTE plausible? ------------------------
# El enrutado ("pidió una herramienta") no dice si la orden sirve.  Un modelo
# puede enrutar siempre a $ shell y acertar cero veces.  Estas son las
# órdenes que de verdad resuelven cada pregunta.
EXPECT = [
    # (substrings de la pregunta, [substrings aceptables en la orden])
    (["núcleos", "cores"], ["nproc", "cpuinfo", "lscpu", "processor"]),
    (["contenido de /data", "content of /data"], ["cat ", "cat\t"]),
    (["'error'", '"error"'], ["grep", "find", "xargs"]),
    (["hora es", "time is it"], ["date"]),
    (["wikipedia", "autómata finito", "finite automaton"], ["w ", "W ", "wikipedia", "?"]),
    (["proceso init", "init process"], ["ps", "init", "/proc/1", "pgrep"]),
    (["pid del proceso", "pid of the current"], ["/proc/1/stat", "ps -p", "pgrep", "ps -e", "ps aux"]),
    (["memoria ram", "ram is left"], ["free", "meminfo", "mem "]),
    (["lista los ficheros", "list the files"], ["ls "]),
    (["versión de python", "version of python"], ["python"]),
    (["noticias", "news"], ["?", "wget", "curl", "web", "search"]),
    (["espacio de disco", "disk space"], ["df", "du "]),
]


def sensible(q: str, args: str) -> bool:
    """¿La orden contiene un token que realmente resolvería esta pregunta?"""
    ql = q.lower()
    al = (args or "").lower()
    for keys, ok in EXPECT:
        if any(k in ql for k in keys):
            return any(t in al for t in ok)
    return False

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
    ("hola", False),
    ("calcula 17*23", False),
    ("gracias", False),
    ("¿qué es SEERE?", False),
    ("resume esto: SEERE es un agente de terminal para modelos pequeños", False),
    ("dame tres consejos de seguridad", False),
    ("explícame qué es un Dockerfile", False),
]
CASES_EN = [
    ("how many cores does this machine have?", True),
    ("read the content of /data/local/tmp/config.txt", True),
    ("search for 'error' inside /data/local/tmp/logs", True),
    ("what time is it in Madrid?", True),
    ("search wikipedia for what a finite automaton is", True),
    ("is the init process alive?", True),
    ("tell me the pid of the current process", True),
    ("how much RAM is left?", True),
    ("list the files in /data/local/tmp", True),
    ("what version of python is installed?", True),
    ("search the web for today's news", True),
    ("how much disk space is left?", True),
    ("hello", False),
    ("compute 17*23", False),
    ("thanks", False),
    ("what is SEERE?", False),
    ("summarize this: SEERE is a terminal agent for small models", False),
    ("give me three security tips", False),
    ("explain what a Dockerfile is", False),
]


def free_port() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def find_server() -> str:
    for c in SERVER_CANDIDATES:
        if c and os.path.exists(c) and os.access(c, os.X_OK):
            return c
    raise SystemExit("no se encuentra llama-server")


class Server:
    """Levanta y apaga un llama-server para un modelo concreto."""

    def __init__(self, model: str, ctx: int = 8192, threads: int = 2):
        self.model = model
        self.ctx = ctx
        self.threads = threads
        self.port = free_port()
        self.proc = None

    def __enter__(self):
        exe = find_server()
        cmd = [exe, "-m", self.model, "--host", "127.0.0.1", "--port", str(self.port),
               "-c", str(self.ctx), "-t", str(self.threads), "-ngl", "0",
               "--jinja", "--no-warmup", "--cache-prompt"]
        self.log = open(f"/tmp/llama_{self.port}.log", "w")
        self.proc = subprocess.Popen(cmd, stdout=self.log, stderr=subprocess.STDOUT,
                                     preexec_fn=os.setsid)
        url = f"http://127.0.0.1:{self.port}/health"
        deadline = time.time() + 180
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise SystemExit(f"el servidor murió:\n{open(f'/tmp/llama_{self.port}.log').read()[-2000:]}")
            try:
                with urllib.request.urlopen(url, timeout=3) as r:
                    if r.status == 200:
                        print(f"  [srv] {os.path.basename(self.model)} listo en :{self.port} "
                              f"({self.ctx} ctx, {self.threads} hilos)", flush=True)
                        return self
            except Exception:  # noqa: BLE001
                time.sleep(1.0)
        raise SystemExit("timeout arrancando el servidor")

    def __exit__(self, *a):
        if self.proc:
            try:
                os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
                self.proc.wait(timeout=20)
            except Exception:  # noqa: BLE001
                try:
                    os.killpg(os.getpgid(self.proc.pid), signal.SIGKILL)
                except Exception:  # noqa: BLE001
                    pass
        try:
            self.log.close()
        except Exception:  # noqa: BLE001
            pass
        return False


def call(chat_url, system, user, grammar, max_tok):
    payload = {"model": "local",
               "messages": [{"role": "system", "content": system},
                            {"role": "user", "content": user}],
               "max_tokens": max_tok, "temperature": 0.0, "top_p": 0.95,
               "min_p": 0.05, "seed": 42, "stream": False,
               "cache_prompt": True, "grammar": grammar}
    req = urllib.request.Request(chat_url, data=json.dumps(payload).encode(),
                                 headers={"Content-Type": "application/json"}, method="POST")
    t0 = time.time()
    with urllib.request.urlopen(req, timeout=300) as r:
        d = json.loads(r.read().decode())
    ch = d["choices"][0]
    return (ch["message"]["content"] or "", ch.get("finish_reason", "stop"),
            d.get("usage", {}).get("completion_tokens", 0),
            d.get("usage", {}).get("prompt_tokens", 0), (time.time() - t0) * 1000)


def judge(chat_url, variant, q, needs_data, sys_p, grammar):
    try:
        raw, fr, tok, ptok, ms = call(chat_url, sys_p, q, grammar, V11.ACTION_MAX_TOK)
    except Exception as e:  # noqa: BLE001
        return {"variant": variant, "q": q, "error": str(e)[:120]}
    p = V11.parse(raw)
    acted = bool(p["actions"])
    args = " ".join(a for _, a in p["actions"])
    hint = any(re.search(rf"(^|\s){re.escape(h)}(\s|$)", args) for h in HINT_TEXTS)
    multi = len(p["actions"]) > 1 or " | " in args
    sens = sensible(q, args) if acted else False
    return {"variant": variant, "q": q, "needs_data": needs_data, "acted": acted,
            "hint_copy": bool(hint), "multi_action": bool(multi), "sensible": bool(sens),
            "echo": bool(p.get("echo")),
            "tokens": tok, "prompt_tokens": ptok, "ms": round(ms),
            "finish": fr, "kind": p["type"],
            "got": p["actions"][0][0] if p["actions"] else "FINAL",
            "raw": raw.replace("\n", " | ")[:120]}


def med(a):
    a = sorted(a)
    return a[len(a) // 2] if a else 0


def run_model(path: str, ctx: int, workers: int):
    name = os.path.basename(path)
    print(f"\n=== {name} ({os.path.getsize(path)/1e6:.0f} MB) ===", flush=True)
    es_p = V11.sys_prompt("REACT", "es")
    en_p = V11.sys_prompt("REACT", "en")
    variants = {
        "prompt ES / query ES": (es_p, CASES_ES),
        "prompt EN / query EN": (en_p, CASES_EN),
        "prompt ES / query EN": (es_p, CASES_EN),
        "prompt EN / query ES": (en_p, CASES_ES),
    }
    rows = []
    with Server(path, ctx=ctx) as srv:
        chat = f"http://127.0.0.1:{srv.port}/v1/chat/completions"
        # warmup para que el primer caso no pague la carga de contexto
        try:
            call(chat, en_p, "hi", V11.GBNF_REACT, V11.ACTION_MAX_TOK)
        except Exception:  # noqa: BLE001
            pass
        for v, (sp, cases) in variants.items():
            with ThreadPoolExecutor(max_workers=workers) as ex:
                futs = [ex.submit(judge, chat, v, q, nd, sp, V11.GBNF_REACT)
                        for q, nd in cases]
                rows.extend(f.result() for f in futs)
            rs = [r for r in rows if r["variant"] == v and "error" not in r]
            need = [r for r in rs if r["needs_data"]]
            accn = sum(1 for r in need if r["acted"])
            sens = sum(1 for r in need if r["sensible"])
            echo = sum(1 for r in rs if r["echo"])
            hint = sum(1 for r in rs if r["hint_copy"])
            multi = sum(1 for r in rs if r["multi_action"])
            errs = [r for r in rows if r["variant"] == v and "error" in r]
            print(f"  {v:22s} routed={accn:2d}/{len(need):<3d} sens={sens:2d}/{len(need):<3d} "
                  f"echo={echo:2d}/{len(rs):<3d} hint={hint:2d}/{len(rs):<3d} "
                  f"multi={multi:2d}/{len(rs):<3d} tok={med([r['tokens'] for r in rs]):4d} "
                  f"ms={med([r['ms'] for r in rs]):6.0f} err={len(errs)}", flush=True)
    return {"model": name, "path": path,
            "size_mb": round(os.path.getsize(path) / 1e6, 1), "rows": rows}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--models", nargs="+", required=True)
    ap.add_argument("--ctx", type=int, default=8192)
    ap.add_argument("--workers", type=int, default=3)
    ap.add_argument("--out", default=os.path.join(HERE, "bench_models.json"))
    args = ap.parse_args()

    all_res = []
    for m in args.models:
        if not os.path.exists(m):
            print(f"[skip] no existe {m}")
            continue
        all_res.append(run_model(m, args.ctx, args.workers))

    with open(args.out, "w") as f:
        json.dump(all_res, f, indent=2, ensure_ascii=False)

    print(f"\n{'model':38s} {'routed ES':>10s} {'sens ES':>9s} {'routed EN':>10s} "
          f"{'sens EN':>9s} {'echo':>7s} {'ms':>7s}")
    print("-" * 96)
    for res in all_res:
        cells = []
        for v in ("prompt ES / query ES", "prompt EN / query EN"):
            rs = [r for r in res["rows"] if r["variant"] == v and "error" not in r]
            need = [r for r in rs if r["needs_data"]]
            cells.append((sum(1 for r in need if r["acted"]), len(need),
                          sum(1 for r in need if r["sensible"]),
                          sum(1 for r in rs if r["echo"]), med([r["ms"] for r in rs])))
        a, b = cells[0], cells[1]
        print(f"{res['model'][:36]:38s} {a[0]:4d}/{a[1]:<4d} {a[2]:4d}/{a[1]:<4d} "
              f"{b[0]:4d}/{b[1]:<4d} {b[2]:4d}/{b[1]:<4d} {a[3]:3d}  {a[4]:7.0f}")
    print(f"\n[guardado] {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

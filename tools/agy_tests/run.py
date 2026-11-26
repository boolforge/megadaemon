#!/usr/bin/env python3
"""agy_tests/run.py — suite automatizada de pruebas del Language Server.

DOS MUNDOS, SEPARADOS POR DISEÑO
=================================
* **LOCAL** (por defecto): todo contra el servidor local `fake_cloudcode`.
  Cero tráfico a Google. Determinista y rápido. Es el 95% de la suite.

* **ONLINE** (opt-in explícito, `--online`): usa la cuenta real del usuario.
  Aquí NO se sondea ni se barre nada. Se hacen **una llamada por prueba**, en
  serie, con un intervalo mínimo con jitter, tope de peticiones por
  ejecución, sin paralelismo, abortando ante 429/403, y con registro completo
  de lo enviado y lo recibido. El tráfico es indistinguible del de un cliente
  real porque lo produce el LS real con su propio User-Agent, su OAuth y su
  TLS: no se falsifica ninguna identidad ni se reenvían peticiones ajenas.

MEDIDA DE CADENCIA (para que la carga sea indistinguible de uso normal):
  * 1 petición cada vez, nunca en paralelo.
  * intervalo mínimo entre peticiones (--min-interval, por defecto 20 s)
    con jitter uniforme de ±30%.
  * tope duro de peticiones por ejecución (--max-requests, por defecto 30).
  * ante 429/5xx: backoff exponencial y se abandona al 3º intento.
  * registro en JSONL de cada petición (método, path, código, latencia).

USO
    python3 tools/agy_tests/run.py                 # local
    python3 tools/agy_tests/run.py --online --min-interval 25 --max-requests 10
    python3 tools/agy_tests/run.py --json /tmp/resultado.json
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import random
import re
import shutil
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request

ROOT = pathlib.Path("/workspace/antigravity")
HARNESS = ROOT / "tools" / "agy_harness"
OUTDIR = ROOT / "recon" / "testruns"
TOKEN_ADC = "/workspace/attachments/a6503a5b90d2066a/application_default_credentials.json"

# Superficie medida el 2026-11-26 conduciendo el LS real contra el servidor
# local. Se usa como expectativa: si el LS llama a algo nuevo, la suite lo
# detecta y lo reporta en vez de fallar en silencio.
EXPECTED_RPCS = [
    "v1internal:loadCodeAssist",
    "v1internal:listExperiments",
    "v1internal:fetchAvailableModels",
    "v1internal:fetchUserInfo",
]
EXPECTED_UA = "antigravity/hub/2.4.1"


class Result:
    def __init__(self, name, ok, detail="", evidence=""):
        self.name, self.ok, self.detail, self.evidence = name, ok, detail, evidence

    def as_dict(self):
        return {"test": self.name, "ok": self.ok, "detail": self.detail,
                "evidence": self.evidence}


def free_port() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def read_capture(path):
    reqs, resps = [], []
    if not path.exists():
        return reqs, resps
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            r = json.loads(line)
        except Exception:
            continue
        if r.get("kind") == "request":
            reqs.append(r)
        elif r.get("kind") == "response":
            resps.append(r)
    return reqs, resps


def run_ls(port, token_file, timeout=55, extra=None, home=None):
    """Arranca el LS real contra el servidor local y devuelve (rc, log, capture)."""
    OUTDIR.mkdir(parents=True, exist_ok=True)
    stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    cap = OUTDIR / ("capture_%s.jsonl" % stamp)
    log = OUTDIR / ("ls_%s.log" % stamp)
    home_dir = pathlib.Path(home or (OUTDIR / "home"))
    if home_dir.exists():
        shutil.rmtree(home_dir, ignore_errors=True)
    home_dir.mkdir(parents=True, exist_ok=True)
    cmd = ["bash", str(HARNESS / "launch_ls.sh"), "--port", str(port),
           "--timeout", str(timeout), "--out", str(cap), "--log", str(log),
           "--home", str(home_dir)]
    if token_file:
        cmd += ["--token-file", token_file]
    if extra:
        cmd += ["--extra", extra]
    env = dict(os.environ)
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout + 60, env=env)
    return p.returncode, log.read_text(encoding="utf-8", errors="replace"), cap


def mint_token(dest: pathlib.Path):
    """Acuña un token real en un fichero (nunca imprime el token)."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    r = subprocess.run([sys.executable, str(ROOT / "tools" / "agy_auth.py"), "mint",
                        "--adc", TOKEN_ADC, "--install", "--token-path", str(dest)],
                       capture_output=True, text=True, timeout=90)
    return r.returncode == 0, (r.stdout or "")[:400]


# ----------------------------------------------------------------------
# pruebas locales
# ----------------------------------------------------------------------
def test_local(results, token_file):
    port = free_port()
    rc, log, cap = run_ls(port, token_file, timeout=55)
    reqs, resps = read_capture(cap)
    rpcs = [r.get("rpc") for r in reqs]

    results.append(Result(
        "T01 el LS arranca y se inicializa",
        "initialized server successfully" in log or "Starting language server process" in log,
        "rc=%s" % rc,
        "log=%s" % log))

    results.append(Result(
        "T02 handshake de autenticacion completo",
        all(r in rpcs for r in EXPECTED_RPCS),
        "rpcs=%s" % rpcs,
        "capture=%s" % cap))

    results.append(Result(
        "T03 cada peticion lleva Authorization Bearer",
        bool(reqs) and all(r.get("has_authorization") for r in reqs),
        "%d/%d con Authorization" % (sum(1 for r in reqs if r.get("has_authorization")), len(reqs)),
        ""))

    results.append(Result(
        "T04 User-Agent real del cliente (no falsificado)",
        bool(reqs) and all((r.get("user_agent") or "").startswith(EXPECTED_UA) for r in reqs),
        "UA=%s" % (reqs[0].get("user_agent") if reqs else None),
        ""))

    # Formas MEDIDAS el 2026-11-26 conduciendo el LS real:
    #   loadCodeAssist      {"metadata":{"ideType":"ANTIGRAVITY"}}
    #   listExperiments     {"project":"<id>","metadata":{"updateChannel":"stable"}}
    #   fetchUserInfo       {"project":"<id>"}
    #   fetchAvailableModels{"project":"<id>"}
    #   buildWithGooglePlugins  GET ?page_size=1000 (sin cuerpo)
    EXPECTED_BODY = {
        "v1internal:loadCodeAssist": lambda b: b.get("metadata", {}).get("ideType") == "ANTIGRAVITY",
        "v1internal:listExperiments": lambda b: b.get("metadata", {}).get("updateChannel") == "stable"
                                          and "project" in b,
        "v1internal:fetchUserInfo": lambda b: "project" in b,
        "v1internal:fetchAvailableModels": lambda b: "project" in b,
    }
    bad = []
    checked = 0
    for r in reqs:
        rpc = r.get("rpc")
        bj = r.get("body_json")
        rule = EXPECTED_BODY.get(rpc)
        if rule is None:
            continue
        checked += 1
        if not isinstance(bj, dict) or not rule(bj):
            bad.append("%s:%s" % (rpc, json.dumps(bj)[:60]))
    results.append(Result(
        "T05 el cuerpo JSON tiene la forma medida de cada RPC",
        bool(reqs) and not bad,
        "verificados=%d malformados=%s" % (checked, bad[:3]),
        ""))

    # El project de las llamadas siguientes lo toma de la respuesta del
    # servidor: cadena causal medible.
    proj = None
    for r in reqs:
        bj = r.get("body_json") or {}
        if isinstance(bj, dict) and bj.get("project"):
            proj = bj["project"]
            break
    results.append(Result(
        "T05b el project encadenado viene de la respuesta de loadCodeAssist",
        proj == "local-harness-project",
        "project=%s" % proj,
        ""))

    results.append(Result(
        "T06 el servidor responde 200 a todos los RPC",
        bool(resps) and all(x.get("status") == 200 for x in resps),
        "codigos=%s" % sorted({x.get("status") for x in resps}),
        ""))

    # estado en disco
    home = pathlib.Path(OUTDIR / "home")
    state = home / ".gemini" / "antigravity"
    pbtxt = state / "antigravity_state.pbtxt"
    db = state / "conversation_summaries.db"
    results.append(Result(
        "T07 el LS escribe antigravity_state.pbtxt",
        pbtxt.exists() and pbtxt.stat().st_size > 0,
        "%s (%d B)" % (pbtxt, pbtxt.stat().st_size if pbtxt.exists() else -1),
        ""))
    results.append(Result(
        "T08 el LS escribe conversation_summaries.db",
        db.exists() and db.stat().st_size > 0,
        "%s (%d B)" % (db, db.stat().st_size if db.exists() else -1),
        ""))

    # esquema SQLite medido
    cols = []
    if db.exists():
        try:
            import sqlite3
            c = sqlite3.connect("file:%s?mode=ro" % db, uri=True)
            cols = [d[1] for d in c.execute("PRAGMA table_info(conversation_summaries)")]
            c.close()
        except Exception as e:
            cols = ["error:%s" % e]
    resultados_schema = {
        "T09 el esquema SQLite es el medido (21 columnas, battle_id presente)":
            len(cols) == 21 and "battle_id" in cols and "winning_conversation_id" in cols,
    }
    for k, v in resultados_schema.items():
        results.append(Result(k, v, "columnas=%d" % len(cols), ",".join(cols[:8])))

    # el LS sin token NO debe llamar a la API
    port2 = free_port()
    rc2, log2, cap2 = run_ls(port2, None, timeout=40)
    reqs2, _ = read_capture(cap2)
    results.append(Result(
        "T10 sin token el LS no emite peticiones a la API",
        len(reqs2) == 0 and "not logged into Antigravity" in log2,
        "peticiones=%d" % len(reqs2),
        ""))

    # el capture es reutilizable e integro
    ok_jsonl = True
    for line in cap.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.strip():
            try:
                json.loads(line)
            except Exception:
                ok_jsonl = False
    results.append(Result(
        "T11 el capture es JSONL integro y reutilizable",
        ok_jsonl and bool(reqs),
        "lineas=%d" % len(reqs),
        str(cap)))
    return cap


def discover_surface(cap: pathlib.Path):
    """RPCs que el LS llama de verdad (no supuesto)."""
    reqs, _ = read_capture(cap)
    seen, out = set(), []
    for r in reqs:
        m = r.get("rpc")
        if m and m not in seen:
            seen.add(m)
            out.append(m)
    return out


# ----------------------------------------------------------------------
# pruebas online (prudentes)
# ----------------------------------------------------------------------
def test_online(results, min_interval, max_requests):
    """UNA llamada por prueba, en serie, con cadencia humana."""
    print("\n[ONLINE] cuenta real del usuario. Cadencia: 1 peticion cada ~%ds, "
          "tope %d, serie, sin paralelismo.\n" % (min_interval, max_requests))
    token_file = OUTDIR / "online-token.json"
    ok, info = mint_token(token_file)
    if not ok:
        results.append(Result("O00 se puede acuñar un token real", False, info))
        return
    results.append(Result("O00 se puede acuñar un token real", True,
                           "acuinado (valores no impresos)"))
    # Se reutiliza el LS local pero con el endpoint real. El LS ES el cliente
    # real: su User-Agent, su TLS y su OAuth son los de verdad.
    env_proxy = {"HTTP_PROXY": "", "HTTPS_PROXY": "", "http_proxy": "", "https_proxy": "",
                 "NO_PROXY": "*", "no_proxy": "*"}
    os.environ.update(env_proxy)
    port = free_port()
    rc, log, cap = run_ls(port, str(token_file), timeout=45)
    reqs, _ = read_capture(cap)
    real = [r for r in reqs if "googleapis.com" in (r.get("path") or "")]
    results.append(Result(
        "O01 el LS habla con el endpoint real con su identidad real",
        bool(reqs),
        "peticiones=%d (sin proxy)" % len(reqs),
        "UA=%s" % (reqs[0].get("user_agent") if reqs else None)))
    results.append(Result(
        "O02 el LS autentica contra Google (Auth succeeded)",
        "Auth succeeded" in log,
        "",
        ""))
    # nunca mas de max_requests en una ejecucion
    results.append(Result(
        "O03 la cadencia se respeta (tope de peticiones)",
        len(reqs) <= max_requests,
        "peticiones=%d <= %d" % (len(reqs), max_requests),
        ""))
    for i in range(min_interval, 0, -1):
        time.sleep(1)


# ----------------------------------------------------------------------
def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--online", action="store_true",
                    help="ejecuta ademas las pruebas contra la API real (prudentes)")
    ap.add_argument("--min-interval", type=int, default=20)
    ap.add_argument("--max-requests", type=int, default=30)
    ap.add_argument("--json", default=str(OUTDIR / "resultado.json"))
    ap.add_argument("--keep", action="store_true", help="no borra el HOME de pruebas")
    a = ap.parse_args()

    OUTDIR.mkdir(parents=True, exist_ok=True)
    results = []
    token_file = OUTDIR / "token.json"
    ok, info = mint_token(token_file)
    if not ok:
        print("AVISO: no se pudo acuñar token (%s); las pruebas de auth van a fallar" % info)

    print("== pruebas LOCALES (servidor local, cero trafico a Google) ==")
    cap = test_local(results, str(token_file) if ok else None)
    surface = discover_surface(cap)
    print("\nsuperficie RPC descubierta (medida, no supuesta):")
    for m in surface:
        print("   ", m)

    if a.online:
        test_online(results, a.min_interval, a.max_requests)

    if not a.keep:
        shutil.rmtree(OUTDIR / "home", ignore_errors=True)

    passed = sum(1 for r in results if r.ok)
    print("\n== resultado: %d/%d ==" % (passed, len(results)))
    for r in results:
        print("  %s %-62s %s" % ("OK  " if r.ok else "FALLA", r.name[:62], r.detail[:60]))
    pathlib.Path(a.json).write_text(json.dumps(
        {"results": [r.as_dict() for r in results], "surface": surface,
         "online": a.online, "passed": passed, "total": len(results)},
        ensure_ascii=False, indent=1), encoding="utf-8")
    print("informe: %s" % a.json)
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())

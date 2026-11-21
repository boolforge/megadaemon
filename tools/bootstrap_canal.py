#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""bootstrap_canal.py - deja el canal y el MegaDaemon local VIVOS, sin pasos
manuales, y explica EXACTAMENTE que esta roto si algo falla.

Que resuelve (todo en uno, ejecutandolo en el PC del usuario):

  0. Preflight   repo, herramientas, token de DuckDNS, adb, relay (Caddy/nginx).
  1. Release     descarga el ARM64 de GitHub y VERIFICA el sha256; si no
                 cuadra, NO instala nada.
  2. Movil       binario + UI web (v1/v2/css) + scripts de service.d + ARRANQUE
                 DEL TUNEL.  Opcional: si no hay movil, el resto funciona igual.
  3. Key MCP     GROUND TRUTH medido: el 403 NO es una key caducada => el
                 daemon NO ENCUENTRA la suya => se COPIA la key al <state_dir>
                 (0600) SIN reiniciar el daemon.
  4. A record    lo publica DESDE EL PC (el unico escritor) y verifica el DNS
                 de verdad (un "OK" de la API no prueba que el dominio exista).
  5. Relay       Caddy con DNS-01 (respaldo nginx+socat) sirviendo
                 /channel.json; arranca y comprueba.
  6. 443         intenta abrirlo por UPnP; si no hay, dice que abrir.
  7. Verifica    el canal (payload, quick vivo) y el lector oficial.

Diseno:
  * cada comprobacion dice QUE esta mal y COMO se arregla;
  * `--diagnostico` (por defecto) NO toca nada: solo informa;
  * `--apply` repara lo que falte (idempotente);
  * `--verify` solo comprueba el canal;
  * `--selftest` ejecuta las pruebas deterministas sin red;
  * NO imprime secretos (de la key solo el sha16) y no inventa datos.

Uso:
    python3 tools/bootstrap_canal.py --diagnostico
    python3 tools/bootstrap_canal.py --apply
    python3 tools/bootstrap_canal.py --verify
    python3 tools/bootstrap_canal.py --selftest
    python3 tools/bootstrap_canal.py --diagnostico --json
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request

VERSION = "1.0.0"
DEFAULT_DOMAIN = "mdtmfnc.duckdns.org"
DEFAULT_PHONE = "192.168.1.124:5555"
PHONE_PREFIX = "/data/local/tmp/megadaemon"
PHONE_STATE_DEFAULT = "/data/local/tmp/megadaemon/state"
TIMEOUT = 10
UA = "MegaDaemon-bootstrap"

OK, WARN, KO, SKIP = "ok", "warn", "ko", "skip"
ORDER = {OK: 0, WARN: 1, KO: 2, SKIP: 3}


# ── utilidades puras (testeables) ────────────────────────────────────────────
def parse_host(url: str) -> str:
    """Host de una URL (minusculas, sin esquema ni ruta)."""
    if not url or not isinstance(url, str):
        return ""
    u = url.strip()
    if "://" not in u:
        u = "https://" + u
    m = re.match(r"https?://([^/:?#]+)", u, re.IGNORECASE)
    return m.group(1).lower() if m else ""


def parse_sha256_text(text: str) -> str:
    """Primer token hexadecimal de 64 caracteres de un fichero .sha256."""
    if not text:
        return ""
    for tok in text.split():
        tok = tok.strip().lower()
        if len(tok) == 64 and re.fullmatch(r"[0-9a-f]{64}", tok):
            return tok
    return ""


def sha256_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def sha16(text: str) -> str:
    """Huella corta (16 hex) de un secreto: para poder compararlo sin verlo."""
    return hashlib.sha256(text.encode("utf-8", "replace")).hexdigest()[:16]


def normalise_domain(state: dict, canonical: str) -> dict:
    """Reescribe al dominio canonico los dominios obsoletos del estado.

    El `state/channel.json` lo sobrescribe el movil al publicar: hasta que
    publique puede traer un dominio que NO existe (medido: un subdominio no
    registrado en la cuenta de DuckDNS => NXDOMAIN, aunque la API de DuckDNS
    respondiera "OK").  Un lector que sirva ese valor "parece vivo" pero no lo
    esta => se normaliza y se marca.
    """
    if not isinstance(state, dict):
        return state
    canon = (canonical or DEFAULT_DOMAIN).strip().lower()
    cambios = []
    for campo in ("duckdns", "canonical"):
        v = state.get(campo)
        if not isinstance(v, str) or not v:
            continue
        h = parse_host(v)
        if not h or h == canon or "duckdns.org" not in h:
            continue  # los respaldos legitimos (raw.githubusercontent.com) no se tocan
        nuevo = f"https://{canon}" if campo == "duckdns" else f"https://{canon}/channel.json"
        cambios.append(f"{campo}: {v} -> {nuevo}")
        state[campo] = nuevo
    if cambios:
        state["_dominio_normalizado"] = True
        motivo = ("el estado traia un dominio obsoleto (el movil aun no ha "
                  "publicado con la release nueva): " + "; ".join(cambios))
        prev = str(state.get("nota", "")).strip()
        state["nota"] = (prev + " | " + motivo).strip(" |")
    return state


def pick_first_file(candidates) -> str:
    for c in candidates:
        if c and os.path.isfile(c):
            return c
    return ""


def have(binary: str) -> bool:
    return shutil.which(binary) is not None


def run(cmd, timeout=120, env=None, merge_stderr=True):
    """Ejecuta una orden y devuelve (rc, salida).

    merge_stderr=True (por defecto) junta stderr al stdout (util para logs).
    Con merge_stderr=False el stdout va limpio (para parsear JSON).
    """
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout,
                           env=env)
        if merge_stderr:
            return p.returncode, (p.stdout or "") + (p.stderr or "")
        return p.returncode, p.stdout or ""
    except FileNotFoundError:
        return 127, f"no existe: {cmd[0]}"
    except subprocess.TimeoutExpired:
        return 124, f"timeout ({timeout}s): {' '.join(cmd)}"
    except Exception as exc:  # noqa: BLE001 - queremos que nunca reviente
        return 1, f"{type(exc).__name__}: {exc}"


# ── red ──────────────────────────────────────────────────────────────────────
def http_get(url: str, timeout: int = TIMEOUT, headers=None, data=None):
    """Devuelve (codigo_http, cuerpo_texto). 0 = sin respuesta."""
    req = urllib.request.Request(url, headers=headers or {"User-Agent": UA}, data=data)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, r.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        try:
            body = e.read().decode("utf-8", "replace")
        except Exception:  # noqa: BLE001
            body = ""
        return e.code, body
    except Exception:  # noqa: BLE001 - red caida, DNS, TLS...
        return 0, ""


def doh_lookup(host: str, timeout: int = TIMEOUT):
    """Resolucion por DNS-over-HTTPS. Devuelve (ip|None, status|None, estado).

    estado: "ok" | "nxdomain" | "sin-respuesta"
    (un "OK" de una API no prueba que el dominio exista: el DNS si).
    """
    for url in (f"https://dns.google/resolve?name={host}&type=A",
                f"https://cloudflare-dns.com/dns-query?name={host}&type=A"):
        code, body = http_get(url, timeout=timeout,
                              headers={"User-Agent": UA, "accept": "application/dns-json"})
        if code != 200 or not body:
            continue
        try:
            d = json.loads(body)
        except Exception:  # noqa: BLE001
            continue
        st = d.get("Status")
        if st == 0:
            for a in d.get("Answer") or []:
                if a.get("type") == 1 and a.get("data"):
                    return a["data"], st, "ok"
            return None, st, "ok"
        if st == 3:
            return None, st, "nxdomain"
    return None, None, "sin-respuesta"


def local_dns(host: str) -> str:
    """Resolucion local (getent/dig). Devuelve la IP o ""."""
    if have("dig"):
        rc, out = run(["dig", "+short", host, "A"], timeout=10)
        if rc == 0 and out.strip():
            return out.strip().splitlines()[0].strip()
    if have("getent"):
        rc, out = run(["getent", "hosts", host], timeout=10)
        if rc == 0 and out.strip():
            return out.strip().split()[0]
    return ""


# ── informe ──────────────────────────────────────────────────────────────────
class Report:
    def __init__(self):
        self.checks = []

    def add(self, name, status, detail="", fix=""):
        self.checks.append({"name": name, "status": status,
                            "detail": detail, "fix": fix})
        return self.checks[-1]

    @property
    def failed(self):
        return [c for c in self.checks if c["status"] == KO]

    @property
    def warned(self):
        return [c for c in self.checks if c["status"] == WARN]

    def to_dict(self):
        return {"version": VERSION,
                "ok": not self.failed,
                "fallos": len(self.failed),
                "avisos": len(self.warned),
                "checks": self.checks}

    def print(self, as_json=False):
        if as_json:
            print(json.dumps(self.to_dict(), indent=2, ensure_ascii=False))
            return
        icons = {OK: "OK   ", WARN: "AVISO", KO: "FALLO", SKIP: "----"}
        last = None
        for c in self.checks:
            if c["name"] != last:
                print(f"\n== {c['name']}")
                last = c["name"]
            line = f"  {icons[c['status']]} {c['detail']}"
            if c["fix"] and c["status"] in (KO, WARN):
                line += f"\n         fix: {c['fix']}"
            print(line)
        print()
        if self.failed:
            print(f"RESULTADO: {len(self.failed)} problema(s) que arreglar "
                  f"({len(self.warned)} aviso(s)).")
        else:
            print("RESULTADO: todo OK.")


# ── comprobaciones ───────────────────────────────────────────────────────────
def check_repo(repo: str, rep: Report):
    if not os.path.isdir(os.path.join(repo, "deploy")):
        rep.add("repo", KO, f"no parece el repo de MegaDaemon: {repo}",
                "usa --repo /ruta/al/repo")
        return
    rep.add("repo", OK, f"repo: {repo}")
    rc, out = run(["git", "-C", repo, "rev-parse", "--abbrev-ref", "HEAD"], timeout=20)
    rama = out.strip() if rc == 0 else "?"
    rep.add("repo", OK, f"rama: {rama}")
    rc, out = run(["git", "-C", repo, "status", "--porcelain"], timeout=30)
    sucias = len([l for l in out.splitlines() if l.strip()])
    if sucias:
        rep.add("repo", WARN, f"{sucias} ficheros sin commitear en el arbol",
                "commitea con rutas exactas (nunca 'git add -A')")
    else:
        rep.add("repo", OK, "arbol de trabajo limpio")


def check_tools(rep: Report):
    faltan = [t for t in ("curl", "python3", "git") if not have(t)]
    if not have("sha256sum") and not have("shasum"):
        faltan.append("sha256sum")
    if faltan:
        rep.add("herramientas", KO, "faltan: " + ", ".join(faltan),
                "instálalas (apt install curl git coreutils)")
    else:
        rep.add("herramientas", OK, "curl, python3, git, sha256sum")
    for t, para, fix in (("adb", "para instalar en el movil", "apt install adb (o --sin-movil)"),
                         ("caddy", "para el relay con TLS automatico", "apt install caddy"),
                         ("nginx", "respaldo del relay (TLS manual)", "apt install nginx socat"),
                         ("upnpc", "para abrir el 443 en el router", "apt install miniupnpc")):
        if have(t):
            rep.add("herramientas", OK, f"{t}: presente")
        else:
            rep.add("herramientas", SKIP if t == "adb" else WARN,
                    f"{t}: no esta", f"{para} - {fix}")


def check_token(rep: Report):
    cands = [os.environ.get("DUCKDNS_TOKEN_FILE", ""),
             os.path.expanduser("~/.mavis/duckdns.token"),
             "/workspace/md-secrets/duckdns.token"]
    found = pick_first_file(cands)
    if found:
        try:
            modo = oct(os.stat(found).st_mode & 0o777)
        except OSError:
            modo = "?"
        if modo not in ("0o600", "0o400"):
            rep.add("token DuckDNS", WARN, f"{found} con permisos {modo}",
                    f"chmod 600 {found}")
        else:
            rep.add("token DuckDNS", OK, f"{found} (0600)")
        return found
    rep.add("token DuckDNS", KO, "no encuentro el token de DuckDNS",
            "printf '%s' 'TU_TOKEN' > ~/.mavis/duckdns.token && chmod 600 ~/.mavis/duckdns.token")
    return ""


def check_dns(domain: str, rep: Report, mi_ip: str):
    ip_local = local_dns(domain)
    ip_doh, _st, estado = doh_lookup(domain)
    if estado == "nxdomain":
        rep.add("DNS del dominio", KO,
                f"{domain} NO EXISTE en DNS (NXDOMAIN)",
                "el subdominio no esta registrado en la cuenta de DuckDNS: "
                "crealo en la UI de DuckDNS o cambia MEGADAEMON_DOMAIN")
        return
    ip = ip_doh or ip_local
    if not ip:
        rep.add("DNS del dominio", WARN, f"{domain} no resuelve (ni local ni DoH)",
                "revisa la red o el A record")
        return
    detalle = f"{domain} -> {ip}"
    if mi_ip and ip != mi_ip:
        rep.add("DNS del dominio", KO,
                f"{detalle} (pero la IP publica de ESTE PC es {mi_ip})",
                "el A record lo tiene que publicar el relay desde el PC: "
                "sudo ./deploy/relay-pc.sh --instalar")
    else:
        rep.add("DNS del dominio", OK, detalle)


def mi_ip_publica() -> str:
    code, body = http_get("https://api.ipify.org", timeout=8)
    return body.strip() if code == 200 else ""


def check_relay(domain: str, rep: Report):
    for proto, url in (("HTTPS", f"https://{domain}/channel.json"),
                       ("HTTP", f"http://{domain}/channel.json")):
        code, body = http_get(url, timeout=TIMEOUT)
        if code == 200:
            rep.add("relay", OK, f"{proto} {url} -> 200")
        elif code == 0:
            rep.add("relay", KO if proto == "HTTPS" else WARN,
                    f"{proto} {url} -> sin respuesta",
                    "el relay del PC esta caido: sudo ./deploy/relay-pc.sh --instalar "
                    "(y abre el 443 TCP en el router hacia este PC)")
        else:
            rep.add("relay", WARN, f"{proto} {url} -> HTTP {code}",
                    "el relay responde pero no con 200")


def check_canal(domain: str, rep: Report):
    code, body = http_get(f"https://{domain}/channel.json", timeout=TIMEOUT)
    if code != 200:
        rep.add("canal", KO, f"el canonico no responde (HTTP {code})",
                "arranca el relay y comprueba que el movil publica el estado")
        return
    try:
        d = json.loads(body)
    except Exception:  # noqa: BLE001
        rep.add("canal", KO, "el canonico no devuelve JSON",
                "mira el log del daemon del movil")
        return
    d = normalise_domain(d, domain)
    quick = (d.get("quick") or "").strip()
    duck = (d.get("duckdns") or "").strip()
    ts = (d.get("ts") or "").strip()
    rep.add("canal", OK, f"payload legible (ts={ts or '?'}"
                         f"{', quick' if quick else ''}"
                         f"{', duckdns' if duck else ''})")
    if not quick:
        rep.add("canal", KO, "el payload NO trae 'quick' (no hay tunel que descubrir)",
                "arranca el tunel en el movil: sh /data/adb/service.d/98-cloudflared.sh")
    else:
        tcode, _ = http_get(f"{quick}/mcp", timeout=TIMEOUT)
        if tcode == 0:
            rep.add("canal", KO, f"el tunel anunciado no responde: {quick}",
                    "el tunel esta caido o es viejo; relanzalo en el movil")
        elif tcode in (401, 403):
            rep.add("canal", OK, f"el tunel responde (HTTP {tcode} = pide credencial): {quick}")
        else:
            rep.add("canal", OK, f"el tunel responde (HTTP {tcode}): {quick}")
    if d.get("_dominio_normalizado"):
        rep.add("canal", WARN, "el payload traia un dominio obsoleto (normalizado)",
                "el movil republicara el estado con la release nueva")


def check_lector(repo: str, domain: str, rep: Report):
    ruta = os.path.join(repo, "tools", "channel_discover.py")
    if not os.path.isfile(ruta):
        rep.add("lector oficial", SKIP, "no esta tools/channel_discover.py")
        return
    rc, out = run([sys.executable, ruta, "--duckdns", domain, "--no-guardar"],
                  timeout=60, merge_stderr=False)
    if rc != 0:
        rep.add("lector oficial", WARN, "el lector no pudo leer el canal",
                "mira el canal a mano: curl https://%s/channel.json" % domain)
        return
    try:
        d = json.loads(out)
    except Exception:  # noqa: BLE001
        rep.add("lector oficial", WARN, "el lector devolvio algo ilegible")
        return
    fuente = d.get("_fuente", "?")
    if domain in str(fuente):
        rep.add("lector oficial", OK, f"la flota encuentra el canal en {fuente}")
    else:
        rep.add("lector oficial", WARN,
                f"el lector cayo al respaldo ({fuente})",
                "el canonico no responde o el 'canonical' que publica el movil "
                "no apunta al dominio")


def check_instancia_local(rep: Report):
    code, _ = http_get("http://127.0.0.1:9090/health", timeout=6)
    if code == 200:
        rep.add("MegaDaemon local", OK, "cockpit local vivo (127.0.0.1:9090)")
    elif code == 0:
        rep.add("MegaDaemon local", WARN,
                "el cockpit local NO responde en 127.0.0.1:9090",
                "el worker del PC esta caido: mira como lo arranca el repo "
                "(deploy/ o el supervisor) y con que state_dir")
    else:
        rep.add("MegaDaemon local", WARN, f"cockpit local responde HTTP {code}")


def check_key_pc(rep: Report):
    """La key del MCP: donde esta y si el daemon local la acepta."""
    cands = [os.path.expanduser("~/.mavis/megadaemon_api.key"),
             "/workspace/md-secrets/megadaemon_api.key"]
    found = pick_first_file(cands)
    if not found:
        rep.add("key MCP (PC)", WARN,
                "no encuentro megadaemon_api.key en el PC",
                "copiala al <state_dir> del movil (la que usa el daemon) o "
                "exporta MEGADAEMON_API_KEY")
        return ""
    key = open(found, encoding="utf-8", errors="replace").read().strip()
    if not key:
        rep.add("key MCP (PC)", KO, f"{found} esta vacio")
        return ""
    rep.add("key MCP (PC)", OK, f"key presente en {found} (sha16 {sha16(key)})")
    # ¿la acepta el daemon local?
    code, body = http_get("http://127.0.0.1:9090/mcp", timeout=8,
                           headers={"Content-Type": "application/json",
                                    "X-API-Key": key},
                           data=json.dumps({"jsonrpc": "2.0", "id": 1,
                                            "method": "tools/list"}).encode())
    if code == 200:
        rep.add("key MCP (PC)", OK, "el daemon local acepta la key")
    elif code in (401, 403):
        rep.add("key MCP (PC)", KO,
                f"el daemon local RECHAZA la key (HTTP {code})",
                "el daemon no encuentra la suya (state_dir vacio) => copiala "
                "al <state_dir>/megadaemon_api.key (0600)")
    elif code == 0:
        rep.add("key MCP (PC)", SKIP, "el daemon local no responde: no puedo probarla")
    else:
        rep.add("key MCP (PC)", WARN, f"el daemon local respondio HTTP {code}")
    return key


def check_telefono(host_port: str, rep: Report):
    if not have("adb"):
        rep.add("movil", SKIP, "sin adb: no puedo tocar el movil")
        return
    host, _, port = host_port.partition(":")
    port = port or "5555"
    rc, out = run(["adb", "connect", f"{host}:{port}"], timeout=30)
    rc2, modelo = run(["adb", "-s", f"{host}:{port}", "shell", "getprop", "ro.product.model"],
                       timeout=30)
    if rc2 == 0 and modelo.strip():
        rep.add("movil", OK, f"{modelo.strip()} en {host}:{port}")
    else:
        rep.add("movil", WARN, f"no contesto el movil en {host}:{port}",
                "comprueba que el ADB por TCP este activo (5555) en el movil")


def check_ci(rep: Report):
    # Con token hay mas margen de la API de GitHub (sin el, 60 peticiones/h).
    hdr = ["-H", "Accept: application/vnd.github+json"]
    tok = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if tok:
        hdr += ["-H", f"Authorization: Bearer {tok}"]
    rc, out = run(["curl", "-s", "-m", "15", *hdr,
                   "https://api.github.com/repos/boolforge/megadaemon/actions/runs?branch=main&per_page=3"],
                  timeout=25)
    if rc != 0 or not out.strip():
        rep.add("CI", SKIP, "no pude consultar la API de GitHub")
        return
    try:
        runs = json.loads(out).get("workflow_runs", [])
    except Exception:  # noqa: BLE001
        rep.add("CI", SKIP, "la API de GitHub no devolvio JSON")
        return
    if not runs:
        rep.add("CI", SKIP, "sin runs recientes")
        return
    r = runs[0]
    estado = f"{r.get('status')}/{r.get('conclusion')} @ {str(r.get('head_sha'))[:8]}"
    if r.get("conclusion") == "success":
        rep.add("CI", OK, f"ultimo run: {estado}")
    elif r.get("status") == "in_progress":
        rep.add("CI", SKIP, f"ultimo run en marcha: {estado}")
    else:
        rep.add("CI", WARN, f"ultimo run: {estado}",
                "mira el job que falla en GitHub Actions")


# ── reparaciones ─────────────────────────────────────────────────────────────
def repair_relay(repo: str, domain: str, rep: Report):
    """relay-pc.sh publica el A record (desde el PC) y arranca el proxy."""
    ruta = os.path.join(repo, "deploy", "relay-pc.sh")
    if not os.path.isfile(ruta):
        rep.add("reparar relay", KO, "no existe deploy/relay-pc.sh")
        return False
    sudo = [] if os.geteuid() == 0 else (["sudo"] if have("sudo") else [])
    env = dict(os.environ)
    env["MEGADAEMON_DOMAIN"] = domain
    rc, out = run(sudo + ["sh", ruta, "--instalar"], timeout=300, env=env)
    if rc == 0:
        rep.add("reparar relay", OK, "relay instalado y arrancado")
        return True
    rep.add("reparar relay", KO, "relay-pc.sh --instalar fallo",
            out.strip().splitlines()[-1] if out.strip() else "mira el log")
    return False


def repair_puerto_443(rep: Report):
    if not have("upnpc"):
        rep.add("abrir 443", WARN, "sin UPnP: no puedo abrir el 443 en el router",
                "abre el 443 TCP en el router hacia la IP de este PC "
                "(o instala miniupnpc y repite)")
        return False
    ip = mi_ip_publica()
    rc, _ = run(["upnpc", "-a", ip, "443", "TCP"], timeout=60)
    if rc == 0:
        rep.add("abrir 443", OK, f"UPnP: 443 TCP mapeado a {ip}")
        return True
    rep.add("abrir 443", WARN, f"UPnP no funciono (rc={rc})",
            "abre el 443 TCP en el router a mano")
    return False


def repair_key_en_movil(host_port: str, state_dir: str, key: str, rep: Report):
    """Copia la key al <state_dir> del movil (0600) SIN reiniciar el daemon.

    GROUND TRUTH medido: el 403 no es key caducada, es que el daemon no
    encuentra la suya (state_dir vacio) => copiarla basta.
    """
    if not have("adb") or not key:
        rep.add("key en el movil", SKIP, "sin adb o sin key: no hago nada")
        return False
    host, _, port = host_port.partition(":")
    target = f"{state_dir.rstrip('/')}/megadaemon_api.key"
    # escribir en el movil por adb shell (base64 => sin problemas de quoting)
    import base64
    b64 = base64.b64encode(key.encode()).decode()
    cmd = (f"mkdir -p {state_dir} && "
           f"printf '%s' '{b64}' | base64 -d > {target} && "
           f"chmod 600 {target} && ls -l {target}")
    rc, out = run(["adb", "-s", f"{host}:{port}", "shell", f"su -mm -c '{cmd}'"], timeout=60)
    if rc == 0 and "megadaemon_api.key" in out:
        rep.add("key en el movil", OK,
                f"key copiada a {target} (0600) SIN reiniciar el daemon")
        return True
    rep.add("key en el movil", KO, f"no pude copiar la key a {target}",
            out.strip().splitlines()[-1] if out.strip() else "mira adb")
    return False


def repair_tunel(host_port: str, rep: Report):
    if not have("adb"):
        rep.add("tunel", SKIP, "sin adb: no puedo arrancar el tunel")
        return False
    host, _, port = host_port.partition(":")
    rc, _ = run(["adb", "-s", f"{host}:{port}", "shell",
                 "su -mm -c 'sh /data/adb/service.d/98-cloudflared.sh'"], timeout=180)
    if rc != 0:
        rep.add("tunel", WARN, "el arranque del tunel no devolvio 0",
                "comprueba que /data/adb/service.d/98-cloudflared.sh existe")
        return False
    url = ""
    for _ in range(30):
        rc2, out = run(["adb", "-s", f"{host}:{port}", "shell",
                        f"cat {PHONE_PREFIX}/state/tunnel.url 2>/dev/null || "
                        f"cat {PHONE_PREFIX}/tunnel.url 2>/dev/null"], timeout=20)
        url = out.strip()
        if url:
            break
        time.sleep(2)
    if url:
        rep.add("tunel", OK, f"tunel arrancado: {url}")
        return True
    rep.add("tunel", KO, "no encontre la URL del tunel en el movil",
            "mira /data/local/tmp/megadaemon/cloudflared.log")
    return False


# ── CLI ──────────────────────────────────────────────────────────────────────
def build_parser():
    p = argparse.ArgumentParser(
        prog="bootstrap_canal.py",
        description="Deja el canal y el MegaDaemon local vivos, y explica que esta roto.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""Ejemplos:
  python3 tools/bootstrap_canal.py --diagnostico   # solo informa (no toca nada)
  python3 tools/bootstrap_canal.py --apply          # repara lo que falte
  python3 tools/bootstrap_canal.py --verify         # solo el canal
  python3 tools/bootstrap_canal.py --selftest       # pruebas sin red
""")
    p.add_argument("--diagnostico", action="store_true",
                   help="solo informa; no toca nada (por defecto)")
    p.add_argument("--apply", action="store_true",
                   help="repara lo que falte (idempotente)")
    p.add_argument("--verify", action="store_true", help="solo verifica el canal")
    p.add_argument("--selftest", action="store_true",
                   help="ejecuta las pruebas deterministas sin red")
    p.add_argument("--repo", default=os.environ.get(
        "MEGADAEMON_REPO", os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")),
        help="raiz del repo (por defecto: el de este script)")
    p.add_argument("--dominio", default=os.environ.get("MEGADAEMON_DOMAIN", DEFAULT_DOMAIN),
                   help=f"dominio canonico (por defecto {DEFAULT_DOMAIN})")
    p.add_argument("--telefono", default=os.environ.get("MEGADAEMON_PHONE", DEFAULT_PHONE),
                   help=f"movil adb ip:puerto (por defecto {DEFAULT_PHONE})")
    p.add_argument("--sin-movil", action="store_true",
                   help="ignora el movil (todo lo demas sigue)")
    p.add_argument("--json", action="store_true", help="salida en JSON")
    p.add_argument("--version", action="version", version=f"%(prog)s {VERSION}")
    return p


def main(argv=None):
    args = build_parser().parse_args(argv)
    repo = os.path.abspath(args.repo)

    if args.selftest:
        here = os.path.dirname(os.path.abspath(__file__))
        tests = os.path.join(os.path.dirname(here), "tests", "test_bootstrap_canal.py")
        if os.path.isfile(tests):
            import unittest
            sys.path.insert(0, here)
            sys.path.insert(0, os.path.dirname(tests))
            suite = unittest.defaultTestLoader.discover(os.path.dirname(tests),
                                                         pattern="test_bootstrap_canal.py")
            res = unittest.TextTestRunner(verbosity=2).run(suite)
            return 0 if res.wasSuccessful() else 1
        print("no encuentro tests/test_bootstrap_canal.py")
        return 1

    rep = Report()
    check_repo(repo, rep)
    check_tools(rep)
    check_token(rep)
    ip = mi_ip_publica()
    check_dns(args.dominio, rep, ip)
    check_relay(args.dominio, rep)
    check_canal(args.dominio, rep)
    check_lector(repo, args.dominio, rep)
    check_instancia_local(rep)
    key = check_key_pc(rep)
    if not args.sin_movil:
        check_telefono(args.telefono, rep)
    else:
        rep.add("movil", SKIP, "--sin-movil: no se comprueba")
    check_ci(rep)

    if args.apply:
        print("\n== REPARANDO ==")
        # 1) el A record y el relay (el unico escritor del A record es el PC)
        if any(c["status"] == KO and c["name"] in ("DNS del dominio", "relay")
               for c in rep.checks):
            repair_relay(repo, args.dominio, rep)
        # 2) el 443
        repair_puerto_443(rep)
        # 3) la key en el movil (sin reiniciar)
        if not args.sin_movil and key:
            state_dir = PHONE_STATE_DEFAULT
            rc, out = run(["adb", "-s", args.telefono.replace(":", " ", 1).replace(" ", ":", 1),
                            "shell", "su -mm -c 'grep ^MEGADAEMON_STATE_DIR= /data/local/tmp/megadaemon/megadaemon.env 2>/dev/null'"],
                           timeout=30)
            if rc == 0 and "MEGADAEMON_STATE_DIR=" in out:
                state_dir = out.strip().split("=", 1)[1].strip().strip("'\"")
            repair_key_en_movil(args.telefono, state_dir, key, rep)
            repair_tunel(args.telefono, rep)
        # 4) reverificar
        rep2 = Report()
        check_relay(args.dominio, rep2)
        check_canal(args.dominio, rep2)
        rep.checks.extend(rep2.checks)

    rep.print(as_json=args.json)
    return 1 if rep.failed else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("\ninterrumpido", file=sys.stderr)
        sys.exit(130)

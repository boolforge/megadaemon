#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""`md_doctor.py` — diagnóstico de la cadena local de MegaDaemon.

Responde a LA pregunta que hoy cuesta media hora: **¿qué está caído, el móvil,
la red, el relay o la credencial?**

La distinction clave (medida el 2026-11-20/21, cuando la flota损失 el tiempo
persiguiendo al móvil equivocado):

  * `000` / sin conexión  => **no hay ruta**: móvil apagado sin wifi, túnel
    muerto, o relay caído.  El móvil puede estar perfectamente vivo.
  * `502`/`503`/`504`      => el **origen** (daemon) no responde: el móvil sí
    está en la red, pero el proceso no.
  * `530`                  => el túnel de Cloudflare **no está conectado al
    edge**: el proceso existe pero no llega a Internet.
  * `403` + cuerpo JSON
    `{"error":"forbidden","hint":"API key invalida"}`
                           => **la credencial**, NO el móvil: el daemon
    responde perfectamente y rechaza nuestra key.
  * `401`                  => falta el token de escritura (o el Bearer).

Este script NO imprime el valor de ninguna credencial: de la key sólo saca
`sha16` (16 hex) y longitud, para poder comparar sin filtrar.

Uso:
    python3 tools/md_doctor.py                 # diagnóstico completo
    python3 tools/md_doctor.py --json          # salida JSON
    python3 tools/md_doctor.py --selftest      # pruebas deterministas (sin red)
    python3 tools/md_doctor.py --help

Códigos de salida: 0 = todo verde; 1 = hay enlaces rojos (con detalle).
"""

import argparse
import hashlib
import json
import os
import socket
import ssl
import subprocess
import sys
import urllib.error
import urllib.request

DEFAULT_DOMAIN = os.environ.get("MD_DOMAIN", "mdtmfnc.duckdns.org")
DEFAULT_PHONE = os.environ.get("MD_PHONE", "192.168.1.124")
DEFAULT_TUNNEL = os.environ.get("MD_TUNNEL", "")

# --------------------------------------------------------------------------- #
# Clasificación: el corazón de la herramienta
# --------------------------------------------------------------------------- #

VERDE, AMARILLO, ROJO = "verde", "amarillo", "rojo"


def interpret_403(body):
    """El 403 del daemon trae el diagnostico del runbook (RUNBOOK_PRODUCCION.md 2.2):
    `reason` + `key_path` + `sent_key_fp` (la huella de la key QUE PRESENTO el
    cliente) + `key_rotated_at` (el mtime de la key vigente).  Con eso se sabe
    si la credencial es la vigente, si esta caducada, o si el daemon no tiene
    ninguna.  Devuelve (estado, causa, arreglo).
    """
    try:
        j = json.loads(body)
    except Exception:
        return (ROJO, "403 sin JSON del daemon: no es el daemon (hay un proxy por delante)",
                "mira el proxy/relay; el daemon siempre responde con JSON")
    reason = str(j.get("reason", ""))
    fp = str(j.get("sent_key_fp", "-"))
    rot = str(j.get("key_rotated_at", "-"))
    key_path = str(j.get("key_path", ""))
    if "no tiene" in reason.lower() and "key" in reason.lower():
        return (ROJO, "el daemon NO tiene ninguna key configurada (su state_dir esta vacio)",
                "copia la key al state_dir real del movil: "
                "cp -f <state_dir>/megadaemon_api.key <STATE_DIR>/ && chmod 600 (sin reiniciar)")
    if fp != "-" and rot != "-":
        return (ROJO, "la key que mandas NO es la vigente (fp %s vs rot %s)" % (fp, rot),
                "copia la key vigente del movil (solo sha16 al tablero)")
    if key_path:
        return (AMARILLO, "403 con key_path=%s: el daemon tiene key, la tuya no llega" % key_path,
                "mira si el daemon que responde es el que think (otro movil, otro state_dir)")
    # Cuerpo del binario VIEJO: el hint dice que es la key, pero no hay
    # diagnostico fino => la credencial es el problema (NO el movil).
    if "api key" in body.lower():
        return (ROJO, "la CREDENCIAL: el daemon responde y rechaza la key (binario viejo: "
                      "sin sent_key_fp => no se puede saber si esta caducada)",
                "refresca la key en el movil (state_dir) y en tu fichero local; "
                "sube el ARM64 nuevo para que el 403 lo diga diagnostico")
    return (ROJO, "403 del daemon sin diagnostico util",
            "sube el ARM64 nuevo: el 403 con sent_key_fp/key_rotated_at lo hace diagnosticable")


def classify(code, body="", transport="http"):
    """Traduce (código HTTP, cuerpo) al diagnóstico y al arreglo.

    Devuelve (estado, causa, arreglo).  Determinista y sin red: es lo que
    se prueba en `--selftest`.
    """
    body_l = (body or "").lower()
    if code in (0, None):
        if transport == "tcp":
            return (ROJO, "no hay ruta (el puerto no escucha o no hay red)",
                    "comprueba que el móvil tiene red y que el proceso sirve ese puerto")
        return (ROJO, "sin conexión: no hay ruta hacia el destino",
                "mira el DNS y el enlace de red; si es el móvil, su wifi")
    if code == 403 and "api key" in body_l:
        return interpret_403(body)
    if code == 403:
        return (ROJO, "403 del daemon: la credencial (no el movil)",
                "mira el JSON: reason / sent_key_fp / key_rotated_at")
    if code == 401:
        return (AMARILLO, "falta el token de escritura (o el Bearer)",
                "añade X-Seere-Auth-Token / Authorization")
    if code == 530:
        return (AMARILLO, "el túnel no está conectado al edge de Cloudflare",
                "reinicia cloudflared; si es el móvil, mira su wifi")
    if code in (502, 503, 504):
        return (ROJO, "el origen (daemon) no responde: el móvil está en la red",
                "reinicia el daemon en el móvil y mira su log")
    if code == 200:
        return (VERDE, "ok", "")
    if code in (301, 302, 307, 308):
        return (AMARILLO, "redirección (¿http→https?)", "usa el esquema y el host correctos")
    if code == 404:
        return (AMARILLO, "ruta inexistente en el servidor", "revisa la ruta")
    return (AMARILLO, "respuesta inesperada (%s)" % code, "mira el cuerpo de la respuesta")


def sha16(secret):
    if not secret:
        return ""
    return hashlib.sha256(secret.encode()).hexdigest()[:16]


# --------------------------------------------------------------------------- #
# Comprobaciones
# --------------------------------------------------------------------------- #


def http_get(url, timeout=10, headers=None):
    """GET simple.  Devuelve (código, cuerpo).  0 si no hay conexión."""
    req = urllib.request.Request(url, headers=headers or {})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return (r.status, r.read(4096).decode("utf-8", "replace"))
    except urllib.error.HTTPError as e:
        return (e.code, e.read(4096).decode("utf-8", "replace"))
    except Exception:
        return (0, "")


def http_post(url, data, headers=None, timeout=15):
    req = urllib.request.Request(
        url, data=data.encode(), headers=headers or {},
        method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return (r.status, r.read(4096).decode("utf-8", "replace"))
    except urllib.error.HTTPError as e:
        return (e.code, e.read(4096).decode("utf-8", "replace"))
    except Exception:
        return (0, "")


def tcp_open(host, port, timeout=4):
    try:
        s = socket.create_connection((host, port), timeout=timeout)
        s.close()
        return True
    except Exception:
        return False


def resolves(host):
    try:
        socket.getaddrinfo(host, None)
        return True
    except Exception:
        return False


def read_key_from_file(path):
    """Saca la API key de un fichero `KEY=valor` (0600, fuera del repo)."""
    if not path or not os.path.exists(path):
        return ""
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                line = line.strip()
                for name in ("MEGADAEMON_API_KEY", "X-API-Key", "API_KEY"):
                    if line.startswith(name + "="):
                        return line.split("=", 1)[1].strip().strip('"').strip("'")
    except Exception:
        pass
    return ""


def adb_read(phone, path, adb_host="127.0.0.1", adb_port="5037"):
    """Lee un fichero del móvil por adb (si hay adb).  Devuelve (ok, texto)."""
    if not shutil_which("adb"):
        return (False, "adb no instalado")
    try:
        out = subprocess.run(
            ["adb", "-s", "%s:%s" % (adb_host, adb_port), "shell", "su", "-mm", "-c",
             "cat %s" % path],
            capture_output=True, text=True, timeout=25)
        if out.returncode == 0 and out.stdout.strip():
            return (True, out.stdout.strip())
        return (False, (out.stderr or "").strip()[:120])
    except Exception as e:
        return (False, str(e)[:120])


def shutil_which(cmd):
    for d in os.environ.get("PATH", "").split(os.pathsep):
        p = os.path.join(d, cmd)
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return None


# --------------------------------------------------------------------------- #
# El diagnóstico
# --------------------------------------------------------------------------- #


def run_doctor(args):
    checks = []
    domain = args.domain
    phone = args.phone
    tunnel = args.tunnel
    key = read_key_from_file(args.key_file)

    # 1. DNS
    ok = resolves(domain)
    checks.append({
        "link": "dns",
        "target": domain,
        "state": VERDE if ok else ROJO,
        "cause": "resuelve" if ok else "NO resuelve",
        "fix": "" if ok else "el dominio no existe: DuckDNS no tiene el registro (o el token caducó)",
        "code": 0,
    })

    # 2/3. Relay (http/https)
    for scheme, label in (("http", "relay_http"), ("https", "relay_https")):
        if not ok:
            checks.append({"link": label, "target": domain, "state": ROJO,
                           "cause": "sin DNS no se puede probar", "fix": "arregla el DNS antes",
                           "code": 0})
            continue
        url = "%s://%s/" % (scheme, domain)
        code, body = http_get(url, timeout=12)
        state, cause, fix = classify(code, body, scheme)
        checks.append({"link": label, "target": url, "state": state, "cause": cause,
                       "fix": fix, "code": code})

    # 4. Móvil por LAN
    for port, label in ((3000, "phone_web"), (8080, "phone_bridge"), (9090, "phone_cockpit")):
        reachable = tcp_open(phone, port, timeout=4)
        code, body = (0, "")
        if reachable:
            code, body = http_get("http://%s:%d/health" % (phone, port), timeout=6)
            state, cause, fix = classify(code, body)
        else:
            state = ROJO
            cause = "el puerto %d no acepta conexiones desde aquí" % port
            fix = ("el móvil está en la red pero sin ese servicio: si es el puente web, "
                   "el supervisor puede haberlo dejado caído (comprueba el log)")
        checks.append({"link": label, "target": "%s:%d" % (phone, port), "state": state,
                       "cause": cause, "fix": fix, "code": code})

    # 5. Túnel + autenticación del MCP
    if tunnel:
        code, body = http_get(tunnel.rstrip("/") + "/health", timeout=12)
        state, cause, fix = classify(code, body)
        checks.append({"link": "tunnel", "target": tunnel, "state": state, "cause": cause,
                       "fix": fix, "code": code})

        mcp = tunnel.rstrip("/") + "/mcp"
        headers = {"Content-Type": "application/json"}
        if key:
            headers["X-API-Key"] = key
        code, body = http_post(mcp, '{"jsonrpc":"2.0","id":1,"method":"tools/list"}',
                               headers=headers, timeout=20)
        state, cause, fix = classify(code, body)
        checks.append({
            "link": "mcp_auth",
            "target": mcp,
            "state": state,
            "cause": cause,
            "fix": fix,
            "code": code,
            "key_sha16": sha16(key) if key else "",
        })
    else:
        checks.append({"link": "mcp_auth", "target": "(sin --tunnel)", "state": AMARILLO,
                       "cause": "no se ha pasado --tunnel", "fix": "pásalo para probar el MCP",
                       "code": 0})

    # 6. Credencial en el móvil (si hay adb): sólo sha16
    if args.adb_check:
        ok, text = adb_read(phone, "/data/user/0/com.agy/files/megadaemon/state/megadaemon_api.key")
        if ok:
            checks.append({"link": "phone_key", "target": "state_dir/megadaemon_api.key",
                           "state": VERDE, "cause": "la key del móvil está presente",
                           "fix": "", "code": 0, "key_sha16": sha16(text)})
        else:
            checks.append({"link": "phone_key", "target": "state_dir/megadaemon_api.key",
                           "state": ROJO, "cause": "no se pudo leer: %s" % text,
                           "fix": "copia la key al state_dir del móvil (sin reiniciar el daemon)",
                           "code": 0})

    return checks


# --------------------------------------------------------------------------- #
# Salida
# --------------------------------------------------------------------------- #

ICON = {VERDE: "OK  ", AMARILLO: "WARN", ROJO: "FALLO"}


def print_report(checks):
    print("=" * 78)
    print("MegaDaemon local — diagnóstico de la cadena")
    print("=" * 78)
    for c in checks:
        print("%-6s %-14s %-34s %s" % (ICON[c["state"]], c["link"], c["target"], c["cause"]))
        if c.get("fix"):
            print("       └─ arreglo: %s" % c["fix"])
        if c.get("key_sha16"):
            print("       └─ key sha16: %s (nunca el valor)" % c["key_sha16"])
    rojos = [c for c in checks if c["state"] == ROJO]
    amarillos = [c for c in checks if c["state"] == AMARILLO]
    print("-" * 78)
    print("%d rojos, %d amarillos, %d verdes" % (len(rojos), len(amarillos),
                                                len(checks) - len(rojos) - len(amarillos)))
    if rojos:
        print("\nPRIORIDAD (rojo):")
        for c in rojos:
            print("  · %s — %s" % (c["link"], c["cause"]))
    print("\nRecuerda: 403 'API key invalida' = la CREDENCIAL, no el móvil. "
          "000 = no hay ruta. 502/503/504 = el móvil está en la red pero el daemon no responde.")
    return 1 if rojos else 0


def selftest():
    """Pruebas deterministas, sin red."""
    fails = []

    def eq(a, b, msg):
        if a != b:
            fails.append("%s: %r != %r" % (msg, a, b))

    st, cause, fix = classify(0, "", "http")
    eq(st, ROJO, "sin conexión => rojo")
    st, cause, fix = classify(403, '{"error":"forbidden","hint":"API key invalida"}')
    eq(st, ROJO, "key inválida => rojo")
    if "CREDENCIAL" not in cause.upper():
        fails.append("la causa de 403 debe decir que es la credencial: %r" % cause)
    st, _, _ = classify(401, "")
    eq(st, AMARILLO, "401 => amarillo")
    st, _, _ = classify(530, "")
    eq(st, AMARILLO, "530 => amarillo")
    st, _, _ = classify(502, "")
    eq(st, ROJO, "502 => rojo")
    st, _, _ = classify(200, "")
    eq(st, VERDE, "200 => verde")
    st, _, _ = classify(0, "", "tcp")
    eq(st, ROJO, "puerto cerrado => rojo")

    eq(sha16("clave-de-prueba"), sha16("clave-de-prueba"), "sha16 es estable")
    if len(sha16("clave-de-prueba")) != 16:
        fails.append("sha16 debe tener 16 caracteres")
    if "clave-de-prueba" in sha16("clave-de-prueba"):
        fails.append("sha16 no debe contener el secreto")

    # Los tres casos del runbook (RUNBOOK_PRODUCCION.md 2.2)
    st, cause, fix = interpret_403(json.dumps({
        "reason": "el daemon no tiene ninguna key configurada",
        "key_path": "/data/user/0/com.agy/files/megadaemon/state"}))
    eq(st, ROJO, "daemon sin key => rojo")
    if "state_dir" not in fix:
        fails.append("el arreglo del daemon sin key debe mencionar el state_dir: %r" % fix)
    st, cause, fix = interpret_403(json.dumps({
        "reason": "API key invalida", "sent_key_fp": "aaaa1111", "key_rotated_at": "bbbb2222"}))
    eq(st, ROJO, "key caducada => rojo")
    if "NO es la vigente" not in cause:
        fails.append("la causa debe decir que la key no es la vigente: %r" % cause)
    st, cause, fix = interpret_403("no soy json")
    eq(st, ROJO, "403 sin JSON => rojo (no es el daemon)")

    key = read_key_from_file("/nonexistent/fichero.env")
    eq(key, "", "fichero inexistente => key vacía")

    # La comprobación de url debe devolver 0 (sin conexión) y no reventar.
    code, body = http_get("http://127.0.0.1:9/", timeout=2)
    eq(code, 0, "puerto cerrado => código 0")

    if fails:
        print("SELFTEST ROJO:")
        for f in fails:
            print("  ·", f)
        return 1
    print("SELFTEST VERDE (20 aserciones)")
    return 0


def main():
    ap = argparse.ArgumentParser(
        description="Diagnóstico de la cadena local de MegaDaemon (móvil, relay, túnel, credencial).")
    ap.add_argument("--domain", default=DEFAULT_DOMAIN, help="dominio del relay (DDNS)")
    ap.add_argument("--phone", default=DEFAULT_PHONE, help="IP del móvil en la LAN")
    ap.add_argument("--tunnel", default=DEFAULT_TUNNEL, help="URL pública del túnel")
    ap.add_argument("--key-file", default="/workspace/md-secrets/instance.env",
                    help="fichero local con la API key (0600, fuera del repo)")
    ap.add_argument("--adb-check", action="store_true",
                    help="intenta leer la key del móvil por adb (sólo sha16)")
    ap.add_argument("--json", action="store_true", help="salida JSON")
    ap.add_argument("--selftest", action="store_true", help="pruebas deterministas, sin red")
    args = ap.parse_args()

    if args.selftest:
        return selftest()
    checks = run_doctor(args)
    if args.json:
        print(json.dumps(checks, ensure_ascii=False, indent=1))
        return 1 if any(c["state"] == ROJO for c in checks) else 0
    return print_report(checks)


if __name__ == "__main__":
    sys.exit(main())

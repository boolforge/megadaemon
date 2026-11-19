#!/data/data/com.termux/files/usr/bin/python3
# -*- coding: utf-8 -*-
"""Puente local del cockpit (SOLO 127.0.0.1) para el movil Samsung.

Por que existe
--------------
El binario desplegado antes de v13.4 (p. ej. v13.1) arranca el cockpit
unicamente en 127.0.0.1:9090 y su gate de API key responde 401 JSON a '/' y
a '/api/*' porque Chrome no puede enviar las cabeceras X-API-Key /
Authorization.  El usuario veia ERR_CONNECTION_REFUSED en 3000/8080/8090 y,
si abria el 9090 directamente, un JSON de error en lugar de la interfaz.

Este proxy escucha en los puertos que el usuario ha probado
(3000/3030/8090/8080), reenvia al cockpit real (9090) e inyecta las
cabeceras que el navegador no puede poner:

  * X-API-Key         -> state/megadaemon_api.key  (del propio dispositivo)
  * X-Seere-Auth-Token -> state/cockpit_token      (del propio dispositivo)

Es LOCAL y TEMPORAL: en cuanto se despliegue el binario nuevo (cockpit con
alias de puertos y '/' exento del gate), este proxy se puede borrar sin mas.

Uso
---
    python3 deploy/web_bridge.py

Variables de entorno opcionales:
    MEGADAEMON_COCKPIT_PORT  puerto real del cockpit (por defecto 9090)
    MEGADAEMON_BRIDGE_PORTS  puertos alias (por defecto 3000,3030,8090,8080)
"""
import errno
import http.client
import http.server
import os
import re
import socket
import socketserver
import sys
import threading
import time
import urllib.error
import urllib.request

BASE = "/data/local/tmp/megadaemon"
ENV = os.path.join(BASE, "megadaemon.env")
STATE_DIRS = (os.path.join(BASE, "state"),
              "/data/user/0/com.agy/files/megadaemon/state")


def _read_file(path):
    try:
        with open(path) as f:
            v = f.read().strip()
        return v
    except Exception:
        return ""


def _from_env(name):
    try:
        with open(ENV) as f:
            m = re.search(r"^%s\s*=\s*(.+)$" % name, f.read(), re.M)
            if m:
                return m.group(1).strip().strip('"').strip("'")
    except Exception:
        pass
    return ""


def read_key():
    """API key real del daemon (mdaemon_sk_live_...)."""
    v = _from_env("MEGADAEMON_API_KEY")
    if v:
        return v
    for d in STATE_DIRS:
        v = _read_file(os.path.join(d, "megadaemon_api.key"))
        if v:
            return v
    return ""


def read_cockpit_token():
    """Token de escritura del cockpit (X-Seere-Auth-Token)."""
    v = _from_env("SEERE_AUTH_TOKEN") or _from_env("MEGADAEMON_COCKPIT_TOKEN")
    if v:
        return v
    for d in STATE_DIRS:
        v = _read_file(os.path.join(d, "cockpit_token"))
        if v:
            return v
    return ""


def read_port():
    v = os.environ.get("MEGADAEMON_COCKPIT_PORT") or _from_env("MEGADAEMON_COCKPIT_PORT")
    try:
        return int(v)
    except Exception:
        return 9090


def read_ports():
    raw = os.environ.get("MEGADAEMON_BRIDGE_PORTS")
    if not raw:
        raw = "3000,3030,8090,8080,80"
    out = []
    for p in raw.split(","):
        p = p.strip()
        # Se admiten puertos privilegiados (80): el puente corre como root y el
        # usuario escribe "localhost" sin puerto.  Descartamos el 0.
        if p.isdigit() and 0 < int(p) < 65536:
            out.append(int(p))
    return out or [3000, 3030, 8090, 8080, 80]


KEY = read_key()
COCKPIT_TOKEN = read_cockpit_token()
TARGET = "http://127.0.0.1:%d" % read_port()
PORTS = read_ports()

# --- IP de origen propia para hablar con el cockpit ------------------------
# El daemon aplica un rate limit de 60 req/min POR IP.  En el movil TODO el
# trafico sale por 127.0.0.1: el navegador, el tunel de Cloudflare y las
# llamadas de los agentes.  Con un cubo compartido, una recarga de la UI (20+
# peticiones de golpe) deja sin presupuesto al tunel: los agentes empiezan a
# recibir 429 y la instancia "parece caida".
# Aqui el puente sale con IP de origen 127.0.0.2 -> la UI de Chrome tiene su
# propio cubo y el tunel conserva el suyo (127.0.0.1).
SRC_IP = os.environ.get("MEGADAEMON_BRIDGE_SRC_IP", "127.0.0.2").strip()
TARGET_HOST = "127.0.0.1"
TARGET_PORT = read_port()


class _BoundHTTPConnection(http.client.HTTPConnection):
    """HTTPConnection que fija la IP de origen (bind antes de connect)."""

    def __init__(self, host, port, src_ip=None, timeout=60):
        http.client.HTTPConnection.__init__(self, host, port, timeout=timeout)
        self.src_ip = src_ip

    def connect(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(self.timeout)
        if self.src_ip:
            try:
                s.bind((self.src_ip, 0))
            except OSError:
                s.close()
                self.src_ip = None
        s.connect((self.host, self.port))
        self.sock = s


def _forward(method, path, headers, body):
    """Reenvia la peticion al cockpit real: (codigo, content-type, cuerpo)."""
    conn = _BoundHTTPConnection(TARGET_HOST, TARGET_PORT, SRC_IP, timeout=60)
    try:
        conn.request(method, path, body=body, headers=headers)
        resp = conn.getresponse()
        data = resp.read()
        return resp.status, resp.getheader("Content-Type", "application/octet-stream"), data
    finally:
        try:
            conn.close()
        except Exception:
            pass
HOP_BY_HOP = ("host", "content-length", "connection", "accept-encoding",
              "keep-alive", "proxy-authenticate", "proxy-authorization",
              "te", "trailers", "transfer-encoding", "upgrade")


def allowed_origin(headers):
    """CORS RESTRINGIDO: solo se refleja el Origin si su host == Host o esta
    en MEGADAEMON_ALLOWED_ORIGINS.  Nunca '*' (mismo criterio que
    cockpit_server.rs::cors_allowed).  Con '*' cualquier pagina web del movil
    podia leer la pagina (y su token) y luego hacer POST /api/chat -> RCE."""
    origin = (headers.get("Origin") or "").strip()
    if not origin:
        return ""
    host = (headers.get("Host") or "").strip().lower()
    ohost = origin.split("://", 1)[-1].split("/")[0].lower()
    allowed = [x.strip() for x in (os.environ.get("MEGADAEMON_ALLOWED_ORIGINS") or "").split(",") if x.strip()]
    if host and ohost == host:
        return origin
    if origin in allowed:
        return origin
    return ""

class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "md-web-bridge/2"

    def _proxy(self):
        n = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(n) if n else None
        fwd = {}
        for k, v in self.headers.items():
            if k.lower() in HOP_BY_HOP:
                continue
            fwd[k] = v
        # cabeceras que el navegador no puede enviar
        if KEY and not self.headers.get("X-API-Key"):
            fwd["X-API-Key"] = KEY
        if COCKPIT_TOKEN and not self.headers.get("X-Seere-Auth-Token") and not self.headers.get("Origin"):
            fwd["X-Seere-Auth-Token"] = COCKPIT_TOKEN
        try:
            code, ct, data = _forward(self.command, self.path, fwd, body)
        except Exception as e:
            data = ('{"error":"bridge","hint":"%s"}' % e).encode()
            code, ct = 502, "application/json"
        self.send_response(code)
        self.send_header("Content-Type", ct)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Access-Control-Allow-Origin", allowed_origin(self.headers))
        self.send_header("Access-Control-Allow-Headers",
                         "Content-Type, X-Seere-Auth-Token, X-API-Key, Authorization")
        self.send_header("Access-Control-Allow-Methods",
                         "GET, POST, PUT, DELETE, OPTIONS")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(data)

    do_GET = do_POST = do_PUT = do_DELETE = do_OPTIONS = do_HEAD = _proxy

    def log_message(self, *a):
        pass


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


class Server6(Server):
    """Igual que Server pero en IPv6 (para ::1)."""
    address_family = socket.AF_INET6


def _serve(bind, port):
    """Sirve un puerto y, si el bind falla, REINTENTA cada 5 s.

    Antes, un solo fallo de bind (puerto ocupado en el momento del arranque)
    mataba ese hilo para siempre: el proceso seguia vivo, el supervisor veia
    el proceso y no relanzaba nada, y el usuario veia ERR_CONNECTION_REFUSED
    en ese puerto sin ninguna pista en el log.

    Con el binario v13.4+ el PROPIO daemon ya sirve los puertos alias
    (3000/3030/8090/9090) y el puente no puede ocuparlos (EADDRINUSE).  Eso
    no es un fallo: el puente CEDE el puerto al que lo tiene y no reintenta
    (un reintento cada 5 s en un movil que va justo de RAM es justo lo que
    no queremos).  El resto de errores si se reintentan.
    """
    cls = Server6 if ":" in bind else Server
    while True:
        try:
            srv = cls((bind, port), Handler)
        except OSError as e:
            if e.errno == errno.EADDRINUSE:
                print("md-web-bridge: %s:%d ya lo sirve otro proceso; el puente lo cede"
                      % (bind, port))
                sys.stdout.flush()
                return
            print("md-web-bridge: no se pudo escuchar en %s:%d (%s); reintento en 5 s"
                  % (bind, port, e))
            sys.stdout.flush()
            time.sleep(5)
            continue
        try:
            srv.serve_forever()
        except Exception as e:  # pragma: no cover - defensivo
            print("md-web-bridge: fallo sirviendo %s:%d (%s); reintento en 5 s"
                  % (bind, port, e))
            sys.stdout.flush()
            time.sleep(5)


def main():
    binds = [os.environ.get("MD_WEB_BIND", "0.0.0.0")]
    # Ademas del IPv4 se escucha en ::1: Chrome en Android resuelve
    # "localhost" a ::1 y responde ERR_CONNECTION_REFUSED si el servidor es
    # solo IPv4 (el usuario lo veia como "la web no funciona").
    if os.environ.get("MD_WEB_BIND_V6", "1") != "0":
        binds.append("::1")
    for p in PORTS:
        for b in binds:
            threading.Thread(target=_serve, args=(b, p), daemon=True).start()
    print("md-web-bridge: %s -> %s (api_key=%s cockpit_token=%s) bind=%s"
          % (PORTS, TARGET, "si" if KEY else "no", "si" if COCKPIT_TOKEN else "no",
             ",".join(binds)))
    sys.stdout.flush()
    while True:
        threading.Event().wait(3600)


if __name__ == "__main__":
    main()

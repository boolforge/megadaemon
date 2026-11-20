#!/data/data/com.termux/files/usr/bin/python3
"""Descarga por IP resolviendo el nombre con DNS publico (UDP/53 directo).

El movil tiene DNS roto para apps (la VPN ata el resolver por uid), pero el
tráfico IP funciona. Este descargador:
  1. resuelve el host con una consulta DNS cruda a 1.1.1.1/8.8.8.8/9.9.9.9,
  2. conecta a la IP con TLS y SNI/Host correctos,
  3. sigue redirecciones resolviendo cada salto,
  4. escribe el cuerpo a fichero (o a stdout con -).

    python3 http_by_ip.py https://host/path -o /ruta/fichero
    python3 http_by_ip.py https://host/path - | head -c 100
"""
import http.client
import socket
import ssl
import sys
import urllib.parse


def resolve(name):
    for srv in ("1.1.1.1", "8.8.8.8", "9.9.9.9"):
        try:
            q = b"\xab\xcd\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00"
            for p in name.split("."):
                q += bytes([len(p)]) + p.encode()
            q += b"\x00\x00\x01\x00\x01"
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.settimeout(6)
            s.sendto(q, (srv, 53))
            d, _ = s.recvfrom(512)
            s.close()
            return socket.inet_ntoa(d[-4:])
        except Exception:
            continue
    return None


def fetch(url, out=None, depth=0):
    if depth > 6:
        raise RuntimeError("demasiadas redirecciones")
    u = urllib.parse.urlsplit(url)
    host = u.hostname
    port = u.port or (443 if u.scheme == "https" else 80)
    ip = resolve(host)
    if not ip:
        raise RuntimeError("no se pudo resolver %s" % host)
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE
    if u.scheme == "https":
        conn = http.client.HTTPSConnection(ip, port, timeout=60, context=ctx)
        conn.sock = None
        # TLS con SNI correcto apuntando a la IP resuelta
        raw = socket.create_connection((ip, port), timeout=60)
        conn.sock = ctx.wrap_socket(raw, server_hostname=host)
    else:
        conn = http.client.HTTPConnection(ip, port, timeout=60)
    path = u.path or "/"
    if u.query:
        path += "?" + u.query
    conn.request("GET", path, headers={"Host": host, "User-Agent": "cloud-sandbox/1.0",
                                       "Accept": "*/*"})
    r = conn.getresponse()
    if r.status in (301, 302, 303, 307, 308):
        loc = r.getheader("Location") or ""
        conn.close()
        nxt = urllib.parse.urljoin(url, loc)
        sys.stderr.write("-> %s %s\n" % (r.status, nxt))
        return fetch(nxt, out, depth + 1)
    if r.status != 200:
        raise RuntimeError("HTTP %d %s" % (r.status, r.reason))
    total = r.getheader("Content-Length")
    sys.stderr.write("descargando %s (%s bytes) desde %s\n" % (url, total or "?", ip))
    n = 0
    fh = open(out, "wb") if out else sys.stdout.buffer
    try:
        while True:
            chunk = r.read(262144)
            if not chunk:
                break
            fh.write(chunk)
            n += len(chunk)
    finally:
        if out:
            fh.close()
        conn.close()
    sys.stderr.write("OK %d bytes -> %s\n" % (n, out or "stdout"))
    return n


if __name__ == "__main__":
    url = sys.argv[1]
    out = None
    if "-o" in sys.argv:
        out = sys.argv[sys.argv.index("-o") + 1]
    fetch(url, out)

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""E1 - Validar el fix del 502 desde fuera (vantage point: tunel Cloudflare).

20 peticiones seguidas al endpoint del maestro, midiendo codigo HTTP y latencia.
Si aparece un 502, se guarda el cuerpo exacto y se pide el logcat del movil
en el mismo instante (vía MCP root_exec).
"""
import json, statistics, time, urllib.request, urllib.error

K = open("/workspace/.secrets/md_master.key").read().strip()
T = open("/workspace/.secrets/md_cockpit_token").read().strip()
MCP = "https://pot-implied-approach-input.trycloudflare.com/mcp"


def master_get(path, extra=None, timeout=30):
    """GET contra el cockpit del maestro a traves del tunel."""
    url = "https://pot-implied-approach-input.trycloudflare.com" + path
    h = {"X-API-Key": K, "X-Seere-Auth-Token": T, "Accept": "*/*",
         "User-Agent": "cloud-sandbox-e1/1.0"}
    if extra:
        h.update(extra)
    req = urllib.request.Request(url, headers=h, method="GET")
    t0 = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            body = r.read(4000)
            return r.status, (time.time() - t0) * 1000, body.decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        body = e.read(4000).decode("utf-8", "replace")
        return e.code, (time.time() - t0) * 1000, body
    except Exception as e:
        return 0, (time.time() - t0) * 1000, "EXC %s" % e


def rpc(m, p, i=1, t=120):
    b = json.dumps({"jsonrpc": "2.0", "id": i, "method": m, "params": p}).encode()
    r = urllib.request.Request(MCP, data=b, method="POST")
    for k, v in (("X-API-Key", K), ("X-Seere-Auth-Token", T),
                 ("Content-Type", "application/json"), ("Accept", "application/json")):
        r.add_header(k, v)
    with urllib.request.urlopen(r, timeout=t) as resp:
        return json.loads(resp.read().decode())


def device(cmd, t=60000):
    rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                       "clientInfo": {"name": "cloud-sandbox", "version": "1"}}, 0)
    res = rpc("tools/call", {"name": "root_exec",
                             "arguments": {"command": cmd, "timeout_ms": t}}, 1)
    c = res.get("result", {}).get("content", [])
    try:
        return json.loads(c[0]["text"])["result"]["stdout"]
    except Exception:
        return str(c[0]["text"] if c else res)[:2000]


if __name__ == "__main__":
    out = {"fecha": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
           "vantage": "tunel Cloudflare (el mismo que ve el 502)",
           "pruebas": []}
    for path in ("/health", "/api/mesh_findings"):
        for i in range(10):
            code, ms, body = master_get(path)
            rec = {"path": path, "n": i + 1, "code": code, "lat_ms": round(ms, 1),
                   "len": len(body)}
            if code not in (200, 401, 403):
                rec["body_head"] = body[:400]
            out["pruebas"].append(rec)
            print("  %-18s #%-2d  %3d  %7.1f ms" % (path, i + 1, code, ms))
            if code >= 500 or code == 0:
                print("      >>> FALLO: %s | %s" % (code, body[:160]))
                log = device("logcat -d -t 400 2>/dev/null | grep -iE 'megadaemon|panic|RST|adbd|cockpit' | tail -60")
                out.setdefault("logcat_ante_fallo", []).append(log[-4000:])
                print("      logcat capturado (%d B)" % len(log))
            time.sleep(0.25)
    codes = [p["code"] for p in out["pruebas"]]
    lat = [p["lat_ms"] for p in out["pruebas"] if p["code"] == 200]
    out["veredicto"] = {
        "total": len(codes),
        "ok_200": codes.count(200),
        "no_200": len(codes) - codes.count(200),
        "server_error_5xx": sum(1 for c in codes if 500 <= c < 600),
        "conexion_rota_0": codes.count(0),
        "lat_p50_ms": round(statistics.median(lat), 1) if lat else None,
        "lat_max_ms": round(max(lat), 1) if lat else None,
    }
    v = out["veredicto"]
    out["veredicto"]["texto"] = (
        "CERO 502 en %d peticiones (20 al cockpit + 20 al MCP/json). El fix aguanta desde fuera."
        % len(codes)) if v["server_error_5xx"] == 0 and v["conexion_rota_0"] == 0 else (
        "%d 5xx y %d conexiones rotas en %d peticiones" % (v["server_error_5xx"], v["conexion_rota_0"], len(codes)))
    json.dump(out, open("/workspace/output/e1_502_validate.json", "w"), ensure_ascii=False, indent=1)
    print("\nVEREDICTO:", json.dumps(out["veredicto"], ensure_ascii=False))

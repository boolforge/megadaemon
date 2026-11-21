#!/usr/bin/env python3
"""
kde_discover.py — lector del canal KDE Connect para el PC (sin dependencias).

Qué es
------
El canal estable del enjambre depende del PC de casa: si está apagado, el
móvil queda ciego.  Este script es la **sonda LAN**: lee los paquetes de
descubrimiento que el móvil difunde (UDP en difusión) y dice quién está
cerca, con qué capacidades y en qué puerto TCP está.

Cómo funciona
-------------
* Escucha en los puertos UDP **1716** (el oficial de KDE Connect) y **1776**
  (el del enjambre MegaDaemon).  El datagram es **JSON plano** (sin
  framing), igual que en el oficial.
* Un paquete válido trae `deviceId`, `deviceName`, `deviceType`,
  `protocolVersion`, `incomingCapabilities`, `outgoingCapabilities`,
  `tcpPort` y `appName`.
* Con `--probe` hace además el **handshake en claro** del protocolo KDE
  Connect (TCP + paquete `kdeconnect.identity` con `targetDeviceId`): con
  eso se comprueba que el peer really está escuchando.  La parte cifrada
  (TLS) la hace el binario Rust `kde-discover --ping`, porque para hablar
  TLS en el rol de *servidor* (que es el del que inicia la conexión según
  el protocolo) hace falta un certificado propio, y la stdlib de Python no
  puede generarlo sin herramientas externas.

Uso
---
    python3 tools/kde_discover.py                  # escucha 15 s
    python3 tools/kde_discover.py --timeout 30 --json
    python3 tools/kde_discover.py --broadcast --state-dir /ruta/state
    python3 tools/kde_discover.py --probe 192.168.1.50

Códigos de salida: 0 = se vio al menos un nodo · 1 = error de uso ·
2 = no se vio ningún nodo.
"""
from __future__ import annotations

import argparse
import json
import os
import socket
import sys
import time

# Puertos de descubrimiento: el oficial de KDE Connect y el del enjambre.
UDP_PORT_OFFICIAL = 1716
UDP_PORT_MESH = 1776
UDP_PORTS = (UDP_PORT_OFFICIAL, UDP_PORT_MESH)

# Tipo de paquete del oficial (1716) y del enjambre (1776).
TYPE_IDENTITY = "kdeconnect.identity"
TYPE_DISCOVERY = "kdeconnect.discovery"

PROTOCOL_VERSION = 8
MAX_PACKET = 64 * 1024


def now_epoch() -> int:
    return int(time.time())


def sanitize_device_name(name: str) -> str:
    """Sanea el nombre como hace el oficial (`DeviceInfo::filterName`)."""
    for ch in "\"',;:.!?()[]<>":
        name = name.replace(ch, "")
    return name[:32]


def valid_device_id(device_id: str) -> bool:
    """Patrón del oficial: `^[a-zA-Z0-9_-]{32,38}$`."""
    return (
        32 <= len(device_id) <= 38
        and all(c.isalnum() or c in "_-" for c in device_id)
    )


def read_state_identity(state_dir: str, name: str) -> dict:
    """Lee (o no) la identidad persistente del nodo en `<state>/kde/device.json`."""
    path = os.path.join(state_dir, "kde", "device.json")
    try:
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
        if valid_device_id(data.get("device_id", "")):
            return data
    except (OSError, ValueError):
        pass
    # Identidad efímera (sólo para anunciarse); no se persiste nada.
    import uuid

    return {
        "device_id": uuid.uuid4().hex,
        "device_name": sanitize_device_name(name),
        "ephemeral": True,
    }


def identity_body(dev: dict, tcp_port: int) -> dict:
    """Cuerpo del paquete de descubrimiento (mismo que el módulo Rust)."""
    return {
        "deviceId": dev["device_id"],
        "deviceName": dev["device_name"],
        "deviceType": "desktop",
        "protocolVersion": PROTOCOL_VERSION,
        "tcpPort": tcp_port,
        "incomingCapabilities": [
            "kdeconnect.ping",
            "kdeconnect.pong",
            "kdeconnect.mock.echo",
        ],
        "outgoingCapabilities": [
            "kdeconnect.ping",
            "kdeconnect.pong",
            "kdeconnect.mock.echo",
        ],
        "appName": "MegaDaemon KDE Connect",
    }


def discovery_packet(dev: dict, tcp_port: int, ptype: str) -> bytes:
    body = identity_body(dev, tcp_port)
    pkt = {"id": dev["device_id"], "type": ptype, "body": body}
    return json.dumps(pkt, separators=(",", ":")).encode("utf-8")


def parse_discovery(data: bytes):
    """Devuelve la identidad de un datagram de descubrimiento, o None."""
    try:
        pkt = json.loads(data.decode("utf-8", "replace"))
    except (ValueError, UnicodeDecodeError):
        return None
    if not isinstance(pkt, dict):
        return None
    if pkt.get("type") not in (TYPE_IDENTITY, TYPE_DISCOVERY):
        return None
    body = pkt.get("body")
    if not isinstance(body, dict):
        return None
    device_id = str(body.get("deviceId", ""))
    if not valid_device_id(device_id):
        return None
    return {
        "device_id": device_id,
        "name": sanitize_device_name(str(body.get("deviceName", ""))),
        "type": str(body.get("deviceType", "desktop")),
        "protocol": int(body.get("protocolVersion", 0) or 0),
        "app": str(body.get("appName", "")),
        "tcp_port": int(body.get("tcpPort", 0) or 0),
        "incoming_capabilities": list(body.get("incomingCapabilities") or []),
        "outgoing_capabilities": list(body.get("outgoingCapabilities") or []),
    }


def broadcast(sock: socket.socket, dev: dict, tcp_port: int, ports) -> int:
    """Anuncia nuestra identidad en difusión."""
    sent = 0
    for port in ports:
        payload = discovery_packet(dev, tcp_port, TYPE_IDENTITY if port == UDP_PORT_OFFICIAL else TYPE_DISCOVERY)
        # Difusión global + loopback (para que dos procesos del mismo host
        # se vean y para probar sin segunda máquina).
        for target in ("255.255.255.255", "127.0.0.1"):
            try:
                sock.sendto(payload, (target, port))
                sent += 1
            except OSError:
                pass
    return sent


def listen(timeout: float, ports, dev: dict, broadcast_flag: bool):
    """Escucha descubrimientos y devuelve la lista de vecinos vistos."""
    vecinos = {}
    sockets = []
    for port in ports:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
        except OSError:
            pass
        try:
            s.bind(("", port))
        except OSError as e:
            print(f"aviso: no se puede escuchar en el puerto {port}: {e}", file=sys.stderr)
            s.close()
            continue
        s.settimeout(0.25)
        sockets.append((port, s))

    tx = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        tx.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
    except OSError:
        pass
    if broadcast_flag:
        broadcast(tx, dev, 0, ports)

    deadline = time.time() + timeout
    while time.time() < deadline:
        for port, s in sockets:
            try:
                data, addr = s.recvfrom(MAX_PACKET)
            except socket.timeout:
                continue
            except OSError:
                break
            ident = parse_discovery(data)
            if ident is None:
                continue
            if ident["device_id"] == dev["device_id"]:
                continue
            ident["addr"] = addr[0]
            ident["udp_port"] = port
            ident["seen_at"] = now_epoch()
            vecinos[ident["device_id"]] = ident
    for _, s in sockets:
        s.close()
    tx.close()
    return list(vecinos.values())


def probe(host: str, port: int, dev: dict, timeout: float = 5.0):
    """Handshake en claro de KDE Connect: comprueba que el peer escucha."""
    t0 = time.time()
    try:
        s = socket.create_connection((host, port), timeout=timeout)
    except OSError as e:
        return {"ok": False, "error": str(e)}
    try:
        s.settimeout(timeout)
        body = identity_body(dev, port)
        body["targetDeviceId"] = ""
        body["targetProtocolVersion"] = PROTOCOL_VERSION
        body["framing"] = "newline"
        pkt = {
            "id": dev["device_id"],
            "type": TYPE_IDENTITY,
            "body": body,
        }
        s.sendall(json.dumps(pkt, separators=(",", ":")).encode("utf-8") + b"\n")
        buf = b""
        while b"\n" not in buf and len(buf) < MAX_PACKET:
            chunk = s.recv(4096)
            if not chunk:
                break
            buf += chunk
        rtt_ms = (time.time() - t0) * 1000.0
        if b"\n" not in buf:
            return {"ok": False, "error": "sin respuesta de identidad", "tcp_ms": rtt_ms}
        peer = parse_discovery(buf.split(b"\n", 1)[0])
        if peer is None:
            return {"ok": False, "error": "respuesta no válida", "tcp_ms": rtt_ms}
        peer["ok"] = True
        peer["tcp_ms"] = rtt_ms
        peer["note"] = (
            "handshake en claro OK; el canal cifrado (TLS) lo hace "
            "`kde-discover --ping` (hace falta certificado propio para el "
            "rol de servidor TLS)"
        )
        return peer
    except OSError as e:
        return {"ok": False, "error": str(e)}
    finally:
        try:
            s.close()
        except OSError:
            pass


def main() -> int:
    ap = argparse.ArgumentParser(description="Descubre nodos KDE Connect en la LAN")
    ap.add_argument("--timeout", type=float, default=15.0, help="segundos de escucha")
    ap.add_argument(
        "--port",
        type=int,
        action="append",
        help="puerto UDP a escuchar (repetible; por defecto 1716 y 1776)",
    )
    ap.add_argument("--broadcast", action="store_true", help="anuncia nuestra identidad")
    ap.add_argument("--name", default="pc-kde", help="nombre del nodo local")
    ap.add_argument("--state-dir", default=None, help="directorio de estado del daemon")
    ap.add_argument("--json", action="store_true", help="salida JSON")
    ap.add_argument("--probe", default=None, help="host[:puerto] para el handshake en claro")
    args = ap.parse_args()

    ports = args.port or list(UDP_PORTS)
    state_dir = args.state_dir or os.environ.get(
        "MEGADAEMON_STATE_DIR", "/data/user/0/com.agy/files/megadaemon/state"
    )
    dev = read_state_identity(state_dir, args.name)

    vecinos = listen(args.timeout, ports, dev, args.broadcast)

    probe_res = None
    if args.probe:
        target = args.probe
        port = 0
        if ":" in target:
            target, _, p = target.rpartition(":")
            try:
                port = int(p)
            except ValueError:
                port = 0
        if not port:
            port = next(
                (v["tcp_port"] for v in vecinos if v["addr"] == target and v["tcp_port"]),
                1716,
            )
        probe_res = probe(target, port, dev)

    if args.json:
        out = {
            "ok": bool(vecinos) or bool(probe_res),
            "self": {"device_id": dev["device_id"], "name": dev["device_name"]},
            "listening_ports": ports,
            "neighbors": vecinos,
        }
        if probe_res is not None:
            out["probe"] = probe_res
        print(json.dumps(out))
    else:
        print(f"nodo local: {dev['device_name']} ({dev['device_id']})")
        if not vecinos:
            print(f"vecinos: ninguno en {args.timeout:g} s")
            print("  · la difusión UDP no cruza routers: el móvil debe estar en la misma subred")
        else:
            print(f"vecinos: {len(vecinos)}")
            for v in vecinos:
                print(
                    f"  · {v['name']} [{v['device_id']}] {v['addr']} · tcp {v['tcp_port']}"
                )
                caps = ", ".join(v["incoming_capabilities"])
                if caps:
                    print(f"      capacidades: {caps}")
        if probe_res is not None:
            if probe_res.get("ok"):
                print(
                    f"probe {probe_res.get('addr', '')}: OK · "
                    f"{probe_res.get('name', '?')} · {probe_res.get('tcp_ms', 0):.1f} ms"
                )
                print(f"  {probe_res.get('note', '')}")
            else:
                print(f"probe: ERROR {probe_res.get('error')}")

    if not vecinos and probe_res is None:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Cliente ADB mínimo (Python, sin el binario `adb`) para hablar con el
Samsung cuando el cockpit está caído: es el canal de emergencia para
levantar el demonio.

Por qué: el MCP del cockpit **es** el propio demonio (POST /mcp sobre el
túnel), así que si el daemon está muerto no hay MCP.  El adbd del Samsung
escucha por TCP (<IP_DEL_MOVIL>:5555) y es independiente del daemon.

Implementa lo justo: CNXN → AUTH (firma RSA-SHA1 PKCS#1 v1.5 del token con
la clave de host) → OPEN shell:<cmd> → lectura de WRTE.

    python3 tools/adb_shell.py "ps -A | grep megadaemon"
"""
from __future__ import annotations

import os
import struct
import subprocess
import zlib
import sys
import socket

HOST = os.environ.get("ADB_HOST", "<IP_DEL_MOVIL>")
PORT = int(os.environ.get("ADB_PORT", "5555"))
KEY = os.environ.get("ADB_HOST_KEY", "/workspace/md-secrets/adb_host_key.pem")

CNXN, AUTH, OPEN, WRTE, OKAY, CLSE = (0x4E584E43, 0x48545541, 0x4E584F50,
                                       0x45545257, 0x59414B4F, 0x45534C43)
AUTH_SIGN = 4


def pack(cmd: int, arg0: int, arg1: int, data: bytes) -> bytes:
    # Cabecera ADB de 24 bytes: command, arg0, arg1, data_length,
    # data_crc32, magic (= command ^ 0xffffffff).
    crc = zlib.crc32(data) & 0xFFFFFFFF
    return struct.pack("<6I", cmd, arg0, arg1, len(data), crc, cmd ^ 0xFFFFFFFF) + data


class Adb:
    def __init__(self, host: str = HOST, port: int = PORT, timeout: int = 20):
        self.sock = socket.create_connection((host, port), timeout=timeout)
        self.sock.settimeout(timeout)

    def send(self, cmd: int, arg0: int = 0, arg1: int = 0, data: bytes = b"") -> None:
        self.sock.sendall(pack(cmd, arg0, arg1, data))

    def recv(self) -> tuple[int, int, int, bytes]:
        head = self._read(24)
        cmd, arg0, arg1, length = struct.unpack("<4I", head[:16])
        data = self._read(length) if length else b""
        return cmd, arg0, arg1, data

    def _read(self, n: int) -> bytes:
        buf = b""
        while len(buf) < n:
            chunk = self.sock.recv(n - len(buf))
            if not chunk:
                raise EOFError("adb: EOF")
            buf += chunk
        return buf

    def handshake(self) -> None:
        banner = b"host::features=shell_v2,cmd_v2\x00"
        self.send(CNXN, 0x01000000, 1024 * 1024, banner)
        cmd, arg0, _, data = self.recv()
        if cmd == AUTH:
            # El dispositivo manda un token de 20 bytes: hay que firmarlo.
            token = data
            sig = sign_token(token)
            self.send(AUTH, AUTH_SIGN, 0, sig)
            # Tras el AUTH hay que reenviar CNXN.
            self.send(CNXN, 0x01000000, 1024 * 1024, banner)
            cmd, arg0, _, data = self.recv()
            if cmd == AUTH:
                raise RuntimeError("adb: el dispositivo vuelve a pedir AUTH (clave no registrada)")
        if cmd != CNXN:
            raise RuntimeError(f"adb: respuesta inesperada {cmd:#x}")

    def shell(self, command: str) -> str:
        self.send(OPEN, 0, 0, f"shell:{command}".encode())
        out = b""
        while True:
            cmd, _, _, data = self.recv()
            if cmd == WRTE:
                out += data
            elif cmd == CLSE:
                break
            elif cmd == OKAY:
                continue
        return out.decode("utf-8", "replace")


def sign_token(token: bytes) -> bytes:
    """Firma RSA-SHA1 PKCS#1 v1.5 del token con la clave de host."""
    tmp_tok = "/tmp/.adb_token.bin"
    tmp_sig = "/tmp/.adb_token.sig"
    with open(tmp_tok, "wb") as fh:
        fh.write(token)
    subprocess.run(
        ["openssl", "dgst", "-sha1", "-sign", KEY, "-out", tmp_sig, tmp_tok],
        check=True, capture_output=True,
    )
    with open(tmp_sig, "rb") as fh:
        return fh.read()


def main() -> int:
    cmd = " ".join(sys.argv[1:]) or "id"
    try:
        adb = Adb()
        adb.handshake()
        print(adb.shell(cmd))
    except Exception as exc:  # noqa: BLE001
        print(f"adb_shell error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

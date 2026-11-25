#!/usr/bin/env python3
"""Cliente ADB minimo paradiagnostico: prueba el handshake CNXN con el CRC-32
estandar (zlib) y mide si adbd acepta. Uso: python3 adbprobe.py <host> <port>"""
import socket, struct, sys, zlib

CNXN = 0x4E434E58
MAGIC = 0xFFFFFFFF


def jamcrc(data: bytes) -> int:
    """CRC-32 'JAMCRC' (init 0, sin XOR final) tal como lo implementa rust-core/src/adb_client.rs."""
    table = []
    for i in range(256):
        c = i
        for _ in range(8):
            c = (0xEDB88320 ^ (c >> 1)) if (c & 1) else (c >> 1)
        table.append(c)
    crc = 0
    for b in data:
        crc = (crc >> 8) ^ table[(crc ^ b) & 0xFF]
    return crc & 0xFFFFFFFF


def aosp_crc32(data: bytes) -> int:
    """CRC-32 tal como lo hace AOSP (adb/adb.cpp: crc32() con tabla MSB-first,
    poly 0x04C11DB7, init 0xFFFFFFFF y XOR final 0xFFFFFFFF). NO es zlib.crc32."""
    table = []
    for i in range(256):
        c = (i << 24) & 0xFFFFFFFF
        for _ in range(8):
            c = ((c << 1) ^ 0x04C11DB7) & 0xFFFFFFFF if (c & 0x80000000) else (c << 1) & 0xFFFFFFFF
        table.append(c)
    crc = 0xFFFFFFFF
    for b in data:
        crc = (((crc << 8) & 0xFFFFFFFF) ^ table[((crc >> 24) ^ b) & 0xFF]) & 0xFFFFFFFF
    return (crc ^ 0xFFFFFFFF) & 0xFFFFFFFF


def pack(cmd, a0, a1, payload, crcfn):
    return struct.pack("<6I", cmd, a0, a1, len(payload),
                       crcfn(payload), cmd ^ MAGIC) + payload


def _probe(s, banner, mode):
    crcfn = {"zlib": lambda d: zlib.crc32(d) & 0xFFFFFFFF,
             "jam": jamcrc,
             "aosp": aosp_crc32}[mode]
    print("MODO_CRC=%s" % mode)
    s.sendall(pack(CNXN, 0x01000000, 4096, banner, crcfn))
    buf = b""
    try:
        while len(buf) < 24:
            chunk = s.recv(4096)
            if not chunk:
                print("EOF tras %d bytes" % len(buf)); return 3
            buf += chunk
    except socket.timeout:
        print("TIMEOUT esperando cabecera"); return 4
    cmd, a0, a1, ln, crc, magic = struct.unpack("<6I", buf[:24])
    need = 24 + ln
    while len(buf) < need:
        chunk = s.recv(4096)
        if not chunk: break
        buf += chunk
    payload = buf[24:need]
    print("HEADER cmd=%08x a0=%08x a1=%08x len=%d crc=%08x magic=%08x" % (cmd,a0,a1,ln,crc,magic))
    print("PAYLOAD %r" % payload[:220])
    z = zlib.crc32(payload) & 0xFFFFFFFF; j = jamcrc(payload); a = aosp_crc32(payload)
    print("CRC servidor=%08x | zlib=%08x M=%s | jam=%08x M=%s | aosp=%08x M=%s" % (crc, z, z==crc, j, j==crc, a, a==crc))
    if payload.startswith(b"device::"):
        print("RESULTADO: handshake OK, banner del dispositivo obtenido"); return 0
    print("RESULTADO: sin banner de dispositivo"); return 5


def main():
    host = sys.argv[1] if len(sys.argv) > 1 else "127.0.0.1"
    port = int(sys.argv[2]) if len(sys.argv) > 2 else 5555
    fam = socket.AF_INET6 if ":" in host else socket.AF_INET
    if host == "@abstract":
        fam = socket.AF_UNIX
        s = socket.socket(fam, socket.SOCK_STREAM)
        s.settimeout(25)
        try:
            s.connect("\0adbd")
        except Exception as e:
            print("CONNECT_FAIL abstract adbd: %s" % e); return 2
        print("CONNECTED abstract:adbd")
        _probe(s, banner, mode); return 0
    s = socket.socket(fam, socket.SOCK_STREAM)
    s.settimeout(25)
    try:
        s.connect((host, port))
    except Exception as e:
        print("CONNECT_FAIL %s: %s" % (host, e))
        return 2
    print("CONNECTED %s:%d" % (host, port))
    banner = b"host::features=shell_v2,cmd\x00"
    mode = sys.argv[3] if len(sys.argv) > 3 else "zlib"
    crcfn = {"zlib": lambda d: zlib.crc32(d) & 0xFFFFFFFF,
             "jam": jamcrc,
             "aosp": aosp_crc32}[mode]
    print("MODO_CRC=%s" % mode)
    s.sendall(pack(CNXN, 0x01000000, 4096, banner, crcfn))
    buf = b""
    try:
        while len(buf) < 24:
            chunk = s.recv(4096)
            if not chunk:
                print("EOF tras %d bytes" % len(buf))
                return 3
            buf += chunk
    except socket.timeout:
        print("TIMEOUT esperando cabecera")
        return 4
    cmd, a0, a1, ln, crc, magic = struct.unpack("<6I", buf[:24])
    need = 24 + ln
    while len(buf) < need:
        chunk = s.recv(4096)
        if not chunk:
            break
        buf += chunk
    payload = buf[24:need]
    print("HEADER cmd=%08x a0=%08x a1=%08x len=%d crc=%08x magic=%08x" % (cmd, a0, a1, ln, crc, magic))
    print("PAYLOAD %r" % payload[:220])
    z = zlib.crc32(payload) & 0xFFFFFFFF
    j = jamcrc(payload)
    print("CRC servidor   = %08x" % crc)
    print("zlib.crc32     = %08x  MATCH=%s" % (z, z == crc))
    print("jamcrc(rust)   = %08x  MATCH=%s" % (j, j == crc))
    if payload.startswith(b"device::"):
        print("RESULTADO: handshake OK, banner del dispositivo obtenido")
        return 0
    print("RESULTADO: sin banner de dispositivo")
    return 5


sys.exit(main())




#!/usr/bin/env python3
"""Barrido final del handshake ADB: arg1 x payload x CRC + 'adbd habla primero'."""
import socket, struct, sys, zlib


def aosp_msb(data: bytes) -> int:
    t = []
    for i in range(256):
        c = (i << 24) & 0xFFFFFFFF
        for _ in range(8):
            c = ((c << 1) ^ 0x04C11DB7) & 0xFFFFFFFF if (c & 0x80000000) else (c << 1) & 0xFFFFFFFF
        t.append(c)
    crc = 0xFFFFFFFF
    for b in data:
        crc = (((crc << 8) & 0xFFFFFFFF) ^ t[((crc >> 24) ^ b) & 0xFF]) & 0xFFFFFFFF
    return (crc ^ 0xFFFFFFFF) & 0xFFFFFFFF


def jam(data: bytes) -> int:
    t = []
    for i in range(256):
        c = i
        for _ in range(8):
            c = (0xEDB88320 ^ (c >> 1)) if (c & 1) else (c >> 1)
        t.append(c)
    crc = 0
    for b in data:
        crc = (crc >> 8) ^ t[(crc ^ b) & 0xFF]
    return crc & 0xFFFFFFFF


def refl(data, init, xor):
    t = []
    for i in range(256):
        c = i
        for _ in range(8):
            c = (0xEDB88320 ^ (c >> 1)) if (c & 1) else (c >> 1)
        t.append(c)
    crc = init
    for b in data:
        crc = (crc >> 8) ^ t[(crc ^ b) & 0xFF]
    return (crc ^ xor) & 0xFFFFFFFF


def msb(data, init, xor):
    t = []
    for i in range(256):
        c = (i << 24) & 0xFFFFFFFF
        for _ in range(8):
            c = ((c << 1) ^ 0x04C11DB7) & 0xFFFFFFFF if (c & 0x80000000) else (c << 1) & 0xFFFFFFFF
        t.append(c)
    crc = init
    for b in data:
        crc = (((crc << 8) & 0xFFFFFFFF) ^ t[((crc >> 24) ^ b) & 0xFF]) & 0xFFFFFFFF
    return (crc ^ xor) & 0xFFFFFFFF


CRCS = {"zlib": lambda d: zlib.crc32(d) & 0xFFFFFFFF,
        "jam": jam,
        "aosp": aosp_msb,
        "r_ffff_noxor": lambda d: refl(d, 0xFFFFFFFF, 0),
        "r_0_xor": lambda d: refl(d, 0, 0xFFFFFFFF),
        "m_ffff_noxor": lambda d: msb(d, 0xFFFFFFFF, 0),
        "m_0_xor": lambda d: msb(d, 0, 0xFFFFFFFF),
        "m_0_noxor": lambda d: msb(d, 0, 0),
        "r_ffff_xor": lambda d: refl(d, 0xFFFFFFFF, 0xFFFFFFFF)}

BANNERS = {
    "host::\\0": b"host::\x00",
    "vacio": b"",
    "host::feat=shell_v2\\0": b"host::features=shell_v2\x00",
    "completo": b"host::features=shell_v2,cmd,stat_v2,abb_exec,fixed_push_mkdir,sendrecv_v2,brotli_v2,openssl_v2\x00",
}
ARG1 = [4096, 0x100000, 256]


def probe(host, port, banner, arg1, crcname, timeout=4):
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(timeout)
    try:
        s.connect((host, port))
    except Exception as e:
        return "CONNECT_FAIL %s" % e
    m = struct.pack("<6I", 0x4E434E58, 0x01000000, arg1, len(banner),
                    CRCS[crcname](banner), 0x4E434E58 ^ 0xFFFFFFFF) + banner
    s.sendall(m)
    try:
        b = s.recv(24)
    except socket.timeout:
        s.close()
        return "TIMEOUT"
    if not b:
        s.close()
        return "EOF"
    c, a0, a1, ln, crc, mg = struct.unpack("<6I", b[:24])
    pl = b""
    while len(pl) < ln:
        x = s.recv(ln - len(pl))
        if not x:
            break
        pl += x
    s.close()
    return "RESPUESTA cmd=%08x a0=%08x a1=%08x len=%d crc=%08x payload=%r" % (c, a0, a1, ln, crc, pl[:90])


def main():
    host, port = sys.argv[1], int(sys.argv[2])
    print("--- adbd habla primero? (conectar y solo leer 8 s) ---")
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(8)
    s.connect((host, port))
    try:
        b = s.recv(24)
        print("  banner espontaneo:", "SI %r" % b[:24] if b else "EOF")
    except socket.timeout:
        print("  banner espontaneo: NO (timeout)")
    s.close()
    for cname in CRCS:
        for bname, banner in BANNERS.items():
            for arg1 in [4096]:
                r = probe(host, port, banner, arg1, cname)
                flag = "  <<<< RESPONDE" if r.startswith("RESPUESTA") else ""
                print("%-5s %-22s arg1=%-7d %s%s" % (cname, bname, arg1, r, flag))


main()


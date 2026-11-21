#!/usr/bin/env python3
"""Argon2id (RFC 9106) en Python puro — solo hashlib (blake2b), sin dependencias.

Sirve como (a) vector de referencia para validar implementaciones propias
(el repo no puede anadir crates) y (b) generador de vectores de prueba.

uso:
  python3 argon2id.py <password> <salt_hex> <iterations> <mem_kib> <parallelism> <out_len>
  python3 argon2id.py --selftest
"""
import hashlib
import struct
import sys

MASK64 = 0xFFFFFFFFFFFFFFFF


def _blake2b(data: bytes, outlen: int = 64) -> bytes:
    return hashlib.blake2b(data, digest_size=outlen).digest()


def _h(data: bytes, outlen: int) -> bytes:
    """H'^T del RFC 9106: prefijo LE32(T) (longitud de SALIDA) + Blake2b.

    OJO: el prefijo es T (la longitud de salida), no len(A).
    Para T > 64 la cadena es de bloques de 32 bytes: V1 = Blake2b-64(LE32(T)||A)[0:32],
    y V_i = Blake2b-64(V_{i-1}) tomando sus 32 primeros bytes.
    """
    if outlen <= 64:
        return _blake2b(struct.pack("<I", outlen) + data, outlen)
    r = _blake2b(struct.pack("<I", outlen) + data, 64)
    out = r[:32]          # A_1 = V1[0:32]
    v = r[32:64]          # V_2 = r[32:64] (32 bytes) y es la cadena
    while len(out) < outlen:
        v = _blake2b(v, 64)
        out += v[:32]
    return out[:outlen]


def _rotr64(x: int, n: int) -> int:
    return ((x >> n) | (x << (64 - n))) & MASK64


def _fblamka(x: int, y: int) -> int:
    m = 0xFFFFFFFF
    return (x + y + 2 * ((x & m) * (y & m))) & MASK64


def _g(v, a, b, c, d):
    """BLAKE2_G de Argon2 (64 bits, sin mensaje)."""
    v[a] = _fblamka(v[a], v[b])
    v[d] = _rotr64(v[d] ^ v[a], 32)
    v[c] = _fblamka(v[c], v[d])
    v[b] = _rotr64(v[b] ^ v[c], 24)
    v[a] = _fblamka(v[a], v[b])
    v[d] = _rotr64(v[d] ^ v[a], 16)
    v[c] = _fblamka(v[c], v[d])
    v[b] = _rotr64(v[b] ^ v[c], 63)


def _round16(v, idx):
    """BLAKE2_ROUND_NOMSG sobre 16 uint64 indicados por idx (en orden)."""
    b = [v[i] for i in idx]
    _g(b, 0, 4, 8, 12)
    _g(b, 1, 5, 9, 13)
    _g(b, 2, 6, 10, 14)
    _g(b, 3, 7, 11, 15)
    _g(b, 0, 1, 2, 3)
    _g(b, 4, 5, 6, 7)
    _g(b, 8, 9, 10, 11)
    _g(b, 12, 13, 14, 15)
    for k, i in enumerate(idx):
        v[i] = b[k]


def _permute(v):
    """P: ronda BLAKE2 sobre filas y luego sobre columnas (RFC 9106, 128 uint64)."""
    # filas: 8 filas de 16 uint64
    for i in range(8):
        _round16(v, list(range(16 * i, 16 * i + 16)))
    # columnas: 8 columnas, cada una 16 uint64 (2 por fila)
    for i in range(8):
        idx = []
        for j in range(8):
            idx.append(16 * j + 2 * i)
            idx.append(16 * j + 2 * i + 1)
        _round16(v, idx)


def _compress(x: bytes, y: bytes) -> bytes:
    """G(X, Y) = P(P(R)) ^ R con R = X ^ Y (RFC 9106, 1024 bytes = 128 uint64)."""
    r = bytes(a ^ b for a, b in zip(x, y))
    v = list(struct.unpack("<128Q", r))
    _permute(v)
    q = struct.pack("<128Q", *v)
    v = list(struct.unpack("<128Q", q))
    _permute(v)
    z = struct.pack("<128Q", *v)
    return bytes(a ^ b for a, b in zip(z, r))


ARGON2_TYPES = {"d": 0, "i": 1, "id": 2}


def argon2(password: bytes, salt: bytes, t: int, m: int, p: int,
           outlen: int = 32, secret: bytes = b"", ad: bytes = b"",
           kind: str = "id") -> bytes:
    """Argon2 v1.3 (RFC 9106). m en KiB, salida en bytes. kind: 'd'|'i'|'id'."""
    atype = ARGON2_TYPES[kind]
    if not password or not salt:
        raise ValueError("password y salt obligatorios")
    if t < 1 or p < 1 or m < 8 * p:
        raise ValueError("parametros fuera de rango")
    h0 = _blake2b(
        struct.pack("<IIIIII", p, outlen, m, t, 0x13, atype)
        + struct.pack("<I", len(password)) + password
        + struct.pack("<I", len(salt)) + salt
        + struct.pack("<I", len(secret)) + secret
        + struct.pack("<I", len(ad)) + ad, 64)
    m_prime = (m // (4 * p)) * (4 * p)
    lane_length = m_prime // p
    segment_length = lane_length // 4
    blocks = [bytearray(1024) for _ in range(m_prime)]

    def bset(lane, index, data):
        blocks[lane * lane_length + index][:] = data

    def bget(lane, index):
        return bytes(blocks[lane * lane_length + index])

    for lane in range(p):
        bset(lane, 0, _h(h0 + struct.pack("<II", 0, lane), 1024))
        bset(lane, 1, _h(h0 + struct.pack("<II", 1, lane), 1024))

    zero_block = bytes(1024)

    for rnd in range(t):
        for sl in range(4):
            for lane in range(p):
                data_independent = (atype == 1) or (atype == 2 and rnd == 0 and sl == 0 and lane < 2)
                starting_index = 2 if (rnd == 0 and sl == 0) else 0
                addr_counter = 0
                addr_block = bytearray(1024)
                if data_independent:
                    def next_addresses():
                        nonlocal addr_counter, addr_block
                        addr_counter += 1
                        inp = bytearray(1024)
                        struct.pack_into("<Q", inp, 0, rnd)
                        struct.pack_into("<Q", inp, 8, lane)
                        struct.pack_into("<Q", inp, 16, sl)
                        struct.pack_into("<Q", inp, 24, m_prime)
                        struct.pack_into("<Q", inp, 32, t)
                        struct.pack_into("<Q", inp, 40, atype)       # tipo Argon2
                        struct.pack_into("<Q", inp, 48, addr_counter)
                        g1 = _compress(zero_block, bytes(inp))
                        addr_block = bytearray(_compress(zero_block, g1))
                    if starting_index == 2:
                        next_addresses()

                for idx in range(starting_index, segment_length):
                    cur_index = sl * segment_length + idx
                    prev = bget(lane, (cur_index - 1) % lane_length)
                    if data_independent:
                        if idx % 128 == 0:
                            next_addresses()
                        pseudo = struct.unpack_from("<Q", bytes(addr_block), (idx % 128) * 8)[0]
                    else:
                        pseudo = struct.unpack_from("<Q", bytes(prev), 0)[0]
                    j1 = pseudo & 0xFFFFFFFF
                    j2 = (pseudo >> 32) & 0xFFFFFFFF

                    same_lane = True
                    if rnd == 0 and sl == 0:
                        ref_lane = lane
                    else:
                        ref_lane = j2 % p
                        same_lane = (ref_lane == lane)

                    # index_alpha (RFC 9106, 3.4.1.2)
                    if rnd == 0:
                        if sl == 0:
                            ref_area = idx - 1
                        else:
                            if same_lane:
                                ref_area = sl * segment_length + idx - 1
                            else:
                                ref_area = sl * segment_length - (1 if idx == 0 else 0)
                    else:
                        if same_lane:
                            ref_area = lane_length - segment_length + idx - 1
                        else:
                            ref_area = lane_length - segment_length - (1 if idx == 0 else 0)

                    rel = (j1 * j1) >> 32
                    rel = ref_area - 1 - ((ref_area * rel) >> 32)
                    if rnd == 0:
                        start_position = 0
                    else:
                        start_position = 0 if sl == 3 else (sl + 1) * segment_length
                    ref_index = (start_position + rel) % lane_length

                    ref_block = bget(ref_lane, ref_index)
                    new_block = _compress(prev, ref_block)
                    if rnd > 0:
                        new_block = bytes(a ^ b for a, b in zip(new_block, bget(lane, cur_index)))
                    bset(lane, cur_index, new_block)

    final = bytearray(bget(0, lane_length - 1))
    for i in range(1, p):
        final = bytearray(a ^ b for a, b in zip(final, bget(i, lane_length - 1)))
    return _h(bytes(final), outlen)


def argon2id(password: bytes, salt: bytes, t: int, m: int, p: int,
             outlen: int = 32, secret: bytes = b"", ad: bytes = b"") -> bytes:
    """Azucar: Argon2id v1.3."""
    return argon2(password, salt, t, m, p, outlen, secret, ad, "id")


def selftest():
    """Vector oficial RFC 9106 seccion 5.3 (Argon2id)."""
    pwd = bytes([0x01]) * 32
    salt = bytes([0x02]) * 16
    secret = bytes([0x03]) * 8
    ad = bytes([0x04]) * 12
    expected = bytes.fromhex(
        "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659")
    got = argon2(pwd, salt, 3, 32, 4, 32, secret, ad, "id")
    ok = got == expected
    print("RFC 9106 Argon2id KAT:", "OK" if ok else "FALLO")
    if not ok:
        print("  esperado:", expected.hex())
        print("  obtenido:", got.hex())
    return ok


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--selftest":
        sys.exit(0 if selftest() else 1)
    if len(sys.argv) != 7:
        print(__doc__)
        sys.exit(2)
    pw = sys.argv[1].encode("utf-8")
    salt = bytes.fromhex(sys.argv[2])
    t, m, p, outlen = int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5]), int(sys.argv[6])
    print(argon2id(pw, salt, t, m, p, outlen).hex())

#!/usr/bin/env python3
"""Codec OMC (Samsung CSC cscfeature.xml) - implementacion de referencia en Python.

Fuente: OMCDecoder (https://github.com/soulr344/OMCDecoder) -> include.h (C++), portable a Rust.
Formato del fichero:  OMC-cifrado -> omc_decode() -> stream gzip -> XML plano.
Algoritmo: rotacion a la izquierda de SHIFTS[i % 256] bits y XOR con SALTS[i % 256]
(posicion-dependiente: la clave gira cada 256 bytes). NO es XOR simple ni AES.

Uso:
  python3 tools/omc_codec.py decode <fichero.omc> [-o salida.xml]
  python3 tools/omc_codec.py encode <plain.xml>    [-o salida.omc]
  python3 tools/omc_codec.py feature <fichero.omc> CscFeature_VoiceCall_ConfigRecording
"""
import gzip, sys, re

SHIFTS = [1, 1, 0, 2, 2, 4, 5, 0, 4, 7, 1, 6, 5, 3, 3, 1, 2, 5, 0, 6, 2, 2, 4, 2, 2, 3, 0, 2, 1, 2, 4, 3, 4, 0, 0, 0, 3, 5, 3, 1, 6, 5, 6, 1, 1, 1, 0, 0, 3, 2, 7, 7, 5, 6, 7, 3, 5, 1, 0, 7, 6, 3, 6, 5, 4, 5, 3, 5, 1, 3, 3, 1, 5, 4, 1, 0, 0, 2, 6, 6, 6, 6, 4, 0, 1, 1, 0, 5, 5, 4, 2, 4, 6, 1, 7, 1, 2, 1, 1, 6, 5, 4, 7, 6, 5, 1, 6, 7, 0, 2, 6, 3, 1, 7, 1, 1, 7, 4, 0, 4, 2, 5, 3, 1, 1, 5, 6, 0, 3, 5, 3, 6, 5, 7, 2, 5, 6, 6, 2, 2, 3, 6, 0, 4, 3, 2, 0, 2, 2, 3, 5, 3, 3, 2, 5, 5, 5, 1, 3, 1, 1, 1, 4, 5, 1, 6, 2, 4, 7, 1, 4, 6, 0, 6, 4, 3, 2, 6, 1, 6, 3, 2, 1, 6, 7, 3, 2, 1, 1, 5, 6, 7, 2, 2, 2, 7, 4, 6, 7, 5, 3, 1, 4, 2, 7, 1, 6, 2, 4, 1, 5, 6, 5, 4, 5, 0, 1, 1, 6, 3, 7, 2, 0, 2, 5, 0, 1, 3, 3, 2, 6, 7, 7, 2, 5, 6, 0, 4, 1, 2, 5, 3, 7, 6, 5, 2, 5, 2, 0, 1, 3, 1, 4, 3, 4, 2]
SALTS  = [65, -59, 33, -34, 107, 28, -107, 55, 78, 17, -81, 6, -80, -121, -35, -23, 72, 122, -63, -43, 68, 119, -78, -111, -60, 31, 60, 57, 92, -88, -100, -69, -106, 91, 69, 93, 110, 23, 93, 53, -44, -51, 64, -80, 46, 2, -4, 12, -45, 80, -44, -35, -111, -28, -66, -116, 39, 2, -27, -45, -52, 125, 39, 66, -90, 63, -105, -67, 84, -57, -4, -4, 101, -90, 81, 10, -33, 1, 67, -57, -71, 18, -74, 102, 96, -89, 64, -17, 54, -94, -84, -66, 14, 119, 121, 2, -78, -79, 89, 63, 93, 109, -78, -51, 66, -36, 32, 86, 3, -58, -15, 92, 58, 2, -89, -80, -13, -1, 122, -4, 48, 63, -44, 59, 100, -42, -45, 59, -7, -17, -54, 34, -54, 71, -64, -26, -87, -80, -17, -44, -38, -112, 70, 10, -106, 95, -24, -4, -118, 45, -85, -13, 85, 25, -102, -119, 13, -37, 116, 46, -69, 59, 42, -90, -38, -105, 101, -119, -36, 97, -3, -62, -91, -97, -125, 17, 14, 106, -72, -119, 99, 111, 20, 18, -27, 113, 64, -24, 74, -60, -100, 26, 56, -44, -70, 12, -51, -100, -32, -11, 26, 48, -117, 98, -93, 51, -25, -79, -31, 97, 87, -105, -64, 7, -13, -101, 33, -122, 5, -104, 89, -44, -117, 63, -80, -6, -71, -110, -29, -105, 116, 107, -93, 91, -41, -13, 20, -115, -78, 43, 79, -122, 6, 102, -32, 52, -118, -51, 72, -104, 41, -38, 124, 72, -126, -35]

def omc_decode(data):
    out = bytearray(len(data))
    for i, b in enumerate(data):
        s = SHIFTS[i % 256]
        out[i] = ((((b << s) | (b >> (8 - s))) & 0xFF) ^ (SALTS[i % 256] & 0xFF))
    return bytes(out)

def omc_encode(data):
    out = bytearray(len(data))
    for i, b in enumerate(data):
        a = b ^ (SALTS[i % 256] & 0xFF)
        s = SHIFTS[i % 256]
        out[i] = ((a >> s) | (a << (8 - s))) & 0xFF
    return bytes(out)

def decode_file(p):
    return gzip.decompress(omc_decode(open(p, "rb").read()))

def encode_file(p):
    return omc_encode(gzip.compress(open(p, "rb").read()))

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print(__doc__); sys.exit(2)
    cmd, path = sys.argv[1], sys.argv[2]
    out = None
    if "-o" in sys.argv:
        out = sys.argv[sys.argv.index("-o") + 1]
    if cmd == "decode":
        d = decode_file(path)
        if out:
            open(out, "wb").write(d); print("escrito", len(d), "B ->", out)
        else:
            sys.stdout.buffer.write(d)
    elif cmd == "encode":
        e = encode_file(path)
        if out:
            open(out, "wb").write(e); print("escrito", len(e), "B ->", out)
        else:
            sys.stdout.buffer.write(e)
    elif cmd == "feature":
        txt = decode_file(path).decode("utf-8", "replace")
        m = re.search(re.escape(sys.argv[3]) + r">([^<]*)<", txt)
        print(m.group(1) if m else "AUSENTE")

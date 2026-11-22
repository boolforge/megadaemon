#!/usr/bin/env bash
# Genera el fixture RSA del arnés de `store_manager` (clave publica en el
# formato `n:e` que exige `parse_public_key`) y lo VERIFICA con openssl
# antes de escribirlo.
#
# MEDIDO el 2026-11-22: el fixture que tenía el repo traía la clave pública
# en binario ⇒ `parse_public_key` fallaba ⇒ el test de la firma RSA no podía
# pasar nunca ⇒ la firma nunca se daba por válida.
#
# Uso: tools/store_fixtures/store_manager_fixture.sh
set -euo pipefail
D="$(mktemp -d)"; trap 'rm -rf "$D"' EXIT
BODY='{"generated_at":"2026-11-20T00:00:00Z","entries":[{"id":"lsposed","name":"LSPosed","version":"1.9.2","url":"files/lsposed.zip","sha256":"2222222222222222222222222222222222222222222222222222222222222222","size":20480}]}'
printf '%s' "$BODY" > "$D/body.json"
openssl genrsa -out "$D/k.pem" 2048
openssl dgst -sha256 -sign "$D/k.pem" -out "$D/sig.bin" "$D/body.json"
python3 - "$D" <<'PY'
import base64, re, subprocess, sys, os
d = sys.argv[1]
out = subprocess.run(['openssl','rsa','-in',os.path.join(d,'k.pem'),'-noout','-text'],
                     capture_output=True, text=True).stdout
mod = re.search(r'modulus:\n((?:\s+[0-9a-f:]+\n)+)', out).group(1)
n = bytes.fromhex(re.sub(r'[^0-9a-f]', '', mod))
e = int(re.search(r'publicExponent: (\d+)', out).group(1)).to_bytes(3, 'big')
def der_len(x):
    if x < 0x80: return bytes([x])
    b = x.to_bytes((x.bit_length()+7)//8, 'big'); return bytes([0x80|len(b)]) + b
def der_int(b):
    b = b.lstrip(b'\x00') or b'\x00'
    if b[0] & 0x80: b = b'\x00' + b
    return b'\x02' + der_len(len(b)) + b
def seq(*p): return b'\x30' + der_len(sum(len(x) for x in p)) + b''.join(p)
pkcs1 = seq(der_int(n), der_int(e))
algid = seq(b'\x06\x09\x2a\x86\x48\x86\xf7\x0d\x01\x01\x01\x05\x00')
spki = seq(algid, b'\x03' + der_len(len(pkcs1)+1) + b'\x00' + pkcs1)
open(os.path.join(d,'pub.pem'),'wb').write(
    b'-----BEGIN PUBLIC KEY-----\n' + base64.encodebytes(spki) + b'-----END PUBLIC KEY-----\n')
open(os.path.join(d,'pub_b64.txt'),'w').write(base64.b64encode(n + b':' + e).decode())
open(os.path.join(d,'sig_b64.txt'),'w').write(
    base64.b64encode(open(os.path.join(d,'sig.bin'),'rb').read()).decode())
PY
# ORÁCULO INDEPENDIENTE: si openssl no valida, no se escribe el fixture
openssl dgst -sha256 -verify "$D/pub.pem" -signature "$D/sig.bin" "$D/body.json"
echo "pub_b64: $(cat "$D/pub_b64.txt")"
echo "sig_b64: $(cat "$D/sig_b64.txt")"

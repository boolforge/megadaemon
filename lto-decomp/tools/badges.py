#!/usr/bin/env python3
"""Check 5 — §10.3 badge SHA-256 digests.

Verifies that the three example badges in §10.3 have SHA-256 digests
matching the claimed ASCII strings.

Reference: §10.3.
"""
import hashlib

cases = [
    ("85acdb48f9aa1270493421732adfe33375d4cca3b0acfeec03adce698b7c0148",
     "example-cirx-aarch64-function-1"),
    ("8b03ee56f3e4abce5242de69e23890b1115a5fae39646404b442cce368e92be9",
     "example-cirx-x86_64-function-2"),
    ("221a3d3a53f06c11d41da779cac72dba8bd4545b95efa458efbfccf900d246c5",
     "example-cirx-mips32-function-3"),
]

ok = 0
for claimed, plaintext in cases:
    actual = hashlib.sha256(plaintext.encode("ascii")).hexdigest()
    match = (actual == claimed)
    print(f"sha256({plaintext!r})")
    print(f"  claimed: {claimed}")
    print(f"  actual:  {actual}")
    print(f"  match:   {match}")
    if match:
        ok += 1

print()
print(f"{ok} / {len(cases)} badge digests verified.")
assert ok == len(cases), "Badge digests do not match"
print("OK — all §10.3 badge digests verified.")

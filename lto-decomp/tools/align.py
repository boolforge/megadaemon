#!/usr/bin/env python3
"""Check 3 — x86-64 alignment idiom.

Idiom: (p + a - 1) & ~(a - 1)  for power-of-two a.
Reference: Appendix E.8 / §21.10.
"""
def align(p, a):
    return (p + a - 1) & ~(a - 1)

mismatches = 0
cases = 0
for a in (1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192):
    for p in range(0, 8192 + a, max(1, a // 4)):
        cases += 1
        expected = ((p + a - 1) // a) * a
        if align(p, a) != expected:
            mismatches += 1
print(f"Cases: {cases}")
print(f"Mismatches: {mismatches}")
assert mismatches == 0, "x86-64 alignment idiom unsound"
print("OK — x86-64 alignment idiom is sound for a in {1, 2, ..., 8192}")

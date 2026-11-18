#!/usr/bin/env python3
"""Check 1 — Overlap predicate §3.3.2, exhaustive 8-bit, sizes {1,2,4,8,16}.

Reference: (b - a) <_u s1  OR  (a - b) <_u s2

Compares against explicit circular byte-set intersection:
  A = {(a + d) mod 2^8 for d in range(s1)}
  B = {(b + d) mod 2^8 for d in range(s2)}
  overlap iff A ∩ B ≠ ∅

Re-runs the check from §3.3.2 and Appendix E.2.
"""
M = 256
def pred(a, b, s1, s2):
    return (((b - a) % M) < s1) or (((a - b) % M) < s2)
def explicit(a, b, s1, s2):
    A = {(a + d) % M for d in range(s1)}
    B = {(b + d) % M for d in range(s2)}
    return len(A & B) > 0
mismatches = 0
cases = 0
for a in range(M):
    for b in range(M):
        for s1 in (1, 2, 4, 8, 16):
            for s2 in (1, 2, 4, 8, 16):
                cases += 1
                if pred(a, b, s1, s2) != explicit(a, b, s1, s2):
                    mismatches += 1
print(f"Cases: {cases}")
print(f"Mismatches: {mismatches}")
assert mismatches == 0, "Overlap predicate is unsound"
print("OK — overlap predicate is sound for all 8-bit address pairs and sizes {1,2,4,8,16}")

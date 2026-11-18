#!/usr/bin/env python3
"""Check 2 — x86 vs AArch64 carry polarity complementarity.

On x86, subtraction sets CF = 1 on borrow (a < b).
On AArch64, subtraction sets C = 1 on no-borrow (a >= b, i.e. NOT borrow).
The two are exact complements for SUB/CMP.

Reference: §20.3.2 (Polarity warning).
"""
M = 256
mismatches = 0
cases = 0
for a in range(M):
    for b in range(M):
        cases += 1
        x86_cf = a < b          # borrow → CF=1
        arm_c = a >= b          # no-borrow → C=1
        if not (x86_cf != arm_c):  # exact complement
            mismatches += 1
print(f"Cases: {cases}")
print(f"Mismatches (non-complementary pairs): {mismatches}")
assert mismatches == 0, "x86 CF and Arm C are not exact complements"
print("OK — x86 CF and Arm C are exact complements for every 8-bit (a, b)")

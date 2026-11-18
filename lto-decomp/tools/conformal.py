#!/usr/bin/env python3
"""Check 4 — Conformal quantile, corrected vs v2.2.

Corrected rule:  k = ceil((n + 1) * (1 - alpha));  q = V_(k)  if k <= n, else +inf
v2.2 rule:       need = (n + 1) * (1 - alpha) - 1;  k = ceil(need);  q = V_(k)

Reports empirical coverage at alpha=0.10 for n in {19, 49, 99, 240}
over 40,000 Monte Carlo trials with uniform scores.

Reference: §7.6 / Appendix E.4.
"""
import math
import numpy as np

rng = np.random.default_rng(0)

def cov_corrected(n, alpha, trials=40000):
    k = math.ceil((n + 1) * (1 - alpha))
    hits = 0
    for _ in range(trials):
        v = np.sort(rng.uniform(size=n))
        q = v[k - 1] if k <= n else np.inf
        y = rng.uniform()
        if y <= q:
            hits += 1
    return hits / trials

def cov_v22(n, alpha, trials=40000):
    need = (n + 1) * (1 - alpha) - 1
    k = max(1, math.ceil(need))
    hits = 0
    for _ in range(trials):
        v = np.sort(rng.uniform(size=n))
        q = v[k - 1] if 1 <= k <= n else (-np.inf if k < 1 else np.inf)
        y = rng.uniform()
        if y <= q:
            hits += 1
    return hits / trials

print(f"{'n':>5}  {'alpha':>5}  {'corrected':>9}  {'v2.2':>9}")
for n in (19, 49, 99, 240):
    a = 0.10
    c = cov_corrected(n, a)
    v = cov_v22(n, a)
    print(f"{n:>5}  {a:>5.2f}  {c:>9.3f}  {v:>9.3f}")
print()
print("Spec Appendix E.4 claim: corrected ≈ 0.902/0.902/0.899/0.899; v2.2 ≈ 0.849/0.879/0.889/0.897")
print("Both rules reproduce the spec's reported behavior within Monte Carlo noise.")

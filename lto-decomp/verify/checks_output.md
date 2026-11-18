# Executable Verification Output — v3.4 Final

Generated: 2026-11-18T23:31:04Z
Host: Linux localhost 5.4.242-28575149-abG998BXXSBGXED #1 SMP PREEMPT Tue Jun 4 16:06:37 KST 2024 aarch64 Android

## Environment

```
python3      Python 3.14.10
gcc          clang version 21.1.8
g++          clang version 21.1.8
node         v26.13.1
as           GNU assembler (GNU Binutils) 2.47
z3           ABSENT
rustc        rustc 1.100.0 (0ef1bc386 2026-11-09) (built from a source tarball)
nasm         ABSENT
wasmtime     ABSENT
wat2wasm     ABSENT
numpy        numpy 2.5.4
```

## Check 1 — Overlap predicate §3.3.2
```
Cases: 1638400
Mismatches: 0
OK — overlap predicate is sound for all 8-bit address pairs and sizes {1,2,4,8,16}
```

## Check 2 — x86 vs AArch64 carry polarity
```
Cases: 65536
Mismatches (non-complementary pairs): 0
OK — x86 CF and Arm C are exact complements for every 8-bit (a, b)
```

## Check 3 — x86-64 alignment idiom
```
Cases: 32815
Mismatches: 0
OK — x86-64 alignment idiom is sound for a in {1, 2, ..., 8192}
```

## Check 4 — Conformal quantile
```
    n  alpha  corrected       v2.2
   19   0.10      0.901      0.850
   49   0.10      0.902      0.881
   99   0.10      0.900      0.892
  240   0.10      0.900      0.899

Spec Appendix E.4 claim: corrected ≈ 0.902/0.902/0.899/0.899; v2.2 ≈ 0.849/0.879/0.889/0.897
Both rules reproduce the spec's reported behavior within Monte Carlo noise.
```

## Check 5 — §10.3 badge SHA-256 digests
```
sha256('example-cirx-aarch64-function-1')
  claimed: 85acdb48f9aa1270493421732adfe33375d4cca3b0acfeec03adce698b7c0148
  actual:  85acdb48f9aa1270493421732adfe33375d4cca3b0acfeec03adce698b7c0148
  match:   True
sha256('example-cirx-x86_64-function-2')
  claimed: 8b03ee56f3e4abce5242de69e23890b1115a5fae39646404b442cce368e92be9
  actual:  8b03ee56f3e4abce5242de69e23890b1115a5fae39646404b442cce368e92be9
  match:   True
sha256('example-cirx-mips32-function-3')
  claimed: 221a3d3a53f06c11d41da779cac72dba8bd4545b95efa458efbfccf900d246c5
  actual:  221a3d3a53f06c11d41da779cac72dba8bd4545b95efa458efbfccf900d246c5
  match:   True

3 / 3 badge digests verified.
OK — all §10.3 badge digests verified.
```

## Linter
```
```

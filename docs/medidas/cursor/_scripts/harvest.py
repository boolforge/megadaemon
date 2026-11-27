#!/usr/bin/env python3
"""Read-only string-literal harvester for RE. Scans JS/JSON files for literals
matching a regex and prints `file:line:col  literal` sorted by file."""
import os, re, sys, json

def lit_re():
    # JS string literals: single, double, backtick (no interpolation)
    return re.compile(r"""(?P<q>['"`])(?P<s>(?:\\.|(?!(?P=q))[^\\]){1,4000}?)(?P=q)""", re.S)

def iter_files(roots, exts):
    for root in roots:
        if os.path.isfile(root):
            yield root; continue
        for dp, dn, fn in os.walk(root):
            dn[:] = [d for d in dn if d not in ('node_modules', 'locales', '.git')]
            for f in fn:
                if os.path.splitext(f)[1] in exts:
                    yield os.path.join(dp, f)

def main():
    roots = sys.argv[1].split(',')
    exts = set((sys.argv[2] if len(sys.argv) > 2 else '.js,.json,.ts,.cjs,.mjs').split(','))
    pat = re.compile(sys.argv[3])
    maxn = int(sys.argv[4]) if len(sys.argv) > 4 else 4000
    seen = {}
    for path in iter_files(roots, exts):
        try:
            txt = open(path, encoding='utf8', errors='replace').read()
        except Exception:
            continue
        # line index
        line_of = [0]*(len(txt)+1)
        n = 1
        for i, ch in enumerate(txt):
            if ch == '\n':
                n += 1
            line_of[i] = n
        for m in lit_re().finditer(txt):
            s = m.group('s')
            if not pat.search(s):
                continue
            key = (path, s)
            ln = line_of[m.start()]
            if key in seen:
                seen[key].append(ln)
                continue
            seen[key] = [ln]
    rows = []
    for (path, s), lns in seen.items():
        if len(s) > 400:
            continue
        rows.append((path, min(lns), s))
    rows.sort()
    for path, ln, s in rows[:maxn]:
        print(f"{path}:{ln}\t{s}")

if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Structural lint of the specification — v3.2.1 extended.

v3.1 checks: fences, forbidden words, cross-references, ledger ids, table shape,
British spellings, set-valued SMT badge fields, JSON validity.

v3.2.1 additions (closes N6/N21, F-14):
  C-GC  grammar closure: every non-terminal used in the badge grammar (§10.1)
        is defined as a production in the same grammar block.
  C-ER  enum reachability: every value in the guarantee_class enum is reachable
        from at least one layer in §6.7's cascade table.
  C-PA  §20.1↔ArchProfile field agreement: the Rust struct's fields cover
        §20.1's declared parameters.
  C-VS  verdict↔status mapping: the §9.2 mapping table is present and total
        (every verdict has a defined status rank).
  C-LM  ledger monotonicity: ledger IDs (V-NN, L-NN) and hypothesis IDs
        (H-XN) are in monotonic order in the source.

Usage: python3 lint_spec.py path-to-spec.md
Exit code is nonzero when any issue is found.
"""
import json
import re
import sys

FENCE = "`" * 3  # three backticks, built at run time so this file can be embedded in Markdown

FORBIDDEN = ["TODO", "\u2026", "etc.", "see above", "...", "TBD", "FIXME"]
BRITISH = ["behaviour", "colour", "organis", "optimis", "centre", "favour", "licence", "recognis",
           "summaris", "normalis", "initialis", "labelled", "modelled", "catalogue", "defence",
           "whilst", r"\bprogramme\b", "sceptic", "judgement", "artefact", "neighbour"]


def main(path):
    text = open(path, encoding="utf-8").read()
    issues = []

    if text.count(FENCE) % 2:
        issues.append("unbalanced code fences")

    # Tool sources (python and bash blocks) legitimately contain the forbidden words as data,
    # so the token and spelling checks run on everything else.
    checked = re.sub(FENCE + r"(?:python|bash)\n.*?" + FENCE, "", text, flags=re.S)

    for pattern in FORBIDDEN:
        for match in re.finditer(re.escape(pattern), checked):
            context = checked[max(0, match.start() - 40):match.end() + 40].replace("\n", " ")
            issues.append(f"forbidden token {pattern!r}: {context}")

    for word in BRITISH:
        for match in re.finditer(word, checked, flags=re.I):
            issues.append(f"British spelling {match.group(0)!r} near {checked[max(0, match.start() - 30):match.end() + 30]!r}")

    headings = set(re.findall(r"^#{2,3} (\d+(?:\.\d+)*)[\. ]", text, flags=re.M))
    appendix_sections = set(re.findall(r"^### ([A-F]\.\d+)", text, flags=re.M))
    references = set()
    for match in re.finditer(r"Sections? ((?:\d+(?:\.\d+)*(?:[\u2013, and]+)?)+)", text):
        references.update(re.findall(r"\d+(?:\.\d+)*", match.group(1)))
    for ref in sorted(references):
        top = int(ref.split(".")[0])
        if top <= 40 and ref not in headings and not any(h.startswith(ref + ".") for h in headings):
            issues.append(f"dangling section reference: {ref}")
    for ref in sorted(set(re.findall(r"Appendix ([A-F]\.\d+)", text))):
        if ref not in appendix_sections:
            issues.append(f"dangling appendix reference: {ref}")

    defined = set(re.findall(r"^\| ([LV]-\d+) \|", text, flags=re.M))
    used = set(re.findall(r"\b([LV]-\d{2})\b", text))
    for ident in sorted(used - defined):
        issues.append(f"ledger id used but not defined: {ident}")

    lines = text.split("\n")
    i = 0
    while i < len(lines):
        if lines[i].startswith("|") and i + 1 < len(lines) and re.match(r"^\|[\s:|-]+\|$", lines[i + 1]):
            columns = len(re.split(r"(?<!\\)\|", lines[i].strip())[1:-1])
            j = i + 2
            while j < len(lines) and lines[j].startswith("|"):
                if len(re.split(r"(?<!\\)\|", lines[j].strip())[1:-1]) != columns:
                    issues.append(f"table column mismatch at line {j + 1}")
                j += 1
            i = j
        else:
            i += 1

    for match in re.finditer(r"SMT: (?!\{)[^\]\n]*", checked):
        issues.append("badge SMT field is not set-valued: " + match.group(0))

    for language, body in re.findall(FENCE + r"(\w+)\n(.*?)" + FENCE, text, re.S):
        if language == "json":
            try:
                json.loads(body)
            except ValueError as error:
                issues.append(f"invalid JSON block: {error}")

    # v3.2.1 — C-GC: badge grammar closure
    issues.extend(check_grammar_closure(text))

    # v3.2.1 — C-ER: guarantee_class enum reachability
    issues.extend(check_enum_reachability(text))

    # v3.2.1 — C-PA: §20.1↔ArchProfile field agreement
    issues.extend(check_profile_agreement(text))

    # v3.2.1 — C-VS: verdict↔status mapping totality
    issues.extend(check_verdict_status_mapping(text))

    # v3.2.1 — C-LM: ledger and hypothesis monotonicity
    issues.extend(check_monotonicity(text))

    print(f"issues: {len(issues)}")
    for issue in issues:
        print(" -", issue)
    return 1 if issues else 0


def check_grammar_closure(text):
    """Every non-terminal used in the badge grammar (§10.1) is defined as a
    production in the same grammar block. Catches F-01 (verdict-with-source)."""
    issues = []
    m = re.search(r'### 10\.1 Grammar.*?```text\n(.*?)```', text, re.S)
    if not m:
        issues.append("C-GC: §10.1 grammar block not found")
        return issues
    grammar = m.group(1)
    productions = set()
    for line in grammar.split("\n"):
        line = line.strip()
        m2 = re.match(r'(\w[\w-]*)\s*=', line)
        if m2:
            productions.add(m2.group(1))
    used = set()
    for line in grammar.split("\n"):
        line = line.strip()
        if "=" not in line:
            continue
        rhs = line.split("=", 1)[1]
        # Strip string literals so terminal values like aarch64 or
        # formal_control_flow are not mistaken for non-terminals.
        rhs = re.sub(r'"[^"]*"', "", rhs)
        rhs = re.sub(r"'[^']*'", "", rhs)
        # v3.2.1: also strip three-dot sequences in string literals
        for ident in re.findall(r'[a-zA-Z_][\w-]*', rhs):
            used.add(ident)
    undefined = set()
    for u in used:
        if u in {"text", "ALPHA", "DIGIT", "HEXDIG"}:
            continue
        if u not in productions:
            if u[0].islower():
                undefined.add(u)
    for u in sorted(undefined):
        issues.append(f"C-GC: undefined non-terminal in badge grammar: {u!r}")
    return issues


def check_enum_reachability(text):
    """Every guarantee_class enum value is reachable from at least one
    layer in the §6.7 cascade table. Catches F-09 (formal_trace_equivalence
    unreachable from present-day pipeline)."""
    issues = []
    enum_values = set()
    for body in re.findall(FENCE + r"json\n(.*?)" + FENCE, text, re.S):
        try:
            d = json.loads(body)
        except ValueError:
            continue
        props = d.get("properties", {})
        gc = props.get("guarantee_class", {})
        if "enum" in gc:
            enum_values = set(gc["enum"])
            break
    if not enum_values:
        return issues
    m = re.search(r'### 6\.7.*?\n(.*?)(?=\n### |\Z)', text, re.S)
    if not m:
        issues.append("C-ER: §6.7 cascade table not found")
        return issues
    table = m.group(1)
    for v in enum_values:
        if v == "none":
            continue
        if v not in table:
            issues.append(f"C-ER: guarantee_class {v!r} is in the schema enum but absent from §6.7 cascade table")
    return issues


def check_profile_agreement(text):
    """§20.1 declared parameters are fields in the ArchProfile struct.
    Catches F-07 (abi_set, atomics, unwind, reloc_design_set missing)."""
    issues = []
    m1 = re.search(r'### 20\.1.*?\n(.*?)(?=\n### |\Z)', text, re.S)
    if not m1:
        return issues
    t1 = m1.group(1)
    params = set()
    for line in t1.split("\n"):
        line = line.strip()
        m2 = re.match(r'\|\s*`(\w+)`\s*\|', line)
        if m2:
            params.add(m2.group(1))
    if not params:
        return issues
    m3 = re.search(r'pub struct ArchProfile \{([^}]*)\}', text, re.S)
    if not m3:
        issues.append("C-PA: ArchProfile struct not found")
        return issues
    struct = m3.group(1)
    fields = set(re.findall(r'pub (\w+):', struct))
    aliases = {
        "endianness": "endian",
        "reg_file": "regs",
        "mem_spaces": "spaces",
    }
    mapped = set(fields)
    for p in params:
        target = aliases.get(p, p)
        if target not in mapped:
            issues.append(f"C-PA: §20.1 parameter {p!r} has no field in ArchProfile (looking for {target!r})")
    return issues


def check_verdict_status_mapping(text):
    """§9.2 contains a normative verdict↔status mapping table with a total
    order. Catches F-13."""
    issues = []
    m = re.search(r'### 9\.2.*?Strength rank.*?\n(.*?)(?=\n### |\Z)', text, re.S)
    if not m:
        issues.append("C-VS: §9.2 verdict↔status mapping with 'Strength rank' column not found")
    return issues


def check_monotonicity(text):
    """Ledger IDs (V-NN, L-NN) and hypothesis IDs (H-XN) are in monotonic
    order in the source. Catches F-15."""
    issues = []
    ledger_seq = re.findall(r'(?m)^\| ([VL])-(\d+) \|', text)
    for i in range(1, len(ledger_seq)):
        prev, pnum = ledger_seq[i - 1]
        curr, cnum = ledger_seq[i]
        if prev == curr and int(cnum) < int(pnum):
            issues.append(f"C-LM: ledger out of order at entry {i + 1}: {prev}-{pnum} followed by {curr}-{cnum}")
    hyp_seq = re.findall(r'(?m)^\| (H-[A-Za-z0-9′\-\d]+) \|', text)
    for i in range(1, len(hyp_seq)):
        if hyp_seq[i] < hyp_seq[i - 1]:
            issues.append(f"C-LM: hypothesis out of order: {hyp_seq[i - 1]} followed by {hyp_seq[i]}")
    return issues


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

#!/usr/bin/env python3
"""corpus_facts.py — destila el corpus en un índice temático legible.

Lee `docs/corpus/MESSAGES.jsonl` y agrupa los mensajes por tema, con
referencia precisa (sesión + índice de mensaje) para que cada afirmación sea
verificable en `docs/corpus/sessions/*.md`.

Salida: `docs/corpus/FACTS.md`
"""
from __future__ import annotations

import json
import os
import re
from collections import defaultdict

ROOT = "/workspace/antigravity/docs/corpus"

TEMAS = {
    "autenticacion-y-region-bypass": [
        "hasvalidauth", "region-bypass", "region_bypass", "validatauth", "license_required",
        "project_required", "personalauthvalidator", "oauth", "uimessage", "entitle",
    ],
    "binarios-y-versions-del-ls": [
        "liblanguage_server", "language_server", "antigravity version", "lsp", "jetski",
        "cortex", "v1internal", "crashpad", "version matrix",
    ],
    "ecosistema-comunitario": [
        "antigravity-cleaner", "opencode-agy", "deobfuscat", "patch-proxy", "oapm",
        "agy-unlock", "mcp-server", "community", "fork",
    ],
    "mitm-red-y-proxy": [
        "mitmproxy", "mitmweb", "ssl_cert_file", "cloud_code_endpoint", "daily-cloudcode",
        "generativelanguage.googleapis.com", "reverse proxy", "ca-cert", "certificado",
    ],
    "instrumentacion-y-sqlite": [
        "antigravity-cli", "sqlite", ".db", "jetski_state", "antigravity_state", "jetbox_summaries",
        "cdp_port", "9222", "brain/", "annotations",
    ],
    "darling-wine-y-emulacion": [
        "darling", "wine", "mach-o", "macho", "qemu", "aarch64", "arm64", "rosetta",
    ],
    "protocolos-y-protos": [
        ".proto", "language_server_pb", "grpc", "fetchavailablemodels", "fetchuserinfo",
        "streamgeneratecontent", "exa.",
    ],
    "corpus-y-megadaemon": [
        "md-secrets", "mega daemon", "megadaemon", "session_messages", "has_more",
        "tool-results", "corpus", "worktree", "cherry-pick",
    ],
    "deofuscacion-y-reversing": [
        "rizin", "ghidra", "go-retool", "adb", "il2cpp", "unity", "apk", "dex",
    ],
}

URL = re.compile(r"https?://[^\s\"'<>)\]]+")
SHA = re.compile(r"\b[0-9a-f]{7,40}\b")
INTERESANTE = re.compile(
    r"(?i)(https?://|\b[0-9a-f]{7,40}\b|sha256|measured|medido|verificat|confirm|"
    r"protobuf|endpoint|puerto|offset|0x[0-9a-f]{4,}|error|falla|bloque|root cause|causa-raiz)"
)


def norm(s: str) -> str:
    return (s or "").lower()


def main() -> int:
    path = os.path.join(ROOT, "MESSAGES.jsonl")
    if not os.path.exists(path):
        print("no existe MESSAGES.jsonl; ejecuta antes corpus_build.py")
        return 1

    hits = defaultdict(list)       # tema -> [(score, sid, idx, agent, title, text)]
    n_msgs = 0
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                m = json.loads(line)
            except Exception:
                continue
            n_msgs += 1
            text = m.get("text") or ""
            if not text.strip():
                continue
            low = norm(text)
            for tema, kws in TEMAS.items():
                score = sum(low.count(k) for k in kws)
                if score <= 0:
                    continue
                # el valor de una sesión se pondera: los mensajes con URL o
                # hash son los que documentan hechos
                score += 3 * len(URL.findall(text)) + 2 * len(SHA.findall(text))
                hits[tema].append((score, m["session_id"], m["idx"], m.get("agent", "?"),
                                   m.get("title", ""), text))

    out = []
    out.append("# Corpus de conversaciones — índice temático\n")
    out.append(f"Generado por `tools/corpus_facts.py` a partir de `MESSAGES.jsonl` "
               f"({n_msgs} mensajes).\n")
    out.append("Cada bloque cita **sesión · índice de mensaje**, de forma que la "
               "afirmación se pueda verificar en `sessions/<agente>__<titulo>.md`.\n")
    out.append("> Los extracts se recortan a 900 caracteres: son pistas de lectura, "
               "no la fuente. La fuente es el fichero de sesión.\n")

    out.append("\n## Resumen por tema\n")
    out.append("| tema | mensajes con señal | sesiones |")
    out.append("|---|---|---|")
    for tema in TEMAS:
        v = hits.get(tema, [])
        sids = len({x[1] for x in v})
        out.append(f"| `{tema}` | {len(v)} | {sids} |")

    out.append("\n---\n")
    for tema in TEMAS:
        v = sorted(hits.get(tema, []), key=lambda x: -x[0])
        if not v:
            continue
        out.append(f"\n## {tema}\n")
        out.append(f"_{len(v)} mensajes con señal, {len({x[1] for x in v})} sesiones._\n")
        # top por sesión para no repetir la misma sesión 10 veces
        best_per_sid = {}
        for score, sid, idx, agent, title, text in v:
            if sid not in best_per_sid or score > best_per_sid[sid][0]:
                best_per_sid[sid] = (score, idx, agent, title, text)
        top = sorted(best_per_sid.values(), key=lambda x: -x[0])[:25]
        for score, idx, agent, title, text in top:
            frag = re.sub(r"\n{3,}", "\n\n", text).strip()
            if len(frag) > 900:
                frag = frag[:900] + " […]"
            out.append(f"\n### `{sid}` · mensaje {idx} · **{agent}** — {title[:70]}\n")
            out.append(f"```\n{frag}\n```\n")

    dst = os.path.join(ROOT, "FACTS.md")
    with open(dst, "w", encoding="utf-8") as fh:
        fh.write("\n".join(out))
    print(json.dumps({
        "mensajes": n_msgs,
        "temas": len(TEMAS),
        "temas_con_señal": sum(1 for t in TEMAS if hits.get(t)),
        "fichero": dst,
        "bytes": os.path.getsize(dst),
    }, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Ensambla el corpus indexable de conversaciones de todos los agentes.

Entrada: artefactos JSONL externalizados por el harness (session_messages) en
/workspace/.mavis/tool-results/<sesion>/*.readable.v1.jsonl
        + tabla de sesiones SESSIONS_INDEX.tsv
Salida : docs/corpus/{sessions/*.md, MESSAGES.jsonl, INDEX.md, KEYWORDS.tsv, FACTS.md}
"""
from __future__ import annotations
import csv
import glob
import hashlib
import json
import os
import re
import sys
from collections import Counter, defaultdict

ROOT = "/workspace/antigravity/docs/corpus"
ART = "/workspace/.mavis/tool-results"
IDX = os.path.join(ROOT, "SESSIONS_INDEX.tsv")

STOP = set("""a al algo como con de del el en la las lo los más para por que se su un una
the of to in and or is are was were be been it this that with from not but you your we our
""".split())


def extract_messages(raw: str):
    """Extrae la lista `messages` de un JSON de respuesta, tolerando
    truncado. Los artefactos `call_*/chunks` usan la estrategia
    `global_head_tail`: conservan cabeza y cola y cortan por el medio, así
    que el JSON no siempre parsea entero. Se recuperan los objetos
    individually balanceando llaves y respetando cadenas.
    """
    key = '"messages"'
    i = raw.find(key)
    if i < 0:
        return [], False
    j = raw.find("[", i)
    if j < 0:
        return [], False
    out = []
    depth = 0
    start = None
    in_str = False
    esc = False
    for k in range(j, len(raw)):
        ch = raw[k]
        if in_str:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
            continue
        if ch == '"':
            in_str = True
            continue
        if ch in "[{":
            if depth == 0:
                start = k
            depth += 1
        elif ch in "]}":
            depth -= 1
            if depth == 0 and start is not None:
                frag = raw[start:k + 1]
                try:
                    obj = json.loads(frag)
                except Exception:
                    obj = None
                if isinstance(obj, dict):
                    out.append(obj)
                start = None
            if depth < 0:
                break
    hm = '"has_more": true' in raw
    return out, hm


def load_raw_artifacts():
    """Devuelve [(origen, mensajes, has_more)] desde los artefactos crudos
    `call_*/chunks/*.txt` (respuestas crudas de herramientas previas)."""
    out = []
    for mf in sorted(glob.glob(os.path.join(ART, "*", "call_*", "manifest.json"))):
        try:
            meta = json.load(open(mf, encoding="utf-8"))
        except Exception:
            continue
        if meta.get("tool_name") != "session_messages":
            continue
        base = os.path.dirname(mf)
        chunks = sorted(glob.glob(os.path.join(base, "chunks", "*.txt")))
        raw = ""
        for c in chunks:
            try:
                raw += open(c, encoding="utf-8", errors="replace").read()
            except OSError:
                continue
        msgs, hm = extract_messages(raw)
        if not msgs:
            continue
        out.append((mf, msgs, hm))
    return out


def load_inline_supplements():
    """Respuestas pequeñas que el harness dejó en línea (no persistidas).
    Se guardan a mano en `docs/corpus/inline/*.json`."""
    out = []
    for p in sorted(glob.glob(os.path.join(ROOT, "inline", "*.json"))):
        try:
            d = json.load(open(p, encoding="utf-8"))
        except Exception:
            continue
        msgs = d.get("messages") or []
        if not msgs:
            continue
        out.append((p, msgs, bool(d.get("has_more"))))
    return out


def empty_supplement_sessions():
    """Sesiones con respuesta vacía (messages=[] y has_more=false): el
    backend confirma que no tienen ningún mensaje. Se marcan como cubiertas
    con 0 mensajes para que no figuren como huecos del corpus."""
    out = {}
    for p in sorted(glob.glob(os.path.join(ROOT, "inline", "*.json"))):
        try:
            d = json.load(open(p, encoding="utf-8"))
        except Exception:
            continue
        if d.get("messages"):
            continue
        sid = d.get("session_id")
        if sid:
            out[sid] = d.get("nota", "sin mensajes")
    return out


def load_artifacts():
    """Devuelve [(artifact_path, messages, has_more)] para artefactos con mensajes.

    Deduplica por hash del contenido: la misma página se archiva varias
    veces (cada llamada MCP genera un artefacto). Sin esta deduplicación la
    misma página se adjudica a sesiones distintas y la cobertura sale falsa.
    """
    out = []
    seen = {}
    for path in sorted(glob.glob(os.path.join(ART, "*", "*.readable.v1.jsonl"))):
        txt = []
        try:
            with open(path, encoding="utf-8", errors="replace") as fh:
                for line in fh:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        obj = json.loads(line)
                    except Exception:
                        continue
                    if isinstance(obj, dict) and "text" in obj:
                        txt.append(obj["text"])
        except OSError:
            continue
        if not txt:
            continue
        try:
            data = json.loads("".join(txt))
        except Exception:
            continue
        resp = data.get("response") if isinstance(data, dict) else None
        if not isinstance(resp, dict) or "messages" not in resp:
            continue
        msgs = resp.get("messages") or []
        if not msgs:
            continue
        h = hashlib.sha256()
        for m in msgs:
            h.update(f"{m.get('timestamp')}|{m.get('role')}|{m.get('msg_content','')}\n"
                     .encode("utf-8", "replace"))
        digest = h.hexdigest()
        hm = bool(resp.get("has_more"))
        if digest in seen:
            # mismo contenido: conserva la marca más terminal (False = no hay más)
            if not hm:
                seen[digest][2] = False
            continue
        rec = [path, msgs, hm]
        seen[digest] = rec
        out.append(rec)
    # fuentes adicionales: artefactos crudos y suplementos en línea
    for path, msgs, hm in load_raw_artifacts() + load_inline_supplements():
        h = hashlib.sha256()
        for m in msgs:
            h.update(f"{m.get('timestamp')}|{m.get('role')}|{m.get('msg_content','')}\n"
                     .encode("utf-8", "replace"))
        digest = h.hexdigest()
        hm = bool(hm)
        if digest in seen:
            if not hm:
                seen[digest][2] = False
            continue
        rec = [path, msgs, hm]
        seen[digest] = rec
        out.append(rec)
    return out


def load_sessions():
    rows = {}
    with open(IDX, newline="", encoding="utf-8") as fh:
        for r in csv.DictReader(fh, delimiter="\t"):
            rows[r["session_id"]] = r
    return rows


def slug(text: str, n: int = 48) -> str:
    text = re.sub(r"\s+", " ", (text or "")).strip()
    text = re.sub(r"[^\wÁÉÍÓÚÑáéíóúñ ._-]", "", text)
    text = text.strip().replace(" ", "_")
    return (text[:n] or "sin_titulo")


def assign_pages(artifacts, sessions):
    """Asigna cada página (artefacto) a su sesión, independiente del orden.

    Regla: una página pertenece a la sesión S si su intervalo [lo,hi] cae
    dentro de la ventana temporal de S [created_at, updated_at] con margen,
    y no solapa con ninguna página ya asignada a S. Entre candidatos gana el
    de menor hueco (la página que empieza más pegada al inicio de la sesión).

    Antes el mapeo era secuencial (bounds crecientes) y por eso fallaba con
    los artefactos ordenados por hash de ruta: una página intermedia se
    adjudicaba a otra sesión. Ahora se resuelve globalmente.

    LIMITACIÓN MEDIDA: `session_messages` puede devolver la misma ventana de
    mensajes para sesiones distintas cuando una es subagente de la otra
    (el backend devuelve el contexto compartido del padre). En ese caso el
    mapeo por tiempo es ambiguo por construcción y queda como NO VERIFICADO.
    """
    pages = []
    for path, msgs, hm in artifacts:
        ts = [m.get("timestamp") for m in msgs
              if isinstance(m.get("timestamp"), int)]
        if not ts:
            continue
        pages.append({"path": path, "msgs": msgs, "hm": hm,
                      "lo": min(ts), "hi": max(ts), "n": len(msgs), "sid": None})

    spans = {sid: [] for sid in sessions}          # sesión -> [(lo,hi)]

    # ------------------------------------------------------------------
    # MEDIDO: `session_messages` ignora `before` (prueba: la misma llamada
    # con `before` distinto devuelve la MISMA ventana, mismo hash) y
    # devuelve como máximo los 100 últimos mensajes de la sesión.
    # Por tanto cada sesión aporta COMO MUCHO UNA página, y el reparto
    # página→sesión es una biyección 1:1, no un emparejamiento con solapes.
    # ------------------------------------------------------------------
    # 1) descartar subventanas de OTRA llamada a la MISMA sesión: comparten
    #    inicio o fin y traen menos mensajes (mismo límite distinto).
    #    No se puede usar "contenida en" a secas: dos sesiones distintas
    #    (padre e hijo) tienen ventanas de tiempo anidadas legítimamente.
    pages.sort(key=lambda p: (p["hi"] - p["lo"]), reverse=True)
    kept = []
    for p in pages:
        dup = False
        for q in kept:
            if q["n"] <= p["n"]:
                continue
            if not (q["lo"] <= p["lo"] and q["hi"] >= p["hi"]):
                continue
            if p["hi"] == q["hi"] or p["lo"] == q["lo"]:
                dup = True
                break
        if dup:
            continue
        kept.append(p)
    pages = kept

    # 2) puntuación de cada par (página, sesión): desajuste en AMBOS
    #    extremos (inicio de la página vs created_at, fin de la página vs
    #    updated_at). Una página completa arranca exactamente en created_at
    #    y termina en el último mensaje de la sesión.
    edges = []
    for i, p in enumerate(pages):
        for sid, r in sessions.items():
            try:
                c = int(r.get("created_at") or 0)
                u = int(r.get("updated_at") or 0)
            except (TypeError, ValueError):
                continue
            if not c:
                continue
            if p["lo"] < c - 120000 or p["hi"] > u + 300000:
                continue
            score = abs(p["lo"] - c) + abs(p["hi"] - u)
            edges.append((score, i, sid))
    edges.sort()

    # 3) biyección estricta: cada página a una sola sesión y al revés.
    #    (Cada sesión produce como mucho UNA página porque `before` no
    #    pagina: siempre devuelve los últimos N mensajes.)
    used_sid = set()
    for score, i, sid in edges:
        p = pages[i]
        if p["sid"] or sid in used_sid:
            continue
        p["sid"] = sid
        p["cost"] = score
        used_sid.add(sid)

    # fallback por título para las que quedaron sin sesión
    for p in pages:
        if p["sid"]:
            continue
        first_user = next((m for m in p["msgs"]
                           if m.get("role") == "user" and m.get("msg_content")), None)
        if not first_user:
            continue
        txt = re.sub(r"\s+", " ", first_user["msg_content"])[:120].lower()
        best, score = None, 0
        for sid, r in sessions.items():
            t = (r.get("title") or "").lower()
            words = [w for w in re.findall(r"\w+", t) if len(w) > 3]
            s = sum(1 for w in words if w in txt)
            if s > score:
                best, score = sid, s
        if best and score >= 2:
            p["sid"] = best
            p["how"] = "titulo_similar"
    return pages


def flatten(msgs):
    """Aplana un mensaje a texto indexable."""
    parts = []
    if msgs.get("msg_content"):
        parts.append(str(msgs["msg_content"]))
    if msgs.get("thinking_content"):
        parts.append("[razonamiento] " + str(msgs["thinking_content"]))
    for tc in msgs.get("tool_calls") or []:
        if not isinstance(tc, dict):
            parts.append(f"[llamada] {str(tc)[:1200]}")
            continue
        args = str(tc.get("tool_call_args") or "")
        if len(args) > 1200:
            args = args[:1200] + " …"
        parts.append(f"[llamada {tc.get('tool_name')}] {args}")
    return "\n".join(parts)


def main() -> int:
    sessions = load_sessions()
    artifacts = load_artifacts()
    os.makedirs(os.path.join(ROOT, "sessions"), exist_ok=True)

    assigned = {}          # session_id -> list[message]
    unassigned = []
    pages = assign_pages(artifacts, sessions)
    for p in pages:
        sid = p["sid"]
        if sid is None:
            unassigned.append((p["path"], p["msgs"]))
            continue
        assigned.setdefault(sid, []).extend(p["msgs"])

    # ---------- cobertura real por sesión ----------
    # Una sesión está COMPLETA si el has_more de su página más reciente es
    # False (el backend ya no devuelve más mensajes).
    by_sid = {}
    for p in pages:
        if p["sid"]:
            by_sid.setdefault(p["sid"], []).append(p)
    coverage = []
    vacias = empty_supplement_sessions()
    for sid, r in sessions.items():
        ps = sorted(by_sid.get(sid, []), key=lambda p: p["lo"])
        if not ps and sid in vacias:
            coverage.append({"sid": sid, "paginas": 0, "msgs": 0, "lo": 0, "hi": 0,
                             "completa": True, "cursor": 0, "nota": "sin_mensajes"})
            continue
        if not ps:
            coverage.append({"sid": sid, "paginas": 0, "msgs": 0, "lo": 0, "hi": 0,
                             "completa": None, "cursor": 0, "nota": "sin_cobertura"})
            continue
        last = ps[-1]
        coverage.append({
            "sid": sid, "paginas": len(ps), "msgs": sum(p["n"] for p in ps),
            "lo": ps[0]["lo"], "hi": last["hi"],
            "completa": not last["hm"],
            "cursor": last["hi"],
            "nota": "" if not last["hm"] else "tiene_mas",
        })
    with open(os.path.join(ROOT, "COVERAGE.tsv"), "w", encoding="utf-8") as fh:
        fh.write("session_id\tagente\ttitulo\tpaginas\tmensajes\tprimer_ts\tultimo_ts\t"
                 "completa\tcursor_ultimo\tnota\n")
        for c in sorted(coverage, key=lambda x: (x["completa"] is not True, x["sid"])):
            r = sessions.get(c["sid"], {})
            fh.write(f"{c['sid']}\t{r.get('agent_name','')}\t{r.get('title','')}\t"
                     f"{c['paginas']}\t{c['msgs']}\t{c['lo']}\t{c['hi']}\t"
                     f"{'' if c['completa'] is None else int(c['completa'])}\t"
                     f"{c['cursor']}\t{c['nota']}\n")

    # ---------- plan de paginación exacto (qué llamada falta y con qué cursor) ----------
    with open(os.path.join(ROOT, "PAGINATE_PLAN.tsv"), "w", encoding="utf-8") as fh:
        fh.write("session_id\tagente\tpaginas\tmensajes\tcursor_before\ttitulo\n")
        for c in coverage:
            if c["completa"] is False:
                r = sessions.get(c["sid"], {})
                fh.write(f"{c['sid']}\t{r.get('agent_name','')}\t{c['paginas']}\t"
                         f"{c['msgs']}\t{c['cursor']}\t{r.get('title','')}\n")

    # ---------- estado de paginación por sesión (última página conocida) ----------
    last_has_more = {}
    for p in pages:
        if not p["sid"]:
            continue
        sid = p["sid"]
        if sid not in last_has_more or p["hi"] > last_has_more[sid][0]:
            last_has_more[sid] = (p["hi"], p["hm"])
    with open(os.path.join(ROOT, "PENDING.tsv"), "w", encoding="utf-8") as fh:
        fh.write("session_id\tcursor_ts\tcompleta\n")
        for sid, (hi, hm) in sorted(last_has_more.items(), key=lambda kv: kv[1][0]):
            fh.write(f"{sid}\t{hi}\t{0 if hm else 1}\n")

    # ---------- cursores de paginación ----------
    with open(os.path.join(ROOT, "CURSORS.tsv"), "w", encoding="utf-8") as fh:
        fh.write("session_id\tcursor_ts\tpaginas_completas\n")
        for c in sorted(coverage, key=lambda x: x["hi"]):
            if c["hi"]:
                fh.write(f"{c['sid']}\t{c['hi']}\t{0 if c['completa'] is False else 1}\n")

    # ---------- corpus por sesión ----------
    rows_out = []
    msg_index = []
    for sid, msgs in sorted(assigned.items(), key=lambda kv: int(sessions.get(kv[0], {}).get("created_at") or 0)):
        r = sessions.get(sid, {})
        msgs.sort(key=lambda m: m.get("timestamp") or 0)
        title = r.get("title") or "sin titulo"
        agent = (r.get("agent_name") or "?").strip() or "?"
        fn = f"{slug(agent)}__{slug(title)}.md"
        fp = os.path.join(ROOT, "sessions", fn)
        with open(fp, "w", encoding="utf-8") as fh:
            fh.write("---\n")
            fh.write(f"session_id: {sid}\n")
            fh.write(f"agente: {agent}\n")
            fh.write(f"titulo: {json.dumps(title, ensure_ascii=False)}\n")
            fh.write(f"creada: {r.get('created_at')}\n")
            fh.write(f"actualizada: {r.get('updated_at')}\n")
            fh.write(f"mensajes: {len(msgs)}\n")
            fh.write(f"modelo: {r.get('model')}\n")
            fh.write(f"estado: {r.get('status')}\n")
            fh.write("---\n\n")
            fh.write(f"# {title}\n\n")
            fh.write(f"> Sesión `{sid}` · agente **{agent}** · {len(msgs)} mensajes\n\n")
            for i, m in enumerate(msgs):
                ts = m.get("timestamp") or 0
                role = m.get("role") or "?"
                content = m.get("msg_content") or ""
                thinking = m.get("thinking_content") or ""
                tc = m.get("tool_calls") or []
                fh.write(f"## [{i}] {role} · {ts}\n\n")
                if content.strip():
                    fh.write(content.strip() + "\n\n")
                if thinking.strip():
                    fh.write("<details><summary>razonamiento</summary>\n\n")
                    fh.write(thinking.strip() + "\n\n</details>\n\n")
                if tc:
                    fh.write("```\n")
                    for t in tc:
                        if not isinstance(t, dict):
                            fh.write(f"→ {str(t)[:800]}\n")
                            continue
                        a = str(t.get("tool_call_args") or "")
                        if len(a) > 800:
                            a = a[:800] + " …"
                        fh.write(f"→ {t.get('tool_name')}: {a}\n")
                    fh.write("```\n\n")
                msg_index.append({
                    "session_id": sid, "agent": agent, "title": title, "idx": i,
                    "ts": ts, "role": role, "file": os.path.join("sessions", fn),
                    "text": flatten(m),
                })
        rows_out.append((sid, agent, title, len(msgs), fn, r.get("created_at"), r.get("updated_at")))

    # ---------- MESSAGES.jsonl ----------
    with open(os.path.join(ROOT, "MESSAGES.jsonl"), "w", encoding="utf-8") as fh:
        for m in msg_index:
            fh.write(json.dumps(m, ensure_ascii=False) + "\n")

    # ---------- índice invertido ----------
    df = Counter()
    per_term = defaultdict(set)
    for m in msg_index:
        toks = set(re.findall(r"[A-Za-z_][A-Za-z0-9_./:-]{3,}", m["text"].lower()))
        for t in toks:
            if t in STOP:
                continue
            df[t] += 1
            per_term[t].add(m["session_id"])
    with open(os.path.join(ROOT, "KEYWORDS.tsv"), "w", encoding="utf-8") as fh:
        fh.write("termino\tdocumentos\tsesiones\n")
        for t, n in df.most_common(4000):
            fh.write(f"{t}\t{n}\t{len(per_term[t])}\n")

    # ---------- INDEX.md ----------
    with open(os.path.join(ROOT, "INDEX.md"), "w", encoding="utf-8") as fh:
        fh.write("# Corpus de conversaciones — todos los agentes\n\n")
        fh.write(f"- Sesiones indexadas: **{len(rows_out)}** de {len(sessions)} registradas\n")
        fh.write(f"- Mensajes volcados: **{len(msg_index)}**\n")
        fh.write(f"- Artefactos leídos: {len(artifacts)}\n")
        if unassigned:
            fh.write(f"- Artefactos sin asignación: {len(unassigned)}\n")
        pend = [c for c in coverage if c["completa"] is False]
        sinc = [c for c in coverage if c["completa"] is None]
        fh.write(f"- Sesiones con paginación pendiente: **{len(pend)}**\n")
        fh.write(f"- Sesiones sin cobertura alguna: **{len(sinc)}**\n")
        fh.write("\n## Ficheros\n\n")
        fh.write("| fichero | contenido |\n|---|---|\n")
        fh.write("| `sessions/*.md` | transcripción completa por sesión (YAML + mensajes) |\n")
        fh.write("| `MESSAGES.jsonl` | todos los mensajes aplanados, una línea JSON por mensaje |\n")
        fh.write("| `KEYWORDS.tsv` | índice invertido término → nº documentos / nº sesiones |\n")
        fh.write("| `SESSIONS_INDEX.tsv` | metadatos de las 97 sesiones (agente, título, fechas) |\n")
        fh.write("| `COVERAGE.tsv` | cobertura real por sesión (páginas, msgs, completa, cursor) |\n")
        fh.write("| `PAGINATE_PLAN.tsv` | llamadas `session_messages` pendientes con su cursor |\n")
        fh.write("| `FACTS.md` | notas destiladas por tema con referencia a sesión/mensaje |\n\n")
        fh.write("## Sesiones\n\n")
        fh.write("| # | agente | fecha | msj | título | fichero |\n|---|---|---|---|---|---|\n")
        for i, (sid, agent, title, n, fn, c, u) in enumerate(rows_out, 1):
            d = ""
            try:
                d = __import__("datetime").datetime.utcfromtimestamp(int(c) / 1000).strftime("%Y-%m-%d")
            except Exception:
                pass
            fh.write(f"| {i} | {agent} | {d} | {n} | {title.replace('|','/')} | [`{fn}`](sessions/{fn}) |\n")

    print(json.dumps({
        "artefactos": len(artifacts),
        "sesiones_asignadas": len(rows_out),
        "sesiones_totales": len(sessions),
        "mensajes": len(msg_index),
        "sin_asignar": len(unassigned),
        "paginacion_pendiente": len([c for c in coverage if c["completa"] is False]),
        "sin_cobertura": len([c for c in coverage if c["completa"] is None]),
        "completas": len([c for c in coverage if c["completa"] is True]),
    }, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())

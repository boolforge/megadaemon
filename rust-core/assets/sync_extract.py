#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MegaDaemon v13.3 — Extractor de conversaciones (parte Python del motor de sync).

Este fichero va EMBEBIDO en el binario de megadaemon (`include_str!`) y se
escribe en el directorio de estado en tiempo de ejecución.  Se ejecuta con el
`python3` del dispositivo porque SQLite y el análisis de JSON nativo de las
apps (esquemas no documentados) cambian en cada versión; el resto de la
lógica —orquestación, normalización, almacén, índice y búsqueda— vive en
Rust, en `src/sync.rs`.

Salida: NDJSON con una conversación por línea.

    {"service":"claude","id":"...","title":"...","updated_at":1700000000,
     "model":"...","messages":[{"role":"user","content":"...","ts":1700,
     "meta":{"blocks":2}}],"source":"/data/data/.../x.db"}

Contrato:
  * Nunca lanza excepción hacia fuera: los errores se emiten en el flujo de
    error (`{"__error__": ...}`) y el proceso termina con 0.
  * Copia cada DB a un temporal antes de leerla: las apps escriben a diario
    y leer en caliente puede dar un snapshot incoherente.
"""
from __future__ import annotations

import glob
import json
import os
import re
import shutil
import sqlite3
import sys
import tempfile
import time

SNAP_DIR = "/data/local/tmp/megadaemon/sync_snap"
MAX_MSG_CHARS = 200000


# --------------------------------------------------------------------------- #
# utilidades
# --------------------------------------------------------------------------- #
def emit(obj) -> None:
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")


def emit_error(service: str, exc: Exception) -> None:
    emit({"__error__": True, "service": service, "error": str(exc)[:500]})


def snapshot_db(path: str) -> str:
    """Copia la DB (y su -wal) a un temporal para leer un snapshot coherente."""
    os.makedirs(SNAP_DIR, exist_ok=True)
    base = os.path.basename(path).replace("/", "_")
    dst = os.path.join(SNAP_DIR, "%d_%s" % (int(time.time() * 1000) % 10_000_000, base))
    try:
        shutil.copyfile(path, dst)
    except Exception:
        return path
    for suffix in ("-wal", "-shm"):
        side = path + suffix
        if os.path.exists(side):
            try:
                shutil.copyfile(side, dst + suffix)
            except Exception:
                pass
    return dst


def ro(db: str):
    con = sqlite3.connect("file:%s?mode=ro" % db, uri=True, timeout=5)
    con.row_factory = sqlite3.Row
    return con


def tables(con) -> list:
    return [r[0] for r in con.execute("select name from sqlite_master where type='table'")]


def as_text(v) -> str:
    if v is None:
        return ""
    if isinstance(v, bytes):
        try:
            v = v.decode("utf-8", "replace")
        except Exception:
            return ""
    s = str(v)
    return s[:MAX_MSG_CHARS]


def jload(v, default=None):
    if v is None:
        return default
    if isinstance(v, (bytes, bytearray)):
        v = v.decode("utf-8", "replace")
    if isinstance(v, (dict, list)):
        return v
    try:
        return json.loads(v)
    except Exception:
        return default


def clean(s: str) -> str:
    return " ".join(as_text(s).split())


def title_from_text(text: str, n: int = 70) -> str:
    t = clean(text)
    return t[:n] + ("…" if len(t) > n else "")


# --------------------------------------------------------------------------- #
# Claude — conversation_snapshots.db
#   snapshot_headers : account_id, conversation_id, leaf_message_id, updated_at...
#   snapshot_messages: ... fingerprint, complete_blocks, body(JSON)
# --------------------------------------------------------------------------- #
def extract_claude(path: str) -> list:
    out = []
    db = snapshot_db(path)
    con = ro(db)
    try:
        headers = {}
        for r in con.execute("select conversation_id, updated_at, newest_message_at from snapshot_headers"):
            headers[r["conversation_id"]] = {
                "updated_at": (r["updated_at"] or 0) / 1000.0,
                "newest_at": (r["newest_message_at"] or 0) / 1000.0,
            }
        msgs = {}
        for r in con.execute(
            "select conversation_id, message_id, parent_message_id, body from snapshot_messages"
        ):
            body = jload(r["body"], {}) or {}
            sender = body.get("sender") or ("user" if body.get("input_mode") else "assistant")
            role = "user" if sender == "human" else ("assistant" if sender == "assistant" else sender)
            parts, blocks = [], 0
            for blk in body.get("blocks") or []:
                content = blk.get("content") or {}
                b = content.get("block") or {}
                btype = b.get("type")
                blocks += 1
                if btype == "text" and b.get("text"):
                    parts.append(b["text"])
                elif btype == "thinking":
                    th = (b.get("thinking") or "").strip()
                    if th:
                        parts.append("[razonamiento] " + th)
                    for s in b.get("summaries") or []:
                        if s.get("summary"):
                            parts.append("[resumen] " + s["summary"])
                elif btype == "tool_use":
                    parts.append("[tool:%s] %s" % (b.get("name", "?"), as_text(b.get("input"))[:2000]))
                elif btype == "tool_result":
                    parts.append("[resultado tool] %s" % as_text(b.get("content"))[:2000])
            text = "\n".join(p for p in parts if p).strip()
            if not text:
                continue
            msgs.setdefault(r["conversation_id"], []).append(
                {
                    "role": role,
                    "content": text,
                    "ts": (body.get("created_at") or 0) / 1000.0,
                    "id": r["message_id"],
                    "parent": r["parent_message_id"],
                    "meta": {"blocks": blocks},
                }
            )
        for cid, lst in msgs.items():
            lst.sort(key=lambda m: (m.get("ts") or 0, m.get("id") or ""))
            title = ""
            for m in lst:
                if m["role"] == "user":
                    title = title_from_text(m["content"])
                    break
            meta = headers.get(cid, {})
            out.append(
                {
                    "service": "claude",
                    "id": cid,
                    "title": title or "Conversación Claude",
                    "updated_at": meta.get("updated_at") or (lst[-1].get("ts") if lst else 0),
                    "model": "",
                    "messages": lst,
                    "source": path,
                }
            )
    finally:
        con.close()
        try:
            os.unlink(db)
        except Exception:
            pass
    return out


# --------------------------------------------------------------------------- #
# DeepSeek — chat_session_list + chat_session_messages_<id>
#   fragments = JSON array: REQUEST / THINK / RESPONSE / SEARCH / TOOL_SEARCH
# --------------------------------------------------------------------------- #
def extract_deepseek(path: str) -> list:
    out = []
    db = snapshot_db(path)
    con = ro(db)
    try:
        sessions = {}
        for r in con.execute("select id, title, inserted_at, updated_at, model_type from chat_session_list"):
            sessions[r["id"]] = {
                "title": r["title"] or "",
                "inserted_at": r["inserted_at"] or 0,
                "updated_at": r["updated_at"] or 0,
                "model": r["model_type"] or "",
            }
        for tname in tables(con):
            if not tname.startswith("chat_session_messages_"):
                continue
            sid = tname[len("chat_session_messages_"):]
            rows = list(con.execute('select message_id, role, fragments, inserted_at from "%s"' % tname))
            if not rows:
                continue
            meta = sessions.get(sid, {})
            msgs = []
            for r in rows:
                role = (r["role"] or "").lower()
                role = {"user": "user", "assistant": "assistant", "system": "system"}.get(role, role or "user")
                frags = jload(r["fragments"], []) or []
                if isinstance(frags, dict):
                    frags = [frags]
                parts = []
                for fr in frags:
                    if not isinstance(fr, dict):
                        continue
                    ftype = (fr.get("type") or "").upper()
                    content = fr.get("content")
                    if ftype == "THINK":
                        if content:
                            parts.append("[razonamiento] " + as_text(content))
                    elif ftype == "SEARCH":
                        if content:
                            parts.append("[búsqueda] " + as_text(content))
                    elif ftype == "TOOL_SEARCH":
                        results = fr.get("results") or []
                        for res in results[:6]:
                            parts.append(
                                "[resultado %s] %s" % (res.get("title", "?"), as_text(res.get("snippet"))[:500])
                            )
                    else:
                        if content:
                            parts.append(as_text(content))
                text = "\n".join(p for p in parts if p).strip()
                if not text:
                    continue
                msgs.append(
                    {
                        "role": role,
                        "content": text,
                        "ts": float(r["inserted_at"] or 0),
                        "id": r["message_id"],
                        "meta": {"fragments": len(frags)},
                    }
                )
            if not msgs:
                continue
            msgs.sort(key=lambda m: (m.get("ts") or 0, m.get("id") or 0))
            out.append(
                {
                    "service": "deepseek",
                    "id": sid,
                    "title": meta.get("title") or title_from_text(msgs[0]["content"]),
                    "updated_at": meta.get("updated_at") or msgs[-1]["ts"],
                    "model": meta.get("model", ""),
                    "messages": msgs,
                    "source": path,
                }
            )
    finally:
        con.close()
        try:
            os.unlink(db)
        except Exception:
            pass
    return out


# --------------------------------------------------------------------------- #
# Kimi — DBChatItem / DBChatMessage(extra=JSON)
# --------------------------------------------------------------------------- #
def extract_kimi(path: str) -> list:
    out = []
    db = snapshot_db(path)
    con = ro(db)
    try:
        chats = {}
        try:
            for r in con.execute("select id, name, update_time, extra from DBChatItem"):
                chats[r["id"]] = {"name": r["name"] or "", "update_time": r["update_time"] or 0}
        except Exception:
            pass
        by_chat = {}
        for r in con.execute("select id, chat_id, event_offset, status, extra from DBChatMessage"):
            extra = jload(r["extra"], {}) or {}
            role_code = extra.get("role")
            role = {1: "assistant", 2: "user", 3: "system"}.get(role_code, "user")
            parts = []
            for blk in extra.get("blocks") or []:
                if not isinstance(blk, dict):
                    continue
                for key in ("text", "think"):
                    node = blk.get(key)
                    if isinstance(node, dict) and node.get("content"):
                        prefix = "[razonamiento] " if key == "think" else ""
                        parts.append(prefix + as_text(node["content"]))
                for key in ("search", "tool", "file"):
                    node = blk.get(key)
                    if isinstance(node, dict) and node.get("content"):
                        parts.append("[%s] %s" % (key, as_text(node["content"])[:1500]))
            text = "\n".join(p for p in parts if p).strip()
            if not text:
                continue
            by_chat.setdefault(r["chat_id"], []).append(
                {
                    "role": role,
                    "content": text,
                    "ts": 0,
                    "id": r["id"],
                    "meta": {"status": r["status"], "offset": r["event_offset"]},
                }
            )
        for cid, msgs in by_chat.items():
            msgs.sort(key=lambda m: m.get("id") or "")
            meta = chats.get(cid, {})
            out.append(
                {
                    "service": "kimi",
                    "id": cid,
                    "title": meta.get("name") or title_from_text(msgs[0]["content"]),
                    "updated_at": meta.get("update_time") or 0,
                    "model": "",
                    "messages": msgs,
                    "source": path,
                }
            )
    finally:
        con.close()
        try:
            os.unlink(db)
        except Exception:
            pass
    return out


# --------------------------------------------------------------------------- #
# ChatGPT — DBConversation / DBMessage / DBMessageChunk(chunk=JSON)
# --------------------------------------------------------------------------- #
def extract_chatgpt(path: str) -> list:
    out = []
    db = snapshot_db(path)
    con = ro(db)
    try:
        convs = {}
        for r in con.execute("select id, conversation from DBConversation"):
            c = jload(r["conversation"], {}) or {}
            convs[r["id"]] = {
                "title": c.get("title") or "",
                "created": c.get("creation_date") or "",
                "modified": c.get("modification_date") or "",
                "model": c.get("default_model_slug") or "",
            }
        chunks = {}
        for r in con.execute("select messageId, chunkIndex, chunk from DBMessageChunk order by messageId, chunkIndex"):
            body = jload(r["chunk"], {}) or {}
            role = (body.get("role") or "user").lower()
            if role == "tool":
                role = "tool"
            content = body.get("content") or {}
            text = as_text(content.get("content")) if isinstance(content, dict) else as_text(content)
            if not text:
                text = as_text(body.get("text"))
            if not text:
                continue
            chunks.setdefault(r["messageId"], []).append(
                {
                    "role": role,
                    "content": text,
                    "ts": 0,
                    "model": body.get("model") or "",
                    "meta": {"type": content.get("type") if isinstance(content, dict) else ""},
                }
            )
        # DBMessage liga messageId -> conversationId
        link = {}
        try:
            for r in con.execute("select id, conversationId from DBMessage"):
                link[r["id"]] = r["conversationId"]
        except Exception:
            pass
        by_conv = {}
        for mid, msgs in chunks.items():
            cid = link.get(mid)
            if cid is None:
                cid = mid
            by_conv.setdefault(cid, []).extend(msgs)
        for cid, msgs in by_conv.items():
            meta = convs.get(cid, {})
            msgs.sort(key=lambda m: (m.get("model") or "", m.get("content") or ""))
            # reordenar por fecha de creación del chunk si existe
            out.append(
                {
                    "service": "chatgpt",
                    "id": cid,
                    "title": meta.get("title") or title_from_text(msgs[0]["content"]),
                    "updated_at": meta.get("modified") or meta.get("created") or 0,
                    "model": meta.get("model", ""),
                    "messages": msgs,
                    "source": path,
                }
            )
    finally:
        con.close()
        try:
            os.unlink(db)
        except Exception:
            pass
    return out


# --------------------------------------------------------------------------- #
# Genérico — cualquier tabla con (role, content)
# --------------------------------------------------------------------------- #
def extract_generic(path: str, service: str) -> list:
    out = []
    db = snapshot_db(path)
    con = ro(db)
    try:
        for t in tables(con):
            cols = [r[1].lower() for r in con.execute('pragma table_info("%s")' % t)]
            if "role" not in cols or not any(c in cols for c in ("content", "text", "body", "message")):
                continue
            ccol = next(c for c in ("content", "text", "body", "message") if c in cols)
            try:
                rows = list(con.execute('select * from "%s"' % t))
            except Exception:
                continue
            msgs = []
            for r in rows:
                d = {k.lower(): r[k] for k in r.keys()}
                text = as_text(d.get(ccol))
                if not text:
                    continue
                msgs.append(
                    {
                        "role": (d.get("role") or "user").lower(),
                        "content": text,
                        "ts": float(d.get("created_at") or d.get("timestamp") or d.get("inserted_at") or 0),
                        "meta": {},
                    }
                )
            if msgs:
                out.append(
                    {
                        "service": service,
                        "id": t,
                        "title": title_from_text(msgs[0]["content"]),
                        "updated_at": max((m["ts"] for m in msgs), default=0),
                        "model": "",
                        "messages": msgs,
                        "source": path,
                    }
                )
    finally:
        con.close()
        try:
            os.unlink(db)
        except Exception:
            pass
    return out


# --------------------------------------------------------------------------- #
# Cookies — para el pull web-side (servicios sin store local)
# --------------------------------------------------------------------------- #
COOKIE_SOURCES = [
    ("/data/data/com.android.chrome/app_chrome/Default/Cookies", "chrome"),
    ("/data/data/com.microsoft.emmx/app_chrome/Default/Cookies", "edge"),
    ("/data/data/com.openai.chatgpt/app_webview/Default/Cookies", "chatgpt-app"),
    ("/data/data/com.anthropic.claude/app_webview/Default/Cookies", "claude-app"),
    ("/data/data/com.deepseek.chat/app_webview/Default/Cookies", "deepseek-app"),
    ("/data/data/com.moonshot.kimichat/app_webview/Default/Cookies", "kimi-app"),
    ("/data/data/com.google.android.apps.bard/app_webview/Default/Cookies", "gemini-app"),
]

# NO se filtra aquí por nombre: la decisión de qué cookie sirve para
# autenticar la toma el motor Rust (denylist de analítica + orden por
# relevancia).  Filtrar aquí descartaba cookies de estado de Cloudflare
# (__cf_bm) y de cliente (oai-did), y la API respondía 401.
INTERESTING = None


def harvest_app_tokens() -> None:
    """Tokens de sesión que las apps guardan en su almacenamiento privado.

    Se emiten como registros `__app_token__` para que el motor Rust los
    incorpore a la bóveda de sesiones.
    """
    # DeepSeek: la tabla `app_user_info` guarda el token de sesión de la app.
    for db in glob.glob("/data/data/com.deepseek.chat/databases/deepseek_chat.db"):
        try:
            con = ro(db)
            row = con.execute("select token, email from app_user_info limit 1").fetchone()
            con.close()
            if row and row[0]:
                emit({"__app_token__": True, "service": "deepseek", "token": row[0],
                      "source": "app_user_info.token", "account": row[1] or ""})
        except Exception as e:
            emit_error("deepseek-token", e)
    # Kimi: la "last_app_key" es la credencial que usa la app.
    kp = "/data/data/com.moonshot.kimichat/shared_prefs/VERIFY_SP.xml"
    try:
        txt = open(kp, "r", errors="replace").read()
        m = re.search(r'name="last_app_key">([^<]+)<', txt)
        if m and m.group(1):
            emit({"__app_token__": True, "service": "kimi", "token": m.group(1).strip(),
                  "source": "shared_prefs/VERIFY_SP.xml:last_app_key", "account": ""})
    except Exception as e:
        emit_error("kimi-token", e)
    # ChatGPT / Claude / Gemini: tokens en el almacenamiento local de la WebView.
    for svc, roots in (
        ("chatgpt", ["/data/data/com.openai.chatgpt/app_webview/Default/Local Storage",
                     "/data/data/com.openai.chatgpt/files"]),
        ("claude", ["/data/data/com.anthropic.claude/app_webview/Default/Local Storage"]),
        ("gemini", ["/data/data/com.google.android.apps.bard/app_webview/Default/Local Storage"]),
    ):
        for root in roots:
            if not os.path.isdir(root):
                continue
            for dirpath, _d, files in os.walk(root):
                for f in files:
                    p = os.path.join(dirpath, f)
                    try:
                        if os.path.getsize(p) > 8 * 1024 * 1024:
                            continue
                        blob = open(p, "rb").read()
                    except Exception:
                        continue
                    for pat in (rb"sessionKey\x22?:\x22([A-Za-z0-9_\-\.]{40,})",
                                rb"accessToken\x22?:\x22([A-Za-z0-9_\-\.]{40,})",
                                rb"authToken\x22?:\x22([A-Za-z0-9_\-\.]{40,})"):
                        for m in re.finditer(pat, blob):
                            emit({"__app_token__": True, "service": svc,
                                  "token": m.group(1).decode("utf-8", "replace"),
                                  "source": p.replace("/data/data/", ""), "account": ""})


def harvest_cookies() -> None:
    for path, origin in COOKIE_SOURCES:
        if not os.path.exists(path):
            continue
        db = snapshot_db(path)
        try:
            con = ro(db)
            rows = con.execute(
                "select host_key, name, value, encrypted_value, expires_utc, is_secure from cookies"
            ).fetchall()
        except Exception:
            try:
                os.unlink(db)
            except Exception:
                pass
            continue
        found = []
        for r in rows:
            name = (r["name"] or "").lower()
            val = r["value"] or ""
            enc = len(r["encrypted_value"] or b"")
            found.append(
                {
                    "host": r["host_key"],
                    "name": r["name"],
                    "value": val,
                    "encrypted": bool(enc),
                    "expires": r["expires_utc"],
                    "secure": r["is_secure"],
                }
            )
        emit(
            {
                "__cookies__": True,
                "origin": origin,
                "path": path,
                "count": len(found),
                "cookies": found[:200],
            }
        )
        con.close()
        try:
            os.unlink(db)
        except Exception:
            pass


# --------------------------------------------------------------------------- #
# Orquestación
# --------------------------------------------------------------------------- #
SERVICES = {
    "claude": {
        "glob": "/data/data/com.anthropic.claude/databases/*conversation_snapshots*.db",
        "fn": extract_claude,
    },
    "deepseek": {
        "glob": "/data/data/com.deepseek.chat/databases/deepseek_chat_*.db",
        "fn": extract_deepseek,
    },
    "kimi": {
        "glob": "/data/data/com.moonshot.kimichat/databases/kimi_*_chat.db",
        "fn": extract_kimi,
    },
    "chatgpt": {
        "glob": "/data/data/com.openai.chatgpt/files/sessions/*/default/databases/*_conversations.db",
        "fn": extract_chatgpt,
    },
    "gemini": {
        "glob": "/data/data/com.google.android.apps.bard/databases/*.db",
        "fn": lambda p: extract_generic(p, "gemini"),
    },
    "perplexity": {
        "glob": "/data/data/ai.perplexity.app.android/databases/*.db",
        "fn": lambda p: extract_generic(p, "perplexity"),
    },
    "mistral": {
        "glob": "/data/data/ai.mistral.chat/databases/*.db",
        "fn": lambda p: extract_generic(p, "mistral"),
    },
    "copilot": {
        "glob": "/data/data/com.microsoft.copilot/databases/*.db",
        "fn": lambda p: extract_generic(p, "copilot"),
    },
    "poe": {
        "glob": "/data/data/com.poe.android/databases/*.db",
        "fn": lambda p: extract_generic(p, "poe"),
    },
}


def main() -> None:
    args = sys.argv[1:]
    if "--cookies" in args:
        harvest_cookies()
        return
    if "--harvest" in args:
        harvest_app_tokens()
        harvest_cookies()
        return
    wanted = [a for a in args if not a.startswith("-")]
    services = wanted if wanted else list(SERVICES.keys())
    stats = {}
    for svc in services:
        spec = SERVICES.get(svc)
        if not spec:
            emit_error(svc, RuntimeError("servicio desconocido"))
            continue
        paths = sorted(glob.glob(spec["glob"]))
        n_conv = n_msg = 0
        for p in paths:
            try:
                convs = spec["fn"](p)
            except Exception as e:
                emit_error(svc, e)
                continue
            for c in convs:
                n_msg += len(c.get("messages") or [])
                n_conv += 1
                emit(c)
        stats[svc] = {"files": len(paths), "conversations": n_conv, "messages": n_msg}
    emit({"__stats__": True, "stats": stats})


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # nunca dead
        emit_error("fatal", exc)
        sys.exit(0)

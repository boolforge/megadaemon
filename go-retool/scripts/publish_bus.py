#!/usr/bin/env python3
"""Publica un aviso en el bus de coordinacion (tablón + journal).

El bus de la flota son DOS artefactos del repo:

* `state/mesh_journal.ndjson`  -> journal append-only, una linea JSON por
  evento (es lo que leen los demás agentes).
* `state/board.sqlite`          -> tabla `events` (kind, actor, payload) y
  tabla `tasks` (el tablón propiamente dicho).

Este script publica en los dos sin depender de `md-coord` (que hay que
compilar).  Es idempotente en el sentido de que cada aviso lleva su propia
marca de tiempo y su propio id, y de que NUNCA pisa una tarea ajena: si la
tarea no existe la crea, si existe sólo actualiza `report`/`status` cuando la
tarea es nuestra.

Uso:

    python3 publish_bus.py --title "..." --summary "..." \\
        --branch agent/go-re-completo --commits 2d4ced4 \\
        --artifact docs/GO_DECOMP_ANTIGRAVITY.md --task-id go-re-completo

Medido 2026-11-22 sobre el repo `/workspace/md-gore`.
"""

import argparse
import datetime
import json
import os
import sqlite3
import time

REPO = os.environ.get("MD_REPO", "/workspace/md-gore")
STATE = os.path.join(REPO, "state")
JOURNAL = os.path.join(STATE, "mesh_journal.ndjson")
BOARD = os.path.join(STATE, "board.sqlite")


def now_iso() -> str:
    return datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")


def append_journal(entry: dict) -> None:
    os.makedirs(STATE, exist_ok=True)
    with open(JOURNAL, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(entry, ensure_ascii=False) + "\n")


def board_event(kind: str, actor: str, payload: dict) -> None:
    if not os.path.exists(BOARD):
        return
    conn = sqlite3.connect(BOARD, timeout=20)
    try:
        conn.execute(
            "INSERT INTO events (ts, kind, actor, payload) VALUES (?,?,?,?)",
            (int(time.time()), kind, actor, json.dumps(payload, ensure_ascii=False)),
        )
        conn.commit()
    finally:
        conn.close()


def board_task(task_id: str, title: str, report: str, owner: str, priority: int) -> str:
    """Crea la tarea si no existe y deja el informe.  Devuelve el estado."""
    if not os.path.exists(BOARD):
        return "sin-tablón"
    now = int(time.time())
    conn = sqlite3.connect(BOARD, timeout=20)
    try:
        row = conn.execute("SELECT id FROM tasks WHERE id=?", (task_id,)).fetchone()
        if row is None:
            conn.execute(
                "INSERT INTO tasks (id,title,owner,status,priority,owner_hint,report,"
                "created_at,updated_at,lease_expires_at) VALUES (?,?,?,?,?,?,?,?,?,?)",
                (task_id, title, owner, "in_progress", priority, owner, report, now, now, 0),
            )
            conn.commit()
            return "creada"
        # Tarea existente: sólo se actualiza si el dueño es el mismo (o no hay).
        cur_owner = conn.execute("SELECT owner FROM tasks WHERE id=?", (task_id,)).fetchone()
        if cur_owner and cur_owner[0] in (None, owner):
            conn.execute(
                "UPDATE tasks SET status=?, report=?, updated_at=? WHERE id=?",
                ("in_progress", report, now, task_id),
            )
            conn.commit()
            return "actualizada"
        return f"ajena({cur_owner[0] if cur_owner else '?'})"
    finally:
        conn.close()


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--title", required=True)
    ap.add_argument("--summary", required=True)
    ap.add_argument("--agent", default="agent-go-re")
    ap.add_argument("--kind", default="status")
    ap.add_argument("--event", default="status")
    ap.add_argument("--target", default="enjambre")
    ap.add_argument("--branch", default="")
    ap.add_argument("--commits", default="")
    ap.add_argument("--finding", default="")
    ap.add_argument("--artifact", action="append", default=[])
    ap.add_argument("--task-id", default="")
    ap.add_argument("--task-title", default="")
    ap.add_argument("--priority", type=int, default=2)
    ap.add_argument("--sha256", default="")
    args = ap.parse_args()

    entry = {
        "ts": now_iso(),
        "event": args.event,
        "kind": args.kind,
        "agent": args.agent,
        "target": args.target,
        "title": args.title,
        "summary": args.summary,
        "sha256": args.sha256 or None,
    }
    if args.branch:
        entry["branch"] = args.branch
    if args.commits:
        entry["commits"] = [c.strip() for c in args.commits.split(",") if c.strip()]
    if args.finding:
        entry["finding"] = args.finding
    if args.artifact:
        entry["artifacts"] = [{"ruta": a, "tipo": "artefacto"} for a in args.artifact]

    append_journal(entry)
    board_event(args.event, args.agent, {"title": args.title, "report": args.summary})
    estado = "-"
    if args.task_id:
        estado = board_task(
            args.task_id,
            args.task_title or args.title,
            args.summary,
            args.agent,
            args.priority,
        )
    print("journal:", JOURNAL)
    print("evento de board:", args.event, "| tarea:", args.task_id or "-", "->", estado)


if __name__ == "__main__":
    main()

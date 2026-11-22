#!/usr/bin/env python3
"""Lee `tasks.json` del daemon de Antigravity y lo vuelca en TSV.

Por qué existe (medido el 2026-11-22 en el móvil, por root):

*   `tasks.json` es un array JSON de tareas. El daemon **no** lo valida ni lo
    exporta a ningún sitio donde el enjambre pueda leerlo, y la salida de cada
    tarea se recorta a ~1 KB (`last_output` = 1039 B medidos). Este script da
    una vista plana y sin credenciales del backlog real.

Uso (con el `python3` del runtime de la app: `/data/user/0/com.agy/files/runtime/bin/python3`):

    antigravity-tasks.py            -> todas las tareas (TSV)
    antigravity-tasks.py --pending  -> sólo las que NUNCA corrieron y whose
                                       `run_at` ya pasó (catch-up)
    antigravity-tasks.py --json     -> las mismas tareas en JSON

Columnas TSV: id, name, enabled, trigger_type, interval_sec, run_at_iso,
last_run_status, last_run_duration_s, last_run_iso, command_digest, command

`command_digest` es un cksum del comando: identifica el comando sin volcar
secretos. El `command` completo sí se emite (aquí no hay credenciales, pero
quien lo use debe revisar el fichero antes de publicarlo).
"""

import json
import os
import sys
import time

DEFAULT_TASKS = "/data/user/0/com.agy/files/.gemini/antigravity-app/daemon/tasks.json"


def digest(text: str) -> str:
    """cksum portable (sin dependencias) -> 'sum-len'."""
    total = 0
    length = len(text)
    for index, char in enumerate(text):
        total = (total + ord(char) * (index + 1)) % 0xFFFFFFFF
    return "%d-%d" % (total, length)


def iso(epoch):
    if not epoch:
        return ""
    try:
        return time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime(float(epoch)))
    except (TypeError, ValueError):
        return ""


def load(path):
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            data = json.load(handle)
    except FileNotFoundError:
        return []
    except json.JSONDecodeError as exc:
        print("tasks.json ilegible: %s" % exc, file=sys.stderr)
        return []
    if isinstance(data, dict):
        data = data.get("tasks", [])
    return data if isinstance(data, list) else []


def row(task):
    trigger = task.get("trigger") or {}
    command = task.get("command") or ""
    return {
        "id": task.get("id", ""),
        "name": task.get("name", ""),
        "enabled": bool(task.get("enabled", False)),
        "trigger_type": trigger.get("type", ""),
        "interval_sec": trigger.get("interval_sec", ""),
        "run_at": trigger.get("run_at", ""),
        "last_run_status": task.get("last_run_status", ""),
        "last_run_duration_s": task.get("last_run_duration", ""),
        "last_run_ts": task.get("last_run_ts", ""),
        "command_digest": digest(command),
        "command": command,
    }


def is_pending(task):
    """Nunca corrió y su `run_at` ya pasó (o no tiene `run_at` y es delay)."""
    trigger = task.get("trigger") or {}
    if trigger.get("type") != "delay":
        return False
    if task.get("last_run_ts") or task.get("last_run_status"):
        return False
    run_at = trigger.get("run_at") or 0
    try:
        return float(run_at) <= time.time()
    except (TypeError, ValueError):
        return False


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("-")]
    flags = [a for a in sys.argv[1:] if a.startswith("-")]
    path = args[0] if args else os.environ.get("ANTIGRAVITY_TASKS_JSON", DEFAULT_TASKS)

    tasks = load(path)
    selected = [t for t in tasks if is_pending(t)] if "--pending" in flags else tasks

    if "--json" in flags:
        print(json.dumps([row(t) for t in selected], indent=2, ensure_ascii=False))
        return 0

    # MODO PENDIENTES: para el catch-up del supervisor sólo hacen falta el
    # identificador y el COMANDO.  Antes se emitía la fila completa (11
    # columnas) y el supervisor leía `id cmd`Tomando la 2ª columna, que es
    # el NOMBRE de la tarea: medido en el móvil,
    #   "sh: 1: minimax_bootstrap: not found"
    # Ahora `--pending` emite exactamente 2 columnas (id y command) y el
    # supervisor las lee sin ambigüedad.
    if "--pending" in flags:
        # Cabecera OBLIGATORIA: el supervisor lee con `tail -n +2` para
        # saltarse la primera linea.  Sin cabecera, `tail -n +2` se come la
        # PRIMERA tarea pendiente y el catch-up no ejecuta nada (medido
        # 2026-11-22: la tarea `minimax_bootstrap` seguia sin ejecutarse).
        print("\t".join(["id", "command"]))
        for task in selected:
            data = row(task)
            print("\t".join([
                str(data["id"]),
                str(data["command"]).replace("\t", " ").replace("\n", " "),
            ]))
        return 0

    print("\t".join([
        "id", "name", "enabled", "trigger_type", "interval_sec", "run_at_iso",
        "last_run_status", "last_run_duration_s", "last_run_iso",
        "command_digest", "command",
    ]))
    for task in selected:
        data = row(task)
        print("\t".join([
            str(data["id"]),
            str(data["name"]).replace("\t", " "),
            "true" if data["enabled"] else "false",
            str(data["trigger_type"]),
            str(data["interval_sec"]),
            iso(data["run_at"]),
            str(data["last_run_status"]),
            str(data["last_run_duration_s"]),
            iso(data["last_run_ts"]),
            str(data["command_digest"]),
            str(data["command"]).replace("\t", " "),
        ]))
    return 0


if __name__ == "__main__":
    sys.exit(main())

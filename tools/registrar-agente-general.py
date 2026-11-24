#!/usr/bin/env python3
"""registrar-agente-general — mete al agente `general` en el SISTEMA MegaDaemon.

Por qué existe: el agente debe estar dentro del sistema (tablón + bus +
registro de agentes), no trabajar por fuera con scripts sueltos.

QUÉ HACE (todo ADITIVO: no borra ni modifica filas ajenas):
  1. `state/board.sqlite` tabla `instances` → alta/actualización del nodo
     `agent-general@<host>` (mismo INSERT que `coordination.rs::register_instance`:
     id, role, device, platform, version, protocol, capabilities, state_dir,
     last_seen, registered_at).
  2. `state/board.sqlite` tabla `tasks` → las tareas del backlog de este agente
     (id `t-…` estable ⇒ **idempotente**: si ya existen, no se duplican).
  3. `state/agents/agent-general.json` → registro por fichero (mismo esquema que
     `state/agents/inventory-agent.json`).

POR DEFECTO ES **SÓLO LECTURA** (dry-run): imprime lo que haría y sale.
`--apply` es lo que escribe.  Motivo (medido en este repo): un diagnóstico que
escribe estado puede romper el sistema que está diagnosticando.

USO:
    tools/registrar-agente-general.py                     # dry-run
    tools/registrar-agente-general.py --apply
    tools/registrar-agente-general.py --state-dir /ruta/state --apply
"""
import argparse, json, os, sqlite3, sys, time, shutil

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

NODE_ID_SUFFIX = "@cloud-compute-new-hjn5f"
NODE_DEVICE = "cloud-compute-new-hjn5f"
NODE_PLATFORM = "linux-x86_64"
NODE_VERSION = "0.1.0"
NODE_PROTOCOL = 1

CAPABILITIES = [
    "branch-audit", "credential-rotation-audit", "conflict-marker-audit",
    "board-tasks", "docs-authoring", "git-plumbing",
    "api-integration", "web-ui-fusion",
]

# (id, título, owner_hint, prioridad, owner, estado, informe)
TASKS = [
    ("t-auditoria-ramas-2026-11-23",
     "Auditoría de ramas: inventario de las 80 ramas de agente, clasificación "
     "(integrada / mergeable / obsoleta / riesgosa / sin resolver) y tabla priorizada de integración",
     "general", 1, "general", "done",
     "qué: docs/AUDITORIA-RAMAS-2026-11-23.md con las 80 ramas medidas "
     "(patch-id contra main + merge-tree + marcadores de conflicto). "
     "P0 encontrado: main (PÚBLICO) tiene marcadores de conflicto sin resolver en "
     "state/mesh_board.json:3-7, docs/ANTIGRAVITY_BACKLOG.md:30-33,340-343 y "
     "docs/AUDIT_ADVERSARIAL_V15.md:285-289,309-313 ⇒ el bus no parsea. "
     "Culpables: d148c95 (merge(flota)) y d902a7e (docs=theirs)."),
    ("t-rotacion-credenciales-2026-11-23",
     "Rotar las 4 API keys de Google (AIzaSy…, tools/drive_ls.py:23-26) hardcodeadas en un repo PÚBLICO "
     "+ reescribir el historial (commit 235c399)",
     "general", 1, None, "queued",
     "qué: docs/ROTACION-CREDENCIALES.md con el inventario y el procedimiento exacto. "
     "MEDIDO: boolforge/megadaemon es público (private=false); las 4 keys entraron en "
     "235c399 (2026-11-21T12:40:23Z). Las credenciales VIVAS (mdaemon_sk_live_…) NO están "
     "en el repo (0 coincidencias por sha256 del valor completo) ⇒ esa rotación YA está hecha. "
     "Pendiente de hacer (lo hace el root, tiene /workspace/md-secrets): revocar+emitir las 4 keys "
     "de Google, borrar el array KEYS de tools/drive_ls.py, y reescribir el historial."),
    ("t-agente-general-nodo-bus-2026-11-23",
     "Agente general registrado como nodo del bus (instances + state/agents/agent-general.json) "
     "para poder recibir tareas por la API de MegaDaemon (/api/v2/nodes/assign, /api/v2/nodes/message)",
     "general", 1, "general", "done",
     "qué: tools/registrar-agente-general.py (idempotente, dry-run por defecto) + "
     "state/agents/agent-general.json. La API v2 ya expone /api/v2/nodes/assign y "
     "/api/v2/nodes/message (rust-core/src/webui.rs:74-75) y /api/v2/tasks/claim|release|report."),
    ("t-gui-antigravity-fusion-2026-11-23",
     "Fusión del GUI de Antigravity (shell Jetski Web) con el cockpit v2 de MegaDaemon: "
     "una sola página web que habla /api/v2/*, chat con el agente y marco de la IDE de Antigravity",
     "general", 2, "general", "done",
     "qué: web/cockpit_antigravity.html — shell Jetski (tema de /workspace/agy-intel/assets_web_index.html) "
     "+ cliente de la API v2 (session/exchange → X-Seere-Auth-Token) + pestañas Panel/Agentes/Tareas/Chat/LLM/"
     "Alertas/Antigravity/API. El chat usa POST /api/chat (cockpit_server.rs:2383). "
     "NO COMPILEDADO aquí (prohibido compilar); el JS pasa `node --check`."),
    ("t-conflict-markers-main-2026-11-23",
     "P0: main (PÚBLICO) tiene marcadores de conflicto sin resolver ⇒ state/mesh_board.json NO parsea como JSON",
     "coord", 1, None, "queued",
     "qué: git grep -nE '^(<<<<<<< |>>>>>>> )' main --  →  state/mesh_board.json:3,5,7 · "
     "docs/ANTIGRAVITY_BACKLOG.md:30-33,340-343 · docs/AUDIT_ADVERSARIAL_V15.md:285-289,309-313. "
     "Introducidos por d148c95 (merge(flota)); propagados por d902a7e (docs=theirs, codigo=ours). "
     "El bus no arranca hasta que se limpien."),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--state-dir", default=os.path.join(REPO, "state"))
    ap.add_argument("--apply", action="store_true",
                    help="escribe de verdad (por defecto es dry-run)")
    ap.add_argument("--repo", default=REPO)
    args = ap.parse_args()

    sd = os.path.abspath(args.state_dir)
    board = os.path.join(sd, "board.sqlite")
    agents = os.path.join(sd, "agents")
    node = "agent-general" + NODE_ID_SUFFIX
    now = int(time.time())
    apply = args.apply

    print(f"state-dir : {sd}")
    print(f"tablón    : {board} {'(EXISTE)' if os.path.exists(board) else '(NO EXISTE)'}")
    print(f"modo      : {'APPLY (escribe)' if apply else 'DRY-RUN (no escribe nada)'}")
    print()

    if not os.path.exists(board):
        print("ERROR: no hay board.sqlite en ese state_dir")
        return 2

    con = sqlite3.connect(board)
    con.execute("PRAGMA busy_timeout=15000")
    cur = con.cursor()

    # ---- 1) instancia ----
    row = cur.execute("SELECT role,device,last_seen FROM instances WHERE id=?", (node,)).fetchone()
    if row:
        print(f"[instances] {node} YA EXISTE (role={row[0]}, last_seen={row[2]}) -> se actualiza")
    else:
        print(f"[instances] {node} NO EXISTE -> se inserta (capabilities={len(CAPABILITIES)})")
    if apply:
        cur.execute("""INSERT INTO instances (id, role, device, platform, version, protocol,
                         capabilities, state_dir, last_seen, registered_at)
                       VALUES (?,?,?,?,?,?,?,?,?,?)
                       ON CONFLICT(id) DO UPDATE SET
                         role=excluded.role, device=excluded.device, platform=excluded.platform,
                         version=excluded.version, protocol=excluded.protocol,
                         capabilities=excluded.capabilities, state_dir=excluded.state_dir,
                         last_seen=excluded.last_seen""",
                    (node, "worker", NODE_DEVICE, NODE_PLATFORM, NODE_VERSION, NODE_PROTOCOL,
                     json.dumps(CAPABILITIES), sd, now, now))
        cur.execute("INSERT INTO events (ts, kind, actor, payload) VALUES (?,?,?,?)",
                    (now, "instance_registered", node, json.dumps({"role": "worker"})))

    # ---- 2) tareas ----
    for tid, title, hint, prio, owner, state, report in TASKS:
        ex = cur.execute("SELECT status FROM tasks WHERE id=?", (tid,)).fetchone()
        if ex:
            print(f"[tasks] {tid} ya existe (status={ex[0]}) -> no se duplica")
        else:
            print(f"[tasks] {tid} NO existe -> se inserta (estado={state}, prio={prio})")
        if apply and not ex:
            cur.execute("""INSERT INTO tasks (id, title, owner, status, priority, owner_hint,
                             report, created_at, updated_at, lease_expires_at)
                           VALUES (?,?,?,?,?,?,?,?,?,0)""",
                        (tid, title, owner, state, prio, hint, report, now, now))
            cur.execute("INSERT INTO events (ts, kind, actor, payload) VALUES (?,?,?,?)",
                        (now, "task_added", "board", json.dumps({"id": tid, "title": title[:90]})))

    if apply:
        con.commit()
    n_i = cur.execute("SELECT count(*) FROM instances").fetchone()[0]
    n_t = cur.execute("SELECT count(*) FROM tasks").fetchone()[0]
    con.close()
    print(f"\nboard: instances={n_i} tasks={n_t}")

    # ---- 3) registro por fichero ----
    p = os.path.join(agents, "agent-general.json")
    print(f"[agents] {p} {'ya existe -> no se sobrescribe' if os.path.exists(p) else 'se crea'}")
    if apply and not os.path.exists(p):
        os.makedirs(agents, exist_ok=True)
        reg = {
            "id": "agent-general",
            "name": "agent-general (Mavis · auditoría de ramas + credenciales + integración API/GUI)",
            "role": "worker",
            "device": NODE_DEVICE,
            "repo": "https://github.com/boolforge/megadaemon",
            "branch": "rama/ramas-auditoria-2026-11-23",
            "commit": "pending",
            "last_seen": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now)),
            "capabilities": CAPABILITIES,
            "focus": ["auditoria-de-ramas", "rotacion-de-credenciales", "integracion-api-gui"],
            "api": {
                "base": "MEGADAEMON_URL (ver /workspace/md-secrets/instance.env)",
                "mcp": "MEGADAEMON_MCP (POST /mcp, JSON-RPC 2.0)",
                "session_exchange": "POST /api/session/exchange  (api_key EN EL CUERPO)",
                "cabeceras": ["X-API-Key", "X-Seere-Auth-Token"],
                "nota": "el token de sesión rota en cada reinicio del daemon: se canjea, no se cachea",
            },
            "gui": {
                "cockpit": "web/cockpit_v2.html (API v2)",
                "fusion_antigravity": "web/cockpit_antigravity.html (Jetski Web + API v2 + chat)",
            },
            "artifacts": [
                {"name": "AUDITORIA-RAMAS-2026-11-23.md", "where": ["repo:docs/"]},
                {"name": "ROTACION-CREDENCIALES.md", "where": ["repo:docs/"]},
            ],
        }
        with open(p, "w") as f:
            json.dump(reg, f, indent=2, ensure_ascii=False)
            f.write("\n")
    if apply:
        shutil.copy2(board, board + ".bak-registro-agente-general")
        print(f"copia de seguridad: {board}.bak-registro-agente-general")
    print("OK" if apply else "DRY-RUN: nada escrito (usa --apply para escribir)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

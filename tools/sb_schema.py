#!/usr/bin/env python3
"""Trae el esquema de una base SQLite del móvil y lo imprime.

Uso: sb_schema.py <ruta_absoluta_en_el_movil> [bytes_cabecera]

Hace: dd de los primeros N bytes -> base64 -> MCP -> fichero disperso del
tamaño real declarado en la cabecera -> sqlite3 (sólo lectura) -> sqlite_master.
"""
import base64
import json
import os
import shutil
import sqlite3
import subprocess
import sys

TOOL = "/tmp/tool.sh"
HEAD_KB = int(sys.argv[2]) if len(sys.argv) > 2 else 256
path = sys.argv[1]

cmd = (
    f"dd if={path} bs=1024 count={HEAD_KB} 2>/dev/null | base64 -w0"
)
args = {
    "command": cmd,
    "max_chars": 524288,
    "confirm_destructive": True,
}
out = subprocess.run(
    [TOOL, "root_exec", json.dumps(args)],
    capture_output=True, text=True, timeout=300,
).stdout

try:
    d = json.loads(out)
    b64 = d["result"]["stdout"]
except Exception:
    m = out.find('"stdout"')
    print("no se pudo leer la salida:\n", out[:500])
    sys.exit(1)

raw = base64.b64decode(b64)
print(f"traidos {len(raw)} bytes de {path}")

# Tamaño real declarado en la cabecera SQLite (bytes 28..32, big-endian).
page_size = int.from_bytes(raw[16:18], "big") or 4096
page_count = int.from_bytes(raw[28:32], "big")
size = page_size * page_count
print(f"page_size={page_size} paginas={page_count} tamano_real={size}")

dst = "/tmp/_sb_sparse.db"
with open(dst, "wb") as g:
    g.truncate(size)
    g.seek(0)
    g.write(raw)
print("disco:", os.stat(dst).st_blocks * 512, "bytes reales")

con = sqlite3.connect(f"file:{dst}?mode=ro", uri=True)
try:
    rows = list(con.execute(
        "select type,name,sql from sqlite_master order by type,name"))
except Exception as e:
    print("no se pudo abrir:", e)
    sys.exit(1)
print(f"objetos: {len(rows)}")
for t, n, sql in rows:
    print(f"  [{t}] {n}")
    if sql and t == "table":
        print("      ", " ".join(sql.split())[:400])

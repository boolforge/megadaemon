#!/usr/bin/env bash
# construir-bundle.sh — la BUILD META: un artefacto único con TODO dentro.
#
# POR QUÉ
# --------
# El usuario pidió (2026-11-25) que "el propio megadaemon actualice TODO y
# sea autocontenido y la build meta todo dentro".
#
# Hay dos capas complementarias:
#   * **autocontenido**: el binario lleva DENTRO el stack (supervisor,
#     keeper, scripts y Web UI) y lo reconcilia solo ⇒ autorreparación
#     offline, sin red (módulo `stack`).
#   * **build meta**: un `.tar.gz` con el binario + el stack + un
#     `manifest.json` con el SHA-256 de cada fichero ⇒ instalación y
#     actualización COMPLETA de una vez, verificable (módulo `bundle`).
#
# USO:
#   tools/construir-bundle.sh <binario> <salida.tar.gz> [prefijo]
#
# El manifiesto usa destinos RELATIVOS al prefijo de instalación:
#   prefijo/<fichero>   -> <prefijo>/<fichero>
#   servicio/<fichero>  -> <padre del prefijo>/service.d/<fichero>
set -euo pipefail

BINARIO="${1:?falta el binario}"
SALIDA="${2:?falta la salida}"
PREFIJO="${3:-/data/adb/megadaemon}"

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

echo "== bundle meta: $SALIDA"
mkdir -p "$STAGE/deploy" "$STAGE/web" "$STAGE/bin"

# 1) El binario.
cp -f "$BINARIO" "$STAGE/bin/megadaemon-core"
chmod 755 "$STAGE/bin/megadaemon-core"
# El mddoctor se compila aqui si no esta: en el movil no hay compilador, y en
# el PC el gcc siempre esta.  Si falla, el bundle sigue igual (es accesorio).
if [ ! -f "$STAGE/bin/mddoctor" ]; then
    sh "$RAIZ/tools/build-mddoctor.sh" >/dev/null 2>&1 || true
    [ -f "$RAIZ/tools/mddoctor" ] && cp -f "$RAIZ/tools/mddoctor" "$STAGE/bin/mddoctor"
fi
[ -f "$STAGE/bin/mddoctor" ] && chmod 755 "$STAGE/bin/mddoctor"

# 2) El stack (los mismos ficheros que el daemon lleva embebidos).
for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh \
         antigravity-task-runner.sh antigravity-task-supervisor.sh tunnel-watchdog.sh; do
    cp -f "$RAIZ/deploy/$f" "$STAGE/deploy/$f"
    chmod 755 "$STAGE/deploy/$f"
done
for f in cockpit_v2.html webui_v2.css cockpit_template.html; do
    cp -f "$RAIZ/web/$f" "$STAGE/web/$f"
    chmod 644 "$STAGE/web/$f"
done

# 3) El manifiesto: SHA-256 de CADA fichero + su destino.
python3 - "$STAGE" "$PREFIJO" "$BINARIO" <<'PY'
import hashlib, json, os, sys

stage, prefijo, binario = sys.argv[1], sys.argv[2], sys.argv[3]

# (origen dentro del bundle, destino, modo)
miembros = [
    ("bin/megadaemon-core", "prefijo/megadaemon-core", 0o755),
    # El nucleo forense en C: 26 KB, sin dependencias, y funciona aunque el
    # daemon este roto.  Va al prefijo para que el movil lo tenga siempre.
    ("bin/mddoctor", "prefijo/mddoctor", 0o755),
    ("deploy/99-megadaemon.sh", "servicio/99-megadaemon.sh", 0o755),
    ("deploy/97-megadaemon-keeper.sh", "servicio/97-megadaemon-keeper.sh", 0o755),
    ("deploy/98-cloudflared.sh", "servicio/98-cloudflared.sh", 0o755),
    ("deploy/91-adb-tcp.sh", "servicio/91-adb-tcp.sh", 0o755),
    ("deploy/antigravity-task-runner.sh", "prefijo/antigravity-task-runner.sh", 0o755),
    ("deploy/antigravity-task-supervisor.sh", "prefijo/antigravity-task-supervisor.sh", 0o755),
    ("web/cockpit_v2.html", "prefijo/web/cockpit_v2.html", 0o644),
    ("web/webui_v2.css", "prefijo/web/webui_v2.css", 0o644),
    ("web/cockpit_template.html", "prefijo/web/cockpit_template.html", 0o644),
]

def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()

build = "desconocido"
try:
    out = os.popen(f"sha256sum {binario} 2>/dev/null").read().split()[0]
    build = out[:7] if out else "desconocido"
except Exception:
    pass

lista = []
for origen, destino, modo in miembros:
    p = os.path.join(stage, origen)
    if not os.path.isfile(p):
        print(f"AVISO: falta {origen} en el bundle", file=sys.stderr)
        continue
    lista.append({
        "origen": origen,
        "destino": destino,
        "modo": modo,
        "sha256": sha(p),
        "bytes": os.path.getsize(p),
    })

man = {
    "version": 1,
    "build": build,
    "prefijo": prefijo,
    "ficheros": lista,
}
with open(os.path.join(stage, "manifest.json"), "w", encoding="utf-8") as f:
    json.dump(man, f, indent=2, ensure_ascii=False)
print(f"   manifiesto: {len(lista)} ficheros, build {build}")
PY

# 4) Empaquetar (reproducible: mtime fijo y orden estable).
tar --sort=name --mtime='UTC 2020-01-01' --owner=0 --group=0 --numeric-owner \
    -czf "$SALIDA" -C "$STAGE" manifest.json bin deploy web

echo "   bundle: $(stat -c %s "$SALIDA" 2>/dev/null || stat -f %z "$SALIDA") B"
sha256sum "$SALIDA" | awk '{print "   sha256: " $1}' > "$SALIDA.sha256" 2>/dev/null || \
    shasum -a 256 "$SALIDA" | awk '{print $1 "  '"$SALIA"'"' > "$SALIDA.sha256"
cat "$SALIDA.sha256"

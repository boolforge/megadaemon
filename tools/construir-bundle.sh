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
#   tools/construir-bundle.sh <binario> <salida.tar.gz> [prefijo] [mddoctor]
#
#   <mddoctor> es el binario forense YA COMPILADO para la arquitectura del
#   daemon (el CI le pasa dist/mddoctor-android-arm64).  Si no se pasa, se
#   compila el del host, que NO sirve para un movil: por eso se COMPRUEBA
#   que el binario embebido tenga la misma arquitectura que el daemon.
#
# El manifiesto usa destinos RELATIVOS al prefijo de instalación:
#   prefijo/<fichero>   -> <prefijo>/<fichero>
#   servicio/<fichero>  -> <padre del prefijo>/service.d/<fichero>
set -euo pipefail

BINARIO="${1:?falta el binario}"
SALIDA="${2:?falta la salida}"
PREFIJO="${3:-/data/adb/megadaemon}"
MDDOCTOR="${4:-}"

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

echo "== bundle meta: $SALIDA"
mkdir -p "$STAGE/deploy" "$STAGE/web" "$STAGE/bin"

# 1) El binario.
cp -f "$BINARIO" "$STAGE/bin/megadaemon-core"
chmod 755 "$STAGE/bin/megadaemon-core"
# El nucleo forense (`mddoctor`).  MEDIDO 2026-11-27: sin esto el bundle
# llevaba el binario COMPILADO EN EL HOST (x86-64) y el movil se descargaba
# un ejecutable que no puede correr: el enlace "online -> root" estaba roto.
# Ahora se usa el binario ya compilado para la arquitectura del daemon (el
# CI lo pasa como 4o argumento) y, si no hay, se compila el del host PERO
# solo se embebe si su arquitectura coincide con la del daemon.
arquitectura() {
    # e_machine del ELF (offset 18, 2 bytes little endian): 183 = aarch64,
    # 62 = x86-64.  0 si el fichero no es un ELF.
    od -An -tu1 -j18 -N2 "$1" 2>/dev/null | awk '{print $1+0}'
}
if [ -n "$MDDOCTOR" ] && [ -f "$MDDOCTOR" ]; then
    cp -f "$MDDOCTOR" "$STAGE/bin/mddoctor"
elif [ ! -f "$STAGE/bin/mddoctor" ]; then
    sh "$RAIZ/tools/build-mddoctor.sh" >/dev/null 2>&1 || true
    [ -f "$RAIZ/tools/mddoctor" ] && cp -f "$RAIZ/tools/mddoctor" "$STAGE/bin/mddoctor"
fi
if [ -f "$STAGE/bin/mddoctor" ]; then
    a_bin="$(arquitectura "$BINARIO")"
    a_doc="$(arquitectura "$STAGE/bin/mddoctor")"
    if [ -n "$a_doc" ] && [ "$a_doc" != "$a_bin" ]; then
        echo "bundle: AVISO - el mddoctor ($a_doc) NO es de la arquitectura del daemon ($a_bin): no se embebe"
        rm -f "$STAGE/bin/mddoctor"
    else
        chmod 755 "$STAGE/bin/mddoctor"
        echo "bundle: mddoctor embebido (arquitectura $a_doc)"
    fi
fi

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

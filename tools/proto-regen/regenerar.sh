#!/usr/bin/env bash
# Regenera los .proto y el .go del binario de Antigravity (MegaDaemon).
#
# PROTOCOLO: esto NO compila el crate de Rust (el sandbox tiene 3 GB y
# enlazar el crate es un OOM garantizado).  Solo Python + protoc.
#
# ENTRADA:
#   $1  ruta del binario ELF a analizar (liblanguage_server.so)
#   $2  directorio de salida (por defecto: /tmp/proto-regen)
#
# SALIDAS (todas medidas, ninguna inventada):
#   <out>/descriptor_set.pb        FileDescriptorSet con los 557 descriptores
#   <out>/manifiesto.tsv           descriptor -> vaddr, bytes, sha256, cuentas
#   <out>/duplicados.tsv           el mismo .proto con otra variante
#   <out>/descartados.tsv         candidatos que no pasaron el filtro
#   <out>/protos/                  los .proto reconstruidos
#   <out>/informe_validacion.json  protoc OK + fidelidad vs. el original
#   <out>/go/                      el Go regenerado con protoc-gen-go
#   <out>/informe_go.json          validacion lexica del Go
#   <out>/resumen.json             todas las cifras
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="$RAIZ/tools/proto-regen"

BIN="${1:-/workspace/agy-re/liblanguage_server.so}"
OUT="${2:-/tmp/proto-regen}"

# --- interprete Python ----------------------------------------------------
# El venv del sandbox trae `protobuf` y `grpcio-tools` (con `protoc`).
# El Python del sistema es PEP 668: no se instala nada ahi.
PY="${PY:-}"
if [ -z "$PY" ]; then
  if [ -x /workspace/venv-frida/bin/python ]; then
    PY=/workspace/venv-frida/bin/python
  else
    PY="$(command -v python3)"
  fi
fi
if [ -z "$PY" ]; then
  echo "FALLO: no hay interprete Python" >&2
  exit 1
fi

# --- tabla de funciones Go (para saber el paquete Go de cada .proto) -----
GO_FUNCS="${GO_FUNCS:-/workspace/agy-re/tools-out/go.funcs.tsv}"

# --- plugin protoc-gen-go -------------------------------------------------
PLUGIN="${PLUGIN:-}"
if [ -z "$PLUGIN" ] && [ -x /tmp/pgg/protoc-gen-go ]; then
  PLUGIN=/tmp/pgg/protoc-gen-go
fi

mkdir -p "$OUT"
echo "== 1/5 extrayendo descriptores del binario =="
"$PY" "$HERE/extraer_descriptores.py" --bin "$BIN" --out "$OUT" || exit 1

echo "== 2/5 reconstruyendo los .proto =="
"$PY" "$HERE/proto_a_texto.py" \
    --descriptor-set "$OUT/descriptor_set.pb" \
    --out "$OUT/protos" \
    --stats "$OUT/proto_stats.json" \
    --no-representables "$OUT/opciones_no_representables.tsv" || exit 1

echo "== 3/5 validando los .proto con protoc y midiendo la fidelidad =="
"$PY" "$HERE/validar_proto.py" \
    --protos "$OUT/protos" \
    --descriptor-set "$OUT/descriptor_set.pb" \
    --informe "$OUT/informe_validacion.json" || exit 1

echo "== 4/5 regenerando el Go =="
GO_ARGS=(--protos "$OUT/protos" --out "$OUT/go"
         --descriptor-set "$OUT/descriptor_set.pb"
         --stats "$OUT/go_stats.json")
if [ -n "$GO_FUNCS" ] && [ -f "$GO_FUNCS" ]; then
  GO_ARGS+=(--go-funcs "$GO_FUNCS")
fi
if [ -n "$PLUGIN" ]; then
  GO_ARGS+=(--plugin "$PLUGIN")
else
  echo "   (sin protoc-gen-go: se usa el generador propio)"
  GO_ARGS+=(--sin-plugin)
fi
"$PY" "$HERE/generar_go.py" "${GO_ARGS[@]}" || exit 1

echo "== 5/5 validando la sintaxis del Go =="
"$PY" "$HERE/validar_go.py" --raiz "$OUT/go" --informe "$OUT/informe_go.json" || exit 1

echo
echo "== resumen =="
"$PY" - "$OUT" <<'PYEOF'
import json, os, sys
out = sys.argv[1]
def carga(n):
    p = os.path.join(out, n)
    if os.path.exists(p):
        with open(p) as fh:
            return json.load(fh)
    return {}
res = carga("resumen.json")
pro = carga("proto_stats.json")
val = carga("informe_validacion.json")
go = carga("go_stats.json")
goval = carga("informe_go.json")
print(f"  descriptores extraidos ....... {res.get('descriptores_validos')}")
print(f"  bytes de descriptor .......... {res.get('bytes_descriptores')}")
print(f"  .proto reconstruidos ......... {pro.get('ficheros_proto')}")
print(f"  protoc ....................... {'OK' if val.get('protoc_ok') else 'FALLO'}"
      f"  (fidelidad: {val.get('fidelidad')})")
print(f"  campos comparados ............ {val.get('campos_comparados')}")
print(f"  ficheros .go generados ....... {go.get('ficheros_go')}")
print(f"  lineas de Go ................. {go.get('lineas_go')}")
print(f"  Go lexico valido ............. {goval.get('todos_validos')}")
PYEOF
echo
echo "Salida en: $OUT"

#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# verify_canal.sh — comprueba, DE UNA VEZ, que el canal del enjambre vive
#
# Para qué: después de los 3 pasos (túnel en el móvil · release nueva ·
# relay + 443→PC en el router) hay que confirmar que el canónico responde
# y que trae la URL viva del túnel. Antes eso exigía 4 comandos sueltos y
# un ojo humano; aquí sale un OK/FAIL por cada punto.
#
# Es de **sólo lectura**: no escribe nada, no publica nada, no toca el móvil
# ni el repo. NO imprime secretos (ni la key ni el token).
#
# Uso:
#     tools/verify_canal.sh                      # dominio por defecto
#     tools/verify_canal.sh --dominio mdtmfnc.duckdns.org
#     tools/verify_canal.sh --canonico http://127.0.0.1:8099/channel.json
#     tools/verify_canal.sh --json               # sólo el resumen JSON
#
# Códigos de salida: 0 = todo OK · 1 = algún fallo · 2 = uso incorrecto
# ---------------------------------------------------------------------------
set -u

DOMAIN="${MEGADAEMON_DOMAIN:-mdtmfnc.duckdns.org}"
CANON="${MEGADAEMON_CANONICAL_URL:-https://${DOMAIN}/channel.json}"
TIMEOUT="${TIMEOUT:-10}"
LECTOR_TIMEOUT="${LECTOR_TIMEOUT:-45}"
SOLO_JSON=0
FALLS=0
RES=""

while [ $# -gt 0 ]; do
    case "$1" in
        --dominio)  DOMAIN="$2"; CANON="https://${DOMAIN}/channel.json"; shift 2 ;;
        --canonico) CANON="$2"; shift 2 ;;
        --json)     SOLO_JSON=1; shift ;;
        -h|--help)  sed -n '2,22p' "$0"; exit 2 ;;
        *) echo "opción desconocida: $1" >&2; exit 2 ;;
    esac
done

say() { [ "$SOLO_JSON" -eq 1 ] || echo "==> $*"; }
ok()  { [ "$SOLO_JSON" -eq 1 ] || echo "    OK    $1"; RES="$RES\"$2\":$3,"; }
fail() {
    [ "$SOLO_JSON" -eq 1 ] || echo "    FAIL  $1"
    FALLS=$((FALLS + 1)); RES="$RES\"$2\":\"ko\","
}
info() { [ "$SOLO_JSON" -eq 1 ] || echo "    ..    $*"; }

# curl con tiempo límite; devuelve el código HTTP (000 si no hay respuesta)
http_code() {
    local url="$1" code
    code="$(curl -s -m "$TIMEOUT" -o /dev/null -w '%{http_code}' "$url" 2>/dev/null)"
    [ -n "$code" ] || code=000
    printf '%s' "$code"
}

# ── 1) DNS: el A record del dominio ───────────────────────────────────────
say "1) DNS de $DOMAIN"
A_IP=""
if command -v dig >/dev/null 2>&1; then
    A_IP="$(dig +short "$DOMAIN" A 2>/dev/null | head -1)"
elif command -v getent >/dev/null 2>&1; then
    A_IP="$(getent hosts "$DOMAIN" 2>/dev/null | awk '{print $1}' | head -1)"
fi
if [ -n "$A_IP" ]; then
    ok "$DOMAIN -> $A_IP" "dns" "\"ok\""
    RES="$RES\"dns_ip\":\"$A_IP\","
else
    fail "$DOMAIN NO resuelve (¿arrancó el relay? ¿se publicó el A record?)" "dns"
fi

# ── 2) el canónico por HTTPS ──────────────────────────────────────────────
say "2) canónico $CANON"
BODY_FILE="$(mktemp 2>/dev/null || echo /tmp/.verify_canal.$$)"
CODE="$(curl -s -m "$TIMEOUT" -o "$BODY_FILE" -w '%{http_code}' "$CANON" 2>/dev/null)"
[ -n "$CODE" ] || CODE=000
BODY="$(cat "$BODY_FILE" 2>/dev/null)"; rm -f "$BODY_FILE" 2>/dev/null
CNUM="$CODE"; case "$CNUM" in ''|*[!0-9]*) CNUM=0 ;; 000) CNUM=0 ;; esac
case "$CODE" in
    200) ok "HTTP 200" "canonico" "\"ok\"" ;;
    000) fail "sin respuesta (000): no resuelve o no hay ruta; el relay no está arriba" "canonico" ;;
    *)   fail "HTTP $CODE" "canonico" ;;
esac
RES="$RES\"canonico_http\":$CNUM,"

# ── 3) el payload: ¿trae las URLs y el 'quick'? ────────────────────────────
QUICK=""; DUCK=""; TS=""
if [ -n "$BODY" ] && command -v python3 >/dev/null 2>&1; then
    EVAL="$(printf '%s' "$BODY" | python3 -c '
import json,sys
try:
    d = json.load(sys.stdin)
except Exception:
    print("|maljson"); raise SystemExit
if not isinstance(d, dict):
    print("|noobj"); raise SystemExit
print("|ok|%s|%s|%s" % (d.get("quick") or "", d.get("duckdns") or "", d.get("ts") or ""))
' 2>/dev/null)"
    case "$EVAL" in
        "|ok|"*)
            REST="${EVAL#|ok|}"; QUICK="${REST%%|*}"; REST="${REST#*|}"
            DUCK="${REST%%|*}"; TS="${REST#*|}"
            ok "payload legible (ts=$TS)" "payload" "\"ok\""
            RES="$RES\"ts\":\"$TS\","
            ;;
        *) fail "el payload no es un JSON con campos" "payload" ;;
    esac
fi

if [ -n "$QUICK" ]; then
    ok "quick presente: $QUICK" "quick" "\"$QUICK\""
else
    fail "el payload NO trae 'quick' ⇒ el móvil no ha publicado el túnel" "quick"
fi
if [ -n "$DUCK" ]; then
    ok "duckdns presente: $DUCK" "duckdns" "\"$DUCK\""
else
    info "sin 'duckdns' en el payload (normal si el canónico es el relay)"
    RES="$RES\"duckdns\":\"\","
fi

# ── 4) ¿vive el túnel que anuncia el 'quick'? ─────────────────────────────
if [ -n "$QUICK" ]; then
    say "4) sondeo del túnel anunciado"
    QCODE="$(http_code "$QUICK/mcp")"
    QNUM="$QCODE"; case "$QNUM" in ''|*[!0-9]*) QNUM=0 ;; 000) QNUM=0 ;; esac
    case "$QCODE" in
        000)        fail "el túnel '$QUICK' no responde (000)" "tunel" ;;
        401|403)    ok "el túnel '$QUICK' responde ($QCODE = el daemon exige credencial)" "tunel" "\"ok\"" ;;
        *)          ok "el túnel '$QUICK' responde ($QCODE)" "tunel" "\"ok\"" ;;
    esac
    RES="$RES\"tunel_http\":$QNUM,"
fi

# ── 5) el lector oficial de la flota ──────────────────────────────────────
say "5) lector oficial (tools/channel_discover.py)"
LECTOR="$(dirname "$0")/channel_discover.py"
if [ -f "$LECTOR" ] && command -v python3 >/dev/null 2>&1; then
    if command -v timeout >/dev/null 2>&1; then
        OUT="$(timeout "$LECTOR_TIMEOUT" python3 "$LECTOR" --duckdns "$DOMAIN" --no-guardar 2>/dev/null)"
    else
        OUT="$(python3 "$LECTOR" --duckdns "$DOMAIN" --no-guardar 2>/dev/null)"
    fi
    SRC="$(printf '%s' "$OUT" | python3 -c '
import json,sys
try:
    d=json.load(sys.stdin)
except Exception:
    print("ko"); raise SystemExit
print(d.get("_fuente") or "ok")
' 2>/dev/null)"
    case "$SRC" in
        *"$DOMAIN"*) ok "el lector encontró el canónico ($SRC)" "lector" "\"canonico\"" ;;
        "")           fail "el lector no devolvió nada" "lector" ;;
        *)           info "el lector cayó al respaldo (fuente: $SRC)"
                      RES="$RES\"lector_fuente\":\"$SRC\"," ;;
    esac
else
    info "channel_discover.py no está junto a este script"
fi

# ── resumen ────────────────────────────────────────────────────────────────
if [ "$FALLS" -eq 0 ]; then VERDAD=true; TEXTO="canal OK"; else VERDAD=false; TEXTO="canal con fallos"; fi
RES="${RES%,}"
echo "{\"ok\":$VERDAD,\"fallos\":$FALLS,\"dominio\":\"$DOMAIN\",\"canonico_url\":\"$CANON\",$RES}"
if [ "$FALLS" -eq 0 ]; then
    say "CANAL OK — el enjambre encuentra el móvil"
    exit 0
fi
say "CANAL CON FALLOS ($FALLS) — mira los FAIL de arriba"
exit 1

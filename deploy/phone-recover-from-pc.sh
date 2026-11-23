#!/usr/bin/env bash
# =====================================================================
# phone-recover-from-pc.sh — RECUPERAR EL MÓVIL DESDE EL PC (un comando)
#
# POR QUÉ ESTE SCRIPT
# -------------------
# MEDIDO (2026-11-23): el móvil está CAÍDO y el dominio estable está
# secuestrado:
#
#   * `mdtmfnc.duckdns.org` resuelve (DoH) a 47.253.152.120, que es el
#     **NAT del datacenter**, no la IP de casa ⇒ el camino
#     dominio → router → PC → móvil está roto.
#   * El **A record lo tiene que publicar el MÓVIL** (su IP pública SÍ es
#     la de casa) o el PC (también está en casa).  Publicarlo desde un
#     nodo/sandbox lo vuelve a apuntar al datacenter ⇒ **NUNCA** desde
#     aquí.
#   * Desde el sandbox el móvil es inalcanzable (el proxy de salida traga
#     las conexiones) ⇒ hace falta alguien **en casa**.
#
# QUÉ HACE (todo por ADB, sin reiniciar el móvil y sin borrar nada)
# ----------------------------------------------------------------
#   1. Detecta el móvil (USB o ADB TCP en la LAN) y su estado real.
#   2. Sube el instalador canónico al móvil.
#   3. Sube el token de DuckDNS (0600) para que el móvil pueda publicar
#      el A record **con su IP pública real**.
#   4. Ejecuta `instalador-megadaemon.sh --instalar --movil` **como root**
#      en el móvil (baja el ARM64, unifica la key, instala el trío
#      `service.d`, publica el canal y actualiza DuckDNS + lo verifica
#      por DoH).
#   5. VERIFICA desde el PC (que tiene red de casa de verdad):
#      el A record por DoH, `/health` por el dominio y el túnel rápido.
#
# USO
# ---
#   sh deploy/phone-recover-from-pc.sh                 # recupera
#   sh deploy/phone-recover-from-pc.sh --diagnostico   # sólo informa
#   sh deploy/phone-recover-from-pc.sh --serie=192.168.1.124:5555
#   sh deploy/phone-recover-from-pc.sh --token-ruta ~/.mavis/duckdns.token
#
# REQUISITOS: `adb` en el PC (con el móvil por USB o en la LAN) y, si el
# A record se quiere dejar bien, el token de DuckDNS en el PC
# (`~/.mavis/duckdns.token` o `--token-ruta`).
# =====================================================================
set -uo pipefail

SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SELF/.." && pwd)"
INSTALLER="$REPO/deploy/instalador-megadaemon.sh"
MD_DIR=/data/local/tmp/megadaemon
DDNS_NAME="${MD_DDNS_NAME:-mdtmfnc}"
DDNS_FULL="$DDNS_NAME.duckdns.org"
IP_PUBLICA_PC="${MD_IP_PUBLICA:-}"
TOKEN_RUTA=""
SERIE=""
MODO=instalar

say()  { printf '[recuperar %s] %s\n' "$(date -u +%H:%M:%S)" "$*"; }
warn() { printf '[recuperar %s] AVISO: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
die()  { printf '[recuperar %s] ERROR: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; exit 1; }

for a in "$@"; do
  case "$a" in
    --diagnostico) MODO=diagnostico ;;
    --serie=*)     SERIE="${a#*=}" ;;
    --token-ruta=*) TOKEN_RUTA="${a#*=}" ;;
    --token-ruta)   shift; TOKEN_RUTA="${1:-}" ;;
    --ip=*)        IP_PUBLICA_PC="${a#*=}" ;;
    -h|--help)     sed -n '2,45p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) warn "opción desconocida: $a" ;;
  esac
done

command -v adb >/dev/null 2>&1 || die "no hay 'adb' en el PATH (instala platform-tools)"
[ -f "$INSTALLER" ] || die "no encuentro el instalador en $INSTALLER"

ADB=(adb)
[ -n "$SERIE" ] && ADB=(adb -s "$SERIE")

# ---------------------------------------------------------------------------
# 1) Detectar el móvil
# ---------------------------------------------------------------------------
say "buscando el móvil por ADB..."
SERIE_ACTUAL=""
if [ -n "$SERIE" ]; then
  SERIE_ACTUAL="$SERIE"
  say "usando el serie indicado: $SERIE_ACTUAL"
  "${ADB[@]}" connect "$SERIE" >/dev/null 2>&1 || true
else
  # 1a) USB
  SERIE_ACTUAL=$("${ADB[@]}" devices 2>/dev/null | awk 'NR>1 && $2=="device"{print $1; exit}')
  # 1b) LAN (el móvil con ADB TCP abierto en 5555)
  if [ -z "$SERIE_ACTUAL" ]; then
    for ip in 192.168.1.124 192.168.1.1 ; do
      say "no hay USB; probando ADB TCP en $ip:5555..."
      if "${ADB[@]}" connect "$ip:5555" 2>&1 | grep -qi "connected"; then
        SERIE_ACTUAL="$ip:5555"; break
      fi
    done
  fi
fi
[ -n "$SERIE_ACTUAL" ] || die "no encuentro el móvil (conecta el USB o deja ADB TCP abierto en 5555). Si el móvil está APAGADO, esto no lo arregla: enciéndelo y vuelve a lanzar esto."
ADB=(adb -s "$SERIE_ACTUAL")
say "móvil: $SERIE_ACTUAL"

# Estado del móvil (para el informe): root? daemon vivo? ?
SH_ROOT() { "${ADB[@]}" shell "su -mm -c '$1'" 2>/dev/null | tr -d '\r'; }
DIAG_MOVIL="$(SH_ROOT 'id -u; pgrep -f megadaemon-core | head -1; ls -l /data/local/tmp/megadaemon/megadaemon-core 2>/dev/null | cut -c1-120; echo ---; cat /data/local/tmp/megadaemon/state/channel.json 2>/dev/null | head -c 400')"
say "estado del móvil:"
printf '%s\n' "$DIAG_MOVIL" | sed 's/^/    /'

if [ "$MODO" = diagnostico ]; then
  say "modo diagnóstico: no se toca nada. Fin."
  exit 0
fi

# ---------------------------------------------------------------------------
# 2 + 3) Subir el instalador y el token de DuckDNS
# ---------------------------------------------------------------------------
say "subiendo el instalador al móvil..."
"${ADB[@]}" shell "mkdir -p $MD_DIR" >/dev/null 2>&1 || true
"${ADB[@]}" push "$INSTALLER" "$MD_DIR/instalador-megadaemon.sh" >/dev/null 2>&1 \
  || die "no pude subir el instalador por ADB"
"${ADB[@]}" shell "chmod 755 $MD_DIR/instalador-megadaemon.sh" >/dev/null 2>&1 || true

# El token: nunca está en el repo. Se busca en el PC.
if [ -z "$TOKEN_RUTA" ]; then
  for t in "$HOME/.mavis/duckdns.token" "$HOME/.duckdns.token" "$REPO/../md-secrets/duckdns.token"; do
    [ -f "$t" ] && TOKEN_RUTA="$t" && break
  done
fi
if [ -n "$TOKEN_RUTA" ] && [ -f "$TOKEN_RUTA" ]; then
  say "subiendo el token de DuckDNS (0600) para que el MÓVIL publique el A record..."
  "${ADB[@]}" push "$TOKEN_RUTA" "$MD_DIR/duckdns.token" >/dev/null 2>&1 \
    && "${ADB[@]}" shell "chmod 600 $MD_DIR/duckdns.token" >/dev/null 2>&1 \
    && say "  token subido" \
    || warn "  no pude subir el token (el móvil publicará solo el túnel rápido)"
else
  warn "no hay token de DuckDNS en el PC: el móvil publicará SÓLO el túnel rápido (el dominio estable no se arregla)"
fi

# ---------------------------------------------------------------------------
# 4) Ejecutar el instalador como root en el móvil
# ---------------------------------------------------------------------------
say "ejecutando el instalador en el móvil (root)..."
"${ADB[@]}" shell "su -mm -c 'sh $MD_DIR/instalador-megadaemon.sh --instalar --movil'" 2>&1 | tail -60

# ---------------------------------------------------------------------------
# 5) Verificar desde el PC (esto SÍ tiene red de casa)
# ---------------------------------------------------------------------------
say "verificando..."
IP_PUBLICA_PC="${IP_PUBLICA_PC:-$(curl -fsSL -m 15 https://api.ipify.org 2>/dev/null | tr -d '\r\n ')}"
RESOLVIDO="$(curl -fsSL -m 15 -H 'accept: application/dns-json' \
  "https://cloudflare-dns.com/dns-query?name=$DDNS_FULL&type=A" 2>/dev/null \
  | tr ',' '\n' | grep -o '"data":"[0-9.]*"' | head -1 | cut -d'"' -f4)"
say "IP publica del PC:    ${IP_PUBLICA_PC:-<desconocida>}"
say "$DDNS_FULL -> ${RESOLVIDO:-<no resuelve>}"
if [ -n "$RESOLVIDO" ] && [ -n "$IP_PUBLICA_PC" ] && [ "$RESOLVIDO" != "$IP_PUBLICA_PC" ]; then
  warn "el dominio NO apunta a la IP publica del PC (si apunta a 47.253.x.x, es el NAT del datacenter: alguien lo publico desde un nodo)"
fi

CODE="$(curl -s -o /dev/null -w '%{http_code}' -m 25 "https://$DDNS_FULL/health" 2>/dev/null)"
say "/health por el dominio: HTTP ${CODE:-000}"
QUICK="$("${ADB[@]}" shell "cat $MD_DIR/state/channel.json 2>/dev/null" 2>/dev/null | grep -o '"quick"[^,}]*' | head -1)"
say "tunel rapido en el movil: ${QUICK:-<no publicado>}"
QURL="$(printf '%s' "$QUICK" | sed 's/.*"quick"[^"]*"//; s/^[^:]*: *//; s/^"//; s/"$//')"
if [ -n "$QURL" ] && [ "${QURL#http}" != "$QURL" ]; then
  QCODE="$(curl -s -o /dev/null -w '%{http_code}' -m 25 "$QURL/health" 2>/dev/null)"
  say "/health por el tunel rapido: HTTP ${QCODE:-000}"
fi

cat <<EOF

[recuperar] RESUMEN
  - movil:            $SERIE_ACTUAL
  - $DDNS_FULL:  ${RESOLVIDO:-<no resuelve>}
  - /health dominio:  HTTP ${CODE:-000}
  - tunel rapido:     ${QURL:-<no publicado>} (HTTP ${QCODE:-000})
  - daemon en el movil: $(printf '%s' "$DIAG_MOVIL" | grep -q megadaemon-core && echo "vivo" || echo "SIN PROCESO")

Si /health da 200, el movil esta de vuelta y la flota vuelve a verlo
(el hostname del tunel rapido cambia en cada reinicio: 'tools/md_mcp.sh list'
prueba las URLs guardadas y se queda con la que responde).
EOF

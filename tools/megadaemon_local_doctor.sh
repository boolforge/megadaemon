#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# megadaemon_local_doctor.sh — POR QUÉ NO ME LLEGA EL MEGADAEMON DEL PC
#
# PARA QUÉ
#   Cuando el usuario dice "no me funciona", el enjambreResponsable hace
#   remontar el móvil, pero el problema suele estar en el PC: el RELAY está
#   caído, el dominio no resuelve, el móvil no está en la LAN o el móvil no
#   encuentra su propia key (un 403 NO es "key caducada").
#
#   Este script dice, en una pasada, QUÉ está roto y QUÉ comando exacto
#   arreglarlo.  Es de SÓLO LECTURA: no escribe nada, no reinicia nada y no
#   imprime secretos (de la key, sólo su sha16).
#
# USO
#     tools/megadaemon_local_doctor.sh                 # diagnóstico
#     tools/megadaemon_local_doctor.sh --json          # sólo JSON
#     tools/megadaemon_local_doctor.sh --dominio otro.duckdns.org
#     tools/megadaemon_local_doctor.sh --telefono 192.168.1.124
#
# CÓDIGOS DE SALIDA
#     0 = todo OK · 1 = hay algo roto (mira el REMEDIO) · 2 = uso incorrecto
# ---------------------------------------------------------------------------
set -uo pipefail

DOMAIN="${MEGADAEMON_DOMAIN:-mdtmfnc.duckdns.org}"
PHONE_IP="${MEGADAEMON_PHONE_IP:-192.168.1.124}"
PHONE_PORT="${MEGADAEMON_PHONE_PORT:-3000}"
TIMEOUT="${TIMEOUT:-6}"
SOLO_JSON=0
FALLS=0
JSON=""

say()  { [ "$SOLO_JSON" -eq 1 ] || echo "==> $*"; }
ok()   { [ "$SOLO_JSON" -eq 1 ] || echo "    OK    $*"; JSON="$JSON\"$1\":\"ok\","; }
ko()   { [ "$SOLO_JSON" -eq 1 ] || echo "    FAIL  $*"; JSON="$JSON\"$1\":\"ko\","; FALLS=$((FALLS+1)); }
info() { [ "$SOLO_JSON" -eq 1 ] || echo "    ..    $*"; }
rem()  { [ "$SOLO_JSON" -eq 1 ] || echo "    REMEDIO: $*"; JSON="$JSON\"remedio_$1\":\"$2\","; }

# sha16 de un valor: nunca se imprime el secreto.
sha16() { printf '%s' "$1" | sha256sum | cut -c1-16; }

http_code() {  # 000 si no hay respuesta (curl imprime 000 Y sale con error)
    local code
    code="$(curl -s -o /dev/null -m "$TIMEOUT" -w '%{http_code}' "$1" 2>/dev/null)"
    [ -z "$code" ] && code="000"
    echo "$code"
}


# Cuerpo + código de una URL (para leer el JSON del 403 del daemon).
http_cuerpo() {
    curl -s -m "$TIMEOUT" -w '\n%{http_code}' "$1" 2>/dev/null
}

# Extrae un campo del JSON del daemon (sin depender de jq).
json_campo() {  # $1 = json, $2 = campo
    printf '%s' "$1" | python3 -c "
import json,sys
try:
    d=json.loads(sys.stdin.read() or '{}')
except Exception:
    sys.exit(0)
v=d.get('$2')
print('' if v is None else v)
" 2>/dev/null
}

while [ $# -gt 0 ]; do
    case "$1" in
        --dominio)  DOMAIN="$2"; shift 2 ;;
        --telefono) PHONE_IP="$2"; shift 2 ;;
        --json)     SOLO_JSON=1; shift ;;
        -h|--help)  sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "opción desconocida: $1" >&2; exit 2 ;;
    esac
done

say "MegaDaemon local — diagnóstico (dominio: $DOMAIN)"

# 1) DNS ---------------------------------------------------------------------
IP_RESUELTO="$(getent hosts "$DOMAIN" 2>/dev/null | awk '{print $1}' | head -1)"
if [ -n "$IP_RESUELTO" ]; then
    ok "dns"; info "$DOMAIN -> $IP_RESUELTO"
else
    ko "dns"
    rem "dns" "El dominio no resuelve: el RELAY del PC publica el A record de DuckDNS. En el PC: sudo ./deploy/relay-pc.sh --instalar"
fi

# 2) El relay escucha en 80/443 ----------------------------------------------
CODE80="$(http_code "http://$DOMAIN/")"
CODE443="$(http_code "https://$DOMAIN/")"
if [ "$CODE80" = "000" ] && [ "$CODE443" = "000" ]; then
    ko "relay"
    rem "relay" "El relay del PC está CAÍDO (80 y 443 sin respuesta). En el PC: sudo ./deploy/relay-pc.sh --instalar (o --diagnostico para ver por qué)"
else
    ok "relay"; info "http=$CODE80  https=$CODE443"
fi

# 3) El daemon: aquí se decide 000 vs 530 vs 403-JSON vs 403-sin-JSON ------
# Criterio el MISMO que docs/RUNBOOK_PRODUCCION.md §2.2 y §5.
SALIDA="$(http_cuerpo "https://$DOMAIN/api/status")"
CODE_DAEMON="$(printf '%s' "$SALIDA" | tail -1)"
CUERPO="$(printf '%s' "$SALIDA" | sed '$d')"
case "$CODE_DAEMON" in
    200)
        ok "daemon"; info "GET /api/status -> 200 (el daemon vive y la credencial vale)"
        ;;
    530)
        ko "daemon"
        rem "daemon" "530 = el relay/vpn esta VIVO pero el ORIGEN (movil) esta caido => reinicia el movil (o 'sudo ./deploy/relay-pc.sh --diagnostico')"
        ;;
    403)
        RAZON="$(json_campo "$CUERPO" reason)"
        SENT="$(json_campo "$CUERPO" sent_key_fp)"
        ROTADO="$(json_campo "$CUERPO" key_rotated_at)"
        KEYPATH="$(json_campo "$CUERPO" key_path)"
        SDIR="$(json_campo "$CUERPO" state_dir)"
        if [ -n "$RAZON" ]; then
            ko "daemon"; info "403 con JSON del daemon: reason='$RAZON' sent_key_fp=$SENT key_rotated_at=$ROTADO"
            case "$RAZON" in
                *"no tiene ninguna key configurada"*)
                    rem "daemon" "El daemon NO tiene key (NO es key caducada): su state_dir esta vacio. Copia el fichero a $KEYPATH (sin reiniciar) => deploy/phone_finalize.sh (paso 0). state_dir=$SDIR"
                    ;;
                *)
                    rem "daemon" "La credencial no coincide con la vigente (key rotada). Copia la vigente al $KEYPATH y publica SOLO su sha16 (nunca la key). sent_key_fp=$SENT key_rotated_at=$ROTADO"
                    ;;
            esac
        else
            ko "daemon"
            rem "daemon" "403 SIN JSON del daemon => no es el daemon: es un proxy/relay que corta (runbook §5). Revisa: sudo ./deploy/relay-pc.sh --diagnostico"
        fi
        ;;
    401)
        ko "daemon"
        rem "daemon" "401 = la ruta pide TOKEN DE SESION, no API key: abre la UI en el movil, no desde el PC (runbook §5)"
        ;;
    000)
        ko "daemon"
        rem "daemon" "000 = no hay ruta: no resuelve o el relay no escucha. Mira el DNS y el relay (pasos anteriores)"
        ;;
    *)
        ko "daemon"; info "GET /api/status -> $CODE_DAEMON"
        rem "daemon" "Codigo inesperado ($CODE_DAEMON): mira la respuesta cruda con 'curl -s -i https://$DOMAIN/api/status'"
        ;;
esac

# 4) El canónico (lo que lee el enjambre) ------------------------------------
CODE_CANAL="$(http_code "https://$DOMAIN/channel.json")"
if [ "$CODE_CANAL" = "200" ]; then
    ok "canal"
else
    ko "canal"
    rem "canal" "El canonico /channel.json no responde 200 (code=$CODE_CANAL): el proxy no tiene el upstream (el movil) o la ruta esta mal. Revisa: sudo ./deploy/relay-pc.sh --diagnostico"
fi

# 5) El móvil en la LAN (el upstream del relay) -----------------------------
CODE_MOVIL="$(http_code "http://$PHONE_IP:$PHONE_PORT/health")"
if [ "$CODE_MOVIL" = "200" ]; then
    ok "movil_lan"; info "$PHONE_IP:$PHONE_PORT/health -> 200"
else
    ko "movil_lan"
    rem "movil_lan" "El móvil no responde en $PHONE_IP:$PHONE_PORT/health (code=$CODE_MOVIL): está en otra red o cambió de IP por DHCP. El beacon LAN lo publica: tools/phone_url.py; o fija MEGADAEMON_PHONE_IP"
fi

# 6) El movil tiene SU key (el 403 NO es key caducada) -----------------------
# El daemon lee la key de SU state_dir; si el fichero no está ahí, TODA key
# da 403 y parece que "la clave caducó".
KEY_PATH_LISTA=0
if command -v adb >/dev/null 2>&1; then
    DEV="$(adb devices 2>/dev/null | awk 'NR>1 && $2=="device" {print $1; exit}')"
    if [ -n "$DEV" ]; then
        info "adb: $DEV"
        for sd in /data/user/0/com.agy/files/megadaemon/state /data/local/tmp/megadaemon/state; do
            H="$(adb shell "su -mm -c 'cat $sd/megadaemon_api.key 2>/dev/null'" 2>/dev/null | tr -d '\r\n')"
            if [ -n "$H" ]; then
                KEY_PATH_LISTA=1
                info "key encontrada en $sd (sha16 $(sha16 "$H"))"
            fi
        done
        if [ "$KEY_PATH_LISTA" -eq 0 ]; then
            ko "key_movil"
            rem "key_movil" "El móvil NO tiene megadaemon_api.key en su state_dir: por eso todo da 403 (NO es key caducada). Copia el fichero al state_dir del daemon (sin reiniciar) y publica sólo su sha16"
        else
            ok "key_movil"
        fi
    else
        info "adb: sin dispositivo conectado (no se puede comprobar la key del móvil)"
    fi
else
    info "adb no está instalado: no se puede comprobar la key del móvil"
fi

# 7) El relay en ESTA maquina (si estamos en el PC del usuario) --------------
if command -v systemctl >/dev/null 2>&1; then
    for svc in caddy nginx megadaemon-relay; do
        if systemctl list-unit-files 2>/dev/null | grep -q "^${svc}"; then
            if systemctl is-active --quiet "$svc" 2>/dev/null; then
                ok "servicio_$svc"; info "$svc está activo"
            else
                ko "servicio_$svc"
                rem "servicio_$svc" "El servicio $svc está parado en este PC: sudo systemctl restart $svc (o sudo ./deploy/relay-pc.sh --instalar)"
            fi
        fi
    done
fi

if [ "$SOLO_JSON" -eq 1 ]; then
    printf '{"dominio":"%s","fallos":%s,"resultados":{%s}}\n' "$DOMAIN" "$FALLS" "${JSON%,}"
else
    echo
    if [ "$FALLS" -eq 0 ]; then
        echo "TODO OK: el MegaDaemon local está vivo y el canónico responde."
    else
        echo "HAY $FALLS PROBLEMA(S): mira los REMEDIOS de arriba (en este orden)."
    fi
fi
exit $([ "$FALLS" -eq 0 ] && echo 0 || echo 1)

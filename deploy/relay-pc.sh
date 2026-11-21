#!/usr/bin/env bash
# =============================================================================
# relay-pc.sh — RELAY DEL CANAL en el PC del usuario (Linux)
#
# POR QUÉ
#   El móvil (Samsung) está detrás de NAT en la LAN (192.168.1.124) y su
#   *quick tunnel* de Cloudflare **rota el hostname en cada reinicio** ⇒ el
#   sandbox se queda ciego. La salida es que el **PC** haga de relay:
#
#       DuckDNS ──► router (443) ──► PC (este relay) ──► móvil (LAN:3000)
#
#   Así el dominio `mdtmfnc-s21.duckdns.org` es **estable** y con TLS
#   válido (Let's Encrypt por DNS-01, que es lo que permite DuckDNS), sin
#   depender de que el quick tunnel esté vivo.
#
# USO
#   sudo ./deploy/relay-pc.sh --instalar          # instala y arranca
#   sudo ./deploy/relay-pc.sh --diagnostico       # sólo comprueba
#
#   Flags exactos (lo que puede pegar el usuario):
#     --instalar     : escribe Caddyfile (o config nginx), arranca el proxy
#                      y VERIFICA https://<dominio>/channel.json y /health.
#     --diagnostico  : comprueba móvil, DNS, herramientas y token. NO toca nada.
#
#   Variables de entorno:
#     MEGADAEMON_PHONE_IP   (por defecto 192.168.1.124)
#     MEGADAEMON_PHONE_PORT (por defecto 3000)
#     MEGADAEMON_DOMAIN     (por defecto mdtmfnc-s21.duckdns.org)
#     DUCKDNS_TOKEN         (token de DuckDNS; se lee de ~/.mavis/duckdns.token
#                            si no está en el entorno)
#
# REQUISITOS (en el PC)
#   * caddy  (recomendado: TLS automático con el módulo DNS de DuckDNS)  O
#   * nginx + socat (respaldo manual)
#   * adb opcional: si el móvil no está en la misma LAN, se hace
#     `adb reverse tcp:3000 tcp:3000` y el proxy apunta a 127.0.0.1.
#
# QUÉ HACE
#   1. Comprueba que el móvil responde (http://IP:PUERTO/health).
#   2. Escribe la configuración del proxy (Caddyfile o nginx).
#   3. Arranca/recarga el proxy.
#   4. Verifica el canal a través del dominio y DICE QUÉ ABRIR.
#
# NO toca el móvil: el relay es del PC y el móvil no necesita cambiar nada.
# =============================================================================
set -uo pipefail

PHONE_IP="${MEGADAEMON_PHONE_IP:-192.168.1.124}"
PHONE_PORT="${MEGADAEMON_PHONE_PORT:-3000}"
DOMAIN="${MEGADAEMON_DOMAIN:-mdtmfnc-s21.duckdns.org}"
LISTEN_PORT="${LISTEN_PORT:-443}"
CADDY_DIR="${CADDY_DIR:-/etc/caddy}"
CADDYFILE="$CADDY_DIR/Caddyfile"
TOKEN_FILE="${TOKEN_FILE:-$HOME/.mavis/duckdns.token}"
MODE="${1:---diagnostico}"

say()  { echo "==> $*"; }
warn() { echo "AVISO: $*" >&2; }
die()  { echo "ERROR: $*" >&2; exit 1; }

# ── 0. token de DuckDNS (para el TLS DNS-01) ────────────────────────────────
DUCKDNS_TOKEN="${DUCKDNS_TOKEN:-}"
if [ -z "$DUCKDNS_TOKEN" ] && [ -r "$TOKEN_FILE" ]; then
    DUCKDNS_TOKEN="$(cat "$TOKEN_FILE" | tr -d '\r\n ')"
fi

# ── 1. ¿responde el móvil? ─────────────────────────────────────────────────
health() {
    curl -s -m 5 -o /dev/null -w '%{http_code}' "http://$1:$2/health" 2>/dev/null
}

CODE=$(health "$PHONE_IP" "$PHONE_PORT")
if [ "$CODE" != "200" ]; then
    warn "el móvil no responde en http://$PHONE_IP:$PHONE_PORT/health (HTTP $CODE)"
    warn "si el móvil no está en esta LAN: 'adb reverse tcp:$PHONE_PORT tcp:$PHONE_PORT'"
    warn "y pon MEGADAEMON_PHONE_IP=127.0.0.1"
else
    say "móvil OK en http://$PHONE_IP:$PHONE_PORT/health"
fi

# ── 2. configuración ───────────────────────────────────────────────────────
escribir_caddy() {
    [ -n "$DUCKDNS_TOKEN" ] || die "falta el token de DuckDNS (DUCKDNS_TOKEN o $TOKEN_FILE)"
    mkdir -p "$CADDY_DIR"
    cat > "$CADDYFILE" <<EOF
# Generado por deploy/relay-pc.sh — relay del canal hacia el móvil.
# El TLS se emite con el desafío DNS-01 de DuckDNS (sin puertos abiertos
# para ACME y con un dominio estable).
$DOMAIN {
    tls {
        dns duckdns {
            token {$DUCKDNS_TOKEN}
        }
    }
    # El canónico del canal: lo sirve el daemon (PÚBLICO, payload mínimo).
    # El sandbox lee https://$DOMAIN/channel.json => URL estable, sin PAT y
    # sin caché de CDN (por eso NO se usa el 'raw' de GitHub).
    handle /channel.json {
        reverse_proxy $PHONE_IP:$PHONE_PORT
    }
    reverse_proxy $PHONE_IP:$PHONE_PORT
    header Strict-Transport-Security "max-age=31536000"
}
EOF
    say "Caddyfile escrito en $CADDYFILE"
}

escribir_nginx() {
    mkdir -p /etc/nginx/conf.d
    cat > /etc/nginx/conf.d/megadaemon-relay.conf <<EOF
# Generado por deploy/relay-pc.sh — relay del canal hacia el móvil.
server {
    listen $LISTEN_PORT ssl;
    server_name $DOMAIN;
    ssl_certificate     /etc/ssl/megadaemon/$DOMAIN.pem;
    ssl_certificate_key /etc/ssl/megadaemon/$DOMAIN.key;
    # Canónico del canal: lo sirve el daemon (público, payload mínimo).
    location = /channel.json {
        proxy_pass http://$PHONE_IP:$PHONE_PORT;
        proxy_set_header Host \$host;
        proxy_set_header X-Forwarded-Proto https;
    }
    location / {
        proxy_pass http://$PHONE_IP:$PHONE_PORT;
        proxy_set_header Host \$host;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto https;
    }
}
EOF
    say "config nginx escrita en /etc/nginx/conf.d/megadaemon-relay.conf"
    warn "el TLS en este modo es MANUAL: pide el certificado con certbot --dns-ducks"
}

arrancar() {
    if command -v caddy >/dev/null 2>&1; then
        systemctl reload caddy 2>/dev/null || systemctl restart caddy 2>/dev/null \
            || { say "arrancando caddy a mano"; nohup caddy run --config "$CADDYFILE" >/var/log/caddy-relay.log 2>&1 & }
        say "caddy en marcha"
    elif command -v nginx >/dev/null 2>&1; then
        nginx -t && systemctl reload nginx 2>/dev/null || systemctl restart nginx 2>/dev/null
        say "nginx en marcha"
    else
        die "no hay caddy ni nginx: instala uno de los dos (apt install caddy)"
    fi
}

# ── 3. verificación ────────────────────────────────────────────────────────
verificar() {
    sleep 2
    # El canónico del canal: el sandbox lo leerá aquí.
    ch=$(curl -s -m 10 "https://$DOMAIN/channel.json" 2>/dev/null | head -c 300)
    if [ -n "$ch" ]; then
        say "CANÓNICO del canal: https://$DOMAIN/channel.json -> $ch"
    else
        warn "https://$DOMAIN/channel.json no responde (¿el daemon lo sirve? ¿el daemon está vivo?)"
    fi
    code=$(curl -s -m 10 -o /dev/null -w '%{http_code}' "https://$DOMAIN/health" 2>/dev/null)
    if [ "$code" = "200" ]; then
        say "CANAL ESTABLE: https://$DOMAIN/health -> 200"
        say "  ⇒ ésa es la URL que deben usar los agentes (no el quick tunnel)"
    else
        warn "https://$DOMAIN/health -> $code (¿el router no reenvía $LISTEN_PORT? ¿DNS sin propagar?)"
        warn "  comprueba: 1) el A record de $DOMAIN apunta a la IP pública de casa"
        warn "           2) el router reenvía $LISTEN_PORT -> este PC"
        warn "           3) dig $DOMAIN +short"
    fi
}

case "$MODE" in
    --instalar|--install)
        if command -v caddy >/dev/null 2>&1; then escribir_caddy
        else escribir_nginx; fi
        arrancar
        verificar
        ;;
    --diagnostico|--diag|"")
        CODE=$(health "$PHONE_IP" "$PHONE_PORT")
        say "movil: http://$PHONE_IP:$PHONE_PORT/health -> $CODE"
        say "dominio: $DOMAIN -> $(dig +short "$DOMAIN" 2>/dev/null | tail -1)"
        say "caddy: $(command -v caddy || echo 'no instalado') | nginx: $(command -v nginx || echo 'no instalado')"
        say "token DuckDNS: $([ -n "$DUCKDNS_TOKEN" ] && echo 'disponible' || echo 'FALTA')"
        ;;
    *)
        die "uso: $0 [--instalar|--diagnostico]"
        ;;
esac

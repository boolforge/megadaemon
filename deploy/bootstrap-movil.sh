#!/usr/bin/env bash
# =====================================================================
# bootstrap-movil.sh — DEJA EL MÓVIL CON MEGADAEMON EN PIE, de una vez.
#
# POR QUÉ (MEDIDO 2026-11-23, en el móvil del usuario):
#   * el binario vivía en `/data/local/tmp/megadaemon` y esa ruta es
#     VOLÁTIL: al reiniciar el móvil desapareció el binario y el supervisor
#     se quedó relanzando en bucle con "no existe megadaemon-core"
#     (=> sin daemon, sin túnel, sin nada);
#   * el fichero `tunnel.url` guardaba `https://api.trycloudflare.com`
#     (la URL de la API que aparece en el log de cloudflared), no el túnel.
#
# Este script hace TODO, en un comando, y NO necesita ADB ni PC:
#
#     bash <(curl -sL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/bootstrap-movil.sh)
#
# Pasos: ruta PERSISTENTE (/data/adb/megadaemon) · build del release con
# SHA-256 verificado · supervisor + keeper de Magisk · entorno con
# auto-update cada 30 min · arranque · y te imprime la URL del túnel.
# =====================================================================
set -u

PREFIX=/data/adb/megadaemon
SD=/data/adb/service.d
RAW=https://raw.githubusercontent.com/boolforge/megadaemon/main
RELEASES=https://github.com/boolforge/megadaemon/releases/latest/download
ASSET=megadaemon-core-android-arm64
LOG=/sdcard/megadaemon.log

log() { printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"; }

# --- 1) root (se re-lanza con `su -mm` si no lo somos) --------------------
# MEDIDO (2026-11-23, Termux en Android): `bash <(curl ...)` NO funciona:
# Android no expone /proc/self/fd para el proceso de Termux =>
# "No such file or directory".  Con este script hay DOS formas:
#   A) curl -sL <url> -o bootstrap-movil.sh && bash bootstrap-movil.sh   (mejor)
#   B) curl -sL <url> | bash          (funciona: el script se guarda solo)
if [ "$(id -u)" != "0" ]; then
    if [ -f "$0" ]; then
        log "pidiendo root (su -mm) ..."
        exec su -mm -c "bash $0"
    fi
    # Venimos por tuberia: no hay fichero que re-ejecutar con root.  Se
    # guarda el script (lo que queda de stdin) y se re-ejecuta con root.
    SELF=/data/local/tmp/bootstrap-movil.$$.sh
    cat > "$SELF"
    chmod 755 "$SELF" 2>/dev/null
    log "pidiendo root (su -mm) ..."
    exec su -mm -c "bash $SELF"
fi
log "root ok"

# --- 2) un directorio de trabajo y el prefijo PERSISTENTE ---------------
WORK=$(mktemp -d 2>/dev/null || echo /data/local/tmp/mdb)
mkdir -p "$WORK" "$PREFIX" "$PREFIX/state" "$SD"
chmod 700 "$PREFIX" 2>/dev/null

# --- 3) la build del release, con su SHA-256 -----------------------------
log "descargando $ASSET ..."
if ! curl -fL --retry 3 --max-time 900 -o "$WORK/$ASSET" "$RELEASES/$ASSET"; then
    log "ERROR: no se pudo descargar la build"
    exit 1
fi
curl -fL --max-time 60 -o "$WORK/$ASSET.sha256" "$RELEASES/$ASSET.sha256" 2>/dev/null || true
ESPERADO=$(awk '{print $1}' "$WORK/$ASSET.sha256" 2>/dev/null | head -1)
REAL=$(sha256sum "$WORK/$ASSET" | cut -d' ' -f1)
if [ -n "$ESPERADO" ] && [ "$ESPERADO" != "$REAL" ]; then
    log "ERROR: sha256 no coincide (esperado $ESPERADO, real $REAL) -> no se instala nada"
    exit 1
fi
log "build verificada: $REAL"

# --- 4) instalacion ATOMICA (el anterior queda en .prev) ----------------
if [ -f "$PREFIX/megadaemon-core" ]; then
    cp -f "$PREFIX/megadaemon-core" "$PREFIX/megadaemon-core.prev" 2>/dev/null
fi
cp -f "$WORK/$ASSET" "$PREFIX/megadaemon-core.new" 2>/dev/null && \
    chmod 755 "$PREFIX/megadaemon-core.new" 2>/dev/null && \
    mv -f "$PREFIX/megadaemon-core.new" "$PREFIX/megadaemon-core" 2>/dev/null
INSTALLADO=$(sha256sum "$PREFIX/megadaemon-core" 2>/dev/null | cut -d' ' -f1)
[ "$INSTALLADO" = "$REAL" ] || { log "ERROR: la build instalada no cuadra ($INSTALLADO)"; exit 1; }
log "build instalada en $PREFIX/megadaemon-core"

# --- 5) supervisor + keeper (Magisk service.d) ---------------------------
for f in 99-megadaemon.sh 97-megadaemon-keeper.sh; do
    if curl -fL --max-time 120 -o "$WORK/$f" "$RAW/deploy/$f"; then
        cp -f "$WORK/$f" "$SD/$f" 2>/dev/null && chmod 755 "$SD/$f" 2>/dev/null
        log "$f instalado ($(sha256sum "$SD/$f" 2>/dev/null | cut -c1-16))"
    else
        log "AVISO: no se pudo descargar $f"
    fi
done

# --- 6) entorno (secretos aparte; el auto-update cada 30 min) -----------
ENVF="$PREFIX/megadaemon.env"
touch "$ENVF"; chmod 600 "$ENVF"
grep -q MD_AUTO_UPDATE_INTERVAL_MIN "$ENVF" || echo 'MD_AUTO_UPDATE_INTERVAL_MIN=30' >> "$ENVF"
grep -q MEGADAEMON_STATE_DIR "$ENVF" || echo "MEGADAEMON_STATE_DIR=$PREFIX/state" >> "$ENVF"
grep -q MEGADAEMON_LISTEN "$ENVF" || echo 'MEGADAEMON_LISTEN=127.0.0.1:9090' >> "$ENVF"

# --- 7) arrancar (el supervisor se queda; el daemon, debajo) ------------
log "arrancando el supervisor ..."
pkill -9 -x megadaemon-core 2>/dev/null
pkill -f '99-megadaemon.sh' 2>/dev/null
sleep 2
setsid nohup "$SD/99-megadaemon.sh" >> "$LOG" 2>&1 < /dev/null &
sleep 12

# --- 8) verificar y ANUNCIAR -------------------------------------------
H=$(curl -s --max-time 10 http://127.0.0.1:9090/health 2>/dev/null)
if [ -z "$H" ]; then
    log "AVISO: el daemon no responde todavia (puede tardar unos segundos)"
    sleep 8
    H=$(curl -s --max-time 10 http://127.0.0.1:9090/health 2>/dev/null)
fi
echo
echo "===================== MEGADAEMON EN EL MOVIL ====================="
echo "health : ${H:0:400}"
echo "prefijo: $PREFIX"
echo "supervisor: $(ls -l "$SD/99-megadaemon.sh" 2>/dev/null | tr -s ' ')"
echo "keeper   : $(ls -l "$SD/97-megadaemon-keeper.sh" 2>/dev/null | tr -s ' ')"
URL=$(grep -ao 'https://[a-z0-9-]*\.trycloudflare\.com' "$PREFIX/tunnel.log" 2>/dev/null | grep -v '^https://api\.' | tail -1)
[ -z "$URL" ] && URL=$(cat "$PREFIX/tunnel.url" 2>/dev/null)
echo "TUNEL PUBLICO: ${URL:-<aun no hay: mira $PREFIX/tunnel.log>}"
echo "================================================================="
echo "copia esta linea para que la flota se conecte:"
echo "${URL:-PENDIENTE}"
rm -rf "$WORK" 2>/dev/null

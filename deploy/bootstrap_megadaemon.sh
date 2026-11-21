#!/bin/sh
# =============================================================================
# bootstrap_megadaemon.sh — deja MegaDaemon INSTALADO, con URL FIJA y vivo
#
# QUÉ HACE (y qué NO hace)
#   Es un *bootstrap*: prepara el móvil y deja el daemon funcionando.  El
#   trabajo real (descargar, verificar, publicar el canal, sondear) lo hace
#   el propio binario de MegaDaemon:
#       megadaemon-core update            # última build de Android + sha256
#       megadaemon-core probe <url>       # sonda HTTP (sin curl ni wget)
#       megadaemon-core channel publish   # publica el canal canónico
#   El script NO usa Python, NO usa wget/curl en el móvil (no existen: MEDIDO
#   2026-11-21) y NO depende de ningún script externo para el servicio.
#
#   1. PREVIO:       root, directorios, binario, DuckDNS, supervisor.
#   2. VERDE:        NO instala si el CI de main NO está en verde (--force lo salta).
#   3. INSTALAR:     la ÚLTIMA build de Android del release, con sha256
#                    verificado e instalación atómica (nunca a medias).
#   4. URL FIJA:     DuckDNS canónico (no caduca) + publicación del canal.
#   5. ULTRA ROBUSTO: supervisor siempre activo + sonda con backoff (3 fallos
#                    SEGUIDOS, 30 s) y espera activa al reiniciar (45 s):
#                    nunca un bucle de reinicios.
#   6. ROLLBACK:     si la build nueva no responde, se restaura la anterior.
#
# USO
#   Desde un PC con adb (recomendado):   ./deploy/bootstrap_megadaemon.sh
#   En el móvil (Termux + root):         su -c sh bootstrap_megadaemon.sh
#   Opciones:  --check (diagnóstico, no toca nada)
#              --force (instala aunque el CI no esté verde)
#              --no-restart (no reinicia el daemon al final)
#
# IDEMPOTENTE: se puede repetir tantas veces como haga falta.
# =============================================================================

set -u

PREFIX="${MEGADAEMON_PREFIX:-/data/local/tmp/megadaemon}"
STATE="${MEGADAEMON_STATE:-$PREFIX/state}"
BIN="$PREFIX/megadaemon-core"
PREV="$PREFIX/megadaemon-core.prev"
ENV_FILE="$STATE/megadaemon.env"
CHANNEL="$STATE/channel.json"
DUCKDNS_TOKEN="${MEGADAEMON_DUCKDNS_TOKEN_FILE:-$PREFIX/duckdns.token}"
DUCKDNS_DOMAIN="${MEGADAEMON_DUCKDNS_DOMAIN:-mdtmfnc.duckdns.org}"
CANONICAL_URL="https://$DUCKDNS_DOMAIN"
SUPERVISOR="/data/adb/service.d/99-megadaemon.sh"
REPO="${MEGADAEMON_REPO:-boolforge/megadaemon}"
GITHUB_API="${MEGADAEMON_GITHUB_API:-https://api.github.com}"
LOG="${MEGADAEMON_LOG:-/sdcard/megadaemon.log}"
PORTS="${MEGADAEMON_PORTS:-9090 3000 3030 8090 8080}"
# Ventana de sondas: 3 fallos SEGUIDOS y 30 s entre sondas (nunca 5 s: un
# arranque lento se leería como "caído" y reiniciaría el daemon sano).
PROBE_FAILS=3
PROBE_SLEEP=30
# Espera ACTIVA tras un reinicio: el arranque tarda (el móvil va al 98 % de RAM).
RESTART_WAIT=45

CHECK_ONLY=0
FORCE=0
NO_RESTART=0
for a in "$@"; do
  case "$a" in
    --check) CHECK_ONLY=1 ;;
    --force) FORCE=1 ;;
    --no-restart) NO_RESTART=1 ;;
    -h|--help) sed -n '2,40p' "$0"; exit 0 ;;
  esac
done

say()  { echo "== $*"; }
warn() { echo "!! $*" >&2; }
die()  { echo "!! $*" >&2; exit 1; }

ON_PHONE=0
[ -d /system/app ] && ON_PHONE=1

# ── ejecutar en el móvil (o aquí mismo si ya estamos en él) ──────────────────
if [ "$ON_PHONE" = "1" ]; then
  ph() { su -mm -c "$1" 2>/dev/null; }
  ph_sh() { su -mm -c "sh -c '$1'" 2>/dev/null; }
else
  ph() { adb shell "su -c '$1'" 2>/dev/null | tr -d '\r'; }
  ph_sh() { adb shell "su -c \"sh -c '$1'\"" 2>/dev/null | tr -d '\r'; }
fi

have() { command -v "$1" >/dev/null 2>&1; }

# ── 0. PREVIO ───────────────────────────────────────────────────────────────
say "0 · previo"
[ "$ON_PHONE" = "1" ] || have adb || die "sin adb: ejecuta esto en un PC con adb o en el móvil con root"
if [ "$ON_PHONE" = "0" ]; then
  adb connect "${MEGADAEMON_ADB_HOST:-192.168.1.124}:${MEGADAEMON_ADB_PORT:-5555}" >/dev/null 2>&1 || true
fi
ph_sh "mkdir -p '$PREFIX' '$STATE' && chmod 700 '$PREFIX'" || true
ph_sh "[ -d '$STATE' ]" || die "no se pudo crear $STATE"

# ── 1. VERDE: el CI de main tiene que estar en verde ────────────────────────
say "1 · ¿el CI está en verde?"
ci_state() {
  # Último run del workflow en main.  Sin GitHub API (o sin red) no se puede
  # comprobar: en ese caso se avisa y se continúa (el gate es una AYUDA, no
  # una condición sin salida).
  if have curl; then
    curl -sS -m 20 "$GITHUB_API/repos/$REPO/actions/runs?branch=main&per_page=1" 2>/dev/null
  else
    echo ""
  fi
}
if [ "$FORCE" = "0" ] && [ "$CHECK_ONLY" = "0" ]; then
  RUNS=$(ci_state)
  case "$RUNS" in
    *'"conclusion": "success"'*)
      say "CI en verde: se puede instalar" ;;
    '')
      warn "no se pudo consultar el CI (sin red o sin curl): se instala igualmente" ;;
    *)
      warn "el CI de main NO está en verde: NO se instala (usa --force si de verdad quieres)"
      exit 3 ;;
  esac
fi

# ── 2. INSTALAR la última build de Android (sha256 + atómico) ───────────────
LAST_RELEASE_URL="https://github.com/$REPO/releases/latest/download"
say "2 · última build de Android"
if [ "$CHECK_ONLY" = "1" ]; then
  say "  (--check: no se instala nada)"
else
  # El propio daemon sabe actualizarse (verifica el sha256 del manifiesto):
  # si ya está instalado, se delega en él (cero dependencias externas).
  if [ -x "$BIN" ] || [ "$ON_PHONE" = "1" ]; then
    say "  delegando en el propio daemon: megadaemon-core update"
    ph_sh "[ -x '$BIN' ] && MEGADAEMON_BIN='$BIN' '$BIN' update --check" || true
    ph_sh "MEGADAEMON_BIN='$BIN' '$BIN' update" && say "  actualización aplicada por el daemon" \
      || warn "el daemon no pudo actualizarse (sigo con el binario actual)"
  fi
  # Si no hay binario (o la actualización falló), se instala desde el release.
  if [ ! -x "$BIN" ]; then
    say "  no hay binario: se instala desde el release"
    TMP="$(mktemp -d 2>/dev/null || echo /tmp/mdboot.$$)"
    mkdir -p "$TMP"
    if have curl; then
      curl -fsSL -m 300 -o "$TMP/megadaemon-core" "$LAST_RELEASE_URL/megadaemon-core-android-arm64" \
        || die "no se pudo descargar la build"
      curl -fsSL -m 60 -o "$TMP/megadaemon-core.sha256" "$LAST_RELEASE_URL/megadaemon-core-android-arm64.sha256" \
        || true
    else
      die "sin curl en esta máquina: instala desde un PC con red o usa el propio daemon (update)"
    fi
    if [ -s "$TMP/megadaemon-core.sha256" ]; then
      WANT=$(awk '{print $1}' "$TMP/megadaemon-core.sha256" | tr -d '\r')
      GOT=$(sha256sum "$TMP/megadaemon-core" 2>/dev/null | awk '{print $1}')
      [ "$WANT" = "$GOT" ] || die "sha256 NO cuadra: esperado $WANT, obtenido $GOT"
      say "  sha256 verificado: $GOT"
    else
      warn "no hay .sha256 en el release: se instala sin verificar"
    fi
    if [ "$ON_PHONE" = "1" ]; then
      cp "$TMP/megadaemon-core" "$BIN.new" && chmod 755 "$BIN.new" && mv -f "$BIN.new" "$BIN"
    else
      adb push "$TMP/megadaemon-core" /data/local/tmp/megadaemon-core.new >/dev/null \
        && ph_sh "cp -f /data/local/tmp/megadaemon-core.new '$BIN.new' && chmod 755 '$BIN.new' && mv -f '$BIN.new' '$BIN' && rm -f /data/local/tmp/megadaemon-core.new"
    fi
    ph_sh "[ -x '$BIN' ]" || die "el binario instalado no es ejecutable (chmod 755)"
  fi
  # Binario anterior para el rollback.
  ph_sh "[ -x '$BIN' ] && cp -f '$BIN' '$PREV' 2>/dev/null" || true
fi

# ── 3. URL FIJA (DuckDNS) + publicación del canal ────────────────────────────
say "3 · URL fija ($CANONICAL_URL)"
if [ "$CHECK_ONLY" = "0" ]; then
  ph_sh "[ -s '$DUCKDNS_TOKEN' ] || echo 'falta el token de DuckDNS en $DUCKDNS_TOKEN'"
  # El A record canónico lo escribe el updater (no el script): el dominio NO
  # caduca, así que la URL sirve aunque el túnel quick se caiga.
  if [ -x "$PREFIX/duckdns-updater" ]; then
    ph_sh "MEGADAEMON_DUCKDNS_DOMAIN='$DUCKDNS_DOMAIN' '$PREFIX/duckdns-updater' --once" || warn "duckdns-updater falló"
  else
    warn "no está duckdns-updater en $PREFIX: el dominio no se refresca"
  fi
  # Configuración del daemon: la credencial de Drive y el DNS canónico.
  ph_sh "touch '$ENV_FILE' && chmod 600 '$ENV_FILE'"
  ph_sh "grep -q '^MEGADAEMON_ALLOW_CANONICAL_DNS=' '$ENV_FILE' || echo 'MEGADAEMON_ALLOW_CANONICAL_DNS=1' >> '$ENV_FILE'"
  ph_sh "grep -q '^MEGADAEMON_GDRIVE_CREDENTIALS=' '$ENV_FILE' || echo 'MEGADAEMON_GDRIVE_CREDENTIALS=$STATE/gdrive_credentials.json' >> '$ENV_FILE'"
  ph_sh "grep -q '^MEGADAEMON_DUCKDNS_DOMAIN=' '$ENV_FILE' || echo 'MEGADAEMON_DUCKDNS_DOMAIN=$DUCKDNS_DOMAIN' >> '$ENV_FILE'"
  # Publicar el canal (URL fija + fingerprint de la key, NUNCA la key).
  ph_sh "[ -x '$BIN' ] && '$BIN' channel publish" || warn "channel publish falló"
fi

# ── 4. SIEMPRE VIVO: supervisor + sonda con backoff + rollback ─────────────
say "4 · siempre vivo"
if [ "$CHECK_ONLY" = "1" ]; then
  say "  (--check: no se reinicia nada)"
else
  ph_sh "[ -f '$SUPERVISOR' ] && chmod 755 '$SUPERVISOR'" || warn "no encuentro el supervisor $SUPERVISOR"
  if [ "$NO_RESTART" = "0" ]; then
    ph_sh "pkill -f megadaemon-core" 2>/dev/null || true
    sleep 2
    ph_sh "cd '$PREFIX' && nohup '$BIN' >> '$LOG' 2>&1 &" || true
    say "  reiniciado; esperando hasta ${RESTART_WAIT}s a que responda"
  fi
  # Sonda: 3 fallos SEGUIDOS, 30 s entre sondas.  Tras un reinicio se espera
  # de forma ACTIVA: sin esto, un arranque lento se lee como "caído" y se
  # reinicia el daemon sano (tormenta de reinicios).
  fails=0
  up=0
  i=0
  while [ $i -lt 12 ]; do
    i=$((i + 1))
    code=$(ph_sh "for p in $PORTS; do (exec 3<>/dev/tcp/127.0.0.1/\$p) 2>/dev/null && exec 3>&- && echo \$p && break; done" | tr -d '\r' | head -1)
    if [ -n "$code" ]; then
      up=1; fails=0
      say "  vivo (puerto $code)"
      break
    fi
    fails=$((fails + 1))
    [ "$fails" -ge "$PROBE_FAILS" ] && break
    sleep "$PROBE_SLEEP"
  done
  if [ "$up" = "0" ]; then
    warn "el daemon NO responde en los puertos $PORTS"
    if ph_sh "[ -x '$PREV' ]"; then
      warn "ROLLBACK: se restaura la build anterior"
      ph_sh "cp -f '$PREV' '$BIN' && chmod 755 '$BIN' && pkill -f megadaemon-core" 2>/dev/null || true
      sleep 3
      ph_sh "cd '$PREFIX' && nohup '$BIN' >> '$LOG' 2>&1 &" || true
      sleep "$RESTART_WAIT"
    fi
  fi
fi

# ── 5. INFORME ───────────────────────────────────────────────────────────────
say "5 · estado"
ph_sh "ls -l '$BIN' '$PREFIX'/duckdns-updater 2>/dev/null" || true
if ph_sh "[ -s '$CHANNEL' ]"; then
  say "canal publicado:"
  ph_sh "cat '$CHANNEL'" || true
fi
say "hecho. URL fija: $CANONICAL_URL"
exit 0

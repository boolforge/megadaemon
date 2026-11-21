#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# levantar_instancia.sh — deja la instancia local del usuario en pie.
#
# El runbook (docs/RUNBOOK_PRODUCCION.md) explica el PORQUÉ; este script es
# el CÓMO en una orden.  Todo lo que hace es verificable: cada paso dice qué
# comprobó y qué tiene que salir.
#
# Uso:
#   tools/levantar_instancia.sh diagnostico   # qué está bien y qué falta (no toca nada)
#   tools/levantar_instancia.sh movil         # descarga el ARM64, lo verifica y lo sube
#   tools/levantar_instancia.sh movil --bin <ruta> --serial <serial-adb>
#   tools/levantar_instancia.sh relay         # pasos del relay del PC (URL estable)
#
# MEDIDO el 2026-11-21:
#   * el 403 del enjambre NO es "key caducada": el daemon busca su key en
#     /data/user/0/com.agy/files/megadaemon/state y el instalador la deja en
#     /data/local/tmp/megadaemon/state;
#   * el quick tunnel del movil diede NXDOMAIN => no hay canal publico;
#   * el sandbox del enjambre es SOLO egress => el relay va en el PC;
#   * `phone_finalize.sh` era el pegado unico, pero tenia el release FIJO a un
#     binario retirado => ahora usa el ultimo (y ya no depende de curl).
# ---------------------------------------------------------------------------
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASES_API="https://api.github.com/repos/boolforge/megadaemon/releases/latest"
ASSET="megadaemon-core-android-arm64"
WORK="${TMPDIR:-/tmp}/md-instancia"
mkdir -p "$WORK"

say()  { printf '\033[1m[instancia]\033[0m %s\n' "$*"; }
ok()   { printf '  ✓ %s\n' "$*"; }
warn() { printf '  ! %s\n' "$*"; }
die()  { printf '  ✗ %s\n' "$*" >&2; exit 1; }

descargar() { # descargar <url> <destino>
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "$2" "$1"
  else
    python3 -c "import sys,urllib.request as u; open(sys.argv[2],'wb').write(u.urlopen(sys.argv[1],timeout=120).read())" "$1" "$2"
  fi
}

latest_tag() {
  if descargar "$RELEASES_API" "$WORK/rel.json" 2>/dev/null; then
    grep -oE '"tag_name"[[:space:]]*:[[:space:]]*"[^"]+"' "$WORK/rel.json" \
      | head -1 | sed 's/.*"\(v[^"]*\)"/\1/'
  fi
}

# --------------------------------------------------------------- diagnostico
cmd_diagnostico() {
  say "1/4 release publicado"
  local tag; tag="$(latest_tag || true)"
  [ -n "$tag" ] && ok "último release: $tag" || warn "no se pudo consultar la API de GitHub"

  say "2/4 artefacto ARM64 y su sha256"
  if [ -n "$tag" ]; then
    if descargar "https://github.com/boolforge/megadaemon/releases/download/$tag/$ASSET" "$WORK/$ASSET" \
       && descargar "https://github.com/boolforge/megadaemon/releases/download/$tag/$ASSET.sha256" "$WORK/$ASSET.sha256"; then
      chmod 755 "$WORK/$ASSET"
      if bash "$REPO/tools/acceptance_release.sh" "$WORK/$ASSET" arm64; then
        ok "artefacto listo en $WORK/$ASSET"
      else
        die "el artefacto NO pasa la puerta de aceptación: no lo subas"
      fi
    else
      warn "no se pudo descargar el release (¿sin red?)"
    fi
  fi

  say "3/4 canal desde el móvil"
  local url; url="$(cat "$WORK/tunnel.url" 2>/dev/null || true)"
  if [ -z "$url" ]; then
    url="$(grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' "$WORK/cloudflared.log" 2>/dev/null | head -1 || true)"
  fi
  if [ -n "$url" ]; then
    local code
    code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 15 "$url/health" 2>/dev/null || echo 000)"
    case "$code" in
      200) ok "el túnel del móvil responde 200 en $url" ;;
      403|401) warn "el túnel responde $code: llega el daemon ⇒ es la CREDENCIAL (ver §2.2 del runbook)" ;;
      530) warn "el túnel responde 530: el relay vive pero el MÓVIL no está conectado" ;;
      000) warn "el túnel responde 000: no resuelve o no hay ruta (¿el cloudflared del móvil está muerto?)" ;;
      *) warn "el túnel responde $code" ;;
    esac
  else
    warn "no se encontró la URL del túnel (state/tunnel.url en el móvil)"
  fi

  say "4/4 ¿tienes adb y un móvil?"
  if command -v adb >/dev/null 2>&1; then
    local devs; devs="$(adb devices | awk 'NR>1 && $2=="device" {print $1}' | tr '\n' ' ')"
    [ -n "$devs" ] && ok "adb ve: $devs" || warn "adb no ve ningún móvil en estado 'device'"
  else
    warn "no hay 'adb' (instala platform-tools) ⇒ los pasos del móvil los hace otro"
  fi

  cat <<'TXT'

  PASOS QUE QUEDAN (en el móvil, con root):
    su -mm -c 'sh /data/local/tmp/megadaemon/phone_finalize.sh'
      (credencial + binario oficial + CSC legal + reinicio + túnel + mediciones)
  Y en el PC, para la URL estable:
    sudo ./deploy/relay-pc.sh --diagnostico && sudo ./deploy/relay-pc.sh --instalar
TXT
}

# --------------------------------------------------------------------- movil
cmd_movil() {
  local BIN="" SERIAL=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --bin) BIN="$2"; shift 2 ;;
      --serial) SERIAL="$2"; shift 2 ;;
      *) shift ;;
    esac
  done
  say "descargando el último release"
  local tag; tag="$(latest_tag || true)"
  [ -n "$tag" ] || die "no se pudo resolver el último release"
  ok "release $tag"
  descargar "https://github.com/boolforge/megadaemon/releases/download/$tag/$ASSET" "$WORK/$ASSET" \
    || die "no se pudo descargar $ASSET"
  descargar "https://github.com/boolforge/megadaemon/releases/download/$tag/$ASSET.sha256" "$WORK/$ASSET.sha256" \
    || die "no se pudo descargar el .sha256"
  chmod 755 "$WORK/$ASSET"
  say "puerta de aceptación"
  bash "$REPO/tools/acceptance_release.sh" "$WORK/$ASSET" arm64 || die "el artefacto NO vale: no se sube"
  say "subiendo al móvil"
  local extra=""
  [ -n "$SERIAL" ] && extra="--serial $SERIAL"
  bash "$REPO/deploy/phone_push_arm64.sh" --bin "$WORK/$ASSET" $extra \
    || die "el push falló (¿adb? ¿root? ¿el móvil saturado?)"
  ok "binario subido; falta el paso 0 de la credencial (phone_finalize.sh)"
  cat <<'TXT'

  SIGUIENTE (en el móvil, con root) — credencial + CSC legal + túnel:
    su -mm -c 'sh /data/local/tmp/megadaemon/phone_finalize.sh'
  y comprueba:
    su -mm -c 'curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:9090/health'   # 200
TXT
}

# --------------------------------------------------------------------- relay
cmd_relay() {
  say "relay del PC (URL estable para los agentes)"
  if [ "$(id -u)" != "0" ]; then
    warn "hace falta root (sudo) para instalar caddy y el certificado"
  fi
  say "diagnóstico"
  bash "$REPO/deploy/relay-pc.sh" --diagnostico || true
  cat <<'TXT'

  Si el diagnóstico dice que el móvil no aparece por el beacon LAN:
    export MEGADAEMON_PHONE_IP=<ip-del-móvil>
  y luego:
    sudo ./deploy/relay-pc.sh --instalar
  (necesita el token de DuckDNS en ~/.mavis/duckdns.token, 0600)
TXT
}

case "${1:-diagnostico}" in
  diagnostico) cmd_diagnostico ;;
  movil) shift; cmd_movil "$@" ;;
  relay) cmd_relay ;;
  *) echo "uso: $0 {diagnostico|movil|relay}" >&2; exit 2 ;;
esac

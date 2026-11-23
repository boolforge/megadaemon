#!/data/data/com.termux/files/usr/bin/bash
# =============================================================================
# MegaDaemon v14 — auto-actualización de la instancia
# =============================================================================
# Qué hace:
#   1. Consulta el manifiesto `latest.json` del release de GitHub.
#   2. Si hay un commit nuevo, descarga el binario de SU arquitectura.
#   3. Verifica el SHA-256 contra el `.sha256` publicado.
#   4. Instala en una ruta nueva (versionada), hace swap atómico y reinicia.
#   5. Si el nuevo binario falla al arrancar, ROLLBACK automático a la
#      versión anterior (que queda en el directorio anterior).
#
# Garantías:
#   * Nunca borra la versión anterior hasta que la nueva arranca bien.
#   * Todo queda auditado en $STATE/update.log.
#   * Es idempotente: se puede llamar cada N minutos desde el supervisor.
#
# Variables de entorno (opcionales):
#   MEGADAEMON_REPO      owner/repo            (por defecto boolforge/megadaemon)
#   MEGADAEMON_CHANNEL   main | <tag>          (por defecto main)
#   MEGADAEMON_PREFIX    prefijo de instalación (por defecto /data/local/tmp/megadaemon)
#   MEGADAEMON_STATE     dir de estado          (por defecto $MEGADAEMON_PREFIX/state)
#   MEGADAEMON_NO_UPDATE=1  desactiva la comprobación (debug)
# =============================================================================
set -u

PREFIX="${MEGADAEMON_PREFIX:-/data/local/tmp/megadaemon}"
STATE="${MEGADAEMON_STATE:-$PREFIX/state}"
REPO="${MEGADAEMON_REPO:-boolforge/megadaemon}"
CHANNEL="${MEGADAEMON_CHANNEL:-main}"
BIN="$PREFIX/megadaemon-core"
VERSIONS="$PREFIX/versions"
LOG="$STATE/update.log"
mkdir -p "$STATE" "$VERSIONS"

log() { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" >>"$LOG"; echo "$*"; }

[ "${MEGADAEMON_NO_UPDATE:-0}" = "1" ] && { log "auto-update desactivado (MEGADAEMON_NO_UPDATE=1)"; exit 0; }

# --- arquitectura -------------------------------------------------------------
detect_asset() {
  case "$(uname -m)" in
    aarch64|arm64)
      if [ -n "${MEGADAEMON_ANDROID:-}" ]; then echo "megadaemon-core-android-arm64"
      else echo "megadaemon-core-linux-arm64"; fi ;;
    x86_64|amd64) echo "megadaemon-core-linux-x86_64" ;;
    *) echo "" ;;
  esac
}

# Si estamos en Android (Termux) lo detectamos de forma fiable.
if [ -z "${MEGADAEMON_ANDROID:-}" ] && [ -d "/data/data/com.termux/files/usr" ]; then
  MEGADAEMON_ANDROID=1
fi
ASSET="$(detect_asset)"
[ -z "$ASSET" ] && { log "arquitectura no soportada: $(uname -m)"; exit 0; }

# --- manifiesto ---------------------------------------------------------------
MANIFEST_URL="https://raw.githubusercontent.com/$REPO/$CHANNEL/dist/latest.json"
# Fallback: el manifiesto se publica como artefacto del workflow; se mantiene
# también una copia en el propio repo para que la descarga no dependa del run.
if command -v curl >/dev/null 2>&1; then
  MANIFEST="$(curl -fsSL --max-time 25 "$MANIFEST_URL" 2>/dev/null || true)"
fi
if [ -z "${MANIFEST:-}" ]; then
  # Sin manifiesto: se usa el commit remoto de la rama como versión.
  if command -v git >/dev/null 2>&1; then
    SHA="$(git ls-remote "https://github.com/$REPO.git" "$CHANNEL" 2>/dev/null | awk '{print $1}')"
    [ -n "$SHA" ] && MANIFEST="{\"version\":\"$SHA\"}"
  fi
fi
[ -z "${MANIFEST:-}" ] && { log "no se pudo obtener el manifiesto; sin cambios"; exit 0; }

NEW_SHA="$(printf '%s' "$MANIFEST" | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
[ -z "$NEW_SHA" ] && { log "manifiesto sin versión"; exit 0; }

CURRENT_SHA="$(cat "$STATE/current_sha" 2>/dev/null || echo '')"
if [ "$NEW_SHA" = "$CURRENT_SHA" ] && [ -x "$BIN" ]; then
  log "ya en la última versión ($NEW_SHA)"
  exit 0
fi
log "actualización: $CURRENT_SHA -> $NEW_SHA (asset: $ASSET)"

# --- descarga -----------------------------------------------------------------
URL="https://github.com/$REPO/releases/download/v$NEW_SHA/$ASSET"
SHA_URL="$URL.sha256"
TMP="$STATE/dl.$ASSET"
if ! curl -fL --max-time 300 --retry 3 -o "$TMP" "$URL" 2>>"$LOG"; then
  log "ERROR: descarga fallida de $URL"; rm -f "$TMP"; exit 1
fi
EXPECT=""
if curl -fsSL --max-time 60 -o "$STATE/dl.sha256" "$SHA_URL" 2>/dev/null; then
  EXPECT="$(awk '{print $1}' "$STATE/dl.sha256" | head -1)"
fi
if [ -n "$EXPECT" ]; then
  GOT="$(sha256sum "$TMP" | awk '{print $1}')"
  if [ "$EXPECT" != "$GOT" ]; then
    log "ERROR: SHA-256 no coincide (esperado $EXPECT, obtenido $GOT)"; rm -f "$TMP"; exit 1
  fi
  log "SHA-256 verificado: $GOT"
else
  log "AVISO: sin .sha256 publicado; se instala sin verificación externa"
fi

# --- instalación con rollback -------------------------------------------------
NEW_VER="$VERSIONS/$NEW_SHA"
mkdir -p "$NEW_VER"
cp -f "$TMP" "$NEW_VER/megadaemon-core"
chmod 755 "$NEW_VER/megadaemon-core"
# MEDIDO (2026-11-23): una plantilla web VIEJA en disco tiene prioridad sobre
# la embebida => tras actualizar el binario, el movil podia seguir sirviendo
# la UI vieja.  Regla: si la plantilla es mas antigua que el binario nuevo,
# se borra => el daemon sirve la embebida (que es la del binario nuevo).
for _tpl in "$PREFIX/web/cockpit_v2.html" "$PREFIX/web/webui_v2.css" "$PREFIX/web/cockpit_template.html"; do
    if [ -f "$_tpl" ] && [ -f "$NEW_VER/megadaemon-core" ]; then
        if [ "$_tpl" -ot "$NEW_VER/megadaemon-core" ]; then
            rm -f "$_tpl" 2>/dev/null || true
            log "plantilla obsoleta retirada (se sirve la embebida): $_tpl"
        fi
    fi
done
# La plantilla web embebida va en el binario, pero si existe en disco tiene
# prioridad: se refresca para que la UI llegue al día.
if [ -f "$PREFIX/web/cockpit_template.html" ]; then
  mkdir -p "$NEW_VER/web"
  cp -f "$PREFIX/web/cockpit_template.html" "$NEW_VER/web/" 2>/dev/null || true
fi
# La V2 (la que se sirve ahora) tambien viaja con la version nueva: si no,
# el movil se queda con la UI vieja aunque el binario sea nuevo.
if [ -f "$PREFIX/web/cockpit_v2.html" ]; then
  mkdir -p "$NEW_VER/web"
  cp -f "$PREFIX/web/cockpit_v2.html" "$NEW_VER/web/" 2>/dev/null || true
fi
if [ -f "$PREFIX/web/webui_v2.css" ]; then
  mkdir -p "$NEW_VER/web"
  cp -f "$PREFIX/web/webui_v2.css" "$NEW_VER/web/" 2>/dev/null || true
fi

PREV=""
[ -x "$BIN" ] && PREV="$(readlink -f "$BIN" 2>/dev/null || echo "$BIN")"

# swap atómico
ln -sf "$NEW_VER/megadaemon-core" "$BIN.new" 2>/dev/null && mv -f "$BIN.new" "$BIN"
ln -sf "$NEW_VER/megadaemon-core" "$PREFIX/current" 2>/dev/null || true

log "binario instalado en $NEW_VER; reiniciando"
if [ -x "$PREFIX/restart.sh" ]; then
  if "$PREFIX/restart.sh" >>"$LOG" 2>&1; then
    echo "$NEW_SHA" > "$STATE/current_sha"
    log "reinicio correcto; versión activa $NEW_SHA"
    # Limpieza: se conservan las 3 últimas versiones.
    ls -1dt "$VERSIONS"/* 2>/dev/null | tail -n +4 | while read -r old; do
      [ "$old" = "$NEW_VER" ] && continue
      rm -rf "$old" && log "limpieza: $old eliminado"
    done
    exit 0
  fi
fi

# Falló el reinicio: rollback.
log "ERROR: el reinicio falló; rollback a ${PREV:-ninguna}"
if [ -n "$PREV" ] && [ -x "$PREV" ]; then
  ln -sf "$PREV" "$BIN.new" 2>/dev/null && mv -f "$BIN.new" "$BIN"
  [ -x "$PREFIX/restart.sh" ] && "$PREFIX/restart.sh" >>"$LOG" 2>&1
  log "rollback completado a $PREV"
else
  log "no hay versión anterior para restaurar"
fi
exit 1

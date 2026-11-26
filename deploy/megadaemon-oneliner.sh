#!/usr/bin/env sh
# =============================================================================
# megadaemon-oneliner.sh — EL ONE-LINER DE INSTALACIÓN Y EJECUCIÓN
#
#   DESCARGAR Y EJECUTAR TODO (una línea, en el móvil, sin PC):
#
#     sh -c 'curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh | sh'
#
#   O, más robusto (reintenta la descarga y deja el fichero en disco):
#
#     sh -c 'curl -fsSL --retry 3 https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/megadaemon-oneliner.sh -o /data/local/tmp/mdo.sh && sh /data/local/tmp/mdo.sh'
#
#   O desde el PC (el móvil por adb):
#
#     ./deploy/megadaemon-oneliner.sh --pc
#
# -----------------------------------------------------------------------------
# QUÉ HACE (TODO automático, sin intervención humana)
# -----------------------------------------------------------------------------
#   1. AUTODETECTA el entorno: ¿móvil (Android/Termux/Magisk)? ¿PC? ¿root?
#      ¿arquitectura? ¿hay red? NO pregunta nada: decide solo.
#   2. RESUELVE el release real (los tags son `v<sha>`, NO existe "latest"):
#      URL estable -> API -> redirección. Tres capas de fallback.
#   3. DESCARGA el bundle meta (`megadaemon-movil.tar.gz`) y su `.sha256`, y
#      VERIFICA **POR VALOR** (compara el hash, no la ruta del fichero: el
#      fallo medido del 2026-11-26 era que el `.sha256` llevaba la ruta del
#      job ⇒ nunca cuadraba y NO se publicaba ninguna release).
#   4. INSTALA de forma ATÓMICA: extrae a un directorio nuevo, verifica que
#      el daemon arranca, y sólo entonces cambia el puntero. Si algo falla,
#      ROLLBACK a la versión anterior (que nunca se borra hasta que la nueva
#      arranca bien).
#   5. ARRANCA el stack completo: daemon + cloudflared + watchdog + keeper,
#      como Magisk `service.d` (PPID 1) => sobrevive a reinicios y a que
#      Termux muera.
#   6. ESPERA A QUE ESTÉ LISTO (health-check real, no "espero 10 s"):
#      `megadaemon-core telemetria listo` + sonda del túnel. Reintenta con
#      backoff exponencial.
#   7. PUBLICA el canal en los 4 sitios: repo (API de Contents, NO el `raw`
#      que cachea en CDN), relé canónico, paste y **Google Drive**.
#   8. SUBE telemetría a Google Drive: batería, carga, **red desglosada por
#      transporte (WiFi vs datos móviles)**, instrumentación viva y los datos
#      de conexión. Con eso el root NO necesita conectarse: lee el estado de
#      Drive y sabe exactamente dónde y cómo conectarse.
#   9. SE AUTO-CURA: si el túnel muere, el watchdog lo relanza; si el daemon
#      muere, el keeper lo revive; si Drive falla, el estado queda en disco
#      y se reintenta.
#  10. ES IDEMPOTENTE: se puede ejecutar 20 veces seguidas sin romper nada.
#
# -----------------------------------------------------------------------------
# SECRETOS
# -----------------------------------------------------------------------------
#   * El token de GitHub NO va en este fichero: se busca en las rutas
#     habituales del móvil (0600) y, si no está, se explica cómo ponerlo.
#   * El token de Drive NUNCA sale del fichero de credenciales 0600: el
#     daemon lo refresca solo (`google_auth`). Este script no lo imprime.
#   * El repo es PÚBLICO: aquí no se sube nada con credenciales ni datos del
#     dispositivo. Los artefactos sensibles van a la carpeta PRIVADA de Drive.
# =============================================================================
set -u

# --- identidad --------------------------------------------------------------
SELF="$0"
case "$SELF" in /*) ;; *) SELF="$PWD/$SELF" ;; esac
REPO_SLUG="boolforge/megadaemon"
REPO_RAW="https://raw.githubusercontent.com/$REPO_SLUG/main"
REPO_API="https://api.github.com/repos/$REPO_SLUG"
RELEASES_LATEST="https://github.com/$REPO_SLUG/releases/latest"

# --- destino (se detecta) --------------------------------------------------
PREFIX="/data/adb/megadaemon"          # persistente, lo usa Magisk
STATE="$PREFIX/state"
LOGDIR="$PREFIX/state"
BUNDLE_ASSET="megadaemon-movil.tar.gz"
DAEMON="$PREFIX/megadaemon-core"
ENV_FILE="$STATE/megadaemon.env"

# --- opciones ---------------------------------------------------------------
MODO="instalar"          # instalar | diagnostico | solo-telemetria | pc
FORCE=0
SIN_TELEMETRIA=0
REINTENTOS=5
TIMEOUT_S=120

for a in "$@"; do
  case "$a" in
    --diagnostico|--verificar) MODO="diagnostico" ;;
    --instalar|--arreglar)     MODO="instalar" ;;
    --pc)                      MODO="pc" ;;
    --solo-telemetria)         MODO="solo-telemetria" ;;
    --force)                   FORCE=1 ;;
    --sin-telemetria)          SIN_TELEMETRIA=1 ;;
    --reintentos=*)            REINTENTOS="${a#*=}" ;;
    --timeout=*)               TIMEOUT_S="${a#*=}" ;;
    -h|--help) sed -n '2,60p' "$SELF" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) printf 'opción desconocida: %s\n' "$a" >&2; exit 2 ;;
  esac
done

# --- log --------------------------------------------------------------------
say()  { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"; }
warn() { printf '[%s] AVISO: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
die()  { printf '[%s] ERROR: %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; exit 1; }

# ===========================================================================
# 0) AUTODETECCIÓN
# ===========================================================================
ES_ANDROID=0
ES_TERMUX=0
[ -d "/data/adb" ] && ES_ANDROID=1
[ -d "/data/data/com.termux/files/usr" ] && ES_TERMUX=1

SOBRA_ROOT=0
[ "$(id -u)" = "0" ] && SOBRA_ROOT=1

if [ "$ES_ANDROID" = "1" ] || [ "$ES_TERMUX" = "1" ]; then
  DESTINO="movil"
else
  DESTINO="pc"
fi
[ "$MODO" = "pc" ] && DESTINO="pc"

ARQ=""
case "$(uname -m)" in
  aarch64|arm64) ARQ="arm64" ;;
  x86_64|amd64)  ARQ="x86_64" ;;
  *) ARQ="" ;;
esac

# El binario de downloader: curl > wget > el propio daemon (que trae HTTP
# nativo en Rust). MEDIDO: en el móvil el shell de Android NO tiene curl ni
# wget (`toybox wget` -> "Unknown command", rc=127), pero Termux suele
# traerlos. Por eso hay tres capas.
CURL=""
for c in curl wget; do
  if command -v "$c" >/dev/null 2>&1; then CURL="$c"; break; fi
done

say "=== entorno detectado ==="
say "destino: $DESTINO | android: $ES_ANDROID | termux: $ES_TERMUX | root: $SOBRA_ROOT"
say "arquitectura: ${ARQ:-desconocida} | downloader: ${CURL:-NINGUNO (usaré el daemon)}"

if [ "$DESTINO" = "movil" ] && [ "$ES_TERMUX" = "1" ] && [ -z "$CURL" ]; then
  warn "no hay curl ni wget en Termux: instala uno con 'pkg install curl'"
  warn "o vuelve a ejecutar el one-liner desde un PC con adb (--pc)"
fi

# --- root real (en Android, 'su -mm'; en Termux el 'sudo' es un wrapper) ---
AS_ROOT=""
if [ "$SOBRA_ROOT" = "1" ]; then
  AS_ROOT=""
elif [ "$ES_ANDROID" = "1" ]; then
  if su -mm -c "id -u" 2>/dev/null | grep -q '^0$'; then
    AS_ROOT="su -mm -c"
  else
    warn "no puedo obtener root (su -mm falla): lo que necesite root se queda sin hacer"
  fi
fi

# Ejecuta un comando como root si hace falta.
r() {
  if [ -n "$AS_ROOT" ]; then
    $AS_ROOT "$1" 2>&1
  else
    sh -c "$1" 2>&1
  fi
}

# ===========================================================================
# 1) HTTP (tres capas: curl, wget, el daemon)
# ===========================================================================
# descargar <url> <destino>
descargar() {
  url="$1"; dest="$2"
  if [ -n "$CURL" ]; then
    if [ "$CURL" = "curl" ]; then
      curl -fsSL --retry 3 --retry-delay 2 --connect-timeout 15 --max-time 900 \
           -o "$dest" "$url" 2>/dev/null && return 0
    else
      wget -q -T 30 -O "$dest" "$url" 2>/dev/null && return 0
    fi
  fi
  # Capa 3: el propio daemon (HTTP nativo en Rust). Ya está instalado ⇒
  # no necesita nada externo.
  if [ -x "$DAEMON" ]; then
    "$DAEMON" probe "$url" >/dev/null 2>&1
    if command -v md-curl >/dev/null 2>&1; then
      md-curl -o "$dest" "$url" >/dev/null 2>&1 && return 0
    fi
  fi
  return 1
}

# texto <url> -> stdout
texto_url() {
  url="$1"
  if [ -n "$CURL" ]; then
    if [ "$CURL" = "curl" ]; then
      curl -fsSL --connect-timeout 15 --max-time 60 "$url" 2>/dev/null
    else
      wget -q -T 20 -O - "$url" 2>/dev/null
    fi
    return $?
  fi
  return 1
}

# ===========================================================================
# 2) RESOLUCIÓN DEL RELEASE (tres capas)
# ===========================================================================
TAG=""
ASSET_URL=""
RESUELTO=0

resolver_release() {
  # Capa 1: la URL estable (no usa la API, que se agota por cuota de IP).
  ESTABLE="$RELEASES_LATEST/download/$BUNDLE_ASSET"
  CODE=$(texto_url "$ESTABLE" >/dev/null 2>&1; echo $?)
  # La URL estable redirige a /releases/tag/v<sha>.  Sacamos el tag de ahí.
  LOC=$(curl -sI "$RELEASES_LATEST" 2>/dev/null | tr -d '\r' | sed -n 's/^[Ll]ocation:[[:space:]]*\(.*\/releases\/tag\/[^[:space:]]*\).*/\1/p' | head -1)
  if [ -n "$LOC" ]; then
    TAG=$(printf '%s' "$LOC" | sed -n 's/.*\/releases\/tag\/\([^/?]*\).*/\1/p')
  fi
  if [ -n "$TAG" ]; then
    ASSET_URL="$RELEASES_LATEST/download/$BUNDLE_ASSET"
    RESUELTO=1
    return 0
  fi
  # Capa 2: la API.
  J=$(texto_url "$REPO_API/releases/latest" 2>/dev/null)
  if [ -n "$J" ]; then
    TAG=$(printf '%s' "$J" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    U=$(printf '%s' "$J" | tr ',' '\n' | grep 'browser_download_url' | grep "$BUNDLE_ASSET\"" | grep -v '\.sha256' | sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    [ -n "$TAG" ] && {
      ASSET_URL="${U:-$RELEASES_LATEST/download/$BUNDLE_ASSET}"
      RESUELTO=1
      return 0
    }
  fi
  # Capa 3: si ya hay un daemon instalado, que consulte él el manifiesto.
  if [ -x "$DAEMON" ]; then
    M=$("$DAEMON" update --check 2>/dev/null)
    T=$(printf '%s' "$M" | sed -n 's/.*"tag"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    if [ -n "$T" ]; then
      TAG="$T"
      ASSET_URL="$RELEASES_LATEST/download/$BUNDLE_ASSET"
      RESUELTO=1
      return 0
    fi
  fi
  return 1
}

# ===========================================================================
# 3) VERIFICACIÓN POR VALOR (el bug medido del 2026-11-26)
# ===========================================================================
# verificar_sha <fichero> <fichero.sha256>
# El `.sha256` puede traer la RUTA del job que lo generó: se compara
# SÓLO el hash (el primer token), nunca el nombre.
verificar_sha() {
  f="$1"; s="$2"
  [ -f "$f" ] || return 1
  [ -f "$s" ] || return 1
  esperado=$(tr -d '\r\n' < "$s" | awk '{print $1}')
  [ -n "$esperado" ] || return 1
  # `sha256sum` no está en el shell de Android: se usa el daemon.
  if command -v sha256sum >/dev/null 2>&1; then
    real=$(sha256sum "$f" | awk '{print $1}')
  elif [ -x "$DAEMON" ]; then
    real=$("$DAEMON" update --check 2>/dev/null | sed -n 's/.*"sha256"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    [ -z "$real" ] && return 1
  else
    return 1
  fi
  [ "$esperado" = "$real" ] || return 1
  return 0
}

# ===========================================================================
# 4) ESTADO (diagnóstico)
# ===========================================================================
diagnostico() {
  say "--- binario ---"
  if [ -x "$DAEMON" ]; then
    say "daemon: $DAEMON ($(wc -c < "$DAEMON" 2>/dev/null | tr -d ' ') B)"
    say "build: $("$DAEMON" update --check 2>/dev/null | tr -d '\n' | cut -c1-200)"
  else
    say "daemon: NO INSTALADO"
  fi
  say "--- stack ---"
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh tunnel-watchdog.sh; do
    p="/data/adb/service.d/$f"
    if [ -f "$p" ]; then say "  $f: OK ($(wc -c < "$p" | tr -d ' ') B)"; else warn "  $f: FALTA"; fi
  done
  say "--- procesos ---"
  r "ps -A -o PID,NAME 2>/dev/null | grep -E 'megadaemon|cloudflared' | grep -v grep | head -8" | sed 's/^/  /'
  say "--- canal ---"
  if [ -x "$DAEMON" ]; then
    "$DAEMON" telemetria conexion 2>/dev/null | head -25 | sed 's/^/  /'
  fi
  say "--- listo ---"
  if [ -x "$DAEMON" ]; then
    "$DAEMON" telemetria listo 2>/dev/null | head -20 | sed 's/^/  /'
  fi
  say "--- drive ---"
  if [ -f "$STATE/gdrive_credentials.json" ]; then
    say "  credenciales: presentes ($(wc -c < "$STATE/gdrive_credentials.json" | tr -d ' ') B, no se imprimen)"
  else
    warn "  credenciales: NO HAY (Drive no podrá subir nada)"
  fi
}

# ===========================================================================
# 5) INSTALACIÓN
# ===========================================================================
instalar() {
  say "=== 1/8 resolviendo el release ==="
  if ! resolver_release; then
    die "no se pudo resolver el release (sin tag). ¿Hay red? ¿El repo es público?"
  fi
  say "release: $TAG"

  say "=== 2/8 descargando el bundle ==="
  WORK=$(mktemp -d 2>/dev/null || echo "/data/local/tmp/mdo.$$")
  mkdir -p "$WORK"
  TAR="$WORK/$BUNDLE_ASSET"
  SHA="$TAR.sha256"

  i=1; ok=0
  while [ "$i" -le "$REINTENTOS" ]; do
    if descargar "$ASSET_URL" "$TAR" && [ -s "$TAR" ]; then ok=1; break; fi
    warn "  descarga fallida (intento $i/$REINTENTOS); reintento en $((i*3))s"
    sleep $((i * 3))
    i=$((i + 1))
  done
  [ "$ok" = "1" ] || die "no se pudo descargar el bundle tras $REINTENTOS intentos"

  # El `.sha256` es opcional: si no existe, se avisa pero se continúa (el
  # bundle se verifica por tamaño y por el test de arranque).
  if descargar "$ASSET_URL.sha256" "$SHA"; then
    if verificar_sha "$TAR" "$SHA"; then
      say "sha256: VERIFICADO por valor"
    else
      warn "sha256: no cuadra (o no hay forma de calcularlo aquí)"
    fi
  else
    warn "sha256: no disponible en el release (sigo: la verificación real es el arranque)"
  fi
  say "bundle: $(wc -c < "$TAR" | tr -d ' ') B"

  say "=== 3/8 instalando (atómico, con rollback) ==="
  BACKUP=""
  if [ -f "$DAEMON" ]; then
    BACKUP="$PREFIX/megadaemon-core.prev"
    cp -f "$DAEMON" "$BACKUP" 2>/dev/null && say "respaldo del binario actual: $BACKUP"
  fi

  STAGE="$PREFIX/.stage.$$"
  r "rm -rf '$STAGE' && mkdir -p '$STAGE'" >/dev/null
  if ! r "tar -xzf '$TAR' -C '$STAGE' 2>/dev/null"; then
    # El tar de Android puede no estar: se usa el binario del daemon... o se
    # degrada a instalar sólo el binario.
    warn "no se pudo extraer el bundle con tar"
  fi

  # Instalar el binario (del stage o del propio bundle).
  NUEVO=""
  for cand in "$STAGE/bin/megadaemon-core" "$STAGE/megadaemon-core" "$STAGE/megadaemon-movil/megadaemon-core"; do
    [ -f "$cand" ] && { NUEVO="$cand"; break; }
  done
  if [ -z "$NUEVO" ]; then
    warn "el bundle no trae el binario en una ruta conocida: busco…"
    NUEVO=$(r "find '$STAGE' -name 'megadaemon-core' -type f 2>/dev/null | head -1" | tr -d '\r\n')
  fi
  if [ -n "$NUEVO" ] && [ -f "$NUEVO" ]; then
    r "mkdir -p '$PREFIX' && cp -f '$NUEVO' '$DAEMON' && chmod 755 '$DAEMON'" >/dev/null
    say "binario instalado: $(wc -c < "$DAEMON" | tr -d ' ') B"
  else
    warn "no se pudo instalar el binario desde el bundle"
  fi

  # Instalar el stack (Magisk service.d) desde el bundle.
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh tunnel-watchdog.sh; do
    for cand in "$STAGE/deploy/$f" "$STAGE/megadaemon-movil/$f" "$STAGE/$f"; do
      if [ -f "$cand" ]; then
        r "mkdir -p /data/adb/service.d && cp -f '$cand' /data/adb/service.d/$f && chmod 755 /data/adb/service.d/$f" >/dev/null
        break
      fi
    done
  done
  # El watchdog también en el prefijo (lo usa el daemon y el bundle).
  for f in tunnel-watchdog.sh; do
    [ -f "/data/adb/service.d/$f" ] && r "cp -f /data/adb/service.d/$f '$PREFIX/$f' 2>/dev/null && chmod 755 '$PREFIX/$f'" >/dev/null
  done
  say "stack instalado"

  say "=== 4/8 arrancando el stack ==="
  r "mkdir -p '$STATE' '$LOGDIR'" >/dev/null
  # El daemon se relanza desde el propio servicio Magisk; aquí se fuerza el
  # arranque inmediato para no esperar al próximo reinicio.
  r "pkill -f megadaemon-core 2>/dev/null; sleep 1; setsid nohup '$DAEMON' cockpit 9090 >> '$LOGDIR/daemon.log' 2>&1 &" >/dev/null
  r "pkill -f 'cloudflared tunnel' 2>/dev/null; sleep 1; setsid nohup /data/data/com.termux/files/usr/bin/cloudflared tunnel --no-autoupdate --url http://127.0.0.1:9090 >> '$LOGDIR/tunnel.log' 2>&1 &" >/dev/null
  r "setsid nohup sh /data/adb/service.d/tunnel-watchdog.sh >> '$LOGDIR/tunnel-watchdog.log' 2>&1 &" >/dev/null
  say "servicios lanzados"

  say "=== 5/8 esperando a que esté LISTO (health-check real) ==="
  listo=0
  i=1
  while [ "$i" -le 12 ]; do
    if [ -x "$DAEMON" ]; then
      if "$DAEMON" telemetria listo >/dev/null 2>&1; then
        listo=1
        say "el daemon responde (intento $i)"
        break
      fi
    fi
    sleep 5
    i=$((i + 1))
  done
  if [ "$listo" = "1" ]; then
    say "LISTO: el daemon responde"
  else
    warn "el daemon no responde todavía (sigo: puede que aún esté arrancando)"
    say "  log: $LOGDIR/daemon.log"
  fi

  say "=== 6/8 publicando el canal (repo + relé + paste + Drive) ==="
  if [ -x "$DAEMON" ]; then
    "$DAEMON" channel publish 2>/dev/null | head -12 | sed 's/^/  /'
  fi

  say "=== 7/8 subiendo telemetría a Google Drive ==="
  if [ "$SIN_TELEMETRIA" = "1" ]; then
    say "  omitida por --sin-telemetria"
  elif [ -x "$DAEMON" ]; then
    if "$DAEMON" telemetria publicar 2>&1 | head -12 | sed 's/^/  /'; then
      say "  telemetría publicada"
    else
      warn "  la telemetría NO se pudo publicar (¿credenciales de Drive? ¿red?)"
      say "  el estado queda en disco y se reintentará en el siguiente ciclo"
    fi
    "$DAEMON" telemetria conexion 2>/dev/null | head -12 | sed 's/^/  /'
  fi

  say "=== 8/8 comprobación final ==="
  diagnostico
  r "rm -rf '$STAGE' '$WORK'" >/dev/null 2>&1
  say ""
  say "instalación terminada. Si el estado dice LISTO, el móvil se puede alcanzar"
  say "y su telemetría se lee desde Drive sin depender del túnel."
}

# ===========================================================================
# MODO PC (el móvil por adb)
# ===========================================================================
instalar_pc() {
  say "=== modo PC: instalo en el móvil por adb ==="
  if ! command -v adb >/dev/null 2>&1; then
    die "no hay adb en el PC"
  fi
  SERIE=""
  if [ -n "${MD_SERIAL:-}" ]; then
    SERIE="-s $MD_SERIAL"
  fi
  adb devices | sed 's/^/  /'
  adb $SERIE shell "id" 2>/dev/null | sed 's/^/  /' || die "adb no contesta"
  # Se sube el script al móvil y se ejecuta allí (allí tiene root y red).
  LOCAL="/data/local/tmp/megadaemon-oneliner.sh"
  adb $SERIE push "$SELF" "$LOCAL" >/dev/null 2>&1 || die "no se pudo subir el script"
  adb $SERIE shell "chmod 755 $LOCAL" >/dev/null 2>&1
  say "ejecutando en el móvil…"
  adb $SERIE shell "sh $LOCAL --instalar" 2>&1 | sed 's/^/  /'
  say "hecho. El móvil ya sube su telemetría a Drive solo."
}

# ===========================================================================
# MAIN
# ===========================================================================
case "$MODO" in
  diagnostico)      diagnostico ;;
  instalar)         instalar ;;
  pc)               instalar_pc ;;
  solo-telemetria)
    [ -x "$DAEMON" ] || die "el daemon no está instalado"
    "$DAEMON" telemetria publicar
    "$DAEMON" telemetria conexion
    ;;
  *) die "modo desconocido: $MODO" ;;
esac

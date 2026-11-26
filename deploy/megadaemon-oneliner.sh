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
PUBLISH_ASSET="channel-publish-arm64"
DAEMON="$PREFIX/megadaemon-core"
ENV_FILE="$PREFIX/megadaemon.env"

# --- entorno del daemon (CRÍTICO) ------------------------------------------
# ⚠️ Sin esto el daemon lee/escribe el estado en
# `/data/user/0/com.agy/files/megadaemon/state` (el de la app Antigravity,
# `rust-core/src/state_dir.rs:17`) y NO en el nuestro ⇒ el canal queda
# vacío y la telemetría se sube a Drive sin la URL del túnel.  El supervisor
# (`deploy/99-megadaemon.sh:48`) lo exporta; aquí también.
export MEGADAEMON_PREFIX="$PREFIX"
export MEGADAEMON_STATE_DIR="$STATE"
export MEGADAEMON_BIN="$DAEMON"

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

# Validación de las opciones numéricas (un `--reintentos=abc` rompería el
# bucle `[ "$i" -le "abc" ]` con un error de shell).
case "$REINTENTOS" in ''|*[!0-9]*) REINTENTOS=5 ;; esac
case "$TIMEOUT_S" in ''|*[!0-9]*) TIMEOUT_S=120 ;; esac
[ "$REINTENTOS" -lt 1 ] && REINTENTOS=1
[ "$REINTENTOS" -gt 20 ] && REINTENTOS=20
[ "$TIMEOUT_S" -lt 30 ] && TIMEOUT_S=30

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
  # ⚠️ NO se descarga el tarball entero para comprobar que existe (¡varios MB
  # de datos móviles del Jazztel!): se pide sólo el primer byte con un
  # Range, o se hace un HEAD.
  ESTABLE="$RELEASES_LATEST/download/$BUNDLE_ASSET"
  if [ -n "$CURL" ]; then
    if [ "$CURL" = "curl" ]; then
      CODE=$(curl -sI -L -o /dev/null -w '%{http_code}' --connect-timeout 15 --max-time 30 "$ESTABLE" 2>/dev/null)
    else
      CODE=$(wget -S --spider -T 20 "$ESTABLE" 2>&1 | sed -n 's/.*HTTP\/[0-9.]* \([0-9][0-9][0-9]\).*/\1/p' | tail -1)
    fi
    case "$CODE" in
      200|206|302|301)
        TAG=$(etiqueta_release)
        if [ -n "$TAG" ]; then
          ASSET_URL="$ESTABLE"
          RESUELTO=1
          return 0
        fi
        ;;
    esac
  fi
  # Capa 2: el manifiesto `latest.json` del release (asset real; la API de
  # Contents NO cachea, el `raw` sí).
  J=$(texto_url "$RELEASES_LATEST/download/latest.json" 2>/dev/null)
  if [ -n "$J" ]; then
    V=$(printf '%s' "$J" | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    if [ -n "$V" ]; then
      TAG="v$V"
      ASSET_URL="$RELEASES_LATEST/download/$BUNDLE_ASSET"
      RESUELTO=1
      return 0
    fi
  fi
  # Capa 3: la API de releases (más cara, se agota por cuota de IP).
  J=$(texto_url "$REPO_API/releases/latest" 2>/dev/null)
  if [ -n "$J" ]; then
    TAG=$(printf '%s' "$J" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    U=$(printf '%s' "$J" | tr ',' '\n' | grep 'browser_download_url' | grep "$BUNDLE_ASSET\"" | grep -v '\.sha256' | sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
    if [ -n "$TAG" ]; then
      ASSET_URL="${U:-$RELEASES_LATEST/download/$BUNDLE_ASSET}"
      RESUELTO=1
      return 0
    fi
  fi
  return 1
}

# Etiqueta (`v<sha>`) del último release, sacada de la redirección de
# /releases/latest.  Los tags SON `v<sha>`: NO existe un tag "latest".
etiqueta_release() {
  LOC=""
  if [ "$CURL" = "curl" ]; then
    LOC=$(curl -sI "$RELEASES_LATEST" 2>/dev/null | tr -d '\r' | sed -n 's/^[Ll]ocation:[[:space:]]*\(.*\/releases\/tag\/[^[:space:]]*\).*/\1/p' | head -1)
  elif [ "$CURL" = "wget" ]; then
    LOC=$(wget -S --spider "$RELEASES_LATEST" 2>&1 | tr -d '\r' | sed -n 's/.*[Ll]ocation:[[:space:]]*\(.*\/releases\/tag\/[^[:space:]]*\).*/\1/p' | head -1)
  fi
  [ -n "$LOC" ] || return 1
  printf '%s' "$LOC" | sed -n 's/.*\/releases\/tag\/\([^/?]*\).*/\1/p'
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
  # El `.sha256` del CI lleva la RUTA del job (`dist/megadaemon-movil.tar.gz`)
  # ⇒ se compara SÓLO el hash (el primer token), nunca el nombre.
  esperado=$(tr -d '\r\n' < "$s" | awk '{print $1}')
  [ -n "$esperado" ] || return 1
  real=""
  if command -v sha256sum >/dev/null 2>&1; then
    real=$(sha256sum "$f" | awk '{print $1}')
  else
    # Sin `sha256sum` (el shell de Android no lo trae): el daemon calcula el
    # hash con su cliente nativo.
    real=$(r "'$DAEMON' hash '$f' 2>/dev/null" | tr -d '\r\n ' | awk '{print $1}')
  fi
  [ -n "$real" ] || return 2
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
  # El watchdog NO está en service.d (Magisk lo ejecutaría sin entorno):
  # vive en el prefijo y lo lanza 98-cloudflared.sh.
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    p="/data/adb/service.d/$f"
    if [ -f "$p" ]; then say "  $f: OK ($(wc -c < "$p" | tr -d ' ') B)"; else warn "  $f: FALTA"; fi
  done
  if [ -f "$PREFIX/tunnel-watchdog.sh" ]; then
    say "  tunnel-watchdog.sh: OK en el prefijo ($(wc -c < "$PREFIX/tunnel-watchdog.sh" | tr -d ' ') B)"
  else
    warn "  tunnel-watchdog.sh: FALTA en el prefijo"
  fi
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
  # --- atajo de idempotencia -------------------------------------------------
  # Si ya está instalado Y el túnel responde, NO se toca nada: matar el
  # daemon y relanzar cloudflared cambiaría el hostname y ciegaría a la flota
  # en cada reejecución (el bug más caro de este script).
  if [ "$FORCE" != "1" ] && [ -x "$DAEMON" ]; then
    URL0=$(r "cat '$STATE/tunnel.url' 2>/dev/null" | tr -d '\r\n ')
    if [ -n "$URL0" ] && r "'$DAEMON' telemetria sonda '$URL0'" >/dev/null 2>&1; then
      say "ya está instalado y el túnel responde ($URL0): no toco nada."
      say "usa --force si de verdad quieres reinstalar."
      say ""
      say "estado:"
      diagnostico
      return 0
    fi
  fi

  say "=== 1/8 resolviendo el release ==="
  if ! resolver_release; then
    die "no se pudo resolver el release (sin tag). ¿Hay red? ¿El repo es público?"
  fi
  say "release: ${TAG:-(última)}"

  say "=== 2/8 descargando el bundle ==="
  # ⚠️ `/data/local/tmp` NO es escribible por el uid de Termux ⇒ el temporal
  # va a $TMPDIR (Termux lo define) o a /tmp.
  WORK=$(mktemp -d 2>/dev/null || echo "${TMPDIR:-/tmp}/mdo.$$")
  mkdir -p "$WORK" 2>/dev/null || die "no se pudo crear el temporal en $WORK"
  TAR="$WORK/$BUNDLE_ASSET"
  SHA="$TAR.sha256"
  # Limpieza al salir, pase lo que pase (incluido un `die`).
  trap 'rm -rf "$WORK" "$PREFIX/.stage.$$" 2>/dev/null' EXIT INT TERM

  i=1; ok=0
  while [ "$i" -le "$REINTENTOS" ]; do
    if descargar "$ASSET_URL" "$TAR" && [ -s "$TAR" ]; then ok=1; break; fi
    # Backoff cuadrático con tope (no lineal: si la red está caída, insistir
    # cada 3 s no ayuda y gasta datos).
    d=$((i * i)); [ "$d" -gt 30 ] && d=30
    warn "  descarga fallida (intento $i/$REINTENTOS); reintento en ${d}s"
    sleep "$d"
    i=$((i + 1))
  done
  [ "$ok" = "1" ] || die "no se pudo descargar el bundle tras $REINTENTOS intentos"

  # El `.sha256`: si existe, la verificación es OBLIGATORIA.  Si el hash
  # NO cuadra, el bundle está manipulado ⇒ se aborta (no se instala).
  if descargar "$ASSET_URL.sha256" "$SHA" && [ -s "$SHA" ]; then
    verificar_sha "$TAR" "$SHA"
    rc=$?
    if [ "$rc" = "0" ]; then
      say "sha256: VERIFICADO por valor"
    elif [ "$rc" = "2" ]; then
      die "no se pudo calcular el sha256 del bundle (ni sha256sum ni el daemon) => no instalo nada"
    else
      die "el sha256 del bundle NO cuadra => el fichero está manipulado o corrupto. NO instalo nada"
    fi
  else
    warn "sha256: no disponible en el release (sigo: la verificación real es que el daemon arranque)"
  fi
  say "bundle: $(wc -c < "$TAR" | tr -d ' ') B"

  # El publicador del canal (lo necesita para publicar en el REPO, que es el
  # punto canónico de descubrimiento: el `raw` de GitHub cachea en CDN).
  PUBLISH_BIN="$PREFIX/channel-publish"
  if [ ! -x "$PUBLISH_BIN" ]; then
    if descargar "$RELEASES_LATEST/download/$PUBLISH_ASSET" "$WORK/$PUBLISH_ASSET"; then
      r "cp -f '$WORK/$PUBLISH_ASSET' '$PUBLISH_BIN' && chmod 755 '$PUBLISH_BIN'" >/dev/null 2>&1 \
        && say "channel-publish instalado" \
        || warn "no se pudo instalar channel-publish (el canal no irá al repo)"
    else
      warn "no se pudo descargar channel-publish (el canal no irá al repo)"
    fi
  fi

  say "=== 3/8 instalando (atómico, con rollback) ==="
  BACKUP=""
  if [ -f "$DAEMON" ]; then
    BACKUP="$PREFIX/megadaemon-core.prev"
    r "cp -f '$DAEMON' '$BACKUP'" >/dev/null 2>&1 && say "respaldo del binario actual: $BACKUP"
  fi

  STAGE="$PREFIX/.stage.$$"
  r "rm -rf '$STAGE' && mkdir -p '$STAGE'" >/dev/null
  # Si el bundle no se extrae, NO se toca nada y se aborta: el peor resultado
  # posible es "móvil sin daemon + dice que lo instaló".
  if ! r "tar -xzf '$TAR' -C '$STAGE'"; then
    r "rm -rf '$STAGE'" >/dev/null 2>&1
    die "no se pudo extraer el bundle (tar) => no instalo nada"
  fi

  # Instalar el binario (del stage o del propio bundle).
  NUEVO=""
  for cand in "$STAGE/bin/megadaemon-core" "$STAGE/megadaemon-core" "$STAGE/megadaemon-movil/megadaemon-core"; do
    [ -f "$cand" ] && { NUEVO="$cand"; break; }
  done
  if [ -z "$NUEVO" ]; then
    NUEVO=$(r "find '$STAGE' -name 'megadaemon-core' -type f 2>/dev/null | head -1" | tr -d '\r\n')
  fi
  [ -n "$NUEVO" ] && [ -f "$NUEVO" ] || {
    r "rm -rf '$STAGE'" >/dev/null 2>&1
    die "el bundle no trae el binario (esperado: bin/megadaemon-core)"
  }
  # .new + mv (atómico en el MISMO sistema de ficheros) + ejecutable.
  # Nunca se copia ENCIMA del binario en ejecución: si el móvil se duerme a
  # mitad, el binario queda truncado y el móvil se queda sin daemon.
  r "mkdir -p '$PREFIX' && cp -f '$NUEVO' '$DAEMON.new' && chmod 755 '$DAEMON.new' && mv -f '$DAEMON.new' '$DAEMON'" >/dev/null
  r "test -x '$DAEMON'" >/dev/null 2>&1 || {
    [ -n "$BACKUP" ] && r "cp -f '$BACKUP' '$DAEMON' && chmod 755 '$DAEMON'" >/dev/null 2>&1
    die "el binario nuevo no quedó ejecutable: binario anterior restaurado"
  }
  say "binario instalado: $(r "wc -c < '$DAEMON'" | tr -d ' \r\n') B"

  # Instalar el stack (Magisk service.d) desde el bundle.
  # ⚠️ `tunnel-watchdog.sh` NO va a service.d: Magisk lo ejecutaría en cada
  # arranque SIN entorno (LOG_DIR, DAEMON_BIN, MEGADAEMON_STATE_DIR) ⇒
  # entraría en un ciclo de relanzamiento de cloudflared (cambia el hostname
  # cada 60 s y ciega a la flota).  Su sitio es el PREFIJO
  # (`rust-core/src/stack.rs`: `Destino::Prefijo("tunnel-watchdog.sh")`).
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    for cand in "$STAGE/deploy/$f" "$STAGE/megadaemon-movil/$f" "$STAGE/$f"; do
      if [ -f "$cand" ]; then
        r "mkdir -p /data/adb/service.d && cp -f '$cand' /data/adb/service.d/$f && chmod 755 /data/adb/service.d/$f" >/dev/null
        break
      fi
    done
  done
  # El watchdog, al prefijo (lo lanza 98-cloudflared.sh con su entorno).
  for cand in "$STAGE/deploy/tunnel-watchdog.sh" "$STAGE/tunnel-watchdog.sh"; do
    if [ -f "$cand" ]; then
      r "cp -f '$cand' '$PREFIX/tunnel-watchdog.sh' && chmod 755 '$PREFIX/tunnel-watchdog.sh'" >/dev/null
      break
    fi
  done
  say "stack instalado"

  say "=== 4/8 arrancando el stack ==="
  r "mkdir -p '$STATE' '$LOGDIR'" >/dev/null
  # El daemon, CON SU ENTORNO (sin esto escribe el estado en el sitio
  # equivocado) y CON `< /dev/null` (si hereda el stdin, la sesión adb se
  # queda colgada).
  r "pkill -f 'megadaemon-core cockpit' 2>/dev/null; sleep 1" >/dev/null
  r "MEGADAEMON_PREFIX='$PREFIX' MEGADAEMON_STATE_DIR='$STATE' MEGADAEMON_BIN='$DAEMON' setsid nohup '$DAEMON' cockpit 9090 >> '$LOGDIR/daemon.log' 2>&1 < /dev/null &" >/dev/null

  # El túnel: si ya hay un cloudflared (el supervisor puede estar vivo), se
  # RESPETA.  Relanzarlo a ciegas genera un hostname nuevo ⇒ la flota se
  # queda ciega, que es justo lo que este script debe evitar.
  CF_BIN="$PREFIX/cloudflared"
  [ -x "$CF_BIN" ] || CF_BIN="/data/data/com.termux/files/usr/bin/cloudflared"
  if ! r "ps -A -o NAME 2>/dev/null | grep -q '^cloudflared$'" >/dev/null 2>&1; then
    r "setsid nohup '$CF_BIN' tunnel --no-autoupdate --url http://127.0.0.1:9090 >> '$LOGDIR/tunnel.log' 2>&1 < /dev/null &" >/dev/null
  else
    say "hay un cloudflared vivo: no lo relanzo (el hostname se conservaría)"
  fi

  # El watchdog, con SU entorno (tunnel-watchdog.sh:25,39,40,43,52,53).
  r "LOG_DIR='$PREFIX' DAEMON_BIN='$DAEMON' PUBLISH='$PREFIX/channel-publish' MEGADAEMON_STATE_DIR='$STATE' MEGADAEMON_ENV='$PREFIX/megadaemon.env' CF='$CF_BIN' CFT_TARGET='http://127.0.0.1:9090' setsid nohup sh '$PREFIX/tunnel-watchdog.sh' >> '$LOGDIR/tunnel-watchdog.log' 2>&1 < /dev/null &" >/dev/null
  say "servicios lanzados"

  # El daemon tiene que ARRANCAR: si no, se restaura el anterior.
  i=0
  while [ "$i" -lt 25 ]; do
    r "pgrep -f 'megadaemon-core cockpit' >/dev/null 2>&1" && break
    sleep 1
    i=$((i + 1))
  done
  if [ "$i" -ge 25 ]; then
    [ -n "$BACKUP" ] && r "cp -f '$BACKUP' '$DAEMON' && chmod 755 '$DAEMON'" >/dev/null 2>&1
    r "pkill -f 'megadaemon-core cockpit' 2>/dev/null" >/dev/null
    r "setsid nohup '$DAEMON' cockpit 9090 >> '$LOGDIR/daemon.log' 2>&1 < /dev/null &" >/dev/null
    die "el daemon no arrancó con el binario nuevo: binario anterior restaurado"
  fi

  say "=== 5/8 esperando a que esté LISTO (health-check real) ==="
  # La URL viva: del log de cloudflared -> <state_dir>/tunnel.url (lo lee el
  # daemon: `channel.rs`).  Sin esto, `telemetria listo` NUNCA es true.
  URL=""
  i=1
  while [ "$i" -le 24 ]; do
    URL=$(r "cat '$STATE/tunnel.url' 2>/dev/null" | tr -d '\r\n ')
    [ -n "$URL" ] && break
    URL=$(r "grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' '$LOGDIR/tunnel.log' 2>/dev/null | tail -1" | tr -d '\r\n ')
    if [ -n "$URL" ]; then
      r "printf '%s' '$URL' > '$STATE/tunnel.url' && chmod 644 '$STATE/tunnel.url'" >/dev/null
      r "printf '%s' '$URL' > '$PREFIX/tunnel.url'" >/dev/null 2>&1
      break
    fi
    sleep 5
    i=$((i + 1))
  done
  [ -n "$URL" ] || warn "no se encontró la URL del túnel (mira $LOGDIR/tunnel.log)"

  listo=0
  i=1
  while [ "$i" -le 12 ]; do
    # El health-check REAL: 401/403 = el daemon contesta = túnel VIVO;
    # 0/502/503/504/530 = muerto (`channel.rs::es_canal_muerto`).
    if [ -n "$URL" ] && r "'$DAEMON' telemetria sonda '$URL'" >/dev/null 2>&1; then
      listo=1
      say "el túnel responde (intento $i): $URL"
      break
    fi
    sleep 5
    i=$((i + 1))
  done
  if [ "$listo" = "1" ]; then
    say "LISTO: el móvil es alcanzable en $URL"
  else
    warn "el móvil NO es alcanzable todavía (daemon vivo ≠ túnel vivo)"
    say "  log: $LOGDIR/daemon.log"
  fi

  say "=== 6/8 publicando el canal (repo + relé + paste + Drive) ==="
  if [ -x "$DAEMON" ]; then
    r "'$DAEMON' channel publish" 2>/dev/null | head -12 | sed 's/^/  /'
  fi
  # El REPO (Contents API: NO cachea en CDN) es el punto canónico de
  # descubrimiento: sin esto, cuando cambia el hostname del quick tunnel la
  # flota se queda ciega.  Lo publica `channel-publish --repo-publish`.
  if [ -x "$PREFIX/channel-publish" ]; then
    publico=0
    for t in "$PREFIX/.gh_token" "$PREFIX/github.token" "$STATE/.gh_token" "$STATE/github.token"; do
      [ -f "$t" ] || continue
      r "'$PREFIX/channel-publish' --repo-publish --state-dir '$STATE' --token-file '$t'" >/dev/null 2>&1 && publico=1
      break
    done
    if [ "$publico" = "1" ]; then
      say "  canal publicado en el repo (state/channel.json)"
    else
      warn "  el canal NO se pudo publicar en el repo (¿token de GitHub?)"
    fi
  else
    warn "  falta channel-publish: el canal no se publica en el repo"
  fi

  say "=== 7/8 subiendo telemetría a Google Drive ==="
  if [ "$SIN_TELEMETRIA" = "1" ]; then
    say "  omitida por --sin-telemetria"
  elif [ -x "$DAEMON" ]; then
    if r "'$DAEMON' telemetria publicar" 2>&1 | head -12 | sed 's/^/  /'; then
      say "  telemetría publicada"
    else
      warn "  la telemetría NO se pudo publicar (¿credenciales de Drive? ¿red?)"
      say "  el estado queda en disco y se reintentará en el siguiente ciclo"
    fi
    r "'$DAEMON' telemetria conexion" 2>/dev/null | head -12 | sed 's/^/  /'
  fi

  say "=== 8/8 comprobación final ==="
  diagnostico
  r "rm -rf '$STAGE' '$WORK'" >/dev/null 2>&1
  say ""
  if [ "$listo" = "1" ]; then
    say "instalación terminada: el móvil es alcanzable en $URL"
    say "y su telemetría se lee desde Drive sin depender del túnel."
  else
    warn "instalación terminada, pero el túnel no responde aún."
    warn "revisar $LOGDIR/daemon.log y $LOGDIR/tunnel.log"
    return 1
  fi
  return 0
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
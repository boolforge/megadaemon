#!/usr/bin/env bash
# =============================================================================
# test-oneliner.sh — BANCO DEL INSTALADOR DEL MÓVIL (el one-liner)
#
# POR QUÉ EXISTE ESTE BANCO
# ------------------------
# El instalador real (`deploy/megadaemon-oneliner.sh`) NO se puede probar en
# el móvil real: cada ejecución real tarda minutos, consume datos y, cuando
# falla, deja el móvil a medias.  Pero el instalador es LA PUERTA DE ENTRADA
# del usuario (una línea pegada en Termux) ⇒ un fallo suyo es un fallo
# público.  Los 4 bugs que se vieron en el SM-G998B (MEDIDO 2026-11-27) sólo
# aparecieron en el móvil.
#
# CÓMO ESTE BANCO ES DIFERENTE (v2, 2026-11-27)
# ---------------------------------------------
# La v1 (10 escenarios) era un guiso: un único laboratorio, el `curl` de
# mentira en el PATH, y `grep` sobre el fuente del instalador para "probar"
# cosas que sólo se pueden probar ejecutándolo.  Eso deja pasar bugs reales.
#
# La v2:
#   * AISLA cada escenario: laboratorio NUEVO (prefijo, estado, service.d,
#     fakes, logs) ⇒ un escenario no puede contaminar a otro.
#   * RESTRINGE el PATH a una "caja de herramientas" ⇒ el banco es HERMÉTICO
#     (nunca toca la red real) y reproduce el shell de Android, que no tiene
#     curl, ni wget, ni sha256sum, ni ps, ni pgrep.
#   * SIMULA /proc: el banco controla cuántos "daemons", "supervisores",
#     "watchdogs" y "cloudflared" hay ⇒ se puede probar el diagnóstico,
#     el rollback y la limpieza de duplicados sin esperar a que nada arranque.
#   * SIMULA la red: cada URL del release se puede fazer fallar por separado
#     (estable, manifiesto, API, .sha256) ⇒ se prueban las TRES capas de
#     resolución del release y el backoff de descarga.
#   * Aísla el fallo: un escenario que no se puede hacer en este entorno
#     (sin root, sin compilador) se OMITE con un motivo explícito; NUNCA se
#     marca como "OK" para que el banco siga verde.
#
# USO
# ---
#   bash tools/test-oneliner.sh                  # todo el banco
#   bash tools/test-oneliner.sh --fichero=tools/test-oneliner-casos-b.sh
#   bash tools/test-oneliner.sh --solo='caso_base_*'
#   bash tools/test-oneliner.sh --listar
#   MD_PAR=8 bash tools/test-oneliner.sh         # más paralelismo
#
# CÓMO SE ESCRIBE UN ESCENARIO (ver tools/test-oneliner-casos-*.sh)
# ---------------------------------------------------------------
#   caso_mi_escenario() {
#     NET=down          # knob: se fija ANTES de preparar_lab
#     preparar_lab      # (re)crea el laboratorio con los knobs actuales
#     correr_instalador --instalar --sin-telemetria
#     comprobar "aborta sin red" [ "$CODIGO" != "0" ]
#     salida_mensaje "explica que no hay red" "no se pudo resolver el release"
#   }
#
# Los knobs disponibles están documentados en `preparar_lab`.
# =============================================================================
set -u

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
BASE="${MD_LAB_BASE:-/tmp/md-lab}"
PAR="${MD_PAR:-4}"

# --- opciones ---------------------------------------------------------------
FICHEROS=()
SOLO=""
LISTAR=0
for a in "$@"; do
  case "$a" in
    --fichero=*) FICHEROS+=("${a#*=}") ;;
    --solo=*)    SOLO="${a#*=}" ;;
    --par=*)     PAR="${a#*=}" ;;
    --listar)    LISTAR=1 ;;
    -h|--help)   sed -n '2,60p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) printf 'opción desconocida: %s\n' "$a" >&2; exit 2 ;;
  esac
done
case "$PAR" in ''|*[!0-9]*) PAR=4 ;; esac
[ "$PAR" -lt 1 ] && PAR=1
[ "$PAR" -gt 16 ] && PAR=16

# --- el banco necesita root para los escenarios de escalada -----------------
# MEDIDO (2026-11-27): sin esto el job `sintaxis` del CI salía rojo entero
# (23 fallos) porque el instalador moría con "necesito root".
if [ "$(id -u)" != "0" ] && command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
  exec sudo -n -E bash "$0" "$@"
fi

# ---Verboso helpers ---------------------------------------------------------
say()  { printf '\n== %s ==\n' "$*"; }
ok()   { printf '  OK    %s\n' "$*"; }
mal()  { printf '  FALLO %s\n' "$*"; printf '%s\n' "$*" >> "$L/fallos.txt"; }
omitir(){ printf '  OMIT  %s\n' "$*"; printf '%s\n' "$*" >> "$L/omites.txt"; }

# =============================================================================
# LOS KNOBS (lo que cada escenario puede falsear del entorno real)
# =============================================================================
# NET        ok | down | 404 | sin-manifiesto | solo-api | sin-sha
# BUNDLE     ok | corrupto | vacio | sin-binario | sin-mddoctor | sin-stack |
#            no-ejecutable
# SHA        ok | distinto | ausente | vacio | con-ruta
# DL         curl | wget | ninguno        (qué downloader encuentra)
# SIN_SHA256SUM 1 = la caja de herramientas NO trae sha256sum
# PREFIX_0700    1 = el prefijo es de root con 0700 (hay que escalar)
# TUNEL      ok | caido | sin-url
# HEALTH     ok | ko
# ESPACIO_MB número = MB libres que ve `df` (0 = disco lleno)
# SIN_CURL   1 = no hay curl en la caja (ni en el PATH)
# DAEMONS    número = cuántos daemons "viven"
# SUPERVISORS/WATCHDOGS/CLOUDFLARED/KEEPERS = cuántos viven de cada clase
# SIN_MDOCTOR 1 = el bundle NO trae el núcleo forense
knobs_reset() {
  NET=ok; BUNDLE=ok; SHA=ok; DL=curl; SIN_SHA256SUM=0; PREFIX_0700=0
  TUNEL=ok; HEALTH=ok; ESPACIO_MB=999999; SIN_CURL=0; SIN_MDOCTOR=0
  # TERMUX=1 simula el movil (Termux): /data/data/com.termux y uname -o.
  TERMUX=0
  # RELOJ_VIRTUAL=1: el `sleep` es instantaneo y el `date +%s` avanza 6 s por
  # llamada => se puede MEDIR el presupuesto de espera del instalador sin
  # esperar de verdad (y comparar --timeout=30 con el de por defecto).
  RELOJ_VIRTUAL=0
  # /proc SINTETICO: el arranque del proceso (starttime) y el exe.  Sin esto
  # la comprobacion "el binario en servicio es el nuevo" depende de un
  # proceso REAL => no es igual entre runners (MEDIDO 2026-11-27: en el CI
  # el `sleep` del runner es el binario MULTICALL de coreutils y el
  # escenario fallaba; en local pasaba).  Con esto, DETERMINISTA.
  MD_STARTTIME=100
  MD_EXE=""
  # Por defecto el móvil está VACÍO (instalación en limpio): nada corriendo.
  # Los escenarios que necesitan "ya hay un daemon/supervisor/watchdog vivo"
  # lo piden con DAEMONS=1 / SUPERVISORS=1 / WATCHDOGS=1.
  DAEMONS=0; SUPERVISORS=0; WATCHDOGS=0; CLOUDFLARED=0; KEEPERS=0
  USUARIO_DISPONIBLE=0
}

# =============================================================================
# PREPARAR EL LABORATORIO
# =============================================================================
# Todo lo que hay aquí es el "mundo falso" del móvil:
#   $L   raíz del laboratorio (nueva en cada escenario)
#   $PRE prefijo del daemon        $ST  directorio de estado
#   $SB  fakes (curl, wget, cloudflared, el daemon)   $TBR caja de herramientas
#   $SVC service.d de Magisk        $HOME home falso    $TMPDIR temporal falso
#   $LOG registro de invocaciones de los fakes        $OUT salida del instalador
preparar_lab() {
  local real_grep real_sha real_cat
  real_grep="$(command -v grep || true)"
  real_sha="$(command -v sha256sum || true)"
  real_cat="$(command -v cat || true)"

  rm -rf "$PRE" "$SB" "$TBR" "$SVC" "$L/termux" "$L/proc" "$L/tmp" "$L/bundle" 2>/dev/null
  mkdir -p "$PRE" "$ST" "$SB" "$TBR" "$SVC" "$L/termux" "$L/proc" "$L/tmp" "$HOME" "$TMPDIR"
  : > "$LOG"
  : > "${MD_KILLS:-$L/kills.log}"

  # --- el daemon de mentira -------------------------------------------------
  cat > "$SB/megadaemon-core" <<EOF
#!/bin/sh
# Falso de megadaemon-core: implementa el CLI REAL que usa el instalador.
# El entorno SÓLO se registra en el lanzamiento del daemon (cockpit): si se
# registrase en toda llamada, el aserto "el daemon se lanzó con su entorno"
# no probaría nada (el instalador exporta esas variables para todo).
{
  echo "--- invocacion: \$*"
  if [ "\$1" = "cockpit" ]; then
    echo "    MEGADAEMON_STATE_DIR=\${MEGADAEMON_STATE_DIR:-<vacio>}"
    echo "    MEGADAEMON_PREFIX=\${MEGADAEMON_PREFIX:-<vacio>}"
    echo "    MEGADAEMON_BIN=\${MEGADAEMON_BIN:-<vacio>}"
  fi
  # MEDIDO (2026-11-28, movil): el daemon vive en /data/adb (0700 de root)
  # => si el instalador lo llama SIN root sale "Permission denied" y no se
  # publica nada.  El banco anota el uid de cada invocacion para poder
  # comprobar que la telemetria se publica COMO ROOT.
  echo "    uid=\$(id -u 2>/dev/null || echo '?')"
} >> "\${MD_LOG:-/dev/null}"
case "\$1" in
  telemetria)
    case "\$2" in
      sonda)
        case "\$3" in
          https://*)
            if [ "\${MD_HEALTH:-ok}" = "ok" ]; then
              echo '{"ok":true,"codigo":401}'; exit 0
            fi
            echo '{"ok":false,"codigo":0}'; exit 1 ;;
          *) echo '{"ok":false}'; exit 1 ;;
        esac ;;
      listo)
        if [ -f "\${MEGADAEMON_STATE_DIR:-/nonexistent}/tunnel.url" ]; then
          echo '{"listo":true}'; exit 0
        fi
        echo '{"listo":false}'; exit 1 ;;
      *) echo '{"ok":true}'; exit 0 ;;
    esac ;;
  channel)    echo '{"ok":true,"quick":"https://falso.trycloudflare.com"}'; exit 0 ;;
  diagnostico)
    # MEDIDO (2026-11-28): el fake devolvia solo "ok":true => las
    # comprobaciones del instalador que leen el JSON real (supervisores,
    # puertos, stack) salian SIEMPRE "sin datos" y el banco no las
    # ejercitaba.  Ahora devuelve la FORMA REAL (diagnostico.rs: recoger()).
    # ⚠️ Sin backticks y sin $( ): este heredoc NO va entre comillas => aqui
    # un backtick seria una sustitucion de comandos y el banco lo ejecutaria.
    _nsup=0
    if [ -f "\$LAB_PROC/99-megadaemon.sh.pids" ]; then
      while read -r _line; do
        [ -n "\$_line" ] && _nsup=\$((_nsup + 1))
      done < "\$LAB_PROC/99-megadaemon.sh.pids"
    fi
    echo "{\"ok\":true,\"supervisores\":\$_nsup,\"puertos\":{\"8080\":true,\"9090\":true,\"2235\":false},\"stack\":{\"total\":4,\"correctos\":4,\"desfasados\":0}}"
    exit 0 ;;
  update)     echo '{"ok":true,"estado":"actual"}'; exit 0 ;;
  probe)      exit 0 ;;
  hash)
    shift
    if [ -n "\${REAL_SHA:-}" ] && [ -x "\${REAL_SHA}" ]; then
      "\${REAL_SHA}" "\$1" 2>/dev/null
    fi
    exit 0 ;;
  cockpit)    echo "daemon simulado en \$2" >&2; exit 0 ;;
  doctor)     echo "diagnostico nativo simulado"; exit 0 ;;
  *)          exit 0 ;;
esac
EOF
  chmod 755 "$SB/megadaemon-core"
  REAL_SHA="$real_sha"; export REAL_SHA

  # --- el nucleo forense de mentira (mddoctor) ------------------------------
  cat > "$SB/mddoctor" <<'EOF'
#!/bin/sh
# Falso de mddoctor (el nucleo forense en C).
echo "--- mddoctor $*"
case "$1" in
  reconectar)
    # MEDIDO (2026-11-28, movil): si MD_TUNEL_LOG apunta al log del tunel,
    # el nucleo forense "reconecta" el tunel y su log trae la URL (la de la
    # API y la del tunel).  Asi se puede ejercitar el SEGUNDO sitio del
    # instalador que extrae la URL del log (la autorreparacion).
    if [ -n "${MD_TUNEL_LOG:-}" ]; then
      printf '%s\n' "${MD_TUNEL_LOG_CONTENIDO:-}" >> "$MD_TUNEL_LOG"
    fi
    echo "reconectado"; exit 0 ;;
  todo)       echo "veredicto: todo bien"; exit 0 ;;
  *)          echo "entorno: ok"; exit 0 ;;
esac
EOF
  chmod 755 "$SB/mddoctor"

  # --- el cloudflared de mentira: publica una URL en su log ------------------
  cat > "$SB/cloudflared" <<'EOF'
#!/bin/sh
# Falso de cloudflared: imprime la URL del quick tunnel en el log, como el real.
# Con MD_TUNEL=sin-url no imprime nada (el túnel no arranca).
[ "${MD_TUNEL:-ok}" = "sin-url" ] && exit 0
echo "2024-01-01T00:00:00Z INF |  https://falso-subdominio-abc.trycloudflare.com  |"
exit 0
EOF
  chmod 755 "$SB/cloudflared"
  # El cloudflared del "Termux" (la ruta que el instalador usa en el móvil:
  # /data/data/com.termux/files/usr/bin/cloudflared).
  cp "$SB/cloudflared" "$L/termux/cloudflared"
  chmod 755 "$L/termux/cloudflared"

  # --- curl de mentira: sirve el bundle local y sabe fallar por URL --------
  cat > "$SB/curl" <<EOF
#!/bin/sh
# Falso de curl.  Entiende: -o DEST, -w FORMATO, -I/-sI (HEAD) y las URLs
# del release de MegaDaemon.  El knob NET decide qué URLs fallan.
out=""; url=""; head=0; code=""; fail=0
while [ \$# -gt 0 ]; do
  case "\$1" in
    -o) out="\$2"; shift 2 ;;
    -w) code="\$2"; shift 2 ;;
    -I|-sI|--head) head=1; shift ;;
    -L|--retry|--retry-delay|--connect-timeout|--max-time|--max-filesize|-fsSL|-fsS|-s|-S|--spider) shift ;;
    -H|--header) shift 2 ;;
    http*) url="\$1"; shift ;;
    *) shift ;;
  esac
done
LAB="$L"; NET="\${MD_NET:-ok}"; BUNDLE="\${MD_BUNDLE:-ok}"; SHA="\${MD_SHA:-ok}"
# Se registra la URL pedida: sin esto no se puede comprobar "el instalador
# preguntó por la URL estable" (ni por el manifiesto, ni por la API).
[ -n "\${MD_LOG:-}" ] && echo "--- curl: \$url" >> "\$MD_LOG"
emitir() { [ -n "\$code" ] && printf '%s' "\$1"; }
fallar() { emitir "\${2:-000}"; exit "\${1:-7}"; }

case "\$url" in
  *127.0.0.1*|*localhost*)
    if [ "\${MD_HEALTH:-ok}" = "ko" ]; then fallar 22 000; fi
    [ -n "\$out" ] && : > "\$out"
    emitir 200; exit 0 ;;
  *latest.json*)
    if [ "\$NET" = "down" ]; then fallar 7 000; fi
    if [ "\$NET" = "sin-manifiesto" ] || [ "\$NET" = "solo-api" ]; then fallar 22 404; fi
    if [ "\$head" = "1" ]; then emitir 200; exit 0; fi
    printf '{"version":"abc1234","short":"abc123"}\n'; exit 0 ;;
  *megadaemon-movil.tar.gz.sha256*)
    if [ "\$NET" = "down" ]; then fallar 7 000; fi
    if [ "\$NET" = "sin-sha" ] || [ "\$SHA" = "ausente" ]; then fallar 22 404; fi
    if [ "\$head" = "1" ]; then emitir 200; exit 0; fi
    cp "\$LAB/bundle/mdo.tar.gz.sha256" "\$out" 2>/dev/null || fallar 7 000
    exit 0 ;;
  *megadaemon-movil.tar.gz*)
    if [ "\$NET" = "down" ]; then fallar 7 000; fi
    # con solo-api, la URL ESTABLE no existe (404) pero el asset que da la
    # API sí: por eso se distingue una de otra.
    case "\$url" in
      */releases/download/vabc1234/*) : ;;
      *) if [ "\$NET" = "404" ] || [ "\$NET" = "solo-api" ]; then fallar 22 404; fi ;;
    esac
    if [ "\$head" = "1" ]; then emitir 200; exit 0; fi
    # falla-bundle: el release EXISTE (el HEAD va bien) pero la descarga del
    # tarball falla => se prueban los reintentos de descarga.
    if [ "\$NET" = "falla-bundle" ]; then fallar 7 000; fi
    cp "\$LAB/bundle/mdo.tar.gz" "\$out" 2>/dev/null || fallar 7 000
    exit 0 ;;
  *channel-publish-arm64*)
    if [ "\$NET" = "down" ]; then fallar 7 000; fi
    if [ "\$head" = "1" ]; then emitir 200; exit 0; fi
    printf '#!/bin/sh\nexit 0\n' > "\$out"; exit 0 ;;
  *api.github.com*releases/latest*)
    if [ "\$NET" = "down" ]; then fallar 7 000; fi
    if [ "\$NET" = "solo-api" ] || [ "\$NET" = "sin-manifiesto" ]; then
      if [ "\$head" = "1" ]; then emitir 200; exit 0; fi
      printf '{"tag_name":"vabc1234","assets":[{"browser_download_url":"https://github.com/x/y/releases/download/vabc1234/megadaemon-movil.tar.gz"}]}\n'
      exit 0
    fi
    emitir 404; exit 0 ;;
  *releases/latest*)
    if [ "\$NET" = "down" ]; then fallar 7 000; fi
    if [ "\$NET" = "solo-api" ]; then emitir 404; exit 0; fi
    if [ "\$head" = "1" ]; then emitir 302; echo "location: https://github.com/x/y/releases/tag/vabc1234"; exit 0; fi
    echo "location: https://github.com/x/y/releases/tag/vabc1234"; exit 0 ;;
  *) emitir 404; exit 0 ;;
esac
exit 0
EOF
  chmod 755 "$SB/curl"

  # --- wget de mentira (para el escenario "sólo hay wget") ------------------
  cat > "$SB/wget" <<EOF
#!/bin/sh
# Falso de wget: -q, -T N, -O DEST, --spider, -S, --timeout=N.
out=""; url=""; spider=0
while [ \$# -gt 0 ]; do
  case "\$1" in
    -O) out="\$2"; shift 2 ;;
    -T|--timeout=*) shift 2 ;;
    -q|--quiet) shift ;;
    -S|--server-response) shift ;;
    --spider) spider=1; shift ;;
    http*) url="\$1"; shift ;;
    *) shift ;;
  esac
done
LAB="$L"; NET="\${MD_NET:-ok}"
[ -n "\${MD_LOG:-}" ] && echo "--- wget: \$url" >> "\$MD_LOG"
case "\$url" in
  *127.0.0.1*|*localhost*)
    if [ "\${MD_HEALTH:-ok}" = "ko" ]; then exit 4; fi
    [ -n "\$out" ] && [ "\$out" != "-" ] && : > "\$out"
    exit 0 ;;
  *latest.json*)
    if [ "\$NET" = "down" ] || [ "\$NET" = "solo-api" ]; then exit 8; fi
    [ "\$spider" = "1" ] && { echo "  HTTP/1.1 200 OK"; exit 0; }
    printf '{"version":"abc1234"}\n'; exit 0 ;;
  *megadaemon-movil.tar.gz.sha256*)
    if [ "\$NET" = "down" ] || [ "\$NET" = "sin-sha" ]; then exit 8; fi
    cp "\$LAB/bundle/mdo.tar.gz.sha256" "\$out" 2>/dev/null || exit 8
    exit 0 ;;
  *megadaemon-movil.tar.gz*)
    if [ "\$NET" = "down" ] || [ "\$NET" = "404" ] || [ "\$NET" = "solo-api" ]; then exit 8; fi
    cp "\$LAB/bundle/mdo.tar.gz" "\$out" 2>/dev/null || exit 8
    exit 0 ;;
  *channel-publish-arm64*)
    if [ "\$NET" = "down" ]; then exit 8; fi
    printf '#!/bin/sh\nexit 0\n' > "\$out"; exit 0 ;;
  *releases/latest*)
    if [ "\$NET" = "down" ]; then exit 8; fi
    if [ "\$spider" = "1" ]; then
      echo "  HTTP/1.1 302 Found"
      echo "  location: https://github.com/x/y/releases/tag/vabc1234"
      exit 0
    fi
    echo "location: https://github.com/x/y/releases/tag/vabc1234"; exit 0 ;;
  *) exit 8 ;;
esac
exit 0
EOF
  chmod 755 "$SB/wget"

  # --- el bundle de mentira -------------------------------------------------
  mkdir -p "$L/bundle/stage/bin" "$L/bundle/stage/deploy"
  cp "$SB/megadaemon-core" "$L/bundle/stage/bin/megadaemon-core"
  [ "$SIN_MDOCTOR" = "1" ] || cp "$SB/mddoctor" "$L/bundle/stage/bin/mddoctor"
  if [ "$BUNDLE" != "sin-stack" ]; then
    for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh tunnel-watchdog.sh 91-adb-tcp.sh; do
      cat > "$L/bundle/stage/deploy/$f" <<EOF
#!/bin/sh
{ echo "--- script: $f"; echo "    LOG_DIR=\${LOG_DIR:-<vacio>}"; echo "    DAEMON_BIN=\${DAEMON_BIN:-<vacio>}"; echo "    PUBLISH=\${PUBLISH:-<vacio>}"; echo "    CF=\${CF:-<vacio>}"; echo "    CFT_TARGET=\${CFT_TARGET:-<vacio>}"; echo "    MEGADAEMON_STATE_DIR=\${MEGADAEMON_STATE_DIR:-<vacio>}"; echo "    MEGADAEMON_ENV=\${MEGADAEMON_ENV:-<vacio>}"; } >> "\${MD_LOG:-/dev/null}"
exit 0
EOF
    done
  fi
  if [ "$BUNDLE" = "no-ejecutable" ]; then
    chmod 644 "$L/bundle/stage/bin/megadaemon-core"
  fi
  if [ "$BUNDLE" = "sin-binario" ]; then
    rm -f "$L/bundle/stage/bin/megadaemon-core"
  fi
  case "$BUNDLE" in
    corrupto) printf 'esto no es un tar.gz' > "$L/bundle/mdo.tar.gz" ;;
    vacio)    : > "$L/bundle/mdo.tar.gz" ;;
    *)        ( cd "$L/bundle/stage" && tar -czf "$L/bundle/mdo.tar.gz" bin deploy 2>/dev/null ) ;;
  esac
  ( cd "$L/bundle" && sha256sum mdo.tar.gz > mdo.tar.gz.sha256 ) 2>/dev/null
  case "$SHA" in
    distinto) printf '0000000000000000000000000000000000000000000000000000000000000000  dist/mdo.tar.gz\n' > "$L/bundle/mdo.tar.gz.sha256" ;;
    ausente)  rm -f "$L/bundle/mdo.tar.gz.sha256" ;;
    vacio)    : > "$L/bundle/mdo.tar.gz.sha256" ;;
    con-ruta) ( cd "$L/bundle" && sha256sum mdo.tar.gz | sed 's|mdo.tar.gz|dist/mdo.tar.gz|' > mdo.tar.gz.sha256 ) ;;
  esac

  # --- la caja de herramientas: PATH restringido (como el shell de Android) --
  mkdir -p "$TBR"
  local t
  # ⚠️ MEDIDO (2026-11-27): `df` y `kill` NO se enlazan aquí a propósito: los
  # fakes de abajo se escriben en $TBR y, si ahí hubiera un symlink al
  # binario del SISTEMA, el `cat >` lo pisaría (pasó: el `df` del sandbox
  # quedó sustituido por el falso y todo el sistema mentía).
  for t in sh dash bash uname date sed awk tr cut head tail wc mktemp mkdir rm cp mv \
           chmod ls cat sleep env touch basename dirname tar gzip find stat \
           readlink expr printf id whoami; do
    local p; p="$(command -v "$t" 2>/dev/null || true)"
    [ -n "$p" ] && [ -x "$p" ] && ln -sf "$p" "$TBR/$t" 2>/dev/null
  done
  [ "$SIN_SHA256SUM" = "1" ] || { [ -n "$real_sha" ] && ln -sf "$real_sha" "$TBR/sha256sum"; }
  [ "$DL" = "wget" ] || ln -sf "$SB/curl" "$TBR/curl"
  [ "$DL" = "curl" ] || ln -sf "$SB/wget" "$TBR/wget"
  [ "$SIN_CURL" = "1" ] && rm -f "$TBR/curl"
  if [ "$DL" = "ninguno" ]; then
    # Ni en la caja NI en el directorio de fakes: el instalador busca el
    # downloader con `command -v` sobre el PATH, así que tiene que faltar
    # de verdad (si no, el aviso de "no hay curl ni wget" nunca sale).
    rm -f "$TBR/curl" "$TBR/wget" "$SB/curl" "$SB/wget"
  fi

  # --- grep de mentira: las búsquedas en /proc devuelven la lista FICTICIA --
  rm -f "$TBR/grep"   # NUNCA escribir por encima de un symlink al binario real
  cat > "$TBR/grep" <<EOF
#!/bin/sh
# grep de mentira: si busca en /proc/*/cmdline devuelve los procesos
# FICTICIOS que fija el escenario; en cualquier otro caso, el grep de verdad.
REAL_GREP="$real_grep"
pat=""; proc=""
for a in "\$@"; do
  case "\$a" in
    /proc/*/cmdline) proc="\$a" ;;
    -*) ;;
    *) [ -z "\$pat" ] && pat="\$a" ;;
  esac
done
if [ -n "\$proc" ]; then
  key=\$(printf '%s' "\$pat" | tr -c 'A-Za-z0-9._-' '_')
  list="\$LAB_PROC/\$key.pids"
  [ -f "\$list" ] || exit 1
  while read -r p; do
    [ -n "\$p" ] && printf '/proc/%s/cmdline\n' "\$p"
  done < "\$list"
  exit 0
fi
exec "\$REAL_GREP" "\$@"
EOF
  chmod 755 "$TBR/grep"

  # --- pgrep/pkill/ps de mentira: los procesos del escenario ----------------
  rm -f "$TBR/pgrep"   # NUNCA escribir por encima de un symlink al binario real
  cat > "$TBR/pgrep" <<EOF
#!/bin/sh
# pgrep de mentira: imprime los PIDs ficticios del patrón pedido.
# El patrón se reduce a su PRIMERA PALABRA (como el -f de verdad, que busca
# una subcadena): "megadaemon-core cockpit" -> "megadaemon-core".
pat=""
while [ \$# -gt 0 ]; do
  case "\$1" in -*) shift ;; *) pat="\$1"; break ;; esac
done
key=\$(printf '%s' "\${pat%% *}" | tr -c 'A-Za-z0-9._-' '_')
list="\$LAB_PROC/\$key.pids"
[ -f "\$list" ] || exit 1
cat "\$list"
exit 0
EOF
  chmod 755 "$TBR/pgrep"
  rm -f "$TBR/pkill"   # NUNCA escribir por encima de un symlink al binario real
  cat > "$TBR/pkill" <<EOF
#!/bin/sh
# pkill de mentira: mata los PIDs ficticios del patrón (vacía su lista).
# Si existe el marcador pkill_falla, NO mata nada (así se simula un pkill
# que no entiende -f, como el de toybox).
pat=""
while [ \$# -gt 0 ]; do
  case "\$1" in -*) shift ;; *) pat="\$1"; break ;; esac
done
[ -f "\$LAB_PROC/pkill_falla" ] && exit 1
key=\$(printf '%s' "\${pat%% *}" | tr -c 'A-Za-z0-9._-' '_')
list="\$LAB_PROC/\$key.pids"
[ -f "\$list" ] || exit 1
: > "\$list.kill"
mv "\$list.kill" "\$list"
exit 0
EOF
  chmod 755 "$TBR/pkill"
  rm -f "$TBR/kill"   # NUNCA escribir por encima de un symlink al binario real
  cat > "$TBR/kill" <<EOF
#!/bin/sh
# kill de mentira: el PID "muere" (se quita de las listas de procesos).
pids=""
for a in "\$@"; do
  case "\$a" in -*) ;; *) pids="\$pids \$a" ;; esac
done
for p in \$pids; do
  # MEDIDO (2026-11-28): el ORDEN de las muertes es lo que arregla el
  # instalador (el inverso al de arranque) => el banco lo registra.
  [ -n "\${MD_KILLS:-}" ] && echo "kill \$p" >> "\$MD_KILLS"
  for f in "\$LAB_PROC"/*.pids; do
    [ -f "\$f" ] || continue
    # MEDIDO (2026-11-28): con "grep -v ... && mv", si el PID matado era la
    # ULTIMA linea, "grep -v" no imprime nada y sale con codigo 1 => el "&&"
    # cortocircuitaba y el fichero NO se movia => el PID "sobrevivia" y el
    # diagnostico contaba 2 daemons con 1 vivo.  Un "kill" de verdad quita el
    # proceso SIEMPRE => aqui se mueve el fichero siempre.
    # ⚠️ Sin backticks: este heredoc NO va entre comillas => un backtick
    # seria una sustitucion de comandos (y el banco lo ejecutaria).
    grep -v "^\$p\$" "\$f" > "\$f.tmp" 2>/dev/null
    mv -f "\$f.tmp" "\$f" 2>/dev/null
  done
done
exit 0
EOF
  chmod 755 "$TBR/kill"
  rm -f "$TBR/ps"   # NUNCA escribir por encima de un symlink al binario real
  cat > "$TBR/ps" <<EOF
#!/bin/sh
# ps de mentira: sólo sabe si hay un cloudflared vivo.
if [ -f "\$LAB_PROC/cloudflared.vivo" ]; then echo "cloudflared"; fi
exit 0
EOF
  chmod 755 "$TBR/ps"
  # setsid/nohup de mentira: ejecutan el comando y, además, ANOTAN el
  # proceso en el mundo falso (si no, el instalador lanzaría el daemon y su
  # propia comprobación "pgrep" no lo encontraría nunca).
  rm -f "$TBR/setsid"   # NUNCA escribir por encima de un symlink al binario real
  cat > "$TBR/setsid" <<'EOF'
#!/bin/sh
cmd="$*"
case "$cmd" in
  *megadaemon-core*) printf '429999\n' >> "$LAB_PROC/megadaemon-core.pids" ;;
esac
case "$cmd" in
  *tunnel-watchdog*) printf '429998\n' >> "$LAB_PROC/tunnel-watchdog.sh.pids" ;;
esac
case "$cmd" in
  *cloudflared*) : > "$LAB_PROC/cloudflared.vivo" ;;
esac
[ $# -gt 0 ] && shift
exec "$@"
EOF
  chmod 755 "$TBR/setsid"
  rm -f "$TBR/nohup"   # NUNCA escribir por encima de un symlink al binario real
  cat > "$TBR/nohup" <<'EOF'
#!/bin/sh
[ $# -gt 0 ] && shift
exec "$@"
EOF
  chmod 755 "$TBR/nohup"

  # --- df de mentira (el escenario "disco lleno") ----------------------------
  local KB
  KB=$(( ${ESPACIO_MB:-999999} * 1024 ))
  # `cat` de mentira: si lee /proc/<pid>/stat devuelve una linea SINTETICA
  # con el starttime que fija el escenario (campo 22 = la posicion 20 tras
  # el comm).  En cualquier otro caso, el cat de verdad.
  local real_cat; real_cat="$(command -v cat || true)"
  rm -f "$TBR/cat"
  cat > "$TBR/cat" <<EOF
#!/bin/sh
for a in "\$@"; do
  case "\$a" in
    /proc/*/stat)
      # El starttime es el campo 22 de /proc/<pid>/stat; tras quitar
      # "pid (comm) " es la posicion 20 => van 19 palabras delante.
      printf '9999 (megadaemon-core) S 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 %s 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n' "\${MD_STARTTIME:-100}"
      exit 0 ;;
    /proc/[0-9]*/cmdline)
      # MEDIDO (2026-11-28): el instalador lee el cmdline de cada PID para
      # saber CUAL es el canonico.  El argv lo fija el escenario (helper
      # "fijar_argv"); si el PID no lo tiene, no se puede leer => vacio.
      _p=\${a#/proc/}
      _p=\${_p%/cmdline}
      if [ -f "\$LAB_PROC/\$_p.cmdline" ]; then
        "$real_cat" "\$LAB_PROC/\$_p.cmdline"
        exit 0
      fi
      exit 1 ;;
  esac
done
exec "$real_cat" "\$@"
EOF
  chmod 755 "$TBR/cat"

  # `readlink` de mentira: /proc/<pid>/exe devuelve el exe que fija el
  # escenario (por defecto, el propio daemon; se puede poner "(deleted)").
  local real_readlink; real_readlink="$(command -v readlink || true)"
  rm -f "$TBR/readlink"
  cat > "$TBR/readlink" <<EOF
#!/bin/sh
for a in "\$@"; do
  case "\$a" in
    /proc/*/exe) printf '%s\n' "\${MD_EXE:-$PRE/megadaemon-core}"; exit 0 ;;
  esac
done
exec "$real_readlink" "\$@"
EOF
  chmod 755 "$TBR/readlink"

  # `mv` de mentira: REGISTRA lo que renombra (para comprobar que el binario
  # se instala con .new + mv y no sobrescribiendo el que está en ejecución).
  local real_mv; real_mv="$(command -v mv || true)"
  rm -f "$TBR/mv"
  cat > "$TBR/mv" <<EOF
#!/bin/sh
{ echo "--- mv: \$*"; } >> "\${MD_LOG:-/dev/null}"
exec "$real_mv" "\$@"
EOF
  chmod 755 "$TBR/mv"

  # RELOJ_VIRTUAL: `sleep` instantaneo + `date +%s` que avanza 6 s por llamada.
  if [ "$RELOJ_VIRTUAL" = "1" ]; then
    local real_date; real_date="$(command -v date || true)"
    rm -f "$TBR/sleep" "$TBR/date"
    cat > "$TBR/sleep" <<'EOF'
#!/bin/sh
{ echo "sleep $*"; } >> "${MD_LOG:-/dev/null}"
exit 0
EOF
    chmod 755 "$TBR/sleep"
    cat > "$TBR/date" <<EOF
#!/bin/sh
# date de mentira: +%s devuelve un reloj virtual que avanza 6 s por llamada.
case "\$1" in
  +%s)
    v=\$(cat "\$LAB_RELOJ" 2>/dev/null || echo 1700000000)
    printf '%s\n' "\$v"
    printf '%s\n' "\$(( v + 6 ))" > "\$LAB_RELOJ"
    exit 0 ;;
esac
exec "$real_date" "\$@"
EOF
    chmod 755 "$TBR/date"
    printf '1700000000\n' > "$LAB_RELOJ"
  fi

  rm -f "$TBR/df"   # NUNCA escribir por encima de un symlink al binario real
  cat > "$TBR/df" <<EOF
#!/bin/sh
# df de mentira: el knob ESPACIO_MB decide los MB libres.
printf 'Filesystem 1024-blocks Used Available Capacity Mounted on\n'
printf '/dev/fake $KB 1024 $KB 50%% /data/adb\n'
exit 0
EOF
  chmod 755 "$TBR/df"

  # --- el instalador con las rutas reescritas -------------------------------
  # Las rutas de Termux que sirven para DETECTAR el móvil
  # (/data/data/com.termux y .../files/usr) sólo se reescriben si el
  # escenario ha pedido simular el móvil (TERMUX=1): si no, el instalador
  # los vería en el host y TODOS los escenarios parecerían un móvil.
  if [ "$TERMUX" = "1" ]; then
    mkdir -p "$L/termuxusr" "$L/termuxdir"
    sed "s#/data/adb/megadaemon#$PRE#g; s#/data/adb/service.d#$SVC#g; s#/data/data/com.termux/files/usr/bin#$L/termux#g; s#/data/data/com.termux/files/usr#$L/termuxusr#g; s#/data/data/com.termux#$L/termuxdir#g" \
      "$RAIZ/deploy/megadaemon-oneliner.sh" > "$L/oneliner.sh"
  else
    sed "s#/data/adb/megadaemon#$PRE#g; s#/data/adb/service.d#$SVC#g; s#/data/data/com.termux/files/usr/bin#$L/termux#g" \
      "$RAIZ/deploy/megadaemon-oneliner.sh" > "$L/oneliner.sh"
  fi
  chmod 755 "$L/oneliner.sh"
  # TERMUX=1: el instalador debe detectar el MOVIL (para probar sus avisos:
  # "no hay curl ni wget en Termux", "necesito root para instalar en ...").
  if [ "$TERMUX" = "1" ]; then
    local real_uname; real_uname="$(command -v uname || true)"
    rm -f "$TBR/uname"
    cat > "$TBR/uname" <<EOF
#!/bin/sh
# uname de mentira: -o responde "Android" (detección de móvil).
case "\$1" in
  -o) echo "Android"; exit 0 ;;
esac
exec "$real_uname" "\$@"
EOF
    chmod 755 "$TBR/uname"
  fi

  # --- los procesos ficticios del escenario ---------------------------------
  fijar_pids megadaemon-core $(seq 424242 $((424242 + DAEMONS - 1)) 2>/dev/null)
  [ "$DAEMONS" -gt 0 ] || rm -f "$LAB_PROC/megadaemon-core.pids"
  fijar_pids 99-megadaemon.sh $(seq 424342 $((424342 + SUPERVISORS - 1)) 2>/dev/null)
  [ "$SUPERVISORS" -gt 0 ] || rm -f "$LAB_PROC/99-megadaemon.sh.pids"
  fijar_pids tunnel-watchdog.sh $(seq 424442 $((424442 + WATCHDOGS - 1)) 2>/dev/null)
  [ "$WATCHDOGS" -gt 0 ] || rm -f "$LAB_PROC/tunnel-watchdog.sh.pids"
  # MEDIDO (2026-11-28, movil): el keeper tambien multiplica (cada keeper
  # relanza un supervisor => 4 keepers = 4 supervisores) => el banco lo
  # simula tambien.
  fijar_pids 97-megadaemon-keeper.sh $(seq 424642 $((424642 + KEEPERS - 1)) 2>/dev/null)
  [ "$KEEPERS" -gt 0 ] || rm -f "$LAB_PROC/97-megadaemon-keeper.sh.pids"
  # Y el cloudflared: hasta ahora solo habia el marcador (`ps`), no una lista
  # de PIDs => la limpieza de duplicados del cloudflared no se podia
  # examinar en el banco.
  fijar_pids cloudflared $(seq 424542 $((424542 + CLOUDFLARED - 1)) 2>/dev/null)
  [ "$CLOUDFLARED" -gt 0 ] || rm -f "$LAB_PROC/cloudflared.pids"
  [ "$CLOUDFLARED" -gt 0 ] && { touch "$LAB_PROC/cloudflared.vivo"; } || rm -f "$LAB_PROC/cloudflared.vivo"

  # --- el túnel: el log con la URL (salvo que el escenario diga que no) -----
  rm -f "$ST/tunnel.url"
  if [ "$TUNEL" != "sin-url" ]; then
    mkdir -p "$ST"
    if [ "$TUNEL" = "ok" ]; then
      printf '2024-01-01T00:00:00Z INF |  https://falso-subdominio-abc.trycloudflare.com  |\n' > "$ST/tunnel.log"
    else
      printf '2024-01-01T00:00:00Z INF |  arrancando…\n' > "$ST/tunnel.log"
    fi
  fi

  # --- el prefijo de root con 0700 (el bug del móvil) ----------------------
  if [ "$PREFIX_0700" = "1" ]; then
    chown -R root:root "$PRE" 2>/dev/null || true
    chmod 700 "$PRE" "$ST" 2>/dev/null || true
  fi

  # --- el `su` de mentira: escalada REAL (setuid) y BAJADA de privilegios ---
  # MEDIDO (2026-11-27): la v1 del banco usaba un `su` que SIEMPRE hacía
  # setuid(0) => el instalador acababa corriendo como root y el escenario
  # "usuario normal con prefijo 0700" no probaba nada.  Este hace las DOS
  # cosas: `su -c CMD` sube a root (como el Magisk) y `su <user> -c CMD`
  # baja a ese usuario (como un `su` de verdad).
  USUARIO_DISPONIBLE=0
  if [ "$(id -u)" = "0" ] && command -v cc >/dev/null 2>&1; then
    cat > "$L/su.c" <<'EOF'
/* `su` de mentira con escalada real y bajada de privilegios. */
#include <unistd.h>
#include <stdio.h>
#include <pwd.h>
#include <string.h>
int main(int argc, char **argv) {
    int i = 1;
    uid_t target = 0;                 /* por defecto: root (escalada) */
    if (argc > 1 && argv[1][0] != '-') {
        struct passwd *pw = getpwnam(argv[1]);
        if (!pw) { fprintf(stderr, "su: usuario desconocido\n"); return 1; }
        target = pw->pw_uid; i = 2;
    }
    if (setgid(target) != 0) { fprintf(stderr, "su: sin privilegios\n"); return 1; }
    if (setuid(target) != 0) { fprintf(stderr, "su: sin privilegios\n"); return 1; }
    /* -s /bin/sh -c CMD  |  -c CMD */
    char *cmd = NULL;
    for (; i < argc; i++) {
        if (strcmp(argv[i], "-c") == 0 && i + 1 < argc) { cmd = argv[i + 1]; break; }
    }
    if (cmd) { execl("/bin/sh", "sh", "-c", cmd, (char *)NULL); return 127; }
    execvp(argv[i], argv + i);
    return 127;
}
EOF
    if cc -O2 -o "$L/su" "$L/su.c" 2>/dev/null; then
      chown root:root "$L/su" 2>/dev/null || true
      chmod 4755 "$L/su" 2>/dev/null || true
      ln -sf "$L/su" "$TBR/su" 2>/dev/null
      if id -u mdlab >/dev/null 2>&1; then
        USUARIO_DISPONIBLE=1
      elif command -v useradd >/dev/null 2>&1; then
        useradd -m mdlab >/dev/null 2>&1 && USUARIO_DISPONIBLE=1
      fi
    fi
  fi
}

# --- definir los procesos ficticios -----------------------------------------
fijar_pids() {
  local nombre="$1"; shift
  local p; : > "$LAB_PROC/$nombre.pids"
  for p in "$@"; do
    [ -n "$p" ] && printf '%s\n' "$p" >> "$LAB_PROC/$nombre.pids"
  done
}
sin_pids() { rm -f "$LAB_PROC/$1.pids"; }
# El argv de un proceso ficticio (NUL-separado, como /proc/<pid>/cmdline).
# El "cat" de mentira lo sirve cuando el instalador lo lee.
fijar_argv() {
  local pid="$1"; shift
  : > "$LAB_PROC/$pid.cmdline"
  local a
  for a in "$@"; do printf '%s\000' "$a" >> "$LAB_PROC/$pid.cmdline"; done
}

# =============================================================================
# EJECUTAR EL INSTALADOR
# =============================================================================
# Corre el instalador con el PATH restringido (caja de herramientas + fakes).
# Deja la salida en $OUT y el código en $CODIGO.
correr_instalador() {
  # El presupuesto de espera por defecto se recorta a 30 s: el instalador
  # espera hasta ese tiempo a que aparezca el túnel y a que responda.  Con
  # 120 s (el valor por defecto del script) un escenario "el túnel no
  # responde" tardaría 2 minutos y el banco entero sería inviable.
  local extra=""
  for a in "$@"; do
    case "$a" in --timeout=*) extra="" ;; esac
  done
  case " $* " in *" --timeout="*) ;; *) extra="--timeout=30" ;; esac
  MD_NET="$NET" MD_BUNDLE="$BUNDLE" MD_SHA="$SHA" MD_HEALTH="$HEALTH" MD_TUNEL="$TUNEL" \
  PATH="$SB:$TBR" HOME="$HOME" TMPDIR="$TMPDIR" MD_LOG="$LOG" \
    sh "$L/oneliner.sh" "$@" $extra > "$OUT" 2>&1
  CODIGO=$?
  return $CODIGO
}

# Igual, pero como un usuario NORMAL (para el prefijo 0700 de root).
# Usa el `su` de mentira con BAJADA de privilegios (setuid al uid de mdlab):
# el instalador ve un /data/adb 0700 de root y tiene que escalar él mismo.
correr_instalador_como_usuario() {
  if [ "$USUARIO_DISPONIBLE" != "1" ] || [ ! -x "$TBR/su" ]; then
    CODIGO=127
    return 127
  fi
  chown -R root:root "$PRE" 2>/dev/null || true
  chmod 700 "$PRE" "$ST" 2>/dev/null || true
  chown -R mdlab "$HOME" "$TMPDIR" "$L" 2>/dev/null || true
  # El `su` de mentira TIENE que seguir siendo de root con el bit setuid:
  # un `chown -R` se lo quitaría y la escalada dejaría de funcionar.
  chown root:root "$L/su" 2>/dev/null || true
  chmod 4755 "$L/su" 2>/dev/null || true
  chmod 755 "$L" "$HOME" "$TMPDIR" 2>/dev/null || true
  "$TBR/su" mdlab -s /bin/sh -c "cd '$L' && HOME='$HOME' TMPDIR='$TMPDIR' MD_LOG='$LOG' PATH='$SB:$TBR' MD_NET='$NET' MD_BUNDLE='$BUNDLE' MD_SHA='$SHA' MD_HEALTH='$HEALTH' sh '$L/oneliner.sh' $*" \
    > "$OUT" 2>&1
  CODIGO=$?
  return $CODIGO
}

# Ejecuta el instalador como un usuario NORMAL y SIN ninguna vía de escalada
# (ni `su` ni `sudo` en el PATH): es lo que ve un usuario de Termux al que
# Magisk no le ha concedido root.  El instalador DEBE morir explicando cómo
# conseguir root, en vez de seguir y dejar el móvil a medias.
correr_instalador_sin_escalada() {
  if [ "$USUARIO_DISPONIBLE" != "1" ] || [ ! -x "$TBR/su" ]; then
    CODIGO=127
    return 127
  fi
  # Una caja SIN `su`/`sudo`: no hay forma de escalar.
  rm -rf "$L/tools-noroot"; mkdir -p "$L/tools-noroot"
  local f
  for f in "$TBR"/*; do
    case "$(basename "$f")" in su|sudo) continue ;; esac
    ln -sf "$f" "$L/tools-noroot/$(basename "$f")" 2>/dev/null
  done
  chown -R root:root "$PRE" 2>/dev/null || true
  chmod 700 "$PRE" "$ST" 2>/dev/null || true
  chown -R mdlab "$HOME" "$TMPDIR" 2>/dev/null || true
  chown root:root "$L/su" 2>/dev/null || true
  chmod 4755 "$L/su" 2>/dev/null || true
  chmod 755 "$L" "$HOME" "$TMPDIR" "$L/tools-noroot" 2>/dev/null || true
  "$TBR/su" mdlab -s /bin/sh -c "cd '$L' && HOME='$HOME' TMPDIR='$TMPDIR' MD_LOG='$LOG' PATH='$SB:$L/tools-noroot' MD_NET='$NET' MD_BUNDLE='$BUNDLE' MD_SHA='$SHA' MD_HEALTH='$HEALTH' MD_TUNEL='$TUNEL' sh '$L/oneliner.sh' $*" \
    > "$OUT" 2>&1
  CODIGO=$?
  return $CODIGO
}

# =============================================================================
# ASSERTIONS
# =============================================================================
comprobar()      { local d="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$d"; else mal "$d"; fi; }
verificar()      { local d="$1"; shift; if "$@" >/dev/null 2>&1; then mal "$d"; else ok "$d"; fi; }
contiene()       { local d="$1" f="$2" r="$3"; if grep -qE "$r" "$f" 2>/dev/null; then ok "$d"; else mal "$d (no se encontró /$r/ en $(basename "$f"))"; fi; }
no_contiene()    { local d="$1" f="$2" r="$3"; if grep -qE "$r" "$f" 2>/dev/null; then mal "$d (apareció /$r/ en $(basename "$f"))"; else ok "$d"; fi; }
log_mensaje()    { local d="$1" r="$2"; if grep -qE "$r" "$LOG" 2>/dev/null; then ok "$d"; else mal "$d (no se encontró /$r/ en el registro de los fakes)"; fi; }
no_log_mensaje() { local d="$1" r="$2"; if grep -qE "$r" "$LOG" 2>/dev/null; then mal "$d (apareció /$r/)"; else ok "$d"; fi; }
salida_mensaje() { local d="$1" r="$2"; if grep -qE "$r" "$OUT" 2>/dev/null; then ok "$d"; else mal "$d (no se encontró /$r/ en la salida)"; fi; }
no_salida_mensaje() { local d="$1" r="$2"; if grep -qE "$r" "$OUT" 2>/dev/null; then mal "$d (apareció /$r/ en la salida)"; else ok "$d"; fi; }
# alias tolerante (los dos nombres se usan en los escenarios)
salida_no_mensaje() { no_salida_mensaje "$@"; }
existe()         { [ -e "$1" ]; }
ejecutable()     { [ -x "$1" ]; }

# Espera a que un fichero contenga un patrón (hasta N segundos).
espera_linea() {
  local r="$1" f="$2" s="${3:-5}"
  local i=0
  while [ "$i" -lt $((s * 4)) ]; do
    grep -qE "$r" "$f" 2>/dev/null && return 0
    sleep 0.25
    i=$((i + 1))
  done
  return 1
}

# Guarda la salida del último instalador con un nombre (para escenarios con
# varias fases: cada fase guarda la suya).
guardar_salida() { cp "$OUT" "$L/$1.salida" 2>/dev/null; }

# Comprueba que un JSON es válido (python3).
json_valido() {
  local d="$1" f="$2"
  if python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$f" 2>/dev/null; then ok "$d"; else mal "$d (JSON inválido: $f)"; fi
}

# --- escribir un falso en la caja de herramientas ----------------------------
# MEDIDO (2026-11-27): escribir con `cat > "$TBR/uname"` seguía el SYMLINK al
# binario real del sistema y lo pisaba (el `uname` del sistema pasó a decir
# "Android" y el instalador dejó de detectar PCs).  Este helper borra el
# symlink ANTES de escribir: una sola forma de hacerlo, para todo el banco.
falso_en_caja() {
  local nombre="$1"; shift
  rm -f "$TBR/$nombre"
  cat > "$TBR/$nombre"
  chmod 755 "$TBR/$nombre"
}

# =============================================================================
# LOS ESCENARIOS
# =============================================================================
FUENTES_ALL=("$RAIZ"/tools/test-oneliner-casos*.sh)
if [ "${#FICHEROS[@]}" -gt 0 ]; then
  FUENTES=("${FICHEROS[@]}")
else
  FUENTES=()
  for f in "${FUENTES_ALL[@]}"; do
    [ -f "$f" ] && FUENTES+=("$f")
  done
fi

CASOS=()
for f in "${FUENTES[@]}"; do
  # shellcheck disable=SC1090
  . "$f"
done
for n in $(declare -F | awk '{print $3}' | grep '^caso_'); do
  case "$n" in
    *[!a-zA-Z0-9_]*) continue ;;
  esac
  [ -n "$SOLO" ] && { case "$n" in $SOLO) ;; *) continue ;; esac; }
  CASOS+=("$n")
done

if [ "$LISTAR" = "1" ]; then
  printf '%s\n' "${CASOS[@]}"
  exit 0
fi

if [ "${#CASOS[@]}" -eq 0 ]; then
  printf 'no hay escenarios que ejecutar\n' >&2
  exit 1
fi

# --- ejecutarlos ------------------------------------------------------------
mkdir -p "$BASE"
say "banco del instalador: ${#CASOS[@]} escenarios, $PAR en paralelo"

ejecutar_uno() {
  local nombre="$1"
  local id="$nombre"
  local dir="$BASE/$id"
  rm -rf "$dir" 2>/dev/null
  mkdir -p "$dir"
  (
    L="$dir"
    PRE="$dir/md"
    ST="$dir/md/state"
    SB="$dir/bin"
    TBR="$dir/tools"
    SVC="$dir/svc"
    LOG="$dir/invocaciones.log"
    OUT="$dir/salida.log"
    LAB_PROC="$dir/proc"
    LAB_RELOJ="$dir/reloj"
    HOME="$dir/home"
    TMPDIR="$dir/tmp"
    MD_LOG="$dir/invocaciones.log"
    MD_KILLS="$dir/kills.log"
    MD_STARTTIME=100
    MD_EXE=""
    export L PRE ST SB TBR SVC LOG OUT LAB_PROC HOME TMPDIR MD_LOG LAB_RELOJ
    export MD_STARTTIME MD_EXE MD_KILLS
    mkdir -p "$HOME" "$TMPDIR" "$LAB_PROC"
    : > "$LOG"
    : > "$dir/fallos.txt"
    : > "$dir/omites.txt"
    cd "$dir" || exit 1
    knobs_reset
    preparar_lab
    "$nombre"
  ) > "$BASE/$id.salida" 2>&1
}

PIDS=""
LANZADOS=0
for c in "${CASOS[@]}"; do
  ejecutar_uno "$c" &
  PIDS="$PIDS $!"
  LANZADOS=$((LANZADOS + 1))
  while [ "$(jobs -rp | wc -l)" -ge "$PAR" ]; do
    wait -n 2>/dev/null || true
  done
done
for p in $PIDS; do wait "$p" 2>/dev/null || true; done

# --- recoger ----------------------------------------------------------------
FALLOS=0
OMIS=0
for c in "${CASOS[@]}"; do
  dir="$BASE/$c"
  printf '\n=== %s ===\n' "$c"
  if [ -f "$BASE/$c.salida" ]; then
    sed 's/^/  /' "$BASE/$c.salida" | grep -v '^  $' || true
  else
    printf '  (sin salida: el escenario no llegó a ejecutarse)\n'
    FALLOS=$((FALLOS + 1))
  fi
  if [ -f "$dir/fallos.txt" ]; then
    n=$(grep -c . "$dir/fallos.txt" 2>/dev/null || true)
    case "$n" in ''|*[!0-9]*) n=0 ;; esac
    FALLOS=$((FALLOS + n))
  fi
  if [ -f "$dir/omites.txt" ]; then
    m=$(grep -c . "$dir/omites.txt" 2>/dev/null || true)
    case "$m" in ''|*[!0-9]*) m=0 ;; esac
    OMIS=$((OMIS + m))
  fi
done

printf '\n===== BANCO DEL INSTALADOR: %s escenario(s), %s fallo(s), %s omitido(s) =====\n' \
  "${#CASOS[@]}" "$FALLOS" "$OMIS"
exit "$FALLOS"

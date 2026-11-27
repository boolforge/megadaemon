#!/usr/bin/env bash
# =============================================================================
# test-oneliner.sh — simula el instalador del móvil en un entorno falso.
#
# Por qué: el instalador real no se puede probar aquí (el móvil está
# inalcanzable y su shell es el de Android).  Este script reescribe las
# rutas absolutas (`/data/adb/megadaemon` -> un directorio temporal) y
# sustituye el binario real y `curl` por unos de mentira que implementan la
# MISMA interfaz, para comprobar la LÓGICA del instalador:
#
#   * extrae el bundle y encuentra el binario;
#   * el binario se instala con .new + mv (atómico) y queda ejecutable;
#   * el daemon se lanza CON su entorno (MEGADAEMON_STATE_DIR…);
#   * el watchdog se lanza CON su entorno (LOG_DIR, DAEMON_BIN…);
#   * el watchdog NO se instala en service.d;
#   * el health-check usa la URL del túnel y la sonda;
#   * un bundle corrupto ABORTA (código de salida != 0) y NO toca el daemon;
#   * un sha256 que no cuadra ABORTA.
#
# Uso: bash tools/test-oneliner.sh
# =============================================================================
set -u

# El instalador EXIGE root (instala en /data/adb), asi que el banco tiene que
# correr como root.  En el runner del CI el usuario NO es root pero tiene
# sudo sin contrasena: se reejecuta el banco como root una sola vez.
# MEDIDO (2026-11-27): sin esto el job `sintaxis` del CI salia rojo entero
# (23 fallos) porque el instalador moria con "necesito root".
if [ "$(id -u)" != "0" ] && command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
  exec sudo -n -E bash "$0" "$@"
fi

RAIZ="$(cd "$(dirname "$0")/.." && pwd)"
LAB="${LAB:-/tmp/md-lab}"
PREFIX_LAB="$LAB/md"
STATE_LAB="$PREFIX_LAB/state"

say() { printf '\n== %s ==\n' "$*"; }
ok()  { printf '  OK   %s\n' "$*"; }
mal() { printf '  FALLO %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
FALLOS=0

rm -rf "$LAB"
mkdir -p "$PREFIX_LAB/state" "$LAB/bin"

# --- el binario de mentira: implementa el CLI real --------------------------
cat > "$LAB/fake-core" <<'EOF'
#!/bin/sh
# Fake de megadaemon-core.  Graba en $MD_LOG lo que se le invoca, con su
# entorno, para poder ASSERT lo que el instalador hace.
{
  echo "--- invocacion: $*"
  echo "    MEGADAEMON_STATE_DIR=${MEGADAEMON_STATE_DIR:-<vacío>}"
  echo "    MEGADAEMON_PREFIX=${MEGADAEMON_PREFIX:-<vacío>}"
  echo "    LOG_DIR=${LOG_DIR:-<vacío>}"
  echo "    DAEMON_BIN=${DAEMON_BIN:-<vacío>}"
} >> "${MD_LOG:-/dev/null}"

case "$1 $2" in
  "telemetria sonda")
    # Sólo responde si la URL es la buena.
    case "$3" in
      *trycloudflare.com*) echo '{"ok":true,"codigo":401}'; exit 0 ;;
      *) echo '{"ok":false,"codigo":0}'; exit 1 ;;
    esac ;;
  "telemetria listo")
    if [ -f "${MEGADAEMON_STATE_DIR:-/nonexistent}/tunnel.url" ]; then
      echo '{"listo":true}'; exit 0
    fi
    echo '{"listo":false}'; exit 1 ;;
  "telemetria publicar")   echo '{"ok":true}'; exit 0 ;;
  "telemetria conexion")   echo '{"ok":true}'; exit 0 ;;
  "channel publish")       echo '{"ok":true}'; exit 0 ;;
  "update --check")        echo '{"ok":true,"estado":"actual"}'; exit 0 ;;
  "hash ")                 shift; sha256sum "$1" 2>/dev/null ;;
esac
exit 0
EOF
chmod 755 "$LAB/fake-core"

# --- el bundle de mentira ---------------------------------------------------
mkdir -p "$LAB/stage/bin" "$LAB/stage/deploy"
cp "$LAB/fake-core" "$LAB/stage/bin/megadaemon-core"
# Los scripts del stack son fakes que REGISTRAN su entorno: así se puede
# comprobar que el watchdog se lanza con LOG_DIR/DAEMON_BIN/… y no a ciegas.
for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh tunnel-watchdog.sh 91-adb-tcp.sh; do
  cat > "$LAB/stage/deploy/$f" <<EOF
#!/bin/sh
{ echo "--- script: $f"; echo "    LOG_DIR=\${LOG_DIR:-<vacío>}"; echo "    DAEMON_BIN=\${DAEMON_BIN:-<vacío>}"; echo "    PUBLISH=\${PUBLISH:-<vacío>}"; echo "    MEGADAEMON_STATE_DIR=\${MEGADAEMON_STATE_DIR:-<vacío>}"; } >> "\${MD_LOG:-/dev/null}"
exit 0
EOF
done
( cd "$LAB/stage" && tar -czf "$LAB/mdo.tar.gz" bin deploy )
( cd "$LAB" && sha256sum mdo.tar.gz | sed 's|mdo.tar.gz|dist/mdo.tar.gz|' > mdo.tar.gz.sha256 )

# --- curl de mentira: sirve el bundle local --------------------------------
cat > "$LAB/curl" <<'EOF'
#!/bin/sh
# Fake de curl: entiende -o, -I/-sI (HEAD), -w '%{http_code}' y las URLs del
# release de MegaDaemon.
out=""; url=""; head=0; code=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    -w) code="$2"; shift 2 ;;
    -I|-sI|-sI*|--head) head=1; shift ;;
    -L|--retry|--retry-delay|--connect-timeout|--max-time|--max-filesize|-fsSL|-s|-S|--spider|-fsS)
        shift ;;
    -H|--header) shift 2 ;;
    http*) url="$1"; shift ;;
    *) shift ;;
  esac
done
LAB="${MD_LAB:?}"

# El código HTTP que se imprime con -w (vacío si no se pidió).
emitir_codigo() {
  [ -n "$code" ] && printf '%s' "$1"
}

case "$url" in
  *latest.json*)
      # ⚠️ Esta URL TAMBIÉN contiene "releases/latest": el caso va ANTES.
      if [ "$head" = "1" ]; then emitir_codigo 200; exit 0; fi
      printf '{"version":"abc1234","short":"abc123"}\n' ;;
  *megadaemon-movil.tar.gz.sha256*)
      if [ "$head" = "1" ]; then emitir_codigo 200; exit 0; fi
      cp "$LAB/mdo.tar.gz.sha256" "$out" ;;
  *megadaemon-movil.tar.gz*)
      if [ "$head" = "1" ]; then emitir_codigo 200; exit 0; fi
      cp "$LAB/mdo.tar.gz" "$out" ;;
  *channel-publish-arm64*)
      if [ "$head" = "1" ]; then emitir_codigo 200; exit 0; fi
      printf '#!/bin/sh\nexit 0\n' > "$out" ;;
  *releases/latest*)
      # La redirección con el tag (HEAD: se imprime la cabecera location).
      if [ "$head" = "1" ]; then
        emitir_codigo 302
        echo "location: https://github.com/x/y/releases/tag/vabc1234"
      else
        echo "location: https://github.com/x/y/releases/tag/vabc1234"
      fi ;;
  *) emitir_codigo 404 ;;
esac
exit 0
EOF
chmod 755 "$LAB/curl"

# --- el instalador con las rutas reescritas --------------------------------
sed "s#/data/adb/megadaemon#$PREFIX_LAB#g; s#/data/adb/service.d#$LAB/service.d#g; s#/data/data/com.termux/files/usr/bin#$LAB/termux#g" \
  "$RAIZ/deploy/megadaemon-oneliner.sh" > "$LAB/oneliner.sh"
chmod 755 "$LAB/oneliner.sh"
mkdir -p "$LAB/service.d" "$LAB/termux"
printf '#!/bin/sh\nexit 0\n' > "$LAB/termux/cloudflared"

# El cloudflared de mentira: al arrancar, publica una URL como el real (que
# la imprime en el log).  Así el instalador puede extraerla.
cat > "$LAB/termux/cloudflared" <<'EOF'
#!/bin/sh
# Fake de cloudflared: imprime la URL del quick tunnel en el log, como el real.
# El instalador la lee de `$LOGDIR/tunnel.log`; aquí se deriva del $2 del
# redireccionamiento (el >> lo pone el shell que lo lanza).
echo "2024-01-01T00:00:00Z INF |  https://falso-subdominio-abc.trycloudflare.com  |"
sleep 300
EOF
chmod 755 "$LAB/termux/cloudflared"

# --- ejecución --------------------------------------------------------------
say "1) instalación limpia"
export MD_LAB="$LAB"
export MD_LOG="$LAB/invocaciones.log"
: > "$MD_LOG"
PATH="$LAB:$PATH" sh "$LAB/oneliner.sh" --instalar --sin-telemetria > "$LAB/salida.log" 2>&1
RC=$?
[ "$RC" = "0" ] && ok "salida 0" || mal "salida $RC (se esperaba 0)"
[ -x "$PREFIX_LAB/megadaemon-core" ] && ok "binario instalado y ejecutable" || mal "binario no instalado"
[ -f "$STATE_LAB/tunnel.url" ] && ok "tunnel.url escrito" || mal "tunnel.url NO escrito"
grep -q 'trycloudflare.com' "$STATE_LAB/tunnel.url" 2>/dev/null && ok "tunnel.url con la URL del túnel" || mal "tunnel.url sin URL"

say "2) el daemon se lanzó CON su entorno"
grep -q 'MEGADAEMON_STATE_DIR=' "$MD_LOG" && ok "el daemon recibe MEGADAEMON_STATE_DIR" || mal "el daemon NO recibe el estado"
grep 'MEGADAEMON_STATE_DIR=' "$MD_LOG" | grep -q "$STATE_LAB" && ok "el estado apunta al prefijo del laboratorio" || mal "el estado apunta a otro sitio"

say "3) el watchdog se lanzó CON su entorno y NO está en service.d"
# El watchdog se identifica por su línea "--- script: tunnel-watchdog.sh".
grep -q -- '--- script: tunnel-watchdog.sh' "$MD_LOG" && ok "el watchdog se lanzó" || mal "el watchdog no se lanzó"
WATCH=$(grep -A 5 -- '--- script: tunnel-watchdog.sh' "$MD_LOG" | head -6)
printf '%s' "$WATCH" | grep -q 'LOG_DIR=' && ok "el watchdog recibe LOG_DIR" || mal "el watchdog NO recibe LOG_DIR"
printf '%s' "$WATCH" | grep -q "LOG_DIR=$PREFIX_LAB" && ok "LOG_DIR apunta al prefijo" || mal "LOG_DIR apunta a otro sitio"
printf '%s' "$WATCH" | grep -q 'DAEMON_BIN=' && ok "el watchdog recibe DAEMON_BIN" || mal "el watchdog NO recibe DAEMON_BIN"
printf '%s' "$WATCH" | grep -q "DAEMON_BIN=$PREFIX_LAB" && ok "DAEMON_BIN apunta al prefijo" || mal "DAEMON_BIN apunta a otro sitio"
printf '%s' "$WATCH" | grep -q "MEGADAEMON_STATE_DIR=$STATE_LAB" && ok "el watchdog recibe el state dir" || mal "el watchdog NO recibe el state dir"
if [ -f "$LAB/service.d/tunnel-watchdog.sh" ]; then
  mal "tunnel-watchdog.sh está en service.d (Magisk lo ejecutaría SIN entorno)"
else
  ok "tunnel-watchdog.sh NO está en service.d"
fi
[ -f "$PREFIX_LAB/tunnel-watchdog.sh" ] && ok "tunnel-watchdog.sh está en el prefijo" || mal "tunnel-watchdog.sh no está en el prefijo"

# MEDIDO (2026-11-27, movil): el stack NUNCA se copiaba a service.d (el
# chequeo del bundle se hacia como usuario normal sobre /data/adb, 0700) =>
# el movil quedaba sin supervisor ni keeper => no arrancaba nadie.
for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
  if [ -f "$LAB/service.d/$f" ]; then
    ok "$f está en service.d (arranque en boot)"
  else
    mal "$f NO está en service.d: el móvil no arrancará nada"
  fi
done

say "4) el health-check usó la sonda con la URL"
grep -q 'telemetria sonda' "$MD_LOG" && ok "se sondó el túnel" || mal "no se sondeó el túnel"
grep -q 'LISTO: el móvil es alcanzable' "$LAB/salida.log" && ok "anunció que está listo" || mal "no anunció que está listo"

say "5) idempotencia: la 2ª ejecución NO toca nada"
URL1=$(cat "$STATE_LAB/tunnel.url" 2>/dev/null)
PATH="$LAB:$PATH" sh "$LAB/oneliner.sh" --instalar --sin-telemetria > "$LAB/salida2.log" 2>&1
RC2=$?
URL2=$(cat "$STATE_LAB/tunnel.url" 2>/dev/null)
[ "$RC2" = "0" ] && ok "salida 0" || mal "salida $RC2"
grep -q 'no toco nada' "$LAB/salida2.log" && ok "detectó que ya estaba sano" || mal "no detectó el atajo de idempotencia"
[ "$URL1" = "$URL2" ] && ok "la URL del túnel NO cambió" || mal "la URL cambió ($URL1 -> $URL2)"

say "6) un bundle CORRUPTO aborta y no rompe el móvil"
cp "$LAB/mdo.tar.gz" "$LAB/mdo.tar.gz.bak"
printf 'esto no es un tar.gz' > "$LAB/mdo.tar.gz"
RC3=$(PATH="$LAB:$PATH" sh "$LAB/oneliner.sh" --instalar --sin-telemetria --force > "$LAB/salida3.log" 2>&1; echo $?)
[ "$RC3" != "0" ] && ok "aborta con código $RC3" || mal "NO abortó (código 0)"
[ -x "$PREFIX_LAB/megadaemon-core" ] && ok "el binario vivo sigue ahí" || mal "el binario vivo desapareció"
cp "$LAB/mdo.tar.gz.bak" "$LAB/mdo.tar.gz"

say "7) un sha256 que NO cuadra aborta"
printf '0000000000000000000000000000000000000000000000000000000000000000  dist/mdo.tar.gz\n' > "$LAB/mdo.tar.gz.sha256"
RC4=$(PATH="$LAB:$PATH" sh "$LAB/oneliner.sh" --instalar --sin-telemetria --force > "$LAB/salida4.log" 2>&1; echo $?)
[ "$RC4" != "0" ] && ok "aborta con código $RC4" || mal "NO abortó con sha incorrecto"
grep -q 'NO cuadra' "$LAB/salida4.log" && ok "lo dice claramente" || mal "no explica el fallo"
( cd "$LAB" && sha256sum mdo.tar.gz | sed 's|mdo.tar.gz|dist/mdo.tar.gz|' > mdo.tar.gz.sha256 )

say "8) el modo PC se detecta"
grep -q 'instalar_pc()' "$LAB/oneliner.sh" && ok "el modo PC existe" || mal "el modo PC no existe"

# ---------------------------------------------------------------------------
# 9) EL BUG DEL PREFIJO DE ROOT (MEDIDO 2026-11-27 en el movil del usuario).
#
# En el movil /data/adb es 0700 (solo root) y el one-liner lo ejecuta un
# usuario normal de Termux.  El bundle SI traia bin/megadaemon-core
# (13.101.098 B, sha256 verificado) y el instalador decia "el bundle no trae
# el binario" => no instalaba nada.  La causa: la comprobacion se hacia como
# usuario normal (`[ -f ... ]`), que no puede ni mirar dentro de /data/adb.
#
# Aqui se reproduce de verdad: el prefijo es 0700 de root y el instalador
# corre como un usuario NORMAL, escalando con un `su` de mentira (setuid).
# ---------------------------------------------------------------------------
say "9) prefijo 0700 de ROOT + instalador como usuario NORMAL (con escalada)"

# En el móvil /data/adb es 0700 (solo root) y el one-liner lo ejecuta un
# usuario normal de Termux.  El bundle SÍ traía bin/megadaemon-core
# (13.101.098 B, sha256 verificado) y el instalador decía "el bundle no trae
# el binario" => no instalaba nada.  La causa: la comprobación se hacía como
# usuario normal (`[ -f ... ]`), que no puede ni mirar dentro de /data/adb.
#
# Aquí se reproduce de verdad: el prefijo es 0700 de root y el instalador
# corre como un usuario NORMAL, escalando a root de verdad.
PRIV="$LAB/priv"
rm -rf "$PRIV" "$LAB/home"
mkdir -p "$PRIV/state" "$LAB/home"
LAB_SU=0
AS_USER=""

if [ "$(id -u)" = "0" ]; then
  # Somos root: hace falta un usuario SIN root y una escalada real.  El `su`
  # de mentira hace setuid(0); sólo vive en este laboratorio.
  cat > "$LAB/fakesu.c" <<'EOF'
/* `su` de mentira con escalada real: hace setuid(0) y ejecuta la orden. */
#include <unistd.h>
#include <stdio.h>
int main(int argc, char **argv) {
    if (setuid(0) != 0) { fprintf(stderr, "fakesu: sin privilegios\n"); return 1; }
    if (argc >= 3 && argv[1][0] == '-') {   /* -c "cmd" */
        execl("/bin/sh", "sh", "-c", argv[2], (char *)NULL);
        return 127;
    }
    execvp(argv[1], argv + 1);
    return 127;
}
EOF
  if cc -O2 -o "$LAB/su" "$LAB/fakesu.c" 2>/dev/null; then
    chown root:root "$LAB/su" 2>/dev/null || true
    chmod 4755 "$LAB/su"
    id -u mdlab >/dev/null 2>&1 || useradd -m mdlab 2>/dev/null || true
    if id -u mdlab >/dev/null 2>&1; then
      AS_USER="mdlab"
      LAB_SU=1
      chown -R root:root "$PRIV"
      chmod 700 "$PRIV" "$PRIV/state"
      chown -R mdlab "$LAB/home" 2>/dev/null || true
    fi
  fi
else
  # No somos root (runner del CI): su propio usuario tiene sudo sin
  # contraseña y NO puede recorrer un prefijo 0700 de root.
  if sudo -n true 2>/dev/null; then
    AS_USER="yo"
    sudo -n chown -R root:root "$PRIV" 2>/dev/null || true
    sudo -n chmod 700 "$PRIV" "$PRIV/state" 2>/dev/null || true
  fi
fi

# Desde fuera de un prefijo 0700 de root no se puede ni mirar: las
# comprobaciones de este escenario se hacen COMO ROOT tambien.
priv_x()  { if [ "$(id -u)" = "0" ]; then [ -x "$1" ]; else sudo -n test -x "$1"; fi; }
priv_f()  { if [ "$(id -u)" = "0" ]; then [ -f "$1" ]; else sudo -n test -f "$1"; fi; }
priv_cat(){ if [ "$(id -u)" = "0" ]; then cat "$1" 2>/dev/null; else sudo -n cat "$1" 2>/dev/null; fi; }

if [ -z "$AS_USER" ]; then
  printf '  OMITIDO este entorno no permite el escenario (ni root con cc, ni sudo sin contraseña)\n'
else
  # El instalador con el prefijo de root:
  sed "s#/data/adb/megadaemon#$PRIV#g; s#/data/adb/service.d#$LAB/service.d#g; s#/data/data/com.termux/files/usr/bin#$LAB/termux#g" \
    "$RAIZ/deploy/megadaemon-oneliner.sh" > "$LAB/oneliner_priv.sh"
  chmod 755 "$LAB/oneliner_priv.sh"
  : > "$MD_LOG"
  if [ "$LAB_SU" = "1" ]; then
    RUNP="su $AS_USER -s /bin/sh -c"
  else
    RUNP="sh -c"
  fi
  $RUNP "cd '$LAB' && HOME='$LAB/home' PATH='$LAB':\"\$PATH\" MD_LAB='$LAB' MD_LOG='$MD_LOG' sh '$LAB/oneliner_priv.sh' --instalar --sin-telemetria" \
    > "$LAB/salida_priv.log" 2>&1
  RCP=$?
  if grep -q 'no trae el binario' "$LAB/salida_priv.log"; then
    mal "el bundle SÍ traía el binario y el instalador dijo que no (el bug de /data/adb 0700)"
  else
    ok "no confundió 'no puedo mirar dentro de /data/adb' con 'el bundle no lo trae'"
  fi
  [ "$RCP" = "0" ] && ok "instaló como usuario normal escalando a root (salida 0)" || mal "salida $RCP"
  priv_x "$PRIV/megadaemon-core" && ok "el binario quedó instalado en el prefijo de root" || mal "el binario NO se instaló"
  priv_f "$PRIV/state/tunnel.url" && ok "tunnel.url escrito en el estado de root" || mal "tunnel.url NO escrito"
  # Idempotencia vista como usuario NORMAL: la 2ª vez NO debe reinstalar
  # (el bug de `[ -x "$DAEMON" ]` hacía reinstalar siempre y cambiar la URL).
  URLP1=$(priv_cat "$PRIV/state/tunnel.url" | tr -d '\r\n ')
  $RUNP "cd '$LAB' && HOME='$LAB/home' PATH='$LAB':\"\$PATH\" MD_LAB='$LAB' MD_LOG='$MD_LOG' sh '$LAB/oneliner_priv.sh' --instalar --sin-telemetria" \
    > "$LAB/salida_priv2.log" 2>&1
  URLP2=$(priv_cat "$PRIV/state/tunnel.url" | tr -d '\r\n ')
  [ "$URLP1" = "$URLP2" ] && ok "la 2ª ejecución NO cambió la URL (idempotencia real)" \
                         || mal "la URL cambió ($URLP1 -> $URLP2): reinstala cada vez"
  grep -q 'no toco nada' "$LAB/salida_priv2.log" \
    && ok "la 2ª ejecución usó el atajo de idempotencia (no reinstala)" \
    || mal "la 2ª ejecución reinstala: el chequeo de DAEMON se hace sin root"
fi

printf '\n===== RESULTADO: %s fallo(s) =====\n' "$FALLOS"
[ "$FALLOS" = "0" ] || { echo "--- salida de la instalación ---"; tail -30 "$LAB/salida.log"; }
exit "$FALLOS"

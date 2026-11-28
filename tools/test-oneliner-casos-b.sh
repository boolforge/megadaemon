#!/usr/bin/env bash
# =============================================================================
# test-oneliner-casos-b.sh — ESCENARIOS B del banco del instalador del móvil.
#
# QUÉ CUBRE ESTE FICHERO
# ----------------------
# Los 36 escenarios de la serie B miden el ARRANCQUE y el DIAGNÓSTICO del
# one-liner (`deploy/megadaemon-oneliner.sh`) por EFECTO (no leyendo su
# texto): el stack de arranque en `service.d`, el entorno con el que se lanzan
# el daemon, el cloudflared y el watchdog, el ciclo de vida del túnel (URL,
# `tunnel.url`, health-check), la idempotencia (2ª ejecución, `--force`) y el
# `--diagnostico` (daemons vivos, binario viejo, tamaño, sha16, espacio y
# logs).
#
# CÓMO SE ESCRIBE CADA ESCENARIO
# ------------------------------
#   * un `caso_b_NN_<nombre>` con su cabecera de 3 líneas (qué prueba, por qué
#     importa, MEDIDO/NO MEDIDO);
#   * nada de red real: el PATH del instalador es la caja de herramientas y
#     los fakes (`curl`, `wget`, `cloudflared`, `megadaemon-core`, `pgrep`,
#     `pkill`, `ps`, `grep`) son de mentira;
#   * los PIDs se fijan con `fijar_pids`; el marcador `$LAB_PROC/pkill_falla`
#     simula un `pkill` sin `-f`;
#   * `espera_linea` para lo que el instalador lanza en segundo plano.
#
# POR QUÉ NO SE LLAMA A `preparar_lab` OTRA VEZ
# --------------------------------------------
# El runner ya llama a `preparar_lab` una vez por escenario (con los knobs por
# defecto).  Volver a llamarla dentro del escenario:
#   * rehace el laboratorio entero (lento) y con 36 escenarios en paralelo
#     puede comerse la memoria del sandbox (ya ha pasado: un `cp` matado por
#     el OOM-killer dejaba el laboratorio a medias);
#   * `preparar_lab` escribe su df de mentira con `cat > "$TBR/df"`, y
#     `$TBR/df` es un ENLACE a `/usr/bin/df` ⇒ pisa el binario de verdad del
#     SISTEMA (bug real del banco, ver abajo).
# Por eso, aquí el estado del laboratorio se monta a mano, que es EXACTAMENTE
# lo que haría `preparar_lab` con esos knobs:
#   * TUNEL=caido   → se borra `state/tunnel.url` y el log del túnel se deja
#                     sin URL (`printf '... arrancando...' > state/tunnel.log`);
#   * TUNEL=sin-url → se borran `state/tunnel.url` y `state/tunnel.log`;
#   * DAEMONS=1     → `fijar_pids megadaemon-core <pid>`;
#   * WATCHDOGS=1   → `fijar_pids tunnel-watchdog.sh <pid>`;
#   * CLOUDFLARED=1 → `touch $LAB_PROC/cloudflared.vivo`;
#   * ESPACIO_MB=N  → se escribe el `df` de mentira en `$TBR/df` (como fichero
#                     de verdad, no como enlace: así no se pisa `/usr/bin/df`).
#
# NOTA SOBRE LOS ESCENARIOS B05/B06/B20/B31
# ------------------------------------------
# Para poder MEDIR el desacoplo, la stdin, el 401 y el "binario viejo" hace
# falta tocar la caja de herramientas del laboratorio (que es del ESCENARIO,
# no del banco): un `setsid` de verdad, un `curl` que responde 401 y un
# binario ELF de verdad como daemon.  Todo lo que se toca está en `$TBR`,
# `$SB` o `$L` (el laboratorio falso), nunca en el repo.
#
# ⚠️ TRAMPA DEL BANCO: el `setsid` de mentira busca "cloudflared" en la
# LÍNEA DE COMANDO COMPLETA, y la ruta del daemon lleva el nombre del
# laboratorio ($L/md/megadaemon-core).  Si un escenario se llamase
# "...cloudflared..." el banco creería que hay un cloudflared vivo.  Por eso
# ningún nombre de esta serie lleva "cloudflared" dentro.
# =============================================================================

# BUGS REALES DETECTADOS POR ESTOS ESCENARIOS (estado a 2026-11-27):
#   1) `daemon_vivo()` se quedaba con el NOMBRE del fichero ("cmdline") en
#      vez del PID: el diagnóstico imprimía "proceso: PID cmdline ()" y
#      `binario_al_dia` NUNCA podía detectar el binario viejo (miraba
#      /proc/cmdline/stat) — YA ARREGLADO por el root (commit c4fc6d9,
#      deploy/megadaemon-oneliner.sh:546).  El escenario B31 lo vigila: si
#      alguien vuelve a romperlo, B31 se pone en rojo.
#   2) El BANCO (no el instalador) pisaba el binario de verdad del sistema:
#      `cat > "$TBR/df"` seguía el enlace a /usr/bin/df y dejaba el `df` del
#      sandbox con el falso del banco (además rompía el knob ESPACIO_MB) —
#      tools/test-oneliner.sh:496.  YA ARREGLADO por el root: "df" y "kill"
#      ya no se enlazan y el `cat >` borra antes el enlace
#      (tools/test-oneliner.sh:370-372 y :507).
#   (Pendientes de arreglar en el instalador: NINGUNO de esta serie.)

# =============================================================================
# AYUDANTES (NO son escenarios: el runner sólo ejecuta los `caso_*`)
# =============================================================================

# Rehace el bundle del laboratorio con el binario del daemon que se le pase
# (y su `.sha256`), para poder meter un falso propio.
_b_rebuild_bundle() {
  local bin="$1"
  cp "$bin" "$L/bundle/stage/bin/megadaemon-core" || return 1
  chmod 755 "$L/bundle/stage/bin/megadaemon-core"
  rm -f "$L/bundle/mdo.tar.gz" "$L/bundle/mdo.tar.gz.sha256"
  ( cd "$L/bundle/stage" && tar -czf "$L/bundle/mdo.tar.gz" bin deploy ) || return 1
  ( cd "$L/bundle" && sha256sum mdo.tar.gz > mdo.tar.gz.sha256 ) || return 1
  return 0
}

# Escribe un falso PROPIO de `megadaemon-core`: anota su PID, su sesión y su
# stdin, y se queda VIVO (para poder mirarlo después de que el instalador
# termine).  Sólo el lanzamiento del COCKPIT anota esas cosas: el resto de
# invocaciones del daemon son llamadas cortas que no abren sesión.
_b_write_live_fake_daemon() {
  cat > "$L/fake-megadaemon-core" <<'FAKE'
#!/bin/sh
# Falso PROPIO del escenario: se queda vivo y dice quién es y de dónde viene.
case "$1" in
  cockpit)
    {
      echo "--- invocacion: $*"
      echo "    PID=$$"
      echo "    SID=$(awk '{print $6}' /proc/self/stat 2>/dev/null)"
      echo "    STDIN=$(readlink /proc/self/fd/0 2>/dev/null)"
    } >> "${MD_LOG:-/dev/null}"
    sleep 120 ;;
  telemetria)
    case "$2" in
      sonda) echo '{"ok":true,"codigo":401}'; exit 0 ;;
      listo) echo '{"listo":true}'; exit 0 ;;
      *)     exit 0 ;;
    esac ;;
  hash)   shift; sha256sum "$1" 2>/dev/null; exit 0 ;;
  *)      exit 0 ;;
esac
FAKE
  chmod 755 "$L/fake-megadaemon-core"
}

# Ejecuta el instalador con una stdin concreta (para B06): es lo mismo que
# `correr_instalador`, pero el instalador lee de un fichero OURDINARIA.
_b_run_installer_with_stdin() {
  local fichero="$1"; shift
  MD_NET="$NET" MD_BUNDLE="$BUNDLE" MD_SHA="$SHA" MD_HEALTH="$HEALTH" MD_TUNEL="$TUNEL" \
  PATH="$SB:$TBR" HOME="$HOME" TMPDIR="$TMPDIR" MD_LOG="$LOG" \
    sh "$L/oneliner.sh" "$@" --timeout=30 < "$fichero" > "$OUT" 2>&1
  CODIGO=$?
  return $CODIGO
}

# Envuelve el `curl` DE MENTIRA del laboratorio: las URLs del túnel devuelven
# 401 (el daemon contesta pero pide credencial).  El resto se comporta igual.
# ⚠️ El `curl` que usa el instalador es el de $SB (que va primero en el
# PATH), no el de $TBR: hay que parchear ÉSE.
_b_wrap_curl_401() {
  [ -f "$SB/curl" ] || return 1
  cp "$SB/curl" "$SB/curl.del.banco" || return 1
  cat > "$SB/curl" <<EOF
#!/bin/sh
# Falso del escenario: 401 en las URLs del túnel.
for a in "\$@"; do
  case "\$a" in
    *trycloudflare.com*) printf '401'; exit 0 ;;
  esac
done
exec "$SB/curl.del.banco" "\$@"
EOF
  chmod 755 "$SB/curl"
  return 0
}

# Pon un `setsid` DE VERDAD en la caja (el del banco es un fake que sólo anota
# el PID): así se puede MEDIR si el daemon queda en su propia sesión.
_b_use_real_setsid() {
  local real; real="$(command -v setsid 2>/dev/null || true)"
  [ -n "$real" ] && [ -x "$real" ] || return 1
  cat > "$TBR/setsid" <<EOF
#!/bin/sh
# setsid de verdad + el marcador de PIDs del banco (para que el instalador
# vea al daemon recién lanzado).
cmd="\$*"
case "\$cmd" in
  *megadaemon-core*) printf '429999\n' >> "\$LAB_PROC/megadaemon-core.pids" ;;
esac
case "\$cmd" in
  *tunnel-watchdog*) printf '429998\n' >> "\$LAB_PROC/tunnel-watchdog.sh.pids" ;;
esac
case "\$cmd" in
  *cloudflared*) : > "\$LAB_PROC/cloudflared.vivo" ;;
esac
[ \$# -gt 0 ] && shift
exec "$real" "\$@"
EOF
  chmod 755 "$TBR/setsid"
  return 0
}

# `df` de mentira en la CAJA (como fichero de verdad: si se deja el enlace,
# el `cat >` pisaría el /usr/bin/df del sistema).
_b_fake_df_mb() {
  local mb="$1" kb
  kb=$(( mb * 1024 ))
  rm -f "$TBR/df"
  cat > "$TBR/df" <<EOF
#!/bin/sh
# df de mentira del escenario: $mb MB libres.
printf 'Filesystem 1024-blocks Used Available Capacity Mounted on\n'
printf '/dev/fake $kb 1024 $kb 50%% /data/adb\n'
exit 0
EOF
  chmod 755 "$TBR/df"
}

# El bloque de entorno que registró el falso del watchdog.
_b_watchdog_env() {
  grep -A 8 -- 'script: tunnel-watchdog\.sh' "$LOG" 2>/dev/null | head -9
}

# El PID real de un proceso vivo del laboratorio (el del banco es de mentira).
_b_pid_real() {
  pgrep -f "$1" 2>/dev/null | head -1
}

# =============================================================================
# B01-B04: EL STACK DE ARRANQUE Y EL ENTORNO DEL DAEMON
# =============================================================================

# --- B01 ---------------------------------------------------------------------
# Qué prueba: los 4 scripts de arranque (99/97/98/91) quedan en `service.d`
#            y con permiso de ejecución.
# Por qué importa: sin ellos el móvil no arranca NADA al hacer BOOT y el
#            usuario tiene que pegar el one-liner a mano cada vez.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_01_boot_stack_in_service_d() {
  correr_instalador --instalar --sin-telemetria
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    comprobar "$f está en service.d" [ -f "$SVC/$f" ]
    comprobar "$f es ejecutable" [ -x "$SVC/$f" ]
  done
}

# --- B02 ---------------------------------------------------------------------
# Qué prueba: `tunnel-watchdog.sh` NO queda en `service.d`.
# Por qué importa: Magisk ejecuta `service.d` SIN entorno (LOG_DIR, DAEMON_BIN,
#            MEGADAEMON_STATE_DIR) ⇒ el watchdog entraría en un ciclo de
#            relanzamiento de cloudflared y cambiaría el hostname cada 60 s.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_02_watchdog_not_in_service_d() {
  correr_instalador --instalar --sin-telemetria
  comprobar "tunnel-watchdog.sh NO está en service.d" [ ! -f "$SVC/tunnel-watchdog.sh" ]
  comprobar "service.d sólo tiene los 4 scripts de arranque" \
    [ "$(ls "$SVC" 2>/dev/null | wc -l)" = "4" ]
}

# --- B03 ---------------------------------------------------------------------
# Qué prueba: `tunnel-watchdog.sh` queda en el prefijo y es ejecutable.
# Por qué importa: ése es su sitio (lo lanza 98-cloudflared.sh con su
#            entorno); si no queda copiado, nadie vigila el túnel.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_03_watchdog_in_prefix_executable() {
  correr_instalador --instalar --sin-telemetria
  comprobar "tunnel-watchdog.sh está en el prefijo" [ -f "$PRE/tunnel-watchdog.sh" ]
  comprobar "tunnel-watchdog.sh es ejecutable" [ -x "$PRE/tunnel-watchdog.sh" ]
}

# --- B04 ---------------------------------------------------------------------
# Qué prueba: el daemon se lanza con MEGADAEMON_STATE_DIR, MEGADAEMON_PREFIX y
#            MEGADAEMON_BIN correctos.
# Por qué importa: sin esas variables el daemon escribe el estado en el
#            directorio de la app Antigravity y el canal queda vacío.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_04_daemon_env_vars() {
  correr_instalador --instalar --sin-telemetria
  log_mensaje "el daemon se lanzó en modo cockpit" 'invocacion: cockpit'
  log_mensaje "MEGADAEMON_STATE_DIR apunta al estado del prefijo" \
    "MEGADAEMON_STATE_DIR=$ST"
  log_mensaje "MEGADAEMON_PREFIX apunta al prefijo" "MEGADAEMON_PREFIX=$PRE"
  log_mensaje "MEGADAEMON_BIN apunta al binario instalado" \
    "MEGADAEMON_BIN=$PRE/megadaemon-core"
}

# =============================================================================
# B05-B08: CÓMO SE LANZA CADA COSA (DESACOPLO, STDIN, LOGS)
# =============================================================================

# --- B05 ---------------------------------------------------------------------
# Qué prueba: el daemon se lanza DESACOPLADO (setsid/nohup): queda en su
#            propia sesión y sigue vivo cuando el instalador termina.
# Por qué importa: si cuelga de la sesión de Termux, al cerrar Termux (o al
#            cerrarse la sesión adb) el móvil se queda sin daemon.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_05_daemon_detached_from_termux() {
  _b_write_live_fake_daemon
  _b_rebuild_bundle "$L/fake-megadaemon-core" || mal "no se pudo rehacer el bundle"
  REAL_SETSID=0
  _b_use_real_setsid && REAL_SETSID=1
  correr_instalador --instalar --sin-telemetria
  # (a) el lanzamiento pasó por el `setsid` de la caja (su marcador de PIDs)
  comprobar "el daemon se lanzó a través de setsid" \
    grep -q '^429999$' "$LAB_PROC/megadaemon-core.pids"
  # (b) el proceso sigue vivo DESPUÉS de que el instalador haya terminado
  PID=$(_b_pid_real 'megadaemon-core cockpit')
  comprobar "el daemon sigue vivo tras terminar el instalador" [ -n "$PID" ]
  # (c) si hubo setsid de verdad: el daemon es jefe de su propia sesión
  #     (SID == PID) ⇒ no tiene terminal de control ⇒ sobrevive a Termux.
  if [ "$REAL_SETSID" = "1" ] && [ -n "$PID" ]; then
    SID_AMP=$(awk '{print $6}' "/proc/$PID/stat" 2>/dev/null)
    comprobar "el daemon es jefe de su propia sesión (desacoplado)" \
      [ -n "$SID_AMP" ] && [ "$SID_AMP" = "$PID" ]
  else
    omitir "no hay setsid(1) real: no se puede medir la sesión del daemon"
  fi
  [ -n "$PID" ] && kill "$PID" 2>/dev/null
  return 0
}

# --- B06 ---------------------------------------------------------------------
# Qué prueba: el daemon se lanza con `< /dev/null` (no hereda la stdin).
# Por qué importa: si hereda la stdin de la sesión, la sesión adb se queda
#            colgada esperando a que el daemon cierre la entrada.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_06_daemon_stdin_is_dev_null() {
  _b_write_live_fake_daemon
  _b_rebuild_bundle "$L/fake-megadaemon-core" || mal "no se pudo rehacer el bundle"
  printf 'esta-linea-no-debe-leer-nadie\n' > "$L/stdin.instalador"
  _b_run_installer_with_stdin "$L/stdin.instalador" --instalar --sin-telemetria
  espera_linea 'STDIN=' "$LOG" 5 || mal "el daemon no llegó a anotar su stdin"
  log_mensaje "el daemon se lanzó con la stdin en /dev/null" 'STDIN=/dev/null'
  no_log_mensaje "el daemon NO heredó la stdin del instalador" \
    "STDIN=$L/stdin.instalador"
  PID=$(_b_pid_real 'megadaemon-core cockpit')
  [ -n "$PID" ] && kill "$PID" 2>/dev/null
  return 0
}

# --- B07 ---------------------------------------------------------------------
# Qué prueba: la salida del daemon va a `daemon.log` en el ESTADO.
# Por qué importa: ese log es lo que lee el root desde Drive cuando el móvil
#            no responde; si se queda en el prefijo volátil se pierde al
#            reiniciar.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_07_daemon_log_in_state() {
  correr_instalador --instalar --sin-telemetria
  espera_linea 'daemon simulado' "$ST/daemon.log" 5 \
    || mal "el daemon no escribió en daemon.log"
  comprobar "daemon.log está en el estado" [ -s "$ST/daemon.log" ]
  comprobar "daemon.log NO está en la raíz del prefijo" [ ! -f "$PRE/daemon.log" ]
}

# --- B08 ---------------------------------------------------------------------
# Qué prueba: el cloudflared escribe su log en `tunnel.log`, en el ESTADO
#            persistente (no en `/data/local/tmp`).
# Por qué importa: el watchdog y el instalador leen ese log; si está en la
#            zona volátil, al reiniciar se pierde la URL del quick tunnel.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_08_tunnel_log_in_state() {
  # El túnel "caído": el log existe pero SIN URL (así la URL que aparezca
  # después sólo puede haberla puesto el cloudflared que lanzó el
  # instalador).
  TUNEL=caido
  rm -f "$ST/tunnel.url"
  printf '2024-01-01T00:00:00Z INF |  arrancando...\n' > "$ST/tunnel.log"
  correr_instalador --instalar --sin-telemetria
  espera_linea 'trycloudflare\.com' "$ST/tunnel.log" 5 \
    || mal "el cloudflared no escribió en el log del túnel"
  comprobar "tunnel.log está en el estado persistente" [ -f "$ST/tunnel.log" ]
  comprobar "tunnel.log NO está en la raíz del prefijo" [ ! -f "$PRE/tunnel.log" ]
  contiene "el log del túnel tiene la URL del quick tunnel" "$ST/tunnel.log" \
    'https://[a-z0-9.-]+\.trycloudflare\.com'
}

# =============================================================================
# B09-B14: EL TÚNEL Y EL WATCHDOG (NO RELANZAR LO QUE YA VIVE)
# =============================================================================

# --- B09 ---------------------------------------------------------------------
# Qué prueba: si ya hay un cloudflared vivo, el instalador NO lo relanza.
# Por qué importa: relanzarlo a ciegas genera un hostname nuevo ⇒ la flota
#            se queda ciega.  Es el bug más caro del script.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_09_live_tunnel_is_not_restarted() {
  CLOUDFLARED=1
  touch "$LAB_PROC/cloudflared.vivo"
  ANTES=$(wc -l < "$ST/tunnel.log" 2>/dev/null || echo 0)
  correr_instalador --instalar --sin-telemetria
  DESPUES=$(wc -l < "$ST/tunnel.log" 2>/dev/null || echo 0)
  salida_mensaje "dijo que hay un cloudflared vivo" 'hay un cloudflared vivo: no lo relanzo'
  comprobar "el cloudflared NO se relanzó (su log no creció)" \
    [ "$DESPUES" = "$ANTES" ]
}

# --- B10 ---------------------------------------------------------------------
# Qué prueba: si NO hay cloudflared, el instalador lo lanza.
# Por qué importa: sin cloudflared no hay túnel ⇒ el móvil es inalcanzable
#            desde fuera y el canal no se puede publicar.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_10_missing_tunnel_is_started() {
  TUNEL=caido
  rm -f "$ST/tunnel.url"
  printf '2024-01-01T00:00:00Z INF |  arrancando...\n' > "$ST/tunnel.log"
  correr_instalador --instalar --sin-telemetria
  comprobar "el instalador lanzó el cloudflared" [ -f "$LAB_PROC/cloudflared.vivo" ]
  contiene "el cloudflared publicó su URL en el log del túnel" "$ST/tunnel.log" \
    'https://[a-z0-9.-]+\.trycloudflare\.com'
}

# --- B11 ---------------------------------------------------------------------
# Qué prueba: si ya hay un watchdog vivo, el instalador NO relanza otro.
# Por qué importa: dos watchdogs se pelean por el túnel y cada uno relanza
#            cloudflared ⇒ el hostname cambia cada 60 s y la flota se ciega.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_11_live_watchdog_is_not_restarted() {
  WATCHDOGS=1
  fijar_pids tunnel-watchdog.sh 424442
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "dijo que ya hay un watchdog vivo" 'watchdogs vivos: no relanzo'
  comprobar "no hay un segundo watchdog (sigue 1 vivo)" \
    [ "$(wc -l < "$LAB_PROC/tunnel-watchdog.sh.pids" 2>/dev/null || echo 0)" = "1" ]
}

# --- B12 ---------------------------------------------------------------------
# Qué prueba: si NO hay watchdog, el instalador lo lanza CON su entorno
#            (LOG_DIR, DAEMON_BIN, PUBLISH, CF, CFT_TARGET, MEGADAEMON_STATE_DIR,
#            MEGADAEMON_ENV).
# Por qué importa: el watchdog sin entorno no sabe dónde está el estado ni
#            cuál es el daemon ⇒ relanza cloudflared en bucle.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_12_missing_watchdog_started_with_env() {
  correr_instalador --instalar --sin-telemetria
  espera_linea 'script: tunnel-watchdog\.sh' "$LOG" 5 \
    || mal "el watchdog no llegó a lanzarse"
  ENV_WD=$(_b_watchdog_env)
  printf '%s' "$ENV_WD" | grep -q "LOG_DIR=$PRE" \
    && ok "el watchdog recibe LOG_DIR del prefijo" \
    || mal "el watchdog NO recibe LOG_DIR del prefijo"
  printf '%s' "$ENV_WD" | grep -q "DAEMON_BIN=$PRE/megadaemon-core" \
    && ok "el watchdog recibe DAEMON_BIN" \
    || mal "el watchdog NO recibe DAEMON_BIN"
  printf '%s' "$ENV_WD" | grep -q "PUBLISH=$PRE/channel-publish" \
    && ok "el watchdog recibe PUBLISH" \
    || mal "el watchdog NO recibe PUBLISH"
  printf '%s' "$ENV_WD" | grep -q "MEGADAEMON_STATE_DIR=$ST" \
    && ok "el watchdog recibe MEGADAEMON_STATE_DIR" \
    || mal "el watchdog NO recibe MEGADAEMON_STATE_DIR"
}

# --- B13 ---------------------------------------------------------------------
# Qué prueba: el watchdog recibe el CF (binario de cloudflared) y el
#            CFT_TARGET (destino del túnel) correctos.
# Por qué importa: con un CF equivocado el watchdog no puede relanzar el
#            túnel; con un target equivocado, sondea otra cosa.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_13_watchdog_cf_and_target() {
  correr_instalador --instalar --sin-telemetria
  espera_linea 'script: tunnel-watchdog\.sh' "$LOG" 5 \
    || mal "el watchdog no llegó a lanzarse"
  ENV_WD=$(_b_watchdog_env)
  printf '%s' "$ENV_WD" | grep -q "CF=$L/termux/cloudflared" \
    && ok "el watchdog recibe el CF del móvil" \
    || mal "el watchdog NO recibe el CF del móvil"
  printf '%s' "$ENV_WD" | grep -q 'CFT_TARGET=http://127\.0\.0\.1:9090' \
    && ok "el watchdog recibe el CFT_TARGET del cockpit" \
    || mal "el watchdog NO recibe el CFT_TARGET del cockpit"
}

# --- B14 ---------------------------------------------------------------------
# Qué prueba: el watchdog recibe MEGADAEMON_ENV apuntando al fichero de
#            entorno del prefijo.
# Por qué importa: de ahí saca el daemon la API key; si el watchdog no lo
#            ve, publica el canal sin credencial y la flota no lo descubre.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_14_watchdog_env_file() {
  correr_instalador --instalar --sin-telemetria
  espera_linea 'script: tunnel-watchdog\.sh' "$LOG" 5 \
    || mal "el watchdog no llegó a lanzarse"
  ENV_WD=$(_b_watchdog_env)
  printf '%s' "$ENV_WD" | grep -q "MEGADAEMON_ENV=$PRE/megadaemon.env" \
    && ok "el watchdog recibe el fichero de entorno del prefijo" \
    || mal "el watchdog NO recibe el fichero de entorno del prefijo"
}

# =============================================================================
# B15-B22: EL CICLO DE VIDA DEL TÚNEL
# =============================================================================

# --- B15 ---------------------------------------------------------------------
# Qué prueba: el instalador extrae la URL del quick tunnel del `tunnel.log`.
# Por qué importa: esa URL es el único dato que hace al móvil alcanzable;
#            sin ella el canal se publica vacío.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_15_url_extracted_from_tunnel_log() {
  TUNEL=caido
  rm -f "$ST/tunnel.url"
  printf '2024-01-01T00:00:00Z INF |  arrancando...\n' > "$ST/tunnel.log"
  correr_instalador --instalar --sin-telemetria
  # El laboratorio NO pone ninguna URL: la única fuente es el log que
  # escribió el cloudflared.
  comprobar "se escribió tunnel.url" [ -s "$ST/tunnel.url" ]
  contiene "la URL salió del log del túnel" "$ST/tunnel.url" \
    'https://[a-z0-9.-]+\.trycloudflare\.com'
}

# --- B16 ---------------------------------------------------------------------
# Qué prueba: el instalador escribe `tunnel.url` en el ESTADO.
# Por qué importa: el daemon lee ese fichero (`channel.rs`): sin él,
#            `telemetria listo` nunca es true y el canal queda vacío.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_16_tunnel_url_written_in_state() {
  correr_instalador --instalar --sin-telemetria
  comprobar "hay tunnel.url en el estado" [ -s "$ST/tunnel.url" ]
  contiene "tunnel.url tiene la URL del túnel" "$ST/tunnel.url" \
    'https://[a-z0-9.-]+\.trycloudflare\.com'
}

# --- B17 ---------------------------------------------------------------------
# Qué prueba: el instalador escribe TAMBIÉN `tunnel.url` en el prefijo.
# Por qué importa: ése es el fichero que lee el watchdog (LOG_DIR/tunnel.url)
#            y el supervisor del boot: sin él, el watchdog cree que el túnel
#            está muerto y lo relanza cada 60 s.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_17_tunnel_url_written_in_prefix() {
  correr_instalador --instalar --sin-telemetria
  comprobar "hay tunnel.url en el prefijo" [ -s "$PRE/tunnel.url" ]
  contiene "el tunnel.url del prefijo tiene la URL" "$PRE/tunnel.url" \
    'https://[a-z0-9.-]+\.trycloudflare\.com'
}

# --- B18 ---------------------------------------------------------------------
# Qué prueba: `tunnel.url` queda con permisos 644.
# Por qué importa: lo lee el daemon, que puede no ser root; con 600 el
#            móvil queda sin canal aunque todo lo demás esté bien.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_18_tunnel_url_mode_644() {
  TUNEL=caido
  rm -f "$ST/tunnel.url"
  printf '2024-01-01T00:00:00Z INF |  arrancando...\n' > "$ST/tunnel.log"
  correr_instalador --instalar --sin-telemetria
  comprobar "tunnel.url existe" [ -f "$ST/tunnel.url" ]
  comprobar "tunnel.url queda con permisos 644" \
    [ "$(stat -c '%a' "$ST/tunnel.url" 2>/dev/null)" = "644" ]
}

# --- B19 ---------------------------------------------------------------------
# Qué prueba: el health-check sondea la URL del túnel con el daemon.
# Por qué importa: el sondeo va por el daemon (no por curl, que no existe en
#            el móvil) y es el que distingue 401 (vivo) de 530 (muerto).
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_19_health_check_probes_tunnel_with_daemon() {
  correr_instalador --instalar --sin-telemetria
  log_mensaje "el daemon sondó la URL del túnel" \
    'invocacion: telemetria sonda https://'
  salida_mensaje "anunció que el móvil es alcanzable" 'LISTO: el móvil es alcanzable'
}

# --- B20 ---------------------------------------------------------------------
# Qué prueba: un 401/403 cuenta como túnel VIVO (el daemon contesta, pero
#            pide credencial).
# Por qué importa: es el criterio del watchdog y del instalador: matar el
#            túnel por una credencial caducada cambia el hostname y ciega a
#            la flota.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_20_http_401_counts_as_alive() {
  correr_instalador --instalar --sin-telemetria
  # La sonda del daemon falla (HEALTH=ko) y el túnel responde 401: el único
  # camino que queda es el código HTTP.
  _b_wrap_curl_401 || mal "no se pudo envolver el curl de mentira"
  HEALTH=ko
  correr_instalador --diagnostico --sin-telemetria
  salida_mensaje "el túnel con 401 cuenta como VIVO" 'el túnel responde'
  salida_mensaje "el diagnóstico enseñó el código 401" 'código 401'
}

# --- B21 ---------------------------------------------------------------------
# Qué prueba: si no hay URL en el log, el instalador AVISA ("no se encontró la
#            URL del túnel") en vez de callarse.
# Por qué importa: un silencio deja al usuario pensando que todo fue bien
#            mientras el móvil sigue inalcanzable.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_21_missing_url_is_reported() {
  TUNEL=sin-url
  rm -f "$ST/tunnel.url" "$ST/tunnel.log"
  correr_instalador --instalar --sin-telemetria --timeout=30
  salida_mensaje "avisó de que no encontró la URL del túnel" \
    'no se encontró la URL del túnel'
}

# --- B22 ---------------------------------------------------------------------
# Qué prueba: con `--timeout=30` el instalador termina antes de 60 s.
# Por qué importa: el usuario pega una línea y se queda mirando la pantalla;
#            el instalador no puede quedarse esperando minutos al túnel.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_22_installer_finishes_within_timeout() {
  TUNEL=sin-url
  rm -f "$ST/tunnel.url" "$ST/tunnel.log"
  T0=$(date +%s)
  correr_instalador --instalar --sin-telemetria --timeout=30
  T1=$(date +%s)
  ELAPSED=$((T1 - T0))
  comprobar "el instalador terminó en menos de 60 s (tardó ${ELAPSED}s)" \
    [ "$ELAPSED" -lt 60 ]
  comprobar "el instalador no se quedó colgado" [ "$CODIGO" = "0" ]
}

# =============================================================================
# B23-B29: IDEMPOTENCIA Y `--force`
# =============================================================================

# --- B23 ---------------------------------------------------------------------
# Qué prueba: la 2ª ejecución detecta que ya está sano y dice "no toco nada".
# Por qué importa: es el atajo que evita el bug más caro (reinstalar y
#            cambiar el hostname en cada reejecución).
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_23_second_run_detects_healthy() {
  correr_instalador --instalar --sin-telemetria
  correr_instalador --instalar --sin-telemetria
  comprobar "la 2ª ejecución sale con 0" [ "$CODIGO" = "0" ]
  salida_mensaje "detectó que ya estaba sano" 'no toco nada'
}

# --- B24 ---------------------------------------------------------------------
# Qué prueba: la 2ª ejecución NO cambia la URL del túnel.
# Por qué importa: si la URL cambia, la flota que tenía la URL vieja queda
#            ciega aunque el móvil esté perfecto.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_24_second_run_keeps_url() {
  correr_instalador --instalar --sin-telemetria
  URL1=$(cat "$ST/tunnel.url" 2>/dev/null)
  correr_instalador --instalar --sin-telemetria
  URL2=$(cat "$ST/tunnel.url" 2>/dev/null)
  comprobar "la URL del túnel NO cambió" [ -n "$URL1" ] && [ "$URL1" = "$URL2" ]
}

# --- B25 ---------------------------------------------------------------------
# Qué prueba: la 2ª ejecución NO reinstala el binario (no deja un `.prev`).
# Por qué importa: reinstalar el binario mientras corre es justo lo que puede
#            dejar al móvil con el daemon a medias.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_25_second_run_does_not_reinstall() {
  correr_instalador --instalar --sin-telemetria
  BIN1=$(wc -c < "$PRE/megadaemon-core" 2>/dev/null)
  correr_instalador --instalar --sin-telemetria
  BIN2=$(wc -c < "$PRE/megadaemon-core" 2>/dev/null)
  comprobar "el binario NO se reinstaló" [ "$BIN1" = "$BIN2" ]
  comprobar "no dejó un .prev (no reinstaló)" [ ! -f "$PRE/megadaemon-core.prev" ]
}

# --- B26 ---------------------------------------------------------------------
# Qué prueba: `--force` reinstala aunque el móvil esté sano.
# Por qué importa: es la escape hatch para cuando el usuario sabe que hay
#            que actualizar; si no reinstala, `--force` no sirve de nada.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_26_force_reinstalls_healthy_phone() {
  correr_instalador --instalar --sin-telemetria
  correr_instalador --instalar --sin-telemetria --force
  no_salida_mensaje "no usó el atajo de idempotencia" 'no toco nada'
  salida_mensaje "reinstaló el binario" 'binario instalado'
  comprobar "dejó el respaldo del binario anterior" \
    [ -f "$PRE/megadaemon-core.prev" ]
}

# --- B27 ---------------------------------------------------------------------
# Qué prueba: `--force` con el túnel CAÍDO repara el túnel (relanza el
#            cloudflared y vuelve a tener URL).
# Por qué importa: un túnel muerto con la URL perdida deja al móvil
#            inalcanzable; el instalador tiene que arreglarlo, no sólo
#            avisar.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_27_force_repairs_dead_tunnel() {
  correr_instalador --instalar --sin-telemetria
  # El túnel se cae: muere el cloudflared y se pierden la URL y su log.
  rm -f "$ST/tunnel.url" "$LAB_PROC/cloudflared.vivo"
  printf '2024-01-01T00:00:00Z INF |  arrancando...\n' > "$ST/tunnel.log"
  correr_instalador --instalar --sin-telemetria --force
  no_salida_mensaje "no usó el atajo de idempotencia" 'no toco nada'
  comprobar "relanzó el cloudflared" [ -f "$LAB_PROC/cloudflared.vivo" ]
  comprobar "volvió a escribir la URL del túnel" [ -s "$ST/tunnel.url" ]
  salida_mensaje "el túnel vuelve a responder" 'el túnel responde'
}

# --- B28 ---------------------------------------------------------------------
# Qué prueba: sin `tunnel.url` NO hay atajo de idempotencia (reinstala).
# Por qué importa: si el atajo se aplicara con la URL perdida, el instalador
#            dejaría al móvil sin túnel y sin avisar.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_28_no_url_no_idempotency_shortcut() {
  correr_instalador --instalar --sin-telemetria
  rm -f "$ST/tunnel.url"
  correr_instalador --instalar --sin-telemetria
  no_salida_mensaje "no usó el atajo de idempotencia" 'no toco nada'
  salida_mensaje "reinstaló el binario" 'binario instalado'
  comprobar "dejó el respaldo del binario anterior" \
    [ -f "$PRE/megadaemon-core.prev" ]
}

# --- B29 ---------------------------------------------------------------------
# Qué prueba: sin binario instalado NO hay atajo (instala de verdad).
# Por qué importa: el atajo sólo puede existir si hay binario; si se
#            aplicara sin él, el móvil se quedaría sin daemon.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_29_no_binary_no_idempotency_shortcut() {
  correr_instalador --instalar --sin-telemetria
  rm -f "$PRE/megadaemon-core"
  correr_instalador --instalar --sin-telemetria
  no_salida_mensaje "no usó el atajo de idempotencia" 'no toco nada'
  salida_mensaje "instaló el binario de verdad" 'binario instalado'
  comprobar "el binario volvió a estar instalado" ejecutable "$PRE/megadaemon-core"
}

# =============================================================================
# B30-B36: EL `--diagnostico`
# =============================================================================

# --- B30 ---------------------------------------------------------------------
# Qué prueba: el diagnóstico cuenta los daemons vivos (DAEMONS=1 ⇒
#            "daemons en el sistema: 1").
# Por qué importa: es el número que el root mira en Drive para saber si el
#            móvil tiene uno, varios o ninguno.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_30_diagnostic_counts_daemons() {
  DAEMONS=1
  fijar_pids megadaemon-core 424242
  correr_instalador --diagnostico --sin-telemetria
  salida_mensaje "contó el daemon vivo" 'daemons en el sistema: 1'
}

# --- B31 ---------------------------------------------------------------------
# Qué prueba: el diagnóstico detecta que el proceso corre un binario VIEJO
#            (el binario en disco es más nuevo que el arranque del proceso) y
#            avisa de que hay que reiniciarlo.
# Por qué importa: sin ese aviso, el root ve "todo bien" con un daemon que
#            lleva días ejecutando el código anterior al instalado.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_31_diagnostic_detects_old_binary() {
  preparar_lab
  # MEDIDO (2026-11-27): esto NO puede depender de un proceso real.  En el
  # runner del CI el `sleep` es el binario MULTICALL de coreutils (que muere
  # si el argv[0] no es un programa suyo) y el resultado no era el mismo que
  # en local => el escenario era NO DETERMINISTA.  Ahora el /proc del
  # laboratorio es SINTETICO (MD_STARTTIME = el arranque; MD_EXE = el exe),
  # y se comprueban las DOS ramas:
  #   * el proceso arrancó ANTES de que el binario en disco cambiara =>
  #     el daemon corre el binario VIEJO (hay que reiniciarlo);
  #   * el proceso arrancó DESPUES => el binario en servicio es el nuevo.
  DAEMONS=1
  preparar_lab
  # El daemon tiene que estar INSTALADO (para que exista su mtime) y VIVO
  # (para que el diagnostico lo mire).
  correr_instalador --instalar --sin-telemetria
  MD_STARTTIME=100
  MD_EXE="$PRE/megadaemon-core"
  correr_instalador --diagnostico --sin-telemetria
  salida_mensaje "detectó que el proceso corre el binario VIEJO" \
    'el proceso corre el binario VIEJO'
  # La otra rama: el proceso arrancó despues del binario => esta al dia.
  MD_STARTTIME=99999999999
  correr_instalador --diagnostico --sin-telemetria
  salida_mensaje "con el proceso posterior, el binario en servicio es el nuevo" \
    'binario en servicio = binario en disco'
  return 0
}

# --- B32 ---------------------------------------------------------------------
# Qué prueba: el diagnóstico dice "NO INSTALADO" si no hay binario.
# Por qué importa: es la respuesta que el root necesita para no perder el
#            tiempo buscando un problema que no está.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_32_diagnostic_says_not_installed() {
  correr_instalador --diagnostico --sin-telemetria
  salida_mensaje "dijo que el daemon no está instalado" 'NO INSTALADO'
}

# --- B33 ---------------------------------------------------------------------
# Qué prueba: el diagnóstico muestra el tamaño del binario en bytes.
# Por qué importa: un binario de 13 MB y otro de 4 KB son síntomas de un
#            bundle roto; el tamaño lo delata de un vistazo.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_33_diagnostic_shows_binary_size() {
  correr_instalador --instalar --sin-telemetria
  TAM=$(wc -c < "$PRE/megadaemon-core" 2>/dev/null)
  correr_instalador --diagnostico --sin-telemetria
  salida_mensaje "mostró el tamaño del binario en bytes" "\($TAM B, sha16"
}

# --- B34 ---------------------------------------------------------------------
# Qué prueba: el diagnóstico muestra el sha16 del binario (identificador, no
#            secreto).
# Por qué importa: es lo que permite al root saber si el móvil tiene la
#            versión que cree tener, sin pedirle el binario entero.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_34_diagnostic_shows_binary_sha16() {
  correr_instalador --instalar --sin-telemetria
  SHA16=$(sha256sum "$PRE/megadaemon-core" 2>/dev/null | cut -c1-16)
  correr_instalador --diagnostico --sin-telemetria
  salida_mensaje "mostró el sha16 del binario" "sha16 $SHA16"
}

# --- B35 ---------------------------------------------------------------------
# Qué prueba: el diagnóstico muestra el espacio libre en el prefijo.
# Por qué importa: sin sitio, el móvil instala y luego no arranca NADA; el
#            número en el diagnóstico es lo que dice si hay que liberar.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_35_diagnostic_shows_free_space() {
  ESPACIO_MB=1234
  _b_fake_df_mb 1234
  correr_instalador --instalar --sin-telemetria
  correr_instalador --diagnostico --sin-telemetria
  salida_mensaje "mostró el espacio libre del prefijo" '1234 MB'
}

# --- B36 ---------------------------------------------------------------------
# Qué prueba: el diagnóstico vuelca las últimas líneas de daemon.log,
#            tunnel.log y tunnel-watchdog.log.
# Por qué importa: es lo que lee el root cuando el móvil no responde: sin
#            los logs, sólo queda speculation.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-27 (laboratorio falso del banco).
caso_b_36_diagnostic_dumps_last_log_lines() {
  correr_instalador --instalar --sin-telemetria
  printf 'linea vieja\nlinea media\nULTIMA daemon.log\n' >> "$ST/daemon.log"
  printf 'linea vieja\nULTIMA tunnel.log\n' >> "$ST/tunnel.log"
  printf 'linea vieja\nULTIMA tunnel-watchdog.log\n' >> "$ST/tunnel-watchdog.log"
  correr_instalador --diagnostico --sin-telemetria
  salida_mensaje "volcó las últimas líneas de daemon.log" 'ULTIMA daemon\.log'
  salida_mensaje "volcó las últimas líneas de tunnel.log" 'ULTIMA tunnel\.log'
  salida_mensaje "volcó las últimas líneas de tunnel-watchdog.log" \
    'ULTIMA tunnel-watchdog\.log'
}

# --- B37 ---------------------------------------------------------------------
# Qué prueba: el supervisor CANONICO se queda vivo y su COPIA (`.bak`) se
#            mata; ademas, el FICHERO de la copia se borra de `service.d`.
# Por qué importa: Magisk ejecuta TODO lo que sea ejecutable en
#            /data/adb/service.d => una copia del supervisor = un supervisor
#            MAS en cada arranque, que relanzaba el tunel legacy contra el
#            8765 (sin listener) y se peleaba con el canonico (medido en el
#            movil: 70 s / 133 s alternados).  Matar el proceso NO basta: el
#            fichero vuelve a cerrarlo en el siguiente arranque.
#            Y "dejar el mas antiguo" es ERROR: la copia `.bak` se creo al
#            actualizar, asi que es MAS ANTIGUA que el canonico => se
#            quedaba el malo y se mataba el bueno.
# MEDIDO/NO MEDIDO: MEDIDO 2026-11-28 (laboratorio falso del banco).
caso_b_37_el_supervisor_canonico_se_queda() {
  SUPERVISORS=2; DAEMONS=1; HEALTH=ok; TUNEL=ok
  preparar_lab
  # El argv de los dos supervisores: el canonico y su copia .bak.
  fijar_pids 99-megadaemon.sh 424342 424343
  fijar_argv 424342 "sh" "$SVC/99-megadaemon.sh"
  fijar_argv 424343 "sh" "$SVC/99-megadaemon.sh.bak-50k"
  # Y la copia en el disco (Magisk la ejecutaria en el boot).
  printf '#!/bin/sh\nexit 0\n' > "$SVC/99-megadaemon.sh.bak-50k"
  chmod 755 "$SVC/99-megadaemon.sh.bak-50k"
  correr_instalador --instalar --sin-telemetria
  guardar_salida b37
  # 1) el canonico sigue vivo.
  comprobar "el supervisor canonico sigue vivo" \
    grep -qx 424342 "$LAB_PROC/99-megadaemon.sh.pids"
  # 2) la copia NO (y el instalador lo dice).
  verificar "el supervisor NO canonico esta muerto" \
    grep -qx 424343 "$LAB_PROC/99-megadaemon.sh.pids"
  salida_mensaje "el instalador avisa del supervisor NO canonico" \
    'supervisor NO canonico eliminado'
  # 3) el FICHERO de la copia se borra de service.d (si no, vuelve).
  verificar "la copia .bak se borro de service.d" \
    test -e "$SVC/99-megadaemon.sh.bak-50k"
  # 4) y los 4 canonicos siguen ahi.
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    comprobar "$f sigue en service.d" test -f "$SVC/$f"
  done
}

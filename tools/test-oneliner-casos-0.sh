#!/usr/bin/env bash
# =============================================================================
# test-oneliner-casos-0.sh — ESCENARIOS BASE del instalador del móvil.
#
# Los 10 escenarios originales, PORTADOS al harness v2 y REFUERZADOS:
# antes se miraba el TEXTO del script (grep) para "probar" cosas que sólo
# se pueden probar ejecutándolo; ahora se EJECUTA el instalador en un
# laboratorio falso y se comprueba el EFECTO real.
#
# Estos 10 son el mínimo histórico (no se quitan: son la regresión de los
# bugs que se vieron en el SM-G998B, MEDIDO 2026-11-27).
# =============================================================================

# --- 1) instalación limpia --------------------------------------------------
caso_base_01_instalacion_limpia() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "el instalador termina con 0" [ "$CODIGO" = "0" ]
  comprobar "el binario queda instalado y ejecutable" ejecutable "$PRE/megadaemon-core"
  comprobar "el binario no está vacío" [ -s "$PRE/megadaemon-core" ]
  comprobar "se escribió tunnel.url" [ -s "$ST/tunnel.url" ]
  contiene "tunnel.url tiene la URL del túnel" "$ST/tunnel.url" 'https://[a-z0-9.-]+\.trycloudflare\.com'
  comprobar "se escribió el informe de instalación" [ -s "$ST/instalacion.json" ]
  json_valido "el informe de instalación es JSON válido" "$ST/instalacion.json"
  comprobar "el núcleo forense quedó instalado" ejecutable "$PRE/mddoctor"
  comprobar "el publicador del canal quedó instalado" ejecutable "$PRE/channel-publish"
}

# --- 2) el daemon se lanzó CON su entorno -----------------------------------
caso_base_02_entorno_del_daemon() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  log_mensaje "el daemon se lanzó" 'invocacion: cockpit'
  log_mensaje "el daemon recibe MEGADAEMON_STATE_DIR" 'MEGADAEMON_STATE_DIR=.+'
  log_mensaje "el daemon recibe MEGADAEMON_PREFIX" 'MEGADAEMON_PREFIX=.+'
  log_mensaje "el daemon recibe MEGADAEMON_BIN" 'MEGADAEMON_BIN=.+'
  # Y que apunten AL PREFIXIO de este laboratorio, no a otro sitio.
  if grep -A 5 'invocacion: cockpit' "$LOG" 2>/dev/null | grep -q "MEGADAEMON_STATE_DIR=$ST"; then
    ok "el estado del daemon apunta al prefijo del laboratorio"
  else
    mal "el estado del daemon NO apunta al prefijo del laboratorio"
  fi
}

# --- 3) el watchdog se lanzó CON su entorno y NO está en service.d ----------
caso_base_03_entorno_del_watchdog() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  espera_linea 'script: tunnel-watchdog\.sh' "$LOG" 5 \
    || mal "el watchdog no llegó a lanzarse"
  ok "el watchdog se lanzó"
  W=$(grep -A 8 'script: tunnel-watchdog\.sh' "$LOG" 2>/dev/null | head -9)
  printf '%s' "$W" | grep -q 'LOG_DIR=' && ok "el watchdog recibe LOG_DIR" || mal "el watchdog NO recibe LOG_DIR"
  printf '%s' "$W" | grep -q "LOG_DIR=$PRE" && ok "LOG_DIR apunta al prefijo" || mal "LOG_DIR apunta a otro sitio"
  printf '%s' "$W" | grep -q 'DAEMON_BIN=' && ok "el watchdog recibe DAEMON_BIN" || mal "el watchdog NO recibe DAEMON_BIN"
  printf '%s' "$W" | grep -q "DAEMON_BIN=$PRE" && ok "DAEMON_BIN apunta al prefijo" || mal "DAEMON_BIN apunta a otro sitio"
  printf '%s' "$W" | grep -q "MEGADAEMON_STATE_DIR=$ST" && ok "el watchdog recibe el state dir" || mal "el watchdog NO recibe el state dir"
  printf '%s' "$W" | grep -q 'CFT_TARGET=' && ok "el watchdog recibe CFT_TARGET" || mal "el watchdog NO recibe CFT_TARGET"
  if [ -f "$SVC/tunnel-watchdog.sh" ]; then
    mal "tunnel-watchdog.sh está en service.d (Magisk lo ejecutaría SIN entorno)"
  else
    ok "tunnel-watchdog.sh NO está en service.d"
  fi
  comprobar "tunnel-watchdog.sh está en el prefijo" ejecutable "$PRE/tunnel-watchdog.sh"
}

# --- 4) el stack de arranque está en service.d ------------------------------
caso_base_04_stack_en_service_d() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  for f in 99-megadaemon.sh 97-megadaemon-keeper.sh 98-cloudflared.sh 91-adb-tcp.sh; do
    if [ -f "$SVC/$f" ]; then
      ok "$f está en service.d (arranque en boot)"
    else
      mal "$f NO está en service.d: el móvil no arrancará nada"
    fi
  done
}

# --- 5) el health-check usó la sonda con la URL -----------------------------
caso_base_05_health_check() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  log_mensaje "se sondó el túnel" 'telemetria sonda https://'
  salida_mensaje "anunció que está listo" 'LISTO: el móvil es alcanzable'
  comprobar "el informe dice que el health-check pasó" \
    python3 -c "import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if any(p['nombre']=='health_check' and p['ok'] for p in d.get('pasos',[])) else 1)" "$ST/instalacion.json"
}

# --- 6) idempotencia: la 2ª ejecución NO toca nada --------------------------
caso_base_06_idempotencia() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  URL1=$(cat "$ST/tunnel.url" 2>/dev/null)
  BIN1=$(wc -c < "$PRE/megadaemon-core" 2>/dev/null)
  correr_instalador --instalar --sin-telemetria
  URL2=$(cat "$ST/tunnel.url" 2>/dev/null)
  BIN2=$(wc -c < "$PRE/megadaemon-core" 2>/dev/null)
  comprobar "la 2ª ejecución sale con 0" [ "$CODIGO" = "0" ]
  salida_mensaje "detectó que ya estaba sano" 'no toco nada'
  comprobar "la URL del túnel NO cambió" [ "$URL1" = "$URL2" ]
  comprobar "el binario NO se reinstaló" [ "$BIN1" = "$BIN2" ]
  # Y no debe haber dejado un .prev (sería señal de que reinstaló).
  if [ -f "$PRE/megadaemon-core.prev" ]; then
    mal "la 2ª ejecución dejó un .prev: reinstaló el binario"
  else
    ok "la 2ª ejecución no dejó .prev (no reinstaló)"
  fi
}

# --- 7) un bundle CORRUPTO aborta y no rompe el móvil ------------------------
caso_base_07_bundle_corrupto() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  BINOK=$(wc -c < "$PRE/megadaemon-core" 2>/dev/null)
  BUNDLE=corrupto
  preparar_lab
  # Se reinstala el binario sano a mano para simular el móvil ya instalado.
  cp "$SB/megadaemon-core" "$PRE/megadaemon-core"; chmod 755 "$PRE/megadaemon-core"
  correr_instalador --instalar --sin-telemetria --force
  comprobar "aborta con código != 0" [ "$CODIGO" != "0" ]
  salida_mensaje "explica que no se pudo extraer" 'no se pudo extraer el bundle'
  comprobar "el binario vivo sigue ahí" ejecutable "$PRE/megadaemon-core"
  comprobar "el binario vivo NO se truncó" [ "$(wc -c < "$PRE/megadaemon-core" 2>/dev/null)" = "$BINOK" ]
}

# --- 8) un sha256 que NO cuadra aborta --------------------------------------
caso_base_08_sha_distinto() {
  SHA=distinto
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "aborta con código != 0" [ "$CODIGO" != "0" ]
  salida_mensaje "lo dice claramente" 'NO cuadra'
  comprobar "NO instaló el binario" [ ! -f "$PRE/megadaemon-core" ]
}

# --- 9) el modo PC se detecta ----------------------------------------------
caso_base_09_modo_pc() {
  preparar_lab
  correr_instalador --pc
  comprobar "sin adb aborta" [ "$CODIGO" != "0" ]
  salida_mensaje "dice que falta adb" 'no hay adb en el PC'
}

# --- 10) prefijo 0700 de ROOT + instalador como usuario NORMAL --------------
caso_base_10_prefijo_0700_root() {
  PREFIX_0700=1
  preparar_lab
  if [ "$USUARIO_DISPONIBLE" != "1" ]; then
    omitir "este entorno no permite escalada (ni root con cc, ni usuario de pruebas)"
    return
  fi
  correr_instalador_como_usuario --instalar --sin-telemetria
  salida_no_mensaje "no confunde 'no puedo mirar /data/adb' con 'el bundle no lo trae'" 'no trae el binario'
  comprobar "instala como usuario normal escalando a root" [ "$CODIGO" = "0" ]
  comprobar "el binario quedó en el prefijo de root" ejecutable "$PRE/megadaemon-core"
  comprobar "tunnel.url escrito en el estado de root" [ -s "$ST/tunnel.url" ]
  URLP1=$(cat "$ST/tunnel.url" 2>/dev/null | tr -d '\r\n ')
  correr_instalador_como_usuario --instalar --sin-telemetria
  URLP2=$(cat "$ST/tunnel.url" 2>/dev/null | tr -d '\r\n ')
  comprobar "la 2ª ejecución NO cambió la URL (idempotencia real)" [ "$URLP1" = "$URLP2" ]
  salida_mensaje "la 2ª ejecución usó el atajo de idempotencia" 'no toco nada'
}

# --- 11) LOS 4 BUGS DEL INSTALADOR QUE VIERON EN EL MÓVIL ------------------
# MEDIDO 2026-11-27 (SM-G998B).  Aquí se comprueban por EFECTO, no por texto.
caso_base_11_los_4_bugs_del_movil() {
  # (a) "sh: 1226: autoreparar: not found": la función se llamaba sin paréntesis.
  #     Se provoca el camino que la invoca (túnel caído) y se comprueba que
  #     la autoreparación se EJECUTA y no se busca como binario externo.
  TUNEL=caido; HEALTH=ko
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  guardar_salida fase_a
  no_salida_mensaje "autoreparar no se busca como comando externo" 'autoreparar: not found'
  contiene "la autoreparación se ejecutó" "$L/fase_a.salida" 'autorreparaci'

  # (b) "daemon NO CORRE" con el daemon SANO: se leía /proc/*/cmdline como
  #     usuario normal.  Con UN daemon fictorio vivo, el diagnóstico NO puede
  #     decir "NO CORRE".
  DAEMONS=1; HEALTH=ok; TUNEL=ok
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  guardar_salida fase_b
  no_salida_mensaje "el diagnóstico NO dice 'NO CORRE' con el daemon vivo" 'NO CORRE'
  contiene "el diagnóstico cuenta el daemon vivo" "$L/fase_b.salida" 'daemons en el sistema: 1'

  # (c) "Permission denied" en gdrive_credentials.json (0600 de root): el
  #     `wc -c <` se pedía como usuario normal.
  DAEMONS=1
  preparar_lab
  printf '{"installed":{"client_id":"x"}}' > "$ST/gdrive_credentials.json"
  chmod 600 "$ST/gdrive_credentials.json"
  correr_instalador --instalar --sin-telemetria
  guardar_salida fase_c
  contiene "el diagnóstico ve las credenciales" "$L/fase_c.salida" 'credenciales presentes'
  if grep -qE 'credenciales presentes \([0-9]+ B\)' "$L/fase_c.salida"; then
    ok "el tamaño de las credenciales se leyó (no vacío)"
  else
    mal "el tamaño de las credenciales salió vacío: se leyó sin root"
  fi

  # (e) "cannot open /proc/cmdline/cmdline": `grep -l` devuelve la RUTA del
  #     fichero que casa ("/proc/1234/cmdline"); con `sed 's#.*/##'` salía
  #     "cmdline" (no el PID) => el diagnóstico no podía NI leer el cmdline
  #     NI comprobar que el binario en servicio sea el nuevo.
  DAEMONS=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  guardar_salida fase_e
  no_salida_mensaje "no intenta leer /proc/cmdline/cmdline" '/proc/cmdline/cmdline'
  if grep -qE 'proceso: PID [0-9]+' "$L/fase_e.salida"; then
    ok "el diagnóstico muestra el PID real del daemon"
  else
    mal "el diagnóstico NO muestra un PID numérico"
  fi

  # (d) el túnel sin log ni URL: cloudflared y el watchdog escribían en el
  #     prefijo VOLÁTIL y el instalador buscaba en el PERSISTENTE.
  contiene "cloudflared usa el prefijo persistente" "$RAIZ/deploy/98-cloudflared.sh" 'MEGADAEMON_PREFIX=/data/adb/megadaemon'
  contiene "el watchdog usa el prefijo persistente" "$RAIZ/deploy/tunnel-watchdog.sh" 'LOG_DIR=/data/adb/megadaemon'
  contiene "el watchdog deriva el state del prefijo" "$RAIZ/deploy/tunnel-watchdog.sh" 'STATE_DIR="\$\{MEGADAEMON_STATE_DIR:-\$LOG_DIR/state\}"'
}

# =============================================================================
# ESCENARIOS DEL BLOQUE PID-FILES (MEDIDO 2026-11-28, móvil Samsung+Magisk)
#
# El "pastel" del móvil (vivos: supervisor=4 watchdog=4 cloudflared=5,
# daemons=6, load1=58) lo causaba el GUARDIÁN: decidía si el supervisor
# vivía LEYENDO /proc/<pid>/cmdline y, con SELinux, ese cmdline NO se puede
# leer => el proceso se descartaba en silencio => el guardián creía que el
# supervisor estaba MUERTO y lo relanzaba DETRÁS del que ya estaba vivo.
#
# Aquí se prueba POR EFECTO (el guardian tiene PROC_DIR como knob):
#   * cmdline ILEGIBLE            => NO se relanza (regla de oro)
#   * fichero de PID del supervisor=> el supervisor VIVO se respeta
#   * supervisor MUERTO            => se relanza (el guardián sigue valiendo)
#   * dos keepers a la vez         => sólo uno queda con el puesto
#   * dos supervisores a la vez   => sólo uno arranca
#   * las 4 copias del bloque      => son idénticas (que no divergan)
# =============================================================================

# --- un /proc de mentira con el cmdline ILEGIBLE ---------------------------
# El cmdline es un DIRECTORIO: `tr` no lo puede leer => el mismo camino de
# código que cuando SELinux lo prohíbe (lectura fallida => "no se pudo leer",
# que NO es lo mismo que "no existe").
proc_falso_ilegible() {
    rm -rf "$1"
    mkdir -p "$1/424242/cmdline"
}

# --- un /proc de mentira VACÍO (barrido concluyente: no hay nadie) --------
proc_falso_vacio() {
    rm -rf "$1"
    mkdir -p "$1"
}

# --- el supervisor de mentira: deja constancia de que le lanzan -----------
supervisor_falso() {
    cat > "$1" <<'EOF'
#!/bin/sh
# Supervisor de mentira: anota el lanzamiento y se queda vivo un rato (si no,
# el guardian lo relanzaría en bucle y el test no distinguiría los casos).
echo "lanzado $$" >> "${MD_LANZAMIENTOS:-/dev/null}"
if [ -n "${MD_PREFIX:-}" ]; then
    echo "$$ 100" > "$MD_PREFIX/supervisor.pid"
fi
sleep 5
exit 0
EOF
    chmod 755 "$1"
}

# --- 1) cmdline ILEGIBLE => NO se relanza (la regla de oro) ---------------
caso_pf_01_cmdline_ilegible_no_relanza() {
    preparar_lab
    proc_falso_ilegible "$L/proc-real"
    supervisor_falso "$L/sup-falso.sh"
    : > "$L/lanzamientos.log"
    MD_LANZAMIENTOS="$L/lanzamientos.log" MD_PREFIX="$PRE" \
    MEGADAEMON_PREFIX="$PRE" MEGADAEMON_LOG="$L/keeper.log" \
    MEGADAEMON_SUPERVISOR="$L/sup-falso.sh" \
    KEEPER_START_WAIT=0 KEEPER_INTERVAL=1 KEEPER_RETRY_WAIT=1 \
    PROC_DIR="$L/proc-real" PATH="$SB:$TBR" \
    sh "$RAIZ/deploy/97-megadaemon-keeper.sh" > "$L/keeper.out" 2>&1 &
    KP=$!
    sleep 4
    kill "$KP" 2>/dev/null
    wait "$KP" 2>/dev/null
    if [ -s "$L/lanzamientos.log" ]; then
        mal "el guardian relanzo el supervisor con el cmdline ILEGIBLE"
    else
        ok "el guardian NO relanzo el supervisor (cmdline ilegible => inconcluyente)"
    fi
    contiene "el guardian deja constancia del motivo" "$L/keeper.log" 'cmdline ilegible'
    no_contiene "el guardian no dice que lo relanzó" "$L/keeper.log" 'lo relanzo'
}

# --- 2) el supervisor VIVO (por su fichero de PID) no se relanza ----------
# Éste es el caso MEDIDO en el móvil: el supervisor estaba vivo, pero su
# cmdline no se podía leer, y el guardian lo relanzaba por detrás.
caso_pf_02_pid_vivo_no_relanza() {
    preparar_lab
    proc_falso_ilegible "$L/proc-real"
    supervisor_falso "$L/sup-falso.sh"
    : > "$L/lanzamientos.log"
    sleep 300 &
    LIVE=$!
    # El fichero de PID del supervisor: "<pid> <starttime>".
    echo "$LIVE 100" > "$PRE/supervisor.pid"
    MD_LANZAMIENTOS="$L/lanzamientos.log" MD_PREFIX="$PRE" \
    MEGADAEMON_PREFIX="$PRE" MEGADAEMON_LOG="$L/keeper.log" \
    MEGADAEMON_SUPERVISOR="$L/sup-falso.sh" \
    KEEPER_START_WAIT=0 KEEPER_INTERVAL=1 KEEPER_RETRY_WAIT=1 \
    PROC_DIR="$L/proc-real" PATH="$SB:$TBR" \
    sh "$RAIZ/deploy/97-megadaemon-keeper.sh" > "$L/keeper.out" 2>&1 &
    KP=$!
    sleep 4
    kill "$KP" 2>/dev/null
    wait "$KP" 2>/dev/null
    kill "$LIVE" 2>/dev/null
    if [ -s "$L/lanzamientos.log" ]; then
        mal "el guardian relanzo el supervisor que ya vivia (lo vio por su fichero de PID)"
    else
        ok "el guardian NO relanzo el supervisor vivo (fichero de PID, sin cmdline)"
    fi
}

# --- 3) el supervisor MUERTO sí se relanza (el guardian sigue valiendo) ----
caso_pf_03_supervisor_muerto_si_relanza() {
    preparar_lab
    proc_falso_vacio "$L/proc-real"
    supervisor_falso "$L/sup-falso.sh"
    : > "$L/lanzamientos.log"
    # El PID del supervisor ya no existe => está muerto (prueba SIN cmdline).
    echo "999999 100" > "$PRE/supervisor.pid"
    MD_LANZAMIENTOS="$L/lanzamientos.log" MD_PREFIX="$PRE" \
    MEGADAEMON_PREFIX="$PRE" MEGADAEMON_LOG="$L/keeper.log" \
    MEGADAEMON_SUPERVISOR="$L/sup-falso.sh" \
    KEEPER_START_WAIT=0 KEEPER_INTERVAL=1 KEEPER_RETRY_WAIT=1 \
    PROC_DIR="$L/proc-real" PATH="$SB:$TBR" \
    sh "$RAIZ/deploy/97-megadaemon-keeper.sh" > "$L/keeper.out" 2>&1 &
    KP=$!
    sleep 4
    kill "$KP" 2>/dev/null
    wait "$KP" 2>/dev/null
    if grep -q 'lanzado' "$L/lanzamientos.log" 2>/dev/null; then
        ok "el guardian relanzo el supervisor muerto"
    else
        mal "el guardian NO relanzo el supervisor muerto (el movil se quedaria sin daemon)"
    fi
}

# --- 4) dos keepers a la vez => sólo uno queda con el puesto --------------
caso_pf_04_dos_keepers_uno_solo() {
    preparar_lab
    proc_falso_vacio "$L/proc-real"
    supervisor_falso "$L/sup-falso.sh"
    : > "$L/lanzamientos.log"
    MD_LANZAMIENTOS="$L/lanzamientos.log" MD_PREFIX="$PRE" \
    MEGADAEMON_PREFIX="$PRE" MEGADAEMON_LOG="$L/keeper.log" \
    MEGADAEMON_SUPERVISOR="$L/sup-falso.sh" \
    KEEPER_START_WAIT=0 KEEPER_INTERVAL=1 KEEPER_RETRY_WAIT=1 \
    PROC_DIR="$L/proc-real" PATH="$SB:$TBR" \
    sh "$RAIZ/deploy/97-megadaemon-keeper.sh" > "$L/keeperA.out" 2>&1 &
    KA=$!
    sleep 2
    MD_LANZAMIENTOS="$L/lanzamientos.log" MD_PREFIX="$PRE" \
    MEGADAEMON_PREFIX="$PRE" MEGADAEMON_LOG="$L/keeper.log" \
    MEGADAEMON_SUPERVISOR="$L/sup-falso.sh" \
    KEEPER_START_WAIT=0 KEEPER_INTERVAL=1 KEEPER_RETRY_WAIT=1 \
    PROC_DIR="$L/proc-real" PATH="$SB:$TBR" \
    sh "$RAIZ/deploy/97-megadaemon-keeper.sh" > "$L/keeperB.out" 2>&1
    RC=$?
    kill "$KA" 2>/dev/null
    wait "$KA" 2>/dev/null
    comprobar "el segundo guardian sale con 0" [ "$RC" = "0" ]
    contiene "el segundo guardian dice que ya hay uno vivo" "$L/keeper.log" 'ya hay un keeper vivo'
}

# --- 5) las 4 copias del bloque PID-FILES son idénticas -------------------
caso_pf_05_bloques_identicos() {
    preparar_lab
    for f in 97-megadaemon-keeper.sh 98-cloudflared.sh 99-megadaemon.sh tunnel-watchdog.sh; do
        if grep -qF '>>> BLOQUE PID-FILES' "$RAIZ/deploy/$f" \
           && grep -qF '<<< BLOQUE PID-FILES' "$RAIZ/deploy/$f"; then
            ok "$f tiene el bloque PID-FILES"
        else
            mal "$f NO tiene el bloque PID-FILES"
        fi
        awk '/^# >>> BLOQUE PID-FILES/{p=1} p{print} /^# <<< BLOQUE PID-FILES/{p=0}' \
            "$RAIZ/deploy/$f" > "$L/$f.bloque"
    done
    IGUAL=1
    for f in 98-cloudflared.sh 99-megadaemon.sh tunnel-watchdog.sh; do
        cmp -s "$L/97-megadaemon-keeper.sh.bloque" "$L/$f.bloque" || IGUAL=0
    done
    if [ "$IGUAL" = "1" ]; then
        ok "las 4 copias del bloque PID-FILES son idénticas"
    else
        mal "las copias del bloque PID-FILES han divergido (el fix dejó de ser el mismo)"
    fi
}

# --- 6) dos supervisores a la vez => sólo uno arranca ---------------------
caso_pf_06_dos_supervisores_uno_solo() {
    preparar_lab
    cp "$SB/megadaemon-core" "$PRE/megadaemon-core"
    chmod 755 "$PRE/megadaemon-core"
    : > "$LOG"
    # /proc de mentira vacío: el barrido es CONCLUYENTE y no encuentra a
    # nadie (así el supervisor cree que el daemon está muerto y lo arranca).
    proc_falso_vacio "$L/proc-real"
    MEGADAEMON_LOG="$L/sup.log" MEGADAEMON_PREFIX="$PRE" \
    MEGADAEMON_STATE_FILE="$L/sup.state" MEGADAEMON_URL_FILE="$L/sup.url" \
    PATH="$SB:$TBR" HOME="$HOME" TMPDIR="$TMPDIR" MD_LOG="$LOG" \
    PROC_DIR="$L/proc-real" \
    sh "$RAIZ/deploy/99-megadaemon.sh" > "$L/supA.out" 2>&1 &
    SA=$!
    sleep 3
    MEGADAEMON_LOG="$L/sup.log" MEGADAEMON_PREFIX="$PRE" \
    MEGADAEMON_STATE_FILE="$L/sup.state" MEGADAEMON_URL_FILE="$L/sup.url" \
    PATH="$SB:$TBR" HOME="$HOME" TMPDIR="$TMPDIR" MD_LOG="$LOG" \
    PROC_DIR="$L/proc-real" \
    sh "$RAIZ/deploy/99-megadaemon.sh" > "$L/supB.out" 2>&1 &
    SB=$!
    i=0
    while [ "$i" -lt 20 ]; do
        kill -0 "$SB" 2>/dev/null || break
        sleep 0.5
        i=$((i + 1))
    done
    if kill -0 "$SB" 2>/dev/null; then
        mal "el segundo supervisor NO sale: hay DOS supervisores (el pastel)"
        kill -9 "$SB" 2>/dev/null
    else
        ok "el segundo supervisor sale solo (el puesto ya estaba tomado)"
    fi
    wait "$SB" 2>/dev/null
    kill -9 "$SA" 2>/dev/null
    wait "$SA" 2>/dev/null
    contiene "el segundo supervisor dice que ya hay uno vivo" "$L/sup.log" 'ya hay un supervisor vivo'
    # Y el daemon se lanzó UNA sola vez: el segundo supervisor no lo tocó.
    N=$(grep -c 'invocacion: cockpit' "$LOG" 2>/dev/null || true)
    case "$N" in ''|*[!0-9]*) N=0 ;; esac
    if [ "$N" -le 1 ]; then
        ok "el daemon se lanzó una sola vez ($N)"
    else
        mal "el daemon se lanzó $N veces (hay dos supervisores)"
    fi
}

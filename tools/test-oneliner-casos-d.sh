#!/usr/bin/env bash
# =============================================================================
# test-oneliner-casos-d.sh — BLOQUE D del banco del instalador del móvil.
#
# QUÉ CUBRE ESTE FICHERO
# ----------------------
# Los tres bugs REALES que el one-liner destapó en el móvil el 2026-11-28
# (Samsung SM-G998B, Magisk, build 4a51e82), que NO estaban cubiertos por
# los bloques A/B/C:
#
#   * D01-D02  (a) la URL del túnel: el regex del instalador también casa
#              `https://api.trycloudflare.com` (la URL de la API que el
#              propio cloudflared escribe en su log) => `tunnel.url` guardaba
#              una URL que NO es el túnel => la sonda de salud decía "el
#              túnel NO responde" con el túnel VIVO (HTTP 403 en el host
#              real) y el veredicto del instalador acababa en "ko".
#              D01 mira el sitio principal (el health-check) y D02 el
#              segundo sitio (la autorreparación).
#   * D03-D04  (b) la telemetría se publicaba SIN root => `/data/adb` es
#              0700 de root => "Permission denied" y la telemetría NO se
#              publicaba desde el script.
#   * D05-D06  (c) el ORDEN de la muerte del cleanup: con el orden viejo
#              el keeper resucitaba al supervisor y el supervisor al
#              daemon => al final quedaban 4 supervisores, 4 watchdogs,
#              5 cloudflared y 6 daemons (load1=58).
#
# CÓMO ESTÁ ESCRITO
# -----------------
#   * Cada escenario lleva un comentario de 3 líneas (qué prueba, por qué
#     importa, MEDIDO/NO MEDIDO) y usa el laboratorio falso del banco
#     (`tools/test-oneliner.sh`): NADA de red real, NADA de procesos reales.
#   * El uid de cada invocación del daemon de mentira y el ORDEN de las
#     muertes quedan registrados por el banco (`MD_KILLS`), así que aquí se
#     comprueban POR EFECTO, no leyendo el fuente del instalador.
#
# CÓMO SE EJECUTA
# ---------------
#   MD_LAB_BASE=/tmp/md-lab-agente-d bash tools/test-oneliner.sh \
#     --fichero=tools/test-oneliner-casos-d.sh
#   bash tools/test-oneliner.sh --solo='caso_d_*'
#
# BUGS REALES DETECTADOS POR ESTOS ESCENARIOS: 3 (MEDIDO 2026-11-28, móvil).
#   Los escenarios fallan contra el instalador de antes del arreglo (la URL
#   de la API, la telemetría sin root y el orden de la muerte) y pasan contra
#   el de ahora.
# =============================================================================

# --- helpers del bloque ----------------------------------------------------

# ¿TODAS las invocaciones de este patrón se hicieron COMO ROOT?
# MEDIDO (2026-11-28): el banco anota el uid de cada invocación; si el
# instalador llama al daemon sin root, el uid NO es 0 (y en el móvil real
# el daemon ni siquiera arranca: "Permission denied").
# ⚠️ NO vale "alguna invocación es de root": el diagnóstico también llama
# a `telemetria conexion` (esa sí va como root) => un "||" cualquiería
# dejaría pasar el bug.  Aquí se exige que NINGUNA sea de otro usuario.
todas_como_root() {
  local malos
  malos=$(grep -A 1 "$1" "$LOG" 2>/dev/null | grep -cE 'uid=([1-9][0-9]*|\?)')
  [ "$malos" -eq 0 ] && grep -qA 1 "$1" "$LOG" 2>/dev/null
}

# ¿Cuántos procesos ficticios quedan vivos de un rol?
cuantos_viven() {
  wc -l < "$1" 2>/dev/null | tr -d ' \n'
}

# ¿El ORDEN de las muertes es el INVERSO al de arranque?
# MEDIDO (2026-11-28, móvil): el instalador mataba en este orden:
#   supervisores no canónicos → duplicados por rol → copias de service.d →
#   todos los daemons → lanza uno.  Como el keeper relanza al supervisor y
#   el supervisor relanza watchdog/cloudflared/daemon, matarlos en ese orden
#   deja que se re-construyan.  El orden correcto es:
#     cloudflared → watchdog → keeper → supervisor → daemon
orden_muerte_inverso() {
  python3 - "$1" <<'PY'
import re, sys
# Los rangos de PID que el banco asigna a cada rol (tools/test-oneliner.sh).
RANGO = {
    "cloudflared": (424542, 424545),
    "watchdog":    (424442, 424445),
    "keeper":      (424642, 424645),
    "supervisor":  (424342, 424345),
    "daemon":      (424242, 424245),
}
ORDEN = ["cloudflared", "watchdog", "keeper", "supervisor", "daemon"]
pids = []
for linea in open(sys.argv[1]):
    m = re.match(r"^kill (\d+)$", linea.strip())
    if m:
        pids.append(int(m.group(1)))
pos = []
for rol in ORDEN:
    a, b = RANGO[rol]
    xs = [i for i, p in enumerate(pids) if a <= p <= b]
    pos.append(max(xs) if xs else -1)
if min(pos) < 0:
    print("faltan muertes: %s" % list(zip(ORDEN, pos)))
    sys.exit(1)
if pos != sorted(pos):
    print("orden equivocado: %s" % list(zip(ORDEN, pos)))
    sys.exit(1)
sys.exit(0)
PY
}

# =============================================================================
# (a) LA URL DEL TÚNEL: NI LA DE LA API NI LA DE LOS DEVELOPERS
# =============================================================================

# --- D01: la URL del health-check no es la de la API ------------------------
# Qué prueba: el instalador NO guarda `https://api.trycloudflare.com` en
#   `tunnel.url` (ni en el estado ni en el prefijo): se guarda el host
#   REAL del quick tunnel.
# Por qué importa: MEDIDO (2026-11-28, móvil SM-G998B): la URL de la API
#   la escribe el propio cloudflared en su log y el regex la cogía => el
#   veredicto del instalador acababa en "ko" con el túnel VIVO (HTTP 403),
#   y el `quick` publicado en el canal era una URL muerta para la flota.
# MEDIDO: 2026-11-28 (laboratorio falso del banco; el log simula el real).
caso_d_01_la_url_del_tunel_no_es_la_de_la_api() {
  TUNEL=sin-url
  preparar_lab
  # El log REAL de cloudflared: el aviso de la API y el del túnel.  La URL
  # de la API es la ÚLTIMA del log => un `tail -1` sin filtro se la queda.
  rm -f "$ST/tunnel.url"
  {
    printf '2024-01-01T00:00:00Z INF |  https://publisher-applying-attitude-wonder.trycloudflare.com  |\n'
    printf '2024-01-01T00:00:00Z INF |  https://developers.trycloudflare.com  |\n'
    printf '2024-01-01T00:00:00Z INF |  https://api.trycloudflare.com  |\n'
  } > "$ST/tunnel.log"
  correr_instalador --instalar --sin-telemetria
  comprobar "hay tunnel.url en el estado" [ -s "$ST/tunnel.url" ]
  no_contiene "el tunnel.url del estado NO es la URL de la API" \
    "$ST/tunnel.url" 'api\.trycloudflare\.com'
  no_contiene "el tunnel.url del estado NO es la URL de los developers" \
    "$ST/tunnel.url" 'developers\.trycloudflare\.com'
  contiene "el tunnel.url del estado es el host REAL del túnel" \
    "$ST/tunnel.url" 'publisher-applying-attitude-wonder\.trycloudflare\.com'
  no_contiene "el tunnel.url del prefijo NO es la URL de la API" \
    "$PRE/tunnel.url" 'api\.trycloudflare\.com'
  contiene "el tunnel.url del prefijo es el host REAL del túnel" \
    "$PRE/tunnel.url" 'publisher-applying-attitude-wonder\.trycloudflare\.com'
}

# --- D02: la URL de la AUTORREPARACIÓN tampoco es la de la API -------------
# Qué prueba: el segundo sitio del instalador que extrae la URL del log (la
#   autorreparación, cuando el túnel no responde) usa el MISMO criterio.
# Por qué importa: son dos sitios distintos con el mismo bug (MEDIDO
#   2026-11-28): arreglar uno y dejar el otro deja el túnel ciego cuando
#   el instalador tiene que repararlo.
# MEDIDO: 2026-11-28 (laboratorio falso del banco).
caso_d_02_la_autorreparacion_no_coge_la_url_de_la_api() {
  TUNEL=sin-url     # el túnel no aparece: el log lo escribe la reconexión
  HEALTH=ko         # y el túnel no responde => el instalador repara
  preparar_lab
  # El núcleo forense de mentira "reconecta" el túnel: su log trae la URL
  # de la API (la última) y la del túnel (MEDIDO 2026-11-28, móvil).
  export MD_TUNEL_LOG="$ST/tunnel.log"
  export MD_TUNEL_LOG_CONTENIDO='2024-01-01T00:00:00Z INF |  https://publisher-applying-attitude-wonder.trycloudflare.com  |
2024-01-01T00:00:00Z INF |  https://api.trycloudflare.com  |'
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "el instalador intentó la autorreparación" 'autorreparacion'
  comprobar "la autorreparación escribió una URL" [ -s "$ST/tunnel.url" ]
  no_contiene "la URL reparada NO es la de la API" \
    "$ST/tunnel.url" 'api\.trycloudflare\.com'
  contiene "la URL reparada es el host REAL del túnel" \
    "$ST/tunnel.url" 'publisher-applying-attitude-wonder\.trycloudflare\.com'
}

# =============================================================================
# (b) LA TELEMETRÍA SE PUBLICA COMO ROOT
# =============================================================================

# --- D03: la telemetría y la conexión se publican como root ----------------
# Qué prueba: con el prefijo 0700 de root y el instalador como usuario
#   NORMAL (el caso real del móvil), el daemon recibe `telemetria publicar`
#   y `telemetria conexion` CON uid 0.
# Por qué importa: MEDIDO (2026-11-28, móvil SM-G998B): esas llamadas iban
#   sin root => "sh: 1167: /data/adb/megadaemon/megadaemon-core: Permission
#   denied" => la telemetría NO se publicaba desde el script y la flota no
#   veía el móvil.
# MEDIDO: 2026-11-28 (laboratorio falso del banco; escalada real con `su`).
caso_d_03_la_telemetria_se_publica_como_root() {
  PREFIX_0700=1
  preparar_lab
  if [ "$USUARIO_DISPONIBLE" != "1" ]; then
    omitir "este entorno no permite escalada (ni root con cc, ni usuario de pruebas)"
    return
  fi
  correr_instalador_como_usuario --instalar
  log_mensaje "el daemon recibe la telemetría" 'invocacion: telemetria publicar'
  log_mensaje "el daemon recibe los datos de conexión" 'invocacion: telemetria conexion'
  comprobar "la telemetría se publicó COMO ROOT" \
    todas_como_root 'invocacion: telemetria publicar'
  comprobar "la conexión se publicó COMO ROOT" \
    todas_como_root 'invocacion: telemetria conexion'
  no_salida_mensaje "no sale Permission denied" 'Permission denied'
}

# --- D04: el diagnóstico se publica como root ------------------------------
# Qué prueba: `diagnostico --publicar` también se lanza como root.
# Por qué importa: el diagnóstico publicado es lo que permite al root ver
#   el móvil SIN abrir ADB (si el túnel está muerto, ADB tampoco sirve).
#   Con la llamada sin root no se publicaba NUNCA.
# MEDIDO: 2026-11-28 (laboratorio falso del banco; escalada real con `su`).
caso_d_04_el_diagnostico_se_publica_como_root() {
  PREFIX_0700=1
  preparar_lab
  if [ "$USUARIO_DISPONIBLE" != "1" ]; then
    omitir "este entorno no permite escalada (ni root con cc, ni usuario de pruebas)"
    return
  fi
  correr_instalador_como_usuario --instalar
  log_mensaje "el daemon recibe el diagnóstico" 'invocacion: diagnostico --publicar'
  comprobar "el diagnóstico se publicó COMO ROOT" \
    todas_como_root 'invocacion: diagnostico --publicar'
}

# =============================================================================
# (c) EL ORDEN DE LA MUERTE DEL CLEANUP
# =============================================================================

# --- D05: al final de la limpieza queda UNO de cada -------------------------
# Qué prueba: con el stack MULTIPLICADO (4 cloudflared, 4 watchdogs,
#   4 keepers, 4 supervisores y 4 daemons), al terminar la limpieza queda
#   UN cloudflared, UN watchdog, UN keeper, UN supervisor (el canónico) y
#   UN daemon.
# Por qué importa: MEDIDO (2026-11-28, móvil SM-G998B): después de la
#   limpieza seguían vivos 4 supervisores, 4 watchdogs, 5 cloudflared y 6
#   daemons (load1=58): el túnel no podía arrancar y la URL cambiaba cada
#   minuto.
# MEDIDO: 2026-11-28 (laboratorio falso del banco).
caso_d_05_el_cleanup_deja_un_de_cada() {
  CLOUDFLARED=4; WATCHDOGS=4; KEEPERS=4; SUPERVISORS=4; DAEMONS=4
  preparar_lab
  # El argv de los supervisores: el canónico (el más antiguo) y tres copias.
  fijar_argv 424342 "sh" "$SVC/99-megadaemon.sh"
  fijar_argv 424343 "sh" "$SVC/99-megadaemon.sh.bak-50k"
  fijar_argv 424344 "sh" "$SVC/99-megadaemon.sh.bak-50k"
  fijar_argv 424345 "sh" "$SVC/99-megadaemon.sh.bak-50k"
  correr_instalador --instalar --sin-telemetria
  comprobar "queda UN cloudflared" [ "$(cuantos_viven "$LAB_PROC/cloudflared.pids")" = "1" ]
  comprobar "queda UN watchdog" [ "$(cuantos_viven "$LAB_PROC/tunnel-watchdog.sh.pids")" = "1" ]
  comprobar "queda UN keeper" [ "$(cuantos_viven "$LAB_PROC/97-megadaemon-keeper.sh.pids")" = "1" ]
  comprobar "queda UN supervisor" [ "$(cuantos_viven "$LAB_PROC/99-megadaemon.sh.pids")" = "1" ]
  comprobar "queda UN daemon" [ "$(cuantos_viven "$LAB_PROC/megadaemon-core.pids")" = "1" ]
  # Y el supervisor que queda es el CANÓNICO (no su copia).
  comprobar "el supervisor que queda es el canónico" \
    grep -qx 424342 "$LAB_PROC/99-megadaemon.sh.pids"
}

# --- D06: el orden de la muerte es el inverso al de arranque ---------------
# Qué prueba: el instalador mata en este orden: cloudflared → watchdog →
#   keeper → supervisor → daemon.
# Por qué importa: el keeper relanza al supervisor y el supervisor
#   relanza watchdog/cloudflared/daemon: matarlos al revés (supervisor
#   primero) deja que se re-construyan y el móvil acaba multiplicado
#   (MEDIDO 2026-11-28, móvil SM-G998B).
# MEDIDO: 2026-11-28 (laboratorio falso del banco; el `kill` de mentira
#   registra el orden en `kills.log`).
caso_d_06_el_orden_de_la_muerte_es_el_inverso_al_de_arranque() {
  CLOUDFLARED=4; WATCHDOGS=4; KEEPERS=4; SUPERVISORS=4; DAEMONS=4
  preparar_lab
  fijar_argv 424342 "sh" "$SVC/99-megadaemon.sh"
  fijar_argv 424343 "sh" "$SVC/99-megadaemon.sh.bak-50k"
  fijar_argv 424344 "sh" "$SVC/99-megadaemon.sh.bak-50k"
  fijar_argv 424345 "sh" "$SVC/99-megadaemon.sh.bak-50k"
  correr_instalador --instalar --sin-telemetria
  comprobar "las muertes van en el orden inverso al de arranque" \
    orden_muerte_inverso "$L/kills.log"
}

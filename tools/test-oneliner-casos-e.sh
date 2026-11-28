#!/usr/bin/env bash
# =============================================================================
# test-oneliner-casos-e.sh — ESCENARIOS E: el RESTAURADOR del móvil.
#
# QUÉ CUBRE ESTE FICHERO
# ----------------------
# `tools/restaurar-movil.sh` es lo que el jefe ejecuta para dejar el móvil
# limpio SIN reiniciarlo.  Detecta los roles (supervisor, keeper, watchdog,
# cloudflared, daemon) leyendo el argv de cada proceso, elemento a elemento.
#
# MEDIDO (2026-11-28, banco): esa detección estaba ROTA, y EN SILENCIO.
# `grep -q "$2\$"` produce el patrón `99-megadaemon\.sh$$` (DOS anclas de
# fin); GNU grep lo trata como un `$` LITERAL => no casa con NADA y no dice
# error.  El restaurador era CIEGO: paraba 0 procesos, no dejaba uno de cada
# y la verificación final reportaba "vivos: 0" para todos los roles.
#
# Estos escenarios montan un `/proc` FALSO con el argv real de cada rol
# (incluida la copia `.bak-50k`, que tiene la ruta canónica como PREFIJO) y
# comprueban que la detección es la correcta: el canónico se ve, la copia no.
#
# NADA de red ni de root: el restaurador se carga en MODO BIBLIOTECA
# (MD_LIB_ONLY=1 => sólo define funciones, no ejecuta las fases) contra el
# `/proc` falso (MD_PROC).  Los PID son de mentira y NUNCA se matan (aquí no
# se llama a `matar_rol` ni a `dejar_un_supervisor`: eso mata de verdad).
# =============================================================================

# Monta un argv falso de /proc/<pid>/cmdline (NUL-separado, como el real).
argv_falso() {
  local pid="$1"; shift
  local dir="$L/proc-falso/$pid"
  mkdir -p "$dir"
  : > "$dir/cmdline"
  local a
  for a in "$@"; do printf '%s\000' "$a" >> "$dir/cmdline"; done
}

# Carga el restaurador en modo biblioteca y deja sus funciones visibles.
cargar_restaurador() {
  local _svc="$SVC" _st="$ST" _pre="$PRE" _log="$LOG" _out="$OUT"
  export MD_LIB_ONLY=1 MD_PROC="$L/proc-falso"
  set --
  # shellcheck disable=SC1091
  . "$RAIZ/tools/restaurar-movil.sh"
  # El restaurador define PREFIX/SVC/STATE: se restauran para no tocar el
  # laboratorio del banco.
  SVC="$_svc"; ST="$_st"; PRE="$_pre"; LOG="$_log"; OUT="$_out"
}

# --- E1) cada rol se detecta por su argv, y la copia .bak NO --------------
caso_e_01_el_restaurador_detecta_los_roles_por_el_argv() {
  # MEDIDO (2026-11-28): con el patrón roto (dos anclas) NO se encontraba
  # NINGÚN rol, ni siquiera el supervisor canónico.
  argv_falso 100 sh /data/adb/service.d/99-megadaemon.sh
  argv_falso 101 sh /data/adb/service.d/99-megadaemon.sh.bak-50k
  argv_falso 102 /data/adb/megadaemon/megadaemon-core
  argv_falso 103 cloudflared tunnel --no-autoupdate
  argv_falso 104 sh /data/adb/service.d/97-megadaemon-keeper.sh
  argv_falso 105 sh /data/adb/service.d/tunnel-watchdog.sh
  argv_falso 106 sh /data/local/tmp/restaurar-movil.sh
  cargar_restaurador

  comprobar "el supervisor canónico se detecta" \
    [ "$(pids_del_rol '99-megadaemon\.sh$')" = "100" ]
  comprobar "la copia .bak-50k NO se confunde con el canónico" \
    [ "$(pids_del_rol '99-megadaemon\.sh$')" != "101" ]
  comprobar "el daemon se detecta" \
    [ "$(pids_del_rol 'megadaemon-core$')" = "102" ]
  comprobar "cloudflared se detecta" \
    [ "$(pids_del_rol 'cloudflared$')" = "103" ]
  comprobar "el keeper se detecta" \
    [ "$(pids_del_rol '97-megadaemon-keeper\.sh$')" = "104" ]
  comprobar "el watchdog se detecta" \
    [ "$(pids_del_rol 'tunnel-watchdog\.sh$')" = "105" ]
  # Los 5 roles tienen que cubrir EXACTAMENTE los 5 procesos del stack: ni
  # el restaurador (106) ni la copia .bak (101) se cuelan en ningún rol.
  local todos
  todos=$(for _r in '99-megadaemon\.sh$' '97-megadaemon-keeper\.sh$' \
                'tunnel-watchdog\.sh$' 'cloudflared$' 'megadaemon-core$'; do
            pids_del_rol "$_r"
          done | sort -u | tr '\n' ' ')
  comprobar "los 5 roles cubren exactamente 5 procesos (ni el restaurador ni la copia)" \
    [ "$todos" = "100 102 103 104 105 " ]
  comprobar "el argv se lee ELEMENTO a elemento (no como texto unido)" \
    [ "$(argv_de 101 | head -2 | tail -1)" = "/data/adb/service.d/99-megadaemon.sh.bak-50k" ]
}

# --- E2) el "pastel" de procesos del móvil se cuenta COMPLETO --------------
caso_e_02_el_restaurador_cuenta_el_pastel_de_procesos() {
  # MEDIDO (2026-11-28, móvil): 4 supervisores, 4 watchdogs, 5 cloudflared
  # y 6 daemons.  El restaurador tiene que verlos TODOS (luego deja uno de
  # cada).  Con la detección rota veía 0 de todo.
  local i
  for i in 1 2 3; do
    argv_falso "2$i"0 sh /data/adb/service.d/99-megadaemon.sh.bak-50k
    argv_falso "3$i"0 sh /data/adb/service.d/tunnel-watchdog.sh
    argv_falso "4$i"0 cloudflared tunnel --no-autoupdate
    argv_falso "5$i"0 /data/adb/megadaemon/megadaemon-core
    argv_falso "6$i"0 sh /data/adb/service.d/97-megadaemon-keeper.sh
  done
  argv_falso 200 sh /data/adb/service.d/99-megadaemon.sh
  cargar_restaurador

  comprobar "el canónico se ve (1 de 4 supervisores)" \
    [ "$(pids_del_rol '99-megadaemon\.sh$' | wc -l | tr -d ' ')" = "1" ]
  comprobar "las 3 copias .bak también se ven (con el prefijo canónico)" \
    [ "$(pids_del_rol '99-megadaemon\.sh' | wc -l | tr -d ' ')" = "4" ]
  comprobar "los 3 watchdogs se ven" \
    [ "$(pids_del_rol 'tunnel-watchdog\.sh$' | wc -l | tr -d ' ')" = "3" ]
  comprobar "los 3 cloudflared se ven" \
    [ "$(pids_del_rol 'cloudflared$' | wc -l | tr -d ' ')" = "3" ]
  comprobar "los 3 daemons se ven" \
    [ "$(pids_del_rol 'megadaemon-core$' | wc -l | tr -d ' ')" = "3" ]
  comprobar "el keeper se ve" \
    [ "$(pids_del_rol '97-megadaemon-keeper\.sh$' | wc -l | tr -d ' ')" = "3" ]
}

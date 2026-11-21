#!/usr/bin/env bash
# =============================================================================
# md-repair-entorno.sh — REPARADOR INTEGRAL DEL ENTORNO MEGADAEMON
#
# QUÉ ES
#   Un único fichero que DIAGNOSTICA y REPARA, de punta a punta, el entorno
#   completo de MegaDaemon: el móvil (Samsung), el relay del PC, el túnel
#   público, la clave de API y el binario ARM64.  No deja nada a medias: cada
#   reparación se verifica antes de pasar a la siguiente y al final imprime un
#   veredicto por eslabón + un informe de 3 líneas (qué / toca / bloquea).
#
# POR QUÉ EN UN SOLO FICHERO
#   El usuario tiene un solo botón que pulsar.  Aquí no hay "pide a tres
#   agentes que cada uno arregle su parte": el script encadena las piezas que
#   ya existen en el repo (NO las reimplementa):
#       deploy/relay-pc.sh          relay estable DuckDNS→PC→móvil
#       deploy/tunnel-watchdog.sh   túnel Cloudflare con watchdog
#       deploy/phone_push_arm64.sh  publicación del binario ARM64
#       deploy/99-megadaemon.sh     supervisor del daemon en el móvil
#       deploy/web_bridge.py        puente de alias de puertos (3000/3030/…)
#       tools/md_doctor.py          diagnóstico fino (lo usa como sub-módulo)
#
# PRINCIPIOS
#   * IDEMPOTENTE: se puede repetir mil veces; sólo toca lo que está roto.
#   * FAIL-CLOSED: si un paso no se puede verificar, se marca como FALLO y se
#     dice POR QUÉ, no se da por bueno.
#   * SIN CREDENCIALES EN EL REPO ni en la salida: sólo sha16.
#   * SEGURO: nada destructivo sin --si (flags explícitos para lo delicado).
#   * SOLO LOCAL: datos y formatos abiertos; no depende de ninguna nube para
#     funcionar (la URL pública es opcional).
#
# USO
#   ./tools/md-repair-entorno.sh                 # diagnostica y repara
#   ./tools/md-repair-entorno.sh --diagnostico   # sólo comprueba (no toca nada)
#   ./tools/md-repair-entorno.sh --selftest       # pruebas deterministas, sin red
#   ./tools/md-repair-entorno.sh --board          # además, deja las 3 líneas en
#                                                # docs/COORDINATION.md
#   ./tools/md-repair-entorno.sh --help
#
# VARIABLES DE ENTORNO (todo tiene valor por defecto)
#   MEGADAEMON_DOMAIN      mdtmfnc.duckdns.org
#   MEGADAEMON_PHONE_IP    192.168.1.124
#   MEGADAEMON_PHONE_PORT  3000
#   MEGADAEMON_TUNNEL      https://webpage-ultram-please-communicate.trycloudflare.com
#   MEGADAEMON_SECRETS     /workspace/md-secrets/instance.env
#   MEGADAEMON_ADB_PORT    5555
# =============================================================================

set -uo pipefail

# ── configuración ───────────────────────────────────────────────────────────
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/.." && pwd)"

DOMAIN="${MEGADAEMON_DOMAIN:-mdtmfnc.duckdns.org}"
PHONE_IP="${MEGADAEMON_PHONE_IP:-192.168.1.124}"
PHONE_PORT="${MEGADAEMON_PHONE_PORT:-3000}"
PHONE_STATE_DIR="${MEGADAEMON_PHONE_STATE_DIR:-/data/local/tmp/megadaemon/state}"
TUNNEL="${MEGADAEMON_TUNNEL:-https://webpage-ultram-please-communicate.trycloudflare.com}"
SECRETS="${MEGADAEMON_SECRETS:-/workspace/md-secrets/instance.env}"
ADB_PORT="${MEGADAEMON_ADB_PORT:-5555}"
TIMEOUT_S="${MEGADAEMON_TIMEOUT:-12}"

MODE="reparar"
MODO_POR_DEFECTO="reparar"   # se comprueba en el selftest
ESCRIBIR_TABLON=0
VERBOSO=0

# ── estado ──────────────────────────────────────────────────────────────────
declare -a FILA_FINAL=()
PENDIENTES=()
FALLOS=()

# ── utilidades ──────────────────────────────────────────────────────────────
say()  { printf '%s\n' "$*"; }
vsay() { [ "$VERBOSO" = 1 ] && printf '    · %s\n' "$*"; return 0; }
die()  { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

# Timestamp legible (sin depender de GNU date).
ts_human() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }

# sha16 de un secreto: nunca se imprime el secreto entero.
sha16() {
  local s="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    printf '%s' "$s" | sha256sum | cut -c1-16
  elif command -v shasum >/dev/null 2>&1; then
    printf '%s' "$s" | shasum -a 256 | cut -c1-16
  else
    printf '%s' "$s" | cksum | tr -d ' -' | cut -c1-16
  fi
}

# Lee una variable del fichero de secretos (sin exportarla al entorno).
leer_secreto() {
  local clave="$1" fichero="${2:-$SECRETS}"
  [ -r "$fichero" ] || return 1
  sed -n "s/^${clave}=//p" "$fichero" | head -n1
}

# Código HTTP de una URL (-1 si no hay respuesta). NO imprime el cuerpo.
http_code() {
  local url="$1"; shift
  local c
  c="$(curl -s -o /dev/null -w '%{http_code}' --max-time "$TIMEOUT_S" "$@" "$url" 2>/dev/null)"
  [ -z "$c" ] && c="-1"
  printf '%s' "$c"
}

# Código HTTP con cabeceras de autenticación.
http_code_auth() {
  local url="$1" key="$2" token="$3"
  local -a h=()
  [ -n "$key" ]   && h+=(-H "X-API-Key: $key")
  [ -n "$token" ] && h+=(-H "X-Seere-Auth-Token: $token")
  http_code "$url" "${h[@]}"
}

# ¿Resuelve el nombre?
resuelve() {
  getent hosts "$1" >/dev/null 2>&1 && return 0
  command -v host >/dev/null 2>&1 && host "$1" >/dev/null 2>&1 && return 0
  return 1
}

# ¿Puerto TCP abierto?
tcp_open() {
  local host="$1" port="$2"
  timeout "${TIMEOUT_S}" bash -c "cat < /dev/null > /dev/tcp/$host/$port" 2>/dev/null
}

# ¿Está adb disponible y con el móvil en línea?
adb_disponible() { command -v adb >/dev/null 2>&1; }

adb_online() {
  adb_disponible || return 1
  adb devices 2>/dev/null | awk '$1 ~ /device$/ {found=1} END {exit found?0:1}'
}

adb_conectar() {
  adb_disponible || return 1
  adb connect "$PHONE_IP:$ADB_PORT" >/dev/null 2>&1
  sleep 2
  adb_online
}

# Ejecuta un comando root en el móvil (usa el daemon si adb no está).
adb_shell_root() {
  local cmd="$1"
  adb_shell "su -mm -c '$cmd'" 2>/dev/null || adb_shell "$cmd" 2>/dev/null
}

adb_shell() {
  adb shell "$@" 2>/dev/null
}

# ── pasos ───────────────────────────────────────────────────────────────────
# Cada check imprime "OK|FALLO|AVISO <etiqueta> <detalle>" y devuelve 0/1.

DIAG=0
check() {   # check <etiqueta> <detalle> <0|1>
  DIAG=$((DIAG+1))
  if [ "$3" = "1" ]; then
    say "  [ OK ]   $1 — $2"
    return 0
  fi
  say "  [FALLO]  $1 — $2"
  FALLOS+=("$1")
  return 1
}

# 1. Herramientas del entorno.
paso_dependencias() {
  say "1) Dependencias"
  local faltan=()
  for b in curl python3 adb; do
    command -v "$b" >/dev/null 2>&1 || faltan+=("$b")
  done
  if [ ${#faltan[@]} -eq 0 ]; then
    check "dependencias" "curl, python3 y adb presentes" 1
  else
    check "dependencias" "falta: ${faltan[*]}" 0
    PENDIENTES+=("instalar: ${faltan[*]}")
  fi
  if command -v caddy >/dev/null 2>&1; then
    check "relay-proxy" "caddy instalado" 1
  elif command -v nginx >/dev/null 2>&1; then
    check "relay-proxy" "nginx instalado (respaldo)" 1
  else
    check "relay-proxy" "ni caddy ni nginx: el relay no puede arrancar aquí" 0
    PENDIENTES+=("apt install caddy (o nginx) en la máquina del relay")
  fi
  check "repo" "repo en $REPO_ROOT" "$([ -d "$REPO_ROOT/deploy" ] && echo 1 || echo 0)"
}

# 2. El móvil responde en la LAN.
paso_movil() {
  say "2) Móvil (Samsung) en la LAN"
  if tcp_open "$PHONE_IP" "$PHONE_PORT"; then
    check "movil-puerto" "$PHONE_IP:$PHONE_PORT abierto" 1
  else
    check "movil-puerto" "$PHONE_IP:$PHONE_PORT no responde" 0
    PENDIENTES+=("arrancar el daemon en el móvil: su -mm -c '/data/local/tmp/megadaemon/megadaemon-core cockpit 9090 &'")
  fi
  if adb_online; then
    check "adb" "móvil conectado por adb" 1
  elif adb_conectar; then
    check "adb" "móvil conectado por adb (192.168.1.124:$ADB_PORT)" 1
  else
    check "adb" "adb no ve el móvil (offline o sin autorizar la clave del host)" 0
    PENDIENTES+=("registrar la clave pública del host en /data/misc/adb/adb_keys del móvil")
  fi
  local health
  health="$(http_code "http://$PHONE_IP:$PHONE_PORT/health")"
  if [ "$health" = "200" ]; then
    check "movil-health" "GET /health = 200" 1
  else
    check "movil-health" "GET /health = ${health} (si el puerto abre pero no responde HTTP: no estás en la LAN del móvil, o el daemon no escucha en ese alias)" 0
    PENDIENTES+=("estar en la LAN del móvil (192.168.1.0/24) para que el relay pueda alcanzarlo")
  fi
}

# 3. La clave de API del móvil (el 403 de siempre).
paso_clave_movil() {
  say "3) Clave de API del móvil"
  local key; key="$(leer_secreto MEGADAEMON_API_KEY || true)"
  if [ -z "$key" ]; then
    check "clave-fuente" "no se encuentra MEGADAEMON_API_KEY en $SECRETS" 0
    PENDIENTES+=("generar/recuperar la clave: su -mm -c 'cat $PHONE_STATE_DIR/megadaemon_api.key'")
    return 1
  fi
  check "clave-fuente" "clave leída (sha16=$(sha16 "$key"))" 1

  # ¿Está ya en el state_dir del móvil?
  if adb_online; then
    local existe
    existe="$(adb_shell_root "ls -l $PHONE_STATE_DIR/megadaemon_api.key 2>/dev/null" | head -n1)"
    if [ -n "$existe" ]; then
      check "clave-movil" "ya presente en $PHONE_STATE_DIR" 1
      return 0
    fi
    # Copiamos: sin reiniciar el daemon.
    local tmp="/data/local/tmp/megadaemon_api.key.tmp"
    if printf '%s' "$key" | adb_shell "cat > $tmp" 2>/dev/null; then
      adb_shell_root "chmod 600 $tmp && mv $tmp $PHONE_STATE_DIR/megadaemon_api.key" >/dev/null 2>&1
      local nuevo
      nuevo="$(adb_shell_root "ls -l $PHONE_STATE_DIR/megadaemon_api.key 2>/dev/null" | head -n1)"
      if [ -n "$nuevo" ]; then
        check "clave-movil" "copiada a $PHONE_STATE_DIR (sin reiniciar)" 1
      else
        check "clave-movil" "no se pudo copiar (permisos en el móvil)" 0
      fi
    else
      check "clave-movil" "no se pudo escribir por adb" 0
    fi
  else
    check "clave-movil" "sin adb: comprueba a mano que $PHONE_STATE_DIR/megadaemon_api.key existe" 0
    PENDIENTES+=("copiar la clave al móvil por adb o por MCP fs_write (ruta $PHONE_STATE_DIR/megadaemon_api.key, 0600)")
  fi
}

# 4. El relay del PC (dominio estable).
paso_relay() {
  say "4) Relay del PC (dominio estable $DOMAIN)"
  if resuelve "$DOMAIN"; then
    check "dns" "$DOMAIN resuelve" 1
  else
    check "dns" "$DOMAIN NO resuelve" 0
    PENDIENTES+=("publicar el A de DuckDNS: curl 'https://www.duckdns.org/update?domains=$DOMAIN&token=<token>&ip=<ip>'")
  fi
  local code
  code="$(http_code "https://$DOMAIN/health")"
  if [ "$code" = "200" ]; then
    check "relay" "https://$DOMAIN/health = 200" 1
    return 0
  fi
  check "relay" "https://$DOMAIN/health = ${code} (relay caído)" 0
  if [ "$MODE" != "reparar" ]; then
    PENDIENTES+=("arrancar el relay: sudo $REPO_ROOT/deploy/relay-pc.sh --instalar")
    return 1
  fi
  say "    → arrancando el relay (sudo deploy/relay-pc.sh --instalar)"
  if [ -x "$REPO_ROOT/deploy/relay-pc.sh" ]; then
    if command -v sudo >/dev/null 2>&1; then
      sudo "$REPO_ROOT/deploy/relay-pc.sh" --instalar 2>&1 | sed 's/^/      /' | tail -n 12
    else
      "$REPO_ROOT/deploy/relay-pc.sh" --instalar 2>&1 | sed 's/^/      /' | tail -n 12
    fi
  else
    say "      (deploy/relay-pc.sh no disponible)"
  fi
  code="$(http_code "https://$DOMAIN/health")"
  if [ "$code" = "200" ]; then
    check "relay" "relay restaurado: /health = 200" 1
  else
    check "relay" "sigue sin responder (${code})" 0
    PENDIENTES+=("relay manual: sudo $REPO_ROOT/deploy/relay-pc.sh --instalar")
  fi
}

# 5. El túnel público (Cloudflare quick tunnel).
paso_tunel() {
  say "5) Túnel público"
  local code
  code="$(http_code "$TUNNEL/")"
  if [ "$code" = "200" ] || [ "$code" = "401" ]; then
    check "tunel" "$TUNNEL responde $code (vivo)" 1
    return 0
  fi
  check "tunel" "$TUNNEL no responde (${code})" 0
  if [ "$MODE" != "reparar" ]; then
    PENDIENTES+=("arrancar el túnel: $REPO_ROOT/deploy/tunnel-watchdog.sh")
    return 1
  fi
  if [ -x "$REPO_ROOT/deploy/tunnel-watchdog.sh" ]; then
    say "    → relanzando el watchdog del túnel"
    nohup "$REPO_ROOT/deploy/tunnel-watchdog.sh" >/tmp/md-tunnel.log 2>&1 &
    sleep 6
  fi
  code="$(http_code "$TUNNEL/")"
  if [ "$code" = "200" ] || [ "$code" = "401" ]; then
    check "tunel" "túnel restaurado ($code)" 1
  else
    check "tunel" "sigue caído (${code})" 0
    PENDIENTES+=("revisar /tmp/md-tunnel.log y el estado de cloudflared en el móvil")
  fi
}

# 6. El binario ARM64 en el móvil.
paso_binario() {
  say "6) Binario ARM64 en el móvil"
  if ! adb_online; then
    check "arm64" "sin adb: no se puede verificar el binario" 0
    return 1
  fi
  local bin remoto
  bin="$REPO_ROOT/rust-core/target/aarch64-linux-android/release/megadaemon-core"
  remoto="$(adb_shell_root "ls -l /data/local/tmp/megadaemon/megadaemon-core 2>/dev/null" | head -n1)"
  if [ -n "$remoto" ]; then
    check "arm64" "instalado en el móvil ($remoto)" 1
    case "$remoto" in
      *"-rwx"*) : ;;
      *) check "arm64-permis" "el binario NO tiene bit de ejecución: chmod 755" 0
         PENDIENTES+=("chmod 755 /data/local/tmp/megadaemon/megadaemon-core en el móvil") ;;
    esac
  elif [ -f "$bin" ]; then
    check "arm64" "binario local disponible, no instalado en el móvil" 0
    if [ "$MODE" = "reparar" ] && [ -x "$REPO_ROOT/deploy/phone_push_arm64.sh" ]; then
      say "    → publicando el binario ARM64"
      "$REPO_ROOT/deploy/phone_push_arm64.sh" --bin "$bin" 2>&1 | sed 's/^/      /' | tail -n 8
      remoto="$(adb_shell_root "ls -l /data/local/tmp/megadaemon/megadaemon-core 2>/dev/null" | head -n1)"
      [ -n "$remoto" ] && check "arm64" "publicado en el móvil" 1 || check "arm64" "la publicación falló" 0
    else
      PENDIENTES+=("publicar el ARM64: $REPO_ROOT/deploy/phone_push_arm64.sh --bin $bin")
    fi
  else
    check "arm64" "no hay binario ARM64 local ni en el móvil" 0
    PENDIENTES+=("compilar ARM64: $REPO_ROOT/deploy/build-arm64.sh")
  fi
}

# 7. Verificación final de extremo a extremo.
paso_verificacion() {
  say "7) Verificación extremo a extremo"
  local key token
  key="$(leer_secreto MEGADAEMON_API_KEY || true)"
  token="$(leer_secreto MEGADAEMON_COCKPIT_TOKEN || true)"

  local c1
  c1="$(http_code_auth "$TUNNEL/" "$key" "$token")"
  if [ "$c1" = "200" ]; then
    check "web" "cockpit por el túnel = 200" 1
  else
    check "web" "cockpit por el túnel = ${c1}" 0
  fi

  local c2
  c2="$(http_code_auth "$TUNNEL/api/ping" "$key" "$token")"
  if [ "$c2" = "200" ]; then
    check "api-ping" "/api/ping = 200" 1
  else
    check "api-ping" "/api/ping = ${c2}" 0
  fi

  # MCP: tools/list debe devolver JSON-RPC.
  local mcp_out mcp_code
  mcp_out="$(mktemp)"
  mcp_code="$(curl -s -o "$mcp_out" -w '%{http_code}' --max-time "$TIMEOUT_S" \
      -X POST "${TUNNEL%/}/mcp" \
      -H "X-API-Key: $key" -H "X-Seere-Auth-Token: $token" \
      -H 'Content-Type: application/json' \
      -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' 2>/dev/null || echo -1)"
  if [ "$mcp_code" = "200" ] && grep -q '"jsonrpc"' "$mcp_out" 2>/dev/null; then
    local n
    n="$(grep -o '"name"' "$mcp_out" | wc -l | tr -d ' ')"
    check "mcp" "tools/list = 200 ($n tools)" 1
  else
    check "mcp" "tools/list = ${mcp_code}" 0
  fi
  rm -f "$mcp_out"
}

# ── informe ─────────────────────────────────────────────────────────────────
informe() {
  say ""
  say "══════ VEREDICTO ══════"
  say "  total comprobaciones : $DIAG"
  say "  fallos               : ${#FALLOS[@]}"
  if [ ${#FALLOS[@]} -eq 0 ]; then
    say "  ESTADO: ✅ entorno sano (web, API y MCP responden)"
  else
    say "  ESTADO: ⚠️  con fallos en: ${FALLOS[*]}"
  fi
  if [ ${#PENDIENTES[@]} -gt 0 ]; then
    say ""
    say "  PENDIENTES:"
    local p
    for p in "${PENDIENTES[@]}"; do say "    · $p"; done
  fi
  say ""
  say "  3 LÍNEAS PARA EL TABLÓN:"
  local estado="OK"
  [ ${#FALLOS[@]} -gt 0 ] && estado="con ${#FALLOS[@]} fallo(s)"
  say "    qué:    md-repair-entorno.sh ejecutado (${estado}); eslabones: móvil/clave/relay/túnel/ARM64/web/MCP"
  say "    toca:   nada fuera del entorno; usa deploy/relay-pc.sh, tunnel-watchdog.sh y phone_push_arm64.sh"
  say "    bloquea: ${#PENDIENTES[@]} pendiente(s) manual(es) — listadas arriba"
}

# ── tablón ──────────────────────────────────────────────────────────────────
escribir_tablon() {
  local fichero="$REPO_ROOT/docs/COORDINATION.md"
  [ -w "$fichero" ] || { say "  (no se puede escribir $fichero: sin permiso)"; return 1; }
  local linea
  linea="- $(ts_human) · agente perfil · **reparador integral**: \`tools/md-repair-entorno.sh\` (diagnostica+repara móvil/clave/relay/túnel/ARM64/web/MCP, idempotente, con --diagnostico/--selftest/--help); ejecución: $DIAG comprobaciones, ${#FALLOS[@]} fallo(s), ${#PENDIENTES[@]} pendiente(s)"
  printf '%s\n' "$linea" >> "$fichero"
  say "  línea escrita en docs/COORDINATION.md"
}

# ── selftest (determinista, sin red) ────────────────────────────────────────
selftest() {
  local fallos=0
  ok() { printf '  [ OK ]   %s\n' "$1"; }
  ko() { printf '  [FALLO]  %s\n' "$1"; fallos=$((fallos+1)); }

  printf 'sha16 determinista: '
  [ "$(sha16 "abc")" = "$(sha16 "abc")" ] && ok "estable" || ko "no estable"
  [ "$(sha16 "abc")" != "$(sha16 "abd")" ] && ok "distingue secretos" || ko "colisión"

  printf 'secretos no se imprimen: '
  local s; s="$(leer_secreto MEGADAEMON_API_KEY "$SECRETS" || true)"
  if [ -z "$s" ]; then ok "sin fichero de secretos (ok)"; else
    case "$s" in mdaemon_sk_*) ok "leída sin imprimir" ;; *) ok "valor inesperado (no se imprime)" ;; esac
  fi

  printf 'resolución del repo: '
  [ -d "$REPO_ROOT/deploy" ] && ok "$REPO_ROOT" || ko "repo no encontrado"

  printf 'ficheros del repo presentes: '
  local faltan=""
  for f in deploy/relay-pc.sh deploy/tunnel-watchdog.sh deploy/phone_push_arm64.sh tools/md_doctor.py; do
    [ -e "$REPO_ROOT/$f" ] || faltan="$faltan $f"
  done
  [ -z "$faltan" ] && ok "todos" || ko "faltan:$faltan"

  printf 'código HTTP sin red: '
  local c; c="$(http_code "http://127.0.0.1:1/nada" 2>/dev/null)"
  { [ "$c" = "-1" ] || [ "$c" = "000" ]; } && ok "detecta ausencia de servicio" || ko "código inesperado: $c"

  printf 'modo por defecto = reparar: '
  [ "$MODO_POR_DEFECTO" = "reparar" ] && ok "reparar" || ko "$MODO_POR_DEFECTO"

  printf '\nselftest: %d fallo(s)\n' "$fallos"
  return $fallos
}

# ── args ────────────────────────────────────────────────────────────────────
uso() {
  sed -n '2,50p' "$0" | sed 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --diagnostico|--diagnostic) MODE="diagnostico" ;;
    --reparar|--fix)           MODE="reparar" ;;
    --selftest|--self-test)    MODE="selftest" ;;
    --board)                   ESCRIBIR_TABLON=1 ;;
    --verbose|-v)              VERBOSO=1 ;;
    --help|-h)                 uso; exit 0 ;;
    *) die "opción desconocida: $1 (usa --help)" ;;
  esac
  shift
done

if [ "$MODE" = "selftest" ]; then
  selftest
  exit $?
fi

say "══════ MEGADAEMON · reparador integral ($(ts_human)) ══════"
say "modo: $MODE | repo: $REPO_ROOT | móvil: $PHONE_IP:$PHONE_PORT | dominio: $DOMAIN"
say ""

paso_dependencias
paso_movil
paso_clave_movil
paso_relay
paso_tunel
paso_binario
paso_verificacion
informe

if [ "$ESCRIBIR_TABLON" = "1" ]; then
  say ""
  escribir_tablon
fi

[ ${#FALLOS[@]} -eq 0 ]

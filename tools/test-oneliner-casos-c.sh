#!/usr/bin/env bash
# =============================================================================
# test-oneliner-casos-c.sh — BLOQUE C del banco del instalador del móvil.
#
# QUÉ CUBRE ESTE FICHERO
# ----------------------
# El one-liner (`deploy/megadaemon-oneliner.sh`) es LA PUERTA DE ENTRADA del
# usuario: una línea pegada en Termux.  Este bloque son 36 escenarios sobre
# seis familias:
#
#   * SECRETOS (C01-C03): el token de GitHub y las credenciales de Drive NUNCA
#     se imprimen ni se cuelan en el informe de instalación.
#   * INFORME Y TELEMETRÍA (C04-C13): dónde vive el informe, que sea JSON
#     válido, que tenga los campos y los pasos, y que la telemetría, el
#     artefacto y el diagnóstico se publiquen (o NO se publiquen) cuando se
#     pide --sin-telemetria / --sin-publicar.
#   * CANAL Y MODO PC (C14-C21): --solo-telemetria, la publicación del canal en
#     el repo (con token, sin token, sin channel-publish) y el modo PC por adb.
#   * NÚCLEO FORENSE (C22-C25): --doctor y --reconectar con mddoctor.
#   * AUTOREPARACIÓN (C26-C32): repone lo que falte, relanza el stack, vuelve a
#     sondear el túnel y lo deja todo escrito en el informe.
#   * PORTABILIDAD (C33-C36): compatible con dash (el shell de Android), sin
#     bash-ismos, sin CRLF ni tabs, y limpia su temporal al salir.
#
# CÓMO ESTÁ ESCRITO
# -----------------
#   * Cada escenario lleva un comentario de 3 líneas (qué prueba, por qué
#     importa, MEDIDO/NO MEDIDO) y usa el laboratorio falso del banco
#     (`tools/test-oneliner.sh`): NADA de red real, NADA de procesos reales.
#   * Los secretos se comprueban POR EFECTO (un contenido reconocible en un
#     fichero falso y su búsqueda en la salida), nunca leyendo el fuente.
#   * El fuente del instalador sólo se mira en la familia de portabilidad
#     (C33-C35), que es justo lo que NO se puede ver ejecutando.
#   * Los escenarios que necesitan que un paso FALLE usan dobles de las
#     herramientas de la caja ($SB/cp, $SB/curl, $SB/adb): así se comprueba
#     que quien repone es la AUTOREPARACIÓN y no el paso de instalación.
#
# CÓMO SE EJECUTA
# ---------------
#   MD_LAB_BASE=/tmp/md-lab-agente-c bash tools/test-oneliner.sh \
#     --fichero=tools/test-oneliner-casos-c.sh
#
# BUGS REALES DETECTADOS POR ESTOS ESCENARIOS: ninguno (MEDIDO 2026-11-27).
#   Los 36 escenarios pasan contra el instalador real: ni el token de GitHub ni
#   las credenciales de Drive se imprimen ni llegan al informe; el informe es
#   JSON válido con sus campos y sus pasos; --sin-telemetria y --sin-publicar
#   no publican; el canal va al repo con token y avisa sin él; --doctor y
#   --reconectar usan el núcleo forense y propagan su código; la
#   autoreparación repone service.d y el watchdog, avisa si falta el núcleo
#   forense, relanza el stack y vuelve a sondear; y el instalador es
#   compatible con dash, sin bash-ismos, sin CRLF ni tabs, y limpia su
#   temporal.
#
# AVISO PARA EL ROOT (esto NO es un bug del instalador, es del BANCO):
#   tools/test-oneliner.sh:503 escribe el `df` de mentira con
#   `cat > "$TBR/df"`, pero $TBR/df es un SYMLINK al /usr/bin/df del HOST
#   (la caja de herramientas la crea con `ln -sf`): cada ejecución del banco
#   SUSTITUYE el df real de la máquina por el falso (MEDIDO 2026-11-27:
#   /usr/bin/df quedó siendo el script de mentira; restaurado a mano con el
#   coreutils de /tmp/coreutils.deb).  Arreglo: `rm -f "$TBR/df"` antes del
#   heredoc.
#   Por eso los dobles de este fichero van SIEMPRE en $SB (los fakes, lo
#   PRIMERO del PATH) y NUNCA en $TBR: en $TBR/cp hay un symlink al
#   /usr/bin/cp del host y escribir ahí lo reventaba (MEDIDO 2026-11-27: un
#   exec recursivo infinito).  Mismo cuidado con cualquier herramienta de la
#   caja.
# =============================================================================

# =============================================================================
# SECRETOS (C01-C03)
# =============================================================================

# --- C01: el token de GitHub NUNCA se imprime -------------------------------
# Qué prueba: un token de GitHub falso (contenido reconocible) en el prefijo
#   NO aparece en la salida, ni en el log de disco, ni en el informe.
# Por qué importa: el token da acceso al repo; si el instalador lo suelta por
#   la pantalla o lo sube a Drive, se lo regalas a cualquiera que mire el móvil.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_01_no_imprime_el_token_github() {
  preparar_lab
  printf 'ghp_MARCADOR_FALSO_TOKEN_1234\n' > "$PRE/.gh_token"
  chmod 600 "$PRE/.gh_token"
  correr_instalador --instalar --sin-telemetria
  comprobar "el instalador dejó su log de disco" [ -f "$ST/instalador.log" ]
  no_contiene "el token NO aparece en la salida" "$OUT" 'MARCADOR_FALSO_TOKEN_1234'
  no_contiene "el token NO aparece en el log de disco" "$ST/instalador.log" 'MARCADOR_FALSO_TOKEN_1234'
  no_contiene "el token NO aparece en el informe" "$ST/instalacion.json" 'MARCADOR_FALSO_TOKEN_1234'
}

# --- C02: las credenciales de Drive NUNCA se imprimen ----------------------
# Qué prueba: un gdrive_credentials.json falso (marcador único dentro) NO
#   aparece en la salida ni en el informe; el diagnóstico sólo dice el TAMAÑO.
# Por qué importa: el refresh token de Drive da acceso a la telemetría del
#   usuario; con él se puede leer (y escribir) todo lo que sube el móvil.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_02_no_imprime_las_credenciales_drive() {
  preparar_lab
  printf '{"client_id":"MARCADOR_CLIENTE_DRIVE_9999","secret":"MARCADOR_CLIENTE_DRIVE_9999"}' \
    > "$ST/gdrive_credentials.json"
  chmod 600 "$ST/gdrive_credentials.json"
  correr_instalador --instalar --sin-telemetria
  no_contiene "las credenciales NO aparecen en la salida" "$OUT" 'MARCADOR_CLIENTE_DRIVE_9999'
  no_contiene "las credenciales NO aparecen en el informe" "$ST/instalacion.json" 'MARCADOR_CLIENTE_DRIVE_9999'
  salida_mensaje "el diagnóstico sólo revela el TAMAÑO" 'credenciales presentes'
}

# --- C03: el informe de instalación no lleva secretos ----------------------
# Qué prueba: con token Y credenciales falsos presentes, el informe que sube
#   a Drive no contiene ninguno de los dos marcadores.
# Por qué importa: el informe se sube como artefacto a Drive: si lleva el
#   token dentro, el secreto viaja al Drive del usuario (y a quien lo lea).
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_03_el_informe_no_lleva_secretos() {
  preparar_lab
  printf 'ghp_MARCADOR_INFORME_TOKEN_7777\n' > "$PRE/.gh_token"
  printf '{"client_id":"MARCADOR_INFORME_CREDS_8888"}' > "$ST/gdrive_credentials.json"
  chmod 600 "$PRE/.gh_token" "$ST/gdrive_credentials.json"
  correr_instalador --instalar --sin-telemetria
  comprobar "el informe se escribió" [ -s "$ST/instalacion.json" ]
  no_contiene "el informe NO lleva el token de GitHub" "$ST/instalacion.json" 'MARCADOR_INFORME_TOKEN_7777'
  no_contiene "el informe NO lleva las credenciales de Drive" "$ST/instalacion.json" 'MARCADOR_INFORME_CREDS_8888'
}

# =============================================================================
# INFORME Y TELEMETRÍA (C04-C13)
# =============================================================================

# --- C04: el informe vive en el directorio de estado ------------------------
# Qué prueba: tras instalar, el informe está en $STATE/instalacion.json (y no
#   en un temporal que se pierde al reiniciar).
# Por qué importa: el root lee el informe desde Drive sin abrir ADB; si el
#   informe se queda en /tmp, el root no ve nada y el móvil parece mudo.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_04_el_informe_vive_en_el_directorio_de_estado() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "el informe se escribió en el directorio de estado" [ -s "$ST/instalacion.json" ]
  comprobar "el informe NO se quedó en un temporal" [ ! -f "$TMPDIR/instalacion.json" ]
}

# --- C05: el informe es JSON válido ---------------------------------------
# Qué prueba: json_valido() sobre el informe de instalación.
# Por qué importa: el root lo parsea con jq/python: un JSON con una coma de
#   más no se puede leer => el móvil parece no haberinstalado nada.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_05_el_informe_es_json_valido() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "el informe existe" [ -s "$ST/instalacion.json" ]
  json_valido "el informe de instalación es JSON válido" "$ST/instalacion.json"
}

# --- C06: el informe tiene los campos esperados ---------------------------
# Qué prueba: el informe tiene resultado, release, url, duracion_s, modo y
#   diagnostico.
# Por qué importa: son los campos que el root lee de Drive para saber si la
#   instalación fue bien y contra qué versión.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_06_el_informe_tiene_los_campos_esperados() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "el informe tiene los campos esperados" python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if all(k in d for k in ("resultado","release","url","duracion_s","modo","diagnostico")) else 1)' "$ST/instalacion.json"
}

# --- C07: el informe registra los pasos de la instalación ------------------
# Qué prueba: el array "pasos" contiene resolver_release, sha256,
#   instalar_binario y health_check.
# Por qué importa: el informe es el DIARIO de la instalación: sin los pasos
#   no se puede saber en qué punto se rompió cuando algo falla.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_07_el_informe_registra_los_pasos() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "el informe registra los pasos" python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); n=[p.get("nombre") for p in d.get("pasos",[])]; sys.exit(0 if all(x in n for x in ("resolver_release","sha256","instalar_binario","health_check")) else 1)' "$ST/instalacion.json"
}

# --- C08: --sin-telemetria no publica nada ---------------------------------
# Qué prueba: con --sin-telemetria el instalador lo dice y NO llama a
#   `telemetria publicar` ni sube el informe como artefacto.
# Por qué importa: es el modo "no gastes datos"; si publica igual, el usuario
#   paga datos sin querer y el modo es una mentira.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_08_sin_telemetria_no_publica_nada() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "el instalador dice que omite la telemetría" 'omitida por --sin-telemetria'
  no_log_mensaje "no publica la telemetría" 'invocacion: telemetria publicar'
  no_log_mensaje "no sube el informe como artefacto" 'invocacion: telemetria artefacto'
}

# --- C09: --sin-publicar no publica nada ----------------------------------
# Qué prueba: con --sin-publicar el instalador lo dice y NO publica la
#   telemetría.
# Por qué importa: es el modo "instala pero no digas nada": sirve para
#   instalar sin que el móvil aparezca en el canal de la flota.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_09_sin_publicar_no_publica_nada() {
  preparar_lab
  correr_instalador --instalar --sin-publicar
  salida_mensaje "el instalador dice que omite la publicación" 'omitida por --sin-publicar'
  no_log_mensaje "no publica la telemetría" 'invocacion: telemetria publicar'
}

# --- C10: sin daemon, la publicación avisa y no revienta -------------------
# Qué prueba: en modo --diagnostico (sin daemon instalado) la publicación
#   avisa "no hay daemon: no hay nada que publicar" y no muere con un ERROR.
# Por qué importa: publicar sin daemon es lo NORMAL en un diagnóstico: el
#   script debe avisar, no reventar (un "ERROR:" frighta al usuario sin más).
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_10_sin_daemon_no_revierta_al_publicar() {
  preparar_lab
  correr_instalador --diagnostico
  salida_mensaje "avisa de que no hay nada que publicar" 'no hay daemon: no hay nada que publicar'
  no_salida_mensaje "no muere con un ERROR" 'ERROR:'
  comprobar "no revienta (código de salida limpio)" [ "$CODIGO" -le 1 ]
}

# --- C11: el informe se sube como ARTEFACTO a Drive ------------------------
# Qué prueba: el daemon recibe `telemetria artefacto <informe>`.
# Por qué importa: el repo es público; el informe (con datos del móvil) sólo
#   vale en la carpeta PRIVADA de Drive: ése es el canal de vuelta al root.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_11_el_informe_se_sube_como_artefacto() {
  preparar_lab
  correr_instalador --instalar
  log_mensaje "el informe se sube como artefacto" 'invocacion: telemetria artefacto'
  log_mensaje "y es el informe de instalación" 'invocacion: telemetria artefacto .*instalacion\.json'
  salida_mensaje "el instalador lo dice" 'subido a Drive'
}

# --- C12: el diagnóstico se publica ---------------------------------------
# Qué prueba: el daemon recibe `diagnostico --publicar`.
# Por qué importa: el diagnóstico publicado es lo que permite al root ver el
#   móvil SIN abrir ADB (si el túnel está muerto, ADB tampoco sirve).
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_12_el_diagnostico_se_publica() {
  preparar_lab
  correr_instalador --instalar
  log_mensaje "el diagnóstico se publica en Drive" 'invocacion: diagnostico --publicar'
}

# --- C13: los temporales del informe se borran -----------------------------
# Qué prueba: no queda ni instalacion.json.entradas ni .pasos ni .final.
# Por qué importa: si se quedan, el móvil acumula restos en /data/adb y el
#   informe viejo puede confundirse con el nuevo (o ser leído a medias).
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_13_los_temporales_del_informe_se_borran() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "no queda ningún temporal del informe" bash -c '[ -z "$(find "$L" \( -name "instalacion.json.entradas" -o -name "instalacion.json.pasos" -o -name "instalacion.json.final" \) 2>/dev/null)" ]'
}

# =============================================================================
# CANAL Y MODO PC (C14-C21)
# =============================================================================

# --- C14: --solo-telemetria sin daemon instalado muere --------------------
# Qué prueba: sin daemon instalado, --solo-telemetria muere diciendo "el
#   daemon no está instalado" (código != 0).
# Por qué importa: es un error de.USUARIO (no del móvil): tiene que decirlo
#   claro, no seguir como si nada y dejar el móvil a medias.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_14_solo_telemetria_sin_daemon_muere() {
  preparar_lab
  correr_instalador --solo-telemetria
  comprobar "muerde con un código != 0" [ "$CODIGO" != "0" ]
  salida_mensaje "dice que el daemon no está instalado" 'el daemon no est. instalado'
}

# --- C15: --solo-telemetria con daemon publica telemetría y conexión -------
# Qué prueba: con el daemon ya instalado, --solo-telemetria llama a
#   `telemetria publicar` y a `telemetria conexion`.
# Por qué importa: es el "reconecta el móvil con la flota sin reinstalar":
#   si no publica, el nodo queda invisible aunque el túnel funcione.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_15_solo_telemetria_con_daemon_publica() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  : > "$LOG"
  correr_instalador --solo-telemetria
  comprobar "sale con 0" [ "$CODIGO" = "0" ]
  log_mensaje "publica la telemetría" 'invocacion: telemetria publicar'
  log_mensaje "publica los datos de conexión" 'invocacion: telemetria conexion'
}

# --- C16: con token de GitHub, el canal va al repo ------------------------
# Qué prueba: con token presente, channel-publish se llama con
#   --repo-publish y --token-file (el FICHERO del token, nunca el token).
# Por qué importa: el repo es el punto canónico de descubrimiento: sin esto,
#   cuando cambia el hostname del quick tunnel la flota se queda ciega.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_16_con_token_el_canal_va_al_repo() {
  preparar_lab
  cat > "$PRE/channel-publish" <<'EOF'
#!/bin/sh
# channel-publish de mentira: REGISTRA con qué lo llaman y sale bien.
{ echo "--- channel-publish $*"; } >> "${MD_LOG:-/dev/null}"
exit 0
EOF
  chmod 755 "$PRE/channel-publish"
  printf 'ghp_MARCADOR_TOKEN_REPO_5555\n' > "$PRE/.gh_token"
  chmod 600 "$PRE/.gh_token"
  correr_instalador --instalar --sin-telemetria
  log_mensaje "publica el canal en el repo" 'channel-publish --repo-publish'
  log_mensaje "pasa el FICHERO del token" 'channel-publish .*--token-file'
  salida_mensaje "el instalador lo dice" 'canal publicado en el repo'
}

# --- C17: sin token de GitHub, avisa de que el canal no va al repo --------
# Qué prueba: sin token, el instalador avisa "el canal NO se pudo publicar en
#   el repo" y lo anota en el informe como fallo.
# Por qué importa: sin aviso, el usuario cree que la flota lo ve cuando en
#   realidad nadie lo ve: el peor fallo es el silencioso.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_17_sin_token_el_canal_no_va_al_repo() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "avisa de que el canal NO se publicó" 'el canal NO se pudo publicar en el repo'
  comprobar "el informe lo registra como fallo" python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if any(p.get("nombre")=="canal_repo" and p.get("ok") is False for p in d.get("pasos",[])) else 1)' "$ST/instalacion.json"
}

# --- C18: sin channel-publish, avisa ---------------------------------------
# Qué prueba: si el publicador del canal no se puede descargar, el
#   instalador avisa "falta channel-publish" y no finge que sí.
# Por qué importa: el canal del repo es la vía de descubrimiento de la flota:
#   sin avisar, el nodo queda mudo y nadie lo echa de menos.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_18_sin_channel_publish_avisa() {
  preparar_lab
  # el curl del banco, pero el asset del publicador del canal NO se baja:
  # así se llega de verdad a la rama "falta channel-publish".
  mv "$SB/curl" "$SB/curl.real"
  rm -f "$SB/curl"
  cat > "$SB/curl" <<'EOF'
#!/bin/sh
# curl de mentira (variante del banco): todo igual, pero el asset
# channel-publish-arm64 NO se puede descargar.
for a in "$@"; do
  case "$a" in
    *channel-publish-arm64*) exit 7 ;;
  esac
done
exec "$(dirname "$0")/curl.real" "$@"
EOF
  chmod 755 "$SB/curl"
  correr_instalador --instalar --sin-telemetria
  comprobar "el publicador del canal NO quedó instalado" [ ! -x "$PRE/channel-publish" ]
  salida_mensaje "avisa de que falta channel-publish" 'falta channel-publish'
}

# --- C19: --pc sin adb muere ----------------------------------------------
# Qué prueba: --pc sin adb en el PATH muere con "no hay adb en el PC".
# Por qué importa: es el error más tonto posible en modo PC (el 90% de los
#   usuarios no tienen adb): tiene que decir qué falta, no fallar en silencio.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_19_pc_sin_adb_muere() {
  preparar_lab
  correr_instalador --pc
  comprobar "muerde con un código != 0" [ "$CODIGO" != "0" ]
  salida_mensaje "dice que no hay adb en el PC" 'no hay adb en el PC'
}

# --- C20: --pc con adb sube el script al móvil y lo ejecuta allí -----------
# Qué prueba: con un adb de mentira, el instalador hace `push` del script y
#   `shell` para ejecutarlo en el móvil.
# Por qué importa: el modo PC es el camino de los usuarios sin Termux-root:
#   el script se ejecuta EN EL MÓVIL (allí hay root y red), no en el PC.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_20_pc_con_adb_sube_y_ejecuta_en_el_movil() {
  preparar_lab
  rm -f "$SB/adb"
  cat > "$SB/adb" <<'EOF'
#!/bin/sh
# adb de mentira: REGISTRA lo que le piden hacer.
{ echo "--- adb $*"; } >> "${MD_LOG:-/dev/null}"
case "$1" in
  devices) echo "List of devices attached"; echo "falso-movil	device" ;;
  *) exit 0 ;;
esac
EOF
  chmod 755 "$SB/adb"
  correr_instalador --pc
  log_mensaje "sube el script al móvil" 'adb push .*oneliner\.sh'
  log_mensaje "lo ejecuta en el móvil" 'adb shell sh .*oneliner\.sh --instalar'
  salida_mensaje "avisa de que lo ejecuta en el móvil" 'ejecutando en el móvil'
}

# --- C21: --pc con MD_SERIAL usa -s con la serie ---------------------------
# Qué prueba: con MD_SERIAL=..., todas las llamadas al adb llevan `-s <serie>`.
# Por qué importa: con varios móviles conectado, sin `-s` el instalador
#   puede instalar en el móvil equivocado (el del usuario, no el suyo).
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_21_pc_con_md_serial_usa_menos_s() {
  preparar_lab
  rm -f "$SB/adb"
  cat > "$SB/adb" <<'EOF'
#!/bin/sh
# adb de mentira: REGISTRA lo que le piden hacer.
{ echo "--- adb $*"; } >> "${MD_LOG:-/dev/null}"
case "$1" in
  devices) echo "List of devices attached"; echo "falso-movil	device" ;;
  *) exit 0 ;;
esac
EOF
  chmod 755 "$SB/adb"
  export MD_SERIAL=FAKE-SERIAL-1234
  correr_instalador --pc
  log_mensaje "usa -s con la serie al subir" 'adb -s FAKE-SERIAL-1234 push'
  log_mensaje "usa -s con la serie al ejecutar" 'adb -s FAKE-SERIAL-1234 shell'
}

# =============================================================================
# NÚCLEO FORENSE (C22-C25)
# =============================================================================

# --- C22: --doctor sin mddoctor avisa y sale con código != 0 --------------
# Qué prueba: sin mddoctor ejecutable, --doctor avisa "no hay mddoctor
#   ejecutable" y devuelve un código de error.
# Por qué importa: --doctor se usa cuando el daemon está MUERTO: si el
#   instalador dice "todo bien" y sale con 0, el usuario se queda sin saber.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_22_doctor_sin_mddoctor_avisa_y_falla() {
  preparar_lab
  correr_instalador --doctor
  salida_mensaje "avisa de que no hay mddoctor ejecutable" 'no hay mddoctor ejecutable'
  comprobar "sale con un código != 0" [ "$CODIGO" != "0" ]
}

# --- C23: --doctor con mddoctor ejecuta su diagnóstico --------------------
# Qué prueba: con mddoctor instalado, --doctor lo ejecuta (sub-comandos
#   "salud" y "entorno") y sale con 0.
# Por qué importa: el núcleo forense es el único que puede diagnosticar el
#   móvil cuando el daemon no arranca: si --doctor no lo llama, no sirve.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_23_doctor_con_mddoctor_diagnostica() {
  preparar_lab
  cat > "$PRE/mddoctor" <<'EOF'
#!/bin/sh
# mddoctor de mentira: dice qué sub-comando le piden.
echo "mddoctor $*"
exit 0
EOF
  chmod 755 "$PRE/mddoctor"
  correr_instalador --doctor
  salida_mensaje "ejecuta el diagnóstico de salud" 'mddoctor salud'
  salida_mensaje "ejecuta el diagnóstico de entorno" 'mddoctor entorno'
  comprobar "sale con 0" [ "$CODIGO" = "0" ]
}

# --- C24: --reconectar le pide reconectar --apply a mddoctor --------------
# Qué prueba: --reconectar invoca mddoctor con `reconectar --apply`.
# Por qué importa: es lo que relanza túnel y daemon cuando el móvil quedó
#   ciego; sin --apply el núcleo forense sólo informa y no arregla nada.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_24_reconectar_pide_reconectar_apply() {
  preparar_lab
  cat > "$PRE/mddoctor" <<'EOF'
#!/bin/sh
# mddoctor de mentira: dice qué sub-comando le piden.
echo "mddoctor $*"
exit 0
EOF
  chmod 755 "$PRE/mddoctor"
  correr_instalador --reconectar
  salida_mensaje "le pide reconectar con --apply" 'mddoctor reconectar --apply'
}

# --- C25: --reconectar propaga el código de salida de mddoctor ------------
# Qué prueba: si mddoctor sale con 3, el instalador sale con 3.
# Por qué importa: el código de salida es lo que un script de reparación
#   mira para decidir si reintentar: si el instalador siempre sale con 0, el
#   sistema cree que todo fue bien cuando el núcleo forense falló.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_25_reconectar_propaga_el_codigo_de_mddoctor() {
  preparar_lab
  cat > "$PRE/mddoctor" <<'EOF'
#!/bin/sh
# mddoctor de mentira: falla con 3 (como el núcleo forense cuando no puede
# reconectar el túnel).
echo "mddoctor $*"
exit 3
EOF
  chmod 755 "$PRE/mddoctor"
  correr_instalador --reconectar
  comprobar "el instalador propaga el código 3" [ "$CODIGO" = "3" ]
}

# =============================================================================
# AUTOREPARACIÓN (C26-C32)
# =============================================================================

# --- C26: la autoreparación repone los scripts de service.d --------------
# Qué prueba: un `cp` de mentira hace FALLAR el primer copiado a service.d;
#   el instalador avisa "falta .../91-adb-tcp.sh" y, al final, el script
#   vuelve a estar (lo repone la autoreparación, no el paso de instalación).
# Por qué importa: /data/adb/service.d es lo que Magisk ejecuta en cada
#   arranque: si un script falta, el móvil no arranca NADA tras un reboot.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_26_autorreparacion_repone_service_d() {
  TUNEL=caido; HEALTH=ko
  preparar_lab
  # `cp` de mentira: el PRIMER copiado a 91-adb-tcp.sh falla (prefijo
  # ocupado); los siguientes funcionan.  Así se ve que quien repone es la
  # autoreparación (que va después) y no el paso de instalación.
  # OJO: el doble va en $SB (los fakes, lo PRIMERO del PATH), NO en $TBR:
  # en $TBR/cp hay un symlink al /usr/bin/cp del HOST y escribir ahí lo
  # sustituiría (y el ejecutable real quedaría reventado).
  REAL_CP="$(readlink -f "$TBR/cp")"
  rm -f "$SB/cp"
  cat > "$SB/cp" <<EOF
#!/bin/sh
# cp de mentira: el primer cp a 91-adb-tcp.sh (en service.d) falla.
dest=""
for a in "\$@"; do dest="\$a"; done
case "\$dest" in
  */91-adb-tcp.sh)
    if [ ! -f "\$LAB_PROC/cp_fallado_una_vez" ]; then
      : > "\$LAB_PROC/cp_fallado_una_vez"
      echo "cp: \$dest: no se puede escribir (prefijo ocupado)" >&2
      exit 1
    fi
    ;;
esac
exec "$REAL_CP" "\$@"
EOF
  chmod 755 "$SB/cp"
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "avisa de que faltaba en service.d" 'falta .*91-adb-tcp\.sh'
  comprobar "la autoreparación lo repuso en service.d" ejecutable "$SVC/91-adb-tcp.sh"
}

# --- C27: la autoreparación repone el watchdog en el prefijo --------------
# Qué prueba: el watchdog falta del prefijo => el instalador avisa "falta el
#   watchdog en el prefijo" y lo repone.
# Por qué importa: el watchdog es el que relanza cloudflared cuando el
#   túnel se cae: sin él, el móvil funciona hasta el primer reinicio y
#   luego queda ciego para siempre.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_27_autorreparacion_repone_el_watchdog() {
  TUNEL=caido; HEALTH=ko
  preparar_lab
  # `cp` de mentira: el PRIMER copiado del watchdog al prefijo falla.
  # El doble va en $SB (los fakes), NO en $TBR (ahí cp es un symlink al
  # /usr/bin/cp del host: escribir ahí lo reventaría).
  REAL_CP="$(readlink -f "$TBR/cp")"
  rm -f "$SB/cp"
  cat > "$SB/cp" <<EOF
#!/bin/sh
# cp de mentira: el primer cp del watchdog al prefijo falla.
dest=""
for a in "\$@"; do dest="\$a"; done
case "\$dest" in
  */tunnel-watchdog.sh)
    if [ ! -f "\$LAB_PROC/cp_fallado_una_vez" ]; then
      : > "\$LAB_PROC/cp_fallado_una_vez"
      echo "cp: \$dest: no se puede escribir (prefijo ocupado)" >&2
      exit 1
    fi
    ;;
esac
exec "$REAL_CP" "\$@"
EOF
  chmod 755 "$SB/cp"
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "avisa de que faltaba el watchdog" 'falta el watchdog en el prefijo'
  comprobar "la autoreparación lo repuso en el prefijo" ejecutable "$PRE/tunnel-watchdog.sh"
}

# --- C28: la autoreparación avisa si falta el núcleo forense --------------
# Qué prueba: con un bundle SIN mddoctor, la autoreparación avisa "falta el
#   nucleo forense".
# Por qué importa: sin núcleo forense no hay Who puede reconectar el móvil
#   cuando el daemon está muerto: hay que decirlo, no pasar desapercibido.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_28_autorreparacion_avisa_si_falta_el_nucleo_forense() {
  SIN_MDOCTOR=1; TUNEL=caido; HEALTH=ko
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "avisa de que falta el núcleo forense" 'falta el nucleo forense'
}

# --- C29: si el túnel no responde, relanza el stack con el núcleo forense --
# Qué prueba: con el túnel caído, la autoreparación dice "el tunel no
#   responde: relanzando el stack con el nucleo forense" y llama a mddoctor.
# Por qué importa: es el camino que enciende el móvil cuando quedó a medias
#   (lo que pasaba en el SM-G998B: instalaba "bien" y quedaba muerto).
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_29_autorreparacion_relanza_el_stack_con_el_nucleo_forense() {
  TUNEL=caido; HEALTH=ko
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "dice que relanza el stack con el núcleo forense" 'el tunel no responde: relanzando el stack con el nucleo forense'
  salida_mensaje "y el núcleo forense reconecta" 'mddoctor reconectar --apply'
}

# --- C30: tras reparar, vuelve a sondear el túnel -------------------------
# Qué prueba: con un bundle de mentira donde el daemon sólo responde a la
#   URL que deja el núcleo forense al reconectar, tras la autoreparación el
#   instalador vuelve a sondear y dice "el túnel responde TRAS la
#   autorreparacion".
# Por qué importa: reparar y NO volver a comprobar es sólo una mitad: si el
#   túnel sigue muerto, el informe debe decirlo (y el móvil, Stay muerto).
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_30_tras_reparar_vuelve_a_sondear() {
  preparar_lab
  # Bundle de mentira: el daemon sólo responde a la URL "reparada" y el
  # núcleo forense deja esa URL en el estado al reconectar.
  rm -rf "$L/bundle/stage"
  mkdir -p "$L/bundle/stage"
  ( cd "$L/bundle/stage" && tar -xzf "$L/bundle/mdo.tar.gz" )
  cat > "$L/bundle/stage/bin/megadaemon-core" <<'EOF'
#!/bin/sh
# daemon de mentira: la sonda responde SÓLO a la URL reparada.
case "$1" in
  telemetria)
    case "$2" in
      sonda)
        case "$3" in
          https://reparado.trycloudflare.com) echo '{"ok":true,"codigo":401}'; exit 0 ;;
          https://*) echo '{"ok":false,"codigo":0}'; exit 1 ;;
          *) echo '{"ok":false}'; exit 1 ;;
        esac ;;
      listo)
        if [ -f "${MEGADAEMON_STATE_DIR:-/nonexistent}/tunnel.url" ]; then
          echo '{"listo":true}'; exit 0
        fi
        echo '{"listo":false}'; exit 1 ;;
      *) echo '{"ok":true}'; exit 0 ;;
    esac ;;
  *) exit 0 ;;
esac
EOF
  chmod 755 "$L/bundle/stage/bin/megadaemon-core"
  cat > "$L/bundle/stage/bin/mddoctor" <<'EOF'
#!/bin/sh
# núcleo forense de mentira: al reconectar deja una URL viva en el estado.
case "$1" in
  reconectar)
    printf 'https://reparado.trycloudflare.com' > "${MEGADAEMON_STATE_DIR:-/tmp}/tunnel.url" 2>/dev/null
    echo "reconectado"; exit 0 ;;
  todo) echo "veredicto: todo bien"; exit 0 ;;
  *) echo "entorno: ok"; exit 0 ;;
esac
EOF
  chmod 755 "$L/bundle/stage/bin/mddoctor"
  ( cd "$L/bundle/stage" && tar -czf "$L/bundle/mdo.tar.gz" bin deploy )
  ( cd "$L/bundle" && sha256sum mdo.tar.gz > mdo.tar.gz.sha256 )
  TUNEL=caido; HEALTH=ok
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "vuelve a sondear el túnel tras reparar" 'el tunel responde TRAS la autorreparacion'
}

# --- C31: si tras reparar sigue caído, avisa ------------------------------
# Qué prueba: con el túnel muerto de verdad (health=ko), tras la
#   autoreparación el instalador avisa "tras la autorreparacion el tunel
#   sigue sin responder".
# Por qué importa: es el aviso que evita el "instalado y todo bien" falso:
#   el móvil queda muerto y el usuario tiene que saberlo.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_31_si_sigue_caido_avisa() {
  TUNEL=caido; HEALTH=ko
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "avisa de que el túnel sigue caído" 'tras la autorreparacion el tunel sigue sin responder'
}

# --- C32: el informe registra el paso "autorreparacion" -------------------
# Qué prueba: el informe de instalación incluye un paso llamado
#   "autorreparacion".
# Por qué importa: el root lo lee desde Drive: sin ese paso no sabe si el
#   móvil se tuvo que reparar ni con qué resultado.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_32_el_informe_registra_la_autorreparacion() {
  TUNEL=caido; HEALTH=ko
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "el informe registra el paso autoreparacion" python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if any(p.get("nombre")=="autorreparacion" for p in d.get("pasos",[])) else 1)' "$ST/instalacion.json"
}

# =============================================================================
# PORTABILIDAD (C33-C36)
# =============================================================================

# --- C33: el instalador es compatible con dash ----------------------------
# Qué prueba: `dash -n deploy/megadaemon-oneliner.sh` no dice NADA y sale 0.
# Por qué importa: en el móvil el script se ejecuta con el shell de Android
#   (dash): un error de sintaxis bash-ista deja al usuario con un error de
#   sintaxis en pantalla en lugar de un daemon.
# MEDIDO: 2026-11-27 (dash -n sobre el fuente real del instalador).
caso_c_33_el_instalador_es_compatible_con_dash() {
  if ! command -v dash >/dev/null 2>&1; then
    omitir "este entorno no tiene dash"
    return
  fi
  dash -n "$RAIZ/deploy/megadaemon-oneliner.sh" > "$L/dash.out" 2>&1
  RC=$?
  comprobar "dash analiza el instalador sin errores" [ "$RC" = "0" ]
  comprobar "dash no dice nada" [ ! -s "$L/dash.out" ]
}

# --- C34: el instalador no usa bash-ismos ---------------------------------
# Qué prueba: en el fuente no hay [[ ]], local, declare -a, arrays (=(), ni
#   source, ni read -a, ni echo -e.
# Por qué importa: aunque `dash -n` pase, un bash-ismo dentro de una
#   rama puede petar en el móvil (que corre el shell de Android, no bash).
# MEDIDO: 2026-11-27 (grep sobre el fuente real del instalador).
caso_c_34_el_instalador_no_usa_bashismos() {
  F="$RAIZ/deploy/megadaemon-oneliner.sh"
  # [[ ]] (las clases POSIX [[:space:]] NO son bash-ismos)
  comprobar "no usa [[ ]]" [ -z "$(grep -nE '\[\[[^:]' "$F" 2>/dev/null)" ]
  comprobar "no usa local" [ -z "$(grep -nE '^[[:space:]]*local[[:space:]]' "$F" 2>/dev/null)" ]
  comprobar "no declara arrays" [ -z "$(grep -nE 'declare[[:space:]]+-a' "$F" 2>/dev/null)" ]
  comprobar "no asigna arrays" [ -z "$(grep -nE '[A-Za-z_][A-Za-z0-9_]*=\((\)|[^$])' "$F" 2>/dev/null)" ]
  comprobar "no usa source" [ -z "$(grep -nE '(^|[[:space:];&|])source[[:space:]]' "$F" 2>/dev/null)" ]
  comprobar "no usa read -a" [ -z "$(grep -nE '\bread[[:space:]]+-a' "$F" 2>/dev/null)" ]
  comprobar "no usa echo -e" [ -z "$(grep -nE '\becho[[:space:]]+-e' "$F" 2>/dev/null)" ]
}

# --- C35: el instalador no tiene CRLF ni tabs -----------------------------
# Qué prueba: el fuente no tiene ningún retorno de carro ni ningún tabulador.
# Por qué importa: el móvil se lo copia por HTTP: un CR rompe el shebang
#   ("/bin/sh^M") y el script no arranca; un tab es un resto de edición.
# MEDIDO: 2026-11-27 (grep sobre el fuente real del instalador).
caso_c_35_el_instalador_no_tiene_crlf_ni_tabs() {
  F="$RAIZ/deploy/megadaemon-oneliner.sh"
  CR="$(printf '\r')"
  TAB="$(printf '\t')"
  comprobar "no tiene retornos de carro (CRLF)" [ "$(grep -F -c "$CR" "$F" 2>/dev/null)" = "0" ]
  comprobar "no tiene tabuladores" [ "$(grep -F -c "$TAB" "$F" 2>/dev/null)" = "0" ]
}

# --- C36: el instalador limpia su temporal al salir -----------------------
# Qué prueba: al terminar, el temporal de trabajo (mktemp) y el stage del
#   prefijo NO quedan, y el TMPDIR del laboratorio queda vacío.
# Por qué importa: en el móvil el temporal es /data/local/tmp: 13 MB de
#   bundle y el stage por cada intento llenan el disco si no se limpian.
# MEDIDO: 2026-11-27 (instalador real en el laboratorio falso).
caso_c_36_el_instalador_limpia_su_temporal() {
  HEALTH=ko
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "no queda el temporal de trabajo" [ -z "$(ls -d "$TMPDIR"/tmp.* 2>/dev/null)" ]
  comprobar "no queda el stage en el prefijo" [ -z "$(ls -d "$PRE"/.stage.* 2>/dev/null)" ]
  comprobar "el temporal queda limpio" [ -z "$(ls -A "$TMPDIR" 2>/dev/null)" ]
}

#!/usr/bin/env bash
# =============================================================================
# test-oneliner-casos-a.sh — ESCENARIOS A del banco del instalador del móvil.
#
# QUÉ CUBRE (36 escenarios)
# -------------------------
#   A1-A4    la detección del entorno: destino, android, termux, root,
#            arquitectura y la falta de curl/wget.
#   A5-A11   la interfaz: --help, opción desconocida y el recorte de
#            --reintentos y --timeout (valores imposibles, 0, 999, 1).
#   A12-A17  el root: pedir root, la escalada por `su -c` y las preguntas
#            sobre rutas de root (existe/ejecutable/r).
#   A18-A21  las TRES capas de resolución del release (URL estable, manifiesto
#            latest.json y API de releases) y la muerte sin red.
#   A22-A29  la descarga con reintentos y la verificación POR VALOR del
#            .sha256 (incluida la ruta del CI y el caso sin sha256sum).
#   A30-A36  la instalación atómica (.new + mv), el rollback y la limpieza.
#
# CÓMO ESTÁN ESCRITOS
# --------------------
#   Cada escenario EJECUTA el instalador en el laboratorio falso del harness
#   (`tools/test-oneliner.sh`) y comprueba el EFECTO: la salida, el código de
#   salida y el estado del prefijo.  NO se "prueba" con grep sobre el texto
#   del instalador (eso deja pasar bugs reales).
#
#   NO hay una segunda capa de ayudantes aquí: TODO se apoya en los knobs y
#   las funciones del harness (TERMUX, DL, SHA, BUNDLE, NET, SIN_SHA256SUM,
#   RELOJ_VIRTUAL, correr_instalador, correr_instalador_como_usuario,
#   correr_instalador_sin_escalada…).  Una sola caja, una sola API: si cada
#   fichero trae la suya, acabamos con tres "verdades" del mismo banco.
#   Lo que el laboratorio NO puede dar se OMITE con `omitir "motivo"`.
#
#   MEDIDO: 2026-11-27, banco v2 (harness aislado, /proc y red falsos).
# =============================================================================

# --- A1) el instalador anuncia el entorno detectado --------------------------
# Qué prueba: que dice destino, android, termux, root y arquitectura.
# Por qué importa: es lo primero que lee el usuario; si dice "pc" en un
#   móvil, el resto del script se comporta como si no hubiera móvil.
caso_a_01_anuncia_el_entorno() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "anuncia el destino" 'destino: (movil|pc)'
  salida_mensaje "anuncia si detecta Android" 'android: [01]'
  salida_mensaje "anuncia si detecta Termux" 'termux: [01]'
  salida_mensaje "anuncia si tiene root" 'root: [01]'
  salida_mensaje "anuncia la arquitectura" 'arquitectura: '
}

# --- A2) con Termux simulado se detecta el móvil ----------------------------
# Qué prueba: que con /data/data/com.termux y `uname -o`=Android el
#   instalador detecta el móvil (y no un PC).
# Por qué importa: la detección decide el mensaje de root y el prefijo.
caso_a_02_detecta_el_movil_con_termux() {
  TERMUX=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "detecta el móvil" 'destino: movil'
  salida_mensaje "detecta Android" 'android: 1'
  salida_mensaje "detecta Termux" 'termux: 1'
}

# --- A3) arquitectura desconocida: avisa ------------------------------------
# Qué prueba: que si `uname -m` no devuelve una arquitectura conocida, lo dice
#   en vez de seguir como si fuera arm64.
# Por qué importa: en un ARM de 32 bits el binario no arranca; el aviso es
#   la diferencia entre "no funciona" y "sé lo que pasa".
caso_a_03_arquitectura_desconocida() {
  preparar_lab
  # un `uname` de mentira que devuelve una arquitectura rara.
  # (falso_en_caja borra el symlink antes: escribir encima del symlink
  #  pisaría el /usr/bin/uname del sistema - MEDIDO 2026-11-27.)
  falso_en_caja uname <<'EOF'
#!/bin/sh
case "$1" in
  -o) echo "Android"; exit 0 ;;
  -m) echo "mips"; exit 0 ;;
esac
exec /usr/bin/uname "$@"
EOF
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "avisa de que no sabe la arquitectura" 'arquitectura: desconocid'
}

# --- A4) sin curl ni wget avisa (y dice cómo arreglarlo) --------------------
# Qué prueba: que en un móvil sin downloader avisa y dice cómo instalarlo.
# Por qué importa: sin curl ni wget el script no puede descargar nada; si
#   no lo dice, el usuario ve un fallo más adelante y no entiende la causa.
caso_a_04_avisa_sin_curl_ni_wget() {
  TERMUX=1
  DL=ninguno
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "avisa de que no hay downloader" 'downloader: NINGUNO'
  salida_mensaje "dice cómo instalar curl" 'pkg install curl'
}

# --- A5) --help sale con 0 e imprime el uso ---------------------------------
# Qué prueba: que `--help` enseña el uso y sale con código 0.
# Por qué importa: es lo primero que pega un usuario que duda; si devuelve
#   error, parece que el one-liner está roto.
caso_a_05_help_sale_con_cero() {
  preparar_lab
  correr_instalador --help
  comprobar "sale con 0" [ "$CODIGO" = "0" ]
  salida_mensaje "imprime el uso" 'curl -fsSL'
}

# --- A6) una opción desconocida sale con 2 ----------------------------------
# Qué prueba: que una opción inválida se rechaza con código 2 (uso) y no se
#   instala nada.
# Por qué importa: un typo en la línea de comando no debe instalar un
#   bundle en un móvil.
caso_a_06_opcion_desconocida() {
  preparar_lab
  correr_instalador --instalar --inventada
  comprobar "sale con 2" [ "$CODIGO" = "2" ]
  salida_mensaje "dice que la opción no existe" 'opción desconocida'
  comprobar "no instaló nada" [ ! -f "$PRE/megadaemon-core" ]
}

# --- A7) --reintentos=abc se recorta a 5 -----------------------------------
# Qué prueba: que un valor no numérico no rompe el bucle de descarga.
# Por qué importa: `[ "$i" -le "abc" ]` es un error de shell: el instalador
#   moriría con un mensaje incomprensible en vez de instalar.
caso_a_07_reintentos_no_numerico() {
  NET=falla-bundle
  RELOJ_VIRTUAL=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria --reintentos=abc
  salida_mensaje "usa 5 reintentos" 'intento 1/5'
  no_salida_mensaje "no usa un número de reintentos absurdo" 'intento 1/abc'
  salida_mensaje "acaba avisando que no pudo descargar" 'no se pudo descargar el bundle'
}

# --- A8) --reintentos=0 se recorta a 1 --------------------------------------
# Qué prueba: que 0 reintentos se recorta a 1 (con 0 el bucle no probaría
#   ni una vez).
# Por qué importa: con 0 el instalador declararía que no hay bundle sin
#   haberlo intentado nunca.
caso_a_08_reintentos_cero() {
  NET=falla-bundle
  RELOJ_VIRTUAL=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria --reintentos=0
  salida_mensaje "usa 1 intento" 'intento 1/1'
  no_salida_mensaje "no hay un intento 0" 'intento 0/'
}

# --- A9) --reintentos=999 se recorta a 20 ----------------------------------
# Qué prueba: que un número absurdo se recorta a 20 (el tope del instalador).
# Por qué importa: 999 intentos con backoff son horas de datos móviles.
caso_a_09_reintentos_demasiados() {
  NET=falla-bundle
  RELOJ_VIRTUAL=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria --reintentos=999
  salida_mensaje "usa 20 reintentos" 'intento 1/20'
  no_salida_mensaje "no supera el tope de 20" 'intento 21/'
}

# --- A10) --timeout=abc se recorta al de por defecto ------------------------
# Qué prueba: que un timeout no numérico no rompe y se queda en el de por
#   defecto (120 s).
# Por qué importa: `$(( AHORA + TIMEOUT_S * 2 / 3 ))` con un valor no
#   numérico es un error de shell en el móvil.
caso_a_10_timeout_no_numerico() {
  TUNEL=sin-url; HEALTH=ko
  RELOJ_VIRTUAL=1
  preparar_lab
  : > "$LOG"
  correr_instalador --instalar --sin-telemetria --force --timeout=abc
  C1=$(grep -c '^sleep ' "$LOG" 2>/dev/null || echo 0)
  : > "$LOG"
  correr_instalador --instalar --sin-telemetria --force --timeout=30
  C2=$(grep -c '^sleep ' "$LOG" 2>/dev/null || echo 0)
  comprobar "no hay error de shell" [ "$CODIGO" = "0" ] || [ "$CODIGO" = "1" ]
  no_salida_mensaje "no hay error de shell" 'arithmetic|syntax error|parameter not set'
  if [ "${C1:-0}" -gt "${C2:-0}" ]; then
    ok "el presupuesto por defecto (120 s) espera MÁS que --timeout=30 ($C1 > $C2)"
  else
    mal "el presupuesto por defecto no se midió distinto de --timeout=30 ($C1 vs $C2)"
  fi
}

# --- A11) --timeout=1 se recorta a 30 (mínimo) ------------------------------
# Qué prueba: que un timeout de 1 s se recorta al mínimo de 30 s.
# Por qué importa: con 1 s el instalador declararía el móvil inalcanzable
#   siempre, sin dar tiempo a que aparezca el túnel.
caso_a_11_timeout_minimo() {
  TUNEL=sin-url; HEALTH=ko
  RELOJ_VIRTUAL=1
  preparar_lab
  : > "$LOG"
  correr_instalador --instalar --sin-telemetria --force --timeout=1
  C1=$(grep -c '^sleep ' "$LOG" 2>/dev/null || echo 0)
  : > "$LOG"
  correr_instalador --instalar --sin-telemetria --force --timeout=30
  C2=$(grep -c '^sleep ' "$LOG" 2>/dev/null || echo 0)
  comprobar "--timeout=1 se comporta como --timeout=30" [ "${C1:-0}" = "${C2:-0}" ]
}

# --- A12) sin root en el móvil muere diciendo que necesita root -------------
# Qué prueba: que en un móvil sin root el instalador para y lo explica.
# Por qué importa: sin root no se puede escribir en /data/adb; si siguiera,
#   dejaría el móvil a medias fingiendo que instaló.
caso_a_12_sin_root_en_movil() {
  TERMUX=1
  preparar_lab
  if [ "${USUARIO_DISPONIBLE:-0}" != "1" ]; then
    omitir "este entorno no permite ejecutar como usuario normal"
    return
  fi
  correr_instalador_sin_escalada --instalar --sin-telemetria
  comprobar "muerre con código 1" [ "$CODIGO" = "1" ]
  salida_mensaje "pide root para Termux" 'necesito root'
  no_salida_mensaje "no dice que sea un PC" 'en un PC, ejecuta esto con sudo'
}

# --- A13) sin root en un PC muere con el mensaje del PC ---------------------
# Qué prueba: que en un PC sin root dice cómo se resuelve: "con sudo".
# Por qué importa: es el mensaje que ve el usuario de escritorio; si dijera
#   "necesito root para Termux" en un PC, el consejo sería inútil.
caso_a_13_sin_root_en_pc() {
  preparar_lab
  if [ "${USUARIO_DISPONIBLE:-0}" != "1" ]; then
    omitir "este entorno no permite ejecutar como usuario normal"
    return
  fi
  correr_instalador_sin_escalada --instalar --sin-telemetria
  comprobar "muerre con código 1" [ "$CODIGO" = "1" ]
  salida_mensaje "dice cómo ascender en un PC" 'no hay root: en un PC, ejecuta esto con sudo'
  no_salida_mensaje "no pide root de Termux" 'necesito root'
}

# --- A14) con root NO vuelve a pedirlo -------------------------------------
# Qué prueba: que cuando ya hay root el instalador no lo pide otra vez.
# Por qué importa: pedir root cuando ya se tiene hace que el usuario piense
#   que algo va mal (y en Magisk abre el diálogo de permisos otra vez).
caso_a_14_con_root_no_pide_root() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  no_salida_mensaje "no pide root" 'necesito root'
  no_salida_mensaje "no dice que no hay root" 'no hay root'
  salida_mensaje "sigue.root: 1" 'root: 1'
}

# --- A15) la escalada detecta `su -c` --------------------------------------
# Qué prueba: que como usuario normal con `su` disponible, el instalador
#   escala y dice por qué vía.
# Por qué importa: en Magisk moderno la vía es `su -c`; sin detectarla el
#   móvil se queda sin instalar.
caso_a_15_escalada_por_su() {
  preparar_lab
  if [ "${USUARIO_DISPONIBLE:-0}" != "1" ]; then
    omitir "este entorno no permite ejecutar como usuario normal"
    return
  fi
  correr_instalador_como_usuario --instalar --sin-telemetria
  salida_mensaje "dice por qué vía tiene root" 'root obtenido con: su'
}

# --- A16) las preguntas sobre el prefijo de root las hace con root ----------
# Qué prueba: que un fichero 0600 de root dentro del prefijo se ve (su
#   tamaño se lee), o sea que las preguntas van en el ámbito de root.
# Por qué importa: el bug medido en el móvil: el `wc -c <` como usuario
#   normal daba "Permission denied" y el tamaño salía vacío.
caso_a_16_preguntas_en_ambito_de_root() {
  preparar_lab
  printf '{"installed":{"client_id":"x"}}' > "$ST/gdrive_credentials.json"
  chmod 600 "$ST/gdrive_credentials.json"
  correr_instalador --instalar --sin-telemetria
  if grep -qE 'credenciales presentes \([0-9]+ B\)' "$OUT"; then
    ok "lee el tamaño de un fichero 0600"
  else
    mal "no lee el tamaño de un fichero 0600 (lo leyó sin root)"
  fi
}

# --- A17) con el prefijo 0700 de root instala escalando --------------------
# Qué prueba: que con el prefijo 0700 de root y el instalador como usuario
#   normal, la escalada permite instalar.
# Por qué importa: es el caso real del móvil (/data/adb es 0700).
caso_a_17_escala_con_prefijo_0700() {
  PREFIX_0700=1
  preparar_lab
  if [ "${USUARIO_DISPONIBLE:-0}" != "1" ]; then
    omitir "este entorno no permite ejecutar como usuario normal"
    return
  fi
  correr_instalador_como_usuario --instalar --sin-telemetria
  comprobar "instala" [ "$CODIGO" = "0" ]
  comprobar "el binario quedó en el prefijo" ejecutable "$PRE/megadaemon-core"
}

# --- A18) resuelve el release por la URL estable ----------------------------
# Qué prueba: que primero pregunta por la URL estable (que no consume cuota
#   de la API de GitHub).
# Por qué importa: la API de releases se agota por cuota de IP: si el
#   instalador empieza por ahí, la flota no puede instalarse.
caso_a_18_resuelve_por_la_url_estable() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  log_mensaje "preguntó por la URL estable" 'curl: .*releases/latest/download/megadaemon-movil\.tar\.gz'
  salida_mensaje "anuncia el tag resuelto" 'release: v'
}

# --- A19) si la URL estable falla, cae al manifiesto latest.json -------------
# Qué prueba: que la segunda capa (el asset `latest.json`) se usa cuando la
#   URL estable no está.
# Por qué importa: la release puede no tener el asset todavía.
caso_a_19_cae_al_manifiesto() {
  NET=404
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  log_mensaje "preguntó por el manifiesto" 'curl: .*latest\.json'
  salida_mensaje "anuncia el tag resuelto" 'release: v'
}

# --- A20) si la estable y el manifiesto fallan, cae a la API -----------------
# Qué prueba: que la tercera capa (la API de releases) se usa como último
#   recurso, y que se descarga el asset que da la API.
# Por qué importa: sin esta capa, un release recién publicado (sin caché de
#   CDN) no se puede instalar.
caso_a_20_cae_a_la_api() {
  NET=solo-api
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  log_mensaje "preguntó a la API" 'curl: .*api\.github\.com/.*/releases/latest'
  log_mensaje "usa la URL del asset que da la API" 'curl: .*releases/download/vabc1234/megadaemon-movil\.tar\.gz'
  comprobar "instala" [ "$CODIGO" = "0" ]
}

# --- A21) sin red muere con un mensaje claro -------------------------------
# Qué prueba: que sin red el instalador muere explicando que no pudo
#   resolver el release.
# Por qué importa: sin red el móvil no puede instalar nada; el mensaje dice
#   si es un problema de red o del repo.
caso_a_21_sin_red_muerre() {
  NET=down
  RELOJ_VIRTUAL=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "murre con código 1" [ "$CODIGO" = "1" ]
  salida_mensaje "explica que no pudo resolver el release" 'no se pudo resolver el release'
  comprobar "no instaló nada" [ ! -f "$PRE/megadaemon-core" ]
}

# --- A22) la descarga reintenta y avisa -------------------------------------
# Qué prueba: que un fallo de descarga se reintenta con aviso (y backoff).
# Por qué importa: una descarga a medias es lo más normal en datos móviles.
caso_a_22_la_descarga_reintenta() {
  NET=falla-bundle
  RELOJ_VIRTUAL=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria --reintentos=3
  salida_mensaje "avisa del fallo de descarga" 'descarga fallida \(intento 1/3\)'
  salida_mensaje "avisa del segundo intento" 'descarga fallida \(intento 2/3\)'
  salida_mensaje "avisa del tercer intento" 'descarga fallida \(intento 3/3\)'
}

# --- A23) --reintentos=2 acota los intentos --------------------------------
# Qué prueba: que el número de reintentos que se pasa es el que se usa.
# Por qué importa: el backoff es cuadrático: 20 reintentos son horas.
caso_a_23_reintentos_acotados() {
  NET=falla-bundle
  RELOJ_VIRTUAL=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria --reintentos=2
  salida_mensaje "hizo 2 intentos" 'intento 2/2'
  no_salida_mensaje "no hizo un tercer intento" 'intento 3/'
}

# --- A24) el .sha256 se verifica POR VALOR ----------------------------------
# Qué prueba: que el bundle se verifica comparando el HASH, no la ruta que
#   trae el fichero del CI.
# Por qué importa: el `.sha256` del CI lleva la ruta del job ⇒ comparar la
#   ruta nunca cuadraba y no se publicaba ninguna release.
caso_a_24_sha256_verificado_por_valor() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "dice que está verificado" 'sha256: VERIFICADO por valor'
}

# --- A25) un .sha256 con la ruta del CI también cuadra ---------------------
# Qué prueba: que el `.sha256` con la ruta `dist/megadaemon-movil.tar.gz`
#   (la que genera el CI) verifica igual.
# Por qué importa: es EXACTAMENTE el formato que publica nuestro CI.
caso_a_25_sha256_con_ruta_del_ci() {
  SHA=con-ruta
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "verifica igual con la ruta del CI" 'sha256: VERIFICADO por valor'
}

# --- A26) un sha256 que NO cuadra aborta -----------------------------------
# Qué prueba: que un bundle manipulado NO se instala.
# Por qué importa: es el control de integridad de la única vía de
#   instalación que tiene el usuario.
caso_a_26_sha256_distinto_aborta() {
  SHA=distinto
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "aborta" [ "$CODIGO" = "1" ]
  salida_mensaje "lo dice claramente" 'NO cuadra'
  comprobar "NO instaló el binario" [ ! -f "$PRE/megadaemon-core" ]
}

# --- A27) si el .sha256 no está en el release, avisa y sigue ---------------
# Qué prueba: que un release sin `.sha256` no bloquea la instalación (con
#   aviso).
# Por qué importa: el CI puede publicar el bundle antes que su hash; mejor
#   instalar con aviso que dejar el móvil sin daemon.
caso_a_27_sha256_ausente_ava_y_sigue() {
  SHA=ausente
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "avisa de que no hay hash" 'sha256: no disponible'
  comprobar "instala igualmente" [ "$CODIGO" = "0" ]
}

# --- A28) un .sha256 VACÍO aborta (no se puede verificar nada) -------------
# Qué prueba: que un `.sha256` descargado pero vacío hace abortar, en vez de
#   tratarse como "no disponible" (y verificar de más).
# Por qué importa: MEDIDO (2026-11-27) por este banco: el `&& [ -s "$SHA" ]`
#   convertía un hash vacío en "sigo sin verificar", que es justo lo que
#   quiere un atacante.
caso_a_28_sha256_vacio_aborta() {
  SHA=vacio
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "aborta" [ "$CODIGO" = "1" ]
  salida_mensaje "dice que no puede verificar" 'VACÍO|no se pudo calcular el sha256'
  comprobar "NO instaló el binario" [ ! -f "$PRE/megadaemon-core" ]
}

# --- A29) sin sha256sum el hash lo calcula el daemon ------------------------
# Qué prueba: que sin `sha256sum` en el PATH (el shell de Android no lo
#   trae) el hash lo calcula el daemon y la verificación sigue funcionando.
# Por qué importa: sin este camino, en el móvil real la verificación se
#   saltaría siempre.
caso_a_29_sin_sha256sum_lo_calcula_el_daemon() {
  SIN_SHA256SUM=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  salida_mensaje "verifica igual, con el hash del daemon" 'sha256: VERIFICADO por valor'
  log_mensaje "el daemon calculó el hash" 'invocacion: hash'
}

# --- A30) un bundle corrupto aborta y no rompe el móvil ---------------------
# Qué prueba: que un bundle ilegible aborta y deja el binario anterior
#   intacto.
# Por qué importa: el peor resultado posible es "móvil sin daemon + dice que
#   lo instaló".
caso_a_30_bundle_corrupto_aborta() {
  preparar_lab
  runner_binario_sano
  BUNDLE=corrupto
  preparar_lab
  runner_binario_sano
  correr_instalador --instalar --sin-telemetria --force
  comprobar "aborta" [ "$CODIGO" = "1" ]
  salida_mensaje "explica que no se pudo extraer" 'no se pudo extraer el bundle'
  comprobar "el binario vivo sigue ahí" ejecutable "$PRE/megadaemon-core"
}

# --- A31) un bundle vacío aborta -------------------------------------------
# Qué prueba: que un bundle de 0 bytes no se instala (el instalador reintenta
#   la descarga y, si no puede, muere).
# Por qué importa: instalar un binario de 0 bytes deja el móvil sin daemon
#   y con "dice que lo instaló".
caso_a_31_bundle_vacio_aborta() {
  BUNDLE=vacio
  RELOJ_VIRTUAL=1
  preparar_lab
  correr_instalador --instalar --sin-telemetria --reintentos=2
  comprobar "aborta" [ "$CODIGO" = "1" ]
  salida_mensaje "dice que no pudo descargar" 'no se pudo descargar el bundle'
  comprobar "NO instaló el binario" [ ! -f "$PRE/megadaemon-core" ]
}

# --- A32) un bundle sin el binario aborta ----------------------------------
# Qué prueba: que si el bundle no trae el daemon, el instalador lo dice y no
#   finge que instaló.
# Por qué importa: es el bug medido en el móvil ("el bundle no trae el
#   binario" con el binario delante, por mirar /data/adb sin root).
caso_a_32_bundle_sin_binario_aborta() {
  BUNDLE=sin-binario
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "aborta" [ "$CODIGO" = "1" ]
  salida_mensaje "lo dice claramente" 'no trae el binario'
  comprobar "NO instaló el binario" [ ! -f "$PRE/megadaemon-core" ]
}

# --- A33) un binario no ejecutable aborta ----------------------------------
# Qué prueba: que si el binario del bundle no es ejecutable, no se instala.
# Por qué importa: el móvil se queda sin daemon y el instalador dice que
#   terminó bien.
caso_a_33_binario_no_ejecutable_aborta() {
  BUNDLE=no-ejecutable
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "aborta" [ "$CODIGO" = "1" ]
  comprobar "NO dejó un binario roto" [ ! -x "$PRE/megadaemon-core" ]
}

# --- A34) la instalación es ATÓMICA (.new + mv) -----------------------------
# Qué prueba: que el binario se copia a `.new` y se renombra encima (nunca
#   se sobrescribe el que está en ejecución).
# Por qué importa: si el móvil se duerme a mitad de la copia, el binario
#   queda truncado y el móvil se queda sin daemon.
caso_a_34_instalacion_atomica() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  log_mensaje "copia el binario a .new" 'mv: .*megadaemon-core\.new'
  log_mensaje "renombra el .new sobre el destino" 'mv: .*megadaemon-core\.new .*megadaemon-core'
  comprobar "el resultado final es ejecutable" ejecutable "$PRE/megadaemon-core"
}

# --- A35) deja el respaldo del binario anterior -----------------------------
# Qué prueba: que con un binario ya instalado se guarda un `.prev` (de ahí
#   sale el rollback).
# Por qué importa: sin respaldo, el rollback no tiene de qué volver.
caso_a_35_deja_el_respaldo() {
  preparar_lab
  runner_binario_sano
  SIZE=$(wc -c < "$PRE/megadaemon-core" 2>/dev/null || echo 0)
  correr_instalador --instalar --sin-telemetria --force
  comprobar "dejó el .prev" [ -f "$PRE/megadaemon-core.prev" ]
  comprobar "el .prev es el binario anterior" [ "$(wc -c < "$PRE/megadaemon-core.prev" 2>/dev/null || echo 0)" = "$SIZE" ]
}

# --- A36) al terminar no quedan restos --------------------------------------
# Qué prueba: que no quedan `.new` ni directorios `.stage.*` en el prefijo.
# Por qué importa: los restos ocupan espacio y confunden al diagnóstico.
caso_a_36_no_quedan_restos() {
  preparar_lab
  correr_instalador --instalar --sin-telemetria
  comprobar "no queda el .new" [ ! -f "$PRE/megadaemon-core.new" ]
  if ls -d "$PRE"/.stage.* >/dev/null 2>&1; then
    mal "quedan directorios .stage.* en el prefijo"
  else
    ok "no quedan directorios .stage.* en el prefijo"
  fi
}

# --- ayudante interno: deja un binario "sano" instalado ---------------------
# (el que el bundle de mentira instalaría) para simular un móvil ya
#  instalado.  NO es un escenario: el runner sólo ejecuta `caso_*`.
runner_binario_sano() {
  mkdir -p "$PRE"
  cp "$SB/megadaemon-core" "$PRE/megadaemon-core" 2>/dev/null
  chmod 755 "$PRE/megadaemon-core" 2>/dev/null
}

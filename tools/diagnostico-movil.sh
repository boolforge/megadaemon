#!/system/bin/sh
# diagnostico-movil.sh — el móvil publica su estado en Drive.
#
# POR QUÉ EXISTE
# --------------
# MEDIDO (2026-11-25): el móvil estaba en un bucle (arrancaba, moría, el
# supervisor lo reiniciaba) y cada diagnóstico exigía que el usuario pegase
# un bloque de texto en el chat.  Eso es lento, se pierde y no se
# reproducible.
#
# AHORA: el daemon publica su estado completo en Drive
# (`MegaDaemon/diagnostico/<nodo>/`) y el root lo lee desde el sandbox con
#   tools/leer-diagnostico.py
# sin ADB, sin túnel y sin que nadie pegue nada.
#
# USO (desde Termux, con root):
#   su -mm -c 'sh ~/storage/downloads/MiniMax/diagnostico.sh'
#
# Qué hace:
#   1. Sube el diagnóstico a Drive con el propio binario del daemon (que
#      ya tiene el cliente de Drive y las credenciales) ⇒ funciona aunque
#      el daemon NO esté corriendo.
#   2. Si la subida falla, deja el informe en local y lo dice (para poder
#      pegarlo a mano como último recurso).

PREFIX=/data/adb/megadaemon
BIN="$PREFIX/megadaemon-core"
STATE="$PREFIX/state"

echo "== diagnostico: recogiendo estado"
if [ ! -x "$BIN" ]; then
    echo "ERROR: no hay binario en $BIN" >&2
    exit 1
fi

# 1) Intento de publicación (el binario sabe de Drive).
if "$BIN" diagnostico --publicar; then
    echo "OK: publicado en Drive (MegaDaemon/diagnostico/<nodo>/)"
    echo "   el root lo lee con: tools/leer-diagnostico.py"
    exit 0
fi

echo
echo "AVISO: la publicacion fallo: se deja el informe en local"
"$BIN" diagnostico 2>/dev/null || true
ls -la "$STATE/diagnostico" 2>/dev/null | tail -5
echo
echo "Para pasarlo a mano, copia el ultimo JSON:"
echo "  cat $STATE/diagnostico/ultimo.json 2>/dev/null | head -c 4000"
exit 1

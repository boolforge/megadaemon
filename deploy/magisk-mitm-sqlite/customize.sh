#!/system/bin/sh
# =====================================================================
# customize.sh — se ejecuta al INSTALAR el modulo.
#
# Copia el helper al directorio del modulo (para que service.sh y
# uninstall.sh lo puedan usar) y deja los permisos correctos.
#
# Magisk NO ejecuta customize.sh en la instalacion por defecto si el
# modulo se instala desde la app; se ejecuta cuando se flashea el zip.
# En ese caso el helper ya viene dentro del zip (mismo directorio).
# =====================================================================
MODDIR="${0%/customize.sh}"
UI="$MODDIR/mitm-sqlite.sh"

if [ -f "$UI" ]; then
  cp -f "$UI" "$MODDIR/mitm-sqlite.sh" 2>/dev/null
fi

chmod 755 "$MODDIR/mitm-sqlite.sh" 2>/dev/null
chmod 755 "$MODDIR/service.sh" 2>/dev/null
chmod 755 "$MODDIR/post-fs-data.sh" 2>/dev/null
chmod 755 "$MODDIR/uninstall.sh" 2>/dev/null
exit 0

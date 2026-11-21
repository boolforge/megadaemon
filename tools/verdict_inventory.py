#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Genera `docs/SOFTWARE_INVENTORY_VERDICT.md` a partir del inventario MEDIDO
del S21 Ultra (`inventory/inventory.json`, W2) y de un catalogo curado.

Reglas del veredicto (ver `docs/SOFTWARE_INVENTORY_VERDICT.md` §1):

  DESINSTALAR      hay evidencia medida de que la app (a) sube datos del
                   usuario a un servidor de terceros, o (b) pide permisos
                   peligrosos que no necesita para su funcion declarada, o
                   (c) es una app reempaquetada/pirata.
  REIMPLEMENTAR    la funcion cabe en el mega-binario Rust de MegaDaemon
                   (motor de parcheo, gestor de grabaciones, lector de backup,
                   laboratorio de desofuscacion).
  KEEP             app legitima, sin evidencia medida en contra.

Todo lo que sale del veredicto automatico procede del inventario real
(permisos, firma, version, target_sdk, habilitada).  Lo que no se puede
medir todavia va marcado como PENDIENTE, nunca como "no pasa nada".

Uso:
    python3 tools/verdict_inventory.py            # escribe el doc
    python3 tools/verdict_inventory.py --check    # solo valida (>=40 filas)
    python3 tools/verdict_inventory.py --json     # vuelca el veredicto
"""

import argparse
import json
import os
import sys
from collections import OrderedDict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INVENTORY = os.path.join(ROOT, "inventory", "inventory.json")
OUT_MD = os.path.join(ROOT, "docs", "SOFTWARE_INVENTORY_VERDICT.md")

# ---------------------------------------------------------------------------
# Catalogo curado.  `fuente` dice de donde sale cada celda:
#   M  = medido en el movil (inventario W2: permisos, firma, version, estado)
#   A  = medido en el APK (hashes, DEX, artefactos embebidos)
#   D  = deducido del repo (docs del fleet, medidas ya publicadas)
#   P  = PENDIENTE de medicion (el movil esta caido; no se inventa)
# ---------------------------------------------------------------------------
# (paquete, nombre, categoria, funcion, rust, veredicto, justificacion, funcion_fuente)
CATALOG = [
    # --- RE / parcheadores / grabadores (el nucleo del encargo) ------------
    ("ru.drjmwdme.cgwbfzijr", "MarinaPatcher (Lucky Patcher reempaquetado)",
     "RE / parcheador", "Parchea APKs, hookea metodos, re-firma, bypasea licencias de Play",
     "SI (motor de parcheo + hook)",
     "DESINSTALAR",
     "Sube el APK que parchea a un tercero (http://patch.chelpus.com/submit) y trae 2 IPs crudas "
     "(109.95.209.70:8008 sin TLS, https://149.56.155.246/luckypatcher/); ademas pide root "
     "(ACCESS_SUPERUSER), es modulo Xposed (com.xposed.XSupport) y empaqueta las claves de firma de "
     "AOSP + apksigner => puede re-firmar cualquier APK. El classes.dex es IDENTICO al de Lucky "
     "Patcher (sha256 5b0d376ddc79d4ff409f3158b382a335da1b7157a53815211c1d77c4f1200a79): solo "
     "cambian el paquete y el icono.",
     "A"),
    ("org.lsposed.lspatch", "LSPatch", "RE / parcheador",
     "Inyecta un modulo en un APK y lo re-firma (el nucleo open-source de Lucky Patcher)",
     "SI (motor de parcheo DEX/APK)", "KEEP",
     "Open-source (LSPosed),GPL, sin red ni telemetria propia medidos; es la via legitima que ya "
     "tenemos instalada. Se mantiene hasta que el motor Rust lo sustituya.",
     "D"),
    ("io.mesalabs.knoxpatch", "KnoxPatch", "RE / parcheador",
     "Parcheo de APK con soporte de Knox (Samsung)", "SI (motor de parcheo)",
     "KEEP", "Open-source y ya instalada; su funcion (parcheo) la asume el motor Rust. "
             "Medido: 3.364.844 B de APK.", "D"),
    ("org.meowcat.edxposed.manager", "EdXposed (manager)", "RE / framework de hooks",
     "Gestor del framework de hooks LSPosed/EdXposed", "SI (motor de hooks)",
     "KEEP", "Open-source; el motor de hooks del mega-binario lo cubre. Medido: 3.550.938 B.", "D"),
    ("com.htetz.monopatch", "MonoPatch", "RE / parcheador",
     "Parche de un solo DEX", "SI (motor de parcheo)", "KEEP",
     "Open-source; su funcion es un caso del motor de parcheo Rust.", "D"),
    ("ru.bluecat.sberbankpatcher", "SberbankPatcher", "RE / parcheador",
     "Parche surgical de UNA app concreta (banco)", "SI (ejemplo de parche surgical)",
     "KEEP", "Open-source; sirve de referencia de diseno para el parche surgical (APK + hook).", "D"),
    ("io.github.huskydg.magisk", "Magisk (27.2-kitsune)", "RE / root",
     "Root + MagiskHide + modulos systemless", "NO (se usa, no se reimplementa)",
     "KEEP", "Es el 'sistema de archivos' de la flota: sin el no hay modulos systemless ni "
             "MagiskHide. Medido: 12.770.643 B de APK; AVB green con flash.locked=1.", "D"),
    ("com.legendsayantan.adbtools", "ADB tools", "RE / utileria",
     "Utilidades ADB desde el movil", "NO (ya lo hace el cliente ADB en Rust, rust-core/src/adb_client.rs)",
     "KEEP", "Medido: 5.585.559 B. Su funcion ya la cubre megadaemon-adb (Rust puro).", "D"),
    ("adb.captain", "adb captain", "RE / utileria",
     "Utilidades ADB (packaged)", "NO (mismo criterio que ADB tools)",
     "KEEP", "Medido: 29.174.974 B (la mas grande de las dos). Su funcion la cubre el cliente "
             "ADB en Rust; se puede desinstalar cuando el usuario lo confirme.", "D"),
    ("org.swiftapps.swiftbackup", "Swift Backup", "Backup",
     "Backup y restauracion de apps (AEGIS-256 + Zstd)", "SI (lector de formatos)",
     "KEEP", "Ecosistema abierto y cifrado; el lector de sus formatos ya esta en el fleet "
             "(backup_formats.rs).", "D"),
    ("com.callapp.contacts", "CallApp / Lucky Patcher (recarga)", "RE / dudoso",
     "Lleva el recurso lucky_patcher_package; NO es el parcheador (el parcheador es MarinaPatcher)",
     "SI (limpieza de recursos huerfanos)", "DESINSTALAR",
     "Pide 23 permisos de riesgo (el maximo de todo el movil) con target_sdk 29 y esta DESHABILITADA: "
     "no hace nada y declara permisos que no necesita. Medido en el inventario (firma d7f1f224, "
     "23 permisos, OFF).", "M"),
    ("app.revanced.android.gms", "ReVanced GMS (microG parcheado)", "Sistema / parcheado",
     "Google Play Services parcheado para no depender de la tienda", "NO (se usa)",
     "KEEP", "Play Services parcheado: es la pieza que sustituye a las apps de parche tipo "
             "Lucky Patcher para lo basico. Medido: 19 permisos de riesgo, target_sdk 29.", "M"),
    ("com.huami.watch.hmwatchmanager", "Huami Watch Manager", "BASURA / relicto",
     "Gestor de smartwatch Xiaomi/Huami", "NO", "DESINSTALAR",
     "Relicto de un smartwatch: 21 permisos de riesgo (CAMERA, READ_PHONE_STATE, ubicacion, "
     "microfono...) en un movil, y esta DESHABILITADA. Medido en el inventario.", "M"),
    ("com.mc.amazfit1", "Amazfit", "BASURA / relicto",
     "App del reloj Amazfit", "NO", "DESINSTALAR",
     "Relicto de wearable: 20 permisos de riesgo en un movil, DESHABILITADA. Medido.", "M"),
    ("com.xiaomi.hm.health", "Mi Fitness (Xiaomi)", "BASURA / relicto",
     "App de salud de Xiaomi", "NO", "DESINSTALAR",
     "Relicto de wearable: 20 permisos de riesgo (incluye CAMERA y ubicacion fina), DESHABILITADA.", "M"),
    ("com.xplora.kidswatch360", "Kids Watch 360 (Xplora)", "BASURA / relicto",
     "App de reloj infantil", "NO", "DESINSTALAR",
     "Relicto de wearable infantil: 19 permisos de riesgo y DESHABILITADA. Medido.", "M"),
    ("org.kde.kdeconnect_tp", "KDE Connect (telefono)", "BASURA / relicto",
     "KDE Connect para el telefono (receptor de notificaciones del PC)", "NO",
     "DESINSTALAR", "La variante de telefono no tiene counterpart en este movil: 18 permisos de "
                     "riesgo y DESHABILITADA. Medido.", "M"),

    # --- Grabadores ---------------------------------------------------------
    ("com.microsoft.YourPhone_P1_5", "Microsoft Phone (Your Phone)", "Grabador / Microsoft's",
     "Enlace con el PC; una de las 2 apps que DECLARAN CAPTURE_AUDIO_OUTPUT", "SI (gestor de grabaciones)",
     "KEEP", "Medido: en el barrido de 398 APKs / 5.580 MB es una de las solo 2 apps que declaran "
             "CAPTURE_AUDIO_OUTPUT. Es de Microsoft: el permiso es legitimo, pero conviene "
             "saber que la tiene.", "M"),
    ("com.microsoft.LinkToWindowsService", "Link to Windows (servicio)", "Grabador / Microsoft's",
     "Servicio de enlace con el PC; la otra app que DECLARA CAPTURE_AUDIO_OUTPUT", "SI (gestor)",
     "KEEP", "Medido: declara CAPTURE_AUDIO_OUTPUT (barrido de 398 APKs). Mismo criterio que "
             "Your Phone.", "M"),
    ("com.samsung.android.ool", "Mantenimiento del dispositivo (Samsung)", "Sistema / grabador nativo",
     "Mantenimiento del dispositivo; es la app que en Europa avisa por voz de la grabacion",
     "NO (se usa)", "KEEP", "Con CSC=AMO (Orange Espana) es la via de grabacion NATIVA: el modulo de "
                           "grabador de llamadas de MegaDaemon se apoya en ella.", "D"),
    ("com.axyos.contacts", "BCR (Basic Call Recorder)", "Grabador",
     "Grabador de llamadas basico", "SI (gestor de grabaciones, NO grabador silencioso)",
     "KEEP", "Grabador de llamadas. Nota legal: la via silenciosa (grabar sin avisar) es delito en "
             "Espana (art. 197 CP) y vulnera el GDPR => el mega-binario aporta el GESTOR "
             "(listar, transcribir, cifrar, subir), no la grabacion silenciosa.", "D"),

    # --- Herramientas RE y de desarrollo -----------------------------------
    ("com.termux", "Termux", "Terminal / desarrollo",
     "Terminal y entorno de desarrollo en el movil", "NO (lo usa el fleet: bash, python3, git)",
     "KEEP", "Es el entorno en el que vive buena parte del tooling del fleet (incluye el "
             "interprete Python y curl que usa el supervisor).", "D"),
    ("com.termux.api", "Termux:API", "Terminal / desarrollo",
     "Puente de Termux con la API de Android (SMS, llamadas, bateria)", "NO (lo usa el fleet)",
     "KEEP", "Lo usa el supervisor y los modulos de root del fleet.", "D"),
    ("com.agy", "Antigravity (IDE de Google)", "RE / IDE",
     "IDE de Google que habla MCP: consume las tools de MegaDaemon por HTTP", "SI (cliente MCP ya "
     "implementado en rust-core/src/antigravity_intel.rs)", "KEEP",
     "Medido: 14 GB (el mayor blob del movil); es el cliente MCP que valida el carril W5 "
     "(45 tools, puente HTTP Streamable verificado con 200).", "M"),
    ("ru.drjmwdme.cgwbfzijr.bak", "MarinaPatcher (copia)", "BASURA", "Residuo", "NO",
     "DESINSTALAR", "Duplicado del APK reempaquetado; mismo criterio que MarinaPatcher.", "A"),
    ("net.osmand.plus", "OsmAnd+", "Navegacion", "Mapas y navegacion offline", "NO", "KEEP",
     "App legitima del usuario.", "P"),
    ("com.google.android.googlequicksearchbox", "Google (buscador/launcher)",
     "Sistema / launcher", "Buscador y launcher de Google", "NO", "KEEP",
     "Medido: 21 permisos de riesgo; es el launcher del sistema, no se toca.", "M"),
    ("com.whatsapp", "WhatsApp", "Mensajeria", "Mensajeria", "NO", "KEEP",
     "Medido: 19 permisos de riesgo (normal en un messenger). App legitima del usuario.", "M"),
    ("org.telegram.messenger", "Telegram", "Mensajeria", "Mensajeria", "NO", "KEEP",
     "Medido: 18 permisos de riesgo. App legitima del usuario.", "M"),
    ("us.zoom.videomeetings", "Zoom", "Videollamadas", "Videollamadas", "NO", "KEEP",
     "Medido: 17 permisos de riesgo. App legitima del usuario.", "M"),
    ("com.facebook.stella", "Messenger Lite", "Mensajeria", "Mensajeria ligera", "NO", "KEEP",
     "Medido: 20 permisos de riesgo. Si el usuario no lo usa, es una app mas que pesa.", "M"),
    ("com.samsung.android.spay", "Samsung Pay", "Pagos", "Pagos Samsung", "NO", "KEEP",
     "Medido: 18 permisos de riesgo. App de sistema del fabricante.", "M"),
    ("com.google.android.dialer", "Telefono (Google)", "Sistema", "Marcador y llamadas", "NO",
     "KEEP", "Medido: 18 permisos de riesgo. App de sistema.", "M"),
    ("com.samsung.android.oneconnect", "Samsung OneConnect", "Sistema", "Enlace con el PC Samsung",
     "NO", "KEEP", "Medido: 19 permisos de riesgo. Equivalente Samsung de Your Phone.", "M"),
    ("com.sec.android.easyMover.Agent", "EasyMover (Samsung)", "Sistema Samsung",
     "Migracion de datos entre moviles Samsung", "NO", "KEEP",
     "Medido: APK 3.195.723 B, sha256 3800f7fd..., DEX 1.460.228 B, 225 identificadores de 1-2 "
     "caracteres (ofuscados). Vive en /system/app: no se actualiza sin particion de sistema.", "A"),
    ("com.samsung.android.rampart", "Rampart (Samsung)", "Sistema Samsung",
     "Modulo de seguridad/anti-tamper de Samsung", "NO", "KEEP",
     "Medido: APK 4.681.780 B, sha256 fee09706..., DEX 2.265.340 B.", "A"),
    ("com.samsung.android.ipsgeofence", "IP Geofence (Samsung)", "Sistema Samsung",
     "Geofencing por IP (redes WiFi)", "NO", "KEEP",
     "Medido: APK 7.300.848 B, sha256 1b70519c..., DEX 5.673.768 B, 28.757 strings.", "A"),
    ("com.samsung.android.carkey", "CarKey (Samsung)", "Sistema Samsung",
     "Llaves de coche digitales; trae deteccion de manipulacion propia", "NO", "KEEP",
     "Medido: APK 7.708.419 B, sha256 e5def9f2..., DEX 8.378.044 B. Strings "
     "HiddenToOvercomeSignatureClash / TAMPER_FUSE_FAIL => sirve de REFERENCIA para el modulo "
     "anti-tamper del fleet.", "A"),
    ("com.samsung.android.knox.attestation", "Knox Attestation (Samsung)", "Sistema Samsung",
     "Cliente de Enhanced Attestation: enlaza con la cadena /DAK/ de la TEE", "NO", "KEEP",
     "Medido (P5): version 1.4.01.5, clase EnhancedAttestationAgent. Es el cliente del mundo "
     "seguro: no se toca.", "D"),
    ("com.samsung.android.smartcallprovider", "SmartCall (Samsung)", "Sistema Samsung",
     "Proveedor de llamadas inteligente (spam/identificacion)", "NO", "KEEP",
     "Medido (P5): version 15.0.01.5.", "D"),
    ("com.samsung.android.vtcamerasettings", "Camaras (Samsung)", "Sistema Samsung",
     "Ajustes de camara", "NO", "KEEP", "Medido (P5): version 4.0.04.16.", "D"),
    ("com.samsung.android.peripheral.framework", "Peripheral framework (Samsung)", "Sistema Samsung",
     "Marco de periféricos (BD, perfil)", "NO", "KEEP", "Medido (P5): version 2.5.01.0.", "D"),
    ("com.aura.oobe.samsung", "Manual Selector (OOBE del operador)", "Sistema / operador",
     "OOBE del operador: el CSC decide出厂 la configuracion regional", "NO", "KEEP",
     "Medido: sus ficheros cuelgan de /prism/etc/carriers/single/AMO/ con CSC "
     "com.orange.update => AMO = Orange Espana (ver docs/ANDROID_EXYNOS2100_FINDINGS.md 5.1).", "D"),
    ("com.samsung.android.app.contacts", "Contactos (Samsung)", "Sistema Samsung", "Contactos",
     "NO", "KEEP", "Medido (P5): version 15.1.87.", "D"),
    ("com.google.android.gms", "Google Play Services", "Sistema", "Servicios de Google",
     "NO", "KEEP", "Es el nucleo del movil; ademas hay una variante parcheada (ReVanced GMS).", "M"),
    ("com.android.chrome", "Chrome", "Navegador", "Navegador", "NO", "KEEP",
     "App legitima del usuario.", "P"),
    ("com.samsung.android.app.notes", "Samsung Notes", "Productividad", "Notas", "NO", "KEEP",
     "App de sistema.", "P"),
    ("com.acc.weather", "Samsung Weather", "Productividad", "Meteorologia", "NO", "KEEP",
     "App de sistema.", "P"),
    ("com.sec.android.gallery3d", "Galeria (Samsung)", "Productividad", "Galeria/fotos", "NO",
     "KEEP", "App de sistema.", "P"),
    ("com.samsung.android.dialer", "Telefono (Samsung)", "Sistema", "Marcador", "NO", "KEEP",
     "App de sistema.", "P"),
    ("com.google.android.apps.maps", "Maps (Google)", "Navegacion", "Mapas", "NO", "KEEP",
     "App legitima del usuario.", "P"),
    ("net.gsantner.markor", "Markor", "Notas", "Editor de notas (Markdown)", "NO", "KEEP",
     "Visible en el inventario real del movil (captura Apk Extractor).", "P"),
    ("gpyn.fi", "Marina", "RE / parcheador",
     "Marina: motor de parcheo ART en runtime con descarga de parches de terceros", "SI (motor de parcheo)",
     "DESINSTALAR",
     "Es el motor de parcheo PROPIETARIO de la familia Lucky Patcher: descarga parches de "
     "servidores de terceros y parchea ART en runtime (docs/SAMSUNG_APK_DEOBFUSCATION.md 2.2). "
     "Ademas pide root. Su funcion legitima la cubren LSPatch/KnoxPatch (open-source) y el motor "
     "Rust del fleet.", "D"),
    ("ru.drjmwdme.cgwbfzijr.x", "MarinaPatcher (variante)", "BASURA", "Variante del mismo APK",
     "NO", "DESINSTALAR", "Mismo binario reempaquetado; mismo criterio.", "A"),
    ("com.kitsumed.shizucallrecorder", "Shizuku Call Recorder", "Grabador (open-source)",
     "Grabador de llamadas sobre Shizuku, con codigo abierto", "SI (gestor de grabaciones)",
     "KEEP", "Open-source y sin cuenta: es la base natural del gestor de grabaciones del "
             "mega-binario (lista, metadatos, transcripcion, cifrado con el vault, subida).", "M"),
    ("com.github.axet.audiorecorder", "Audio Recorder (open-source)", "Grabador (open-source)",
     "Grabador de audio open-source", "SI (gestor de grabaciones)", "KEEP",
     "Open-source; el gestor del mega-binario lo sustituye cuando este.", "M"),
    ("com.digipom.easyvoicerecorder.pro", "Easy Voice Recorder Pro", "Grabador",
     "Grabador de voz comercial", "SI (gestor de grabaciones)", "KEEP",
     "Comercial; su funcion la cubre el gestor.", "M"),
    ("com.samsung.android.incallui", "InCallUI (Samsung)", "Sistema Samsung",
     "App de llamada en curso (la que dibuja el HUD de llamada)", "NO", "KEEP",
     "Medido por el carril DEX (el DEX sano de SamsungInCallUI fue el caso que destapo el bug "
     "del parser de tipos de valor).", "M"),
    ("com.samsung.android.game.gametools", "Game Booster (Samsung)", "Sistema Samsung",
     "Optimizacion de juegos (GPU/CPU)", "NO", "KEEP", "App de sistema del fabricante.", "M"),
]

# Reglas de veredicto (texto de §1 del doc)
REGLAS = [
    ("DESINSTALAR",
     "hay evidencia **medida** de que la app (a) sube datos del usuario a un servidor de "
     "terceros, (b) pide permisos peligrosos que no necesita para su funcion declarada, o "
     "(c) es una reempaquetada/pirata.  **Una app que sube datos del usuario a un servidor "
     "desconocido es DESINSTALAR, salvo que el usuario lo diga expresamente.**"),
    ("REIMPLEMENTAR EN RUST",
     "la funcion cabe en el mega-binario Rust de MegaDaemon (motor de parcheo DEX/APK, gestor "
     "de grabaciones, lector de backup, laboratorio de desofuscacion).  La app se puede quitar "
     "cuando el motor este."),
    ("KEEP",
     "no hay evidencia medida en contra: app legitima, o de sistema, o su funcion ya la "
     "necesitamos pero todavia no esta reimplementada."),
]


def load_inventory():
    if not os.path.exists(INVENTORY):
        sys.exit("FALTA %s (recuperable del historial: git show a56b1c3:inventory/inventory.json)"
                 % INVENTORY)
    with open(INVENTORY, encoding="utf-8") as f:
        return json.load(f)


def inv_index(inv):
    return {a["paquete"]: a for a in inv["apps"]["paquetes"]}


def risk_summary(app):
    """Riesgo MEDIDO de una app del inventario."""
    if not app:
        return "PENDIENTE (no esta en el inventario)"
    bits = []
    perms = app.get("permisos_riesgo") or []
    n = app.get("n_permisos_pedidos", len(perms))
    bits.append("%d permisos de riesgo" % len(perms))
    bits.append("%d pedidos" % n)
    if not app.get("habilitada", True):
        bits.append("DESHABILITADA")
    if app.get("es_sistema"):
        bits.append("sistema")
    else:
        bits.append("tercera")
    ts = app.get("target_sdk")
    if ts:
        bits.append("target_sdk %d" % ts)
    bits.append("firma %s" % app.get("firma_sha256_corto"))
    return ", ".join(bits)


def dangerous_perms(app):
    if not app:
        return []
    out = []
    for p in app.get("permisos_riesgo") or []:
        short = p.split(".")[-1]
        if short in ("RECORD_AUDIO", "READ_PHONE_STATE", "CAMERA", "ACCESS_FINE_LOCATION",
                     "ACCESS_COARSE_LOCATION", "SYSTEM_ALERT_WINDOW", "GET_ACCOUNTS",
                     "READ_CONTACTS", "READ_CALENDAR", "READ_SMS", "READ_CALL_LOG"):
            out.append(short)
    return out


def build(inv):
    idx = inv_index(inv)
    rows = []
    for (pkg, nombre, cat, func, rust, verdict, just, func_fuente) in CATALOG:
        app = idx.get(pkg)
        rows.append(OrderedDict([
            ("nombre", nombre),
            ("paquete", pkg),
            ("categoria", cat),
            ("funcion", func),
            ("rust", rust),
            ("veredicto", verdict),
            ("riesgo", risk_summary(app)),
            ("perm_peligrosos", ", ".join(dangerous_perms(app)) or "-"),
            ("justificacion", just),
            ("fuente", func_fuente),
            ("medida", bool(app)),
        ]))
    return rows


def md_table(rows):
    out = []
    out.append("| App | Paquete | Categoría | Qué aporta | ¿Reimplementar en Rust? | "
               "Riesgo **medido** | Veredicto |")
    out.append("|---|---|---|---|---|---|---|")
    for r in rows:
        out.append("| %s | `%s` | %s | %s | %s | %s | **%s** |" % (
            r["nombre"], r["paquete"], r["categoria"], r["funcion"], r["rust"],
            r["riesgo"], r["veredicto"]))
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()

    inv = load_inventory()
    rows = build(inv)
    n_med = sum(1 for r in rows if r["medida"])

    if a.json:
        print(json.dumps(rows, ensure_ascii=False, indent=1))
        return 0

    if a.check:
        if len(rows) < 40:
            print("FALLO: solo %d filas (se exigen >=40)" % len(rows))
            return 1
        sin_med = [r["paquete"] for r in rows if not r["medida"]]
        print("OK: %d filas (%d con datos medidos del inventario)" % (len(rows), n_med))
        if sin_med:
            print("sin medir (PENDIENTE): %s" % ", ".join(sin_med))
        return 0

    a_ = inv["apps"]
    doc = []
    doc.append("# Inventario de software del S21 Ultra con veredicto\n")
    doc.append("> Generado por `tools/verdict_inventory.py` desde el inventario **medido** del "
               "móvil (`inventory/inventory.json`, W2) + el análisis de APKs del fleet "
               "(W5/C2).  Regenerable: `python3 tools/verdict_inventory.py`.\n")
    doc.append("## 0. Qué es este documento y de dónde sale cada celda\n")
    doc.append("El móvil tiene **%d apps** (%d de usuario, %d de sistema, %d deshabilitadas) y "
               "**%d piden permisos de riesgo**.  El inventario guarda por app: versión, "
               "`firma_sha256` (truncado a 4 bytes por Android 14), permisos de riesgo, "
               "`target_sdk`, estado y ruta del APK.\n"
               % (len(a_["paquetes"]), a_["de_usuario"], a_["de_sistema"],
                  a_["deshabilitadas_total"], a_["con_permisos_de_riesgo"]))
    doc.append("Cada fila mezcla **dato medido** y **juicio**:\n")
    doc.append("| Marca | Significado |")
    doc.append("|---|---|")
    doc.append("| **M** | Medido en el móvil (inventario W2) o en el APK (hashes, DEX, "
               "artefactos embebidos) |")
    doc.append("| **D** | Deducido del repo: medidas ya publicadas por el fleet |")
    doc.append("| **P** | **PENDIENTE de medición**: el móvil lleva caído desde las 03:37 UTC "
               "(túneles 530) ⇒ no se inventa el dato, se marca |")
    doc.append("")
    doc.append("En la columna *Riesgo* sólo hay **hechos** del inventario (permisos, firma, "
               "`target_sdk`, estado).  Cuando el riesgo real depende de algo que aún no se ha "
               "medido (red, telemetría, uploading), la fila lo dice explícitamente en la "
               "justificación.\n")
    doc.append("## 1. Reglas de veredicto\n")
    for name, txt in REGLAS:
        doc.append("- **%s** — %s\n" % (name, txt))
    doc.append("## 2. Veredicto por app (%d filas; %d con datos medidos del inventario)\n"
               % (len(rows), n_med))
    doc.append(md_table(rows))
    doc.append("")
    doc.append("## 3. Los casos que necesitan una decisión **del usuario**\n")
    doc.append("1. **MarinaPatcher** (`ru.drjmwdme.cgwbfzijr`): *DESINSTALAR*.  Es Lucky "
               "Patcher con otro nombre y otro icono, y **sube el APK que parchea a un "
               "servidor de terceros** (`patch.chelpus.com/submit`) con IPs crudas y sin TLS.  "
               "Además pide root y es un módulo Xposed.  Si el usuario dice que lo quiere, se "
               "queda; si no, se va.")
    doc.append("2. **Marina** (`g.pyn.fi`): *DESINSTALAR* (su función la cubren LSPatch/KnoxPatch "
               "open-source y el motor Rust del fleet).")
    doc.append("3. **Relictos de smartwatch** (Huami/Amazfit/Mi Fitness/Kids Watch/KDE Connect "
               "teléfono): *DESINSTALAR* — están deshabilitados y piden 18-21 permisos de riesgo "
               "en un móvil sin reloj conectado.")
    doc.append("4. **Microsoft Your Phone / Link to Windows**: *KEEP*, pero son las **únicas 2 "
               "apps que declaran `CAPTURE_AUDIO_OUTPUT`** en el móvil (medido sobre 398 APKs / "
               "5.580 MB).  Si el usuario quierePrivacy al grabador, estas son las que hay que "
               "mirar.\n")
    doc.append("## 4. Ahorro\n")
    doc.append("Lo que se puede quitar sin perder nada de lo propio del móvil: **5 relictos "
               "de wearable** + **Marina** + **MarinaPatcher** + **ADB tools/adb captain** "
               "(su función ya está en `megadaemon-adb`, Rust puro).  Los dos APK de ADB solos "
               "son ~35 MB.  El resto son apps del usuario: la decisión es suya, no nuestra.\n")
    doc.append("## 5. Lo que falta por medir (móvil caído)\n")
    doc.append("Sin canal al móvil no se puede cerrar el inventario.  En cuanto vuelva:\n")
    doc.append("1. `pm list packages -3 -U` + `dumpsys package <pkg> | grep -A40 permissions` "
               "para los paquetes marcados **P**.")
    doc.append("2. Tamaño real por APK (`stat -c %s <ruta_apk>`) — el inventario guarda la ruta, "
               "no el tamaño.")
    doc.append("3. `CAPTURE_AUDIO_OUTPUT` y `REQUEST_INSTALL_PACKAGES` no están en la lista de "
               "27 permisos del inventario: barrido de APKs como el que ya se hizo (398 APKs / "
               "5.580 MB) para cerrarlos.")
    doc.append("4. Confirmar qué wearables/relactos quiere conservar el usuario.\n")
    doc.append("## 6. Reproducir\n")
    doc.append("```bash")
    doc.append("# regenerar este documento")
    doc.append("python3 tools/verdict_inventory.py")
    doc.append("# solo validar (>=40 filas)")
    doc.append("python3 tools/verdict_inventory.py --check")
    doc.append("# el inventario medido (recuperado del historial si lo perdio un reset)")
    doc.append("git show a56b1c3:inventory/inventory.json > inventory/inventory.json")
    doc.append("```\n")

    with open(OUT_MD, "w", encoding="utf-8") as f:
        f.write("\n".join(doc))
    print("escrito %s (%d filas, %d medidas)" % (OUT_MD, len(rows), n_med))
    return 0


if __name__ == "__main__":
    sys.exit(main())

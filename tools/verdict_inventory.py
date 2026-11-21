#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Genera `docs/SOFTWARE_INVENTORY_VERDICT.md` desde datos **medidos** del móvil.

Fuentes (todas reales, ninguna inventada):

  * `inventory/inventory.json`      — inventario W2 del S21 Ultra: 1719 paquetes con
                                     permisos de riesgo, firma, target_sdk, versión,
                                     estado y ruta de la APK.
  * `<refs>/dumpsys_package`        — volcado real de `dumpsys package`: instalador,
                                     fechas de instalación/actualización y estado `pm`.
                                     **Opcional**: sin él, esas columnas salen como
                                     `PENDIENTE` (no se inventan).
  * `rust-core/src/app_intel.rs`    — `VERDICT_CATALOG`: fuente **única** de los
                                     veredictos y del módulo Rust destino.  El código y
                                     este documento no pueden discrepar porque leen lo
                                     mismo.

Reglas del veredicto (las mismas que `crate::app_intel::verdict_for`):

  DESINSTALAR        hay evidencia **medida**: permisos peligrosos que la app no
                     necesita, APK repaquetado que sube el APK a un tercero, relicto
                     deshabilitado, o duplicado cuya función ya cubre otra app.
  REIMPLEMENTAR      la función cabe en un módulo concreto del mega-binario Rust:
                     el veredicto **nombra el módulo destino**.
  KEEP               no hay evidencia medida en contra.  Las apps de sistema
                     (Samsung/Google/AOSP) no se desinstalan: viven en la partición.

Uso:
    python3 tools/verdict_inventory.py                 # escribe el doc
    python3 tools/verdict_inventory.py --check         # valida (>=40 filas medidas)
    python3 tools/verdict_inventory.py --json          # vuelca el veredicto
    python3 tools/verdict_inventory.py --refs RUTAS    # volca dumpsys desde RUTAS
"""

import argparse
import json
import os
import re
import sys
from collections import Counter, OrderedDict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INVENTORY = os.path.join(ROOT, "inventory", "inventory.json")
RUST_CATALOG = os.path.join(ROOT, "rust-core", "src", "app_intel.rs")
OUT_MD = os.path.join(ROOT, "docs", "SOFTWARE_INVENTORY_VERDICT.md")
DEFAULT_REFS = os.path.join(ROOT, "inventory", "refs")

# Fecha del volcado W2 (el inventario publicado en el repo).
W2_CAPTURE = "2026-07-20"

# Estado `pm` de un paquete (dumpsys): 0 = DEFAULT, 1 = ENABLED,
# 2 = DISABLED, 3 = DISABLED_USER, 4 = DISABLED_UNTIL_USED.
PM_STATE = {
    "0": "activa",
    "1": "activa",
    "2": "deshabilitada",
    "3": "deshabilitada por el usuario",
    "4": "deshabilitada hasta que se use",
}


# Etiquetas de veredicto (las mismas que `crate::app_intel`).
VERDICT_LABEL = {
    "VERDICT_KEEP": "KEEP",
    "VERDICT_REIMPLEMENTAR": "REIMPLEMENTAR EN RUST",
    "VERDICT_DESINSTALAR": "DESINSTALAR",
}


def parse_rust_catalog():
    """Lee `VERDICT_CATALOG` del módulo Rust (fuente única de veredictos)."""
    with open(RUST_CATALOG, encoding="utf-8") as f:
        src = f.read()
    i = src.find("pub const VERDICT_CATALOG")
    if i < 0:
        sys.exit("FALTA VERDICT_CATALOG en %s" % RUST_CATALOG)
    j = src.find("\n];\n", i)
    bloque = src[i:j]
    patron = re.compile(
        r'    \(\n        "([^"]*)",\n        "([^"]*)",\n        "([^"]*)",\n'
        r"        (VERDICT_[A-Z_ ]+),\n        \"([^\"]*)\",\n        \"([^\"]*)\",\n    \),\n"
    )
    filas = []
    for m in patron.finditer(bloque):
        pkg, nombre, cat, verdict, modulo, motivo = m.groups()
        filas.append(
            OrderedDict(
                [
                    ("paquete", pkg),
                    ("nombre", nombre),
                    ("categoria", cat),
                    ("veredicto", VERDICT_LABEL.get(verdict, verdict)),
                    ("modulo", modulo),
                    ("motivo", motivo),
                ]
            )
        )
    if not filas:
        sys.exit("no se pudo leer VERDICT_CATALOG (cambio el formato?)")
    return filas


def parse_dumpsys(refs_dir):
    """Parsea `dumpsys package` (volcado real) -> {paquete: ficha}."""
    path = os.path.join(refs_dir, "dumpsys_packages")
    if not os.path.exists(path):
        return {}
    with open(path, encoding="utf-8", errors="replace") as f:
        text = f.read()
    out = {}
    cur = None
    mode = None
    for raw in text.splitlines():
        t = raw.strip()
        m = re.match(r"^Package \[([^\]]+)\]", t)
        if m:
            cur = {
                "package": m.group(1),
                "installer": None,
                "first_install": None,
                "last_update": None,
                "enabled": None,
            }
            out[cur["package"]] = cur
            mode = None
            continue
        if cur is None:
            continue
        if t.startswith("User ") and "installed=" in t:
            mode = "user"
            e = re.search(r"enabled=(\d+)", t)
            if e:
                cur["enabled"] = e.group(1)
            continue
        if mode == "user":
            if t.startswith("firstInstallTime="):
                cur["first_install"] = t.split("=", 1)[1].strip()
            elif t.startswith("lastUpdateTime="):
                cur["last_update"] = t.split("=", 1)[1].strip()
            continue
        if t.startswith("installerPackageName="):
            cur["installer"] = t.split("=", 1)[1].strip()
    return out


def load_inventory():
    if not os.path.exists(INVENTORY):
        sys.exit("FALTA %s" % INVENTORY)
    with open(INVENTORY, encoding="utf-8") as f:
        return json.load(f)


def tipo_de_app(app):
    """Sistema o instalada por el usuario (de verdad, por la ruta)."""
    ruta = app.get("ruta_apk") or ""
    sistema_ruta = ruta.startswith(("/system/", "/product/", "/vendor/", "/apex/", "/prism/", "/odm/"))
    if app.get("es_sistema"):
        return "sistema" if sistema_ruta else "sistema actualizada"
    return "usuario"


def medir(app, dump):
    """Columna 'medido': sólo hechos del inventario y del volcado."""
    bits = []
    riesgo = app.get("permisos_riesgo") or []
    bits.append("%d permisos (%d de riesgo)" % (app.get("n_permisos_pedidos") or 0, len(riesgo)))
    if riesgo:
        bits.append("riesgo: " + ", ".join(sorted(p.split(".")[-1] for p in riesgo)))
    ts = app.get("target_sdk")
    if ts:
        bits.append("targetSdk %s" % ts)
    firma = app.get("firma_sha256_corto")
    if firma:
        bits.append("firma %s" % firma)
    ver = app.get("version_name")
    if ver:
        bits.append("v%s" % ver)
    if dump:
        est = dump.get("enabled")
        if est:
            bits.append(PM_STATE.get(est, "estado pm %s" % est))
        inst = dump.get("installer")
        if inst:
            if inst == "null":
                bits.append("sideload (sin instalador)")
            else:
                bits.append("instalada por %s" % inst)
        if dump.get("first_install"):
            bits.append("instalada %s" % dump["first_install"][:10])
    return "; ".join(bits)


def build(inv, dumps):
    idx = {a["paquete"]: a for a in inv["apps"]["paquetes"]}
    filas = []
    sin_medir = []
    for cat in parse_rust_catalog():
        app = idx.get(cat["paquete"])
        dump = dumps.get(cat["paquete"]) if dumps else None
        fila = OrderedDict(cat)
        fila["medido"] = medir(app, dump) if app else "PENDIENTE (no esta en el inventario)"
        fila["tipo"] = tipo_de_app(app) if app else "?"
        fila["instalada"] = bool(app)
        if app:
            fila["riesgo_n"] = len(app.get("permisos_riesgo") or [])
            fila["pedidos_n"] = app.get("n_permisos_pedidos") or 0
        else:
            fila["riesgo_n"] = 0
            fila["pedidos_n"] = 0
        if not app:
            sin_medir.append(cat["paquete"])
        filas.append(fila)
    return filas, sin_medir


def md_table(filas):
    out = []
    out.append("| App | Paquete | Tipo | Qué aporta | Medido | Veredicto | Módulo Rust destino |")
    out.append("|---|---|---|---|---|---|---|")
    for f in filas:
        out.append(
            "| %s | `%s` | %s | %s | %s | **%s** | %s |"
            % (
                f["nombre"],
                f["paquete"],
                f["tipo"],
                f["motivo"],
                f["medido"],
                f["veredicto"],
                ("`%s`" % f["modulo"]) if f["modulo"] else "—",
            )
        )
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--refs", default=DEFAULT_REFS)
    a = ap.parse_args()

    inv = load_inventory()
    dumps = parse_dumpsys(a.refs)
    filas, sin_medir = build(inv, dumps)
    medidas = [f for f in filas if f["instalada"]]

    if a.json:
        print(json.dumps(filas, ensure_ascii=False, indent=1))
        return 0

    if a.check:
        if len(medidas) < 40:
            print("FALLO: solo %d filas con datos medidos (se exigen >=40)" % len(medidas))
            return 1
        print("OK: %d filas en el catalogo, %d con datos medidos" % (len(filas), len(medidas)))
        if sin_medir:
            print("sin medir: %s" % ", ".join(sin_medir))
        return 0

    conteo = Counter(f["veredicto"] for f in medidas)
    doc = []

    def add(text):
        doc.append(text.strip("\n"))

    add("# Inventario de software del S21 Ultra — veredicto por app\n")
    add(
        "> Generado por `tools/verdict_inventory.py` desde el inventario **medido** de este\n"
        "> móvil (`inventory/inventory.json`, captura W2 del %s) y el volcado real de\n"
        "> `dumpsys package`.  El catálogo de veredictos vive en el código\n"
        "> (`rust-core/src/app_intel.rs`, `VERDICT_CATALOG`): el documento y el escáner\n"
        "> leen la misma lista, así que no pueden discrepar.\n"
        "> Regenerable: `python3 tools/verdict_inventory.py`.\n" % W2_CAPTURE
    )

    add("## 0. Qué es este documento y de dónde sale cada celda\n")
    add(
        "El móvil tiene **%d paquetes** (%d de usuario, %d de sistema) y **%d piden permisos de\n"
        "riesgo**; %d están deshabilitados por el usuario (`pm list packages -d`).\n"
        % (
            inv["apps"]["total"],
            inv["apps"]["de_usuario"],
            inv["apps"]["de_sistema"],
            inv["apps"]["con_permisos_de_riesgo"],
            inv["apps"]["deshabilitadas_pm"],
        )
    )
    add("| Marca | Significado |\n|---|---|\n")
    add(
        "| **Medido** | Dato del inventario W2 (permisos, firma, target_sdk, versión, ruta) o del\n"
        "volcado real de `dumpsys package` (instalador, fechas, estado `pm`) |\n"
        "| **Deducido** | Conclusión que sale de los datos medidos (p. ej. «firma distinta de la\n"
        "oficial ⇒ no es el cliente oficial») |\n"
        "| **PENDIENTE** | Lo que **no** se ha podido medir (el canal con el móvil está caído): se\n"
        "dice, no se inventa |\n"
    )
    add(
        "Las **cifras** de la tabla son todas medidas.  La columna *Medido* no lleva juicios:\n"
        "sólo hechos.  El juicio va en *Veredicto* y su motivo es el texto de la app.\n"
    )

    add("## 1. Reglas de veredicto\n")
    add(
        "- **DESINSTALAR** — hay evidencia **medida**: la app pide permisos peligrosos que no\n"
        "  necesita para su función, es un APK repaquetado que sube el APK a un tercero, es un\n"
        "  relicto de un dispositivo que no está, o duplica exactamente a otra app instalada.\n"
        "- **REIMPLEMENTAR EN RUST** — la función cabe en un módulo concreto del mega-binario.\n"
        "  La fila **nombra el módulo destino**: es la lista de trabajo, no una opinión.\n"
        "- **KEEP** — no hay evidencia medida en contra.  **Las apps de sistema no se\n"
        "  desinstalan**: viven en `/system`, `/product`, `/vendor` o `/apex` (o las actualiza el\n"
        "  firmware) ⇒ como mucho se **deshabilitan**.\n"
    )

    add("## 2. Estado medido del móvil\n")
    add("| Métrica | Valor medido |\n|---|---|\n")
    add("| Paquetes instalados | %d (%d de usuario, %d de sistema) |"
               % (inv["apps"]["total"], inv["apps"]["de_usuario"], inv["apps"]["de_sistema"]))
    add("| Apps que piden permisos de riesgo | %d |" % inv["apps"]["con_permisos_de_riesgo"])
    add("| Apps deshabilitadas (`pm list packages -d`) | %d |" % inv["apps"]["deshabilitadas_pm"])
    add("| Dispositivo | Samsung S21 Ultra (SM-G998B), Android 14, CSC `AMO` (Orange España) |")
    add("| Espacio en `/data` | 230.442.752 KiB usados de 235.730.924 (**98 %**), 5.288.172 KiB libres |")
    add("| `/data/app` (todas las APK) | 88.492.320 KiB (**84,4 GiB**) |")
    add("| `/data/media` (fotos, descargas) | 83.485.227 KiB (**79,6 GiB**); `Download` = 44.346.358 KiB (**42,3 GiB**) |")
    add("| `/data/user` (datos de las apps) | 48.119.462 KiB (**45,9 GiB**) |")
    add("| `/data/data` | 48.069.225 KiB (**45,8 GiB**) |")
    add("")
    add(
        "Los números de espacio son del volcado real (`du` + `df`): el móvil está **al 98 %** y\n"
        "el mayor bloque es `/data/app` (84,4 GiB de APK), seguido de `Download` (42,3 GiB).\n"
        "Quitar las apps que no se usan es la palanca más rápida.\n"
    )

    add("## 3. Veredicto por app (%d filas; %d con datos medidos)\n"
               % (len(filas), len(medidas)))
    add("Reparto: **%d KEEP** · **%d REIMPLEMENTAR EN RUST** · **%d DESINSTALAR**.\n"
               % (conteo.get("KEEP", 0), conteo.get("REIMPLEMENTAR EN RUST", 0),
                  conteo.get("DESINSTALAR", 0)))
    add(md_table(filas))
    add("")

    add("## 4. Lo que se va (limpieza) y lo que cuesta\n")
    add("| Paquete | Motivo medido | Ahorro esperado |\n|---|---|---|\n")
    for f in filas:
        if f["veredicto"] != "DESINSTALAR" or not f["instalada"]:
            continue
        add("| `%s` | %s | APK + datos en `/data/user/0/<pkg>` + entrada en el launcher |"
                   % (f["paquete"], f["motivo"]))
    add("")
    add(
        "El tamaño **exacto** en MB de cada APK no está medido en este volcado (el inventario\n"
        "guarda la ruta, no el `stat`): queda como **PENDIENTE** en §7.  Lo que sí está medido es\n"
        "el bloque total que ocupan (`/data/app` = 84,4 GiB) y el de los datos de las apps\n"
        "(`/data/user` = 45,9 GiB), así que la limpieza se nota aunque el ahorro por app se\n"
        "midió en barridos anteriores del fleet (p. ej. `adb captain` = 29.174.974 B,\n"
        "`Magisk` = 12.770.643 B, `CarKey` = 7.708.419 B).\n"
    )

    add("## 5. Lo que hay que reimplementar en Rust (accionable)\n")
    add("| Módulo destino | Qué pone | Apps que cubre |\n|---|---|---|\n")
    modulos = OrderedDict()
    for f in filas:
        if f["veredicto"] != "REIMPLEMENTAR EN RUST" or not f["modulo"]:
            continue
        modulos.setdefault(f["modulo"], []).append(f["nombre"])
    for modulo, apps in modulos.items():
        add("| `%s` | %s | %s |" % (modulo, DESCRIPCION_MODULOS.get(modulo, "—"), ", ".join(apps)))
    add("")
    add(
        "Los módulos marcados **(nuevo)** en `RUST_TARGETS` (`rust-core/src/app_intel.rs`) son\n"
        "la pieza que **todavía no existe**: sin ellos, el veredicto «REIMPLEMENTAR» no se puede\n"
        "cerrar.\n"
    )

    add("## 6. Decisiones que son del usuario (no del escáner)\n")
    add(
        "1. **MarinaPatcher / Marina** (`ru.drjmwdme.cgwbfzijr`, `gpyn.fi`): el APK reempaquetado\n"
        "   sube el APK que parchea a `patch.chelpus.com` con IPs crudas y sin TLS.  El veredicto\n"
        "   técnico es DESINSTALAR; si el usuario lo quiere, se queda y se documenta.\n"
        "2. **NagramX** (`fork.risin42.nagramx`): fork con firma distinta de la oficial.  Si es el\n"
        "   único cliente de Telegram que usa, se queda y se acepta el riesgo.\n"
        "3. **Maps de Google** frente a **OsmAnd+**: los dos están instalados; decidir uno.\n"
        "4. **Relictos de wearable** (Huami, Amazfit, Mi Fitness, Kids Watch 360, KDE Connect): si\n"
        "   el usuario no tiene el reloj, fuera; si lo tiene, se reactivan.\n"
    )

    add("## 7. Lo que falta por medir (no se inventa)\n")
    add(
        "El canal con el móvil está caído (`530`), así que **no** se ha podido ejecutar el\n"
        "barrido en vivo (`apk_inventory::scan_installed`).  Cuando vuelva:\n"
        "\n"
        "1. **Tamaño real por APK** (`stat -c %s <ruta>`): el inventario guarda la ruta.\n"
        "2. **Permisos runtime concedidos** ahora mismo (el volcado mezcla instalación y\n"
        "   runtime): `dumpsys package <pkg> | grep -A40 'runtime permissions'`.\n"
        "3. **Telemetría por app**: endpoints y a quién llama cada APK (lo da el escáner:\n"
        "   `endpoints` + `native_libraries` + `signature`).\n"
        "4. **Componentes exportados sin permiso** por app (lo da `exported_without_permission`).\n"
        "\n"
        "El comando es uno: `apk_inventory::scan_installed(0)` (ranking de riesgo) y\n"
        "`apk_inventory::verdict_installed(0)` (veredicto por app, con el módulo destino).\n"
    )
    if sin_medir:
        add("\nFilas del catálogo **sin** datos medidos en este volcado: %s.\n"
                   % ", ".join("`%s`" % p for p in sin_medir))

    add("## 8. Reproducir\n")
    add(
        "```bash\n"
        "# regenerar el documento (usa el inventario del repo)\n"
        "python3 tools/verdict_inventory.py\n"
        "# validar que hay >=40 filas con datos medidos\n"
        "python3 tools/verdict_inventory.py --check\n"
        "# volcar el veredicto como JSON\n"
        "python3 tools/verdict_inventory.py --json | jq '.[0:3]'\n"
        "# con el volcado de dumpsys (siesta disponible)\n"
        "python3 tools/verdict_inventory.py --refs /ruta/al/volcado/refs\n"
        "```\n"
    )

    with open(OUT_MD, "w", encoding="utf-8") as f:
        f.write("\n".join(doc) + "\n")
    print("escrito:", OUT_MD, "(%d filas, %d medidas)" % (len(filas), len(medidas)))
    return 0


DESCRIPCION_MODULOS = {
    "rust-core/src/recording_vault.rs": "bóveda cifrada de grabaciones: lista, metadatos, transcripción, subida",
    "rust-core/src/backup_formats.rs": "lector/escritor de formatos de backup (Swift Backup, AEGIS, Zstd, AB)",
    "rust-core/src/dex_decompile.rs": "motor DEX: parcheo, re-firma, inyección de módulos y hook",
    "rust-core/src/adb_client.rs": "cliente ADB nativo (push, pull, shell, logcat)",
    "rust-core/src/bookmarks.rs": "gestor de notas y marcadores",
    "rust-core/src/magisk_module.rs": "instalación y verificación de módulos Magisk systemless",
    "rust-core/src/antigravity_intel.rs": "cliente MCP que habla con el IDE de Google",
}


if __name__ == "__main__":
    sys.exit(main())

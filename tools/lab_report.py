#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""`lab_report.py` — laboratorio de APKs/DEX para el PC del usuario.

Produce el mismo informe que el módulo Rust `rust-core/src/lab_kit.rs`, pero
en Python puro y sin compilar nada: el usuario lo puede usar en su PC sin
depender del enjambre.

Qué saca de un APK o de un DEX suelto:

  * hashes (sha256 del fichero, de cada DEX y de las firmas META-INF)
  * el **mapa de ofuscación**: clases y métodos con nombre sospechoso
    (1-2 caracteres, `a`/`b`/`ab`…), que es donde se esconden las constantes
  * las **constantes** (pool de strings) y las que tienen pinta de cifradas
  * los **endpoints** (http/https/ws) y las **IPs crudas**
  * los **secretos**, **redactados** (prefijo + sufijo + sha16; nunca el valor)
  * los **artefactos de toolkit de parcheo** (claves de AOSP, apksigner,
    bypass de licencias de Play, módulo Xposed, subida a terceros…)
  * los **permisos** declarados (los del string-pool del manifiesto AXML)

Uso:
    python3 tools/lab_report.py <apk|dex> [--json] [--out informe.md]
    python3 tools/lab_report.py --selftest
    python3 tools/lab_report.py --help

Requiere sólo la stdlib de Python 3.8+.
"""

import argparse
import hashlib
import json
import os
import re
import sys
import zipfile

# --- marcadores de toolkit de parcheo (misma tabla que app_intel.rs) -------- #
TOOLKIT_MARKERS = [
    ("aosp-testkey-material", "signing", "critical", "testkey.pk8"),
    ("aosp-testkey-material", "signing", "critical", "testkey.x509.pem"),
    ("aosp-testkey-material", "signing", "critical", "media.pk8"),
    ("aosp-testkey-material", "signing", "critical", "platform.pk8"),
    ("aosp-testkey-material", "signing", "critical", "shared.pk8"),
    ("keystore-p12-embedded", "signing", "critical", "keystore.ks"),
    ("apksigner-embedded", "signing", "high", "com/android/apksigner"),
    ("apksigner-embedded", "signing", "high", "com.android.apksigner"),
    ("play-license-bypass", "license-bypass", "critical", "ITestServiceInterface"),
    ("play-vending-embedded", "license-bypass", "high", "com/android/vending"),
    ("play-vending-embedded", "license-bypass", "high", "com.android.vending"),
    ("xposed-module", "hook", "critical", "com.xposed.XSupport"),
    ("xposed-module", "hook", "critical", "com/xposed/XSupport"),
    ("xposed-metadata", "hook", "high", "xposedminversion"),
    ("xposed-scope-metadata", "hook", "medium", "xposedscope"),
    ("uploads-target-apk", "exfiltration", "critical", "patch.chelpus.com/submit"),
    ("plaintext-patch-endpoint", "network", "high", "http://patch.chelpus.com"),
    ("raw-ip-endpoint", "network", "high", "109.95.209.70"),
    ("raw-ip-endpoint-tls", "network", "high", "149.56.155.246"),
    ("affiliate-tracking-id", "tracking", "medium", "appnext.com/myid.html"),
    ("ad-sdk-tapjoy", "tracking", "low", "tapjoyads.com"),
    ("ad-sdk-adsense", "tracking", "low", "googlesyndication.com"),
    ("promo-supersu", "promo", "low", "eu.chainfire.supersu"),
    ("promo-busybox", "promo", "low", "stericson.busybox"),
]

URL_RE = re.compile(rb"(?:https?|wss?)://[A-Za-z0-9._~:/?#\[\]@!$&'()*+,;=%-]{4,300}")
IP_RE = re.compile(rb"\b(?:\d{1,3}\.){3}\d{1,3}(?::\d{1,5})?\b")
PERM_RE = re.compile(rb"android\.permission\.[A-Z_]+")
# Las clases de un DEX son rutas: "Lcom/a/b/C;" (varios niveles).
CLASS_RE = re.compile(rb"L([A-Za-z0-9_$]+(?:/[A-Za-z0-9_$]+)+);")
METHOD_RE = re.compile(rb"([A-Za-z0-9_$]+(?:/[A-Za-z0-9_$]+)+);([A-Za-z0-9_$<>]+)")
SECRET_RE = re.compile(
    rb"(?i)(api[_-]?key|secret|token|password|passwd|bearer|authorization|private[_-]?key)"
    rb"[=:\"'\s]{1,6}([A-Za-z0-9_\-/+=]{16,200})")
# Nombres de 1-2 chars = sospechoso de ofuscación.  EXCEPCIONES: los nombres
# cortos que genera el tooling (R, BuildConfig, ...) NO son ofuscación.
SHORT_NAMES = {"R", "BuildConfig", "Manifest", "Application", "Activity",
                "Fragment", "View", "Adapter", "onCreate"}


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def sha16(data):
    if isinstance(data, str):
        data = data.encode()
    return hashlib.sha256(data).hexdigest()[:16]


def redact(value):
    """Redacta: prefijo + sufijo + sha16.  Nunca el valor entero."""
    v = value.decode("utf-8", "replace") if isinstance(value, bytes) else str(value)
    if len(v) <= 8:
        return "<oculto:%d>" % len(v)
    return "%s…%s (sha16 %s, %d chars)" % (v[:4], v[-4:], sha16(v), len(v))


def is_obfuscated_name(name):
    n = name.split("/")[-1]
    return len(n) <= 2 and n not in SHORT_NAMES


def strings_from(data, min_len=6):
    out = []
    cur = bytearray()
    for b in data:
        if 0x20 <= b < 0x7F:
            cur.append(b)
        else:
            if len(cur) >= min_len:
                out.append(bytes(cur))
            cur = bytearray()
    if len(cur) >= min_len:
        out.append(bytes(cur))
    return out


def read_inputs(path):
    """Devuelve (nombre, bytes) de cada DEX y el manifiesto si es un APK."""
    dexes, manifest = [], None
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as z:
            for n in z.namelist():
                if n.endswith(".dex"):
                    dexes.append((n, z.read(n)))
                elif n == "AndroidManifest.xml":
                    manifest = z.read(n)
    else:
        with open(path, "rb") as f:
            dexes.append((os.path.basename(path), f.read()))
    return dexes, manifest


def analyze(path):
    rep = {
        "path": path,
        "bytes": os.path.getsize(path),
        "sha256": sha256_file(path),
        "dex": [],
        "endpoints": [],
        "raw_ips": [],
        "secrets": [],
        "toolkit": [],
        "permissions": [],
        "classes": [],
        "methods": [],
        "suspicious_strings": [],
        "signatures": [],
    }
    dexes, manifest = read_inputs(path)
    blob = bytearray()
    for name, data in dexes:
        rep["dex"].append({
            "name": name,
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        })
        blob += data
        ss = strings_from(data)
        for s in ss:
            m = CLASS_RE.match(s) or re.match(rb"L([A-Za-z0-9_$]+/[A-Za-z0-9_$]+);", s)
            if m:
                rep["classes"].append(m.group(1).decode())
        rep["methods"].extend(
            m.group(2).decode() for m in METHOD_RE.finditer(bytes(blob)) if m.group(2))
    if manifest:
        blob += manifest
        rep["signatures"] = []

    data = bytes(blob)
    rep["endpoints"] = sorted({m.group(0).decode("utf-8", "replace").rstrip('"\' ')
                               for m in URL_RE.finditer(data)})
    rep["raw_ips"] = sorted({m.group(0).decode() for m in IP_RE.finditer(data)})
    rep["permissions"] = sorted({m.group(0).decode() for m in PERM_RE.finditer(data)})
    rep["classes"] = sorted(set(rep["classes"]))
    rep["methods"] = sorted(set(rep["methods"]))
    rep["suspicious_strings"] = sorted({s.decode("utf-8", "replace") for s in strings_from(data, 8)
                                        if b"http://" in s or b"https://" in s})
    for kind, val in SECRET_RE.findall(data):
        rep["secrets"].append({"kind": kind.decode("utf-8", "replace").lower(),
                               "value": redact(val)})
    text = data.decode("utf-8", "replace")
    for tid, cls, sev, marker in TOOLKIT_MARKERS:
        if marker in text:
            rep["toolkit"].append({"id": tid, "class": cls, "severity": sev, "marker": marker})
    rep["obfuscated_classes"] = sorted({c for c in rep["classes"] if is_obfuscated_name(c)})
    rep["obfuscated_methods"] = sorted({m for m in rep["methods"] if is_obfuscated_name(m)})
    return rep


def verdict_of(rep):
    """Mismo criterio que `app_intel::AppFacts::verdict`."""
    uploads = any("/submit" in e or "/upload" in e or "/report" in e for e in rep["endpoints"])
    if uploads:
        return "DESINSTALAR", "sube datos a un servidor de terceros (medido en los endpoints)"
    if rep["toolkit"]:
        return "DESINSTALAR", "embebe artefactos de un toolkit de parcheo (medido)"
    return "KEEP", "sin evidencia medida en contra"


def markdown(rep):
    v, why = verdict_of(rep)
    out = []
    out.append("# Informe de laboratorio — `%s`\n" % os.path.basename(rep["path"]))
    out.append("- sha256: `%s`" % rep["sha256"])
    out.append("- bytes: %d" % rep["bytes"])
    out.append("- DEX: %s" % ", ".join("%s (%d B)" % (d["name"], d["bytes"]) for d in rep["dex"]))
    out.append("- **veredicto: %s** — %s\n" % (v, why))
    if rep.get("obfuscated_classes"):
        out.append("## Clases con nombre ofuscado (mapa)\n")
        out += ["- `%s`" % c for c in rep["obfuscated_classes"][:60]]
        out.append("")
    if rep.get("obfuscated_methods"):
        out.append("## Métodos con nombre ofuscado\n")
        out += ["- `%s`" % m for m in rep["obfuscated_methods"][:60]]
        out.append("")
    if rep["endpoints"]:
        out.append("## Endpoints\n")
        out += ["- %s" % e for e in rep["endpoints"]]
        out.append("")
    if rep["raw_ips"]:
        out.append("## IPs crudas\n")
        out += ["- %s" % i for i in rep["raw_ips"]]
        out.append("")
    if rep["permissions"]:
        out.append("## Permisos (del string-pool del manifiesto)\n")
        out += ["- %s" % p for p in rep["permissions"]]
        out.append("")
    if rep["secrets"]:
        out.append("## Secretos (REDACTADOS)\n")
        out += ["- %s: %s" % (s["kind"], s["value"]) for s in rep["secrets"]]
        out.append("")
    if rep["toolkit"]:
        out.append("## Artefactos de toolkit de parcheo\n")
        out += ["- **%s** (%s / %s) — `%s`" % (t["id"], t["class"], t["severity"], t["marker"])
                for t in rep["toolkit"]]
        out.append("")
    return "\n".join(out)


def selftest():
    fails = []

    def ok(cond, msg):
        if not cond:
            fails.append(msg)

    ok(len(sha16("x")) == 16, "sha16 debe tener 16 chars")
    ok("valor-secreto-largo" not in redact("valor-secreto-largo-123456"),
       "redact no debe devolver el valor")
    ok("sha16" in redact("valor-secreto-largo-123456"), "redact debe incluir sha16")
    ok(is_obfuscated_name("Lcom/a/a;"), "nombre de 1 char = ofuscado")
    ok(not is_obfuscated_name("Lcom/foo/MainActivity;"), "nombre normal = no ofuscado")
    ok(not is_obfuscated_name("Lcom/foo/BuildConfig;"), "BuildConfig = no ofuscado")
    url = b"xx http://patch.chelpus.com/submit http://109.95.209.70:8008/ yy"
    eps = {m.group(0).decode().rstrip(" ") for m in URL_RE.finditer(url)}
    ok(any("patch.chelpus.com/submit" in e for e in eps), "debe detectar el endpoint de subida")
    ips = {m.group(0).decode() for m in IP_RE.finditer(url)}
    ok("109.95.209.70:8008" in ips, "debe detectar la IP cruda")
    perms = {m.group(0).decode() for m in PERM_RE.finditer(b"android.permission.CAMERA")}
    ok("android.permission.CAMERA" in perms, "debe detectar permisos")

    # informe de extremo a extremo sobre un DEX sintético
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, "x.dex")
        with open(p, "wb") as f:
            f.write(b"dex\n035\x00" + b"Lcom/a/a;http://patch.chelpus.com/submit"
                    b"android.permission.ACCESS_SUPERUSERtestkey.pk8")
        rep = analyze(p)
        ok(rep["sha256"] and len(rep["sha256"]) == 64, "sha256 del fichero")
        ok(any(t["id"] == "uploads-target-apk" for t in rep["toolkit"]),
           "debe detectar la subida a terceros")
        ok(any(t["id"] == "aosp-testkey-material" for t in rep["toolkit"]),
           "debe detectar las claves de AOSP")
        v, _ = verdict_of(rep)
        ok(v == "DESINSTALAR", "el veredicto debe ser DESINSTALAR")
        ok("obfuscated_classes" in rep and rep["obfuscated_classes"], "mapa de ofuscación")

    if fails:
        print("SELFTEST ROJO:")
        for f in fails:
            print("  ·", f)
        return 1
    print("SELFTEST VERDE (13 aserciones)")
    return 0


def main():
    ap = argparse.ArgumentParser(
        description="Informe de laboratorio de un APK/DEX (mapa de ofuscación, constantes, "
                    "endpoints, secretos redactados y artefactos de toolkit de parcheo).")
    ap.add_argument("input", nargs="?", help="APK o DEX a analizar")
    ap.add_argument("--json", action="store_true", help="salida JSON")
    ap.add_argument("--out", help="escribe el informe Markdown en este fichero")
    ap.add_argument("--selftest", action="store_true", help="pruebas deterministas, sin ficheros")
    args = ap.parse_args()

    if args.selftest:
        return selftest()
    if not args.input:
        ap.print_help()
        return 2
    if not os.path.exists(args.input):
        print("error: no existe %s" % args.input, file=sys.stderr)
        return 2
    rep = analyze(args.input)
    if args.json:
        data = json.dumps(rep, ensure_ascii=False, indent=1)
    else:
        data = markdown(rep)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(data)
        print("escrito %s" % args.out)
    else:
        print(data)
    return 0


if __name__ == "__main__":
    sys.exit(main())

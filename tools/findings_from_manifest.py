#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Convierte la salida del agente externo (Android RE) en `findings.ndjson`.

Correr en el PC.  NO necesita el binario nuevo de MegaDaemon.

ENTRADA (directorio del agente, p. ej. /data/local/tmp/megadaemon/incoming/<id>/):
    MANIFEST.ndjson   una linea JSON por artefacto:
                      {"id","name","path","origin","type","size_bytes",
                       "sha256","summary","ts"}
    REPORT.md         informe del agente (prosa +, opcionalmente, JSON,
                      tablas o bloques "clave: valor")
    findings.ndjson   SALIDA (se reescribe entera; nunca se anade)

SALIDA (una linea JSON por hallazgo, esquema cerrado v1.2, 19 campos):
    {"ts","package","version","detector","class","method","signal",
     "java_class","java_method","original","decoded","kind","severity",
     "confidence","bypassable","offset","evidence_file","evidence_offset",
     "notes"}   # 19 campos, orden canonico

REGLAS (las mismas que fijo el bus de coordinacion):
  * `detector` = CLASE de deteccion. `signal` = VALOR CRUDO observado.
    `class` = TIPO de evidencia.  `method` = COMO se obtuvo.
  * Enums cerrados; lo que no se sepa => "unknown".  NUNCA se inventa un
    valor: los campos que el informe no declara se rellenan con el valor
    neutro del esquema y se anota en `notes` por que.
  * `bypassable` = "si lo cubrimos" (no "si es facile").  Por defecto False
    (no prometemos cobertura) y anotado.
  * `offset` = indice en el pool de constantes del DEX (-1 si no aplica).
    `evidence_offset` = byte offset DENTRO del artefacto (-1 si no aplica).
  * Si el hallazgo viene del propio dispositivo: `evidence_file` = ruta
    absoluta en el movil y `evidence_offset` = -1.
  * Colision historica entre el esquema de DEX y el de app_intel: DEX usaba
    `signal` como CLASE y app_intel como VALOR.  Se resuelve aqui: si un
    campo llega con un valor que pertenece al enum del otro campo y el
    destino esta vacio, se mueve (y se anota) en vez de duplicarse.

IDEMPOTENCIA: la salida se reescribe desde cero y se ordena con una clave
determinista, de modo que relanzar el conversor sobre el mismo directorio
produce un fichero byte a byte identico.  Los hallazgos duplicados se
colapsan a uno (contando cuantos habia).

USO:
    python3 tools/findings_from_manifest.py --incoming <DIR> [--out FILE]
                                              [--check] [--selftest]

    --check   comprueba idempotencia: convierte y compara con el fichero
              existente; sale 0 si coincide, 1 si no.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys

SCHEMA_FIELDS = [
    "ts", "package", "version", "detector", "class", "method", "signal",
    "java_class", "java_method", "original", "decoded", "kind",
    "severity", "confidence", "bypassable", "offset", "evidence_file",
    "evidence_offset", "notes",
]

DETECTORS = {
    "magisk", "su", "frida", "xposed", "safetynet", "playintegrity",
    "tamper", "emulator", "root_behavior", "obf_string", "crypto_api",
    "net_endpoint",
}
CLASSES = {
    "fs_path", "property", "process", "maps", "ptrace", "keystore",
    "signature", "network", "behavior", "pool_string",
}
METHODS = {
    "stat", "readlink", "grep_props", "proc_read", "package_manager",
    "signature_check", "runtime_probe", "dex_pool_scan", "manifest_parse",
    "static_scan",
}
SEVERITIES = {"info", "low", "medium", "high"}

# Sinónimos aceptados -> valor canonico del enum.
DETECTOR_ALIASES = {
    "magisk": "magisk", "magisk_": "magisk", "magisk-hide": "magisk",
    "su": "su", "superuser": "su", "supersu": "su", "root": "su",
    "rootcheck": "su", "frida": "frida", "xposed": "xposed",
    "lsposed": "xposed", "edxposed": "xposed", "safetynet": "safetynet",
    "safety_net": "safetynet", "attestation": "playintegrity",
    "playintegrity": "playintegrity", "play_integrity": "playintegrity",
    "play integrity": "playintegrity", "tamper": "tamper",
    "tampering": "tamper", "emulator": "emulator", "root_behavior": "root_behavior",
    "root-behavior": "root_behavior", "rootbehavior": "root_behavior",
    "obf_string": "obf_string", "obfuscated_string": "obf_string",
    "obfuscation": "obf_string", "crypto_api": "crypto_api",
    "crypto": "crypto_api", "net_endpoint": "net_endpoint",
    "netendpoint": "net_endpoint", "network_endpoint": "net_endpoint",
}
CLASS_ALIASES = {
    "fs_path": "fs_path", "fspath": "fs_path", "path": "fs_path",
    "file": "fs_path", "fs": "fs_path", "property": "property",
    "prop": "property", "props": "property", "process": "process",
    "proc": "process", "maps": "maps", "proc_maps": "maps",
    "proc_maps": "maps", "ptrace": "ptrace", "keystore": "keystore",
    "signature": "signature", "network": "network", "net": "network",
    "behavior": "behavior", "behaviour": "behavior",
    "pool_string": "pool_string", "string_pool": "pool_string",
    "pool": "pool_string", "strings": "pool_string",
}
METHOD_ALIASES = {
    "stat": "stat", "readlink": "readlink", "grep_props": "grep_props",
    "getprop": "grep_props", "props": "grep_props", "proc_read": "proc_read",
    "cat_proc": "proc_read", "package_manager": "package_manager",
    "pm": "package_manager", "signature_check": "signature_check",
    "runtime_probe": "runtime_probe", "probe": "runtime_probe",
    "dex_pool_scan": "dex_pool_scan", "pool_scan": "dex_pool_scan",
    "manifest_parse": "manifest_parse", "apk_parse": "manifest_parse",
    "static_scan": "static_scan", "scan": "static_scan",
}
SEVERITY_ALIASES = {
    "info": "info", "informational": "info", "none": "info", "ok": "info",
    "low": "low", "minor": "low", "medium": "medium", "moderate": "medium",
    "warn": "medium", "warning": "medium", "high": "high",
    "error": "high", "critical": "high",
}

# Alias de nombre de campo -> campo canonico (para lo que venga del agente).
FIELD_ALIASES = {
    "ts": "ts", "timestamp": "ts", "time": "ts",
    "package": "package", "pkg": "package", "package_name": "package",
    "app": "package",
    "version": "version", "version_name": "version", "ver": "version",
    "detector": "detector", "type": "detector", "category": "detector",
    "check": "detector", "kind_of_check": "detector", "detection": "detector",
    "class": "class", "evidence_class": "class", "cls": "class",
    "method": "method", "how": "method", "technique": "method",
    "signal": "signal", "value": "signal", "raw": "signal",
    "raw_value": "signal", "observed": "signal", "evidence_value": "signal",
    "java_class": "java_class", "target_class": "java_class",
    "class_name": "java_class",
    "java_method": "java_method", "target_method": "java_method",
    "method_name": "java_method",
    "original": "original", "orig": "original", "literal": "original",
    "decoded": "decoded", "plain": "decoded",
    "kind": "kind", "subtype": "kind",
    "severity": "severity", "sev": "severity", "level": "severity",
    "confidence": "confidence", "conf": "confidence",
    "bypassable": "bypassable", "coverable": "bypassable",
    "offset": "offset", "pool_offset": "offset", "string_index": "offset",
    "evidence_file": "evidence_file", "file": "evidence_file",
    "artifact": "evidence_file", "artefacto": "evidence_file",
    "path": "evidence_file", "evidence": "evidence_file",
    "evidence_offset": "evidence_offset", "byte_offset": "evidence_offset",
    "notes": "notes", "note": "notes", "comment": "notes",
}

# Claves que apuntan a un artefacto del manifiesto (para resolver evidencia).
EVIDENCE_REF_KEYS = ("evidence_file", "evidence", "artifact", "artefacto",
                     "file", "path", "sha256", "artifact_id", "id")

UNKNOWN = "unknown"


def _norm_key(k: str) -> str:
    return str(k).strip().lower().replace("-", "_").replace(" ", "_")


def _norm_enum(value, table, canonical_enum, field: str, notes: list):
    """Normaliza a un enum cerrado; si no se puede => 'unknown' + nota."""
    if value is None:
        return UNKNOWN
    raw = str(value).strip()
    if not raw:
        return UNKNOWN
    low = raw.lower().replace("-", "_").replace(" ", "_")
    if low == UNKNOWN:
        # El informe ya dice "no lo se": no es un dato, no hace falta notar.
        return UNKNOWN
    if low in canonical_enum:
        return low
    if low in table:
        canon = table[low]
        if canon != low:
            notes.append("%s:%s->%s" % (field, low, canon))
        return canon
    notes.append("%s:desconocido(%s)" % (field, raw))
    return UNKNOWN


def _severity(value, notes: list) -> str:
    """Severity es un enum cerrado SIN 'unknown': si no se puede determinar
    se usa 'info' (la minima, que noalarmiza) y se anota el motivo."""
    if value is None or str(value).strip() == "":
        notes.append("severity:no_declarada->info")
        return "info"
    low = str(value).strip().lower().replace("-", "_").replace(" ", "_")
    if low in SEVERITIES:
        return low
    if low in SEVERITY_ALIASES:
        canon = SEVERITY_ALIASES[low]
        notes.append("severity:%s->%s" % (low, canon))
        return canon
    notes.append("severity:desconocida(%s)->info" % value)
    return "info"


def _as_bool(value, notes: list) -> bool:
    if isinstance(value, bool):
        return value
    if value is None:
        notes.append("bypassable:no_declarado")
        return False
    low = str(value).strip().lower()
    if low in ("true", "yes", "1", "si", "sí", "y"):
        return True
    if low in ("false", "no", "0", "n"):
        return False
    notes.append("bypassable:desconocido(%s)" % value)
    return False


def _as_int(value, default=-1, notes=None, field=""):
    if value is None or value == "":
        if notes is not None:
            notes.append("%s:no_declarado" % field)
        return default
    try:
        return int(value)
    except (TypeError, ValueError):
        if notes is not None:
            notes.append("%s:no_numerico(%s)" % (field, value))
        return default


def load_manifest(path: str):
    """Carga MANIFEST.ndjson -> (artefactos, indice_evidence)."""
    artifacts, broken = [], 0
    if not os.path.isfile(path):
        return artifacts, {}, broken
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except ValueError:
                broken += 1
                continue
            if isinstance(obj, dict):
                artifacts.append(obj)
            else:
                broken += 1
    index = {}
    for art in artifacts:
        for key in ("id", "name", "path", "sha256"):
            val = art.get(key)
            if val:
                index.setdefault(str(val).strip().lower(), art)
        sha = art.get("sha256")
        if sha:
            index.setdefault(str(sha).strip().lower()[:16], art)
    return artifacts, index, broken


def _scan_json_objects(text: str):
    """Objetos JSON embebidos en texto libre (llaves balanceadas)."""
    out, i, n = [], 0, len(text)
    while i < n:
        if text[i] != "{":
            i += 1
            continue
        depth, j, in_str, esc = 0, i, False, False
        while j < n:
            ch = text[j]
            if in_str:
                if esc:
                    esc = False
                elif ch == "\\":
                    esc = True
                elif ch == '"':
                    in_str = False
            else:
                if ch == '"':
                    in_str = True
                elif ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if depth == 0:
                        break
            j += 1
        if j < n:
            try:
                obj = json.loads(text[i:j + 1])
            except ValueError:
                obj = None
            if isinstance(obj, dict):
                out.append(obj)
        i = j + 1
    return out


def _iter_json_candidates(text: str):
    """Extrae objetos JSON de un texto (bloques ```json, lineas sueltas, arrays)."""
    out = []
    # Objetos JSON embebidos en texto libre (p. ej. tras "Colision: {...}").
    out.extend(_scan_json_objects(text))
    # Bloques cercados.
    for m in re.finditer(r"```(?:json|jsonc)?\s*\n(.*?)```", text, re.S):
        block = m.group(1).strip()
        try:
            parsed = json.loads(block)
        except ValueError:
            for line in block.splitlines():
                line = line.strip().rstrip(",")
                if line.startswith("{"):
                    try:
                        parsed = json.loads(line)
                    except ValueError:
                        continue
                    out.append(parsed)
            continue
        if isinstance(parsed, list):
            out.extend(p for p in parsed if isinstance(p, dict))
        elif isinstance(parsed, dict):
            out.append(parsed)
    # Objetos JSON en lineas sueltas (fuera de bloques).
    for line in text.splitlines():
        s = line.strip()
        if not s.startswith("{") or s.endswith("}"):
            body = s
            if body.startswith("{") and body.endswith("}"):
                try:
                    obj = json.loads(body)
                except ValueError:
                    continue
                if isinstance(obj, dict):
                    out.append(obj)
    # Array JSON completo.
    stripped = text.strip()
    if stripped.startswith("[") and stripped.endswith("]"):
        try:
            parsed = json.loads(stripped)
        except ValueError:
            parsed = None
        if isinstance(parsed, list):
            out.extend(p for p in parsed if isinstance(p, dict))
    return out


KV_RE = re.compile(r"^\s*[-*]?\s*([A-Za-z_][A-Za-z0-9 _-]{0,40})\s*[:=]\s*(.+?)\s*$")


def _iter_kv_candidates(text: str):
    """Bloques 'clave: valor' -> un hallazgo por bloque."""
    out, block = [], {}
    for raw in text.splitlines():
        line = raw.rstrip()
        if not line.strip():
            if block:
                out.append(dict(block))
                block = {}
            continue
        m = KV_RE.match(line)
        if m and _norm_key(m.group(1)) in FIELD_ALIASES:
            block[_norm_key(m.group(1))] = m.group(2).strip()
        else:
            if block:
                out.append(dict(block))
                block = {}
    if block:
        out.append(dict(block))
    return out


def _iter_table_candidates(text: str):
    """Tablas markdown -> una fila por hallazgo."""
    out, header = [], None
    for raw in text.splitlines():
        line = raw.strip()
        if not line.startswith("|"):
            header = None
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if all(re.fullmatch(r":?-{2,}:?", c or "-") for c in cells):
            continue
        if header is None:
            if any(_norm_key(c) in FIELD_ALIASES for c in cells):
                header = [_norm_key(c) for c in cells]
            continue
        if len(cells) != len(header):
            continue
        obj = {}
        for key, val in zip(header, cells):
            if key in FIELD_ALIASES and val:
                obj[key] = val
        if obj:
            out.append(obj)
    return out


def normalize(raw: dict, artifacts_index: dict, notes: list) -> dict:
    """Normaliza un hallazgo al esquema cerrado.  Nunca inventa valores."""
    src = {}
    for key, val in raw.items():
        canon = FIELD_ALIASES.get(_norm_key(key))
        if canon and canon not in src:
            src[canon] = val
        elif canon:
            src.setdefault(canon, val)

    # Colision DEX/app_intel: un valor que pertenece al enum del otro campo.
    detector = src.get("detector")
    klass = src.get("class")
    signal = src.get("signal")
    java_class = src.get("java_class")
    java_method = src.get("java_method")

    def _looks_java_class(v):
        return bool(v) and bool(re.match(r"^[A-Za-z_][\w$]*(\.[A-Za-z_][\w$]*)+$",
                                         str(v).strip()))

    def _looks_java_method(v):
        return bool(v) and ("(" in str(v) or "." in str(v))

    if klass is not None:
        low_k = str(klass).strip().lower().replace("-", "_").replace(" ", "_")
        if low_k in DETECTOR_ALIASES and (detector in (None, "") or
                                         str(detector).strip().lower() == UNKNOWN):
            # Formato viejo de DEX: la clase de deteccion venia en "class".
            notes.append("class->detector:%s" % low_k)
            detector = klass
            klass = None
        elif (low_k not in CLASSES and _looks_java_class(klass)
              and (java_class in (None, ""))):
            # Clase JAVA (no enum de evidencia): va a su campo propio.
            notes.append("class->java_class")
            java_class = klass
            klass = None
    if signal is not None:
        low = str(signal).strip().lower().replace("-", "_").replace(" ", "_")
        if low in CLASS_ALIASES and (klass in (None, "") or
                                     str(klass).strip().lower() == UNKNOWN):
            notes.append("signal->class:%s" % low)
            klass = signal
            signal = None
    method_raw = src.get("method")
    if method_raw is not None:
        low_m = str(method_raw).strip().lower().replace("-", "_").replace(" ", "_")
        if (low_m not in METHODS and _looks_java_method(method_raw)
                and (java_method in (None, ""))):
            notes.append("method->java_method")
            java_method = method_raw
            method_raw = None

    out_notes = list(notes)

    # Evidencia: resolver contra el manifiesto.
    evidence_file = src.get("evidence_file")
    evidence_offset = src.get("evidence_offset")
    resolved = None
    for key in EVIDENCE_REF_KEYS:
        val = src.get(key)
        if not val:
            continue
        key_low = str(val).strip().lower()
        art = artifacts_index.get(key_low)
        if art is None and len(key_low) >= 8:
            art = artifacts_index.get(key_low[:16])
        if art is None:
            continue
        resolved = art
        # El manifiesto manda: la evidencia se ancla a la ruta ABSOLUTA del
        # artefacto en el dispositivo (el informe puede dar un id o un nombre).
        if art.get("path"):
            if str(val).strip() != str(art.get("path")).strip():
                out_notes.append("evidence:resuelto_por_%s" % key)
            evidence_file = str(art.get("path")).strip()
        break
    if evidence_file is None:
        evidence_file = ""
        out_notes.append("evidence_file:no_declarado")
    else:
        evidence_file = str(evidence_file).strip()
    if resolved is not None and resolved.get("sha256"):
        out_notes.append("sha256:%s" % str(resolved.get("sha256"))[:16])

    # ts: epoch.  Sin declarar => 0 (no inventar la hora).
    ts = _as_int(src.get("ts"), 0, out_notes, "ts")

    out = {
        "ts": ts,
        "package": str(src.get("package") or UNKNOWN),
        "version": str(src.get("version") or UNKNOWN),
        "detector": _norm_enum(detector, DETECTOR_ALIASES, DETECTORS,
                               "detector", out_notes),
        "class": _norm_enum(klass, CLASS_ALIASES, CLASSES, "class", out_notes),
        "method": _norm_enum(src.get("method"), METHOD_ALIASES, METHODS,
                             "method", out_notes),
        "signal": str(src.get("signal") or UNKNOWN),
        "java_class": str(java_class or ""),
        "java_method": str(java_method or ""),
        "original": str(src.get("original") or ""),
        "decoded": str(src.get("decoded") or ""),
        "kind": str(src.get("kind") or ""),
        "severity": _severity(src.get("severity"), out_notes),
        "confidence": _as_int(src.get("confidence"), -1, out_notes,
                              "confidence"),
        "bypassable": _as_bool(src.get("bypassable"), out_notes),
        "offset": _as_int(src.get("offset"), -1, out_notes, "offset"),
        "evidence_file": evidence_file,
        "evidence_offset": _as_int(evidence_offset, -1, out_notes,
                                   "evidence_offset"),
        "notes": "; ".join(out_notes),
    }
    return out


def _sort_key(f: dict):
    return (f["package"], f["detector"], f["class"], f["method"],
            f["signal"], f["evidence_file"], f["evidence_offset"],
            f["original"], f["decoded"])


def convert(incoming: str, out_path: str):
    """Convierte el directorio -> findings.ndjson.  Devuelve (n, resumen)."""
    manifest_path = os.path.join(incoming, "MANIFEST.ndjson")
    report_path = os.path.join(incoming, "REPORT.md")
    artifacts, index, broken = load_manifest(manifest_path)

    candidates = []
    if os.path.isfile(report_path):
        with open(report_path, "r", encoding="utf-8", errors="replace") as fh:
            text = fh.read()
        candidates.extend(_iter_json_candidates(text))
        candidates.extend(_iter_kv_candidates(text))
        candidates.extend(_iter_table_candidates(text))
    # El propio manifiesto puede traer hallazgos ya con forma de esquema.
    for art in artifacts:
        if any(k in art for k in ("detector", "signal", "class")):
            candidates.append(art)

    findings, seen = [], {}
    for cand in candidates:
        try:
            f = normalize(cand, index, [])
        except Exception as exc:  # nunca rompe la conversion por un dato raro
            f = normalize({}, index, ["parseo_fallido:%s" % type(exc).__name__])
        key = _sort_key(f)
        if key in seen:
            seen[key]["_dups"] = seen[key].get("_dups", 0) + 1
            seen[key]["notes"] = (seen[key]["notes"].replace("; duplicado", "")
                                  + "; duplicado:%d" % (seen[key]["_dups"] + 1)
                                  ).strip("; ")
            continue
        f["_dups"] = 0
        seen[key] = f
        findings.append(f)

    findings.sort(key=_sort_key)
    dups = sum(f.pop("_dups", 0) for f in findings)
    for f in findings:
        if not f["notes"]:
            f["notes"] = ""

    os.makedirs(os.path.dirname(os.path.abspath(out_path)) or ".",
                exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as fh:
        for f in findings:
            fh.write(json.dumps({k: f[k] for k in SCHEMA_FIELDS},
                                ensure_ascii=False, sort_keys=False) + "\n")

    summary = {
        "out": out_path,
        "findings": len(findings),
        "duplicates_collapsed": dups,
        "candidates": len(candidates),
        "manifest_artifacts": len(artifacts),
        "manifest_broken_lines": broken,
        "report_present": os.path.isfile(report_path),
    }
    return findings, summary


SELFTEST_INPUT = {
    "MANIFEST.ndjson": "\n".join([
        json.dumps({"id": "a1", "name": "app.apk", "type": "apk",
                    "path": "/data/local/tmp/megadaemon/incoming/re/app.apk",
                    "sha256": "a" * 64, "size_bytes": 1234, "ts": 1753000000}),
        json.dumps({"id": "a2", "name": "dump.txt", "type": "report",
                    "path": "/data/local/tmp/megadaemon/incoming/re/dump.txt",
                    "sha256": "b" * 64}),
    ]),
    "REPORT.md": """
# Informe

```json
{"package":"com.bbva.bbvacontigo","version":"5.4.1","detector":"magisk",
 "class":"fs_path","method":"stat","signal":"/sbin/.magisk",
 "severity":"high","confidence":90,"bypassable":true,"evidence":"app.apk",
 "evidence_offset":1234,"offset":-1,"original":"/sbin/.magisk","kind":"fs",
 "java_class":"de.robv.android.xposed.XposedBridge","java_method":"isActive()"}
```

| package | detector | class | signal | severity |
|---|---|---|---|---|
| com.mcdonalds.app | frida | maps | /proc/self/maps | high |

- detector: xposed
  signal: de.robv.android.xposed.installer
  package: com.bank.app
  severity: medium

Colision antigua de DEX: {"class":"frida","signal":"TracerPid=1234",
 "package":"com.x.y","detector":"unknown"}

Clase JAVA en el campo "class": {"class":"com.tencent.mm.opensdk.utils.Util",
 "method":"getSign()","package":"com.tencent.mm","detector":"unknown",
 "signal":"wx6893"}
""",
}


def _selftest():
    import shutil
    import tempfile
    tmp = tempfile.mkdtemp(prefix="findings_selftest_")
    try:
        for name, content in SELFTEST_INPUT.items():
            with open(os.path.join(tmp, name), "w", encoding="utf-8") as fh:
                fh.write(content)
        out = os.path.join(tmp, "findings.ndjson")
        findings, summary = convert(tmp, out)
        first = open(out, "rb").read()
        # Idempotencia: segunda pasada -> mismos bytes.
        convert(tmp, out)
        second = open(out, "rb").read()
        assert first == second, "no es idempotente"
        by_pkg = {f["package"]: f for f in findings}
        assert len(findings) == 5, "hallazgos: %d" % len(findings)
        # v1.2: exactamente 19 campos y en orden canonico.
        for f in findings:
            assert list(f.keys()) == SCHEMA_FIELDS, list(f.keys())
            assert len(f) == 19, len(f)
        bbva = by_pkg["com.bbva.bbvacontigo"]
        assert bbva["detector"] == "magisk"
        assert bbva["class"] == "fs_path"
        assert bbva["signal"] == "/sbin/.magisk"
        assert bbva["severity"] == "high"
        assert bbva["confidence"] == 90
        assert bbva["bypassable"] is True
        assert bbva["evidence_file"].endswith("/app.apk"), bbva["evidence_file"]
        assert bbva["evidence_offset"] == 1234
        assert bbva["java_class"] == "de.robv.android.xposed.XposedBridge"
        assert bbva["java_method"] == "isActive()"
        mcd = by_pkg["com.mcdonalds.app"]
        assert mcd["detector"] == "frida" and mcd["class"] == "maps"
        assert mcd["evidence_offset"] == -1
        # java_class/java_method ausentes => "" (vacio), nunca "unknown".
        assert mcd["java_class"] == "" and mcd["java_method"] == ""
        bank = by_pkg["com.bank.app"]
        assert bank["detector"] == "xposed"
        assert bank["severity"] == "medium"
        # Colision: "class":"frida" debe pasar a detector, no perderse.
        col = by_pkg["com.x.y"]
        assert col["detector"] == "frida", col
        assert col["signal"] == "TracerPid=1234", col
        # Clase JAVA en "class" -> java_class (y "class" queda unknown).
        tencent = by_pkg["com.tencent.mm"]
        assert tencent["java_class"] == "com.tencent.mm.opensdk.utils.Util", tencent
        assert tencent["java_method"] == "getSign()", tencent
        assert tencent["class"] == UNKNOWN, tencent
        # Enums cerrados: ningun valor fuera de enum.
        for f in findings:
            assert f["detector"] in DETECTORS or f["detector"] == UNKNOWN
            assert f["class"] in CLASSES or f["class"] == UNKNOWN
            assert f["method"] in METHODS or f["method"] == UNKNOWN
            assert f["severity"] in SEVERITIES
        print("SELFTEST OK: %d hallazgos, idempotente" % len(findings))
        for line in first.decode("utf-8").splitlines():
            print("  " + line)
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--incoming", help="directorio del agente externo")
    ap.add_argument("--out", help="fichero de salida (por defecto "
                                  "<incoming>/findings.ndjson)")
    ap.add_argument("--check", action="store_true",
                    help="comprueba que la salida actual es idempotente")
    ap.add_argument("--selftest", action="store_true",
                    help="prueba interna con fixtures")
    args = ap.parse_args(argv)

    if args.selftest:
        return _selftest()
    if not args.incoming:
        ap.error("falta --incoming (o usa --selftest)")
    if not os.path.isdir(args.incoming):
        print("no existe el directorio: %s" % args.incoming, file=sys.stderr)
        return 2

    out_path = args.out or os.path.join(args.incoming, "findings.ndjson")
    findings, summary = convert(args.incoming, out_path)

    if args.check:
        import hashlib
        with open(out_path, "rb") as fh:
            digest = hashlib.sha256(fh.read()).hexdigest()[:16]
        before = summary["findings"]
        convert(args.incoming, out_path)
        with open(out_path, "rb") as fh:
            digest2 = hashlib.sha256(fh.read()).hexdigest()[:16]
        ok = digest == digest2
        print(json.dumps({"check": "ok" if ok else "difiere",
                          "sha256_16": digest, "findings": before},
                         ensure_ascii=False))
        return 0 if ok else 1

    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())

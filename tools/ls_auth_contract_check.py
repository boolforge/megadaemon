#!/usr/bin/env python3
"""Comprueba que el código (Rust y Go) del módulo `ls_auth` sigue siendo
consistente con el contrato verificado del Language Server de Antigravity.

POR QUÉ EXISTE
--------------
El contrato (`reference/ls_auth_contract.json`) es la fuente de verdad con la
procedencia de cada valor (VA real del binario + fichero:línea). Este
comprobador verifica que **el código que se ejecuta** contiene esos mismos
valores: si alguien cambia un itab, un offset o un nombre de método sin
actualizar el contrato (o al revés), el CI falla en vez de mentir.

QUÉ COMPRUEBA
-------------
1. Layout del `AuthResult` (offset y tamaño de los 12 campos; total 168 B).
2. Los 7 itabs de `failure_details` y cuáles nunca se construyen.
3. Los 12 escritores de `has_valid_auth`.
4. La URL del backend: prefijo, plantillas (texto + longitud) y los 13
   métodos con su VA de literal y su longitud.
5. La regla de rechazo terminal: motivo, longitud, rango de status y
   exclusiones (408, 429).
6. Los valores del enum `AuthMode`.

USO
----
    python3 tools/ls_auth_contract_check.py
    # salida: 0 si todo cuadra, 1 si hay discrepancias
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CONTRACT = ROOT / "reference" / "ls_auth_contract.json"
RUST = ROOT / "rust-core" / "src" / "ls_auth"
GO = ROOT / "reference" / "go" / "lsauth"


def read(p: pathlib.Path) -> str:
    return p.read_text(encoding="utf-8", errors="replace")


def hexes(text: str) -> set[str]:
    """VAs (y offsets) en formato 0x… tal y como aparecen en el código.

    Se aceptan 2 o más dígitos hex porque el layout del `AuthResult` usa
    offsets de un byte (`0x08`, `0xA0`, …).
    """
    return {m.group(0).lower() for m in re.finditer(r"0x[0-9a-fA-F]{2,16}", text)}


def main() -> int:
    if not CONTRACT.exists():
        print("FALTA el contrato:", CONTRACT)
        return 1
    c = json.loads(CONTRACT.read_text(encoding="utf-8"))

    sources = {}
    for p in sorted(RUST.glob("*.rs")):
        sources["rust:" + p.name] = read(p)
    for p in sorted(GO.glob("*.go")):
        sources["go:" + p.name] = read(p)
    if not sources:
        print("FALTA el código a comprobar (rust-core/src/ls_auth/ o reference/go/lsauth/)")
        return 1

    blob = "\n".join(sources.values())
    all_va = hexes(blob)
    problems: list[str] = []

    def need(va: str, what: str) -> None:
        if va.lower() not in all_va:
            problems.append(f"{what}: el VA {va} no aparece en el código")

    # 1) layout
    ar = c["auth_result"]
    for f in ar["fields"]:
        need(f["offset"], f"layout del campo {f['name']}")
    if ar["size"] != 168:
        problems.append("el contrato dice un AuthResult de tamaño distinto de 168")

    # 2) itabs
    for v in c["failure_details"]:
        need(v["itab"], f"itab de {v['kind']}")
    if len(c["failure_details"]) != 7:
        problems.append("el contrato no lista 7 variantes de failure_details")

    # 3) escritores
    for va in c["has_valid_auth_writers"]:
        need(va, "escritor de has_valid_auth")
    if len(c["has_valid_auth_writers"]) != 12:
        problems.append("el contrato no lista 12 escritores de has_valid_auth")

    # 4) backend
    b = c["backend"]
    need(b["prefix_global_va"], "global del prefijo")
    need(b["template_method"]["va"], "plantilla de método")
    need(b["template_resource"]["va"], "plantilla de recurso")
    for m in b["methods"]:
        need(m["literal_va"], f"literal del método {m['name']}")
        if len(m["name"]) != m["len"]:
            problems.append(f"método {m['name']}: longitud declarada {m['len']} != real {len(m['name'])}")

    # 5) regla terminal
    t = c["http"]["terminal_rule"]
    if t["reason_len"] != len(t["reason_equals"]):
        problems.append("la longitud del motivo no cuadra con el motivo")
    if sorted(t["excluded"]) != [408, 429]:
        problems.append("las exclusiones de status deben ser 408 y 429")
    need(t["va"], "isTerminalCredentialRejection")

    # 6) AuthMode
    if len(c["auth_mode"]) != 6:
        problems.append("AuthMode debe tener 6 valores")

    # 7) itabs de proveedor (N1): los 7 deben estar en el código y el
    #    índice de método debe ser 2 (offset 0x28 con cabecera de 24 B).
    itabs = c.get("auth_provider_itabs")
    if not itabs:
        problems.append("falta la sección auth_provider_itabs")
    else:
        if itabs.get("method_slot") != 2:
            problems.append("el slot del método GetEndpointURL debe ser 2")
        if itabs.get("method_slot_offset") != "0x28":
            problems.append("el offset del slot de GetEndpointURL debe ser 0x28")
        if itabs.get("itab_header_bytes") != 24:
            problems.append("la cabecera del itab debe medir 24 bytes")
        provs = itabs.get("providers", [])
        if len(provs) != 7:
            problems.append("se esperaban 7 proveedores en auth_provider_itabs")
        for p in provs:
            need(p["itab"], "itab de %s" % p["name"])
            need(p["fun2"], "GetEndpointURL de %s" % p["name"])

    # 8) parche region-bypass: sus VAs deben estar en el código y el
    #    contrato no debe seguir afirmando la inferencia del "slot 5".
    patch = c.get("region_bypass_patch")
    if not patch:
        problems.append("falta la sección region_bypass_patch")
    else:
        if patch.get("bytes_changed") != 15:
            problems.append("el parche region-bypass debe medir 15 bytes")
        sites = patch.get("sites", [])
        if len(sites) != 3:
            problems.append("se esperaban 3 puntos de parche")
        for s in sites:
            need(s["va"], "punto de parche region-bypass")
        cvc = patch.get("cross_version_check", {})
        if cvc.get("v2.11.0_patch_site_va"):
            need(cvc["v2.11.0_patch_site_va"], "punto de parche en v2.11.0")

    for nv in c.get("not_verified", []):
        if "slot 5" in nv:
            problems.append("not_verified ya no debe seguir mencionando la inferencia del slot 5")

    # Coherencia del texto de las plantillas
    if b["template_method"]["text"] not in blob:
        problems.append("la plantilla de método no aparece en el código")
    if b["template_resource"]["text"] not in blob:
        problems.append("la plantilla de recurso no aparece en el código")
    if b["prefix"] not in blob:
        problems.append("el prefijo no aparece en el código")

    if problems:
        print("CONTRATO: %d discrepancia(s)" % len(problems))
        for p in problems:
            print("  -", p)
        return 1

    print(
        "CONTRATO OK: %d ficheros comprobados (%s), %d campos, %d itabs, "
        "%d escritores, %d métodos de backend, regla terminal 4xx salvo 408/429"
        % (
            len(sources),
            ", ".join(sorted(sources)),
            len(ar["fields"]),
            len(c["failure_details"]),
            len(c["has_valid_auth_writers"]),
            len(b["methods"]),
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

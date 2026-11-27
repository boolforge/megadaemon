#!/usr/bin/env python3
"""Demangler de nombres Rust (v0 legacy y v0 moderno) en Python puro.

POR QUE (medido 2026-11-27): el sandbox no tiene `rustfilt`, ni
`rustc-demangle`, ni `cargo` (prohibido por el root: el enlace se come la RAM).
El asset de simbolos de Codex (`codex-app-server.debug`, 1.085.115.680 B) trae
la `symtab` completa con nombres mangled de Rust, asi que hace falta
des-manglar para que los nombres sean legibles.

FORMAS MEDIDAS en ese fichero:
  v0 legacy: `_ZN4core3ptr13drop_in_place17h0123456789abcdefE`  -> ruta + hash
  v0 moderno: `_RNvNtCs1234_4core3ptr`                          -> prefijo `_R`

MODULO: `tools/rust-demangle.py` (usado por `extraer-simbolos-codex.py`).
"""
from __future__ import annotations

import re

# v0 legacy: [_ZN] <ruta::...> [::h<16hex>] E
LEGACY = re.compile(r"^_ZN(?P<cuerpo>.+)E$")
HASH_SUFFIX = re.compile(r"^h[0-9a-f]{16}$")
# v0 moderno (RFC 2603): _R<visibilidad><ruta><genericos>E
MODERNO = re.compile(r"^_R(?P<cuerpo>[A-Za-z_][A-Za-z0-9_]*)(?P<rest>.*)E$", re.S)


def _partes(ruta: str) -> list[str]:
    """Parte la ruta de un mangled v0 legacy.

    MEDIDO en `codex-app-server.debug`: los elementos del path NO van separados
    por `::`; cada uno lleva su longitud en decimal delante (`4core3ptr13drop
    _in_place17h0123456789abcdef` = core / ptr / drop_in_place / h<hash>).
    """
    out, i = [], 0
    while i < len(ruta):
        j = i
        while j < len(ruta) and ruta[j].isdigit():
            j += 1
        if j == i:  # no hay longitud => no es legacy: devolver tal cual
            return [ruta]
        n = int(ruta[i:j])
        elem = ruta[j : j + n]
        if len(elem) != n:
            return [ruta]
        out.append(elem)
        i = j + n
    return out


def demangle_v0_legacy(cuerpo: str) -> str:
    """`_ZN...E` -> `ruta::a::b` (con el hash eliminado)."""
    partes = _partes(cuerpo)
    if len(partes) == 1 and partes[0] == cuerpo:
        return cuerpo, ""
    hash_ = ""
    if partes and HASH_SUFFIX.match(partes[-1]):
        hash_ = partes[-1][3:]
        partes = partes[:-1]
    limpio = [p for p in partes if p]
    return "::".join(limpio), hash_


def _moderno(rest: str) -> str:
    """Parser pragmatico del v0 moderno (RFC 2603).

    MEDIDO en `codex-app-server.debug`: los nombres son v0 moderno (`_RNv...`).
    La gramatica completa es una maquina de estados; aqui se aplica la regla
    que SI reparte los nombres reales: todo identificador Rust es
    `<longitud-decimal><nombre>` con la longitud en decimal (puede llevar ceros
    a la izquierda: `0023___RUST_STD_INTERNAL_VAL` = 23 chars), y el
    desambiguador va pegado detras. Los codigos de tipo (C, N, v, t, s, r, h...)
    van ANTES del numero, nunca despues, asi que no colisionan.

    NO es el demangler canonico de rustc-demangle: es una APROXIMACION MEDIDA,
    suficiente para catalogar (crate + ruta) y para leer el codigo.
    """
    IDENT = re.compile(r"(\d+)([A-Za-z_][A-Za-z0-9_]*)")
    ids: list[str] = []
    for m in IDENT.finditer(rest):
        n = int(m.group(1))
        nombre = m.group(2)
        if n != len(nombre):
            continue
        if len(nombre) == 17 and nombre[0] == "h" and re.fullmatch(r"[0-9a-f]{16}", nombre[1:]):
            continue  # hash del legacy, no un identificador
        ids.append(nombre)
    return "::".join(ids)


def demangle(nombre: str) -> tuple[str, str]:
    """Devuelve (nombre_legible, hash_o_vacio). Si no es Rust, devuelve el original."""
    if nombre.startswith("_R") and len(nombre) > 2:
        m = MODERNO.match(nombre)
        if m:
            return _moderno(m.group("rest")) or nombre, ""
        return nombre, ""
    m = LEGACY.match(nombre)
    if m:
        return demangle_v0_legacy(m.group("cuerpo"))
    return nombre, ""


def crate_de(nombre_legible: str) -> str:
    """Primer segmento de la ruta (el crate), para agrupar por modulo."""
    p = nombre_legible.split("::")
    return p[0] if p else "?"


if __name__ == "__main__":
    import sys

    for linea in sys.stdin:
        s = linea.strip()
        if not s:
            continue
        leg, h = demangle(s)
        print(f"{s}\t{leg}\t{h}")

#!/usr/bin/env bash
# guardia-compilacion.sh — GUARDIÁN DE COMPILACIÓN de MegaDaemon.
#
# POR QUÉ EXISTE (medido el 2026-11-23, 3 OOM en el sandbox):
#   El sandbox de trabajo tiene ~3 GB de RAM.  El ENLAZADO del crate (o de
#   los tests) se come >1,5 GB.  Con varios actores compilando a la vez, sus
#   `rustc` (2,8 GB cada uno medido) provocan OOM: matan a los workers de
#   Ghidra (que estaban decompilando el 100 % del binario Go de Antigravity:
#   61.896 funciones) y al propio build.  MEDIDO: 3 eventos de OOM en
#   `dmesg`, todos matando un `rustc`.
#
#   Por eso en MegaDaemon **compila un solo actor**: el root (la sesión
#   principal).  Los agentes entregan CÓDIGO (rama + commit + nota) y
#   comprueban la sintaxis con `tools/check-sintaxis.sh`, que no usa RAM.
#
# QUÉ HACE ESTE GUARDIÁN (salida 0 = se puede compular, 1 = no):
#   1. Lock: si otro build ya está corriendo, NO se compila (se espera).
#   2. RAM: si la memoria disponible baja del umbral, NO se compila.
#   3. Actores: si hay otro `cargo`/`rustc` vivo, NO se compila.
#
# USO:
#   tools/guardia-compilacion.sh            # veredicto (0 se puede, 1 no)
#   tools/guardia-compilacion.sh --esperar  # esperar hasta que se pueda
set -uo pipefail

# Umbral de RAM libre para empezar a compilar (MB).  Con 3 GB de total, 1.400
# MB deja margen para el link con el perfil LIGERO (medido: `cargo test --lib`
# con debug 0 y -j 1 entra en ~1,4 GB) sin que el OOM killer elija a
MIN_LIBRE_MB="${MD_BUILD_MIN_LIBRE_MB:-1400}"
LOCK="${MD_BUILD_LOCK:-/workspace/.build-lock}"

mem_libre_mb() {
    # /proc/meminfo: MemAvailable es la mejor estimación (no cuenta la cache
    # reclamable).  Si no está, cae a MemFree.
    local kb
    kb=$(awk '/^MemAvailable:/ {print $2; exit}' /proc/meminfo 2>/dev/null)
    [ -n "$kb" ] || kb=$(awk '/^MemFree:/ {print $2; exit}' /proc/meminfo 2>/dev/null)
    echo $(( kb / 1024 ))
}

otros_actores() {
    # Cuenta rustc/cargo vivos EXCLUYENDO este script y sus hijos directos.
    # Los ZOMBIS (estado Z) se ignoran: no tienen RAM ni CPU (ya MURieron,
    # normalmente por el OOM) y contarlos bloquearia la compilacion para
    # siempre.  MEDIDO: 15 procesos `cargo` defunct tras los OOM.
    ps -A -o stat,args 2>/dev/null \
        | grep -E "[r]ustc|[c]argo (build|check|test|clippy|bench)" \
        | grep -v "guardia-compilacion" \
        | awk '$1 !~ /^Z/ {print}' | wc -l
}

veredicto() {
    local libre otros
    libre=$(mem_libre_mb)
    otros=$(otros_actores)
    if [ -d "$LOCK" ]; then
        echo "NO se puede compilar: hay un build en curso (lock $LOCK)."
        echo "   Espera a que termine (o usa tools/build.sh, que ya espera el lock)."
        return 1
    fi
    if [ "${otros:-0}" -gt 0 ]; then
        echo "NO se puede compilar: hay $otros proceso(s) cargo/rustc vivos."
        echo "   En MegaDaemon compila UN SOLO actor: el root."
        return 1
    fi
    if [ "${libre:-0}" -lt "$MIN_LIBRE_MB" ]; then
        echo "NO se puede compilar: memoria libre ${libre} MB < ${MIN_LIBRE_MB} MB."
        echo "   Con ~3 GB de RAM, un link en paralelo tumba el sandbox (3 OOM medidos)."
        echo "   Libera memoria o espera; para comprobar sintaxis sin RAM:"
        echo "     tools/check-sintaxis.sh"
        return 1
    fi
    echo "OK: se puede compilar (libre ${libre} MB, $otros actores, sin lock)."
    return 0
}

if [ "${1:-}" = "--esperar" ]; then
    t=0
    while ! veredicto >/dev/null 2>&1; do
        t=$((t+1))
        if [ "$t" -gt 300 ]; then
            veredicto
            exit 1
        fi
        sleep 5
    done
    veredicto
    exit 0
fi

veredicto

#!/bin/sh
# Compila el binario `md-backup` para ARM64 (Android) con el NDK r26c.
#
# USA EL CRATE DELGADO (`rust-core/md-backup-crate`), que compila **los mismos
# ficheros fuente** del motor de backup pero sin los 200+ módulos del daemon.
#
# POR QUÉ (medido el 2026-11-22 en este sandbox de 3 GB): compilar el crate
# entero para ARM64 pedía ~2,2 GB al rustc ⇒ el OOM killer lo mataba (6
# SIGKILL). Con el crate delgado el pico es ~400 MB.
#
# Los knobs del perfil son los del CI (`.github/workflows/build.yml`).
set -eu

NDK=/workspace/ndk/android-ndk-r26c
BIN="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"

export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$BIN/aarch64-linux-android26-clang"
export CC_aarch64_linux_android="$BIN/aarch64-linux-android26-clang"
export AR_aarch64_linux_android="$BIN/llvm-ar"
export RANLIB_aarch64_linux_android="$BIN/llvm-ranlib"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/workspace/md-target-bk2}"

cd "$(dirname "$0")/../rust-core/md-backup-crate"
exec cargo build --release --target aarch64-linux-android --bin md-backup "$@"

#!/usr/bin/env bash
# Build del binario ARM64 (aarch64-linux-android) para el movil Samsung.
#
#   NDK=/opt/ndk/android-ndk-r27c API=24 ./deploy/build-arm64.sh
#
# Notas:
#  - El target de rust `aarch64-linux-android` ya viene instalado; lo unico
#    que faltaba era el NDK (clang + sysroot).
#  - El `CARGO_TARGET_DIR` va a disco LOCAL (no NFS): en NFS los builds
#    fallan con "failed to copy bitcode" / "cannot open .rcgu.o".
#  - Los overrides de perfil evitan que el rustc muera por OOM: con
#    `lto = true` + `codegen-units = 1` el rustc acaba en SIGKILL.
set -euo pipefail

NDK="${NDK:-/opt/ndk/android-ndk-r27c}"
API="${API:-24}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TC="$NDK/toolchains/llvm/prebuilt/linux-x86_64"

if [ ! -x "$TC/bin/aarch64-linux-android${API}-clang" ]; then
  echo "ERROR: no encuentro el clang del NDK en $TC (API=$API)" >&2
  echo "       ajusta NDK= o API= ; NDKs en /opt/ndk:" >&2
  ls -d /opt/ndk/android-ndk-* 2>/dev/null >&2 || true
  exit 1
fi

export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TC/bin/aarch64-linux-android${API}-clang"
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export CXX_aarch64_linux_android="$TC/bin/aarch64-linux-android${API}-clang++"
export AR_aarch64_linux_android="$TC/bin/llvm-ar"
export RANLIB_aarch64_linux_android="$TC/bin/llvm-ranlib"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/run/agent-runtime/md/target-arm64}"
export CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
export CARGO_PROFILE_RELEASE_LTO=false

cd "$ROOT/rust-core"
cargo build --release --target aarch64-linux-android -j "${JOBS:-1}"

mkdir -p "$ROOT/dist"
BIN="$CARGO_TARGET_DIR/aarch64-linux-android/release/megadaemon-core"
cp "$BIN" "$ROOT/dist/megadaemon-core-arm64"
sha256sum "$ROOT/dist/megadaemon-core-arm64" | tee "$ROOT/dist/megadaemon-core-arm64.sha256"
file "$ROOT/dist/megadaemon-core-arm64" 2>/dev/null || true

#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/out"
GO_BIN="${GO_BIN:-go}"
mkdir -p "$OUT"
GO_VERSION="$("$GO_BIN" version | awk '{print $3}')"
cd "$ROOT/fixtures"
for target in linux/amd64 linux/arm64 windows/amd64 darwin/arm64; do
  os=${target%/*}; arch=${target#*/}
  name="fixture-${GO_VERSION}-${os}-${arch}"
  echo "[fixture] $GO_VERSION -> $name"
  GOOS="$os" GOARCH="$arch" CGO_ENABLED=0 "$GO_BIN" build -trimpath -o "$OUT/$name" .
done
if [[ "${BUILD_PLUGIN:-0}" == "1" ]]; then
  GOOS=linux GOARCH=amd64 "$GO_BIN" build -buildmode=plugin -o "$OUT/fixture-plugin-${GO_VERSION}.so" ./plugin
  GOOS=linux GOARCH=amd64 "$GO_BIN" build -o "$OUT/fixture-pluginhost-${GO_VERSION}" ./pluginhost
  "$OUT/fixture-pluginhost-${GO_VERSION}" "$OUT/fixture-plugin-${GO_VERSION}.so"
fi

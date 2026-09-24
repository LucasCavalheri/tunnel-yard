#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TMP_DIR"' EXIT

required=(
  tunnel-yard-linux-x64
  tunnel-yard-linux-arm64
  tunnel-yard-linux-x64.tar.gz
  tunnel-yard-linux-arm64.tar.gz
  tunnel-yard_3.0.8_amd64.deb
  tunnel-yard_3.0.8_arm64.deb
  tunnel-yard-3.0.8-1.x86_64.rpm
  tunnel-yard-3.0.8-1.aarch64.rpm
  tunnel-yard-3.0.8-1-x86_64.pkg.tar.zst
  tunnel-yard-3.0.8-r0-x86_64.apk
)
for artifact in "${required[@]}"; do
  printf '%s\n' "$artifact" > "$TMP_DIR/$artifact"
done

bash "$ROOT_DIR/scripts/validate-release-artifacts.sh" "$TMP_DIR" 3.0.8
bash "$ROOT_DIR/scripts/generate-sha256sums.sh" "$TMP_DIR"
(
  cd "$TMP_DIR"
  sha256sum --check SHA256SUMS >/dev/null
)

rm "$TMP_DIR/tunnel-yard-linux-arm64"
if bash "$ROOT_DIR/scripts/validate-release-artifacts.sh" "$TMP_DIR" 3.0.8 >/dev/null 2>&1; then
  echo "expected missing ARM64 artifact to fail validation" >&2
  exit 1
fi

echo "release-artifacts tests passed"

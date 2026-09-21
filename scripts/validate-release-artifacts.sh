#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <dist-dir> <version>" >&2
  exit 2
fi

DIST_DIR="$1"
VERSION="$2"

if [[ ! -d "$DIST_DIR" ]]; then
  echo "release artifact directory does not exist: $DIST_DIR" >&2
  exit 1
fi

required=(
  "tunnel-yard-linux-x64"
  "tunnel-yard-linux-arm64"
  "tunnel-yard-linux-x64.tar.gz"
  "tunnel-yard-linux-arm64.tar.gz"
  "tunnel-yard_${VERSION}_amd64.deb"
  "tunnel-yard_${VERSION}_arm64.deb"
  "tunnel-yard-${VERSION}-1.x86_64.rpm"
  "tunnel-yard-${VERSION}-1.aarch64.rpm"
)
for artifact in "${required[@]}"; do
  if [[ ! -s "$DIST_DIR/$artifact" ]]; then
    echo "missing or empty release artifact: $artifact" >&2
    exit 1
  fi
done

shopt -s nullglob
deb=("$DIST_DIR"/*.deb)
rpm=("$DIST_DIR"/*.rpm)
if (( ${#deb[@]} != 2 || ${#rpm[@]} != 2 )); then
  echo "expected two .deb and two .rpm artifacts" >&2
  exit 1
fi

printf 'validated release artifacts in %s\n' "$DIST_DIR"

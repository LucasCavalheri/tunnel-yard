#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$ROOT_DIR/scripts/validate-release.sh"

expect_failure() {
  if bash "$SCRIPT" "$@" >/dev/null 2>&1; then
    echo "expected release validation to fail for: $*" >&2
    exit 1
  fi
}

bash "$SCRIPT" "$ROOT_DIR" v3.0.2
bash "$SCRIPT" "$ROOT_DIR" v3.0.2-rc.1
expect_failure "$ROOT_DIR" 3.0.2
expect_failure "$ROOT_DIR" v3.0.3

TMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TMP_DIR"' EXIT
cp "$ROOT_DIR/Cargo.toml" "$TMP_DIR/Cargo.toml"
cp "$ROOT_DIR/CHANGELOG.md" "$TMP_DIR/CHANGELOG.md"
sed -i 's/^version = "3\.0\.2"$/version = "3.0.3"/' "$TMP_DIR/Cargo.toml"
expect_failure "$TMP_DIR" v3.0.3

echo "validate-release tests passed"

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${1:-.}"
if [[ $# -ge 2 ]]; then
  TAG="$2"
else
  TAG="${GITHUB_REF_NAME:-}"
fi

if [[ -z "$TAG" ]]; then
  echo "release tag is required" >&2
  exit 2
fi

if [[ ! "$TAG" =~ ^v([0-9]+\.[0-9]+\.[0-9]+)([-+][0-9A-Za-z.-]+)?$ ]]; then
  echo "release tag must be vX.Y.Z, optionally followed by prerelease/build metadata: $TAG" >&2
  exit 1
fi

VERSION="${BASH_REMATCH[1]}"
CARGO_TOML="$ROOT_DIR/Cargo.toml"
CHANGELOG="$ROOT_DIR/CHANGELOG.md"

if [[ ! -f "$CARGO_TOML" ]]; then
  echo "missing Cargo.toml: $CARGO_TOML" >&2
  exit 1
fi
if [[ ! -f "$CHANGELOG" ]]; then
  echo "missing CHANGELOG.md: $CHANGELOG" >&2
  exit 1
fi

CARGO_VERSION="$(awk -F'"' '
  /^[[:space:]]*version[[:space:]]*=/ { print $2; exit }
' "$CARGO_TOML")"

if [[ -z "$CARGO_VERSION" ]]; then
  echo "could not read package version from $CARGO_TOML" >&2
  exit 1
fi
if [[ "$VERSION" != "$CARGO_VERSION" ]]; then
  echo "tag $TAG declares $VERSION but Cargo.toml declares $CARGO_VERSION" >&2
  exit 1
fi

if ! awk -v version="$VERSION" '
  /^## / {
    heading = $0
    sub(/^##[[:space:]]+/, "", heading)
    sub(/^v/, "", heading)
    split(heading, fields, /[[:space:]]+/)
    if (fields[1] == version) {
      found = 1
      next
    }
    if (found) {
      exit 0
    }
  }
  END { exit(found ? 0 : 1) }
' "$CHANGELOG"; then
  echo "CHANGELOG.md has no section for $VERSION" >&2
  exit 1
fi

printf 'validated release %s against Cargo.toml and CHANGELOG.md\n' "$TAG"

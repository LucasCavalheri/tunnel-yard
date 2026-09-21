#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <dist-dir>" >&2
  exit 2
fi

DIST_DIR="$1"
if [[ ! -d "$DIST_DIR" ]]; then
  echo "release artifact directory does not exist: $DIST_DIR" >&2
  exit 1
fi

(
  cd "$DIST_DIR"
  mapfile -t files < <(
    find . -maxdepth 1 -type f ! -name SHA256SUMS -printf '%f\n' | LC_ALL=C sort
  )
  if ((${#files[@]} == 0)); then
    echo "no release artifacts found in $DIST_DIR" >&2
    exit 1
  fi
  for file in "${files[@]}"; do
    sha256sum "$file"
  done
) > "$DIST_DIR/SHA256SUMS"

test -s "$DIST_DIR/SHA256SUMS"
printf 'generated %s\n' "$DIST_DIR/SHA256SUMS"

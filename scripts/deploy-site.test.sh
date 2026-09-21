#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$ROOT_DIR/scripts/deploy-site.sh"
FIXTURE="$ROOT_DIR/scripts/test-fixtures/fake-curl.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TMP_DIR"' EXIT

expect_failure() {
  if "$@" >/dev/null 2>&1; then
    echo "expected deploy trigger to fail: $*" >&2
    exit 1
  fi
}

expect_failure bash "$SCRIPT"
expect_failure bash "$SCRIPT" http://api.vercel.com/v1/integrations/deploy/test
expect_failure env CURL_BIN=/bin/false bash "$SCRIPT" https://api.vercel.com/v1/integrations/deploy/test

ARGS_FILE="$TMP_DIR/curl-args"
output="$(
  CURL_BIN="$FIXTURE" \
  FAKE_CURL_ARGS_FILE="$ARGS_FILE" \
  bash "$SCRIPT" https://api.vercel.com/v1/integrations/deploy/test
)"

[[ "$output" == *"HTTP 202"* ]]
for expected in --request POST https://api.vercel.com/v1/integrations/deploy/test; do
  rg --fixed-strings --line-regexp -- "$expected" "$ARGS_FILE" >/dev/null
done

echo "deploy-site tests passed"

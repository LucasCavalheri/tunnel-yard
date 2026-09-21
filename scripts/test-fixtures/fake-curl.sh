#!/usr/bin/env bash
set -euo pipefail

: "${FAKE_CURL_ARGS_FILE:?FAKE_CURL_ARGS_FILE is required}"
printf '%s\n' "$@" > "$FAKE_CURL_ARGS_FILE"
printf '202'

#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <vercel-deploy-hook-url>" >&2
  exit 2
fi

HOOK_URL="$1"
if [[ ! "$HOOK_URL" =~ ^https://api\.vercel\.com/v1/integrations/deploy/[^[:space:]]+$ ]]; then
  echo "Vercel deploy hook URL must use the api.vercel.com integrations endpoint" >&2
  exit 1
fi

CURL_BIN="${CURL_BIN:-curl}"
http_code="$($CURL_BIN \
  --fail \
  --silent \
  --show-error \
  --retry 3 \
  --retry-delay 2 \
  --output /dev/null \
  --write-out '%{http_code}' \
  --request POST \
  "$HOOK_URL")"

if [[ ! "$http_code" =~ ^2 ]]; then
  echo "Vercel deploy hook returned unexpected HTTP status: $http_code" >&2
  exit 1
fi

printf 'Vercel deploy hook accepted the request (HTTP %s).\n' "$http_code"

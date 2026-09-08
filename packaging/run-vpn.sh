#!/usr/bin/env bash
set -euo pipefail

CONF="${1:?caminho do .conf obrigatório}"

if [[ ! -f "$CONF" ]]; then
  echo "Arquivo de configuração não encontrado: $CONF" >&2
  exit 1
fi

NAME="$(basename "$CONF" .conf)"
PIDFILE="/run/tunnel-yard-${NAME}.pid"
mkdir -p /run

cleanup() {
  rm -f "$PIDFILE"
}

terminate() {
  if [[ -n "${CHILD_PID:-}" ]] && kill -0 "$CHILD_PID" 2>/dev/null; then
    kill -INT "$CHILD_PID" 2>/dev/null || true
    wait "$CHILD_PID" 2>/dev/null || true
  fi
  cleanup
  exit 0
}

trap terminate INT TERM

openfortivpn -c "$CONF" &
CHILD_PID=$!
echo "$CHILD_PID" > "$PIDFILE"

wait "$CHILD_PID"
STATUS=$?
cleanup
exit "$STATUS"

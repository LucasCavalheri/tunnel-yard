#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "$TMP_DIR"' EXIT

export TUNNEL_YARD_SOURCE_ONLY=1
export TUNNEL_YARD_RUN_DIR="$TMP_DIR/run"
mkdir -p "$TUNNEL_YARD_RUN_DIR"
source "$ROOT_DIR/public/install.sh"
run_root() {
  "$@"
}

ln -s "$(command -v bash)" "$TMP_DIR/openfortivpn"
"$TMP_DIR/openfortivpn" -c 'trap "exit 0" INT TERM; while :; do read -r -t 1 _ || :; done' &
tunnel_pid=$!
printf '%s\n' "$tunnel_pid" > "$TUNNEL_YARD_RUN_DIR/tunnel-yard-lab.pid"
[[ "$(active_tunnel_count)" == 1 ]]
stop_running_tunnels
wait "$tunnel_pid" 2>/dev/null || true
! pid_is_alive "$tunnel_pid"

"$TMP_DIR/openfortivpn" -c 'trap "exit 0" INT TERM; while :; do read -r -t 1 _ || :; done' &
active_tunnel_pid=$!
printf '%s\n' "$active_tunnel_pid" > "$TUNNEL_YARD_RUN_DIR/tunnel-yard-active.pid"
[[ "$(active_tunnel_count)" == 1 ]]

sleep 30 &
app_pid=$!
TUNNEL_YARD_RUNNING_PIDS="$app_pid"
TUNNEL_YARD_AUTO_CONFIRM=1
prepare_running_app
wait "$active_tunnel_pid" 2>/dev/null || true
wait "$app_pid" 2>/dev/null || true
[[ "$APP_WAS_RUNNING" == 1 ]]
! pid_is_alive "$active_tunnel_pid"
! pid_is_alive "$app_pid"

TUNNEL_YARD_LAUNCHER=/bin/true
relaunch_running_app
[[ "$APP_RESTARTED" == 1 ]]

echo "install flow tests passed"

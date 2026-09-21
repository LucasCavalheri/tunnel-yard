#!/usr/bin/env bash
# TunnelYard Linux installer. Detects the chip and the distro package manager,
# then installs the matching GitHub release asset.
# curl -fsSL https://tunnelyard.lucascavalheri.com.br/install.sh | bash
set -euo pipefail

REPO="LucasCavalheri/tunnel-yard"
RELEASES="https://github.com/${REPO}/releases"
APP_BIN="tunnel-yard"
LEGACY_APP_BIN="my-vpns"
RUN_DIR="${TUNNEL_YARD_RUN_DIR:-/run}"
APP_WAS_RUNNING=0
APP_RESTARTED=0
INSTALL_SUCCEEDED=0
RUNNING_APP_PIDS=""

usage() {
  cat <<'EOF'
usage: install.sh [--print-plan] [--version VERSION]

  --print-plan   print arch, family, asset and URL, then exit
  --version VER  install this tag (default: latest GitHub release)
EOF
}

PRINT_PLAN=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --print-plan) PRINT_PLAN=1; shift ;;
    --version)
      TUNNEL_YARD_VERSION="${2:?}"
      shift 2
      ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "install.sh needs $1 on PATH" >&2
    exit 1
  }
}

normalize_arch() {
  case "$1" in
    x86_64 | amd64) echo x86_64 ;;
    aarch64 | arm64) echo aarch64 ;;
    *)
      echo "TunnelYard install.sh supports x86_64 and aarch64, not: $1" >&2
      return 1
      ;;
  esac
}

detect_arch() {
  if [[ -n "${TUNNEL_YARD_ARCH:-}" ]]; then
    normalize_arch "$TUNNEL_YARD_ARCH"
    return
  fi
  normalize_arch "$(uname -m)"
}

detect_pm() {
  if [[ -n "${TUNNEL_YARD_PM:-}" ]]; then
    echo "$TUNNEL_YARD_PM"
    return
  fi
  if command -v pacman >/dev/null 2>&1; then
    echo pacman
  elif command -v apk >/dev/null 2>&1; then
    echo apk
  elif command -v apt-get >/dev/null 2>&1; then
    echo apt
  elif command -v dnf >/dev/null 2>&1; then
    echo dnf
  elif command -v yum >/dev/null 2>&1; then
    echo yum
  elif command -v zypper >/dev/null 2>&1; then
    echo zypper
  else
    echo tar
  fi
}

latest_version() {
  if [[ -n "${TUNNEL_YARD_VERSION:-}" ]]; then
    echo "${TUNNEL_YARD_VERSION#v}"
    return
  fi
  need curl
  local url
  url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "${RELEASES}/latest")"
  local tag="${url%/}"
  tag="${tag##*/}"
  echo "${tag#v}"
}

asset_for() {
  local arch="$1" pm="$2" ver="$3"
  case "$pm" in
    apt)
      if [[ "$arch" == x86_64 ]]; then echo "tunnel-yard_${ver}_amd64.deb"
      else echo "tunnel-yard_${ver}_arm64.deb"
      fi
      ;;
    dnf | yum | zypper)
      echo "tunnel-yard-${ver}-1.${arch}.rpm"
      ;;
    pacman)
      echo "tunnel-yard-${ver}-1-${arch}.pkg.tar.zst"
      ;;
    apk)
      echo "tunnel-yard-${ver}-r0-${arch}.apk"
      ;;
    tar)
      if [[ "$arch" == x86_64 ]]; then echo "tunnel-yard-linux-x64.tar.gz"
      else echo "tunnel-yard-linux-arm64.tar.gz"
      fi
      ;;
    *)
      echo "unknown package family: $pm" >&2
      return 1
      ;;
  esac
}

run_root() {
  if [[ "$(id -u)" -eq 0 ]]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    echo "install.sh needs root or sudo" >&2
    exit 1
  fi
}

pid_is_alive() {
  local stat state
  [[ -r "/proc/$1/stat" ]] || return 1
  stat="$(<"/proc/$1/stat")"
  state="${stat##*) }"
  state="${state%% *}"
  [[ "$state" != Z ]]
}

process_name_is() {
  local name
  [[ -r "/proc/$1/comm" ]] || return 1
  name="$(<"/proc/$1/comm")"
  [[ "$name" == "$2" ]]
}

running_app_pids() {
  if [[ -n "${TUNNEL_YARD_RUNNING_PIDS:-}" ]]; then
    printf '%s\n' "$TUNNEL_YARD_RUNNING_PIDS"
    return
  fi

  local uid="$(id -u)"
  local name
  for name in "$APP_BIN" "$LEGACY_APP_BIN"; do
    if command -v pgrep >/dev/null 2>&1; then
      pgrep -u "$uid" -x "$name" || true
    else
      ps -u "$uid" -o pid= -o comm= | awk -v wanted="$name" '$2 == wanted { print $1 }'
    fi
  done
}

active_tunnel_pidfiles() {
  local file pid
  shopt -s nullglob
  for file in "$RUN_DIR"/tunnel-yard-*.pid "$RUN_DIR"/my-vpns-*.pid; do
    [[ -f "$file" ]] || continue
    pid="$(<"$file")"
    if [[ "$pid" =~ ^[0-9]+$ ]] && pid_is_alive "$pid" && process_name_is "$pid" openfortivpn; then
      printf '%s\n' "$file"
    fi
  done
}

active_tunnel_pids() {
  local file pid
  while IFS= read -r file; do
    pid="$(<"$file")"
    [[ "$pid" =~ ^[0-9]+$ ]] && printf '%s\n' "$pid"
  done < <(active_tunnel_pidfiles)
  # Portable/no-helper installs run openfortivpn as the current user instead
  # of writing the privileged helper PID file under /run.
  if command -v pgrep >/dev/null 2>&1; then
    pgrep -u "$(id -u)" -x openfortivpn || true
  fi
}

active_tunnel_count() {
  active_tunnel_pids | sort -nu | wc -l | tr -d ' '
}

confirm_running_app() {
  local active="$1"
  local answer
  if (( active > 0 )); then
    echo "TunnelYard is running with ${active} active VPN tunnel(s)." >&2
    echo "Continuing will disconnect them and restart TunnelYard." >&2
  else
    echo "TunnelYard is running." >&2
    echo "Continuing will close and restart the app after the update." >&2
  fi

  if [[ "${TUNNEL_YARD_AUTO_CONFIRM:-0}" == 1 ]]; then
    return 0
  fi
  if [[ ! -r /dev/tty ]]; then
    echo "Cannot ask for confirmation because no terminal is available; close TunnelYard and retry." >&2
    return 1
  fi
  read -r -p "Continue? [y/N] " answer < /dev/tty
  if [[ ! "$answer" =~ ^[Yy]([Ee][Ss])?$ ]]; then
    echo "Installation cancelled." >&2
    return 1
  fi
}

stop_tunnel_pid() {
  local pid="$1"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 0
  if ! kill -INT "$pid" 2>/dev/null; then
    run_root kill -INT "$pid" 2>/dev/null || true
  fi
  for _ in {1..25}; do
    pid_is_alive "$pid" || return 0
    sleep 0.2
  done
  if ! kill -KILL "$pid" 2>/dev/null; then
    run_root kill -KILL "$pid" 2>/dev/null || true
  fi
}

stop_running_tunnels() {
  local pid
  while IFS= read -r pid; do
    [[ -n "$pid" ]] && stop_tunnel_pid "$pid"
  done < <(active_tunnel_pids | sort -nu)
}

stop_running_app() {
  local pid
  for pid in $RUNNING_APP_PIDS; do
    kill -TERM "$pid" 2>/dev/null || true
  done
  for _ in {1..50}; do
    local alive=0
    for pid in $RUNNING_APP_PIDS; do
      if pid_is_alive "$pid"; then
        alive=1
        break
      fi
    done
    (( alive == 0 )) && return 0
    sleep 0.2
  done
  for pid in $RUNNING_APP_PIDS; do
    pid_is_alive "$pid" && kill -KILL "$pid" 2>/dev/null || true
  done
}

prepare_running_app() {
  RUNNING_APP_PIDS="$(running_app_pids)"
  if [[ -z "$RUNNING_APP_PIDS" ]]; then
    return 0
  fi
  local active="$(active_tunnel_count)"
  confirm_running_app "$active"
  APP_WAS_RUNNING=1
  stop_running_tunnels
  stop_running_app
}

relaunch_running_app() {
  local launcher="${TUNNEL_YARD_LAUNCHER:-}"
  if [[ -z "$launcher" ]]; then
    launcher="$(command -v "$APP_BIN" || command -v "$LEGACY_APP_BIN" || true)"
  fi
  if [[ -z "$launcher" || ! -x "$launcher" ]]; then
    echo "Update installed. Launch $APP_BIN manually to finish restarting it." >&2
    return 0
  fi
  nohup "$launcher" >/dev/null 2>&1 </dev/null &
  APP_RESTARTED=1
}

install_asset() {
  local pm="$1" file="$2" arch="$3"
  case "$pm" in
    apt) run_root apt-get install -y "$file" ;;
    dnf) run_root dnf install -y "$file" ;;
    yum) run_root yum install -y "$file" ;;
    zypper) run_root zypper --non-interactive install "$file" ;;
    pacman) run_root pacman -U --noconfirm "$file" ;;
    apk)
      run_root apk add gcompat
      run_root apk add --allow-untrusted "$file"
      ;;
    tar)
      local dir unpack
      dir="$(mktemp -d)"
      tar -xzf "$file" -C "$dir"
      unpack="$(find "$dir" -type f -perm -u+x | head -n 1)"
      [[ -n "$unpack" ]] || {
        echo "the archive did not contain an executable" >&2
        exit 1
      }
      run_root install -m 0755 "$unpack" /usr/local/bin/tunnel-yard
      echo "installed /usr/local/bin/tunnel-yard (no PolicyKit helpers; prefer a distro package when you can)"
      ;;
    *)
      echo "cannot install family $pm" >&2
      return 1
      ;;
  esac
}

main() {
ARCH="$(detect_arch)"
PM="$(detect_pm)"
VERSION="$(latest_version)"
ASSET="$(asset_for "$ARCH" "$PM" "$VERSION")"
URL="${RELEASES}/download/v${VERSION}/${ASSET}"

if [[ "$PRINT_PLAN" -eq 1 ]]; then
  printf 'arch=%s\nfamily=%s\nversion=%s\nasset=%s\nurl=%s\n' \
    "$ARCH" "$PM" "$VERSION" "$ASSET" "$URL"
  exit 0
fi

need curl
echo "TunnelYard ${VERSION} · ${ARCH} · ${PM}"
echo "downloading ${ASSET}"
TMP="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP"
  if (( APP_WAS_RUNNING == 1 && INSTALL_SUCCEEDED == 0 && APP_RESTARTED == 0 )); then
    relaunch_running_app || true
  fi
}
trap cleanup EXIT
FILE="${TMP}/${ASSET}"
curl -fL --progress-bar -o "$FILE" "$URL"
# apt fetches local files as user `_apt`. mktemp dirs are 0700, so that
# user cannot read the .deb and prints a noisy permission note.
chmod 0755 "$TMP"
chmod 0644 "$FILE"
prepare_running_app
install_asset "$PM" "$FILE" "$ARCH"
INSTALL_SUCCEEDED=1
if (( APP_WAS_RUNNING == 1 )); then
  relaunch_running_app
fi
echo "done. launch tunnel-yard from the menu or run: tunnel-yard"
}

if [[ "${TUNNEL_YARD_SOURCE_ONLY:-0}" != 1 ]]; then
  main "$@"
fi

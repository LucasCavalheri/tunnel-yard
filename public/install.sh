#!/usr/bin/env bash
# TunnelYard Linux installer. Detects the chip and the distro package manager,
# then installs the matching GitHub release asset.
# curl -fsSL https://tunnelyard.lucascavalheri.com.br/install.sh | bash
set -euo pipefail

REPO="LucasCavalheri/tunnel-yard"
RELEASES="https://github.com/${REPO}/releases"

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
trap 'rm -rf "$TMP"' EXIT
FILE="${TMP}/${ASSET}"
curl -fL --progress-bar -o "$FILE" "$URL"
# apt fetches local files as user `_apt`. mktemp dirs are 0700, so that
# user cannot read the .deb and prints a noisy permission note.
chmod 0755 "$TMP"
chmod 0644 "$FILE"
install_asset "$PM" "$FILE" "$ARCH"
echo "done. launch tunnel-yard from the menu or run: tunnel-yard"

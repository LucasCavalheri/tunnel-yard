#!/usr/bin/env bash
# Build an Arch pacman package and an Alpine apk from an already-populated
# filesystem root (usr/bin, usr/lib, usr/share, …).
set -euo pipefail

usage() {
  echo "usage: $0 <root-dir> <version> <x86_64|aarch64> <output-dir>" >&2
  exit 2
}

[[ $# -eq 4 ]] || usage

ROOT="$1"
VERSION="$2"
NATIVE_ARCH="$3"
OUTPUT_DIR="$4"

case "$NATIVE_ARCH" in
  x86_64 | aarch64) ;;
  *)
    echo "unsupported native architecture: $NATIVE_ARCH" >&2
    exit 2
    ;;
esac

[[ -d "$ROOT/usr" ]] || { echo "package root is missing usr/: $ROOT" >&2; exit 1; }
command -v tar >/dev/null || { echo "tar is required" >&2; exit 1; }
command -v zstd >/dev/null || { echo "zstd is required for the Arch package" >&2; exit 1; }
mkdir -p "$OUTPUT_DIR"

SIZE="$(du -sb "$ROOT" | awk '{print $1}')"
BUILDDATE="$(date -u +%s)"
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

stage_copy() {
  local dest="$1"
  rm -rf "$dest"
  mkdir -p "$dest"
  cp -a "$ROOT/." "$dest/"
  rm -rf "$dest/DEBIAN"
}

write_pkginfo() {
  local dest="$1"
  local kind="$2"
  if [[ "$kind" == arch ]]; then
    cat > "$dest/.PKGINFO" <<EOF
pkgname = tunnel-yard
pkgbase = tunnel-yard
pkgver = ${VERSION}-1
pkgdesc = TunnelYard graphical OpenFortiVPN manager
url = https://github.com/LucasCavalheri/tunnel-yard
builddate = ${BUILDDATE}
packager = Lucas Cavalheri <lucas.dev.carvalho@gmail.com>
size = ${SIZE}
arch = ${NATIVE_ARCH}
license = MIT
depend = glibc
depend = polkit
depend = libxcb
depend = libxkbcommon
depend = libxkbcommon-x11
EOF
  else
    cat > "$dest/.PKGINFO" <<EOF
pkgname = tunnel-yard
pkgver = ${VERSION}-r0
pkgdesc = TunnelYard graphical OpenFortiVPN manager
url = https://github.com/LucasCavalheri/tunnel-yard
builddate = ${BUILDDATE}
size = ${SIZE}
arch = ${NATIVE_ARCH}
origin = tunnel-yard
maintainer = Lucas Cavalheri <lucas.dev.carvalho@gmail.com>
license = MIT
depend = gcompat
depend = polkit
EOF
  fi
}

ARCH_STAGE="$WORKDIR/arch"
stage_copy "$ARCH_STAGE"
write_pkginfo "$ARCH_STAGE" arch
ARCH_OUT="$OUTPUT_DIR/tunnel-yard-${VERSION}-1-${NATIVE_ARCH}.pkg.tar.zst"
(
  cd "$ARCH_STAGE"
  tar --format=gnu --owner=0 --group=0 --numeric-owner -cf - .PKGINFO usr
) | zstd -T0 -19 -c > "$ARCH_OUT"

APK_STAGE="$WORKDIR/apk"
stage_copy "$APK_STAGE"
write_pkginfo "$APK_STAGE" apk
APK_OUT="$OUTPUT_DIR/tunnel-yard-${VERSION}-r0-${NATIVE_ARCH}.apk"
(
  cd "$APK_STAGE"
  tar --format=ustar --owner=0 --group=0 --numeric-owner -czf "$APK_OUT" .PKGINFO usr
)

echo "created: $ARCH_OUT"
echo "created: $APK_OUT"

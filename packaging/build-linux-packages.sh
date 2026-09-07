#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <binary> <version> <amd64|arm64> <output-dir>" >&2
  exit 2
}

[[ $# -eq 4 ]] || usage

BINARY="$1"
VERSION="$2"
PACKAGE_ARCH="$3"
OUTPUT_DIR="$4"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

case "$PACKAGE_ARCH" in
  amd64)
    DEB_ARCH="amd64"
    RPM_ARCH="x86_64"
    ;;
  arm64)
    DEB_ARCH="arm64"
    RPM_ARCH="aarch64"
    ;;
  *)
    echo "unsupported package architecture: $PACKAGE_ARCH" >&2
    exit 2
    ;;
esac

[[ -f "$BINARY" ]] || { echo "binary not found: $BINARY" >&2; exit 1; }
mkdir -p "$OUTPUT_DIR"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

populate_root() {
  local stage="$1"

  install -d \
    "$stage/usr/bin" \
    "$stage/usr/lib/my-vpns" \
    "$stage/usr/share/applications" \
    "$stage/usr/share/polkit-1/actions" \
    "$stage/usr/share/icons/hicolor/32x32/apps" \
    "$stage/usr/share/icons/hicolor/64x64/apps" \
    "$stage/usr/share/icons/hicolor/256x256/apps" \
    "$stage/usr/share/keyrings" \
    "$stage/usr/share/licenses/my-vpns"

  install -m 0755 "$BINARY" "$stage/usr/bin/my-vpns"
  install -m 0755 "$ROOT_DIR/packaging/run-vpn.sh" "$stage/usr/lib/my-vpns/run-vpn.sh"
  install -m 0755 "$ROOT_DIR/packaging/stop-vpn.sh" "$stage/usr/lib/my-vpns/stop-vpn.sh"
  install -m 0644 "$ROOT_DIR/packaging/my-vpns.desktop" \
    "$stage/usr/share/applications/dev.cavallheri.myvpns.desktop"
  install -m 0644 "$ROOT_DIR/packaging/polkit/dev.cavallheri.myvpns.policy" \
    "$stage/usr/share/polkit-1/actions/dev.cavallheri.myvpns.policy"
  install -m 0644 "$ROOT_DIR/public/icon-32.png" "$stage/usr/share/icons/hicolor/32x32/apps/my-vpns.png"
  install -m 0644 "$ROOT_DIR/public/icon-64.png" "$stage/usr/share/icons/hicolor/64x64/apps/my-vpns.png"
  install -m 0644 "$ROOT_DIR/public/icon.png" "$stage/usr/share/icons/hicolor/256x256/apps/my-vpns.png"
  install -m 0644 "$ROOT_DIR/packaging/my-vpns-archive-keyring.asc" \
    "$stage/usr/share/keyrings/my-vpns-archive-keyring.asc"
  install -m 0644 "$ROOT_DIR/LICENSE" "$stage/usr/share/licenses/my-vpns/LICENSE"
}

DEB_ROOT="$TMP_DIR/deb-root"
populate_root "$DEB_ROOT"
install -d "$DEB_ROOT/DEBIAN"
cat > "$DEB_ROOT/DEBIAN/control" <<EOF
Package: my-vpns
Version: $VERSION
Section: net
Priority: optional
Architecture: $DEB_ARCH
Maintainer: Lucas Cavalheri <lucas.dev.carvalho@gmail.com>
Depends: libc6, libxcb1, libxkbcommon0, libxkbcommon-x11-0, libx11-xcb1, libxau6, libxdmcp6, policykit-1
Description: My VPNs graphical OpenFortiVPN manager
 A native desktop application for managing FortiGate SSL VPN connections.
 Includes PolicyKit helpers and a desktop launcher.
EOF
install -m 0755 "$ROOT_DIR/packaging/after-install.sh" "$DEB_ROOT/DEBIAN/postinst"
install -m 0755 "$ROOT_DIR/packaging/after-remove.sh" "$DEB_ROOT/DEBIAN/postrm"
dpkg-deb --build --root-owner-group "$DEB_ROOT" \
  "$OUTPUT_DIR/my-vpns_${VERSION}_${DEB_ARCH}.deb" >/dev/null

RPM_TOP="$TMP_DIR/rpm"
RPM_SOURCE_ROOT="$TMP_DIR/my-vpns-$VERSION"
mkdir -p "$RPM_SOURCE_ROOT"
populate_root "$RPM_SOURCE_ROOT"
mkdir -p "$RPM_TOP"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
tar -C "$TMP_DIR" -czf "$RPM_TOP/SOURCES/my-vpns-$VERSION.tar.gz" "my-vpns-$VERSION"
cat > "$RPM_TOP/SPECS/my-vpns.spec" <<EOF
Name:           my-vpns
Version:        $VERSION
Release:        1%{?dist}
Summary:        My VPNs graphical OpenFortiVPN manager
License:        MIT
URL:            https://github.com/LucasCavalheri/my-vpns
Source0:        my-vpns-%{version}.tar.gz
BuildArch:      $RPM_ARCH
AutoReqProv:    no
Requires:       glibc
Requires:       libxcb
Requires:       libxkbcommon
Requires:       libxkbcommon-x11
Requires:       libX11-xcb
Requires:       libXau
Requires:       libXdmcp
Requires:       polkit

%description
A native desktop application for managing FortiGate SSL VPN connections.
Includes PolicyKit helpers and a desktop launcher.

%prep
%setup -q

%install
rm -rf %{buildroot}
cp -a . %{buildroot}/

%files
/usr/bin/my-vpns
/usr/lib/my-vpns
/usr/share/applications/dev.cavallheri.myvpns.desktop
/usr/share/icons/hicolor/32x32/apps/my-vpns.png
/usr/share/icons/hicolor/64x64/apps/my-vpns.png
/usr/share/icons/hicolor/256x256/apps/my-vpns.png
/usr/share/keyrings/my-vpns-archive-keyring.asc
/usr/share/licenses/my-vpns/LICENSE
/usr/share/polkit-1/actions/dev.cavallheri.myvpns.policy

%post
if [ -x /usr/lib/my-vpns/run-vpn.sh ]; then
  chmod 0755 /usr/lib/my-vpns/run-vpn.sh /usr/lib/my-vpns/stop-vpn.sh || :
fi

%postun
if [ "\$1" -eq 0 ]; then
  rm -f /etc/apt/sources.list.d/my-vpns.list
  rm -f /usr/share/keyrings/my-vpns-archive-keyring.asc
fi
EOF

rpmbuild --define "_topdir $RPM_TOP" --target "$RPM_ARCH" -bb "$RPM_TOP/SPECS/my-vpns.spec" >/dev/null
RPM_FILE="$(find "$RPM_TOP/RPMS" -type f -name '*.rpm' -print -quit)"
[[ -n "$RPM_FILE" ]] || { echo "rpmbuild did not produce an RPM" >&2; exit 1; }
cp "$RPM_FILE" "$OUTPUT_DIR/my-vpns-${VERSION}-1.${RPM_ARCH}.rpm"

echo "created: $OUTPUT_DIR/my-vpns_${VERSION}_${DEB_ARCH}.deb"
echo "created: $OUTPUT_DIR/my-vpns-${VERSION}-1.${RPM_ARCH}.rpm"

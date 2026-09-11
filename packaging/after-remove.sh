#!/bin/bash
set -e

# TUNNEL_YARD_ROOT is used only by the isolated maintainer-script regression
# test.  With no override these resolve to the real filesystem paths expected
# by dpkg.
ROOT_PREFIX="${TUNNEL_YARD_ROOT:-}"
root_path() {
  printf '%s%s' "$ROOT_PREFIX" "$1"
}

# During an upgrade dpkg runs the old package's postrm after unpacking the
# replacement package.  Removing shared paths here would therefore delete the
# files that the new package just installed, leaving dpkg convinced the package
# is healthy while the launcher and executable are gone.
case "${1:-}" in
  remove|purge)
    ;;
  *)
    exit 0
    ;;
esac

rm -f "$(root_path /usr/lib/tunnel-yard/run-vpn.sh)"
rm -f "$(root_path /usr/lib/tunnel-yard/stop-vpn.sh)"
rmdir "$(root_path /usr/lib/tunnel-yard)" 2>/dev/null || true
rm -f "$(root_path /usr/lib/my-vpns/run-vpn.sh)"
rm -f "$(root_path /usr/lib/my-vpns/stop-vpn.sh)"
rmdir "$(root_path /usr/lib/my-vpns)" 2>/dev/null || true
rm -f "$(root_path /usr/share/polkit-1/actions/lucas.cavalheri.tunnelyard.policy)"
rm -f "$(root_path /usr/share/polkit-1/actions/dev.cavallheri.myvpns.policy)"
rm -f "$(root_path /usr/bin/tunnel-yard)"
rm -f "$(root_path /usr/bin/my-vpns)"
rm -f "$(root_path /usr/share/applications/lucas.cavalheri.tunnelyard.desktop)"
rm -f "$(root_path /usr/share/applications/tunnel-yard.desktop)"
rm -f "$(root_path /usr/share/applications/dev.cavallheri.myvpns.desktop)"
rm -f "$(root_path /usr/share/applications/my-vpns.desktop)"
rm -f "$(root_path /usr/share/icons/hicolor/256x256/apps/tunnel-yard.png)"
rm -f "$(root_path /usr/share/icons/hicolor/64x64/apps/tunnel-yard.png)"
rm -f "$(root_path /usr/share/icons/hicolor/32x32/apps/tunnel-yard.png)"
rm -f "$(root_path /usr/share/pixmaps/tunnel-yard.png)"
rm -f "$(root_path /usr/share/icons/hicolor/256x256/apps/my-vpns.png)"
rm -f "$(root_path /usr/share/icons/hicolor/64x64/apps/my-vpns.png)"
rm -f "$(root_path /usr/share/icons/hicolor/32x32/apps/my-vpns.png)"
rm -f "$(root_path /usr/share/pixmaps/my-vpns.png)"
rm -f "$(root_path /etc/apt/sources.list.d/tunnel-yard.list)"
rm -f "$(root_path /etc/apt/sources.list.d/my-vpns.list)"
rm -f "$(root_path /usr/share/keyrings/tunnel-yard-archive-keyring.asc)"
rm -f "$(root_path /usr/share/keyrings/my-vpns-archive-keyring.asc)"
rm -f "$(root_path /etc/apparmor.d/tunnel-yard)"
rm -f "$(root_path /etc/apparmor.d/my-vpns)"

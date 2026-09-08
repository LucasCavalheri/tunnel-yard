#!/bin/bash
set -e

rm -f /usr/lib/tunnel-yard/run-vpn.sh
rm -f /usr/lib/tunnel-yard/stop-vpn.sh
rmdir /usr/lib/tunnel-yard 2>/dev/null || true
rm -f /usr/lib/my-vpns/run-vpn.sh
rm -f /usr/lib/my-vpns/stop-vpn.sh
rmdir /usr/lib/my-vpns 2>/dev/null || true
rm -f /usr/share/polkit-1/actions/lucas.cavalheri.tunnelyard.policy
rm -f /usr/share/polkit-1/actions/dev.cavallheri.myvpns.policy
rm -f /usr/bin/tunnel-yard
rm -f /usr/bin/my-vpns
rm -f /usr/share/applications/lucas.cavalheri.tunnelyard.desktop
rm -f /usr/share/applications/tunnel-yard.desktop
rm -f /usr/share/applications/dev.cavallheri.myvpns.desktop
rm -f /usr/share/applications/my-vpns.desktop
rm -f /usr/share/icons/hicolor/256x256/apps/tunnel-yard.png
rm -f /usr/share/icons/hicolor/64x64/apps/tunnel-yard.png
rm -f /usr/share/icons/hicolor/32x32/apps/tunnel-yard.png
rm -f /usr/share/pixmaps/tunnel-yard.png
rm -f /usr/share/icons/hicolor/256x256/apps/my-vpns.png
rm -f /usr/share/icons/hicolor/64x64/apps/my-vpns.png
rm -f /usr/share/icons/hicolor/32x32/apps/my-vpns.png
rm -f /usr/share/pixmaps/my-vpns.png
rm -f /etc/apt/sources.list.d/tunnel-yard.list
rm -f /etc/apt/sources.list.d/my-vpns.list
rm -f /usr/share/keyrings/tunnel-yard-archive-keyring.asc
rm -f /usr/share/keyrings/my-vpns-archive-keyring.asc
rm -f /etc/apparmor.d/tunnel-yard
rm -f /etc/apparmor.d/my-vpns

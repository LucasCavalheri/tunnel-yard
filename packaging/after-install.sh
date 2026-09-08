#!/bin/bash
set -e

install -d /usr/lib/tunnel-yard

APP_DIR=""
for candidate in "/opt/TunnelYard" "/opt/tunnel-yard" "/opt/My VPNs" "/opt/my-vpns"; do
  if [ -x "$candidate/tunnel-yard" ] || [ -x "$candidate/my-vpns" ]; then
    APP_DIR="$candidate"
    break
  fi
done

if [ -z "$APP_DIR" ] && [ -x /usr/bin/tunnel-yard ]; then
  chmod 0755 /usr/bin/tunnel-yard
  if [ -f /usr/lib/tunnel-yard/run-vpn.sh ]; then
    chmod 0755 /usr/lib/tunnel-yard/run-vpn.sh /usr/lib/tunnel-yard/stop-vpn.sh
  fi
  if [ -f /usr/share/applications/lucas.cavalheri.tunnelyard.desktop ]; then
    sed -i 's|^Exec=.*|Exec=/usr/bin/tunnel-yard %U|' \
      /usr/share/applications/lucas.cavalheri.tunnelyard.desktop
    sed -i 's|^StartupWMClass=.*|StartupWMClass=lucas.cavalheri.tunnelyard|' \
      /usr/share/applications/lucas.cavalheri.tunnelyard.desktop
  fi
fi

if [ -n "$APP_DIR" ]; then
  for res in "$APP_DIR/packaging" "$APP_DIR/helpers" "$APP_DIR/share/tunnel-yard" "$APP_DIR/share/my-vpns"; do
    if [ -f "$res/run-vpn.sh" ]; then
      install -m 0755 "$res/run-vpn.sh" /usr/lib/tunnel-yard/run-vpn.sh
      install -m 0755 "$res/stop-vpn.sh" /usr/lib/tunnel-yard/stop-vpn.sh
      break
    fi
  done

  for polkit in \
    "$APP_DIR/packaging/polkit/lucas.cavalheri.tunnelyard.policy" \
    "$APP_DIR/polkit/lucas.cavalheri.tunnelyard.policy"; do
    if [ -f "$polkit" ]; then
      install -d /usr/share/polkit-1/actions
      install -m 0644 "$polkit" /usr/share/polkit-1/actions/lucas.cavalheri.tunnelyard.policy
      break
    fi
  done

  BIN="$APP_DIR/tunnel-yard"
  [ -x "$BIN" ] || BIN="$APP_DIR/my-vpns"
  cat > /usr/bin/tunnel-yard << EOF
#!/bin/bash
exec "$BIN" "\$@"
EOF
  chmod 755 /usr/bin/tunnel-yard

  for src_desktop in \
    "$APP_DIR/packaging/tunnel-yard.desktop" \
    "$APP_DIR/tunnel-yard.desktop" \
    "$APP_DIR/packaging/my-vpns.desktop"; do
    if [ -f "$src_desktop" ]; then
      install -d /usr/share/applications
      install -m 0644 "$src_desktop" /usr/share/applications/lucas.cavalheri.tunnelyard.desktop
      break
    fi
  done
  for DESKTOP in \
    /usr/share/applications/lucas.cavalheri.tunnelyard.desktop \
    /usr/share/applications/tunnel-yard.desktop \
    /usr/share/applications/dev.cavallheri.myvpns.desktop \
    /usr/share/applications/my-vpns.desktop; do
    if [ -f "$DESKTOP" ]; then
      sed -i 's|^Exec=.*|Exec=/usr/bin/tunnel-yard %U|' "$DESKTOP"
      sed -i 's|^StartupWMClass=.*|StartupWMClass=lucas.cavalheri.tunnelyard|' "$DESKTOP"
      if ! grep -q '^StartupWMClass=' "$DESKTOP"; then
        printf '\nStartupWMClass=lucas.cavalheri.tunnelyard\n' >> "$DESKTOP"
      fi
    fi
  done
  if [ -f "$APP_DIR/public/icon.png" ] || [ -f "$APP_DIR/icon.png" ]; then
    src256="$APP_DIR/public/icon.png"
    [ -f "$src256" ] || src256="$APP_DIR/icon.png"
    install -d /usr/share/icons/hicolor/256x256/apps
    install -m 0644 "$src256" /usr/share/icons/hicolor/256x256/apps/tunnel-yard.png
    if [ -f "$APP_DIR/public/icon-64.png" ]; then
      install -d /usr/share/icons/hicolor/64x64/apps
      install -m 0644 "$APP_DIR/public/icon-64.png" /usr/share/icons/hicolor/64x64/apps/tunnel-yard.png
    fi
    if [ -f "$APP_DIR/public/icon-32.png" ]; then
      install -d /usr/share/icons/hicolor/32x32/apps
      install -m 0644 "$APP_DIR/public/icon-32.png" /usr/share/icons/hicolor/32x32/apps/tunnel-yard.png
    fi
    if [ -f "$APP_DIR/public/icon.png" ]; then
      install -d /usr/share/pixmaps
      install -m 0644 "$APP_DIR/public/icon.png" /usr/share/pixmaps/tunnel-yard.png
    fi
  fi
fi

rm -f /usr/share/applications/dev.cavallheri.myvpns.desktop \
  /usr/share/applications/my-vpns.desktop \
  /usr/share/polkit-1/actions/dev.cavallheri.myvpns.policy \
  /usr/share/icons/hicolor/256x256/apps/my-vpns.png \
  /usr/share/icons/hicolor/64x64/apps/my-vpns.png \
  /usr/share/icons/hicolor/32x32/apps/my-vpns.png \
  /usr/share/pixmaps/my-vpns.png

KEYRING_SOURCE=""
for key in \
  /usr/share/keyrings/tunnel-yard-archive-keyring.asc \
  "$APP_DIR/packaging/tunnel-yard-archive-keyring.asc" \
  "$APP_DIR/tunnel-yard-archive-keyring.asc" \
  /usr/share/keyrings/my-vpns-archive-keyring.asc; do
  if [ -n "$key" ] && [ -f "$key" ]; then KEYRING_SOURCE="$key"; break; fi
done
KEYRING_DEST="/usr/share/keyrings/tunnel-yard-archive-keyring.asc"
SOURCE_LIST="/etc/apt/sources.list.d/tunnel-yard.list"

if [ -d /etc/apt/sources.list.d ] && [ -f "$KEYRING_SOURCE" ]; then
  install -d /usr/share/keyrings
  if [ "$KEYRING_SOURCE" != "$KEYRING_DEST" ]; then
    install -m 0644 "$KEYRING_SOURCE" "$KEYRING_DEST"
  fi
  cat > "$SOURCE_LIST" << EOF
deb [arch=amd64,arm64 signed-by=$KEYRING_DEST] https://lucascavalheri.github.io/tunnel-yard/apt ./
EOF
  rm -f /etc/apt/sources.list.d/my-vpns.list
elif [ -d /etc/apt/sources.list.d ]; then
  rm -f "$SOURCE_LIST" /etc/apt/sources.list.d/my-vpns.list
fi

exit 0

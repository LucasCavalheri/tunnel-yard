#!/bin/bash
set -e

install -d /usr/lib/my-vpns

APP_DIR=""
for candidate in "/opt/My VPNs" "/opt/my-vpns"; do
  if [ -x "$candidate/my-vpns" ]; then
    APP_DIR="$candidate"
    break
  fi
done

if [ -n "$APP_DIR" ]; then
  for res in "$APP_DIR/packaging" "$APP_DIR/helpers" "$APP_DIR/share/my-vpns"; do
    if [ -f "$res/run-vpn.sh" ]; then
      install -m 0755 "$res/run-vpn.sh" /usr/lib/my-vpns/run-vpn.sh
      install -m 0755 "$res/stop-vpn.sh" /usr/lib/my-vpns/stop-vpn.sh
      break
    fi
  done

  for polkit in \
    "$APP_DIR/packaging/polkit/dev.cavallheri.myvpns.policy" \
    "$APP_DIR/polkit/dev.cavallheri.myvpns.policy"; do
    if [ -f "$polkit" ]; then
      install -d /usr/share/polkit-1/actions
      install -m 0644 "$polkit" /usr/share/polkit-1/actions/dev.cavallheri.myvpns.policy
      break
    fi
  done

  cat > /usr/bin/my-vpns << EOF
#!/bin/bash
exec "$APP_DIR/my-vpns" "\$@"
EOF
  chmod 755 /usr/bin/my-vpns

  for src_desktop in \
    "$APP_DIR/packaging/my-vpns.desktop" \
    "$APP_DIR/my-vpns.desktop"; do
    if [ -f "$src_desktop" ]; then
      install -d /usr/share/applications
      install -m 0644 "$src_desktop" /usr/share/applications/dev.cavallheri.myvpns.desktop
      break
    fi
  done
  for DESKTOP in \
    /usr/share/applications/dev.cavallheri.myvpns.desktop \
    /usr/share/applications/my-vpns.desktop \
    /usr/share/applications/My\ VPNs.desktop; do
    if [ -f "$DESKTOP" ]; then
      sed -i 's|^Exec=.*|Exec=/usr/bin/my-vpns %U|' "$DESKTOP"
      sed -i 's|^StartupWMClass=.*|StartupWMClass=dev.cavallheri.myvpns|' "$DESKTOP"
      if ! grep -q '^StartupWMClass=' "$DESKTOP"; then
        printf '\nStartupWMClass=dev.cavallheri.myvpns\n' >> "$DESKTOP"
      fi
    fi
  done
  if [ -f "$APP_DIR/public/icon.png" ] || [ -f "$APP_DIR/icon.png" ]; then
    src256="$APP_DIR/public/icon.png"
    [ -f "$src256" ] || src256="$APP_DIR/icon.png"
    install -d /usr/share/icons/hicolor/256x256/apps
    install -m 0644 "$src256" /usr/share/icons/hicolor/256x256/apps/my-vpns.png
    if [ -f "$APP_DIR/public/icon-64.png" ]; then
      install -d /usr/share/icons/hicolor/64x64/apps
      install -m 0644 "$APP_DIR/public/icon-64.png" /usr/share/icons/hicolor/64x64/apps/my-vpns.png
    fi
    if [ -f "$APP_DIR/public/icon-32.png" ]; then
      install -d /usr/share/icons/hicolor/32x32/apps
      install -m 0644 "$APP_DIR/public/icon-32.png" /usr/share/icons/hicolor/32x32/apps/my-vpns.png
    fi
    if [ -f "$APP_DIR/public/icon.ico" ]; then
      install -d /usr/share/pixmaps
      install -m 0644 "$APP_DIR/public/icon.png" /usr/share/pixmaps/my-vpns.png
    fi
  fi
fi

# Signed APT repo so `sudo apt upgrade` can pull newer builds from GitHub Pages
KEYRING_SOURCE=""
for key in \
  "$APP_DIR/packaging/my-vpns-archive-keyring.asc" \
  "$APP_DIR/my-vpns-archive-keyring.asc"; do
  if [ -f "$key" ]; then KEYRING_SOURCE="$key"; break; fi
done
KEYRING_DEST="/usr/share/keyrings/my-vpns-archive-keyring.asc"
SOURCE_LIST="/etc/apt/sources.list.d/my-vpns.list"

if [ -d /etc/apt/sources.list.d ] && [ -f "$KEYRING_SOURCE" ]; then
  install -d /usr/share/keyrings
  install -m 0644 "$KEYRING_SOURCE" "$KEYRING_DEST"
  cat > "$SOURCE_LIST" << EOF
deb [arch=amd64 signed-by=$KEYRING_DEST] https://lucascavalheri.github.io/my-vpns/apt ./
EOF
elif [ -d /etc/apt/sources.list.d ]; then
  # Never leave an old, untrusted source enabled after an incomplete upgrade.
  rm -f "$SOURCE_LIST"
fi

exit 0

#!/bin/bash
set -e

ROOT_PREFIX="${TUNNEL_YARD_ROOT:-}"
root_path() {
  printf '%s%s' "$ROOT_PREFIX" "$1"
}

normalize_desktop_entry() {
  local desktop="$1"
  sed -i.bak 's|^Exec=.*|Exec=/usr/bin/tunnel-yard %U|' "$desktop"
  sed -i.bak 's|^StartupWMClass=.*|StartupWMClass=lucas.cavalheri.tunnelyard|' "$desktop"
  rm -f "$desktop.bak"
}

PACKAGE_ROOT="$(root_path /usr/lib/tunnel-yard)"
PACKAGE_BIN="$PACKAGE_ROOT/tunnel-yard"
PACKAGE_PAYLOAD="$PACKAGE_ROOT/payload"
USR_BIN="$(root_path /usr/bin)"
APPLICATIONS="$(root_path /usr/share/applications)"
POLKIT_ACTIONS="$(root_path /usr/share/polkit-1/actions)"
ICON_32="$(root_path /usr/share/icons/hicolor/32x32/apps)"
ICON_64="$(root_path /usr/share/icons/hicolor/64x64/apps)"
ICON_256="$(root_path /usr/share/icons/hicolor/256x256/apps)"
PIXMAPS="$(root_path /usr/share/pixmaps)"
KEYRINGS="$(root_path /usr/share/keyrings)"
APT_SOURCES="$(root_path /etc/apt/sources.list.d)"

install -d "$PACKAGE_ROOT"

APP_DIR=""
for candidate in \
  "$(root_path '/opt/TunnelYard')" \
  "$(root_path '/opt/tunnel-yard')" \
  "$(root_path '/opt/My VPNs')" \
  "$(root_path '/opt/my-vpns')"; do
  if [ -x "$candidate/tunnel-yard" ] || [ -x "$candidate/my-vpns" ]; then
    APP_DIR="$candidate"
    break
  fi
done

if [ -z "$APP_DIR" ] && [ -x "$USR_BIN/tunnel-yard" ]; then
  chmod 0755 "$USR_BIN/tunnel-yard"
  if [ -f "$PACKAGE_ROOT/run-vpn.sh" ]; then
    chmod 0755 "$PACKAGE_ROOT/run-vpn.sh" "$PACKAGE_ROOT/stop-vpn.sh"
  fi
  if [ -f "$APPLICATIONS/lucas.cavalheri.tunnelyard.desktop" ]; then
    normalize_desktop_entry "$APPLICATIONS/lucas.cavalheri.tunnelyard.desktop"
  fi
fi

# Recover package-owned files after upgrading from a release whose postrm
# removed the newly unpacked files.  The payload lives below /usr/lib/tunnel-yard
# and is deliberately outside every path that the legacy postrm deletes.
if [ -x "$PACKAGE_BIN" ]; then
  install -d "$USR_BIN"
  ln -sfn ../lib/tunnel-yard/tunnel-yard "$USR_BIN/tunnel-yard"
  chmod 0755 "$PACKAGE_BIN"

  for helper in run-vpn.sh stop-vpn.sh; do
    if [ -f "$PACKAGE_PAYLOAD/$helper" ]; then
      install -m 0755 "$PACKAGE_PAYLOAD/$helper" "$PACKAGE_ROOT/$helper"
    fi
  done

  if [ -f "$PACKAGE_PAYLOAD/lucas.cavalheri.tunnelyard.policy" ]; then
    install -d "$POLKIT_ACTIONS"
    install -m 0644 "$PACKAGE_PAYLOAD/lucas.cavalheri.tunnelyard.policy" \
      "$POLKIT_ACTIONS/lucas.cavalheri.tunnelyard.policy"
  fi
  if [ -f "$PACKAGE_PAYLOAD/tunnel-yard.desktop" ]; then
    install -d "$APPLICATIONS"
    install -m 0644 "$PACKAGE_PAYLOAD/tunnel-yard.desktop" \
      "$APPLICATIONS/lucas.cavalheri.tunnelyard.desktop"
    normalize_desktop_entry "$APPLICATIONS/lucas.cavalheri.tunnelyard.desktop"
    if ! grep -q '^StartupWMClass=' "$APPLICATIONS/lucas.cavalheri.tunnelyard.desktop"; then
      printf '\nStartupWMClass=lucas.cavalheri.tunnelyard\n' >> \
        "$APPLICATIONS/lucas.cavalheri.tunnelyard.desktop"
    fi
  fi
  if [ -f "$PACKAGE_PAYLOAD/icon.png" ]; then
    install -d "$ICON_256" "$PIXMAPS"
    install -m 0644 "$PACKAGE_PAYLOAD/icon.png" \
      "$ICON_256/tunnel-yard.png"
    install -m 0644 "$PACKAGE_PAYLOAD/icon.png" "$PIXMAPS/tunnel-yard.png"
  fi
  for size in 32 64; do
    if [ -f "$PACKAGE_PAYLOAD/icon-$size.png" ]; then
      icon_dir="$(root_path "/usr/share/icons/hicolor/${size}x${size}/apps")"
      install -d "$icon_dir"
      install -m 0644 "$PACKAGE_PAYLOAD/icon-$size.png" \
        "$icon_dir/tunnel-yard.png"
    fi
  done
  if [ -f "$PACKAGE_PAYLOAD/tunnel-yard-archive-keyring.asc" ]; then
    install -d "$KEYRINGS"
    install -m 0644 "$PACKAGE_PAYLOAD/tunnel-yard-archive-keyring.asc" \
      "$KEYRINGS/tunnel-yard-archive-keyring.asc"
  fi
fi

if [ -n "$APP_DIR" ]; then
  for res in "$APP_DIR/packaging" "$APP_DIR/helpers" "$APP_DIR/share/tunnel-yard" "$APP_DIR/share/my-vpns"; do
    if [ -f "$res/run-vpn.sh" ]; then
      install -m 0755 "$res/run-vpn.sh" "$PACKAGE_ROOT/run-vpn.sh"
      install -m 0755 "$res/stop-vpn.sh" "$PACKAGE_ROOT/stop-vpn.sh"
      break
    fi
  done

  for polkit in \
    "$APP_DIR/packaging/polkit/lucas.cavalheri.tunnelyard.policy" \
    "$APP_DIR/polkit/lucas.cavalheri.tunnelyard.policy"; do
    if [ -f "$polkit" ]; then
      install -d "$POLKIT_ACTIONS"
      install -m 0644 "$polkit" "$POLKIT_ACTIONS/lucas.cavalheri.tunnelyard.policy"
      break
    fi
  done

  BIN="$APP_DIR/tunnel-yard"
  [ -x "$BIN" ] || BIN="$APP_DIR/my-vpns"
  cat > "$USR_BIN/tunnel-yard" << EOF
#!/bin/bash
exec "$BIN" "\$@"
EOF
  chmod 755 "$USR_BIN/tunnel-yard"

  for src_desktop in \
    "$APP_DIR/packaging/tunnel-yard.desktop" \
    "$APP_DIR/tunnel-yard.desktop" \
    "$APP_DIR/packaging/my-vpns.desktop"; do
    if [ -f "$src_desktop" ]; then
      install -d "$APPLICATIONS"
      install -m 0644 "$src_desktop" "$APPLICATIONS/lucas.cavalheri.tunnelyard.desktop"
      break
    fi
  done
  for DESKTOP in \
    "$APPLICATIONS/lucas.cavalheri.tunnelyard.desktop" \
    "$APPLICATIONS/tunnel-yard.desktop" \
    "$APPLICATIONS/dev.cavallheri.myvpns.desktop" \
    "$APPLICATIONS/my-vpns.desktop"; do
    if [ -f "$DESKTOP" ]; then
      normalize_desktop_entry "$DESKTOP"
      if ! grep -q '^StartupWMClass=' "$DESKTOP"; then
        printf '\nStartupWMClass=lucas.cavalheri.tunnelyard\n' >> "$DESKTOP"
      fi
    fi
  done
  if [ -f "$APP_DIR/public/icon.png" ] || [ -f "$APP_DIR/icon.png" ]; then
    src256="$APP_DIR/public/icon.png"
    [ -f "$src256" ] || src256="$APP_DIR/icon.png"
    install -d "$ICON_256"
    install -m 0644 "$src256" "$ICON_256/tunnel-yard.png"
    if [ -f "$APP_DIR/public/icon-64.png" ]; then
      install -d "$ICON_64"
      install -m 0644 "$APP_DIR/public/icon-64.png" "$ICON_64/tunnel-yard.png"
    fi
    if [ -f "$APP_DIR/public/icon-32.png" ]; then
      install -d "$ICON_32"
      install -m 0644 "$APP_DIR/public/icon-32.png" "$ICON_32/tunnel-yard.png"
    fi
    if [ -f "$APP_DIR/public/icon.png" ]; then
      install -d "$PIXMAPS"
      install -m 0644 "$APP_DIR/public/icon.png" "$PIXMAPS/tunnel-yard.png"
    fi
  fi
fi

rm -f "$APPLICATIONS/dev.cavallheri.myvpns.desktop" \
  "$APPLICATIONS/my-vpns.desktop" \
  "$POLKIT_ACTIONS/dev.cavallheri.myvpns.policy" \
  "$(root_path /usr/share/icons/hicolor/256x256/apps/my-vpns.png)" \
  "$(root_path /usr/share/icons/hicolor/64x64/apps/my-vpns.png)" \
  "$(root_path /usr/share/icons/hicolor/32x32/apps/my-vpns.png)" \
  "$PIXMAPS/my-vpns.png"

KEYRING_SOURCE=""
for key in \
  "$KEYRINGS/tunnel-yard-archive-keyring.asc" \
  "$PACKAGE_PAYLOAD/tunnel-yard-archive-keyring.asc" \
  "$APP_DIR/packaging/tunnel-yard-archive-keyring.asc" \
  "$APP_DIR/tunnel-yard-archive-keyring.asc" \
  "$KEYRINGS/my-vpns-archive-keyring.asc"; do
  if [ -n "$key" ] && [ -f "$key" ]; then KEYRING_SOURCE="$key"; break; fi
done
KEYRING_DEST="$KEYRINGS/tunnel-yard-archive-keyring.asc"
SOURCE_LIST="$APT_SOURCES/tunnel-yard.list"

if [ -d "$APT_SOURCES" ] && [ -f "$KEYRING_SOURCE" ]; then
  install -d "$KEYRINGS"
  if [ "$KEYRING_SOURCE" != "$KEYRING_DEST" ]; then
    install -m 0644 "$KEYRING_SOURCE" "$KEYRING_DEST"
  fi
  cat > "$SOURCE_LIST" << EOF
deb [arch=amd64,arm64 signed-by=$KEYRING_DEST] https://lucascavalheri.github.io/tunnel-yard/apt ./
EOF
  rm -f "$APT_SOURCES/my-vpns.list"
elif [ -d "$APT_SOURCES" ]; then
  rm -f "$SOURCE_LIST" "$APT_SOURCES/my-vpns.list"
fi

exit 0

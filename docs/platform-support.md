# Platform support

TunnelYard is a Linux desktop. The UI, profile editor and `.conf` files are
shared; execution, privilege elevation, networking and autostart use PolicyKit
and openfortivpn.

| Architecture | VPN engine | Release artifact | Profile directory |
| --- | --- | --- | --- |
| Linux x86_64 | openfortivpn | `.deb`, `.rpm`, `.pkg.tar.zst`, `.apk` or `.tar.gz` | `/etc/openfortivpn` |
| Linux ARM64 | openfortivpn | same set, `arm64` / `aarch64` in the filename | `/etc/openfortivpn` |

The GUI never runs as root. Each connection starts through PolicyKit (`pkexec`)
and a helper under `/usr/lib/tunnel-yard/`. Profiles stay as openfortivpn
`.conf` files.

## Distros

TunnelYard reads `/etc/os-release` (and the package manager on disk) to pick
how to install `openfortivpn`:

| Family | Typical distros | One-click install |
| --- | --- | --- |
| apt | Debian, Ubuntu, Mint, Pop!_OS, Kali, Raspberry Pi OS, elementary, Zorin | `apt-get install -y openfortivpn` |
| dnf / yum | Fedora, RHEL, Rocky, Alma, Amazon Linux, Mageia | `dnf` / `yum install -y openfortivpn` |
| zypper | openSUSE Leap/Tumbleweed, SLES | `zypper --non-interactive install openfortivpn` |
| pacman | Arch, Manjaro, EndeavourOS, CachyOS, Garuda | `pacman -S --noconfirm openfortivpn` |
| apk | Alpine, postmarketOS, Chimera | `apk add openfortivpn` |
| xbps | Void | `xbps-install -y openfortivpn` |
| emerge | Gentoo, Funtoo | `emerge --ask=n net-vpn/openfortivpn` |
| eopkg | Solus | `eopkg install -y openfortivpn` |
| rpm-ostree | Silverblue, Kinoite, Bazzite | shows `rpm-ostree install openfortivpn` |
| nix / guix / slackpkg | NixOS, Guix, Slackware | shows the native command |

`.deb`, `.rpm`, the Arch pacman package and the Alpine apk also install the
desktop launcher, icons, PolicyKit action and VPN helpers. The Alpine apk
depends on `gcompat` because the desktop binary is a glibc build. The portable
`.tar.gz` is the same binary for Gentoo, Void, NixOS and similar.

## Configuration

Files remain **openfortivpn `.conf` files**. Options the form does not expose
are kept in `extraOptions` instead of being dropped.

| Field | Linux behavior |
| --- | --- |
| `host`, `port` | Fortinet HTTPS gateway |
| `username` / `user`, `password` | Passed to openfortivpn via the helper |
| `trusted-cert` | openfortivpn certificate fingerprint |
| `set-dns` / `set-routes` | openfortivpn DNS and routes |
| `realm` | Fortinet login realm |
| `persistent` | TunnelYard retries after the interval; auth failures and cancelled elevation do not |

This is FortiGate **SSL VPN**, not IPsec.

## Process lifecycle

Closing the window keeps the app in the tray; quitting requests a graceful VPN
shutdown. If the GUI crashes, the helper still owns the openfortivpn process
and can be stopped with PolicyKit.

Autostart is an XDG desktop file in `~/.config/autostart/`. Notifications use
`notify-send` (and `gdbus` as a fallback). Importing a `.conf` uses zenity,
qarma, yad or kdialog, whichever is on the session.

## Distribution

The mark is a coral squircle with a white circular tunnel portal. The concept
lives in `public/icon-master.png` and is rebuilt in `scripts/generate-icon.py`.
On Linux the app also writes a per-user launcher named
`lucas.cavalheri.tunnelyard`, matching the Wayland app id.

CI runs `cargo fmt --check` (x64), `cargo test` and `cargo build` on Linux x64
and ARM64. Release tags build those targets and publish `.deb`, `.rpm`,
`.pkg.tar.zst`, `.apk` and `.tar.gz`.

## Verification

`cargo test` covers conf round-trip, distro family detection, reconnect policy,
i18n parity, autostart, tray model, file pickers and an isolated Debian
upgrade/repair. `tunnel-yard --smoke` prints engine, platform, profile
directory, client status and locale from the same binary. It does not install
dependencies or touch system networking.

Linux acceptance checklist (a test FortiGate helps):

- Install the package for the distro and complete dependency setup, including
  cancellation of the PolicyKit prompt.
- Import a copy of a real `.conf`; verify host, port, username, realm and
  retained extra options. Use test credentials; never commit private profiles.
- Connect and reach an internal IP; confirm the app reports connected.
- Test `set-dns` and `set-routes` both on and off.
- Check an invalid password: it must not establish a tunnel or retry forever.
- Test two simultaneous VPNs, then disconnect in both orders.
- Test reconnect after a drop, cancel during startup, quit from the tray.

## Upstream

- [openfortivpn](https://github.com/adrienverge/openfortivpn)

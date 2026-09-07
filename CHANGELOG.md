# Changelog

## 2.5.0

### Multi-architecture release

- Added native Linux x64 and ARM64 release builds.
- Added Debian (`.deb`) and RPM (`.rpm`) packages for Linux x64 and ARM64.
- Debian packages accept both the current `polkitd`/`pkexec` split and older
  `policykit-1` transitional packages.
- Added executable Linux `.tar.gz` archives so downloaded binaries retain
  their execute permission.
- Added a universal macOS binary containing Intel and Apple Silicon slices.
- Added a universal macOS `.dmg` installer.
- The macOS binary and application bundle are ad-hoc signed during packaging.
- Added native Windows x64 and ARM64 GUI builds.
- Made release metadata architecture-aware so the updater can identify the
  matching platform artifact.
- Made the APT repository infer and publish `amd64` and `arm64` package
  indexes instead of hard-coding `amd64`.
- Windows ARM64 now fails closed when the pinned x64 OpenConnect installer is
  selected; a native OpenConnect + Wintun ARM64 package is still required for
  VPN connections on that target.

## 2.0.0

My VPNs 2.0 introduces a new native desktop experience built on GPUI Kit while
keeping the existing VPN engine, profiles and privilege boundaries intact.

### Highlights

- Rebuilt the desktop shell with GPUI Kit 0.6: native title bar, buttons,
  inputs, switches, icons, scroll containers and theme integration
- Refined the window around a compact operations rail, searchable profile
  cards, fixed-height live console and a scrolling profile editor
- Preserved multi-tunnel control, tray behavior, pt-BR/EN, setup bootstrap,
  notifications and update checks across the UI migration

### Breaking changes

- Raised the minimum supported Rust version to 1.90
- Removed the legacy `--screenshot` CLI option because the GPUI renderer does
  not expose an equivalent live-window capture API

## 1.2.0

First native Rust desktop host. Electron, Node and the Chromium shell are gone.
Profiles stay as openfortivpn `.conf` files; the GUI process still never runs as
root or Administrator.

### What changed

- Native `eframe`/`egui` desk: profile list, editor, live console, setup gate,
  light/dark theme, pt-BR + EN
- Close the window to park in the tray; **Quit** in the sidebar or tray menu
  disconnects then exits
- Linux/macOS continue to use **openfortivpn**; Windows continues to use
  **OpenConnect 9.21 + Wintun**
- Privileged work stays in `packaging/` helpers (PolicyKit, macOS administrator
  prompt, Windows UAC)
- In-app update check opens the GitHub release page; it does not download or
  run installers

### Downloads

| File | Platform |
|------|----------|
| `my-vpns-linux-x64` | Linux |
| `my-vpns-macos` | macOS |
| `my-vpns-windows-x64.exe` | Windows x64 |

```bash
chmod +x my-vpns-linux-x64
./my-vpns-linux-x64
```

On first launch the app can install the platform VPN client. Unsigned binaries
may trigger SmartScreen / Gatekeeper.

### Upgrading from 1.1.x (Electron)

This is a new desktop host, not an installer drop-in. **Do not use the in-app
updater in 1.1.x** to apply this release — those builds look for `.deb` /
`.rpm` / `.dmg` / NSIS `.exe` packages, which this tag does not ship.

1. Download the binary for your OS from this page
2. Quit the old Electron app
3. Run the new binary
4. Existing `.conf` profiles continue to work:
   - Linux: `/etc/openfortivpn`
   - macOS: `~/Library/Application Support/My VPNs/profiles`
   - Windows: `%APPDATA%\My VPNs\profiles`

Distro packages (`.deb` / `.rpm` / `.dmg`) for the Rust host are not produced
yet. Older **1.1.x** tags still have the Electron-era installers.

### Validation

Automated checks cover configuration translation, certificate pins, native
status policy and i18n. Windows MTU, split-DNS and service probes were checked
on a real FortiGate session in 1.1.x; other gateways and macOS still need
native acceptance testing. Windows currently supports IPv4 SSL VPN. SAML and
all MFA variants are not verified.

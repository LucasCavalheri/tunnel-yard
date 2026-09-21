# Changelog

## Unreleased

## 3.0.3

- Centralized language and appearance controls in Preferences, with Brazil and
  United States flags for the locale choices.
- CI now aggregates quality, platform, site and security checks behind one
  required `CI` status check, and releases publish checksums with provenance.
- Rust, npm and GitHub Actions dependencies were updated and validated by the
  complete CI matrix.

## 3.0.2

- PT / EN switch in the title bar and the sidebar, plus the existing
  Preferences row.
- First launch uses Portuguese only in Brazil and Portugal. Anywhere else
  starts in English.
- Public installer at `/install.sh`.

## 3.0.1

- Arch and Manjaro download a pacman package (`.pkg.tar.zst`). Alpine
  downloads an `.apk` (needs `gcompat` for the glibc desktop binary).
- The download board asks for the processor first, in two large Intel/AMD
  and ARM64 cards, then the distro.

## 3.0.0

TunnelYard 3 is a Linux desktop for FortiGate SSL VPN. The desk, packaging,
website and CI follow the Linux way: openfortivpn, PolicyKit,
`/etc/openfortivpn`, and the distro's own package manager.

### Runtime

- Profiles, autostart, notifications, file picking and in-app updates use the
  Linux desktop.
- Connecting goes through PolicyKit helpers and openfortivpn.

### Distro coverage

- Auto-install of `openfortivpn` covers apt, dnf, yum, zypper, pacman, apk,
  xbps, emerge and eopkg.
- Immutable ostree images, NixOS, Guix and Slackware show the native command
  instead of a host-mutating pkexec.
- Releases ship `.deb`, `.rpm` and a portable `.tar.gz`.

### Website and docs

- Downloads and notes cover Linux distros.

## 2.9.0

TunnelYard 2.9.0 is a product release: the desk finally has a real appearance
system, auto-reconnect that survives a restart, a new portal mark, and a
website that does not throw you back to the top when you switch language.

### Auto-reconnect

- The Preferences toggle is saved in `settings.json` and restored on launch.
  New installs default to **on**.
- An unexpected drop starts a new tunnel without going through the user
  disconnect path. On Linux that means no pkexec prompt and no 2.5s wait on a
  process that is already gone.
- Per-profile `persistent` is honoured on Linux. Authentication failures,
  rejected cookies, cancelled elevation (126/127) and a server that forbids
  reconnect-after-drop still disable automatic retry. A manual disconnect never
  retries.

### Appearance

- Light, dark and system themes actually apply and persist. The title bar has
  a one-click toggle; Preferences has a three-way picker.
- Preferences is a grouped settings sheet (appearance, language, connections,
  maintenance) instead of a thin strip of buttons.
- Desk chrome uses Hugeicons throughout: tunnels, search, console, profile
  actions, setup and confirmations.
- Removed the stray unclickable "Cancelar" chip caused by a tooltip on the
  editor close button leaking into the title bar.

### App mark

- Replaced the two-arch glyph that read as an "m" with a circular tunnel
  portal. `public/icon-master.png` is the generated source;
  `scripts/generate-icon.py` rebuilds PNG, ICO and SVG with true alpha for the
  dock, tray, installers and website.

### Website

- The landing page uses the portal mark in the header, app preview, tray mock,
  CTA, favicons and `og:image`.
- Switching English / Portuguese fades in place and keeps the current scroll
  position.
- Header and CTA invite a GitHub star, with the live count from the API.
- Download package pickers use a styled list instead of the native OS
  dropdown.
- Landing CSS is inlined in the HTML. A hashed `/_astro/*.css` 404 — cached
  by Vercel as immutable — can no longer leave the page as unstyled markup.

## 2.8.1

### Reliable Linux upgrades

- Fixed Debian package upgrades removing the installed executable, desktop
  launcher, helpers, icons and APT metadata.
- Added a recovery payload so installations affected by the 2.8.0 upgrade can
  be repaired by the next package upgrade.
- Added an isolated package lifecycle regression test covering upgrade,
  destructive legacy cleanup and post-install recovery.

## 2.8.0

### Desktop experience redesign

- Rebuilt the connection workspace around a clearer operational hierarchy,
  with live profile, connected and connecting summaries in the first view.
- Introduced a higher-contrast graphite visual system while preserving the
  TunnelYard orange brand, with more readable typography and consistent
  spacing, surfaces, borders and status colors.
- Redesigned VPN profile cards so connection state, gateway, user and live
  details are easier to scan, and made profile editing an explicit action.
- Made the live console collapsible, added a clear action and increased its
  useful log capacity without taking space from the profile list by default.
- Improved empty and filtered states with clearer guidance and a direct way to
  clear searches that return no results.
- Reorganized preferences with descriptive connection settings and refined the
  profile editor, setup, error and confirmation surfaces for legibility.
- Removed the whole-card hover effect from connection rows, eliminating visual
  jitter and keeping hover feedback on the controls that are actually clickable.
- Updated the complete desktop experience in both English and Brazilian
  Portuguese.

## 2.7.0

### Rebrand to TunnelYard

- Renamed the product to **TunnelYard**. The GitHub repository is
  `LucasCavalheri/tunnel-yard`, the desktop id is `lucas.cavalheri.tunnelyard`,
  and packages/binaries use the `tunnel-yard` slug.
- New profile metadata uses `# tunnel-yard-*` markers. Existing profiles and
  installations migrate compatibly, including macOS and Windows directories.
- Rebranded the Astro website and added architecture/package selection with
  direct downloads resolved from the published release assets.
- Documented exploratory Linux RAM measurements and their methodology.

### In-app updates

- **Download and install** applies the matching GitHub release artifact on
  Linux (deb/rpm or portable binary), macOS (`.app` from the disk image or
  portable binary) and Windows (portable or Program Files, with UAC when needed).
- The Linux tray now refreshes per-profile status when tunnels change, and no
  longer draws the coral mark as a red overlay badge on GNOME.

## 2.6.0

### Product interface refresh

- Reworked the native desktop surface to match the website hero: compact
  operations rail, centered brand bar, dark workspace, profile cards and live
  console.
- Moved secondary controls into Preferences so the main tunnel view stays
  focused on connecting and monitoring environments.
- Added a visual connection journey and a multi-tunnel network animation to
  the landing page, with meaningful motion and reduced-motion support.
- Fixed the Linux development build so `cargo run` no longer requires a
  distro-specific Fontconfig development package.

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

TunnelYard 2.0 introduces a new native desktop experience built on GPUI Kit while
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
| `tunnel-yard-linux-x64` | Linux |
| `tunnel-yard-macos` | macOS |
| `tunnel-yard-windows-x64.exe` | Windows x64 |

```bash
chmod +x tunnel-yard-linux-x64
./tunnel-yard-linux-x64
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
   - macOS: `~/Library/Application Support/TunnelYard/profiles`
   - Windows: `%APPDATA%\TunnelYard\profiles`

Distro packages (`.deb` / `.rpm` / `.dmg`) for the Rust host are not produced
yet. Older **1.1.x** tags still have the Electron-era installers.

### Validation

Automated checks cover configuration translation, certificate pins, native
status policy and i18n. Windows MTU, split-DNS and service probes were checked
on a real FortiGate session in 1.1.x; other gateways and macOS still need
native acceptance testing. Windows currently supports IPv4 SSL VPN. SAML and
all MFA variants are not verified.

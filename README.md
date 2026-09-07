# 🛡️ My VPNs

**A desktop app for managing FortiGate SSL VPN connections on Linux, macOS and Windows.**

Linux and macOS use **openfortivpn**. Windows uses **OpenConnect 9.21 + Wintun**, translating the existing openfortivpn `.conf` format. Windows includes dynamic MTU configuration, tunnel health checks and Fortinet split-DNS, tested against a real gateway. macOS and other gateway/authentication policies still need native acceptance testing. See [platform support and validation](docs/platform-support.md).

No more babysitting a terminal with `sudo openfortivpn`. Connect one or many tunnels, park the app in the tray, get notified when a link drops, and manage profiles without editing files by hand.

[![License: MIT](https://img.shields.io/badge/License-MIT-teal.svg)](./LICENSE)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-blue.svg)](#-installation)
[![Packages](https://img.shields.io/badge/packages-GitHub%20binaries%20%7C%20deb%20%7C%20rpm%20%7C%20dmg-orange.svg)](#-installation)
[![Built with](https://img.shields.io/badge/built%20with-Rust-informational.svg)](#-tech-stack)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](#-contributing)

---

## ✨ Features

| Feature | What it does |
|--------|----------------|
| 🔌 Multi-link connect | Bring up several VPNs at the same time |
| 📝 Profile editor | Create / edit / delete `.conf` files in the platform's profile directory |
| 📥 Import `.conf` | Pick an existing openfortivpn config and save it |
| 🧺 System tray | Close the window — tunnels keep running |
| 🔔 Notifications | Know immediately when a tunnel goes up or dies |
| ♻️ Auto-reconnect | Optional recovery after unexpected disconnects |
| 🚀 Start at login | XDG autostart on Linux; native login items on macOS / Windows |
| 🌐 pt-BR + EN | Full UI language switch, persisted |
| 📦 Dependency bootstrap | Detect and install the platform's VPN client |
| 📜 Live console | Stream VPN client output while connected |
| 🔐 Administrator authorization | PolicyKit on Linux, native administrator prompt on macOS, UAC on Windows; UI remains unprivileged |

---

## 🚀 Installation

### From GitHub Releases (recommended)

Tagged releases attach native binaries and installable packages built by CI:

| File | Platform |
|------|----------|
| `my-vpns-linux-x64` | Linux x86_64 |
| `my-vpns-linux-arm64` | Linux ARM64 |
| `my-vpns-linux-x64.tar.gz` | Linux x86_64 archive (executable) |
| `my-vpns-linux-arm64.tar.gz` | Linux ARM64 archive (executable) |
| `my-vpns_2.5.0_amd64.deb` | Debian/Ubuntu x86_64 |
| `my-vpns_2.5.0_arm64.deb` | Debian/Ubuntu ARM64 |
| `my-vpns-2.5.0-1.x86_64.rpm` | Fedora/RHEL x86_64 |
| `my-vpns-2.5.0-1.aarch64.rpm` | Fedora/RHEL ARM64 |
| `my-vpns-macos` | macOS universal (Intel + Apple Silicon) |
| `my-vpns-macos.dmg` | macOS installer (Intel + Apple Silicon) |
| `my-vpns-windows-x64.exe` | Windows x64 |
| `my-vpns-windows-arm64.exe` | Windows ARM64 (GUI) |

```bash
chmod +x my-vpns-linux-x64
./my-vpns-linux-x64
```

On first launch the app can install the platform VPN client (`openfortivpn` on Linux/macOS; official OpenConnect 9.21 + Wintun on Windows x64). Privileged helpers live in `packaging/`.

On Debian/Ubuntu, install the matching package with `sudo apt install ./my-vpns_<version>_<arch>.deb`. On Fedora/RHEL, use `sudo dnf install ./my-vpns-<version>-1.<arch>.rpm`. The packages install the desktop launcher, icons, PolicyKit action and VPN helpers.

If you prefer the standalone Linux binary, download the matching `.tar.gz`; it preserves the executable permission when extracted:

```bash
tar -xzf my-vpns-linux-x64.tar.gz
./my-vpns-linux-x64
```

On Linux, the app creates the per-user launcher
`~/.local/share/applications/dev.cavallheri.myvpns.desktop`. Its filename
matches the Wayland app id, so GNOME associates the window with the branded
dock icon instead of showing a generic gear. Packagers can use
`packaging/my-vpns.desktop` as the system-wide source.

Use **1.1.3 or newer on Windows** for the Wintun MTU, tunnel-state, service-check, and HTTPS-only DTLS fixes. Unsigned builds may trigger SmartScreen / Gatekeeper.

Older **1.1.x** tags still have Electron-era installers. The Rust host packages in this release are native packages and are not drop-in upgrades for that Electron build. The macOS `.dmg` is ad-hoc signed but not Apple-notarized, so Gatekeeper may still require opening it from Finder with secondary confirmation.

#### macOS

Install [Homebrew](https://brew.sh) if needed, then `brew install openfortivpn` (or use the app's install button when Homebrew already exists). Connecting requests macOS administrator authorization. Profiles live in `~/Library/Application Support/My VPNs/profiles`.

#### Windows

On x64, **Install now** downloads the pinned official OpenConnect 9.21 installer, checks its SHA256, and requests UAC authorization. Wintun is included; WSL and FortiClient are not required. The ARM64 GUI is available, but the pinned OpenConnect installer is x64-only; ARM64 needs a native OpenConnect + Wintun package installed separately before connecting. Profiles live in `%APPDATA%\My VPNs\profiles`.

#### Architecture compatibility

The 2.5.0 release publishes native Linux x64/ARM64 binaries, a universal macOS binary containing Intel and Apple Silicon slices, and native Windows x64/ARM64 GUI binaries. The release checker identifies the platform and architecture so the correct download is easy to select. Runtime VPN compatibility still depends on the native client and the gateway's authentication policy; SAML/browser login, every MFA variant, IPv6 tunnels and arbitrary engine options remain outside the verified matrix.

#### Linux

Profiles live in `/etc/openfortivpn`. Connecting uses PolicyKit (`pkexec`). From source:

```bash
cargo build --release
./target/release/my-vpns
```

### Atualizações dentro do app

Quando uma release nova aparece no GitHub, o banner e o item da bandeja
**Verificar atualizações** apontam para as notas da release. Baixe o binário
da sua plataforma (ou rode `cargo build --release`).

### How to publish a release

**Automatic (preferred):** bump `version` in `Cargo.toml`, tag, and push. CI runs `cargo test` / `cargo build --release`, creates the native packages and attaches all artifacts.

```bash
# 1) bump version in Cargo.toml and add a CHANGELOG.md section (e.g. 1.2.1)
# 2) commit, tag, push
git tag v1.2.1
git push origin master --follow-tags
```

That triggers [`.github/workflows/release.yml`](.github/workflows/release.yml) on tags like `v1.2.1`. The workflow copies the matching `CHANGELOG.md` section into the GitHub release notes.

> GitHub always offers **Source code** downloads on the release page for the tagged commit — you don’t upload those yourself.

The release workflow signs the APT repository with the archive key committed at
[`packaging/my-vpns-archive-keyring.asc`](./packaging/my-vpns-archive-keyring.asc).
The matching private key must be configured once as the GitHub Actions secret
`APT_SIGNING_KEY`; it must never be committed to the repository.
The key fingerprint is `A9F137BEE74B623131071358FB0EC1D5A01262F0`.

### Linux requirements

| Requirement | Notes |
|-------------|--------|
| 🐧 Linux desktop | GNOME, KDE, and friends |
| 🔑 PolicyKit (`pkexec`) | Used for privileged VPN start/stop and profile writes |
| 📁 `/etc/openfortivpn/` | Where profiles live (create, import, or drop files manually) |
| 🛰️ `openfortivpn` | Optional at install time — the app can install it for you |
| 🖥️ GPUI native libraries | Required only when compiling from source; see the [GPUI Kit notes](docs/gpui-kit.md) |

> **Tip:** On first launch, if `openfortivpn` is not on `PATH`, My VPNs reads `/etc/os-release`, picks the right package manager (`apt`, `dnf`/`yum`, `zypper`, or `pacman`), and offers a one-click install via PolicyKit.

---

## 🧰 VPN profiles

Profiles are standard openfortivpn configs:

```text
/etc/openfortivpn/
  ├── work.conf
  ├── client.conf
  └── lab.conf
```

### Create / import in the app

Use **New profile** or **Import .conf**. The form covers the options used in real FortiGate SSL setups:

| Field | Conf key |
|-------|----------|
| Host / Port | `host`, `port` |
| Username / Password | `username`, `password` |
| Trusted cert | `trusted-cert` |
| DNS / routes | `set-dns`, `set-routes` |
| Realm | `realm` (optional) |
| Persistent | `persistent` (seconds, `0` = off) |

Example file:

```ini
host = vpn.example.com
port = 10443
username = alice
password = hunter2
trusted-cert = <sha256 fingerprint>
set-dns = 0
set-routes = 1
```

### Privacy notes

- 🔒 Credentials are stored in plaintext `.conf` files in the platform's profile directory (same model as CLI openfortivpn); native directories have restricted access
- 👁️ The main list shows host/port/username — not the password
- 🧾 Harden directory permissions on shared machines (`chmod` / root-only reads as needed)
- 🚫 Never commit personal `.conf` files or passwords to git

---

## 🎮 Usage

1. Open **My VPNs**
2. Create, import, or pick an existing profile
3. Click **Bring up** / **Conectar** and approve your system's administrator prompt
4. Optionally enable **Auto-relink**, **Start at login**, and switch **PT / EN**
5. Close the window or **Hide to tray** — tunnels keep running
6. Fully quit from **Quit** in the sidebar or the tray menu

Keyboard: `Ctrl+N` new profile, `Ctrl+W` hide to tray, `Ctrl+Q` quit, `Esc` close dialogs.

### Tray menu

- Show window
- Per-profile connect / disconnect
- Disconnect all
- Refresh profiles
- Quit

---

## 🛠️ Development

### Prerequisites

- Rust **1.90+** (`rustup`)
- A Linux desktop, Windows x64, or macOS host
- Linux: install the native GPUI dependencies listed in [`docs/gpui-kit.md`](docs/gpui-kit.md) (the same binary also supports `--smoke` without a GUI)

### Setup

```bash
git clone https://github.com/LucasCavalheri/my-vpns.git
cd my-vpns
cargo run
```

### Useful commands

| Command | Description |
|---------|-------------|
| `cargo run` | Desktop UI |
| `cargo run -- --hidden` | Start hidden (tray / login item) |
| `cargo run -- --smoke` | Print engine/settings/profile probe JSON and exit |
| `cargo test` | Domain tests (conf, markers, OpenConnect translation, i18n, …) |
| `cargo build --release` | Optimized `my-vpns` binary |

The product is the `my-vpns` binary (`cargo run` / `cargo build --release`).

The native interface is built with [GPUI Kit](https://gpui-kit.com/): its
window shell, buttons, inputs, switches, icons, scroll containers and theme
system underpin the profile desk. See [`docs/gpui-kit.md`](docs/gpui-kit.md)
for the component mapping and Linux build dependencies.

---

## 🧪 Tests

```bash
cargo test
./target/debug/my-vpns --smoke
```

Coverage includes:

- `/etc/os-release` parsing + package-family detection
- openfortivpn install plan per distro
- VPN `.conf` parse / serialize (including profile drafts)
- Log markers for tunnel up / errors
- OpenConnect argument translation (password never on argv)
- trusted-cert SHA256 leaf + SPKI pin
- native supervisor status rejection
- i18n catalog key parity (`en` ↔ `pt-BR`)
- XDG autostart `.desktop` snippet

---

## 🏗️ Project structure

```text
my-vpns/
├── src/                # Rust library + desktop binary
│   ├── main.rs         # Entry: UI or --smoke
│   ├── ui.rs           # GPUI Kit window, tray, notifications
│   ├── vpn.rs          # Multi-session VPN manager
│   ├── conf.rs         # Create / import / save / delete .conf
│   ├── openconnect.rs  # Windows .conf → OpenConnect plan
│   ├── deps.rs         # Distro detect + client install
│   └── i18n.rs         # en + pt-BR
├── packaging/          # Elevated helpers + PolicyKit
├── tests/              # cargo tests + TLS fixtures
├── REFACTOR.md         # Map + resume checkpoint
└── Cargo.toml
```

### Under the hood (Linux)

1. Profiles are discovered from `/etc/openfortivpn/*.conf`
2. Connect runs through PolicyKit helpers (`/usr/lib/my-vpns/` after package install)
3. Multiple tunnels are tracked as independent sessions
4. Profile writes use `pkexec install` into `/etc/openfortivpn`
5. Status changes drive tray + desktop notifications

Installed helper paths:

| Path | Role |
|------|------|
| `/usr/lib/my-vpns/run-vpn.sh` | Starts `openfortivpn` and tracks PID |
| `/usr/lib/my-vpns/stop-vpn.sh` | Stops the tunnel gracefully |
| `/usr/share/polkit-1/actions/dev.cavallheri.myvpns.policy` | PolicyKit action (`auth_admin_keep`) |

---

## 🧱 Tech stack

- 🦀 **Rust + GPUI Kit** — unprivileged native desktop host
- 🧪 **cargo test** — domain tests that call the shipped functions
- 🔐 **Native authorization** — PolicyKit, macOS administrator prompt, or Windows UAC
- 🛠️ **packaging/** — elevated `openfortivpn` / OpenConnect helpers (not the GUI)

---

## 🤝 Contributing

Contributions are very welcome — bug fixes, UI polish, distro support, docs, packaging, tests, translations.

### Quick start

1. 🍴 Fork the repo
2. 🌿 Create a branch: `git checkout -b feat/my-idea`
3. 🧪 Smoke-test with `cargo run -- --smoke` (and `cargo run` if you have a display)
4. ✅ Ensure `cargo test` passes
5. 📨 Open a Pull Request with a clear *why*

### Good first issues

- Stronger connection health checks
- Flatpak / AppImage experiments
- Extra openfortivpn options in the profile form

### Code style

- Keep changes focused — small, reviewable PRs
- Match existing Rust patterns in `src/`
- Don’t commit secrets, VPN passwords, or personal `.conf` files
- Don’t add drive-by refactors unrelated to your PR

### Reporting bugs

Please include:

- Distro + desktop environment (`cat /etc/os-release`)
- App version / commit
- Whether `openfortivpn` is installed (`openfortivpn --version`)
- Steps to reproduce + relevant console output (**redact credentials**)

---

## 🗺️ Roadmap

- [x] pt-BR + EN i18n
- [x] Create / import / edit profiles
- [x] Multi-VPN concurrent sessions
- [x] Start with Linux (autostart)
- [x] GitHub Actions release pipeline (tag → binaries, `.deb`, `.rpm`, `.dmg`)
- [ ] Stronger health checks / richer log parsing
- [ ] Flatpak / AppImage experiments

Have a better idea? Open an issue or PR 💬

---

## ⚠️ Security

- Elevated privileges are required to create VPN tunnels and write under `/etc/openfortivpn` — expected
- Elevation goes through PolicyKit on Linux, the macOS administrator prompt, or Windows UAC
- Review `/etc/openfortivpn/*.conf` permissions on shared machines
- Never paste passwords into issues or PRs

If you find a security issue, please report it privately when possible instead of opening a public issue with exploit details.

---

## 📄 License

Released under the [MIT License](./LICENSE).

---

## 💜 Acknowledgements

- [openfortivpn](https://github.com/adrienverge/openfortivpn) — the VPN engine
- Everyone who is tired of leaving a root terminal open forever

---

<p align="center">
  <strong>For people who just want the tunnel up. 🛡️✨</strong><br/>
  <sub>Star the repo if it helps — it keeps the project visible for new contributors.</sub>
</p>

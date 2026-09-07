# My VPNs — current map and resume log

Rust desktop host for FortiGate SSL VPN. Unprivileged GUI; privileged work stays in `packaging/` helpers.

---

## Resume checkpoint

- **Last finished:** Rust `my-vpns` binary with restyled egui UI (sidebar, cards, toggles, light/dark). Window close hides to tray (deferred minimize after CancelClose); Quit is explicit. `--screenshot`. 38 `tests/shipped.rs` cases.
- **Next:** Native OS runtime on matching runners (Windows Wintun/NRPT/MTU, macOS scutil). Optional distro packages.
- **Remaining:** See **Native OS runtime still remaining**.

---

## Product

**My VPNs** (`dev.cavallheri.myvpns`, version from `Cargo.toml`): several FortiGate SSL tunnels at once.

| OS | Engine | Profiles | Elevation |
| --- | --- | --- | --- |
| Linux | openfortivpn | `/etc/openfortivpn` | `pkexec` + `packaging/run-vpn.sh` / `stop-vpn.sh` |
| macOS | openfortivpn (Homebrew) | `~/Library/Application Support/My VPNs/profiles` | `osascript` + `macos-vpn.sh` |
| Windows | OpenConnect 9.21 + Wintun | `%APPDATA%\My VPNs\profiles` | UAC + `windows-vpn.ps1` |

The GUI process never runs as root/Administrator. Password never appears on argv. `.conf` files stay openfortivpn syntax (`# my-vpns-*` comments for Windows metadata).

Entry: `cargo run` / `target/release/my-vpns`. Flags: `--hidden`, `--autostart`, `--smoke`, `--screenshot PATH`, `--version`, `--help`.

---

## Crate layout

| File | Role |
| --- | --- |
| `src/main.rs` | CLI: UI or `--smoke` |
| `src/ui.rs` | egui desk: setup gate, list, editor, console, tray, updates, locale |
| `src/theme.rs` | Palette, fonts, cards |
| `src/app_icon.rs` | Embedded PNG/ICO, cache path, ARGB pixmap for the tray |
| `src/icons.rs` | Rust/UI's official `icons` SVG registry, cached as egui textures |
| `src/lib.rs` | Domain library used by the binary and `tests/shipped.rs` |
| `src/conf.rs` | Parse/serialize `.conf`, slugify ids, save/delete/import |
| `src/vpn.rs` | Multi-session manager, log markers, reconnect, native close decision |
| `src/native.rs` | Elevated session, status.json policy, quoting |
| `src/openconnect.rs` | `.conf` → OpenConnect plan, TLS pin |
| `src/platform.rs` | Engine, profile dirs, helpers |
| `src/deps.rs` | os-release, install plan, client status |
| `src/install_native.rs` | brew / OpenConnect 9.21 download+SHA256+UAC `/S` |
| `src/autostart.rs` | XDG, LaunchAgent, HKCU Run |
| `src/os_ui.rs` | Notifications, file picker |
| `src/tray_menu.rs` | Tray model shared by ksni and tray-icon |
| `src/i18n.rs` | en + pt-BR |
| `src/settings.rs` | locale, theme, dismissed update |
| `src/updates.rs` | GitHub latest release check |
| `src/smoke.rs` | Headless probe JSON |
| `src/desktop.rs` | UI surface contract for tests |

---

## UI surfaces

Window 1080×720 (min 900×600). Close hides to tray (second close within 80–900ms quits); Quit in the sidebar/tray disconnects then exits.

- Setup gate when the VPN client is missing
- Profile list / empty state; create, import, edit, delete
- Editor: id, host, port, username, password, realm, trusted-cert, health host/port, no-dtls, legacy-tunnel, persistent, set-dns, set-routes, extra-options notice
- Live console
- Auto-reconnect, start at login, disconnect all
- Locale PT/EN, theme system/light/dark, park in tray
- Update banner (open release URL / dismiss); footer check
- Tray: show (also activate/click), per-profile toggle, disconnect all, refresh, check updates, quit
- Connect/disconnect notifications

---

## Tests

`tests/shipped.rs` calls shipped functions: conf round-trip, markers, OpenConnect args without password, pins, native status, close-path reconnect, os-release, i18n parity, autostart builders, Windows bootstrap pipeline, tray model, notify/picker scripts, GitHub update JSON.

```bash
cargo test
./target/debug/my-vpns --smoke
```

---

## Native OS runtime still remaining

Cannot run here (Linux, no FortiGate):

- Windows Wintun / NRPT / negotiated MTU / bound service probe
- macOS administrator dialog + scutil DNS
- Live FortiGate auth / SAML / MFA

Logic tests + Linux launch cover the rest.

---

## How to resume

1. Read this checkpoint.
2. Product entry is `my-vpns` (`cargo run`).
3. Run `cargo test` after domain changes.

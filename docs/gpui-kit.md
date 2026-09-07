# GPUI Kit desktop interface

My VPNs uses [GPUI Kit](https://gpui-kit.com/) for its native desktop shell.
The VPN engine, profile storage, privilege boundaries and tray integration stay
in the existing Rust domain modules; `src/ui.rs` owns presentation and user
interaction.

## Component mapping

| Product surface | GPUI Kit primitive |
| --- | --- |
| Window chrome | `Root` + `TitleBar` |
| Primary and secondary actions | `Button` variants |
| Search and profile fields | `Input` / `InputState` |
| Settings and VPN options | `Switch` |
| Profile form and confirmations | layered modal surfaces |
| Profile list, editor and console | GPUI flex layout + scroll containers |
| Light, dark and system appearance | `Theme` / `ThemeMode` |

The app keeps a small brand layer on top of the kit: orange primary actions,
compact profile cards, a persistent operations rail and an internal scrolling
profile editor. Both pt-BR and English product copy continue to come from the
app's own i18n catalog.

## Building on Linux

GPUI Kit 0.6 requires Rust 1.90 or newer. On Debian/Ubuntu, install the native
build dependencies before running Cargo:

```bash
sudo apt update
sudo apt install gcc g++ clang libfontconfig-dev libwayland-dev \
  libwebkit2gtk-4.1-dev libxkbcommon-x11-dev libx11-xcb-dev \
  libssl-dev libzstd-dev vulkan-validationlayers libvulkan1
cargo run
```

See GPUI Kit's [installation guide](https://gpui-kit.com/docs/installation)
for other Linux distributions and current platform notes.

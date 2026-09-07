# Rust/UI adaptation

Rust/UI is a copy-and-paste component registry for Leptos + Tailwind, not a
native widget crate. The `ui-cli` tool can be installed with:

```bash
cargo install ui-cli --force
```

Running `ui view button` in this repository currently reports
`No supported framework (leptos/dioxus) found in Cargo.toml`, as expected: My
VPNs uses `eframe`/`egui` so it can remain a small native desktop application.
Running `ui init` here would scaffold a second web UI instead of styling the
existing app.

The compatible parts of the Rust/UI visual language are implemented natively:

| Rust/UI primitive | My VPNs implementation |
| --- | --- |
| Card / surfaces | `theme::card`, `theme::live_card`, `theme::console_frame` |
| Button variants | `icons::action_button`, `accent_button`, `ghost_button` |
| Switch | `toggle_row` |
| Badge / status | `theme::pill`, `status_chip`, `chip` |
| Input | themed `egui::TextEdit` fields and search surface |
| Dialog | themed centered `egui::Window` editors and confirmations |
| Icons | official Rust/UI `icons` crate, rendered from its SVG registry |

The result keeps Rust/UI's quiet surfaces, rounded cards, compact controls,
subtle borders, strong state colors and Lucide icon vocabulary without pulling
Leptos, a browser renderer and Tailwind into the native process.

References:

- <https://rust-ui.com/docs/components/installation>
- <https://rust-ui.com/docs/components/icons>
- <https://github.com/rust-ui/ui>

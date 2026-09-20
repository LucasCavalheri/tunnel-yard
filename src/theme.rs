//! Light and dark palettes copied onto GPUI Kit's Theme.
//!
//! Preference values are `light`, `dark` or `system`. The desk resolves
//! `system` against the window appearance; tests cover that mapping and that
//! the two palettes actually diverge.

use crate::settings::normalize_theme;

pub const BRAND: u32 = 0xff6b35;
pub const BRAND_HOVER: u32 = 0xff7c4d;
pub const BRAND_ACTIVE: u32 = 0xe95522;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub workspace_bg: u32,
    pub card_bg: u32,
    pub card_hover: u32,
    pub card_active: u32,
    pub sidebar_bg: u32,
    pub sidebar_raised: u32,
    pub sidebar_border: u32,
    pub sidebar_text: u32,
    pub sidebar_muted: u32,
    pub console_bg: u32,
    pub title_bar: u32,
    pub foreground: u32,
    pub muted: u32,
    pub input: u32,
    pub button: u32,
    pub button_hover: u32,
    pub button_active: u32,
    pub button_foreground: u32,
    pub success: u32,
    pub warning: u32,
    pub danger: u32,
    pub overlay: u32,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            workspace_bg: 0x0c1017,
            card_bg: 0x141b24,
            card_hover: 0x1a232e,
            card_active: 0x202a36,
            sidebar_bg: 0x080b10,
            sidebar_raised: 0x10161e,
            sidebar_border: 0x24303c,
            sidebar_text: 0xf3f6f8,
            sidebar_muted: 0x8b98a4,
            console_bg: 0x070a0e,
            title_bar: 0x0a0e14,
            foreground: 0xf3f6f8,
            muted: 0x1c2530,
            input: 0x24303c,
            button: 0x18212a,
            button_hover: 0x212c37,
            button_active: 0x293642,
            button_foreground: 0xe8edef,
            success: 0x2ecb8a,
            warning: 0xf0ad4e,
            danger: 0xef6a64,
            overlay: 0x05080ee6,
        }
    }

    pub fn light() -> Self {
        Self {
            workspace_bg: 0xf4f0ea,
            card_bg: 0xffffff,
            card_hover: 0xfaf7f2,
            card_active: 0xf3eee6,
            sidebar_bg: 0xebe5dc,
            sidebar_raised: 0xffffff,
            sidebar_border: 0xddd4c8,
            sidebar_text: 0x1c1917,
            sidebar_muted: 0x78716c,
            console_bg: 0xfaf7f2,
            title_bar: 0xf4f0ea,
            foreground: 0x1c1917,
            muted: 0xe8e1d6,
            input: 0xe4dcd0,
            button: 0xffffff,
            button_hover: 0xf4efe8,
            button_active: 0xeae3da,
            button_foreground: 0x1c1917,
            success: 0x0d9f6e,
            warning: 0xd97706,
            danger: 0xe11d48,
            overlay: 0x1c191799,
        }
    }

    pub fn named(mode: &str) -> Self {
        if mode == "light" {
            Self::light()
        } else {
            Self::dark()
        }
    }

    pub fn is_light(&self) -> bool {
        self.workspace_bg > 0x808080
    }
}

/// Resolve a stored preference against the current system appearance.
pub fn resolve_theme_mode(preference: &str, system_is_dark: bool) -> &'static str {
    match normalize_theme(preference) {
        "light" => "light",
        "dark" => "dark",
        _ => {
            if system_is_dark {
                "dark"
            } else {
                "light"
            }
        }
    }
}

/// Cycle light ↔ dark for the title-bar toggle. `system` becomes the opposite
/// of the currently resolved appearance so the click always has a visible effect.
pub fn toggle_light_dark(preference: &str, system_is_dark: bool) -> &'static str {
    match resolve_theme_mode(preference, system_is_dark) {
        "light" => "dark",
        _ => "light",
    }
}

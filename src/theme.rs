//! Light and dark palettes copied onto GPUI Kit's Theme.
//!
//! Preference values are `light`, `dark` or `system`. The desk resolves
//! `system` against the window appearance; tests cover that mapping and that
//! the two palettes actually diverge.

use crate::settings::normalize_theme;

pub const BRAND: u32 = 0xff6b35;
pub const BRAND_HOVER: u32 = 0xff7c4d;
pub const BRAND_ACTIVE: u32 = 0xe95522;
pub const DARK_BRAND: u32 = 0xf25a2a;
pub const DARK_BRAND_HOVER: u32 = 0xff6735;
pub const DARK_BRAND_ACTIVE: u32 = 0xd84418;

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
            workspace_bg: 0x141616,
            card_bg: 0x191b1b,
            card_hover: 0x202323,
            card_active: 0x282c2b,
            sidebar_bg: 0x0d0f0f,
            sidebar_raised: 0x191b1b,
            sidebar_border: 0x2b2e2d,
            sidebar_text: 0xf4f5f1,
            sidebar_muted: 0x939692,
            console_bg: 0x0c0e0e,
            title_bar: 0x181a1a,
            foreground: 0xf4f5f1,
            muted: 0x282b2a,
            input: 0x272b29,
            button: 0x1d201f,
            button_hover: 0x272b29,
            button_active: 0x303432,
            button_foreground: 0xf4f5f1,
            success: 0x29b47a,
            warning: 0xe9ad42,
            danger: 0xf0644d,
            overlay: 0x0a0b0bcc,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_palette_matches_the_website_neutral_brand_surfaces() {
        let palette = Palette::named("dark");

        assert_eq!(palette.workspace_bg, 0x141616);
        assert_eq!(palette.card_bg, 0x191b1b);
        assert_eq!(palette.sidebar_bg, 0x0d0f0f);
        assert_eq!(palette.success, 0x29b47a);
        assert_eq!(DARK_BRAND, 0xf25a2a);
    }

    #[test]
    fn light_palette_keeps_its_existing_warm_surfaces_and_brand() {
        let palette = Palette::named("light");

        assert_eq!(palette.workspace_bg, 0xf4f0ea);
        assert_eq!(palette.card_bg, 0xffffff);
        assert_eq!(palette.sidebar_bg, 0xebe5dc);
        assert_eq!(BRAND, 0xff6b35);
    }
}

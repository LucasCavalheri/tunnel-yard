//! Visual system: Lato + a quiet palette. Not the default egui grey.

use eframe::egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Shadow,
    Stroke, Style, TextStyle, Visuals,
};

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub rail: Color32,
    pub surface: Color32,
    pub surface2: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub live: Color32,
    pub live_soft: Color32,
    pub fault: Color32,
    pub fault_soft: Color32,
    pub hold: Color32,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(0x0b, 0x0c, 0x0f),
            rail: Color32::from_rgb(0x11, 0x13, 0x18),
            surface: Color32::from_rgb(0x16, 0x18, 0x1e),
            surface2: Color32::from_rgb(0x1c, 0x1f, 0x27),
            line: Color32::from_rgb(0x2a, 0x2e, 0x38),
            text: Color32::from_rgb(0xf3, 0xf4, 0xf6),
            muted: Color32::from_rgb(0x8b, 0x91, 0xa0),
            accent: Color32::from_rgb(0xff, 0x6a, 0x2b),
            accent_soft: Color32::from_rgb(0x3a, 0x1c, 0x12),
            live: Color32::from_rgb(0x3d, 0xd6, 0x8c),
            live_soft: Color32::from_rgb(0x12, 0x2a, 0x1e),
            fault: Color32::from_rgb(0xff, 0x6b, 0x6b),
            fault_soft: Color32::from_rgb(0x33, 0x16, 0x16),
            hold: Color32::from_rgb(0xf5, 0xc0, 0x4e),
        }
    }

    pub fn light() -> Self {
        Self {
            bg: Color32::from_rgb(0xf4, 0xf3, 0xf0),
            rail: Color32::from_rgb(0xee, 0xec, 0xe8),
            surface: Color32::from_rgb(0xff, 0xff, 0xff),
            surface2: Color32::from_rgb(0xf7, 0xf6, 0xf3),
            line: Color32::from_rgb(0xe4, 0xe1, 0xdb),
            text: Color32::from_rgb(0x1a, 0x1c, 0x21),
            muted: Color32::from_rgb(0x6b, 0x70, 0x7c),
            accent: Color32::from_rgb(0xe0, 0x4a, 0x12),
            accent_soft: Color32::from_rgb(0xff, 0xee, 0xe6),
            live: Color32::from_rgb(0x0f, 0x8a, 0x4b),
            live_soft: Color32::from_rgb(0xe8, 0xf7, 0xee),
            fault: Color32::from_rgb(0xc4, 0x2b, 0x2b),
            fault_soft: Color32::from_rgb(0xfc, 0xee, 0xee),
            hold: Color32::from_rgb(0xb4, 0x73, 0x09),
        }
    }
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette::dark()
    } else {
        Palette::light()
    }
}

pub fn card(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.line))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::symmetric(18, 16))
}

pub fn live_card(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.live_soft)
        .stroke(Stroke::new(1.0_f32, p.live.gamma_multiply(0.45)))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::symmetric(18, 16))
}

pub fn rail_frame(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.rail)
        .inner_margin(Margin::symmetric(20, 22))
        .stroke(Stroke::NONE)
}

pub fn workspace_frame(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.bg)
        .inner_margin(Margin::symmetric(22, 20))
        .stroke(Stroke::NONE)
}

pub fn console_frame(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0_f32, p.line))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::symmetric(14, 12))
}

pub fn pill(fill: Color32) -> Frame {
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(20))
        .inner_margin(Margin::symmetric(10, 4))
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let regular_paths = [
        "/usr/share/fonts/truetype/lato/Lato-Regular.ttf",
        "/usr/share/fonts/lato/Lato-Regular.ttf",
        "/Library/Fonts/Lato-Regular.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ];
    let bold_paths = [
        "/usr/share/fonts/truetype/lato/Lato-Bold.ttf",
        "/usr/share/fonts/lato/Lato-Bold.ttf",
        "/Library/Fonts/Lato-Bold.ttf",
        "C:\\Windows\\Fonts\\segoeuib.ttf",
    ];
    let mono_paths = [
        "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/System/Library/Fonts/Menlo.ttc",
        "C:\\Windows\\Fonts\\consola.ttf",
    ];
    if let Some(bytes) = read_first(&regular_paths) {
        fonts.font_data.insert(
            "lato".into(),
            std::sync::Arc::new(FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "lato".into());
    }
    let fallback_paths = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    if let Some(bytes) = read_first(&fallback_paths) {
        fonts.font_data.insert(
            "ui-fallback".into(),
            std::sync::Arc::new(FontData::from_owned(bytes)),
        );
        let family = fonts.families.entry(FontFamily::Proportional).or_default();
        let idx = if family.first().map(|s| s == "lato").unwrap_or(false) {
            1
        } else {
            0
        };
        family.insert(idx, "ui-fallback".into());
    }
    if let Some(bytes) = read_first(&bold_paths) {
        fonts.font_data.insert(
            "lato-bold".into(),
            std::sync::Arc::new(FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(FontFamily::Name("lato-bold".into()))
            .or_default()
            .insert(0, "lato-bold".into());
    }
    if let Some(bytes) = read_first(&mono_paths) {
        fonts.font_data.insert(
            "appmono".into(),
            std::sync::Arc::new(FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "appmono".into());
    }
    ctx.set_fonts(fonts);
}

fn read_first(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|p| std::fs::read(p).ok())
}

pub fn apply_style(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut style = (*ctx.style()).clone();
    apply_visuals(&mut style, &p, dark);
    ctx.set_style(style);
}

fn apply_visuals(style: &mut Style, p: &Palette, dark: bool) {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.dark_mode = dark;
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.surface2;
    v.faint_bg_color = p.surface2;
    v.override_text_color = Some(p.text);
    v.window_stroke = Stroke::new(1.0_f32, p.line);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 40 }),
    };
    v.popup_shadow = v.window_shadow;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.text);
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.widgets.noninteractive.bg_fill = p.surface;
    v.widgets.inactive.bg_fill = p.surface2;
    v.widgets.hovered.bg_fill = p.surface2;
    v.widgets.active.bg_fill = p.accent;
    v.widgets.open.bg_fill = p.surface2;
    v.widgets.inactive.weak_bg_fill = p.surface2;
    v.widgets.hovered.weak_bg_fill = p.line;
    v.widgets.active.weak_bg_fill = p.accent;
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.line);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);
    v.widgets.active.bg_stroke = Stroke::NONE;
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.accent);
    v.widgets.inactive.corner_radius = CornerRadius::same(10);
    v.widgets.hovered.corner_radius = CornerRadius::same(10);
    v.widgets.active.corner_radius = CornerRadius::same(10);
    v.widgets.open.corner_radius = CornerRadius::same(10);
    v.selection.bg_fill = p.accent.gamma_multiply(0.85);
    v.selection.stroke = Stroke::new(1.0_f32, p.accent);
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.hold;
    v.error_fg_color = p.fault;
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.window_margin = Margin::same(18);
    style.spacing.indent = 16.0;
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.interaction.tooltip_delay = 0.35;
    let heading_family = FontFamily::Name("lato-bold".into());
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(22.0, heading_family.clone()),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(14.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(12.5, FontFamily::Monospace),
    );
}

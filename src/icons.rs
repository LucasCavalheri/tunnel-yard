//! Rust/UI's official Lucide SVG registry, rasterized once per glyph for egui.
//!
//! The public framework components target Leptos/Dioxus, but the crate's
//! `common` registry is renderer-agnostic. Using it here keeps the native egui
//! shell while consuming the exact icon data shipped by Rust/UI.

use ::icons::common::icon_registry_getter::get_icon_elements;
use ::icons::common::{IconType, StaticSvgElement};
use eframe::egui::{
    pos2, vec2, Align2, Color32, CornerRadius, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    TextStyle, TextureHandle, TextureOptions, Ui, Vec2,
};

#[derive(Clone, Copy, Hash)]
#[repr(u8)]
pub enum Glyph {
    Plus,
    Import,
    Refresh,
    Plug,
    Unplug,
    Pencil,
    Trash,
    Search,
    Download,
    Eye,
    EyeOff,
    Sun,
    Moon,
    Monitor,
    Hide,
    Quit,
    Check,
    Console,
    Close,
}

impl Glyph {
    fn rust_ui(self) -> IconType {
        match self {
            Glyph::Plus => IconType::Plus,
            Glyph::Import => IconType::FileInput,
            Glyph::Refresh => IconType::RefreshCw,
            Glyph::Plug => IconType::Plug,
            Glyph::Unplug => IconType::Unplug,
            Glyph::Pencil => IconType::Pencil,
            Glyph::Trash => IconType::Trash2,
            Glyph::Search => IconType::Search,
            Glyph::Download => IconType::Download,
            Glyph::Eye => IconType::Eye,
            Glyph::EyeOff => IconType::EyeOff,
            Glyph::Sun => IconType::Sun,
            Glyph::Moon => IconType::Moon,
            Glyph::Monitor => IconType::Monitor,
            Glyph::Hide => IconType::Minimize2,
            Glyph::Quit => IconType::LogOut,
            Glyph::Check => IconType::Check,
            Glyph::Console => IconType::Terminal,
            Glyph::Close => IconType::X,
        }
    }
}

fn svg_element(element: &StaticSvgElement, out: &mut String) {
    use std::fmt::Write;
    match element {
        StaticSvgElement::Path { d } => write!(out, "<path d=\"{d}\"/>").unwrap(),
        StaticSvgElement::Circle { cx, cy, r } => {
            write!(out, "<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\"/>").unwrap()
        }
        StaticSvgElement::Rect {
            x,
            y,
            width,
            height,
            rx,
            ry,
        } => {
            write!(
                out,
                "<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\""
            )
            .unwrap();
            if let Some(rx) = rx {
                write!(out, " rx=\"{rx}\"").unwrap();
            }
            if let Some(ry) = ry {
                write!(out, " ry=\"{ry}\"").unwrap();
            }
            out.push_str("/>");
        }
        StaticSvgElement::Ellipse { cx, cy, rx, ry } => write!(
            out,
            "<ellipse cx=\"{cx}\" cy=\"{cy}\" rx=\"{rx}\" ry=\"{ry}\"/>"
        )
        .unwrap(),
        StaticSvgElement::Line { x1, y1, x2, y2 } => write!(
            out,
            "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\"/>"
        )
        .unwrap(),
        StaticSvgElement::Polyline { points } => {
            write!(out, "<polyline points=\"{points}\"/>").unwrap()
        }
        StaticSvgElement::Polygon { points } => {
            write!(out, "<polygon points=\"{points}\"/>").unwrap()
        }
    }
}

fn render_icon(glyph: Glyph) -> Option<egui::ColorImage> {
    let elements = get_icon_elements(glyph.rust_ui())?;
    let mut svg = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">"##,
    );
    for element in elements {
        svg_element(element, &mut svg);
    }
    svg.push_str("</svg>");

    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 64)?;
    // `usvg` already applies the 24×24 viewBox to the requested 64×64 root.
    // A second scale here would crop every glyph to its upper-left quadrant.
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [64, 64],
        pixmap.data(),
    ))
}

fn texture(ui: &Ui, glyph: Glyph) -> Option<TextureHandle> {
    let id = Id::new(("rust-ui-icon", glyph));
    if let Some(texture) = ui.ctx().data(|data| data.get_temp::<TextureHandle>(id)) {
        return Some(texture);
    }
    let image = render_icon(glyph)?;
    let texture = ui.ctx().load_texture(
        format!("rust-ui-icon-{}", glyph as u8),
        image,
        TextureOptions::LINEAR,
    );
    ui.ctx()
        .data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}

pub fn paint(ui: &Ui, rect: Rect, glyph: Glyph, color: Color32) {
    if let Some(texture) = texture(ui, glyph) {
        ui.painter().image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            color,
        );
    }
}

pub fn status_dot(ui: &Ui, center: Pos2, radius: f32, color: Color32) {
    ui.painter().circle_filled(center, radius, color);
}

pub fn action_button(
    ui: &mut Ui,
    glyph: Glyph,
    text: &str,
    fill: Color32,
    stroke: Stroke,
    fg: Color32,
    min: Vec2,
) -> Response {
    let desired = vec2(ui.available_width().min(min.x), min.y);
    let (rect, resp) = ui.allocate_exact_size(desired, Sense::click());
    let hovered = resp.hovered();
    let bg = if hovered {
        fill.gamma_multiply(1.08)
    } else {
        fill
    };
    ui.painter().rect_filled(rect, CornerRadius::same(10), bg);
    if stroke.width > 0.0 {
        ui.painter()
            .rect_stroke(rect, CornerRadius::same(10), stroke, StrokeKind::Middle);
    }
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(16.0, 16.0));
    paint(ui, icon_rect, glyph, fg);
    let font = TextStyle::Button.resolve(ui.style());
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        font,
        fg,
    );
    resp
}

pub fn icon_button(ui: &mut Ui, glyph: Glyph, color: Color32, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
    let resp = resp.on_hover_text(tip);
    let bg = if resp.hovered() {
        ui.visuals().widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), bg);
    paint(
        ui,
        Rect::from_center_size(rect.center(), vec2(18.0, 18.0)),
        glyph,
        color,
    );
    resp
}

pub fn icon_chip(
    ui: &mut Ui,
    glyph: Glyph,
    selected: bool,
    fill: Color32,
    stroke: Color32,
    fg: Color32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 28.0), Sense::click());
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0_f32, stroke),
        StrokeKind::Middle,
    );
    let _ = selected;
    paint(
        ui,
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        glyph,
        fg,
    );
    resp
}

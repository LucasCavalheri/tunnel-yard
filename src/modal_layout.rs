//! Shared sizing rules for application modal content.

use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};

/// Give the preferences sheet a bounded height so its settings list can scroll.
pub fn preferences_modal_frame(surface: Div) -> Div {
    surface
        .w(px(680.))
        .h(px(680.))
        .max_h(relative(0.9))
        .v_flex()
        .overflow_hidden()
}

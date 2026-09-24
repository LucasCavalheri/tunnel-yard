//! Layout rules for the main window: one centered column, rows that never overlap.

use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};

/// Widest the profile column gets. Past this the window just gains margin.
pub const COLUMN_MAX: Pixels = px(820.);

/// The single centered column that holds the header, the profile list and the console.
pub fn workspace_column(column: Div) -> Div {
    column
        .w_full()
        .max_w(COLUMN_MAX)
        .mx_auto()
        .flex_1()
        .min_h_0()
        .v_flex()
        .px_8()
}

/// A row with a fixed lead (avatar), a body that shrinks and actions that always stay whole.
/// Long hosts or user names truncate inside `body` instead of pushing the actions away.
pub fn row(
    lead: impl Styled + IntoElement,
    body: impl Styled + IntoElement,
    actions: impl Styled + IntoElement,
) -> Div {
    div()
        .w_full()
        .h_flex()
        .items_center()
        .gap_4()
        .child(lead.flex_none())
        .child(body.flex_1().min_w_0().v_flex().overflow_hidden())
        .child(actions.flex_none().h_flex().items_center().gap_2())
}

/// One line of text that ends in an ellipsis when it does not fit.
pub fn line(text: impl Into<SharedString>) -> Div {
    div().w_full().min_w_0().truncate().child(text.into())
}

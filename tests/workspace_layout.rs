use gpui_kit::component::StyledExt as _;
use gpui_kit::prelude::*;
use gpui_kit::test::{TestSupportExt as _, TestWindowExt as _};
use gpui_kit::{div, px, TestAppContext, Window, WindowOptions};
use tunnel_yard::layout::{line, row, workspace_column, COLUMN_MAX};

const LONG_HOST: &str = "sslvpn.a-really-long-company-name.example.com.br:10443 · someone.with.a.long.login@example.com.br";

struct RowProbe;

impl Render for RowProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .w(px(420.))
                .child(row(
                    div().child(div().size(px(36.)).id("lead").test_support()),
                    div()
                        .child(line("Profile"))
                        .child(line(LONG_HOST))
                        .child(div().w_full().h(px(1.)).id("body").test_support()),
                    div()
                        .child(div().w(px(36.)).h(px(32.)).id("edit").test_support())
                        .child(div().w(px(120.)).h(px(32.)).id("connect").test_support()),
                ))
                .child(div().w_full().h(px(1.)).id("frame").test_support()),
        )
    }
}

struct ColumnProbe;

impl Render for ColumnProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(1600.))
            .h(px(600.))
            .v_flex()
            .child(div().w_full().h(px(1.)).id("window").test_support())
            .child(workspace_column(
                div().child(div().w_full().h(px(10.)).id("column").test_support()),
            ))
    }
}

fn open<V: Render + 'static>(
    cx: &mut TestAppContext,
    view: fn() -> V,
) -> gpui_kit::WindowHandle<V> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| view()))
            .expect("test window should open")
    })
}

#[test]
fn a_long_host_truncates_and_never_covers_the_buttons() {
    let mut cx = TestAppContext::single();
    let window = open(&mut cx, || RowProbe);

    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        let frame = window.find("frame").bounds();
        let body = window.find("body").bounds();
        let edit = window.find("edit").bounds();
        let connect = window.find("connect").bounds();

        assert_eq!(edit.size.width, px(36.), "edit button was squeezed");
        assert_eq!(connect.size.width, px(120.), "connect button was squeezed");
        assert!(
            connect.right() <= frame.right(),
            "buttons spill out of the row"
        );
        assert!(body.right() <= edit.left(), "text runs under the buttons");
        assert!(body.size.width > px(100.), "the text has no room left");
        assert!(
            window.find("lead").bounds().size.width == px(36.),
            "avatar shrank"
        );
    })
    .expect("test window should remain open");
}

#[test]
fn the_column_stops_growing_on_wide_windows_and_stays_centered() {
    let mut cx = TestAppContext::single();
    let window = open(&mut cx, || ColumnProbe);

    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        let outer = window.find("window").bounds();
        let column = window.find("column").bounds();

        // The probe sits inside the column's 32px side padding.
        assert_eq!(column.size.width, COLUMN_MAX - px(64.));
        let left = column.left() - outer.left();
        let right = outer.right() - column.right();
        assert!(
            (left - right).abs() <= px(1.),
            "column is off center: {left:?} vs {right:?}"
        );
    })
    .expect("test window should remain open");
}

use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::prelude::*;
use gpui_kit::test::{TestSupportExt as _, TestWindowExt as _};
use gpui_kit::{div, px, TestAppContext, Window, WindowOptions};
use tunnel_yard::modal_layout::preferences_modal_frame;

struct PreferencesLayoutProbe;

impl Render for PreferencesLayoutProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                preferences_modal_frame(div())
                    .child(div().h(px(72.)))
                    .child(
                        div().flex_1().min_h_0().overflow_y_scrollbar().child(
                            div()
                                .id("preferences-content-probe")
                                .test_support()
                                .h(px(900.)),
                        ),
                    )
                    .child(div().h(px(64.)))
                    .id("preferences-panel")
                    .test_support(),
            )
    }
}

#[test]
fn preferences_layout_keeps_a_visible_scroll_region() {
    let mut cx = TestAppContext::single();
    let window = cx.update(|cx| {
        gpui_kit::init(cx);
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| PreferencesLayoutProbe)
        })
        .expect("test window should open")
    });

    cx.update_window(window.into(), |_, window, cx| {
        window.render_frame(cx);
        let panel_height = window.find("preferences-panel").bounds().size.height;
        let settings_content = window.find("preferences-content-probe");

        assert!(
            panel_height > px(250.),
            "panel collapsed to {panel_height:?}"
        );
        assert!(settings_content.visible(), "preferences content is clipped");
    })
    .expect("test window should remain open");
}

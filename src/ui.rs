//! Native GPUI Kit desktop shell.
//!
//! VPN processes, profile persistence and tray behavior remain in the domain
//! modules. This module owns only presentation and interaction.

use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    scroll::ScrollableElement as _,
    switch::Switch,
    ActiveTheme, Disableable, Icon, IconName, Root, Selectable, Sizable, StyledExt as _, Theme,
    ThemeMode, TitleBar,
};
use gpui_kit::{prelude::*, *};
use my_vpns::autostart::{get_autostart_path, is_autostart_enabled, set_autostart_enabled};
use my_vpns::conf::{
    delete_profile_file, draft_from_imported_file, empty_draft, save_profile_draft, VpnProfileDraft,
};
use my_vpns::deps::{get_dependency_status, install_vpn_client, DependencyStatus, InstallResult};
use my_vpns::desktop::{EDITOR_FIELDS, SETUP_GATE_KEYS, TRAY_MENU_KEYS, UI_SURFACES};
use my_vpns::i18n::translate;
use my_vpns::settings::{load_settings, save_settings, AppSettingsPatch};
use my_vpns::updates::{perform_update_check, UpdateCheckResult, UpdateInfo, FIRST_CHECK_DELAY_MS};
use my_vpns::vpn::{summarize_vpn_state, VpnEvent, VpnManager, VpnProfile, VpnState, VpnStatus};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BRAND: u32 = 0xff5f2d;
const BRAND_HOVER: u32 = 0xf04f20;
const BRAND_ACTIVE: u32 = 0xd94317;
const SIDEBAR_BG: u32 = 0x0d0f0f;
const SIDEBAR_RAISED: u32 = 0x171919;
const SIDEBAR_BORDER: u32 = 0x292c2b;
const SIDEBAR_TEXT: u32 = 0xf4f5f1;
const SIDEBAR_MUTED: u32 = 0x858b87;
const CONSOLE_BG: u32 = 0x0c0e0e;
const SIDEBAR_WIDTH: Pixels = px(218.);

#[allow(dead_code)]
fn surfaces_are_shipped() -> bool {
    !UI_SURFACES.is_empty()
        && TRAY_MENU_KEYS.len() == 4
        && EDITOR_FIELDS.len() >= 14
        && SETUP_GATE_KEYS.len() >= 5
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckFeedback {
    Idle,
    Checking,
    UpToDate,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditorMode {
    Create,
    Edit,
    Import,
}

#[derive(Clone)]
struct ProfileEditor {
    mode: EditorMode,
    id: Entity<InputState>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    trusted_cert: Entity<InputState>,
    realm: Entity<InputState>,
    persistent: Entity<InputState>,
    health_host: Entity<InputState>,
    health_port: Entity<InputState>,
    set_dns: bool,
    set_routes: bool,
    no_dtls: bool,
    legacy_tunnel: bool,
    extra_options: Vec<(String, String)>,
}

enum SetupMsg {
    Log(String),
    Done(Box<InstallResult>),
}

#[derive(Clone)]
enum TrayCmd {
    Show,
    Quit,
    Refresh,
    DisconnectAll,
    CheckUpdates,
    Toggle(String),
}

struct Desk {
    locale: String,
    theme: String,
    version: String,
    deps: Option<DependencyStatus>,
    deps_ready: bool,
    boot_error: Option<String>,
    profiles: Vec<VpnProfile>,
    state: VpnState,
    logs: Vec<String>,
    install_logs: Vec<String>,
    autostart: bool,
    editor: Option<ProfileEditor>,
    editor_error: Option<String>,
    confirm_delete: Option<String>,
    confirm_quit: bool,
    vpn: Arc<Mutex<VpnManager>>,
    events: Receiver<VpnEvent>,
    last_connected: std::collections::HashSet<String>,
    setup_busy: bool,
    quitting: Arc<Mutex<bool>>,
    tray_rx: Option<Receiver<TrayCmd>>,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    os_tray: Option<tray_icon::TrayIcon>,
    update: Option<UpdateInfo>,
    check_feedback: CheckFeedback,
    update_rx: Receiver<UpdateCheckResult>,
    update_tx: mpsc::Sender<UpdateCheckResult>,
    started_at: Instant,
    auto_check_sent: bool,
    dismissed_update: Option<String>,
    setup_rx: Receiver<SetupMsg>,
    setup_tx: mpsc::Sender<SetupMsg>,
    search: Entity<InputState>,
    theme_applied: bool,
    preferences_open: bool,
}

pub fn run(hidden: bool) -> Result<(), String> {
    let _ = my_vpns::app_icon::ensure_app_icon_files();
    #[cfg(target_os = "linux")]
    let _ = my_vpns::app_icon::ensure_linux_desktop_entry();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let settings = load_settings();
            // The product UI is intentionally dark: the desktop application and
            // the interface shown on the website share the same visual spec.
            let initial_theme = "dark".to_string();
            let icon = image::load_from_memory(my_vpns::app_icon::APP_ICON_PNG)
                .ok()
                .map(|image| Arc::new(image.into_rgba8()));
            let mut options = TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::centered(size(px(1040.), px(690.)), cx));
            options.window_min_size = Some(size(px(860.), px(580.)));
            options.app_id = Some(my_vpns::APP_ID.into());
            options.show = !hidden;
            options.icon = icon;

            let locale = settings.locale;
            let dismissed_update = settings.dismissed_update_version;
            cx.spawn(async move |cx| {
                cx.open_window(options, move |window, cx| {
                    apply_theme(&initial_theme, window, cx);
                    let desk =
                        cx.new(|cx| Desk::new(locale, initial_theme, dismissed_update, window, cx));
                    let quitting = desk.read(cx).quitting.clone();
                    window.on_window_should_close(cx, move |window, _| {
                        if *quitting.lock().unwrap() {
                            true
                        } else {
                            window.minimize_window();
                            false
                        }
                    });
                    cx.new(|cx| Root::new(desk, window, cx))
                })
                .map_err(|error| eprintln!("[my-vpns] failed to open window: {error}"))
                .ok();
            })
            .detach();
        });
    Ok(())
}

impl Drop for Desk {
    fn drop(&mut self) {
        *self.quitting.lock().unwrap() = true;
        self.vpn.lock().unwrap().disconnect(None);
    }
}

impl Desk {
    fn new(
        locale: String,
        theme: String,
        dismissed_update: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (vpn, events) = VpnManager::subscribe();
        let vpn = Arc::new(Mutex::new(vpn));
        let quitting = Arc::new(Mutex::new(false));
        let (tray_tx, tray_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let (setup_tx, setup_rx) = mpsc::channel();
        spawn_tray(vpn.clone(), locale.clone(), tray_tx, quitting.clone());

        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(translate(&locale, "ops.search", &[]))
        });

        let mut desk = Self {
            locale,
            theme,
            version: my_vpns::APP_VERSION.into(),
            deps: None,
            deps_ready: false,
            boot_error: None,
            profiles: Vec::new(),
            state: VpnState {
                sessions: Default::default(),
                auto_reconnect: false,
            },
            logs: Vec::new(),
            install_logs: Vec::new(),
            autostart: is_autostart_enabled(),
            editor: None,
            editor_error: None,
            confirm_delete: None,
            confirm_quit: false,
            vpn,
            events,
            last_connected: Default::default(),
            setup_busy: false,
            quitting,
            tray_rx: Some(tray_rx),
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            os_tray: None,
            update: None,
            check_feedback: CheckFeedback::Idle,
            update_rx,
            update_tx,
            started_at: Instant::now(),
            auto_check_sent: false,
            dismissed_update,
            setup_rx,
            setup_tx,
            search,
            theme_applied: false,
            preferences_open: false,
        };

        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            desk.os_tray = create_os_tray(&desk.locale, &desk.vpn.lock().unwrap());
        }

        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(get_dependency_status)) {
            Ok(status) => {
                desk.deps_ready = status.client_installed;
                desk.deps = Some(status);
            }
            Err(_) => desk.boot_error = Some("Failed to probe VPN client.".into()),
        }
        if desk.deps_ready {
            desk.profiles = desk.vpn.lock().unwrap().get_profiles();
            desk.state = desk.vpn.lock().unwrap().get_state();
        }

        if std::env::var("MY_VPNS_SHOT").as_deref() == Ok("editor") {
            desk.editor = Some(ProfileEditor::from_draft(
                EditorMode::Create,
                empty_draft(),
                window,
                cx,
            ));
        }

        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            if this
                .update_in(cx, |this, window, cx| {
                    this.pump(window, cx);
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        })
        .detach();

        desk
    }

    fn t(&self, key: &str) -> String {
        translate(&self.locale, key, &[])
    }

    fn tv(&self, key: &str, vars: &[(&str, String)]) -> String {
        translate(&self.locale, key, vars)
    }

    fn spawn_update_check(&self) {
        let tx = self.update_tx.clone();
        let version = self.version.clone();
        std::thread::spawn(move || {
            let _ = tx.send(perform_update_check(&version));
        });
    }

    fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        *self.quitting.lock().unwrap() = true;
        self.vpn.lock().unwrap().disconnect(None);
        window.remove_window();
        cx.quit();
    }

    fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.auto_check_sent
            && self.started_at.elapsed().as_millis() as u64 >= FIRST_CHECK_DELAY_MS
        {
            self.auto_check_sent = true;
            self.spawn_update_check();
        }
        while let Ok(message) = self.setup_rx.try_recv() {
            match message {
                SetupMsg::Log(line) => {
                    self.install_logs.push(line);
                    if self.install_logs.len() > 200 {
                        self.install_logs.drain(0..self.install_logs.len() - 200);
                    }
                }
                SetupMsg::Done(result) => {
                    let result = *result;
                    self.setup_busy = false;
                    if result.ok {
                        self.deps = Some(result.status);
                        self.deps_ready = true;
                        self.profiles = self.vpn.lock().unwrap().refresh_profiles();
                    } else {
                        self.editor_error = Some(if result.output.is_empty() {
                            self.t("setup.installFailed")
                        } else {
                            result.output
                        });
                    }
                }
            }
        }
        while let Ok(result) = self.update_rx.try_recv() {
            match result {
                UpdateCheckResult::Available(info) => {
                    if self.dismissed_update.as_deref() != Some(info.latest.as_str()) {
                        self.update = Some(info);
                    }
                    self.check_feedback = CheckFeedback::Idle;
                }
                UpdateCheckResult::UpToDate { .. } => self.check_feedback = CheckFeedback::UpToDate,
                UpdateCheckResult::Error { .. } => self.check_feedback = CheckFeedback::Error,
            }
        }
        while let Ok(event) = self.events.try_recv() {
            match event {
                VpnEvent::State(state) => self.on_state(state),
                VpnEvent::Log(line) => {
                    self.logs.push(line);
                    if self.logs.len() > 400 {
                        self.logs.drain(0..self.logs.len() - 400);
                    }
                }
                VpnEvent::Profiles(profiles) => {
                    self.profiles = profiles;
                    self.rebuild_os_tray();
                }
                VpnEvent::NeedReconnect(id) => self.vpn.lock().unwrap().connect(&id),
            }
        }

        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
                if matches!(
                    event,
                    tray_icon::TrayIconEvent::DoubleClick { .. }
                        | tray_icon::TrayIconEvent::Click {
                            button: tray_icon::MouseButton::Left,
                            button_state: tray_icon::MouseButtonState::Up,
                            ..
                        }
                ) {
                    window.activate_window();
                }
            }
            while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                self.handle_tray_id(event.id.as_ref(), window, cx);
            }
        }

        let mut commands = Vec::new();
        if let Some(receiver) = &self.tray_rx {
            while let Ok(command) = receiver.try_recv() {
                commands.push(command);
            }
        }
        for command in commands {
            match command {
                TrayCmd::Show => window.activate_window(),
                TrayCmd::Quit => self.request_quit(window, cx),
                TrayCmd::Refresh => {
                    self.profiles = self.vpn.lock().unwrap().refresh_profiles();
                    self.rebuild_os_tray();
                }
                TrayCmd::CheckUpdates => {
                    self.check_feedback = CheckFeedback::Checking;
                    self.spawn_update_check();
                }
                TrayCmd::DisconnectAll => self.vpn.lock().unwrap().disconnect(None),
                TrayCmd::Toggle(id) => self.toggle_profile(&id),
            }
        }
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn handle_tray_id(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match id {
            "show" => window.activate_window(),
            "quit" => self.request_quit(window, cx),
            "refresh" => {
                self.profiles = self.vpn.lock().unwrap().refresh_profiles();
                self.rebuild_os_tray();
            }
            "check_updates" => {
                self.check_feedback = CheckFeedback::Checking;
                self.spawn_update_check();
            }
            "disconnect_all" => self.vpn.lock().unwrap().disconnect(None),
            _ => {
                if let Some(profile_id) = id.strip_prefix("profile:") {
                    self.toggle_profile(profile_id);
                }
            }
        }
    }

    fn toggle_profile(&self, id: &str) {
        let active = self
            .state
            .sessions
            .get(id)
            .map(|session| session.status != VpnStatus::Disconnected)
            .unwrap_or(false);
        if active {
            self.vpn.lock().unwrap().disconnect(Some(id));
        } else {
            self.vpn.lock().unwrap().connect(id);
        }
    }

    fn on_state(&mut self, state: VpnState) {
        let connected_now: std::collections::HashSet<String> = state
            .sessions
            .values()
            .filter(|session| session.status == VpnStatus::Connected)
            .map(|session| session.profile_id.clone())
            .collect();
        let transitional: std::collections::HashSet<String> = state
            .sessions
            .values()
            .filter(|session| session.status == VpnStatus::Connecting)
            .map(|session| session.profile_id.clone())
            .collect();

        for id in &connected_now {
            if !self.last_connected.contains(id) {
                notify(
                    &self.t("notify.connectedTitle"),
                    &self.tv("notify.connectedBody", &[("id", id.clone())]),
                );
            }
        }
        for id in self.last_connected.clone() {
            if !connected_now.contains(&id) && !transitional.contains(&id) {
                let message = state
                    .sessions
                    .get(&id)
                    .map(|session| session.message.clone())
                    .filter(|message| !message.is_empty())
                    .unwrap_or_else(|| self.tv("notify.disconnectedBody", &[("id", id.clone())]));
                notify(&self.t("notify.disconnectedTitle"), &message);
            }
        }
        self.last_connected = connected_now
            .union(
                &self
                    .last_connected
                    .iter()
                    .filter(|id| transitional.contains(*id))
                    .cloned()
                    .collect(),
            )
            .cloned()
            .collect();
        self.state = state;
        self.rebuild_os_tray();
    }

    fn rebuild_os_tray(&mut self) {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        if let Some(tray) = self.os_tray.as_mut() {
            if let Some(menu) = os_tray_menu(&self.locale, &self.profiles, &self.state) {
                let _ = tray.set_menu(Some(Box::new(menu)));
            }
        }
    }

    fn open_editor(
        &mut self,
        mode: EditorMode,
        draft: VpnProfileDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor_error = None;
        self.editor = Some(ProfileEditor::from_draft(mode, draft, window, cx));
    }

    fn open_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = pick_conf_file() {
            let (ok, message, draft) = draft_from_imported_file(&path);
            if ok {
                if let Some(draft) = draft {
                    self.open_editor(EditorMode::Import, draft, window, cx);
                }
            } else {
                self.editor_error = Some(message);
            }
        }
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.clone() else {
            return;
        };
        let draft = editor.to_draft(cx);
        let result = save_profile_draft(&draft, editor.mode == EditorMode::Edit);
        if result.ok {
            self.profiles = self.vpn.lock().unwrap().refresh_profiles();
            self.editor = None;
            self.editor_error = None;
        } else {
            self.editor_error = Some(result.message);
        }
    }

    fn set_locale(&mut self, locale: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.locale = locale.into();
        save_settings(AppSettingsPatch {
            locale: Some(locale.into()),
            ..Default::default()
        });
        self.search.update(cx, |search, cx| {
            search.set_placeholder(self.t("ops.search"), window, cx)
        });
        cx.notify();
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> TitleBar {
        let parked_title = self.t("notify.parkedTitle");
        let parked_body = self.t("notify.parkedBody");
        TitleBar::new()
            .on_close_window(move |_, window, _| {
                window.minimize_window();
                notify(&parked_title, &parked_body);
            })
            .child(
                div()
                    .h_full()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(brand_mark(cx, px(20.)))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xc7cbc7))
                            .child("My VPNs"),
                    ),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let summary = summarize_vpn_state(&self.state);
        let connection_count = summary.connected_count + summary.connecting_count;
        let any_up = connection_count > 0;
        let status_color = if any_up {
            cx.theme().success
        } else {
            rgb(SIDEBAR_MUTED).into()
        };
        let connection_label = self.tv(
            if connection_count == 1 {
                "ops.connectionOne"
            } else {
                "ops.connectionMany"
            },
            &[("count", connection_count.to_string())],
        );

        div()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .v_flex()
            .border_r_1()
            .border_color(rgb(SIDEBAR_BORDER))
            .bg(rgb(SIDEBAR_BG))
            .px_4()
            .py_5()
            .gap_3()
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .justify_between()
                    .px_1()
                    .pb_2()
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(SIDEBAR_TEXT))
                                    .child("My VPNs"),
                            )
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(rgb(SIDEBAR_MUTED))
                                    .child("Control desk"),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(rgb(SIDEBAR_BORDER))
                            .text_size(px(8.))
                            .text_color(rgb(SIDEBAR_MUTED))
                            .font_family("monospace")
                            .child(format!("v{}", self.version)),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .p_3()
                    .rounded(px(7.))
                    .border_1()
                    .border_color(if any_up {
                        cx.theme().success.opacity(0.2)
                    } else {
                        rgb(SIDEBAR_BORDER).into()
                    })
                    .bg(if any_up {
                        cx.theme().success.opacity(0.07)
                    } else {
                        rgb(SIDEBAR_RAISED).into()
                    })
                    .child(
                        div()
                            .size(px(30.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .bg(status_color.opacity(0.13))
                            .text_color(status_color)
                            .child(Icon::new(IconName::Network).size(px(16.))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .v_flex()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if any_up {
                                        rgb(0xdce9e2)
                                    } else {
                                        rgb(SIDEBAR_TEXT)
                                    })
                                    .child(connection_label),
                            )
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(rgb(SIDEBAR_MUTED))
                                    .child(if any_up {
                                        self.t("ops.connectionsStable")
                                    } else {
                                        self.t("ops.noneActive")
                                    }),
                            ),
                    )
                    .child(div().size(px(7.)).rounded(px(999.)).bg(status_color).when(
                        any_up,
                        |this| {
                            this.border_2()
                                .border_color(cx.theme().success.opacity(0.25))
                        },
                    )),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        Button::new("new-profile")
                            .primary()
                            .w_full()
                            .icon(IconName::Plus)
                            .label(self.t("ops.newProfile"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_editor(EditorMode::Create, empty_draft(), window, cx)
                            })),
                    )
                    .child(
                        Button::new("import-profile")
                            .w_full()
                            .ghost()
                            .text_color(rgb(SIDEBAR_TEXT))
                            .icon(IconName::FileText)
                            .label(self.t("ops.importConf"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_import(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("open-preferences")
                            .w_full()
                            .ghost()
                            .text_color(rgb(SIDEBAR_TEXT))
                            .icon(IconName::Settings2)
                            .label(self.t("ops.preferences"))
                            .on_click(cx.listener(|this, _, _, _| {
                                this.preferences_open = true;
                            })),
                    ),
            )
            .child(
                div()
                    .mt_auto()
                    .h_flex()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(SIDEBAR_BORDER))
                    .pt_4()
                    .px_1()
                    .text_color(cx.theme().success)
                    .child(
                        Icon::new(IconName::CircleCheck)
                            .size(px(17.))
                            .text_color(cx.theme().success),
                    )
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0x9da29e))
                                    .child(self.t("ops.protected")),
                            )
                            .child(
                                div()
                                    .text_size(px(8.))
                                    .text_color(rgb(SIDEBAR_MUTED))
                                    .child(self.t("ops.unprivileged")),
                            ),
                    ),
            )
    }

    fn render_preferences_overlay(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.preferences_open {
            return None;
        }
        let summary = summarize_vpn_state(&self.state);
        let any_up = summary.connected_count + summary.connecting_count > 0;
        let auto_reconnect = self.state.auto_reconnect;
        let autostart = self.autostart;

        Some(
            overlay().child(
                surface(cx)
                    .w(px(590.))
                    .max_h(px(540.))
                    .v_flex()
                    .overflow_hidden()
                    .bg(rgb(0x141616))
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .px_5()
                            .py_4()
                            .border_b_1()
                            .border_color(rgb(SIDEBAR_BORDER))
                            .child(
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(20.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(self.t("ops.preferences")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.t("ops.preferencesHint")),
                                    ),
                            )
                            .child(
                                Button::new("close-preferences-top")
                                    .ghost()
                                    .icon(IconName::Close)
                                    .on_click(
                                        cx.listener(|this, _, _, _| this.preferences_open = false),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("preferences-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .v_flex()
                            .gap_5()
                            .p_5()
                            .child(
                                div()
                                    .v_flex()
                                    .gap_3()
                                    .child(sidebar_section_label(self.t("ops.connectionSettings")))
                                    .child(
                                        div()
                                            .h_flex()
                                            .justify_between()
                                            .gap_4()
                                            .p_4()
                                            .rounded(px(7.))
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(cx.theme().secondary)
                                            .child(
                                                Switch::new("preferences-auto-reconnect")
                                                    .checked(auto_reconnect)
                                                    .label(self.t("ops.autoRelink"))
                                                    .on_click(cx.listener(|this, next, _, _| {
                                                        this.vpn
                                                            .lock()
                                                            .unwrap()
                                                            .set_auto_reconnect(*next);
                                                        this.state.auto_reconnect = *next;
                                                    })),
                                            )
                                            .child(
                                                Switch::new("preferences-autostart")
                                                    .checked(autostart)
                                                    .label(self.t("ops.startWithLinux"))
                                                    .tooltip(get_autostart_path())
                                                    .on_click(cx.listener(|this, next, _, _| {
                                                        if set_autostart_enabled(*next) {
                                                            this.autostart = is_autostart_enabled();
                                                        }
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap_3()
                                    .child(sidebar_section_label(self.t("ops.language")))
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("preferences-locale-pt")
                                                    .selected(self.locale == "pt-BR")
                                                    .label("Português")
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.set_locale("pt-BR", window, cx)
                                                        },
                                                    )),
                                            )
                                            .child(
                                                Button::new("preferences-locale-en")
                                                    .selected(self.locale == "en")
                                                    .label("English")
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.set_locale("en", window, cx)
                                                        },
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap_3()
                                    .child(sidebar_section_label(self.t("ops.maintenance")))
                                    .child(self.render_update_feedback(cx))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap_2()
                                            .child(
                                                Button::new("preferences-reload")
                                                    .icon(IconName::RotateCw)
                                                    .label(self.t("ops.reloadProfiles"))
                                                    .on_click(cx.listener(|this, _, _, _| {
                                                        this.profiles = this
                                                            .vpn
                                                            .lock()
                                                            .unwrap()
                                                            .refresh_profiles();
                                                        this.rebuild_os_tray();
                                                    })),
                                            )
                                            .child(
                                                Button::new("preferences-updates")
                                                    .icon(IconName::ArrowDown)
                                                    .loading(
                                                        self.check_feedback
                                                            == CheckFeedback::Checking,
                                                    )
                                                    .label(match self.check_feedback {
                                                        CheckFeedback::Checking => {
                                                            self.t("update.checking")
                                                        }
                                                        _ => self.t("update.checkNow"),
                                                    })
                                                    .on_click(cx.listener(|this, _, _, _| {
                                                        this.check_feedback =
                                                            CheckFeedback::Checking;
                                                        this.spawn_update_check();
                                                    })),
                                            )
                                            .child(
                                                Button::new("preferences-disconnect-all")
                                                    .danger()
                                                    .outline()
                                                    .disabled(!any_up)
                                                    .icon(IconName::Pause)
                                                    .label(self.t("ops.killAll"))
                                                    .on_click(cx.listener(|this, _, _, _| {
                                                        this.vpn.lock().unwrap().disconnect(None)
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .gap_2()
                            .px_5()
                            .py_4()
                            .border_t_1()
                            .border_color(rgb(SIDEBAR_BORDER))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("preferences-hide")
                                            .ghost()
                                            .icon(IconName::PanelLeftClose)
                                            .label(self.t("ops.hideToTray"))
                                            .on_click(cx.listener(|this, _, window, _| {
                                                this.preferences_open = false;
                                                window.minimize_window();
                                            })),
                                    )
                                    .child(
                                        Button::new("preferences-quit")
                                            .danger()
                                            .outline()
                                            .icon(IconName::Close)
                                            .label(self.t("ops.quit"))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.preferences_open = false;
                                                if any_up {
                                                    this.confirm_quit = true;
                                                } else {
                                                    this.request_quit(window, cx);
                                                }
                                            })),
                                    ),
                            )
                            .child(
                                Button::new("close-preferences")
                                    .primary()
                                    .label(self.t("form.done"))
                                    .on_click(
                                        cx.listener(|this, _, _, _| this.preferences_open = false),
                                    ),
                            ),
                    ),
            ),
        )
    }

    fn render_update_feedback(&self, cx: &mut Context<Self>) -> Div {
        let text = match self.check_feedback {
            CheckFeedback::UpToDate => {
                self.tv("update.upToDate", &[("version", self.version.clone())])
            }
            CheckFeedback::Error => self.t("update.checkFailed"),
            _ => String::new(),
        };
        let color = if self.check_feedback == CheckFeedback::Error {
            cx.theme().danger
        } else {
            cx.theme().success
        };
        div().when(!text.is_empty(), |this| {
            this.p_2()
                .rounded(px(8.))
                .bg(color.opacity(0.1))
                .text_color(color)
                .text_size(px(11.))
                .child(text)
        })
    }

    fn render_workspace(&self, cx: &mut Context<Self>) -> Div {
        let filter = self.search.read(cx).value().to_lowercase();
        let visible: Vec<VpnProfile> = self
            .profiles
            .iter()
            .filter(|profile| {
                filter.is_empty()
                    || [
                        profile.id.as_str(),
                        profile.name.as_str(),
                        profile.host.as_str(),
                        profile.username.as_str(),
                    ]
                    .iter()
                    .any(|value| value.to_lowercase().contains(&filter))
            })
            .cloned()
            .collect();

        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .v_flex()
            .bg(rgb(0x141616))
            .children(self.render_update_banner(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .v_flex()
                    .px_6()
                    .pt_6()
                    .pb_5()
                    .gap_4()
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .child(
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(25.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(self.t("ops.tunnels")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.t("brand.subtitleMulti")),
                                    ),
                            )
                            .child(
                                div().w(px(250.)).child(
                                    Input::new(&self.search)
                                        .prefix(IconName::Search)
                                        .cleanable(true),
                                ),
                            ),
                    )
                    .child(self.render_profiles(visible, cx))
                    .child(self.render_console(cx)),
            )
    }

    fn render_update_banner(&self, cx: &mut Context<Self>) -> Option<Div> {
        let info = self.update.clone()?;
        Some(
            div()
                .h_flex()
                .justify_between()
                .px_6()
                .py_3()
                .border_b_1()
                .border_color(cx.theme().primary.opacity(0.24))
                .bg(cx.theme().primary.opacity(0.08))
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .text_color(cx.theme().primary)
                        .child(Icon::new(IconName::Info))
                        .child(self.tv(
                            "update.available",
                            &[
                                ("latest", info.latest.clone()),
                                ("current", info.current.clone()),
                            ],
                        )),
                )
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            Button::new("open-release")
                                .small()
                                .primary()
                                .outline()
                                .icon(IconName::ExternalLink)
                                .label(self.t("update.open"))
                                .on_click(move |_, _, _| {
                                    let _ = open::that(&info.url);
                                }),
                        )
                        .child(
                            Button::new("dismiss-update")
                                .small()
                                .ghost()
                                .label(self.t("update.dismiss"))
                                .on_click(cx.listener(move |this, _, _, _| {
                                    save_settings(AppSettingsPatch {
                                        dismissed_update_version: Some(info.latest.clone()),
                                        ..Default::default()
                                    });
                                    this.dismissed_update = Some(info.latest.clone());
                                    this.update = None;
                                })),
                        ),
                ),
        )
    }

    fn render_profiles(
        &self,
        visible: Vec<VpnProfile>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let body = div()
            .id("profile-list")
            .flex_1()
            .min_h_0()
            .v_flex()
            .gap_2()
            .overflow_y_scrollbar();

        if self.profiles.is_empty() {
            return body.child(self.render_empty_state(cx));
        }
        if visible.is_empty() {
            return body.child(
                div()
                    .flex_1()
                    .v_flex()
                    .items_center()
                    .justify_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(IconName::Search).size(px(28.)))
                    .child(self.t("profiles.noMatch")),
            );
        }
        body.children(
            visible
                .into_iter()
                .map(|profile| self.render_profile_card(profile, cx)),
        )
    }

    fn render_empty_state(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .v_flex()
            .items_center()
            .justify_center()
            .gap_3()
            .rounded(px(16.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted.opacity(0.3))
            .child(
                div()
                    .size(px(54.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(16.))
                    .bg(cx.theme().primary.opacity(0.12))
                    .text_color(cx.theme().primary)
                    .child(Icon::new(IconName::Network).size(px(28.))),
            )
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.t("profiles.emptyTitle")),
            )
            .child(
                div()
                    .max_w(px(440.))
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.t("profiles.emptyBody")),
            )
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        Button::new("empty-new")
                            .primary()
                            .icon(IconName::Plus)
                            .label(self.t("ops.newProfile"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_editor(EditorMode::Create, empty_draft(), window, cx)
                            })),
                    )
                    .child(
                        Button::new("empty-import")
                            .icon(IconName::FileText)
                            .label(self.t("ops.importConf"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_import(window, cx)),
                            ),
                    ),
            )
    }

    fn render_profile_card(&self, profile: VpnProfile, cx: &mut Context<Self>) -> Div {
        let initials = profile_initials(&profile.name);
        let session = self.state.sessions.get(&profile.id).cloned();
        let status = session
            .as_ref()
            .map(|session| session.status)
            .unwrap_or(VpnStatus::Disconnected);
        let active = matches!(status, VpnStatus::Connected | VpnStatus::Connecting);
        let (status_key, status_color) = match status {
            VpnStatus::Connected => ("status.linkUp", cx.theme().success),
            VpnStatus::Connecting => ("status.handshake", cx.theme().warning),
            VpnStatus::Error => ("status.fault", cx.theme().danger),
            VpnStatus::Disconnected => ("status.idle", cx.theme().muted_foreground),
        };
        let metadata = format!("{}  ·  {}", profile.host, profile.port);
        let detail = match status {
            VpnStatus::Connected => session
                .as_ref()
                .and_then(|session| session.connected_at)
                .map(|at| self.tv("profiles.live", &[("uptime", format_duration(at))])),
            VpnStatus::Connecting => Some(self.t("profiles.handshake")),
            _ => session
                .as_ref()
                .map(|session| session.message.clone())
                .filter(|message| !message.is_empty())
                .or_else(|| (!profile.username.is_empty()).then(|| profile.username.clone())),
        };
        let profile_id = profile.id.clone();
        let edit_id = profile.id.clone();
        let mark_color = profile_mark_color(&profile.name);

        div()
            .flex_none()
            .h_flex()
            .justify_between()
            .gap_4()
            .px_4()
            .py_3()
            .rounded(px(7.))
            .border_1()
            .border_color(if active {
                status_color.opacity(0.32)
            } else {
                cx.theme().border
            })
            .bg(rgb(0x191b1b))
            .when(active, |this| this.border_l_2())
            .hover(|style| {
                style
                    .border_color(cx.theme().primary.opacity(0.42))
                    .bg(cx.theme().secondary.opacity(0.72))
            })
            .child(
                div()
                    .h_flex()
                    .min_w_0()
                    .gap_3()
                    .child(
                        div()
                            .size(px(44.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(7.))
                            .bg(rgb(mark_color))
                            .text_color(rgb(0xffffff))
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .child(initials),
                    )
                    .child(
                        div()
                            .v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id(format!("profile-name-{edit_id}"))
                                            .cursor_pointer()
                                            .text_size(px(15.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(profile.name)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                if let Some(draft) =
                                                    my_vpns::read_profile_draft(&edit_id)
                                                {
                                                    this.open_editor(
                                                        EditorMode::Edit,
                                                        draft,
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            })),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py(px(2.))
                                            .h_flex()
                                            .gap_1()
                                            .rounded(px(4.))
                                            .bg(status_color.opacity(0.1))
                                            .text_color(status_color)
                                            .text_size(px(10.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(
                                                div()
                                                    .size(px(5.))
                                                    .rounded(px(999.))
                                                    .bg(status_color),
                                            )
                                            .child(self.t(status_key)),
                                    ),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(metadata),
                            )
                            .when_some(detail, |this, detail| {
                                this.child(
                                    div()
                                        .text_size(px(11.))
                                        .font_family("monospace")
                                        .text_color(cx.theme().muted_foreground)
                                        .child(detail),
                                )
                            }),
                    ),
            )
            .child(
                div().h_flex().flex_none().gap_2().child(
                    Button::new(format!("toggle-{profile_id}"))
                        .when(active, |button| button.danger().outline())
                        .when(!active, |button| button.primary())
                        .disabled(status == VpnStatus::Connecting)
                        .label(if active {
                            self.t("profiles.killLink")
                        } else {
                            self.t("profiles.bringUp")
                        })
                        .on_click(
                            cx.listener(move |this, _, _, _| this.toggle_profile(&profile_id)),
                        ),
                ),
            )
    }

    fn render_console(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_none()
            .h(px(112.))
            .v_flex()
            .rounded(px(7.))
            .border_1()
            .border_color(rgb(SIDEBAR_BORDER))
            .bg(rgb(CONSOLE_BG))
            .child(
                div()
                    .h(px(38.))
                    .h_flex()
                    .justify_between()
                    .px_3()
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8f9590))
                            .child(
                                Icon::new(IconName::SquareTerminal).text_color(rgb(SIDEBAR_MUTED)),
                            )
                            .child(self.t("console.liveTitle")),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_1()
                            .text_size(px(10.))
                            .text_color(cx.theme().success)
                            .child(div().size(px(5.)).rounded(px(999.)).bg(cx.theme().success))
                            .child(self.t("console.receiving")),
                    ),
            )
            .child(
                div()
                    .id("console-lines")
                    .h(px(74.))
                    .overflow_y_scrollbar()
                    .border_t_1()
                    .border_color(rgb(SIDEBAR_BORDER))
                    .px_3()
                    .py_2()
                    .v_flex()
                    .gap_1()
                    .font_family("monospace")
                    .text_size(px(10.))
                    .children(if self.logs.is_empty() {
                        vec![div()
                            .text_color(rgb(SIDEBAR_MUTED))
                            .child(self.t("console.emptyCompact"))]
                    } else {
                        self.logs
                            .iter()
                            .rev()
                            .take(3)
                            .rev()
                            .map(|line| {
                                let lower = line.to_lowercase();
                                let color = if lower.contains("error")
                                    || lower.contains("failed")
                                    || line.contains('✗')
                                {
                                    cx.theme().danger
                                } else if line.contains('→')
                                    || line.contains('↻')
                                    || line.contains('✓')
                                {
                                    cx.theme().success
                                } else {
                                    rgb(SIDEBAR_MUTED).into()
                                };
                                div().text_color(color).child(line.clone())
                            })
                            .collect()
                    }),
            )
    }

    fn render_boot_fault(&self, error: String, cx: &mut Context<Self>) -> Div {
        centered_page(cx).child(
            surface(cx)
                .w(px(460.))
                .v_flex()
                .gap_4()
                .p_6()
                .child(
                    div()
                        .size(px(52.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(14.))
                        .bg(cx.theme().danger.opacity(0.12))
                        .text_color(cx.theme().danger)
                        .child(Icon::new(IconName::TriangleAlert).size(px(26.))),
                )
                .child(
                    div()
                        .text_size(px(22.))
                        .font_weight(FontWeight::BOLD)
                        .child(self.t("boot.fault")),
                )
                .child(div().text_color(cx.theme().danger).child(error))
                .child(
                    Button::new("retry-boot")
                        .primary()
                        .icon(IconName::RotateCw)
                        .label(self.t("boot.retry"))
                        .on_click(cx.listener(|this, _, _, _| {
                            this.boot_error = None;
                            let status = get_dependency_status();
                            this.deps_ready = status.client_installed;
                            this.deps = Some(status);
                        })),
                ),
        )
    }

    fn render_setup(&self, status: DependencyStatus, cx: &mut Context<Self>) -> Div {
        let error = self.editor_error.clone();
        let logs = self.install_logs.clone();
        centered_page(cx).child(
            surface(cx)
                .w(px(600.))
                .max_h(px(600.))
                .v_flex()
                .gap_4()
                .p_6()
                .child(
                    div()
                        .h_flex()
                        .gap_3()
                        .child(
                            div()
                                .size(px(54.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(15.))
                                .bg(cx.theme().primary.opacity(0.12))
                                .text_color(cx.theme().primary)
                                .child(Icon::new(IconName::ArrowDown).size(px(27.))),
                        )
                        .child(
                            div()
                                .v_flex()
                                .child(
                                    div()
                                        .text_size(px(22.))
                                        .font_weight(FontWeight::BOLD)
                                        .child(self.t("setup.title")),
                                )
                                .child(
                                    div()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(self.t("setup.missing")),
                                ),
                        ),
                )
                .child(
                    div()
                        .v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(status.engine.clone()),
                        )
                        .child(self.tv("setup.needsClient", &[("engine", status.engine.clone())]))
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(status.distro.pretty.clone()),
                        ),
                )
                .child(
                    div()
                        .v_flex()
                        .gap_2()
                        .p_4()
                        .rounded(px(12.))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted.opacity(0.35))
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child(self.tv(
                                    "setup.installPlan",
                                    &[("family", status.distro.family.clone())],
                                )),
                        )
                        .child(
                            div().font_family("monospace").text_size(px(12.)).child(
                                status
                                    .install_command
                                    .clone()
                                    .unwrap_or_else(|| self.t("setup.noAutoInstall")),
                            ),
                        ),
                )
                .when_some(error, |this, error| {
                    this.child(
                        div()
                            .p_3()
                            .rounded(px(10.))
                            .bg(cx.theme().danger.opacity(0.1))
                            .text_color(cx.theme().danger)
                            .child(error),
                    )
                })
                .when(!logs.is_empty(), |this| {
                    this.child(
                        div()
                            .h(px(110.))
                            .overflow_y_scrollbar()
                            .rounded(px(10.))
                            .bg(cx.theme().background)
                            .p_3()
                            .font_family("monospace")
                            .text_size(px(11.))
                            .children(logs.into_iter().map(|line| div().child(line))),
                    )
                })
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            Button::new("install-client")
                                .primary()
                                .disabled(!status.can_auto_install || self.setup_busy)
                                .loading(self.setup_busy)
                                .icon(IconName::ArrowDown)
                                .label(if self.setup_busy {
                                    self.t("setup.working")
                                } else {
                                    self.t("setup.installNow")
                                })
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.setup_busy = true;
                                    this.install_logs.clear();
                                    this.editor_error = None;
                                    let tx = this.setup_tx.clone();
                                    std::thread::spawn(move || {
                                        let result = install_vpn_client(|line| {
                                            let _ = tx.send(SetupMsg::Log(line.to_string()));
                                        });
                                        let _ = tx.send(SetupMsg::Done(Box::new(result)));
                                    });
                                })),
                        )
                        .child(
                            Button::new("recheck-client")
                                .disabled(self.setup_busy)
                                .icon(IconName::RotateCw)
                                .label(self.t("setup.recheck"))
                                .on_click(cx.listener(|this, _, _, _| {
                                    let status = get_dependency_status();
                                    if status.client_installed {
                                        this.deps = Some(status);
                                        this.deps_ready = true;
                                        this.profiles = this.vpn.lock().unwrap().refresh_profiles();
                                    } else {
                                        this.editor_error = Some(this.t("setup.stillMissing"));
                                    }
                                })),
                        ),
                ),
        )
    }

    fn render_editor_overlay(&self, cx: &mut Context<Self>) -> Option<Div> {
        let editor = self.editor.clone()?;
        let title = self.t(match editor.mode {
            EditorMode::Create => "form.createTitle",
            EditorMode::Edit => "form.editTitle",
            EditorMode::Import => "form.importTitle",
        });
        let error = self.editor_error.clone();
        let edit_profile_id =
            (editor.mode == EditorMode::Edit).then(|| editor.id.read(cx).value().to_string());
        Some(
            overlay().child(
                surface(cx)
                    .w(px(700.))
                    .h(relative(0.9))
                    .max_h(relative(0.9))
                    .v_flex()
                    .overflow_hidden()
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .px_5()
                            .py_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(20.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child("openfortivpn · .conf"),
                                    ),
                            )
                            .child(
                                Button::new("close-editor")
                                    .ghost()
                                    .icon(IconName::Close)
                                    .tooltip(self.t("form.cancel"))
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.editor = None;
                                        this.editor_error = None;
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("editor-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .v_flex()
                            .gap_5()
                            .p_5()
                            .when(!editor.extra_options.is_empty(), |this| {
                                this.child(
                                    div()
                                        .p_3()
                                        .rounded(px(10.))
                                        .bg(cx.theme().warning.opacity(0.1))
                                        .text_color(cx.theme().warning)
                                        .text_size(px(12.))
                                        .child(self.tv(
                                            "form.extraOptions",
                                            &[("count", editor.extra_options.len().to_string())],
                                        )),
                                )
                            })
                            .child(form_section(
                                self.t("form.sectionConn"),
                                vec![
                                    field(
                                        self.t("form.id"),
                                        self.t("form.idHint"),
                                        &editor.id,
                                        editor.mode == EditorMode::Edit,
                                        cx,
                                    ),
                                    field(
                                        self.t("form.host"),
                                        "vpn.example.com".into(),
                                        &editor.host,
                                        false,
                                        cx,
                                    ),
                                    field(
                                        self.t("form.port"),
                                        "10443".into(),
                                        &editor.port,
                                        false,
                                        cx,
                                    ),
                                    field(
                                        self.t("form.realm"),
                                        self.t("form.optional"),
                                        &editor.realm,
                                        false,
                                        cx,
                                    ),
                                ],
                                cx,
                            ))
                            .child(form_section(
                                self.t("form.sectionAuth"),
                                vec![
                                    field(
                                        self.t("form.username"),
                                        self.t("form.optional"),
                                        &editor.username,
                                        false,
                                        cx,
                                    ),
                                    password_field(
                                        self.t("form.password"),
                                        self.t("form.passwordHint"),
                                        &editor.password,
                                        cx,
                                    ),
                                    field_wide(
                                        self.t("form.trustedCert"),
                                        self.t("form.trustedCertHint"),
                                        &editor.trusted_cert,
                                        false,
                                        cx,
                                    ),
                                ],
                                cx,
                            ))
                            .child(
                                form_section(
                                    self.t("form.sectionOpts"),
                                    vec![
                                        field(
                                            self.t("form.healthHost"),
                                            self.t("form.optional"),
                                            &editor.health_host,
                                            false,
                                            cx,
                                        ),
                                        field(
                                            self.t("form.healthPort"),
                                            "0".into(),
                                            &editor.health_port,
                                            false,
                                            cx,
                                        ),
                                        field(
                                            self.t("form.persistent"),
                                            self.t("form.persistentHint"),
                                            &editor.persistent,
                                            false,
                                            cx,
                                        ),
                                    ],
                                    cx,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap_4()
                                        .pt_3()
                                        .child(editor_switch(
                                            "set-routes",
                                            self.t("form.setRoutes"),
                                            editor.set_routes,
                                            cx.listener(|this, next, _, _| {
                                                if let Some(editor) = this.editor.as_mut() {
                                                    editor.set_routes = *next;
                                                }
                                            }),
                                        ))
                                        .child(editor_switch(
                                            "set-dns",
                                            self.t("form.setDns"),
                                            editor.set_dns,
                                            cx.listener(|this, next, _, _| {
                                                if let Some(editor) = this.editor.as_mut() {
                                                    editor.set_dns = *next;
                                                }
                                            }),
                                        ))
                                        .child(editor_switch(
                                            "no-dtls",
                                            self.t("form.noDtls"),
                                            editor.no_dtls,
                                            cx.listener(|this, next, _, _| {
                                                if let Some(editor) = this.editor.as_mut() {
                                                    editor.no_dtls = *next;
                                                }
                                            }),
                                        ))
                                        .child(editor_switch(
                                            "legacy-tunnel",
                                            self.t("form.legacyTunnel"),
                                            editor.legacy_tunnel,
                                            cx.listener(|this, next, _, _| {
                                                if let Some(editor) = this.editor.as_mut() {
                                                    editor.legacy_tunnel = *next;
                                                }
                                            }),
                                        )),
                                ),
                            )
                            .when_some(error, |this, error| {
                                this.child(
                                    div()
                                        .p_3()
                                        .rounded(px(10.))
                                        .bg(cx.theme().danger.opacity(0.1))
                                        .text_color(cx.theme().danger)
                                        .child(error),
                                )
                            }),
                    )
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .gap_2()
                            .px_5()
                            .py_4()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .child(div().when_some(edit_profile_id, |this, id| {
                                this.child(
                                    Button::new("delete-editor-profile")
                                        .danger()
                                        .outline()
                                        .icon(IconName::Delete)
                                        .label(self.t("profiles.delete"))
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            this.editor = None;
                                            this.confirm_delete = Some(id.clone());
                                        })),
                                )
                            }))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("cancel-editor")
                                            .label(self.t("form.cancel"))
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.editor = None;
                                                this.editor_error = None;
                                            })),
                                    )
                                    .child(
                                        Button::new("save-editor")
                                            .primary()
                                            .icon(IconName::Check)
                                            .label(self.t("form.save"))
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.save_editor(cx)),
                                            ),
                                    ),
                            ),
                    ),
            ),
        )
    }

    fn render_delete_overlay(&self, cx: &mut Context<Self>) -> Option<Div> {
        let id = self.confirm_delete.clone()?;
        Some(
            overlay().child(
                surface(cx)
                    .w(px(430.))
                    .v_flex()
                    .gap_4()
                    .p_5()
                    .child(
                        div()
                            .size(px(46.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(13.))
                            .bg(cx.theme().danger.opacity(0.12))
                            .text_color(cx.theme().danger)
                            .child(Icon::new(IconName::Delete).size(px(23.))),
                    )
                    .child(
                        div()
                            .text_size(px(19.))
                            .font_weight(FontWeight::BOLD)
                            .child(self.t("profiles.delete")),
                    )
                    .child(self.tv("profiles.deleteConfirm", &[("id", id.clone())]))
                    .child(
                        div()
                            .h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-delete")
                                    .label(self.t("form.cancel"))
                                    .on_click(
                                        cx.listener(|this, _, _, _| this.confirm_delete = None),
                                    ),
                            )
                            .child(
                                Button::new("confirm-delete")
                                    .danger()
                                    .icon(IconName::Delete)
                                    .label(self.t("profiles.delete"))
                                    .on_click(cx.listener(move |this, _, _, _| {
                                        this.vpn.lock().unwrap().disconnect(Some(&id));
                                        let result = delete_profile_file(&id);
                                        if result.ok {
                                            this.profiles =
                                                this.vpn.lock().unwrap().refresh_profiles();
                                        } else {
                                            this.editor_error = Some(result.message);
                                        }
                                        this.confirm_delete = None;
                                    })),
                            ),
                    ),
            ),
        )
    }

    fn render_quit_overlay(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.confirm_quit {
            return None;
        }
        Some(
            overlay().child(
                surface(cx)
                    .w(px(430.))
                    .v_flex()
                    .gap_4()
                    .p_5()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::BOLD)
                            .child(self.t("ops.quitConfirm")),
                    )
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.t("ops.quitConfirmBody")),
                    )
                    .child(
                        div()
                            .h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-quit")
                                    .label(self.t("form.cancel"))
                                    .on_click(
                                        cx.listener(|this, _, _, _| this.confirm_quit = false),
                                    ),
                            )
                            .child(
                                Button::new("confirm-quit")
                                    .danger()
                                    .label(self.t("ops.quit"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_quit = false;
                                        this.request_quit(window, cx);
                                    })),
                            ),
                    ),
            ),
        )
    }
}

impl Render for Desk {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.theme_applied {
            let theme = self.theme.clone();
            apply_theme(&theme, window, cx);
            self.theme_applied = true;
        }
        let content = if let Some(error) = self.boot_error.clone() {
            self.render_boot_fault(error, cx)
        } else if !self.deps_ready {
            match self.deps.clone() {
                Some(status) => self.render_setup(status, cx),
                None => centered_page(cx).child(self.t("boot.sequence")),
            }
        } else {
            div()
                .flex_1()
                .min_h_0()
                .h_flex()
                .child(self.render_sidebar(cx))
                .child(self.render_workspace(cx))
        };

        div()
            .relative()
            .size_full()
            .v_flex()
            .overflow_hidden()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_title_bar(cx))
            .child(content)
            .children(self.render_editor_overlay(cx))
            .children(self.render_preferences_overlay(cx))
            .children(self.render_delete_overlay(cx))
            .children(self.render_quit_overlay(cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

impl ProfileEditor {
    fn from_draft(
        mode: EditorMode,
        draft: VpnProfileDraft,
        window: &mut Window,
        cx: &mut Context<Desk>,
    ) -> Self {
        let input = |value: String, window: &mut Window, cx: &mut Context<Desk>| {
            cx.new(|cx| InputState::new(window, cx).default_value(value))
        };
        Self {
            mode,
            id: input(draft.id, window, cx),
            host: input(draft.host, window, cx),
            port: input(draft.port.to_string(), window, cx),
            username: input(draft.username, window, cx),
            password: cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .default_value(draft.password)
            }),
            trusted_cert: input(draft.trusted_cert, window, cx),
            realm: input(draft.realm, window, cx),
            persistent: input(draft.persistent.to_string(), window, cx),
            health_host: input(draft.health_host.unwrap_or_default(), window, cx),
            health_port: input(
                draft
                    .health_port
                    .map(|port| port.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            ),
            set_dns: draft.set_dns,
            set_routes: draft.set_routes,
            no_dtls: draft.no_dtls,
            legacy_tunnel: draft.legacy_tunnel,
            extra_options: draft.extra_options,
        }
    }

    fn to_draft(&self, cx: &App) -> VpnProfileDraft {
        let value = |input: &Entity<InputState>| input.read(cx).value().to_string();
        VpnProfileDraft {
            id: value(&self.id),
            host: value(&self.host),
            port: value(&self.port).parse().unwrap_or(0),
            username: value(&self.username),
            password: value(&self.password),
            trusted_cert: value(&self.trusted_cert),
            realm: value(&self.realm),
            persistent: value(&self.persistent).parse().unwrap_or(0),
            health_host: nonempty(value(&self.health_host)),
            health_port: value(&self.health_port)
                .parse::<u16>()
                .ok()
                .filter(|port| *port > 0),
            set_dns: self.set_dns,
            set_routes: self.set_routes,
            no_dtls: self.no_dtls,
            legacy_tunnel: self.legacy_tunnel,
            extra_options: self.extra_options.clone(),
        }
    }
}

fn nonempty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn profile_initials(name: &str) -> String {
    let initials = name
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    if initials.is_empty() {
        "VPN".into()
    } else {
        initials
    }
}

fn profile_mark_color(name: &str) -> u32 {
    const COLORS: [u32; 3] = [0xa94021, 0x534488, 0x305977];
    let index = name
        .bytes()
        .fold(0usize, |total, byte| total.wrapping_add(byte as usize))
        % COLORS.len();
    COLORS[index]
}

fn apply_theme(_theme: &str, window: &mut Window, cx: &mut App) {
    Theme::change(ThemeMode::Dark, Some(window), cx);
    let brand = rgb(BRAND).into();
    let brand_hover = rgb(BRAND_HOVER).into();
    let brand_active = rgb(BRAND_ACTIVE).into();
    let white: Hsla = rgb(0xffffff).into();
    let theme = Theme::global_mut(cx);
    theme.background = rgb(0x141616).into();
    theme.foreground = rgb(0xf4f5f1).into();
    theme.border = rgb(0x2a2d2c).into();
    theme.secondary = rgb(0x191b1b).into();
    theme.secondary_hover = rgb(0x202323).into();
    theme.secondary_active = rgb(0x272a29).into();
    theme.muted = rgb(0x202323).into();
    theme.muted_foreground = rgb(0x8f9590).into();
    theme.input = rgb(0x303332).into();
    theme.popover = rgb(0x191b1b).into();
    theme.popover_foreground = rgb(0xf4f5f1).into();
    theme.button = rgb(0x1b1e1d).into();
    theme.button_hover = rgb(0x242726).into();
    theme.button_active = rgb(0x2b2e2d).into();
    theme.button_foreground = rgb(0xe9ebe7).into();
    theme.title_bar = rgb(0x181a1a).into();
    theme.title_bar_border = rgb(0x292c2b).into();

    theme.sidebar = rgb(SIDEBAR_BG).into();
    theme.sidebar_foreground = rgb(SIDEBAR_TEXT).into();
    theme.sidebar_border = rgb(SIDEBAR_BORDER).into();
    theme.sidebar_accent = rgb(SIDEBAR_RAISED).into();
    theme.sidebar_accent_foreground = rgb(SIDEBAR_TEXT).into();
    theme.sidebar_primary = brand;
    theme.sidebar_primary_foreground = white;
    theme.success = rgb(0x29b47a).into();
    theme.success_hover = rgb(0x239b69).into();
    theme.success_active = rgb(0x1d8359).into();
    theme.warning = rgb(0xe2a13a).into();
    theme.danger = rgb(0xd65b52).into();
    theme.ring = brand;
    theme.selection = brand.opacity(0.24);
    theme.primary = brand;
    theme.primary_foreground = white;
    theme.primary_hover = brand_hover;
    theme.primary_active = brand_active;
    theme.button_primary = brand;
    theme.button_primary_foreground = white;
    theme.button_primary_hover = brand_hover;
    theme.button_primary_active = brand_active;
    theme.tokens.primary = brand.into();
    theme.tokens.primary_hover = brand_hover.into();
    theme.tokens.primary_active = brand_active.into();
    theme.tokens.button_primary = brand.into();
    theme.tokens.button_primary_hover = brand_hover.into();
    theme.tokens.button_primary_active = brand_active.into();
    theme.tokens.button_primary_foreground = white.into();
    theme.radius = px(6.);
    theme.radius_lg = px(10.);
    theme.shadow = true;
    Theme::sync_base(cx);
    window.refresh();
}

fn brand_mark(cx: &App, size: Pixels) -> Div {
    div()
        .size(size)
        .flex_none()
        .rounded(size * 0.28)
        .overflow_hidden()
        .child(
            img(my_vpns::app_icon::cached_icon_png())
                .size_full()
                .object_fit(ObjectFit::Contain),
        )
        .border_1()
        .border_color(cx.theme().primary.opacity(0.55))
}

fn surface(cx: &App) -> Div {
    div()
        .rounded(px(16.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .shadow_lg()
}

fn sidebar_section_label(text: String) -> Div {
    div()
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(SIDEBAR_MUTED))
        .child(text.to_uppercase())
}

fn centered_page(cx: &App) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .p_6()
        .bg(cx.theme().muted.opacity(0.18))
}

fn overlay() -> Div {
    div()
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .left_0()
        .flex()
        .items_center()
        .justify_center()
        .p_5()
        .bg(rgba(0x00000088))
}

fn form_section(title: String, fields: Vec<Div>, cx: &App) -> Div {
    div()
        .v_flex()
        .gap_3()
        .child(
            div()
                .h_flex()
                .gap_2()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(div().h(px(1.)).flex_1().bg(cx.theme().border)),
        )
        .child(div().flex().flex_wrap().gap_3().children(fields))
}

fn field(label: String, hint: String, input: &Entity<InputState>, disabled: bool, cx: &App) -> Div {
    div()
        .w(relative(0.48))
        .min_w(px(250.))
        .v_flex()
        .gap_1()
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(Input::new(input).disabled(disabled))
        .when(!hint.is_empty(), |this| {
            this.child(
                div()
                    .text_size(px(10.))
                    .text_color(cx.theme().muted_foreground.opacity(0.72))
                    .child(hint),
            )
        })
}

fn field_wide(
    label: String,
    hint: String,
    input: &Entity<InputState>,
    disabled: bool,
    cx: &App,
) -> Div {
    field(label, hint, input, disabled, cx).w_full()
}

fn password_field(label: String, hint: String, input: &Entity<InputState>, cx: &App) -> Div {
    div()
        .w(relative(0.48))
        .min_w(px(250.))
        .v_flex()
        .gap_1()
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(Input::new(input).mask_toggle())
        .child(
            div()
                .text_size(px(10.))
                .text_color(cx.theme().muted_foreground.opacity(0.72))
                .child(hint),
        )
}

fn editor_switch(
    id: &'static str,
    label: String,
    checked: bool,
    listener: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> Switch {
    Switch::new(id)
        .checked(checked)
        .label(label)
        .on_click(listener)
}

fn format_duration(connected_at: u128) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(connected_at);
    let seconds = now.saturating_sub(connected_at) / 1000;
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn notify(title: &str, body: &str) {
    my_vpns::os_ui::send_notification(title, body);
}

fn pick_conf_file() -> Option<PathBuf> {
    my_vpns::os_ui::pick_conf_file()
}

#[cfg(target_os = "linux")]
fn linux_tray_pixmaps() -> Vec<ksni::Icon> {
    let mut icons = Vec::new();
    for bytes in [
        my_vpns::app_icon::APP_ICON_PNG_32,
        my_vpns::app_icon::APP_ICON_PNG,
    ] {
        if let Some((width, height, data)) = my_vpns::app_icon::png_argb_pixmap(bytes) {
            icons.push(ksni::Icon {
                width,
                height,
                data,
            });
        }
    }
    icons
}

fn spawn_tray(
    vpn: Arc<Mutex<VpnManager>>,
    locale: String,
    tx: mpsc::Sender<TrayCmd>,
    _quitting: Arc<Mutex<bool>>,
) {
    #[cfg(target_os = "linux")]
    {
        let tray = AppTray { vpn, locale, tx };
        std::thread::spawn(move || {
            use ksni::blocking::TrayMethods;
            let _ = tray.spawn();
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (vpn, locale, tx);
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn os_tray_menu(
    locale: &str,
    profiles: &[VpnProfile],
    state: &VpnState,
) -> Option<tray_icon::menu::Menu> {
    use my_vpns::tray_menu::{build_tray_menu, TrayEntry};
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
    let menu = Menu::new();
    for entry in build_tray_menu(locale, profiles, state) {
        let result = match entry {
            TrayEntry::Separator => menu.append(&PredefinedMenuItem::separator()).ok(),
            TrayEntry::Action { id, label } => {
                let item = MenuItem::with_id(id, label, true, None);
                menu.append(&item).ok()
            }
            TrayEntry::Profile { profile_id, label } => {
                let item = MenuItem::with_id(format!("profile:{profile_id}"), label, true, None);
                menu.append(&item).ok()
            }
        };
        result?;
    }
    Some(menu)
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn create_os_tray(locale: &str, vpn: &VpnManager) -> Option<tray_icon::TrayIcon> {
    let menu = os_tray_menu(locale, &vpn.get_profiles(), &vpn.get_state())?;
    let icon = os_tray_icon()?;
    tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("My VPNs")
        .with_icon(icon)
        .build()
        .ok()
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn os_tray_icon() -> Option<tray_icon::Icon> {
    let image = image::load_from_memory(my_vpns::app_icon::APP_ICON_PNG)
        .ok()?
        .into_rgba8();
    let image = image::imageops::resize(&image, 32, 32, image::imageops::FilterType::Triangle);
    let (width, height) = image.dimensions();
    tray_icon::Icon::from_rgba(image.into_raw(), width, height).ok()
}

#[cfg(target_os = "linux")]
struct AppTray {
    vpn: Arc<Mutex<VpnManager>>,
    locale: String,
    tx: mpsc::Sender<TrayCmd>,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for AppTray {
    fn id(&self) -> String {
        my_vpns::APP_ID.into()
    }

    fn title(&self) -> String {
        "My VPNs".into()
    }

    fn icon_name(&self) -> String {
        "my-vpns".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        linux_tray_pixmaps()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayCmd::Show);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        use my_vpns::tray_menu::{build_tray_menu, TrayEntry};
        let (profiles, state) = if let Ok(vpn) = self.vpn.lock() {
            (vpn.get_profiles(), vpn.get_state())
        } else {
            return Vec::new();
        };
        let mut items = Vec::new();
        for entry in build_tray_menu(&self.locale, &profiles, &state) {
            match entry {
                TrayEntry::Separator => items.push(MenuItem::Separator),
                TrayEntry::Action { id, label } => {
                    let tx = self.tx.clone();
                    let command = match id {
                        "show" => TrayCmd::Show,
                        "quit" => TrayCmd::Quit,
                        "refresh" => TrayCmd::Refresh,
                        "check_updates" => TrayCmd::CheckUpdates,
                        "disconnect_all" => TrayCmd::DisconnectAll,
                        _ => continue,
                    };
                    items.push(
                        StandardItem {
                            label,
                            activate: Box::new(move |_: &mut Self| {
                                let _ = tx.send(command.clone());
                            }),
                            ..Default::default()
                        }
                        .into(),
                    );
                }
                TrayEntry::Profile { profile_id, label } => {
                    let tx = self.tx.clone();
                    items.push(
                        StandardItem {
                            label,
                            activate: Box::new(move |_: &mut Self| {
                                let _ = tx.send(TrayCmd::Toggle(profile_id.clone()));
                            }),
                            ..Default::default()
                        }
                        .into(),
                    );
                }
            }
        }
        items
    }
}

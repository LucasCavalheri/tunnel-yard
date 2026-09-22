//! Native GPUI Kit desktop shell.
//!
//! VPN processes, profile persistence and tray behavior remain in the domain
//! modules. This module owns only presentation and interaction.

use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    scroll::ScrollableElement as _,
    switch::Switch,
    ActiveTheme, Disableable, Icon, IconNamed, Root, Selectable, Sizable, StyledExt as _, Theme,
    ThemeMode, TitleBar,
};
use gpui_kit::{prelude::*, *};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tunnel_yard::autostart::{get_autostart_path, is_autostart_enabled, set_autostart_enabled};
use tunnel_yard::conf::{
    delete_profile_file, draft_from_imported_file, empty_draft, save_profile_draft, VpnProfileDraft,
};
use tunnel_yard::deps::{
    get_dependency_status, install_vpn_client, DependencyStatus, InstallResult,
};
use tunnel_yard::desktop::{EDITOR_FIELDS, SETUP_GATE_KEYS, TRAY_MENU_KEYS, UI_SURFACES};
use tunnel_yard::i18n::translate;
use tunnel_yard::icons::{Huge, LocaleFlag};
use tunnel_yard::modal_layout::preferences_modal_frame;
use tunnel_yard::settings::{load_settings, normalize_theme, save_settings, AppSettingsPatch};
use tunnel_yard::theme::{
    resolve_theme_mode, Palette, BRAND, BRAND_ACTIVE, BRAND_HOVER, DARK_BRAND, DARK_BRAND_ACTIVE,
    DARK_BRAND_HOVER,
};
use tunnel_yard::updates::{
    next_check_delay_ms, perform_update_check, perform_update_install, UpdateApplyResult,
    UpdateCheckResult, UpdateInfo, FIRST_CHECK_DELAY_MS,
};
use tunnel_yard::vpn::{
    summarize_vpn_state, VpnEvent, VpnManager, VpnProfile, VpnState, VpnStatus,
};

const SIDEBAR_WIDTH: Pixels = px(256.);

#[derive(Clone, Copy)]
struct Hi(Huge);

impl IconNamed for Hi {
    fn path(self) -> SharedString {
        SharedString::from(self.0.asset_path())
    }
}

fn hi(icon: Huge) -> Icon {
    Icon::new(Hi(icon))
}

#[derive(Clone, Copy)]
struct FlagAsset(LocaleFlag);

impl IconNamed for FlagAsset {
    fn path(self) -> SharedString {
        SharedString::from(self.0.asset_path())
    }
}

fn flag(icon: LocaleFlag) -> Icon {
    Icon::new(FlagAsset(icon))
}

struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = tunnel_yard::icons::svg_bytes(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut items = gpui_kit::assets::Assets.list(path)?;
        for name in tunnel_yard::icons::paths() {
            if name.starts_with(path) {
                items.push(SharedString::from(name));
            }
        }
        Ok(items)
    }
}

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
    confirm_update: bool,
    update_busy: bool,
    update_error: Option<String>,
    vpn: Arc<Mutex<VpnManager>>,
    events: Receiver<VpnEvent>,
    last_connected: std::collections::HashSet<String>,
    setup_busy: bool,
    quitting: Arc<Mutex<bool>>,
    tray_rx: Option<Receiver<TrayCmd>>,
    #[cfg(target_os = "linux")]
    linux_tray: Option<ksni::blocking::Handle<AppTray>>,
    update: Option<UpdateInfo>,
    check_feedback: CheckFeedback,
    update_rx: Receiver<UpdateCheckResult>,
    update_tx: mpsc::Sender<UpdateCheckResult>,
    update_install_rx: Receiver<Result<UpdateApplyResult, String>>,
    update_install_tx: mpsc::Sender<Result<UpdateApplyResult, String>>,
    next_update_check_at: Instant,
    update_check_in_flight: bool,
    update_check_failures: u32,
    dismissed_update: Option<String>,
    setup_rx: Receiver<SetupMsg>,
    setup_tx: mpsc::Sender<SetupMsg>,
    search: Entity<InputState>,
    applied_mode: Option<String>,
    preferences_open: bool,
    console_expanded: bool,
}

pub fn run(hidden: bool) -> Result<(), String> {
    let _ = tunnel_yard::app_icon::ensure_app_icon_files();
    #[cfg(target_os = "linux")]
    let _ = tunnel_yard::app_icon::ensure_linux_desktop_entry();

    gpui_kit::application()
        .with_assets(AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let settings = load_settings();
            let initial_theme = normalize_theme(&settings.theme).to_string();
            let icon = image::load_from_memory(tunnel_yard::app_icon::APP_ICON_PNG)
                .ok()
                .map(|image| Arc::new(image.into_rgba8()));
            let mut options = TitleBar::window_options();
            options.window_bounds = Some(WindowBounds::centered(size(px(1120.), px(740.)), cx));
            options.window_min_size = Some(size(px(900.), px(620.)));
            options.app_id = Some(tunnel_yard::APP_ID.into());
            options.show = !hidden;
            options.icon = icon;

            let locale = settings.locale;
            let dismissed_update = settings.dismissed_update_version;
            let auto_reconnect = settings.auto_reconnect;
            cx.spawn(async move |cx| {
                cx.open_window(options, move |window, cx| {
                    apply_theme(&initial_theme, window, cx);
                    let desk = cx.new(|cx| {
                        Desk::new(
                            locale,
                            initial_theme,
                            dismissed_update,
                            auto_reconnect,
                            window,
                            cx,
                        )
                    });
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
                .map_err(|error| eprintln!("[tunnel-yard] failed to open window: {error}"))
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
        #[cfg(target_os = "linux")]
        if let Some(handle) = self.linux_tray.take() {
            handle.shutdown();
        }
    }
}

impl Desk {
    fn new(
        locale: String,
        theme: String,
        dismissed_update: Option<String>,
        auto_reconnect: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (vpn, events) = VpnManager::subscribe();
        vpn.set_auto_reconnect(auto_reconnect);
        let vpn = Arc::new(Mutex::new(vpn));
        let quitting = Arc::new(Mutex::new(false));
        let (tray_tx, tray_rx) = mpsc::channel();
        let (update_tx, update_rx) = mpsc::channel();
        let (update_install_tx, update_install_rx) = mpsc::channel();
        let (setup_tx, setup_rx) = mpsc::channel();
        #[cfg(target_os = "linux")]
        let linux_tray = spawn_tray(vpn.clone(), locale.clone(), tray_tx, quitting.clone());
        #[cfg(not(target_os = "linux"))]
        spawn_tray(vpn.clone(), locale.clone(), tray_tx, quitting.clone());

        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(translate(&locale, "ops.search", &[]))
        });

        let mut desk = Self {
            locale,
            theme,
            version: tunnel_yard::APP_VERSION.into(),
            deps: None,
            deps_ready: false,
            boot_error: None,
            profiles: Vec::new(),
            state: VpnState {
                sessions: Default::default(),
                auto_reconnect,
            },
            logs: Vec::new(),
            install_logs: Vec::new(),
            autostart: is_autostart_enabled(),
            editor: None,
            editor_error: None,
            confirm_delete: None,
            confirm_quit: false,
            confirm_update: false,
            update_busy: false,
            update_error: None,
            vpn,
            events,
            last_connected: Default::default(),
            setup_busy: false,
            quitting,
            tray_rx: Some(tray_rx),
            #[cfg(target_os = "linux")]
            linux_tray,
            update: None,
            check_feedback: CheckFeedback::Idle,
            update_rx,
            update_tx,
            update_install_rx,
            update_install_tx,
            next_update_check_at: Instant::now() + Duration::from_millis(FIRST_CHECK_DELAY_MS),
            update_check_in_flight: false,
            update_check_failures: 0,
            dismissed_update,
            setup_rx,
            setup_tx,
            search,
            applied_mode: None,
            preferences_open: false,
            console_expanded: false,
        };

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

        if std::env::var("TUNNELYARD_SHOT").as_deref() == Ok("editor") {
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

    fn spawn_update_check(&mut self) {
        if self.update_check_in_flight {
            return;
        }
        self.update_check_in_flight = true;
        self.check_feedback = CheckFeedback::Checking;
        let tx = self.update_tx.clone();
        let version = self.version.clone();
        std::thread::spawn(move || {
            let _ = tx.send(perform_update_check(&version));
        });
    }

    fn start_update_install(&mut self) {
        let Some(info) = self.update.clone() else {
            return;
        };
        self.confirm_update = false;
        self.update_busy = true;
        self.update_error = None;
        self.vpn.lock().unwrap().disconnect(None);
        let tx = self.update_install_tx.clone();
        std::thread::spawn(move || {
            let result = perform_update_install(&info);
            let _ = tx.send(result);
        });
    }

    fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        *self.quitting.lock().unwrap() = true;
        self.vpn.lock().unwrap().disconnect(None);
        window.remove_window();
        cx.quit();
    }

    fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.update_check_in_flight && Instant::now() >= self.next_update_check_at {
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
            let failed = matches!(&result, UpdateCheckResult::Error { .. });
            self.update_check_failures = if failed {
                self.update_check_failures.saturating_add(1)
            } else {
                0
            };
            self.next_update_check_at = Instant::now()
                + Duration::from_millis(next_check_delay_ms(Some(0), self.update_check_failures));
            self.update_check_in_flight = false;
            match result {
                UpdateCheckResult::Available(info) => {
                    if self.dismissed_update.as_deref() != Some(info.latest.as_str()) {
                        notify(
                            &self.t("notify.updateTitle"),
                            &self.tv(
                                "notify.updateBody",
                                &[
                                    ("latest", info.latest.clone()),
                                    ("current", info.current.clone()),
                                ],
                            ),
                        );
                        self.update = Some(info);
                    }
                    self.check_feedback = CheckFeedback::Idle;
                }
                UpdateCheckResult::UpToDate { .. } => self.check_feedback = CheckFeedback::UpToDate,
                UpdateCheckResult::Error { .. } => self.check_feedback = CheckFeedback::Error,
            }
        }
        while let Ok(result) = self.update_install_rx.try_recv() {
            self.update_busy = false;
            match result {
                Ok(plan) => {
                    if let Some(path) = plan.relaunch {
                        let _ = std::process::Command::new(&path).spawn();
                    }
                    self.request_quit(window, cx);
                    return;
                }
                Err(message) => {
                    self.update_error = Some(if message == "elevation-declined" {
                        self.t("update.elevationDeclined")
                    } else if message.is_empty() {
                        self.t("update.installFailed")
                    } else {
                        message
                    });
                }
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
                VpnEvent::NeedReconnect(id) => {
                    self.vpn.lock().unwrap().reconnect(&id);
                }
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
                    self.spawn_update_check();
                }
                TrayCmd::DisconnectAll => self.vpn.lock().unwrap().disconnect(None),
                TrayCmd::Toggle(id) => self.toggle_profile(&id),
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
        if let Some(handle) = &self.linux_tray {
            handle.update(|_| ());
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

    fn set_theme(&mut self, theme: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = normalize_theme(theme).into();
        save_settings(AppSettingsPatch {
            theme: Some(self.theme.clone()),
            ..Default::default()
        });
        apply_theme(&self.theme, window, cx);
        self.applied_mode = Some(effective_mode(&self.theme, window).to_string());
        cx.notify();
    }

    fn ensure_theme(&mut self, window: &mut Window, cx: &mut App) {
        let mode = effective_mode(&self.theme, window);
        if self.applied_mode.as_deref() != Some(mode) {
            apply_theme(&self.theme, window, cx);
            self.applied_mode = Some(mode.to_string());
        }
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
                div().h_full().w_full().px_2().flex().items_center().child(
                    div()
                        .flex_1()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(brand_mark(cx, px(18.)))
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child(tunnel_yard::APP_NAME),
                        ),
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
            cx.theme().muted_foreground
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
            .border_color(cx.theme().sidebar_border)
            .bg(cx.theme().sidebar)
            .px_4()
            .py_5()
            .gap_4()
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div().h_flex().gap_3().child(brand_mark(cx, px(38.))).child(
                            div()
                                .v_flex()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(16.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(cx.theme().sidebar_foreground)
                                        .child(tunnel_yard::APP_NAME),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(self.t("brand.subtitle")),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(cx.theme().sidebar_border)
                            .bg(cx.theme().sidebar_accent)
                            .text_size(px(11.))
                            .text_color(cx.theme().muted_foreground)
                            .font_family("monospace")
                            .child(format!("v{}", self.version)),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap_3()
                    .p_4()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(if any_up {
                        cx.theme().success.opacity(0.28)
                    } else {
                        cx.theme().sidebar_border
                    })
                    .bg(if any_up {
                        cx.theme().success.opacity(0.08)
                    } else {
                        cx.theme().sidebar_accent
                    })
                    .child(
                        div()
                            .size(px(40.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(12.))
                            .bg(status_color.opacity(0.14))
                            .text_color(status_color)
                            .child(
                                hi(if any_up { Huge::WifiOn } else { Huge::WifiOff }).size(px(18.)),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .v_flex()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().sidebar_foreground)
                                    .child(connection_label),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if any_up {
                                        self.t("ops.connectionsStable")
                                    } else {
                                        self.t("ops.noneActive")
                                    }),
                            ),
                    )
                    .child(div().size(px(8.)).rounded(px(999.)).bg(status_color).when(
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
                    .child(sidebar_section_label(self.t("ops.quickActions"), cx))
                    .child(
                        Button::new("new-profile")
                            .primary()
                            .w_full()
                            .icon(hi(Huge::Add))
                            .label(self.t("ops.newProfile"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_editor(EditorMode::Create, empty_draft(), window, cx)
                            })),
                    )
                    .child(
                        Button::new("import-profile")
                            .w_full()
                            .outline()
                            .icon(hi(Huge::FileImport))
                            .label(self.t("ops.importConf"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_import(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("check-updates")
                            .w_full()
                            .outline()
                            .icon(hi(Huge::Download))
                            .loading(self.update_check_in_flight)
                            .disabled(self.update_check_in_flight)
                            .label(match self.check_feedback {
                                CheckFeedback::Checking => self.t("update.checking"),
                                _ => self.t("update.checkNow"),
                            })
                            .on_click(cx.listener(|this, _, _, _| {
                                this.spawn_update_check();
                            })),
                    ),
            )
            .child(
                div()
                    .mt_auto()
                    .v_flex()
                    .gap_2()
                    .child(
                        Button::new("open-preferences")
                            .w_full()
                            .ghost()
                            .icon(hi(Huge::Settings))
                            .label(self.t("ops.preferences"))
                            .on_click(cx.listener(|this, _, _, _| {
                                this.preferences_open = true;
                            })),
                    )
                    .child(
                        div()
                            .mt_3()
                            .h_flex()
                            .gap_3()
                            .p_3()
                            .rounded(px(10.))
                            .border_1()
                            .border_color(cx.theme().success.opacity(0.18))
                            .bg(cx.theme().success.opacity(0.06))
                            .child(
                                hi(Huge::ShieldCheck)
                                    .size(px(18.))
                                    .text_color(cx.theme().success),
                            )
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().sidebar_foreground)
                                            .child(self.t("ops.protected")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.t("ops.unprivileged")),
                                    ),
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
            overlay(cx)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, _| this.preferences_open = false),
                )
                .child(
                    preferences_modal_frame(surface(cx))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .h_flex()
                                .items_start()
                                .justify_between()
                                .px_6()
                                .py_5()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div()
                                        .h_flex()
                                        .gap_3()
                                        .child(icon_badge(Huge::Settings, cx.theme().primary, cx))
                                        .child(
                                            div()
                                                .v_flex()
                                                .gap(px(3.))
                                                .child(
                                                    div()
                                                        .text_size(px(20.))
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(self.t("ops.preferences")),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(13.))
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(self.t("ops.preferencesHint")),
                                                ),
                                        ),
                                )
                                .child(
                                    Button::new("close-preferences-top")
                                        .ghost()
                                        .icon(hi(Huge::Close))
                                        .accessibility_label(self.t("form.close"))
                                        .on_click(cx.listener(|this, _, _, _| {
                                            this.preferences_open = false
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .id("preferences-scroll")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scrollbar()
                                .v_flex()
                                .gap_6()
                                .px_6()
                                .py_5()
                                .child(self.render_theme_picker(cx))
                                .child(prefs_section(
                                    Huge::Language,
                                    self.t("ops.language"),
                                    self.t("ops.languageHint"),
                                    div()
                                        .h_flex()
                                        .gap_2()
                                        .child(
                                            Button::new("preferences-locale-pt")
                                                .selected(self.locale == "pt-BR")
                                                .icon(flag(LocaleFlag::Brazil).size(px(20.)))
                                                .label("Português")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.set_locale("pt-BR", window, cx)
                                                })),
                                        )
                                        .child(
                                            Button::new("preferences-locale-en")
                                                .selected(self.locale == "en")
                                                .icon(flag(LocaleFlag::UnitedStates).size(px(20.)))
                                                .label("English")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.set_locale("en", window, cx)
                                                })),
                                        ),
                                    cx,
                                ))
                                .child(prefs_section(
                                    Huge::Wifi,
                                    self.t("ops.connectionSettings"),
                                    self.t("ops.preferencesHint"),
                                    div()
                                        .v_flex()
                                        .gap_2()
                                        .child(pref_switch_row(
                                            Huge::Reload,
                                            self.t("ops.autoRelink"),
                                            self.t("ops.autoRelinkHint"),
                                            Switch::new("preferences-auto-reconnect")
                                                .checked(auto_reconnect)
                                                .on_click(cx.listener(|this, next, _, _| {
                                                    this.vpn
                                                        .lock()
                                                        .unwrap()
                                                        .set_auto_reconnect(*next);
                                                    this.state.auto_reconnect = *next;
                                                    save_settings(AppSettingsPatch {
                                                        auto_reconnect: Some(*next),
                                                        ..Default::default()
                                                    });
                                                })),
                                            cx,
                                        ))
                                        .child(pref_switch_row(
                                            Huge::Laptop,
                                            self.t("ops.startWithLinux"),
                                            self.t("ops.startWithLinuxHint"),
                                            Switch::new("preferences-autostart")
                                                .checked(autostart)
                                                .tooltip(get_autostart_path())
                                                .on_click(cx.listener(|this, next, _, _| {
                                                    if set_autostart_enabled(*next) {
                                                        this.autostart = is_autostart_enabled();
                                                    }
                                                })),
                                            cx,
                                        )),
                                    cx,
                                ))
                                .child(prefs_section(
                                    Huge::Rotate,
                                    self.t("ops.maintenance"),
                                    String::new(),
                                    div()
                                        .v_flex()
                                        .gap_3()
                                        .child(self.render_update_feedback(cx))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_wrap()
                                                .gap_2()
                                                .child(
                                                    Button::new("preferences-reload")
                                                        .icon(hi(Huge::Reload))
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
                                                        .icon(hi(Huge::Download))
                                                        .loading(
                                                            self.check_feedback
                                                                == CheckFeedback::Checking,
                                                        )
                                                        .disabled(self.update_check_in_flight)
                                                        .label(match self.check_feedback {
                                                            CheckFeedback::Checking => {
                                                                self.t("update.checking")
                                                            }
                                                            _ => self.t("update.checkNow"),
                                                        })
                                                        .on_click(cx.listener(|this, _, _, _| {
                                                            this.spawn_update_check();
                                                        })),
                                                )
                                                .child(
                                                    Button::new("preferences-disconnect-all")
                                                        .danger()
                                                        .outline()
                                                        .disabled(!any_up)
                                                        .icon(hi(Huge::WifiOff))
                                                        .label(self.t("ops.killAll"))
                                                        .on_click(cx.listener(|this, _, _, _| {
                                                            this.vpn
                                                                .lock()
                                                                .unwrap()
                                                                .disconnect(None)
                                                        })),
                                                ),
                                        ),
                                    cx,
                                )),
                        )
                        .child(
                            div()
                                .h_flex()
                                .justify_between()
                                .gap_2()
                                .px_6()
                                .py_4()
                                .border_t_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div()
                                        .h_flex()
                                        .gap_2()
                                        .child(
                                            Button::new("preferences-hide")
                                                .ghost()
                                                .icon(hi(Huge::Hide))
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
                                                .icon(hi(Huge::Quit))
                                                .label(self.t("ops.quit"))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.preferences_open = false;
                                                        if any_up {
                                                            this.confirm_quit = true;
                                                        } else {
                                                            this.request_quit(window, cx);
                                                        }
                                                    },
                                                )),
                                        ),
                                )
                                .child(
                                    Button::new("close-preferences")
                                        .primary()
                                        .icon(hi(Huge::Check))
                                        .label(self.t("form.done"))
                                        .on_click(cx.listener(|this, _, _, _| {
                                            this.preferences_open = false
                                        })),
                                ),
                        )
                        .with_animation(
                            "preferences-in",
                            Animation::new(Duration::from_millis(220)),
                            |this, delta| this.opacity(delta),
                        ),
                ),
        )
    }

    fn render_theme_picker(&self, cx: &mut Context<Self>) -> Div {
        let selected = self.theme.as_str();
        prefs_section(
            Huge::Paint,
            self.t("theme.appearance"),
            self.t("theme.appearanceHint"),
            div()
                .h_flex()
                .gap_3()
                .child(self.theme_choice_card(
                    "system",
                    Huge::Laptop,
                    self.t("theme.shortSystem"),
                    self.t("theme.autoHint"),
                    selected == "system",
                    cx,
                ))
                .child(self.theme_choice_card(
                    "light",
                    Huge::Sun,
                    self.t("theme.shortLight"),
                    self.t("theme.lightHint"),
                    selected == "light",
                    cx,
                ))
                .child(self.theme_choice_card(
                    "dark",
                    Huge::Moon,
                    self.t("theme.shortDark"),
                    self.t("theme.darkHint"),
                    selected == "dark",
                    cx,
                )),
            cx,
        )
    }

    fn theme_choice_card(
        &self,
        value: &'static str,
        icon: Huge,
        title: String,
        hint: String,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = format!("theme-choice-{value}");
        div()
            .id(SharedString::from(id.clone()))
            .flex_1()
            .v_flex()
            .gap_2()
            .items_center()
            .px_3()
            .py_4()
            .rounded(px(14.))
            .border_1()
            .border_color(if selected {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .bg(if selected {
                cx.theme().primary.opacity(0.1)
            } else {
                cx.theme().secondary
            })
            .cursor_pointer()
            .hover(|this| this.border_color(cx.theme().primary.opacity(0.7)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| this.set_theme(value, window, cx)),
            )
            .child(
                div()
                    .size(px(36.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.))
                    .bg(if selected {
                        cx.theme().primary.opacity(0.16)
                    } else {
                        cx.theme().muted
                    })
                    .text_color(if selected {
                        cx.theme().primary
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(hi(icon).size(px(18.))),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(hint),
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
                .text_size(px(12.))
                .child(text)
        })
    }

    fn render_workspace(&self, cx: &mut Context<Self>) -> Div {
        let filter_value = self.search.read(cx).value().to_string();
        let filter = filter_value.to_lowercase();
        let summary = summarize_vpn_state(&self.state);
        let active_count = summary.connected_count + summary.connecting_count;
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
            .bg(cx.theme().background)
            .children(self.render_update_banner(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .v_flex()
                    .px_6()
                    .pt_5()
                    .pb_4()
                    .gap_4()
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .gap_5()
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().primary)
                                            .child(self.t("ops.workspace")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(28.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(self.t("ops.tunnels")),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.tv(
                                                "ops.workspaceSummary",
                                                &[
                                                    ("total", self.profiles.len().to_string()),
                                                    ("active", active_count.to_string()),
                                                ],
                                            )),
                                    ),
                            )
                            .child(
                                div().w(px(280.)).flex_none().child(
                                    Input::new(&self.search)
                                        .prefix(hi(Huge::Search))
                                        .cleanable(true),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .child(metric_tile(
                                Huge::Folder,
                                self.profiles.len().to_string(),
                                self.t("ops.totalProfiles"),
                                cx.theme().primary,
                                cx,
                            ))
                            .child(metric_tile(
                                Huge::WifiOn,
                                summary.connected_count.to_string(),
                                self.t("ops.connectedNow"),
                                cx.theme().success,
                                cx,
                            ))
                            .child(metric_tile(
                                Huge::Wifi,
                                summary.connecting_count.to_string(),
                                self.t("ops.connectingNow"),
                                cx.theme().warning,
                                cx,
                            )),
                    )
                    .child(self.render_profiles(visible, &filter_value, cx))
                    .child(self.render_console(cx)),
            )
    }

    fn render_update_banner(&self, cx: &mut Context<Self>) -> Option<Div> {
        let info = self.update.clone()?;
        let error = self.update_error.clone();
        Some(
            div()
                .v_flex()
                .border_b_1()
                .border_color(cx.theme().primary.opacity(0.24))
                .bg(cx.theme().primary.opacity(0.08))
                .child(
                    div()
                        .h_flex()
                        .justify_between()
                        .px_6()
                        .py_3()
                        .child(
                            div()
                                .h_flex()
                                .gap_2()
                                .text_color(cx.theme().primary)
                                .child(hi(Huge::Info))
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
                                    Button::new("install-update")
                                        .small()
                                        .primary()
                                        .icon(hi(Huge::Download))
                                        .loading(self.update_busy)
                                        .disabled(self.update_busy)
                                        .label(if self.update_busy {
                                            self.t("update.installing")
                                        } else {
                                            self.t("update.install")
                                        })
                                        .on_click(cx.listener(|this, _, _, _| {
                                            this.confirm_update = true;
                                        })),
                                )
                                .child(
                                    Button::new("open-release")
                                        .small()
                                        .primary()
                                        .outline()
                                        .icon(hi(Huge::ExternalLink))
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
                                            this.update_error = None;
                                        })),
                                ),
                        ),
                )
                .children(error.map(|message| {
                    div()
                        .px_6()
                        .pb_3()
                        .text_size(px(12.))
                        .text_color(cx.theme().danger)
                        .child(message)
                })),
        )
    }

    fn render_profiles(
        &self,
        visible: Vec<VpnProfile>,
        filter: &str,
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
                    .gap_3()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .child(
                        div()
                            .size(px(48.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(12.))
                            .bg(cx.theme().muted)
                            .text_color(cx.theme().muted_foreground)
                            .child(hi(Huge::Search).size(px(23.))),
                    )
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.t("profiles.noMatchTitle")),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(cx.theme().muted_foreground)
                            .child(self.tv("profiles.noMatch", &[("query", filter.to_string())])),
                    )
                    .child(
                        Button::new("clear-profile-search")
                            .outline()
                            .label(self.t("profiles.clearSearch"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.search
                                    .update(cx, |search, cx| search.set_value("", window, cx));
                            })),
                    ),
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
            .rounded(px(14.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                div()
                    .size(px(56.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(14.))
                    .bg(cx.theme().primary.opacity(0.12))
                    .text_color(cx.theme().primary)
                    .child(hi(Huge::Wifi).size(px(28.))),
            )
            .child(
                div()
                    .text_size(px(22.))
                    .font_weight(FontWeight::BOLD)
                    .child(self.t("profiles.emptyTitle")),
            )
            .child(
                div()
                    .max_w(px(440.))
                    .text_center()
                    .text_size(px(14.))
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
                            .icon(hi(Huge::Add))
                            .label(self.t("ops.newProfile"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_editor(EditorMode::Create, empty_draft(), window, cx)
                            })),
                    )
                    .child(
                        Button::new("empty-import")
                            .icon(hi(Huge::FileImport))
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
        let metadata = format!("{}:{}", profile.host, profile.port);
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
        let username = if profile.username.is_empty() {
            self.t("profiles.noUser")
        } else {
            profile.username.clone()
        };
        let mark_color = profile_mark_color(&profile.name);

        div()
            .flex_none()
            .h_flex()
            .justify_between()
            .gap_5()
            .px_5()
            .py_4()
            .rounded(px(12.))
            .border_1()
            .border_color(if active {
                status_color.opacity(0.32)
            } else {
                cx.theme().border
            })
            .bg(cx.theme().secondary)
            .when(active, |this| this.border_l_2())
            .hover(|this| this.border_color(status_color.opacity(0.45)))
            .child(
                div()
                    .h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_4()
                    .child(
                        div()
                            .size(px(48.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(12.))
                            .bg(rgb(mark_color))
                            .text_color(rgb(0xffffff))
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .child(initials),
                    )
                    .child(
                        div()
                            .v_flex()
                            .min_w_0()
                            .gap(px(3.))
                            .child(
                                div()
                                    .h_flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_size(px(17.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(profile.name),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py(px(3.))
                                            .h_flex()
                                            .gap_1()
                                            .rounded(px(999.))
                                            .bg(status_color.opacity(0.1))
                                            .text_color(status_color)
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(status_dot(status_color, status))
                                            .child(self.t(status_key)),
                                    ),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_2()
                                    .truncate()
                                    .text_size(px(13.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(metadata)
                                    .child("·")
                                    .child(username),
                            )
                            .when_some(detail, |this, detail| {
                                this.child(
                                    div()
                                        .text_size(px(12.))
                                        .font_family("monospace")
                                        .text_color(if status == VpnStatus::Error {
                                            cx.theme().danger
                                        } else if status == VpnStatus::Connected {
                                            cx.theme().success
                                        } else {
                                            cx.theme().muted_foreground
                                        })
                                        .child(detail),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .flex_none()
                    .gap_2()
                    .child(
                        Button::new(format!("edit-{edit_id}"))
                            .small()
                            .ghost()
                            .icon(hi(Huge::Edit))
                            .tooltip(self.t("profiles.edit"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(draft) = tunnel_yard::read_profile_draft(&edit_id) {
                                    this.open_editor(EditorMode::Edit, draft, window, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new(format!("toggle-{profile_id}"))
                            .w(px(132.))
                            .when(active, |button| button.danger().outline())
                            .when(!active, |button| button.primary())
                            .loading(status == VpnStatus::Connecting)
                            .disabled(status == VpnStatus::Connecting)
                            .icon(if active {
                                hi(Huge::WifiOff)
                            } else {
                                hi(Huge::Connect)
                            })
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

    fn render_console(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("live-console")
            .flex_none()
            .h(if self.console_expanded {
                px(176.)
            } else {
                px(48.)
            })
            .v_flex()
            .rounded(px(14.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .h(px(46.))
                    .h_flex()
                    .justify_between()
                    .px_4()
                    .child(
                        div()
                            .h_flex()
                            .gap_3()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child(hi(Huge::Console).text_color(cx.theme().muted_foreground))
                            .child(self.t("console.liveTitle")),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .when(self.console_expanded && !self.logs.is_empty(), |this| {
                                this.child(
                                    Button::new("clear-console")
                                        .small()
                                        .ghost()
                                        .label(self.t("console.clear"))
                                        .on_click(cx.listener(|this, _, _, _| this.logs.clear())),
                                )
                            })
                            .child(
                                Button::new("toggle-console")
                                    .small()
                                    .ghost()
                                    .icon(if self.console_expanded {
                                        hi(Huge::ArrowDown)
                                    } else {
                                        hi(Huge::ArrowUp)
                                    })
                                    .label(if self.console_expanded {
                                        self.t("console.hide")
                                    } else {
                                        self.t("console.show")
                                    })
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.console_expanded = !this.console_expanded;
                                    })),
                            ),
                    ),
            )
            .when(self.console_expanded, |this| {
                this.child(
                    div()
                        .id("console-lines")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scrollbar()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .px_4()
                        .py_3()
                        .v_flex()
                        .gap_2()
                        .font_family("monospace")
                        .text_size(px(12.))
                        .children(if self.logs.is_empty() {
                            vec![div()
                                .text_color(cx.theme().muted_foreground)
                                .child(self.t("console.emptyCompact"))]
                        } else {
                            self.logs
                                .iter()
                                .rev()
                                .take(8)
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
                                        cx.theme().muted_foreground
                                    };
                                    div().text_color(color).child(line.clone())
                                })
                                .collect()
                        }),
                )
            })
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
                        .child(hi(Huge::Alert).size(px(26.))),
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
                        .icon(hi(Huge::Rotate))
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
                                .child(hi(Huge::Download).size(px(27.))),
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
                                .text_size(px(12.))
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
                            .text_size(px(12.))
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
                                .icon(hi(Huge::Download))
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
                                .icon(hi(Huge::Rotate))
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
            overlay(cx).child(
                surface(cx)
                    .w(px(760.))
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
                                            .text_size(px(22.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.t("form.editorHint")),
                                    ),
                            )
                            .child(
                                Button::new("close-editor")
                                    .ghost()
                                    .icon(hi(Huge::Close))
                                    .accessibility_label(self.t("form.close"))
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
                                        .text_size(px(13.))
                                        .child(self.tv(
                                            "form.extraOptions",
                                            &[("count", editor.extra_options.len().to_string())],
                                        )),
                                )
                            })
                            .child(form_section(
                                Huge::Server,
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
                                Huge::Lock,
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
                                    Huge::Settings,
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
                                        .icon(hi(Huge::Delete))
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
                                            .ghost()
                                            .label(self.t("form.cancel"))
                                            .on_click(cx.listener(|this, _, _, _| {
                                                this.editor = None;
                                                this.editor_error = None;
                                            })),
                                    )
                                    .child(
                                        Button::new("save-editor")
                                            .primary()
                                            .icon(hi(Huge::Check))
                                            .label(self.t("form.save"))
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.save_editor(cx)),
                                            ),
                                    ),
                            ),
                    )
                    .with_animation(
                        "editor-in",
                        Animation::new(Duration::from_millis(220)),
                        |this, delta| this.opacity(delta),
                    ),
            ),
        )
    }

    fn render_delete_overlay(&self, cx: &mut Context<Self>) -> Option<Div> {
        let id = self.confirm_delete.clone()?;
        Some(
            overlay(cx).child(
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
                            .child(hi(Huge::Delete).size(px(23.))),
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
                                    .ghost()
                                    .label(self.t("form.cancel"))
                                    .on_click(
                                        cx.listener(|this, _, _, _| this.confirm_delete = None),
                                    ),
                            )
                            .child(
                                Button::new("confirm-delete")
                                    .danger()
                                    .icon(hi(Huge::Delete))
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
                    )
                    .with_animation(
                        "delete-in",
                        Animation::new(Duration::from_millis(220)),
                        |this, delta| this.opacity(delta),
                    ),
            ),
        )
    }

    fn render_update_overlay(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.confirm_update {
            return None;
        }
        let latest = self.update.as_ref()?.latest.clone();
        Some(
            overlay(cx).child(
                surface(cx)
                    .w(px(460.))
                    .v_flex()
                    .gap_4()
                    .p_5()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::BOLD)
                            .child(self.t("update.installTitle")),
                    )
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.tv("update.installMessage", &[("latest", latest)])),
                    )
                    .child(
                        div()
                            .h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-update")
                                    .ghost()
                                    .label(self.t("update.cancel"))
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.confirm_update = false;
                                    })),
                            )
                            .child(
                                Button::new("confirm-update")
                                    .primary()
                                    .icon(hi(Huge::Download))
                                    .label(self.t("update.install"))
                                    .on_click(cx.listener(|this, _, _, _| {
                                        this.start_update_install();
                                    })),
                            ),
                    )
                    .with_animation(
                        "update-in",
                        Animation::new(Duration::from_millis(220)),
                        |this, delta| this.opacity(delta),
                    ),
            ),
        )
    }

    fn render_quit_overlay(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.confirm_quit {
            return None;
        }
        Some(
            overlay(cx).child(
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
                                    .ghost()
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
                    )
                    .with_animation(
                        "quit-in",
                        Animation::new(Duration::from_millis(220)),
                        |this, delta| this.opacity(delta),
                    ),
            ),
        )
    }
}

impl Render for Desk {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_theme(window, cx);
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
            .children(self.render_update_overlay(cx))
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
    const COLORS: [u32; 5] = [0xc14f26, 0x336b87, 0x6652a3, 0x2f7a68, 0x8a5b2d];
    let index = name
        .bytes()
        .fold(0usize, |total, byte| total.wrapping_add(byte as usize))
        % COLORS.len();
    COLORS[index]
}

fn effective_mode(preference: &str, window: &Window) -> &'static str {
    resolve_theme_mode(
        preference,
        matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    )
}

fn apply_theme(preference: &str, window: &mut Window, cx: &mut App) {
    let mode = effective_mode(preference, window);
    Theme::change(
        if mode == "light" {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        },
        Some(window),
        cx,
    );
    let palette = Palette::named(mode);
    let (brand_color, brand_hover_color, brand_active_color) = if mode == "light" {
        (BRAND, BRAND_HOVER, BRAND_ACTIVE)
    } else {
        (DARK_BRAND, DARK_BRAND_HOVER, DARK_BRAND_ACTIVE)
    };
    let brand = rgb(brand_color).into();
    let brand_hover = rgb(brand_hover_color).into();
    let brand_active = rgb(brand_active_color).into();
    let white: Hsla = rgb(0xffffff).into();
    let theme = Theme::global_mut(cx);
    theme.background = rgb(palette.workspace_bg).into();
    theme.foreground = rgb(palette.foreground).into();
    theme.border = rgb(palette.sidebar_border).into();
    theme.secondary = rgb(palette.card_bg).into();
    theme.secondary_hover = rgb(palette.card_hover).into();
    theme.secondary_active = rgb(palette.card_active).into();
    theme.muted = rgb(palette.muted).into();
    theme.muted_foreground = rgb(palette.sidebar_muted).into();
    theme.input = rgb(palette.input).into();
    theme.popover = rgb(palette.card_bg).into();
    theme.popover_foreground = rgb(palette.foreground).into();
    theme.button = rgb(palette.button).into();
    theme.button_hover = rgb(palette.button_hover).into();
    theme.button_active = rgb(palette.button_active).into();
    theme.button_foreground = rgb(palette.button_foreground).into();
    theme.title_bar = rgb(palette.title_bar).into();
    theme.title_bar_border = rgb(palette.sidebar_border).into();
    theme.sidebar = rgb(palette.sidebar_bg).into();
    theme.sidebar_foreground = rgb(palette.sidebar_text).into();
    theme.sidebar_border = rgb(palette.sidebar_border).into();
    theme.sidebar_accent = rgb(palette.sidebar_raised).into();
    theme.sidebar_accent_foreground = rgb(palette.sidebar_text).into();
    theme.sidebar_primary = brand;
    theme.sidebar_primary_foreground = white;
    theme.success = rgb(palette.success).into();
    theme.success_hover = rgb(palette.success).into();
    theme.success_active = rgb(palette.success).into();
    theme.warning = rgb(palette.warning).into();
    theme.danger = rgb(palette.danger).into();
    theme.overlay = rgba(palette.overlay).into();
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
    theme.radius = px(10.);
    theme.radius_lg = px(16.);
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
            img(tunnel_yard::app_icon::cached_icon_png())
                .size_full()
                .object_fit(ObjectFit::Contain),
        )
        .border_1()
        .border_color(cx.theme().primary.opacity(0.55))
}

fn surface(cx: &App) -> Div {
    div()
        .rounded(px(18.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_lg()
}

fn metric_tile(icon: Huge, value: String, label: String, tone: Hsla, cx: &App) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .h_flex()
        .items_center()
        .gap_3()
        .px_4()
        .py_3()
        .rounded(px(14.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            div()
                .size(px(32.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(10.))
                .bg(tone.opacity(0.14))
                .text_color(tone)
                .child(hi(icon).size(px(16.))),
        )
        .child(
            div()
                .v_flex()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(18.))
                        .font_weight(FontWeight::BOLD)
                        .child(value),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                ),
        )
}

fn sidebar_section_label(text: String, cx: &App) -> Div {
    div()
        .px_1()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
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
        .bg(cx.theme().background)
}

fn overlay(cx: &App) -> Div {
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
        .bg(cx.theme().overlay)
}

fn status_dot(color: Hsla, status: VpnStatus) -> impl IntoElement {
    let connecting = status == VpnStatus::Connecting;
    div()
        .size(px(6.))
        .rounded(px(999.))
        .bg(color)
        .with_animation(
            if connecting {
                "status-pulse"
            } else {
                "status-still"
            },
            Animation::new(Duration::from_millis(1200)).repeat(),
            move |this, delta| {
                if connecting {
                    let wave = (delta * std::f32::consts::PI * 2.).sin() * 0.5 + 0.5;
                    this.opacity(0.4 + 0.6 * wave)
                } else {
                    this
                }
            },
        )
}

fn icon_badge(icon: Huge, tone: Hsla, _cx: &App) -> Div {
    div()
        .size(px(40.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(12.))
        .bg(tone.opacity(0.14))
        .text_color(tone)
        .child(hi(icon).size(px(18.)))
}

fn prefs_section(icon: Huge, title: String, hint: String, body: Div, cx: &App) -> Div {
    div()
        .v_flex()
        .gap_3()
        .child(
            div()
                .h_flex()
                .gap_2()
                .items_center()
                .child(hi(icon).size(px(16.)).text_color(cx.theme().primary))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_uppercase()),
                ),
        )
        .when(!hint.is_empty(), |this| {
            this.child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(hint),
            )
        })
        .child(body)
}

fn pref_switch_row(icon: Huge, title: String, hint: String, switch: Switch, cx: &App) -> Div {
    div()
        .h_flex()
        .justify_between()
        .gap_4()
        .p_4()
        .rounded(px(14.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            div()
                .h_flex()
                .gap_3()
                .items_start()
                .child(
                    div()
                        .mt(px(2.))
                        .text_color(cx.theme().primary)
                        .child(hi(icon).size(px(16.))),
                )
                .child(
                    div()
                        .v_flex()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(hint),
                        ),
                ),
        )
        .child(switch)
}

fn form_section(icon: Huge, title: String, fields: Vec<Div>, cx: &App) -> Div {
    div()
        .v_flex()
        .gap_3()
        .child(
            div()
                .h_flex()
                .gap_2()
                .items_center()
                .child(hi(icon).size(px(15.)).text_color(cx.theme().primary))
                .child(
                    div()
                        .text_size(px(15.))
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
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(Input::new(input).disabled(disabled))
        .when(!hint.is_empty(), |this| {
            this.child(
                div()
                    .text_size(px(12.))
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
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(Input::new(input).mask_toggle())
        .child(
            div()
                .text_size(px(12.))
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
    tunnel_yard::os_ui::send_notification(title, body);
}

fn pick_conf_file() -> Option<PathBuf> {
    tunnel_yard::os_ui::pick_conf_file()
}

#[cfg(target_os = "linux")]
fn linux_tray_pixmaps() -> Vec<ksni::Icon> {
    tunnel_yard::app_icon::tray_pixmap_pngs()
        .iter()
        .filter_map(|bytes| tunnel_yard::app_icon::png_argb_pixmap(bytes))
        .map(|(width, height, data)| ksni::Icon {
            width,
            height,
            data,
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn spawn_tray(
    vpn: Arc<Mutex<VpnManager>>,
    locale: String,
    tx: mpsc::Sender<TrayCmd>,
    _quitting: Arc<Mutex<bool>>,
) -> Option<ksni::blocking::Handle<AppTray>> {
    use ksni::blocking::TrayMethods;
    AppTray { vpn, locale, tx }.spawn().ok()
}

#[cfg(not(target_os = "linux"))]
fn spawn_tray(
    vpn: Arc<Mutex<VpnManager>>,
    locale: String,
    tx: mpsc::Sender<TrayCmd>,
    _quitting: Arc<Mutex<bool>>,
) {
    let _ = (vpn, locale, tx);
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
        tunnel_yard::APP_ID.into()
    }

    fn title(&self) -> String {
        tunnel_yard::APP_NAME.into()
    }

    fn icon_name(&self) -> String {
        // Empty on purpose: GNOME AppIndicator prefers IconName over pixmaps, and
        // when both are set it draws the coral mark as a red overlay badge.
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        linux_tray_pixmaps()
    }

    fn overlay_icon_name(&self) -> String {
        String::new()
    }

    fn overlay_icon_pixmap(&self) -> Vec<ksni::Icon> {
        Vec::new()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let description = self
            .vpn
            .lock()
            .ok()
            .map(|vpn| summarize_vpn_state(&vpn.get_state()).message)
            .unwrap_or_default();
        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
            title: tunnel_yard::APP_NAME.into(),
            description,
        }
    }

    fn menu_about_to_show(&mut self) {}

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayCmd::Show);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        use tunnel_yard::tray_menu::{build_tray_menu, TrayEntry};
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
                TrayEntry::Profile {
                    profile_id,
                    label,
                    checked,
                } => {
                    let tx = self.tx.clone();
                    items.push(
                        CheckmarkItem {
                            label,
                            checked,
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

//! Unprivileged egui desktop: profiles, editor, setup gate, console, tray.

use crate::icons::{self, Glyph};
use crate::theme::{self, Palette};
use eframe::egui::{
    self, Color32, CornerRadius, Frame, Key, Margin, RichText, Stroke, TextureHandle, Ui,
    ViewportCommand,
};
use my_vpns::autostart::{get_autostart_path, is_autostart_enabled, set_autostart_enabled};
use my_vpns::conf::{
    delete_profile_file, draft_from_imported_file, empty_draft, save_profile_draft, VpnProfileDraft,
};
use my_vpns::deps::{get_dependency_status, install_vpn_client, DependencyStatus, InstallResult};
use my_vpns::desktop::{EDITOR_FIELDS, SETUP_GATE_KEYS, TRAY_MENU_KEYS, UI_SURFACES};
use my_vpns::i18n::translate;
use my_vpns::settings::{load_settings, normalize_theme, save_settings, AppSettingsPatch};
use my_vpns::updates::{perform_update_check, UpdateCheckResult, UpdateInfo, FIRST_CHECK_DELAY_MS};
use my_vpns::vpn::{summarize_vpn_state, VpnEvent, VpnManager, VpnProfile, VpnState, VpnStatus};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[allow(dead_code)]
fn surfaces_are_shipped() -> bool {
    !UI_SURFACES.is_empty()
        && TRAY_MENU_KEYS.len() == 4
        && EDITOR_FIELDS.len() >= 14
        && SETUP_GATE_KEYS.len() >= 5
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
    editor: Option<Editor>,
    editor_error: Option<String>,
    confirm_delete: Option<String>,
    confirm_quit: bool,
    vpn: Arc<Mutex<VpnManager>>,
    events: Receiver<VpnEvent>,
    last_connected: std::collections::HashSet<String>,
    setup_busy: bool,
    visible: bool,
    quitting: Arc<Mutex<bool>>,
    tray_rx: Option<std::sync::mpsc::Receiver<TrayCmd>>,
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
    last_close_ms: Option<u128>,
    pending_hide: bool,
    filter: String,
    console_open: bool,
    screenshot_path: Option<PathBuf>,
    screenshot_armed: bool,
    icon_tex: TextureHandle,
}

enum SetupMsg {
    Log(String),
    Done(InstallResult),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckFeedback {
    Idle,
    Checking,
    UpToDate,
    Error,
}

struct Editor {
    mode: &'static str,
    draft: VpnProfileDraft,
    show_password: bool,
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

pub fn run(hidden: bool, screenshot: Option<String>) -> Result<(), String> {
    let _ = my_vpns::app_icon::ensure_app_icon_files();
    #[cfg(target_os = "linux")]
    let _ = my_vpns::app_icon::ensure_linux_desktop_entry();
    let icon = eframe::icon_data::from_png_bytes(my_vpns::app_icon::APP_ICON_PNG).ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1080.0, 720.0])
        .with_min_inner_size([900.0, 600.0])
        .with_title("My VPNs")
        .with_app_id(my_vpns::APP_ID)
        .with_decorations(true);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    if hidden {
        viewport = viewport.with_visible(false);
    }
    if screenshot.is_some() {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(12));
            eprintln!("[my-vpns] screenshot watchdog");
            std::process::exit(2);
        });
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "My VPNs",
        options,
        Box::new(move |cc| Ok(Box::new(Desk::new(cc, hidden, screenshot)))),
    )
    .map_err(|e| e.to_string())
}

impl Desk {
    fn new(cc: &eframe::CreationContext<'_>, hidden: bool, screenshot: Option<String>) -> Self {
        let settings = load_settings();
        crate::theme::install_fonts(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx, &settings.theme);
        let icon_tex = cc.egui_ctx.load_texture(
            "my-vpns-mark",
            png_to_color_image(my_vpns::app_icon::APP_ICON_PNG),
            egui::TextureOptions::LINEAR,
        );
        let (vpn, events) = VpnManager::subscribe();
        let vpn = Arc::new(Mutex::new(vpn));
        let quitting = Arc::new(Mutex::new(false));
        let (tray_tx, tray_rx) = std::sync::mpsc::channel();
        let (update_tx, update_rx) = std::sync::mpsc::channel();
        let (setup_tx, setup_rx) = std::sync::mpsc::channel();
        spawn_tray(
            vpn.clone(),
            settings.locale.clone(),
            tray_tx,
            quitting.clone(),
        );
        let mut desk = Self {
            locale: settings.locale,
            theme: settings.theme,
            version: my_vpns::APP_VERSION.into(),
            deps: None,
            deps_ready: false,
            boot_error: None,
            profiles: vec![],
            state: VpnState {
                sessions: Default::default(),
                auto_reconnect: false,
            },
            logs: vec![],
            install_logs: vec![],
            autostart: is_autostart_enabled(),
            editor: None,
            editor_error: None,
            confirm_delete: None,
            confirm_quit: false,
            vpn,
            events,
            last_connected: Default::default(),
            setup_busy: false,
            visible: !hidden,
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
            dismissed_update: settings.dismissed_update_version.clone(),
            setup_rx,
            setup_tx,
            last_close_ms: None,
            pending_hide: hidden,
            filter: String::new(),
            console_open: true,
            screenshot_path: screenshot.map(PathBuf::from),
            screenshot_armed: false,
            icon_tex,
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
            desk.open_create();
        }
        if let Ok(theme) = std::env::var("MY_VPNS_THEME") {
            if matches!(theme.as_str(), "light" | "dark" | "system") {
                desk.theme = theme;
            }
        }
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

    fn request_quit(&mut self, ctx: &egui::Context) {
        *self.quitting.lock().unwrap() = true;
        self.vpn.lock().unwrap().disconnect(None);
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn palette(&self) -> Palette {
        theme::palette(theme_is_dark(&self.theme))
    }

    fn pump(&mut self, ctx: &egui::Context) {
        if !self.auto_check_sent
            && self.started_at.elapsed().as_millis() as u64 >= FIRST_CHECK_DELAY_MS
        {
            self.auto_check_sent = true;
            self.spawn_update_check();
        }
        while let Ok(msg) = self.setup_rx.try_recv() {
            match msg {
                SetupMsg::Log(line) => {
                    self.install_logs.push(line);
                    if self.install_logs.len() > 200 {
                        self.install_logs.drain(0..self.install_logs.len() - 200);
                    }
                }
                SetupMsg::Done(result) => {
                    self.setup_busy = false;
                    if result.ok {
                        self.deps = Some(result.status.clone());
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
                    let dismissed = self
                        .dismissed_update
                        .as_deref()
                        .map(|v| v == info.latest)
                        .unwrap_or(false);
                    if !dismissed {
                        self.update = Some(info);
                    }
                    self.check_feedback = CheckFeedback::Idle;
                }
                UpdateCheckResult::UpToDate { .. } => {
                    self.check_feedback = CheckFeedback::UpToDate;
                }
                UpdateCheckResult::Error { .. } => {
                    self.check_feedback = CheckFeedback::Error;
                }
            }
        }
        while let Ok(ev) = self.events.try_recv() {
            match ev {
                VpnEvent::State(state) => {
                    self.on_state(state);
                    self.rebuild_os_tray();
                }
                VpnEvent::Log(line) => {
                    self.logs.push(line);
                    if self.logs.len() > 400 {
                        self.logs.drain(0..self.logs.len() - 400);
                    }
                }
                VpnEvent::Profiles(p) => {
                    self.profiles = p;
                    self.rebuild_os_tray();
                }
                VpnEvent::NeedReconnect(id) => {
                    self.vpn.lock().unwrap().connect(&id);
                }
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
                    show_window(ctx, &mut self.visible);
                }
            }
            while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                let id = event.id.as_ref();
                if id == "show" {
                    let _ = self.tray_rx.as_ref();
                    show_window(ctx, &mut self.visible);
                } else if id == "quit" {
                    self.request_quit(ctx);
                } else if id == "refresh" {
                    self.profiles = self.vpn.lock().unwrap().refresh_profiles();
                    self.rebuild_os_tray();
                } else if id == "check_updates" {
                    self.check_feedback = CheckFeedback::Checking;
                    self.spawn_update_check();
                } else if id == "disconnect_all" {
                    self.vpn.lock().unwrap().disconnect(None);
                } else if let Some(pid) = id.strip_prefix("profile:") {
                    let session = self.state.sessions.get(pid);
                    if session
                        .map(|s| s.status != VpnStatus::Disconnected)
                        .unwrap_or(false)
                    {
                        self.vpn.lock().unwrap().disconnect(Some(pid));
                    } else {
                        self.vpn.lock().unwrap().connect(pid);
                    }
                }
            }
        }
        if let Some(rx) = &self.tray_rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    TrayCmd::Show => {
                        show_window(ctx, &mut self.visible);
                    }
                    TrayCmd::Quit => {
                        *self.quitting.lock().unwrap() = true;
                        self.vpn.lock().unwrap().disconnect(None);
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                    TrayCmd::Refresh => {
                        self.profiles = self.vpn.lock().unwrap().refresh_profiles();
                    }
                    TrayCmd::CheckUpdates => {
                        self.check_feedback = CheckFeedback::Checking;
                        self.spawn_update_check();
                    }
                    TrayCmd::DisconnectAll => self.vpn.lock().unwrap().disconnect(None),
                    TrayCmd::Toggle(id) => {
                        let session = self.state.sessions.get(&id);
                        if session
                            .map(|s| s.status != VpnStatus::Disconnected)
                            .unwrap_or(false)
                        {
                            self.vpn.lock().unwrap().disconnect(Some(&id));
                        } else {
                            self.vpn.lock().unwrap().connect(&id);
                        }
                    }
                }
            }
        }
    }

    fn on_state(&mut self, state: VpnState) {
        let connected_now: std::collections::HashSet<String> = state
            .sessions
            .values()
            .filter(|s| s.status == VpnStatus::Connected)
            .map(|s| s.profile_id.clone())
            .collect();
        let transitional: std::collections::HashSet<String> = state
            .sessions
            .values()
            .filter(|s| s.status == VpnStatus::Connecting)
            .map(|s| s.profile_id.clone())
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
                let msg = state
                    .sessions
                    .get(&id)
                    .map(|s| s.message.clone())
                    .filter(|m| !m.is_empty())
                    .unwrap_or_else(|| self.tv("notify.disconnectedBody", &[("id", id.clone())]));
                notify(&self.t("notify.disconnectedTitle"), &msg);
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
        {
            if let Some(tray) = self.os_tray.as_mut() {
                if let Some(menu) = os_tray_menu(&self.locale, &self.profiles, &self.state) {
                    let _ = tray.set_menu(Some(Box::new(menu)));
                }
            }
        }
    }

    fn handle_close_and_keys(&mut self, ctx: &egui::Context) {
        let quitting = *self.quitting.lock().unwrap();
        if ctx.input(|i| i.viewport().close_requested()) && !quitting {
            let now = now_ms();
            if my_vpns::desktop::should_quit_on_repeated_close(self.last_close_ms, now) {
                self.request_quit(ctx);
            } else {
                self.last_close_ms = Some(now);
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.pending_hide = true;
            }
        }
        if let Ok(ms) = std::env::var("MY_VPNS_HIDE_AFTER_MS") {
            if let Ok(ms) = ms.parse::<u128>() {
                if self.visible && self.started_at.elapsed().as_millis() as u128 >= ms {
                    self.pending_hide = true;
                }
            }
        }
        if self.pending_hide && !*self.quitting.lock().unwrap() {
            self.pending_hide = false;
            let was_visible = self.visible;
            hide_window(ctx, &mut self.visible);
            if was_visible && self.screenshot_path.is_none() {
                notify(&self.t("notify.parkedTitle"), &self.t("notify.parkedBody"));
            }
        }

        let ctrl = ctx.input(|i| i.modifiers.command);
        if ctrl && ctx.input(|i| i.key_pressed(Key::Q)) {
            let summary = summarize_vpn_state(&self.state);
            if summary.connected_count + summary.connecting_count > 0 {
                self.confirm_quit = true;
            } else {
                self.request_quit(ctx);
            }
        }
        if ctrl && ctx.input(|i| i.key_pressed(Key::W)) {
            self.pending_hide = true;
        }
        if ctrl && ctx.input(|i| i.key_pressed(Key::N)) && self.deps_ready {
            self.open_create();
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.editor = None;
            self.confirm_delete = None;
            self.confirm_quit = false;
        }
    }

    fn open_create(&mut self) {
        self.editor_error = None;
        self.editor = Some(Editor {
            mode: "create",
            draft: empty_draft(),
            show_password: false,
        });
    }

    fn take_screenshot_if_needed(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot_path.clone() else {
            return;
        };
        if !self.screenshot_armed && self.started_at.elapsed().as_millis() > 900 {
            self.screenshot_armed = true;
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
        }
        let mut captured: Option<std::sync::Arc<egui::ColorImage>> = None;
        ctx.input(|i| {
            for ev in &i.events {
                if let egui::Event::Screenshot { image, .. } = ev {
                    captured = Some(image.clone());
                }
            }
        });
        if let Some(image) = captured {
            match save_color_image(&image, &path) {
                Ok(()) => std::process::exit(0),
                Err(err) => {
                    eprintln!("[my-vpns] screenshot failed: {err}");
                    std::process::exit(1);
                }
            }
        }
        if self.screenshot_armed && self.started_at.elapsed().as_secs() >= 8 {
            eprintln!("[my-vpns] screenshot timed out");
            std::process::exit(2);
        }
    }
}

impl eframe::App for Desk {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        self.pump(ctx);
        self.handle_close_and_keys(ctx);

        apply_theme(ctx, &self.theme);

        if let Some(err) = &self.boot_error.clone() {
            self.ui_boot_fault(ctx, err);
            self.take_screenshot_if_needed(ctx);
            return;
        }

        if !self.deps_ready {
            self.ui_setup(ctx);
            self.take_screenshot_if_needed(ctx);
            return;
        }

        self.ui_desk(ctx);
        if self.editor.is_some() {
            self.ui_editor(ctx);
        }
        if let Some(id) = self.confirm_delete.clone() {
            self.ui_confirm_delete(ctx, &id);
        }
        if self.confirm_quit {
            self.ui_confirm_quit(ctx);
        }
        self.take_screenshot_if_needed(ctx);
        let _ = surfaces_are_shipped();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        *self.quitting.lock().unwrap() = true;
        self.vpn.lock().unwrap().disconnect(None);
    }
}

impl Desk {
    fn ui_boot_fault(&mut self, ctx: &egui::Context, err: &str) {
        let p = self.palette();
        egui::CentralPanel::default()
            .frame(theme::workspace_frame(&p))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    theme::card(&p).show(ui, |ui| {
                        ui.set_max_width(420.0);
                        ui.heading(self.t("boot.fault"));
                        ui.add_space(8.0);
                        ui.label(RichText::new(err).color(p.fault));
                        ui.add_space(12.0);
                        if accent_button(ui, &p, Glyph::Refresh, self.t("boot.retry")).clicked() {
                            self.boot_error = None;
                            let status = get_dependency_status();
                            self.deps_ready = status.client_installed;
                            self.deps = Some(status);
                        }
                    });
                });
            });
    }

    fn ui_setup(&mut self, ctx: &egui::Context) {
        let Some(status) = self.deps.clone() else {
            return;
        };
        let p = self.palette();
        egui::CentralPanel::default()
            .frame(theme::workspace_frame(&p))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(48.0);
                    theme::card(&p).show(ui, |ui| {
                        ui.set_max_width(520.0);
                        ui.heading(self.t("setup.title"));
                        ui.add_space(6.0);
                        ui.label(RichText::new(self.t("setup.missing")).color(p.muted));
                        ui.add_space(10.0);
                        ui.label(RichText::new(&status.engine).size(20.0).strong());
                        ui.label(
                            self.tv("setup.needsClient", &[("engine", status.engine.clone())]),
                        );
                        ui.label(RichText::new(&status.distro.pretty).strong());
                        ui.add_space(12.0);
                        Frame::new()
                            .fill(p.surface2)
                            .corner_radius(CornerRadius::same(12))
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.label(self.tv(
                                    "setup.installPlan",
                                    &[("family", status.distro.family.clone())],
                                ));
                                ui.monospace(
                                    status
                                        .install_command
                                        .clone()
                                        .unwrap_or_else(|| self.t("setup.noAutoInstall")),
                                );
                            });
                        if status.platform == "darwin" && !status.can_auto_install {
                            ui.label(self.t("setup.homebrew"));
                        }
                        if let Some(err) = &self.editor_error {
                            ui.colored_label(p.fault, err);
                        }
                        if !self.install_logs.is_empty() {
                            egui::ScrollArea::vertical()
                                .max_height(120.0)
                                .show(ui, |ui| {
                                    for line in &self.install_logs {
                                        ui.monospace(line);
                                    }
                                });
                        }
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let install = ui.add_enabled(
                                status.can_auto_install && !self.setup_busy,
                                egui::Button::new(
                                    RichText::new(if self.setup_busy {
                                        self.t("setup.working")
                                    } else {
                                        self.t("setup.installNow")
                                    })
                                    .color(Color32::WHITE)
                                    .strong(),
                                )
                                .fill(p.accent)
                                .min_size(egui::vec2(140.0, 34.0)),
                            );
                            if install.clicked() {
                                self.setup_busy = true;
                                self.install_logs.clear();
                                self.editor_error = None;
                                let tx = self.setup_tx.clone();
                                std::thread::spawn(move || {
                                    let result = install_vpn_client(|line| {
                                        let _ = tx.send(SetupMsg::Log(line.to_string()));
                                    });
                                    let _ = tx.send(SetupMsg::Done(result));
                                });
                            }
                            if ui
                                .add_enabled(
                                    !self.setup_busy,
                                    egui::Button::new(self.t("setup.recheck")),
                                )
                                .clicked()
                            {
                                let next = get_dependency_status();
                                if next.client_installed {
                                    self.deps = Some(next);
                                    self.deps_ready = true;
                                    self.profiles = self.vpn.lock().unwrap().refresh_profiles();
                                } else {
                                    self.editor_error = Some(self.t("setup.stillMissing"));
                                }
                            }
                        });
                    });
                });
            });
    }

    fn ui_desk(&mut self, ctx: &egui::Context) {
        let p = self.palette();
        let summary = summarize_vpn_state(&self.state);
        let any_up = summary.connected_count + summary.connecting_count > 0;
        let summary_label = if !any_up {
            self.t("ops.noneActive")
        } else {
            self.tv(
                "ops.deskSummary",
                &[
                    ("up", summary.connected_count.to_string()),
                    ("handshake", summary.connecting_count.to_string()),
                ],
            )
        };

        egui::SidePanel::left("rail")
            .exact_width(248.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(theme::rail_frame(&p))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Image::new((self.icon_tex.id(), egui::vec2(28.0, 28.0)))
                            .corner_radius(7),
                    );
                    ui.label(RichText::new("My VPNs").size(20.0).strong().color(p.text));
                });
                ui.label(
                    RichText::new(format!("v{}", self.version))
                        .small()
                        .color(p.muted),
                );
                ui.add_space(14.0);
                status_chip(ui, &p, summary.overall, &self.locale);
                ui.label(RichText::new(&summary_label).color(p.muted).small());
                if let Some(deps) = &self.deps {
                    ui.label(
                        RichText::new(&deps.config_dir)
                            .small()
                            .color(p.muted)
                            .monospace(),
                    );
                }
                ui.add_space(16.0);
                if accent_button(ui, &p, Glyph::Plus, self.t("ops.newProfile")).clicked() {
                    self.open_create();
                }
                if ghost_button(ui, &p, Glyph::Import, self.t("ops.importConf")).clicked() {
                    if let Some(path) = pick_conf_file() {
                        let (ok, message, draft) = draft_from_imported_file(&path);
                        if ok {
                            if let Some(draft) = draft {
                                self.editor_error = None;
                                self.editor = Some(Editor {
                                    mode: "import",
                                    draft,
                                    show_password: false,
                                });
                            }
                        } else {
                            self.editor_error = Some(message);
                        }
                    }
                }
                if ghost_button(ui, &p, Glyph::Refresh, self.t("ops.reloadProfiles")).clicked() {
                    self.profiles = self.vpn.lock().unwrap().refresh_profiles();
                }
                ui.add_enabled_ui(any_up, |ui| {
                    if ghost_button(ui, &p, Glyph::Unplug, self.t("ops.killAll")).clicked() {
                        self.vpn.lock().unwrap().disconnect(None);
                    }
                });
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(10.0);
                let mut auto = self.state.auto_reconnect;
                if toggle_row(ui, &p, &mut auto, &self.t("ops.autoRelink")).clicked() {
                    self.vpn.lock().unwrap().set_auto_reconnect(auto);
                    self.state.auto_reconnect = auto;
                }
                let mut autostart = self.autostart;
                if toggle_row(ui, &p, &mut autostart, &self.t("ops.startWithLinux"))
                    .on_hover_text(get_autostart_path())
                    .clicked()
                    && set_autostart_enabled(autostart)
                {
                    self.autostart = is_autostart_enabled();
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    for (code, label) in [("pt-BR", "PT"), ("en", "EN")] {
                        if chip(ui, &p, label, self.locale == code).clicked() {
                            self.locale = code.into();
                            save_settings(AppSettingsPatch {
                                locale: Some(code.into()),
                                ..Default::default()
                            });
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (value, key, glyph) in [
                        ("system", "theme.shortSystem", Glyph::Monitor),
                        ("light", "theme.shortLight", Glyph::Sun),
                        ("dark", "theme.shortDark", Glyph::Moon),
                    ] {
                        let selected = self.theme == value;
                        if icons::icon_chip(
                            ui,
                            glyph,
                            selected,
                            if selected { p.accent_soft } else { p.surface2 },
                            if selected { p.accent } else { p.line },
                            p.text,
                        )
                        .on_hover_text(self.t(key))
                        .clicked()
                        {
                            self.theme = value.into();
                            save_settings(AppSettingsPatch {
                                theme: Some(value.into()),
                                ..Default::default()
                            });
                        }
                    }
                });
                let bottom_h = 188.0;
                let spacer = (ui.available_height() - bottom_h).max(10.0);
                ui.add_space(spacer);
                ui.separator();
                ui.add_space(8.0);
                let check_label = match self.check_feedback {
                    CheckFeedback::Checking => self.t("update.checking"),
                    _ => self.t("update.checkNow"),
                };
                ui.add_enabled_ui(self.check_feedback != CheckFeedback::Checking, |ui| {
                    if ghost_button(ui, &p, Glyph::Download, check_label).clicked() {
                        self.check_feedback = CheckFeedback::Checking;
                        self.spawn_update_check();
                    }
                });
                if self.check_feedback == CheckFeedback::UpToDate {
                    ui.label(
                        RichText::new(
                            self.tv("update.upToDate", &[("version", self.version.clone())]),
                        )
                        .small()
                        .color(p.live),
                    );
                }
                if self.check_feedback == CheckFeedback::Error {
                    ui.label(
                        RichText::new(self.t("update.checkFailed"))
                            .small()
                            .color(p.fault),
                    );
                }
                ui.add_space(6.0);
                if ghost_button(ui, &p, Glyph::Hide, self.t("ops.hideToTray"))
                    .on_hover_text(self.t("chrome.closeHides"))
                    .clicked()
                {
                    self.pending_hide = true;
                }
                ui.add_space(6.0);
                let quit = icons::action_button(
                    ui,
                    Glyph::Quit,
                    &self.t("ops.quit"),
                    p.fault_soft,
                    Stroke::new(1.0_f32, p.fault.gamma_multiply(0.4)),
                    p.fault,
                    egui::vec2(208.0, 32.0),
                );
                if quit.clicked() {
                    if any_up {
                        self.confirm_quit = true;
                    } else {
                        self.request_quit(ctx);
                    }
                }
            });

        egui::CentralPanel::default()
            .frame(theme::workspace_frame(&p))
            .show(ctx, |ui| {
                if let Some(info) = self.update.clone() {
                    theme::card(&p).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(self.tv(
                                    "update.available",
                                    &[
                                        ("latest", info.latest.clone()),
                                        ("current", info.current.clone()),
                                    ],
                                ))
                                .color(p.accent),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button(self.t("update.dismiss")).clicked() {
                                        save_settings(AppSettingsPatch {
                                            dismissed_update_version: Some(info.latest.clone()),
                                            ..Default::default()
                                        });
                                        self.dismissed_update = Some(info.latest.clone());
                                        self.update = None;
                                    }
                                    if ui.button(self.t("update.open")).clicked() {
                                        let _ = open::that(&info.url);
                                    }
                                },
                            );
                        });
                    });
                    ui.add_space(12.0);
                }

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.t("ops.tunnels"))
                            .size(22.0)
                            .strong()
                            .color(p.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let hint = self.t("ops.search");
                        Frame::new()
                            .fill(p.surface)
                            .stroke(Stroke::new(1.0_f32, p.line))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::symmetric(8, 6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (icon_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(16.0, 16.0),
                                        egui::Sense::hover(),
                                    );
                                    icons::paint(ui, icon_rect, Glyph::Search, p.muted);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.filter)
                                            .hint_text(RichText::new(hint).color(p.muted))
                                            .frame(false)
                                            .desired_width(200.0),
                                    );
                                });
                            });
                    });
                });
                ui.add_space(10.0);

                let console_h = if self.console_open { 168.0 } else { 36.0 };
                let list_h = (ui.available_height() - console_h - 14.0).max(140.0);
                let filter = self.filter.to_lowercase();
                let visible: Vec<VpnProfile> = self
                    .profiles
                    .iter()
                    .filter(|pr| {
                        if filter.is_empty() {
                            return true;
                        }
                        [
                            pr.id.as_str(),
                            pr.name.as_str(),
                            pr.host.as_str(),
                            pr.username.as_str(),
                        ]
                        .iter()
                        .any(|s| s.to_lowercase().contains(&filter))
                    })
                    .cloned()
                    .collect();

                egui::ScrollArea::vertical()
                    .id_salt("profile-list")
                    .max_height(list_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if self.profiles.is_empty() {
                            ui.add_space(40.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(self.t("profiles.emptyTitle"))
                                        .size(20.0)
                                        .strong(),
                                );
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(self.t("profiles.emptyBody")).color(p.muted),
                                );
                                ui.add_space(14.0);
                                ui.horizontal(|ui| {
                                    if accent_button(ui, &p, Glyph::Plus, self.t("ops.newProfile"))
                                        .clicked()
                                    {
                                        self.open_create();
                                    }
                                    if ghost_button(ui, &p, Glyph::Import, self.t("ops.importConf"))
                                        .clicked()
                                    {
                                        if let Some(path) = pick_conf_file() {
                                            let (ok, message, draft) =
                                                draft_from_imported_file(&path);
                                            if ok {
                                                if let Some(draft) = draft {
                                                    self.editor_error = None;
                                                    self.editor = Some(Editor {
                                                        mode: "import",
                                                        draft,
                                                        show_password: false,
                                                    });
                                                }
                                            } else {
                                                self.editor_error = Some(message);
                                            }
                                        }
                                    }
                                });
                            });
                        } else if visible.is_empty() {
                            ui.add_space(32.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(self.t("profiles.noMatch")).color(p.muted));
                            });
                        } else {
                            for profile in visible {
                                self.profile_card(ui, &p, &profile);
                            }
                        }
                    });

                ui.add_space(12.0);
                theme::console_frame(&p).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (icon_rect, _) =
                            ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                        icons::paint(ui, icon_rect, Glyph::Console, p.muted);
                        ui.label(
                            RichText::new(self.tv("console.title", &[("label", summary_label)]))
                                .strong()
                                .color(p.text),
                        );
                        if summary.connecting_count > 0 {
                            ui.label(RichText::new(self.t("console.working")).color(p.hold));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icons::icon_button(
                                ui,
                                if self.console_open {
                                    Glyph::Hide
                                } else {
                                    Glyph::Console
                                },
                                p.muted,
                                &if self.console_open {
                                    self.t("console.hide")
                                } else {
                                    self.t("console.show")
                                },
                            )
                            .clicked()
                            {
                                self.console_open = !self.console_open;
                            }
                            if icons::icon_button(
                                ui,
                                Glyph::Trash,
                                p.muted,
                                &self.t("console.clear"),
                            )
                            .clicked()
                            {
                                self.logs.clear();
                            }
                        });
                    });
                    if self.console_open {
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical()
                            .id_salt("console")
                            .stick_to_bottom(true)
                            .max_height(120.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if self.logs.is_empty() {
                                    ui.label(RichText::new(self.t("console.empty")).color(p.muted));
                                } else {
                                    for line in &self.logs {
                                        let color = if line.to_lowercase().contains("error")
                                            || line.contains('✗')
                                            || line.to_lowercase().contains("failed")
                                        {
                                            p.fault
                                        } else if line.contains('→') || line.contains('↻') {
                                            p.accent
                                        } else {
                                            p.muted
                                        };
                                        ui.label(
                                            RichText::new(line).monospace().size(12.5).color(color),
                                        );
                                    }
                                }
                            });
                    }
                });
            });
    }

    fn profile_card(&mut self, ui: &mut Ui, p: &Palette, profile: &VpnProfile) {
        let session = self.state.sessions.get(&profile.id).cloned();
        let status = session
            .as_ref()
            .map(|s| s.status)
            .unwrap_or(VpnStatus::Disconnected);
        let connected = status == VpnStatus::Connected;
        let connecting = status == VpnStatus::Connecting;
        let frame = if connected {
            theme::live_card(p)
        } else {
            theme::card(p)
        };
        let inner = frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let color = match status {
                    VpnStatus::Connected => p.live,
                    VpnStatus::Connecting => p.hold,
                    VpnStatus::Error => p.fault,
                    VpnStatus::Disconnected => p.muted,
                };
                let (dot_rect, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                if status == VpnStatus::Disconnected {
                    ui.painter()
                        .circle_stroke(dot_rect.center(), 5.0, Stroke::new(1.6_f32, color));
                } else {
                    icons::status_dot(ui, dot_rect.center(), 5.0, color);
                }
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(&profile.name)
                            .size(16.0)
                            .strong()
                            .color(p.text),
                    );
                    let user = if profile.username.is_empty() {
                        self.t("profiles.noUser")
                    } else {
                        profile.username.clone()
                    };
                    let on = self.t("profiles.flagOn");
                    let off = self.t("profiles.flagOff");
                    ui.label(
                        RichText::new(format!(
                            "{}:{}  ·  {}  ·  {} {}  ·  dns {}",
                            profile.host,
                            profile.port,
                            user,
                            self.t("profiles.routesShort"),
                            if profile.set_routes { &on } else { &off },
                            if profile.set_dns { &on } else { &off },
                        ))
                        .small()
                        .color(p.muted),
                    );
                    if connected {
                        if let Some(at) = session.as_ref().and_then(|s| s.connected_at) {
                            ui.label(
                                RichText::new(
                                    self.tv("profiles.live", &[("uptime", format_duration(at))]),
                                )
                                .small()
                                .color(p.live),
                            );
                        }
                    }
                    if connecting {
                        ui.label(
                            RichText::new(self.t("profiles.handshake"))
                                .small()
                                .color(p.hold),
                        );
                    }
                    if let Some(session) = &session {
                        if status != VpnStatus::Disconnected && !session.message.is_empty() {
                            ui.label(RichText::new(&session.message).small().color(p.muted));
                        }
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let id = profile.id.clone();
                    if icons::icon_button(ui, Glyph::Trash, p.fault, &self.t("profiles.delete"))
                        .clicked()
                    {
                        self.confirm_delete = Some(id.clone());
                    }
                    if icons::icon_button(ui, Glyph::Pencil, p.muted, &self.t("profiles.edit"))
                        .clicked()
                    {
                        if let Some(draft) = my_vpns::read_profile_draft(&id) {
                            self.editor_error = None;
                            self.editor = Some(Editor {
                                mode: "edit",
                                draft,
                                show_password: false,
                            });
                        }
                    }
                    let label = if connected || connecting {
                        self.t("profiles.killLink")
                    } else {
                        self.t("profiles.bringUp")
                    };
                    let btn = if connected || connecting {
                        ui.add_enabled_ui(!connecting, |ui| {
                            icons::action_button(
                                ui,
                                Glyph::Unplug,
                                &label,
                                p.fault_soft,
                                Stroke::new(1.0_f32, p.fault.gamma_multiply(0.5)),
                                p.fault,
                                egui::vec2(124.0, 32.0),
                            )
                        })
                        .inner
                    } else {
                        icons::action_button(
                            ui,
                            Glyph::Plug,
                            &label,
                            p.accent,
                            Stroke::NONE,
                            Color32::WHITE,
                            egui::vec2(124.0, 32.0),
                        )
                    };
                    if btn.clicked() {
                        if connected || connecting {
                            self.vpn.lock().unwrap().disconnect(Some(&profile.id));
                        } else {
                            self.vpn.lock().unwrap().connect(&profile.id);
                        }
                    }
                });
            });
        });
        if inner.response.double_clicked() {
            if let Some(draft) = my_vpns::read_profile_draft(&profile.id) {
                self.editor_error = None;
                self.editor = Some(Editor {
                    mode: "edit",
                    draft,
                    show_password: false,
                });
            }
        }
        ui.add_space(10.0);
    }

    fn ui_editor(&mut self, ctx: &egui::Context) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let p = theme::palette(theme_is_dark(&self.theme));
        let title = match editor.mode {
            "edit" => translate(&self.locale, "form.editTitle", &[]),
            "import" => translate(&self.locale, "form.importTitle", &[]),
            _ => translate(&self.locale, "form.createTitle", &[]),
        };
        let mut save = false;
        let mut close = false;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .vscroll(true)
            .default_width(560.0)
            .max_height((ctx.screen_rect().height() - 48.0).max(480.0))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(theme::card(&p))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("openfortivpn  ·  .conf")
                        .small()
                        .color(p.muted),
                );
                if !editor.draft.extra_options.is_empty() {
                    ui.label(translate(
                        &self.locale,
                        "form.extraOptions",
                        &[("count", editor.draft.extra_options.len().to_string())],
                    ));
                    for (key, value) in &editor.draft.extra_options {
                        ui.monospace(format!("{key} = {value}"));
                    }
                }
                ui.add_space(8.0);
                ui.label(RichText::new(translate(&self.locale, "form.sectionConn", &[])).strong());
                egui::Grid::new("editor-conn")
                    .num_columns(2)
                    .spacing([14.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(translate(&self.locale, "form.id", &[]));
                        ui.add_enabled(
                            editor.mode != "edit",
                            egui::TextEdit::singleline(&mut editor.draft.id).desired_width(280.0),
                        );
                        ui.end_row();
                        ui.label(translate(&self.locale, "form.host", &[]));
                        ui.text_edit_singleline(&mut editor.draft.host);
                        ui.end_row();
                        ui.label(translate(&self.locale, "form.port", &[]));
                        ui.add(egui::DragValue::new(&mut editor.draft.port).range(1..=65535));
                        ui.end_row();
                        ui.label(translate(&self.locale, "form.realm", &[]));
                        ui.text_edit_singleline(&mut editor.draft.realm);
                        ui.end_row();
                    });
                ui.add_space(8.0);
                ui.label(RichText::new(translate(&self.locale, "form.sectionAuth", &[])).strong());
                egui::Grid::new("editor-auth")
                    .num_columns(2)
                    .spacing([14.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(translate(&self.locale, "form.username", &[]));
                        ui.text_edit_singleline(&mut editor.draft.username);
                        ui.end_row();
                        ui.label(translate(&self.locale, "form.password", &[]));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut editor.draft.password)
                                    .password(!editor.show_password)
                                    .desired_width(220.0),
                            );
                            let pw = if editor.show_password {
                                translate(&self.locale, "form.hidePassword", &[])
                            } else {
                                translate(&self.locale, "form.showPassword", &[])
                            };
                            let eye = if editor.show_password {
                                Glyph::EyeOff
                            } else {
                                Glyph::Eye
                            };
                            if icons::icon_button(ui, eye, p.muted, &pw).clicked() {
                                editor.show_password = !editor.show_password;
                            }
                        });
                        ui.end_row();
                        ui.label(translate(&self.locale, "form.trustedCert", &[]));
                        ui.text_edit_singleline(&mut editor.draft.trusted_cert);
                        ui.end_row();
                    });
                ui.add_space(8.0);
                ui.label(RichText::new(translate(&self.locale, "form.sectionOpts", &[])).strong());
                egui::Grid::new("editor-opts")
                    .num_columns(2)
                    .spacing([14.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(translate(&self.locale, "form.healthHost", &[]));
                        let mut hh = editor.draft.health_host.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut hh).changed() {
                            editor.draft.health_host = if hh.is_empty() { None } else { Some(hh) };
                        }
                        ui.end_row();
                        ui.label(translate(&self.locale, "form.healthPort", &[]));
                        let mut hp = editor.draft.health_port.unwrap_or(0);
                        if ui
                            .add(egui::DragValue::new(&mut hp).range(0..=65535))
                            .changed()
                        {
                            editor.draft.health_port = if hp == 0 { None } else { Some(hp) };
                        }
                        ui.end_row();
                        ui.label(translate(&self.locale, "form.persistent", &[]));
                        ui.add(
                            egui::DragValue::new(&mut editor.draft.persistent).range(0..=86_400),
                        );
                        ui.end_row();
                    });
                toggle_row(
                    ui,
                    &p,
                    &mut editor.draft.no_dtls,
                    &translate(&self.locale, "form.noDtls", &[]),
                );
                ui.label(
                    RichText::new(translate(&self.locale, "form.noDtlsHint", &[]))
                        .small()
                        .color(p.muted),
                );
                toggle_row(
                    ui,
                    &p,
                    &mut editor.draft.legacy_tunnel,
                    &translate(&self.locale, "form.legacyTunnel", &[]),
                );
                toggle_row(
                    ui,
                    &p,
                    &mut editor.draft.set_routes,
                    &translate(&self.locale, "form.setRoutes", &[]),
                );
                toggle_row(
                    ui,
                    &p,
                    &mut editor.draft.set_dns,
                    &translate(&self.locale, "form.setDns", &[]),
                );
                if let Some(err) = &self.editor_error {
                    ui.colored_label(p.fault, err);
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if accent_button(
                        ui,
                        &p,
                        Glyph::Check,
                        translate(&self.locale, "form.save", &[]),
                    )
                    .clicked()
                    {
                        save = true;
                    }
                    if ghost_button(
                        ui,
                        &p,
                        Glyph::Close,
                        translate(&self.locale, "form.cancel", &[]),
                    )
                    .clicked()
                    {
                        close = true;
                    }
                });
            });
        if close {
            self.editor = None;
            self.editor_error = None;
            return;
        }
        if save {
            if let Some(editor) = self.editor.take() {
                let overwrite = editor.mode == "edit";
                let result = save_profile_draft(&editor.draft, overwrite);
                if result.ok {
                    self.profiles = self.vpn.lock().unwrap().refresh_profiles();
                    self.editor_error = None;
                } else {
                    self.editor_error = Some(result.message);
                    self.editor = Some(editor);
                }
            }
        }
    }

    fn ui_confirm_delete(&mut self, ctx: &egui::Context, id: &str) {
        let p = self.palette();
        egui::Window::new(self.t("profiles.delete"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(theme::card(&p))
            .show(ctx, |ui| {
                ui.label(self.tv("profiles.deleteConfirm", &[("id", id.to_string())]));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(self.t("profiles.delete")).color(Color32::WHITE),
                            )
                            .fill(p.fault),
                        )
                        .clicked()
                    {
                        self.vpn.lock().unwrap().disconnect(Some(id));
                        let result = delete_profile_file(id);
                        if result.ok {
                            self.profiles = self.vpn.lock().unwrap().refresh_profiles();
                        } else {
                            self.editor_error = Some(result.message);
                        }
                        self.confirm_delete = None;
                    }
                    if ghost_button(ui, &p, Glyph::Close, self.t("form.cancel")).clicked() {
                        self.confirm_delete = None;
                    }
                });
            });
    }

    fn ui_confirm_quit(&mut self, ctx: &egui::Context) {
        let p = self.palette();
        egui::Window::new(self.t("ops.quitConfirm"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(theme::card(&p))
            .show(ctx, |ui| {
                ui.label(self.t("ops.quitConfirmBody"));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(self.t("ops.quit")).color(Color32::WHITE),
                            )
                            .fill(p.fault),
                        )
                        .clicked()
                    {
                        self.confirm_quit = false;
                        self.request_quit(ctx);
                    }
                    if ghost_button(ui, &p, Glyph::Close, self.t("form.cancel")).clicked() {
                        self.confirm_quit = false;
                    }
                });
            });
    }
}

fn status_chip(ui: &mut Ui, p: &Palette, status: VpnStatus, locale: &str) {
    let (key, fill, fg) = match status {
        VpnStatus::Connected => ("status.linkUp", p.live_soft, p.live),
        VpnStatus::Connecting => ("status.handshake", p.accent_soft, p.hold),
        VpnStatus::Error => ("status.fault", p.fault_soft, p.fault),
        VpnStatus::Disconnected => ("status.idle", p.surface2, p.muted),
    };
    theme::pill(fill).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            icons::status_dot(ui, r.center(), 3.5, fg);
            ui.label(
                RichText::new(translate(locale, key, &[]))
                    .color(fg)
                    .small()
                    .strong(),
            );
        });
    });
}

fn chip(ui: &mut Ui, p: &Palette, label: &str, selected: bool) -> egui::Response {
    let fill = if selected { p.accent_soft } else { p.surface2 };
    let stroke = if selected { p.accent } else { p.line };
    ui.add(
        egui::Button::new(RichText::new(label).size(12.5))
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, stroke))
            .corner_radius(8)
            .min_size(egui::vec2(48.0, 28.0)),
    )
}

fn toggle_row(ui: &mut Ui, p: &Palette, on: &mut bool, label: &str) -> egui::Response {
    ui.horizontal(|ui| {
        let desired = egui::vec2(38.0, 22.0);
        let (rect, knob) = ui.allocate_exact_size(desired, egui::Sense::click());
        let label_resp = ui.label(RichText::new(label).size(13.5));
        let resp = knob.union(label_resp);
        if resp.clicked() {
            *on = !*on;
        }
        let fill = if *on { p.accent } else { p.line };
        ui.painter().rect_filled(rect, CornerRadius::same(11), fill);
        let knob_x = if *on {
            rect.right() - 11.0
        } else {
            rect.left() + 11.0
        };
        ui.painter()
            .circle_filled(egui::pos2(knob_x, rect.center().y), 8.0, Color32::WHITE);
        resp
    })
    .inner
}

fn accent_button(ui: &mut Ui, p: &Palette, glyph: Glyph, text: String) -> egui::Response {
    icons::action_button(
        ui,
        glyph,
        &text,
        p.accent,
        Stroke::NONE,
        Color32::WHITE,
        egui::vec2(208.0, 34.0),
    )
}

fn ghost_button(ui: &mut Ui, p: &Palette, glyph: Glyph, text: String) -> egui::Response {
    icons::action_button(
        ui,
        glyph,
        &text,
        p.surface2,
        Stroke::new(1.0_f32, p.line),
        p.text,
        egui::vec2(208.0, 32.0),
    )
}

fn png_to_color_image(bytes: &[u8]) -> egui::ColorImage {
    let img = image::load_from_memory(bytes)
        .expect("app icon")
        .into_rgba8();
    let (w, h) = img.dimensions();
    let pixels = img
        .pixels()
        .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    egui::ColorImage::new([w as usize, h as usize], pixels)
}

fn apply_theme(ctx: &egui::Context, theme: &str) {
    crate::theme::apply_style(ctx, theme_is_dark(theme));
}

fn theme_is_dark(theme: &str) -> bool {
    match normalize_theme(theme) {
        "light" => false,
        "dark" => true,
        _ => prefers_dark(),
    }
}

fn prefers_dark() -> bool {
    std::env::var("GTK_THEME")
        .map(|t| t.to_lowercase().contains("dark"))
        .unwrap_or(true)
}

fn format_duration(connected_at: u128) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(connected_at);
    let seconds = now.saturating_sub(connected_at) / 1000;
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn hide_window(ctx: &egui::Context, visible: &mut bool) {
    *visible = false;
    // Wayland (GNOME): set_visible(false) is a no-op. Minimize is the real hide.
    // Deferring this to the frame after CancelClose is what actually dismisses the window.
    ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    ctx.send_viewport_cmd(ViewportCommand::Visible(false));
}

fn show_window(ctx: &egui::Context, visible: &mut bool) {
    *visible = true;
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(ViewportCommand::Focus);
}

fn notify(title: &str, body: &str) {
    my_vpns::os_ui::send_notification(title, body);
}

fn pick_conf_file() -> Option<PathBuf> {
    my_vpns::os_ui::pick_conf_file()
}

fn save_color_image(image: &egui::ColorImage, path: &Path) -> Result<(), String> {
    let mut buf = Vec::with_capacity(image.pixels.len() * 4);
    for px in &image.pixels {
        buf.extend_from_slice(&px.to_array());
    }
    let img = image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, buf)
        .ok_or_else(|| "invalid screenshot buffer".to_string())?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    img.save(path).map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn linux_tray_pixmaps() -> Vec<ksni::Icon> {
    let mut out = Vec::new();
    for bytes in [
        my_vpns::app_icon::APP_ICON_PNG_32,
        my_vpns::app_icon::APP_ICON_PNG,
    ] {
        if let Some((width, height, data)) = my_vpns::app_icon::png_argb_pixmap(bytes) {
            out.push(ksni::Icon {
                width,
                height,
                data,
            });
        }
    }
    out
}

fn spawn_tray(
    vpn: Arc<Mutex<VpnManager>>,
    locale: String,
    tx: std::sync::mpsc::Sender<TrayCmd>,
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
        let ok = match entry {
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
        if ok.is_none() {
            return None;
        }
    }
    Some(menu)
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn create_os_tray(locale: &str, vpn: &VpnManager) -> Option<tray_icon::TrayIcon> {
    let profiles = vpn.get_profiles();
    let state = vpn.get_state();
    let menu = os_tray_menu(locale, &profiles, &state)?;
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
    let img = image::load_from_memory(my_vpns::app_icon::APP_ICON_PNG)
        .ok()?
        .into_rgba8();
    let resized = image::imageops::resize(&img, 32, 32, image::imageops::FilterType::Triangle);
    let (w, h) = resized.dimensions();
    tray_icon::Icon::from_rgba(resized.into_raw(), w, h).ok()
}

#[cfg(target_os = "linux")]
struct AppTray {
    vpn: Arc<Mutex<VpnManager>>,
    locale: String,
    tx: std::sync::mpsc::Sender<TrayCmd>,
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
            return vec![];
        };
        let mut items = Vec::new();
        for entry in build_tray_menu(&self.locale, &profiles, &state) {
            match entry {
                TrayEntry::Separator => items.push(MenuItem::Separator),
                TrayEntry::Action { id, label } => {
                    let tx = self.tx.clone();
                    let cmd = match id {
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
                                let _ = tx.send(cmd.clone());
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

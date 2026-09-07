//! Contract for the unprivileged desktop UI. The egui host must actually
//! render these surfaces; tests assert the shipped strings/keys exist.

/// Tray actions required by the product (plus per-profile connect/disconnect).
pub const TRAY_MENU_KEYS: &[&str] = &[
    "tray.show",
    "tray.disconnectAll",
    "tray.refresh",
    "tray.quit",
];

/// Profile editor fields that must appear in the form.
pub const EDITOR_FIELDS: &[&str] = &[
    "form.id",
    "form.host",
    "form.port",
    "form.username",
    "form.password",
    "form.trustedCert",
    "form.realm",
    "form.setDns",
    "form.setRoutes",
    "form.persistent",
    "form.healthHost",
    "form.healthPort",
    "form.noDtls",
    "form.legacyTunnel",
];

pub const SETUP_GATE_KEYS: &[&str] = &[
    "setup.missing",
    "setup.needsClient",
    "setup.installPlan",
    "setup.installNow",
    "setup.recheck",
];

/// Named surfaces the shipped UI module must implement.
/// First window-close hides to the tray. A second *distinct* close (80–900ms later)
/// quits. Sub-80ms repeats are the same OS close event surviving extra frames.
pub const CLOSE_REPEAT_MIN_MS: u128 = 80;
pub const CLOSE_REPEAT_MAX_MS: u128 = 900;

pub fn should_quit_on_repeated_close(last_close_ms: Option<u128>, now_ms: u128) -> bool {
    last_close_ms
        .map(|t| {
            let dt = now_ms.saturating_sub(t);
            dt >= CLOSE_REPEAT_MIN_MS && dt < CLOSE_REPEAT_MAX_MS
        })
        .unwrap_or(false)
}

pub const UI_SURFACES: &[&str] = &[
    "window",
    "tray",
    "setup_gate",
    "profile_list",
    "profile_editor",
    "console",
    "locale_switch",
    "theme",
    "notifications",
    "autostart",
];

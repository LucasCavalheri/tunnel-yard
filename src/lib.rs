//! My VPNs domain library. The desktop binary (`my-vpns`) is an unprivileged
//! GUI around these units; privileged VPN work stays in `packaging/` helpers.

pub mod app_icon;
pub mod autostart;
pub mod conf;
pub mod deps;
pub mod desktop;
pub mod i18n;
pub mod install_native;
pub mod native;
pub mod openconnect;
pub mod os_ui;
pub mod platform;
pub mod settings;
pub mod smoke;
pub mod tray_menu;
pub mod updates;
pub mod vpn;

pub use conf::{
    conf_path_for_id, delete_profile_file, draft_from_imported_file, empty_draft,
    is_valid_profile_id, parse_vpn_conf_content, parse_vpn_draft, read_profile_draft,
    save_profile_draft, serialize_vpn_draft, slugify_profile_id, ProfileWriteResult, VpnProfile,
    VpnProfileDraft,
};
pub use desktop::{EDITOR_FIELDS, SETUP_GATE_KEYS, TRAY_MENU_KEYS, UI_SURFACES};
pub use i18n::{catalog_keys, list_locales, translate, AppLocale, MessageKey};
pub use platform::{binary_candidates, config_directory, engine_for_platform, VpnEngine};
pub use vpn::{
    interpret_vpn_log_line, list_vpn_profiles, native_close_decision, reconnect_delay_ms,
    should_native_reconnect, summarize_vpn_state, NativeCloseDecision, VpnManager, VpnSession,
    VpnState, VpnStatus,
};

pub const APP_NAME: &str = "My VPNs";
pub const APP_ID: &str = "dev.cavallheri.myvpns";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

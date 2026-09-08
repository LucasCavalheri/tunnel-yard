//! TunnelYard domain library. The desktop binary (`tunnel-yard`) is an
//! unprivileged GUI around these units; privileged VPN work stays in
//! `packaging/` helpers.

pub mod app_icon;
pub mod arch;
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

pub use arch::{current_arch, current_artifact_name};
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

pub const APP_NAME: &str = "TunnelYard";
pub const APP_ID: &str = "lucas.cavalheri.tunnelyard";
pub const APP_BIN: &str = "tunnel-yard";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_OWNER: &str = "LucasCavalheri";
pub const GITHUB_REPO: &str = "tunnel-yard";
pub const ICON_NAME: &str = "tunnel-yard";
pub const LEGACY_APP_ID: &str = "dev.cavallheri.myvpns";
pub const LEGACY_APP_BIN: &str = "my-vpns";
pub const LEGACY_APP_NAME: &str = "My VPNs";

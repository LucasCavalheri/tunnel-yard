//! Headless probe of the same binary: deps + settings + profile list.

use crate::deps::get_dependency_status;
use crate::settings::load_settings;
use crate::vpn::list_vpn_profiles;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmokeReport {
    pub engine: String,
    pub platform: String,
    pub config_dir: String,
    pub client_installed: bool,
    pub locale: String,
    pub theme: String,
    pub version: String,
    pub profile_count: usize,
    pub ok: bool,
}

pub fn smoke_report() -> SmokeReport {
    let deps = get_dependency_status();
    let settings = load_settings();
    let profiles = list_vpn_profiles(Path::new(&deps.config_dir));
    SmokeReport {
        engine: deps.engine,
        platform: deps.platform,
        config_dir: deps.config_dir,
        client_installed: deps.client_installed,
        locale: settings.locale,
        theme: settings.theme,
        version: crate::APP_VERSION.into(),
        profile_count: profiles.len(),
        ok: true,
    }
}

pub fn run() {
    let report = smoke_report();
    println!(
        "{}",
        serde_json::to_string(&report).unwrap_or_else(|_| "{}".into())
    );
}

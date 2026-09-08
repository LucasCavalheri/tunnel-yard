//! Persisted locale, theme, and dismissed-update version.

use crate::i18n::detect_locale;
use crate::platform::current_platform;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppSettings {
    pub locale: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissed_update_version: Option<String>,
}

fn default_theme() -> String {
    "dark".into()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: detect_locale().into(),
            theme: "system".into(),
            dismissed_update_version: None,
        }
    }
}

pub fn normalize_theme(value: &str) -> &'static str {
    match value {
        "light" => "light",
        "dark" => "dark",
        _ => "system",
    }
}

pub fn settings_path() -> PathBuf {
    match current_platform() {
        "macos" => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Application Support/My VPNs/settings.json"),
        "windows" => dirs::data_dir()
            .or_else(dirs::config_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("My VPNs/settings.json"),
        _ => dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            })
            .join("my-vpns/settings.json"),
    }
}

pub fn load_settings() -> AppSettings {
    match fs::read_to_string(settings_path()) {
        Ok(raw) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));
            let locale = parsed
                .get("locale")
                .and_then(|v| v.as_str())
                .filter(|s| *s == "pt-BR" || *s == "en")
                .unwrap_or_else(|| detect_locale())
                .to_string();
            let theme = normalize_theme(
                parsed
                    .get("theme")
                    .and_then(|v| v.as_str())
                    .unwrap_or("system"),
            )
            .to_string();
            let dismissed = parsed
                .get("dismissedUpdateVersion")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            AppSettings {
                locale,
                theme,
                dismissed_update_version: dismissed,
            }
        }
        Err(_) => AppSettings::default(),
    }
}

pub fn save_settings(partial: AppSettingsPatch) -> AppSettings {
    let mut next = load_settings();
    if let Some(locale) = partial.locale {
        next.locale = if locale == "pt-BR" { "pt-BR" } else { "en" }.into();
    }
    if let Some(theme) = partial.theme {
        next.theme = normalize_theme(&theme).into();
    }
    if let Some(v) = partial.dismissed_update_version {
        next.dismissed_update_version = if v.is_empty() { None } else { Some(v) };
    }
    if let Some(dir) = settings_path().parent() {
        let _ = fs::create_dir_all(dir);
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Disk {
        locale: String,
        theme: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        dismissed_update_version: Option<String>,
    }
    let disk = Disk {
        locale: next.locale.clone(),
        theme: next.theme.clone(),
        dismissed_update_version: next.dismissed_update_version.clone(),
    };
    let _ = fs::write(
        settings_path(),
        serde_json::to_string_pretty(&disk).unwrap_or_else(|_| "{}".into()),
    );
    next
}

#[derive(Default)]
pub struct AppSettingsPatch {
    pub locale: Option<String>,
    pub theme: Option<String>,
    pub dismissed_update_version: Option<String>,
}

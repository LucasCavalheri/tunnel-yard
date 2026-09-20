//! Persisted locale, theme, and dismissed-update version.

use crate::i18n::detect_locale;
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
    #[serde(default = "default_auto_reconnect")]
    pub auto_reconnect: bool,
}

fn default_theme() -> String {
    "system".into()
}

fn default_auto_reconnect() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: detect_locale().into(),
            theme: "system".into(),
            dismissed_update_version: None,
            auto_reconnect: true,
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
    dirs::config_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        })
        .join("tunnel-yard/settings.json")
}

fn legacy_settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        })
        .join("my-vpns/settings.json")
}

pub fn parse_settings_json(raw: &str) -> AppSettings {
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap_or(serde_json::json!({}));
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
    let auto_reconnect = parsed
        .get("autoReconnect")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    AppSettings {
        locale,
        theme,
        dismissed_update_version: dismissed,
        auto_reconnect,
    }
}

pub fn encode_settings_json(settings: &AppSettings) -> String {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Disk {
        locale: String,
        theme: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        dismissed_update_version: Option<String>,
        auto_reconnect: bool,
    }
    let disk = Disk {
        locale: settings.locale.clone(),
        theme: settings.theme.clone(),
        dismissed_update_version: settings.dismissed_update_version.clone(),
        auto_reconnect: settings.auto_reconnect,
    };
    serde_json::to_string_pretty(&disk).unwrap_or_else(|_| "{}".into())
}

pub fn apply_settings_patch(mut current: AppSettings, partial: AppSettingsPatch) -> AppSettings {
    if let Some(locale) = partial.locale {
        current.locale = if locale == "pt-BR" { "pt-BR" } else { "en" }.into();
    }
    if let Some(theme) = partial.theme {
        current.theme = normalize_theme(&theme).into();
    }
    if let Some(v) = partial.dismissed_update_version {
        current.dismissed_update_version = if v.is_empty() { None } else { Some(v) };
    }
    if let Some(auto_reconnect) = partial.auto_reconnect {
        current.auto_reconnect = auto_reconnect;
    }
    current
}

pub fn load_settings_from(path: &PathBuf, legacy: Option<&PathBuf>) -> AppSettings {
    let raw = fs::read_to_string(path).or_else(|_| match legacy {
        Some(legacy) => fs::read_to_string(legacy),
        None => Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
    });
    match raw {
        Ok(raw) => parse_settings_json(&raw),
        Err(_) => AppSettings::default(),
    }
}

pub fn save_settings_to(
    path: &PathBuf,
    current: AppSettings,
    partial: AppSettingsPatch,
) -> AppSettings {
    let next = apply_settings_patch(current, partial);
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, encode_settings_json(&next));
    next
}

pub fn load_settings() -> AppSettings {
    load_settings_from(&settings_path(), Some(&legacy_settings_path()))
}

pub fn save_settings(partial: AppSettingsPatch) -> AppSettings {
    save_settings_to(&settings_path(), load_settings(), partial)
}

#[derive(Default)]
pub struct AppSettingsPatch {
    pub locale: Option<String>,
    pub theme: Option<String>,
    pub dismissed_update_version: Option<String>,
    pub auto_reconnect: Option<bool>,
}

//! Start-at-login: XDG autostart on Linux; LaunchAgent on macOS; HKCU Run on Windows.

use crate::platform::current_platform;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn autostart_desktop_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/autostart/tunnel-yard.desktop")
}

fn linux_legacy_autostart_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/autostart/my-vpns.desktop")
}

pub fn macos_launch_agent_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/LaunchAgents/lucas.cavalheri.tunnelyard.plist")
}

fn macos_legacy_launch_agent_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/LaunchAgents/dev.cavallheri.myvpns.plist")
}

pub const WINDOWS_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
pub const WINDOWS_RUN_VALUE: &str = "TunnelYard";
const WINDOWS_LEGACY_RUN_VALUE: &str = "My VPNs";

pub fn resolve_autostart_exec() -> String {
    if PathBuf::from("/usr/bin/tunnel-yard").exists() {
        return "/usr/bin/tunnel-yard --hidden".into();
    }
    if PathBuf::from("/usr/bin/my-vpns").exists() {
        return "/usr/bin/my-vpns --hidden".into();
    }
    if let Ok(exe) = std::env::current_exe() {
        return format!("\"{}\" --hidden", exe.display());
    }
    "tunnel-yard --hidden".into()
}

pub fn build_autostart_desktop_entry(exec: &str) -> String {
    [
        "[Desktop Entry]",
        "Type=Application",
        "Version=1.0",
        "Name=TunnelYard",
        "Comment=OpenFortiVPN control desk",
        "Comment[pt_BR]=Mesa de controle OpenFortiVPN",
        &format!("Exec={exec}"),
        "Icon=tunnel-yard",
        "Terminal=false",
        "Categories=Network;Security;",
        "StartupNotify=true",
        "StartupWMClass=lucas.cavalheri.tunnelyard",
        "X-GNOME-Autostart-enabled=true",
        "X-GNOME-Autostart-Delay=3",
        "",
    ]
    .join("\n")
}

/// macOS LaunchAgent that starts the GUI hidden at login (native login-item equivalent).
pub fn macos_launch_agent_plist(program: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>lucas.cavalheri.tunnelyard</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>--hidden</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
</dict>
</plist>
"#,
        xml_escape(program)
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Command stored in the current-user Run key (`path --hidden`).
pub fn windows_autostart_command(exe: &Path) -> String {
    format!("\"{}\" --hidden", exe.display())
}

pub fn windows_reg_add_args(command: &str) -> Vec<String> {
    vec![
        "add".into(),
        WINDOWS_RUN_KEY.into(),
        "/v".into(),
        WINDOWS_RUN_VALUE.into(),
        "/t".into(),
        "REG_SZ".into(),
        "/d".into(),
        command.into(),
        "/f".into(),
    ]
}

pub fn windows_reg_delete_args() -> Vec<String> {
    vec![
        "delete".into(),
        WINDOWS_RUN_KEY.into(),
        "/v".into(),
        WINDOWS_RUN_VALUE.into(),
        "/f".into(),
    ]
}

pub fn windows_reg_query_args() -> Vec<String> {
    vec![
        "query".into(),
        WINDOWS_RUN_KEY.into(),
        "/v".into(),
        WINDOWS_RUN_VALUE.into(),
    ]
}

pub fn is_autostart_enabled() -> bool {
    match current_platform() {
        "macos" => macos_autostart_enabled(),
        "windows" => windows_autostart_enabled(),
        _ => linux_autostart_enabled(),
    }
}

pub fn set_autostart_enabled(enabled: bool) -> bool {
    match current_platform() {
        "macos" => set_macos_autostart(enabled),
        "windows" => set_windows_autostart(enabled),
        _ => set_linux_autostart(enabled),
    }
}

fn linux_entry_is_enabled(raw: &str) -> bool {
    if raw
        .to_ascii_lowercase()
        .contains("x-gnome-autostart-enabled=false")
        || raw.to_ascii_lowercase().contains("hidden=true")
    {
        return false;
    }
    raw.contains("Exec=")
}

fn linux_autostart_enabled() -> bool {
    for path in [autostart_desktop_path(), linux_legacy_autostart_path()] {
        if let Ok(raw) = fs::read_to_string(path) {
            if linux_entry_is_enabled(&raw) {
                return true;
            }
        }
    }
    false
}

fn set_linux_autostart(enabled: bool) -> bool {
    let path = autostart_desktop_path();
    let _ = fs::remove_file(linux_legacy_autostart_path());
    if !enabled {
        let _ = fs::remove_file(&path);
        return true;
    }
    if let Some(dir) = path.parent() {
        if fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    fs::write(
        &path,
        build_autostart_desktop_entry(&resolve_autostart_exec()),
    )
    .is_ok()
}

fn macos_agent_is_enabled(raw: &str) -> bool {
    (raw.contains("lucas.cavalheri.tunnelyard") || raw.contains("dev.cavallheri.myvpns"))
        && raw.contains("--hidden")
        && raw.contains("RunAtLoad")
        && !raw.contains("<key>Disabled</key>\n  <true/>")
}

fn macos_autostart_enabled() -> bool {
    for path in [macos_launch_agent_path(), macos_legacy_launch_agent_path()] {
        if let Ok(raw) = fs::read_to_string(path) {
            if macos_agent_is_enabled(&raw) {
                return true;
            }
        }
    }
    false
}

fn unload_and_remove_agent(path: &Path) {
    let _ = Command::new("launchctl")
        .args(["unload", &path.to_string_lossy()])
        .status();
    let _ = fs::remove_file(path);
}

fn set_macos_autostart(enabled: bool) -> bool {
    let path = macos_launch_agent_path();
    unload_and_remove_agent(&macos_legacy_launch_agent_path());
    if !enabled {
        unload_and_remove_agent(&path);
        return true;
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if let Some(dir) = path.parent() {
        if fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    if fs::write(&path, macos_launch_agent_plist(&exe.to_string_lossy())).is_err() {
        return false;
    }
    let _ = Command::new("launchctl")
        .args(["unload", &path.to_string_lossy()])
        .status();
    Command::new("launchctl")
        .args(["load", &path.to_string_lossy()])
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

fn windows_autostart_enabled() -> bool {
    Command::new("reg")
        .args(windows_reg_query_args())
        .output()
        .map(|o| {
            o.status.success() && String::from_utf8_lossy(&o.stdout).contains(WINDOWS_RUN_VALUE)
        })
        .unwrap_or(false)
}

fn windows_legacy_reg_delete_args() -> Vec<String> {
    vec![
        "delete".into(),
        WINDOWS_RUN_KEY.into(),
        "/v".into(),
        WINDOWS_LEGACY_RUN_VALUE.into(),
        "/f".into(),
    ]
}

fn set_windows_autostart(enabled: bool) -> bool {
    let _ = Command::new("reg")
        .args(windows_legacy_reg_delete_args())
        .status();
    if !enabled {
        return Command::new("reg")
            .args(windows_reg_delete_args())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let cmd = windows_autostart_command(&exe);
    Command::new("reg")
        .args(windows_reg_add_args(&cmd))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn get_autostart_path() -> String {
    match current_platform() {
        "macos" => macos_launch_agent_path().to_string_lossy().into_owned(),
        "windows" => format!("{WINDOWS_RUN_KEY}\\{WINDOWS_RUN_VALUE}"),
        _ => autostart_desktop_path().to_string_lossy().into_owned(),
    }
}

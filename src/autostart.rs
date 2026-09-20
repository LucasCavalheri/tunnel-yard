//! Start-at-login via XDG autostart on Linux.

use std::fs;
use std::path::PathBuf;

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

pub fn is_autostart_enabled() -> bool {
    for path in [autostart_desktop_path(), linux_legacy_autostart_path()] {
        if let Ok(raw) = fs::read_to_string(path) {
            if linux_entry_is_enabled(&raw) {
                return true;
            }
        }
    }
    false
}

pub fn set_autostart_enabled(enabled: bool) -> bool {
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

pub fn get_autostart_path() -> String {
    autostart_desktop_path().to_string_lossy().into_owned()
}

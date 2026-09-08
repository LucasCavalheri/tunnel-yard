//! Platform notifications and the import file picker. The GUI process stays unprivileged.

use crate::app_icon::{cached_icon_ico, cached_icon_png, ensure_app_icon_files};
use crate::platform::current_platform;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn apple_script_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// argv used to show a user-visible notification on `platform`.
pub fn notification_argv(platform: &str, title: &str, body: &str) -> (String, Vec<String>) {
    notification_argv_with_icon(
        platform,
        title,
        body,
        &cached_icon_png(),
        &cached_icon_ico(),
    )
}

pub fn notification_argv_with_icon(
    platform: &str,
    title: &str,
    body: &str,
    png: &Path,
    ico: &Path,
) -> (String, Vec<String>) {
    match platform {
        "macos" => macos_notification_argv(title, body, png),
        "windows" => (
            crate::native::powershell_path()
                .to_string_lossy()
                .into_owned(),
            crate::native::encoded_powershell(&windows_balloon_script(
                title,
                body,
                &ico.to_string_lossy(),
            )),
        ),
        _ => (
            "notify-send".into(),
            vec![
                "-a".into(),
                crate::APP_NAME.into(),
                "-i".into(),
                png.to_string_lossy().into_owned(),
                format!("--hint=string:desktop-entry:{}", crate::APP_ID),
                title.into(),
                body.into(),
            ],
        ),
    }
}

fn macos_notification_argv(title: &str, body: &str, png: &Path) -> (String, Vec<String>) {
    (
        "osascript".into(),
        vec![
            "-e".into(),
            macos_notification_script(title, body, &png.to_string_lossy()),
        ],
    )
}

pub fn macos_notification_script(title: &str, body: &str, icon_path: &str) -> String {
    format!(
        "set iconPOSIX to {}\ndisplay notification {} with title {}",
        apple_script_string(icon_path),
        apple_script_string(body),
        apple_script_string(title),
    )
}

pub fn macos_terminal_notifier_args(title: &str, body: &str, icon_path: &str) -> Vec<String> {
    vec![
        "-title".into(),
        title.into(),
        "-message".into(),
        body.into(),
        "-appIcon".into(),
        icon_path.into(),
    ]
}

pub fn windows_balloon_script(title: &str, body: &str, icon_path: &str) -> String {
    format!(
        "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $n = New-Object System.Windows.Forms.NotifyIcon; try {{ $n.Icon = New-Object System.Drawing.Icon({icon}) }} catch {{ $n.Icon = [System.Drawing.SystemIcons]::Application }}; $n.Visible = $true; $n.ShowBalloonTip(5000, {title}, {body}, [System.Windows.Forms.ToolTipIcon]::None); Start-Sleep -Milliseconds 800; $n.Dispose()",
        icon = crate::native::ps_quote(icon_path),
        title = crate::native::ps_quote(title),
        body = crate::native::ps_quote(body)
    )
}

pub fn send_notification(title: &str, body: &str) {
    let png = ensure_app_icon_files();
    let ico = cached_icon_ico();
    let platform = current_platform();
    if platform == "macos" {
        let icon = png.to_string_lossy().into_owned();
        if Command::new("terminal-notifier")
            .args(macos_terminal_notifier_args(title, body, &icon))
            .spawn()
            .is_ok()
        {
            return;
        }
    }
    let (cmd, args) = notification_argv_with_icon(platform, title, body, &png, &ico);
    let _ = Command::new(cmd).args(args).spawn();
}

pub fn macos_choose_file_script() -> &'static str {
    r#"POSIX path of (choose file with prompt "Import openfortivpn .conf" of type {"conf"})"#
}

pub fn windows_open_file_dialog_script() -> &'static str {
    r#"Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Filter = 'openfortivpn conf (*.conf)|*.conf|All files (*.*)|*.*'; $d.Title = 'Import openfortivpn .conf'; if ($d.ShowDialog() -eq 'OK') { [Console]::Out.Write($d.FileName) }"#
}

pub fn pick_conf_file() -> Option<PathBuf> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        return rfd::FileDialog::new()
            .add_filter("openfortivpn conf", &["conf"])
            .add_filter("All files", &["*"])
            .set_title("Import openfortivpn .conf")
            .pick_file();
    }
    #[cfg(target_os = "linux")]
    {
        pick_linux_conf_file()
    }
}

#[cfg(target_os = "linux")]
fn pick_linux_conf_file() -> Option<PathBuf> {
    if let Ok(out) = Command::new("zenity")
        .args([
            "--file-selection",
            "--title=Import openfortivpn .conf",
            "--file-filter=openfortivpn conf | *.conf",
        ])
        .output()
    {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
        return None;
    }
    if let Ok(out) = Command::new("kdialog")
        .args(["--getopenfilename", ".", "*.conf"])
        .output()
    {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    None
}

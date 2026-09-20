//! Linux notifications and the import file picker. The GUI process stays unprivileged.

use crate::app_icon::{cached_icon_png, ensure_app_icon_files};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct FilePicker {
    pub command: &'static str,
    pub args: &'static [&'static str],
}

/// Desktop file dialogs, in preference order. zenity covers GNOME/Cinnamon,
/// kdialog KDE, yad/qarma cover lighter GTK desktops and zenity-compatible
/// forks (XFCE, LXQt, MATE without zenity).
pub const FILE_PICKERS: &[FilePicker] = &[
    FilePicker {
        command: "zenity",
        args: &[
            "--file-selection",
            "--title=Import openfortivpn .conf",
            "--file-filter=openfortivpn conf | *.conf",
        ],
    },
    FilePicker {
        command: "qarma",
        args: &[
            "--file-selection",
            "--title=Import openfortivpn .conf",
            "--file-filter=openfortivpn conf | *.conf",
        ],
    },
    FilePicker {
        command: "yad",
        args: &[
            "--file-selection",
            "--title=Import openfortivpn .conf",
            "--file-filter=openfortivpn conf | *.conf",
        ],
    },
    FilePicker {
        command: "kdialog",
        args: &["--getopenfilename", ".", "*.conf"],
    },
];

pub fn path_from_picker_output(success: bool, stdout: &str) -> Option<PathBuf> {
    if !success {
        return None;
    }
    let path = stdout.trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

/// argv used to show a user-visible notification.
pub fn notification_argv(title: &str, body: &str) -> (String, Vec<String>) {
    notification_argv_with_icon(title, body, &cached_icon_png())
}

pub fn notification_argv_with_icon(title: &str, body: &str, png: &Path) -> (String, Vec<String>) {
    (
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
    )
}

pub fn send_notification(title: &str, body: &str) {
    let png = ensure_app_icon_files();
    let (cmd, args) = notification_argv_with_icon(title, body, &png);
    if Command::new(&cmd).args(&args).spawn().is_ok() {
        return;
    }
    let _ = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest=org.freedesktop.Notifications",
            "--object-path=/org/freedesktop/Notifications",
            "--method=org.freedesktop.Notifications.Notify",
            crate::APP_NAME,
            "0",
            &png.to_string_lossy(),
            title,
            body,
            "[]",
            "{}",
            "5000",
        ])
        .spawn();
}

pub fn pick_conf_file() -> Option<PathBuf> {
    for picker in FILE_PICKERS {
        let Ok(out) = Command::new(picker.command).args(picker.args).output() else {
            continue;
        };
        return path_from_picker_output(
            out.status.success(),
            &String::from_utf8_lossy(&out.stdout),
        );
    }
    None
}

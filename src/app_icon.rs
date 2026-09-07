//! The My VPNs mark, embedded and cached so the tray, window and notifications
//! all show the same PNG on every platform.

use std::fs;
use std::path::{Path, PathBuf};

pub const APP_ICON_PNG: &[u8] = include_bytes!("../public/icon.png");
pub const APP_ICON_PNG_32: &[u8] = include_bytes!("../public/icon-32.png");
pub const APP_ICON_ICO: &[u8] = include_bytes!("../public/icon.ico");

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("my-vpns")
}

pub fn cached_icon_png() -> PathBuf {
    cache_dir().join("icon.png")
}

pub fn cached_icon_ico() -> PathBuf {
    cache_dir().join("icon.ico")
}

/// Build the launcher metadata GNOME uses to associate a Wayland surface with
/// its dock icon. The filename must be `${APP_ID}.desktop`, matching eframe's
/// `with_app_id` value; `StartupWMClass` covers the X11 fallback.
pub fn build_linux_desktop_entry(executable: &Path, icon: &Path) -> String {
    let executable = desktop_exec_path(executable);
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=My VPNs\n\
         Comment=OpenFortiVPN control desk\n\
         Comment[pt_BR]=Mesa de controle OpenFortiVPN\n\
         Exec={executable}\n\
         Icon={}\n\
         Terminal=false\n\
         Categories=Network;Security;\n\
         StartupNotify=true\n\
         StartupWMClass={}\n",
        icon.display(),
        crate::APP_ID,
    )
}

fn desktop_exec_path(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("\"{escaped}\"")
}

/// Write the embedded mark to the user cache if missing or stale.
pub fn ensure_app_icon_files() -> PathBuf {
    let dir = cache_dir();
    let _ = fs::create_dir_all(&dir);
    write_if_changed(&cached_icon_png(), APP_ICON_PNG);
    write_if_changed(&cached_icon_ico(), APP_ICON_ICO);
    cached_icon_png()
}

/// Install a per-user launcher on Linux so the running window gets the real
/// app icon instead of GNOME's generic gear, including source builds.
#[cfg(target_os = "linux")]
pub fn ensure_linux_desktop_entry() -> Option<PathBuf> {
    let icon = ensure_app_icon_files();
    let executable = std::env::current_exe().ok()?;
    let applications = dirs::data_local_dir()?.join("applications");
    fs::create_dir_all(&applications).ok()?;
    let destination = applications.join(format!("{}.desktop", crate::APP_ID));
    let entry = build_linux_desktop_entry(&executable, &icon);
    write_if_changed(&destination, entry.as_bytes());
    Some(destination)
}

fn write_if_changed(path: &Path, bytes: &[u8]) {
    match fs::read(path) {
        Ok(existing) if existing == bytes => {}
        _ => {
            let _ = fs::write(path, bytes);
        }
    }
}

/// StatusNotifier / ksni wants ARGB32 (network byte order): A,R,G,B per pixel.
pub fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.chunks_exact_mut(4) {
        px.rotate_right(1);
    }
    out
}

pub fn png_argb_pixmap(png: &[u8]) -> Option<(i32, i32, Vec<u8>)> {
    let img = image::load_from_memory(png).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some((w as i32, h as i32, rgba_to_argb(&img.into_raw())))
}

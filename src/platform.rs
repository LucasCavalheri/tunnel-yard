//! Platform profile directories, VPN engine choice, and helper lookup.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub type VpnEngine = &'static str;

pub fn normalize_platform(platform: &str) -> &str {
    match platform {
        "win32" | "windows" => "windows",
        "darwin" | "macos" => "macos",
        _ => "linux",
    }
}

pub fn current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// Node-style `process.platform` (`linux` / `darwin` / `win32`) for smoke JSON.
pub fn node_platform() -> &'static str {
    match current_platform() {
        "windows" => "win32",
        "macos" => "darwin",
        _ => "linux",
    }
}

pub fn engine_for_platform(platform: &str) -> VpnEngine {
    if normalize_platform(platform) == "windows" {
        "openconnect"
    } else {
        "openfortivpn"
    }
}

pub fn config_directory(platform: &str, home: Option<&str>) -> Result<String, String> {
    let home = home.map(str::to_string).or_else(|| {
        env::var("HOME")
            .ok()
            .or_else(|| dirs::home_dir().map(|p| p.to_string_lossy().into_owned()))
    });
    match normalize_platform(platform) {
        "linux" => Ok("/etc/openfortivpn".into()),
        "macos" => {
            let home = home.ok_or_else(|| "Unsupported platform: macos (no home)".to_string())?;
            Ok(user_profile_dir(
                format!("{home}/Library/Application Support/TunnelYard/profiles"),
                format!("{home}/Library/Application Support/My VPNs/profiles"),
            ))
        }
        "windows" => {
            let appdata = env::var("APPDATA")
                .ok()
                .or_else(|| home.as_ref().map(|h| format!("{h}/AppData/Roaming")));
            let appdata =
                appdata.ok_or_else(|| "Unsupported platform: windows (no APPDATA)".to_string())?;
            Ok(user_profile_dir(
                format!("{appdata}/TunnelYard/profiles"),
                format!("{appdata}/My VPNs/profiles"),
            ))
        }
        other => Err(format!("Unsupported platform: {other}")),
    }
}

pub fn config_directory_current() -> String {
    config_directory(current_platform(), None).expect("supported platform")
}

fn user_profile_dir(preferred: String, legacy: String) -> String {
    let preferred_path = PathBuf::from(&preferred);
    let legacy_path = PathBuf::from(&legacy);
    if preferred_path.exists() {
        return preferred;
    }
    if legacy_path.exists() {
        if copy_dir_files(&legacy_path, &preferred_path) {
            return preferred;
        }
        return legacy;
    }
    preferred
}

fn copy_dir_files(from: &Path, to: &Path) -> bool {
    if fs::create_dir_all(to).is_err() {
        return false;
    }
    let Ok(entries) = fs::read_dir(from) else {
        return false;
    };
    let mut ok = true;
    for entry in entries.flatten() {
        let dest = to.join(entry.file_name());
        if entry.path().is_file() && fs::copy(entry.path(), dest).is_err() {
            ok = false;
        }
    }
    ok
}

pub fn binary_candidates(engine: &str, platform: &str) -> Vec<String> {
    let name = if normalize_platform(platform) == "windows" {
        format!("{engine}.exe")
    } else {
        engine.to_string()
    };
    let mut dirs: Vec<String> = if normalize_platform(platform) == "windows" {
        ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"]
            .iter()
            .filter_map(|k| env::var(k).ok())
            .map(|p| format!("{p}/OpenConnect"))
            .collect()
    } else {
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
            "/usr/sbin",
            "/bin",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
    };
    let sep = if normalize_platform(platform) == "windows" {
        ';'
    } else {
        ':'
    };
    if let Ok(path) = env::var("PATH") {
        for part in path.split(sep) {
            if !part.is_empty() {
                dirs.push(part.to_string());
            }
        }
    }
    let mut out = Vec::new();
    for dir in dirs {
        let candidate = format!("{dir}/{name}");
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

pub fn find_vpn_binary(engine: Option<&str>) -> Option<String> {
    let engine = engine.unwrap_or_else(|| engine_for_platform(current_platform()));
    binary_candidates(engine, current_platform())
        .into_iter()
        .find(|candidate| {
            let p = Path::new(candidate);
            p.is_file() && is_executable(p)
        })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

pub fn helper_path(name: &str) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("helpers").join(name));
            candidates.push(dir.join("resources").join("helpers").join(name));
            candidates.push(dir.join("packaging").join(name));
        }
    }
    if let Ok(root) = env::var("APP_ROOT") {
        candidates.push(PathBuf::from(root).join("packaging").join(name));
    }
    candidates.push(PathBuf::from("packaging").join(name));
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("packaging")
            .join(name),
    );
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("packaging").join(name));
    }
    candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| format!("Missing VPN helper: {name}"))
}

pub fn linux_helpers() -> Option<(PathBuf, PathBuf)> {
    for lib in ["/usr/lib/tunnel-yard", "/usr/lib/my-vpns"] {
        let installed = (
            PathBuf::from(lib).join("run-vpn.sh"),
            PathBuf::from(lib).join("stop-vpn.sh"),
        );
        if installed.0.exists() && installed.1.exists() {
            return Some(installed);
        }
    }
    let mut roots = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("helpers"));
            roots.push(dir.join("resources").join("helpers"));
        }
    }
    if let Ok(root) = env::var("APP_ROOT") {
        roots.push(PathBuf::from(root).join("packaging"));
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("packaging"));
    if let Ok(cwd) = env::current_dir() {
        roots.push(cwd.join("packaging"));
    }
    for root in roots {
        let run = root.join("run-vpn.sh");
        let stop = root.join("stop-vpn.sh");
        if run.exists() && stop.exists() {
            return Some((run, stop));
        }
    }
    None
}

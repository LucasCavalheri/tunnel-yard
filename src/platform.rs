//! Linux profile directory, openfortivpn lookup, and PolicyKit helpers.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub type VpnEngine = &'static str;

pub fn normalize_platform(platform: &str) -> &str {
    match platform {
        "linux" | "gnu/linux" => "linux",
        other => other,
    }
}

pub fn current_platform() -> &'static str {
    "linux"
}

/// Node-style `process.platform` for smoke JSON.
pub fn node_platform() -> &'static str {
    "linux"
}

pub fn engine_for_platform(platform: &str) -> VpnEngine {
    let _ = platform;
    "openfortivpn"
}

pub fn config_directory(platform: &str, _home: Option<&str>) -> Result<String, String> {
    match normalize_platform(platform) {
        "linux" => Ok("/etc/openfortivpn".into()),
        other => Err(format!("Unsupported platform: {other}")),
    }
}

pub fn config_directory_current() -> String {
    "/etc/openfortivpn".into()
}

pub fn binary_candidates(engine: &str, _platform: &str) -> Vec<String> {
    let mut dirs = vec![
        "/usr/bin".into(),
        "/usr/sbin".into(),
        "/usr/local/bin".into(),
        "/usr/local/sbin".into(),
        "/bin".into(),
        "/sbin".into(),
        "/opt/bin".into(),
    ];
    if let Ok(home) = env::var("HOME") {
        dirs.push(format!("{home}/.local/bin"));
    }
    if let Ok(path) = env::var("PATH") {
        for part in path.split(':') {
            if !part.is_empty() {
                dirs.push(part.to_string());
            }
        }
    }
    let mut out = Vec::new();
    for dir in dirs {
        let candidate = format!("{dir}/{engine}");
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

pub fn find_vpn_binary(engine: Option<&str>) -> Option<String> {
    let engine = engine.unwrap_or("openfortivpn");
    binary_candidates(engine, "linux")
        .into_iter()
        .find(|candidate| {
            let p = Path::new(candidate);
            p.is_file() && is_executable(p)
        })
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
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

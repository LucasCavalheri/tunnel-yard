//! GitHub Releases check and one-click install for every supported OS.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::arch::current_arch;
use crate::platform::{current_platform, normalize_platform};
use crate::{APP_BIN, APP_NAME, GITHUB_OWNER, GITHUB_REPO};

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateArtifact {
    pub name: String,
    pub url: String,
    pub kind: String,
    pub digest: Option<String>,
    pub platform: Option<String>,
    pub architecture: Option<String>,
    pub compatible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub url: String,
    pub artifacts: Vec<UpdateArtifact>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateCheckResult {
    UpToDate { current: String },
    Available(UpdateInfo),
    Error { message: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallKind {
    Deb,
    Rpm,
    LinuxPortable,
    MacApp,
    MacPortable,
    WindowsPortable,
    WindowsManaged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateApplyResult {
    /// When set, the UI should spawn this path and then quit.
    /// When `None`, a helper already waits for this process to exit and relaunches.
    pub relaunch: Option<PathBuf>,
}

pub fn compare_versions(a: &str, b: &str) -> i32 {
    let parse = |v: &str| {
        v.trim_start_matches(['v', 'V'])
            .split(['.', '+', '-'])
            .map(|part| part.parse::<i32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let pa = parse(a);
    let pb = parse(b);
    let len = pa.len().max(pb.len());
    for i in 0..len {
        let da = pa.get(i).copied().unwrap_or(0);
        let db = pb.get(i).copied().unwrap_or(0);
        if da != db {
            return da - db;
        }
    }
    0
}

pub fn normalize_tag(tag: &str) -> String {
    tag.trim().trim_start_matches(['v', 'V']).trim().to_string()
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: Option<String>,
    html_url: Option<String>,
    draft: Option<bool>,
    assets: Option<Vec<GithubAsset>>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: Option<String>,
    browser_download_url: Option<String>,
    digest: Option<String>,
}

fn branded_prefix(lower: &str) -> bool {
    lower.starts_with("tunnel-yard-") || lower.starts_with("my-vpns-")
}

pub fn artifact_kind(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.contains("windows")
        && (lower.ends_with(".exe")
            || lower.starts_with("my-vpns-windows-")
            || lower.starts_with("tunnel-yard-windows-"))
    {
        Some("windows")
    } else if (lower.contains("macos") || lower.contains("-mac-"))
        && (lower.ends_with(".dmg")
            || lower.ends_with(".zip")
            || lower.starts_with("my-vpns-macos")
            || lower.starts_with("tunnel-yard-macos"))
    {
        Some("macos")
    } else if lower.starts_with("my-vpns-linux-") || lower.starts_with("tunnel-yard-linux-") {
        Some("linux")
    } else if lower.ends_with(".deb") {
        Some("deb")
    } else if lower.ends_with(".rpm") {
        Some("rpm")
    } else if branded_prefix(&lower) && !lower.contains('.') {
        if lower.contains("windows") {
            Some("windows")
        } else if lower.contains("macos") {
            Some("macos")
        } else if lower.contains("linux") {
            Some("linux")
        } else {
            None
        }
    } else {
        None
    }
}

pub fn artifact_platform(name: &str) -> Option<&'static str> {
    match artifact_kind(name) {
        Some("windows") => Some("windows"),
        Some("macos") => Some("macos"),
        Some("linux") | Some("deb") | Some("rpm") => Some("linux"),
        _ => None,
    }
}

pub fn artifact_architecture(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.contains("universal") {
        Some("universal")
    } else if lower.contains("arm64") || lower.contains("aarch64") {
        Some("arm64")
    } else if lower.contains("x86_64") || lower.contains("amd64") || lower.contains("-x64") {
        Some("x64")
    } else if lower.contains("-x86") || lower.contains("i386") || lower.contains("i686") {
        Some("x86")
    } else {
        None
    }
}

pub fn artifact_is_compatible(name: &str) -> bool {
    let Some(platform) = artifact_platform(name) else {
        return true;
    };
    if platform != current_platform() {
        return false;
    }
    let Some(architecture) = artifact_architecture(name) else {
        return true;
    };
    architecture == "universal" || architecture == current_arch()
}

fn artifact_kind_for_release(name: &str) -> Option<&'static str> {
    if name.to_ascii_lowercase().ends_with(".deb") {
        Some("deb")
    } else if name.to_ascii_lowercase().ends_with(".rpm") {
        Some("rpm")
    } else {
        artifact_kind(name)
    }
}

fn release_artifacts(assets: &[GithubAsset]) -> Vec<UpdateArtifact> {
    assets
        .iter()
        .filter_map(|asset| {
            let name = asset.name.as_ref()?.trim();
            let url = asset.browser_download_url.as_ref()?.trim();
            let kind = artifact_kind_for_release(name)?;
            if !url.to_ascii_lowercase().starts_with("https://github.com/") {
                return None;
            }
            let digest = asset.digest.as_ref().and_then(|d| {
                let d = d.to_ascii_lowercase();
                d.strip_prefix("sha256:").map(|s| s.to_string())
            });
            Some(UpdateArtifact {
                name: name.to_string(),
                url: url.to_string(),
                kind: kind.to_string(),
                digest,
                platform: artifact_platform(name).map(str::to_string),
                architecture: artifact_architecture(name).map(str::to_string),
                compatible: artifact_is_compatible(name),
            })
        })
        .collect()
}

pub fn parse_github_release_json(
    body: &str,
    owner: &str,
    repo: &str,
) -> Result<Option<(String, String, Vec<UpdateArtifact>)>, String> {
    let data: GithubRelease = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if data.draft.unwrap_or(false) {
        return Ok(None);
    }
    let Some(tag) = data.tag_name.filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    let artifacts = data
        .assets
        .as_deref()
        .map(release_artifacts)
        .unwrap_or_default();
    let url = data
        .html_url
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| format!("https://github.com/{owner}/{repo}/releases"));
    Ok(Some((normalize_tag(&tag), url, artifacts)))
}

/// `fetch_body(url) -> JSON`. Production uses `download_github_json`.
pub fn check_for_app_update(
    current_version: &str,
    fetch_body: impl FnOnce(&str) -> Result<String, String>,
) -> Result<Option<UpdateInfo>, String> {
    check_for_app_update_repo(current_version, GITHUB_OWNER, GITHUB_REPO, fetch_body)
}

pub fn check_for_app_update_repo(
    current_version: &str,
    owner: &str,
    repo: &str,
    fetch_body: impl FnOnce(&str) -> Result<String, String>,
) -> Result<Option<UpdateInfo>, String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let body = fetch_body(&url)?;
    let Some((latest, url, artifacts)) = parse_github_release_json(&body, owner, repo)? else {
        return Ok(None);
    };
    if compare_versions(&latest, current_version) <= 0 {
        return Ok(None);
    }
    Ok(Some(UpdateInfo {
        current: normalize_tag(current_version),
        latest,
        url,
        artifacts,
    }))
}

pub fn download_github_json(url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", APP_BIN)
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| e.to_string())?;
    if resp.status() != 200 {
        return Err(format!("GitHub API HTTP {}", resp.status()));
    }
    resp.into_string().map_err(|e| e.to_string())
}

pub fn perform_update_check(current_version: &str) -> UpdateCheckResult {
    match check_for_app_update(current_version, download_github_json) {
        Ok(None) => UpdateCheckResult::UpToDate {
            current: normalize_tag(current_version),
        },
        Ok(Some(info)) => UpdateCheckResult::Available(info),
        Err(message) => UpdateCheckResult::Error { message },
    }
}

pub const FIRST_CHECK_DELAY_MS: u64 = 10_000;
pub const CHECK_INTERVAL_MS: u64 = 6 * 60 * 60 * 1000;
pub const BASE_RETRY_DELAY_MS: u64 = 5 * 60 * 1000;
pub const MAX_RETRY_DELAY_MS: u64 = 60 * 60 * 1000;

pub fn retry_delay_ms(failures: u32) -> u64 {
    let n = failures.max(1).min(16);
    (BASE_RETRY_DELAY_MS.saturating_mul(2u64.pow(n - 1))).min(MAX_RETRY_DELAY_MS)
}

pub fn next_check_delay_ms(last_attempt_at: Option<u64>, consecutive_failures: u32) -> u64 {
    if last_attempt_at.is_none() {
        FIRST_CHECK_DELAY_MS
    } else if consecutive_failures > 0 {
        retry_delay_ms(consecutive_failures)
    } else {
        CHECK_INTERVAL_MS
    }
}

pub fn linux_package_kind(dpkg_owns: bool, rpm_owns: bool) -> InstallKind {
    if dpkg_owns {
        InstallKind::Deb
    } else if rpm_owns {
        InstallKind::Rpm
    } else {
        InstallKind::LinuxPortable
    }
}

pub fn install_kind_for(platform: &str, exe: &Path) -> InstallKind {
    let lower = exe.to_string_lossy().to_lowercase();
    match normalize_platform(platform) {
        "windows" => {
            if lower.contains("program files") {
                InstallKind::WindowsManaged
            } else {
                InstallKind::WindowsPortable
            }
        }
        "macos" => {
            if lower.contains(".app/contents/macos") {
                InstallKind::MacApp
            } else {
                InstallKind::MacPortable
            }
        }
        _ => InstallKind::LinuxPortable,
    }
}

pub fn detect_install_kind() -> InstallKind {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from(APP_BIN));
    match current_platform() {
        "windows" | "macos" => install_kind_for(current_platform(), &exe),
        _ => {
            let path = exe.to_string_lossy().into_owned();
            let dpkg = command_succeeds("dpkg", &["-S", &path]);
            let rpm = command_succeeds("rpm", &["-qf", &path]);
            linux_package_kind(dpkg, rpm)
        }
    }
}

fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn artifact_sort_key(artifact: &UpdateArtifact, kind: InstallKind) -> i32 {
    let name = artifact.name.to_lowercase();
    match kind {
        InstallKind::MacApp => {
            if name.ends_with(".dmg") {
                0
            } else {
                1
            }
        }
        InstallKind::MacPortable => {
            if name.ends_with(".dmg") || name.ends_with(".zip") {
                2
            } else {
                0
            }
        }
        InstallKind::LinuxPortable | InstallKind::Deb | InstallKind::Rpm => {
            if name.ends_with(".tar.gz") {
                2
            } else {
                0
            }
        }
        _ => 0,
    }
}

pub fn select_install_artifact(info: &UpdateInfo, kind: InstallKind) -> Option<&UpdateArtifact> {
    let wanted: &[&str] = match kind {
        InstallKind::Deb => &["deb", "linux"],
        InstallKind::Rpm => &["rpm", "linux"],
        InstallKind::LinuxPortable => &["linux"],
        InstallKind::MacApp | InstallKind::MacPortable => &["macos"],
        InstallKind::WindowsPortable | InstallKind::WindowsManaged => &["windows"],
    };
    let compatible: Vec<&UpdateArtifact> = info
        .artifacts
        .iter()
        .filter(|artifact| artifact.compatible)
        .collect();
    for want in wanted {
        let mut matches: Vec<&UpdateArtifact> = compatible
            .iter()
            .copied()
            .filter(|artifact| artifact.kind == *want)
            .collect();
        if matches.is_empty() {
            continue;
        }
        matches.sort_by_key(|artifact| artifact_sort_key(artifact, kind));
        return Some(matches[0]);
    }
    None
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("Could not read update file: {e}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Could not hash update file: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn verify_sha256_file(path: &Path, expected: &str) -> Result<(), String> {
    let got = sha256_file(path)?;
    if got != expected.to_ascii_lowercase() {
        return Err("Update checksum mismatch. Refusing to install it.".into());
    }
    Ok(())
}

pub fn download_https_file(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let resp = ureq::get(url)
        .set("User-Agent", APP_BIN)
        .timeout(Duration::from_secs(180))
        .call()
        .map_err(|e| format!("Download failed ({e})."))?;
    if resp.status() != 200 {
        return Err(format!("Download failed (HTTP {}).", resp.status()));
    }
    let mut reader = resp.into_reader();
    let mut file = File::create(dest).map_err(|e| format!("Could not save update: {e}"))?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Download failed ({e})."))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("Could not save update: {e}"))?;
    }
    file.flush()
        .map_err(|e| format!("Could not save update: {e}"))?;
    Ok(())
}

fn elevation_declined(code: Option<i32>) -> bool {
    matches!(code, Some(126) | Some(127))
}

fn run_status(cmd: &mut Command) -> Result<(), String> {
    let status = cmd
        .status()
        .map_err(|e| format!("Could not start installer: {e}"))?;
    if status.success() {
        Ok(())
    } else if elevation_declined(status.code()) {
        Err("elevation-declined".into())
    } else {
        Err(format!(
            "Installer exited with status {}.",
            status.code().unwrap_or(1)
        ))
    }
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).map_err(|e| e.to_string())?;
    }
    let _ = path;
    Ok(())
}

fn sh_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn extract_linux_binary(archive: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    if archive
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".tar.gz") || n.ends_with(".tgz"))
    {
        run_status(
            Command::new("tar")
                .args(["-xzf", &archive.to_string_lossy(), "-C"])
                .arg(dest_dir),
        )?;
        let mut found = None;
        if let Ok(entries) = fs::read_dir(dest_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path != *archive {
                    found = Some(path);
                    break;
                }
            }
        }
        found.ok_or_else(|| "The Linux archive did not contain a binary.".into())
    } else {
        let dest = dest_dir.join(APP_BIN);
        fs::copy(archive, &dest).map_err(|e| e.to_string())?;
        Ok(dest)
    }
}

fn unix_replace_after_exit(staged: &Path, dest: &Path) -> Result<UpdateApplyResult, String> {
    make_executable(staged)?;
    let pid = std::process::id();
    let script = format!(
        "while kill -0 {pid} 2>/dev/null; do sleep 0.2; done; mv -f {staged} {dest} && chmod +x {dest} && exec {dest}",
        staged = sh_single_quote(&staged.to_string_lossy()),
        dest = sh_single_quote(&dest.to_string_lossy()),
    );
    Command::new("sh")
        .args(["-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not schedule relaunch: {e}"))?;
    Ok(UpdateApplyResult { relaunch: None })
}

fn windows_replace_after_exit(
    staged: &Path,
    dest: &Path,
    elevate: bool,
) -> Result<UpdateApplyResult, String> {
    let staged_s = staged.display().to_string().replace('"', "");
    let dest_s = dest.display().to_string().replace('"', "");
    let inner = format!(
        "ping 127.0.0.1 -n 2 >NUL & move /Y \"{staged_s}\" \"{dest_s}\" & start \"\" \"{dest_s}\""
    );
    if elevate {
        let script = format!(
            "$ErrorActionPreference='Stop'; $p=Start-Process -FilePath 'cmd.exe' -ArgumentList '/c {inner}' -Verb RunAs -Wait -PassThru; if ($null -eq $p) {{ exit 126 }}; exit $p.ExitCode",
            inner = inner.replace('\'', "''")
        );
        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .status()
            .map_err(|e| format!("Could not start elevated installer: {e}"))?;
        if !status.success() {
            if elevation_declined(status.code()) {
                return Err("elevation-declined".into());
            }
            return Err(format!(
                "Installer exited with status {}.",
                status.code().unwrap_or(1)
            ));
        }
        Ok(UpdateApplyResult { relaunch: None })
    } else {
        Command::new("cmd")
            .args(["/C", "start", "/b", "cmd", "/c", &inner])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Could not schedule relaunch: {e}"))?;
        Ok(UpdateApplyResult { relaunch: None })
    }
}

fn macos_bundle_from_exe(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()?.to_string_lossy() != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()?.to_string_lossy() != "Contents" {
        return None;
    }
    let app = contents.parent()?;
    if app.extension()?.to_string_lossy() == "app" {
        Some(app.to_path_buf())
    } else {
        None
    }
}

fn install_macos_dmg(dmg: &Path, dest_app: &Path) -> Result<PathBuf, String> {
    let mount = std::env::temp_dir().join(format!("{APP_BIN}-dmg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&mount);
    fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    let attach = Command::new("hdiutil")
        .args([
            "attach",
            "-nobrowse",
            "-quiet",
            "-mountpoint",
            &mount.to_string_lossy(),
            &dmg.to_string_lossy(),
        ])
        .status()
        .map_err(|e| format!("Could not mount the disk image: {e}"))?;
    if !attach.success() {
        let _ = fs::remove_dir_all(&mount);
        return Err("Could not mount the macOS disk image.".into());
    }
    let result = (|| {
        let mut app = None;
        if let Ok(entries) = fs::read_dir(&mount) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("app") {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    if name == format!("{APP_NAME}.app") {
                        app = Some(path);
                        break;
                    }
                    app = Some(path);
                }
            }
        }
        let src =
            app.ok_or_else(|| "The disk image did not contain an application.".to_string())?;
        run_status(Command::new("ditto").arg(&src).arg(dest_app))?;
        Ok(dest_app.to_path_buf())
    })();
    let _ = Command::new("hdiutil")
        .args(["detach", "-quiet", &mount.to_string_lossy()])
        .status();
    let _ = fs::remove_dir_all(&mount);
    result
}

fn pkexec_install_package(path: &Path, kind: InstallKind) -> Result<(), String> {
    let file = path.display().to_string();
    let script = match kind {
        InstallKind::Deb => format!(
            "set -e; if command -v apt-get >/dev/null; then DEBIAN_FRONTEND=noninteractive apt-get install -y {q}; else dpkg -i {q}; fi",
            q = sh_single_quote(&file)
        ),
        InstallKind::Rpm => format!(
            "set -e; if command -v dnf >/dev/null; then dnf install -y {q}; elif command -v yum >/dev/null; then yum install -y {q}; elif command -v zypper >/dev/null; then zypper --non-interactive install {q}; else rpm -Uvh {q}; fi",
            q = sh_single_quote(&file)
        ),
        _ => return Err("Not a Linux package install.".into()),
    };
    run_status(
        Command::new("pkexec")
            .args(["bash", "-c", &script])
            .stdin(Stdio::null()),
    )
}

/// Download the matching GitHub artifact and replace this installation.
pub fn perform_update_install(info: &UpdateInfo) -> Result<UpdateApplyResult, String> {
    let kind = detect_install_kind();
    let artifact = select_install_artifact(info, kind).ok_or_else(|| {
        "No compatible TunnelYard package was published for this operating system.".to_string()
    })?;
    let tmp = std::env::temp_dir().join(format!("{APP_BIN}-update-{}", std::process::id()));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let download_path = tmp.join(&artifact.name);
    download_https_file(&artifact.url, &download_path)?;
    if let Some(digest) = &artifact.digest {
        verify_sha256_file(&download_path, digest)?;
    }
    let exe = std::env::current_exe().map_err(|e| format!("Could not locate this binary: {e}"))?;
    let result = match kind {
        InstallKind::Deb | InstallKind::Rpm => {
            pkexec_install_package(&download_path, kind)?;
            let relaunch = PathBuf::from(format!("/usr/bin/{APP_BIN}"));
            Ok(UpdateApplyResult {
                relaunch: Some(if relaunch.exists() { relaunch } else { exe }),
            })
        }
        InstallKind::LinuxPortable => {
            let unpacked = extract_linux_binary(&download_path, &tmp)?;
            make_executable(&unpacked)?;
            let staged = exe.with_file_name(format!(
                "{}.new",
                exe.file_name().unwrap_or_default().to_string_lossy()
            ));
            fs::copy(&unpacked, &staged).map_err(|e| e.to_string())?;
            unix_replace_after_exit(&staged, &exe)
        }
        InstallKind::MacPortable => {
            make_executable(&download_path)?;
            let binary = if artifact.name.to_lowercase().ends_with(".dmg") {
                return Err("This portable macOS build cannot install a disk image.".into());
            } else {
                download_path
            };
            let staged = exe.with_file_name(format!(
                "{}.new",
                exe.file_name().unwrap_or_default().to_string_lossy()
            ));
            fs::copy(&binary, &staged).map_err(|e| e.to_string())?;
            unix_replace_after_exit(&staged, &exe)
        }
        InstallKind::MacApp => {
            let dest_app = macos_bundle_from_exe(&exe)
                .unwrap_or_else(|| PathBuf::from(format!("/Applications/{APP_NAME}.app")));
            if artifact.name.to_lowercase().ends_with(".dmg") {
                let installed = install_macos_dmg(&download_path, &dest_app)?;
                let relaunch = installed.join("Contents/MacOS").join(APP_BIN);
                Ok(UpdateApplyResult {
                    relaunch: Some(if relaunch.exists() {
                        relaunch
                    } else {
                        installed
                    }),
                })
            } else {
                make_executable(&download_path)?;
                let dest = dest_app.join("Contents/MacOS").join(
                    exe.file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new(APP_BIN)),
                );
                let staged = dest.with_file_name(format!(
                    "{}.new",
                    dest.file_name().unwrap_or_default().to_string_lossy()
                ));
                fs::copy(&download_path, &staged).map_err(|e| e.to_string())?;
                unix_replace_after_exit(&staged, &dest)
            }
        }
        InstallKind::WindowsPortable | InstallKind::WindowsManaged => {
            let staged = exe.with_extension("exe.new");
            fs::copy(&download_path, &staged).map_err(|e| e.to_string())?;
            windows_replace_after_exit(&staged, &exe, kind == InstallKind::WindowsManaged)
        }
    };
    let _ = fs::remove_dir_all(&tmp);
    result
}

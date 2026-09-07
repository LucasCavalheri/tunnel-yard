//! GitHub Releases check. The UI opens the release URL; it does not download installers.

use serde::Deserialize;

use crate::arch::current_arch;
use crate::platform::current_platform;

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

pub fn artifact_kind(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.contains("windows")
        && (lower.ends_with(".exe") || lower.starts_with("my-vpns-windows-"))
    {
        Some("windows")
    } else if (lower.contains("macos") || lower.contains("-mac-"))
        && (lower.ends_with(".dmg")
            || lower.ends_with(".zip")
            || lower.starts_with("my-vpns-macos"))
    {
        Some("macos")
    } else if lower.starts_with("my-vpns-linux-") {
        Some("linux")
    } else if lower.ends_with(".deb") {
        Some("deb")
    } else if lower.ends_with(".rpm") {
        Some("rpm")
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
    check_for_app_update_repo(current_version, "LucasCavalheri", "my-vpns", fetch_body)
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
        .set("User-Agent", "my-vpns")
        .timeout(std::time::Duration::from_secs(20))
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

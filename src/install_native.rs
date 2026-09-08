//! macOS Homebrew and Windows OpenConnect 9.21 bootstrap.

use crate::arch::{current_arch, native_windows_client_supported};
use crate::native::{encoded_powershell, powershell_path, ps_quote, secure_directory};
use crate::platform::current_platform;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
pub struct WindowsClient {
    pub version: String,
    pub url: String,
    pub sha256: String,
}

pub fn windows_client() -> WindowsClient {
    serde_json::from_str(include_str!("../packaging/windows-client.json"))
        .expect("packaging/windows-client.json")
}

pub fn windows_client_for_arch(arch: &str) -> Result<WindowsClient, String> {
    if native_windows_client_supported(arch) {
        Ok(windows_client())
    } else {
        Err(format!(
            "Windows {arch} includes the TunnelYard UI, but the pinned OpenConnect 9.21 installer is x64-only. Install a native ARM64 OpenConnect + Wintun package manually before connecting."
        ))
    }
}

pub fn windows_client_for_current_arch() -> Result<WindowsClient, String> {
    windows_client_for_arch(current_arch())
}

pub fn find_brew() -> Option<String> {
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        .into_iter()
        .find(|p| PathBuf::from(p).exists())
        .map(|s| s.to_string())
}

/// SHA-256 of the official OpenConnect installer (lowercase hex).
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn verify_openconnect_installer(bytes: &[u8], expected_sha256: &str) -> Result<(), String> {
    let got = sha256_hex(bytes);
    if got != expected_sha256.to_ascii_lowercase() {
        return Err("OpenConnect installer checksum mismatch. Refusing to execute it.".into());
    }
    Ok(())
}

/// Silent NSIS (`/S`) via UAC (`-Verb RunAs`).
pub fn windows_silent_install_script(installer: &str) -> String {
    format!(
        "$ErrorActionPreference='Stop'; $p=Start-Process -FilePath {} -ArgumentList '/S' -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode",
        ps_quote(installer)
    )
}

pub fn download_https(url: &str) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .timeout(Duration::from_secs(120))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("OpenConnect download failed ({code})."),
            other => format!("OpenConnect download failed ({other})."),
        })?;
    let status = resp.status();
    if status != 200 {
        return Err(format!("OpenConnect download failed ({status})."));
    }
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(200_000_000)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Write the verified installer with `wx` (do not overwrite an existing file).
pub fn stage_openconnect_installer(
    bytes: &[u8],
    dest: &Path,
    expected_sha256: &str,
) -> Result<(), String> {
    verify_openconnect_installer(bytes, expected_sha256)?;
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    opts.open(dest)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(bytes)
        })
        .map_err(|e| e.to_string())
}

pub fn run_uac_installer(installer: &Path) -> Result<(i32, String), String> {
    let args = encoded_powershell(&windows_silent_install_script(&installer.to_string_lossy()));
    let output = Command::new(powershell_path())
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok((output.status.code().unwrap_or(1), text))
}

/// Production Windows path: download pinned 9.21, verify SHA-256, UAC `/S`.
pub fn install_windows_openconnect_with_download(
    client: &WindowsClient,
    download: impl FnOnce(&str) -> Result<Vec<u8>, String>,
    on_log: &mut impl FnMut(&str),
    run_installer: impl FnOnce(&Path) -> Result<(i32, String), String>,
) -> (i32, String) {
    let dir = match tempfile_dir() {
        Ok(d) => d,
        Err(e) => return (1, e),
    };
    let installer = dir.join("openconnect-installer.exe");
    let result = (|| {
        secure_directory(&dir)?;
        on_log("Downloading OpenConnect 9.21 from the official release build…");
        let bytes = download(&client.url)?;
        stage_openconnect_installer(&bytes, &installer, &client.sha256)?;
        on_log("Checksum verified. Approve the Windows administrator prompt to install OpenConnect and Wintun.");
        run_installer(&installer)
    })();
    let _ = fs::remove_file(&installer);
    let _ = fs::remove_dir(&dir);
    match result {
        Ok(v) => v,
        Err(e) => (1, e),
    }
}

pub fn install_native_client(on_log: &mut impl FnMut(&str)) -> (i32, String) {
    if current_platform() == "macos" {
        let Some(brew) = find_brew() else {
            return (
                1,
                "Install Homebrew from https://brew.sh, then run: brew install openfortivpn".into(),
            );
        };
        on_log("brew install openfortivpn (runs as your user, never as root)");
        match Command::new(&brew)
            .args(["install", "openfortivpn"])
            .env("NONINTERACTIVE", "1")
            .env(
                "PATH",
                "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            )
            .output()
        {
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&out.stderr));
                (out.status.code().unwrap_or(1), text)
            }
            Err(err) => (1, err.to_string()),
        }
    } else if current_platform() == "windows" {
        let client = match windows_client_for_current_arch() {
            Ok(client) => client,
            Err(err) => return (1, err),
        };
        install_windows_openconnect_with_download(
            &client,
            download_https,
            on_log,
            run_uac_installer,
        )
    } else {
        (
            1,
            "Native client bootstrap is only available on macOS and Windows.".into(),
        )
    }
}

fn tempfile_dir() -> Result<PathBuf, String> {
    let root = std::env::temp_dir();
    for i in 0..100 {
        let dir = root.join(format!(
            "tunnel-yard-install-{}-{}-{i}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
        if fs::create_dir(&dir).is_ok() {
            return Ok(dir);
        }
    }
    Err("Could not create installer temp dir.".into())
}

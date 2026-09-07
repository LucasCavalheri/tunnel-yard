//! Distro detection, install plan, and VPN client dependency status.

use crate::install_native::{find_brew, install_native_client};
use crate::platform::{
    config_directory_current, current_platform, engine_for_platform, find_vpn_binary, node_platform,
};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Debug, PartialEq)]
pub struct DistroInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub like: Vec<String>,
    pub family: String,
    pub pretty: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DependencyStatus {
    pub engine: String,
    pub config_dir: String,
    pub platform: String,
    pub client_installed: bool,
    pub client_path: Option<String>,
    pub client_version: Option<String>,
    pub distro: DistroInfo,
    pub can_auto_install: bool,
    pub install_command: Option<String>,
}

#[derive(Clone, Debug)]
pub struct InstallResult {
    pub ok: bool,
    pub code: Option<i32>,
    pub output: String,
    pub status: DependencyStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InstallPlan {
    pub can_auto_install: bool,
    pub install_command: Option<String>,
    pub pkexec_args: Option<Vec<String>>,
}

pub fn parse_os_release_text(raw: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(eq) = trimmed.find('=') else {
            continue;
        };
        let key = trimmed[..eq].to_string();
        let mut value = trimmed[eq + 1..].trim().to_string();
        if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            value = value[1..value.len() - 1].to_string();
        }
        map.insert(key, value);
    }
    map
}

pub fn detect_package_family(id: &str, like: &[String], exists: impl Fn(&str) -> bool) -> String {
    let tokens: Vec<String> = std::iter::once(id.to_string())
        .chain(like.iter().cloned())
        .map(|t| t.to_lowercase())
        .collect();
    let has = |names: &[&str]| tokens.iter().any(|t| names.contains(&t.as_str()));
    if has(&[
        "debian",
        "ubuntu",
        "linuxmint",
        "pop",
        "elementary",
        "raspbian",
        "zorin",
    ]) {
        return "apt".into();
    }
    if has(&[
        "fedora",
        "rhel",
        "centos",
        "rocky",
        "almalinux",
        "ol",
        "nobara",
    ]) {
        if exists("/usr/bin/dnf") || exists("/usr/bin/dnf5") {
            return "dnf".into();
        }
        if exists("/usr/bin/yum") {
            return "yum".into();
        }
        return "dnf".into();
    }
    if tokens
        .iter()
        .any(|t| t.contains("suse") || t == "opensuse" || t == "sles")
    {
        return "zypper".into();
    }
    if has(&["arch", "manjaro", "endeavouros", "garuda", "artix"]) {
        return "pacman".into();
    }
    if exists("/usr/bin/apt-get") {
        return "apt".into();
    }
    if exists("/usr/bin/dnf") || exists("/usr/bin/dnf5") {
        return "dnf".into();
    }
    if exists("/usr/bin/yum") {
        return "yum".into();
    }
    if exists("/usr/bin/zypper") {
        return "zypper".into();
    }
    if exists("/usr/bin/pacman") {
        return "pacman".into();
    }
    let _ = tokens;
    "unknown".into()
}

pub fn build_install_plan(family: &str) -> InstallPlan {
    match family {
        "apt" => InstallPlan {
            can_auto_install: true,
            install_command: Some("apt-get install -y openfortivpn".into()),
            pkexec_args: Some(vec![
                "env".into(),
                "DEBIAN_FRONTEND=noninteractive".into(),
                "apt-get".into(),
                "install".into(),
                "-y".into(),
                "openfortivpn".into(),
            ]),
        },
        "dnf" => InstallPlan {
            can_auto_install: true,
            install_command: Some("dnf install -y openfortivpn".into()),
            pkexec_args: Some(vec![
                "dnf".into(),
                "install".into(),
                "-y".into(),
                "openfortivpn".into(),
            ]),
        },
        "yum" => InstallPlan {
            can_auto_install: true,
            install_command: Some("yum install -y openfortivpn".into()),
            pkexec_args: Some(vec![
                "yum".into(),
                "install".into(),
                "-y".into(),
                "openfortivpn".into(),
            ]),
        },
        "zypper" => InstallPlan {
            can_auto_install: true,
            install_command: Some("zypper --non-interactive install openfortivpn".into()),
            pkexec_args: Some(vec![
                "zypper".into(),
                "--non-interactive".into(),
                "install".into(),
                "openfortivpn".into(),
            ]),
        },
        "pacman" => InstallPlan {
            can_auto_install: true,
            install_command: Some("pacman -S --noconfirm openfortivpn".into()),
            pkexec_args: Some(vec![
                "pacman".into(),
                "-S".into(),
                "--noconfirm".into(),
                "openfortivpn".into(),
            ]),
        },
        _ => InstallPlan {
            can_auto_install: false,
            install_command: None,
            pkexec_args: None,
        },
    }
}

fn parse_os_release() -> std::collections::HashMap<String, String> {
    for file in ["/etc/os-release", "/usr/lib/os-release"] {
        if let Ok(raw) = fs::read_to_string(file) {
            return parse_os_release_text(&raw);
        }
    }
    Default::default()
}

pub fn detect_distro() -> DistroInfo {
    let platform = current_platform();
    if platform == "macos" || platform == "windows" {
        let mac = platform == "macos";
        return DistroInfo {
            id: if mac { "darwin" } else { "win32" }.into(),
            name: if mac { "macOS" } else { "Windows" }.into(),
            version: std::env::consts::OS.to_string(),
            like: vec![],
            family: if mac { "brew" } else { "windows" }.into(),
            pretty: if mac { "macOS" } else { "Windows" }.into(),
        };
    }
    let os = parse_os_release();
    let id = os
        .get("ID")
        .cloned()
        .unwrap_or_else(|| "linux".into())
        .to_lowercase();
    let like: Vec<String> = os
        .get("ID_LIKE")
        .map(|s| {
            s.split_whitespace()
                .map(|t| t.trim().to_lowercase())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let name = os.get("NAME").cloned().unwrap_or_else(|| id.clone());
    let version = os
        .get("VERSION_ID")
        .cloned()
        .or_else(|| os.get("VERSION").cloned())
        .unwrap_or_default();
    let family = detect_package_family(&id, &like, |p| Path::new(p).exists());
    let pretty = os
        .get("PRETTY_NAME")
        .cloned()
        .unwrap_or_else(|| format!("{name} {version}").trim().to_string());
    DistroInfo {
        id,
        name,
        version,
        like,
        family,
        pretty,
    }
}

fn read_client_version(bin: &str) -> Option<String> {
    let output = Command::new(bin)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let text = text.trim().to_string();
    if current_platform() == "windows" {
        let dll = Path::new(bin).parent().map(|p| p.join("wintun.dll"));
        if !text.to_lowercase().contains("fortinet") || !dll.map(|p| p.exists()).unwrap_or(false) {
            return None;
        }
    }
    text.lines()
        .find(|line| {
            let l = line.to_lowercase();
            l.contains("version") || {
                let mut chars = line.trim().chars();
                chars.next().map(|c| c.is_ascii_digit()).unwrap_or(false) && line.contains('.')
            }
        })
        .map(|s| s.to_string())
        .or(None)
}

pub fn get_dependency_status() -> DependencyStatus {
    let distro = detect_distro();
    let plan = build_install_plan(&distro.family);
    let client_path = find_vpn_binary(None);
    let client_version = client_path.as_deref().and_then(read_client_version);
    let client_installed = client_path.is_some() && client_version.is_some();
    let platform = current_platform();
    let can_auto_install = if platform == "windows" {
        true
    } else if platform == "macos" {
        find_brew().is_some()
    } else {
        plan.can_auto_install
    };
    let install_command = if platform == "windows" {
        Some("OpenConnect 9.21 + Wintun (Windows administrator prompt)".into())
    } else if platform == "macos" {
        Some("brew install openfortivpn".into())
    } else {
        plan.install_command.as_ref().map(|c| format!("pkexec {c}"))
    };
    DependencyStatus {
        engine: engine_for_platform(platform).into(),
        config_dir: config_directory_current(),
        platform: node_platform().into(),
        client_installed,
        client_path,
        client_version,
        distro,
        can_auto_install,
        install_command,
    }
}

fn run_pkexec(args: &[String]) -> (Option<i32>, String) {
    let mut child = match Command::new("pkexec")
        .args(args)
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => return (Some(1), err.to_string()),
    };
    let mut output = String::new();
    if let Some(mut out) = child.stdout.take() {
        let mut buf = String::new();
        let _ = out.read_to_string(&mut buf);
        output.push_str(&buf);
    }
    if let Some(mut err) = child.stderr.take() {
        let mut buf = String::new();
        let _ = err.read_to_string(&mut buf);
        output.push_str(&buf);
    }
    let code = child.wait().ok().and_then(|s| s.code());
    if output.len() > 20_000 {
        output = output[output.len() - 20_000..].to_string();
    }
    (code, output.trim().to_string())
}

pub fn install_vpn_client(mut on_log: impl FnMut(&str)) -> InstallResult {
    let before = get_dependency_status();
    if before.client_installed {
        return InstallResult {
            ok: true,
            code: Some(0),
            output: "Já instalado".into(),
            status: before,
        };
    }
    let platform = current_platform();
    if platform == "macos" || platform == "windows" {
        let (code, output) = install_native_client(&mut on_log);
        let status = get_dependency_status();
        return InstallResult {
            ok: status.client_installed,
            code: Some(code),
            output,
            status,
        };
    }
    let plan = build_install_plan(&before.distro.family);
    let Some(args) = plan.pkexec_args else {
        return InstallResult {
            ok: false,
            code: Some(1),
            output: "Distro não suportada para instalação automática. Instale o openfortivpn manualmente.".into(),
            status: before,
        };
    };
    on_log(&format!("Distro detectada: {}", before.distro.pretty));
    on_log(&format!("Família de pacotes: {}", before.distro.family));
    on_log(&format!("Comando: pkexec {}", args.join(" ")));
    let (code, output) = run_pkexec(&args);
    if !output.is_empty() {
        for line in output.lines() {
            if !line.trim().is_empty() {
                on_log(line.trim());
            }
        }
    }
    let status = get_dependency_status();
    if !status.client_installed && (code == Some(126) || code == Some(127)) {
        return InstallResult {
            ok: false,
            code,
            output: "Autenticação cancelada ou pkexec indisponível.".into(),
            status,
        };
    }
    let output = if output.is_empty() {
        if status.client_installed {
            "Instalação concluída".into()
        } else {
            "Falha ao instalar openfortivpn".into()
        }
    } else {
        output
    };
    InstallResult {
        ok: status.client_installed,
        code,
        output,
        status,
    }
}

//! openfortivpn `.conf` parse/serialize, profile ids, and file CRUD.

use crate::openconnect::{
    conf_entries, profile_health_check, profile_legacy_tunnel, profile_no_dtls,
};
use crate::platform::{config_directory_current, current_platform};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VpnProfileDraft {
    pub id: String,
    pub host: String,
    pub port: u32,
    pub username: String,
    pub password: String,
    pub trusted_cert: String,
    pub set_dns: bool,
    pub set_routes: bool,
    pub realm: String,
    pub persistent: u32,
    pub health_host: Option<String>,
    pub health_port: Option<u16>,
    pub no_dtls: bool,
    pub legacy_tunnel: bool,
    pub extra_options: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileWriteResult {
    pub ok: bool,
    pub message: String,
    pub profile: Option<VpnProfile>,
}

pub fn empty_draft() -> VpnProfileDraft {
    VpnProfileDraft {
        port: 10443,
        set_dns: false,
        set_routes: true,
        ..VpnProfileDraft::default()
    }
}

pub fn slugify_profile_id(raw: &str) -> String {
    let stripped = raw.trim().to_lowercase();
    let stripped = if stripped.to_lowercase().ends_with(".conf") {
        let end = stripped.len().saturating_sub(5);
        stripped[..end].to_string()
    } else {
        stripped
    };
    let nfd: String = stripped
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect();
    let mut out = String::new();
    let mut dash = false;
    for c in nfd.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    out.chars().take(48).collect()
}

pub fn is_valid_profile_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || bytes.len() > 48 {
        return false;
    }
    let first = bytes[0];
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-' || *b == b'_')
}

pub fn parse_conf_map(raw: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some(eq) = trimmed.find('=') else {
            continue;
        };
        let key = trimmed[..eq].trim().to_lowercase();
        let value = trimmed[eq + 1..].trim().to_string();
        map.insert(key, value);
    }
    map
}

#[derive(Clone, Debug, PartialEq)]
pub struct VpnProfile {
    pub id: String,
    pub name: String,
    pub path: String,
    pub host: String,
    pub port: u32,
    pub username: String,
    pub set_dns: bool,
    pub set_routes: bool,
    pub has_password: bool,
    pub has_trusted_cert: bool,
}

pub fn display_name(file_name: &str) -> String {
    let base = strip_conf_suffix(file_name);
    base.split(|c| c == '-' || c == '_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse_vpn_conf_content(raw: &str, file_path: &str) -> Option<VpnProfile> {
    let map = parse_conf_map(raw);
    let file_name = base_name(file_path);
    let id = strip_conf_suffix(&file_name);
    let host = map.get("host")?.trim().to_string();
    if host.is_empty() {
        return None;
    }
    let username = map
        .get("username")
        .cloned()
        .or_else(|| map.get("user").cloned())
        .unwrap_or_default();
    Some(VpnProfile {
        id,
        name: display_name(&file_name),
        path: file_path.to_string(),
        host,
        port: parse_port(&map),
        username,
        set_dns: parse_bool_lenient(map.get("set-dns").map(String::as_str), true),
        set_routes: parse_bool_lenient(map.get("set-routes").map(String::as_str), true),
        has_password: map.get("password").map(|s| !s.is_empty()).unwrap_or(false),
        has_trusted_cert: map
            .get("trusted-cert")
            .map(|s| !s.is_empty())
            .unwrap_or(false),
    })
}

pub fn parse_bool_lenient(value: Option<&str>, fallback: bool) -> bool {
    match value.map(|s| s.trim().to_lowercase()) {
        None => fallback,
        Some(v) if matches!(v.as_str(), "1" | "true" | "yes" | "on") => true,
        Some(v) if matches!(v.as_str(), "0" | "false" | "no" | "off") => false,
        Some(_) => fallback,
    }
}

fn parse_port(map: &HashMap<String, String>) -> u32 {
    map.get("port")
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|n| *n != 0)
        .unwrap_or(443)
}

fn parse_persistent(map: &HashMap<String, String>) -> u32 {
    map.get("persistent")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0)
}

fn base_name(path_or_id: &str) -> String {
    Path::new(path_or_id)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path_or_id)
        .to_string()
}

fn strip_conf_suffix(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".conf") {
        name[..name.len() - 5].to_string()
    } else {
        name.to_string()
    }
}

const OWNED_KEYS: &[&str] = &[
    "host",
    "port",
    "username",
    "user",
    "password",
    "trusted-cert",
    "set-dns",
    "set-routes",
    "realm",
    "persistent",
];

pub fn parse_vpn_draft(
    raw: &str,
    file_path_or_id: &str,
) -> Result<Option<VpnProfileDraft>, String> {
    let map = parse_conf_map(raw);
    let Some(host) = map.get("host").cloned() else {
        return Ok(None);
    };
    let base = strip_conf_suffix(&base_name(file_path_or_id));
    let id = if is_valid_profile_id(&base) {
        base
    } else {
        let s = slugify_profile_id(&base);
        if s.is_empty() {
            "vpn".into()
        } else {
            s
        }
    };
    let health = profile_health_check(raw)?;
    let entries = conf_entries(raw)?;
    let last_trusted = entries.iter().rposition(|(k, _)| k == "trusted-cert");
    let extra_options = entries
        .iter()
        .enumerate()
        .filter(|(index, (key, _))| {
            (key == "trusted-cert" && Some(*index) != last_trusted)
                || !OWNED_KEYS.contains(&key.as_str())
        })
        .map(|(_, pair)| pair.clone())
        .collect();

    Ok(Some(VpnProfileDraft {
        id,
        host: host.trim().to_string(),
        port: parse_port(&map),
        username: map
            .get("username")
            .cloned()
            .or_else(|| map.get("user").cloned())
            .unwrap_or_default(),
        password: map.get("password").cloned().unwrap_or_default(),
        trusted_cert: map.get("trusted-cert").cloned().unwrap_or_default(),
        set_dns: parse_bool_lenient(map.get("set-dns").map(String::as_str), true),
        set_routes: parse_bool_lenient(map.get("set-routes").map(String::as_str), true),
        realm: map.get("realm").cloned().unwrap_or_default(),
        persistent: parse_persistent(&map),
        health_host: health.health_host,
        health_port: health.health_port,
        no_dtls: profile_no_dtls(raw),
        legacy_tunnel: profile_legacy_tunnel(raw),
        extra_options,
    }))
}

fn reject_multiline(value: &str) -> Result<(), String> {
    if value.contains('\r') || value.contains('\n') || value.contains('\0') {
        Err("Profile fields must be single-line text.".into())
    } else {
        Ok(())
    }
}

pub fn serialize_vpn_draft(draft: &VpnProfileDraft) -> Result<String, String> {
    for value in [
        &draft.host,
        &draft.username,
        &draft.password,
        &draft.trusted_cert,
        &draft.realm,
    ] {
        reject_multiline(value)?;
    }
    let port = if draft.port == 0 { 443 } else { draft.port };
    let mut lines: Vec<String> = vec![
        format!("host = {}", draft.host.trim()),
        format!("port = {port}"),
        String::new(),
        format!("username = {}", draft.username.trim()),
    ];
    if !draft.password.is_empty() {
        lines.push(format!("password = {}", draft.password));
    }
    lines.push(String::new());
    if !draft.trusted_cert.trim().is_empty() {
        lines.push(format!("trusted-cert = {}", draft.trusted_cert.trim()));
        lines.push(String::new());
    }
    if !draft.realm.trim().is_empty() {
        lines.push(format!("realm = {}", draft.realm.trim()));
    }
    lines.push(format!("set-dns = {}", if draft.set_dns { 1 } else { 0 }));
    lines.push(format!(
        "set-routes = {}",
        if draft.set_routes { 1 } else { 0 }
    ));
    if draft.persistent > 0 {
        lines.push(format!("persistent = {}", draft.persistent));
    }
    for (key, value) in &draft.extra_options {
        if !is_extra_key(key)
            || value.contains('\r')
            || value.contains('\n')
            || value.contains('\0')
        {
            return Err("Invalid extra profile option.".into());
        }
        if matches!(
            key.as_str(),
            "host"
                | "port"
                | "username"
                | "user"
                | "password"
                | "set-dns"
                | "set-routes"
                | "realm"
                | "persistent"
        ) {
            return Err(format!("Duplicate profile option: {key}"));
        }
        lines.push(format!("{key} = {value}"));
    }
    if draft.health_host.is_some() || draft.health_port.is_some() {
        let host = draft.health_host.clone().unwrap_or_default();
        if host.chars().any(|c| c.is_whitespace()) || host.contains('\0') {
            return Err("Invalid VPN health-check address.".into());
        }
        let metadata = format!(
            "# tunnel-yard-health-host = {}\n# tunnel-yard-health-port = {}",
            host,
            draft.health_port.map(|p| p.to_string()).unwrap_or_default()
        );
        profile_health_check(&metadata)?;
        lines.push(String::new());
        lines.push(metadata);
    }
    if draft.no_dtls {
        lines.push(String::new());
        lines.push("# tunnel-yard-no-dtls = 1".into());
    }
    if draft.legacy_tunnel {
        lines.push(String::new());
        lines.push("# tunnel-yard-legacy-tunnel = 1".into());
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}

fn is_extra_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {
            chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        }
        _ => false,
    }
}

pub fn conf_path_for_id_in(id: &str, dir: &Path) -> Result<PathBuf, String> {
    if !is_valid_profile_id(id) {
        return Err("Invalid profile id.".into());
    }
    Ok(dir.join(format!("{id}.conf")))
}

pub fn conf_path_for_id(id: &str) -> Result<PathBuf, String> {
    conf_path_for_id_in(id, Path::new(&config_directory_current()))
}

pub fn read_profile_draft(id: &str) -> Option<VpnProfileDraft> {
    let path = conf_path_for_id(id).ok()?;
    let raw = fs::read_to_string(path).ok()?;
    parse_vpn_draft(&raw, &format!("{id}.conf")).ok().flatten()
}

pub fn draft_from_imported_file(file_path: &Path) -> (bool, String, Option<VpnProfileDraft>) {
    match fs::read_to_string(file_path) {
        Ok(raw) => match parse_vpn_draft(&raw, &file_path.to_string_lossy()) {
            Ok(Some(draft)) => (true, "Parsed".into(), Some(draft)),
            Ok(None) => (false, "Could not parse conf (host missing?).".into(), None),
            Err(err) => (false, err, None),
        },
        Err(err) => (false, err.to_string(), None),
    }
}

fn run_pkexec(args: &[&str], stdin: Option<&str>) -> (Option<i32>, String) {
    let mut cmd = Command::new("pkexec");
    cmd.args(args)
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    match cmd.spawn() {
        Ok(mut child) => {
            if let Some(data) = stdin {
                if let Some(mut sin) = child.stdin.take() {
                    let _ = sin.write_all(data.as_bytes());
                }
            }
            match child.wait_with_output() {
                Ok(out) => {
                    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                    text.push_str(&String::from_utf8_lossy(&out.stderr));
                    (out.status.code(), text.trim().to_string())
                }
                Err(err) => (Some(1), err.to_string()),
            }
        }
        Err(err) => (Some(1), err.to_string()),
    }
}

pub fn save_profile_draft(draft: &VpnProfileDraft, overwrite: bool) -> ProfileWriteResult {
    let id = if is_valid_profile_id(&draft.id) {
        draft.id.clone()
    } else {
        slugify_profile_id(&draft.id)
    };
    if !is_valid_profile_id(&id) {
        return ProfileWriteResult {
            ok: false,
            message: "Invalid profile id. Use letters, numbers, - or _.".into(),
            profile: None,
        };
    }
    if draft.host.trim().is_empty() {
        return ProfileWriteResult {
            ok: false,
            message: "Host is required.".into(),
            profile: None,
        };
    }
    let dest = match conf_path_for_id(&id) {
        Ok(p) => p,
        Err(message) => {
            return ProfileWriteResult {
                ok: false,
                message,
                profile: None,
            }
        }
    };
    if !overwrite && dest.exists() {
        return ProfileWriteResult {
            ok: false,
            message: format!("Profile \"{id}\" already exists."),
            profile: None,
        };
    }
    let mut to_write = draft.clone();
    to_write.id = id.clone();
    let content = match serialize_vpn_draft(&to_write) {
        Ok(c) => c,
        Err(message) => {
            return ProfileWriteResult {
                ok: false,
                message,
                profile: None,
            }
        }
    };
    if current_platform() != "linux" {
        if let Err(err) = crate::native::secure_directory(dest.parent().unwrap_or(Path::new("."))) {
            return ProfileWriteResult {
                ok: false,
                message: err,
                profile: None,
            };
        }
        let mut opts = fs::OpenOptions::new();
        opts.write(true);
        if overwrite {
            opts.create(true).truncate(true);
        } else {
            opts.create_new(true);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        return match opts
            .open(&dest)
            .and_then(|mut f| f.write_all(content.as_bytes()))
        {
            Ok(()) => ProfileWriteResult {
                ok: true,
                message: format!("Saved {}", dest.display()),
                profile: parse_vpn_conf_content(&content, &dest.to_string_lossy()),
            },
            Err(err) => ProfileWriteResult {
                ok: false,
                message: err.to_string(),
                profile: None,
            },
        };
    }
    let tmp = std::env::temp_dir().join(format!(
        "tunnel-yard-{id}-{}.conf",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    if let Err(err) = fs::write(&tmp, &content) {
        return ProfileWriteResult {
            ok: false,
            message: err.to_string(),
            profile: None,
        };
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    let dest_str = dest.to_string_lossy().into_owned();
    let tmp_str = tmp.to_string_lossy().into_owned();
    let (code, output) = run_pkexec(&["install", "-m", "0644", "-D", &tmp_str, &dest_str], None);
    let _ = fs::remove_file(&tmp);
    if code != Some(0) {
        let message = if code == Some(126) || code == Some(127) {
            "Authentication cancelled.".into()
        } else if output.is_empty() {
            format!("Failed to write {dest_str}")
        } else {
            output
        };
        return ProfileWriteResult {
            ok: false,
            message,
            profile: None,
        };
    }
    match parse_vpn_conf_content(&content, &dest_str) {
        Some(profile) => ProfileWriteResult {
            ok: true,
            message: format!("Saved {dest_str}"),
            profile: Some(profile),
        },
        None => ProfileWriteResult {
            ok: false,
            message: "Saved, but failed to re-parse profile.".into(),
            profile: None,
        },
    }
}

pub fn delete_profile_file(id: &str) -> ProfileWriteResult {
    let safe = if is_valid_profile_id(id) {
        id.to_string()
    } else {
        slugify_profile_id(id)
    };
    if !is_valid_profile_id(&safe) {
        return ProfileWriteResult {
            ok: false,
            message: "Invalid profile id.".into(),
            profile: None,
        };
    }
    let dest = match conf_path_for_id(&safe) {
        Ok(p) => p,
        Err(message) => {
            return ProfileWriteResult {
                ok: false,
                message,
                profile: None,
            }
        }
    };
    if !dest.exists() {
        return ProfileWriteResult {
            ok: false,
            message: "Profile file not found.".into(),
            profile: None,
        };
    }
    if current_platform() != "linux" {
        return match fs::remove_file(&dest) {
            Ok(()) => ProfileWriteResult {
                ok: true,
                message: format!("Deleted {}", dest.display()),
                profile: None,
            },
            Err(err) => ProfileWriteResult {
                ok: false,
                message: err.to_string(),
                profile: None,
            },
        };
    }
    let dest_str = dest.to_string_lossy().into_owned();
    let (code, output) = run_pkexec(&["rm", "-f", &dest_str], None);
    if code != Some(0) {
        let message = if code == Some(126) || code == Some(127) {
            "Authentication cancelled.".into()
        } else if output.is_empty() {
            format!("Failed to delete {dest_str}")
        } else {
            output
        };
        return ProfileWriteResult {
            ok: false,
            message,
            profile: None,
        };
    }
    ProfileWriteResult {
        ok: true,
        message: format!("Deleted {dest_str}"),
        profile: None,
    }
}

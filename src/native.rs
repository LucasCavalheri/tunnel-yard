//! Native (macOS/Windows) elevated session, quoting, and supervisor status policy.

use crate::openconnect::{
    assert_macos_profile_safe, build_open_connect_plan, conf_entries, resolve_server_pin,
};
use crate::platform::{current_platform, find_vpn_binary, helper_path};
use crate::vpn::VpnStatus;
use serde::Serialize;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq)]
pub struct NativeStatus {
    pub phase: String,
    pub message: String,
}

pub fn prevents_reconnect(line: &str, pinned: bool) -> bool {
    if pinned
        && regex_is(
            line,
            r"(?i)^Server certificate verify failed: signer not found$",
        )
    {
        return false;
    }
    regex_is(
        line,
        r"(?i)invalid credentials|authentication failed|could not authenticate|user input required|cookie (?:was rejected|is no longer valid)|reconnect-after-drop is not allowed|certificate.*(failed|mismatch)|certificate does not match",
    )
}

fn regex_is(line: &str, pattern: &str) -> bool {
    // Small hand-rolled subset so we don't pull the `regex` crate for a few checks.
    let lower = line.to_lowercase();
    if pattern.contains("signer not found") {
        return lower.trim() == "server certificate verify failed: signer not found";
    }
    let needles = [
        "invalid credentials",
        "authentication failed",
        "could not authenticate",
        "user input required",
        "cookie was rejected",
        "cookie is no longer valid",
        "reconnect-after-drop is not allowed",
        "certificate does not match",
    ];
    if needles.iter().any(|n| lower.contains(n)) {
        return true;
    }
    if lower.contains("certificate") && (lower.contains("failed") || lower.contains("mismatch")) {
        return true;
    }
    false
}

pub fn validated_native_status(raw: &str, now: i64) -> Result<NativeStatus, String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| "Invalid supervisor status.".to_string())?;
    let phase = value
        .get("phase")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Invalid supervisor status.".to_string())?;
    if !matches!(phase, "connected" | "disconnected" | "connecting") {
        return Err("Invalid supervisor status.".into());
    }
    let time = value
        .get("time")
        .and_then(|v| v.as_f64())
        .filter(|n| n.is_finite())
        .ok_or_else(|| "Invalid supervisor status.".to_string())?;
    if phase == "connected" && (now as f64 - time > 15_000.0 || time > now as f64 + 5_000.0) {
        return Ok(NativeStatus {
            phase: "disconnected".into(),
            message: "VPN supervisor health checks stopped responding.".into(),
        });
    }
    let message = value
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(NativeStatus {
        phase: phase.into(),
        message,
    })
}

pub fn ps_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

pub fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

pub fn encoded_powershell(script: &str) -> Vec<String> {
    let prelude = "$ProgressPreference='SilentlyContinue'; [Console]::OutputEncoding=New-Object Text.UTF8Encoding($false); ";
    let utf16: Vec<u16> = prelude
        .encode_utf16()
        .chain(script.encode_utf16())
        .collect();
    let mut bytes = Vec::with_capacity(utf16.len() * 2);
    for u in utf16 {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
    vec![
        "-NoLogo".into(),
        "-NoProfile".into(),
        "-NonInteractive".into(),
        "-ExecutionPolicy".into(),
        "Bypass".into(),
        "-EncodedCommand".into(),
        b64,
    ]
}

pub fn powershell_path() -> PathBuf {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    PathBuf::from(root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe")
}

pub fn secure_directory(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        let whoami = Command::new("whoami.exe")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&whoami.stdout);
        let sid = stdout
            .split(',')
            .find_map(|part| {
                let p = part.trim().trim_matches('"');
                if p.starts_with("S-1-5-") {
                    Some(p.to_string())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                "Could not identify the current Windows user for profile permissions.".to_string()
            })?;
        let dir_s = dir.to_string_lossy().into_owned();
        let status = Command::new("icacls.exe")
            .args([
                &dir_s,
                "/inheritance:r",
                "/grant:r",
                &format!("*{sid}:(OI)(CI)F"),
                "*S-1-5-18:(OI)(CI)F",
                "*S-1-5-32-544:(OI)(CI)F",
            ])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Could not set Windows profile directory ACLs.".into());
        }
    }
    let _ = dir;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobFile {
    bin: String,
    args: Vec<String>,
    password: String,
    host: String,
    port: u16,
    trusted_certs: Vec<String>,
    set_dns: bool,
    set_routes: bool,
    persistent: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    health_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    health_port: Option<u16>,
    no_dtls: bool,
    legacy_tunnel: bool,
}

pub enum NativeEvent {
    Line(String),
    Status { phase: VpnStatus, message: String },
    Close(i32),
}

pub struct NativeVpnSession {
    pub persistent: u32,
    pub can_reconnect: Arc<AtomicBool>,
}

impl NativeVpnSession {
    pub fn start(
        config_path: &Path,
        events: Sender<NativeEvent>,
        stop: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let bin = find_vpn_binary(None).ok_or_else(|| {
            "VPN client not installed. Open setup and install the required client.".to_string()
        })?;
        let raw = fs::read_to_string(config_path).map_err(|e| e.to_string())?;
        let root = std::env::temp_dir().join("my-vpns-sessions");
        secure_directory(&root)?;
        let dir = unique_session_dir(&root)?;
        secure_directory(&dir)?;
        if stop.load(Ordering::SeqCst) {
            let _ = events.send(NativeEvent::Close(0));
            return Ok(Self {
                persistent: 0,
                can_reconnect: Arc::new(AtomicBool::new(false)),
            });
        }
        fs::write(dir.join("heartbeat"), b"").map_err(|e| e.to_string())?;
        for name in ["stdout.log", "stderr.log"] {
            fs::write(dir.join(name), b"").map_err(|e| e.to_string())?;
        }
        fs::write(dir.join("exit-code"), b"1").map_err(|e| e.to_string())?;

        let platform = current_platform();
        let persistent;
        let pinned = Arc::new(AtomicBool::new(false));
        let can_reconnect = Arc::new(AtomicBool::new(true));
        let (command, args) = if platform == "windows" {
            let mut plan = build_open_connect_plan(&raw)?;
            match resolve_server_pin(&plan) {
                Ok(Some(pin)) => {
                    plan.args.insert(0, format!("--servercert={pin}"));
                    pinned.store(true, Ordering::SeqCst);
                    let _ = events.send(NativeEvent::Line(
                        "TLS: trusted-cert SHA256 verified; OpenConnect will enforce the matching public-key pin. An unknown-CA warning does not disable this verification.".into(),
                    ));
                }
                Ok(None) => {}
                Err(err) => {
                    let _ = events.send(NativeEvent::Line(format!("ERROR: {err}")));
                    let _ = events.send(NativeEvent::Close(1));
                    return Ok(Self {
                        persistent: plan.persistent,
                        can_reconnect: Arc::new(AtomicBool::new(false)),
                    });
                }
            }
            if stop.load(Ordering::SeqCst) {
                let _ = events.send(NativeEvent::Close(0));
                return Ok(Self {
                    persistent: plan.persistent,
                    can_reconnect: Arc::new(AtomicBool::new(false)),
                });
            }
            let iface = format!(
                "--interface=MyVPNs-{}",
                dir.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("sess")
                    .chars()
                    .rev()
                    .take(6)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
            );
            plan.args.insert(
                0,
                format!("--script={}", helper_path("vpnc-script-win.js")?.display()),
            );
            plan.args.insert(0, iface);
            persistent = plan.persistent;
            let job = JobFile {
                bin,
                args: plan.args.clone(),
                password: plan.password,
                host: plan.host,
                port: plan.port,
                trusted_certs: plan.trusted_certs,
                set_dns: plan.set_dns,
                set_routes: plan.set_routes,
                persistent: plan.persistent,
                health_host: plan.health_host,
                health_port: plan.health_port,
                no_dtls: plan.no_dtls,
                legacy_tunnel: plan.legacy_tunnel,
            };
            fs::write(
                dir.join("job.json"),
                serde_json::to_vec(&job).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            let helper = helper_path("windows-vpn.ps1")?;
            let inner = encoded_powershell(&format!(
                "& {} -SessionDir {}",
                ps_quote(&helper.to_string_lossy()),
                ps_quote(&dir.to_string_lossy())
            ));
            let ps = powershell_path();
            let outer = encoded_powershell(&format!(
                "$ErrorActionPreference = 'Stop'; try {{ $p = Start-Process -FilePath {} -ArgumentList {} -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 126 }}",
                ps_quote(&ps.to_string_lossy()),
                ps_quote(&inner.join(" "))
            ));
            (ps.to_string_lossy().into_owned(), outer)
        } else {
            assert_macos_profile_safe(&raw)?;
            persistent = conf_entries(&raw)?
                .into_iter()
                .find(|(k, _)| k == "persistent")
                .and_then(|(_, v)| v.parse::<i64>().ok())
                .map(|n| n.max(0) as u32)
                .unwrap_or(0);
            fs::write(dir.join("profile.conf"), &raw).map_err(|e| e.to_string())?;
            let helper = helper_path("macos-vpn.sh")?;
            let shell_command = [
                &"/bin/bash".to_string(),
                &helper.to_string_lossy().into_owned(),
                &dir.to_string_lossy().into_owned(),
                &bin,
            ]
            .into_iter()
            .map(|s| sh_quote(s))
            .collect::<Vec<_>>()
            .join(" ");
            (
                "/usr/bin/osascript".into(),
                vec![
                    "-e".into(),
                    format!(
                        "do shell script {} with administrator privileges",
                        serde_json::to_string(&shell_command).unwrap()
                    ),
                ],
            )
        };

        let dir_clone = dir.clone();
        let events_clone = events.clone();
        let stop_clone = stop.clone();
        let can_re = can_reconnect.clone();
        let pinned_c = pinned.clone();
        thread::spawn(move || {
            run_native_supervisor(
                command,
                args,
                dir_clone,
                events_clone,
                stop_clone,
                can_re,
                pinned_c,
            );
        });

        Ok(Self {
            persistent,
            can_reconnect,
        })
    }
}

fn unique_session_dir(root: &Path) -> Result<PathBuf, String> {
    for i in 0..1000 {
        let name = format!(
            "session-{}-{i}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );
        let dir = root.join(name);
        if fs::create_dir(&dir).is_ok() {
            return Ok(dir);
        }
    }
    Err("Could not create VPN session directory.".into())
}

fn run_native_supervisor(
    command: String,
    args: Vec<String>,
    dir: PathBuf,
    events: Sender<NativeEvent>,
    stop: Arc<AtomicBool>,
    can_reconnect: Arc<AtomicBool>,
    pinned: Arc<AtomicBool>,
) {
    let mut child = match Command::new(&command)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => {
            let _ = events.send(NativeEvent::Line(format!("ERROR: {err}")));
            let _ = events.send(NativeEvent::Close(1));
            return;
        }
    };
    let mut offsets = std::collections::HashMap::from([
        ("stdout.log".to_string(), 0u64),
        ("stderr.log".to_string(), 0u64),
    ]);
    let mut pending = std::collections::HashMap::from([
        ("stdout.log".to_string(), String::new()),
        ("stderr.log".to_string(), String::new()),
    ]);
    let mut phase = "connecting".to_string();
    let mut last_healthy: Option<Instant> = None;
    let error_output = Arc::new(Mutex::new(String::new()));
    if let Some(mut stderr) = child.stderr.take() {
        let sink = error_output.clone();
        thread::spawn(move || {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf);
            if let Ok(mut g) = sink.lock() {
                *g = buf;
            }
        });
    }
    loop {
        if stop.load(Ordering::SeqCst) {
            let _ = fs::write(dir.join("stop"), b"");
        }
        let _ = fs::File::open(dir.join("heartbeat")).and_then(|f| f.sync_all());
        let _ = filetime_touch(&dir.join("heartbeat"));
        read_logs(
            &dir,
            &mut offsets,
            &mut pending,
            false,
            &events,
            &can_reconnect,
            pinned.load(Ordering::SeqCst),
        );
        if current_platform() == "windows" {
            match fs::read_to_string(dir.join("status.json")) {
                Ok(raw) => {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    if let Ok(status) = validated_native_status(&raw, now) {
                        if status.phase == "connected" {
                            last_healthy = Some(Instant::now());
                        }
                        if status.phase != phase {
                            phase = status.phase.clone();
                            let vpn_phase = match status.phase.as_str() {
                                "connected" => VpnStatus::Connected,
                                "connecting" => VpnStatus::Connecting,
                                _ => VpnStatus::Disconnected,
                            };
                            let _ = events.send(NativeEvent::Status {
                                phase: vpn_phase,
                                message: status.message,
                            });
                            if status.phase == "disconnected" && !stop.load(Ordering::SeqCst) {
                                stop.store(true, Ordering::SeqCst);
                                let _ = fs::write(dir.join("stop"), b"");
                            }
                        }
                    }
                }
                Err(_) => {
                    if last_healthy
                        .map(|t| t.elapsed() > Duration::from_secs(15))
                        .unwrap_or(false)
                    {
                        if phase != "disconnected" {
                            phase = "disconnected".into();
                            let _ = events.send(NativeEvent::Status {
                                phase: VpnStatus::Disconnected,
                                message: "VPN supervisor status is unavailable.".into(),
                            });
                        }
                    }
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let error_output = error_output.lock().unwrap().clone();
                if !error_output.trim().is_empty() {
                    let _ = events.send(NativeEvent::Line(error_output.trim().to_string()));
                }
                let mut result = status.code().unwrap_or(1);
                if let Ok(code_raw) = fs::read_to_string(dir.join("exit-code")) {
                    if let Ok(n) = code_raw.trim().parse::<i32>() {
                        result = n;
                    }
                }
                if status.code() == Some(126) || error_output.contains("(-128)") {
                    result = 126;
                }
                read_logs(
                    &dir,
                    &mut offsets,
                    &mut pending,
                    true,
                    &events,
                    &can_reconnect,
                    pinned.load(Ordering::SeqCst),
                );
                cleanup_session(&dir);
                let _ = events.send(NativeEvent::Close(result));
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(250)),
            Err(err) => {
                let _ = events.send(NativeEvent::Line(format!("ERROR: {err}")));
                cleanup_session(&dir);
                let _ = events.send(NativeEvent::Close(1));
                break;
            }
        }
    }
}

fn filetime_touch(path: &Path) -> std::io::Result<()> {
    let f = fs::OpenOptions::new().write(true).create(true).open(path)?;
    f.set_modified(SystemTime::now())
}

fn read_logs(
    dir: &Path,
    offsets: &mut std::collections::HashMap<String, u64>,
    pending: &mut std::collections::HashMap<String, String>,
    flush: bool,
    events: &Sender<NativeEvent>,
    can_reconnect: &AtomicBool,
    pinned: bool,
) {
    for name in ["stdout.log", "stderr.log"] {
        let file = dir.join(name);
        let Ok(meta) = fs::metadata(&file) else {
            continue;
        };
        let size = meta.len();
        let offset = *offsets.get(name).unwrap_or(&0);
        if size > offset {
            if let Ok(mut f) = fs::File::open(&file) {
                let _ = f.seek(SeekFrom::Start(offset));
                let mut buf = vec![0u8; (size - offset).min(1024 * 1024) as usize];
                if let Ok(n) = f.read(&mut buf) {
                    offsets.insert(name.into(), offset + n as u64);
                    let chunk = String::from_utf8_lossy(&buf[..n]);
                    let mut data = pending.get(name).cloned().unwrap_or_default();
                    data.push_str(&chunk);
                    let mut parts: Vec<&str> = data.split('\n').collect();
                    let rest = parts.pop().unwrap_or("").trim_end_matches('\r').to_string();
                    for line in parts {
                        let line = line.trim_end_matches('\r');
                        if prevents_reconnect(line, pinned) {
                            can_reconnect.store(false, Ordering::SeqCst);
                        }
                        if line.contains("reconnect-after-drop is not allowed") {
                            let _ = events.send(NativeEvent::Line(
                                "VPN server forbids reconnect-after-drop; automatic retry is disabled for this session. A manual connection will authenticate again.".into(),
                            ));
                        }
                        if !line.trim().is_empty() {
                            let _ = events.send(NativeEvent::Line(line.trim().to_string()));
                        }
                    }
                    pending.insert(name.into(), rest);
                }
            }
        }
        if flush {
            if let Some(rest) = pending.get(name) {
                if !rest.is_empty() {
                    let _ = events.send(NativeEvent::Line(rest.clone()));
                }
            }
            pending.remove(name);
        }
    }
}

fn cleanup_session(dir: &Path) {
    for name in [
        "job.json",
        "profile.conf",
        "heartbeat",
        "stop",
        "exit-code",
        "stdout.log",
        "stderr.log",
        "status.json",
        "split-dns.json",
    ] {
        let _ = fs::remove_file(dir.join(name));
    }
    let _ = fs::remove_dir(dir);
}

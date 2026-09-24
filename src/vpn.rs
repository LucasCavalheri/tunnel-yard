//! Multi-session VPN manager, log markers, and reconnect policy.

use crate::conf::parse_vpn_conf_content;
use crate::i18n::tr;
use crate::platform::linux_helpers;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use crate::conf::VpnProfile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpnStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl VpnStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Error => "error",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        s.parse().ok()
    }
}

impl FromStr for VpnStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "disconnected" => Ok(Self::Disconnected),
            "connecting" => Ok(Self::Connecting),
            "connected" => Ok(Self::Connected),
            "error" => Ok(Self::Error),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VpnSession {
    pub profile_id: String,
    pub status: VpnStatus,
    pub message: String,
    pub connected_at: Option<u128>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VpnState {
    pub sessions: HashMap<String, VpnSession>,
    pub auto_reconnect: bool,
}

#[derive(Clone, Debug)]
pub struct VpnSummary {
    pub connected_count: usize,
    pub connecting_count: usize,
    pub error_count: usize,
    pub overall: VpnStatus,
    pub message: String,
}

#[derive(Clone, Debug)]
pub enum VpnEvent {
    State(VpnState),
    Log(String),
    Profiles(Vec<VpnProfile>),
    NeedReconnect(String),
}

const CONNECTED_MARKERS: &[&str] = &[
    "tunnel is up and running",
    "tunnel interface is up",
    "tunnelyard_tunnel_up",
];

const ERROR_MARKERS: &[&str] = &[
    "could not authenticate",
    "authentication failed",
    "invalid credentials",
    "user input required",
    "permission denied",
    "failed to",
    "error:",
    "unable to",
    "not authorized",
];

pub fn interpret_vpn_log_line(line: &str) -> Option<&'static str> {
    let lower = line.to_lowercase();
    if lower.starts_with("tunnelyard_tunnel_down") {
        return Some("disconnected");
    }
    if CONNECTED_MARKERS.iter().any(|m| lower.contains(m)) {
        return Some("connected");
    }
    if ERROR_MARKERS.iter().any(|m| lower.contains(m)) {
        return Some("error");
    }
    None
}

pub fn list_vpn_profiles(config_dir: &Path) -> Vec<VpnProfile> {
    let Ok(entries) = fs::read_dir(config_dir) else {
        return vec![];
    };
    let mut profiles: Vec<VpnProfile> = entries
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("conf"))
                .unwrap_or(false)
        })
        .filter_map(|e| {
            let path = e.path();
            let raw = fs::read_to_string(&path).ok()?;
            parse_vpn_conf_content(&raw, &path.to_string_lossy())
        })
        .collect();
    profiles.sort_by_key(|profile| profile.name.to_lowercase());
    profiles
}

/// The line under "Your tunnels": how many profiles exist and how many are really up.
/// A tunnel that is still connecting does not count as connected.
pub fn workspace_summary(total_profiles: usize, state: &VpnState) -> String {
    tr(
        "ops.workspaceSummary",
        &[
            ("total", total_profiles.to_string()),
            (
                "active",
                summarize_vpn_state(state).connected_count.to_string(),
            ),
        ],
    )
}

pub fn summarize_vpn_state(state: &VpnState) -> VpnSummary {
    let sessions: Vec<_> = state.sessions.values().collect();
    let connected_count = sessions
        .iter()
        .filter(|s| s.status == VpnStatus::Connected)
        .count();
    let connecting_count = sessions
        .iter()
        .filter(|s| s.status == VpnStatus::Connecting)
        .count();
    let error_count = sessions
        .iter()
        .filter(|s| s.status == VpnStatus::Error)
        .count();
    let overall = if connected_count > 0 {
        VpnStatus::Connected
    } else if connecting_count > 0 {
        VpnStatus::Connecting
    } else if error_count > 0 {
        VpnStatus::Error
    } else {
        VpnStatus::Disconnected
    };
    let message = if connected_count + connecting_count == 0 {
        tr("ops.noneConnected", &[])
    } else {
        tr(
            "ops.deskSummary",
            &[
                ("up", connected_count.to_string()),
                ("connecting", connecting_count.to_string()),
            ],
        )
    };
    VpnSummary {
        connected_count,
        connecting_count,
        error_count,
        overall,
        message,
    }
}

pub fn reconnect_delay_ms(
    auto_reconnect: bool,
    persistent: u32,
    intentional_stop: bool,
) -> Option<u64> {
    if intentional_stop {
        return None;
    }
    if !auto_reconnect && persistent == 0 {
        return None;
    }
    Some(if persistent > 0 {
        u64::from(persistent) * 1000
    } else {
        4000
    })
}

pub fn prevents_reconnect(line: &str, pinned: bool) -> bool {
    let lower = line.to_lowercase();
    if pinned && lower.trim() == "server certificate verify failed: signer not found" {
        return false;
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
    lower.contains("certificate") && (lower.contains("failed") || lower.contains("mismatch"))
}

/// Close → reconnect: skip user disconnect, elevation cancel (126), and
/// `can_reconnect == false`.
pub fn should_native_reconnect(
    intentional_stop: bool,
    exit_code: i32,
    can_reconnect: bool,
    auto_reconnect: bool,
    persistent: u32,
) -> Option<u64> {
    if intentional_stop || exit_code == 126 || !can_reconnect {
        return None;
    }
    reconnect_delay_ms(auto_reconnect, persistent, false)
}

/// Linux helper/openfortivpn process exit → reconnect.
/// 126 = pkexec cancelled, 127 = helper/binary missing.
pub fn linux_exit_reconnect(
    intentional_stop: bool,
    exit_code: Option<i32>,
    can_reconnect: bool,
    auto_reconnect: bool,
    persistent: u32,
) -> Option<u64> {
    if matches!(exit_code, Some(126) | Some(127)) {
        return None;
    }
    should_native_reconnect(
        intentional_stop,
        exit_code.unwrap_or(1),
        can_reconnect,
        auto_reconnect,
        persistent,
    )
}

/// Whether `NeedReconnect` may start a new tunnel for this live session.
/// A missing session is treated as a user disconnect (already dropped).
pub fn reconnect_gate(status: Option<VpnStatus>, intentional_stop: bool) -> bool {
    if intentional_stop {
        return false;
    }
    matches!(
        status,
        Some(VpnStatus::Disconnected) | Some(VpnStatus::Error)
    )
}

/// Outcome of a native supervisor `close` event.
///
/// `helper_stopping` is `NativeVpnSession.stopping` (stop file / status.json
/// disconnected). It is **not** `live.intentionalStop`. A helper-initiated
/// teardown must keep the session and may emit NeedReconnect.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeCloseDecision {
    /// User asked to disconnect: drop the live session, do not reconnect.
    DropSession,
    /// Supervisor ended the tunnel. Keep the session; optionally reconnect.
    KeepSession { reconnect_after_ms: Option<u64> },
}

impl NativeCloseDecision {
    /// True when the Close handler must emit `VpnEvent::NeedReconnect` after delay.
    pub fn emits_need_reconnect(&self) -> bool {
        matches!(
            self,
            Self::KeepSession {
                reconnect_after_ms: Some(_)
            }
        )
    }
}

/// Close-path decision used by `VpnManager` on `NativeEvent::Close`.
/// `helper_stopping` is accepted so callers pass the real supervisor flag;
/// it must not be treated as a user disconnect.
pub fn native_close_decision(
    user_intentional: bool,
    helper_stopping: bool,
    code: i32,
    can_reconnect: bool,
    auto_reconnect: bool,
    persistent: u32,
) -> NativeCloseDecision {
    // helper_stopping (status.json / stop file) must not be read as user_intentional.
    let _ = helper_stopping;
    if user_intentional {
        NativeCloseDecision::DropSession
    } else {
        NativeCloseDecision::KeepSession {
            reconnect_after_ms: should_native_reconnect(
                false,
                code,
                can_reconnect,
                auto_reconnect,
                persistent,
            ),
        }
    }
}

struct LiveSession {
    profile: VpnProfile,
    /// User clicked disconnect / kill. Distinct from helper `stopping`.
    intentional_stop: Arc<AtomicBool>,
    /// Supervisor should write the session `stop` file and exit.
    helper_stop: Arc<AtomicBool>,
    persistent: u32,
    can_reconnect: Arc<AtomicBool>,
    status: VpnStatus,
    message: String,
    connected_at: Option<u128>,
    child_pid: Option<u32>,
}

struct Inner {
    live: HashMap<String, LiveSession>,
    profiles: Vec<VpnProfile>,
    auto_reconnect: bool,
}

pub struct VpnManager {
    inner: Arc<Mutex<Inner>>,
    events: Sender<VpnEvent>,
}

impl VpnManager {
    pub fn new(events: Sender<VpnEvent>) -> Self {
        let mgr = Self {
            inner: Arc::new(Mutex::new(Inner {
                live: HashMap::new(),
                profiles: vec![],
                auto_reconnect: false,
            })),
            events,
        };
        mgr.refresh_profiles();
        mgr
    }

    pub fn subscribe() -> (Self, Receiver<VpnEvent>) {
        let (tx, rx) = mpsc::channel();
        (Self::new(tx), rx)
    }

    fn emit_log(&self, line: &str) {
        let stamp = chrono_stamp();
        let _ = self.events.send(VpnEvent::Log(format!("[{stamp}] {line}")));
    }

    fn emit_state(&self) {
        let _ = self.events.send(VpnEvent::State(self.get_state()));
    }

    pub fn get_state(&self) -> VpnState {
        let inner = self.inner.lock().unwrap();
        let mut sessions = HashMap::new();
        for (id, live) in &inner.live {
            sessions.insert(
                id.clone(),
                VpnSession {
                    profile_id: id.clone(),
                    status: live.status,
                    message: live.message.clone(),
                    connected_at: live.connected_at,
                },
            );
        }
        VpnState {
            sessions,
            auto_reconnect: inner.auto_reconnect,
        }
    }

    pub fn get_profiles(&self) -> Vec<VpnProfile> {
        self.inner.lock().unwrap().profiles.clone()
    }

    pub fn refresh_profiles(&self) -> Vec<VpnProfile> {
        let dir = crate::platform::config_directory_current();
        let profiles = list_vpn_profiles(Path::new(&dir));
        self.inner.lock().unwrap().profiles = profiles.clone();
        let _ = self.events.send(VpnEvent::Profiles(profiles.clone()));
        profiles
    }

    pub fn set_auto_reconnect(&self, enabled: bool) {
        self.inner.lock().unwrap().auto_reconnect = enabled;
        self.emit_state();
    }

    /// Start a new tunnel after an unexpected drop. Does not run the user
    /// disconnect path (no pkexec/stop helper), which would look like a
    /// manual hang-up and could prompt for elevation.
    pub fn reconnect(&self, profile_id: &str) {
        let allowed = {
            let inner = self.inner.lock().unwrap();
            match inner.live.get(profile_id) {
                Some(live) => reconnect_gate(
                    Some(live.status),
                    live.intentional_stop.load(Ordering::SeqCst),
                ),
                None => false,
            }
        };
        if !allowed {
            self.emit_log(&tr(
                "log.reconnectSkipped",
                &[("id", profile_id.to_string())],
            ));
            return;
        }
        self.inner.lock().unwrap().live.remove(profile_id);
        self.connect(profile_id);
    }

    pub fn connect(&self, profile_id: &str) {
        let profile = {
            let inner = self.inner.lock().unwrap();
            inner.profiles.iter().find(|p| p.id == profile_id).cloned()
        };
        let Some(profile) = profile else {
            self.emit_log(&tr("log.profileMissing", &[("id", profile_id.to_string())]));
            return;
        };
        {
            let inner = self.inner.lock().unwrap();
            if let Some(existing) = inner.live.get(profile_id) {
                if existing.status == VpnStatus::Connected
                    || existing.status == VpnStatus::Connecting
                {
                    self.emit_log(&tr("log.alreadyUp", &[("name", profile.name.clone())]));
                    return;
                }
            }
        }
        if self.inner.lock().unwrap().live.contains_key(profile_id) {
            self.disconnect(Some(profile_id));
        }
        let helper_stop = Arc::new(AtomicBool::new(false));
        let intentional_stop = Arc::new(AtomicBool::new(false));
        {
            let mut inner = self.inner.lock().unwrap();
            inner.live.insert(
                profile_id.to_string(),
                LiveSession {
                    profile: profile.clone(),
                    intentional_stop: intentional_stop.clone(),
                    helper_stop: helper_stop.clone(),
                    persistent: profile.persistent,
                    can_reconnect: Arc::new(AtomicBool::new(true)),
                    status: VpnStatus::Connecting,
                    message: tr("vpn.connectingTo", &[("name", profile.name.clone())]),
                    connected_at: None,
                    child_pid: None,
                },
            );
        }
        self.emit_state();
        self.emit_log(&tr(
            "log.connecting",
            &[
                ("name", profile.name.clone()),
                ("host", profile.host.clone()),
                ("port", profile.port.to_string()),
            ],
        ));
        self.emit_log(&tr("log.config", &[("path", profile.path.clone())]));

        self.connect_linux(profile, intentional_stop);
    }

    fn connect_linux(&self, profile: VpnProfile, stop: Arc<AtomicBool>) {
        let helpers = linux_helpers();
        let args: Vec<String> = if let Some((run, _)) = &helpers {
            self.emit_log(&tr("log.policykit", &[("id", profile.id.clone())]));
            vec![run.to_string_lossy().into_owned(), profile.path.clone()]
        } else {
            vec!["openfortivpn".into(), "-c".into(), profile.path.clone()]
        };
        let id = profile.id.clone();
        let mgr_inner = self.inner.clone();
        let events = self.events.clone();
        thread::spawn(move || {
            let mut child = match Command::new("pkexec")
                .args(&args)
                .env("LANG", "C.UTF-8")
                .env("LC_ALL", "C.UTF-8")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(err) => {
                    if let Ok(mut inner) = mgr_inner.lock() {
                        if let Some(live) = inner.live.get_mut(&id) {
                            live.status = VpnStatus::Error;
                            live.message = err.to_string();
                            live.connected_at = None;
                        }
                    }
                    let _ = events.send(VpnEvent::Log(format!(
                        "[{}] {}",
                        chrono_stamp(),
                        tr(
                            "log.startFailed",
                            &[("id", id.clone()), ("error", err.to_string())]
                        )
                    )));
                    emit_state_from(&mgr_inner, &events);
                    return;
                }
            };
            if let Ok(mut inner) = mgr_inner.lock() {
                if let Some(live) = inner.live.get_mut(&id) {
                    live.child_pid = Some(child.id());
                }
            }
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            spawn_log_pump(stdout, mgr_inner.clone(), events.clone(), id.clone());
            spawn_log_pump_err(stderr, mgr_inner.clone(), events.clone(), id.clone());
            let status = child.wait();
            let was_intentional = stop.load(Ordering::SeqCst);
            let code = status.as_ref().ok().and_then(|s| s.code());
            let code_text = code
                .map(|c| c.to_string())
                .unwrap_or_else(|| tr("vpn.unknownCode", &[]));
            let _ = events.send(VpnEvent::Log(format!(
                "[{}] {}",
                chrono_stamp(),
                tr(
                    "log.exited",
                    &[("id", id.clone()), ("code", code_text.clone())]
                )
            )));
            if was_intentional {
                if let Ok(mut inner) = mgr_inner.lock() {
                    inner.live.remove(&id);
                }
                emit_state_from(&mgr_inner, &events);
                return;
            }
            let cancelled = code == Some(126) || code == Some(127);
            let persistent;
            let auto;
            {
                let mut inner = mgr_inner.lock().unwrap();
                auto = inner.auto_reconnect;
                if let Some(live) = inner.live.get_mut(&id) {
                    live.status = VpnStatus::Error;
                    live.message = if cancelled {
                        tr("vpn.authCancelled", &[])
                    } else {
                        tr("vpn.closedCode", &[("code", code_text.clone())])
                    };
                    live.connected_at = None;
                    live.child_pid = None;
                    persistent = live.persistent;
                } else {
                    persistent = 0;
                }
            }
            emit_state_from(&mgr_inner, &events);
            let can_reconnect = mgr_inner
                .lock()
                .ok()
                .and_then(|g| {
                    g.live
                        .get(&id)
                        .map(|live| live.can_reconnect.load(Ordering::SeqCst))
                })
                .unwrap_or(false);
            if let Some(delay) =
                linux_exit_reconnect(was_intentional, code, can_reconnect, auto, persistent)
            {
                let _ = events.send(VpnEvent::Log(format!(
                    "[{}] {}",
                    chrono_stamp(),
                    tr(
                        "log.reconnectIn",
                        &[("id", id.clone()), ("seconds", (delay / 1000).to_string())]
                    )
                )));
                thread::sleep(Duration::from_millis(delay));
                if !stop.load(Ordering::SeqCst) {
                    let _ = events.send(VpnEvent::NeedReconnect(id.clone()));
                }
            }
        });
    }

    pub fn disconnect(&self, profile_id: Option<&str>) {
        let ids: Vec<String> = if let Some(id) = profile_id {
            vec![id.to_string()]
        } else {
            self.inner.lock().unwrap().live.keys().cloned().collect()
        };
        for id in ids {
            self.disconnect_one(&id);
        }
    }

    fn disconnect_one(&self, profile_id: &str) {
        let path;
        let pid;
        {
            let mut inner = self.inner.lock().unwrap();
            let Some(live) = inner.live.get_mut(profile_id) else {
                return;
            };
            live.intentional_stop.store(true, Ordering::SeqCst);
            live.helper_stop.store(true, Ordering::SeqCst);
            live.status = VpnStatus::Connecting;
            live.message = tr("vpn.disconnecting", &[]);
            path = live.profile.path.clone();
            pid = live.child_pid;
        }
        self.emit_log(&tr("log.disconnecting", &[("id", profile_id.to_string())]));
        self.emit_state();
        stop_vpn(&path);
        if let Some(pid) = pid {
            let _ = Command::new("kill")
                .args(["-INT", &pid.to_string()])
                .status();
            thread::sleep(Duration::from_millis(2500));
            let _ = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status();
        }
        self.inner.lock().unwrap().live.remove(profile_id);
        self.emit_log(&tr("log.disconnected", &[("id", profile_id.to_string())]));
        self.emit_state();
    }
}

fn stop_vpn(config_path: &str) {
    let args: Vec<String> = if let Some((_, stop)) = linux_helpers() {
        vec![stop.to_string_lossy().into_owned(), config_path.into()]
    } else {
        vec![
            "pkill".into(),
            "-INT".into(),
            "-f".into(),
            format!("openfortivpn -c {config_path}"),
        ]
    };
    let mut child = Command::new("pkexec")
        .args(&args)
        .stdin(Stdio::null())
        .spawn();
    if let Ok(ref mut c) = child {
        let _ = c.wait();
    }
}

fn interpret_into(inner: &Arc<Mutex<Inner>>, profile_id: &str, line: &str) {
    let mut g = inner.lock().unwrap();
    let Some(live) = g.live.get_mut(profile_id) else {
        return;
    };
    if live.intentional_stop.load(Ordering::SeqCst) {
        return;
    }
    match interpret_vpn_log_line(line) {
        Some("disconnected") => {
            live.status = VpnStatus::Disconnected;
            live.message = line.chars().take(120).collect();
            live.connected_at = None;
        }
        Some("connected") => {
            live.status = VpnStatus::Connected;
            live.message = tr("vpn.tunnelUp", &[]);
            live.connected_at = Some(now_ms());
        }
        Some("error")
            if live.status == VpnStatus::Connecting || live.status == VpnStatus::Error =>
        {
            live.status = VpnStatus::Error;
            live.message = line.chars().take(120).collect();
        }
        _ => {}
    }
}

fn emit_state_from(inner: &Arc<Mutex<Inner>>, events: &Sender<VpnEvent>) {
    let state = {
        let g = inner.lock().unwrap();
        let mut sessions = HashMap::new();
        for (id, live) in &g.live {
            sessions.insert(
                id.clone(),
                VpnSession {
                    profile_id: id.clone(),
                    status: live.status,
                    message: live.message.clone(),
                    connected_at: live.connected_at,
                },
            );
        }
        VpnState {
            sessions,
            auto_reconnect: g.auto_reconnect,
        }
    };
    let _ = events.send(VpnEvent::State(state));
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn chrono_stamp() -> String {
    local_hms()
}

/// Local 24h clock for console lines (`HH:MM:SS`).
pub fn local_hms() -> String {
    let secs = (now_ms() / 1000) as i64 + local_utc_offset_secs();
    let day = ((secs % 86_400) + 86_400) % 86_400;
    format!("{:02}:{:02}:{:02}", day / 3600, (day % 3600) / 60, day % 60)
}

fn local_utc_offset_secs() -> i64 {
    let utc = now_ms() / 1000;
    // Derive offset from the difference between local civil time and UTC,
    // using the process TZ via `strftime` of a known conversion:
    // `chrono` is avoided; parse `date` only as last resort.
    timezone_offset_secs(utc as i64)
}

#[cfg(unix)]
fn timezone_offset_secs(_utc: i64) -> i64 {
    let mut out = std::mem::MaybeUninit::<libc::tm>::uninit();
    let t = _utc as libc::time_t;
    let tm = unsafe { libc::localtime_r(&t, out.as_mut_ptr()) };
    if tm.is_null() {
        0
    } else {
        unsafe { (*tm).tm_gmtoff }
    }
}

#[cfg(not(unix))]
fn timezone_offset_secs(_utc: i64) -> i64 {
    0
}

fn spawn_log_pump(
    stream: Option<std::process::ChildStdout>,
    inner: Arc<Mutex<Inner>>,
    events: Sender<VpnEvent>,
    id: String,
) {
    let Some(out) = stream else { return };
    thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            pump_line(&inner, &events, &id, &line);
        }
    });
}

fn spawn_log_pump_err(
    stream: Option<std::process::ChildStderr>,
    inner: Arc<Mutex<Inner>>,
    events: Sender<VpnEvent>,
    id: String,
) {
    let Some(err) = stream else { return };
    thread::spawn(move || {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            pump_line(&inner, &events, &id, &line);
        }
    });
}

fn pump_line(inner: &Arc<Mutex<Inner>>, events: &Sender<VpnEvent>, id: &str, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    let _ = events.send(VpnEvent::Log(format!(
        "[{}] [{id}] {trimmed}",
        chrono_stamp()
    )));
    {
        let mut g = inner.lock().unwrap();
        if let Some(live) = g.live.get_mut(id) {
            if prevents_reconnect(trimmed, live.profile.has_trusted_cert) {
                live.can_reconnect.store(false, Ordering::SeqCst);
            }
        }
    }
    interpret_into(inner, id, trimmed);
    emit_state_from(inner, events);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(mgr: &VpnManager, id: &str, status: VpnStatus, stop: bool) {
        let mut inner = mgr.inner.lock().unwrap();
        inner.live.insert(
            id.into(),
            LiveSession {
                profile: VpnProfile {
                    id: id.into(),
                    name: id.into(),
                    path: format!("/tmp/{id}.conf"),
                    host: "vpn.example".into(),
                    port: 443,
                    username: "u".into(),
                    set_dns: true,
                    set_routes: true,
                    has_password: false,
                    has_trusted_cert: false,
                    persistent: 0,
                },
                intentional_stop: Arc::new(AtomicBool::new(stop)),
                helper_stop: Arc::new(AtomicBool::new(false)),
                persistent: 0,
                can_reconnect: Arc::new(AtomicBool::new(true)),
                status,
                message: "seed".into(),
                connected_at: None,
                child_pid: None,
            },
        );
    }

    #[test]
    fn reconnect_clears_error_session_without_user_disconnect() {
        let (mgr, _rx) = VpnManager::subscribe();
        seed(&mgr, "work", VpnStatus::Error, false);
        assert!(mgr.get_state().sessions.contains_key("work"));
        mgr.reconnect("work");
        assert!(
            !mgr.get_state().sessions.contains_key("work"),
            "dead session must be dropped so connect() does not pkexec-stop it"
        );
    }

    #[test]
    fn reconnect_leaves_a_healthy_or_user_stopped_session_alone() {
        let (mgr, _rx) = VpnManager::subscribe();
        seed(&mgr, "up", VpnStatus::Connected, false);
        mgr.reconnect("up");
        assert_eq!(mgr.get_state().sessions["up"].status, VpnStatus::Connected);

        seed(&mgr, "hand", VpnStatus::Connecting, false);
        mgr.reconnect("hand");
        assert_eq!(
            mgr.get_state().sessions["hand"].status,
            VpnStatus::Connecting
        );

        seed(&mgr, "user", VpnStatus::Error, true);
        mgr.reconnect("user");
        assert!(mgr.get_state().sessions.contains_key("user"));
    }
}

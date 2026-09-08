//! Multi-session VPN manager, log markers, and reconnect policy.

use crate::conf::parse_vpn_conf_content;
use crate::native::{NativeEvent, NativeVpnSession};
use crate::platform::{current_platform, linux_helpers};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
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

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "disconnected" => Some(Self::Disconnected),
            "connecting" => Some(Self::Connecting),
            "connected" => Some(Self::Connected),
            "error" => Some(Self::Error),
            _ => None,
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
    profiles.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    profiles
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
        "Nenhuma VPN ativa".into()
    } else {
        format!("{connected_count} up · {connecting_count} handshake")
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

/// Native (Windows/macOS) close → reconnect:
/// skip user disconnect, UAC/pkexec cancel (126), and `can_reconnect == false`.
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

    pub fn connect(&self, profile_id: &str) {
        let profile = {
            let inner = self.inner.lock().unwrap();
            inner.profiles.iter().find(|p| p.id == profile_id).cloned()
        };
        let Some(profile) = profile else {
            self.emit_log(&format!("✗ Perfil \"{profile_id}\" não encontrado"));
            return;
        };
        {
            let inner = self.inner.lock().unwrap();
            if let Some(existing) = inner.live.get(profile_id) {
                if existing.status == VpnStatus::Connected
                    || existing.status == VpnStatus::Connecting
                {
                    self.emit_log(&format!("→ {} já está ativa", profile.name));
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
                    persistent: 0,
                    can_reconnect: Arc::new(AtomicBool::new(true)),
                    status: VpnStatus::Connecting,
                    message: format!("Autenticando {}…", profile.name),
                    connected_at: None,
                    child_pid: None,
                },
            );
        }
        self.emit_state();
        self.emit_log(&format!(
            "→ Conectando {} ({}:{})",
            profile.name, profile.host, profile.port
        ));
        self.emit_log(&format!("→ Config: {}", profile.path));

        let platform = current_platform();
        if platform == "macos" || platform == "windows" {
            self.connect_native(profile, helper_stop, intentional_stop);
        } else {
            self.connect_linux(profile, intentional_stop);
        }
    }

    fn connect_linux(&self, profile: VpnProfile, stop: Arc<AtomicBool>) {
        let helpers = linux_helpers();
        let args: Vec<String> = if let Some((run, _)) = &helpers {
            self.emit_log(&format!("→ Helper PolicyKit · {}", profile.id));
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
                        "[{}] ✗ [{id}] Erro ao iniciar: {err}",
                        chrono_stamp()
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
            let _ = events.send(VpnEvent::Log(format!(
                "[{}] ← [{id}] Processo finalizado (código {})",
                chrono_stamp(),
                code.map(|c| c.to_string())
                    .unwrap_or_else(|| "desconhecido".into())
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
                        "Autenticação cancelada".into()
                    } else {
                        format!(
                            "Conexão encerrada (código {})",
                            code.map(|c| c.to_string())
                                .unwrap_or_else(|| "desconhecido".into())
                        )
                    };
                    live.connected_at = None;
                    live.child_pid = None;
                    persistent = live.persistent;
                } else {
                    persistent = 0;
                }
            }
            emit_state_from(&mgr_inner, &events);
            if !cancelled {
                if let Some(delay) = reconnect_delay_ms(auto, persistent, false) {
                    let _ = events.send(VpnEvent::Log(format!(
                        "[{}] ↻ [{id}] Reconexão automática em {}s…",
                        chrono_stamp(),
                        delay / 1000
                    )));
                    thread::sleep(Duration::from_millis(delay));
                    if !stop.load(Ordering::SeqCst) {
                        let _ = events.send(VpnEvent::NeedReconnect(id.clone()));
                    }
                }
            }
        });
    }

    fn connect_native(
        &self,
        profile: VpnProfile,
        helper_stop: Arc<AtomicBool>,
        intentional_stop: Arc<AtomicBool>,
    ) {
        let (tx, rx) = mpsc::channel();
        let id = profile.id.clone();
        match NativeVpnSession::start(Path::new(&profile.path), tx, helper_stop.clone()) {
            Ok(native) => {
                self.inner.lock().unwrap().live.get_mut(&id).map(|l| {
                    l.persistent = native.persistent;
                    l.can_reconnect = native.can_reconnect.clone();
                });
            }
            Err(err) => {
                self.emit_log(&format!("ERROR: {err}"));
                if let Some(live) = self.inner.lock().unwrap().live.get_mut(&id) {
                    live.status = VpnStatus::Error;
                    live.message = err;
                }
                self.emit_state();
                return;
            }
        }
        let inner = self.inner.clone();
        let events = self.events.clone();
        thread::spawn(move || {
            while let Ok(ev) = rx.recv() {
                match ev {
                    NativeEvent::Line(line) => {
                        let _ = events
                            .send(VpnEvent::Log(format!("[{}] [{id}] {line}", chrono_stamp())));
                        let platform = current_platform();
                        if (platform != "windows" && line.contains("TUNNELYARD_TUNNEL_UP"))
                            || interpret_vpn_log_line(&line) == Some("error")
                        {
                            interpret_into(&inner, &id, &line);
                            emit_state_from(&inner, &events);
                        }
                    }
                    NativeEvent::Status { phase, message } => {
                        if intentional_stop.load(Ordering::SeqCst) {
                            continue;
                        }
                        if let Ok(mut g) = inner.lock() {
                            if let Some(live) = g.live.get_mut(&id) {
                                live.status = phase;
                                live.message = if message.is_empty() {
                                    if phase == VpnStatus::Connected {
                                        "Túnel e rede validados".into()
                                    } else {
                                        "Túnel desconectado".into()
                                    }
                                } else {
                                    message
                                };
                                live.connected_at = if phase == VpnStatus::Connected {
                                    Some(now_ms())
                                } else {
                                    None
                                };
                            }
                        }
                        emit_state_from(&inner, &events);
                    }
                    NativeEvent::Close(code) => {
                        let (user_intentional, helper_stopping, can_reconnect, persistent, auto) = {
                            let g = inner.lock().unwrap();
                            let auto = g.auto_reconnect;
                            if let Some(live) = g.live.get(&id) {
                                (
                                    live.intentional_stop.load(Ordering::SeqCst),
                                    live.helper_stop.load(Ordering::SeqCst),
                                    live.can_reconnect.load(Ordering::SeqCst),
                                    live.persistent,
                                    auto,
                                )
                            } else {
                                (true, true, false, 0, auto)
                            }
                        };
                        let decision = native_close_decision(
                            user_intentional,
                            helper_stopping,
                            code,
                            can_reconnect,
                            auto,
                            persistent,
                        );
                        match decision {
                            NativeCloseDecision::DropSession => {
                                inner.lock().unwrap().live.remove(&id);
                                emit_state_from(&inner, &events);
                            }
                            NativeCloseDecision::KeepSession { reconnect_after_ms } => {
                                if let Ok(mut g) = inner.lock() {
                                    if let Some(live) = g.live.get_mut(&id) {
                                        let previous_error = if matches!(
                                            live.status,
                                            VpnStatus::Error | VpnStatus::Disconnected
                                        ) {
                                            Some(live.message.clone())
                                        } else {
                                            None
                                        };
                                        live.status = if live.status == VpnStatus::Connecting
                                            || live.status == VpnStatus::Error
                                        {
                                            VpnStatus::Error
                                        } else {
                                            VpnStatus::Disconnected
                                        };
                                        live.connected_at = None;
                                        live.message = previous_error.unwrap_or_else(|| {
                                            if code == 126 {
                                                "Autenticação cancelada".into()
                                            } else {
                                                format!("Conexão encerrada (código {code})")
                                            }
                                        });
                                    }
                                }
                                emit_state_from(&inner, &events);
                                if let Some(delay) = reconnect_after_ms {
                                    let _ = events.send(VpnEvent::Log(format!(
                                        "[{}] ↻ [{id}] Reconexão automática em {}s…",
                                        chrono_stamp(),
                                        delay / 1000
                                    )));
                                    let inner = inner.clone();
                                    let events = events.clone();
                                    let id = id.clone();
                                    thread::spawn(move || {
                                        thread::sleep(Duration::from_millis(delay));
                                        let still = inner
                                            .lock()
                                            .ok()
                                            .and_then(|g| {
                                                g.live.get(&id).map(|l| {
                                                    !l.intentional_stop.load(Ordering::SeqCst)
                                                })
                                            })
                                            .unwrap_or(false);
                                        if still {
                                            let _ = events.send(VpnEvent::NeedReconnect(id));
                                        }
                                    });
                                }
                            }
                        }
                        break;
                    }
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
            live.message = "Encerrando túnel…".into();
            path = live.profile.path.clone();
            pid = live.child_pid;
        }
        self.emit_log(&format!("→ [{profile_id}] Solicitando desconexão…"));
        self.emit_state();
        if current_platform() == "linux" {
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
        }
        self.inner.lock().unwrap().live.remove(profile_id);
        self.emit_log(&format!("← [{profile_id}] Desconectado"));
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
            live.message = "Túnel ativo".into();
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
    interpret_into(inner, id, trimmed);
    emit_state_from(inner, events);
}

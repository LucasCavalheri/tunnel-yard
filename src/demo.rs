//! Made-up profiles for screenshots and the README.
//!
//! With `TUNNELYARD_SHOT=demo` the window shows these instead of reading
//! `/etc/openfortivpn`, so a capture never leaks a real gateway or login.
//! Every host lives under the reserved `.example` domain (RFC 2606).

use crate::conf::VpnProfile;
use crate::vpn::{VpnSession, VpnState, VpnStatus};
use std::collections::HashMap;

pub fn is_demo(shot: Option<&str>) -> bool {
    shot == Some("demo")
}

fn profile(id: &str, name: &str, host: &str, port: u32, username: &str) -> VpnProfile {
    VpnProfile {
        id: id.into(),
        name: name.into(),
        path: format!("/demo/{id}.conf"),
        host: host.into(),
        port,
        username: username.into(),
        set_dns: true,
        set_routes: true,
        has_password: true,
        has_trusted_cert: true,
        persistent: 0,
    }
}

pub fn profiles() -> Vec<VpnProfile> {
    vec![
        profile("acme-hq", "Acme HQ", "vpn.acme.example", 10443, "ana.souza"),
        profile("north", "North Client", "ssl.north.example", 443, "ana"),
        profile("lab", "Lab", "lab.example", 4443, "ana.souza"),
        profile("staging", "Staging", "vpn.staging.example", 443, "deploy"),
    ]
}

/// Two tunnels up for a while, one starting, one idle. `now_ms` is Unix time in ms.
pub fn state(now_ms: u128) -> VpnState {
    let session = |id: &str, status, up_for_secs: Option<u128>| {
        (
            id.to_string(),
            VpnSession {
                profile_id: id.into(),
                status,
                message: String::new(),
                connected_at: up_for_secs.map(|secs| now_ms.saturating_sub(secs * 1000)),
            },
        )
    };
    VpnState {
        sessions: HashMap::from([
            session("acme-hq", VpnStatus::Connected, Some(12 * 60 + 48)),
            session("north", VpnStatus::Connected, Some(4 * 60 + 22)),
            session("lab", VpnStatus::Connecting, None),
        ]),
        auto_reconnect: true,
    }
}

//! Tray menu model shared by Linux (ksni) and Windows/macOS (tray-icon).

use crate::i18n::translate;
use crate::vpn::{VpnProfile, VpnState, VpnStatus};

#[derive(Clone, Debug, PartialEq)]
pub enum TrayEntry {
    Action {
        id: &'static str,
        label: String,
    },
    Profile {
        profile_id: String,
        label: String,
        checked: bool,
    },
    Separator,
}

pub fn tray_status_key(status: Option<VpnStatus>) -> &'static str {
    match status {
        Some(VpnStatus::Connected) => "status.linkUp",
        Some(VpnStatus::Connecting) => "status.handshake",
        Some(VpnStatus::Error) => "status.fault",
        _ => "status.idle",
    }
}

pub fn tray_profile_checked(status: Option<VpnStatus>) -> bool {
    matches!(
        status,
        Some(VpnStatus::Connected) | Some(VpnStatus::Connecting)
    )
}

pub fn tray_profile_label(locale: &str, name: &str, status: Option<VpnStatus>) -> String {
    format!(
        "{}  ·  {}",
        name,
        translate(locale, tray_status_key(status), &[])
    )
}

/// Show, per-profile toggle, disconnect all, refresh, quit — required tray actions.
pub fn build_tray_menu(locale: &str, profiles: &[VpnProfile], state: &VpnState) -> Vec<TrayEntry> {
    let mut items = vec![
        TrayEntry::Action {
            id: "show",
            label: translate(locale, "tray.show", &[]),
        },
        TrayEntry::Separator,
    ];
    for profile in profiles {
        let status = state.sessions.get(&profile.id).map(|s| s.status);
        items.push(TrayEntry::Profile {
            profile_id: profile.id.clone(),
            label: tray_profile_label(locale, &profile.name, status),
            checked: tray_profile_checked(status),
        });
    }
    if !profiles.is_empty() {
        items.push(TrayEntry::Separator);
    }
    items.push(TrayEntry::Action {
        id: "disconnect_all",
        label: translate(locale, "tray.disconnectAll", &[]),
    });
    items.push(TrayEntry::Action {
        id: "refresh",
        label: translate(locale, "tray.refresh", &[]),
    });
    items.push(TrayEntry::Action {
        id: "check_updates",
        label: translate(locale, "tray.checkUpdates", &[]),
    });
    items.push(TrayEntry::Separator);
    items.push(TrayEntry::Action {
        id: "quit",
        label: translate(locale, "tray.quit", &[]),
    });
    items
}

pub fn tray_action_ids(entries: &[TrayEntry]) -> Vec<&'static str> {
    entries
        .iter()
        .filter_map(|e| match e {
            TrayEntry::Action { id, .. } => Some(*id),
            _ => None,
        })
        .collect()
}

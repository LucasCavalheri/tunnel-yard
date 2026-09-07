//! Tray menu model shared by Linux (ksni) and Windows/macOS (tray-icon).

use crate::i18n::translate;
use crate::vpn::{VpnProfile, VpnState, VpnStatus};

#[derive(Clone, Debug, PartialEq)]
pub enum TrayEntry {
    Action { id: &'static str, label: String },
    Profile { profile_id: String, label: String },
    Separator,
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
        let session = state.sessions.get(&profile.id);
        let mark = match session.map(|s| s.status) {
            Some(VpnStatus::Connected) => "on",
            Some(VpnStatus::Connecting) => "...",
            Some(VpnStatus::Error) => "err",
            _ => "off",
        };
        items.push(TrayEntry::Profile {
            profile_id: profile.id.clone(),
            label: format!("{mark}  {}", profile.name),
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

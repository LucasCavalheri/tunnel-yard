use std::collections::HashSet;
use tunnel_yard::demo;
use tunnel_yard::summarize_vpn_state;

#[test]
fn demo_mode_turns_on_only_for_the_demo_shot() {
    assert!(demo::is_demo(Some("demo")));
    assert!(!demo::is_demo(Some("editor")));
    assert!(!demo::is_demo(None));
}

#[test]
fn demo_profiles_only_use_reserved_example_hosts() {
    let profiles = demo::profiles();
    assert!(profiles.len() >= 3);
    for profile in &profiles {
        let host = profile.host.as_str();
        assert!(
            host == "example" || host.ends_with(".example"),
            "{host} is not under the reserved .example domain"
        );
        assert!(
            profile.path.starts_with("/demo/"),
            "{} points at a real file",
            profile.path
        );
        assert!(!profile.path.starts_with("/etc"));
    }
    let ids: HashSet<_> = profiles.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids.len(), profiles.len(), "demo ids must be unique");
}

#[test]
fn demo_state_shows_live_starting_and_idle_tunnels() {
    let now = 1_800_000_000_000;
    let state = demo::state(now);
    let ids: HashSet<_> = demo::profiles().into_iter().map(|p| p.id).collect();
    assert!(state.sessions.keys().all(|id| ids.contains(id)));

    let summary = summarize_vpn_state(&state);
    assert_eq!(summary.connected_count, 2);
    assert_eq!(summary.connecting_count, 1);
    assert!(
        ids.len() > state.sessions.len(),
        "one profile should stay idle"
    );

    let acme = &state.sessions["acme-hq"];
    assert_eq!(acme.connected_at, Some(now - 768_000));
}

#[test]
fn the_summary_counts_only_tunnels_that_are_up() {
    let state = demo::state(1_800_000_000_000);
    let total = demo::profiles().len();
    // Lab is still connecting, so it must not be counted as connected.
    assert_eq!(
        tunnel_yard::i18n::with_locale("en", || tunnel_yard::workspace_summary(total, &state)),
        "4 profiles · 2 connected"
    );
    assert_eq!(
        tunnel_yard::i18n::with_locale("pt-BR", || tunnel_yard::workspace_summary(total, &state)),
        "4 perfis · 2 conectados"
    );
}

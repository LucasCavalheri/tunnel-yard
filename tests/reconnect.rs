//! Automatic reconnect: policy, settings persistence, profile `persistent`,
//! and the live-session gate that starts a new tunnel without a user hang-up.

use std::sync::mpsc::TryRecvError;
use std::time::Instant;
use tunnel_yard::conf::parse_vpn_conf_content;
use tunnel_yard::prevents_reconnect;
use tunnel_yard::settings::{
    apply_settings_patch, encode_settings_json, load_settings_from, parse_settings_json,
    save_settings_to, AppSettings, AppSettingsPatch,
};
use tunnel_yard::vpn::{
    linux_exit_reconnect, native_close_decision, reconnect_delay_ms, reconnect_gate,
    should_native_reconnect, NativeCloseDecision, VpnEvent, VpnManager, VpnStatus,
};

type ReconnectCase = (bool, Option<i32>, bool, bool, u32, Option<u64>);

fn temp_settings(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tunnel-yard-reconnect-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("settings.json")
}

#[test]
fn delay_requires_auto_or_profile_persistent() {
    assert_eq!(reconnect_delay_ms(false, 0, false), None);
    assert_eq!(reconnect_delay_ms(true, 0, false), Some(4000));
    assert_eq!(reconnect_delay_ms(false, 15, false), Some(15_000));
    assert_eq!(reconnect_delay_ms(true, 15, false), Some(15_000));
    assert_eq!(reconnect_delay_ms(true, 0, true), None);
    assert_eq!(reconnect_delay_ms(false, 30, true), None);
}

#[test]
fn native_unexpected_drop_reconnects_when_auto_is_on() {
    assert_eq!(should_native_reconnect(false, 1, true, true, 0), Some(4000));
    assert_eq!(should_native_reconnect(false, 0, true, true, 0), Some(4000));
    assert_eq!(
        should_native_reconnect(false, 1, true, false, 15),
        Some(15_000)
    );
}

#[test]
fn native_never_reconnects_after_user_hangup_or_auth_cancel() {
    assert_eq!(should_native_reconnect(true, 1, true, true, 0), None);
    assert_eq!(should_native_reconnect(true, 1, true, true, 30), None);
    assert_eq!(should_native_reconnect(false, 126, true, true, 0), None);
    assert_eq!(should_native_reconnect(false, 1, false, true, 0), None);
    assert_eq!(should_native_reconnect(false, 1, true, false, 0), None);
}

#[test]
fn linux_exit_skips_pkexec_cancel_and_missing_helper() {
    assert_eq!(linux_exit_reconnect(false, Some(126), true, true, 0), None);
    assert_eq!(linux_exit_reconnect(false, Some(127), true, true, 0), None);
    assert_eq!(
        linux_exit_reconnect(false, Some(1), true, true, 0),
        Some(4000)
    );
    assert_eq!(linux_exit_reconnect(false, None, true, true, 0), Some(4000));
    assert_eq!(linux_exit_reconnect(true, Some(1), true, true, 0), None);
    assert_eq!(linux_exit_reconnect(false, Some(1), false, true, 0), None);
    assert_eq!(linux_exit_reconnect(false, Some(1), true, false, 0), None);
    assert_eq!(
        linux_exit_reconnect(false, Some(1), true, false, 8),
        Some(8000)
    );
}

#[test]
fn helper_stop_file_is_not_a_user_disconnect() {
    let drop = native_close_decision(false, true, 1, true, true, 0);
    assert_eq!(
        drop,
        NativeCloseDecision::KeepSession {
            reconnect_after_ms: Some(4000)
        }
    );
    assert!(drop.emits_need_reconnect());
    assert_eq!(
        native_close_decision(true, true, 1, true, true, 0),
        NativeCloseDecision::DropSession
    );
}

#[test]
fn auth_and_cookie_failures_forbid_automatic_retry() {
    for line in [
        "Cookie was rejected by server; exiting.",
        "Cookie is no longer valid, ending session",
        "Invalid credentials",
        "Could not authenticate",
        "Authentication failed",
        "User input required",
        "Server reports that reconnect-after-drop is not allowed.",
        "VPN certificate does not match trusted-cert.",
        "Server certificate mismatch",
    ] {
        assert!(prevents_reconnect(line, true), "{line}");
        assert!(prevents_reconnect(line, false), "{line}");
    }
}

#[test]
fn unknown_ca_with_pin_still_allows_retry() {
    let warning = "Server certificate verify failed: signer not found";
    assert!(!prevents_reconnect(warning, true));
    assert!(prevents_reconnect(warning, false));
}

#[test]
fn reconnect_gate_only_fires_on_dead_unstopped_sessions() {
    assert!(!reconnect_gate(None, false));
    assert!(!reconnect_gate(None, true));
    assert!(!reconnect_gate(Some(VpnStatus::Connected), false));
    assert!(!reconnect_gate(Some(VpnStatus::Connecting), false));
    assert!(!reconnect_gate(Some(VpnStatus::Error), true));
    assert!(!reconnect_gate(Some(VpnStatus::Disconnected), true));
    assert!(reconnect_gate(Some(VpnStatus::Error), false));
    assert!(reconnect_gate(Some(VpnStatus::Disconnected), false));
}

#[test]
fn profile_persistent_is_parsed_from_conf() {
    let none = parse_vpn_conf_content("host = vpn.example.com\n", "work.conf").unwrap();
    assert_eq!(none.persistent, 0);
    let set =
        parse_vpn_conf_content("host = vpn.example.com\npersistent = 12\n", "work.conf").unwrap();
    assert_eq!(set.persistent, 12);
    let junk =
        parse_vpn_conf_content("host = vpn.example.com\npersistent = nope\n", "work.conf").unwrap();
    assert_eq!(junk.persistent, 0);
}

#[test]
fn settings_default_enables_auto_reconnect_and_persists_toggle() {
    let parsed = parse_settings_json(r#"{"locale":"en","theme":"dark"}"#);
    assert!(parsed.auto_reconnect, "missing key must default on");
    let off = parse_settings_json(r#"{"locale":"en","theme":"dark","autoReconnect":false}"#);
    assert!(!off.auto_reconnect);
    let on = apply_settings_patch(
        off,
        AppSettingsPatch {
            auto_reconnect: Some(true),
            ..Default::default()
        },
    );
    assert!(on.auto_reconnect);
    let json = encode_settings_json(&on);
    assert!(json.contains("\"autoReconnect\": true"));

    let path = temp_settings("persist");
    let saved = save_settings_to(
        &path,
        AppSettings::default(),
        AppSettingsPatch {
            auto_reconnect: Some(false),
            ..Default::default()
        },
    );
    assert!(!saved.auto_reconnect);
    let loaded = load_settings_from(&path, None);
    assert!(!loaded.auto_reconnect);
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("\"autoReconnect\": false"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn manager_reconnect_does_not_hang_or_raise_a_dead_tunnel() {
    // The skip is logged in the UI language, whichever it is.
    for (locale, expected) in [
        ("en", "↻ [missing] Not reconnecting"),
        ("pt-BR", "↻ [missing] Sem reconexão"),
    ] {
        let (mgr, rx) = VpnManager::subscribe();
        mgr.set_auto_reconnect(true);
        tunnel_yard::i18n::with_locale(locale, || mgr.reconnect("missing"));
        let started = Instant::now();
        let mut skipped = false;
        while started.elapsed().as_millis() < 400 {
            match rx.try_recv() {
                Ok(VpnEvent::Log(line)) if line.contains(expected) => {
                    skipped = true;
                    break;
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => std::thread::sleep(std::time::Duration::from_millis(5)),
                Err(_) => break,
            }
        }
        assert!(
            skipped,
            "missing session must skip reconnect without spawning pkexec ({locale})"
        );
    }
}

#[test]
fn unexpected_drop_plan_covers_every_platform_exit() {
    let cases: &[ReconnectCase] = &[
        (false, Some(1), true, true, 0, Some(4000)),
        (false, Some(0), true, true, 0, Some(4000)),
        (false, None, true, true, 0, Some(4000)),
        (false, Some(1), true, false, 20, Some(20_000)),
        (true, Some(1), true, true, 0, None),
        (false, Some(126), true, true, 0, None),
        (false, Some(127), true, true, 0, None),
        (false, Some(1), false, true, 0, None),
        (false, Some(1), true, false, 0, None),
        (false, Some(1), false, false, 15, None),
        (false, Some(1), true, false, 15, Some(15_000)),
        (true, Some(1), true, false, 15, None),
    ];
    for (stop, code, can, auto, persistent, want) in cases {
        assert_eq!(
            linux_exit_reconnect(*stop, *code, *can, *auto, *persistent),
            *want,
            "linux stop={stop} code={code:?} can={can} auto={auto} persistent={persistent}"
        );
        if let Some(code) = code {
            if *code != 127 {
                assert_eq!(
                    should_native_reconnect(*stop, *code, *can, *auto, *persistent),
                    *want,
                    "native stop={stop} code={code} can={can} auto={auto} persistent={persistent}"
                );
            }
        }
    }
}

//! Behavioral spec ported from tests/*.test.ts — calls shipped library functions.

use std::fs;
use std::process::Command;
use tunnel_yard::app_icon::build_linux_desktop_entry;
use tunnel_yard::autostart::build_autostart_desktop_entry;
use tunnel_yard::conf::{
    conf_entries, conf_path_for_id, empty_draft, is_valid_profile_id, parse_vpn_conf_content,
    parse_vpn_draft, serialize_vpn_draft, slugify_profile_id, VpnProfileDraft,
};
use tunnel_yard::deps::{build_install_plan, detect_package_family, parse_os_release_text};
use tunnel_yard::desktop::{
    should_quit_on_repeated_close, EDITOR_FIELDS, SETUP_GATE_KEYS, TRAY_MENU_KEYS, UI_SURFACES,
};
use tunnel_yard::i18n::{catalog_keys, translate};
use tunnel_yard::platform::{binary_candidates, config_directory, engine_for_platform};
use tunnel_yard::prevents_reconnect;
use tunnel_yard::tray_menu::{
    build_tray_menu, tray_action_ids, tray_profile_checked, tray_profile_label, TrayEntry,
};
use tunnel_yard::vpn::{
    interpret_vpn_log_line, list_vpn_profiles, native_close_decision, reconnect_delay_ms,
    should_native_reconnect, summarize_vpn_state, NativeCloseDecision, VpnProfile, VpnSession,
    VpnState, VpnStatus,
};

#[test]
fn parse_typical_openfortivpn_profile() {
    let profile = parse_vpn_conf_content(
        r#"
host = vpn.example.com
port = 10443
username = alice
password = secret
trusted-cert = abcd
set-dns = 0
set-routes = 1
# comment
"#,
        "/etc/openfortivpn/acme-corp.conf",
    )
    .unwrap();
    assert_eq!(profile.id, "acme-corp");
    assert_eq!(profile.name, "Acme Corp");
    assert_eq!(profile.host, "vpn.example.com");
    assert_eq!(profile.port, 10443);
    assert_eq!(profile.username, "alice");
    assert!(!profile.set_dns);
    assert!(profile.set_routes);
    assert!(profile.has_password);
    assert!(profile.has_trusted_cert);
}

#[test]
fn parse_returns_none_without_host() {
    assert!(parse_vpn_conf_content("username = bob\n", "/tmp/broken.conf").is_none());
}

#[test]
fn accepts_user_alias_and_default_port() {
    let profile =
        parse_vpn_conf_content("host = a.example\nuser = bob\n", "/tmp/lab.conf").unwrap();
    assert_eq!(profile.username, "bob");
    assert_eq!(profile.port, 443);
}

#[test]
fn log_markers() {
    assert_eq!(
        interpret_vpn_log_line("INFO:   Tunnel is up and running."),
        Some("connected")
    );
    assert_eq!(
        interpret_vpn_log_line("ERROR:  Could not authenticate."),
        Some("error")
    );
    assert_eq!(interpret_vpn_log_line("INFO:   Resolving host..."), None);
}

#[test]
fn lists_and_sorts_profiles() {
    let dir = tempfile_dir("list");
    fs::write(dir.join("zeta.conf"), "host = z.example\nport = 443\n").unwrap();
    fs::write(
        dir.join("alpha.conf"),
        "host = a.example\nport = 10443\nusername = u\n",
    )
    .unwrap();
    fs::write(dir.join("ignore.txt"), "nope").unwrap();
    fs::write(dir.join("bad.conf"), "username = only\n").unwrap();
    let profiles = list_vpn_profiles(&dir);
    let ids: Vec<_> = profiles.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["alpha", "zeta"]);
    assert_eq!(profiles[0].port, 10443);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_profile_dir_is_empty() {
    assert!(
        list_vpn_profiles(std::path::Path::new("/tmp/tunnel-yard-does-not-exist-xyz")).is_empty()
    );
}

#[test]
fn summarize_counts_concurrent_sessions() {
    let mut sessions = std::collections::HashMap::new();
    sessions.insert(
        "mkraft".into(),
        VpnSession {
            profile_id: "mkraft".into(),
            status: VpnStatus::Connected,
            message: "up".into(),
            connected_at: Some(1),
        },
    );
    sessions.insert(
        "tecsul".into(),
        VpnSession {
            profile_id: "tecsul".into(),
            status: VpnStatus::Connecting,
            message: "…".into(),
            connected_at: None,
        },
    );
    let summary = summarize_vpn_state(&VpnState {
        auto_reconnect: false,
        sessions,
    });
    assert_eq!(summary.connected_count, 1);
    assert_eq!(summary.connecting_count, 1);
    assert_eq!(summary.overall, VpnStatus::Connected);
}

#[test]
fn reconnect_policy() {
    assert_eq!(reconnect_delay_ms(false, 0, false), None);
    assert_eq!(reconnect_delay_ms(true, 0, false), Some(4000));
    assert_eq!(reconnect_delay_ms(false, 15, false), Some(15_000));
    assert_eq!(reconnect_delay_ms(true, 15, true), None);
}

#[test]
fn draft_round_trip() {
    let draft = VpnProfileDraft {
        id: "tecsul".into(),
        host: "br-spo1-ssl-vpn.wevy.cloud".into(),
        port: 10443,
        username: "alice".into(),
        password: "secret".into(),
        trusted_cert: "abc123".into(),
        set_dns: false,
        set_routes: true,
        realm: "".into(),
        persistent: 0,
        ..empty_draft()
    };
    let raw = serialize_vpn_draft(&draft).unwrap();
    assert!(raw.contains("host = br-spo1-ssl-vpn.wevy.cloud"));
    assert!(raw.contains("set-dns = 0"));
    assert!(raw.contains("set-routes = 1"));
    assert!(raw.contains("trusted-cert = abc123"));
    let parsed = parse_vpn_draft(&raw, "tecsul.conf").unwrap().unwrap();
    assert_eq!(parsed.id, draft.id);
    assert_eq!(parsed.host, draft.host);
    assert_eq!(parsed.port, draft.port);
    assert_eq!(parsed.username, draft.username);
    assert_eq!(parsed.password, draft.password);
    assert_eq!(parsed.trusted_cert, draft.trusted_cert);
    assert_eq!(parsed.set_dns, draft.set_dns);
    assert_eq!(parsed.set_routes, draft.set_routes);
}

#[test]
fn slugify_and_validate_ids() {
    assert_eq!(slugify_profile_id("Acme Corp.conf"), "acme-corp");
    assert!(is_valid_profile_id("mkraft"));
    assert!(!is_valid_profile_id("Bad Id"));
}

#[test]
fn platform_paths_and_engines() {
    assert_eq!(
        config_directory("linux", None).unwrap(),
        "/etc/openfortivpn"
    );
    assert_eq!(
        config_directory("gnu/linux", None).unwrap(),
        "/etc/openfortivpn"
    );
    assert!(config_directory("darwin", Some("/Users/test"))
        .unwrap_err()
        .contains("Unsupported platform"));
    assert!(config_directory("win32", Some("/home/test"))
        .unwrap_err()
        .contains("Unsupported platform"));
    assert_eq!(engine_for_platform("linux"), "openfortivpn");
    assert!(binary_candidates("openfortivpn", "linux")
        .iter()
        .any(|p| p == "/usr/bin/openfortivpn"));
}

#[test]
fn extra_options_are_preserved_on_linux() {
    let imported = parse_vpn_draft(
        "host = vpn.example.com\nset-dns=0\nset-routes=1\npppd-log=/tmp/vpn.log\n",
        "work.conf",
    )
    .unwrap()
    .unwrap();
    let mut edited = imported;
    edited.username = "alice".into();
    let raw = serialize_vpn_draft(&edited).unwrap();
    assert!(raw.contains("pppd-log = /tmp/vpn.log"));
    assert!(raw.contains("username = alice"));
}

#[test]
fn blocks_traversal_and_newline_injection() {
    assert!(conf_path_for_id("../outside").is_err());
    let mut evil = empty_draft();
    evil.host = "host\npppd-plugin = evil".into();
    assert!(serialize_vpn_draft(&evil).is_err());
    let mut dup = empty_draft();
    dup.host = "host".into();
    dup.extra_options = vec![("host".into(), "different".into())];
    assert!(serialize_vpn_draft(&dup).is_err());
}

#[test]
fn no_dtls_marker() {
    let raw = "host=vpn.example\n# my-vpns-no-dtls = 1\n";
    let parsed = parse_vpn_draft(raw, "tecsul.conf").unwrap().unwrap();
    assert!(parsed.no_dtls);
    let mut edited = parsed;
    edited.host = "vpn.example".into();
    let preserved = serialize_vpn_draft(&edited).unwrap();
    assert!(preserved.contains("# tunnel-yard-no-dtls = 1"));
    assert!(
        !parse_vpn_draft("host=vpn.example", "plain.conf")
            .unwrap()
            .unwrap()
            .no_dtls
    );
}

#[test]
fn legacy_tunnel_marker() {
    let raw = "host=vpn.example\n# my-vpns-legacy-tunnel = 1\n";
    assert!(
        parse_vpn_draft(raw, "tecsul.conf")
            .unwrap()
            .unwrap()
            .legacy_tunnel
    );
    let parsed = parse_vpn_draft(raw, "tecsul.conf").unwrap().unwrap();
    let mut edited = parsed;
    edited.host = "vpn.example".into();
    assert!(serialize_vpn_draft(&edited)
        .unwrap()
        .contains("# tunnel-yard-legacy-tunnel = 1"));
    assert!(
        !parse_vpn_draft("host=vpn.example", "plain.conf")
            .unwrap()
            .unwrap()
            .legacy_tunnel
    );
}

#[test]
fn prevents_reconnect_auth_failures() {
    for line in [
        "Cookie was rejected by server; exiting.",
        "Cookie is no longer valid, ending session",
        "Server reports that reconnect-after-drop is not allowed.",
        "Invalid credentials",
        "VPN certificate does not match trusted-cert.",
    ] {
        assert!(prevents_reconnect(line, true), "{line}");
    }
}

#[test]
fn unknown_ca_with_verified_pin() {
    let warning = "Server certificate verify failed: signer not found";
    assert!(!prevents_reconnect(warning, true));
    assert!(prevents_reconnect(warning, false));
    assert!(prevents_reconnect("Server certificate mismatch", true));
}

#[test]
fn health_comments_round_trip() {
    let mut draft = empty_draft();
    draft.host = "vpn.example.test".into();
    draft.health_host = Some("198.18.0.2".into());
    draft.health_port = Some(30015);
    let raw = serialize_vpn_draft(&draft).unwrap();
    let mut edited = parse_vpn_draft(&raw, "test.conf").unwrap().unwrap();
    edited.username = "alice".into();
    let edited_raw = serialize_vpn_draft(&edited).unwrap();
    let again = parse_vpn_draft(&edited_raw, "test.conf").unwrap().unwrap();
    assert_eq!(again.health_host.as_deref(), Some("198.18.0.2"));
    assert_eq!(again.health_port, Some(30015));
    assert!(!conf_entries(&edited_raw)
        .unwrap()
        .iter()
        .any(|(k, _)| k.starts_with("my-vpns")));
    assert!(parse_vpn_draft(
        "host=vpn.example\n# my-vpns-health-host=198.18.0.2",
        "half.conf"
    )
    .is_err());
    let mut bad = empty_draft();
    bad.host = "vpn.example".into();
    bad.health_host = Some("198.18.0.2\npassword=other".into());
    bad.health_port = Some(80);
    assert!(serialize_vpn_draft(&bad).is_err());
}

#[test]
fn os_release_and_install_plan() {
    let map = parse_os_release_text(
        r#"
NAME="Ubuntu"
VERSION_ID="26.04"
ID=ubuntu
ID_LIKE=debian
PRETTY_NAME="Ubuntu 26.04 LTS"
# comment
"#,
    );
    assert_eq!(map.get("ID").unwrap(), "ubuntu");
    assert_eq!(map.get("ID_LIKE").unwrap(), "debian");
    assert_eq!(map.get("PRETTY_NAME").unwrap(), "Ubuntu 26.04 LTS");

    fn none(_: &str) -> bool {
        false
    }
    fn has_yum(p: &str) -> bool {
        p.contains("yum")
    }
    fn has_dnf5(p: &str) -> bool {
        p.contains("dnf5")
    }
    assert_eq!(
        detect_package_family("ubuntu", &["debian".into()], none),
        "apt"
    );
    assert_eq!(
        detect_package_family("linuxmint", &["ubuntu".into(), "debian".into()], none),
        "apt"
    );
    assert_eq!(detect_package_family("fedora", &[], none), "dnf");
    assert_eq!(
        detect_package_family("rocky", &["rhel".into(), "fedora".into()], has_yum),
        "yum"
    );
    assert_eq!(detect_package_family("fedora", &[], has_dnf5), "dnf");
    assert_eq!(
        detect_package_family("opensuse-tumbleweed", &["suse".into()], none),
        "zypper"
    );
    assert_eq!(detect_package_family("arch", &[], none), "pacman");
    assert_eq!(
        detect_package_family("manjaro", &["arch".into()], none),
        "pacman"
    );
    assert_eq!(detect_package_family("weirdos", &[], none), "unknown");

    let plan = build_install_plan("apt");
    assert!(plan.can_auto_install);
    assert!(plan.install_command.unwrap().contains("apt-get install"));
    assert!(plan
        .pkexec_args
        .unwrap()
        .iter()
        .any(|a| a == "openfortivpn"));
    let unknown = build_install_plan("unknown");
    assert!(!unknown.can_auto_install);
    assert!(unknown.pkexec_args.is_none());
}

#[test]
fn i18n_key_parity_and_interpolation() {
    let mut en = catalog_keys("en");
    let mut pt = catalog_keys("pt-BR");
    en.sort();
    pt.sort();
    assert_eq!(en, pt);
    assert_eq!(
        translate(
            "en",
            "ops.deskSummary",
            &[("up", "2".into()), ("handshake", "1".into())]
        ),
        "2 connected · 1 starting"
    );
    assert_eq!(
        translate(
            "pt-BR",
            "ops.deskSummary",
            &[("up", "2".into()), ("handshake", "1".into())]
        ),
        "2 conectadas · 1 iniciando"
    );
    for key in TRAY_MENU_KEYS
        .iter()
        .chain(EDITOR_FIELDS)
        .chain(SETUP_GATE_KEYS)
    {
        assert!(en.contains(key), "missing i18n key {key}");
    }
    assert!(UI_SURFACES.contains(&"tray"));
    assert!(UI_SURFACES.contains(&"profile_editor"));
    assert!(UI_SURFACES.contains(&"console"));
    assert!(UI_SURFACES.contains(&"setup_gate"));
    assert!(UI_SURFACES.contains(&"locale_switch"));
}

#[test]
fn autostart_desktop_entry() {
    let entry = build_autostart_desktop_entry("\"/opt/TunnelYard/tunnel-yard\" --hidden");
    assert!(entry.contains("[Desktop Entry]"));
    assert!(entry.contains("Type=Application"));
    assert!(entry.contains("Name=TunnelYard"));
    assert!(entry.contains("Exec=\"/opt/TunnelYard/tunnel-yard\" --hidden"));
    assert!(entry.contains("X-GNOME-Autostart-enabled=true"));
    assert!(entry.contains("Icon=tunnel-yard"));
    assert!(entry.contains("StartupWMClass=lucas.cavalheri.tunnelyard"));
}

#[test]
fn linux_launcher_matches_wayland_app_id_and_quotes_paths() {
    let entry = build_linux_desktop_entry(
        std::path::Path::new("/opt/TunnelYard/tunnel-yard%dev"),
        std::path::Path::new("/home/test/.cache/tunnel-yard/icon.png"),
    );
    assert!(entry.contains("Exec=\"/opt/TunnelYard/tunnel-yard%%dev\""));
    assert!(entry.contains("Icon=/home/test/.cache/tunnel-yard/icon.png"));
    assert!(entry.contains("StartupNotify=true"));
    assert!(entry.contains(&format!("StartupWMClass={}", tunnel_yard::APP_ID)));
    assert_eq!(tunnel_yard::APP_ID, "lucas.cavalheri.tunnelyard");
}

#[test]
fn debian_postrm_does_not_delete_files_during_upgrade() {
    let script = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/packaging/after-remove.sh"
    ))
    .unwrap();
    let cleanup = script
        .find("root_path /usr/lib/tunnel-yard/run-vpn.sh")
        .expect("postrm cleanup must remain present");
    let guard = &script[..cleanup];
    assert!(guard.contains("case \"${1:-}\" in"));
    assert!(guard.contains("remove|purge)"));
    assert!(guard.contains("exit 0"));

    let package_builder = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/packaging/build-linux-packages.sh"
    ))
    .unwrap();
    assert!(package_builder.contains("/usr/lib/tunnel-yard/payload"));
    assert!(package_builder.contains("/usr/lib/tunnel-yard/tunnel-yard"));

    let postinst = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/packaging/after-install.sh"
    ))
    .unwrap();
    assert!(postinst.contains("PACKAGE_PAYLOAD"));
    assert!(postinst.contains("ln -sfn ../lib/tunnel-yard/tunnel-yard"));
}

#[cfg(unix)]
#[test]
fn debian_package_upgrade_preserves_and_repairs_the_full_installation() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile_dir("debian-upgrade-lifecycle");
    let package_root = root.join("usr/lib/tunnel-yard");
    let payload = package_root.join("payload");
    fs::create_dir_all(&payload).unwrap();
    fs::create_dir_all(root.join("etc/apt/sources.list.d")).unwrap();

    let binary = package_root.join("tunnel-yard");
    fs::write(&binary, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();

    for (name, contents) in [
        ("run-vpn.sh", "#!/bin/sh\n"),
        ("stop-vpn.sh", "#!/bin/sh\n"),
        ("tunnel-yard.desktop", "[Desktop Entry]\nExec=tunnel-yard\n"),
        ("lucas.cavalheri.tunnelyard.policy", "policy\n"),
        ("icon.png", "icon-256\n"),
        ("icon-64.png", "icon-64\n"),
        ("icon-32.png", "icon-32\n"),
        ("tunnel-yard-archive-keyring.asc", "keyring\n"),
    ] {
        fs::write(payload.join(name), contents).unwrap();
    }

    run_packaging_script("after-install.sh", &root);
    assert_debian_installation_complete(&root);

    // This is the real dpkg upgrade call. It must be a no-op for the old
    // package's postrm, otherwise the newly unpacked files disappear again.
    run_packaging_script_with_args("after-remove.sh", &["upgrade", "2.9.0"], &root);
    assert_debian_installation_complete(&root);

    // Reproduce the destructive cleanup from the released 2.8.0 postrm. The
    // recovery payload must let the new postinst repair every user-visible
    // package path without touching the host filesystem.
    for relative in [
        "usr/bin/tunnel-yard",
        "usr/lib/tunnel-yard/run-vpn.sh",
        "usr/lib/tunnel-yard/stop-vpn.sh",
        "usr/share/applications/lucas.cavalheri.tunnelyard.desktop",
        "usr/share/polkit-1/actions/lucas.cavalheri.tunnelyard.policy",
        "usr/share/icons/hicolor/256x256/apps/tunnel-yard.png",
        "usr/share/icons/hicolor/64x64/apps/tunnel-yard.png",
        "usr/share/icons/hicolor/32x32/apps/tunnel-yard.png",
        "usr/share/pixmaps/tunnel-yard.png",
        "usr/share/keyrings/tunnel-yard-archive-keyring.asc",
        "etc/apt/sources.list.d/tunnel-yard.list",
    ] {
        let _ = fs::remove_file(root.join(relative));
    }
    assert!(!root.join("usr/bin/tunnel-yard").exists());
    assert!(!root
        .join("usr/share/applications/lucas.cavalheri.tunnelyard.desktop")
        .exists());

    run_packaging_script("after-install.sh", &root);
    assert_debian_installation_complete(&root);
    let desktop =
        fs::read_to_string(root.join("usr/share/applications/lucas.cavalheri.tunnelyard.desktop"))
            .unwrap();
    assert!(desktop.contains("Exec=/usr/bin/tunnel-yard %U"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn window_close_hides_until_a_second_distinct_click() {
    assert!(!should_quit_on_repeated_close(None, 10_000));
    // Same OS close event re-fired on the next frame (~16ms) must not quit.
    assert!(!should_quit_on_repeated_close(Some(10_000), 10_016));
    assert!(!should_quit_on_repeated_close(Some(10_000), 10_079));
    // A second click shortly after the first quits.
    assert!(should_quit_on_repeated_close(Some(10_000), 10_200));
    assert!(should_quit_on_repeated_close(Some(10_000), 10_899));
    // Too late: treat as a fresh hide, not a quit.
    assert!(!should_quit_on_repeated_close(Some(10_000), 10_900));
    assert!(!should_quit_on_repeated_close(Some(10_000), 12_000));
}

#[test]
fn smoke_report_has_engine_and_platform() {
    let report = tunnel_yard::smoke::smoke_report();
    assert!(!report.engine.is_empty());
    assert!(!report.platform.is_empty());
    assert!(!report.architecture.is_empty());
    assert!(!report.config_dir.is_empty());
    assert!(report.ok);
}

#[test]
fn native_reconnect_policy() {
    assert_eq!(should_native_reconnect(true, 0, true, true, 0), None);
    assert_eq!(should_native_reconnect(false, 126, true, true, 0), None);
    assert_eq!(should_native_reconnect(false, 1, false, true, 0), None);
    assert_eq!(should_native_reconnect(false, 1, true, true, 0), Some(4000));
    assert_eq!(
        should_native_reconnect(false, 1, true, false, 15),
        Some(15_000)
    );
    assert_eq!(should_native_reconnect(false, 1, true, false, 0), None);
}

#[test]
fn native_close_helper_stop_is_not_user_disconnect() {
    // (user_intentional, helper_stopping, code, can_reconnect, auto, persistent)
    // Helper wrote the stop file after status.json disconnected — still NeedReconnect.
    let helper_drop = native_close_decision(false, true, 1, true, true, 0);
    assert_eq!(
        helper_drop,
        NativeCloseDecision::KeepSession {
            reconnect_after_ms: Some(4000)
        }
    );
    assert!(helper_drop.emits_need_reconnect());
    let helper_persistent = native_close_decision(false, true, 0, true, false, 15);
    assert_eq!(
        helper_persistent,
        NativeCloseDecision::KeepSession {
            reconnect_after_ms: Some(15_000)
        }
    );
    assert!(helper_persistent.emits_need_reconnect());
    // helper_stopping must not change the decision vs a clean helper exit.
    assert_eq!(
        native_close_decision(false, true, 1, true, true, 0),
        native_close_decision(false, false, 1, true, true, 0)
    );
    // User disconnect: drop session, never NeedReconnect (even if helper_stopping too).
    assert_eq!(
        native_close_decision(true, true, 1, true, true, 0),
        NativeCloseDecision::DropSession
    );
    assert!(!native_close_decision(true, true, 1, true, true, 0).emits_need_reconnect());
    assert_eq!(
        native_close_decision(true, false, 1, true, true, 0),
        NativeCloseDecision::DropSession
    );
    assert!(!native_close_decision(false, true, 126, true, true, 0).emits_need_reconnect());
    assert!(!native_close_decision(false, true, 1, false, true, 0).emits_need_reconnect());
}

#[test]
fn tray_menu_has_required_actions_on_every_platform_model() {
    let profiles = vec![dummy_profile("work"), dummy_profile("home")];
    let mut sessions = std::collections::HashMap::new();
    sessions.insert(
        "work".into(),
        VpnSession {
            profile_id: "work".into(),
            status: VpnStatus::Connected,
            message: "up".into(),
            connected_at: Some(1),
        },
    );
    let state = VpnState {
        sessions,
        auto_reconnect: false,
    };
    let entries = build_tray_menu("en", &profiles, &state);
    let ids = tray_action_ids(&entries);
    assert_eq!(
        ids,
        ["show", "disconnect_all", "refresh", "check_updates", "quit"]
    );
    let labels: Vec<_> = entries
        .iter()
        .filter_map(|e| match e {
            TrayEntry::Profile {
                label,
                profile_id,
                checked,
            } => Some((profile_id.as_str(), label.as_str(), *checked)),
            _ => None,
        })
        .collect();
    assert!(labels
        .iter()
        .any(|(id, l, checked)| { *id == "work" && *checked && *l == "work  ·  Connected" }));
    assert!(labels
        .iter()
        .any(|(id, l, checked)| { *id == "home" && !*checked && *l == "home  ·  Idle" }));
    let both_up = {
        let mut sessions = std::collections::HashMap::new();
        for id in ["work", "home"] {
            sessions.insert(
                id.into(),
                VpnSession {
                    profile_id: id.into(),
                    status: VpnStatus::Connected,
                    message: "up".into(),
                    connected_at: Some(1),
                },
            );
        }
        VpnState {
            sessions,
            auto_reconnect: false,
        }
    };
    let pt = build_tray_menu("pt-BR", &profiles, &both_up);
    let pt_labels: Vec<_> = pt
        .iter()
        .filter_map(|e| match e {
            TrayEntry::Profile {
                label,
                profile_id,
                checked,
            } => Some((profile_id.clone(), label.clone(), *checked)),
            _ => None,
        })
        .collect();
    assert!(pt_labels
        .iter()
        .any(|(id, l, checked)| id == "work" && *checked && l == "work  ·  Conectado"));
    assert!(pt_labels
        .iter()
        .any(|(id, l, checked)| id == "home" && *checked && l == "home  ·  Conectado"));
    assert!(tray_profile_checked(Some(VpnStatus::Connected)));
    assert!(!tray_profile_checked(Some(VpnStatus::Disconnected)));
    assert_eq!(
        tray_profile_label("pt-BR", "Mkraft", Some(VpnStatus::Connected)),
        "Mkraft  ·  Conectado"
    );
    assert_eq!(tray_profile_label("en", "Mkraft", None), "Mkraft  ·  Idle");
    let empty = tray_action_ids(&build_tray_menu(
        "pt-BR",
        &[],
        &VpnState {
            sessions: Default::default(),
            auto_reconnect: false,
        },
    ));
    assert_eq!(
        empty,
        ["show", "disconnect_all", "refresh", "check_updates", "quit"]
    );
}

#[test]
fn linux_notifications_use_notify_send() {
    use tunnel_yard::os_ui::notification_argv_with_icon;
    let png = std::path::Path::new("/tmp/tunnel-yard-icon.png");
    let (cmd, args) = notification_argv_with_icon("VPN connected", "Tunnel work is up.", png);
    assert_eq!(cmd, "notify-send");
    assert!(args.iter().any(|a| a == "VPN connected"));
    assert!(args.iter().any(|a| a == "-i"));
    assert!(args.iter().any(|a| a.ends_with("tunnel-yard-icon.png")));
    assert!(args
        .iter()
        .any(|a| a.contains("desktop-entry:lucas.cavalheri.tunnelyard")));
}

#[test]
fn github_update_check_compares_tags_and_parses_release_json() {
    use tunnel_yard::updates::{
        artifact_architecture, artifact_kind, artifact_platform, check_for_app_update,
        compare_versions, normalize_tag,
    };
    assert!(compare_versions("1.0.2", "1.0.1") > 0);
    assert!(compare_versions("1.0.1", "1.0.2") < 0);
    assert_eq!(compare_versions("1.0.1", "1.0.1"), 0);
    assert_eq!(normalize_tag("v1.0.2"), "1.0.2");
    assert!(compare_versions("v1.0.2", "1.0.1") > 0);

    let body = r#"{
        "tag_name": "v1.1.8",
        "html_url": "https://github.com/LucasCavalheri/tunnel-yard/releases/tag/v1.1.8",
        "draft": false,
        "assets": [
            {"name": "tunnel-yard-linux-x64", "browser_download_url": "https://github.com/LucasCavalheri/tunnel-yard/releases/download/v1.1.8/tunnel-yard-linux-x64"}
        ]
    }"#;
    let info = check_for_app_update("1.1.7", |_| Ok(body.to_string()))
        .unwrap()
        .expect("newer release");
    assert_eq!(info.latest, "1.1.8");
    assert_eq!(info.current, "1.1.7");
    assert!(info.url.contains("v1.1.8"));
    assert_eq!(info.artifacts.len(), 1);
    assert_eq!(info.artifacts[0].kind, "linux");
    assert_eq!(info.artifacts[0].platform.as_deref(), Some("linux"));
    assert_eq!(info.artifacts[0].architecture.as_deref(), Some("x64"));
    assert_eq!(
        info.artifacts[0].compatible,
        tunnel_yard::platform::current_platform() == "linux"
            && tunnel_yard::arch::current_arch() == "x64"
    );
    assert_eq!(artifact_kind("tunnel-yard-macos-x64"), None);
    assert_eq!(artifact_kind("tunnel-yard-macos.dmg"), None);
    assert_eq!(artifact_kind("tunnel-yard-linux-x64.tar.gz"), Some("linux"));
    assert_eq!(artifact_kind("my-vpns-linux-x64"), Some("linux"));
    assert_eq!(artifact_kind("tunnel-yard_2.7.0_amd64.deb"), Some("deb"));
    assert_eq!(
        artifact_architecture("tunnel-yard_2.7.0_amd64.deb"),
        Some("x64")
    );
    assert_eq!(
        artifact_kind("tunnel-yard-2.7.0-1.aarch64.rpm"),
        Some("rpm")
    );
    assert_eq!(
        artifact_architecture("tunnel-yard-2.7.0-1.aarch64.rpm"),
        Some("arm64")
    );
    assert_eq!(artifact_kind("tunnel-yard-windows-arm64.exe"), None);
    assert_eq!(artifact_platform("tunnel-yard-windows-arm64.exe"), None);

    assert!(check_for_app_update("1.1.8", |_| Ok(body.to_string()))
        .unwrap()
        .is_none());

    use tunnel_yard::updates::{
        linux_package_kind, select_install_artifact, InstallKind, UpdateArtifact, UpdateInfo,
    };
    assert_eq!(linux_package_kind(true, false), InstallKind::Deb);
    assert_eq!(linux_package_kind(false, true), InstallKind::Rpm);
    let info = UpdateInfo {
        current: "2.6.0".into(),
        latest: "2.7.0".into(),
        url: "https://github.com/LucasCavalheri/tunnel-yard/releases/tag/v2.7.0".into(),
        artifacts: vec![
            UpdateArtifact {
                name: "tunnel-yard_2.7.0_amd64.deb".into(),
                url: "https://github.com/LucasCavalheri/tunnel-yard/releases/download/v2.7.0/tunnel-yard_2.7.0_amd64.deb".into(),
                kind: "deb".into(),
                digest: None,
                platform: Some("linux".into()),
                architecture: Some("x64".into()),
                compatible: true,
            },
            UpdateArtifact {
                name: "tunnel-yard-linux-x64".into(),
                url: "https://github.com/LucasCavalheri/tunnel-yard/releases/download/v2.7.0/tunnel-yard-linux-x64".into(),
                kind: "linux".into(),
                digest: None,
                platform: Some("linux".into()),
                architecture: Some("x64".into()),
                compatible: true,
            },
        ],
    };
    assert_eq!(
        select_install_artifact(&info, InstallKind::Deb).map(|a| a.kind.as_str()),
        Some("deb")
    );
    assert_eq!(
        select_install_artifact(&info, InstallKind::LinuxPortable).map(|a| a.name.as_str()),
        Some("tunnel-yard-linux-x64")
    );
}

#[test]
fn helpers_exist_in_tree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in [
        "packaging/run-vpn.sh",
        "packaging/stop-vpn.sh",
        "packaging/build-linux-packages.sh",
        "packaging/polkit/lucas.cavalheri.tunnelyard.policy",
        "packaging/tunnel-yard.desktop",
        "public/icon.png",
        "public/icon-32.png",
        "public/icon.ico",
        "public/icon.svg",
        "public/icon-master.png",
        "scripts/generate-icon.py",
        "assets/icons/wifi-01.svg",
        "assets/icons/sun-03.svg",
        "assets/icons/moon-02.svg",
        "assets/icons/settings-02.svg",
        ".gitattributes",
    ] {
        assert!(root.join(name).exists(), "{name}");
    }
    let desktop = fs::read_to_string(root.join("packaging/tunnel-yard.desktop")).unwrap();
    assert!(desktop.contains("StartupWMClass=lucas.cavalheri.tunnelyard"));
    assert!(desktop.contains("Exec=tunnel-yard"));
    let postinst = fs::read_to_string(root.join("packaging/after-install.sh")).unwrap();
    assert!(!postinst.contains("resources/"));
    assert!(postinst.contains("StartupWMClass=lucas.cavalheri.tunnelyard"));
}

#[test]
fn gitattributes_keeps_github_language_stats_on_rust() {
    let attrs =
        fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".gitattributes"))
            .unwrap();
    assert!(
        attrs
            .lines()
            .any(|line| line.trim() == "* linguist-vendored"),
        "non-Rust paths must be vendored for Linguist"
    );
    assert!(
        attrs
            .lines()
            .any(|line| line.trim() == "*.rs linguist-vendored=false"),
        "Rust sources must remain in language statistics"
    );
    assert!(
        attrs
            .lines()
            .any(|line| line.trim() == "*.rs linguist-detectable"),
        "Rust sources must stay detectable"
    );
    let rust_pos = attrs.rfind("*.rs linguist-vendored=false").unwrap();
    let star_pos = attrs.find("* linguist-vendored").unwrap();
    assert!(
        rust_pos > star_pos,
        "*.rs override must come after the catch-all vendored rule"
    );
}

#[test]
fn gpui_kit_components_are_shipped() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let ui = fs::read_to_string(root.join("src/ui.rs")).unwrap();

    assert!(manifest.contains("gpui-kit = \"0.6\""));
    for component in [
        "Root::new",
        "TitleBar::new",
        "Button::new",
        "Input::new",
        "Switch::new",
    ] {
        assert!(
            ui.contains(component),
            "missing GPUI Kit component: {component}"
        );
    }
    assert!(ui.contains("title-theme-toggle"));
    assert!(ui.contains("theme-choice-"));
    assert!(ui.contains("render_theme_picker"));
    assert!(ui.contains("AppAssets"));
    assert!(
        !ui.contains(".tooltip(self.t(\"form.cancel\"))"),
        "overlay close buttons must not leak a Cancelar tooltip into the title bar"
    );
}

#[test]
fn app_icon_is_a_real_png_and_argb_pixmap() {
    use tunnel_yard::app_icon::{
        png_argb_pixmap, rgba_to_argb, tray_pixmap_pngs, APP_ICON_ICO, APP_ICON_PNG,
        APP_ICON_PNG_32, APP_ICON_PNG_64,
    };
    assert!(APP_ICON_PNG.starts_with(b"\x89PNG"));
    assert!(APP_ICON_ICO.len() > 16);
    let (w, h, data) = png_argb_pixmap(APP_ICON_PNG).expect("decode mark");
    assert!(w >= 32 && h >= 32);
    assert_eq!(data.len(), (w * h * 4) as usize);
    assert_eq!(
        rgba_to_argb(&[0x11, 0x22, 0x33, 0x44]),
        vec![0x44, 0x11, 0x22, 0x33]
    );
    let tray = tray_pixmap_pngs();
    assert!(tray.contains(&APP_ICON_PNG_32));
    assert!(tray.contains(&APP_ICON_PNG_64));
    assert!(
        !tray.contains(&APP_ICON_PNG),
        "256px mark must not be sent as a StatusNotifier pixmap"
    );
    for bytes in tray {
        let (tw, th, _) = png_argb_pixmap(bytes).expect("tray pixmap");
        assert!(tw <= 64 && th <= 64, "tray pixmap {tw}x{th} is too large");
    }
}

#[test]
fn hugeicons_are_current_color_svgs_for_every_desk_icon() {
    use tunnel_yard::icons::{svg_bytes, Huge, FILES, OVERLAY_CANCEL_IDS, TITLE_BAR_ACTION_IDS};
    assert_eq!(FILES.len(), Huge::all().len());
    for icon in Huge::all() {
        let bytes = svg_bytes(icon.asset_path()).expect(icon.asset_path());
        let svg = std::str::from_utf8(bytes).expect(icon.asset_path());
        assert!(svg.contains("<svg"), "{}", icon.asset_path());
        assert!(
            svg.contains("currentColor"),
            "{} is not tintable",
            icon.asset_path()
        );
        assert!(
            !svg.contains("#141B34"),
            "{} still has the Hugeicons default ink",
            icon.asset_path()
        );
    }
    for id in OVERLAY_CANCEL_IDS {
        assert!(
            !TITLE_BAR_ACTION_IDS.contains(id),
            "{id} must not be a title-bar action"
        );
    }
}

#[test]
fn light_and_dark_palettes_diverge_and_keep_brand() {
    use tunnel_yard::theme::{resolve_theme_mode, toggle_light_dark, Palette, BRAND};
    let dark = Palette::dark();
    let light = Palette::light();
    assert_eq!(BRAND, 0xff6b35);
    assert!(light.is_light());
    assert!(!dark.is_light());
    assert_ne!(dark.workspace_bg, light.workspace_bg);
    assert!(dark.workspace_bg < 0x202020);
    assert!(light.workspace_bg > 0xe0e0e0);
    assert_eq!(resolve_theme_mode("system", true), "dark");
    assert_eq!(resolve_theme_mode("system", false), "light");
    assert_eq!(resolve_theme_mode("light", true), "light");
    assert_eq!(resolve_theme_mode("dark", false), "dark");
    assert_eq!(toggle_light_dark("dark", true), "light");
    assert_eq!(toggle_light_dark("system", false), "dark");
}

#[test]
fn app_mark_is_a_portal_with_true_alpha() {
    use tunnel_yard::app_icon::{png_argb_pixmap, APP_ICON_PNG};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let svg = fs::read_to_string(root.join("public/icon.svg")).unwrap();
    assert!(svg.contains("<circle cx=\"256\" cy=\"256\" r=\"156\""));
    assert!(svg.contains("<circle cx=\"256\" cy=\"256\" r=\"64\""));
    assert!(
        !svg.contains("M128 358"),
        "old two-arch mark must not ship in the SVG"
    );
    assert!(root.join("public/icon-master.png").exists());

    let script = fs::read_to_string(root.join("scripts/generate-icon.py")).unwrap();
    assert!(script.contains("icon-master.png"));
    assert!(script.contains("missing public/icon-master.png"));
    assert!(
        script.contains("<circle cx=\"256\" cy=\"256\" r=\"156\""),
        "generator must paint the portal ring"
    );
    assert!(
        !script.contains("M128 358"),
        "generator must not still draw the two-arch mark"
    );

    let (w, h, data) = png_argb_pixmap(APP_ICON_PNG).expect("decode portal");
    assert_eq!((w, h), (256, 256));
    // ARGB corner must be fully transparent (no black square behind the squircle).
    assert_eq!(&data[0..4], &[0, 0, 0, 0]);
    let mid = ((128 * 256 + 128) * 4) as usize;
    assert_eq!(data[mid], 255, "core alpha");
    assert!(data[mid + 1] > 200 && data[mid + 2] > 200 && data[mid + 3] > 200);
}

#[test]
fn landing_page_ships_the_portal_mark() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let landing = fs::read_to_string(root.join("site/src/components/Landing.astro")).unwrap();
    assert!(landing.contains("href=\"/icon.svg\""));
    assert!(landing.contains("property=\"og:image\""));
    assert!(landing.contains("content={absolute(\"/icon.png\")}"));
    assert!(landing.contains("rel=\"apple-touch-icon\""));
    assert!(landing.contains("href=\"/icon-32.png\""));
    assert!(landing.contains("class=\"side-brand-copy\""));
    assert!(
        landing.matches("src=\"/icon.svg\"").count() >= 6,
        "header, preview, features, tray, CTA and footer should show the mark"
    );

    let dist_svg = root.join("site/dist/icon.svg");
    if dist_svg.exists() {
        let svg = fs::read_to_string(dist_svg).unwrap();
        assert!(
            !svg.contains("M128 358"),
            "site/dist still ships the old two-arch mark"
        );
        assert!(svg.contains("<circle cx=\"256\" cy=\"256\" r=\"156\""));
    }
}

#[test]
fn locale_scroll_payload_roundtrips_and_expires() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = root.join("site/src/locale-nav.js");
    let status = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            &format!(
                r#"
import {{ encodeLocaleScroll, decodeLocaleScroll, LOCALE_SCROLL_TTL_MS }} from '{url}';
const now = 1_700_000_000_000;
const raw = encodeLocaleScroll(842.7, now);
if (decodeLocaleScroll(raw, now) !== 843) throw new Error('roundtrip');
if (decodeLocaleScroll(raw, now + LOCALE_SCROLL_TTL_MS + 1) !== null) throw new Error('ttl');
if (decodeLocaleScroll(null, now) !== null) throw new Error('empty');
if (decodeLocaleScroll('nope', now) !== null) throw new Error('junk');
"#,
                url = file_url(&script)
            ),
        ])
        .status()
        .expect("node");
    assert!(status.success(), "locale-nav.js contract failed");
}

#[test]
fn landing_language_switch_and_github_star_are_wired() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let landing = fs::read_to_string(root.join("site/src/components/Landing.astro")).unwrap();
    let switcher =
        fs::read_to_string(root.join("site/src/components/LanguageSwitcher.astro")).unwrap();
    let release = fs::read_to_string(root.join("site/src/data/release.ts")).unwrap();
    let en = fs::read_to_string(root.join("site/src/i18n/en.ts")).unwrap();
    let pt = fs::read_to_string(root.join("site/src/i18n/pt-BR.ts")).unwrap();
    assert!(landing.contains("ClientRouter"));
    assert!(landing.contains("astro:after-swap"));
    assert!(landing.contains("LOCALE_SCROLL_KEY"));
    assert!(landing.contains("class=\"star-link\""));
    assert!(switcher.contains("data-i18n-switch"));
    assert!(release.contains("stargazers_count"));
    assert!(en.contains("star: \"Star us\""));
    assert!(pt.contains("star: \"Estrelar\""));
    assert!(en.contains("github: \"Star on GitHub\""));
    assert!(pt.contains("github: \"Dá uma estrela no GitHub\""));
}

#[test]
fn download_picker_styles_choices_instead_of_native_options() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let downloads = fs::read_to_string(root.join("site/src/components/Downloads.astro")).unwrap();
    let css = fs::read_to_string(root.join("site/src/styles/global.css")).unwrap();
    assert!(!downloads.contains("<select"));
    assert!(downloads.contains("role=\"listbox\""));
    assert!(downloads.contains("role=\"option\""));
    assert!(downloads.contains("data-download-picker"));
    assert!(css.contains(".picker-menu [role=\"option\"]"));

    let script = root.join("site/src/download-picker.js");
    let status = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            &format!(
                r#"
import {{ fileNameFromUrl, downloadButtonState, moveActiveIndex }} from '{url}';
if (fileNameFromUrl('https://example.test/a/tunnel-yard_1.2.3_amd64.deb') !== 'tunnel-yard_1.2.3_amd64.deb') throw new Error('name');
if (fileNameFromUrl('') !== '') throw new Error('empty');
const idle = downloadButtonState('', {{ idle: 'Pick', ready: 'Go', hint: 'hint' }});
if (idle.enabled || idle.buttonLabel !== 'Pick' || idle.fileLabel !== 'hint') throw new Error('idle');
const ready = downloadButtonState('https://x/y/file.exe', {{ idle: 'Pick', ready: 'Go', hint: 'hint' }});
if (!ready.enabled || ready.buttonLabel !== 'Go' || ready.fileLabel !== 'file.exe') throw new Error('ready');
if (moveActiveIndex(-1, 1, 6) !== 0) throw new Error('start');
if (moveActiveIndex(5, 1, 6) !== 0) throw new Error('wrap');
if (moveActiveIndex(0, -1, 6) !== 5) throw new Error('back');
"#,
                url = file_url(&script)
            ),
        ])
        .status()
        .expect("node");
    assert!(status.success(), "download-picker.js contract failed");
}

fn file_url(path: &std::path::Path) -> String {
    let path = path.canonicalize().unwrap();
    let mut s = path.to_string_lossy().into_owned();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }
    s = s.replace('\\', "/");
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    format!("file://{s}")
}

#[test]
fn node_can_import_a_module_through_file_url() {
    let dir = tempfile_dir("file-url");
    let js = dir.join("mod.js");
    fs::write(&js, "export const n = 1;\n").unwrap();
    let url = file_url(&js);
    assert!(url.starts_with("file:///"), "{url}");
    assert!(!url.contains('\\'), "{url}");
    let status = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            &format!("import {{ n }} from '{url}'; if (n !== 1) throw new Error('n')"),
        ])
        .status()
        .expect("node");
    assert!(status.success(), "node could not import {url}");
}

fn assert_built_styles(dist: &std::path::Path) -> std::process::Output {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("site/scripts/assert-built-styles.mjs");
    Command::new("node")
        .arg(&script)
        .arg(dist)
        .output()
        .expect("node")
}

fn write_landing_pair(root: &std::path::Path, en: &str, pt: &str) {
    fs::create_dir_all(root.join("pt-br")).unwrap();
    fs::write(root.join("index.html"), en).unwrap();
    fs::write(root.join("pt-br/index.html"), pt).unwrap();
}

const LANDING_LAYOUT_CSS: &str =
    ".desktop-nav{display:flex}.site-header{position:fixed}.hero{min-height:80vh}";

#[test]
fn landing_pages_without_reachable_css_are_rejected() {
    let missing = tempfile_dir("site-css-missing");
    write_landing_pair(
        &missing,
        "<html><head></head><body><nav class=\"desktop-nav\"></nav></body></html>",
        "<html><head></head><body></body></html>",
    );
    let out = assert_built_styles(&missing);
    assert!(
        !out.status.success(),
        "unstyled HTML must fail: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let dangling = tempfile_dir("site-css-404");
    write_landing_pair(
        &dangling,
        "<html><head><link rel=\"stylesheet\" href=\"/_astro/Landing.missing.css\"></head><body></body></html>",
        "<html><head><link rel=\"stylesheet\" href=\"/_astro/Landing.missing.css\"></head><body></body></html>",
    );
    let out = assert_built_styles(&dangling);
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success(),
        "dangling stylesheet must fail: {err}"
    );
    assert!(
        err.contains("stylesheet 404"),
        "expected a 404 diagnostic, got: {err}"
    );
}

#[test]
fn landing_pages_accept_inlined_or_present_layout_css() {
    let inlined = tempfile_dir("site-css-inline");
    let page =
        format!("<html><head><style>{LANDING_LAYOUT_CSS}</style></head><body></body></html>");
    write_landing_pair(&inlined, &page, &page);
    let out = assert_built_styles(&inlined);
    assert!(
        out.status.success(),
        "inlined CSS must pass: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let linked = tempfile_dir("site-css-linked");
    fs::create_dir_all(linked.join("assets")).unwrap();
    fs::write(linked.join("assets/Landing.css"), LANDING_LAYOUT_CSS).unwrap();
    let page =
        "<html><head><link rel=\"stylesheet\" href=\"/assets/Landing.css\"></head><body></body></html>";
    write_landing_pair(&linked, page, page);
    let out = assert_built_styles(&linked);
    assert!(
        out.status.success(),
        "linked CSS on disk must pass: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn astro_build_inlines_landing_css() {
    let config = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("site/astro.config.mjs"),
    )
    .unwrap();
    assert!(
        config.contains("inlineStylesheets: \"always\""),
        "landing CSS must be inlined so a hashed stylesheet 404 cannot unstyle the page"
    );
    assert!(config.contains("assets: \"assets\""));
}

#[test]
fn built_site_dist_keeps_landing_css_reachable() {
    let site = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("site");
    let dist = site.join("dist");
    if site.join("node_modules/astro").is_dir() {
        let output = Command::new("npm")
            .args(["run", "build"])
            .current_dir(&site)
            .output()
            .expect("npm");
        assert!(
            output.status.success(),
            "astro build failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    } else if !dist.join("index.html").is_file() {
        return;
    }
    let out = assert_built_styles(&dist);
    assert!(
        out.status.success(),
        "site/dist CSS check failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let html = fs::read_to_string(dist.join("pt-br/index.html")).unwrap();
    assert!(
        html.contains("<style") && html.contains(".desktop-nav"),
        "Portuguese page must ship layout CSS in the HTML, not a hashed file that can 404"
    );
}

fn dummy_profile(id: &str) -> VpnProfile {
    VpnProfile {
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
    }
}

fn tempfile_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tunnel-yard-test-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(unix)]
fn run_packaging_script(name: &str, root: &std::path::Path) {
    run_packaging_script_with_args(name, &[], root);
}

#[cfg(unix)]
fn run_packaging_script_with_args(name: &str, args: &[&str], root: &std::path::Path) {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packaging")
        .join(name);
    let output = Command::new("bash")
        .arg(script)
        .args(args)
        .env("TUNNEL_YARD_ROOT", root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{name} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn assert_debian_installation_complete(root: &std::path::Path) {
    assert!(root.join("usr/lib/tunnel-yard/tunnel-yard").is_file());
    assert_eq!(
        fs::read_link(root.join("usr/bin/tunnel-yard")).unwrap(),
        std::path::Path::new("../lib/tunnel-yard/tunnel-yard")
    );
    for relative in [
        "usr/lib/tunnel-yard/run-vpn.sh",
        "usr/lib/tunnel-yard/stop-vpn.sh",
        "usr/share/applications/lucas.cavalheri.tunnelyard.desktop",
        "usr/share/polkit-1/actions/lucas.cavalheri.tunnelyard.policy",
        "usr/share/icons/hicolor/256x256/apps/tunnel-yard.png",
        "usr/share/icons/hicolor/64x64/apps/tunnel-yard.png",
        "usr/share/icons/hicolor/32x32/apps/tunnel-yard.png",
        "usr/share/pixmaps/tunnel-yard.png",
        "usr/share/keyrings/tunnel-yard-archive-keyring.asc",
        "etc/apt/sources.list.d/tunnel-yard.list",
    ] {
        assert!(root.join(relative).is_file(), "missing {relative}");
    }
}

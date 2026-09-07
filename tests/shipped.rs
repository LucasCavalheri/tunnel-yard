//! Behavioral spec ported from tests/*.test.ts — calls shipped library functions.

use my_vpns::app_icon::build_linux_desktop_entry;
use my_vpns::autostart::{
    build_autostart_desktop_entry, macos_launch_agent_plist, windows_autostart_command,
    windows_reg_add_args, WINDOWS_RUN_KEY, WINDOWS_RUN_VALUE,
};
use my_vpns::conf::{
    conf_path_for_id, empty_draft, is_valid_profile_id, parse_vpn_conf_content, parse_vpn_draft,
    serialize_vpn_draft, slugify_profile_id, VpnProfileDraft,
};
use my_vpns::deps::{build_install_plan, detect_package_family, parse_os_release_text};
use my_vpns::desktop::{
    should_quit_on_repeated_close, EDITOR_FIELDS, SETUP_GATE_KEYS, TRAY_MENU_KEYS, UI_SURFACES,
};
use my_vpns::i18n::{catalog_keys, translate};
use my_vpns::install_native::{
    install_windows_openconnect_with_download, sha256_hex, stage_openconnect_installer,
    verify_openconnect_installer, windows_client, windows_silent_install_script,
};
use my_vpns::native::{prevents_reconnect, validated_native_status};
use my_vpns::openconnect::{
    assert_macos_profile_safe, build_open_connect_plan, certificate_public_key_pin, conf_entries,
    resolve_server_pin,
};
use my_vpns::os_ui::{
    macos_choose_file_script, windows_balloon_script, windows_open_file_dialog_script,
};
use my_vpns::platform::{binary_candidates, config_directory, engine_for_platform};
use my_vpns::tray_menu::{build_tray_menu, tray_action_ids, TrayEntry};
use my_vpns::vpn::{
    interpret_vpn_log_line, list_vpn_profiles, native_close_decision, reconnect_delay_ms,
    should_native_reconnect, summarize_vpn_state, NativeCloseDecision, VpnProfile, VpnSession,
    VpnState, VpnStatus,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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
    assert!(list_vpn_profiles(std::path::Path::new("/tmp/my-vpns-does-not-exist-xyz")).is_empty());
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
    assert!(config_directory("darwin", Some("/Users/test"))
        .unwrap()
        .contains("Application Support"));
    assert!(config_directory("win32", Some("/home/test"))
        .unwrap()
        .contains("My VPNs"));
    assert_eq!(engine_for_platform("win32"), "openconnect");
    assert_eq!(engine_for_platform("darwin"), "openfortivpn");
    assert!(binary_candidates("openfortivpn", "darwin")
        .iter()
        .any(|p| p == "/opt/homebrew/bin/openfortivpn"));
}

#[test]
fn extra_options_retained_windows_rejects_pppd_log() {
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
    let err = build_open_connect_plan(&raw).unwrap_err();
    assert!(err.contains("pppd-log"));
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
fn openconnect_translation_no_password_on_argv() {
    let mut draft = empty_draft();
    draft.id = "work".into();
    draft.host = "vpn.example.com".into();
    draft.port = 10443;
    draft.username = "DOMAIN\\alice".into();
    draft.password = "s e c r e t $`\"".into();
    draft.trusted_cert = "a".repeat(64);
    draft.realm = "My Realm".into();
    draft.set_dns = false;
    draft.set_routes = true;
    draft.persistent = 15;
    let plan = build_open_connect_plan(&serialize_vpn_draft(&draft).unwrap()).unwrap();
    assert!(plan.args.iter().any(|a| a == "--protocol=fortinet"));
    assert!(plan.args.iter().any(|a| a == "--user=DOMAIN\\alice"));
    assert!(plan.args.iter().any(|a| a == "--usergroup=My%20Realm"));
    assert!(plan
        .args
        .iter()
        .any(|a| a == "https://vpn.example.com:10443"));
    assert!(!plan.args.join(" ").contains("s e c r e t"));
    assert_eq!(plan.password, "s e c r e t $`\"\n");
    assert!(!plan.set_dns);
    assert!(plan.set_routes);
    assert_eq!(plan.persistent, 15);
    assert_eq!(plan.trusted_certs, vec!["a".repeat(64)]);
}

#[test]
fn no_dtls_marker() {
    let raw = "host=vpn.example\n# my-vpns-no-dtls = 1\n";
    let plan = build_open_connect_plan(raw).unwrap();
    assert!(plan.no_dtls);
    assert!(plan.args.iter().any(|a| a == "--no-dtls"));
    let parsed = parse_vpn_draft(raw, "tecsul.conf").unwrap().unwrap();
    let mut edited = parsed;
    edited.host = "vpn.example".into();
    let preserved = serialize_vpn_draft(&edited).unwrap();
    assert!(preserved.contains("# my-vpns-no-dtls = 1"));
    assert!(!build_open_connect_plan("host=vpn.example")
        .unwrap()
        .args
        .iter()
        .any(|a| a == "--no-dtls"));
}

#[test]
fn legacy_tunnel_marker() {
    let raw = "host=vpn.example\n# my-vpns-legacy-tunnel = 1\n";
    assert!(build_open_connect_plan(raw).unwrap().legacy_tunnel);
    let parsed = parse_vpn_draft(raw, "tecsul.conf").unwrap().unwrap();
    let mut edited = parsed;
    edited.host = "vpn.example".into();
    assert!(serialize_vpn_draft(&edited)
        .unwrap()
        .contains("# my-vpns-legacy-tunnel = 1"));
    assert!(
        !build_open_connect_plan("host=vpn.example")
            .unwrap()
            .legacy_tunnel
    );
}

#[test]
fn multiple_pins_and_ca_default() {
    let plan = build_open_connect_plan(&format!(
        "host=vpn.example\ntrusted-cert={}\ntrusted-cert={}",
        "a".repeat(64),
        "b".repeat(64)
    ))
    .unwrap();
    assert_eq!(plan.trusted_certs.len(), 2);
    assert!(build_open_connect_plan("host=vpn.example")
        .unwrap()
        .trusted_certs
        .is_empty());
    let err = build_open_connect_plan("host=vpn.example\ntrusted-cert=abcd").unwrap_err();
    assert!(err.contains("complete SHA256"));
}

#[test]
fn rejects_ambiguous_hosts_and_options() {
    for text in [
        "host=https://vpn.example",
        "host=vpn.example\nport=abc",
        "host=vpn.example\nset-dns=maybe",
        "host=vpn.example\npppd-plugin=evil",
        "host=vpn.example\npersistent=-1",
    ] {
        assert!(build_open_connect_plan(text).is_err(), "{text}");
    }
    assert!(build_open_connect_plan("host=::1")
        .unwrap()
        .args
        .iter()
        .any(|a| a == "https://[::1]:443"));
}

#[test]
fn macos_rejects_unsafe_options() {
    let err = assert_macos_profile_safe("host=vpn.example\npppd-plugin=evil\n").unwrap_err();
    assert!(err.contains("pppd-plugin") || err.to_lowercase().contains("macos"));
}

#[test]
fn native_status_stale_and_malformed() {
    assert_eq!(
        validated_native_status(r#"{"phase":"connected","time":1000}"#, 2000)
            .unwrap()
            .phase,
        "connected"
    );
    assert_eq!(
        validated_native_status(r#"{"phase":"connected","time":1000}"#, 17000)
            .unwrap()
            .phase,
        "disconnected"
    );
    assert_eq!(
        validated_native_status(r#"{"phase":"connected","time":99999}"#, 2000)
            .unwrap()
            .phase,
        "disconnected"
    );
    assert!(validated_native_status("{", 0).is_err());
    assert!(validated_native_status(r#"{"phase":"connected"}"#, 0).is_err());
    assert_eq!(
        interpret_vpn_log_line("MYVPNS_TUNNEL_DOWN: adapter absent"),
        Some("disconnected")
    );
    assert_eq!(interpret_vpn_log_line("MYVPNS_NETWORK_READY"), None);
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
    let plan = build_open_connect_plan(&edited_raw).unwrap();
    assert_eq!(plan.health_host.as_deref(), Some("198.18.0.2"));
    assert_eq!(plan.health_port, Some(30015));
    assert!(!conf_entries(&edited_raw)
        .unwrap()
        .iter()
        .any(|(k, _)| k.starts_with("my-vpns")));
    assert!(plan.args.iter().any(|a| a == "--force-dpd=10"));
    assert!(build_open_connect_plan("host=vpn.example\n# my-vpns-health-host=198.18.0.2").is_err());
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
        "2 up · 1 handshake"
    );
    assert_eq!(
        translate(
            "pt-BR",
            "ops.deskSummary",
            &[("up", "2".into()), ("handshake", "1".into())]
        ),
        "2 ativas · 1 handshake"
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
    let entry = build_autostart_desktop_entry("\"/opt/My VPNs/my-vpns\" --hidden");
    assert!(entry.contains("[Desktop Entry]"));
    assert!(entry.contains("Type=Application"));
    assert!(entry.contains("Name=My VPNs"));
    assert!(entry.contains("Exec=\"/opt/My VPNs/my-vpns\" --hidden"));
    assert!(entry.contains("X-GNOME-Autostart-enabled=true"));
    assert!(entry.contains("Icon=my-vpns"));
    assert!(entry.contains("StartupWMClass=dev.cavallheri.myvpns"));
}

#[test]
fn linux_launcher_matches_wayland_app_id_and_quotes_paths() {
    let entry = build_linux_desktop_entry(
        std::path::Path::new("/opt/My VPNs/my-vpns%dev"),
        std::path::Path::new("/home/test/.cache/my-vpns/icon.png"),
    );
    assert!(entry.contains("Exec=\"/opt/My VPNs/my-vpns%%dev\""));
    assert!(entry.contains("Icon=/home/test/.cache/my-vpns/icon.png"));
    assert!(entry.contains("StartupNotify=true"));
    assert!(entry.contains(&format!("StartupWMClass={}", my_vpns::APP_ID)));
    assert_eq!(my_vpns::APP_ID, "dev.cavallheri.myvpns");
}

#[test]
fn certificate_leaf_then_spki_pin() {
    let cert_pem = fs::read(fixture("localhost-cert.pem")).unwrap();
    let raw = pem_to_der(&cert_pem);
    let digest = hex::encode(Sha256::digest(&raw));
    let pin = certificate_public_key_pin(&raw, &[digest.clone()]).unwrap();
    assert!(pin.starts_with("pin-sha256:"));
    assert!(!pin.contains(&digest));
    assert!(certificate_public_key_pin(&raw, &["0".repeat(64)])
        .unwrap_err()
        .contains("does not match"));
}

#[test]
fn tls_probe_sends_no_http_and_refuses_wrong_pin() {
    let cert_pem = fs::read(fixture("localhost-cert.pem")).unwrap();
    let key_pem = fs::read(fixture("localhost-key.pem")).unwrap();
    let raw = pem_to_der(&cert_pem);
    let digest = hex::encode(Sha256::digest(&raw));

    let _ = rustls::crypto::ring::default_provider().install_default();
    let certs = rustls_pemfile::certs(&mut cert_pem.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = rustls_pemfile::private_key(&mut key_pem.as_slice())
        .unwrap()
        .unwrap();
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let requests_c = requests.clone();
    thread::spawn(move || {
        for _ in 0..8 {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut conn = rustls::ServerConnection::new(Arc::new(config.clone())).unwrap();
                let _ = conn.complete_io(&mut stream);
                // Do not serve HTTP.
                thread::sleep(Duration::from_millis(50));
                let _ = requests_c;
            }
        }
    });
    thread::sleep(Duration::from_millis(50));
    let plan = build_open_connect_plan(&format!(
        "host=127.0.0.1\nport={port}\npassword=secret\ntrusted-cert={digest}"
    ))
    .unwrap();
    let pin = resolve_server_pin(&plan).unwrap().unwrap();
    assert_eq!(pin, certificate_public_key_pin(&raw, &[digest]).unwrap());
    let mut bad = plan.clone();
    bad.trusted_certs = vec!["0".repeat(64)];
    assert!(resolve_server_pin(&bad)
        .unwrap_err()
        .contains("does not match"));
    assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 0);
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
    let report = my_vpns::smoke::smoke_report();
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
fn windows_openconnect_bootstrap_verifies_sha256_and_uac_silent_install() {
    let client = windows_client();
    assert_eq!(client.version, "9.21");
    assert!(client.url.contains("openconnect"));
    assert_eq!(client.sha256.len(), 64);
    assert!(my_vpns::arch::native_windows_client_supported("x64"));
    assert!(!my_vpns::arch::native_windows_client_supported("arm64"));
    assert!(my_vpns::install_native::windows_client_for_arch("arm64")
        .unwrap_err()
        .contains("x64-only"));

    let bytes = b"openconnect-installer-body";
    let sha = sha256_hex(bytes);
    assert!(verify_openconnect_installer(bytes, &sha).is_ok());
    assert!(verify_openconnect_installer(bytes, &"0".repeat(64))
        .unwrap_err()
        .contains("checksum mismatch"));

    let dir = tempfile_dir("oc-stage");
    let dest = dir.join("openconnect-installer.exe");
    stage_openconnect_installer(bytes, &dest, &sha).unwrap();
    assert_eq!(fs::read(&dest).unwrap(), bytes);
    fs::remove_dir_all(&dir).ok();

    let script = windows_silent_install_script(r"C:\Temp\openconnect-installer.exe");
    assert!(script.contains("/S"));
    assert!(script.contains("-Verb RunAs"));
    assert!(script.contains("Start-Process"));
    assert!(!script.contains("Windows OpenConnect bootstrap requires a Windows host"));

    let mut logs = Vec::new();
    let mut ran_path = None;
    let custom = my_vpns::install_native::WindowsClient {
        version: "9.21".into(),
        url: "https://example.test/openconnect-installer.exe".into(),
        sha256: sha.clone(),
    };
    let (code, _) = install_windows_openconnect_with_download(
        &custom,
        |url| {
            assert_eq!(url, "https://example.test/openconnect-installer.exe");
            Ok(bytes.to_vec())
        },
        &mut |line| logs.push(line.to_string()),
        |path| {
            ran_path = Some(path.to_path_buf());
            assert_eq!(sha256_hex(&fs::read(path).unwrap()), sha);
            Ok((0, String::new()))
        },
    );
    assert_eq!(code, 0);
    assert!(ran_path.is_some());
    assert!(logs
        .iter()
        .any(|l| l.contains("Downloading OpenConnect 9.21")));
    assert!(logs.iter().any(|l| l.contains("Checksum verified")));

    let mut logs2 = Vec::new();
    let mut ran = false;
    let bad = my_vpns::install_native::WindowsClient {
        version: "9.21".into(),
        url: "https://example.test/oc.exe".into(),
        sha256: "0".repeat(64),
    };
    let (code, output) = install_windows_openconnect_with_download(
        &bad,
        |_| Ok(bytes.to_vec()),
        &mut |line| logs2.push(line.to_string()),
        |_| {
            ran = true;
            Ok((0, String::new()))
        },
    );
    assert_eq!(code, 1);
    assert!(!ran);
    assert!(output.contains("checksum mismatch"));
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
            TrayEntry::Profile { label, profile_id } => Some((profile_id.as_str(), label.as_str())),
            _ => None,
        })
        .collect();
    assert!(labels
        .iter()
        .any(|(id, l)| *id == "work" && l.starts_with("on ")));
    assert!(labels
        .iter()
        .any(|(id, l)| *id == "home" && l.starts_with("off ")));
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
fn autostart_macos_plist_and_windows_run_key() {
    let plist = macos_launch_agent_plist("/Applications/My VPNs.app/Contents/MacOS/my-vpns");
    assert!(plist.contains("dev.cavallheri.myvpns"));
    assert!(plist.contains("--hidden"));
    assert!(plist.contains("/Applications/My VPNs.app/Contents/MacOS/my-vpns"));
    assert!(plist.contains("RunAtLoad"));
    let cmd = windows_autostart_command(std::path::Path::new(
        r"C:\Program Files\My VPNs\my-vpns.exe",
    ));
    assert!(cmd.contains("--hidden"));
    assert!(cmd.contains(r"C:\Program Files\My VPNs\my-vpns.exe"));
    let add = windows_reg_add_args(&cmd);
    assert!(add.contains(&WINDOWS_RUN_KEY.to_string()));
    assert!(add.contains(&WINDOWS_RUN_VALUE.to_string()));
    assert!(add.contains(&cmd));
}

#[test]
fn notifications_and_import_picker_are_platform_specific() {
    use my_vpns::os_ui::notification_argv_with_icon;
    let png = std::path::Path::new("/tmp/my-vpns-icon.png");
    let ico = std::path::Path::new("/tmp/my-vpns-icon.ico");
    let (cmd, args) =
        notification_argv_with_icon("linux", "VPN connected", "Tunnel work is up.", png, ico);
    assert_eq!(cmd, "notify-send");
    assert!(args.iter().any(|a| a == "VPN connected"));
    assert!(args.iter().any(|a| a == "-i"));
    assert!(args.iter().any(|a| a.ends_with("my-vpns-icon.png")));
    let (cmd, args) =
        notification_argv_with_icon("macos", "VPN connected", "Tunnel work is up.", png, ico);
    assert_eq!(cmd, "osascript");
    let script = args.last().cloned().unwrap_or_default();
    assert!(script.contains("display notification"));
    assert!(script.contains("VPN connected"));
    assert!(script.contains("my-vpns-icon.png"));
    let (cmd, args) =
        notification_argv_with_icon("windows", "VPN connected", "Tunnel work is up.", png, ico);
    assert!(cmd.to_lowercase().contains("powershell"));
    assert!(args.iter().any(|a| a == "-EncodedCommand"));
    let balloon = windows_balloon_script(
        "VPN connected",
        "Tunnel work is up.",
        "/tmp/my-vpns-icon.ico",
    );
    assert!(balloon.contains("NotifyIcon"));
    assert!(balloon.contains("ShowBalloonTip"));
    assert!(balloon.contains("my-vpns-icon.ico"));
    assert!(balloon.contains("System.Drawing.Icon"));
    assert!(macos_choose_file_script().contains("choose file"));
    assert!(macos_choose_file_script().contains(".conf"));
    assert!(windows_open_file_dialog_script().contains("OpenFileDialog"));
    assert!(windows_open_file_dialog_script().contains("*.conf"));
}

#[test]
fn github_update_check_compares_tags_and_parses_release_json() {
    use my_vpns::updates::{
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
        "html_url": "https://github.com/LucasCavalheri/my-vpns/releases/tag/v1.1.8",
        "draft": false,
        "assets": [
            {"name": "my-vpns-linux-x64", "browser_download_url": "https://github.com/LucasCavalheri/my-vpns/releases/download/v1.1.8/my-vpns-linux-x64"}
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
        my_vpns::platform::current_platform() == "linux" && my_vpns::arch::current_arch() == "x64"
    );
    assert_eq!(artifact_kind("my-vpns-macos-x64"), Some("macos"));
    assert_eq!(artifact_architecture("my-vpns-macos-x64"), Some("x64"));
    assert_eq!(artifact_kind("my-vpns-windows-arm64.exe"), Some("windows"));
    assert_eq!(
        artifact_platform("my-vpns-windows-arm64.exe"),
        Some("windows")
    );

    assert!(check_for_app_update("1.1.8", |_| Ok(body.to_string()))
        .unwrap()
        .is_none());
}

#[test]
fn helpers_exist_in_tree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in [
        "packaging/run-vpn.sh",
        "packaging/stop-vpn.sh",
        "packaging/macos-vpn.sh",
        "packaging/windows-vpn.ps1",
        "packaging/windows-network.ps1",
        "packaging/vpnc-script-win.js",
        "packaging/windows-client.json",
        "packaging/polkit/dev.cavallheri.myvpns.policy",
        "packaging/my-vpns.desktop",
        "public/icon.png",
        "public/icon-32.png",
        "public/icon.ico",
        "public/icon.svg",
        "scripts/generate-icon.py",
    ] {
        assert!(root.join(name).exists(), "{name}");
    }
    let desktop = fs::read_to_string(root.join("packaging/my-vpns.desktop")).unwrap();
    assert!(desktop.contains("StartupWMClass=dev.cavallheri.myvpns"));
    assert!(desktop.contains("Exec=my-vpns"));
    let postinst = fs::read_to_string(root.join("packaging/after-install.sh")).unwrap();
    assert!(!postinst.contains("resources/"));
    assert!(postinst.contains("StartupWMClass=dev.cavallheri.myvpns"));
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
}

#[test]
fn app_icon_is_a_real_png_and_argb_pixmap() {
    use my_vpns::app_icon::{png_argb_pixmap, rgba_to_argb, APP_ICON_ICO, APP_ICON_PNG};
    assert!(APP_ICON_PNG.starts_with(b"\x89PNG"));
    assert!(APP_ICON_ICO.len() > 16);
    let (w, h, data) = png_argb_pixmap(APP_ICON_PNG).expect("decode mark");
    assert!(w >= 32 && h >= 32);
    assert_eq!(data.len(), (w * h * 4) as usize);
    assert_eq!(
        rgba_to_argb(&[0x11, 0x22, 0x33, 0x44]),
        vec![0x44, 0x11, 0x22, 0x33]
    );
}

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn pem_to_der(pem: &[u8]) -> Vec<u8> {
    let s = std::str::from_utf8(pem).unwrap();
    let body: String = s.lines().filter(|l| !l.starts_with("-----")).collect();
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, body.trim()).unwrap()
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
    }
}

fn tempfile_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("my-vpns-test-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

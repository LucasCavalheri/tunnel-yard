//! Linux-only distro detection, helpers, pickers and conf edge cases.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use tunnel_yard::conf::{
    conf_entries, conf_path_for_id, empty_draft, parse_vpn_draft, serialize_vpn_draft,
};
use tunnel_yard::deps::{
    build_install_plan, detect_package_family, distro_from_os_release, parse_client_version_text,
    parse_os_release_text,
};
use tunnel_yard::os_ui::{path_from_picker_output, FILE_PICKERS};
use tunnel_yard::platform::{
    binary_candidates, engine_for_platform, helper_path, linux_helpers, normalize_platform,
};
use tunnel_yard::settings::settings_path;
use tunnel_yard::updates::{artifact_is_compatible, artifact_kind, artifact_platform};

fn none(_: &str) -> bool {
    false
}

fn exists_map(paths: &'static [&'static str]) -> impl Fn(&str) -> bool {
    move |p| paths.iter().any(|known| *known == p)
}

#[test]
fn every_major_distro_family_is_recognized() {
    let cases: &[(&str, &[&str], &str)] = &[
        ("debian", &[], "apt"),
        ("ubuntu", &["debian"], "apt"),
        ("linuxmint", &["ubuntu", "debian"], "apt"),
        ("pop", &["ubuntu", "debian"], "apt"),
        ("kali", &["debian"], "apt"),
        ("neon", &["ubuntu", "debian"], "apt"),
        ("devuan", &["debian"], "apt"),
        ("pureos", &["debian"], "apt"),
        ("raspbian", &["debian"], "apt"),
        ("deepin", &["debian"], "apt"),
        ("fedora", &[], "dnf"),
        ("rhel", &["fedora"], "dnf"),
        ("rocky", &["rhel", "fedora"], "dnf"),
        ("almalinux", &["rhel", "fedora"], "dnf"),
        ("amzn", &["fedora"], "dnf"),
        ("nobara", &["fedora"], "dnf"),
        ("mageia", &[], "dnf"),
        ("opensuse-tumbleweed", &["suse"], "zypper"),
        ("opensuse-leap", &["suse"], "zypper"),
        ("sles", &["suse"], "zypper"),
        ("arch", &[], "pacman"),
        ("manjaro", &["arch"], "pacman"),
        ("endeavouros", &["arch"], "pacman"),
        ("cachyos", &["arch"], "pacman"),
        ("garuda", &["arch"], "pacman"),
        ("alpine", &[], "apk"),
        ("postmarketos", &["alpine"], "apk"),
        ("chimera", &[], "apk"),
        ("void", &[], "xbps"),
        ("gentoo", &[], "emerge"),
        ("funtoo", &["gentoo"], "emerge"),
        ("solus", &[], "eopkg"),
        ("nixos", &[], "nix"),
        ("guix", &[], "guix"),
        ("slackware", &[], "slackpkg"),
    ];
    for (id, like, family) in cases {
        let like: Vec<String> = like.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(detect_package_family(id, &like, none), *family, "id={id}");
    }
}

#[test]
fn ostree_images_do_not_pretend_dnf_can_mutate_the_host() {
    assert_eq!(
        detect_package_family("fedora", &[], exists_map(&["/run/ostree-booted"])),
        "rpm-ostree"
    );
    let plan = build_install_plan("rpm-ostree");
    assert!(!plan.can_auto_install);
    assert!(plan
        .install_command
        .unwrap()
        .contains("rpm-ostree install openfortivpn"));
    assert!(plan.pkexec_args.is_none());
}

#[test]
fn unknown_id_falls_back_to_the_package_manager_on_disk() {
    assert_eq!(
        detect_package_family("weirdos", &[], exists_map(&["/usr/bin/apk"])),
        "apk"
    );
    assert_eq!(
        detect_package_family("weirdos", &[], exists_map(&["/usr/bin/xbps-install"])),
        "xbps"
    );
    assert_eq!(
        detect_package_family("weirdos", &[], exists_map(&["/usr/bin/emerge"])),
        "emerge"
    );
    assert_eq!(
        detect_package_family("weirdos", &[], exists_map(&["/usr/bin/eopkg"])),
        "eopkg"
    );
    assert_eq!(
        detect_package_family("weirdos", &[], exists_map(&["/usr/bin/apt-get"])),
        "apt"
    );
}

#[test]
fn auto_install_plans_name_openfortivpn() {
    for family in [
        "apt", "dnf", "yum", "zypper", "pacman", "apk", "xbps", "emerge", "eopkg",
    ] {
        let plan = build_install_plan(family);
        assert!(plan.can_auto_install, "{family}");
        let cmd = plan.install_command.unwrap();
        assert!(cmd.contains("openfortivpn"), "{family}: {cmd}");
        let args = plan.pkexec_args.unwrap();
        assert!(args.iter().any(|a| a.contains("openfortivpn")), "{family}");
    }
    for family in ["nix", "guix", "slackpkg"] {
        let plan = build_install_plan(family);
        assert!(!plan.can_auto_install, "{family}");
        assert!(plan.install_command.is_some(), "{family}");
        assert!(plan.pkexec_args.is_none(), "{family}");
    }
}

#[test]
fn os_release_parser_skips_comments_and_strips_quotes() {
    let map = parse_os_release_text(
        r#"
# ignore
ID='alpine'
NAME="Alpine Linux"
VERSION_ID=3.20
BROKEN
PRETTY_NAME="Alpine Linux v3.20"
"#,
    );
    assert_eq!(map.get("ID").unwrap(), "alpine");
    assert_eq!(map.get("NAME").unwrap(), "Alpine Linux");
    let distro = distro_from_os_release(
        r#"ID=alpine
PRETTY_NAME="Alpine Linux v3.20"
"#,
        none,
    );
    assert_eq!(distro.family, "apk");
    assert_eq!(distro.pretty, "Alpine Linux v3.20");
}

#[test]
fn empty_os_release_is_still_a_linux_unknown() {
    let distro = distro_from_os_release("", none);
    assert_eq!(distro.id, "linux");
    assert_eq!(distro.family, "unknown");
}

#[test]
fn client_version_parser_accepts_openfortivpn_banners() {
    assert_eq!(
        parse_client_version_text("openfortivpn 1.23.1"),
        Some("openfortivpn 1.23.1".into())
    );
    assert_eq!(parse_client_version_text("1.22.0"), Some("1.22.0".into()));
    assert_eq!(parse_client_version_text("not a banner line"), None);
}

#[test]
fn file_pickers_cover_gnome_kde_and_light_desktops() {
    let commands: Vec<_> = FILE_PICKERS.iter().map(|p| p.command).collect();
    assert_eq!(commands, ["zenity", "qarma", "yad", "kdialog"]);
    assert_eq!(
        path_from_picker_output(true, " /tmp/work.conf \n"),
        Some(std::path::PathBuf::from("/tmp/work.conf"))
    );
    assert_eq!(path_from_picker_output(true, "   \n"), None);
    assert_eq!(path_from_picker_output(false, "/tmp/work.conf"), None);
}

#[test]
fn linux_looks_for_openfortivpn_in_user_and_opt_paths() {
    let cands = binary_candidates("openfortivpn", "linux");
    for expected in [
        "/usr/bin/openfortivpn",
        "/usr/local/sbin/openfortivpn",
        "/opt/bin/openfortivpn",
    ] {
        assert!(cands.iter().any(|p| p == expected), "missing {expected}");
    }
    assert_eq!(engine_for_platform("anything"), "openfortivpn");
    assert_eq!(normalize_platform("gnu/linux"), "linux");
}

#[test]
fn helpers_resolve_from_the_source_tree() {
    let run = helper_path("run-vpn.sh").unwrap();
    assert!(run.ends_with("packaging/run-vpn.sh"));
    let (run, stop) = linux_helpers().expect("packaging helpers");
    assert!(run.ends_with("run-vpn.sh"));
    assert!(stop.ends_with("stop-vpn.sh"));
}

#[test]
fn settings_live_under_xdg_config() {
    let path = settings_path();
    assert!(path.ends_with("tunnel-yard/settings.json"));
}

#[test]
fn updater_ignores_foreign_os_artifacts() {
    assert_eq!(
        artifact_kind("tunnel-yard-3.0.1-1-x86_64.pkg.tar.zst"),
        Some("arch")
    );
    assert_eq!(
        artifact_kind("tunnel-yard-3.0.1-r0-aarch64.apk"),
        Some("apk")
    );
    assert_eq!(
        artifact_platform("tunnel-yard-3.0.1-1-x86_64.pkg.tar.zst"),
        Some("linux")
    );
    assert_eq!(artifact_kind("tunnel-yard-macos.dmg"), None);
    assert_eq!(artifact_kind("tunnel-yard-windows-x64.exe"), None);
    assert_eq!(
        artifact_platform("tunnel-yard_3.0.0_amd64.deb"),
        Some("linux")
    );
    assert!(!artifact_is_compatible("tunnel-yard-macos.dmg"));
    assert!(!artifact_is_compatible("tunnel-yard-windows-x64.exe"));
}

#[test]
fn conf_rejects_binary_junk_and_keeps_comments() {
    assert!(conf_path_for_id("..").is_err());
    assert!(conf_entries("host vpn.example").is_err());
    let raw = "host=vpn.example\n# tunnel-yard-no-dtls = 1\npppd-log=/tmp/x\n";
    let draft = parse_vpn_draft(raw, "lab.conf").unwrap().unwrap();
    assert!(draft.no_dtls);
    assert!(draft
        .extra_options
        .iter()
        .any(|(k, v)| k == "pppd-log" && v == "/tmp/x"));
    let out = serialize_vpn_draft(&draft).unwrap();
    assert!(out.contains("# tunnel-yard-no-dtls = 1"));
    let mut bad = empty_draft();
    bad.host = "vpn.example".into();
    bad.username = "a\0b".into();
    assert!(serialize_vpn_draft(&bad).is_err());
}

#[test]
fn smoke_report_says_linux() {
    let report = tunnel_yard::smoke::smoke_report();
    assert_eq!(report.platform, "linux");
    assert_eq!(report.engine, "openfortivpn");
    assert!(report.ok);
}

#[test]
fn distro_matrix_os_release_snippets() {
    let snippets: HashMap<&str, &str> = HashMap::from([
        ("kali", "ID=kali\nID_LIKE=debian\n"),
        ("void", "ID=void\n"),
        ("alpine", "ID=alpine\n"),
        ("gentoo", "ID=gentoo\n"),
        ("solus", "ID=solus\n"),
        ("nixos", "ID=nixos\n"),
        ("cachyos", "ID=cachyos\nID_LIKE=arch\n"),
        ("amzn", "ID=amzn\nID_LIKE=\"fedora\"\n"),
    ]);
    let expect = HashMap::from([
        ("kali", "apt"),
        ("void", "xbps"),
        ("alpine", "apk"),
        ("gentoo", "emerge"),
        ("solus", "eopkg"),
        ("nixos", "nix"),
        ("cachyos", "pacman"),
        ("amzn", "dnf"),
    ]);
    for (id, raw) in snippets {
        assert_eq!(distro_from_os_release(raw, none).family, expect[id], "{id}");
    }
}

#[test]
fn pacman_and_apk_packages_carry_the_desktop_tree() {
    let root = std::env::temp_dir().join(format!("tunnel-yard-pacman-apk-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("usr/bin")).unwrap();
    fs::create_dir_all(root.join("usr/lib/tunnel-yard")).unwrap();
    fs::create_dir_all(root.join("usr/share/applications")).unwrap();
    let bin = root.join("usr/bin/tunnel-yard");
    fs::write(&bin, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join("usr/lib/tunnel-yard/run-vpn.sh"), "#!/bin/sh\n").unwrap();
    fs::write(
        root.join("usr/share/applications/lucas.cavalheri.tunnelyard.desktop"),
        "[Desktop Entry]\nName=TunnelYard\n",
    )
    .unwrap();

    let cwd =
        std::env::temp_dir().join(format!("tunnel-yard-pacman-apk-cwd-{}", std::process::id()));
    let _ = fs::remove_dir_all(&cwd);
    fs::create_dir_all(cwd.join("dist-release")).unwrap();

    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging/build-pacman-apk.sh");
    let status = Command::new("bash")
        .arg(&script)
        .current_dir(&cwd)
        .args([
            root.as_os_str(),
            std::ffi::OsStr::new("3.0.1"),
            std::ffi::OsStr::new("x86_64"),
            std::ffi::OsStr::new("dist-release"),
        ])
        .status()
        .expect("bash");
    assert!(
        status.success(),
        "build-pacman-apk.sh failed with a relative output dir"
    );

    let out = cwd.join("dist-release");
    let pkg = out.join("tunnel-yard-3.0.1-1-x86_64.pkg.tar.zst");
    let apk = out.join("tunnel-yard-3.0.1-r0-x86_64.apk");
    assert!(pkg.is_file(), "{}", pkg.display());
    assert!(apk.is_file(), "{}", apk.display());

    let pkg_list = Command::new("tar")
        .args(["-tf", pkg.to_str().unwrap()])
        .output()
        .expect("tar");
    let pkg_text = String::from_utf8_lossy(&pkg_list.stdout);
    assert!(pkg_text.contains(".PKGINFO"), "{pkg_text}");
    assert!(pkg_text.contains("usr/bin/tunnel-yard"), "{pkg_text}");
    assert!(
        pkg_text.contains("usr/share/applications/lucas.cavalheri.tunnelyard.desktop"),
        "{pkg_text}"
    );

    let info = Command::new("tar")
        .args(["-xOf", pkg.to_str().unwrap(), ".PKGINFO"])
        .output()
        .expect("tar");
    let info_text = String::from_utf8_lossy(&info.stdout);
    assert!(info_text.contains("pkgname = tunnel-yard"), "{info_text}");
    assert!(info_text.contains("arch = x86_64"), "{info_text}");
    assert!(info_text.contains("depend = polkit"), "{info_text}");

    let apk_info = Command::new("tar")
        .args(["-xOf", apk.to_str().unwrap(), ".PKGINFO"])
        .output()
        .expect("tar");
    let apk_text = String::from_utf8_lossy(&apk_info.stdout);
    assert!(apk_text.contains("pkgver = 3.0.1-r0"), "{apk_text}");
    assert!(apk_text.contains("depend = gcompat"), "{apk_text}");

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&cwd);
}

fn install_sh() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("public/install.sh")
}

fn install_plan(arch: &str, pm: &str, version: &str) -> String {
    let output = Command::new("bash")
        .arg(install_sh())
        .args(["--print-plan", "--version", version])
        .env("TUNNEL_YARD_ARCH", arch)
        .env("TUNNEL_YARD_PM", pm)
        .env("TUNNEL_YARD_VERSION", version)
        .output()
        .expect("install.sh");
    assert!(
        output.status.success(),
        "install.sh --print-plan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn install_sh_plans_native_packages_per_distro() {
    let apt = install_plan("x86_64", "apt", "3.0.1");
    assert!(apt.contains("family=apt"), "{apt}");
    assert!(apt.contains("tunnel-yard_3.0.1_amd64.deb"), "{apt}");
    assert!(apt.contains("/download/v3.0.1/"), "{apt}");

    let arm_apt = install_plan("aarch64", "apt", "3.0.1");
    assert!(arm_apt.contains("tunnel-yard_3.0.1_arm64.deb"), "{arm_apt}");

    let pacman = install_plan("x86_64", "pacman", "3.0.1");
    assert!(pacman.contains("family=pacman"), "{pacman}");
    assert!(
        pacman.contains("tunnel-yard-3.0.1-1-x86_64.pkg.tar.zst"),
        "{pacman}"
    );

    let apk = install_plan("aarch64", "apk", "3.0.1");
    assert!(apk.contains("tunnel-yard-3.0.1-r0-aarch64.apk"), "{apk}");

    let rpm = install_plan("x86_64", "dnf", "3.0.1");
    assert!(rpm.contains("tunnel-yard-3.0.1-1.x86_64.rpm"), "{rpm}");

    let tar = install_plan("x86_64", "tar", "3.0.1");
    assert!(tar.contains("tunnel-yard-linux-x64.tar.gz"), "{tar}");
}

#[test]
fn install_sh_makes_the_package_readable_for_apt() {
    let script = fs::read_to_string(install_sh()).unwrap();
    assert!(
        script.contains("chmod 0755 \"$TMP\""),
        "apt _apt user needs the temp dir to be 0755"
    );
    assert!(
        script.contains("chmod 0644 \"$FILE\""),
        "apt _apt user needs the .deb to be 0644"
    );

    let dir = std::env::temp_dir().join(format!(
        "tunnel-yard-apt-perm-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    let file = dir.join("tunnel-yard.deb");
    fs::write(&file, b"deb").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();

    let status = Command::new("chmod")
        .args(["0755"])
        .arg(&dir)
        .status()
        .expect("chmod dir");
    assert!(status.success());
    let status = Command::new("chmod")
        .args(["0644"])
        .arg(&file)
        .status()
        .expect("chmod file");
    assert!(status.success());
    assert_eq!(
        fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o644
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn install_sh_rejects_foreign_chips() {
    let output = Command::new("bash")
        .arg(install_sh())
        .args(["--print-plan", "--version", "3.0.1"])
        .env("TUNNEL_YARD_ARCH", "riscv64")
        .env("TUNNEL_YARD_PM", "apt")
        .output()
        .expect("install.sh");
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(err.contains("x86_64"), "{err}");
}

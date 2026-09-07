mod icons;
mod theme;
mod ui;

use my_vpns::smoke;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "My VPNs {version}\n\
             Unprivileged FortiGate SSL VPN manager.\n\n\
             Usage:\n\
               my-vpns                     Start the desktop UI\n\
               my-vpns --hidden            Start hidden (tray / login item)\n\
               my-vpns --autostart         Same as --hidden\n\
               my-vpns --smoke             Print engine/settings/profiles JSON and exit\n\
               my-vpns --screenshot PATH   Capture the first UI frame to PATH and exit\n\
               my-vpns --version           Print version and exit\n",
            version = my_vpns::APP_VERSION
        );
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("my-vpns {}", my_vpns::APP_VERSION);
        return;
    }
    if args.iter().any(|a| a == "--smoke") {
        smoke::run();
        return;
    }
    let hidden = args.iter().any(|a| a == "--hidden" || a == "--autostart");
    let screenshot = args
        .windows(2)
        .find(|w| w[0] == "--screenshot")
        .map(|w| w[1].clone());
    if let Err(err) = ui::run(hidden, screenshot) {
        eprintln!("[my-vpns] failed to start UI: {err}");
        std::process::exit(1);
    }
}

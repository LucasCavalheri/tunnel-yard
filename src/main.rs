mod ui;

use tunnel_yard::smoke;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "{name} {version}\n\
             Unprivileged FortiGate SSL VPN manager.\n\n\
             Usage:\n\
               {bin}                     Start the desktop UI\n\
               {bin} --hidden            Start hidden (tray / login item)\n\
               {bin} --autostart         Same as --hidden\n\
               {bin} --smoke             Print engine/settings/profiles JSON and exit\n\
               {bin} --version           Print version and exit\n",
            name = tunnel_yard::APP_NAME,
            bin = tunnel_yard::APP_BIN,
            version = tunnel_yard::APP_VERSION
        );
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{} {}", tunnel_yard::APP_BIN, tunnel_yard::APP_VERSION);
        return;
    }
    if args.iter().any(|a| a == "--smoke") {
        smoke::run();
        return;
    }
    let hidden = args.iter().any(|a| a == "--hidden" || a == "--autostart");
    if let Err(err) = ui::run(hidden) {
        eprintln!("[{}] failed to start UI: {err}", tunnel_yard::APP_BIN);
        std::process::exit(1);
    }
}

//! Hugeicons (stroke rounded) shipped as SVG bytes for the GPUI desk.
//!
//! Paths are `icons/huge/<name>.svg` so they sit beside GPUI Kit's Lucide
//! bundle without colliding. The binary's AssetSource serves these first.

pub struct IconFile {
    pub path: &'static str,
    pub bytes: &'static [u8],
}

pub const FILES: &[IconFile] = &[
    file(
        "icons/huge/add-01.svg",
        include_bytes!("../assets/icons/add-01.svg"),
    ),
    file(
        "icons/huge/alert-02.svg",
        include_bytes!("../assets/icons/alert-02.svg"),
    ),
    file(
        "icons/huge/arrow-down-01.svg",
        include_bytes!("../assets/icons/arrow-down-01.svg"),
    ),
    file(
        "icons/huge/arrow-up-01.svg",
        include_bytes!("../assets/icons/arrow-up-01.svg"),
    ),
    file(
        "icons/huge/cancel-01.svg",
        include_bytes!("../assets/icons/cancel-01.svg"),
    ),
    file(
        "icons/huge/checkmark-circle-02.svg",
        include_bytes!("../assets/icons/checkmark-circle-02.svg"),
    ),
    file(
        "icons/huge/computer.svg",
        include_bytes!("../assets/icons/computer.svg"),
    ),
    file(
        "icons/huge/connect.svg",
        include_bytes!("../assets/icons/connect.svg"),
    ),
    file(
        "icons/huge/console.svg",
        include_bytes!("../assets/icons/console.svg"),
    ),
    file(
        "icons/huge/delete-02.svg",
        include_bytes!("../assets/icons/delete-02.svg"),
    ),
    file(
        "icons/huge/download-01.svg",
        include_bytes!("../assets/icons/download-01.svg"),
    ),
    file(
        "icons/huge/earth.svg",
        include_bytes!("../assets/icons/earth.svg"),
    ),
    file(
        "icons/huge/file-02.svg",
        include_bytes!("../assets/icons/file-02.svg"),
    ),
    file(
        "icons/huge/file-import.svg",
        include_bytes!("../assets/icons/file-import.svg"),
    ),
    file(
        "icons/huge/folder-01.svg",
        include_bytes!("../assets/icons/folder-01.svg"),
    ),
    file(
        "icons/huge/globe-02.svg",
        include_bytes!("../assets/icons/globe-02.svg"),
    ),
    file(
        "icons/huge/information-circle.svg",
        include_bytes!("../assets/icons/information-circle.svg"),
    ),
    file(
        "icons/huge/language-circle.svg",
        include_bytes!("../assets/icons/language-circle.svg"),
    ),
    file(
        "icons/huge/laptop.svg",
        include_bytes!("../assets/icons/laptop.svg"),
    ),
    file(
        "icons/huge/link-square-02.svg",
        include_bytes!("../assets/icons/link-square-02.svg"),
    ),
    file(
        "icons/huge/logout-01.svg",
        include_bytes!("../assets/icons/logout-01.svg"),
    ),
    file(
        "icons/huge/logout-03.svg",
        include_bytes!("../assets/icons/logout-03.svg"),
    ),
    file(
        "icons/huge/moon-02.svg",
        include_bytes!("../assets/icons/moon-02.svg"),
    ),
    file(
        "icons/huge/more-horizontal.svg",
        include_bytes!("../assets/icons/more-horizontal.svg"),
    ),
    file(
        "icons/huge/paint-board.svg",
        include_bytes!("../assets/icons/paint-board.svg"),
    ),
    file(
        "icons/huge/pencil-edit-01.svg",
        include_bytes!("../assets/icons/pencil-edit-01.svg"),
    ),
    file(
        "icons/huge/reload.svg",
        include_bytes!("../assets/icons/reload.svg"),
    ),
    file(
        "icons/huge/rotate-clockwise.svg",
        include_bytes!("../assets/icons/rotate-clockwise.svg"),
    ),
    file(
        "icons/huge/search-01.svg",
        include_bytes!("../assets/icons/search-01.svg"),
    ),
    file(
        "icons/huge/security-check.svg",
        include_bytes!("../assets/icons/security-check.svg"),
    ),
    file(
        "icons/huge/server-stack-01.svg",
        include_bytes!("../assets/icons/server-stack-01.svg"),
    ),
    file(
        "icons/huge/settings-02.svg",
        include_bytes!("../assets/icons/settings-02.svg"),
    ),
    file(
        "icons/huge/shield-01.svg",
        include_bytes!("../assets/icons/shield-01.svg"),
    ),
    file(
        "icons/huge/source-code.svg",
        include_bytes!("../assets/icons/source-code.svg"),
    ),
    file(
        "icons/huge/square-lock-02.svg",
        include_bytes!("../assets/icons/square-lock-02.svg"),
    ),
    file(
        "icons/huge/sun-03.svg",
        include_bytes!("../assets/icons/sun-03.svg"),
    ),
    file(
        "icons/huge/tick-02.svg",
        include_bytes!("../assets/icons/tick-02.svg"),
    ),
    file(
        "icons/huge/user.svg",
        include_bytes!("../assets/icons/user.svg"),
    ),
    file(
        "icons/huge/wifi-01.svg",
        include_bytes!("../assets/icons/wifi-01.svg"),
    ),
    file(
        "icons/huge/wifi-connected-01.svg",
        include_bytes!("../assets/icons/wifi-connected-01.svg"),
    ),
    file(
        "icons/huge/wifi-off-01.svg",
        include_bytes!("../assets/icons/wifi-off-01.svg"),
    ),
];

pub const FLAG_FILES: &[IconFile] = &[
    file(
        "icons/flags/brazil.svg",
        include_bytes!("../assets/icons/flags/brazil.svg"),
    ),
    file(
        "icons/flags/united-states.svg",
        include_bytes!("../assets/icons/flags/united-states.svg"),
    ),
];

const fn file(path: &'static str, bytes: &'static [u8]) -> IconFile {
    IconFile { path, bytes }
}

pub fn svg_bytes(path: &str) -> Option<&'static [u8]> {
    FILES
        .iter()
        .chain(FLAG_FILES.iter())
        .find(|file| file.path == path)
        .map(|file| file.bytes)
}

pub fn paths() -> impl Iterator<Item = &'static str> {
    FILES.iter().chain(FLAG_FILES.iter()).map(|file| file.path)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocaleFlag {
    Brazil,
    UnitedStates,
}

impl LocaleFlag {
    pub fn asset_path(self) -> &'static str {
        match self {
            Self::Brazil => "icons/flags/brazil.svg",
            Self::UnitedStates => "icons/flags/united-states.svg",
        }
    }
}

/// Named icons the desk actually draws. `asset_path` is what GPUI loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Huge {
    Add,
    Alert,
    ArrowDown,
    ArrowUp,
    Close,
    CheckCircle,
    Computer,
    Connect,
    Console,
    Delete,
    Download,
    Earth,
    File,
    FileImport,
    Folder,
    Globe,
    Info,
    Language,
    Laptop,
    ExternalLink,
    Hide,
    Quit,
    Moon,
    More,
    Paint,
    Edit,
    Reload,
    Rotate,
    Search,
    ShieldCheck,
    Server,
    Settings,
    Shield,
    Terminal,
    Lock,
    Sun,
    Check,
    User,
    Wifi,
    WifiOn,
    WifiOff,
}

impl Huge {
    pub fn asset_path(self) -> &'static str {
        match self {
            Huge::Add => "icons/huge/add-01.svg",
            Huge::Alert => "icons/huge/alert-02.svg",
            Huge::ArrowDown => "icons/huge/arrow-down-01.svg",
            Huge::ArrowUp => "icons/huge/arrow-up-01.svg",
            Huge::Close => "icons/huge/cancel-01.svg",
            Huge::CheckCircle => "icons/huge/checkmark-circle-02.svg",
            Huge::Computer => "icons/huge/computer.svg",
            Huge::Connect => "icons/huge/connect.svg",
            Huge::Console => "icons/huge/console.svg",
            Huge::Delete => "icons/huge/delete-02.svg",
            Huge::Download => "icons/huge/download-01.svg",
            Huge::Earth => "icons/huge/earth.svg",
            Huge::File => "icons/huge/file-02.svg",
            Huge::FileImport => "icons/huge/file-import.svg",
            Huge::Folder => "icons/huge/folder-01.svg",
            Huge::Globe => "icons/huge/globe-02.svg",
            Huge::Info => "icons/huge/information-circle.svg",
            Huge::Language => "icons/huge/language-circle.svg",
            Huge::Laptop => "icons/huge/laptop.svg",
            Huge::ExternalLink => "icons/huge/link-square-02.svg",
            Huge::Hide => "icons/huge/logout-01.svg",
            Huge::Quit => "icons/huge/logout-03.svg",
            Huge::Moon => "icons/huge/moon-02.svg",
            Huge::More => "icons/huge/more-horizontal.svg",
            Huge::Paint => "icons/huge/paint-board.svg",
            Huge::Edit => "icons/huge/pencil-edit-01.svg",
            Huge::Reload => "icons/huge/reload.svg",
            Huge::Rotate => "icons/huge/rotate-clockwise.svg",
            Huge::Search => "icons/huge/search-01.svg",
            Huge::ShieldCheck => "icons/huge/security-check.svg",
            Huge::Server => "icons/huge/server-stack-01.svg",
            Huge::Settings => "icons/huge/settings-02.svg",
            Huge::Shield => "icons/huge/shield-01.svg",
            Huge::Terminal => "icons/huge/source-code.svg",
            Huge::Lock => "icons/huge/square-lock-02.svg",
            Huge::Sun => "icons/huge/sun-03.svg",
            Huge::Check => "icons/huge/tick-02.svg",
            Huge::User => "icons/huge/user.svg",
            Huge::Wifi => "icons/huge/wifi-01.svg",
            Huge::WifiOn => "icons/huge/wifi-connected-01.svg",
            Huge::WifiOff => "icons/huge/wifi-off-01.svg",
        }
    }

    pub fn all() -> &'static [Huge] {
        &[
            Huge::Add,
            Huge::Alert,
            Huge::ArrowDown,
            Huge::ArrowUp,
            Huge::Close,
            Huge::CheckCircle,
            Huge::Computer,
            Huge::Connect,
            Huge::Console,
            Huge::Delete,
            Huge::Download,
            Huge::Earth,
            Huge::File,
            Huge::FileImport,
            Huge::Folder,
            Huge::Globe,
            Huge::Info,
            Huge::Language,
            Huge::Laptop,
            Huge::ExternalLink,
            Huge::Hide,
            Huge::Quit,
            Huge::Moon,
            Huge::More,
            Huge::Paint,
            Huge::Edit,
            Huge::Reload,
            Huge::Rotate,
            Huge::Search,
            Huge::ShieldCheck,
            Huge::Server,
            Huge::Settings,
            Huge::Shield,
            Huge::Terminal,
            Huge::Lock,
            Huge::Sun,
            Huge::Check,
            Huge::User,
            Huge::Wifi,
            Huge::WifiOn,
            Huge::WifiOff,
        ]
    }
}

/// Close / cancel buttons that live *inside* overlays. They must not be
/// rendered in the title bar — a leaked "Cancelar" there is unclickable.
pub const OVERLAY_CANCEL_IDS: &[&str] = &[
    "cancel-editor",
    "cancel-delete",
    "cancel-update",
    "cancel-quit",
    "close-editor",
    "close-preferences",
    "close-preferences-top",
];

pub const TITLE_BAR_ACTION_IDS: &[&str] = &[];

//! Target architecture names used by release artifacts and native bootstrap.

/// Stable, human-facing architecture name used in release filenames.
pub fn current_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "x86",
        "arm" => "arm",
        _ => "unknown",
    }
}

/// Name of the binary published for the current target.
pub fn current_artifact_name() -> String {
    let arch = current_arch();
    match crate::platform::current_platform() {
        "macos" => "tunnel-yard-macos".into(),
        "windows" => format!("tunnel-yard-windows-{arch}.exe"),
        _ => format!("tunnel-yard-linux-{arch}"),
    }
}

/// Whether the pinned Windows OpenConnect bootstrap can run on this target.
///
/// The GUI itself is built for both x64 and ARM64. The reviewed OpenConnect
/// 9.21 installer currently shipped by the project is a MinGW64/x64 package,
/// so an ARM64 Windows build must use a separately provided native client.
pub fn native_windows_client_supported(arch: &str) -> bool {
    arch == "x64"
}

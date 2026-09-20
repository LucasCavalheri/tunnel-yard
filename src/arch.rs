//! Target architecture names used by Linux release artifacts.

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
    format!("tunnel-yard-linux-{}", current_arch())
}

//! Per-platform locations. Only Linux and macOS are supported.

use std::path::PathBuf;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("qieyue only supports Linux and macOS");

/// Root of the app's private state (config, shelf, decrypted media).
///
/// `QIEYUE_DATA_DIR` overrides everything, which keeps tests and portable
/// installs out of the user's profile.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("QIEYUE_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/qieyue")
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("qieyue")
    }
}

/// The template written on first launch. Embedded so an installed binary does
/// not depend on the checkout layout.
const CONFIG_TEMPLATE: &str = include_str!("../../assets/config/config.json");

/// Writes `config.json` next to the other state if it is not there yet.
pub(crate) fn ensure_config(config: &std::path::Path) -> Result<(), String> {
    if config.exists() {
        return Ok(());
    }
    if let Some(parent) = config.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    std::fs::write(config, CONFIG_TEMPLATE).map_err(|e| format!("write {}: {e}", config.display()))
}

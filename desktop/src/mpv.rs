//! Launching the system `mpv` for CENC video and audio tracks.
//!
//! Shared by the Tauri shell and the headless smoke/test helpers.

use std::path::{Path, PathBuf};

/// mpv binary. `QIEYUE_MPV` overrides; otherwise common install locations are
/// probed before falling back to a `PATH` lookup (a GUI launched from Finder
/// does not inherit the shell's PATH).
fn mpv_program() -> PathBuf {
    if let Some(explicit) = std::env::var_os("QIEYUE_MPV") {
        return PathBuf::from(explicit);
    }

    #[cfg(target_os = "macos")]
    let candidates = [
        "/opt/homebrew/bin/mpv",
        "/usr/local/bin/mpv",
        "/usr/bin/mpv",
    ];
    #[cfg(target_os = "linux")]
    let candidates = ["/usr/bin/mpv", "/usr/local/bin/mpv", "/snap/bin/mpv"];

    for candidate in candidates {
        if Path::new(candidate).is_file() {
            return PathBuf::from(candidate);
        }
    }
    PathBuf::from("mpv")
}

fn install_hint() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "用 brew install mpv 安装，或设置 QIEYUE_MPV"
    }
    #[cfg(target_os = "linux")]
    {
        "用系统包管理器安装 mpv（如 apt install mpv），或设置 QIEYUE_MPV"
    }
}

pub fn spawn_mpv_audio(url: &str, key: &str, title: &str) -> Result<(), String> {
    let mut command = std::process::Command::new(mpv_program());
    if !key.is_empty() {
        // Audio tracks are `cenc-aes-ctr` too; without the key mpv reports
        // "Reserved bit set" and gives up.
        command.arg(format!("--demuxer-lavf-o=decryption_key={key}"));
    }
    command
        .arg("--no-video")
        .arg(format!("--title={title}"))
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动 mpv 失败（{}）: {e}", install_hint()))
}

/// The downloaded episode is still CENC encrypted, so mpv needs the key as well.
pub fn spawn_mpv(path: &str, key: &str, title: &str) -> Result<(), String> {
    let mut command = std::process::Command::new(mpv_program());
    if !key.is_empty() {
        command.arg(format!("--demuxer-lavf-o=decryption_key={key}"));
    }
    command
        .arg(format!("--title={title}"))
        .arg("--force-window=yes")
        .arg(path);
    command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动 mpv 失败（{}）: {e}", install_hint()))
}

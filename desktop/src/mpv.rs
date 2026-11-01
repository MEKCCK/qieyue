//! Launching the system `mpv` for CENC video and audio tracks.
//!
//! Shared by the Tauri shell and the headless smoke/test helpers.

use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Mutex;

/// The mpv we spawned last. Only our own process is tracked, so stopping never
/// touches an mpv the user started themselves.
static PLAYBACK: Mutex<Option<Child>> = Mutex::new(None);

fn remember(child: Child) {
    if let Ok(mut slot) = PLAYBACK.lock() {
        if let Some(mut previous) = slot.take() {
            let _ = previous.kill();
            let _ = previous.wait();
        }
        *slot = Some(child);
    }
}

/// Stops the mpv this process started, if any. Returns whether something was running.
pub fn stop_playback() -> bool {
    let Ok(mut slot) = PLAYBACK.lock() else {
        return false;
    };
    match slot.take() {
        Some(mut child) => {
            let _ = child.kill();
            let _ = child.wait();
            true
        }
        None => false,
    }
}

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
    let child = command
        .arg("--no-video")
        .arg(format!("--title={title}"))
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 mpv 失败（{}）: {e}", install_hint()))?;
    remember(child);
    Ok(())
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
    let child = command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 mpv 失败（{}）: {e}", install_hint()))?;
    remember(child);
    Ok(())
}

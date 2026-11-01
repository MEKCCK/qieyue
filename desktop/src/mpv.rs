//! Launching the system `mpv` for CENC video and audio tracks.
//!
//! Shared by the Tauri shell and the headless smoke/test helpers.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Mutex;
use std::time::Duration;

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
            let _ = ipc_command(serde_json::json!(["quit"]));
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

/// mpv's JSON IPC endpoint. A Unix socket, so playback can be inspected and
/// steered without a second window or any extra dependency.
fn ipc_socket() -> PathBuf {
    crate::paths::data_dir().join("run").join("mpv-audio.sock")
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct PlayerState {
    /// Whether mpv's IPC socket answered at all; false means the player is gone.
    pub connected: bool,
    pub playing: bool,
    pub paused: bool,
    pub position: f64,
    pub duration: f64,
}

fn ipc_value(name: &str) -> Option<serde_json::Value> {
    let mut stream = UnixStream::connect(ipc_socket()).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    let payload = serde_json::json!({ "command": ["get_property", name] });
    stream.write_all(format!("{payload}\n").as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let reply: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if reply.get("error").and_then(|v| v.as_str()) != Some("success") {
        return None;
    }
    reply.get("data").cloned()
}

/// Where the current audiobook is, and how long it is. Everything is `0.0`
/// when nothing is playing.
pub fn player_state() -> PlayerState {
    // `duration`/`time-pos` are unavailable while the stream is still opening, so a missing
    // value means "not yet", not "not playing" — only the socket tells us mpv has gone away.
    let connected = UnixStream::connect(ipc_socket()).is_ok();
    let mut state = PlayerState {
        connected,
        position: ipc_value("time-pos")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        duration: ipc_value("duration")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        paused: ipc_value("pause")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        playing: false,
    };
    state.playing = connected
        && state.duration > 0.0
        && ipc_value("idle-active").and_then(|v| v.as_bool()) != Some(true);
    state
}

fn ipc_command(command: serde_json::Value) -> bool {
    let Ok(mut stream) = UnixStream::connect(ipc_socket()) else {
        return false;
    };
    let payload = serde_json::json!({ "command": command });
    stream.write_all(format!("{payload}\n").as_bytes()).is_ok()
}

/// Jumps to an absolute position, in seconds.
pub fn seek(position: f64) -> bool {
    ipc_command(serde_json::json!(["seek", position.max(0.0), "absolute"]))
}

pub fn set_paused(paused: bool) -> bool {
    ipc_command(serde_json::json!(["set_property", "pause", paused]))
}

pub fn toggle_paused() -> bool {
    let paused = !ipc_value("pause")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    set_paused(paused)
}

pub fn spawn_mpv_audio(url: &str, key: &str, title: &str) -> Result<(), String> {
    let mut command = std::process::Command::new(mpv_program());
    if !key.is_empty() {
        // Audio tracks are `cenc-aes-ctr` too; without the key mpv reports
        // "Reserved bit set" and gives up.
        command.arg(format!("--demuxer-lavf-o=decryption_key={key}"));
    }
    // A stale socket from a killed instance would make mpv refuse to listen.
    let socket = ipc_socket();
    if let Some(parent) = socket.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(&socket);
    let child = command
        .arg("--no-video")
        .arg(format!("--input-ipc-server={}", socket.display()))
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

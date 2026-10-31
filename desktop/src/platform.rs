//! Linux and macOS glue: CJK font discovery and mpv launching.
//!
//! Both platforms are supported; everything else is a compile error (see
//! `lib.rs` / `main.rs`). Paths are probed because a GUI launched from Finder
//! does not inherit the shell's `PATH`, where Homebrew's mpv usually lives.

use std::path::{Path, PathBuf};

use eframe::egui;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("qieyue only supports Linux and macOS");

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// CJK-capable fonts, in preference order. `QIEYUE_CJK_FONT` wins when set,
/// which is the escape hatch for a font that is not in any standard location.
fn candidate_fonts() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(explicit) = std::env::var_os("QIEYUE_CJK_FONT") {
        paths.push(PathBuf::from(explicit));
    }

    #[cfg(target_os = "macos")]
    {
        for path in [
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/Library/Fonts/Arial Unicode.ttf",
            "/opt/homebrew/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/local/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        ] {
            paths.push(PathBuf::from(path));
        }
        if let Some(home) = home_dir() {
            paths.push(home.join("Library/Fonts/NotoSansCJK-Regular.ttc"));
            paths.push(home.join("Library/Fonts/PingFang.ttc"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        for path in [
            "/usr/share/fonts/TTF/JetBrains-Maple-Mono-NF-XX-XX/JetBrainsMapleMono-Regular.ttf",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        ] {
            paths.push(PathBuf::from(path));
        }
        if let Some(home) = home_dir() {
            paths.push(home.join(".local/share/fonts/NotoSansCJK-Regular.ttc"));
        }
    }

    paths
}

pub(crate) fn install_fonts(ctx: &egui::Context) {
    let Some(bytes) = candidate_fonts()
        .iter()
        .find_map(|path| std::fs::read(path).ok())
    else {
        eprintln!("no CJK font found, Chinese text will not render");
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "cjk".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// mpv binary. `QIEYUE_MPV` overrides; otherwise common install locations are
/// probed before falling back to a `PATH` lookup.
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

fn mpv_install_hint() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "用 brew install mpv 安装，或设置 QIEYUE_MPV"
    }
    #[cfg(target_os = "linux")]
    {
        "用系统包管理器安装 mpv（如 apt install mpv），或设置 QIEYUE_MPV"
    }
}

pub(crate) fn spawn_mpv_audio(url: &str, key: &str, title: &str) -> Result<(), String> {
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
        .map_err(|e| format!("启动 mpv 失败（{}）: {e}", mpv_install_hint()))
}

/// The downloaded episode is still CENC encrypted, so mpv needs the key as well.
pub(crate) fn spawn_mpv(path: &str, key: &str, title: &str) -> Result<(), String> {
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
        .map_err(|e| format!("启动 mpv 失败（{}）: {e}", mpv_install_hint()))
}

//! Headless entry point for the Rust core: initializes the loopback HTTP adapter and keeps it
//! running, so the REST API can be exercised without the GUI.
//!
//! Run with `cargo run --release -p fqcore`, then use the printed base URL and capability
//! token, e.g.
//!
//! ```text
//! curl -H "Authorization: Bearer <token>" "http://127.0.0.1:<port>/api/v1/search?query=..."
//! ```

use std::path::PathBuf;

/// Written on first run so the daemon is usable without copying a config by
/// hand. Kept in sync with the GUI's template by living in the same asset.
const CONFIG_TEMPLATE: &str = include_str!("../../assets/config/config.json");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = data_dir();
    std::fs::create_dir_all(&data_dir)?;

    let config = data_dir.join("config.json");
    if !config.exists() {
        std::fs::write(&config, CONFIG_TEMPLATE)?;
    }

    let runtime_dir = data_dir.join("run");
    std::fs::create_dir_all(&runtime_dir)?;

    let port: u16 = std::env::var("FQAPP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let result = fqapi_core::api::init(
        config.display().to_string(),
        data_dir.join("devices.json").display().to_string(),
        data_dir.join("filter.json").display().to_string(),
        runtime_dir.display().to_string(),
        port,
        false,
        String::new(),
    )
    .await;

    match result {
        Ok(state) => println!("init: {state}"),
        Err(err) => {
            eprintln!("init failed: {err}");
            std::process::exit(1);
        }
    }

    println!("base_url={}", fqapi_core::api::base_url().await);
    println!("token={}", fqapi_core::api::session_capability().await);
    println!("data_dir={}", data_dir.display());
    println!("--- listening; press Ctrl+C to stop ---");

    // No signal handling: SIGINT terminates the process, which is enough for a dev daemon.
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

/// Data directory for the daemon. Linux follows XDG; macOS follows Apple's
/// convention. `QIEYUE_DATA_DIR` (or the legacy `FQAPP_DATA_DIR`) wins.
fn data_dir() -> PathBuf {
    for key in ["QIEYUE_DATA_DIR", "FQAPP_DATA_DIR"] {
        if let Some(dir) = std::env::var_os(key) {
            return PathBuf::from(dir);
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("no data directory: set QIEYUE_DATA_DIR or HOME");

    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/fqcore")
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("fqcore")
    }
}

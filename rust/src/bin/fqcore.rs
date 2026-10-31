//! Headless entry point for the Rust core: initializes the loopback HTTP adapter and keeps it
//! running, so the REST API can be exercised without Flutter.
//!
//! Run with `cargo run --release --bin fqcore`, then use the printed base URL and capability
//! token, e.g.
//!
//! ```text
//! curl -H "Authorization: Bearer <token>" "http://127.0.0.1:<port>/api/v1/search?query=..."
//! ```

use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = std::env::var_os("FQAPP_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .map(|p| p.join("fqcore"))
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|p| p.join(".local/share/fqcore"))
        })
        .expect("no data directory: set FQAPP_DATA_DIR");

    let runtime_dir = data_dir.join("run");
    std::fs::create_dir_all(&runtime_dir)?;

    let port: u16 = std::env::var("FQAPP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let result = fqapi_core::api::init(
        data_dir.join("config.json").display().to_string(),
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

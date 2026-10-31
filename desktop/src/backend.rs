//! The only place that talks to `fqapi_core`'s request entry point.

use serde_json::Value;

use crate::paths;

/// Initialises the core with the desktop data directory. Idempotent from the
/// caller's point of view: the worker calls it once before handling commands.
pub async fn init_backend() -> Result<(), String> {
    let data = paths::data_dir();
    std::fs::create_dir_all(data.join("run")).map_err(|e| format!("create data dir: {e}"))?;

    let config = data.join("config.json");
    paths::ensure_config(&config)?;

    fqapi_core::api::init(
        config.display().to_string(),
        data.join("devices.json").display().to_string(),
        data.join("filter.json").display().to_string(),
        data.join("run").display().to_string(),
        0,
        false,
        String::new(),
    )
    .await
    .map(|_| ())
}

pub async fn api_get(path: &str, query: &str) -> Result<Value, String> {
    // The upstream throttles bursts: the same request that answers instantly can come back as
    // an error envelope (or with empty rows) a moment later. One short retry smooths that over.
    let mut last_error = String::new();
    for attempt in 0..3 {
        match api_get_once(path, query).await {
            Ok(value) => return Ok(value),
            Err(err) => {
                last_error = err;
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_millis(350 * (attempt + 1))).await;
                }
            }
        }
    }
    Err(last_error)
}

async fn api_get_once(path: &str, query: &str) -> Result<Value, String> {
    // A fresh request id per call: sharing one (or an empty one) makes the core treat the
    // requests as the same in-flight call and hand back the earlier response.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let request_id = format!(
        "desktop-{}",
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let response = fqapi_core::api::request(
        request_id,
        "GET".to_string(),
        path.to_string(),
        query.to_string(),
        Vec::new(),
        30_000,
    )
    .await
    .map_err(|e| format!("request {path}: {e}"))?;

    if response.status != 200 {
        return Err(format!("{path}: HTTP {}", response.status));
    }

    serde_json::from_slice(&response.body).map_err(|e| format!("{path}: bad json: {e}"))
}

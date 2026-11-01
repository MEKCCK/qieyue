//! Comic chapters: decrypted pages on disk, exposed as local URIs.

use serde_json::Value;

use crate::backend::api_get;
use crate::parse::{first_non_empty, text_of, url_encode};
use crate::paths;

/// Turns a `/src/...` path the backend serves into a `file://` URI the image loader can read.
pub fn served_uri(path: &str) -> String {
    if let Some(relative) = path.strip_prefix("/src/") {
        let local = paths::data_dir().join("run").join("src").join(relative);
        format!("file://{}", local.display())
    } else if path.starts_with("http") {
        path.to_string()
    } else {
        format!("file://{path}")
    }
}

/// `/api/v1/manga/{item_id}` decrypts every page of one chapter into `{"images": ["/src/..."]}`.
pub async fn manga_chapter(item_id: &str) -> Result<Vec<String>, String> {
    let json = api_get(&format!("/api/v1/manga/{}", url_encode(item_id)), "").await?;
    let pages: Vec<String> = json
        .get("images")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(served_uri))
        .collect();
    if pages.is_empty() {
        return Err(format!(
            "这一话没有图片（{}）",
            first_non_empty(&[text_of(&json, "error"), text_of(&json, "message")])
        ));
    }
    Ok(pages)
}

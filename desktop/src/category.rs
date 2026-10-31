//! Category page: channels, tag dimensions and tiles.

use serde_json::Value;

use crate::backend::api_get;
use crate::models::{CategoryDim, CategoryFront, CategoryTab, CategoryTile};
use crate::parse::text_of;

/// `/api/v1/category` answers with `rule.tab` (channels), `rule.dim` (tag dimensions) and a flat
/// `down_category[]` of tiles, each carrying the dimension it belongs to.
pub async fn category_front(tab: u32) -> Result<CategoryFront, String> {
    let json = api_get("/api/v1/category", &format!("category_tab={tab}")).await?;
    let data = json.get("data").cloned().unwrap_or(Value::Null);
    let rule = data.get("rule").cloned().unwrap_or(Value::Null);

    let mut tabs = Vec::new();
    for item in rule
        .get("tab")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = text_of(item, "name");
        let value = text_of(item, "value");
        if !name.is_empty() && !value.is_empty() {
            tabs.push(CategoryTab { name, value });
        }
    }

    let mut dims = Vec::new();
    for item in rule
        .get("dim")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = text_of(item, "name");
        if !name.is_empty() {
            dims.push(CategoryDim {
                name,
                value: text_of(item, "value"),
            });
        }
    }

    let mut tiles = Vec::new();
    for item in data
        .get("down_category")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = text_of(item, "name");
        if name.is_empty() {
            continue;
        }
        let tag = item.get("tag");
        let hot = matches!(tag, Some(Value::Number(n)) if n.as_i64() == Some(1))
            || matches!(tag, Some(Value::String(s)) if s == "1");
        tiles.push(CategoryTile {
            name,
            dim: text_of(item, "dim"),
            hot,
        });
    }

    Ok(CategoryFront { tabs, dims, tiles })
}

//! Book and series comments.

use serde_json::Value;

use crate::backend::api_get;
use crate::models::Comment;
use crate::parse::{format_timestamp, text_of, url_encode};

/// `/api/v1/books/{id}/comments` answers `data.comment[]` (`text`, `user_info.user_name`,
/// `create_timestamp`, `digg_count`, `reply_count`) plus `has_more`/`next_offset`.
pub async fn book_comments(
    book_id: &str,
    offset: usize,
) -> Result<(Vec<Comment>, Option<usize>), String> {
    let mut query = String::from("count=10");
    if offset > 0 {
        query.push_str(&format!("&offset={offset}"));
    }
    let json = api_get(
        &format!("/api/v1/books/{}/comments", url_encode(book_id)),
        &query,
    )
    .await?;
    parse_comments(json)
}

/// Short dramas expose their reviews under `/api/v1/series/{id}/comments` with the same rows.
pub async fn series_comments(
    series_id: &str,
    offset: usize,
) -> Result<(Vec<Comment>, Option<usize>), String> {
    let mut query = String::from("count=10");
    if offset > 0 {
        query.push_str(&format!("&offset={offset}"));
    }
    let json = api_get(
        &format!("/api/v1/series/{}/comments", url_encode(series_id)),
        &query,
    )
    .await?;
    parse_comments(json)
}

fn parse_comments(json: Value) -> Result<(Vec<Comment>, Option<usize>), String> {
    let data = json.get("data").cloned().unwrap_or(Value::Null);
    let rows = data
        .get("comment")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for row in rows {
        let text = text_of(&row, "text");
        if text.trim().is_empty() {
            continue;
        }
        let user = row
            .get("user_info")
            .map(|info| text_of(info, "user_name"))
            .unwrap_or_default();
        let stamp = text_of(&row, "create_timestamp");
        out.push(Comment {
            user,
            text,
            when: format_timestamp(&stamp),
            digg: text_of(&row, "digg_count"),
            replies: text_of(&row, "reply_count"),
        });
    }

    let has_more = matches!(data.get("has_more"), Some(Value::Bool(true)));
    let next = if has_more {
        text_of(&data, "next_offset").parse::<usize>().ok()
    } else {
        None
    };
    Ok((out, next))
}

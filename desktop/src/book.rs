//! Book detail, chapter directory and chapter text.

use serde_json::Value;

use crate::backend::api_get;
use crate::models::{BookDetail, Chapter};
use crate::parse::{first_non_empty, html_to_paragraphs, text_of, url_encode};

pub async fn open_book(id: &str) -> Result<BookDetail, String> {
    let detail = api_get(&format!("/api/v1/books/{}/detail", url_encode(id)), "").await?;
    let data = detail.get("data").cloned().unwrap_or(Value::Null);
    let title = text_of(&data, "book_name");
    let author = text_of(&data, "author");
    let summary = first_non_empty(&[
        text_of(&data, "abstract"),
        text_of(&data, "book_abstract_v2"),
    ]);
    let is_comic = text_of(&data, "comic_book_type") == "1";
    let cover = first_non_empty(&[
        text_of(&data, "thumb_url"),
        text_of(&data, "horiz_thumb_url"),
        text_of(&data, "audio_thumb_uri"),
    ]);

    let directory = api_get(&format!("/api/v1/books/{}/directory", url_encode(id)), "").await?;
    let mut chapters = Vec::new();
    if let Some(list) = directory
        .get("data")
        .and_then(|d| d.get("item_data_list"))
        .and_then(Value::as_array)
    {
        for item in list {
            let item_id = text_of(item, "item_id");
            if item_id.is_empty() {
                continue;
            }
            chapters.push(Chapter {
                item_id,
                title: text_of(item, "title"),
            });
        }
    }
    Ok(BookDetail {
        title,
        author,
        summary,
        cover,
        is_comic,
        chapters,
    })
}

pub async fn read_chapter(item_id: &str) -> Result<(String, Vec<String>), String> {
    let json = api_get(&format!("/api/v1/chapters/{}", url_encode(item_id)), "").await?;
    let data = json.get("data").cloned().unwrap_or(Value::Null);
    let title = text_of(&data, "title");
    let content = text_of(&data, "content");
    Ok((title, html_to_paragraphs(&content)))
}

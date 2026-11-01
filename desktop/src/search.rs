//! Search results and paging.

use serde_json::Value;

use crate::backend::api_get;
use crate::models::{Book, Hit, SearchPage};
use crate::parse::{hit_from_item, text_of, url_encode};

/// Flat book list, kept for the headless smoke test.
pub async fn search(query: &str) -> Result<Vec<Book>, String> {
    let json = api_get("/api/v1/search", &format!("query={}", url_encode(query))).await?;
    let mut books = Vec::new();
    let tabs = json.get("search_tabs").and_then(Value::as_array);
    for tab in tabs.into_iter().flatten() {
        for item in tab
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for book in item
                .get("book_data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let id = text_of(book, "book_id");
                if id.is_empty() || books.iter().any(|b: &Book| b.id == id) {
                    continue;
                }
                books.push(Book {
                    id,
                    title: text_of(book, "book_name"),
                    author: text_of(book, "author"),
                    summary: crate::parse::first_non_empty(&[
                        text_of(book, "abstract"),
                        text_of(book, "book_abstract_v2"),
                    ]),
                });
            }
        }
    }
    Ok(books)
}

pub async fn search_page(
    query: &str,
    offset: usize,
    tab_type: Option<u32>,
) -> Result<SearchPage, String> {
    let mut query_string = format!("query={}&offset={offset}&count=10", url_encode(query));
    if let Some(tab_type) = tab_type {
        query_string.push_str(&format!("&tab_type={tab_type}"));
    }
    let json = api_get("/api/v1/search", &query_string).await?;

    let mut hits: Vec<Hit> = Vec::new();
    let mut next_offset: Option<usize> = None;
    let tabs = json.get("search_tabs").and_then(Value::as_array);

    for tab in tabs.into_iter().flatten() {
        let has_more = matches!(
            tab.get("has_more"),
            Some(Value::Bool(true)) | Some(Value::Number(_)) | Some(Value::String(_))
        ) && !matches!(tab.get("has_more"), Some(Value::Bool(false)));
        if has_more {
            if let Some(next) = tab.get("next_offset").and_then(|value| {
                value
                    .as_u64()
                    .map(|n| n as usize)
                    .or_else(|| text_of(tab, "next_offset").parse().ok())
            }) {
                next_offset = Some(next_offset.map_or(next, |current: usize| current.max(next)));
            } else {
                next_offset = Some(next_offset.unwrap_or(offset + 10));
            }
        }

        for item in tab
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(hit) = hit_from_item(item) {
                if !hits.iter().any(|existing: &Hit| existing.id == hit.id) {
                    hits.push(hit);
                }
            }
        }
    }

    Ok(SearchPage { hits, next_offset })
}

pub async fn search_hits(query: &str) -> Result<Vec<Hit>, String> {
    Ok(search_page(query, 0, None).await?.hits)
}

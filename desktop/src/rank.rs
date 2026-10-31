//! Rank catalogue and board pages.

use serde_json::Value;

use crate::backend::api_get;
use crate::models::{Hit, RankBoard, RankCatalog, RankCategory};
use crate::parse::{first_non_empty, text_of, url_encode};

/// The catalogue rides inside the novel homepage payload, in the card that carries
/// `rank_with_category_data`.
pub async fn rank_catalog() -> Result<RankCatalog, String> {
    let json = api_get(
        "/api/v1/recommend/homepage",
        "tab_type=2&offset=0&tab_index=0&last_tab_index=-1",
    )
    .await?;
    let data = json.get("data").cloned().unwrap_or(Value::Null);

    let mut rank_id = String::new();
    let mut boards = Vec::new();
    let mut categories = Vec::new();
    for tab in data
        .get("tab_item")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        for cell in tab
            .get("cell_data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(rank) = cell.get("rank_with_category_data") else {
                continue;
            };
            rank_id = first_non_empty(&[text_of(cell, "cell_id_str"), text_of(cell, "cell_id")]);
            for entry in rank
                .get("rank_algo_list")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let name = text_of(entry, "rank_name");
                let algo = text_of(entry, "rank_algo");
                if !name.is_empty() && !algo.is_empty() {
                    boards.push(RankBoard { name, algo });
                }
            }
            for entry in rank
                .get("sub_info_list")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let name = text_of(entry, "info_name");
                if !name.is_empty() {
                    categories.push(RankCategory {
                        name,
                        id: text_of(entry, "info_id"),
                    });
                }
            }
        }
    }

    if rank_id.is_empty() {
        return Err("榜单目录不可用".to_string());
    }
    Ok(RankCatalog {
        rank_id,
        boards,
        categories,
    })
}

/// One page of a board. Rows are either flat cells with `book_data` or a group cell nesting
/// further cells (巅峰榜 answers the latter).
pub async fn rank_page(
    rank_id: &str,
    algo: &str,
    category_id: &str,
    offset: usize,
) -> Result<Vec<Hit>, String> {
    let mut query = format!("algo_type={algo}&rank_sub_info_id={category_id}");
    if offset > 0 {
        query.push_str(&format!("&offset={offset}"));
    }
    let json = api_get(&format!("/api/v1/rank/{}", url_encode(rank_id)), &query).await?;
    let data = json.get("data").cloned().unwrap_or(Value::Null);

    let mut hits: Vec<Hit> = Vec::new();
    let push_book = |book: &Value, hits: &mut Vec<Hit>| {
        let id = text_of(book, "book_id");
        if id.is_empty() || hits.iter().any(|hit| hit.id == id) {
            return;
        }
        hits.push(Hit {
            id,
            title: text_of(book, "book_name"),
            author: text_of(book, "author"),
            summary: first_non_empty(&[
                text_of(book, "abstract"),
                text_of(book, "book_abstract_v2"),
            ]),
            cover: first_non_empty(&[text_of(book, "thumb_url"), text_of(book, "horiz_thumb_url")]),
            series: None,
        });
    };

    for cell in data
        .get("cell_view")
        .and_then(|view| view.get("cell_data"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(books) = cell.get("book_data").and_then(Value::as_array) {
            for book in books {
                push_book(book, &mut hits);
            }
        }
        if let Some(nested) = cell.get("cell_data").and_then(Value::as_array) {
            for group in nested {
                for book in group
                    .get("book_data")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    push_book(book, &mut hits);
                }
            }
        }
    }
    Ok(hits)
}

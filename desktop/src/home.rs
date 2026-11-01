//! Home discovery feed: channels and their paged blocks.

use serde_json::Value;

use crate::backend::api_get;
use crate::models::{Hit, Home, HomeSection, HomeTab};
use crate::parse::{first_non_empty, hit_from_item, text_of};

/// One page of a feed block. The payload nests the cards one level deep:
/// `data.cell_view.cell_data[0].cell_data[]` holds the actual rows.
pub async fn series_feed_page(
    cell_id: &str,
    tab_type: u32,
    client_template: &str,
    offset: usize,
) -> Result<(Vec<Hit>, Option<usize>), String> {
    // A channel without a `tab_type` cannot answer a feed request at all (HTTP 400); its blocks
    // were inline anyway.
    if tab_type == 0 {
        return Ok((Vec::new(), None));
    }

    // Parameter sets differ per block: some accept the selector hint, some reject it. Try the
    // richest first and fall back to the bare form instead of failing the whole page.
    let selector = if offset == 0 { 2 } else { 1 };
    let template = if client_template.is_empty() {
        "2"
    } else {
        client_template
    };
    let candidates = [
        format!(
            "cell_id={cell_id}&tab_type={tab_type}&client_template={template}&client_req_type=2&offset={offset}&unlimited_selector_change_type={selector}"
        ),
        format!(
            "cell_id={cell_id}&tab_type={tab_type}&client_template={template}&client_req_type=2&offset={offset}"
        ),
        format!("cell_id={cell_id}&tab_type={tab_type}&offset={offset}"),
    ];

    let mut json = Value::Null;
    let mut last_error = String::new();
    for query in &candidates {
        match api_get("/api/v1/recommend/series-feed", query).await {
            Ok(value) => {
                json = value;
                break;
            }
            Err(err) => last_error = err,
        }
    }
    if json.is_null() {
        return Err(last_error);
    }
    let data = json.get("data").cloned().unwrap_or(Value::Null);

    let has_more = !matches!(data.get("has_more"), Some(Value::Bool(false)) | None);
    let next_offset = text_of(&data, "next_offset")
        .parse::<usize>()
        .ok()
        .or_else(|| {
            data.get("next_offset")
                .and_then(Value::as_u64)
                .map(|n| n as usize)
        })
        .filter(|_| has_more);

    let mut hits = Vec::new();
    // Groups carry their payload in one of three shapes: `video_data`, `book_data`, or a nested
    // `cell_data` list of rows. `hit_from_item` understands the first two.
    let push = |item: &Value, hits: &mut Vec<Hit>| {
        if let Some(hit) = hit_from_item(item) {
            // A row with neither a title nor artwork would render as an empty card.
            if hit.title.trim().is_empty() && hit.cover.is_empty() {
                return;
            }
            if !hits.iter().any(|existing: &Hit| existing.id == hit.id) {
                hits.push(hit);
            }
        }
    };
    for group in data
        .get("cell_view")
        .and_then(|view| view.get("cell_data"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        push(group, &mut hits);
        for item in group
            .get("cell_data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            push(item, &mut hits);
        }
        for item in group
            .get("book_data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            push(item, &mut hits);
        }
    }
    Ok((hits, next_offset))
}

pub async fn series_feed(
    cell_id: &str,
    tab_type: u32,
    client_template: &str,
    offset: usize,
) -> Result<Vec<Hit>, String> {
    Ok(series_feed_page(cell_id, tab_type, client_template, offset)
        .await?
        .0)
}

/// Pulls the next page of one home block.
pub async fn home_more(
    cell_id: &str,
    tab_type: u32,
    client_template: &str,
    offset: usize,
) -> Result<(Vec<Hit>, Option<usize>), String> {
    series_feed_page(cell_id, tab_type, client_template, offset).await
}

/// Loads one channel of the discovery page. `tab_index` selects the channel, and
/// `client_template` has to match that channel or the service answers with no blocks at all.
pub async fn home_channel(
    tab_type: u32,
    tab_index: u32,
    client_template: &str,
) -> Result<Home, String> {
    let query = format!(
        "tab_type={tab_type}&offset=0&tab_index={tab_index}&last_tab_index=-1&client_template={client_template}"
    );
    let json = api_get("/api/v1/recommend/homepage", &query).await?;
    let home = parse_home(json.clone(), tab_index as usize, client_template).await;
    if matches!(&home, Ok(home) if home.sections.is_empty()) {
        // An empty page is what a throttled call looks like; give it one more chance.
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        let retry = api_get("/api/v1/recommend/homepage", &query).await?;
        if let Ok(retry) = parse_home(retry, tab_index as usize, client_template).await {
            if !retry.sections.is_empty() {
                return Ok(retry);
            }
        }
    }
    home
}

pub async fn home() -> Result<Home, String> {
    home_channel(2, 0, "0").await
}

/// A block before its feed page is fetched.
struct RawCell {
    title: String,
    hits: Vec<Hit>,
    cell_id: String,
    is_rank_preview: bool,
    tab_type: u32,
}

async fn parse_home(json: Value, active: usize, client_template: &str) -> Result<Home, String> {
    let data = json.get("data").cloned().unwrap_or(Value::Null);

    let mut tabs = Vec::new();
    let mut raw_cells: Vec<RawCell> = Vec::new();
    for (index, tab) in data
        .get("tab_item")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let title = text_of(tab, "title");
        let tab_type: u32 = text_of(tab, "tab_type").parse().unwrap_or(0);
        let tab_template = text_of(tab, "client_template");
        if !title.is_empty() {
            tabs.push(HomeTab {
                title,
                tab_type,
                index: index as u32,
                client_template: tab_template,
            });
        }
        if index != active {
            continue;
        }

        for cell in tab
            .get("cell_data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let title = text_of(cell, "cell_name");
            let mut hits = Vec::new();
            // Two shapes: rows under `cell_data`, or a plain `book_data` list when the block is a
            // single grid of books (the comic channel renders this way).
            for item in cell
                .get("cell_data")
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
            for book in cell
                .get("book_data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let id = text_of(book, "book_id");
                if id.is_empty() || hits.iter().any(|hit| hit.id == id) {
                    continue;
                }
                hits.push(Hit {
                    id,
                    title: text_of(book, "book_name"),
                    author: text_of(book, "author"),
                    summary: first_non_empty(&[
                        text_of(book, "abstract"),
                        text_of(book, "book_abstract_v2"),
                    ]),
                    cover: first_non_empty(&[
                        text_of(book, "thumb_url"),
                        text_of(book, "horiz_thumb_url"),
                    ]),
                    series: None,
                });
            }
            raw_cells.push(RawCell {
                is_rank_preview: title.contains("排行榜"),
                title,
                hits,
                // Any block with an id can also be paged through `series-feed`: that is where
                // 看剧/视频 blocks get their remaining rows and the offsets for 加载更多.
                cell_id: text_of(cell, "cell_id_str"),
                tab_type,
            });
        }
    }

    // Blocks are independent. Fetching their first feed page concurrently is
    // what turns a channel switch from "sum of every block" into one round
    // trip; `join_all` keeps the section order stable.
    let fetched = futures::future::join_all(raw_cells.into_iter().map(|cell| {
        let template = client_template.to_string();
        async move {
            let RawCell {
                title,
                mut hits,
                cell_id,
                is_rank_preview,
                tab_type,
            } = cell;
            let mut feed_id = String::new();
            let mut next_offset = None;
            if !is_rank_preview && !cell_id.is_empty() && tab_type != 0 {
                match series_feed_page(&cell_id, tab_type, &template, 0).await {
                    Ok((extra, next)) => {
                        for hit in extra {
                            if !hits.iter().any(|existing: &Hit| existing.id == hit.id) {
                                hits.push(hit);
                            }
                        }
                        if next.is_some() {
                            feed_id = cell_id;
                            next_offset = next;
                        }
                    }
                    Err(err) => eprintln!("series-feed {cell_id}: {err}"),
                }
            }
            (title, hits, feed_id, tab_type, next_offset)
        }
    }))
    .await;

    let sections = fetched
        .into_iter()
        .filter(|(_, hits, ..)| !hits.is_empty())
        .map(
            |(title, hits, feed_id, tab_type, next_offset)| HomeSection {
                title,
                hits,
                cell_id: feed_id,
                tab_type,
                client_template: client_template.to_string(),
                next_offset,
            },
        )
        .collect();

    let active = text_of(&data, "tab_index")
        .parse::<u32>()
        .unwrap_or(active as u32);

    Ok(Home {
        tabs,
        sections,
        active,
    })
}

//! Backend-facing helpers shared by the GUI and the smoke test.
//!
//! Everything here talks to `fqapi_core`, which owns the protocol, crypto and cache. The
//! functions return exactly what a view needs, already parsed out of the service JSON.

use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct Chapter {
    pub item_id: String,
    pub title: String,
}

pub async fn init_backend() -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    let data = std::path::PathBuf::from(home).join(".local/share/fqapp-desktop");
    std::fs::create_dir_all(data.join("run")).map_err(|e| format!("create data dir: {e}"))?;

    let config = data.join("config.json");
    if !config.exists() {
        let template = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../assets/config/config.json");
        std::fs::copy(&template, &config)
            .map_err(|e| format!("copy {}: {e}", template.display()))?;
    }

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

pub async fn search(query: &str) -> Result<Vec<Book>, String> {
    let json = api_get("/api/v1/search", &format!("query={}", url_encode(query))).await?;
    let mut books = Vec::new();
    let tabs = json.get("search_tabs").and_then(Value::as_array);
    for tab in tabs.into_iter().flatten() {
        for item in tab.get("data").and_then(Value::as_array).into_iter().flatten() {
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
                    summary: first_non_empty(&[text_of(book, "abstract"), text_of(book, "book_abstract_v2")]),
                });
            }
        }
    }
    Ok(books)
}

pub struct BookDetail {
    pub title: String,
    pub author: String,
    pub summary: String,
    pub cover: String,
    /// Comics answer `comic_book_type = 1`; their chapters are images, not text.
    pub is_comic: bool,
    pub chapters: Vec<Chapter>,
}

pub async fn open_book(id: &str) -> Result<BookDetail, String> {
    let detail = api_get(&format!("/api/v1/books/{}/detail", url_encode(id)), "").await?;
    let data = detail.get("data").cloned().unwrap_or(Value::Null);
    let title = text_of(&data, "book_name");
    let author = text_of(&data, "author");
    let summary = first_non_empty(&[text_of(&data, "abstract"), text_of(&data, "book_abstract_v2")]);
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

fn text_of(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn first_non_empty(candidates: &[String]) -> String {
    candidates
        .iter()
        .find(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_default()
}

pub fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Turns the chapter HTML into plain paragraphs: drops tags and comments, keeps line breaks at
/// block boundaries and decodes the handful of entities the source actually uses.
pub fn html_to_paragraphs(html: &str) -> Vec<String> {
    // The service hands out metadata as `{!-- ... --}`: `<!-- -->` with the angle brackets
    // swapped for braces so that a naive client does not parse them as comments. Normalise so
    // the scanner below strips them like any other comment.
    let normalized = html.replace("{!--", "<!--").replace("--}", "-->");
    let html = normalized.as_str();

    let mut text = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<!--") {
            match html[i + 4..].find("-->") {
                Some(end) => {
                    i += 4 + end + 3;
                    continue;
                }
                None => break,
            }
        }
        if bytes[i] == b'<' {
            match html[i..].find('>') {
                Some(end) => {
                    let tag = &html[i + 1..i + end];
                    let name = tag
                        .trim_start_matches('/')
                        .split(|c: char| c.is_whitespace() || c == '/')
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if matches!(name.as_str(), "p" | "br" | "div" | "section" | "h1" | "h2" | "h3" | "li") {
                        text.push('\n');
                    }
                    i += end + 1;
                    continue;
                }
                None => break,
            }
        }
        let ch = html[i..].chars().next().unwrap_or(' ');
        text.push(ch);
        i += ch.len_utf8();
    }

    decode_entities(&text)
        .split('\n')
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && !line.contains("PGC_"))
        .collect()
}

fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        // Scan by chars: the payload may be multibyte, and slicing by bytes would panic.
        let end = match rest
            .char_indices()
            .skip(1)
            .take(12)
            .find(|(_, c)| *c == ';')
            .map(|(index, _)| index)
        {
            Some(end) => end,
            None => {
                out.push('&');
                rest = &rest[1..];
                continue;
            }
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16).ok().and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => entity[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(ch) => {
                out.push(ch);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------------------------
// short dramas (短剧)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Series {
    pub id: String,
    pub title: String,
    pub intro: String,
    pub cover: String,
    pub episodes: Vec<Episode>,
}

#[derive(Clone, Debug)]
pub struct Episode {
    pub vid: String,
    pub title: String,
    pub index: usize,
    pub cover: String,
    pub duration: String,
}

/// Every search hit that carries a `video_data` entry is a short drama; the id we need for
/// the series endpoint is `series_id`, not the `book_id` the hit also carries.
pub fn series_from_hit(value: &Value) -> Option<(String, String, String)> {
    let first = value.get("video_data")?.as_array()?.first()?;
    let id = text_of(first, "series_id");
    let detail = value
        .get("video_detail")
        .or_else(|| first.get("video_detail"))
        .cloned()
        .unwrap_or(Value::Null);
    let title = first_non_empty(&[
        text_of(first, "raw_book_name"),
        text_of(first, "series_title"),
        text_of(first, "title"),
        text_of(&detail, "series_title"),
        text_of(&detail, "title"),
    ]);
    let cover = first_non_empty(&[
        text_of(first, "cover"),
        text_of(first, "horiz_cover"),
        text_of(&detail, "cover"),
        text_of(&detail, "series_cover"),
        text_of(&detail, "horiz_cover"),
        text_of(first, "episode_cover"),
    ]);
    if id.is_empty() {
        None
    } else {
        Some((id, title, cover))
    }
}

pub async fn open_series(id: &str) -> Result<Series, String> {
    let json = api_get(&format!("/api/v1/series/{}", url_encode(id)), "").await?;
    let data = json
        .get("data")
        .and_then(|d| d.get("video_data"))
        .cloned()
        .unwrap_or(Value::Null);

    let title = text_of(&data, "series_title");
    let intro = text_of(&data, "series_intro");
    let cover = text_of(&data, "series_cover");

    let mut episodes = Vec::new();
    if let Some(list) = data.get("video_list").and_then(Value::as_array) {
        for (index, item) in list.iter().enumerate() {
            let vid = text_of(item, "vid");
            if vid.is_empty() {
                continue;
            }
            // On short dramas this field often carries the whole series synopsis instead of an
            // episode name; fall back to the index so the list stays readable.
            let raw_title = text_of(item, "title");
            let title = if raw_title.chars().count() > 24 {
                String::new()
            } else {
                raw_title
            };
            episodes.push(Episode {
                vid,
                title,
                index: index + 1,
                cover: text_of(item, "episode_cover"),
                duration: text_of(item, "duration"),
            });
        }
    }

    Ok(Series {
        id: id.to_string(),
        title,
        intro,
        cover,
        episodes,
    })
}

/// Resolves an episode to something mpv can actually open.
///
/// The default mode asks the backend to download the episode; what lands on disk is still CENC
/// encrypted, so the returned key is required as well (`decryption_key`). The stream mode is the
/// only place that exposes the key, hence the second round trip.
pub async fn resolve_episode(vid: &str) -> Result<(String, String), String> {
    let download = api_get(&format!("/api/v1/videos/{}", url_encode(vid)), "").await?;
    let served = text_of(&download, "video_url");
    if served.is_empty() {
        return Err("后端没有返回视频地址".to_string());
    }

    let stream = api_get(
        &format!("/api/v1/videos/{}", url_encode(vid)),
        "mode=stream",
    )
    .await?;
    let key = text_of(&stream, "key_hex");

    // `/src/foo.mp4` is served from `<data>/run/src/foo.mp4`.
    let local = if let Some(relative) = served.strip_prefix("/src/") {
        data_dir().join("run").join("src").join(relative)
    } else {
        std::path::PathBuf::from(served)
    };

    if !local.exists() {
        return Err(format!("视频文件不在预期位置: {}", local.display()));
    }
    Ok((local.display().to_string(), key))
}

// ---------------------------------------------------------------------------------------------
// shelf and reading progress
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ShelfEntry {
    pub id: String,
    pub title: String,
    pub author: String,
    /// Cover URL, so the shelf page can render a real grid like the original.
    #[serde(default)]
    pub cover: String,
    /// `book` or `series`; series are short dramas / videos and open the episode list instead of
    /// a chapter directory.
    #[serde(default = "default_kind")]
    pub kind: String,
    pub item_id: String,
    pub chapter_title: String,
    pub index: usize,
}

fn default_kind() -> String {
    "book".to_string()
}

/// Shelf row for a short drama: `item_id` holds the episode to resume at.
pub fn series_shelf_entry(series: &Series, progress: Option<&Episode>) -> ShelfEntry {
    ShelfEntry {
        id: series.id.clone(),
        title: series.title.clone(),
        author: String::new(),
        cover: series.cover.clone(),
        kind: "series".to_string(),
        item_id: progress.map(|episode| episode.vid.clone()).unwrap_or_default(),
        chapter_title: progress
            .map(|episode| format!("第 {} 集", episode.index))
            .unwrap_or_else(|| format!("共 {} 集", series.episodes.len())),
        index: progress.map(|episode| episode.index.saturating_sub(1)).unwrap_or(0),
    }
}

pub fn data_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".local/share/fqapp-desktop")
}

pub fn load_shelf() -> Vec<ShelfEntry> {
    let path = data_dir().join("shelf.json");
    std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save_shelf(entries: &[ShelfEntry]) -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(entries).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("shelf.json"), bytes).map_err(|e| e.to_string())
}

/// Records the reading position of a book, moving it to the front of the shelf.
pub fn remember(entry: ShelfEntry) -> Result<(), String> {
    let mut shelf = load_shelf();
    shelf.retain(|existing| existing.id != entry.id);
    shelf.insert(0, entry);
    shelf.truncate(50);
    save_shelf(&shelf)
}

/// One row of the search result list: either a book (novel, audiobook, manga) or a short drama.
#[derive(Clone, Debug)]
pub struct Hit {
    pub id: String,
    pub title: String,
    pub author: String,
    pub summary: String,
    pub cover: String,
    /// `Some(series_id)` marks a short drama; `id` is then the series id as well.
    pub series: Option<String>,
}

pub struct SearchPage {
    pub hits: Vec<Hit>,
    /// Offset to request next, or `None` when the service reports no more rows.
    pub next_offset: Option<usize>,
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
            if let Some(next) = tab
                .get("next_offset")
                .and_then(|value| value.as_u64().map(|n| n as usize).or_else(|| text_of(tab, "next_offset").parse().ok()))
            {
                next_offset = Some(next_offset.map_or(next, |current: usize| current.max(next)));
            } else {
                next_offset = Some(next_offset.map_or(offset + 10, |current: usize| current));
            }
        }

        for item in tab.get("data").and_then(Value::as_array).into_iter().flatten() {
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
    let mut push = |item: &Value, hits: &mut Vec<Hit>| {
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

/// `/api/v1/recommend/homepage` returns `data.tab_item[]`, and each tab carries `cell_data[]`
/// blocks that hold the actual cards. Only the first tab is requested here: its blocks are the
/// mixed recommendation feed the original lands on.
#[derive(Clone, Debug)]
pub struct HomeSection {
    pub title: String,
    pub hits: Vec<Hit>,
    /// Feed blocks can be paged; a plain card grid (e.g. 排行榜) arrives complete and has none.
    pub cell_id: String,
    pub tab_type: u32,
    /// The channel's own template; feed requests are rejected when it does not match.
    pub client_template: String,
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct HomeTab {
    pub title: String,
    pub tab_type: u32,
    /// Index of this channel inside the response (`tab_index` on the wire).
    pub index: u32,
    pub client_template: String,
}

#[derive(Clone, Debug)]
pub struct Home {
    /// Top channels of the discovery page (`推荐`, `小说`, `听书`, …).
    pub tabs: Vec<HomeTab>,
    pub sections: Vec<HomeSection>,
    /// The channel this payload describes; the view highlights this, not whatever was clicked.
    pub active: u32,
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

async fn parse_home(json: Value, active: usize, client_template: &str) -> Result<Home, String> {
    let data = json.get("data").cloned().unwrap_or(Value::Null);

    let mut tabs = Vec::new();
    let mut sections = Vec::new();
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
            // Any block with an id can also be paged through `series-feed`: that is where
            // 看剧/视频 blocks get their remaining rows and the offsets for 加载更多.
            let mut next_offset = None;
            let mut feed_id = String::new();
            let cell_id = text_of(cell, "cell_id_str");
            let is_rank_preview = title.contains("排行榜");
            if !is_rank_preview && !cell_id.is_empty() && tab_type != 0 {
                match series_feed_page(&cell_id, tab_type, client_template, 0).await {
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

            if !hits.is_empty() {
                sections.push(HomeSection {
                    title,
                    hits,
                    cell_id: feed_id,
                    tab_type,
                    client_template: client_template.to_string(),
                    next_offset,
                });
            }
        }
    }

    let active = text_of(&data, "tab_index")
        .parse::<u32>()
        .unwrap_or(active as u32);

    Ok(Home {
        tabs,
        sections,
        active,
    })
}

/// One card from a feed cell, search result or shelf row.
fn hit_from_item(item: &Value) -> Option<Hit> {
    if let Some((series_id, title, cover)) = series_from_hit(item) {
        return Some(Hit {
            id: series_id.clone(),
            title,
            author: String::new(),
            summary: String::new(),
            cover,
            series: Some(series_id),
        });
    }
    // Flat video rows (the 视频 channel) carry `series_id`/`vid` directly, with the artwork and
    // title under `video_detail`.
    let series_id = text_of(item, "series_id");
    if !series_id.is_empty() {
        let detail = item.get("video_detail").cloned().unwrap_or(Value::Null);
        let title = first_non_empty(&[
            text_of(&detail, "title"),
            text_of(item, "title"),
            text_of(&detail, "series_title"),
        ]);
        let cover = first_non_empty(&[
            text_of(&detail, "cover"),
            text_of(&detail, "horiz_cover"),
            text_of(item, "cover"),
            text_of(item, "horiz_cover"),
        ]);
        return Some(Hit {
            id: series_id.clone(),
            title,
            author: String::new(),
            summary: String::new(),
            cover,
            series: Some(series_id),
        });
    }

    let book = item.get("book_data")?.as_array()?.first()?;
    let id = text_of(book, "book_id");
    if id.is_empty() {
        return None;
    }
    Some(Hit {
        id,
        title: text_of(book, "book_name"),
        author: text_of(book, "author"),
        summary: first_non_empty(&[text_of(book, "abstract"), text_of(book, "book_abstract_v2")]),
        cover: first_non_empty(&[
            text_of(book, "thumb_url"),
            text_of(book, "horiz_thumb_url"),
        ]),
        series: None,
    })
}

// ---------------------------------------------------------------------------------------------
// categories
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct CategoryTab {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct CategoryDim {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct CategoryTile {
    pub name: String,
    pub dim: String,
    pub hot: bool,
}

#[derive(Clone, Debug)]
pub struct CategoryFront {
    pub tabs: Vec<CategoryTab>,
    pub dims: Vec<CategoryDim>,
    pub tiles: Vec<CategoryTile>,
}

/// `/api/v1/category` answers with `rule.tab` (channels), `rule.dim` (tag dimensions) and a flat
/// `down_category[]` of tiles, each carrying the dimension it belongs to.
pub async fn category_front(tab: u32) -> Result<CategoryFront, String> {
    let json = api_get("/api/v1/category", &format!("category_tab={tab}")).await?;
    let data = json.get("data").cloned().unwrap_or(Value::Null);
    let rule = data.get("rule").cloned().unwrap_or(Value::Null);

    let mut tabs = Vec::new();
    for item in rule.get("tab").and_then(Value::as_array).into_iter().flatten() {
        let name = text_of(item, "name");
        let value = text_of(item, "value");
        if !name.is_empty() && !value.is_empty() {
            tabs.push(CategoryTab { name, value });
        }
    }

    let mut dims = Vec::new();
    for item in rule.get("dim").and_then(Value::as_array).into_iter().flatten() {
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

// ---------------------------------------------------------------------------------------------
// shelf mutations
// ---------------------------------------------------------------------------------------------

pub fn shelf_contains(book_id: &str) -> bool {
    load_shelf().iter().any(|entry| entry.id == book_id)
}

/// Adds a book and returns whether it is now on the shelf. Adding an existing book is a no-op.
pub fn add_shelf(entry: ShelfEntry) -> Result<bool, String> {
    if shelf_contains(&entry.id) {
        return Ok(true);
    }
    let mut shelf = load_shelf();
    shelf.retain(|existing| existing.id != entry.id);
    shelf.insert(0, entry);
    shelf.truncate(50);
    save_shelf(&shelf)?;
    Ok(true)
}

pub fn remove_shelf(book_id: &str) -> Result<(), String> {
    let mut shelf = load_shelf();
    shelf.retain(|entry| entry.id != book_id);
    save_shelf(&shelf)
}

/// Reading updates the stored position, but only for books already on the shelf — matching the
/// original client, where opening a chapter never silently subscribes you to a book.
pub fn update_progress(book_id: &str, item_id: &str, chapter_title: &str, index: usize) {
    let mut shelf = load_shelf();
    let Some(entry) = shelf.iter_mut().find(|entry| entry.id == book_id) else {
        return;
    };
    entry.item_id = item_id.to_string();
    entry.chapter_title = chapter_title.to_string();
    entry.index = index;
    if let Err(err) = save_shelf(&shelf) {
        eprintln!("update shelf: {err}");
    }
}

// ---------------------------------------------------------------------------------------------
// audiobooks
// ---------------------------------------------------------------------------------------------

/// Resolves the audio track of one chapter. The service answers with a play-info envelope whose
/// `data[]` carries `main_url`/`backup_url`; both point at the same mp3/m4a stream.
pub async fn audio_source(
    book_id: &str,
    item_id: &str,
    tone_id: &str,
) -> Result<(String, String), String> {
    // `NO_THIS_TONE` also shows up when the upstream is merely busy, so retry once before
    // declaring the voice unusable.
    let mut last_error = String::new();
    for attempt in 0..3 {
        match audio_source_once(book_id, item_id, tone_id).await {
            Ok(found) => return Ok(found),
            Err(err) => {
                last_error = err;
                if attempt < 2 {
                    // `NO_THIS_TONE` also appears when the upstream is busy; back off and retry.
                    tokio::time::sleep(std::time::Duration::from_millis(800 * (attempt as u64 + 1)))
                        .await;
                }
            }
        }
    }
    Err(last_error)
}

async fn audio_source_once(
    book_id: &str,
    item_id: &str,
    tone_id: &str,
) -> Result<(String, String), String> {
    let tone = if tone_id.trim().is_empty() {
        "0"
    } else {
        tone_id.trim()
    };
    let json = api_get(
        &format!(
            "/api/v1/audio/books/{}/chapters/{}",
            url_encode(book_id),
            url_encode(item_id)
        ),
        &format!("tone_id={tone}"),
    )
    .await?;

    let data = json.get("data").cloned().unwrap_or(Value::Null);
    let streams = data
        .as_array()
        .cloned()
        .or_else(|| data.get("data").and_then(Value::as_array).cloned())
        .unwrap_or_default();

    for stream in &streams {
        // The playable key lives inside the `video_model` JSON string, under
        // `video_list[].encrypt_info.key_hex` (`cenc-aes-ctr`). The sibling `encryption_key`
        // field is key material for the app's own player, not a CENC key.
        let model = match stream.get("video_model") {
            Some(Value::String(raw)) => serde_json::from_str::<Value>(raw).ok(),
            Some(value @ Value::Object(_)) => Some(value.clone()),
            _ => None,
        };
        if let Some(model) = model {
            for entry in model
                .get("video_list")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let key = entry
                    .get("encrypt_info")
                    .map(|info| text_of(info, "key_hex"))
                    .unwrap_or_default();
                let url = first_non_empty(&[
                    text_of(entry, "main_url"),
                    text_of(entry, "backup_url"),
                ]);
                if url.starts_with("http") {
                    return Ok((url, key));
                }
            }
        }

        let key = stream
            .get("encrypt_info")
            .map(|info| text_of(info, "key_hex"))
            .unwrap_or_default();
        let url = first_non_empty(&[
            text_of(stream, "main_url"),
            text_of(stream, "backup_url"),
        ]);
        if url.starts_with("http") {
            return Ok((url, key));
        }
    }

    Err(format!("没拿到音频地址（{}）", text_of(&json, "message")))
}

/// Tries the default voice first, then the book's 真人讲书 narrators: a pure-TTS book answers
/// `NO_THIS_TONE` for tone 0, and only the `abook_id` rows yield audio.
pub async fn audio_for_chapter(
    book_id: &str,
    item_id: &str,
) -> Result<(String, String, String), String> {
    // Order matters: the default voice first (it is what the original plays), then the book's
    // 真人讲书 narrators (`abook_id` rows), then tone 1, which some pure-TTS books answer.
    let mut candidates = vec!["0".to_string()];

    if let Ok(tones) = api_get(&format!("/api/v1/books/{}/tones", url_encode(book_id)), "").await {
        let data = tones.get("data").cloned().unwrap_or(Value::Null);
        for row in data
            .get("audio_tones")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let tone = text_of(row, "abook_id");
            if !tone.is_empty() && !candidates.contains(&tone) {
                candidates.push(tone);
            }
        }
        let recommended = data
            .get("recommend_tone")
            .map(|value| match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();
        if !recommended.is_empty() && recommended != "null" && !candidates.contains(&recommended) {
            candidates.push(recommended);
        }
    }
    candidates.push("1".to_string());

    let mut last_error = String::from("没有可用的朗读音色");
    for tone in candidates {
        match audio_source(book_id, item_id, &tone).await {
            Ok((url, key)) => return Ok((url, key, tone)),
            Err(err) => last_error = err,
        }
    }
    Err(format!("朗读失败：{last_error}"))
}

/// `/api/v1/books/{id}/comments` answers `data.comment[]` (`text`, `user_info.user_name`,
/// `create_timestamp`, `digg_count`, `reply_count`) plus `has_more`/`next_offset`.
#[derive(Clone, Debug)]
pub struct Comment {
    pub user: String,
    pub text: String,
    pub when: String,
    pub digg: String,
    pub replies: String,
}

pub async fn book_comments(
    book_id: &str,
    offset: usize,
) -> Result<(Vec<Comment>, Option<usize>), String> {
    let mut query = String::from("count=10");
    if offset > 0 {
        query.push_str(&format!("&offset={offset}"));
    }
    let json = api_get(&format!("/api/v1/books/{}/comments", url_encode(book_id)), &query).await?;
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

/// `create_timestamp` is seconds; render it as `YYYY-MM-DD` without pulling in a date crate
/// (days-to-civil, Howard Hinnant's algorithm).
fn format_timestamp(raw: &str) -> String {
    let Ok(seconds) = raw.parse::<i64>() else {
        return String::new();
    };
    let days = seconds.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------------------------------------
// manga
// ---------------------------------------------------------------------------------------------

/// Turns a `/src/...` path the backend serves into a `file://` URI the image loader can read.
pub fn served_uri(path: &str) -> String {
    if path.starts_with("/src/") {
        let local = data_dir().join("run").join("src").join(&path[5..]);
        format!("file://{}", local.display())
    } else if path.starts_with("http") {
        path.to_string()
    } else {
        format!("file://{}", path)
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

// ---------------------------------------------------------------------------------------------
// rank boards
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct RankBoard {
    pub name: String,
    pub algo: String,
}

#[derive(Clone, Debug)]
pub struct RankCategory {
    pub name: String,
    pub id: String,
}

#[derive(Clone, Debug)]
pub struct RankCatalog {
    pub rank_id: String,
    pub boards: Vec<RankBoard>,
    pub categories: Vec<RankCategory>,
}

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
    let mut push_book = |book: &Value, hits: &mut Vec<Hit>| {
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
            cover: first_non_empty(&[
                text_of(book, "thumb_url"),
                text_of(book, "horiz_thumb_url"),
            ]),
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

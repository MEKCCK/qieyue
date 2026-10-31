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
    let response = fqapi_core::api::request(
        String::new(),
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

pub async fn open_book(id: &str) -> Result<(String, String, String, Vec<Chapter>), String> {
    let detail = api_get(&format!("/api/v1/books/{}/detail", url_encode(id)), "").await?;
    let data = detail.get("data").cloned().unwrap_or(Value::Null);
    let title = text_of(&data, "book_name");
    let author = text_of(&data, "author");
    let summary = first_non_empty(&[text_of(&data, "abstract"), text_of(&data, "book_abstract_v2")]);

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
    Ok((title, author, summary, chapters))
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


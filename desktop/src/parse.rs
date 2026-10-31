//! JSON and HTML parsing helpers shared by the endpoint modules.

use crate::models::Hit;
use serde_json::Value;

pub(crate) fn text_of(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn first_non_empty(candidates: &[String]) -> String {
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
                    if matches!(
                        name.as_str(),
                        "p" | "br" | "div" | "section" | "h1" | "h2" | "h3" | "li"
                    ) {
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
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => {
                entity[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
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

/// `create_timestamp` is seconds; render it as `YYYY-MM-DD` without pulling in a date crate
/// (days-to-civil, Howard Hinnant's algorithm).
pub(crate) fn format_timestamp(raw: &str) -> String {
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

/// Every search hit that carries a `video_data` entry is a short drama; the id we need for
/// the series endpoint is `series_id`, not the `book_id` the hit also carries.
pub(crate) fn series_from_hit(value: &Value) -> Option<(String, String, String)> {
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

/// One card from a feed cell, search result or shelf row.
pub(crate) fn hit_from_item(item: &Value) -> Option<Hit> {
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
        cover: first_non_empty(&[text_of(book, "thumb_url"), text_of(book, "horiz_thumb_url")]),
        series: None,
    })
}

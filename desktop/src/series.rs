//! Short dramas: series detail and episode resolution.

use serde_json::Value;

use crate::backend::api_get;
use crate::models::{Episode, Series};
use crate::parse::{first_non_empty, text_of, url_encode};
use crate::paths;

pub async fn open_series(id: &str) -> Result<Series, String> {
    let json = api_get(&format!("/api/v1/series/{}", url_encode(id)), "").await?;
    let data = json
        .get("data")
        .and_then(|d| d.get("video_data"))
        .cloned()
        .unwrap_or(Value::Null);

    let title = text_of(&data, "series_title");
    let intro = text_of(&data, "series_intro");
    // `series_cover` is often missing; the episode rows still carry artwork.
    let mut cover = text_of(&data, "series_cover");
    if cover.is_empty() {
        cover = data
            .get("video_list")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|item| {
                first_non_empty(&[
                    text_of(item, "episode_cover"),
                    text_of(item, "cover"),
                    text_of(item, "horiz_cover"),
                ])
            })
            .find(|value| !value.is_empty())
            .unwrap_or_default();
    }

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
        paths::data_dir().join("run").join("src").join(relative)
    } else {
        std::path::PathBuf::from(served)
    };

    if !local.exists() {
        return Err(format!("视频文件不在预期位置: {}", local.display()));
    }
    Ok((local.display().to_string(), key))
}

/// One quality tier of an episode, as returned by the stream endpoint.
#[derive(Clone, Debug, serde::Serialize)]
pub struct VideoVariant {
    pub name: String,
    pub height: u32,
    pub url: String,
    pub key: String,
}

/// Quality tiers for one episode.
///
/// The download path always picks whatever the backend considers best (it has no quality
/// parameter), so switching quality means streaming the chosen variant URL straight into mpv
/// with its own `cenc-aes-ctr` key. The top-level `video_url` is repeated as 默认 when the
/// variants list is empty.
pub async fn episode_variants(vid: &str) -> Result<Vec<VideoVariant>, String> {
    let stream = api_get(
        &format!("/api/v1/videos/{}", url_encode(vid)),
        "mode=stream",
    )
    .await?;

    let mut out: Vec<VideoVariant> = Vec::new();

    for entry in stream
        .get("variants")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = text_of(entry, "name");
        let height = text_of(entry, "height").parse::<u32>().unwrap_or(0);
        push_variant(
            &mut out,
            if name.is_empty() {
                format!("{height}P")
            } else {
                name
            },
            height,
            text_of(entry, "url"),
            text_of(entry, "key_hex"),
        );
    }

    if out.is_empty() {
        push_variant(
            &mut out,
            "默认".to_string(),
            0,
            text_of(&stream, "video_url"),
            text_of(&stream, "key_hex"),
        );
    }

    if out.is_empty() {
        return Err("这一集没有可用的清晰度".to_string());
    }
    Ok(out)
}

/// Adds a tier unless the URL is already listed or is not a playable stream.
fn push_variant(out: &mut Vec<VideoVariant>, name: String, height: u32, url: String, key: String) {
    if !url.starts_with("http") || out.iter().any(|existing| existing.url == url) {
        return;
    }
    out.push(VideoVariant {
        name,
        height,
        url,
        key,
    });
}

//! Audiobook track resolution for mpv.

use serde_json::Value;

use crate::backend::api_get;
use crate::parse::{first_non_empty, text_of, url_encode};

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
                    tokio::time::sleep(std::time::Duration::from_millis(
                        800 * (attempt as u64 + 1),
                    ))
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
                let url =
                    first_non_empty(&[text_of(entry, "main_url"), text_of(entry, "backup_url")]);
                if url.starts_with("http") {
                    return Ok((url, key));
                }
            }
        }

        let key = stream
            .get("encrypt_info")
            .map(|info| text_of(info, "key_hex"))
            .unwrap_or_default();
        let url = first_non_empty(&[text_of(stream, "main_url"), text_of(stream, "backup_url")]);
        if url.starts_with("http") {
            return Ok((url, key));
        }
    }

    Err(format!("没拿到音频地址（{}）", text_of(&json, "message")))
}

/// Tries the default voice first, then the book's 真人讲书 narrators: a pure-TTS book answers
/// `NO_THIS_TONE` for tone 0, and only the `abook_id` rows yield audio.
/// One selectable voice. `id` is what `tone_id` expects: `0` for 智能朗读,
/// a narrator's `abook_id` for 真人讲书.
#[derive(Clone, Debug, serde::Serialize)]
pub struct AudioTone {
    pub id: String,
    pub name: String,
}

pub async fn audio_tones(book_id: &str) -> Vec<AudioTone> {
    let mut tones = vec![AudioTone {
        id: "0".to_string(),
        name: "智能朗读".to_string(),
    }];

    let Ok(json) = api_get(&format!("/api/v1/books/{}/tones", url_encode(book_id)), "").await
    else {
        return tones;
    };
    let data = json.get("data").cloned().unwrap_or(Value::Null);
    for row in data
        .get("audio_tones")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = text_of(row, "abook_id");
        if id.is_empty() || tones.iter().any(|tone| tone.id == id) {
            continue;
        }
        let name = text_of(row, "title");
        tones.push(AudioTone {
            id,
            name: if name.is_empty() {
                "真人讲书".to_string()
            } else {
                name
            },
        });
    }
    tones
}

/// Like [`audio_for_chapter`], but tries the user's pick first.
pub async fn audio_for_chapter_with(
    book_id: &str,
    item_id: &str,
    tone_id: &str,
) -> Result<(String, String, String), String> {
    if !tone_id.trim().is_empty() {
        if let Ok((url, key)) = audio_source(book_id, item_id, tone_id).await {
            return Ok((url, key, tone_id.trim().to_string()));
        }
    }
    audio_for_chapter(book_id, item_id).await
}

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

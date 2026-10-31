//! Shelf persistence and reading progress (`shelf.json` in the data dir).

use crate::models::{Episode, Series, ShelfEntry};
use crate::paths;

pub fn load_shelf() -> Vec<ShelfEntry> {
    let path = paths::data_dir().join("shelf.json");
    std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save_shelf(entries: &[ShelfEntry]) -> Result<(), String> {
    let dir = paths::data_dir();
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

/// Shelf row for a short drama: `item_id` holds the episode to resume at.
pub fn series_shelf_entry(series: &Series, progress: Option<&Episode>) -> ShelfEntry {
    ShelfEntry {
        id: series.id.clone(),
        title: series.title.clone(),
        author: String::new(),
        cover: series.cover.clone(),
        kind: "series".to_string(),
        item_id: progress
            .map(|episode| episode.vid.clone())
            .unwrap_or_default(),
        chapter_title: progress
            .map(|episode| format!("第 {} 集", episode.index))
            .unwrap_or_else(|| format!("共 {} 集", series.episodes.len())),
        index: progress
            .map(|episode| episode.index.saturating_sub(1))
            .unwrap_or(0),
    }
}

//! IPC surface for the webview.
//!
//! These wrappers are deliberately thin: the parsing, crypto and caching live
//! in `qieyue` (the backend adapter over `fqapi_core`), so the frontend never
//! sees protocol details.

use qieyue::{BookDetail, CategoryFront, Hit, Home, RankCatalog, SearchPage, Series, ShelfEntry};

#[tauri::command(rename_all = "snake_case")]
pub async fn backend_ready() -> Result<(), String> {
    qieyue::init_backend().await
}

/// Bridges webview errors to the terminal in debug builds; harmless in release.
#[tauri::command(rename_all = "snake_case")]
pub fn debug_log(message: String) {
    eprintln!("[webview] {message}");
}

#[tauri::command(rename_all = "snake_case")]
pub async fn home() -> Result<Home, String> {
    qieyue::home().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn home_channel(
    tab_type: u32,
    tab_index: u32,
    client_template: String,
) -> Result<Home, String> {
    qieyue::home_channel(tab_type, tab_index, &client_template).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn home_more(
    cell_id: String,
    tab_type: u32,
    client_template: String,
    offset: usize,
) -> Result<(Vec<Hit>, Option<usize>), String> {
    qieyue::home_more(&cell_id, tab_type, &client_template, offset).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn search_page(
    query: String,
    offset: usize,
    tab_type: Option<u32>,
) -> Result<SearchPage, String> {
    qieyue::search_page(&query, offset, tab_type).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn open_book(id: String) -> Result<BookDetail, String> {
    qieyue::open_book(&id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn read_chapter(item_id: String) -> Result<(String, Vec<String>), String> {
    qieyue::read_chapter(&item_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn open_series(id: String) -> Result<Series, String> {
    qieyue::open_series(&id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn manga_chapter(item_id: String) -> Result<Vec<String>, String> {
    qieyue::manga_chapter(&item_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn category_front(tab: u32) -> Result<CategoryFront, String> {
    qieyue::category_front(tab).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn rank_catalog() -> Result<RankCatalog, String> {
    qieyue::rank_catalog().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn rank_page(
    rank_id: String,
    algo: String,
    category_id: String,
    offset: usize,
) -> Result<Vec<Hit>, String> {
    qieyue::rank_page(&rank_id, &algo, &category_id, offset).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn book_comments(
    book_id: String,
    offset: usize,
) -> Result<(Vec<qieyue::Comment>, Option<usize>), String> {
    qieyue::book_comments(&book_id, offset).await
}

#[tauri::command(rename_all = "snake_case")]
pub fn load_shelf() -> Vec<ShelfEntry> {
    qieyue::load_shelf()
}

#[tauri::command(rename_all = "snake_case")]
pub fn add_shelf(entry: ShelfEntry) -> Result<bool, String> {
    qieyue::add_shelf(entry)
}

#[tauri::command(rename_all = "snake_case")]
pub fn remove_shelf(id: String) -> Result<(), String> {
    qieyue::remove_shelf(&id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn update_progress(book_id: String, item_id: String, chapter_title: String, index: usize) {
    qieyue::update_progress(&book_id, &item_id, &chapter_title, index);
}

/// Resolves the episode and hands it to mpv. The downloaded file is still CENC
/// encrypted, so the key is passed as `decryption_key`.
#[tauri::command(rename_all = "snake_case")]
pub async fn play_episode(vid: String, title: String) -> Result<(), String> {
    let (path, key) = qieyue::resolve_episode(&vid).await?;
    qieyue::spawn_mpv(&path, &key, &title)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn play_audio(book_id: String, item_id: String, title: String) -> Result<String, String> {
    let (url, key, tone) = qieyue::audio_for_chapter(&book_id, &item_id).await?;
    qieyue::spawn_mpv_audio(&url, &key, &title)?;
    Ok(tone)
}

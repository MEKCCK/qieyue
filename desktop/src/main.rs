//! Tauri v2 entry point.
//!
//! The webview frontend lives in `ui/` and is embedded at compile time. Every
//! backend call is a `#[tauri::command]` in `commands.rs`; the protocol/crypto
//! work stays in `fqapi_core`, exactly as before.

mod commands;

fn main() {
    tauri::Builder::default()
        .setup(|_app| {
            // Warm the core while the webview paints its first frame. The
            // frontend also awaits `backend_ready`, which is idempotent.
            tauri::async_runtime::spawn(async {
                if let Err(err) = qieyue::init_backend().await {
                    eprintln!("backend init failed: {err}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::backend_ready,
            commands::debug_log,
            commands::home,
            commands::home_channel,
            commands::home_more,
            commands::search_page,
            commands::open_book,
            commands::read_chapter,
            commands::open_series,
            commands::manga_chapter,
            commands::category_front,
            commands::rank_catalog,
            commands::rank_page,
            commands::book_comments,
            commands::load_shelf,
            commands::add_shelf,
            commands::remove_shelf,
            commands::update_progress,
            commands::play_episode,
            commands::episode_variants,
            commands::play_variant,
            commands::play_audio,
        ])
        .run(tauri::generate_context!())
        .expect("error while running qieyue");
}

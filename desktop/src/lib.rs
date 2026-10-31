//! Backend adapter shared by the GUI, the smoke test and the headless daemon.
//!
//! Everything here talks to `fqapi_core`, which owns the protocol, crypto and
//! cache. The modules return exactly what a view needs, already parsed out of
//! the service JSON; no GUI type leaks into this layer.
//!
//! `rust/` is a pristine copy of the upstream core (see `scripts/sync-backend.sh`);
//! all project code lives here and in the binary modules, never there.

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("qieyue only supports Linux and macOS");

mod audio;
mod backend;
mod book;
mod category;
mod comments;
mod home;
mod manga;
mod models;
mod parse;
mod paths;
mod rank;
mod search;
mod series;
mod shelf;

pub use audio::audio_for_chapter;
pub use backend::{api_get, init_backend};
pub use book::{open_book, read_chapter};
pub use category::category_front;
pub use comments::{book_comments, series_comments};
pub use home::{home, home_channel, home_more, series_feed, series_feed_page};
pub use manga::{manga_chapter, served_uri};
pub use models::*;
pub use parse::{html_to_paragraphs, url_encode};
pub use paths::data_dir;
pub use rank::{rank_catalog, rank_page};
pub use search::{search, search_hits, search_page};
pub use series::{open_series, resolve_episode};
pub use shelf::{
    add_shelf, load_shelf, remember, remove_shelf, save_shelf, series_shelf_entry, shelf_contains,
    update_progress,
};

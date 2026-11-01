//! Data shapes the backend adapter hands to the views.
//!
//! They are deliberately flat: every field is already parsed out of the
//! service JSON, so a view never touches `serde_json::Value`.

/// A search hit that is a plain book (novel, audiobook, manga).
#[derive(Clone, Debug, serde::Serialize)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    pub summary: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Chapter {
    pub item_id: String,
    pub title: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct BookDetail {
    pub title: String,
    pub author: String,
    pub summary: String,
    pub cover: String,
    /// Comics answer `comic_book_type = 1`; their chapters are images, not text.
    pub is_comic: bool,
    pub chapters: Vec<Chapter>,
}

/// One row of the search result list: either a book or a short drama.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Hit {
    pub id: String,
    pub title: String,
    pub author: String,
    pub summary: String,
    pub cover: String,
    /// `Some(series_id)` marks a short drama; `id` is then the series id as well.
    pub series: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SearchPage {
    pub hits: Vec<Hit>,
    /// Offset to request next, or `None` when the service reports no more rows.
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Series {
    pub id: String,
    pub title: String,
    pub intro: String,
    pub cover: String,
    pub episodes: Vec<Episode>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Episode {
    pub vid: String,
    pub title: String,
    pub index: usize,
    pub cover: String,
    pub duration: String,
}

/// One shelf row. `kind` is `book` or `series`; a series row resumes at an
/// episode, a book row at a chapter.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ShelfEntry {
    pub id: String,
    pub title: String,
    pub author: String,
    /// Cover URL, so the shelf page renders a real grid like the original.
    #[serde(default)]
    pub cover: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub item_id: String,
    pub chapter_title: String,
    pub index: usize,
}

fn default_kind() -> String {
    "book".to_string()
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Comment {
    pub user: String,
    pub text: String,
    pub when: String,
    pub digg: String,
    pub replies: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct CategoryTab {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct CategoryDim {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct CategoryTile {
    pub name: String,
    pub dim: String,
    pub hot: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct CategoryFront {
    pub tabs: Vec<CategoryTab>,
    pub dims: Vec<CategoryDim>,
    pub tiles: Vec<CategoryTile>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RankBoard {
    pub name: String,
    pub algo: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RankCategory {
    pub name: String,
    pub id: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RankCatalog {
    pub rank_id: String,
    pub boards: Vec<RankBoard>,
    pub categories: Vec<RankCategory>,
}

#[derive(Clone, Debug, serde::Serialize)]
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

#[derive(Clone, Debug, serde::Serialize)]
pub struct HomeTab {
    pub title: String,
    pub tab_type: u32,
    /// Index of this channel inside the response (`tab_index` on the wire).
    pub index: u32,
    pub client_template: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Home {
    /// Top channels of the discovery page (`推荐`, `小说`, `听书`, …).
    pub tabs: Vec<HomeTab>,
    pub sections: Vec<HomeSection>,
    /// The channel this payload describes; the view highlights this, not whatever was clicked.
    pub active: u32,
}

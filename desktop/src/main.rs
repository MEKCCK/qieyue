//! Native Linux front end for the `fqapi_core` backend.
//!
//! The backend owns all protocol, crypto and caching work and exposes a local REST surface
//! (`/api/v1/...`). This binary links it as a library and drives it through
//! `fqapi_core::api::request`, so no HTTP server or Flutter runtime is involved. Short dramas
//! are handed to the system `mpv` together with the CENC key the backend resolves.

use std::sync::mpsc::{channel, Receiver, Sender};

use eframe::egui;
use egui::{Color32, RichText};
use fqapp_desktop::{
    add_shelf, category_front, home, init_backend, load_shelf, open_book, open_series,
    read_chapter, remove_shelf, resolve_episode, shelf_contains, update_progress,
    CategoryFront, Chapter, Hit, Home, ShelfEntry, Series,
};
use tokio::sync::mpsc as tokio_mpsc;

#[derive(Clone, Debug)]
struct BookInfo {
    id: String,
    title: String,
    author: String,
    summary: String,
    cover: String,
    is_comic: bool,
}

enum Cmd {
    Search { query: String, offset: usize },
    OpenBook(String),
    OpenSeries { id: String, cover: String },
    OpenChapter(String),
    PlayEpisode { vid: String, title: String },
    PlayAudio { book_id: String, item_id: String, title: String },
    OpenManga(String),
    Category(u32),
    RankCatalog,
    Comments { book_id: String, offset: usize },
    RankPage { rank_id: String, algo: String, category: String, offset: usize },
    Home,
    HomeChannel {
        tab_type: u32,
        tab_index: u32,
        client_template: String,
    },
    HomeMore {
        section: usize,
        cell_id: String,
        tab_type: u32,
        client_template: String,
        offset: usize,
    },
}

enum Msg {
    Hits {
        hits: Vec<Hit>,
        next: Option<usize>,
        append: bool,
    },
    Detail {
        id: String,
        title: String,
        author: String,
        summary: String,
        cover: String,
        is_comic: bool,
        chapters: Vec<Chapter>,
    },
    Series(Series),
    Chapter {
        title: String,
        paragraphs: Vec<String>,
    },
    Played(String),
    Category(CategoryFront),
    Home(Home),
    RankCatalog(fqapp_desktop::RankCatalog),
    RankPage { hits: Vec<Hit>, append: bool },
    HomeMoreFailed { section: usize },
    Comments { rows: Vec<fqapp_desktop::Comment>, next: Option<usize>, append: bool },
    Manga { pages: Vec<String> },
    HomeAppend {
        section: usize,
        hits: Vec<Hit>,
        next: Option<usize>,
    },
    Error(String),
}

/// Drives the backend: one command at a time, results posted back as messages.
async fn worker(mut cmds: tokio_mpsc::UnboundedReceiver<Cmd>, msgs: Sender<Msg>) {
    if let Err(err) = init_backend().await {
        let _ = msgs.send(Msg::Error(format!("backend init failed: {err}")));
        return;
    }

    while let Some(cmd) = cmds.recv().await {
        let message = match cmd {
            Cmd::Search { query, offset } => {
                match fqapp_desktop::search_page(&query, offset, None).await {
                    Ok(page) => Msg::Hits {
                        hits: page.hits,
                        next: page.next_offset,
                        append: offset > 0,
                    },
                    Err(err) => Msg::Error(err),
                }
            }
            Cmd::OpenBook(id) => match open_book(&id).await {
                Ok(detail) => Msg::Detail {
                    id,
                    title: detail.title,
                    author: detail.author,
                    summary: detail.summary,
                    cover: detail.cover,
                    is_comic: detail.is_comic,
                    chapters: detail.chapters,
                },
                Err(err) => Msg::Error(err),
            },
            Cmd::OpenSeries { id, cover } => match open_series(&id).await {
                Ok(mut series) => {
                    // The series endpoint often omits the artwork the card already had.
                    if series.cover.is_empty() {
                        series.cover = cover;
                    }
                    Msg::Series(series)
                }
                Err(err) => Msg::Error(err),
            },
            Cmd::OpenChapter(item_id) => match read_chapter(&item_id).await {
                Ok((title, paragraphs)) => Msg::Chapter { title, paragraphs },
                Err(err) => Msg::Error(err),
            },
            Cmd::PlayAudio {
                book_id,
                item_id,
                title,
            } => match fqapp_desktop::audio_for_chapter(&book_id, &item_id).await {
                Ok((url, key, tone)) => match spawn_mpv_audio(&url, &key, &title) {
                    Ok(()) => Msg::Played(format!("mpv 正在朗读: {title}（音色 {tone}）")),
                    Err(err) => Msg::Error(err),
                },
                Err(err) => Msg::Error(err),
            },
            Cmd::OpenManga(item_id) => match fqapp_desktop::manga_chapter(&item_id).await {
                Ok(pages) => Msg::Manga { pages },
                Err(err) => Msg::Error(err),
            },
            Cmd::Comments { book_id, offset } => {
                match fqapp_desktop::book_comments(&book_id, offset).await {
                    Ok((rows, next)) => Msg::Comments {
                        rows,
                        next,
                        append: offset > 0,
                    },
                    Err(err) => Msg::Error(err),
                }
            }
            Cmd::RankCatalog => match fqapp_desktop::rank_catalog().await {
                Ok(catalog) => Msg::RankCatalog(catalog),
                Err(err) => Msg::Error(err),
            },
            Cmd::RankPage {
                rank_id,
                algo,
                category,
                offset,
            } => {
                match fqapp_desktop::rank_page(&rank_id, &algo, &category, offset).await {
                    Ok(hits) => Msg::RankPage {
                        hits,
                        append: offset > 0,
                    },
                    Err(err) => Msg::Error(err),
                }
            }
            Cmd::Home => match home().await {
                Ok(home) => Msg::Home(home),
                Err(err) => Msg::Error(err),
            },
            Cmd::HomeChannel {
                tab_type,
                tab_index,
                client_template,
            } => match fqapp_desktop::home_channel(tab_type, tab_index, &client_template).await {
                Ok(home) => Msg::Home(home),
                Err(err) => Msg::Error(err),
            },
            Cmd::HomeMore {
                section,
                cell_id,
                tab_type,
                client_template,
                offset,
            } => match fqapp_desktop::home_more(&cell_id, tab_type, &client_template, offset).await {
                Ok((hits, next)) => Msg::HomeAppend {
                    section,
                    hits,
                    next,
                },
                Err(err) => {
                    eprintln!("home_more: {err}");
                    Msg::HomeMoreFailed { section }
                }
            },
            Cmd::Category(tab) => match category_front(tab).await {
                Ok(front) => Msg::Category(front),
                Err(err) => Msg::Error(err),
            },
            Cmd::PlayEpisode { vid, title } => match resolve_episode(&vid).await {
                Ok((path, key)) => match spawn_mpv(&path, &key, &title) {
                    Ok(()) => Msg::Played(format!("mpv 已启动: {title}")),
                    Err(err) => Msg::Error(err),
                },
                Err(err) => Msg::Error(err),
            },
        };
        if msgs.send(message).is_err() {
            break;
        }
    }
}

fn spawn_mpv_audio(url: &str, key: &str, title: &str) -> Result<(), String> {
    let mut command = std::process::Command::new("mpv");
    if !key.is_empty() {
        // Audio tracks are `cenc-aes-ctr` too; without the key mpv reports
        // "Reserved bit set" and gives up.
        command.arg(format!("--demuxer-lavf-o=decryption_key={key}"));
    }
    command
        .arg("--no-video")
        .arg(format!("--title={title}"))
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动 mpv 失败（PATH 里有 mpv 吗？）: {e}"))
}

/// The downloaded episode is still CENC encrypted, so mpv needs the key as well.
fn spawn_mpv(path: &str, key: &str, title: &str) -> Result<(), String> {
    let mut command = std::process::Command::new("mpv");
    if !key.is_empty() {
        command.arg(format!("--demuxer-lavf-o=decryption_key={key}"));
    }
    command
        .arg(format!("--title={title}"))
        .arg("--force-window=yes")
        .arg(path);
    command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("启动 mpv 失败（PATH 里有 mpv 吗？）: {e}"))
}

// ---------------------------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------------------------

const CJK_FONTS: &[&str] = &[
    "/usr/share/fonts/TTF/JetBrains-Maple-Mono-NF-XX-XX/JetBrainsMapleMono-Regular.ttf",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
];

fn install_fonts(ctx: &egui::Context) {
    let Some(bytes) = CJK_FONTS.iter().find_map(|path| std::fs::read(path).ok()) else {
        eprintln!("no CJK font found, Chinese text will not render");
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "cjk".to_owned());
    }
    ctx.set_fonts(fonts);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Empty,
    Home,
    Library,
    Manga,
    Rank,
    Results,
    Book,
    Series,
    Reader,
    Category,
}

struct App {
    cmds: tokio_mpsc::UnboundedSender<Cmd>,
    msgs: Receiver<Msg>,
    query: String,
    status: String,
    loading: bool,

    view: View,
    history: Vec<View>,
    hits: Vec<Hit>,
    search_query: String,
    search_next: Option<usize>,
    book: Option<BookInfo>,
    chapters: Vec<Chapter>,
    chapter_index: usize,
    chapter_title: String,
    paragraphs: Vec<String>,
    reader_pos: f32,
    series: Option<Series>,
    rank: Option<fqapp_desktop::RankCatalog>,
    rank_id: String,
    rank_algo: String,
    rank_category: String,
    rank_hits: Vec<Hit>,
    rank_more: bool,
    comments: Vec<fqapp_desktop::Comment>,
    comments_next: Option<usize>,
    manga_pages: Vec<String>,
    manga_textures: Vec<egui::TextureHandle>,
    manga_pos: f32,
    home: Option<Home>,
    home_tab_index: u32,
    home_loading_more: bool,
    /// Scroll offsets from the previous frame, per list. Auto-paging triggers on a change, so a
    /// restored offset or a first layout pass never starts paging on its own.
    home_offset: f32,
    results_offset: f32,
    rank_offset: f32,
    home_requested: bool,
    category: Option<CategoryFront>,
    category_tab: u32,
    shelf: Vec<ShelfEntry>,
    /// Set when a shelf entry is opened: the chapter to jump to once the directory arrives.
    pending_item: Option<String>,

    /// Optional scripted walkthrough used to screenshot each view without a human clicking:
    /// `FQAPP_DEMO_QUERY=斗破 FQAPP_DEMO_STAGE=3` searches, opens the first book and its first
    /// chapter. Stage 1 stops at the result list, 2 at the book page, 3 in the reader.
    demo: Option<(String, u8)>,
    demo_done: u8,
    /// `FQAPP_DEMO_MANGA=<book id>` opens that comic's first chapter once its detail arrives.
    demo_manga_stage: u8,
    /// `FQAPP_DEMO_CHANNEL=5:4:12` switches to that home channel once the first page arrives.
    demo_channel: Option<(u32, u32, String)>,
}

impl App {
    fn new(cmds: tokio_mpsc::UnboundedSender<Cmd>, msgs: Receiver<Msg>) -> Self {
        Self {
            cmds,
            msgs,
            query: String::new(),
            status: "输入关键词开始搜索".to_string(),
            loading: false,
            view: View::Empty,
            history: Vec::new(),
            hits: Vec::new(),
            search_query: String::new(),
            search_next: None,
            book: None,
            chapters: Vec::new(),
            chapter_index: 0,
            chapter_title: String::new(),
            paragraphs: Vec::new(),
            reader_pos: 0.0,
            series: None,
            rank: None,
            rank_id: String::new(),
            rank_algo: "101".to_string(),
            rank_category: "0".to_string(),
            rank_hits: Vec::new(),
            rank_more: true,
            comments: Vec::new(),
            comments_next: None,
            manga_pages: Vec::new(),
            manga_textures: Vec::new(),
            manga_pos: 0.0,
            home: None,
            home_tab_index: 0,
            home_loading_more: false,
            home_offset: -1.0,
            results_offset: -1.0,
            rank_offset: -1.0,
            home_requested: false,
            category: None,
            category_tab: 1,
            shelf: load_shelf(),
            pending_item: None,
            demo: std::env::var("FQAPP_DEMO_QUERY").ok().map(|query| {
                let stage = std::env::var("FQAPP_DEMO_STAGE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(3);
                (query, stage)
            }),
            demo_done: 0,
            demo_manga_stage: 0,
            demo_channel: std::env::var("FQAPP_DEMO_CHANNEL").ok().and_then(|value| {
                let mut parts = value.split(':');
                let tab_type = parts.next()?.parse().ok()?;
                let index = parts.next()?.parse().ok()?;
                let template = parts.next().unwrap_or("0").to_string();
                Some((tab_type, index, template))
            }),
        }
    }

    /// Navigates and remembers where we came from, so 返回 always has somewhere to go.
    fn go(&mut self, view: View) {
        if view != self.view {
            self.history.push(self.view);
            self.view = view;
        }
    }

    fn go_back(&mut self) {
        self.view = self.history.pop().unwrap_or(View::Home);
    }

    fn send(&mut self, cmd: Cmd, status: &str) {
        self.loading = true;
        self.status = status.to_string();
        if self.cmds.send(cmd).is_err() {
            self.status = "后台线程已退出".to_string();
            self.loading = false;
        }
    }

    fn poll(&mut self) {
        while let Ok(msg) = self.msgs.try_recv() {
            self.loading = false;
            match msg {
                Msg::Hits { hits, next, append } => {
                    if append {
                        for hit in hits {
                            if !self.hits.iter().any(|existing| existing.id == hit.id) {
                                self.hits.push(hit);
                            }
                        }
                    } else {
                        self.hits = hits;
                    }
                    self.search_next = next;
                    let dramas = self.hits.iter().filter(|hit| hit.series.is_some()).count();
                    self.status = format!(
                        "{} 条结果（短剧 {}）{}",
                        self.hits.len(),
                        dramas,
                        if self.search_next.is_some() {
                            " · 继续下滑加载更多"
                        } else {
                            ""
                        }
                    );
                    if !append {
                        self.go(View::Results);
                    }
                }
                Msg::Detail {
                    id,
                    title,
                    author,
                    summary,
                    cover,
                    is_comic,
                    chapters,
                } => {
                    self.status = format!("{} · {} 章", title, chapters.len());
                    self.book = Some(BookInfo {
                        id,
                        title,
                        author,
                        summary,
                        cover,
                        is_comic,
                    });
                    self.chapters = chapters;
                    self.comments.clear();
                    self.comments_next = None;
                    if !is_comic {
                        let book_id = self.book.as_ref().map(|b| b.id.clone()).unwrap_or_default();
                        self.send(Cmd::Comments { book_id, offset: 0 }, "加载评论…");
                    }
                    self.go(View::Book);
                    if self.demo_manga_stage == 0 {
                        if let Ok(series_id) = std::env::var("FQAPP_DEMO_SERIES") {
            if !self.home_requested && !self.loading {
                self.home_requested = true;
                self.send(
                    Cmd::OpenSeries {
                        id: series_id,
                        cover: String::new(),
                    },
                    "加载剧集…",
                );
            }
        } else if let Ok(book) = std::env::var("FQAPP_DEMO_MANGA") {
                            if book == self.book.as_ref().map(|b| b.id.clone()).unwrap_or_default() {
                                if let Some(chapter) = self.chapters.first().cloned() {
                                    self.demo_manga_stage = 1;
                                    self.send(Cmd::OpenManga(chapter.item_id), "解密漫画…");
                                }
                            }
                        }
                    }
                    if let Some(item) = self.pending_item.take() {
                        let index = self
                            .chapters
                            .iter()
                            .position(|chapter| chapter.item_id == item)
                            .unwrap_or(0);
                        self.open_chapter(index);
                    }
                }
                Msg::Series(series) => {
                    self.status = format!("{} · {} 集", series.title, series.episodes.len());
                    self.series = Some(series);
                    self.go(View::Series);
                }
                Msg::Chapter { title, paragraphs } => {
                    self.chapter_title = title.clone();
                    self.paragraphs = paragraphs;
                    self.reader_pos = 0.0;
                    self.go(View::Reader);
                    self.status = format!(
                        "{} / {}",
                        self.chapter_index + 1,
                        self.chapters.len().max(1)
                    );
                    // Only books already on the shelf get their position updated; adding is an
                    // explicit action on the detail page or in the reader toolbar.
                    if let Some(book) = self.book.clone() {
                        if shelf_contains(&book.id) {
                            if let Some(chapter) = self.chapters.get(self.chapter_index).cloned() {
                                update_progress(
                                    &book.id,
                                    &chapter.item_id,
                                    &title,
                                    self.chapter_index,
                                );
                                self.shelf = load_shelf();
                            }
                        }
                    }
                }
                Msg::Category(front) => {
                    self.status = format!(
                        "{} 个标签 · {} 个频道",
                        front.tiles.len(),
                        front.tabs.len()
                    );
                    self.category = Some(front);
                    self.go(View::Category);
                }
                Msg::Home(home) => {
                    self.home_tab_index = home.active;
                    if let Some((tab_type, tab_index, client_template)) = self.demo_channel.take() {
                        self.home_tab_index = tab_index;
                        self.send(
                            Cmd::HomeChannel {
                                tab_type,
                                tab_index,
                                client_template,
                            },
                            "加载频道…",
                        );
                    }
                    self.status = format!(
                        "{} 个推荐区块 · {} 张卡片",
                        home.sections.len(),
                        home.sections.iter().map(|s| s.hits.len()).sum::<usize>()
                    );
                    self.home = Some(home);
                    self.go(View::Home);
                }
                Msg::HomeAppend {
                    section,
                    hits,
                    next,
                } => {
                    if let Some(home) = self.home.as_mut() {
                        if let Some(target) = home.sections.get_mut(section) {
                            for hit in hits {
                                if !target.hits.iter().any(|existing| existing.id == hit.id) {
                                    target.hits.push(hit);
                                }
                            }
                            // Hard cap: past this the block is a performance hazard (every card
                            // keeps a texture) and the user cannot meaningfully browse it.
                            target.next_offset = if target.hits.len() >= 240 { None } else { next };
                            let total: usize = home.sections.iter().map(|s| s.hits.len()).sum();
                            self.status = format!("已加载 {total} 张卡片");
                        }
                    }
                    self.home_loading_more = false;
                }
                Msg::Manga { pages } => {
                    self.status = format!("{} 页", pages.len());
                    self.manga_pages = pages;
                    self.manga_textures.clear();
                    self.manga_pos = 0.0;
                    self.go(View::Manga);
                }
                Msg::RankCatalog(catalog) => {
                    self.rank_id = catalog.rank_id.clone();
                    self.status = format!(
                        "{} 个榜单 · {} 个分类",
                        catalog.boards.len(),
                        catalog.categories.len()
                    );
                    self.rank = Some(catalog);
                    self.go(View::Rank);
                }
                Msg::RankPage { hits, append } => {
                    let fresh = if append {
                        let before = self.rank_hits.len();
                        for hit in hits {
                            if !self.rank_hits.iter().any(|existing| existing.id == hit.id) {
                                self.rank_hits.push(hit);
                            }
                        }
                        self.rank_hits.len() - before
                    } else {
                        self.rank_hits = hits;
                        self.rank_hits.len()
                    };
                    // No new rows means the board is exhausted.
                    self.rank_more = fresh > 0;
                    self.status = format!("榜单 {} 条", self.rank_hits.len());
                    if !self.rank_more {
                        self.status = format!("榜单到底了（{} 条）", self.rank_hits.len());
                    }
                }
                Msg::HomeMoreFailed { section } => {
                    // Drop the offset so the view stops asking; the block simply has no more.
                    if let Some(home) = self.home.as_mut() {
                        if let Some(target) = home.sections.get_mut(section) {
                            target.next_offset = None;
                        }
                    }
                    self.home_loading_more = false;
                    self.status = "这个区块到底了".to_string();
                }
                Msg::Comments {
                    rows,
                    next,
                    append,
                } => {
                    if append {
                        self.comments.extend(rows);
                    } else {
                        self.comments = rows;
                    }
                    self.comments_next = next;
                }
                Msg::Played(text) => self.status = text,
                Msg::Error(err) => {
                    self.home_loading_more = false;
                    self.status = err;
                }
            }
        }
    }

    fn advance_demo(&mut self) {
        let Some((query, stage)) = self.demo.clone() else {
            return;
        };
        if self.demo_done == 0 && !self.loading {
            self.demo_done = 1;
            self.search_query = query.clone();
            self.send(Cmd::Search { query, offset: 0 }, "搜索中…");
            return;
        }
        if stage >= 2 && self.demo_done == 1 && self.view == View::Results && !self.loading {
            let first_book = self
                .hits
                .iter()
                .find(|hit| hit.series.is_none())
                .map(|hit| hit.id.clone());
            if let Some(id) = first_book {
                self.demo_done = 2;
                self.send(Cmd::OpenBook(id), "加载目录…");
            }
            return;
        }
        if stage >= 3 && self.demo_done == 2 && self.view == View::Book && !self.loading {
            self.demo_done = 3;
            self.open_chapter(0);
        }
    }

    fn open_chapter(&mut self, index: usize) {
        let Some(chapter) = self.chapters.get(index).cloned() else {
            return;
        };
        self.chapter_index = index;
        let is_comic = self.book.as_ref().map(|book| book.is_comic).unwrap_or(false);
        if is_comic {
            // A comic chapter is a list of images; asking the text endpoint returns markup.
            self.send(Cmd::OpenManga(chapter.item_id), "解密漫画…");
        } else {
            self.send(Cmd::OpenChapter(chapter.item_id), "加载章节…");
        }
    }

    /// Same toggle for short dramas: the shelf entry remembers the episode to resume at.
    fn toggle_series_shelf(&mut self) {
        let Some(series) = self.series.clone() else {
            return;
        };
        if shelf_contains(&series.id) {
            self.status = match remove_shelf(&series.id) {
                Ok(()) => "已移出书架".to_string(),
                Err(err) => err,
            };
        } else {
            let entry = fqapp_desktop::series_shelf_entry(&series, series.episodes.first());
            self.status = match add_shelf(entry) {
                Ok(_) => "已加入书架".to_string(),
                Err(err) => err,
            };
        }
        self.shelf = load_shelf();
    }

    /// Adds the current book to the shelf, or removes it when it is already there.
    fn toggle_shelf(&mut self) {
        let Some(book) = self.book.clone() else {
            self.status = "先打开一本书".to_string();
            return;
        };
        if shelf_contains(&book.id) {
            self.status = match remove_shelf(&book.id) {
                Ok(()) => "已移出书架".to_string(),
                Err(err) => err,
            };
        } else {
            let (item_id, chapter_title) = self
                .chapters
                .get(self.chapter_index)
                .map(|chapter| (chapter.item_id.clone(), chapter.title.clone()))
                .unwrap_or_default();
            let entry = ShelfEntry {
                id: book.id.clone(),
                title: book.title.clone(),
                author: book.author.clone(),
                cover: book.cover.clone(),
                kind: "book".to_string(),
                item_id,
                chapter_title,
                index: self.chapter_index,
            };
            self.status = match add_shelf(entry) {
                Ok(_) => "已加入书架".to_string(),
                Err(err) => err,
            };
        }
        self.shelf = load_shelf();
    }

    /// Reads the current chapter aloud through mpv (audio only).
    fn play_audio(&mut self) {
        let Some(book) = self.book.clone() else {
            self.status = "先打开一本书".to_string();
            return;
        };
        let Some(chapter) = self.chapters.get(self.chapter_index).cloned() else {
            self.status = "还没有章节".to_string();
            return;
        };
        self.send(
            Cmd::PlayAudio {
                book_id: book.id,
                item_id: chapter.item_id,
                title: chapter.title,
            },
            "解析音频…",
        );
    }

    fn open_shelf_entry(&mut self, entry: ShelfEntry) {
        if entry.kind == "series" {
            self.send(
                Cmd::OpenSeries {
                    id: entry.id,
                    cover: entry.cover.clone(),
                },
                "加载剧集…",
            );
            return;
        }
        self.pending_item = Some(entry.item_id);
        self.send(Cmd::OpenBook(entry.id), "加载目录…");
    }

    /// Decodes the served pages into textures. `egui_extras` cannot open `file://` URIs, so the
    /// bytes are read and uploaded here, downscaled to a readable width.
    fn build_manga_textures(&mut self, ctx: &egui::Context) {
        const MAX_WIDTH: u32 = 900;
        for (index, uri) in self.manga_pages.iter().enumerate() {
            let path = uri.trim_start_matches("file://");
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let Ok(image) = image::load_from_memory(&bytes) else {
                continue;
            };
            let scaled = if image.width() > MAX_WIDTH {
                image.resize(MAX_WIDTH, u32::MAX, image::imageops::FilterType::Triangle)
            } else {
                image
            };
            let rgba = scaled.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = rgba.into_raw();
            let texture = ctx.load_texture(
                format!("manga-{index}"),
                egui::ColorImage::from_rgba_unmultiplied(size, &pixels),
                egui::TextureOptions::LINEAR,
            );
            self.manga_textures.push(texture);
        }
    }

    fn cover(ui: &mut egui::Ui, url: &str, size: egui::Vec2) {
        if url.is_empty() {
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter().rect_filled(rect, 4.0, Color32::from_gray(40));
            return;
        }
        ui.add(
            egui::Image::new(url)
                .fit_to_exact_size(size)
                .corner_radius(4.0),
        );
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-paging must follow a real scroll gesture. egui restores the scroll offset of a
        // ScrollArea between frames, so "the offset is near the bottom" is true even when the
        // user has not touched anything — that is what made the home page page forever.

        // egui only redraws on input by default; without this the result of an in-flight
        // request can sit in the channel forever because nothing wakes the event loop.
        if self.loading || self.home.is_none() {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }
        self.poll();
        if let Ok(series_id) = std::env::var("FQAPP_DEMO_SERIES") {
            if !self.home_requested && !self.loading {
                self.home_requested = true;
                self.send(
                    Cmd::OpenSeries {
                        id: series_id,
                        cover: String::new(),
                    },
                    "加载剧集…",
                );
            }
        } else if let Ok(book) = std::env::var("FQAPP_DEMO_MANGA") {
            if self.demo_manga_stage == 0 && !self.home_requested && !self.loading {
                self.home_requested = true;
                self.send(Cmd::OpenBook(book), "打开漫画…");
            }
        } else if let Ok(view) = std::env::var("FQAPP_DEMO_VIEW") {
            if self.demo_done == 0 && !self.home_requested {
                self.home_requested = true;
                self.view = match view.as_str() {
                    "library" => View::Library,
                    "category" => View::Category,
                    "rank" => View::Rank,
                    _ => View::Home,
                };
                self.shelf = load_shelf();
                match self.view {
                    View::Home => self.send(Cmd::Home, "加载首页…"),
                    View::Rank => self.send(Cmd::RankCatalog, "加载榜单…"),
                    View::Category => self.send(Cmd::Category(1), "加载分类…"),
                    _ => {}
                }
            }
        } else if !self.home_requested && !self.loading {
            self.home_requested = true;
            self.send(Cmd::Home, "加载首页…");
        }
        self.advance_demo();

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("fqapp").strong());
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("搜索书名 / 作者 / 短剧")
                        .desired_width(320.0),
                );
                let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("搜索").clicked() || submitted) && !self.query.trim().is_empty() {
                    let query = self.query.trim().to_string();
                    self.search_query = query.clone();
                    self.send(
                        Cmd::Search {
                            query,
                            offset: 0,
                        },
                        "搜索中…",
                    );
                }
                if ui.button("书架").clicked() {
                    self.shelf = load_shelf();
                    self.go(View::Library);
                }
                if ui.button("首页").clicked() {
                    self.send(Cmd::Home, "加载首页…");
                }
                if ui.button("排行").clicked() {
                    self.send(Cmd::RankCatalog, "加载榜单…");
                }
                if ui.button("分类").clicked() {
                    let tab = self.category_tab;
                    self.send(Cmd::Category(tab), "加载分类…");
                }
                if self.view != View::Empty && ui.button("返回").clicked() {
                    self.go_back();
                }
                ui.separator();
                let color = if self.loading {
                    Color32::from_rgb(200, 150, 60)
                } else {
                    Color32::GRAY
                };
                ui.label(RichText::new(&self.status).color(color));
            });
            ui.add_space(4.0);
        });

        match self.view {
            View::Empty => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(RichText::new("纯 Rust 版 fqapp").size(26.0).strong());
                        ui.label(
                            RichText::new("小说阅读、书架、短剧播放（交给 mpv）")
                                .color(Color32::GRAY),
                        );
                    });
                });
            }
            View::Home => {
                let Some(front) = self.home.clone() else {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(80.0);
                            ui.label(RichText::new("加载首页…").color(Color32::GRAY));
                        });
                    });
                    return;
                };
                let mut clicked: Option<Hit> = None;
                let mut switch_tab: Option<(u32, u32, String)> = None;
                let mut load_more: Option<(usize, String, u32, String, usize)> = None;
                let mut open_rank = false;
                let mut at_bottom = false;

                egui::CentralPanel::default().show(ctx, |ui| {
                    let output = egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for tab in &front.tabs {
                                let selected = tab.index == self.home_tab_index;
                                if ui
                                    .selectable_label(selected, RichText::new(&tab.title).size(14.0))
                                    .clicked()
                                    && !selected
                                {
                                    switch_tab =
                                        Some((tab.tab_type, tab.index, tab.client_template.clone()));
                                }
                            }
                        });
                        ui.separator();
                        for section in &front.sections {
                            ui.add_space(10.0);
                            // The 排行榜 block is only a preview; the real boards live in the rank
                            // page, so cap the cards and link there instead of pretending paging.
                            let is_rank = section.title.contains("排行榜");
                            let visible = if is_rank { 10 } else { section.hits.len() };
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&section.title).size(17.0).strong());
                                if is_rank && ui.link("查看完整榜单 〉").clicked() {
                                    open_rank = true;
                                }
                            });
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                for hit in section.hits.iter().take(visible) {
                                    let card = ui.allocate_ui(egui::vec2(126.0, 214.0), |ui| {
                                        ui.vertical(|ui| {
                                            Self::cover(ui, &hit.cover, egui::vec2(118.0, 158.0));
                                            let title: String = hit.title.chars().take(16).collect();
                                            ui.label(RichText::new(title).size(12.5));
                                            if !hit.author.is_empty() {
                                                let author: String =
                                                    hit.author.chars().take(12).collect();
                                                ui.label(
                                                    RichText::new(author)
                                                        .size(11.0)
                                                        .color(Color32::GRAY),
                                                );
                                            }
                                        });
                                    });
                                    if card.response.interact(egui::Sense::click()).clicked() {
                                        clicked = Some(hit.clone());
                                    }
                                }
                            });
                        }
                        if let Some((index, section)) = front
                            .sections
                            .iter()
                            .enumerate()
                            .find(|(_, section)| section.next_offset.is_some())
                        {
                            ui.add_space(10.0);
                            if ui.button("加载更多").clicked() {
                                load_more = Some((
                                    index,
                                    section.cell_id.clone(),
                                    section.tab_type,
                                    section.client_template.clone(),
                                    section.next_offset.unwrap_or(0),
                                ));
                            }
                        }
                    });
                    // Guard against the "content shorter than the viewport" case: there the sum
                    // is always past the threshold and the view would page forever.
                    let scrollable = output.content_size.y > output.inner_rect.height() + 32.0;
                    let offset = output.state.offset.y;
                    let moved = self.home_offset >= 0.0 && (offset - self.home_offset).abs() > 1.0;
                    self.home_offset = offset;
                    at_bottom = moved
                        && scrollable
                        && offset + output.inner_rect.height() >= output.content_size.y - 300.0;
                });

                if load_more.is_none() && at_bottom {
                    if let Some((index, section)) = front
                        .sections
                        .iter()
                        .enumerate()
                        .find(|(_, section)| section.next_offset.is_some())
                    {
                        load_more = Some((
                            index,
                            section.cell_id.clone(),
                            section.tab_type,
                            section.client_template.clone(),
                            section.next_offset.unwrap_or(0),
                        ));
                    }
                }
                if let Some((section, cell_id, tab_type, client_template, offset)) = load_more {
                    if !self.home_loading_more {
                        self.home_loading_more = true;
                        self.send(
                            Cmd::HomeMore {
                                section,
                                cell_id,
                                tab_type,
                                client_template,
                                offset,
                            },
                            "加载更多…",
                        );
                    }
                }
                if open_rank {
                    self.send(Cmd::RankCatalog, "加载榜单…");
                }
                if let Some((tab_type, index, template)) = switch_tab {
                    self.home_tab_index = index;
                    self.send(
                        Cmd::HomeChannel {
                            tab_type,
                            tab_index: index,
                            client_template: template,
                        },
                        "加载频道…",
                    );
                }
                if let Some(hit) = clicked {
                    match hit.series {
                        Some(series_id) => self.send(Cmd::OpenSeries { id: series_id, cover: hit.cover.clone() }, "加载剧集…"),
                        None => self.send(Cmd::OpenBook(hit.id), "加载目录…"),
                    }
                }
            }
            View::Results => {
                let mut clicked: Option<Hit> = None;
                let mut load_more = false;
                egui::CentralPanel::default().show(ctx, |ui| {
                    let output = egui::ScrollArea::vertical().show(ui, |ui| {
                        for hit in &self.hits {
                            let response = ui
                                .horizontal(|ui| {
                                    Self::cover(ui, &hit.cover, egui::vec2(56.0, 76.0));
                                    ui.vertical(|ui| {
                                        let tag = if hit.series.is_some() { "短剧 · " } else { "" };
                                        ui.label(
                                            RichText::new(format!("{tag}{}", hit.title))
                                                .size(16.0)
                                                .strong(),
                                        );
                                        if !hit.author.is_empty() {
                                            ui.label(
                                                RichText::new(&hit.author)
                                                    .size(12.0)
                                                    .color(Color32::GRAY),
                                            );
                                        }
                                        if !hit.summary.is_empty() {
                                            let mut snippet: String =
                                                hit.summary.chars().take(110).collect();
                                            if hit.summary.chars().count() > 110 {
                                                snippet.push('…');
                                            }
                                            ui.label(RichText::new(snippet).size(12.0));
                                        }
                                    });
                                })
                                .response
                                .interact(egui::Sense::click());
                            if response.clicked() {
                                clicked = Some(hit.clone());
                            }
                            ui.separator();
                        }
                        // The service pages in tens; keep pulling while the user is at the end.
                        if self.search_next.is_some() {
                            ui.add_space(6.0);
                            if ui.button("加载更多").clicked() {
                                load_more = true;
                            }
                        }
                    });
                    if self.search_next.is_some() {
                        let scrollable = output.content_size.y > output.inner_rect.height() + 32.0;
                        let offset = output.state.offset.y;
                        let moved =
                            self.results_offset >= 0.0 && (offset - self.results_offset).abs() > 1.0;
                        self.results_offset = offset;
                        if moved
                            && scrollable
                            && offset + output.inner_rect.height() >= output.content_size.y - 300.0
                        {
                            load_more = true;
                        }
                    }
                });
                if load_more && !self.loading {
                    if let Some(offset) = self.search_next {
                        let query = self.search_query.clone();
                        self.send(Cmd::Search { query, offset }, "加载更多…");
                    }
                }
                if let Some(hit) = clicked {
                    match hit.series {
                        Some(series_id) => self.send(Cmd::OpenSeries { id: series_id, cover: hit.cover.clone() }, "加载剧集…"),
                        None => self.send(Cmd::OpenBook(hit.id), "加载目录…"),
                    }
                }
            }
            View::Library => {
                let mut open = None;
                let mut drop = None;
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("我的书架").size(19.0).strong());
                        ui.label(
                            RichText::new(format!("{} 本", self.shelf.len()))
                                .size(13.0)
                                .color(Color32::GRAY),
                        );
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if self.shelf.is_empty() {
                            ui.add_space(60.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new("书架还是空的，在详情页点「加入书架」")
                                        .color(Color32::GRAY),
                                );
                            });
                            return;
                        }
                        ui.horizontal_wrapped(|ui| {
                            for entry in &self.shelf {
                                let card = ui.allocate_ui(egui::vec2(126.0, 220.0), |ui| {
                                    ui.vertical(|ui| {
                                        Self::cover(ui, &entry.cover, egui::vec2(118.0, 158.0));
                                        let title: String = entry.title.chars().take(16).collect();
                                        ui.label(RichText::new(title).size(12.5));
                                        let chapter: String =
                                            entry.chapter_title.chars().take(14).collect();
                                        ui.label(
                                            RichText::new(chapter)
                                                .size(11.0)
                                                .color(Color32::GRAY),
                                        );
                                        if ui.small_button("移出").clicked() {
                                            drop = Some(entry.id.clone());
                                        }
                                    });
                                });
                                if card.response.interact(egui::Sense::click()).clicked() {
                                    open = Some(entry.clone());
                                }
                            }
                        });
                    });
                });
                if let Some(id) = drop {
                    let _ = remove_shelf(&id);
                    self.shelf = load_shelf();
                }
                if let Some(entry) = open {
                    self.open_shelf_entry(entry);
                }
            }
            View::Book => {
                let Some(book) = self.book.clone() else {
                    self.go(View::Results);
                    return;
                };
                let mut toggle = false;
                let mut listen = false;
                let mut load_comments: Option<usize> = None;
                egui::SidePanel::right("chapters")
                    .resizable(true)
                    .default_width(320.0)
                    .show(ctx, |ui| {
                        ui.add_space(4.0);
                        ui.label(RichText::new("目录").strong());
                        ui.separator();
                        let mut jump = None;
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (index, chapter) in self.chapters.iter().enumerate() {
                                if ui
                                    .selectable_label(
                                        index == self.chapter_index,
                                        RichText::new(&chapter.title).size(14.0),
                                    )
                                    .clicked()
                                {
                                    jump = Some(index);
                                }
                            }
                        });
                        if let Some(index) = jump {
                            self.open_chapter(index);
                        }
                    });
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add_space(8.0);
                        ui.horizontal_top(|ui| {
                            Self::cover(ui, &book.cover, egui::vec2(96.0, 128.0));
                            ui.vertical(|ui| {
                        ui.label(RichText::new(&book.title).size(24.0).strong());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&book.author).color(Color32::GRAY));
                            ui.separator();
                            let in_shelf = shelf_contains(&book.id);
                            let label = if in_shelf { "已在书架（点击移出）" } else { "加入书架" };
                            if ui.button(label).clicked() {
                                toggle = true;
                            }
                        });
                            });
                        });
                        ui.add_space(10.0);
                        ui.label(&book.summary);
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            let start = if book.is_comic { "从第一话开始看" } else { "从第一章开始读" };
                            if ui.button(RichText::new(start).size(16.0)).clicked() {
                                self.open_chapter(0);
                            }
                            if !book.is_comic
                                && ui
                                    .button(RichText::new("听书（本章）").size(16.0))
                                    .clicked()
                            {
                                listen = true;
                            }
                        });
                    });
                });
                if toggle {
                    self.toggle_shelf();
                }
                if listen {
                    self.play_audio();
                }
                if let Some(offset) = load_comments {
                    let book_id = self.book.as_ref().map(|b| b.id.clone()).unwrap_or_default();
                    self.send(Cmd::Comments { book_id, offset }, "加载评论…");
                }
            }
            View::Series => {
                let Some(series) = self.series.clone() else {
                    self.go(View::Results);
                    return;
                };
                let mut play = None;
                let mut toggle_shelf_now = false;
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            Self::cover(ui, &series.cover, egui::vec2(92.0, 124.0));
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&series.title).size(22.0).strong());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("共 {} 集", series.episodes.len()))
                                            .color(Color32::GRAY),
                                    );
                                    ui.separator();
                                    let in_shelf = shelf_contains(&series.id);
                                    let label = if in_shelf {
                                        "已在书架（点击移出）"
                                    } else {
                                        "加入书架"
                                    };
                                    if ui.button(label).clicked() {
                                        toggle_shelf_now = true;
                                    }
                                });
                                ui.label(RichText::new(&series.intro).size(13.0));
                            });
                        });
                        ui.add_space(10.0);
                        ui.separator();
                        for episode in &series.episodes {
                            if ui
                                .add(
                                    egui::Button::new(RichText::new(format!(
                                        "第 {} 集{}  ({}s)",
                                        episode.index,
                                        if episode.title.is_empty() {
                                            String::new()
                                        } else {
                                            format!("  {}", episode.title)
                                        },
                                        episode.duration
                                    )))
                                    .min_size(egui::vec2(ui.available_width(), 0.0)),
                                )
                                .clicked()
                            {
                                play = Some((episode.vid.clone(), episode.title.clone()));
                            }
                        }
                    });
                });
                if toggle_shelf_now {
                    self.toggle_series_shelf();
                }
                if let Some((vid, title)) = play {
                    // Keep the resume point in step with what is being watched.
                    if let Some(series) = self.series.clone() {
                        if shelf_contains(&series.id) {
                            if let Some(episode) = series
                                .episodes
                                .iter()
                                .find(|episode| episode.vid == vid)
                                .cloned()
                            {
                                update_progress(
                                    &series.id,
                                    &vid,
                                    &format!("第 {} 集", episode.index),
                                    episode.index.saturating_sub(1),
                                );
                                self.shelf = load_shelf();
                            }
                        }
                    }
                    self.send(Cmd::PlayEpisode { vid, title }, "解析视频地址…");
                }
            }
            View::Category => {
                let Some(front) = self.category.clone() else {
                    self.go(View::Results);
                    return;
                };
                let mut switch_tab = None;
                let mut search_tag = None;
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("频道").strong());
                        for tab in &front.tabs {
                            let value: u32 = tab.value.parse().unwrap_or(1);
                            let selected = self.category_tab == value;
                            if ui
                                .selectable_label(selected, RichText::new(&tab.name).size(15.0))
                                .clicked()
                                && !selected
                            {
                                switch_tab = Some(value);
                            }
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // Groups follow `rule.dim`; tiles whose dimension is unknown land in 其他.
                        let mut groups: Vec<(String, Vec<&fqapp_desktop::CategoryTile>)> = Vec::new();
                        for dim in &front.dims {
                            if dim.value == "0" {
                                continue;
                            }
                            let tiles: Vec<&fqapp_desktop::CategoryTile> = front
                                .tiles
                                .iter()
                                .filter(|tile| tile.dim == dim.value)
                                .collect();
                            if !tiles.is_empty() {
                                groups.push((dim.name.clone(), tiles));
                            }
                        }
                        let known: Vec<&str> =
                            front.dims.iter().map(|dim| dim.value.as_str()).collect();
                        let rest: Vec<&fqapp_desktop::CategoryTile> = front
                            .tiles
                            .iter()
                            .filter(|tile| !known.contains(&tile.dim.as_str()) || tile.dim == "0")
                            .collect();
                        if !rest.is_empty() {
                            groups.insert(0, ("全部".to_string(), rest));
                        }

                        for (name, tiles) in &groups {
                            ui.add_space(10.0);
                            ui.label(RichText::new(name).size(12.0).color(Color32::GRAY));
                            ui.horizontal_wrapped(|ui| {
                                for tile in tiles {
                                    let label = if tile.hot {
                                        format!("🔥 {}", tile.name)
                                    } else {
                                        tile.name.clone()
                                    };
                                    if ui.button(RichText::new(label).size(14.0)).clicked() {
                                        search_tag = Some(tile.name.clone());
                                    }
                                }
                            });
                        }
                        ui.add_space(20.0);
                    });
                });
                if let Some(tab) = switch_tab {
                    self.category_tab = tab;
                    self.send(Cmd::Category(tab), "加载分类…");
                }
                if let Some(tag) = search_tag {
                    self.query = tag.clone();
                    self.search_query = tag.clone();
                    self.send(Cmd::Search { query: tag, offset: 0 }, "搜索中…");
                }
            }
            View::Rank => {
                let Some(catalog) = self.rank.clone() else {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(80.0);
                            ui.label(RichText::new("加载榜单…").color(Color32::GRAY));
                        });
                    });
                    return;
                };
                let mut switch: Option<(String, String)> = None;
                let mut clicked: Option<Hit> = None;
                let mut load_more = false;

                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for board in &catalog.boards {
                            let selected = board.algo == self.rank_algo;
                            if ui
                                .selectable_label(selected, RichText::new(&board.name).size(15.0))
                                .clicked()
                                && !selected
                            {
                                switch = Some((board.algo.clone(), self.rank_category.clone()));
                            }
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        for category in &catalog.categories {
                            let selected = category.id == self.rank_category;
                            if ui
                                .selectable_label(selected, RichText::new(&category.name).size(13.0))
                                .clicked()
                                && !selected
                            {
                                switch = Some((self.rank_algo.clone(), category.id.clone()));
                            }
                        }
                    });
                    ui.separator();
                    let output = egui::ScrollArea::vertical().show(ui, |ui| {
                        for (index, hit) in self.rank_hits.iter().enumerate() {
                            let row = ui
                                .horizontal(|ui| {
                                    let rank = format!("{:>2}", index + 1);
                                    ui.label(
                                        RichText::new(rank)
                                            .size(18.0)
                                            .strong()
                                            .color(if index < 3 {
                                                Color32::from_rgb(220, 160, 60)
                                            } else {
                                                Color32::GRAY
                                            }),
                                    );
                                    Self::cover(ui, &hit.cover, egui::vec2(48.0, 64.0));
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(&hit.title).size(15.0).strong());
                                        ui.label(
                                            RichText::new(&hit.author)
                                                .size(12.0)
                                                .color(Color32::GRAY),
                                        );
                                    });
                                })
                                .response
                                .interact(egui::Sense::click());
                            if row.clicked() {
                                clicked = Some(hit.clone());
                            }
                            ui.separator();
                        }
                        if self.rank_more {
                            ui.add_space(6.0);
                            if ui.button("加载更多").clicked() {
                                load_more = true;
                            }
                        } else if !self.rank_hits.is_empty() {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new("到底了").size(12.0).color(Color32::GRAY),
                            );
                        }
                    });
                    let scrollable = output.content_size.y > output.inner_rect.height() + 32.0;
                    let offset = output.state.offset.y;
                    let moved = self.rank_offset >= 0.0 && (offset - self.rank_offset).abs() > 1.0;
                    self.rank_offset = offset;
                    if self.rank_more
                        && moved
                        && scrollable
                        && offset + output.inner_rect.height() >= output.content_size.y - 300.0
                    {
                        load_more = true;
                    }
                });

                if let Some((algo, category)) = switch {
                    self.rank_algo = algo.clone();
                    self.rank_category = category.clone();
                    self.rank_hits.clear();
                    self.rank_more = true;
                    let rank_id = self.rank_id.clone();
                    self.send(
                        Cmd::RankPage {
                            rank_id,
                            algo,
                            category,
                            offset: 0,
                        },
                        "加载榜单…",
                    );
                } else if load_more && self.rank_more && !self.loading {
                    let offset = self.rank_hits.len();
                    let rank_id = self.rank_id.clone();
                    self.send(
                        Cmd::RankPage {
                            rank_id,
                            algo: self.rank_algo.clone(),
                            category: self.rank_category.clone(),
                            offset,
                        },
                        "加载更多…",
                    );
                }
                if let Some(hit) = clicked {
                    match hit.series {
                        Some(series_id) => self.send(Cmd::OpenSeries { id: series_id, cover: hit.cover.clone() }, "加载剧集…"),
                        None => self.send(Cmd::OpenBook(hit.id), "加载目录…"),
                    }
                }
            }
            View::Manga => {
                egui::TopBottomPanel::bottom("manga_nav").show(ctx, |ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(self.chapter_index > 0, egui::Button::new("上一话"))
                            .clicked()
                        {
                            self.chapter_index -= 1;
                            if let Some(chapter) = self.chapters.get(self.chapter_index).cloned() {
                                self.send(Cmd::OpenManga(chapter.item_id), "解密漫画…");
                            }
                        }
                        if ui
                            .add_enabled(
                                self.chapter_index + 1 < self.chapters.len(),
                                egui::Button::new("下一话"),
                            )
                            .clicked()
                        {
                            self.chapter_index += 1;
                            if let Some(chapter) = self.chapters.get(self.chapter_index).cloned() {
                                self.send(Cmd::OpenManga(chapter.item_id), "解密漫画…");
                            }
                        }
                        if ui.button("目录").clicked() {
                            self.go(View::Book);
                        }
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "{} · {} 页",
                                self.chapters
                                    .get(self.chapter_index)
                                    .map(|chapter| chapter.title.clone())
                                    .unwrap_or_default(),
                                self.manga_pages.len()
                            ))
                            .color(Color32::GRAY),
                        );
                    });
                    ui.add_space(4.0);
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    if self.manga_textures.is_empty() && !self.manga_pages.is_empty() {
                        self.build_manga_textures(ctx);
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // Cap the column width and centre it, so a maximised window keeps the
                        // pages in the middle instead of hugging the left edge.
                        let width = (ui.available_width() - 24.0).min(900.0);
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.set_width(ui.available_width());
                            for texture in &self.manga_textures {
                                let size = texture.size_vec2();
                                let scale = (width / size.x).min(1.0);
                                ui.add(egui::Image::new((texture.id(), size * scale)));
                                ui.add_space(6.0);
                            }
                        });
                    });
                });
            }
            View::Reader => {
                let mut toggle = false;
                let mut listen = false;
                egui::TopBottomPanel::bottom("reader_nav").show(ctx, |ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(self.chapter_index > 0, egui::Button::new("上一章"))
                            .clicked()
                        {
                            self.chapter_index -= 1;
                            self.open_chapter(self.chapter_index);
                        }
                        if ui
                            .add_enabled(
                                self.chapter_index + 1 < self.chapters.len(),
                                egui::Button::new("下一章"),
                            )
                            .clicked()
                        {
                            self.chapter_index += 1;
                            self.open_chapter(self.chapter_index);
                        }
                        if ui.button("目录").clicked() {
                            self.go(View::Book);
                        }
                        let in_shelf = self
                            .book
                            .as_ref()
                            .map(|book| shelf_contains(&book.id))
                            .unwrap_or(false);
                        if ui
                            .button(if in_shelf { "已在书架" } else { "加入书架" })
                            .clicked()
                        {
                            toggle = true;
                        }
                        if ui.button("听书").clicked() {
                            listen = true;
                        }
                        ui.separator();
                        ui.label(RichText::new(&self.chapter_title).color(Color32::GRAY));
                    });
                    ui.add_space(4.0);
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut area = egui::ScrollArea::vertical();
                    if self.reader_pos > 0.0 {
                        area = area.vertical_scroll_offset(self.reader_pos);
                    }
                    let output = area.show(ui, |ui| {
                        ui.add_space(12.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(&self.chapter_title).size(20.0).strong());
                        });
                        ui.add_space(12.0);
                        for paragraph in &self.paragraphs {
                            ui.label(RichText::new(paragraph).size(17.0));
                            ui.add_space(10.0);
                        }
                        ui.add_space(40.0);
                    });
                    self.reader_pos = output.state.offset.y;
                });
                if toggle {
                    self.toggle_shelf();
                }
                if listen {
                    self.play_audio();
                }
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let (cmd_tx, cmd_rx) = tokio_mpsc::unbounded_channel::<Cmd>();
    let (msg_tx, msg_rx) = channel::<Msg>();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(worker(cmd_rx, msg_tx));
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 860.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("fqapp"),
        ..Default::default()
    };

    eframe::run_native(
        "fqapp",
        options,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(App::new(cmd_tx, msg_rx)))
        }),
    )
}

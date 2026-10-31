//! Application state and the command/response pump.
//!
//! Views are pure functions over this state (`views::*::show`); everything that
//! mutates state or talks to the worker lives here.

use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::egui;
use egui::Color32;
use tokio::sync::mpsc as tokio_mpsc;

use qieyue::{
    add_shelf, load_shelf, remove_shelf, update_progress, CategoryFront, Chapter, Hit, Home,
    Series, ShelfEntry,
};

use crate::views;
use crate::worker::{Cmd, Command, Msg, Reply, RequestKind};

#[derive(Clone, Debug)]
pub(crate) struct BookInfo {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) summary: String,
    pub(crate) cover: String,
    pub(crate) is_comic: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
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

pub(crate) struct App {
    cmds: tokio_mpsc::UnboundedSender<Command>,
    msgs: Receiver<Reply>,

    pub(crate) query: String,
    pub(crate) status: String,
    pub(crate) loading: bool,

    pub(crate) view: View,
    history: Vec<View>,
    pub(crate) hits: Vec<Hit>,
    pub(crate) search_query: String,
    pub(crate) search_next: Option<usize>,
    pub(crate) book: Option<BookInfo>,
    pub(crate) chapters: Vec<Chapter>,
    pub(crate) chapter_index: usize,
    pub(crate) chapter_title: String,
    pub(crate) paragraphs: Vec<String>,
    pub(crate) reader_pos: f32,
    pub(crate) series: Option<Series>,
    pub(crate) rank: Option<qieyue::RankCatalog>,
    pub(crate) rank_id: String,
    pub(crate) rank_algo: String,
    pub(crate) rank_category: String,
    pub(crate) rank_hits: Vec<Hit>,
    pub(crate) rank_more: bool,
    pub(crate) comments: Vec<qieyue::Comment>,
    pub(crate) comments_next: Option<usize>,
    pub(crate) manga_pages: Vec<String>,
    pub(crate) manga_textures: Vec<egui::TextureHandle>,
    /// How many pages have been decoded. Decoding runs a couple of pages per
    /// frame, so the window keeps responding on long chapters.
    pub(crate) manga_scanned: usize,
    pub(crate) home: Option<Home>,
    pub(crate) home_tab_index: u32,
    pub(crate) home_loading_more: bool,
    home_requested: bool,
    pub(crate) category: Option<CategoryFront>,
    pub(crate) category_tab: u32,
    pub(crate) shelf: Vec<ShelfEntry>,
    /// Set when a shelf entry is opened: the chapter to jump to once the directory arrives.
    pending_item: Option<String>,

    /// Monotonic request id and the newest id per domain. A reply whose id is
    /// older than the domain's newest request is dropped, so a slow request can
    /// never overwrite a newer view.
    next_request: u64,
    latest: HashMap<RequestKind, u64>,
    /// Requests in flight per domain. Used for paging guards; the global
    /// `loading` flag is just "any of these is non-zero".
    pending: HashMap<RequestKind, usize>,

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
    pub(crate) fn new(cmds: tokio_mpsc::UnboundedSender<Command>, msgs: Receiver<Reply>) -> Self {
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
            manga_scanned: 0,
            home: None,
            home_tab_index: 0,
            home_loading_more: false,
            home_requested: false,
            category: None,
            category_tab: 1,
            shelf: load_shelf(),
            pending_item: None,
            next_request: 0,
            latest: HashMap::new(),
            pending: HashMap::new(),
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
    pub(crate) fn go(&mut self, view: View) {
        if view != self.view {
            self.history.push(self.view);
            self.view = view;
        }
    }

    pub(crate) fn go_back(&mut self) {
        self.view = self.history.pop().unwrap_or(View::Home);
    }

    /// Queues one request. A write to the channel never blocks the UI: the
    /// worker runs it as its own task.
    pub(crate) fn send(&mut self, job: Cmd, status: &str) {
        let kind = job.kind();
        // HomeMore and Comments are refinements of an existing view: they keep
        // the parent request's id so switching channel/book makes them stale.
        let id = match kind {
            RequestKind::HomeMore => self.latest.get(&RequestKind::Home).copied().unwrap_or(0),
            RequestKind::Comments => self.latest.get(&RequestKind::Book).copied().unwrap_or(0),
            _ => {
                self.next_request += 1;
                let id = self.next_request;
                self.latest.insert(kind, id);
                id
            }
        };

        *self.pending.entry(kind).or_insert(0) += 1;
        self.loading = true;
        self.status = status.to_string();
        if self.cmds.send(Command { id, job }).is_err() {
            let left = self.pending.entry(kind).or_insert(0);
            *left = left.saturating_sub(1);
            self.loading = self.pending.values().any(|&count| count > 0);
            self.status = "后台线程已退出".to_string();
        }
    }

    /// True while a request of this domain is in flight. Views use this for
    /// paging guards so an unrelated request never blocks them.
    pub(crate) fn is_pending(&self, kind: RequestKind) -> bool {
        self.pending.get(&kind).copied().unwrap_or(0) > 0
    }

    /// Shelf membership from the in-memory copy. The previous implementation
    /// read and parsed `shelf.json` on every frame, which stalled the redraw.
    pub(crate) fn on_shelf(&self, id: &str) -> bool {
        self.shelf.iter().any(|entry| entry.id == id)
    }

    fn poll(&mut self) {
        while let Ok(reply) = self.msgs.try_recv() {
            let left = self.pending.entry(reply.kind).or_insert(0);
            *left = left.saturating_sub(1);
            self.loading = self.pending.values().any(|&count| count > 0);

            let accepted = match reply.kind {
                RequestKind::HomeMore => {
                    self.latest.get(&RequestKind::Home).copied().unwrap_or(0) == reply.id
                }
                RequestKind::Comments => {
                    self.latest.get(&RequestKind::Book).copied().unwrap_or(0) == reply.id
                }
                // `0` is the init-failure reply; every real id starts at 1.
                _ => self.latest.get(&reply.kind).copied().unwrap_or(0) <= reply.id,
            };
            if !accepted {
                // A dropped page must not leave auto-paging wedged.
                if reply.kind == RequestKind::HomeMore {
                    self.home_loading_more = false;
                }
                continue;
            }

            match reply.msg {
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
                        if let Ok(book) = std::env::var("FQAPP_DEMO_MANGA") {
                            if book == self.book.as_ref().map(|b| b.id.clone()).unwrap_or_default()
                            {
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
                        if self.on_shelf(&book.id) {
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
                    self.status =
                        format!("{} 个标签 · {} 个频道", front.tiles.len(), front.tabs.len());
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
                    self.manga_scanned = 0;
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
                Msg::Comments { rows, next, append } => {
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

    /// First load / scripted entry point: decides which view to open once the
    /// app is alive. Kept out of `update` so the layout code stays readable.
    fn bootstrap(&mut self) {
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

    pub(crate) fn open_chapter(&mut self, index: usize) {
        let Some(chapter) = self.chapters.get(index).cloned() else {
            return;
        };
        self.chapter_index = index;
        let is_comic = self
            .book
            .as_ref()
            .map(|book| book.is_comic)
            .unwrap_or(false);
        if is_comic {
            // A comic chapter is a list of images; asking the text endpoint returns markup.
            self.send(Cmd::OpenManga(chapter.item_id), "解密漫画…");
        } else {
            self.send(Cmd::OpenChapter(chapter.item_id), "加载章节…");
        }
    }

    /// Same toggle for short dramas: the shelf entry remembers the episode to resume at.
    pub(crate) fn toggle_series_shelf(&mut self) {
        let Some(series) = self.series.clone() else {
            return;
        };
        if self.on_shelf(&series.id) {
            self.status = match remove_shelf(&series.id) {
                Ok(()) => "已移出书架".to_string(),
                Err(err) => err,
            };
        } else {
            let entry = qieyue::series_shelf_entry(&series, series.episodes.first());
            self.status = match add_shelf(entry) {
                Ok(_) => "已加入书架".to_string(),
                Err(err) => err,
            };
        }
        self.shelf = load_shelf();
    }

    /// Adds the current book to the shelf, or removes it when it is already there.
    pub(crate) fn toggle_shelf(&mut self) {
        let Some(book) = self.book.clone() else {
            self.status = "先打开一本书".to_string();
            return;
        };
        if self.on_shelf(&book.id) {
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
    pub(crate) fn play_audio(&mut self) {
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

    pub(crate) fn open_shelf_entry(&mut self, entry: ShelfEntry) {
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
    pub(crate) fn build_manga_textures(&mut self, ctx: &egui::Context) {
        const MAX_WIDTH: u32 = 900;
        // Decode only a couple of pages per frame: uploading a 34-page chapter
        // in one go froze the window for the whole decode.
        const MAX_PER_FRAME: usize = 2;
        let mut decoded = 0;
        while self.manga_scanned < self.manga_pages.len() && decoded < MAX_PER_FRAME {
            let index = self.manga_scanned;
            self.manga_scanned += 1;
            decoded += 1;

            let uri = self.manga_pages[index].clone();
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
        if self.manga_scanned < self.manga_pages.len() {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Mouse wheel / trackpad scrolling this frame. Listening to the input itself matters:
        // when a list is shorter than the viewport its scroll offset never changes, yet the user
        // is clearly asking for more content. Auto-paging must follow a real scroll gesture:
        // egui restores the scroll offset between frames, so "near the bottom" alone is true
        // even when nothing was touched (that is what made the home page page forever).
        let wheel_down =
            ctx.input(|i| i.raw_scroll_delta.y < -1.0 || i.key_pressed(egui::Key::PageDown));

        // egui only redraws on input by default; without this the result of an in-flight
        // request can sit in the channel forever because nothing wakes the event loop.
        if self.loading || self.home.is_none() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
        self.poll();
        self.bootstrap();
        self.advance_demo();

        views::chrome::show(self, ctx);
        match self.view {
            View::Empty => views::empty::show(self, ctx),
            View::Home => views::home::show(self, ctx, wheel_down),
            View::Results => views::results::show(self, ctx, wheel_down),
            View::Library => views::library::show(self, ctx),
            View::Book => views::book::show(self, ctx),
            View::Series => views::series::show(self, ctx),
            View::Reader => views::reader::show(self, ctx),
            View::Manga => views::manga::show(self, ctx),
            View::Category => views::category::show(self, ctx),
            View::Rank => views::rank::show(self, ctx, wheel_down),
        }
    }
}

/// Status colour shared by the top bar.
pub(crate) fn status_color(loading: bool) -> Color32 {
    if loading {
        Color32::from_rgb(200, 150, 60)
    } else {
        Color32::GRAY
    }
}

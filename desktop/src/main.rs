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
    add_shelf, category_front, init_backend, load_shelf, open_book, open_series, read_chapter,
    remove_shelf, resolve_episode, search_hits, shelf_contains, update_progress, CategoryFront,
    Chapter, Hit, ShelfEntry, Series,
};
use tokio::sync::mpsc as tokio_mpsc;

#[derive(Clone, Debug)]
struct BookInfo {
    id: String,
    title: String,
    author: String,
    summary: String,
}

enum Cmd {
    Search(String),
    OpenBook(String),
    OpenSeries(String),
    OpenChapter(String),
    PlayEpisode { vid: String, title: String },
    Category(u32),
}

enum Msg {
    Hits(Vec<Hit>),
    Detail {
        id: String,
        title: String,
        author: String,
        summary: String,
        chapters: Vec<Chapter>,
    },
    Series(Series),
    Chapter {
        title: String,
        paragraphs: Vec<String>,
    },
    Played(String),
    Category(CategoryFront),
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
            Cmd::Search(query) => match search_hits(&query).await {
                Ok(hits) => Msg::Hits(hits),
                Err(err) => Msg::Error(err),
            },
            Cmd::OpenBook(id) => match open_book(&id).await {
                Ok((title, author, summary, chapters)) => Msg::Detail {
                    id,
                    title,
                    author,
                    summary,
                    chapters,
                },
                Err(err) => Msg::Error(err),
            },
            Cmd::OpenSeries(id) => match open_series(&id).await {
                Ok(series) => Msg::Series(series),
                Err(err) => Msg::Error(err),
            },
            Cmd::OpenChapter(item_id) => match read_chapter(&item_id).await {
                Ok((title, paragraphs)) => Msg::Chapter { title, paragraphs },
                Err(err) => Msg::Error(err),
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

#[derive(PartialEq)]
enum View {
    Empty,
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
    hits: Vec<Hit>,
    book: Option<BookInfo>,
    chapters: Vec<Chapter>,
    chapter_index: usize,
    chapter_title: String,
    paragraphs: Vec<String>,
    reader_pos: f32,
    series: Option<Series>,
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
            hits: Vec::new(),
            book: None,
            chapters: Vec::new(),
            chapter_index: 0,
            chapter_title: String::new(),
            paragraphs: Vec::new(),
            reader_pos: 0.0,
            series: None,
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
        }
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
                Msg::Hits(hits) => {
                    let dramas = hits.iter().filter(|hit| hit.series.is_some()).count();
                    self.status = format!("{} 条结果（短剧 {}）", hits.len(), dramas);
                    self.hits = hits;
                    self.view = View::Results;
                }
                Msg::Detail {
                    id,
                    title,
                    author,
                    summary,
                    chapters,
                } => {
                    self.status = format!("{} · {} 章", title, chapters.len());
                    self.book = Some(BookInfo {
                        id,
                        title,
                        author,
                        summary,
                    });
                    self.chapters = chapters;
                    self.view = View::Book;
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
                    self.view = View::Series;
                }
                Msg::Chapter { title, paragraphs } => {
                    self.chapter_title = title.clone();
                    self.paragraphs = paragraphs;
                    self.reader_pos = 0.0;
                    self.view = View::Reader;
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
                    self.view = View::Category;
                }
                Msg::Played(text) => self.status = text,
                Msg::Error(err) => self.status = err,
            }
        }
    }

    fn advance_demo(&mut self) {
        let Some((query, stage)) = self.demo.clone() else {
            return;
        };
        if self.demo_done == 0 && !self.loading {
            self.demo_done = 1;
            self.send(Cmd::Search(query), "搜索中…");
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
        self.send(Cmd::OpenChapter(chapter.item_id), "加载章节…");
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

    fn open_shelf_entry(&mut self, entry: ShelfEntry) {
        self.pending_item = Some(entry.item_id);
        self.send(Cmd::OpenBook(entry.id), "加载目录…");
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
        self.poll();
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
                    self.send(Cmd::Search(query), "搜索中…");
                }
                if ui.button("分类").clicked() {
                    let tab = self.category_tab;
                    self.send(Cmd::Category(tab), "加载分类…");
                }
                if self.view != View::Empty && ui.button("返回").clicked() {
                    self.view = match self.view {
                        View::Reader => View::Book,
                        _ => View::Results,
                    };
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

        if !self.shelf.is_empty() {
            egui::SidePanel::left("shelf")
                .resizable(true)
                .default_width(210.0)
                .show(ctx, |ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new("书架").strong());
                    ui.separator();
                    let mut open = None;
                    let mut remove = None;
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for entry in &self.shelf {
                            let label = format!("{}\n{}", entry.title, entry.chapter_title);
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::Label::new(RichText::new(label).size(13.0))
                                            .sense(egui::Sense::click()),
                                    )
                                    .clicked()
                                {
                                    open = Some(entry.clone());
                                }
                                if ui.small_button("✕").on_hover_text("移出书架").clicked() {
                                    remove = Some(entry.id.clone());
                                }
                            });
                            ui.separator();
                        }
                    });
                    if let Some(id) = remove {
                        if let Err(err) = remove_shelf(&id) {
                            eprintln!("remove shelf: {err}");
                        }
                        self.shelf = load_shelf();
                    }
                    if let Some(entry) = open {
                        self.open_shelf_entry(entry);
                    }
                });
        }

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
            View::Results => {
                let mut clicked: Option<Hit> = None;
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
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
                    });
                });
                if let Some(hit) = clicked {
                    match hit.series {
                        Some(series_id) => self.send(Cmd::OpenSeries(series_id), "加载剧集…"),
                        None => self.send(Cmd::OpenBook(hit.id), "加载目录…"),
                    }
                }
            }
            View::Book => {
                let Some(book) = self.book.clone() else {
                    self.view = View::Results;
                    return;
                };
                let mut toggle = false;
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
                        ui.add_space(10.0);
                        ui.label(&book.summary);
                        ui.add_space(14.0);
                        if ui
                            .button(RichText::new("从第一章开始读").size(16.0))
                            .clicked()
                        {
                            self.open_chapter(0);
                        }
                    });
                });
                if toggle {
                    self.toggle_shelf();
                }
            }
            View::Series => {
                let Some(series) = self.series.clone() else {
                    self.view = View::Results;
                    return;
                };
                let mut play = None;
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            Self::cover(ui, &series.cover, egui::vec2(92.0, 124.0));
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&series.title).size(22.0).strong());
                                ui.label(
                                    RichText::new(format!("共 {} 集", series.episodes.len()))
                                        .color(Color32::GRAY),
                                );
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
                if let Some((vid, title)) = play {
                    self.send(Cmd::PlayEpisode { vid, title }, "解析视频地址…");
                }
            }
            View::Category => {
                let Some(front) = self.category.clone() else {
                    self.view = View::Results;
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
                    self.send(Cmd::Search(tag), "搜索中…");
                }
            }
            View::Reader => {
                let mut toggle = false;
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
                            self.view = View::Book;
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

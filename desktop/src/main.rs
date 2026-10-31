//! Native Linux front end for the `fqapi_core` backend.
//!
//! The backend owns all protocol, crypto and caching work and exposes a local REST surface
//! (`/api/v1/...`). This binary links it as a library and drives it through
//! `fqapi_core::api::request`, so no HTTP server or Flutter runtime is involved.

use std::sync::mpsc::{channel, Receiver, Sender};

use eframe::egui;
use egui::{Color32, RichText};
use fqapp_desktop::{init_backend, open_book, read_chapter, search, Book, Chapter};
use tokio::sync::mpsc as tokio_mpsc;

// ---------------------------------------------------------------------------------------------
// backend plumbing
// ---------------------------------------------------------------------------------------------

enum Cmd {
    Search(String),
    OpenBook(String),
    OpenChapter(String),
}

enum Msg {
    Search(Vec<Book>),
    Detail {
        id: String,
        title: String,
        author: String,
        summary: String,
        chapters: Vec<Chapter>,
    },
    Chapter {
        title: String,
        paragraphs: Vec<String>,
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
            Cmd::Search(query) => match search(&query).await {
                Ok(books) => Msg::Search(books),
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
            Cmd::OpenChapter(item_id) => match read_chapter(&item_id).await {
                Ok((title, paragraphs)) => Msg::Chapter { title, paragraphs },
                Err(err) => Msg::Error(err),
            },
        };
        if msgs.send(message).is_err() {
            break;
        }
    }
}

pub

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
    Reader,
}

struct App {
    cmds: tokio_mpsc::UnboundedSender<Cmd>,
    msgs: Receiver<Msg>,
    query: String,
    status: String,
    loading: bool,

    view: View,
    results: Vec<Book>,
    book: Option<Book>,
    chapters: Vec<Chapter>,
    chapter_index: usize,
    chapter_title: String,
    paragraphs: Vec<String>,
    reader_pos: f32,

    /// Optional scripted walkthrough used to screenshot each view without a human clicking:
    /// `FQAPP_DEMO_QUERY=斗破 FQAPP_DEMO_STAGE=3` searches, opens the first hit and its first
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
            results: Vec::new(),
            book: None,
            chapters: Vec::new(),
            chapter_index: 0,
            chapter_title: String::new(),
            paragraphs: Vec::new(),
            reader_pos: 0.0,
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
                Msg::Search(books) => {
                    self.status = format!("找到 {} 本", books.len());
                    self.results = books;
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
                    self.book = Some(Book {
                        id,
                        title,
                        author,
                        summary,
                    });
                    self.chapters = chapters;
                    self.view = View::Book;
                }
                Msg::Chapter { title, paragraphs } => {
                    self.chapter_title = title;
                    self.paragraphs = paragraphs;
                    self.reader_pos = 0.0;
                    self.view = View::Reader;
                    self.status = format!(
                        "{} / {}",
                        self.chapter_index + 1,
                        self.chapters.len().max(1)
                    );
                }
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
            if let Some(book) = self.results.first().cloned() {
                self.demo_done = 2;
                self.send(Cmd::OpenBook(book.id), "加载目录…");
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

    fn toggle_book(&mut self, book_id: String) {
        if self.book.as_ref().map(|b| b.id == book_id).unwrap_or(false) {
            self.view = View::Book;
        } else {
            self.send(Cmd::OpenBook(book_id), "加载目录…");
        }
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
                        .hint_text("搜索书名 / 作者")
                        .desired_width(360.0),
                );
                let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("搜索").clicked() || submitted) && !self.query.trim().is_empty() {
                    let query = self.query.trim().to_string();
                    self.send(Cmd::Search(query), "搜索中…");
                }
                if self.view != View::Empty && ui.button("返回").clicked() {
                    self.view = match self.view {
                        View::Reader if self.book.is_some() => View::Book,
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

        match self.view {
            View::Empty => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(RichText::new("纯 Rust 版 fqapp").size(26.0).strong());
                        ui.label(RichText::new("搜索、目录、正文已经打通，视频交给 mpv").color(Color32::GRAY));
                    });
                });
            }
            View::Results => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut clicked: Option<String> = None;
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for book in &self.results {
                            let response = ui
                                .group(|ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(RichText::new(&book.title).size(17.0).strong());
                                    ui.label(
                                        RichText::new(&book.author).size(13.0).color(Color32::GRAY),
                                    );
                                    let mut snippet: String =
                                        book.summary.chars().take(120).collect();
                                    if book.summary.chars().count() > 120 {
                                        snippet.push('…');
                                    }
                                    ui.label(RichText::new(snippet).size(13.0));
                                })
                                .response
                                .interact(egui::Sense::click());
                            if response.clicked() {
                                clicked = Some(book.id.clone());
                            }
                            ui.add_space(6.0);
                        }
                    });
                    if let Some(id) = clicked {
                        self.send(Cmd::OpenBook(id), "加载目录…");
                    }
                });
            }
            View::Book => {
                let Some(book) = self.book.clone() else {
                    self.view = View::Results;
                    return;
                };
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
                        ui.label(RichText::new(&book.author).color(Color32::GRAY));
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
            }
            View::Reader => {
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
            Ok(Box::new(App::new(cmd_tx, msg_rx)))
        }),
    )
}

//! Book detail: cover, summary, chapter directory and comments.
//!
//! The book is borrowed from `App` while drawing; every action that mutates
//! state is deferred until the panels have closed, so a frame never clones the
//! chapter list or the comment list.

use eframe::egui::{self, Color32, RichText, Vec2};

use crate::app::{App, View};
use crate::theme;
use crate::views::widgets;
use crate::worker::{Cmd, RequestKind};

const USER_COLOR: Color32 = Color32::from_rgb(125, 175, 220);

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if app.book.is_none() {
        app.go(View::Results);
        return;
    }

    let mut jump = None;
    egui::SidePanel::right("chapters")
        .resizable(true)
        .default_width(320.0)
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(14, 12)),
        )
        .show(ctx, |ui| {
            ui.label(RichText::new("目录").size(15.0).strong());
            ui.add_space(6.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (index, chapter) in app.chapters.iter().enumerate() {
                    if ui
                        .selectable_label(
                            index == app.chapter_index,
                            RichText::new(&chapter.title).size(13.0),
                        )
                        .clicked()
                    {
                        jump = Some(index);
                    }
                }
            });
        });
    if let Some(index) = jump {
        app.open_chapter(index);
    }

    let mut toggle = false;
    let mut listen = false;
    let mut start_reading = false;
    let mut load_comments: Option<usize> = None;

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(22, 16)),
        )
        .show(ctx, |ui| {
            let Some(book) = app.book.as_ref() else {
                return;
            };
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    widgets::cover(ui, &book.cover, Vec2::new(104.0, 140.0));
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&book.title).size(24.0).strong());
                        ui.add_space(2.0);
                        ui.label(RichText::new(&book.author).size(13.0).color(theme::MUTED));
                        ui.add_space(12.0);
                        let in_shelf = app.on_shelf(&book.id);
                        let label = if in_shelf {
                            "已在书架"
                        } else {
                            "加入书架"
                        };
                        if ui.button(label).clicked() {
                            toggle = true;
                        }
                    });
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label(RichText::new(&book.summary).size(14.0));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    let start = if book.is_comic {
                        "从第一话开始看"
                    } else {
                        "从第一章开始读"
                    };
                    if ui.button(RichText::new(start).size(15.0)).clicked() {
                        start_reading = true;
                    }
                    if !book.is_comic
                        && ui
                            .button(RichText::new("听书（本章）").size(15.0))
                            .clicked()
                    {
                        listen = true;
                    }
                });

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("评论").size(16.0).strong());
                    ui.label(
                        RichText::new(if app.comments_next.is_some() {
                            format!("已加载 {}", app.comments.len())
                        } else {
                            format!("共 {}", app.comments.len())
                        })
                        .size(12.0)
                        .color(theme::MUTED),
                    );
                });
                ui.add_space(8.0);

                if app.comments.is_empty() {
                    ui.label(RichText::new("还没有评论").size(12.0).color(theme::MUTED));
                }
                for comment in &app.comments {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&comment.user).size(13.0).color(USER_COLOR));
                        ui.label(RichText::new(&comment.when).size(11.0).color(theme::MUTED));
                    });
                    ui.add_space(2.0);
                    ui.label(RichText::new(&comment.text).size(14.0));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("赞 {} · 回复 {}", comment.digg, comment.replies))
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                    ui.add_space(12.0);
                }
                if let Some(next) = app.comments_next {
                    ui.add_enabled_ui(!app.is_pending(RequestKind::Comments), |ui| {
                        if ui.button("加载更多评论").clicked() {
                            load_comments = Some(next);
                        }
                    });
                }
            });
        });

    if start_reading {
        app.open_chapter(0);
    }
    if toggle {
        app.toggle_shelf();
    }
    if listen {
        app.play_audio();
    }
    if let Some(offset) = load_comments {
        let book_id = app
            .book
            .as_ref()
            .map(|book| book.id.clone())
            .unwrap_or_default();
        app.send(Cmd::Comments { book_id, offset }, "加载评论…");
    }
}

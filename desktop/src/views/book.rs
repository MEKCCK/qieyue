//! Book detail: cover, summary, chapter directory and comments.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::{App, View};
use crate::views::widgets;
use crate::worker::{Cmd, RequestKind};

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let Some(book) = app.book.clone() else {
        app.go(View::Results);
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
                for (index, chapter) in app.chapters.iter().enumerate() {
                    if ui
                        .selectable_label(
                            index == app.chapter_index,
                            RichText::new(&chapter.title).size(14.0),
                        )
                        .clicked()
                    {
                        jump = Some(index);
                    }
                }
            });
            if let Some(index) = jump {
                app.open_chapter(index);
            }
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal_top(|ui| {
                widgets::cover(ui, &book.cover, egui::vec2(96.0, 128.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new(&book.title).size(24.0).strong());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&book.author).color(Color32::GRAY));
                        ui.separator();
                        let in_shelf = app.on_shelf(&book.id);
                        let label = if in_shelf {
                            "已在书架（点击移出）"
                        } else {
                            "加入书架"
                        };
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
                let start = if book.is_comic {
                    "从第一话开始看"
                } else {
                    "从第一章开始读"
                };
                if ui.button(RichText::new(start).size(16.0)).clicked() {
                    app.open_chapter(0);
                }
                if !book.is_comic
                    && ui
                        .button(RichText::new("听书（本章）").size(16.0))
                        .clicked()
                {
                    listen = true;
                }
            });

            ui.add_space(18.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("评论").size(16.0).strong());
                ui.label(
                    RichText::new(if app.comments_next.is_some() {
                        format!("已加载 {}", app.comments.len())
                    } else {
                        format!("共 {}", app.comments.len())
                    })
                    .size(12.0)
                    .color(Color32::GRAY),
                );
            });
            ui.add_space(6.0);
            if app.comments.is_empty() {
                ui.label(RichText::new("还没有评论").size(12.0).color(Color32::GRAY));
            }
            for comment in &app.comments {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&comment.user)
                            .size(13.0)
                            .color(Color32::from_rgb(120, 170, 220)),
                    );
                    ui.label(RichText::new(&comment.when).size(11.0).color(Color32::GRAY));
                });
                ui.label(RichText::new(&comment.text).size(14.0));
                ui.label(
                    RichText::new(format!("赞 {} · 回复 {}", comment.digg, comment.replies))
                        .size(11.0)
                        .color(Color32::GRAY),
                );
                ui.add_space(10.0);
            }
            if let Some(next) = app.comments_next {
                if ui
                    .add_enabled(
                        !app.is_pending(RequestKind::Comments),
                        egui::Button::new("加载更多评论"),
                    )
                    .clicked()
                {
                    load_comments = Some(next);
                }
            }
        });
    });

    if toggle {
        app.toggle_shelf();
    }
    if listen {
        app.play_audio();
    }
    if let Some(offset) = load_comments {
        let book_id = app.book.as_ref().map(|b| b.id.clone()).unwrap_or_default();
        app.send(Cmd::Comments { book_id, offset }, "加载评论…");
    }
}

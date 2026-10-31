//! Shelf grid, virtualised by row and rendered from the in-memory shelf.

use eframe::egui::{self, Align, Layout, RichText, Sense, Vec2};

use crate::app::App;
use crate::theme;
use crate::views::{widgets, widgets::clamp};

const CARD_W: f32 = 126.0;
const CARD_H: f32 = 224.0;
const GAP: f32 = 10.0;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let mut open = None;
    let mut drop = None;

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(18, 12)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("我的书架").size(17.0).strong());
                ui.label(
                    RichText::new(format!("{} 本", app.shelf.len()))
                        .size(12.0)
                        .color(theme::MUTED),
                );
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(8.0);

            if app.shelf.is_empty() {
                widgets::empty_state(ui, "书架还是空的，在详情页点「加入书架」");
                return;
            }

            let columns = (((ui.available_width() + GAP) / (CARD_W + GAP)).floor() as usize).max(1);
            let rows = app.shelf.len().div_ceil(columns);
            egui::ScrollArea::vertical().show_rows(ui, CARD_H, rows, |ui, range| {
                for row in range {
                    ui.horizontal(|ui| {
                        for column in 0..columns {
                            let index = row * columns + column;
                            let Some(entry) = app.shelf.get(index) else {
                                break;
                            };
                            let response = ui
                                .allocate_ui_with_layout(
                                    Vec2::new(CARD_W, CARD_H),
                                    Layout::top_down(Align::Center),
                                    |ui| {
                                        ui.add_space(3.0);
                                        widgets::cover(ui, &entry.cover, Vec2::new(118.0, 158.0));
                                        ui.add_space(5.0);
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(clamp(&entry.title, 16)).size(12.5),
                                            )
                                            .truncate(),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(clamp(&entry.chapter_title, 14))
                                                    .size(11.0)
                                                    .color(theme::MUTED),
                                            )
                                            .truncate(),
                                        );
                                        if ui.small_button("移出").clicked() {
                                            drop = Some(entry.id.clone());
                                        }
                                    },
                                )
                                .response
                                .interact(Sense::click());
                            if response.clicked() {
                                open = Some(entry.clone());
                            }
                        }
                    });
                }
            });
        });

    if let Some(id) = drop {
        let _ = qieyue::remove_shelf(&id);
        app.shelf = qieyue::load_shelf();
    }
    if let Some(entry) = open {
        app.open_shelf_entry(entry);
    }
}

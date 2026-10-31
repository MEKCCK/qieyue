//! Shelf grid: books and short dramas with their resume position.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::App;
use crate::views::widgets;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let mut open = None;
    let mut drop = None;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("我的书架").size(19.0).strong());
            ui.label(
                RichText::new(format!("{} 本", app.shelf.len()))
                    .size(13.0)
                    .color(Color32::GRAY),
            );
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            if app.shelf.is_empty() {
                ui.add_space(60.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("书架还是空的，在详情页点「加入书架」").color(Color32::GRAY),
                    );
                });
                return;
            }
            ui.horizontal_wrapped(|ui| {
                for entry in &app.shelf {
                    let card = ui.allocate_ui(egui::vec2(126.0, 220.0), |ui| {
                        ui.vertical(|ui| {
                            widgets::cover(ui, &entry.cover, egui::vec2(118.0, 158.0));
                            ui.label(RichText::new(widgets::clamp(&entry.title, 16)).size(12.5));
                            ui.label(
                                RichText::new(widgets::clamp(&entry.chapter_title, 14))
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
        let _ = qieyue::remove_shelf(&id);
        app.shelf = qieyue::load_shelf();
    }
    if let Some(entry) = open {
        app.open_shelf_entry(entry);
    }
}

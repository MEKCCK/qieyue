//! Text reader with chapter navigation and shelf/audio actions.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::{App, View};

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let mut toggle = false;
    let mut listen = false;

    egui::TopBottomPanel::bottom("reader_nav").show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(app.chapter_index > 0, egui::Button::new("上一章"))
                .clicked()
            {
                app.chapter_index -= 1;
                app.open_chapter(app.chapter_index);
            }
            if ui
                .add_enabled(
                    app.chapter_index + 1 < app.chapters.len(),
                    egui::Button::new("下一章"),
                )
                .clicked()
            {
                app.chapter_index += 1;
                app.open_chapter(app.chapter_index);
            }
            if ui.button("目录").clicked() {
                app.go(View::Book);
            }
            let in_shelf = app
                .book
                .as_ref()
                .map(|book| app.on_shelf(&book.id))
                .unwrap_or(false);
            if ui
                .button(if in_shelf {
                    "已在书架"
                } else {
                    "加入书架"
                })
                .clicked()
            {
                toggle = true;
            }
            if ui.button("听书").clicked() {
                listen = true;
            }
            ui.separator();
            ui.label(RichText::new(&app.chapter_title).color(Color32::GRAY));
        });
        ui.add_space(4.0);
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let mut area = egui::ScrollArea::vertical();
        if app.reader_pos > 0.0 {
            area = area.vertical_scroll_offset(app.reader_pos);
        }
        let output = area.show(ui, |ui| {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(&app.chapter_title).size(20.0).strong());
            });
            ui.add_space(12.0);
            for paragraph in &app.paragraphs {
                ui.label(RichText::new(paragraph).size(17.0));
                ui.add_space(10.0);
            }
            ui.add_space(40.0);
        });
        app.reader_pos = output.state.offset.y;
    });

    if toggle {
        app.toggle_shelf();
    }
    if listen {
        app.play_audio();
    }
}

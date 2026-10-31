//! Text reader with a centred reading column and chapter navigation.

use eframe::egui::{self, RichText};

use crate::app::{App, View};
use crate::theme;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let mut toggle = false;
    let mut listen = false;

    egui::TopBottomPanel::bottom("reader_nav")
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(14, 8)),
        )
        .show(ctx, |ui| {
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
                ui.separator();
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(&app.chapter_title)
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                });
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::BG))
        .show(ctx, |ui| {
            let mut area = egui::ScrollArea::vertical();
            if app.reader_pos > 0.0 {
                area = area.vertical_scroll_offset(app.reader_pos);
            }
            let output = area.show(ui, |ui| {
                let width = (ui.available_width() - 48.0).clamp(320.0, 760.0);
                let margin = ((ui.available_width() - width) * 0.5).max(0.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(margin);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.add_space(22.0);
                        ui.label(RichText::new(&app.chapter_title).size(22.0).strong());
                        ui.add_space(18.0);
                        for paragraph in &app.paragraphs {
                            ui.label(RichText::new(paragraph).size(17.0));
                            ui.add_space(12.0);
                        }
                        ui.add_space(60.0);
                    });
                });
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

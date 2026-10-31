//! Comic reader: decrypted pages rendered as a centred vertical column.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::{App, View};
use crate::worker::Cmd;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("manga_nav").show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(app.chapter_index > 0, egui::Button::new("上一话"))
                .clicked()
            {
                app.chapter_index -= 1;
                if let Some(chapter) = app.chapters.get(app.chapter_index).cloned() {
                    app.send(Cmd::OpenManga(chapter.item_id), "解密漫画…");
                }
            }
            if ui
                .add_enabled(
                    app.chapter_index + 1 < app.chapters.len(),
                    egui::Button::new("下一话"),
                )
                .clicked()
            {
                app.chapter_index += 1;
                if let Some(chapter) = app.chapters.get(app.chapter_index).cloned() {
                    app.send(Cmd::OpenManga(chapter.item_id), "解密漫画…");
                }
            }
            if ui.button("目录").clicked() {
                app.go(View::Book);
            }
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "{} · {} 页",
                    app.chapters
                        .get(app.chapter_index)
                        .map(|chapter| chapter.title.clone())
                        .unwrap_or_default(),
                    app.manga_pages.len()
                ))
                .color(Color32::GRAY),
            );
        });
        ui.add_space(4.0);
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        if app.manga_scanned < app.manga_pages.len() {
            app.build_manga_textures(ctx);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Cap the column width and centre it, so a maximised window keeps the
            // pages in the middle instead of hugging the left edge.
            let width = (ui.available_width() - 24.0).min(900.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.set_width(ui.available_width());
                for texture in &app.manga_textures {
                    let size = texture.size_vec2();
                    let scale = (width / size.x).min(1.0);
                    ui.add(egui::Image::new((texture.id(), size * scale)));
                    ui.add_space(6.0);
                }
            });
        });
    });
}

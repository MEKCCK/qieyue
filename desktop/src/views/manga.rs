//! Comic reader: decrypted pages rendered incrementally as a centred column.

use eframe::egui::{self, RichText};

use crate::app::{App, View};
use crate::theme;
use crate::worker::Cmd;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("manga_nav")
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(14, 8)),
        )
        .show(ctx, |ui| {
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
                    .size(12.0)
                    .color(theme::MUTED),
                );
                if app.manga_scanned < app.manga_pages.len() {
                    ui.add(egui::Spinner::new().size(13.0));
                }
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::BG))
        .show(ctx, |ui| {
            if app.manga_scanned < app.manga_pages.len() {
                app.build_manga_textures(ctx);
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Cap the column width and centre it, so a maximised window keeps the
                // pages in the middle instead of hugging the left edge.
                let width = (ui.available_width() - 32.0).min(900.0);
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.set_width(ui.available_width());
                    ui.add_space(10.0);
                    for texture in &app.manga_textures {
                        let size = texture.size_vec2();
                        let scale = (width / size.x).min(1.0);
                        ui.add(egui::Image::new((texture.id(), size * scale)).corner_radius(6.0));
                        ui.add_space(8.0);
                    }
                });
            });
        });
}

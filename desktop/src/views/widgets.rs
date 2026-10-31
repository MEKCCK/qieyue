//! Small shared widgets and loading states.

use eframe::egui::{self, RichText, Vec2};

use crate::theme;

/// Cover thumbnail. An empty URL draws a neutral placeholder instead of a
/// broken image, so missing artwork never collapses the card layout.
pub(crate) fn cover(ui: &mut egui::Ui, url: &str, size: Vec2) {
    if url.is_empty() {
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), theme::CARD);
        return;
    }
    ui.add(
        egui::Image::new(url)
            .fit_to_exact_size(size)
            .corner_radius(6.0),
    );
}

/// Cuts a title to `limit` characters without splitting a multibyte glyph.
pub(crate) fn clamp(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// Full-content loading placeholder with a spinner and a short label.
pub(crate) fn loading(ctx: &egui::Context, text: &str) {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::BG))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(140.0);
                ui.add(egui::Spinner::new().size(22.0));
                ui.add_space(8.0);
                ui.label(RichText::new(text).size(13.0).color(theme::MUTED));
            });
        });
}

/// Thin separator drawn at the bottom of a virtualised row.
pub(crate) fn row_separator(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 1.0,
        egui::Stroke::new(1.0_f32, theme::LINE),
    );
}

/// Muted hint used by empty states.
pub(crate) fn empty_state(ui: &mut egui::Ui, text: &str) {
    ui.add_space(70.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(text).size(13.0).color(theme::MUTED));
    });
}

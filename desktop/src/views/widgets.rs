//! Small shared widgets.

use eframe::egui;
use egui::Color32;

/// Cover thumbnail. An empty URL draws a neutral placeholder instead of a
/// broken image, so a missing artwork never collapses the card layout.
pub(crate) fn cover(ui: &mut egui::Ui, url: &str, size: egui::Vec2) {
    if url.is_empty() {
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter().rect_filled(rect, 4.0, Color32::from_gray(40));
        return;
    }
    ui.add(
        egui::Image::new(url)
            .fit_to_exact_size(size)
            .corner_radius(4.0),
    );
}

/// Cuts a title to `limit` characters without splitting a multibyte glyph.
pub(crate) fn clamp(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

//! Landing screen shown before the first tab is selected.

use eframe::egui::{self, RichText};

use crate::app::App;
use crate::theme;

pub(crate) fn show(_app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::BG))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.add(egui::Spinner::new().size(22.0));
                ui.add_space(10.0);
                ui.label(RichText::new("茄阅").size(24.0).strong());
                ui.label(
                    RichText::new("小说阅读、书架、短剧播放")
                        .size(13.0)
                        .color(theme::MUTED),
                );
            });
        });
}

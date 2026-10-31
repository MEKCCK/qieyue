//! Landing screen shown before the first navigation.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::App;

pub(crate) fn show(_app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);
            ui.label(RichText::new("茄阅").size(26.0).strong());
            ui.label(RichText::new("小说阅读、书架、短剧播放（交给 mpv）").color(Color32::GRAY));
        });
    });
}

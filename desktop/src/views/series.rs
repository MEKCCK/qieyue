//! Short drama detail: episode list, shelf toggle, mpv playback.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::{App, View};
use crate::views::widgets;
use crate::worker::Cmd;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let Some(series) = app.series.clone() else {
        app.go(View::Results);
        return;
    };

    let mut play = None;
    let mut toggle_shelf_now = false;

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                widgets::cover(ui, &series.cover, egui::vec2(92.0, 124.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new(&series.title).size(22.0).strong());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("共 {} 集", series.episodes.len()))
                                .color(Color32::GRAY),
                        );
                        ui.separator();
                        let in_shelf = app.on_shelf(&series.id);
                        let label = if in_shelf {
                            "已在书架（点击移出）"
                        } else {
                            "加入书架"
                        };
                        if ui.button(label).clicked() {
                            toggle_shelf_now = true;
                        }
                    });
                    ui.label(RichText::new(&series.intro).size(13.0));
                });
            });
            ui.add_space(10.0);
            ui.separator();
            for episode in &series.episodes {
                if ui
                    .add(
                        egui::Button::new(RichText::new(format!(
                            "第 {} 集{}  ({}s)",
                            episode.index,
                            if episode.title.is_empty() {
                                String::new()
                            } else {
                                format!("  {}", episode.title)
                            },
                            episode.duration
                        )))
                        .min_size(egui::vec2(ui.available_width(), 0.0)),
                    )
                    .clicked()
                {
                    play = Some((episode.vid.clone(), episode.title.clone()));
                }
            }
        });
    });

    if toggle_shelf_now {
        app.toggle_series_shelf();
    }
    if let Some((vid, title)) = play {
        // Keep the resume point in step with what is being watched.
        if let Some(series) = app.series.clone() {
            if app.on_shelf(&series.id) {
                if let Some(episode) = series
                    .episodes
                    .iter()
                    .find(|episode| episode.vid == vid)
                    .cloned()
                {
                    qieyue::update_progress(
                        &series.id,
                        &vid,
                        &format!("第 {} 集", episode.index),
                        episode.index.saturating_sub(1),
                    );
                    app.shelf = qieyue::load_shelf();
                }
            }
        }
        app.send(Cmd::PlayEpisode { vid, title }, "解析视频地址…");
    }
}

//! Short drama detail: episode list, shelf toggle, mpv playback.

use eframe::egui::{self, RichText, Vec2};

use crate::app::{App, View};
use crate::theme;
use crate::views::widgets;
use crate::worker::Cmd;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if app.series.is_none() {
        app.go(View::Results);
        return;
    }

    let mut play: Option<(String, String, usize)> = None;
    let mut toggle = false;

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(22, 16)),
        )
        .show(ctx, |ui| {
            let Some(series) = app.series.as_ref() else {
                return;
            };
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    widgets::cover(ui, &series.cover, Vec2::new(104.0, 140.0));
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&series.title).size(24.0).strong());
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(format!("共 {} 集", series.episodes.len()))
                                .size(13.0)
                                .color(theme::MUTED),
                        );
                        ui.add_space(10.0);
                        let in_shelf = app.on_shelf(&series.id);
                        let label = if in_shelf {
                            "已在书架"
                        } else {
                            "加入书架"
                        };
                        if ui.button(label).clicked() {
                            toggle = true;
                        }
                    });
                });
                ui.add_space(12.0);
                ui.label(RichText::new(&series.intro).size(13.0).color(theme::MUTED));
                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);

                for episode in &series.episodes {
                    let label = format!(
                        "第 {} 集{}    {}s",
                        episode.index,
                        if episode.title.is_empty() {
                            String::new()
                        } else {
                            format!("  {}", episode.title)
                        },
                        episode.duration
                    );
                    if ui
                        .add(
                            egui::Button::new(RichText::new(label).size(14.0))
                                .min_size(egui::vec2(ui.available_width(), 34.0)),
                        )
                        .clicked()
                    {
                        play = Some((episode.vid.clone(), episode.title.clone(), episode.index));
                    }
                    ui.add_space(2.0);
                }
            });
        });

    if toggle {
        app.toggle_series_shelf();
    }
    if let Some((vid, title, index)) = play {
        let shelf_id = app.series.as_ref().map(|series| series.id.clone());
        if let Some(series_id) = shelf_id {
            if app.on_shelf(&series_id) {
                qieyue::update_progress(
                    &series_id,
                    &vid,
                    &format!("第 {index} 集"),
                    index.saturating_sub(1),
                );
                app.shelf = qieyue::load_shelf();
            }
        }
        app.send(Cmd::PlayEpisode { vid, title }, "解析视频地址…");
    }
}

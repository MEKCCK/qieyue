//! Rank boards: catalogue selectors and a paged ranked list.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::App;
use crate::views::widgets;
use crate::worker::{Cmd, RequestKind};

pub(crate) fn show(app: &mut App, ctx: &egui::Context, wheel_down: bool) {
    let Some(catalog) = app.rank.clone() else {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(RichText::new("加载榜单…").color(Color32::GRAY));
            });
        });
        return;
    };

    let mut switch: Option<(String, String)> = None;
    let mut clicked: Option<qieyue::Hit> = None;
    let mut load_more = false;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for board in &catalog.boards {
                let selected = board.algo == app.rank_algo;
                if ui
                    .selectable_label(selected, RichText::new(&board.name).size(15.0))
                    .clicked()
                    && !selected
                {
                    switch = Some((board.algo.clone(), app.rank_category.clone()));
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            for category in &catalog.categories {
                let selected = category.id == app.rank_category;
                if ui
                    .selectable_label(selected, RichText::new(&category.name).size(13.0))
                    .clicked()
                    && !selected
                {
                    switch = Some((app.rank_algo.clone(), category.id.clone()));
                }
            }
        });
        ui.separator();
        let output = egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, hit) in app.rank_hits.iter().enumerate() {
                let row = ui
                    .horizontal(|ui| {
                        let rank = format!("{:>2}", index + 1);
                        ui.label(RichText::new(rank).size(18.0).strong().color(if index < 3 {
                            Color32::from_rgb(220, 160, 60)
                        } else {
                            Color32::GRAY
                        }));
                        widgets::cover(ui, &hit.cover, egui::vec2(48.0, 64.0));
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&hit.title).size(15.0).strong());
                            ui.label(RichText::new(&hit.author).size(12.0).color(Color32::GRAY));
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                if row.clicked() {
                    clicked = Some(hit.clone());
                }
                ui.separator();
            }
            if app.rank_more {
                ui.add_space(6.0);
                if ui.button("加载更多").clicked() {
                    load_more = true;
                }
            } else if !app.rank_hits.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("到底了").size(12.0).color(Color32::GRAY));
            }
        });
        let scrollable = output.content_size.y > output.inner_rect.height() + 32.0;
        let offset = output.state.offset.y;
        let near_bottom = offset + output.inner_rect.height() >= output.content_size.y - 300.0;
        if app.rank_more && wheel_down && (near_bottom || !scrollable) {
            load_more = true;
        }
    });

    if let Some((algo, category)) = switch {
        app.rank_algo = algo.clone();
        app.rank_category = category.clone();
        app.rank_hits.clear();
        app.rank_more = true;
        let rank_id = app.rank_id.clone();
        app.send(
            Cmd::RankPage {
                rank_id,
                algo,
                category,
                offset: 0,
            },
            "加载榜单…",
        );
    } else if load_more && app.rank_more && !app.is_pending(RequestKind::RankPage) {
        let offset = app.rank_hits.len();
        let rank_id = app.rank_id.clone();
        app.send(
            Cmd::RankPage {
                rank_id,
                algo: app.rank_algo.clone(),
                category: app.rank_category.clone(),
                offset,
            },
            "加载更多…",
        );
    }
    if let Some(hit) = clicked {
        match hit.series {
            Some(series_id) => app.send(
                Cmd::OpenSeries {
                    id: series_id,
                    cover: hit.cover.clone(),
                },
                "加载剧集…",
            ),
            None => app.send(Cmd::OpenBook(hit.id), "加载目录…"),
        }
    }
}

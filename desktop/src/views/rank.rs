//! Rank boards: fixed selectors plus a virtualised, paged ranked list.

use eframe::egui::{self, Align, Layout, RichText, Sense, Vec2};

use crate::app::App;
use crate::theme;
use crate::views::widgets;
use crate::worker::{Cmd, RequestKind};

const ROW_HEIGHT: f32 = 72.0;

pub(crate) fn show(app: &mut App, ctx: &egui::Context, wheel_down: bool) {
    let mut switch: Option<(String, String)> = None;
    let mut clicked: Option<qieyue::Hit> = None;
    let mut load_more = false;

    let has_catalog = app.rank.is_some();
    if !has_catalog {
        widgets::loading(ctx, "加载榜单…");
        return;
    }

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(18, 12)),
        )
        .show(ctx, |ui| {
            let Some(catalog) = app.rank.as_ref() else {
                return;
            };
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

            if app.rank_hits.is_empty() {
                widgets::empty_state(ui, "还没有榜单数据");
                return;
            }

            let rows = app.rank_hits.len();
            let output =
                egui::ScrollArea::vertical().show_rows(ui, ROW_HEIGHT, rows, |ui, range| {
                    for index in range {
                        let hit = &app.rank_hits[index];
                        let response = ui
                            .allocate_ui_with_layout(
                                Vec2::new(ui.available_width(), ROW_HEIGHT),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    let color = if index < 3 { theme::GOLD } else { theme::MUTED };
                                    ui.add_sized(
                                        Vec2::new(34.0, ROW_HEIGHT),
                                        egui::Label::new(
                                            RichText::new(format!("{:>2}", index + 1))
                                                .size(18.0)
                                                .strong()
                                                .color(color),
                                        ),
                                    );
                                    widgets::cover(ui, &hit.cover, Vec2::new(48.0, 64.0));
                                    ui.add_space(12.0);
                                    ui.vertical(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&hit.title).size(15.0).strong(),
                                            )
                                            .truncate(),
                                        );
                                        if !hit.author.is_empty() {
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&hit.author)
                                                        .size(12.0)
                                                        .color(theme::MUTED),
                                                )
                                                .truncate(),
                                            );
                                        }
                                    });
                                },
                            )
                            .response
                            .interact(Sense::click());
                        widgets::row_separator(ui, response.rect);
                        if response.clicked() {
                            clicked = Some(hit.clone());
                        }
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

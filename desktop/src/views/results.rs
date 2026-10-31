//! Search results with offset paging, virtualised by row.

use eframe::egui::{self, Align, Layout, RichText, Sense, Vec2};

use crate::app::App;
use crate::theme;
use crate::views::{widgets, widgets::clamp};
use crate::worker::{Cmd, RequestKind};

const ROW_HEIGHT: f32 = 92.0;

pub(crate) fn show(app: &mut App, ctx: &egui::Context, wheel_down: bool) {
    let mut clicked: Option<qieyue::Hit> = None;
    let mut load_more = false;

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(18, 10)),
        )
        .show(ctx, |ui| {
            if app.hits.is_empty() {
                widgets::empty_state(ui, "没有结果，换个关键词试试");
                return;
            }

            let rows = app.hits.len() + usize::from(app.search_next.is_some());
            let output =
                egui::ScrollArea::vertical().show_rows(ui, ROW_HEIGHT, rows, |ui, range| {
                    for index in range {
                        if index >= app.hits.len() {
                            ui.allocate_ui_with_layout(
                                Vec2::new(ui.available_width(), ROW_HEIGHT),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    if ui.button("加载更多").clicked() {
                                        load_more = true;
                                    }
                                },
                            );
                            continue;
                        }

                        let hit = &app.hits[index];
                        let response = ui
                            .allocate_ui_with_layout(
                                Vec2::new(ui.available_width(), ROW_HEIGHT),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    ui.add_space(4.0);
                                    widgets::cover(ui, &hit.cover, Vec2::new(56.0, 76.0));
                                    ui.add_space(12.0);
                                    ui.vertical(|ui| {
                                        let tag = if hit.series.is_some() {
                                            "短剧 · "
                                        } else {
                                            ""
                                        };
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(format!("{tag}{}", hit.title))
                                                    .size(16.0)
                                                    .strong(),
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
                                        if !hit.summary.is_empty() {
                                            let mut snippet = clamp(&hit.summary, 110);
                                            if hit.summary.chars().count() > 110 {
                                                snippet.push('…');
                                            }
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(snippet)
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

            if app.search_next.is_some() {
                let scrollable = output.content_size.y > output.inner_rect.height() + 32.0;
                let offset = output.state.offset.y;
                let near_bottom =
                    offset + output.inner_rect.height() >= output.content_size.y - 300.0;
                if wheel_down && (near_bottom || !scrollable) {
                    load_more = true;
                }
            }
        });

    if load_more && !app.is_pending(RequestKind::Search) {
        if let Some(offset) = app.search_next {
            let query = app.search_query.clone();
            app.send(Cmd::Search { query, offset }, "加载更多…");
        }
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

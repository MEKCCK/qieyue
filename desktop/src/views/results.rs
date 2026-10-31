//! Search results with offset paging.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::App;
use crate::views::widgets;
use crate::worker::{Cmd, RequestKind};

pub(crate) fn show(app: &mut App, ctx: &egui::Context, wheel_down: bool) {
    let mut clicked: Option<qieyue::Hit> = None;
    let mut load_more = false;

    egui::CentralPanel::default().show(ctx, |ui| {
        let output = egui::ScrollArea::vertical().show(ui, |ui| {
            for hit in &app.hits {
                let response = ui
                    .horizontal(|ui| {
                        widgets::cover(ui, &hit.cover, egui::vec2(56.0, 76.0));
                        ui.vertical(|ui| {
                            let tag = if hit.series.is_some() {
                                "短剧 · "
                            } else {
                                ""
                            };
                            ui.label(
                                RichText::new(format!("{tag}{}", hit.title))
                                    .size(16.0)
                                    .strong(),
                            );
                            if !hit.author.is_empty() {
                                ui.label(
                                    RichText::new(&hit.author).size(12.0).color(Color32::GRAY),
                                );
                            }
                            if !hit.summary.is_empty() {
                                let mut snippet = widgets::clamp(&hit.summary, 110);
                                if hit.summary.chars().count() > 110 {
                                    snippet.push('…');
                                }
                                ui.label(RichText::new(snippet).size(12.0));
                            }
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                if response.clicked() {
                    clicked = Some(hit.clone());
                }
                ui.separator();
            }
            // The service pages in tens; keep pulling while the user is at the end.
            if app.search_next.is_some() {
                ui.add_space(6.0);
                if ui.button("加载更多").clicked() {
                    load_more = true;
                }
            }
        });
        if app.search_next.is_some() {
            let scrollable = output.content_size.y > output.inner_rect.height() + 32.0;
            let offset = output.state.offset.y;
            let near_bottom = offset + output.inner_rect.height() >= output.content_size.y - 300.0;
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

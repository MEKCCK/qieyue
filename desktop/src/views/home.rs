//! Home discovery feed: channel tabs plus paged recommendation blocks.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::App;
use crate::views::widgets;
use crate::worker::Cmd;

pub(crate) fn show(app: &mut App, ctx: &egui::Context, wheel_down: bool) {
    let Some(front) = app.home.clone() else {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(RichText::new("加载首页…").color(Color32::GRAY));
            });
        });
        return;
    };

    let mut clicked: Option<qieyue::Hit> = None;
    let mut switch_tab: Option<(u32, u32, String)> = None;
    let mut load_more: Option<(usize, String, u32, String, usize)> = None;
    let mut open_rank = false;
    let mut at_bottom = false;

    egui::CentralPanel::default().show(ctx, |ui| {
        let output = egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for tab in &front.tabs {
                    let selected = tab.index == app.home_tab_index;
                    if ui
                        .selectable_label(selected, RichText::new(&tab.title).size(14.0))
                        .clicked()
                        && !selected
                    {
                        switch_tab = Some((tab.tab_type, tab.index, tab.client_template.clone()));
                    }
                }
            });
            ui.separator();
            for section in &front.sections {
                ui.add_space(10.0);
                // The 排行榜 block is only a preview; the real boards live in the rank
                // page, so cap the cards and link there instead of pretending paging.
                let is_rank = section.title.contains("排行榜");
                let visible = if is_rank { 10 } else { section.hits.len() };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&section.title).size(17.0).strong());
                    if is_rank && ui.link("查看完整榜单 〉").clicked() {
                        open_rank = true;
                    }
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for hit in section.hits.iter().take(visible) {
                        let card = ui.allocate_ui(egui::vec2(126.0, 214.0), |ui| {
                            ui.vertical(|ui| {
                                widgets::cover(ui, &hit.cover, egui::vec2(118.0, 158.0));
                                ui.label(RichText::new(widgets::clamp(&hit.title, 16)).size(12.5));
                                if !hit.author.is_empty() {
                                    ui.label(
                                        RichText::new(widgets::clamp(&hit.author, 12))
                                            .size(11.0)
                                            .color(Color32::GRAY),
                                    );
                                }
                            });
                        });
                        if card.response.interact(egui::Sense::click()).clicked() {
                            clicked = Some(hit.clone());
                        }
                    }
                });
            }
            if let Some((index, section)) = front
                .sections
                .iter()
                .enumerate()
                .find(|(_, section)| section.next_offset.is_some())
            {
                ui.add_space(10.0);
                if ui.button("加载更多").clicked() {
                    load_more = Some((
                        index,
                        section.cell_id.clone(),
                        section.tab_type,
                        section.client_template.clone(),
                        section.next_offset.unwrap_or(0),
                    ));
                }
            }
        });
        // Guard against the "content shorter than the viewport" case: there the sum
        // is always past the threshold and the view would page forever.
        let scrollable = output.content_size.y > output.inner_rect.height() + 32.0;
        let offset = output.state.offset.y;
        let near_bottom = offset + output.inner_rect.height() >= output.content_size.y - 300.0;
        at_bottom = wheel_down && (near_bottom || !scrollable);
    });

    if load_more.is_none() && at_bottom {
        if let Some((index, section)) = front
            .sections
            .iter()
            .enumerate()
            .find(|(_, section)| section.next_offset.is_some())
        {
            load_more = Some((
                index,
                section.cell_id.clone(),
                section.tab_type,
                section.client_template.clone(),
                section.next_offset.unwrap_or(0),
            ));
        }
    }
    if let Some((section, cell_id, tab_type, client_template, offset)) = load_more {
        if !app.home_loading_more {
            app.home_loading_more = true;
            app.send(
                Cmd::HomeMore {
                    section,
                    cell_id,
                    tab_type,
                    client_template,
                    offset,
                },
                "加载更多…",
            );
        }
    }
    if open_rank {
        app.send(Cmd::RankCatalog, "加载榜单…");
    }
    if let Some((tab_type, index, template)) = switch_tab {
        app.home_tab_index = index;
        app.send(
            Cmd::HomeChannel {
                tab_type,
                tab_index: index,
                client_template: template,
            },
            "加载频道…",
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

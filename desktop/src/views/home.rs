//! Home discovery feed: fixed channel tabs plus a virtualised block list.
//!
//! Each block lays its cards out on a fixed grid, so a row that is outside the
//! viewport costs one `allocate_space` instead of a cover, a texture lookup and
//! three labels. The feed is borrowed from `App`; it is never cloned per frame.

use eframe::egui::{self, Align, Layout, Rect, RichText, Sense, Vec2};

use crate::app::{App, Tab};
use crate::theme;
use crate::views::{widgets, widgets::clamp};
use crate::worker::Cmd;

const CARD_W: f32 = 126.0;
const CARD_H: f32 = 214.0;
const GAP: f32 = 8.0;
const HEADER_H: f32 = 30.0;
const SECTION_GAP: f32 = 16.0;

pub(crate) fn show(app: &mut App, ctx: &egui::Context, wheel_down: bool) {
    let (switch_tab, load_more, open_rank, clicked) = {
        let Some(front) = app.home.as_ref() else {
            widgets::loading(ctx, "加载首页…");
            return;
        };

        let mut clicked: Option<qieyue::Hit> = None;
        let mut switch_tab: Option<(u32, u32, String)> = None;
        let mut load_more: Option<(usize, String, u32, String, usize)> = None;
        let mut open_rank = false;
        let mut at_bottom = false;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for tab in &front.tabs {
                        let selected = tab.index == app.home_tab_index;
                        if ui
                            .selectable_label(selected, RichText::new(&tab.title).size(14.0))
                            .clicked()
                            && !selected
                        {
                            switch_tab =
                                Some((tab.tab_type, tab.index, tab.client_template.clone()));
                        }
                    }
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                let width = ui.available_width();
                let columns = (((width + GAP) / (CARD_W + GAP)).floor() as usize).max(1);

                let output = egui::ScrollArea::vertical().show(ui, |ui| {
                    // Exact geometry: vertical spacing is added as part of each
                    // block's measured height instead of by the layout.
                    ui.spacing_mut().item_spacing = Vec2::new(GAP, 0.0);

                    for section in &front.sections {
                        let is_rank = section.title.contains("排行榜");
                        let visible = if is_rank { 10 } else { section.hits.len() };
                        let rows = visible.div_ceil(columns);
                        let height = HEADER_H
                            + rows as f32 * CARD_H
                            + rows.saturating_sub(1) as f32 * GAP
                            + SECTION_GAP;

                        let rect = Rect::from_min_size(ui.cursor().min, Vec2::new(width, height));
                        if !ui.is_rect_visible(rect) {
                            ui.allocate_space(Vec2::new(width, height));
                            continue;
                        }

                        ui.allocate_ui_with_layout(
                            Vec2::new(width, HEADER_H),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.label(RichText::new(&section.title).size(17.0).strong());
                                if is_rank && ui.link("查看完整榜单 〉").clicked() {
                                    open_rank = true;
                                }
                            },
                        );

                        for row in 0..rows {
                            let row_rect =
                                Rect::from_min_size(ui.cursor().min, Vec2::new(width, CARD_H));
                            if ui.is_rect_visible(row_rect) {
                                ui.horizontal(|ui| {
                                    for column in 0..columns {
                                        let index = row * columns + column;
                                        if index >= visible {
                                            break;
                                        }
                                        let Some(hit) = section.hits.get(index) else {
                                            break;
                                        };
                                        let response = ui
                                            .allocate_ui_with_layout(
                                                Vec2::new(CARD_W, CARD_H),
                                                Layout::top_down(Align::Center),
                                                |ui| card(ui, hit),
                                            )
                                            .response
                                            .interact(Sense::click());
                                        if response.clicked() {
                                            clicked = Some(hit.clone());
                                        }
                                    }
                                });
                            } else {
                                ui.allocate_space(Vec2::new(width, CARD_H));
                            }
                            if row + 1 < rows {
                                ui.add_space(GAP);
                            }
                        }
                        ui.add_space(SECTION_GAP);
                    }

                    if let Some((index, section)) = front
                        .sections
                        .iter()
                        .enumerate()
                        .find(|(_, section)| section.next_offset.is_some())
                    {
                        ui.add_space(6.0);
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

                let scrollable = output.content_size.y > output.inner_rect.height() + 32.0;
                let offset = output.state.offset.y;
                let near_bottom =
                    offset + output.inner_rect.height() >= output.content_size.y - 300.0;
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

        (switch_tab, load_more, open_rank, clicked)
    };

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
        app.select_tab(Tab::Rank);
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

fn card(ui: &mut egui::Ui, hit: &qieyue::Hit) {
    ui.add_space(3.0);
    widgets::cover(ui, &hit.cover, Vec2::new(118.0, 158.0));
    ui.add_space(5.0);
    ui.add(egui::Label::new(RichText::new(clamp(&hit.title, 16)).size(12.5)).truncate());
    if !hit.author.is_empty() {
        ui.add(
            egui::Label::new(
                RichText::new(clamp(&hit.author, 12))
                    .size(11.0)
                    .color(theme::MUTED),
            )
            .truncate(),
        );
    }
}

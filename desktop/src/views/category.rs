//! Category page: channel tabs plus tag dimensions.

use eframe::egui::{self, RichText};

use crate::app::App;
use crate::theme;
use crate::views::widgets;
use crate::worker::Cmd;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if app.category.is_none() {
        widgets::loading(ctx, "加载分类…");
        return;
    }

    let mut switch_tab = None;
    let mut search_tag = None;

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(18, 12)),
        )
        .show(ctx, |ui| {
            let Some(front) = app.category.as_ref() else {
                return;
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("频道").size(13.0).color(theme::MUTED));
                for tab in &front.tabs {
                    let value: u32 = tab.value.parse().unwrap_or(1);
                    let selected = app.category_tab == value;
                    if ui
                        .selectable_label(selected, RichText::new(&tab.name).size(15.0))
                        .clicked()
                        && !selected
                    {
                        switch_tab = Some(value);
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Groups follow `rule.dim`; tiles whose dimension is unknown land in 其他.
                let mut groups: Vec<(String, Vec<&qieyue::CategoryTile>)> = Vec::new();
                for dim in &front.dims {
                    if dim.value == "0" {
                        continue;
                    }
                    let tiles: Vec<&qieyue::CategoryTile> = front
                        .tiles
                        .iter()
                        .filter(|tile| tile.dim == dim.value)
                        .collect();
                    if !tiles.is_empty() {
                        groups.push((dim.name.clone(), tiles));
                    }
                }
                let known: Vec<&str> = front.dims.iter().map(|dim| dim.value.as_str()).collect();
                let rest: Vec<&qieyue::CategoryTile> = front
                    .tiles
                    .iter()
                    .filter(|tile| !known.contains(&tile.dim.as_str()) || tile.dim == "0")
                    .collect();
                if !rest.is_empty() {
                    groups.insert(0, ("全部".to_string(), rest));
                }

                for (name, tiles) in &groups {
                    ui.add_space(14.0);
                    ui.label(RichText::new(name).size(12.0).color(theme::MUTED));
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for tile in tiles {
                            let label = if tile.hot {
                                format!("🔥 {}", tile.name)
                            } else {
                                tile.name.clone()
                            };
                            if ui
                                .add(egui::Button::new(RichText::new(label).size(14.0)))
                                .clicked()
                            {
                                search_tag = Some(tile.name.clone());
                            }
                        }
                    });
                }
                ui.add_space(24.0);
            });
        });

    if let Some(tab) = switch_tab {
        app.category_tab = tab;
        app.send(Cmd::Category(tab), "加载分类…");
    }
    if let Some(tag) = search_tag {
        app.query = tag.clone();
        app.search_query = tag.clone();
        app.send(
            Cmd::Search {
                query: tag,
                offset: 0,
            },
            "搜索中…",
        );
    }
}

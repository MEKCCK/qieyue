//! Category page: channel tabs plus tag dimensions.

use eframe::egui;
use egui::{Color32, RichText};

use crate::app::{App, View};
use crate::worker::Cmd;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let Some(front) = app.category.clone() else {
        app.go(View::Results);
        return;
    };

    let mut switch_tab = None;
    let mut search_tag = None;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("频道").strong());
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
                ui.add_space(10.0);
                ui.label(RichText::new(name).size(12.0).color(Color32::GRAY));
                ui.horizontal_wrapped(|ui| {
                    for tile in tiles {
                        let label = if tile.hot {
                            format!("🔥 {}", tile.name)
                        } else {
                            tile.name.clone()
                        };
                        if ui.button(RichText::new(label).size(14.0)).clicked() {
                            search_tag = Some(tile.name.clone());
                        }
                    }
                });
            }
            ui.add_space(20.0);
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

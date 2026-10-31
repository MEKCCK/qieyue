//! Global top bar: search box, main navigation, back, status line.

use eframe::egui;
use egui::RichText;

use crate::app::{status_color, App, View};
use crate::worker::Cmd;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top").show(ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("茄阅").strong());
            let edit = ui.add(
                egui::TextEdit::singleline(&mut app.query)
                    .hint_text("搜索书名 / 作者 / 短剧")
                    .desired_width(320.0),
            );
            let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.button("搜索").clicked() || submitted) && !app.query.trim().is_empty() {
                let query = app.query.trim().to_string();
                app.search_query = query.clone();
                app.send(Cmd::Search { query, offset: 0 }, "搜索中…");
            }
            if ui.button("书架").clicked() {
                app.shelf = qieyue::load_shelf();
                app.go(View::Library);
            }
            if ui.button("首页").clicked() {
                app.send(Cmd::Home, "加载首页…");
            }
            if ui.button("排行").clicked() {
                app.send(Cmd::RankCatalog, "加载榜单…");
            }
            if ui.button("分类").clicked() {
                let tab = app.category_tab;
                app.send(Cmd::Category(tab), "加载分类…");
            }
            if app.view != View::Empty && ui.button("返回").clicked() {
                app.go_back();
            }
            ui.separator();
            ui.label(RichText::new(&app.status).color(status_color(app.loading)));
        });
        ui.add_space(4.0);
    });
}

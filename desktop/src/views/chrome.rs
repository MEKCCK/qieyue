//! Top bar above the content area: back, page title, global search, status.

use eframe::egui::{self, Align, Color32, Layout, RichText};

use crate::app::App;
use crate::theme;
use crate::worker::Cmd;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top")
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(16, 10)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if app.can_go_back() {
                    if ui
                        .add(egui::Button::new(RichText::new("‹ 返回").size(13.0)))
                        .clicked()
                    {
                        app.go_back();
                    }
                    ui.add_space(2.0);
                }

                ui.label(RichText::new(app.page_title()).size(18.0).strong());
                ui.add_space(14.0);

                let edit = ui.add(
                    egui::TextEdit::singleline(&mut app.query)
                        .hint_text("搜索书名 / 作者 / 短剧")
                        .desired_width(340.0),
                );
                let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let clicked = ui.add(egui::Button::new(RichText::new("搜索"))).clicked();
                if (clicked || submitted) && !app.query.trim().is_empty() {
                    let query = app.query.trim().to_string();
                    app.search_query = query.clone();
                    app.send(Cmd::Search { query, offset: 0 }, "搜索中…");
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if app.loading {
                        ui.add(egui::Spinner::new().size(14.0));
                    }
                    ui.label(RichText::new(&app.status).size(12.0).color(if app.loading {
                        theme::GOLD
                    } else {
                        Color32::from_gray(140)
                    }));
                });
            });
        });
}

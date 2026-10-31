//! Left navigation rail: brand mark, four root tabs, version footer.

use eframe::egui::{
    self, Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, Stroke, Vec2,
};

use crate::app::{App, Tab};
use crate::theme;

const RAIL_WIDTH: f32 = 178.0;
const ITEM_HEIGHT: f32 = 40.0;

pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::left("nav")
        .resizable(false)
        .exact_width(RAIL_WIDTH)
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(12, 14)),
        )
        .show(ctx, |ui| {
            brand(ui, app);
            ui.add_space(18.0);

            let mut selected = None;
            for tab in Tab::ALL {
                if item(ui, app, tab) {
                    selected = Some(tab);
                }
                ui.add_space(2.0);
            }

            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("Linux · macOS").small().color(theme::MUTED));
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .small()
                        .color(theme::MUTED),
                );
            });

            if let Some(tab) = selected {
                app.select_tab(tab);
            }
        });
}

fn brand(ui: &mut egui::Ui, app: &App) {
    ui.horizontal(|ui| {
        ui.add(egui::Image::new((app.logo.id(), Vec2::splat(30.0))).corner_radius(8.0));
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.add_space(1.0);
            ui.label(RichText::new("茄阅").size(18.0).strong());
            ui.label(RichText::new("QieYue").size(10.0).color(theme::MUTED));
        });
    });
}

fn item(ui: &mut egui::Ui, app: &App, tab: Tab) -> bool {
    let selected = app.tab == tab;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), ITEM_HEIGHT), Sense::click());
    let hovered = response.hovered();

    if selected || hovered {
        let fill = if selected {
            theme::ACCENT.gamma_multiply(0.16)
        } else {
            theme::CARD
        };
        ui.painter().rect_filled(rect, CornerRadius::same(9), fill);
    }
    if selected {
        let bar = Rect::from_min_size(
            rect.left_top() + Vec2::new(0.0, 9.0),
            Vec2::new(3.0, ITEM_HEIGHT - 18.0),
        );
        ui.painter()
            .rect_filled(bar, CornerRadius::same(2), theme::ACCENT);
    }

    let color = if selected {
        theme::ACCENT
    } else if hovered {
        theme::TEXT
    } else {
        theme::MUTED
    };
    let icon_rect =
        Rect::from_center_size(rect.left_center() + Vec2::new(23.0, 0.0), Vec2::splat(18.0));
    draw_icon(ui.painter(), icon_rect, tab, color);
    ui.painter().text(
        rect.left_center() + Vec2::new(45.0, 0.0),
        egui::Align2::LEFT_CENTER,
        tab.label(),
        egui::FontId::proportional(15.0),
        color,
    );

    response.clicked()
}

fn draw_icon(painter: &egui::Painter, rect: Rect, tab: Tab, color: Color32) {
    let p = |x: f32, y: f32| -> egui::Pos2 {
        egui::pos2(
            rect.left() + rect.width() * x,
            rect.top() + rect.height() * y,
        )
    };
    let stroke = Stroke::new(1.7_f32, color);

    match tab {
        Tab::Home => {
            painter.line_segment([p(0.10, 0.52), p(0.50, 0.16)], stroke);
            painter.line_segment([p(0.50, 0.16), p(0.90, 0.52)], stroke);
            painter.line_segment([p(0.22, 0.46), p(0.22, 0.86)], stroke);
            painter.line_segment([p(0.78, 0.46), p(0.78, 0.86)], stroke);
            painter.line_segment([p(0.22, 0.86), p(0.78, 0.86)], stroke);
            painter.rect_filled(
                Rect::from_min_max(p(0.43, 0.60), p(0.57, 0.86)),
                CornerRadius::same(1),
                color,
            );
        }
        Tab::Library => {
            for (x, top) in [(0.14, 0.26), (0.41, 0.36), (0.68, 0.18)] {
                painter.rect_filled(
                    Rect::from_min_max(p(x, top), p(x + 0.18, 0.86)),
                    CornerRadius::same(2),
                    color,
                );
            }
        }
        Tab::Rank => {
            for (x, top) in [(0.14, 0.56), (0.41, 0.36), (0.68, 0.16)] {
                painter.rect_filled(
                    Rect::from_min_max(p(x, top), p(x + 0.18, 0.86)),
                    CornerRadius::same(2),
                    color,
                );
            }
        }
        Tab::Category => {
            for (x, y) in [(0.14, 0.14), (0.55, 0.14), (0.14, 0.55), (0.55, 0.55)] {
                painter.rect_filled(
                    Rect::from_min_max(p(x, y), p(x + 0.31, y + 0.31)),
                    CornerRadius::same(2),
                    color,
                );
            }
        }
    }
}

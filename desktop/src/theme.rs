//! Visual language for the desktop client.
//!
//! One warm accent on a near-black neutral base. Everything else (cards,
//! rails, buttons) is a step of the same neutral scale, so the accent always
//! means "selected" or "primary action".

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

pub(crate) const BG: Color32 = Color32::from_rgb(15, 15, 18);
pub(crate) const PANEL: Color32 = Color32::from_rgb(21, 21, 25);
pub(crate) const CARD: Color32 = Color32::from_rgb(30, 30, 36);
pub(crate) const CARD_HOVER: Color32 = Color32::from_rgb(38, 38, 45);
pub(crate) const LINE: Color32 = Color32::from_rgb(43, 43, 50);
pub(crate) const TEXT: Color32 = Color32::from_rgb(236, 234, 228);
pub(crate) const MUTED: Color32 = Color32::from_rgb(150, 148, 140);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(239, 75, 55);
pub(crate) const GOLD: Color32 = Color32::from_rgb(246, 180, 76);

pub(crate) fn install(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(10, 10, 12);
    visuals.faint_bg_color = CARD;
    visuals.code_bg_color = CARD;
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.30);
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.window_stroke = Stroke::new(1.0_f32, LINE);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = PANEL;
    widgets.noninteractive.weak_bg_fill = PANEL;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.noninteractive.corner_radius = CornerRadius::same(8);

    widgets.inactive.bg_fill = CARD;
    widgets.inactive.weak_bg_fill = CARD;
    widgets.inactive.bg_stroke = Stroke::NONE;
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.inactive.corner_radius = CornerRadius::same(8);

    widgets.hovered.bg_fill = CARD_HOVER;
    widgets.hovered.weak_bg_fill = CARD_HOVER;
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.55));
    widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.hovered.corner_radius = CornerRadius::same(8);

    widgets.active.bg_fill = ACCENT.gamma_multiply(0.22);
    widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.22);
    widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.active.corner_radius = CornerRadius::same(8);

    widgets.open = widgets.active;

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.scroll.floating = true;
    style.spacing.scroll.bar_width = 7.0;
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(22.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    ctx.set_style(style);
}

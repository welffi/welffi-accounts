use crate::theme::{self, ACCENT, ACCENT_HOVER, ACCENT_INK, FAINT, LINE, MUTED, RAISED, RAISED_HOVER, SURFACE, TEXT};
use eframe::egui::{self, Align, Color32, CornerRadius, CursorIcon, FontId, Margin, Response, RichText, Sense, Stroke, StrokeKind, Vec2};

pub const HEIGHT: f32 = 34.0;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Accent,
    Plain,
    Ghost,
}

pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

pub fn title(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).family(theme::semibold()).size(15.0));
}

pub fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.5).color(MUTED));
}

pub fn button(ui: &mut egui::Ui, label: &str, kind: Kind, enabled: bool, width: f32) -> Response {
    let font = FontId::new(13.5, theme::semibold());
    let text_width = ui.painter().layout_no_wrap(label.to_string(), font.clone(), TEXT).size().x;
    let size = Vec2::new(width.max(text_width + 28.0), HEIGHT);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let hovered = enabled && response.hovered();
    let (fill, text) = match kind {
        Kind::Accent => (if hovered { ACCENT_HOVER } else { ACCENT }, ACCENT_INK),
        Kind::Plain => (if hovered { RAISED_HOVER } else { RAISED }, TEXT),
        Kind::Ghost => (if hovered { RAISED } else { Color32::TRANSPARENT }, MUTED),
    };
    let fade = if enabled { 1.0 } else { 0.45 };
    ui.painter().rect_filled(rect, CornerRadius::same(10), fill.gamma_multiply(fade));
    theme::centered_text(ui.painter(), rect.center(), label, font, text.gamma_multiply(fade));
    if enabled { response.on_hover_cursor(CursorIcon::PointingHand) } else { response }
}

pub fn icon_button(ui: &mut egui::Ui, glyph: &str, tip: &str, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(30.0), sense);
    let hovered = enabled && response.hovered();
    if hovered {
        ui.painter().rect_filled(rect, CornerRadius::same(8), RAISED_HOVER);
    }
    let color = if hovered { TEXT } else if enabled { MUTED } else { FAINT };
    theme::centered_text(ui.painter(), rect.center(), glyph, FontId::new(13.0, theme::icons()), color);
    let response = response.on_hover_text(tip);
    if enabled { response.on_hover_cursor(CursorIcon::PointingHand) } else { response }
}

pub fn text_box(ui: &mut egui::Ui, text: &mut String, hint: &str, limit: usize, width: f32) -> Response {
    let edit = egui::TextEdit::singleline(text)
        .hint_text(RichText::new(hint).color(FAINT))
        .char_limit(limit)
        .desired_width(width)
        .margin(Margin::symmetric(10, 8))
        .background_color(RAISED)
        .font(FontId::proportional(14.0));
    ui.add(edit)
}

pub fn switch(ui: &mut egui::Ui, on: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(40.0, 22.0), Sense::click());
    let progress = ui.ctx().animate_bool(response.id, on);
    let track = if on { ACCENT } else if response.hovered() { RAISED_HOVER } else { RAISED };
    ui.painter().rect(rect, CornerRadius::same(11), track, Stroke::new(1.0, if on { ACCENT } else { LINE }), StrokeKind::Inside);
    let x = egui::lerp(rect.left() + 11.0..=rect.right() - 11.0, progress);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), 7.0, if on { ACCENT_INK } else { MUTED });
    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn setting_row(ui: &mut egui::Ui, name: &str, about: &str, control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(name).size(14.0));
            hint(ui, about);
        });
        ui.with_layout(egui::Layout::right_to_left(Align::Center), control);
    });
}

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{self, Align, Color32, FontData, FontDefinitions, FontFamily, FontId, IconData, Mesh, Painter, Pos2, Rect, Stroke, TextFormat, Vec2};
use std::sync::Arc;

pub const INK: Color32 = Color32::from_rgb(0x0F, 0x10, 0x13);
pub const SURFACE: Color32 = Color32::from_rgb(0x17, 0x19, 0x1E);
pub const RAISED: Color32 = Color32::from_rgb(0x1F, 0x22, 0x29);
pub const RAISED_HOVER: Color32 = Color32::from_rgb(0x2D, 0x30, 0x39);
pub const LINE: Color32 = Color32::from_rgb(0x26, 0x29, 0x31);
pub const TEXT: Color32 = Color32::from_rgb(0xF1, 0xEF, 0xEA);
pub const MUTED: Color32 = Color32::from_rgb(0x9A, 0x9F, 0xAA);
pub const FAINT: Color32 = Color32::from_rgb(0x6A, 0x6F, 0x7A);
pub const ACCENT: Color32 = Color32::from_rgb(0xF2, 0xB5, 0x44);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0xFF, 0xCB, 0x66);
pub const ACCENT_LINE: Color32 = Color32::from_rgb(0x5C, 0x48, 0x1E);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0x2A, 0x24, 0x15);
pub const ACCENT_INK: Color32 = Color32::from_rgb(0x20, 0x17, 0x03);
pub const DANGER: Color32 = Color32::from_rgb(0xF0, 0x7A, 0x72);
pub const GOOD: Color32 = Color32::from_rgb(0x6A, 0xD3, 0x9A);

pub fn centered_text(painter: &Painter, center: Pos2, text: impl ToString, font: FontId, color: Color32) {
    let galley = painter.layout_no_wrap(text.to_string(), font, color);
    let origin = center - galley.mesh_bounds.center().to_vec2();
    painter.galley(origin, galley, color);
}

pub fn truncated_text(painter: &Painter, rect: Rect, align: Align, text: &str, font: FontId, color: Color32) {
    let mut job = LayoutJob::single_section(text.to_string(), TextFormat::simple(font, color));
    job.wrap = TextWrapping::truncate_at_width(rect.width());
    job.halign = align;
    let galley = painter.layout_job(job);
    let x = match align {
        Align::Min => rect.left(),
        Align::Center => rect.center().x,
        Align::Max => rect.right(),
    };
    painter.galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, color);
}

pub fn glow(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    const STEPS: u32 = 72;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    for step in 0..=STEPS {
        let angle = step as f32 / STEPS as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(center + Vec2::angled(angle) * radius, Color32::TRANSPARENT);
    }
    for step in 0..STEPS {
        mesh.add_triangle(0, step + 1, step + 2);
    }
    painter.add(mesh);
}

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

pub fn icons() -> FontFamily {
    FontFamily::Name("icons".into())
}

pub fn apply(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    add_system_font(&mut fonts, "segoeui.ttf", FontFamily::Proportional);
    for family in [semibold(), icons()] {
        fonts.families.insert(family, fallback.clone());
    }
    add_system_font(&mut fonts, "seguisb.ttf", semibold());
    add_system_font(&mut fonts, "SegoeIcons.ttf", icons());
    ctx.set_fonts(fonts);

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = INK;
    visuals.override_text_color = Some(TEXT);
    visuals.extreme_bg_color = RAISED;
    visuals.text_cursor.stroke = Stroke::new(2.0, ACCENT);
    visuals.selection.bg_fill = ACCENT_LINE;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, RAISED_HOVER);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_LINE);
    visuals.window_fill = SURFACE;
    visuals.window_stroke = Stroke::new(1.0, LINE);
    ctx.set_visuals(visuals);
}

fn add_system_font(fonts: &mut FontDefinitions, file: &str, family: FontFamily) {
    let Ok(bytes) = std::fs::read(std::path::Path::new(r"C:\Windows\Fonts").join(file)) else {
        return;
    };
    fonts.font_data.insert(file.to_string(), Arc::new(FontData::from_owned(bytes)));
    fonts.families.entry(family).or_default().insert(0, file.to_string());
}

pub fn window_icon() -> IconData {
    IconData { rgba: include_bytes!("../assets/icon64.rgba").to_vec(), width: 64, height: 64 }
}

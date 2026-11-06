use crate::app::{App, Confirm};
use crate::theme::{self, ACCENT, ACCENT_INK, ACCENT_LINE, ACCENT_SOFT, FAINT, LINE, MUTED, RAISED, RAISED_HOVER, TEXT};
use crate::widgets::{self, Kind};
use eframe::egui::epaint::Vertex;
use eframe::egui::scroll_area::{DragScroll, ScrollSource};
use eframe::egui::{self, Align, Color32, CornerRadius, CursorIcon, FontId, Layout, Mesh, Painter, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextureId, Vec2};

const MIN_TILE: f32 = 150.0;
const TILE_HEIGHT: f32 = 118.0;
const GAP: f32 = 8.0;
const PITCH: f32 = TILE_HEIGHT + GAP;
const PORTRAIT: f32 = 48.0;
const EDGE_ZONE: f32 = 30.0;
const EDGE_STEP: f32 = 16.0;

pub fn columns_for(width: f32) -> usize {
    ((width / MIN_TILE) as usize).max(1)
}

pub fn tile_index_at(x: f32, y: f32, width: f32, columns: usize, count: usize) -> usize {
    let rows = count.div_ceil(columns);
    let column = ((x / (width / columns as f32)).max(0.0) as usize).min(columns - 1);
    let row = ((y / PITCH).max(0.0) as usize).min(rows - 1);
    (row * columns + column).min(count - 1)
}

fn circle_image(painter: &Painter, texture: TextureId, center: Pos2, radius: f32) {
    const STEPS: u32 = 48;
    let mut mesh = Mesh::with_texture(texture);
    mesh.vertices.push(Vertex { pos: center, uv: Pos2::new(0.5, 0.5), color: Color32::WHITE });
    for step in 0..=STEPS {
        let direction = Vec2::angled(step as f32 / STEPS as f32 * std::f32::consts::TAU);
        mesh.vertices.push(Vertex { pos: center + direction * radius, uv: Pos2::new(0.5, 0.5) + direction * 0.5, color: Color32::WHITE });
    }
    for step in 0..STEPS {
        mesh.add_triangle(0, step + 1, step + 2);
    }
    painter.add(mesh);
}

impl App {
    pub fn accounts_card(&mut self, ui: &mut egui::Ui) {
        widgets::card(ui, |ui| {
            ui.set_min_height(ui.available_height());
            let count = self.store.accounts.len();
            ui.horizontal(|ui| {
                widgets::title(ui, "Accounts");
                if count > 0 {
                    let ticked = self.store.accounts.iter().filter(|account| self.ticked.contains(&account.id)).count();
                    ui.label(RichText::new(format!("{ticked} of {count} ticked")).size(12.5).color(FAINT));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, "Add account", Kind::Accent, !self.busy, 0.0).clicked() {
                        self.add_account();
                    }
                    if count > 0 && widgets::button(ui, "All / none", Kind::Ghost, true, 0.0).clicked() {
                        self.toggle_all();
                    }
                });
            });
            ui.add_space(14.0);
            if count == 0 {
                self.empty_state(ui);
                return;
            }
            egui::ScrollArea::vertical()
                .scroll_source(ScrollSource { drag: DragScroll::Never, ..ScrollSource::default() })
                .auto_shrink([false, false])
                .show(ui, |ui| self.tiles(ui));
        });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() - 190.0).max(0.0) / 2.0);
            ui.label(RichText::new("\u{E716}").family(theme::icons()).size(34.0).color(FAINT));
            ui.add_space(12.0);
            widgets::title(ui, "No accounts yet");
            ui.add_space(4.0);
            ui.label(RichText::new("Add one by logging in, or bring over\nthe accounts you saved in welffi's macros.").size(13.0).color(MUTED));
            ui.add_space(16.0);
            if widgets::button(ui, "Import from macro", Kind::Plain, !self.busy, 0.0).clicked() {
                self.import();
            }
        });
    }

    fn follow_drag(&mut self, ui: &egui::Ui, area: Rect, columns: usize) {
        let (Some(id), Some(pointer)) = (self.dragging.clone(), ui.ctx().pointer_latest_pos()) else {
            return;
        };
        let count = self.store.accounts.len();
        let local = pointer - area.min;
        let target = tile_index_at(local.x, local.y, area.width(), columns, count);
        if let Some(current) = self.store.accounts.iter().position(|account| account.id == id) {
            if current != target {
                let account = self.store.accounts.remove(current);
                self.store.accounts.insert(target, account);
            }
        }
        let view = ui.clip_rect();
        if pointer.y < view.top() + EDGE_ZONE {
            ui.scroll_with_delta(Vec2::new(0.0, EDGE_STEP));
        } else if pointer.y > view.bottom() - EDGE_ZONE {
            ui.scroll_with_delta(Vec2::new(0.0, -EDGE_STEP));
        }
        ui.ctx().request_repaint();
    }

    fn tiles(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let columns = columns_for(width);
        let tile_width = (width - GAP * (columns - 1) as f32) / columns as f32;
        let rows = self.store.accounts.len().div_ceil(columns);
        let (area, _) = ui.allocate_exact_size(Vec2::new(width, rows as f32 * PITCH - GAP), Sense::hover());
        self.follow_drag(ui, area, columns);

        let accounts: Vec<(String, String)> = self.store.accounts.iter().map(|account| (account.id.clone(), account.name.clone())).collect();
        for (index, (id, name)) in accounts.iter().enumerate() {
            let offset = Vec2::new((index % columns) as f32 * (tile_width + GAP), (index / columns) as f32 * PITCH);
            let rect = Rect::from_min_size(area.min + offset, Vec2::new(tile_width, TILE_HEIGHT));
            let response = ui.interact(rect, ui.scope_id().with(("tile", id)), Sense::click_and_drag());
            if response.drag_started() {
                self.dragging = Some(id.clone());
            }
            if response.drag_stopped() {
                self.dragging = None;
                self.save_order();
            }
            if response.clicked() && !self.ticked.remove(id) {
                self.ticked.insert(id.clone());
            }
            let dragging = self.dragging.as_ref() == Some(id);
            let hovered = ui.rect_contains_pointer(rect) && self.dragging.is_none();
            self.paint_tile(ui, rect, id, name, hovered, dragging);
            if hovered {
                let remove = Rect::from_min_size(Pos2::new(rect.right() - 34.0, rect.top() + 6.0), Vec2::splat(28.0));
                let button = ui.interact(remove, ui.scope_id().with(("remove", id)), Sense::click()).on_hover_text("Remove");
                if button.hovered() {
                    ui.painter().rect_filled(remove, CornerRadius::same(8), RAISED);
                }
                theme::centered_text(ui.painter(), remove.center(), "\u{E74D}", FontId::new(12.0, theme::icons()), if button.hovered() { TEXT } else { MUTED });
                if button.clicked() && !self.busy {
                    self.confirm = Some(Confirm::RemoveAccount(id.clone()));
                }
                button.on_hover_cursor(CursorIcon::PointingHand);
            }
            response.on_hover_cursor(if dragging { CursorIcon::Grabbing } else { CursorIcon::PointingHand });
        }
    }

    fn paint_tile(&mut self, ui: &egui::Ui, rect: Rect, id: &str, name: &str, hovered: bool, dragging: bool) {
        let ticked = self.ticked.contains(id);
        let fill = match (dragging, ticked, hovered) {
            (true, _, _) => RAISED_HOVER.gamma_multiply(0.7),
            (_, true, _) => ACCENT_SOFT,
            (_, _, true) => RAISED_HOVER,
            _ => RAISED,
        };
        let stroke = if ticked { Stroke::new(1.0, ACCENT) } else if dragging { Stroke::new(1.0, ACCENT_LINE) } else { Stroke::new(1.0, LINE) };
        let painter = ui.painter();
        painter.rect(rect, CornerRadius::same(12), fill, stroke, StrokeKind::Inside);

        let tick = Rect::from_min_size(rect.min + Vec2::splat(10.0), Vec2::splat(18.0));
        if ticked {
            painter.rect_filled(tick, CornerRadius::same(5), ACCENT);
            theme::centered_text(painter, tick.center(), "\u{E73E}", FontId::new(11.0, theme::icons()), ACCENT_INK);
        } else {
            painter.rect_stroke(tick, CornerRadius::same(5), Stroke::new(1.5, FAINT), StrokeKind::Inside);
        }

        let center = Pos2::new(rect.center().x, rect.top() + 16.0 + PORTRAIT / 2.0);
        match self.portraits.get(ui.ctx(), id) {
            Some(texture) => {
                painter.circle_filled(center, PORTRAIT / 2.0, LINE);
                circle_image(painter, texture.id(), center, PORTRAIT / 2.0);
                painter.circle_stroke(center, PORTRAIT / 2.0, Stroke::new(1.0, fill));
            }
            None => {
                painter.circle_filled(center, PORTRAIT / 2.0, ACCENT_LINE);
                let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
                theme::centered_text(painter, center, initial, FontId::new(20.0, theme::semibold()), ACCENT);
            }
        }

        let line = |top: f32, height: f32| Rect::from_min_size(Pos2::new(rect.left() + 10.0, rect.top() + top), Vec2::new(rect.width() - 20.0, height));
        theme::truncated_text(painter, line(70.0, 20.0), Align::Center, name, FontId::new(14.0, theme::semibold()), TEXT);
        theme::truncated_text(painter, line(91.0, 16.0), Align::Center, self.note(id), FontId::proportional(12.0), MUTED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_the_width() {
        assert_eq!(columns_for(100.0), 1);
        assert_eq!(columns_for(300.0), 2);
        assert_eq!(columns_for(620.0), 4);
    }

    #[test]
    fn pointer_maps_to_a_tile() {
        assert_eq!(tile_index_at(10.0, 10.0, 600.0, 4, 10), 0);
        assert_eq!(tile_index_at(160.0, 10.0, 600.0, 4, 10), 1);
        assert_eq!(tile_index_at(10.0, PITCH + 1.0, 600.0, 4, 10), 4);
        assert_eq!(tile_index_at(590.0, 2000.0, 600.0, 4, 10), 9);
        assert_eq!(tile_index_at(-50.0, -50.0, 600.0, 4, 10), 0);
    }
}

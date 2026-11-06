use crate::app::{App, Confirm};
use crate::theme::{self, ACCENT, ACCENT_INK, DANGER, FAINT, GOOD, INK, LINE, MUTED, SURFACE};
use crate::widgets::{self, Kind};
use crate::win;
use eframe::egui::{self, Align, Color32, CornerRadius, FontId, Id, Layout, Margin, Rect, RichText, Sense, Stroke, UiBuilder, Vec2};

const SIDE_WIDTH: f32 = 344.0;
const COLUMN_GAP: f32 = 16.0;

impl App {
    fn header(ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (mark, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
            ui.painter().rect_filled(mark, CornerRadius::same(9), ACCENT);
            theme::centered_text(ui.painter(), mark.center(), "\u{E716}", FontId::new(16.0, theme::icons()), ACCENT_INK);
            ui.add_space(6.0);
            ui.label(RichText::new(win::TITLE).family(theme::semibold()).size(16.0));
            ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(12.0).color(FAINT));
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        let (message, ok) = &self.status;
        let color = match ok {
            Some(true) => GOOD,
            Some(false) => DANGER,
            None => FAINT,
        };
        ui.horizontal_centered(|ui| {
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.add_space(4.0);
            ui.label(RichText::new(message).size(13.0).color(MUTED));
        });
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(confirm) = &self.confirm else {
            return;
        };
        let (title, body, action) = match confirm {
            Confirm::RemoveAccount(id) => {
                let name = self.store.accounts.iter().find(|account| &account.id == id).map_or(id.as_str(), |account| account.name.as_str());
                (format!("Remove {name}?"), "This only removes it from welffi accounts. You can add it back by logging in.", "Remove")
            }
            Confirm::DeleteServer(name) => (format!("Delete {name}?"), "The saved link is removed from this list.", "Delete"),
        };
        let frame = egui::Frame::NONE.fill(SURFACE).stroke(Stroke::new(1.0, LINE)).corner_radius(CornerRadius::same(16)).inner_margin(Margin::same(22));
        let mut answer = None;
        let modal = egui::Modal::new(Id::unique("confirm")).frame(frame).show(ctx, |ui| {
            ui.set_width(336.0);
            widgets::title(ui, &title);
            ui.add_space(4.0);
            ui.label(RichText::new(body).size(13.0).color(MUTED));
            ui.add_space(18.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, action, Kind::Accent, true, 0.0).clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "Cancel", Kind::Plain, true, 0.0).clicked() {
                    answer = Some(false);
                }
            });
        });
        if modal.should_close() && answer.is_none() {
            answer = Some(false);
        }
        match (answer, self.confirm.take()) {
            (Some(true), Some(Confirm::RemoveAccount(id))) => self.remove_account(&id),
            (Some(true), Some(Confirm::DeleteServer(name))) => self.delete_server(&name),
            (Some(_), _) => {}
            (None, pending) => self.confirm = pending,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let ctx = ui.ctx().clone();
        self.capture_keys(&ctx);
        egui::Panel::bottom("status")
            .frame(egui::Frame::NONE.fill(SURFACE).inner_margin(Margin::symmetric(28, 0)).stroke(Stroke::new(1.0, LINE)))
            .exact_size(36.0)
            .show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(INK)).show(ui, |ui| {
            let panel = ui.max_rect();
            theme::glow(ui.painter(), panel.left_top() + Vec2::new(160.0, 60.0), 520.0, Color32::from_rgba_unmultiplied(0xF2, 0xB5, 0x44, 16));
            egui::Frame::NONE.inner_margin(Margin::symmetric(28, 22)).show(ui, |ui| {
                Self::header(ui);
                ui.add_space(18.0);
                let area = ui.available_rect_before_wrap();
                let side = Rect::from_min_max(egui::pos2(area.right() - SIDE_WIDTH, area.top()), area.max);
                let main = Rect::from_min_max(area.min, egui::pos2(side.left() - COLUMN_GAP, area.bottom()));
                ui.scope_builder(UiBuilder::new().max_rect(main), |ui| self.accounts_card(ui));
                ui.scope_builder(UiBuilder::new().max_rect(side), |ui| {
                    egui::ScrollArea::vertical().id_salt("side").auto_shrink([false, true]).show(ui, |ui| self.side(ui));
                });
            });
        });
        self.confirm_dialog(&ctx);
    }
}
